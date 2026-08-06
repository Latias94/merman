#!/usr/bin/env python3
"""Capture and compare exact stripped native FFI artifact sizes."""

from __future__ import annotations

import argparse
from collections.abc import Callable, Mapping, Sequence
from dataclasses import dataclass, replace
import json
import math
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tomllib
from typing import Any

from artifact_profile_recipe import (
    REPO_ROOT,
    CargoArtifactRecipe,
    cargo_build_args,
    load_artifact_profile,
)
from ffi_contract_dependency_probes import BASELINE_COMMIT
from ffi_contract_baseline_contract import (
    DEFAULT_BASELINE_LOCK,
    FINGERPRINT_RE,
    NATIVE_ARTIFACT_INPUT_PATHS,
    FfiBaselineContractError,
    file_sha256,
    input_records,
    load_baseline_lock,
    rust_toolchain_native_compatibility_projection,
    source_revision_projection,
    validate_input_records,
    validate_finalized_sha256,
    validate_rust_toolchain,
    validate_source_revision,
)
from ffi_contract_reproducibility import (
    FfiContractReproducibilityError,
    ffi_contract_subprocess_environment,
    reject_cargo_configuration as reject_ffi_contract_cargo_configuration,
    reject_ffi_contract_environment,
    rust_toolchain_provenance,
)
from strict_json import StrictJsonContract, bytes_sha256, canonical_sha256


REPORT_ID = "merman-ffi-contract-native-artifact-baseline"
SCHEMA_VERSION = 3
REJECTED_BASELINE_FILE_SHA256 = frozenset(
    {"sha256:c916d88a719834379773923fb225304bfb8d2905f8db14a98c142c44c7d88032"}
)
SEMANTIC_PERCENT = 0.01
SEMANTIC_FLOOR_BYTES = 64 * 1024
FULL_PERCENT = 0.02
FULL_FLOOR_BYTES = 512 * 1024
DEFAULT_NATIVE_SIZE_APPROVALS = (
    REPO_ROOT / "scripts" / "ffi_contract_native_size_approvals.json"
)
APPROVAL_REGISTRY_ID = "merman-ffi-contract-native-size-approvals"
REJECTED_APPROVAL_REASONS = frozenset(
    {"Explain the reviewed source of this full-artifact growth."}
)


class NativeArtifactSizeError(RuntimeError):
    """The native artifact measurement or comparison contract failed."""


STRICT_JSON = StrictJsonContract(
    error_factory=NativeArtifactSizeError,
    read_error_prefix="cannot read native artifact baseline",
)
STRICT_APPROVAL_JSON = StrictJsonContract(
    error_factory=NativeArtifactSizeError,
    read_error_prefix="cannot read native artifact size approvals",
)
ProcessRunner = Callable[
    [Sequence[str], Path],
    subprocess.CompletedProcess[str],
]


@dataclass(frozen=True)
class NativeArtifactProfile:
    label: str
    budget_class: str
    recipe: CargoArtifactRecipe

    def projection(self, host_target: str) -> dict[str, Any]:
        return {
            "label": self.label,
            "budget_class": self.budget_class,
            "target": host_target,
            "cargo": {
                "package": self.recipe.package,
                "manifest": self.recipe.manifest,
                "profile": self.recipe.cargo_profile,
                "default_features": self.recipe.default_features,
                "features": list(self.recipe.features),
                "target_name": self.recipe.target_name,
                "crate_types": ["cdylib", "staticlib"],
                "build_target_kind": self.recipe.build_target_kind,
            },
        }


@dataclass(frozen=True)
class ToolIdentity:
    path: str
    sha256: str

    def projection(self) -> dict[str, str]:
        return {"path": self.path, "sha256": self.sha256}


@dataclass(frozen=True)
class NativeBuildBindings:
    cargo: str
    rustc: str
    linker_driver: str
    developer_dir: str
    sdk: str


@dataclass(frozen=True)
class NativeArtifactSizeApproval:
    profile: str
    artifact_kind: str
    baseline_report_sha256: str
    baseline_stripped_sha256: str
    current_stripped_sha256: str
    baseline_size_bytes: int
    current_size_bytes: int
    budget_bytes: int
    reason: str

    @property
    def key(self) -> tuple[str, str]:
        return self.profile, self.artifact_kind

    def projection(self) -> dict[str, Any]:
        return {
            "profile": self.profile,
            "artifact_kind": self.artifact_kind,
            "baseline_report_sha256": self.baseline_report_sha256,
            "baseline_stripped_sha256": self.baseline_stripped_sha256,
            "current_stripped_sha256": self.current_stripped_sha256,
            "baseline_size_bytes": self.baseline_size_bytes,
            "current_size_bytes": self.current_size_bytes,
            "budget_bytes": self.budget_bytes,
            "reason": self.reason,
        }


@dataclass(frozen=True)
class NativeArtifactSizeDelta:
    profile: str
    budget_class: str
    artifact_kind: str
    baseline_size_bytes: int
    current_size_bytes: int
    delta_bytes: int
    budget_bytes: int
    status: str


@dataclass(frozen=True)
class NativeArtifactComparison:
    deltas: tuple[NativeArtifactSizeDelta, ...]
    failures: tuple[str, ...]


@dataclass(frozen=True)
class AppleToolchain:
    developer_dir: str
    xcode_version: str
    sdk_path: str
    sdk_version: str
    sdk_settings: ToolIdentity
    deployment_target: str
    linker_driver: ToolIdentity
    linker: ToolIdentity
    strip: ToolIdentity

    def projection(self) -> dict[str, Any]:
        return {
            "developer_dir": self.developer_dir,
            "xcode_version": self.xcode_version,
            "sdk": {
                "path": self.sdk_path,
                "version": self.sdk_version,
                "settings": self.sdk_settings.projection(),
            },
            "deployment_target": self.deployment_target,
            "linker_driver": self.linker_driver.projection(),
            "linker": self.linker.projection(),
            "strip": self.strip.projection(),
        }


def load_native_artifact_profiles(
    repo_root: Path = REPO_ROOT,
) -> tuple[NativeArtifactProfile, ...]:
    descriptor = repo_root / "capabilities" / "artifact-profiles-v1.json"
    full = load_artifact_profile("c-abi-native", descriptor)
    profiles = (
        NativeArtifactProfile(
            "ffi-full-native",
            "full",
            replace(full, profile_id="ffi-full-native"),
        ),
        NativeArtifactProfile(
            "ffi-semantic",
            "semantic",
            replace(full, profile_id="ffi-semantic", features=()),
        ),
    )
    if tuple(profile.label for profile in profiles) != tuple(
        sorted(profile.label for profile in profiles)
    ):
        raise NativeArtifactSizeError("native artifact profile labels must be sorted")
    return profiles


