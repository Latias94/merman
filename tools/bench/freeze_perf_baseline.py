#!/usr/bin/env python3
"""Freeze and verify a reproducible Merman performance baseline manifest."""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import inspect
import json
import math
import os
import platform
import re
import subprocess
import sys
import tempfile
from collections.abc import Callable, Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Any, Protocol

from compare_mermaid_renderers import best_effort_cpu_model
from compare_self import cargo_prebuild_command
from native_memory import (
    MemoryContractError,
    suite_exit_code,
    validate_response,
    validate_sample_matrix,
)
from run_native_memory import DriverContractError, analyze_samples, memory_recipe


SCHEMA_VERSION = 1
MANIFEST_KIND = "merman-performance-baseline"
MEMORY_SCALES = (1, 2, 4, 10, 32, 100)
MIN_MEMORY_REPEATS = 5
MIN_BOOTSTRAP_RESAMPLES = 10_000
MAX_BOOTSTRAP_RESAMPLES = 100_000
DEFAULT_CORPUS = "tools/bench/corpus.json"
DEFAULT_LOCK = "Cargo.lock"
DEFAULT_LATENCY_LANE = "render-svg"
DEFAULT_OUTPUT = "docs/performance/perf-baseline-manifest.json"
DEFAULT_SOURCE_PATHS = (
    "crates/merman/benches/pipeline.rs",
    "crates/merman/benches/native_memory.rs",
    "crates/merman/benches/native_memory/allocator.rs",
    "tools/bench/compare_mermaid_renderers.py",
    "tools/bench/compare_self.py",
    "tools/bench/corpus_utils.py",
    "tools/bench/freeze_perf_baseline.py",
    "tools/bench/native_memory.py",
    "tools/bench/perf_runner.py",
    "tools/bench/run_native_memory.py",
)
DEFAULT_MANIFEST_PATHS = ("Cargo.toml", "crates/merman/Cargo.toml")

_SHA256_RE = re.compile(r"^[0-9a-f]{64}$")
_GIT_OBJECT_RE = re.compile(r"^(?:[0-9a-f]{40}|[0-9a-f]{64})$")
_MEMORY_METRICS = frozenset(
    {"allocation_count", "allocated_bytes", "peak_growth_bytes"}
)
_PROCESS_LIFECYCLES = frozenset({"fresh-process", "reused-process"})
_ENGINE_LIFECYCLES = frozenset(
    {"cold-engine", "reused-engine", "not-applicable"}
)

_MANIFEST_FIELDS = frozenset(
    {"schema_version", "kind", "frozen_at", "repository", "artifacts", "recipes", "host"}
)
_REPOSITORY_FIELDS = frozenset({"commit", "tree", "result_tree", "patch_stack"})
_FILE_FIELDS = frozenset({"path", "bytes", "sha256"})
_PATCH_FIELDS = frozenset({"order", "path", "bytes", "sha256"})
_FIXTURE_FIELDS = frozenset({"name", "path", "bytes", "sha256"})
_ARTIFACT_FIELDS = frozenset(
    {
        "source",
        "manifest",
        "lock",
        "corpus",
        "fixtures",
        "native_memory_owner_contract",
        "native_memory_report",
        "native_memory_executable",
    }
)
_RECIPE_FIELDS = frozenset({"latency_common_aa", "native_memory"})
_HOST_FIELDS = frozenset({"rustc", "cargo", "os", "cpu", "architecture"})
_LATENCY_RECIPE_FIELDS = frozenset(
    {
        "package",
        "bench",
        "features",
        "default_features",
        "locked",
        "profile",
        "target_dir",
        "toolchain",
        "target",
        "corpus",
        "suite",
        "lane_id",
        "selector",
        "public_operation",
        "process_lifecycle",
        "engine_lifecycle",
        "logical_operations_per_estimate",
        "transport",
        "preset",
        "sample_size",
        "warm_up_seconds",
        "measurement_seconds",
        "evidence_mode",
        "calibration_pairs",
        "max_pairs",
        "start_side",
        "relative_threshold_percent",
        "absolute_threshold_ns",
        "confidence_level",
        "bootstrap_seed",
        "bootstrap_resamples",
    }
)
_NATIVE_RECIPE_FIELDS = frozenset(
    {
        "package",
        "bench",
        "features",
        "default_features",
        "locked",
        "target_dir",
        "build_command",
        "build_environment",
        "requested_toolchain",
        "lane_id",
        "public_operation",
        "process_lifecycle",
        "engine_lifecycle",
        "logical_operations_per_estimate",
        "transport",
        "workload",
        "scales",
        "repeats",
        "seed",
        "bootstrap_resamples",
        "evidence_class",
        "candidate_admission",
        "subprocess_isolation",
        "pair_order",
    }
)
_CORPUS_FIELDS = frozenset(
    {"schema_version", "default_group", "suites", "lanes", "fixtures"}
)
_CORPUS_LANE_FIELDS = frozenset(
    {
        "id",
        "kind",
        "owner",
        "public_operation",
        "diagnostic_stage",
        "parent_public_lane",
        "process_lifecycle",
        "engine_lifecycle",
        "logical_operations_per_estimate",
        "transport",
        "required_features",
        "selector",
        "history_aliases",
        "size_vector",
        "workload",
        "evidence_contract",
        "measurement_metrics",
        "semantic_output_dimensions",
    }
)
_CORPUS_FIXTURE_FIELDS = frozenset(
    {"name", "family", "size", "category", "source", "suites", "features", "quality"}
)
_REPORT_FIELDS = frozenset(
    {
        "schema_version",
        "generated_at",
        "outcome",
        "exit_code",
        "output",
        "method",
        "environment",
        "contract_errors",
        "schedule",
        "analysis",
        "lane",
        "inputs",
        "recipe",
        "executable",
        "run_id",
        "source",
        "candidate_admission",
    }
)
_REPORT_METHOD_FIELDS = frozenset(
    {
        "scales",
        "repeats",
        "seed",
        "bootstrap_resamples",
        "subprocess_isolation",
        "pair_order",
        "evidence_class",
    }
)
_REPORT_ENVIRONMENT_FIELDS = frozenset(
    {"os", "machine", "processor", "cpu", "python", "rustc", "cargo"}
)
_REPORT_SOURCE_FIELDS = frozenset(
    {"commit", "tree", "clean", "dirty_status_sha256", "dirty_disposition"}
)
_REPORT_LANE_FIELDS = frozenset(
    {
        "id",
        "public_operation",
        "process_lifecycle",
        "engine_lifecycle",
        "logical_operations_per_estimate",
        "transport",
        "workload",
        "size_vector",
        "measurement_metrics",
        "semantic_output_dimensions",
    }
)
_REPORT_INPUT_FIELDS = frozenset(
    {
        "workspace_manifest",
        "package_manifest",
        "cargo_lock",
        "corpus",
        "owner_contract",
    }
)
_REPORT_OWNER_CONTRACT_FIELDS = frozenset({"path", "bytes", "sha256", "value"})
_REPORT_RECIPE_FIELDS = frozenset(
    {
        "package",
        "bench",
        "features",
        "default_features",
        "locked",
        "target_dir",
        "build_command",
        "build_environment",
        "requested_toolchain",
    }
)
_BUILD_ENVIRONMENT_FIELDS = frozenset(
    {
        "CARGO_BUILD_JOBS",
        "CARGO_INCREMENTAL",
        "CARGO_PROFILE_BENCH_DEBUG",
        "CARGO_PROFILE_BENCH_LTO",
        "CARGO_PROFILE_BENCH_CODEGEN_UNITS",
        "CARGO_PROFILE_BENCH_OPT_LEVEL",
        "RUSTFLAGS",
        "CARGO_ENCODED_RUSTFLAGS",
        "RUSTUP_TOOLCHAIN",
        "RUSTC_WRAPPER",
        "RUSTC_WORKSPACE_WRAPPER",
    }
)
_REPORT_EXECUTABLE_FIELDS = frozenset({"path", "bytes", "sha256", "build"})
_REPORT_SCHEDULE_FIELDS = frozenset({"pair_index", "position", "request", "response"})
_REPORT_REQUEST_FIELDS = frozenset(
    {"schema_version", "lane_id", "mode", "scale", "seed", "repeat", "invocation_id", "nonce"}
)
_REPORT_ANALYSIS_FIELDS = frozenset({"matrix", "metrics"})
_REPORT_MATRIX_FIELDS = frozenset(
    {
        "complete",
        "scales",
        "repeat_count_by_scale",
        "incomplete_reasons",
        "pair_count",
        "lane_id",
        "executable_sha256",
    }
)
_OWNER_CONTRACT_FIELDS = frozenset(
    {
        "schema_version",
        "lane_id",
        "workload",
        "evidence_class",
        "candidate_admission",
        "generator",
        "metrics",
    }
)


class ManifestError(ValueError):
    """A baseline manifest or one of its evidence inputs is untrustworthy."""


@dataclass(frozen=True)
class GitSnapshot:
    commit: str
    tree: str
    status: tuple[str, ...]

    @property
    def clean(self) -> bool:
        return not self.status


class GitProbe(Protocol):
    def snapshot(self) -> GitSnapshot: ...

    def is_tracked(self, path: str, commit: str) -> bool: ...


class SubprocessGitProbe:
    def __init__(self, root: Path) -> None:
        self.root = root

    def _run(self, arguments: Sequence[str]) -> str:
        try:
            result = subprocess.run(
                ["git", "-C", str(self.root), *arguments],
                capture_output=True,
                text=True,
                timeout=30,
                check=False,
            )
        except (OSError, subprocess.TimeoutExpired) as error:
            raise ManifestError(f"git inspection failed: {error}") from error
        if result.returncode != 0:
            detail = result.stderr.strip() or result.stdout.strip() or "unknown git error"
            raise ManifestError(f"git inspection failed: {detail}")
        return result.stdout

    def snapshot(self) -> GitSnapshot:
        top_level = Path(self._run(["rev-parse", "--show-toplevel"]).strip()).resolve()
        if top_level != self.root:
            raise ManifestError(
                f"repository root mismatch: expected {self.root}, git reported {top_level}"
            )
        commit = self._run(["rev-parse", "--verify", "HEAD^{commit}"]).strip()
        tree = self._run(["rev-parse", "--verify", "HEAD^{tree}"]).strip()
        raw_status = self._run(
            ["status", "--porcelain=v1", "-z", "--untracked-files=all"]
        )
        status = tuple(entry for entry in raw_status.split("\0") if entry)
        snapshot = GitSnapshot(commit=commit, tree=tree, status=status)
        _validate_git_snapshot(snapshot)
        return snapshot

    def is_tracked(self, path: str, commit: str) -> bool:
        try:
            result = subprocess.run(
                ["git", "-C", str(self.root), "cat-file", "-e", f"{commit}:{path}"],
                capture_output=True,
                text=True,
                timeout=30,
                check=False,
            )
        except (OSError, subprocess.TimeoutExpired) as error:
            raise ManifestError(f"git tracked-file inspection failed: {error}") from error
        return result.returncode == 0


