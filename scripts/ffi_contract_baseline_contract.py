#!/usr/bin/env python3
"""Shared immutable-contract primitives for fixed FFI baseline evidence."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
import hashlib
import os
import re
from pathlib import Path
import stat
from typing import Any

from artifact_profile_recipe import REPO_ROOT
from ffi_contract_dependency_probes import BASELINE_COMMIT, BASELINE_TREE
from strict_json import (
    StrictJsonContract,
)


BASELINE_LOCK_SCHEMA_VERSION = 2
BASELINE_ROOT = REPO_ROOT / "abi" / "ffi-contract-baseline"
DEFAULT_BASELINE_LOCK = BASELINE_ROOT / "baseline-lock.json"
FINGERPRINT_RE = re.compile(r"^sha256:[0-9a-f]{64}$")
BASELINE_INPUT_PATHS = (
    Path("Cargo.lock"),
    Path("Cargo.toml"),
    Path("capabilities/artifact-profiles-v1.json"),
    Path("crates/merman-android-jni/Cargo.toml"),
    Path("crates/merman-bindings-core/Cargo.toml"),
    Path("crates/merman-ffi/Cargo.toml"),
    Path("crates/merman-node/Cargo.lock"),
    Path("crates/merman-node/Cargo.toml"),
    Path("crates/merman-uniffi/Cargo.toml"),
    Path("platforms/node/candidate-builds.json"),
    Path("rust-toolchain.toml"),
)
NATIVE_ARTIFACT_INPUT_PATHS = (
    Path("Cargo.lock"),
    Path("Cargo.toml"),
    Path("capabilities/artifact-profiles-v1.json"),
    Path("crates/merman-ffi/Cargo.toml"),
    Path("rust-toolchain.toml"),
)


class FfiBaselineContractError(RuntimeError):
    """Immutable FFI evidence or its checked-in lock is malformed."""


STRICT_JSON = StrictJsonContract(
    error_factory=FfiBaselineContractError,
    read_error_prefix="cannot read FFI baseline contract",
)


def file_sha256(path: Path) -> str:
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(path, flags)
        try:
            before = os.fstat(descriptor)
            if not stat.S_ISREG(before.st_mode):
                raise OSError("not a regular file")
            digest = hashlib.sha256()
            with os.fdopen(descriptor, "rb", closefd=False) as source:
                for chunk in iter(lambda: source.read(1024 * 1024), b""):
                    digest.update(chunk)
            after = os.fstat(descriptor)
            identity_before = (
                before.st_dev,
                before.st_ino,
                before.st_size,
                before.st_mtime_ns,
                before.st_ctime_ns,
            )
            identity_after = (
                after.st_dev,
                after.st_ino,
                after.st_size,
                after.st_mtime_ns,
                after.st_ctime_ns,
            )
            if identity_after != identity_before:
                raise OSError("file changed while hashing")
        finally:
            os.close(descriptor)
    except OSError as error:
        raise FfiBaselineContractError(
            f"cannot hash regular file {path}: {error}"
        ) from error
    return f"sha256:{digest.hexdigest()}"


def input_records(
    repo_root: Path,
    paths: Sequence[Path],
) -> list[dict[str, str]]:
    return [
        {"path": path.as_posix(), "sha256": file_sha256(repo_root / path)}
        for path in paths
    ]


def source_revision_projection(snapshot_sha256: str) -> dict[str, Any]:
    validate_finalized_sha256(snapshot_sha256, "source snapshot digest")
    return {
        "commit": BASELINE_COMMIT,
        "tree": BASELINE_TREE,
        "materialization": "git-cat-file-batch-v1",
        "captured_from_git_object": True,
        "snapshot_sha256": snapshot_sha256,
    }


def validate_source_revision(value: Any) -> None:
    revision = STRICT_JSON.object(value, "baseline source revision")
    STRICT_JSON.exact_fields(
        revision,
        {
            "commit",
            "tree",
            "materialization",
            "captured_from_git_object",
            "snapshot_sha256",
        },
        "baseline source revision",
    )
    validate_finalized_sha256(
        revision.get("snapshot_sha256"),
        "baseline source snapshot digest",
    )
    if revision != source_revision_projection(revision["snapshot_sha256"]):
        raise FfiBaselineContractError("baseline source revision is not canonical")


def validate_input_records(
    value: Any,
    *,
    expected_paths: Sequence[Path] | None = None,
) -> list[dict[str, str]]:
    records = STRICT_JSON.array(value, "baseline inputs")
    projections: list[tuple[str, str]] = []
    parsed: list[dict[str, str]] = []
    for index, raw in enumerate(records):
        record = STRICT_JSON.object(raw, f"baseline input[{index}]")
        STRICT_JSON.exact_fields(
            record,
            {"path", "sha256"},
            f"baseline input[{index}]",
        )
        path = record.get("path")
        digest = record.get("sha256")
        if (
            not isinstance(path, str)
            or not path
            or Path(path).is_absolute()
            or ".." in Path(path).parts
        ):
            raise FfiBaselineContractError(
                "baseline input path must be repository-relative"
            )
        validate_finalized_sha256(digest, "baseline input digest")
        projections.append((path, digest))
        parsed.append({"path": path, "sha256": digest})
    if projections != sorted(set(projections)):
        raise FfiBaselineContractError(
            "baseline inputs must be sorted and unique"
        )
    if expected_paths is not None and [path for path, _digest in projections] != [
        path.as_posix() for path in expected_paths
    ]:
        raise FfiBaselineContractError("baseline input set drifted")
    return parsed


def validate_rust_toolchain(value: Any) -> dict[str, Any]:
    toolchain = STRICT_JSON.object(value, "baseline toolchain")
    STRICT_JSON.exact_fields(
        toolchain,
        {"cargo", "rustc", "cargo_version", "rustc_verbose", "host_target"},
        "baseline toolchain",
    )
    for field in ("cargo_version", "rustc_verbose", "host_target"):
        STRICT_JSON.string(toolchain.get(field), f"baseline toolchain.{field}")
    for field, expected_name in (("cargo", "cargo"), ("rustc", "rustc")):
        identity = STRICT_JSON.object(
            toolchain.get(field),
            f"baseline toolchain.{field}",
        )
        STRICT_JSON.exact_fields(
            identity,
            {"path", "sha256"},
            f"baseline toolchain.{field}",
        )
        path = identity.get("path")
        digest = identity.get("sha256")
        if (
            not isinstance(path, str)
            or not Path(path).is_absolute()
            or Path(path).name != expected_name
        ):
            raise FfiBaselineContractError(
                f"baseline toolchain.{field} identity is invalid"
            )
        validate_finalized_sha256(
            digest,
            f"baseline toolchain.{field} digest",
        )
    return toolchain


def rust_toolchain_native_compatibility_projection(
    value: Mapping[str, Any],
) -> dict[str, Any]:
    """Compare exact native tool bytes and versions across installation-path moves."""

    return {
        "cargo_sha256": value["cargo"]["sha256"],
        "rustc_sha256": value["rustc"]["sha256"],
        "cargo_version": value["cargo_version"],
        "rustc_verbose": value["rustc_verbose"],
        "host_target": value["host_target"],
    }


def rust_toolchain_dependency_compatibility_projection(
    value: Mapping[str, Any],
) -> dict[str, Any]:
    """Compare dependency-resolution semantics without binding evidence to one host."""

    rustc_semantics = "\n".join(
        line
        for line in value["rustc_verbose"].splitlines()
        if not line.startswith("host: ")
    )
    return {
        "cargo_version": value["cargo_version"],
        "rustc_semantics": rustc_semantics,
    }


def load_baseline_lock(path: Path) -> dict[str, Any]:
    _raw, parsed = STRICT_JSON.load_bytes(path)
    lock = STRICT_JSON.object(parsed, "FFI baseline lock")
    STRICT_JSON.exact_fields(
        lock,
        {
            "schema_version",
            "baseline_commit",
            "baseline_input_sha256",
            "source_snapshot_sha256",
            "baseline_tree",
            "dependency_report_schema_version",
            "dependency_report_file_sha256",
            "native_artifact_report_schema_version",
            "native_artifact_report_file_sha256",
            "probe_registry_sha256",
        },
        "FFI baseline lock",
    )
    if lock.get("schema_version") != BASELINE_LOCK_SCHEMA_VERSION:
        raise FfiBaselineContractError(
            f"FFI baseline lock schema_version must be {BASELINE_LOCK_SCHEMA_VERSION}"
        )
    if lock.get("baseline_commit") != BASELINE_COMMIT:
        raise FfiBaselineContractError("FFI baseline lock commit drifted")
    if lock.get("baseline_tree") != BASELINE_TREE:
        raise FfiBaselineContractError("FFI baseline lock tree drifted")
    validate_finalized_sha256(
        lock.get("source_snapshot_sha256"),
        "FFI baseline lock source snapshot digest",
    )
    for field in (
        "dependency_report_schema_version",
        "native_artifact_report_schema_version",
    ):
        if not isinstance(lock.get(field), int) or lock[field] <= 0:
            raise FfiBaselineContractError(f"FFI baseline lock {field} is invalid")
    for field in (
        "dependency_report_file_sha256",
        "native_artifact_report_file_sha256",
        "probe_registry_sha256",
    ):
        validate_finalized_sha256(
            lock.get(field),
            f"FFI baseline lock {field}",
        )
    inputs = STRICT_JSON.object(
        lock.get("baseline_input_sha256"),
        "FFI baseline lock inputs",
    )
    expected_paths = [path.as_posix() for path in BASELINE_INPUT_PATHS]
    if sorted(inputs) != expected_paths:
        raise FfiBaselineContractError("FFI baseline lock input set is not canonical")
    for relative, digest in inputs.items():
        validate_finalized_sha256(
            digest,
            f"FFI baseline lock input digest for {relative}",
        )
    return lock


def validate_finalized_sha256(value: Any, context: str) -> str:
    if (
        not isinstance(value, str)
        or not FINGERPRINT_RE.fullmatch(value)
        or value == "sha256:" + "0" * 64
    ):
        raise FfiBaselineContractError(f"{context} is not finalized")
    return value