def load_native_artifact_size_approvals(
    path: Path = DEFAULT_NATIVE_SIZE_APPROVALS,
) -> tuple[NativeArtifactSizeApproval, ...]:
    document = STRICT_APPROVAL_JSON.object(
        STRICT_APPROVAL_JSON.load(path),
        "native artifact size approval registry",
    )
    STRICT_APPROVAL_JSON.exact_fields(
        document,
        {"schema_version", "registry_id", "approvals"},
        "native artifact size approval registry",
    )
    if document.get("schema_version") != 1:
        raise NativeArtifactSizeError(
            "native artifact size approval schema_version must be 1"
        )
    if document.get("registry_id") != APPROVAL_REGISTRY_ID:
        raise NativeArtifactSizeError(
            "native artifact size approval registry_id is invalid"
        )
    raw_approvals = STRICT_APPROVAL_JSON.array(
        document.get("approvals"),
        "native artifact size approvals",
    )
    approvals = tuple(
        _parse_native_artifact_size_approval(raw, index)
        for index, raw in enumerate(raw_approvals)
    )
    keys = tuple(approval.key for approval in approvals)
    if keys != tuple(sorted(set(keys))):
        raise NativeArtifactSizeError(
            "native artifact size approvals must be sorted and unique"
        )
    return approvals


def _parse_native_artifact_size_approval(
    value: Any,
    index: int,
) -> NativeArtifactSizeApproval:
    context = f"native artifact size approval[{index}]"
    record = STRICT_APPROVAL_JSON.object(value, context)
    STRICT_APPROVAL_JSON.exact_fields(
        record,
        {
            "profile",
            "artifact_kind",
            "baseline_report_sha256",
            "baseline_stripped_sha256",
            "current_stripped_sha256",
            "baseline_size_bytes",
            "current_size_bytes",
            "budget_bytes",
            "reason",
        },
        context,
    )
    profile = STRICT_APPROVAL_JSON.string(record.get("profile"), f"{context}.profile")
    artifact_kind = STRICT_APPROVAL_JSON.string(
        record.get("artifact_kind"),
        f"{context}.artifact_kind",
    )
    if profile != "ffi-full-native":
        raise NativeArtifactSizeError(
            f"{context} may approve only the full native artifact profile"
        )
    if artifact_kind not in {"cdylib", "staticlib"}:
        raise NativeArtifactSizeError(f"{context} artifact_kind is invalid")
    digests = {}
    for field in (
        "baseline_report_sha256",
        "baseline_stripped_sha256",
        "current_stripped_sha256",
    ):
        try:
            digests[field] = validate_finalized_sha256(
                record.get(field),
                f"{context}.{field}",
            )
        except FfiBaselineContractError as error:
            raise NativeArtifactSizeError(str(error)) from error
    sizes = {}
    for field in (
        "baseline_size_bytes",
        "current_size_bytes",
        "budget_bytes",
    ):
        raw = record.get(field)
        if type(raw) is not int or raw <= 0:
            raise NativeArtifactSizeError(f"{context}.{field} must be a positive integer")
        sizes[field] = raw
    if sizes["budget_bytes"] != artifact_growth_budget(
        sizes["baseline_size_bytes"],
        "full",
    ):
        raise NativeArtifactSizeError(f"{context}.budget_bytes is stale")
    if sizes["current_size_bytes"] <= (
        sizes["baseline_size_bytes"] + sizes["budget_bytes"]
    ):
        raise NativeArtifactSizeError(
            f"{context} does not describe an over-budget artifact"
        )
    reason = STRICT_APPROVAL_JSON.string(record.get("reason"), f"{context}.reason")
    if (
        reason != reason.strip()
        or "\n" in reason
        or "\r" in reason
        or len(reason) > 512
        or reason in REJECTED_APPROVAL_REASONS
    ):
        raise NativeArtifactSizeError(
            f"{context}.reason must be a reviewed, trimmed single line of at most "
            "512 characters"
        )
    return NativeArtifactSizeApproval(
        profile=profile,
        artifact_kind=artifact_kind,
        baseline_report_sha256=digests["baseline_report_sha256"],
        baseline_stripped_sha256=digests["baseline_stripped_sha256"],
        current_stripped_sha256=digests["current_stripped_sha256"],
        baseline_size_bytes=sizes["baseline_size_bytes"],
        current_size_bytes=sizes["current_size_bytes"],
        budget_bytes=sizes["budget_bytes"],
        reason=reason,
    )


def native_profile_configuration(repo_root: Path = REPO_ROOT) -> dict[str, Any]:
    """Return the explicit native SDK profile and its inherited profile overrides."""
    manifest = repo_root / "Cargo.toml"
    if manifest.is_symlink() or not manifest.is_file():
        raise NativeArtifactSizeError(
            f"workspace manifest must be a regular non-symlink file: {manifest}"
        )
    try:
        with manifest.open("rb") as source:
            document = tomllib.load(source)
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise NativeArtifactSizeError(
            f"cannot read workspace Cargo profiles from {manifest}: {error}"
        ) from error
    profiles = document.get("profile")
    if not isinstance(profiles, dict):
        raise NativeArtifactSizeError("workspace manifest has no Cargo profiles")
    native = profiles.get("native-sdk")
    if not isinstance(native, dict):
        raise NativeArtifactSizeError("workspace manifest has no native-sdk profile")
    inherits = native.get("inherits")
    if not isinstance(inherits, str) or not inherits:
        raise NativeArtifactSizeError("native-sdk profile must name an inherited profile")
    inherited = profiles.get(inherits, {})
    if not isinstance(inherited, dict):
        raise NativeArtifactSizeError(
            f"inherited Cargo profile {inherits!r} must be a table"
        )
    return {
        "native-sdk": _canonical_profile_table(native, "native-sdk"),
        "inherited": {
            "name": inherits,
            "overrides": _canonical_profile_table(inherited, inherits),
        },
    }


def _canonical_profile_table(value: Mapping[str, Any], label: str) -> dict[str, Any]:
    normalized: dict[str, Any] = {}
    for key, item in sorted(value.items()):
        if not isinstance(key, str) or not key:
            raise NativeArtifactSizeError(f"Cargo profile {label!r} has an invalid key")
        if not isinstance(item, (bool, int, str)):
            raise NativeArtifactSizeError(
                f"Cargo profile {label!r} field {key!r} has an unsupported value"
            )
        normalized[key] = item
    return normalized


def native_build_command(
    profile: NativeArtifactProfile,
    *,
    repo_root: Path,
    target_dir: Path,
    target: str,
    bindings: NativeBuildBindings,
) -> list[str]:
    recipe = profile.recipe
    selected_target = target if recipe.build_target_kind == "target-set" else None
    command = cargo_build_args(
        recipe,
        locked=True,
        target=selected_target,
        repo_root=repo_root,
    )
    command[0] = bindings.cargo
    command.extend(
        (
            "--config",
            f"build.rustc={json.dumps(bindings.rustc)}",
            "--config",
            "build.incremental=false",
            "--config",
            f"target.{target}.linker={json.dumps(bindings.linker_driver)}",
            "--config",
            _target_sdk_rustflags(target, bindings.sdk),
            "--config",
            _cargo_environment("DEVELOPER_DIR", bindings.developer_dir),
            "--config",
            _cargo_environment("SDKROOT", bindings.sdk),
            "--config",
            _cargo_environment("ZERO_AR_DATE", "1"),
            "--message-format",
            "json-render-diagnostics",
            "--target-dir",
            str(target_dir),
        )
    )
    return command