def repo_root() -> Path:
    return Path(__file__).resolve().parents[2]


def _reject_duplicate_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise ManifestError(f"duplicate JSON key: {key}")
        result[key] = value
    return result


def _strict_float(token: str) -> float:
    value = float(token)
    if not math.isfinite(value):
        raise ManifestError(f"non-finite JSON number: {token}")
    return value


def _reject_constant(token: str) -> None:
    raise ManifestError(f"non-finite JSON number: {token}")


def load_strict_json(path: Path) -> Any:
    """Load UTF-8 JSON while rejecting duplicate keys and every non-finite number."""

    try:
        text = path.read_text(encoding="utf-8")
        return json.loads(
            text,
            object_pairs_hook=_reject_duplicate_keys,
            parse_float=_strict_float,
            parse_constant=_reject_constant,
        )
    except ManifestError:
        raise
    except (OSError, UnicodeError, json.JSONDecodeError, OverflowError, ValueError) as error:
        raise ManifestError(f"cannot read strict JSON {path}: {error}") from error


def _json_normalize(value: object) -> object:
    try:
        return json.loads(json.dumps(value, allow_nan=False, sort_keys=True))
    except (TypeError, ValueError) as error:
        raise ManifestError(f"evidence cannot be represented as strict JSON: {error}") from error


def _object(value: object, context: str) -> Mapping[str, object]:
    if not isinstance(value, Mapping):
        raise ManifestError(f"{context} must be an object")
    return value


def _list(value: object, context: str) -> list[object]:
    if not isinstance(value, list):
        raise ManifestError(f"{context} must be a list")
    return value


def _fields(value: Mapping[str, object], expected: frozenset[str], context: str) -> None:
    actual = frozenset(value)
    if actual != expected:
        missing = sorted(expected - actual)
        unknown = sorted(actual - expected)
        raise ManifestError(
            f"{context} fields differ: missing={missing}, unknown={unknown}"
        )


def _string(value: object, context: str, *, unavailable_ok: bool = True) -> str:
    if not isinstance(value, str) or not value or value != value.strip():
        raise ManifestError(f"{context} must be a trimmed non-empty string")
    if "\x00" in value:
        raise ManifestError(f"{context} must not contain NUL")
    if not unavailable_ok and value.lower() == "unavailable":
        raise ManifestError(f"{context} must record an available value")
    return value


def _optional_string(value: object, context: str) -> str | None:
    if value is None:
        return None
    return _string(value, context)


def _integer(
    value: object,
    context: str,
    *,
    minimum: int | None = None,
    maximum: int | None = None,
) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        raise ManifestError(f"{context} must be an integer")
    if minimum is not None and value < minimum:
        raise ManifestError(f"{context} must be >= {minimum}")
    if maximum is not None and value > maximum:
        raise ManifestError(f"{context} must be <= {maximum}")
    return value


def _number(value: object, context: str, *, positive: bool = False) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ManifestError(f"{context} must be numeric")
    number = float(value)
    if not math.isfinite(number):
        raise ManifestError(f"{context} must be finite")
    if positive and number <= 0.0:
        raise ManifestError(f"{context} must be positive")
    return number


def _boolean(value: object, context: str) -> bool:
    if not isinstance(value, bool):
        raise ManifestError(f"{context} must be a boolean")
    return value


def _sha256(value: object, context: str) -> str:
    if not isinstance(value, str) or _SHA256_RE.fullmatch(value) is None:
        raise ManifestError(f"{context} must be 64-character lowercase hexadecimal")
    return value


def _git_object(value: object, context: str) -> str:
    if not isinstance(value, str) or _GIT_OBJECT_RE.fullmatch(value) is None:
        raise ManifestError(f"{context} must be a lowercase Git object id")
    return value


def _string_list(
    value: object,
    context: str,
    *,
    allow_empty: bool,
) -> list[str]:
    entries = _list(value, context)
    result = [_string(entry, f"{context}[{index}]") for index, entry in enumerate(entries)]
    if not allow_empty and not result:
        raise ManifestError(f"{context} must not be empty")
    if len(result) != len(set(result)):
        raise ManifestError(f"{context} contains duplicate values")
    return result


def _int_list(
    value: object,
    context: str,
    *,
    allow_empty: bool,
    minimum: int = 1,
) -> list[int]:
    entries = _list(value, context)
    result = [
        _integer(entry, f"{context}[{index}]", minimum=minimum)
        for index, entry in enumerate(entries)
    ]
    if not allow_empty and not result:
        raise ManifestError(f"{context} must not be empty")
    if len(result) != len(set(result)):
        raise ManifestError(f"{context} contains duplicate values")
    return result


def _validate_timestamp(value: object, context: str) -> str:
    timestamp = _string(value, context)
    try:
        parsed = dt.datetime.fromisoformat(timestamp.replace("Z", "+00:00"))
    except ValueError as error:
        raise ManifestError(f"{context} must be an ISO-8601 timestamp") from error
    if parsed.utcoffset() is None:
        raise ManifestError(f"{context} must include a timezone offset")
    return timestamp


def _validate_path(value: object, context: str, *, allow_absolute: bool) -> str:
    path = _string(value, context)
    if "\\" in path or "\n" in path or "\r" in path:
        raise ManifestError(f"{context} must use a canonical slash-separated path")
    native = Path(path)
    if native.is_absolute():
        if not allow_absolute:
            raise ManifestError(f"{context} must be repository-relative")
        return native.as_posix()
    pure = PurePosixPath(path)
    if path != pure.as_posix() or path in (".", "") or any(part in (".", "..") for part in pure.parts):
        raise ManifestError(f"{context} must be a canonical repository-relative path")
    return path


def _validate_file_record(
    value: object,
    context: str,
    *,
    allow_absolute: bool,
) -> Mapping[str, object]:
    record = _object(value, context)
    _fields(record, _FILE_FIELDS, context)
    _validate_path(record["path"], f"{context}.path", allow_absolute=allow_absolute)
    _integer(record["bytes"], f"{context}.bytes", minimum=0)
    _sha256(record["sha256"], f"{context}.sha256")
    return record


def _validate_repository(value: object) -> None:
    repository = _object(value, "repository")
    _fields(repository, _REPOSITORY_FIELDS, "repository")
    _git_object(repository["commit"], "repository.commit")
    _git_object(repository["tree"], "repository.tree")
    _git_object(repository["result_tree"], "repository.result_tree")
    patches = _list(repository["patch_stack"], "repository.patch_stack")
    seen_paths: set[str] = set()
    for index, raw_patch in enumerate(patches):
        context = f"repository.patch_stack[{index}]"
        patch = _object(raw_patch, context)
        _fields(patch, _PATCH_FIELDS, context)
        order = _integer(patch["order"], f"{context}.order", minimum=1)
        if order != index + 1:
            raise ManifestError(f"{context}.order must be {index + 1}")
        path = _validate_path(patch["path"], f"{context}.path", allow_absolute=False)
        if path in seen_paths:
            raise ManifestError("repository.patch_stack contains duplicate paths")
        seen_paths.add(path)
        _integer(patch["bytes"], f"{context}.bytes", minimum=0)
        _sha256(patch["sha256"], f"{context}.sha256")


def _validate_artifacts(value: object) -> None:
    artifacts = _object(value, "artifacts")
    _fields(artifacts, _ARTIFACT_FIELDS, "artifacts")
    repository_paths: set[str] = set()
    for field in ("source", "manifest"):
        entries = _list(artifacts[field], f"artifacts.{field}")
        if not entries:
            raise ManifestError(f"artifacts.{field} must not be empty")
        for index, entry in enumerate(entries):
            context = f"artifacts.{field}[{index}]"
            record = _validate_file_record(entry, context, allow_absolute=False)
            path = str(record["path"])
            if path in repository_paths:
                raise ManifestError(f"duplicate repository artifact path: {path}")
            repository_paths.add(path)

    for field in ("lock", "corpus", "native_memory_owner_contract"):
        record = _validate_file_record(
            artifacts[field], f"artifacts.{field}", allow_absolute=False
        )
        path = str(record["path"])
        if path in repository_paths:
            raise ManifestError(f"duplicate repository artifact path: {path}")
        repository_paths.add(path)

    fixtures = _list(artifacts["fixtures"], "artifacts.fixtures")
    if not fixtures:
        raise ManifestError("artifacts.fixtures must not be empty")
    names: set[str] = set()
    paths: set[str] = set()
    for index, raw_fixture in enumerate(fixtures):
        context = f"artifacts.fixtures[{index}]"
        fixture = _object(raw_fixture, context)
        _fields(fixture, _FIXTURE_FIELDS, context)
        name = _string(fixture["name"], f"{context}.name")
        path = _validate_path(fixture["path"], f"{context}.path", allow_absolute=False)
        if name in names or path in paths:
            raise ManifestError("artifacts.fixtures contains duplicate name or path")
        names.add(name)
        paths.add(path)
        _integer(fixture["bytes"], f"{context}.bytes", minimum=0)
        _sha256(fixture["sha256"], f"{context}.sha256")

    report = _validate_file_record(
        artifacts["native_memory_report"],
        "artifacts.native_memory_report",
        allow_absolute=True,
    )
    executable = _validate_file_record(
        artifacts["native_memory_executable"],
        "artifacts.native_memory_executable",
        allow_absolute=True,
    )
    if report["path"] == executable["path"]:
        raise ManifestError("native-memory report and executable paths must differ")


