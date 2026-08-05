#!/usr/bin/env python3
"""Capture and validate matched native FFI clean-build timing evidence."""

from __future__ import annotations

import argparse
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from datetime import datetime, timedelta, timezone
import json
import math
import os
from pathlib import Path
import platform
import shutil
import statistics
import subprocess
import sys
import tarfile
import tempfile
import time
from typing import Any

from artifact_profile_recipe import REPO_ROOT
from ffi_contract_baseline_contract import (
    FfiBaselineContractError,
    rust_toolchain_compatibility_projection,
    validate_rust_toolchain,
)
from ffi_contract_dependency_probes import BASELINE_COMMIT, BASELINE_TREE
from ffi_contract_reproducibility import (
    FfiContractReproducibilityError,
    ffi_contract_subprocess_environment,
    reject_cargo_configuration,
    reject_ffi_contract_environment,
    rust_toolchain_provenance,
)
from strict_json import canonical_sha256
from verify_native_artifact_sizes import (
    NativeArtifactSizeError,
    capture_native_artifact_measurements,
    load_native_artifact_baseline,
    load_native_artifact_profiles,
    validate_native_artifact_build_record,
)


REPORT_ID = "merman-ffi-contract-native-build-timing"
SCHEMA_VERSION = 1
MINIMUM_RUNS = 3
MAXIMUM_RUNS = 9
REVIEW_THRESHOLD_RATIO = 0.10
DEFAULT_REPORT = (
    REPO_ROOT / "docs" / "release" / "evidence" / "ffi-contract-native-build-timing.json"
)
POST_CAPTURE_ALLOWED_PATHS = frozenset(
    {
        "docs/release/FFI_CONTRACT_READINESS.md",
        "docs/release/evidence/ffi-contract-native-build-timing.json",
    }
)


class NativeBuildTimingError(RuntimeError):
    """Native clean-build timing evidence is invalid or could not be captured."""


@dataclass(frozen=True)
class SourceRevision:
    commit: str
    tree: str

    def projection(self) -> dict[str, str]:
        return {"commit": self.commit, "tree": self.tree}


@dataclass(frozen=True)
class RevisionMeasurement:
    duration_ns: int
    profile: dict[str, Any]
    build: dict[str, Any]


class TimedProcessRunner:
    def __init__(self) -> None:
        self._cargo_build_durations_ns: list[int] = []

    def __call__(
        self,
        command: Sequence[str],
        cwd: Path,
    ) -> subprocess.CompletedProcess[str]:
        is_cargo_build = len(command) >= 2 and command[1] == "build"
        started = time.perf_counter_ns() if is_cargo_build else 0
        completed = subprocess.run(
            command,
            cwd=cwd,
            check=False,
            capture_output=True,
            env=ffi_contract_subprocess_environment(),
            text=True,
        )
        if is_cargo_build:
            self._cargo_build_durations_ns.append(time.perf_counter_ns() - started)
        return completed

    def single_cargo_build_duration_ns(self) -> int:
        if len(self._cargo_build_durations_ns) != 1:
            raise NativeBuildTimingError(
                "one native timing sample must execute exactly one Cargo build"
            )
        return self._cargo_build_durations_ns[0]


def _exact_fields(value: Mapping[str, Any], expected: set[str], context: str) -> None:
    actual = set(value)
    if actual != expected:
        missing = sorted(expected - actual)
        extra = sorted(actual - expected)
        raise NativeBuildTimingError(
            f"{context} fields drifted; missing={missing}, extra={extra}"
        )


def _positive_integer(value: Any, context: str) -> int:
    if type(value) is not int or value <= 0:
        raise NativeBuildTimingError(f"{context} must be a positive integer")
    return value