def _target_sdk_rustflags(target: str, sdk_path: str) -> str:
    flags = ["-C", "link-arg=-isysroot", "-C", f"link-arg={sdk_path}"]
    return f"target.{target}.rustflags={json.dumps(flags, separators=(',', ':'))}"


def _cargo_environment(name: str, value: str) -> str:
    return f"env.{name}={json.dumps(value)}"


def capture_native_artifact_measurements(
    profiles: Sequence[NativeArtifactProfile],
    *,
    repo_root: Path,
    output_root: Path,
    rust_toolchain: Mapping[str, Any],
    runner: ProcessRunner,
) -> tuple[dict[str, Any], ...]:
    try:
        validated_toolchain = validate_rust_toolchain(rust_toolchain)
    except FfiBaselineContractError as error:
        raise NativeArtifactSizeError(str(error)) from error
    host_target = validated_toolchain["host_target"]
    if not host_target.endswith("-apple-darwin"):
        raise NativeArtifactSizeError(
            "native artifact size evidence currently supports exact Apple strip recipes only"
        )
    reject_native_measurement_environment()
    reject_cargo_configuration(repo_root)
    apple_toolchain = _resolve_apple_toolchain(
        host_target,
        validated_toolchain["rustc"]["path"],
        repo_root,
        runner,
    )
    actual_bindings = NativeBuildBindings(
        cargo=validated_toolchain["cargo"]["path"],
        rustc=validated_toolchain["rustc"]["path"],
        linker_driver=apple_toolchain.linker_driver.path,
        developer_dir=apple_toolchain.developer_dir,
        sdk=apple_toolchain.sdk_path,
    )
    normalized_bindings = NativeBuildBindings(
        cargo="$CARGO",
        rustc="$RUSTC",
        linker_driver="$LINKER_DRIVER",
        developer_dir="$DEVELOPER_DIR",
        sdk="$SDK",
    )
    measurements = []
    for profile in profiles:
        target_dir = output_root / "build" / profile.label
        _prepare_empty_directory(target_dir, f"{profile.label} target directory")
        artifact_dir = output_root / "artifacts" / profile.label
        _prepare_empty_directory(artifact_dir, f"{profile.label} artifact directory")
        command = native_build_command(
            profile,
            repo_root=repo_root,
            target_dir=target_dir,
            target=host_target,
            bindings=actual_bindings,
        )
        completed = runner(command, repo_root)
        if completed.returncode != 0:
            detail = (completed.stderr or completed.stdout or "").strip()
            raise NativeArtifactSizeError(
                f"native artifact build failed for {profile.label}: {detail}"
            )
        artifacts = select_compiler_artifacts(
            completed.stdout,
            profile=profile,
            repo_root=repo_root,
            target_dir=target_dir,
            host_target=host_target,
        )
        rows = []
        for artifact_kind, source in artifacts:
            rows.append(
                _measure_artifact(
                    profile,
                    artifact_kind,
                    source,
                    repo_root=repo_root,
                    output_root=output_root,
                    strip_tool=apple_toolchain.strip,
                    runner=runner,
                )
            )
        measurements.append(
            {
                "profile": profile.projection(host_target),
                "build": {
                    "command": native_build_command(
                        profile,
                        repo_root=Path("$REPO"),
                        target_dir=Path(f"$EVIDENCE/build/{profile.label}"),
                        target=host_target,
                        bindings=normalized_bindings,
                    ),
                    "target_dir": f"$EVIDENCE/build/{profile.label}",
                    "target": host_target,
                    "cargo_message_format": "json-render-diagnostics",
                    "artifact_selection": "current compiler-artifact event",
                    "apple_toolchain": apple_toolchain.projection(),
                },
                "artifacts": rows,
            }
        )
    after = _resolve_apple_toolchain(
        host_target,
        validated_toolchain["rustc"]["path"],
        repo_root,
        runner,
    )
    if after != apple_toolchain:
        raise NativeArtifactSizeError(
            "Apple toolchain or SDK identity changed during native artifact capture"
        )
    return tuple(measurements)


def reject_native_measurement_environment(
    environment: Mapping[str, str] | None = None,
) -> None:
    try:
        reject_ffi_contract_environment(environment)
    except FfiContractReproducibilityError as error:
        raise NativeArtifactSizeError(str(error)) from error


def reject_cargo_configuration(
    repo_root: Path,
    *,
    environment: Mapping[str, str] | None = None,
    user_home: Path | None = None,
) -> None:
    try:
        reject_ffi_contract_cargo_configuration(
            repo_root,
            environment=environment,
            user_home=user_home,
        )
    except FfiContractReproducibilityError as error:
        raise NativeArtifactSizeError(str(error)) from error


def _prepare_empty_directory(path: Path, label: str) -> None:
    if path.is_symlink():
        raise NativeArtifactSizeError(f"{label} must not be a symlink: {path}")
    if path.exists():
        if not path.is_dir():
            raise NativeArtifactSizeError(f"{label} must be a real directory: {path}")
        try:
            if next(path.iterdir(), None) is not None:
                raise NativeArtifactSizeError(f"{label} must be empty: {path}")
        except OSError as error:
            raise NativeArtifactSizeError(f"cannot inspect {label}: {error}") from error
    path.mkdir(parents=True, exist_ok=True)


def _resolve_apple_toolchain(
    host_target: str,
    rustc_path: str,
    repo_root: Path,
    runner: ProcessRunner,
) -> AppleToolchain:
    xcrun = "/usr/bin/xcrun"
    developer_dir = _checked_process_output(
        ("/usr/bin/xcode-select", "--print-path"),
        repo_root,
        runner,
    )
    developer_path = Path(developer_dir)
    if (
        not developer_path.is_absolute()
        or developer_path.is_symlink()
        or not developer_path.is_dir()
        or developer_path.resolve() != developer_path
    ):
        raise NativeArtifactSizeError("xcode-select returned an invalid developer dir")
    xcode_version = _checked_process_output(
        ("/usr/bin/xcodebuild", "-version"),
        repo_root,
        runner,
    )
    sdk_path = _checked_process_output(
        (xcrun, "--sdk", "macosx", "--show-sdk-path"),
        repo_root,
        runner,
    )
    sdk_version = _checked_process_output(
        (xcrun, "--sdk", "macosx", "--show-sdk-version"),
        repo_root,
        runner,
    )
    deployment_target = _checked_process_output(
        (rustc_path, "--print", "deployment-target", "--target", host_target),
        repo_root,
        runner,
    )
    linker_driver = _resolve_xcode_tool("clang", developer_path, repo_root, runner)
    linker = _resolve_xcode_tool("ld", developer_path, repo_root, runner)
    strip = _resolve_xcode_tool("strip", developer_path, repo_root, runner)
    driver_linker = _checked_process_output(
        (linker_driver.path, "-print-prog-name=ld"),
        repo_root,
        runner,
    )
    if Path(driver_linker) != Path(linker.path):
        raise NativeArtifactSizeError(
            "the selected clang driver resolves a different linker than xcrun"
        )
    sdk = Path(sdk_path)
    resolved_sdk = sdk.resolve()
    if (
        not sdk.is_absolute()
        or not resolved_sdk.is_dir()
        or not resolved_sdk.is_relative_to(developer_path.resolve())
    ):
        raise NativeArtifactSizeError("xcrun returned an invalid macOS SDK path")
    sdk_settings_path = resolved_sdk / "SDKSettings.json"
    try:
        sdk_settings = ToolIdentity(
            str(sdk_settings_path),
            file_sha256(sdk_settings_path),
        )
    except FfiBaselineContractError as error:
        raise NativeArtifactSizeError(str(error)) from error
    return AppleToolchain(
        developer_dir=developer_dir,
        xcode_version=xcode_version,
        sdk_path=str(resolved_sdk),
        sdk_version=sdk_version,
        sdk_settings=sdk_settings,
        deployment_target=deployment_target,
        linker_driver=linker_driver,
        linker=linker,
        strip=strip,
    )