def _validate_latency_recipe(value: object) -> None:
    recipe = _object(value, "recipes.latency_common_aa")
    _fields(recipe, _LATENCY_RECIPE_FIELDS, "recipes.latency_common_aa")
    for field in ("package", "bench", "profile", "corpus", "suite", "lane_id", "selector", "public_operation", "transport", "preset", "evidence_mode", "start_side"):
        _string(recipe[field], f"recipes.latency_common_aa.{field}")
    _string_list(recipe["features"], "recipes.latency_common_aa.features", allow_empty=False)
    _boolean(recipe["default_features"], "recipes.latency_common_aa.default_features")
    if _boolean(recipe["locked"], "recipes.latency_common_aa.locked") is not True:
        raise ManifestError("latency recipe must use Cargo --locked")
    _validate_path(recipe["target_dir"], "recipes.latency_common_aa.target_dir", allow_absolute=False)
    _optional_string(recipe["toolchain"], "recipes.latency_common_aa.toolchain")
    _optional_string(recipe["target"], "recipes.latency_common_aa.target")
    process = _string(
        recipe["process_lifecycle"], "recipes.latency_common_aa.process_lifecycle"
    )
    engine = _string(
        recipe["engine_lifecycle"], "recipes.latency_common_aa.engine_lifecycle"
    )
    if process not in _PROCESS_LIFECYCLES or engine not in _ENGINE_LIFECYCLES:
        raise ManifestError("latency recipe uses an unknown lifecycle")
    _integer(
        recipe["logical_operations_per_estimate"],
        "recipes.latency_common_aa.logical_operations_per_estimate",
        minimum=1,
    )
    if recipe["profile"] != "bench" or recipe["evidence_mode"] != "confirmation":
        raise ManifestError("latency common A/A recipe must be a bench confirmation recipe")
    if recipe["preset"] not in ("quick", "long"):
        raise ManifestError("latency recipe preset must be quick or long")
    _integer(recipe["sample_size"], "recipes.latency_common_aa.sample_size", minimum=10)
    _integer(recipe["warm_up_seconds"], "recipes.latency_common_aa.warm_up_seconds", minimum=1)
    _integer(
        recipe["measurement_seconds"],
        "recipes.latency_common_aa.measurement_seconds",
        minimum=1,
    )
    calibration = _integer(
        recipe["calibration_pairs"],
        "recipes.latency_common_aa.calibration_pairs",
        minimum=8,
    )
    maximum = _integer(
        recipe["max_pairs"], "recipes.latency_common_aa.max_pairs", minimum=calibration
    )
    if calibration % 2 or maximum % 2:
        raise ManifestError("latency A/A calibration and maximum pair counts must be even")
    if recipe["start_side"] not in ("base", "head"):
        raise ManifestError("latency recipe start_side must be base or head")
    _number(
        recipe["relative_threshold_percent"],
        "recipes.latency_common_aa.relative_threshold_percent",
        positive=True,
    )
    _number(
        recipe["absolute_threshold_ns"],
        "recipes.latency_common_aa.absolute_threshold_ns",
        positive=True,
    )
    confidence = _number(
        recipe["confidence_level"], "recipes.latency_common_aa.confidence_level"
    )
    if not math.isclose(confidence, 0.95, rel_tol=0.0, abs_tol=1e-12):
        raise ManifestError("latency common A/A confidence_level must be 0.95")
    _integer(
        recipe["bootstrap_seed"],
        "recipes.latency_common_aa.bootstrap_seed",
        minimum=0,
        maximum=2**64 - 1,
    )
    _integer(
        recipe["bootstrap_resamples"],
        "recipes.latency_common_aa.bootstrap_resamples",
        minimum=MIN_BOOTSTRAP_RESAMPLES,
        maximum=MAX_BOOTSTRAP_RESAMPLES,
    )


def _validate_native_recipe(value: object) -> None:
    recipe = _object(value, "recipes.native_memory")
    _fields(recipe, _NATIVE_RECIPE_FIELDS, "recipes.native_memory")
    for field in ("package", "bench", "lane_id", "public_operation", "transport", "workload", "subprocess_isolation", "pair_order"):
        _string(recipe[field], f"recipes.native_memory.{field}")
    _string_list(recipe["features"], "recipes.native_memory.features", allow_empty=False)
    _boolean(recipe["default_features"], "recipes.native_memory.default_features")
    if _boolean(recipe["locked"], "recipes.native_memory.locked") is not True:
        raise ManifestError("native-memory recipe must use Cargo --locked")
    _validate_path(recipe["target_dir"], "recipes.native_memory.target_dir", allow_absolute=True)
    command = _string_list(
        recipe["build_command"], "recipes.native_memory.build_command", allow_empty=False
    )
    if command[0] != "cargo" or "--locked" not in command or "--no-run" not in command:
        raise ManifestError("native-memory build command must be a locked Cargo prebuild")
    build_environment = _object(
        recipe["build_environment"], "recipes.native_memory.build_environment"
    )
    _fields(
        build_environment,
        _BUILD_ENVIRONMENT_FIELDS,
        "recipes.native_memory.build_environment",
    )
    for field, value in build_environment.items():
        if value is not None and not isinstance(value, str):
            raise ManifestError(
                f"recipes.native_memory.build_environment.{field} "
                "must be a string or null"
            )
    expected_overrides = {
        "CARGO_BUILD_JOBS": "1",
        "CARGO_INCREMENTAL": "0",
        "CARGO_PROFILE_BENCH_DEBUG": "0",
    }
    for field, expected in expected_overrides.items():
        if build_environment[field] != expected:
            raise ManifestError(
                f"native-memory build environment must fix {field}={expected}"
            )
    _optional_string(
        recipe["requested_toolchain"], "recipes.native_memory.requested_toolchain"
    )
    process = _string(
        recipe["process_lifecycle"], "recipes.native_memory.process_lifecycle"
    )
    engine = _string(recipe["engine_lifecycle"], "recipes.native_memory.engine_lifecycle")
    if process != "fresh-process" or engine not in _ENGINE_LIFECYCLES:
        raise ManifestError("native-memory recipe must use fresh-process isolation")
    _integer(
        recipe["logical_operations_per_estimate"],
        "recipes.native_memory.logical_operations_per_estimate",
        minimum=1,
    )
    scales = _int_list(recipe["scales"], "recipes.native_memory.scales", allow_empty=False)
    if tuple(scales) != MEMORY_SCALES:
        raise ManifestError(f"native-memory scales must be {list(MEMORY_SCALES)}")
    _integer(
        recipe["repeats"],
        "recipes.native_memory.repeats",
        minimum=MIN_MEMORY_REPEATS,
        maximum=2**32,
    )
    _integer(
        recipe["seed"],
        "recipes.native_memory.seed",
        minimum=0,
        maximum=2**64 - 1,
    )
    _integer(
        recipe["bootstrap_resamples"],
        "recipes.native_memory.bootstrap_resamples",
        minimum=MIN_BOOTSTRAP_RESAMPLES,
        maximum=MAX_BOOTSTRAP_RESAMPLES,
    )
    evidence_class = _string(
        recipe["evidence_class"], "recipes.native_memory.evidence_class"
    )
    candidate_admission = _boolean(
        recipe["candidate_admission"], "recipes.native_memory.candidate_admission"
    )
    valid_evidence_identity = (
        evidence_class == "infrastructure-smoke" and candidate_admission is False
    ) or (evidence_class == "candidate-bound" and candidate_admission is True)
    if not valid_evidence_identity:
        raise ManifestError(
            "native-memory evidence class and candidate-admission flag disagree"
        )
    if recipe["subprocess_isolation"] != "fresh-process-per-sample":
        raise ManifestError("native-memory subprocess isolation differs from the contract")
    if recipe["pair_order"] != "alternating-operation-zero":
        raise ManifestError("native-memory pair order differs from the contract")


def _validate_recipes(value: object) -> None:
    recipes = _object(value, "recipes")
    _fields(recipes, _RECIPE_FIELDS, "recipes")
    _validate_latency_recipe(recipes["latency_common_aa"])
    _validate_native_recipe(recipes["native_memory"])


def _validate_host(value: object) -> None:
    host = _object(value, "host")
    _fields(host, _HOST_FIELDS, "host")
    for field in sorted(_HOST_FIELDS):
        _string(host[field], f"host.{field}", unavailable_ok=False)


def validate_manifest(value: object) -> None:
    """Validate the complete versioned manifest schema without consulting the filesystem."""

    manifest = _object(value, "manifest")
    _fields(manifest, _MANIFEST_FIELDS, "manifest")
    if _integer(manifest["schema_version"], "schema_version", minimum=1) != SCHEMA_VERSION:
        raise ManifestError(f"unsupported schema_version: {manifest['schema_version']}")
    if manifest["kind"] != MANIFEST_KIND:
        raise ManifestError(f"kind must be {MANIFEST_KIND!r}")
    _validate_timestamp(manifest["frozen_at"], "frozen_at")
    _validate_repository(manifest["repository"])
    _validate_artifacts(manifest["artifacts"])
    _validate_recipes(manifest["recipes"])
    _validate_host(manifest["host"])

    artifacts = _object(manifest["artifacts"], "artifacts")
    recipes = _object(manifest["recipes"], "recipes")
    latency = _object(recipes["latency_common_aa"], "recipes.latency_common_aa")
    native = _object(recipes["native_memory"], "recipes.native_memory")
    corpus = _object(artifacts["corpus"], "artifacts.corpus")
    if latency["corpus"] != corpus["path"]:
        raise ManifestError("latency recipe corpus differs from the frozen corpus artifact")
    if native["lane_id"] == latency["lane_id"]:
        raise ManifestError("latency and native-memory recipes must use distinct lanes")


def _validate_git_snapshot(snapshot: GitSnapshot) -> None:
    _git_object(snapshot.commit, "git.commit")
    _git_object(snapshot.tree, "git.tree")
    if not isinstance(snapshot.status, tuple) or any(
        not isinstance(entry, str) or not entry for entry in snapshot.status
    ):
        raise ManifestError("git status must be a tuple of non-empty porcelain entries")


def _require_clean(snapshot: GitSnapshot, *, action: str) -> None:
    _validate_git_snapshot(snapshot)
    if not snapshot.clean:
        preview = ", ".join(snapshot.status[:4])
        suffix = "" if len(snapshot.status) <= 4 else ", ..."
        raise ManifestError(
            f"{action} requires a clean committed tree; dirty entries: {preview}{suffix}"
        )


def _resolve_root(root: Path) -> Path:
    try:
        resolved = root.resolve(strict=True)
    except OSError as error:
        raise ManifestError(f"repository root is unavailable: {root}: {error}") from error
    if not resolved.is_dir():
        raise ManifestError(f"repository root is not a directory: {resolved}")
    return resolved