def _finite_number(value: Any, context: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise NativeBuildTimingError(f"{context} must be a finite number")
    number = float(value)
    if not math.isfinite(number):
        raise NativeBuildTimingError(f"{context} must be a finite number")
    return number


def _median_ns(samples: Sequence[int]) -> int:
    if len(samples) % 2 == 0:
        raise NativeBuildTimingError("native timing sample count must be odd")
    return int(statistics.median(samples))


def _relative_mad(samples: Sequence[int], median_ns: int) -> float:
    deviations = [abs(sample - median_ns) for sample in samples]
    return float(statistics.median(deviations)) / float(median_ns)


def timing_statistics(samples: Sequence[Mapping[str, Any]]) -> dict[str, Any]:
    baseline = [_positive_integer(sample["baseline_duration_ns"], "baseline duration") for sample in samples]
    candidate = [_positive_integer(sample["candidate_duration_ns"], "candidate duration") for sample in samples]
    baseline_median = _median_ns(baseline)
    candidate_median = _median_ns(candidate)
    baseline_noise = _relative_mad(baseline, baseline_median)
    candidate_noise = _relative_mad(candidate, candidate_median)
    noise_floor = max(baseline_noise, candidate_noise)
    regression = (float(candidate_median) / float(baseline_median)) - 1.0
    review_required = regression > max(REVIEW_THRESHOLD_RATIO, noise_floor)
    return {
        "baseline_median_ns": baseline_median,
        "candidate_median_ns": candidate_median,
        "baseline_relative_mad": baseline_noise,
        "candidate_relative_mad": candidate_noise,
        "noise_floor_ratio": noise_floor,
        "median_regression_ratio": regression,
        "review_required": review_required,
    }


def embedded_report_sha256(report: Mapping[str, Any]) -> str:
    unsigned = dict(report)
    unsigned.pop("report_sha256", None)
    return f"sha256:{canonical_sha256(unsigned)}"


def validate_report(
    report: Mapping[str, Any],
    *,
    repo_root: Path | None = None,
) -> dict[str, Any]:
    _exact_fields(
        report,
        {
            "schema_version",
            "report_id",
            "captured_at_utc",
            "source_revisions",
            "toolchain",
            "machine",
            "measurement_boundary",
            "samples",
            "statistics",
            "review",
            "report_sha256",
        },
        "native build timing report",
    )
    if report.get("schema_version") != SCHEMA_VERSION:
        raise NativeBuildTimingError(
            f"native build timing schema_version must be {SCHEMA_VERSION}"
        )
    if report.get("report_id") != REPORT_ID:
        raise NativeBuildTimingError("native build timing report_id is invalid")
    captured_at = report.get("captured_at_utc")
    if not isinstance(captured_at, str):
        raise NativeBuildTimingError("captured_at_utc must be an ISO-8601 string")
    try:
        captured = datetime.fromisoformat(captured_at)
    except ValueError as error:
        raise NativeBuildTimingError("captured_at_utc is not valid ISO-8601") from error
    if captured.tzinfo is None or captured.utcoffset() != timedelta(0):
        raise NativeBuildTimingError("captured_at_utc must use UTC")

    revisions = report.get("source_revisions")
    if not isinstance(revisions, dict):
        raise NativeBuildTimingError("source_revisions must be an object")
    _exact_fields(revisions, {"baseline", "candidate"}, "source_revisions")
    for label in ("baseline", "candidate"):
        revision = revisions.get(label)
        if not isinstance(revision, dict):
            raise NativeBuildTimingError(f"source_revisions.{label} must be an object")
        _exact_fields(revision, {"commit", "tree"}, f"source_revisions.{label}")
        for field in ("commit", "tree"):
            value = revision.get(field)
            if not isinstance(value, str) or len(value) != 40 or any(
                character not in "0123456789abcdef" for character in value
            ):
                raise NativeBuildTimingError(
                    f"source_revisions.{label}.{field} must be a full Git object ID"
                )
    if revisions["baseline"]["commit"] != BASELINE_COMMIT:
        raise NativeBuildTimingError("native timing baseline commit is not canonical")
    if revisions["baseline"]["tree"] != BASELINE_TREE:
        raise NativeBuildTimingError("native timing baseline tree is not canonical")
    if revisions["candidate"]["commit"] == BASELINE_COMMIT:
        raise NativeBuildTimingError("candidate timing revision must differ from the baseline")

    try:
        toolchain = validate_rust_toolchain(report.get("toolchain"))
    except FfiBaselineContractError as error:
        raise NativeBuildTimingError(str(error)) from error
    machine = report.get("machine")
    if not isinstance(machine, dict):
        raise NativeBuildTimingError("machine provenance must be an object")
    _exact_fields(
        machine,
        {
            "system",
            "release",
            "machine",
            "processor",
            "logical_cpu_count",
            "memory_bytes",
            "python_version",
        },
        "machine provenance",
    )
    for field in ("system", "release", "machine", "processor", "python_version"):
        if not isinstance(machine.get(field), str) or not machine[field]:
            raise NativeBuildTimingError(f"machine.{field} must be a non-empty string")
    if machine.get("logical_cpu_count") is not None:
        _positive_integer(machine.get("logical_cpu_count"), "machine.logical_cpu_count")
    if machine.get("memory_bytes") is not None:
        _positive_integer(machine.get("memory_bytes"), "machine.memory_bytes")
    _validate_machine_target(machine, toolchain["host_target"])

    boundary = report.get("measurement_boundary")
    if not isinstance(boundary, dict):
        raise NativeBuildTimingError("measurement_boundary must be an object")
    _exact_fields(
        boundary,
        {
            "metric",
            "clock",
            "profile",
            "build",
            "fresh_target_directory_per_sample",
            "paired_order",
            "sample_count_per_revision",
            "review_threshold_ratio",
            "noise_floor_method",
            "timing_is_gating",
        },
        "measurement_boundary",
    )
    if boundary.get("metric") != "clean-cargo-build-and-link-wall-nanoseconds":
        raise NativeBuildTimingError("native timing metric drifted")
    if boundary.get("clock") != "time.perf_counter_ns":
        raise NativeBuildTimingError("native timing clock drifted")
    if boundary.get("fresh_target_directory_per_sample") is not True:
        raise NativeBuildTimingError("each native timing sample must use a fresh target directory")
    if boundary.get("paired_order") != "alternating-baseline-candidate":
        raise NativeBuildTimingError("native timing paired order drifted")
    if boundary.get("noise_floor_method") != "max-relative-mad-to-median":
        raise NativeBuildTimingError("native timing noise-floor method drifted")
    if boundary.get("timing_is_gating") is not True:
        raise NativeBuildTimingError("native timing evidence must enforce its review threshold")
    if _finite_number(boundary.get("review_threshold_ratio"), "review threshold") != REVIEW_THRESHOLD_RATIO:
        raise NativeBuildTimingError("native timing review threshold drifted")
    run_count = _positive_integer(
        boundary.get("sample_count_per_revision"),
        "sample_count_per_revision",
    )
    if run_count < MINIMUM_RUNS or run_count > MAXIMUM_RUNS or run_count % 2 == 0:
        raise NativeBuildTimingError(
            f"sample_count_per_revision must be an odd value from {MINIMUM_RUNS} to {MAXIMUM_RUNS}"
        )
    profile = boundary.get("profile")
    if not isinstance(profile, dict):
        raise NativeBuildTimingError("native timing profile must be an object")
    _exact_fields(profile, {"label", "budget_class", "target", "cargo"}, "native timing profile")
    if profile.get("label") != "ffi-full-native" or profile.get("budget_class") != "full":
        raise NativeBuildTimingError("native timing must measure the full native FFI profile")
    if profile.get("target") != toolchain["host_target"]:
        raise NativeBuildTimingError("native timing profile target does not match rustc host")
    if not isinstance(profile.get("cargo"), dict) or not isinstance(boundary.get("build"), dict):
        raise NativeBuildTimingError("native timing build recipe must be structured")
    if repo_root is not None:
        _validate_repository_contract(
            repo_root=repo_root,
            revisions=revisions,
            toolchain=toolchain,
            profile=profile,
            build=boundary["build"],
        )

    samples = report.get("samples")
    if not isinstance(samples, list) or len(samples) != run_count:
        raise NativeBuildTimingError("native timing sample count does not match its boundary")
    for index, sample in enumerate(samples, start=1):
        if not isinstance(sample, dict):
            raise NativeBuildTimingError(f"native timing sample[{index}] must be an object")
        _exact_fields(
            sample,
            {
                "pair_index",
                "order",
                "baseline_duration_ns",
                "candidate_duration_ns",
            },
            f"native timing sample[{index}]",
        )
        if sample.get("pair_index") != index:
            raise NativeBuildTimingError("native timing pairs must be sorted and contiguous")
        expected_order = (
            ["baseline", "candidate"] if index % 2 == 1 else ["candidate", "baseline"]
        )
        if sample.get("order") != expected_order:
            raise NativeBuildTimingError("native timing pair order is not alternating")
        _positive_integer(sample.get("baseline_duration_ns"), "baseline duration")
        _positive_integer(sample.get("candidate_duration_ns"), "candidate duration")

    expected_statistics = timing_statistics(samples)
    actual_statistics = report.get("statistics")
    if not isinstance(actual_statistics, dict):
        raise NativeBuildTimingError("native timing statistics must be an object")
    _exact_fields(actual_statistics, set(expected_statistics), "native timing statistics")
    for field, expected in expected_statistics.items():
        actual = actual_statistics.get(field)
        if isinstance(expected, bool):
            if actual is not expected:
                raise NativeBuildTimingError(f"native timing statistic {field} is stale")
        elif isinstance(expected, int):
            if actual != expected:
                raise NativeBuildTimingError(f"native timing statistic {field} is stale")
        elif not math.isclose(
            _finite_number(actual, f"native timing statistic {field}"),
            expected,
            rel_tol=1e-12,
            abs_tol=1e-12,
        ):
            raise NativeBuildTimingError(f"native timing statistic {field} is stale")

    review = report.get("review")
    if expected_statistics["review_required"]:
        if not isinstance(review, dict):
            raise NativeBuildTimingError(
                "an over-threshold timing regression requires an explicit review"
            )
        _exact_fields(review, {"accepted", "reason"}, "native timing review")
        reason = review.get("reason")
        if review.get("accepted") is not True or not isinstance(reason, str):
            raise NativeBuildTimingError("native timing review must explicitly accept the result")
        if reason != reason.strip() or not reason or len(reason) > 512 or "\n" in reason:
            raise NativeBuildTimingError(
                "native timing review reason must be a trimmed single line of at most 512 characters"
            )
    elif review is not None:
        raise NativeBuildTimingError("non-regressing timing evidence must not carry an approval")

    if report.get("report_sha256") != embedded_report_sha256(report):
        raise NativeBuildTimingError("native timing embedded digest is stale")
    return dict(report)


def load_report(
    path: Path,
    *,
    repo_root: Path | None = None,
) -> dict[str, Any]:
    if path.is_symlink() or not path.is_file():
        raise NativeBuildTimingError(f"native timing report must be a regular file: {path}")
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise NativeBuildTimingError(f"cannot read native timing report {path}: {error}") from error
    if not isinstance(value, dict):
        raise NativeBuildTimingError("native timing report root must be an object")
    return validate_report(value, repo_root=repo_root)


def _run_text(command: Sequence[str], cwd: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        command,
        cwd=cwd,
        check=False,
        capture_output=True,
        env=ffi_contract_subprocess_environment(),
        text=True,
    )


def _checked_output(command: Sequence[str], cwd: Path) -> str:
    completed = _run_text(command, cwd)
    if completed.returncode != 0:
        detail = (completed.stderr or completed.stdout or "").strip()
        raise NativeBuildTimingError(
            f"command failed with {completed.returncode}: {' '.join(command)}: {detail}"
        )
    value = completed.stdout.strip()
    if not value:
        raise NativeBuildTimingError(f"command produced no text: {' '.join(command)}")
    return value


def _validate_git_revision(
    repo_root: Path,
    revision: Mapping[str, Any],
    label: str,
) -> None:
    git = _git_executable()
    commit = revision["commit"]
    tree = revision["tree"]
    resolved_commit = _checked_output(
        (git, "--no-replace-objects", "rev-parse", f"{commit}^{{commit}}"),
        repo_root,
    )
    resolved_tree = _checked_output(
        (git, "--no-replace-objects", "rev-parse", f"{commit}^{{tree}}"),
        repo_root,
    )
    if resolved_commit != commit or resolved_tree != tree:
        raise NativeBuildTimingError(
            f"native timing {label} commit/tree does not match the repository"
        )


def _validate_candidate_ancestry(repo_root: Path, candidate_commit: str) -> None:
    git = _git_executable()
    ancestry = _run_text(
        (
            git,
            "--no-replace-objects",
            "merge-base",
            "--is-ancestor",
            candidate_commit,
            "HEAD",
        ),
        repo_root,
    )
    if ancestry.returncode == 1:
        raise NativeBuildTimingError(
            "native timing candidate is not an ancestor of the checked-out HEAD"
        )
    if ancestry.returncode != 0:
        detail = (ancestry.stderr or ancestry.stdout or "").strip()
        raise NativeBuildTimingError(
            f"cannot validate native timing candidate ancestry: {detail}"
        )
    changed = _run_text(
        (
            git,
            "--no-replace-objects",
            "diff",
            "--no-renames",
            "--name-only",
            "--diff-filter=ACDMRTUXB",
            f"{candidate_commit}..HEAD",
            "--",
        ),
        repo_root,
    )
    if changed.returncode != 0:
        detail = (changed.stderr or changed.stdout or "").strip()
        raise NativeBuildTimingError(
            f"cannot inspect post-capture native timing changes: {detail}"
        )
    changed_paths = {line for line in changed.stdout.splitlines() if line}
    unexpected = sorted(changed_paths - POST_CAPTURE_ALLOWED_PATHS)
    if unexpected:
        raise NativeBuildTimingError(
            "native timing evidence predates implementation changes: "
            + ", ".join(unexpected)
        )


def _validate_machine_target(
    machine: Mapping[str, Any],
    host_target: str,
) -> None:
    if machine["system"] != "Darwin" or not host_target.endswith("-apple-darwin"):
        raise NativeBuildTimingError(
            "native timing evidence must describe an Apple Darwin host"
        )
    target_arch = host_target.split("-", maxsplit=1)[0]
    machine_arch = machine["machine"].lower()
    accepted = {
        "aarch64": {"aarch64", "arm64"},
        "x86_64": {"x86_64", "amd64"},
    }.get(target_arch)
    if accepted is None or machine_arch not in accepted:
        raise NativeBuildTimingError(
            "native timing machine architecture does not match the Rust host target"
        )


def _validate_repository_contract(
    *,
    repo_root: Path,
    revisions: Mapping[str, Any],
    toolchain: Mapping[str, Any],
    profile: Mapping[str, Any],
    build: Mapping[str, Any],
) -> None:
    _validate_git_revision(repo_root, revisions["baseline"], "baseline")
    _validate_git_revision(repo_root, revisions["candidate"], "candidate")
    _validate_candidate_ancestry(repo_root, revisions["candidate"]["commit"])

    baseline_path = (
        repo_root / "abi" / "ffi-contract-baseline" / "native-artifact-sizes.json"
    )
    try:
        baseline = load_native_artifact_baseline(
            baseline_path,
            repo_root=repo_root,
        )
    except NativeArtifactSizeError as error:
        raise NativeBuildTimingError(str(error)) from error
    if rust_toolchain_compatibility_projection(
        toolchain
    ) != rust_toolchain_compatibility_projection(baseline["toolchain"]):
        raise NativeBuildTimingError(
            "native timing toolchain differs from the immutable artifact baseline"
        )

    expected_profile = next(
        (
            candidate
            for candidate in load_native_artifact_profiles(repo_root)
            if candidate.label == "ffi-full-native"
        ),
        None,
    )
    if expected_profile is None:
        raise NativeBuildTimingError("native timing profile ffi-full-native is missing")
    host_target = toolchain["host_target"]
    if profile != expected_profile.projection(host_target):
        raise NativeBuildTimingError(
            "native timing profile differs from the descriptor-owned artifact recipe"
        )
    try:
        validate_native_artifact_build_record(build, expected_profile, host_target)
    except NativeArtifactSizeError as error:
        raise NativeBuildTimingError(str(error)) from error


def _git_executable() -> str:
    value = shutil.which("git")
    if value is None:
        raise NativeBuildTimingError("could not resolve git")
    path = Path(value).resolve()
    if not path.is_file() or not os.access(path, os.X_OK):
        raise NativeBuildTimingError(f"resolved git is not executable: {path}")
    return str(path)


def resolve_revision(repo_root: Path, revision: str) -> SourceRevision:
    git = _git_executable()
    commit = _checked_output(
        (git, "--no-replace-objects", "rev-parse", f"{revision}^{{commit}}"),
        repo_root,
    )
    tree = _checked_output(
        (git, "--no-replace-objects", "rev-parse", f"{commit}^{{tree}}"),
        repo_root,
    )
    return SourceRevision(commit=commit, tree=tree)


def materialize_revision(repo_root: Path, revision: SourceRevision, destination: Path) -> None:
    if destination.exists() or destination.is_symlink():
        raise NativeBuildTimingError(f"snapshot destination already exists: {destination}")
    destination.mkdir(parents=True)
    archive_path = destination.parent / f"{destination.name}.tar"
    git = _git_executable()
    try:
        with archive_path.open("wb") as archive_file:
            completed = subprocess.run(
                (git, "--no-replace-objects", "archive", "--format=tar", revision.commit),
                cwd=repo_root,
                check=False,
                stdout=archive_file,
                stderr=subprocess.PIPE,
                env=ffi_contract_subprocess_environment(),
            )
        if completed.returncode != 0:
            detail = completed.stderr.decode("utf-8", errors="replace").strip()
            raise NativeBuildTimingError(
                f"git archive failed for {revision.commit}: {detail}"
            )
        with tarfile.open(archive_path, mode="r:") as archive:
            _extract_git_archive(archive, destination)
    finally:
        archive_path.unlink(missing_ok=True)


def _extract_git_archive(archive: tarfile.TarFile, destination: Path) -> None:
    root = destination.resolve()
    for member in archive.getmembers():
        if not member.name or member.name == ".":
            continue
        target = (destination / member.name).resolve()
        try:
            target.relative_to(root)
        except ValueError as error:
            raise NativeBuildTimingError(
                f"git archive entry escapes the snapshot root: {member.name}"
            ) from error
        if member.isdir():
            target.mkdir(parents=True, exist_ok=True)
            continue
        if not member.isreg():
            raise NativeBuildTimingError(
                f"git archive entry is not a regular file or directory: {member.name}"
            )
        target.parent.mkdir(parents=True, exist_ok=True)
        source = archive.extractfile(member)
        if source is None:
            raise NativeBuildTimingError(
                f"cannot read git archive entry: {member.name}"
            )
        try:
            with source, target.open("xb") as output:
                shutil.copyfileobj(source, output)
            target.chmod(member.mode & 0o777)
        except OSError as error:
            raise NativeBuildTimingError(
                f"cannot materialize git archive entry {member.name}: {error}"
            ) from error


def _remove_sample_output_root(output_root: Path) -> None:
    try:
        shutil.rmtree(output_root)
    except FileNotFoundError:
        pass
    except OSError as error:
        raise NativeBuildTimingError(
            f"cannot remove native timing sample output root {output_root}: {error}"
        ) from error
    try:
        output_root.lstat()
    except FileNotFoundError:
        return
    except OSError as error:
        raise NativeBuildTimingError(
            f"cannot verify native timing sample output root removal {output_root}: {error}"
        ) from error
    else:
        raise NativeBuildTimingError(
            f"native timing sample output root still exists after removal: {output_root}"
        )


def _measure_revision(
    snapshot_root: Path,
    output_root: Path,
    rust_toolchain: Mapping[str, Any],
) -> RevisionMeasurement:
    try:
        profiles = load_native_artifact_profiles(snapshot_root)
        profile = next(
            (profile for profile in profiles if profile.label == "ffi-full-native"),
            None,
        )
        if profile is None:
            raise NativeBuildTimingError("native timing profile ffi-full-native is missing")
        runner = TimedProcessRunner()
        measurements = capture_native_artifact_measurements(
            (profile,),
            repo_root=snapshot_root,
            output_root=output_root,
            rust_toolchain=rust_toolchain,
            runner=runner,
        )
        if len(measurements) != 1:
            raise NativeBuildTimingError(
                "native timing capture produced an unexpected profile count"
            )
        measurement = measurements[0]
        result = RevisionMeasurement(
            duration_ns=runner.single_cargo_build_duration_ns(),
            profile=dict(measurement["profile"]),
            build=dict(measurement["build"]),
        )
    finally:
        _remove_sample_output_root(output_root)
    return result


def _optional_sysctl(name: str) -> str | None:
    executable = Path("/usr/sbin/sysctl")
    if not executable.is_file():
        return None
    completed = subprocess.run(
        (str(executable), "-n", name),
        check=False,
        capture_output=True,
        text=True,
    )
    value = completed.stdout.strip()
    return value if completed.returncode == 0 and value else None


def machine_provenance() -> dict[str, Any]:
    memory = _optional_sysctl("hw.memsize")
    return {
        "system": platform.system(),
        "release": platform.release(),
        "machine": platform.machine(),
        "processor": _optional_sysctl("machdep.cpu.brand_string") or platform.processor(),
        "logical_cpu_count": os.cpu_count(),
        "memory_bytes": int(memory) if memory is not None and memory.isdigit() else None,
        "python_version": platform.python_version(),
    }


def capture_report(
    *,
    repo_root: Path,
    candidate_revision: str,
    runs: int,
    review_reason: str | None,
) -> dict[str, Any]:
    if runs < MINIMUM_RUNS or runs > MAXIMUM_RUNS or runs % 2 == 0:
        raise NativeBuildTimingError(
            f"--runs must be an odd value from {MINIMUM_RUNS} to {MAXIMUM_RUNS}"
        )
    try:
        reject_ffi_contract_environment()
        reject_cargo_configuration(repo_root)
    except FfiContractReproducibilityError as error:
        raise NativeBuildTimingError(str(error)) from error

    baseline = resolve_revision(repo_root, BASELINE_COMMIT)
    candidate = resolve_revision(repo_root, candidate_revision)
    if candidate == baseline:
        raise NativeBuildTimingError("candidate revision must differ from the fixed baseline")
    rust_toolchain = rust_toolchain_provenance(
        lambda command: _run_text(command, repo_root)
    )

    with tempfile.TemporaryDirectory(prefix="merman-ffi-native-timing-") as temporary:
        root = Path(temporary)
        baseline_root = root / "source-baseline"
        candidate_root = root / "source-candidate"
        materialize_revision(repo_root, baseline, baseline_root)
        materialize_revision(repo_root, candidate, candidate_root)

        samples: list[dict[str, Any]] = []
        reference_profile: dict[str, Any] | None = None
        reference_build: dict[str, Any] | None = None
        for pair_index in range(1, runs + 1):
            order = (
                ("baseline", "candidate")
                if pair_index % 2 == 1
                else ("candidate", "baseline")
            )
            durations: dict[str, int] = {}
            for label in order:
                source_root = baseline_root if label == "baseline" else candidate_root
                output_root = root / "evidence" / f"pair-{pair_index}" / label
                measurement = _measure_revision(source_root, output_root, rust_toolchain)
                durations[label] = measurement.duration_ns
                if reference_profile is None:
                    reference_profile = measurement.profile
                    reference_build = measurement.build
                else:
                    if measurement.profile != reference_profile:
                        raise NativeBuildTimingError(
                            "baseline and candidate native timing recipes are not identical"
                        )
                    if measurement.build != reference_build:
                        raise NativeBuildTimingError(
                            "baseline and candidate native timing build provenance drifted"
                        )
            samples.append(
                {
                    "pair_index": pair_index,
                    "order": list(order),
                    "baseline_duration_ns": durations["baseline"],
                    "candidate_duration_ns": durations["candidate"],
                }
            )

    if reference_profile is None or reference_build is None:
        raise NativeBuildTimingError("native timing capture produced no measurements")
    statistics_projection = timing_statistics(samples)
    review = None
    if statistics_projection["review_required"]:
        if review_reason is None:
            raise NativeBuildTimingError(
                "timing regression crossed the review threshold; rerun with --review-reason after review"
            )
        review = {"accepted": True, "reason": review_reason}
    elif review_reason is not None:
        raise NativeBuildTimingError(
            "--review-reason is permitted only for an over-threshold timing result"
        )

    report: dict[str, Any] = {
        "schema_version": SCHEMA_VERSION,
        "report_id": REPORT_ID,
        "captured_at_utc": datetime.now(timezone.utc).isoformat(),
        "source_revisions": {
            "baseline": baseline.projection(),
            "candidate": candidate.projection(),
        },
        "toolchain": rust_toolchain,
        "machine": machine_provenance(),
        "measurement_boundary": {
            "metric": "clean-cargo-build-and-link-wall-nanoseconds",
            "clock": "time.perf_counter_ns",
            "profile": reference_profile,
            "build": reference_build,
            "fresh_target_directory_per_sample": True,
            "paired_order": "alternating-baseline-candidate",
            "sample_count_per_revision": runs,
            "review_threshold_ratio": REVIEW_THRESHOLD_RATIO,
            "noise_floor_method": "max-relative-mad-to-median",
            "timing_is_gating": True,
        },
        "samples": samples,
        "statistics": statistics_projection,
        "review": review,
    }
    report["report_sha256"] = embedded_report_sha256(report)
    return validate_report(report, repo_root=repo_root)


def write_report(path: Path, report: Mapping[str, Any]) -> None:
    if path.exists() or path.is_symlink():
        raise NativeBuildTimingError(f"native timing report already exists: {path}")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(report, ensure_ascii=True, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def _format_seconds(nanoseconds: int) -> str:
    return f"{nanoseconds / 1_000_000_000:.3f}s"


def print_summary(report: Mapping[str, Any]) -> None:
    statistics_projection = report["statistics"]
    print(
        "ffi-native-build-timing "
        f"baseline={_format_seconds(statistics_projection['baseline_median_ns'])} "
        f"candidate={_format_seconds(statistics_projection['candidate_median_ns'])} "
        f"regression={statistics_projection['median_regression_ratio']:+.2%} "
        f"noise_floor={statistics_projection['noise_floor_ratio']:.2%} "
        f"review_required={str(statistics_projection['review_required']).lower()}"
    )


def parse_args(argv: Sequence[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    capture = subparsers.add_parser("capture", help="capture matched clean-build evidence")
    capture.add_argument("--candidate-revision", default="HEAD")
    capture.add_argument("--runs", type=int, default=MINIMUM_RUNS)
    capture.add_argument("--review-reason")
    capture.add_argument("--output", type=Path, default=DEFAULT_REPORT)
    verify = subparsers.add_parser("verify", help="validate checked-in timing evidence")
    verify.add_argument("--report", type=Path, default=DEFAULT_REPORT)
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(sys.argv[1:] if argv is None else argv)
    try:
        if args.command == "capture":
            report = capture_report(
                repo_root=REPO_ROOT,
                candidate_revision=args.candidate_revision,
                runs=args.runs,
                review_reason=args.review_reason,
            )
            write_report(args.output, report)
        else:
            report = load_report(args.report, repo_root=REPO_ROOT)
        print_summary(report)
        return 0
    except (NativeBuildTimingError, NativeArtifactSizeError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