def _resolve_xcode_tool(
    name: str,
    developer_dir: Path,
    repo_root: Path,
    runner: ProcessRunner,
) -> ToolIdentity:
    tool = _checked_process_output(
        ("/usr/bin/xcrun", "--sdk", "macosx", "--find", name),
        repo_root,
        runner,
    )
    path = Path(tool)
    if (
        not path.is_absolute()
        or path.name != name
        or path.is_symlink()
        or not path.is_file()
        or path.resolve() != path
        or not os.access(path, os.X_OK)
        or not path.is_relative_to(developer_dir)
    ):
        raise NativeArtifactSizeError(
            f"xcrun --find {name} returned a non-canonical Xcode tool"
        )
    try:
        digest = file_sha256(path)
    except FfiBaselineContractError as error:
        raise NativeArtifactSizeError(str(error)) from error
    return ToolIdentity(tool, digest)


def _checked_process_output(
    command: Sequence[str],
    cwd: Path,
    runner: ProcessRunner,
) -> str:
    completed = runner(command, cwd)
    if completed.returncode != 0:
        detail = (completed.stderr or completed.stdout or "").strip()
        raise NativeArtifactSizeError(
            f"command failed with {completed.returncode}: {' '.join(command)}: {detail}"
        )
    if not isinstance(completed.stdout, str) or not completed.stdout.strip():
        raise NativeArtifactSizeError(
            f"command produced no text: {' '.join(command)}"
        )
    return completed.stdout.strip()


def select_compiler_artifacts(
    output: str,
    *,
    profile: NativeArtifactProfile,
    repo_root: Path,
    target_dir: Path,
    host_target: str,
) -> tuple[tuple[str, Path], ...]:
    expected_manifest = (repo_root / profile.recipe.manifest).resolve()
    matching_events: list[dict[str, Any]] = []
    for line_number, line in enumerate(output.splitlines(), start=1):
        if not line.startswith("{"):
            continue
        try:
            event = json.loads(line)
        except json.JSONDecodeError as error:
            raise NativeArtifactSizeError(
                f"Cargo JSON line {line_number} is malformed: {error}"
            ) from error
        if not isinstance(event, dict) or event.get("reason") != "compiler-artifact":
            continue
        target = event.get("target")
        if not isinstance(target, dict) or target.get("name") != profile.recipe.target_name:
            continue
        manifest_path = event.get("manifest_path")
        if not isinstance(manifest_path, str):
            continue
        manifest = Path(manifest_path)
        if not manifest.is_absolute() or manifest.resolve() != expected_manifest:
            continue
        matching_events.append(event)

    if len(matching_events) != 1:
        raise NativeArtifactSizeError(
            f"expected exactly one root compiler-artifact event for {profile.label}; "
            f"found {len(matching_events)}"
        )
    event = matching_events[0]
    _validate_root_compiler_artifact_event(
        event,
        profile=profile,
        repo_root=repo_root,
    )
    filenames = event["filenames"]
    candidates: dict[str, set[Path]] = {"cdylib": set(), "staticlib": set()}
    for raw_path in filenames:
        if not isinstance(raw_path, str):
            raise NativeArtifactSizeError("compiler-artifact filename must be text")
        path = Path(raw_path)
        if not path.is_absolute():
            raise NativeArtifactSizeError(
                "compiler-artifact filename must be an absolute path"
            )
        if path.name == f"lib{profile.recipe.target_name}.dylib":
            candidates["cdylib"].add(path)
        elif path.name == f"lib{profile.recipe.target_name}.a":
            candidates["staticlib"].add(path)

    selected = []
    resolved_target_dir = native_artifact_output_directory(
        profile,
        target_dir=target_dir,
        host_target=host_target,
    ).resolve()
    for artifact_kind in ("cdylib", "staticlib"):
        paths = candidates[artifact_kind]
        if len(paths) != 1:
            raise NativeArtifactSizeError(
                f"expected exactly one {artifact_kind} compiler artifact for "
                f"{profile.label}; found {len(paths)}"
            )
        path = next(iter(paths))
        resolved = path.resolve()
        if not resolved.is_relative_to(resolved_target_dir):
            raise NativeArtifactSizeError(
                f"compiler artifact escaped the exact target directory: {path}"
            )
        if path.is_symlink() or not path.is_file():
            raise NativeArtifactSizeError(
                f"compiler artifact must be a regular non-symlink file: {path}"
            )
        selected.append((artifact_kind, path))
    return tuple(selected)


def native_artifact_output_directory(
    profile: NativeArtifactProfile,
    *,
    target_dir: Path,
    host_target: str,
) -> Path:
    if profile.recipe.build_target_kind == "host":
        return target_dir / profile.recipe.cargo_profile
    if profile.recipe.build_target_kind == "target-set":
        return target_dir / host_target / profile.recipe.cargo_profile
    raise NativeArtifactSizeError(
        f"unsupported native artifact build target kind: "
        f"{profile.recipe.build_target_kind}"
    )


def _validate_root_compiler_artifact_event(
    event: Mapping[str, Any],
    *,
    profile: NativeArtifactProfile,
    repo_root: Path,
) -> None:
    if event.get("fresh") is not False:
        raise NativeArtifactSizeError(
            "root compiler-artifact must come from a fresh clean build"
        )
    package_id = event.get("package_id")
    if not isinstance(package_id, str) or not _matches_workspace_package_id(
        package_id,
        profile.recipe,
        repo_root,
    ):
        raise NativeArtifactSizeError("root compiler-artifact package_id is invalid")
    target = event.get("target")
    if not isinstance(target, dict):
        raise NativeArtifactSizeError("root compiler-artifact target is invalid")
    expected_source = (repo_root / profile.recipe.manifest).parent / "src" / "lib.rs"
    source_path = target.get("src_path")
    if (
        not isinstance(source_path, str)
        or not Path(source_path).is_absolute()
        or Path(source_path).resolve() != expected_source.resolve()
    ):
        raise NativeArtifactSizeError("root compiler-artifact source path is invalid")
    for field, expected in (
        ("kind", profile.recipe.target_kinds),
        ("crate_types", profile.recipe.crate_types),
    ):
        value = target.get(field)
        if (
            not isinstance(value, list)
            or not all(isinstance(item, str) for item in value)
            or tuple(sorted(value)) != tuple(sorted(expected))
        ):
            raise NativeArtifactSizeError(
                f"root compiler-artifact target {field} is invalid"
            )
    required_features = target.get("required-features", [])
    if (
        not isinstance(required_features, list)
        or not all(isinstance(item, str) for item in required_features)
        or tuple(sorted(required_features))
        != tuple(sorted(profile.recipe.required_features))
    ):
        raise NativeArtifactSizeError(
            "root compiler-artifact target required-features is invalid"
        )
    filenames = event.get("filenames")
    if not isinstance(filenames, list) or not filenames:
        raise NativeArtifactSizeError("matching compiler-artifact has no filenames")
    cargo_profile = event.get("profile")
    if not isinstance(cargo_profile, dict) or cargo_profile.get("test") is not False:
        raise NativeArtifactSizeError("root compiler-artifact Cargo profile is invalid")
    configuration = native_profile_configuration(repo_root)["native-sdk"]
    expected_profile_values = {
        "opt_level": str(configuration["opt-level"]),
        "debug_assertions": configuration["debug-assertions"],
        "overflow_checks": configuration["overflow-checks"],
    }
    for field, expected in expected_profile_values.items():
        if cargo_profile.get(field) != expected:
            raise NativeArtifactSizeError(
                f"root compiler-artifact Cargo profile {field} drifted"
            )