def _resolve_repo_file(root: Path, value: str, context: str) -> tuple[Path, str]:
    display = _validate_path(value, context, allow_absolute=False)
    unresolved = root / PurePosixPath(display)
    if unresolved.is_symlink():
        raise ManifestError(f"{context} must not be a symbolic link: {display}")
    try:
        resolved = unresolved.resolve(strict=True)
        resolved.relative_to(root)
    except (OSError, ValueError) as error:
        raise ManifestError(f"{context} escapes or is missing from the repository: {display}") from error
    if not resolved.is_file():
        raise ManifestError(f"{context} is not a regular file: {display}")
    return resolved, display


def _resolve_artifact_file(root: Path, value: str, context: str) -> tuple[Path, str]:
    display = _validate_path(value, context, allow_absolute=True)
    unresolved = Path(display) if Path(display).is_absolute() else root / PurePosixPath(display)
    if unresolved.is_symlink():
        raise ManifestError(f"{context} must not be a symbolic link: {display}")
    try:
        resolved = unresolved.resolve(strict=True)
    except OSError as error:
        raise ManifestError(f"{context} is missing: {display}") from error
    if not resolved.is_file():
        raise ManifestError(f"{context} is not a regular file: {display}")
    return resolved, display


def _display_path(root: Path, path: Path) -> str:
    resolved = path.resolve(strict=True)
    try:
        return resolved.relative_to(root).as_posix()
    except ValueError:
        return resolved.as_posix()


def _hash_file(path: Path, context: str) -> tuple[int, str]:
    digest = hashlib.sha256()
    count = 0
    try:
        with path.open("rb") as source:
            before = os.fstat(source.fileno())
            for chunk in iter(lambda: source.read(1024 * 1024), b""):
                digest.update(chunk)
                count += len(chunk)
            after = os.fstat(source.fileno())
    except OSError as error:
        raise ManifestError(f"cannot hash {context} at {path}: {error}") from error
    identity_before = (before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns)
    identity_after = (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns)
    if identity_before != identity_after or count != after.st_size:
        raise ManifestError(f"{context} changed while it was being hashed: {path}")
    return count, digest.hexdigest()


def _record_file(root: Path, path: Path, context: str) -> dict[str, object]:
    resolved = path.resolve(strict=True)
    size, digest = _hash_file(resolved, context)
    return {"path": _display_path(root, resolved), "bytes": size, "sha256": digest}


def _record_repo_file(root: Path, value: str, context: str) -> dict[str, object]:
    path, display = _resolve_repo_file(root, value, context)
    size, digest = _hash_file(path, context)
    return {"path": display, "bytes": size, "sha256": digest}


def _verify_record(
    root: Path,
    value: object,
    context: str,
    *,
    repository_only: bool,
) -> Path:
    record = _validate_file_record(value, context, allow_absolute=not repository_only)
    if repository_only:
        path, _display = _resolve_repo_file(root, str(record["path"]), f"{context}.path")
    else:
        path, _display = _resolve_artifact_file(
            root, str(record["path"]), f"{context}.path"
        )
    size, digest = _hash_file(path, context)
    if size != record["bytes"]:
        raise ManifestError(
            f"{context} bytes mismatch: expected {record['bytes']}, observed {size}"
        )
    if digest != record["sha256"]:
        raise ManifestError(
            f"{context} sha256 mismatch: expected {record['sha256']}, observed {digest}"
        )
    return path


def _validate_corpus(value: object) -> Mapping[str, object]:
    corpus = _object(value, "corpus")
    _fields(corpus, _CORPUS_FIELDS, "corpus")
    if _integer(corpus["schema_version"], "corpus.schema_version", minimum=1) != 2:
        raise ManifestError("performance baseline requires corpus schema_version 2")
    _string(corpus["default_group"], "corpus.default_group")
    suites = _object(corpus["suites"], "corpus.suites")
    if not suites:
        raise ManifestError("corpus.suites must not be empty")
    for key, description in suites.items():
        _string(key, "corpus suite name")
        _string(description, f"corpus.suites.{key}")

    lanes = _list(corpus["lanes"], "corpus.lanes")
    if not lanes:
        raise ManifestError("corpus.lanes must not be empty")
    lane_ids: set[str] = set()
    selectors: set[str] = set()
    for index, raw_lane in enumerate(lanes):
        context = f"corpus.lanes[{index}]"
        lane = _object(raw_lane, context)
        _fields(lane, _CORPUS_LANE_FIELDS, context)
        lane_id = _string(lane["id"], f"{context}.id")
        if lane_id in lane_ids:
            raise ManifestError(f"duplicate corpus lane id: {lane_id}")
        lane_ids.add(lane_id)
        for field in ("kind", "owner", "public_operation", "process_lifecycle", "engine_lifecycle", "transport", "selector", "workload"):
            _string(lane[field], f"{context}.{field}")
        _optional_string(lane["diagnostic_stage"], f"{context}.diagnostic_stage")
        _optional_string(lane["parent_public_lane"], f"{context}.parent_public_lane")
        _optional_string(lane["evidence_contract"], f"{context}.evidence_contract")
        if lane["process_lifecycle"] not in _PROCESS_LIFECYCLES:
            raise ManifestError(f"{context}.process_lifecycle is unknown")
        if lane["engine_lifecycle"] not in _ENGINE_LIFECYCLES:
            raise ManifestError(f"{context}.engine_lifecycle is unknown")
        _integer(
            lane["logical_operations_per_estimate"],
            f"{context}.logical_operations_per_estimate",
            minimum=1,
        )
        _string_list(lane["required_features"], f"{context}.required_features", allow_empty=True)
        history = _string_list(lane["history_aliases"], f"{context}.history_aliases", allow_empty=True)
        selector_values = [str(lane["selector"]), *history]
        if any(selector in selectors for selector in selector_values):
            raise ManifestError(f"{context} reuses a lane selector or history alias")
        selectors.update(selector_values)
        _int_list(lane["size_vector"], f"{context}.size_vector", allow_empty=True)
        _string_list(
            lane["measurement_metrics"], f"{context}.measurement_metrics", allow_empty=False
        )
        _string_list(
            lane["semantic_output_dimensions"],
            f"{context}.semantic_output_dimensions",
            allow_empty=False,
        )

    fixtures = _list(corpus["fixtures"], "corpus.fixtures")
    if not fixtures:
        raise ManifestError("corpus.fixtures must not be empty")
    names: set[str] = set()
    sources: set[str] = set()
    for index, raw_fixture in enumerate(fixtures):
        context = f"corpus.fixtures[{index}]"
        fixture = _object(raw_fixture, context)
        _fields(fixture, _CORPUS_FIXTURE_FIELDS, context)
        for field in ("name", "family", "size", "category"):
            _string(fixture[field], f"{context}.{field}")
        name = str(fixture["name"])
        source = _validate_path(fixture["source"], f"{context}.source", allow_absolute=False)
        if name in names or source in sources:
            raise ManifestError("corpus fixtures contain duplicate names or sources")
        names.add(name)
        sources.add(source)
        _string_list(fixture["suites"], f"{context}.suites", allow_empty=True)
        _string_list(fixture["features"], f"{context}.features", allow_empty=True)
        _string_list(fixture["quality"], f"{context}.quality", allow_empty=True)
    return corpus


def _load_corpus(path: Path) -> Mapping[str, object]:
    return _validate_corpus(load_strict_json(path))


def _find_lane(corpus: Mapping[str, object], lane_id: str) -> Mapping[str, object]:
    matches = [
        _object(raw, "corpus lane")
        for raw in _list(corpus["lanes"], "corpus.lanes")
        if isinstance(raw, Mapping) and raw.get("id") == lane_id
    ]
    if len(matches) != 1:
        raise ManifestError(f"corpus lane {lane_id!r} resolved to {len(matches)} entries")
    return matches[0]


def _fixture_records(root: Path, corpus: Mapping[str, object]) -> list[dict[str, object]]:
    records: list[dict[str, object]] = []
    for index, raw_fixture in enumerate(_list(corpus["fixtures"], "corpus.fixtures")):
        fixture = _object(raw_fixture, f"corpus.fixtures[{index}]")
        source = str(fixture["source"])
        record = _record_repo_file(root, source, f"corpus fixture {fixture['name']}")
        records.append({"name": fixture["name"], **record})
    return records


def _validate_report_file_record(value: object, context: str) -> Mapping[str, object]:
    return _validate_file_record(value, context, allow_absolute=True)


def _same_resolved_path(root: Path, first: object, second: Path, context: str) -> None:
    first_path, _ = _resolve_artifact_file(root, str(first), context)
    if first_path != second.resolve(strict=True):
        raise ManifestError(
            f"{context} path mismatch: report names {first_path}, expected {second.resolve(strict=True)}"
        )


def _record_matches_report_input(
    root: Path,
    input_record: Mapping[str, object],
    expected_path: Path,
    expected_record: Mapping[str, object],
    context: str,
) -> None:
    _same_resolved_path(root, input_record["path"], expected_path, f"{context}.path")
    if input_record["bytes"] != expected_record["bytes"]:
        raise ManifestError(f"{context}.bytes differs from the frozen artifact")
    if input_record["sha256"] != expected_record["sha256"]:
        raise ManifestError(f"{context}.sha256 differs from the frozen artifact")


def _validate_report_environment(
    environment: Mapping[str, object], host: Mapping[str, object] | None
) -> None:
    _fields(environment, _REPORT_ENVIRONMENT_FIELDS, "native-memory report.environment")
    for field in sorted(_REPORT_ENVIRONMENT_FIELDS):
        if field == "processor":
            if not isinstance(environment[field], str):
                raise ManifestError(
                    "native-memory report.environment.processor must be a string"
                )
            continue
        _string(
            environment[field],
            f"native-memory report.environment.{field}",
            unavailable_ok=field not in ("rustc", "cargo"),
        )
    if host is None:
        return
    expected = {
        "os": host["os"],
        "machine": host["architecture"],
        "cpu": host["cpu"],
        "rustc": host["rustc"],
        "cargo": host["cargo"],
    }
    for field, value in expected.items():
        if environment[field] != value:
            raise ManifestError(
                f"native-memory report host drift for {field}: "
                f"report={environment[field]!r}, current={value!r}"
            )