def _matches_workspace_package_id(
    package_id: str,
    recipe: CargoArtifactRecipe,
    repo_root: Path,
) -> bool:
    manifest = repo_root / recipe.manifest
    for expected_prefix in (
        f"path+{manifest.parent.absolute().as_uri()}#",
        f"path+{manifest.parent.resolve().as_uri()}#",
    ):
        if package_id.startswith(expected_prefix):
            version = _workspace_package_version(manifest, repo_root / "Cargo.toml")
            fragment = package_id.removeprefix(expected_prefix)
            return fragment in {version, f"{recipe.package}@{version}"}
    return False


def _workspace_package_version(manifest: Path, workspace_manifest: Path) -> str:
    try:
        with manifest.open("rb") as source:
            package_document = tomllib.load(source)
        with workspace_manifest.open("rb") as source:
            workspace_document = tomllib.load(source)
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise NativeArtifactSizeError(
            f"cannot read workspace package version: {error}"
        ) from error
    package = package_document.get("package")
    if not isinstance(package, dict):
        raise NativeArtifactSizeError("native artifact package manifest has no package table")
    version = package.get("version")
    if isinstance(version, str) and version:
        return version
    if version == {"workspace": True}:
        workspace = workspace_document.get("workspace")
        workspace_package = workspace.get("package") if isinstance(workspace, dict) else None
        inherited = (
            workspace_package.get("version")
            if isinstance(workspace_package, dict)
            else None
        )
        if isinstance(inherited, str) and inherited:
            return inherited
    raise NativeArtifactSizeError("native artifact package version is not canonical")


def _measure_artifact(
    profile: NativeArtifactProfile,
    artifact_kind: str,
    source: Path,
    *,
    repo_root: Path,
    output_root: Path,
    strip_tool: ToolIdentity,
    runner: ProcessRunner,
) -> dict[str, Any]:
    destination_dir = output_root / "artifacts" / profile.label
    raw_dir = destination_dir / "raw"
    stripped_dir = destination_dir / "stripped"
    raw_dir.mkdir(parents=True, exist_ok=True)
    stripped_dir.mkdir(parents=True, exist_ok=True)
    raw_copy = raw_dir / source.name
    stripped_copy = stripped_dir / source.name
    if raw_copy.exists() or stripped_copy.exists():
        raise NativeArtifactSizeError(
            f"artifact evidence destination already exists for {profile.label}"
        )
    shutil.copy2(source, raw_copy)
    shutil.copy2(source, stripped_copy)
    raw_size = raw_copy.stat().st_size
    raw_sha256 = _native_file_sha256(raw_copy)
    strip_command = [strip_tool.path, "-x", str(stripped_copy)]
    completed = runner(strip_command, repo_root)
    if completed.returncode != 0:
        detail = (completed.stderr or completed.stdout or "").strip()
        raise NativeArtifactSizeError(
            f"strip failed for {profile.label}/{artifact_kind}: {detail}"
        )
    if stripped_copy.is_symlink() or not stripped_copy.is_file():
        raise NativeArtifactSizeError("strip did not leave a regular artifact")
    return {
        "artifact_kind": artifact_kind,
        "file_name": source.name,
        "raw_size_bytes": raw_size,
        "raw_sha256": raw_sha256,
        "stripped_size_bytes": stripped_copy.stat().st_size,
        "stripped_sha256": _native_file_sha256(stripped_copy),
        "strip": {
            "recipe_id": "apple-strip-local-symbols-v1",
            "tool": strip_tool.path,
            "tool_sha256": strip_tool.sha256,
            "command": ["$STRIP", "-x", "$ARTIFACT"],
        },
    }


def _native_file_sha256(path: Path) -> str:
    try:
        return file_sha256(path)
    except FfiBaselineContractError as error:
        raise NativeArtifactSizeError(str(error)) from error


def native_artifact_report(
    measurements: Sequence[dict[str, Any]],
    *,
    repo_root: Path,
    toolchain: Mapping[str, Any],
    source_snapshot_sha256: str,
) -> dict[str, Any]:
    try:
        validated_toolchain = validate_rust_toolchain(toolchain)
    except FfiBaselineContractError as error:
        raise NativeArtifactSizeError(str(error)) from error
    report: dict[str, Any] = {
        "schema_version": SCHEMA_VERSION,
        "report_id": REPORT_ID,
        "baseline_commit": BASELINE_COMMIT,
        "source_revision": source_revision_projection(source_snapshot_sha256),
        "inputs": input_records(repo_root, NATIVE_ARTIFACT_INPUT_PATHS),
        "toolchain": dict(validated_toolchain),
        "measurement_boundary": {
            "cargo_profile": "native-sdk",
            "cargo_profile_strip": "debuginfo",
            "cargo_profile_configuration": native_profile_configuration(repo_root),
            "secondary_strip_recipe": "apple-strip-local-symbols-v1",
            "size_unit": "bytes",
            "timing_is_gating": False,
        },
        "profiles": list(measurements),
    }
    report["report_sha256"] = embedded_report_sha256(report)
    return report


def embedded_report_sha256(report: Mapping[str, Any]) -> str:
    unsigned = dict(report)
    unsigned.pop("report_sha256", None)
    return f"sha256:{canonical_sha256(unsigned)}"


def load_native_artifact_baseline(
    path: Path,
    *,
    lock_path: Path = DEFAULT_BASELINE_LOCK,
    repo_root: Path = REPO_ROOT,
) -> dict[str, Any]:
    raw, parsed = STRICT_JSON.load_bytes(path)
    file_sha256 = bytes_sha256(raw)
    if file_sha256 in REJECTED_BASELINE_FILE_SHA256:
        raise NativeArtifactSizeError(
            "native artifact baseline is the rejected incremental shared-target report"
        )
    report = STRICT_JSON.object(parsed, "native artifact baseline")
    validate_native_artifact_report(report, repo_root=repo_root)
    try:
        lock = load_baseline_lock(lock_path)
    except FfiBaselineContractError as error:
        raise NativeArtifactSizeError(str(error)) from error
    if lock["native_artifact_report_schema_version"] != SCHEMA_VERSION:
        raise NativeArtifactSizeError("native artifact baseline lock schema drifted")
    if (
        report["source_revision"]["snapshot_sha256"]
        != lock["source_snapshot_sha256"]
    ):
        raise NativeArtifactSizeError(
            "native artifact baseline source snapshot does not match the lock"
        )
    expected = lock["native_artifact_report_file_sha256"]
    if file_sha256 != expected:
        raise NativeArtifactSizeError(
            "native artifact baseline whole-file digest does not match the lock"
        )
    locked_inputs = lock["baseline_input_sha256"]
    if any(
        locked_inputs.get(record["path"]) != record["sha256"]
        for record in report["inputs"]
    ):
        raise NativeArtifactSizeError(
            "native artifact baseline inputs do not match the checked-in source lock"
        )
    return report


def validate_native_artifact_report(
    report: dict[str, Any],
    *,
    repo_root: Path,
) -> None:
    STRICT_JSON.exact_fields(
        report,
        {
            "schema_version",
            "report_id",
            "baseline_commit",
            "source_revision",
            "inputs",
            "toolchain",
            "measurement_boundary",
            "profiles",
            "report_sha256",
        },
        "native artifact baseline",
    )
    if report.get("schema_version") != SCHEMA_VERSION:
        raise NativeArtifactSizeError(
            f"native artifact baseline schema_version must be {SCHEMA_VERSION}"
        )
    if report.get("report_id") != REPORT_ID:
        raise NativeArtifactSizeError("native artifact baseline report_id is invalid")
    if report.get("baseline_commit") != BASELINE_COMMIT:
        raise NativeArtifactSizeError("native artifact baseline commit is not canonical")
    try:
        validate_source_revision(report.get("source_revision"))
    except FfiBaselineContractError as error:
        raise NativeArtifactSizeError(str(error)) from error
    if report.get("report_sha256") != embedded_report_sha256(report):
        raise NativeArtifactSizeError("native artifact baseline embedded digest is stale")
    try:
        validate_input_records(
            report.get("inputs"),
            expected_paths=NATIVE_ARTIFACT_INPUT_PATHS,
        )
        validate_rust_toolchain(report.get("toolchain"))
    except FfiBaselineContractError as error:
        raise NativeArtifactSizeError(str(error)) from error
    boundary = STRICT_JSON.object(
        report.get("measurement_boundary"),
        "native artifact measurement boundary",
    )
    STRICT_JSON.exact_fields(
        boundary,
        {
            "cargo_profile",
            "cargo_profile_strip",
            "cargo_profile_configuration",
            "secondary_strip_recipe",
            "size_unit",
            "timing_is_gating",
        },
        "native artifact measurement boundary",
    )
    expected_boundary = {
        "cargo_profile": "native-sdk",
        "cargo_profile_strip": "debuginfo",
        "cargo_profile_configuration": native_profile_configuration(repo_root),
        "secondary_strip_recipe": "apple-strip-local-symbols-v1",
        "size_unit": "bytes",
        "timing_is_gating": False,
    }
    if boundary != expected_boundary:
        raise NativeArtifactSizeError("native artifact measurement boundary drifted")
    expected_profiles = load_native_artifact_profiles(repo_root)
    raw_profiles = STRICT_JSON.array(report.get("profiles"), "native artifact profiles")
    if len(raw_profiles) != len(expected_profiles):
        raise NativeArtifactSizeError("native artifact baseline profile count drifted")
    host_target = STRICT_JSON.object(report.get("toolchain"), "native toolchain").get(
        "host_target"
    )
    if not isinstance(host_target, str) or not host_target:
        raise NativeArtifactSizeError("native artifact baseline has no host target")
    labels = []
    for raw, expected in zip(raw_profiles, expected_profiles, strict=True):
        record = STRICT_JSON.object(raw, f"native profile {expected.label}")
        STRICT_JSON.exact_fields(
            record,
            {"profile", "build", "artifacts"},
            f"native profile {expected.label}",
        )
        if record.get("profile") != expected.projection(host_target):
            raise NativeArtifactSizeError(
                f"native artifact profile recipe drifted: {expected.label}"
            )
        labels.append(expected.label)
        apple_toolchain = validate_native_artifact_build_record(
            record.get("build"),
            expected,
            host_target,
        )
        _validate_artifact_rows(
            record.get("artifacts"),
            expected.label,
            expected_strip=apple_toolchain["strip"],
        )
    if labels != sorted(set(labels)):
        raise NativeArtifactSizeError("native artifact profiles are not sorted unique")


def _validate_artifact_rows(
    value: Any,
    label: str,
    *,
    expected_strip: Mapping[str, Any],
) -> None:
    rows = STRICT_JSON.array(value, f"{label} artifacts")
    kinds = []
    for index, raw in enumerate(rows):
        row = STRICT_JSON.object(raw, f"{label} artifact[{index}]")
        STRICT_JSON.exact_fields(
            row,
            {
                "artifact_kind",
                "file_name",
                "raw_size_bytes",
                "raw_sha256",
                "stripped_size_bytes",
                "stripped_sha256",
                "strip",
            },
            f"{label} artifact[{index}]",
        )
        kind = row.get("artifact_kind")
        if kind not in {"cdylib", "staticlib"}:
            raise NativeArtifactSizeError(f"{label} has an unknown artifact kind")
        kinds.append(kind)
        for field in ("raw_size_bytes", "stripped_size_bytes"):
            if not isinstance(row.get(field), int) or row[field] <= 0:
                raise NativeArtifactSizeError(f"{label} {field} must be positive")
        if row["stripped_size_bytes"] > row["raw_size_bytes"]:
            raise NativeArtifactSizeError(f"{label} strip unexpectedly grew the artifact")
        for field in ("raw_sha256", "stripped_sha256"):
            if not isinstance(row.get(field), str) or not FINGERPRINT_RE.fullmatch(row[field]):
                raise NativeArtifactSizeError(f"{label} {field} is invalid")
        strip = STRICT_JSON.object(row.get("strip"), f"{label} strip")
        STRICT_JSON.exact_fields(
            strip,
            {"recipe_id", "tool", "tool_sha256", "command"},
            f"{label} strip",
        )
        if strip.get("recipe_id") != "apple-strip-local-symbols-v1":
            raise NativeArtifactSizeError(f"{label} strip recipe drifted")
        tool = strip.get("tool")
        if not isinstance(tool, str) or not Path(tool).is_absolute():
            raise NativeArtifactSizeError(f"{label} strip tool must be absolute")
        tool_sha256 = strip.get("tool_sha256")
        if not isinstance(tool_sha256, str) or not FINGERPRINT_RE.fullmatch(
            tool_sha256
        ):
            raise NativeArtifactSizeError(f"{label} strip tool digest is invalid")
        if strip.get("command") != ["$STRIP", "-x", "$ARTIFACT"]:
            raise NativeArtifactSizeError(f"{label} strip command drifted")
        if {
            "path": strip.get("tool"),
            "sha256": strip.get("tool_sha256"),
        } != expected_strip:
            raise NativeArtifactSizeError(
                f"{label} artifact strip tool differs from build provenance"
            )
        file_name = row.get("file_name")
        expected_suffix = ".dylib" if kind == "cdylib" else ".a"
        if (
            not isinstance(file_name, str)
            or Path(file_name).name != file_name
            or not file_name.endswith(expected_suffix)
        ):
            raise NativeArtifactSizeError(f"{label} artifact file name is invalid")
    if kinds != ["cdylib", "staticlib"]:
        raise NativeArtifactSizeError(f"{label} artifacts must be cdylib then staticlib")