def _validate_report_source(
    value: object,
    *,
    expected_commit: str,
    expected_tree: str,
) -> None:
    source = _object(value, "native-memory report.source")
    _fields(source, _REPORT_SOURCE_FIELDS, "native-memory report.source")
    _git_object(source["commit"], "native-memory report.source.commit")
    _git_object(source["tree"], "native-memory report.source.tree")
    _boolean(source["clean"], "native-memory report.source.clean")
    _sha256(
        source["dirty_status_sha256"],
        "native-memory report.source.dirty_status_sha256",
    )
    _string(
        source["dirty_disposition"],
        "native-memory report.source.dirty_disposition",
    )
    if source["commit"] != expected_commit or source["tree"] != expected_tree:
        raise ManifestError(
            "native-memory report source commit/tree differs from the frozen Git snapshot"
        )
    if source["clean"] is not True or source["dirty_disposition"] != "clean":
        raise ManifestError("native-memory report was not collected from a clean source tree")
    if source["dirty_status_sha256"] != hashlib.sha256(b"").hexdigest():
        raise ManifestError("native-memory report clean status digest is not empty")


def _validate_owner_contract(value: object, lane: Mapping[str, object]) -> None:
    contract = _object(value, "native-memory owner contract")
    _fields(contract, _OWNER_CONTRACT_FIELDS, "native-memory owner contract")
    if contract["schema_version"] != 1:
        raise ManifestError("native-memory owner contract schema_version must be 1")
    if contract["lane_id"] != lane["id"] or contract["workload"] != lane["workload"]:
        raise ManifestError("native-memory owner contract identity differs from the corpus lane")
    evidence_class = contract["evidence_class"]
    candidate_admission = contract["candidate_admission"]
    valid_evidence_identity = (
        evidence_class == "infrastructure-smoke" and candidate_admission is False
    ) or (evidence_class == "candidate-bound" and candidate_admission is True)
    if not valid_evidence_identity:
        raise ManifestError(
            "native-memory owner evidence class and candidate-admission flag disagree"
        )
    generator = _object(contract["generator"], "native-memory owner contract.generator")
    _fields(
        generator,
        frozenset({"id", "nodes_per_scale", "edges_per_scale"}),
        "native-memory owner contract.generator",
    )
    if generator["id"] != lane["workload"]:
        raise ManifestError("native-memory generator id differs from the corpus workload")
    _integer(generator["nodes_per_scale"], "generator.nodes_per_scale", minimum=1)
    _integer(generator["edges_per_scale"], "generator.edges_per_scale", minimum=1)
    metrics = _object(contract["metrics"], "native-memory owner contract.metrics")
    if frozenset(metrics) != _MEMORY_METRICS:
        raise ManifestError("native-memory owner contract metrics differ")
    for metric, raw_bounds in metrics.items():
        bounds = _object(raw_bounds, f"native-memory owner contract.metrics.{metric}")
        _fields(
            bounds,
            frozenset({"slope_cap", "max_scale_cap"}),
            f"native-memory owner contract.metrics.{metric}",
        )
        _number(bounds["slope_cap"], f"{metric}.slope_cap", positive=True)
        _number(bounds["max_scale_cap"], f"{metric}.max_scale_cap", positive=True)


def _validate_report_schedule(
    report: Mapping[str, object],
    method: Mapping[str, object],
    lane: Mapping[str, object],
    executable_sha256: str,
) -> tuple[dict[str, object], list[Mapping[str, object]]]:
    schedule = _list(report["schedule"], "native-memory report.schedule")
    repeats = int(method["repeats"])
    expected_count = len(MEMORY_SCALES) * repeats * 2
    if len(schedule) != expected_count:
        raise ManifestError(
            f"native-memory report.schedule has {len(schedule)} entries; expected {expected_count}"
        )
    responses: list[Mapping[str, object]] = []
    entry_index = 0
    pair_index = 0
    for scale in MEMORY_SCALES:
        for repeat in range(repeats):
            modes = ("operation", "zero") if pair_index % 2 == 0 else ("zero", "operation")
            for position, mode in enumerate(modes):
                context = f"native-memory report.schedule[{entry_index}]"
                entry = _object(schedule[entry_index], context)
                _fields(entry, _REPORT_SCHEDULE_FIELDS, context)
                if entry["pair_index"] != pair_index or entry["position"] != position:
                    raise ManifestError(f"{context} does not preserve the preregistered order")
                request = _object(entry["request"], f"{context}.request")
                _fields(request, _REPORT_REQUEST_FIELDS, f"{context}.request")
                expected_request = {
                    "schema_version": 1,
                    "lane_id": lane["id"],
                    "mode": mode,
                    "scale": scale,
                    "seed": method["seed"],
                    "repeat": repeat,
                }
                for field, expected in expected_request.items():
                    if request[field] != expected:
                        raise ManifestError(
                            f"{context}.request.{field} differs: "
                            f"expected {expected!r}, got {request[field]!r}"
                        )
                _string(request["invocation_id"], f"{context}.request.invocation_id")
                _sha256(str(request["nonce"]) * 2, f"{context}.request.nonce-expanded")
                response = _object(entry["response"], f"{context}.response")
                for field in (
                    "schema_version",
                    "lane_id",
                    "mode",
                    "scale",
                    "seed",
                    "repeat",
                    "invocation_id",
                    "nonce",
                ):
                    if response.get(field) != request[field]:
                        raise ManifestError(f"{context}.response echo drift for {field}")
                lane_echo = {
                    "public_operation": lane["public_operation"],
                    "process_lifecycle": lane["process_lifecycle"],
                    "engine_lifecycle": lane["engine_lifecycle"],
                    "logical_operations_per_estimate": lane[
                        "logical_operations_per_estimate"
                    ],
                }
                for field, expected in lane_echo.items():
                    if response.get(field) != expected:
                        raise ManifestError(
                            f"{context}.response lane echo drift for {field}"
                        )
                if response.get("executable_sha256") != executable_sha256:
                    raise ManifestError(f"{context}.response executable digest drift")
                expected_echo = {
                    "lane_id": request["lane_id"],
                    **lane_echo,
                    "mode": request["mode"],
                    "scale": request["scale"],
                    "seed": request["seed"],
                    "repeat": request["repeat"],
                    "executable_sha256": executable_sha256,
                    "invocation_id": request["invocation_id"],
                    "nonce": request["nonce"],
                }
                try:
                    validated_response = validate_response(
                        json.dumps(response, separators=(",", ":"), allow_nan=False)
                        + "\n",
                        "",
                        expected=expected_echo,
                    )
                except (MemoryContractError, TypeError, ValueError) as error:
                    raise ManifestError(
                        f"{context}.response protocol is invalid: {error}"
                    ) from error
                responses.append(validated_response)
                entry_index += 1
            pair_index += 1
    try:
        matrix = validate_sample_matrix(responses)
    except MemoryContractError as error:
        raise ManifestError(f"native-memory report sample matrix is invalid: {error}") from error
    if not matrix["complete"]:
        reasons = "; ".join(str(value) for value in matrix["incomplete_reasons"])
        raise ManifestError(f"native-memory report sample matrix is incomplete: {reasons}")
    return matrix, responses