def validate_native_artifact_build_record(
    value: Any,
    profile: NativeArtifactProfile,
    host_target: str,
) -> dict[str, Any]:
    label = profile.label
    build = STRICT_JSON.object(value, f"{label} build")
    STRICT_JSON.exact_fields(
        build,
        {
            "command",
            "target_dir",
            "target",
            "cargo_message_format",
            "artifact_selection",
            "apple_toolchain",
        },
        f"{label} build",
    )
    if build.get("target_dir") != f"$EVIDENCE/build/{label}":
        raise NativeArtifactSizeError(f"{label} build target directory drifted")
    if build.get("cargo_message_format") != "json-render-diagnostics":
        raise NativeArtifactSizeError(f"{label} Cargo message format drifted")
    if build.get("target") != host_target:
        raise NativeArtifactSizeError(f"{label} observed host target drifted")
    if build.get("artifact_selection") != "current compiler-artifact event":
        raise NativeArtifactSizeError(f"{label} artifact selection drifted")
    command = STRICT_JSON.array(build.get("command"), f"{label} build command")
    if not command or not all(isinstance(argument, str) and argument for argument in command):
        raise NativeArtifactSizeError(f"{label} build command must contain strings")
    if any(argument.startswith("/") for argument in command):
        raise NativeArtifactSizeError(f"{label} build command contains an absolute path")
    apple_toolchain = _validate_apple_toolchain(build.get("apple_toolchain"), label)
    selected_target = (
        host_target if profile.recipe.build_target_kind == "target-set" else None
    )
    expected_command = native_build_command(
        profile,
        repo_root=Path("$REPO"),
        target_dir=Path(f"$EVIDENCE/build/{label}"),
        target=host_target,
        bindings=NativeBuildBindings(
            cargo="$CARGO",
            rustc="$RUSTC",
            linker_driver="$LINKER_DRIVER",
            developer_dir="$DEVELOPER_DIR",
            sdk="$SDK",
        ),
    )
    if command != expected_command:
        raise NativeArtifactSizeError(f"{label} exact build command drifted")
    return apple_toolchain


def _validate_apple_toolchain(value: Any, label: str) -> dict[str, Any]:
    toolchain = STRICT_JSON.object(value, f"{label} Apple toolchain")
    STRICT_JSON.exact_fields(
        toolchain,
        {
            "developer_dir",
            "xcode_version",
            "sdk",
            "deployment_target",
            "linker_driver",
            "linker",
            "strip",
        },
        f"{label} Apple toolchain",
    )
    developer_dir = toolchain.get("developer_dir")
    if not isinstance(developer_dir, str) or not Path(developer_dir).is_absolute():
        raise NativeArtifactSizeError(f"{label} developer directory must be absolute")
    for field in ("xcode_version", "deployment_target"):
        if not isinstance(toolchain.get(field), str) or not toolchain[field]:
            raise NativeArtifactSizeError(f"{label} Apple {field} is invalid")
    sdk = STRICT_JSON.object(toolchain.get("sdk"), f"{label} Apple SDK")
    STRICT_JSON.exact_fields(
        sdk,
        {"path", "version", "settings"},
        f"{label} Apple SDK",
    )
    sdk_path = sdk.get("path")
    if (
        not isinstance(sdk_path, str)
        or not Path(sdk_path).is_absolute()
        or not Path(sdk_path).is_relative_to(Path(developer_dir))
        or not isinstance(sdk.get("version"), str)
        or not sdk["version"]
    ):
        raise NativeArtifactSizeError(f"{label} Apple SDK provenance is invalid")
    settings = STRICT_JSON.object(sdk.get("settings"), f"{label} Apple SDK settings")
    STRICT_JSON.exact_fields(
        settings,
        {"path", "sha256"},
        f"{label} Apple SDK settings",
    )
    if (
        settings.get("path") != str(Path(sdk_path) / "SDKSettings.json")
        or not isinstance(settings.get("sha256"), str)
        or not FINGERPRINT_RE.fullmatch(settings["sha256"])
    ):
        raise NativeArtifactSizeError(f"{label} Apple SDK settings are invalid")
    for field, expected_name in (
        ("linker_driver", "clang"),
        ("linker", "ld"),
        ("strip", "strip"),
    ):
        identity = STRICT_JSON.object(
            toolchain.get(field),
            f"{label} Apple {field}",
        )
        STRICT_JSON.exact_fields(
            identity,
            {"path", "sha256"},
            f"{label} Apple {field}",
        )
        path = identity.get("path")
        digest = identity.get("sha256")
        if (
            not isinstance(path, str)
            or not Path(path).is_absolute()
            or Path(path).name != expected_name
            or not Path(path).is_relative_to(Path(developer_dir))
            or not isinstance(digest, str)
            or not FINGERPRINT_RE.fullmatch(digest)
        ):
            raise NativeArtifactSizeError(
                f"{label} Apple {field} identity is invalid"
            )
    return toolchain


def compare_native_artifact_sizes(
    baseline: Mapping[str, Any],
    current_measurements: Sequence[dict[str, Any]],
    approvals: Sequence[NativeArtifactSizeApproval] = (),
) -> list[str]:
    return list(
        evaluate_native_artifact_sizes(
            baseline,
            current_measurements,
            approvals,
        ).failures
    )