def _inspect_native_memory_report(
    root: Path,
    report_path: Path,
    *,
    corpus_path: Path,
    corpus_record: Mapping[str, object],
    corpus: Mapping[str, object],
    workspace_manifest_record: Mapping[str, object],
    package_manifest_record: Mapping[str, object],
    lock_record: Mapping[str, object],
    expected_commit: str,
    expected_tree: str,
    host: Mapping[str, object] | None,
    host_probe: Callable[..., Mapping[str, object]] | None = None,
) -> dict[str, object]:
    report = _object(load_strict_json(report_path), "native-memory report")
    _fields(report, _REPORT_FIELDS, "native-memory report")
    if report["schema_version"] != 1:
        raise ManifestError("native-memory report schema_version must be 1")
    _validate_timestamp(report["generated_at"], "native-memory report.generated_at")
    outcomes = {"pass": 0, "failed_bound": 1, "inconclusive": 3}
    if report["outcome"] not in outcomes or report["exit_code"] != outcomes.get(report["outcome"]):
        raise ManifestError("native-memory report is not a completed non-contract-failure report")
    if report["contract_errors"] != []:
        raise ManifestError("native-memory report contains contract errors")
    _boolean(
        report["candidate_admission"],
        "native-memory report.candidate_admission",
    )
    _validate_report_source(
        report["source"],
        expected_commit=expected_commit,
        expected_tree=expected_tree,
    )
    _same_resolved_path(root, report["output"], report_path, "native-memory report.output")
    _string(report["run_id"], "native-memory report.run_id")

    recipe = _object(report["recipe"], "native-memory report.recipe")
    _fields(recipe, _REPORT_RECIPE_FIELDS, "native-memory report.recipe")
    requested_toolchain = _optional_string(
        recipe["requested_toolchain"],
        "native-memory report.recipe.requested_toolchain",
    )
    if host is None:
        if host_probe is None:
            raise ManifestError("native-memory report verification requires a host probe")
        host = dict(_call_host_probe(host_probe, root, requested_toolchain))
        _validate_host(host)

    environment = _object(report["environment"], "native-memory report.environment")
    _validate_report_environment(environment, host)

    method = _object(report["method"], "native-memory report.method")
    _fields(method, _REPORT_METHOD_FIELDS, "native-memory report.method")
    scales = _int_list(method["scales"], "native-memory report.method.scales", allow_empty=False)
    if tuple(scales) != MEMORY_SCALES:
        raise ManifestError(f"native-memory report scales must be {list(MEMORY_SCALES)}")
    _integer(
        method["repeats"],
        "native-memory report.method.repeats",
        minimum=MIN_MEMORY_REPEATS,
        maximum=2**32,
    )
    _integer(
        method["seed"],
        "native-memory report.method.seed",
        minimum=0,
        maximum=2**64 - 1,
    )
    _integer(
        method["bootstrap_resamples"],
        "native-memory report.method.bootstrap_resamples",
        minimum=MIN_BOOTSTRAP_RESAMPLES,
        maximum=MAX_BOOTSTRAP_RESAMPLES,
    )
    if method["subprocess_isolation"] != "fresh-process-per-sample":
        raise ManifestError("native-memory report did not use fresh-process-per-sample")
    if method["pair_order"] != "alternating-operation-zero":
        raise ManifestError("native-memory report did not use alternating operation/zero order")
    evidence_class = _string(
        method["evidence_class"], "native-memory report.method.evidence_class"
    )
    if evidence_class not in ("infrastructure-smoke", "candidate-bound"):
        raise ManifestError("native-memory report evidence_class is not decision evidence")

    report_lane = _object(report["lane"], "native-memory report.lane")
    _fields(report_lane, _REPORT_LANE_FIELDS, "native-memory report.lane")
    lane = _find_lane(corpus, str(report_lane["id"]))
    lane_fields = (
        "id",
        "public_operation",
        "process_lifecycle",
        "engine_lifecycle",
        "logical_operations_per_estimate",
        "transport",
        "workload",
        "size_vector",
        "measurement_metrics",
        "semantic_output_dimensions",
    )
    for field in lane_fields:
        if report_lane[field] != lane[field]:
            raise ManifestError(f"native-memory report lane drift for {field}")
    if lane["process_lifecycle"] != "fresh-process":
        raise ManifestError("native-memory corpus lane must use fresh-process lifecycle")
    if lane["transport"] != "native-system-allocator-subprocess":
        raise ManifestError("native-memory corpus lane uses the wrong transport")
    if tuple(lane["size_vector"]) != MEMORY_SCALES:
        raise ManifestError("native-memory corpus lane uses the wrong size vector")
    if frozenset(lane["measurement_metrics"]) != _MEMORY_METRICS:
        raise ManifestError("native-memory corpus lane uses the wrong allocator metrics")

    inputs = _object(report["inputs"], "native-memory report.inputs")
    _fields(inputs, _REPORT_INPUT_FIELDS, "native-memory report.inputs")
    frozen_cargo_inputs = (
        ("workspace_manifest", workspace_manifest_record),
        ("package_manifest", package_manifest_record),
        ("cargo_lock", lock_record),
    )
    for field, expected_record in frozen_cargo_inputs:
        context = f"native-memory report.inputs.{field}"
        report_input = _validate_report_file_record(inputs[field], context)
        expected_path, _ = _resolve_repo_file(
            root, str(expected_record["path"]), f"{context}.expected_path"
        )
        _record_matches_report_input(
            root,
            report_input,
            expected_path,
            expected_record,
            context,
        )
    report_corpus = _validate_report_file_record(
        inputs["corpus"], "native-memory report.inputs.corpus"
    )
    _record_matches_report_input(
        root,
        report_corpus,
        corpus_path,
        corpus_record,
        "native-memory report.inputs.corpus",
    )
    owner_input = _object(
        inputs["owner_contract"], "native-memory report.inputs.owner_contract"
    )
    _fields(
        owner_input,
        _REPORT_OWNER_CONTRACT_FIELDS,
        "native-memory report.inputs.owner_contract",
    )
    owner_file_record = {
        field: owner_input[field] for field in ("path", "bytes", "sha256")
    }
    _validate_report_file_record(
        owner_file_record, "native-memory report.inputs.owner_contract"
    )
    owner_path, _ = _resolve_artifact_file(
        root,
        str(owner_input["path"]),
        "native-memory report.inputs.owner_contract.path",
    )
    owner_value = load_strict_json(owner_path)
    if owner_value != owner_input["value"]:
        raise ManifestError("native-memory report embedded owner contract differs from its file")
    _validate_owner_contract(owner_value, lane)
    if owner_value["evidence_class"] != evidence_class:
        raise ManifestError(
            "native-memory report evidence_class differs from the owner contract"
        )
    if report["candidate_admission"] is not bool(
        owner_value["candidate_admission"]
    ):
        raise ManifestError(
            "native-memory report candidate_admission differs from its clean owner contract"
        )
    owner_record = _record_file(root, owner_path, "native-memory owner contract")
    if owner_record["bytes"] != owner_input["bytes"] or owner_record["sha256"] != owner_input["sha256"]:
        raise ManifestError("native-memory owner contract digest differs from the report")
    evidence_contract = lane["evidence_contract"]
    if not isinstance(evidence_contract, str):
        raise ManifestError("native-memory corpus lane is missing its evidence contract path")
    expected_owner, _ = _resolve_repo_file(
        root, evidence_contract, "native-memory corpus lane evidence_contract"
    )
    if expected_owner != owner_path:
        raise ManifestError("native-memory report used a different owner contract")

    for field in ("package", "bench", "target_dir"):
        _string(recipe[field], f"native-memory report.recipe.{field}")
    _string_list(
        recipe["features"],
        "native-memory report.recipe.features",
        allow_empty=False,
    )
    _boolean(recipe["default_features"], "native-memory report.recipe.default_features")
    _boolean(recipe["locked"], "native-memory report.recipe.locked")
    target_dir = Path(str(recipe["target_dir"]))
    if not target_dir.is_absolute():
        raise ManifestError(
            "native-memory report recipe target_dir cannot form the canonical "
            "native-memory recipe: expected an absolute path"
        )
    try:
        resolved_target_dir = target_dir.resolve(strict=True)
    except OSError as error:
        raise ManifestError(
            "native-memory report recipe target_dir cannot form the canonical "
            f"native-memory recipe: {error}"
        ) from error
    if not resolved_target_dir.is_dir():
        raise ManifestError(
            "native-memory report recipe target_dir cannot form the canonical "
            "native-memory recipe: expected a directory"
        )

    canonical_recipe = memory_recipe(
        root,
        target_dir=target_dir,
        toolchain=requested_toolchain,
    )
    canonical_fields = {
        "package": canonical_recipe.package,
        "bench": canonical_recipe.bench,
        "features": list(canonical_recipe.features),
        "default_features": canonical_recipe.default_features,
        "locked": canonical_recipe.locked,
        "target_dir": str(canonical_recipe.target_dir),
        "requested_toolchain": canonical_recipe.toolchain,
    }
    for field, expected in canonical_fields.items():
        if recipe[field] != expected:
            raise ManifestError(
                f"native-memory report recipe {field} differs from the canonical "
                "native-memory recipe"
            )
    try:
        canonical_build_command = cargo_prebuild_command(canonical_recipe)
    except (OSError, ValueError) as error:
        raise ManifestError(
            f"cannot reconstruct canonical native-memory build command: {error}"
        ) from error
    build_command = _string_list(
        recipe["build_command"],
        "native-memory report.recipe.build_command",
        allow_empty=False,
    )
    if build_command != canonical_build_command:
        raise ManifestError(
            "native-memory report recipe build command differs from the canonical "
            "native-memory build command"
        )
    build_environment = _object(
        recipe["build_environment"], "native-memory report.recipe.build_environment"
    )
    _fields(
        build_environment,
        _BUILD_ENVIRONMENT_FIELDS,
        "native-memory report.recipe.build_environment",
    )
    for field, value in build_environment.items():
        if value is not None and not isinstance(value, str):
            raise ManifestError(
                f"native-memory report.recipe.build_environment.{field} "
                "must be a string or null"
            )
    expected_overrides = {
        "CARGO_BUILD_JOBS": "1",
        "CARGO_INCREMENTAL": "0",
        "CARGO_PROFILE_BENCH_DEBUG": "0",
    }
    for field, expected in expected_overrides.items():
        if build_environment[field] != expected:
            raise ManifestError(
                f"native-memory report build environment must fix {field}={expected}"
            )
    executable = _object(report["executable"], "native-memory report.executable")
    _fields(executable, _REPORT_EXECUTABLE_FIELDS, "native-memory report.executable")
    executable_file_record = {
        field: executable[field] for field in ("path", "bytes", "sha256")
    }
    _validate_report_file_record(
        executable_file_record, "native-memory report.executable"
    )
    build = _object(executable["build"], "native-memory report.executable.build")
    if frozenset(build) == frozenset({"skipped"}):
        _string(build["skipped"], "native-memory report.executable.build.skipped")
        raise ManifestError(
            "persistent baseline requires a Cargo-built executable; "
            "explicit executable build skipping is diagnostic-only"
        )
    elif frozenset(build) == frozenset({"command", "stdout_sha256", "stderr_sha256"}):
        executable_build_command = _string_list(
            build["command"],
            "native-memory report.executable.build.command",
            allow_empty=False,
        )
        if executable_build_command != canonical_build_command:
            raise ManifestError(
                "native-memory executable build command differs from the canonical "
                "native-memory build command"
            )
        _sha256(build["stdout_sha256"], "native-memory report.executable.build.stdout_sha256")
        _sha256(build["stderr_sha256"], "native-memory report.executable.build.stderr_sha256")
    else:
        raise ManifestError("native-memory executable build provenance fields differ")
    executable_path, _ = _resolve_artifact_file(
        root, str(executable["path"]), "native-memory report.executable.path"
    )
    try:
        executable_path.relative_to(resolved_target_dir)
    except ValueError as error:
        raise ManifestError(
            "native-memory executable is outside the canonical recipe target_dir"
        ) from error
    executable_record = _record_file(root, executable_path, "native-memory executable")
    if executable_record["bytes"] != executable["bytes"] or executable_record["sha256"] != executable["sha256"]:
        raise ManifestError("native-memory executable digest differs from the report")

    matrix, responses = _validate_report_schedule(
        report, method, lane, str(executable["sha256"])
    )
    analysis = _object(report["analysis"], "native-memory report.analysis")
    _fields(analysis, _REPORT_ANALYSIS_FIELDS, "native-memory report.analysis")
    reported_matrix = _object(
        analysis["matrix"], "native-memory report.analysis.matrix"
    )
    _fields(
        reported_matrix,
        _REPORT_MATRIX_FIELDS,
        "native-memory report.analysis.matrix",
    )
    if reported_matrix["complete"] is not True or matrix["complete"] is not True:
        raise ManifestError("native-memory report analysis matrix is not complete")
    try:
        expected_analysis, metric_outcomes = analyze_samples(
            responses,
            contract=owner_value,
            bootstrap_resamples=int(method["bootstrap_resamples"]),
            seed_material=(
                f"{lane['id']}:{method['seed']}:{method['repeats']}"
            ),
        )
    except (DriverContractError, MemoryContractError, TypeError, ValueError) as error:
        raise ManifestError(
            f"native-memory report analysis cannot be reproduced: {error}"
        ) from error
    if analysis != _json_normalize(expected_analysis):
        raise ManifestError(
            "native-memory report analysis differs from recomputed samples and bounds"
        )
    expected_exit = suite_exit_code(metric_outcomes)
    expected_outcome = (
        "failed_bound"
        if expected_exit == 1
        else "inconclusive" if expected_exit == 3 else "pass"
    )
    if report["exit_code"] != expected_exit or report["outcome"] != expected_outcome:
        raise ManifestError(
            "native-memory report outcome/exit_code differs from recomputed metric outcomes"
        )

    native_recipe = {
        "package": recipe["package"],
        "bench": recipe["bench"],
        "features": recipe["features"],
        "default_features": recipe["default_features"],
        "locked": recipe["locked"],
        "target_dir": recipe["target_dir"],
        "build_command": recipe["build_command"],
        "build_environment": recipe["build_environment"],
        "requested_toolchain": recipe["requested_toolchain"],
        "lane_id": lane["id"],
        "public_operation": lane["public_operation"],
        "process_lifecycle": lane["process_lifecycle"],
        "engine_lifecycle": lane["engine_lifecycle"],
        "logical_operations_per_estimate": lane["logical_operations_per_estimate"],
        "transport": lane["transport"],
        "workload": lane["workload"],
        "scales": list(method["scales"]),
        "repeats": method["repeats"],
        "seed": method["seed"],
        "bootstrap_resamples": method["bootstrap_resamples"],
        "evidence_class": method["evidence_class"],
        "candidate_admission": report["candidate_admission"],
        "subprocess_isolation": method["subprocess_isolation"],
        "pair_order": method["pair_order"],
    }
    _validate_native_recipe(native_recipe)
    return {
        "report_record": _record_file(root, report_path, "native-memory report"),
        "owner_contract_record": owner_record,
        "executable_record": executable_record,
        "recipe": native_recipe,
        "lane": lane,
        "host": host,
    }