def evaluate_native_artifact_sizes(
    baseline: Mapping[str, Any],
    current_measurements: Sequence[dict[str, Any]],
    approvals: Sequence[NativeArtifactSizeApproval] = (),
) -> NativeArtifactComparison:
    failures: list[str] = []
    deltas: list[NativeArtifactSizeDelta] = []
    approval_by_key = {approval.key: approval for approval in approvals}
    if len(approval_by_key) != len(approvals):
        raise NativeArtifactSizeError(
            "native artifact size approvals contain duplicate profile/artifact keys"
        )
    used_approvals: set[tuple[str, str]] = set()
    baseline_profiles = {
        profile["profile"]["label"]: profile for profile in baseline["profiles"]
    }
    current_profiles = {
        profile["profile"]["label"]: profile for profile in current_measurements
    }
    if len(baseline_profiles) != len(baseline["profiles"]):
        raise NativeArtifactSizeError("baseline native artifact profiles are duplicated")
    if len(current_profiles) != len(current_measurements):
        raise NativeArtifactSizeError("current native artifact profiles are duplicated")
    missing_profiles = sorted(baseline_profiles.keys() - current_profiles.keys())
    unexpected_profiles = sorted(current_profiles.keys() - baseline_profiles.keys())
    if missing_profiles:
        failures.append(
            "native artifact profiles are missing: " + ", ".join(missing_profiles)
        )
    if unexpected_profiles:
        failures.append(
            "native artifact profiles are unexpected: "
            + ", ".join(unexpected_profiles)
        )
    for label in sorted(baseline_profiles.keys() & current_profiles.keys()):
        baseline_profile = baseline_profiles[label]
        current_profile = current_profiles[label]
        if baseline_profile["profile"] != current_profile["profile"]:
            failures.append(f"{label} native artifact profile recipe changed")
            continue
        if (
            baseline_profile["build"]["command"]
            != current_profile["build"]["command"]
            or baseline_profile["build"]["target"]
            != current_profile["build"]["target"]
        ):
            failures.append(
                f"{label} build recipe changed"
            )
            continue
        if _apple_toolchain_compatibility_projection(
            baseline_profile["build"]["apple_toolchain"]
        ) != _apple_toolchain_compatibility_projection(
            current_profile["build"]["apple_toolchain"]
        ):
            failures.append(
                f"{label} Apple toolchain changed"
            )
            continue
        budget_class = baseline_profile["profile"]["budget_class"]
        baseline_artifacts = {
            row["artifact_kind"]: row for row in baseline_profile["artifacts"]
        }
        current_artifacts = {
            row["artifact_kind"]: row for row in current_profile["artifacts"]
        }
        if set(baseline_artifacts) != set(current_artifacts):
            failures.append(f"{label} native artifact kinds changed")
            continue
        for artifact_kind in sorted(baseline_artifacts):
            baseline_row = baseline_artifacts[artifact_kind]
            current_row = current_artifacts[artifact_kind]
            if (
                baseline_row["strip"]["recipe_id"],
                baseline_row["strip"]["tool_sha256"],
                baseline_row["strip"]["command"],
            ) != (
                current_row["strip"]["recipe_id"],
                current_row["strip"]["tool_sha256"],
                current_row["strip"]["command"],
            ):
                failures.append(
                    f"{label}/{artifact_kind} strip tool or recipe changed"
                )
                continue
            baseline_size = baseline_row["stripped_size_bytes"]
            current_size = current_row["stripped_size_bytes"]
            budget = artifact_growth_budget(baseline_size, budget_class)
            delta = current_size - baseline_size
            status = "ok"
            if delta > budget and budget_class == "semantic":
                status = "failed"
                failures.append(
                    f"{label}/{artifact_kind} semantic stripped artifact grew by "
                    f"{delta} bytes; budget={budget}"
                )
            elif delta > budget:
                approval = approval_by_key.get((label, artifact_kind))
                expected = {
                    "profile": label,
                    "artifact_kind": artifact_kind,
                    "baseline_report_sha256": baseline["report_sha256"],
                    "baseline_stripped_sha256": baseline_row["stripped_sha256"],
                    "current_stripped_sha256": current_row["stripped_sha256"],
                    "baseline_size_bytes": baseline_size,
                    "current_size_bytes": current_size,
                    "budget_bytes": budget,
                }
                if approval is not None and all(
                    approval.projection()[field] == value
                    for field, value in expected.items()
                ):
                    status = "approved"
                    used_approvals.add(approval.key)
                else:
                    status = "review-required"
                    failures.append(
                        f"{label}/{artifact_kind} full stripped artifact grew by "
                        f"{delta} bytes; budget={budget}; exact approval required: "
                        + json.dumps(expected, sort_keys=True)
                        + "; add a reviewed reason before recording the approval"
                    )
            deltas.append(
                NativeArtifactSizeDelta(
                    profile=label,
                    budget_class=budget_class,
                    artifact_kind=artifact_kind,
                    baseline_size_bytes=baseline_size,
                    current_size_bytes=current_size,
                    delta_bytes=delta,
                    budget_bytes=budget,
                    status=status,
                )
            )
    for approval in approvals:
        if approval.key not in used_approvals:
            failures.append(
                f"stale or unmatched native artifact size approval: "
                f"{approval.profile}/{approval.artifact_kind}"
            )
    return NativeArtifactComparison(tuple(deltas), tuple(failures))


def _apple_toolchain_compatibility_projection(
    value: Mapping[str, Any],
) -> dict[str, Any]:
    sdk = value["sdk"]
    return {
        "xcode_version": value["xcode_version"],
        "sdk_version": sdk["version"],
        "sdk_settings_sha256": sdk["settings"]["sha256"],
        "deployment_target": value["deployment_target"],
        "linker_driver_sha256": value["linker_driver"]["sha256"],
        "linker_sha256": value["linker"]["sha256"],
        "strip_sha256": value["strip"]["sha256"],
    }


def artifact_growth_budget(baseline_size: int, budget_class: str) -> int:
    if budget_class == "semantic":
        return max(math.ceil(baseline_size * SEMANTIC_PERCENT), SEMANTIC_FLOOR_BYTES)
    if budget_class == "full":
        return max(math.ceil(baseline_size * FULL_PERCENT), FULL_FLOOR_BYTES)
    raise NativeArtifactSizeError(f"unknown native artifact budget class {budget_class!r}")


def _default_runner(
    command: Sequence[str],
    cwd: Path,
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        command,
        cwd=cwd,
        check=False,
        capture_output=True,
        env=ffi_contract_subprocess_environment(),
        text=True,
    )


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--baseline-lock", type=Path, default=DEFAULT_BASELINE_LOCK)
    parser.add_argument(
        "--size-approvals",
        type=Path,
        default=DEFAULT_NATIVE_SIZE_APPROVALS,
    )
    parser.add_argument(
        "--output-root",
        type=Path,
        default=REPO_ROOT / "target" / "ffi-contract-current-artifacts",
    )
    args = parser.parse_args(argv)
    try:
        baseline = load_native_artifact_baseline(
            args.baseline,
            lock_path=args.baseline_lock,
        )
        try:
            current_toolchain = rust_toolchain_provenance(
                lambda command: _default_runner(command, REPO_ROOT)
            )
        except FfiContractReproducibilityError as error:
            raise NativeArtifactSizeError(str(error)) from error
        if rust_toolchain_native_compatibility_projection(
            current_toolchain
        ) != rust_toolchain_native_compatibility_projection(baseline["toolchain"]):
            raise NativeArtifactSizeError(
                "native artifact toolchain differs from baseline: "
                f"baseline={baseline['toolchain']!r} current={current_toolchain!r}"
            )
        measurements = capture_native_artifact_measurements(
            load_native_artifact_profiles(),
            repo_root=REPO_ROOT,
            output_root=args.output_root.resolve(),
            rust_toolchain=current_toolchain,
            runner=_default_runner,
        )
        comparison = evaluate_native_artifact_sizes(
            baseline,
            measurements,
            load_native_artifact_size_approvals(args.size_approvals),
        )
        if comparison.failures:
            raise NativeArtifactSizeError(
                "native artifact size verification failed:\n- "
                + "\n- ".join(comparison.failures)
            )
    except (NativeArtifactSizeError, RuntimeError) as error:
        print(error, file=sys.stderr)
        return 1
    for delta in comparison.deltas:
        print(
            "native-artifact-size "
            f"status={delta.status} "
            f"profile={delta.profile} "
            f"budget-class={delta.budget_class} "
            f"kind={delta.artifact_kind} "
            f"baseline-bytes={delta.baseline_size_bytes} "
            f"current-bytes={delta.current_size_bytes} "
            f"delta-bytes={delta.delta_bytes} "
            f"budget-bytes={delta.budget_bytes}"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