def _default_latency_recipe(
    corpus_path: str, lane: Mapping[str, object]
) -> dict[str, object]:
    recipe = {
        "package": "merman",
        "bench": "pipeline",
        "features": ["svg"],
        "default_features": True,
        "locked": True,
        "profile": "bench",
        "target_dir": "target",
        "toolchain": None,
        "target": None,
        "corpus": corpus_path,
        "suite": "canary",
        "lane_id": lane["id"],
        "selector": lane["selector"],
        "public_operation": lane["public_operation"],
        "process_lifecycle": lane["process_lifecycle"],
        "engine_lifecycle": lane["engine_lifecycle"],
        "logical_operations_per_estimate": lane["logical_operations_per_estimate"],
        "transport": lane["transport"],
        "preset": "long",
        "sample_size": 30,
        "warm_up_seconds": 2,
        "measurement_seconds": 3,
        "evidence_mode": "confirmation",
        "calibration_pairs": 8,
        "max_pairs": 32,
        "start_side": "base",
        "relative_threshold_percent": 10.0,
        "absolute_threshold_ns": 50_000.0,
        "confidence_level": 0.95,
        "bootstrap_seed": 0,
        "bootstrap_resamples": 10_000,
    }
    _validate_latency_recipe(recipe)
    return recipe


def _validate_latency_lane_recipe(
    recipe: Mapping[str, object],
    *,
    corpus_path: str,
    lane: Mapping[str, object],
) -> None:
    _validate_latency_recipe(recipe)
    expected = {
        "corpus": corpus_path,
        "lane_id": lane["id"],
        "selector": lane["selector"],
        "public_operation": lane["public_operation"],
        "process_lifecycle": lane["process_lifecycle"],
        "engine_lifecycle": lane["engine_lifecycle"],
        "logical_operations_per_estimate": lane["logical_operations_per_estimate"],
        "transport": lane["transport"],
    }
    for field, value in expected.items():
        if recipe[field] != value:
            raise ManifestError(f"latency recipe {field} differs from the corpus lane")
    if lane["kind"] != "public" or lane["transport"] != "native-criterion":
        raise ManifestError("latency common A/A lane must be a native public operation")


def _command_output(command: Sequence[str], *, root: Path, context: str) -> str:
    try:
        result = subprocess.run(
            list(command),
            cwd=root,
            capture_output=True,
            text=True,
            timeout=30,
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise ManifestError(f"cannot inspect {context}: {error}") from error
    value = result.stdout.strip()
    if result.returncode != 0 or not value:
        detail = result.stderr.strip() or f"exit {result.returncode}"
        raise ManifestError(f"cannot inspect {context}: {detail}")
    return value


def _call_host_probe(
    probe: Callable[..., Mapping[str, object]],
    root: Path,
    requested_toolchain: str | None,
) -> Mapping[str, object]:
    """Call new two-argument probes while retaining one-argument test integrations."""

    try:
        signature = inspect.signature(probe)
    except (TypeError, ValueError):
        return probe(root, requested_toolchain)
    try:
        signature.bind(root, requested_toolchain)
    except TypeError:
        return probe(root)
    return probe(root, requested_toolchain)


def collect_host(
    root: Path, requested_toolchain: str | None = None
) -> dict[str, object]:
    """Collect the toolchain and machine identity used to freeze the baseline."""

    rustc_command = ["rustc", "-Vv"]
    cargo_command = ["cargo", "-V"]
    if requested_toolchain is not None:
        rustc_command = [
            "rustup",
            "run",
            requested_toolchain,
            "rustc",
            "-Vv",
        ]
        cargo_command = [
            "rustup",
            "run",
            requested_toolchain,
            "cargo",
            "-V",
        ]
    host = {
        "rustc": _command_output(rustc_command, root=root, context="rustc"),
        "cargo": _command_output(cargo_command, root=root, context="cargo"),
        "os": platform.platform(),
        "cpu": best_effort_cpu_model(),
        "architecture": platform.machine(),
    }
    _validate_host(host)
    return host


def _require_tracked(
    probe: GitProbe,
    records: Sequence[Mapping[str, object]],
    *,
    commit: str,
    context: str,
) -> None:
    for index, record in enumerate(records):
        path = str(record["path"])
        if not probe.is_tracked(path, commit):
            raise ManifestError(f"{context}[{index}] is not committed at {commit}: {path}")


def _record_for_path(
    records: Sequence[Mapping[str, object]], path: str, context: str
) -> Mapping[str, object]:
    matches = [record for record in records if record.get("path") == path]
    if len(matches) != 1:
        raise ManifestError(
            f"{context} requires exactly one frozen {path!r} record; found {len(matches)}"
        )
    return matches[0]


def _ensure_same_snapshot(before: GitSnapshot, after: GitSnapshot, *, action: str) -> None:
    _require_clean(after, action=action)
    if before != after:
        raise ManifestError(
            f"repository changed while {action}: before={before}, after={after}"
        )


def freeze_baseline(
    root: Path,
    native_memory_report: Path,
    *,
    source_paths: Sequence[str] = DEFAULT_SOURCE_PATHS,
    manifest_paths: Sequence[str] = DEFAULT_MANIFEST_PATHS,
    lock_path: str = DEFAULT_LOCK,
    corpus_path: str = DEFAULT_CORPUS,
    latency_lane: str = DEFAULT_LATENCY_LANE,
    latency_recipe: Mapping[str, object] | None = None,
    git_probe: GitProbe | None = None,
    host_probe: Callable[..., Mapping[str, object]] = collect_host,
    frozen_at: str | None = None,
) -> dict[str, object]:
    """Build a manifest from one clean committed tree and completed memory report."""

    resolved_root = _resolve_root(Path(root))
    probe = git_probe or SubprocessGitProbe(resolved_root)
    before = probe.snapshot()
    _require_clean(before, action="performance baseline freeze")

    if not source_paths or not manifest_paths:
        raise ManifestError("source_paths and manifest_paths must not be empty")
    if len(source_paths) != len(set(source_paths)) or len(manifest_paths) != len(set(manifest_paths)):
        raise ManifestError("source_paths and manifest_paths must not contain duplicates")

    sources = [
        _record_repo_file(resolved_root, path, f"source[{index}]")
        for index, path in enumerate(source_paths)
    ]
    manifests = [
        _record_repo_file(resolved_root, path, f"manifest[{index}]")
        for index, path in enumerate(manifest_paths)
    ]
    workspace_manifest = _record_for_path(
        manifests, "Cargo.toml", "native-memory report provenance"
    )
    package_manifest = _record_for_path(
        manifests,
        "crates/merman/Cargo.toml",
        "native-memory report provenance",
    )
    lock = _record_repo_file(resolved_root, lock_path, "Cargo.lock")
    corpus_record = _record_repo_file(resolved_root, corpus_path, "benchmark corpus")
    resolved_corpus, _ = _resolve_repo_file(resolved_root, corpus_path, "benchmark corpus")
    corpus = _load_corpus(resolved_corpus)
    fixtures = _fixture_records(resolved_root, corpus)

    report_path = Path(native_memory_report)
    if not report_path.is_absolute():
        report_path = resolved_root / report_path
    try:
        report_path = report_path.resolve(strict=True)
    except OSError as error:
        raise ManifestError(f"native-memory report is unavailable: {report_path}") from error
    report_evidence = _inspect_native_memory_report(
        resolved_root,
        report_path,
        corpus_path=resolved_corpus,
        corpus_record=corpus_record,
        corpus=corpus,
        workspace_manifest_record=workspace_manifest,
        package_manifest_record=package_manifest,
        lock_record=lock,
        expected_commit=before.commit,
        expected_tree=before.tree,
        host=None,
        host_probe=host_probe,
    )
    host = dict(_object(report_evidence["host"], "native-memory host"))

    latency_lane_value = _find_lane(corpus, latency_lane)
    if latency_recipe is None:
        resolved_latency_recipe = _default_latency_recipe(corpus_path, latency_lane_value)
    else:
        resolved_latency_recipe = dict(latency_recipe)
    _validate_latency_lane_recipe(
        resolved_latency_recipe,
        corpus_path=corpus_path,
        lane=latency_lane_value,
    )

    repository_records = [
        *sources,
        *manifests,
        lock,
        corpus_record,
        *fixtures,
        _object(report_evidence["owner_contract_record"], "owner contract record"),
    ]
    _require_tracked(
        probe,
        repository_records,
        commit=before.commit,
        context="frozen repository artifact",
    )

    timestamp = frozen_at or dt.datetime.now(dt.timezone.utc).astimezone().isoformat()
    manifest: dict[str, object] = {
        "schema_version": SCHEMA_VERSION,
        "kind": MANIFEST_KIND,
        "frozen_at": timestamp,
        "repository": {
            "commit": before.commit,
            "tree": before.tree,
            "result_tree": before.tree,
            "patch_stack": [],
        },
        "artifacts": {
            "source": sources,
            "manifest": manifests,
            "lock": lock,
            "corpus": corpus_record,
            "fixtures": fixtures,
            "native_memory_owner_contract": report_evidence["owner_contract_record"],
            "native_memory_report": report_evidence["report_record"],
            "native_memory_executable": report_evidence["executable_record"],
        },
        "recipes": {
            "latency_common_aa": resolved_latency_recipe,
            "native_memory": report_evidence["recipe"],
        },
        "host": host,
    }
    validate_manifest(manifest)
    _verify_manifest_artifacts(
        resolved_root,
        manifest,
        probe=probe,
        verify_report=True,
        expected_host=host,
    )
    after = probe.snapshot()
    _ensure_same_snapshot(before, after, action="performance baseline freeze")
    return manifest


def _atomic_json(path: Path, value: Mapping[str, object]) -> None:
    payload = json.dumps(value, indent=2, sort_keys=True, allow_nan=False) + "\n"
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        descriptor, temporary_name = tempfile.mkstemp(
            prefix=f".{path.name}.", suffix=".tmp", dir=path.parent
        )
    except OSError as error:
        raise ManifestError(f"cannot prepare atomic output {path}: {error}") from error
    temporary = Path(temporary_name)
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8", newline="\n") as output:
            output.write(payload)
            output.flush()
            os.fsync(output.fileno())
        os.chmod(temporary, 0o644)
        os.replace(temporary, path)
        try:
            directory_fd = os.open(path.parent, os.O_RDONLY)
        except OSError:
            directory_fd = None
        if directory_fd is not None:
            try:
                os.fsync(directory_fd)
            finally:
                os.close(directory_fd)
    except OSError as error:
        raise ManifestError(f"cannot write atomic output {path}: {error}") from error
    finally:
        try:
            temporary.unlink()
        except FileNotFoundError:
            pass


def freeze_to_file(
    root: Path,
    native_memory_report: Path,
    output: Path,
    **kwargs: object,
) -> dict[str, object]:
    """Freeze a manifest and replace the output atomically after all checks pass."""

    manifest = freeze_baseline(root, native_memory_report, **kwargs)  # type: ignore[arg-type]
    validate_manifest(manifest)
    _atomic_json(Path(output), manifest)
    return manifest


def _verify_fixture_manifest(
    corpus: Mapping[str, object], fixtures: Sequence[object]
) -> None:
    expected = [
        (str(_object(raw, "corpus fixture")["name"]), str(_object(raw, "corpus fixture")["source"]))
        for raw in _list(corpus["fixtures"], "corpus.fixtures")
    ]
    observed = [
        (
            str(_object(raw, "manifest fixture")["name"]),
            str(_object(raw, "manifest fixture")["path"]),
        )
        for raw in fixtures
    ]
    if observed != expected:
        raise ManifestError(
            "manifest fixture list differs from the corpus order or source paths"
        )


def _verify_manifest_artifacts(
    root: Path,
    manifest: Mapping[str, object],
    *,
    probe: GitProbe,
    verify_report: bool,
    expected_host: Mapping[str, object] | None,
) -> None:
    repository = _object(manifest["repository"], "repository")
    artifacts = _object(manifest["artifacts"], "artifacts")
    patches = _list(repository["patch_stack"], "repository.patch_stack")
    for index, raw_patch in enumerate(patches):
        patch = _object(raw_patch, f"repository.patch_stack[{index}]")
        _verify_record(
            root,
            {field: patch[field] for field in ("path", "bytes", "sha256")},
            f"patch_stack[{index}]",
            repository_only=True,
        )

    repository_records: list[Mapping[str, object]] = []
    for field in ("source", "manifest"):
        for index, record in enumerate(_list(artifacts[field], f"artifacts.{field}")):
            validated = _validate_file_record(
                record, f"artifacts.{field}[{index}]", allow_absolute=False
            )
            repository_records.append(validated)
            _verify_record(
                root,
                record,
                f"artifacts.{field}[{index}]",
                repository_only=True,
            )
    for field in ("lock", "corpus", "native_memory_owner_contract"):
        validated = _validate_file_record(
            artifacts[field], f"artifacts.{field}", allow_absolute=False
        )
        repository_records.append(validated)
        _verify_record(
            root, artifacts[field], f"artifacts.{field}", repository_only=True
        )

    fixtures = _list(artifacts["fixtures"], "artifacts.fixtures")
    for index, fixture in enumerate(fixtures):
        validated = _object(fixture, f"artifacts.fixtures[{index}]")
        repository_records.append(validated)
        _verify_record(
            root,
            {field: validated[field] for field in ("path", "bytes", "sha256")},
            f"artifacts.fixtures[{index}]",
            repository_only=True,
        )

    patch_records = [
        _object(patch, f"repository.patch_stack[{index}]")
        for index, patch in enumerate(patches)
    ]
    _require_tracked(
        probe,
        [*repository_records, *patch_records],
        commit=str(repository["commit"]),
        context="manifest repository artifact",
    )

    corpus_path = _verify_record(
        root, artifacts["corpus"], "artifacts.corpus", repository_only=True
    )
    corpus = _load_corpus(corpus_path)
    _verify_fixture_manifest(corpus, fixtures)

    latency_recipe = _object(
        _object(manifest["recipes"], "recipes")["latency_common_aa"],
        "recipes.latency_common_aa",
    )
    latency_lane = _find_lane(corpus, str(latency_recipe["lane_id"]))
    _validate_latency_lane_recipe(
        latency_recipe,
        corpus_path=str(_object(artifacts["corpus"], "artifacts.corpus")["path"]),
        lane=latency_lane,
    )

    report_path = _verify_record(
        root,
        artifacts["native_memory_report"],
        "artifacts.native_memory_report",
        repository_only=False,
    )
    _verify_record(
        root,
        artifacts["native_memory_executable"],
        "artifacts.native_memory_executable",
        repository_only=False,
    )
    if not verify_report:
        return
    manifest_records = [
        _object(record, f"artifacts.manifest[{index}]")
        for index, record in enumerate(
            _list(artifacts["manifest"], "artifacts.manifest")
        )
    ]
    evidence = _inspect_native_memory_report(
        root,
        report_path,
        corpus_path=corpus_path,
        corpus_record=_object(artifacts["corpus"], "artifacts.corpus"),
        corpus=corpus,
        workspace_manifest_record=_record_for_path(
            manifest_records,
            "Cargo.toml",
            "native-memory report provenance",
        ),
        package_manifest_record=_record_for_path(
            manifest_records,
            "crates/merman/Cargo.toml",
            "native-memory report provenance",
        ),
        lock_record=_object(artifacts["lock"], "artifacts.lock"),
        expected_commit=str(repository["commit"]),
        expected_tree=str(repository["tree"]),
        host=expected_host,
    )
    comparisons = (
        (
            "native_memory_owner_contract",
            evidence["owner_contract_record"],
        ),
        ("native_memory_report", evidence["report_record"]),
        ("native_memory_executable", evidence["executable_record"]),
    )
    for field, observed in comparisons:
        if artifacts[field] != observed:
            raise ManifestError(f"artifacts.{field} differs from the completed report evidence")
    recipes = _object(manifest["recipes"], "recipes")
    if recipes["native_memory"] != evidence["recipe"]:
        raise ManifestError("recipes.native_memory differs from the completed report")


def verify_baseline(
    root: Path,
    manifest_path: Path,
    *,
    git_probe: GitProbe | None = None,
) -> None:
    """Verify Git identity, every frozen artifact, and native report provenance."""

    resolved_root = _resolve_root(Path(root))
    value = load_strict_json(Path(manifest_path))
    validate_manifest(value)
    manifest = _object(value, "manifest")
    probe = git_probe or SubprocessGitProbe(resolved_root)
    before = probe.snapshot()
    _require_clean(before, action="performance baseline verification")
    repository = _object(manifest["repository"], "repository")
    if before.commit != repository["commit"]:
        raise ManifestError(
            f"commit mismatch: expected {repository['commit']}, observed {before.commit}"
        )
    if before.tree != repository["tree"]:
        raise ManifestError(
            f"tree mismatch: expected {repository['tree']}, observed {before.tree}"
        )
    if before.tree != repository["result_tree"]:
        raise ManifestError(
            f"result_tree mismatch: expected {repository['result_tree']}, observed {before.tree}"
        )
    _verify_manifest_artifacts(
        resolved_root,
        manifest,
        probe=probe,
        verify_report=True,
        expected_host=_object(manifest["host"], "host"),
    )
    after = probe.snapshot()
    _ensure_same_snapshot(before, after, action="performance baseline verification")


def _load_latency_recipe(path: str) -> Mapping[str, object] | None:
    if not path:
        return None
    value = load_strict_json(Path(path))
    recipe = _object(value, "latency recipe")
    _validate_latency_recipe(recipe)
    return recipe


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Freeze or verify a decision-grade Merman performance baseline manifest."
    )
    subparsers = parser.add_subparsers(dest="command", required=True)

    freeze = subparsers.add_parser("freeze", help="freeze one clean committed baseline")
    freeze.add_argument("--repo-root", default=str(repo_root()))
    freeze.add_argument("--native-memory-report", required=True)
    freeze.add_argument("--out", default=DEFAULT_OUTPUT)
    freeze.add_argument("--source", action="append", default=None)
    freeze.add_argument("--cargo-manifest", action="append", default=None)
    freeze.add_argument("--lock", default=DEFAULT_LOCK)
    freeze.add_argument("--corpus", default=DEFAULT_CORPUS)
    freeze.add_argument("--latency-lane", default=DEFAULT_LATENCY_LANE)
    freeze.add_argument("--latency-recipe", default="")

    verify = subparsers.add_parser("verify", help="verify a frozen baseline")
    verify.add_argument("--repo-root", default=str(repo_root()))
    verify.add_argument("--manifest", required=True)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = _build_parser().parse_args(list(argv) if argv is not None else None)
    try:
        root = Path(args.repo_root).resolve()
        if args.command == "freeze":
            output = Path(args.out)
            if not output.is_absolute():
                output = root / output
            recipe = _load_latency_recipe(args.latency_recipe)
            freeze_to_file(
                root,
                Path(args.native_memory_report),
                output,
                source_paths=tuple(args.source or DEFAULT_SOURCE_PATHS),
                manifest_paths=tuple(args.cargo_manifest or DEFAULT_MANIFEST_PATHS),
                lock_path=args.lock,
                corpus_path=args.corpus,
                latency_lane=args.latency_lane,
                latency_recipe=recipe,
            )
            print(f"Wrote: {output}")
            return 0
        verify_baseline(root, Path(args.manifest))
        print(f"Verified: {Path(args.manifest)}")
        return 0
    except ManifestError as error:
        print(f"performance baseline manifest error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
