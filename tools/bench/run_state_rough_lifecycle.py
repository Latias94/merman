#!/usr/bin/env python3
"""Run the State Rough cache-lifecycle probe and enforce its admission contract."""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import math
import os
import platform
import subprocess
import sys
from collections.abc import Mapping, Sequence
from pathlib import Path, PurePosixPath

from corpus_utils import LaneMetadata, load_corpus, resolve_lane_selector


DEFAULT_CORPUS = Path("tools/bench/corpus.json")
DEFAULT_LANE = "state-rough-retained-lifecycle"
DEFAULT_REPORT_ROOT = Path("target/bench")
DEFAULT_TIMEOUT_SECONDS = 1_800
DRIVER_SCHEMA = "merman.state_rough_lifecycle_driver.v1"

_CONTRACT_FIELDS = frozenset(
    {
        "schema_version",
        "lane_id",
        "workload",
        "evidence_class",
        "candidate_admission",
        "baseline_provenance",
        "receipt",
        "probe",
        "controls",
        "schedule",
    }
)
_CONTRACT_BASELINE_PROVENANCE_FIELDS = frozenset(
    {"relation", "candidate_parent_count", "baseline_source_clean"}
)
_CONTRACT_RECEIPT_FIELDS = frozenset(
    {
        "schema",
        "marker",
        "owned_bytes",
        "configured_seed_zero",
        "fallback_capable_configured_seeds",
    }
)
_CONTRACT_PROBE_FIELDS = frozenset(
    {
        "package",
        "package_manifest",
        "target",
        "test_name",
        "default_features",
        "features",
    }
)
_CONTRACT_CONTROLS_FIELDS = frozenset(
    {
        "schema",
        "marker",
        "test_name",
        "scenarios",
        "error_sentinel",
        "unwind_sentinel",
        "configured_seed",
        "workers",
    }
)
_CONTRACT_SCHEDULE_FIELDS = frozenset(
    {
        "detailed_requests",
        "detailed_specs",
        "same_seed_request_ordinals",
        "distinct_seed_request_ordinals",
        "fallback_bypass_request_ordinals",
        "geometry_label_byte_checkpoints",
        "long_lived_requests",
        "long_lived_checkpoints",
        "long_lived_seed_base",
        "long_lived_geometry_label_bytes",
        "long_lived_ordinary_nodes_min",
        "long_lived_ordinary_nodes_cycle",
    }
)
_DETAILED_SPEC_FIELDS = frozenset(
    {
        "case",
        "configured_seed",
        "render_thread",
        "geometry_label_bytes",
        "ordinary_nodes",
    }
)

_RECEIPT_FIELDS = frozenset(
    {
        "schema",
        "contracts",
        "engine_lifecycle",
        "schedule",
        "requests",
        "checkpoints",
        "long_lived",
        "rollup",
    }
)
_RECEIPT_CONTRACT_FIELDS = frozenset(
    {
        "owned_bytes",
        "configured_seed_zero",
        "fallback_capable_configured_seeds",
    }
)
_ENGINE_FIELDS = frozenset(
    {
        "engine_instances",
        "engine_reused_across_requests",
        "request_count",
        "detailed_request_count",
        "long_lived_request_count",
        "render_threads",
    }
)
_SCHEDULE_FIELDS = frozenset(
    {
        "same_seed_request_ordinals",
        "distinct_seed_request_ordinals",
        "fallback_bypass_request_ordinals",
        "geometry_label_byte_checkpoints",
        "request_count_checkpoints",
    }
)
_REQUEST_FIELDS = frozenset(
    {
        "ordinal",
        "case",
        "render_thread",
        "geometry_label_bytes",
        "ordinary_nodes",
        "svg",
        "operation",
    }
)
_SVG_FIELDS = frozenset({"bytes", "elements", "identity"})
_OPERATION_FIELDS = frozenset(
    {
        "configured_seed",
        "resolved_seed",
        "seed_resolution",
        "cache_allowed",
        "outcome",
        "counters",
        "operation_peak",
        "post_operation_retained",
    }
)
_COUNTER_FIELDS = frozenset({"circle", "paths"})
_COUNTER_KINDS = ("circle", "paths")
_KIND_COUNTER_FIELDS = frozenset(
    {
        "draw_requests",
        "operation_lookups",
        "operation_hits",
        "operation_misses",
        "operation_builds",
        "tls_hits",
        "global_hits",
        "bypass_builds",
    }
)
_FOOTPRINT_FIELDS = frozenset({"entries", "owned_bytes"})
_RETAINED_FIELDS = frozenset({"global", "tls"})
_CHECKPOINT_FIELDS = frozenset(
    {"request_count", "geometry_label_bytes", "configured_seed", "retained"}
)
_LONG_LIVED_FIELDS = frozenset(
    {
        "request_count",
        "request_count_checkpoints",
        "checkpoints",
        "svg",
        "counters",
        "max_operation_peak",
        "max_post_operation_retained",
        "final_retained",
    }
)
_ROLLUP_FIELDS = frozenset(
    {
        "svg",
        "counters",
        "max_operation_peak",
        "initial_retained",
        "final_retained",
        "retained_growth",
        "operation_cache_reuse_observed",
        "legacy_cross_operation_cache_observed",
        "configured_zero_operation_resolution_observed",
        "fallback_bypass_observed",
    }
)

_CONTROLS_RECEIPT_FIELDS = frozenset(
    {"schema", "error", "unwind", "concurrency"}
)
_FAILURE_CONTROL_FIELDS = frozenset({"sentinel", "operation"})
_CONCURRENCY_CONTROL_FIELDS = frozenset(
    {"workers", "overlap_observed", "serial_svg", "worker_svgs", "operations"}
)

_REPORT_FIELDS = frozenset(
    {
        "schema",
        "generated_at_utc",
        "mode",
        "lane",
        "contract",
        "source",
        "host",
        "harness",
        "build",
        "probe",
        "controls",
        "baseline_comparison",
        "receipt",
        "checks",
        "outcome",
        "exit_code",
    }
)
_REPORT_LANE_FIELDS = frozenset(
    {
        "id",
        "owner",
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
_FILE_DESCRIPTOR_FIELDS = frozenset({"path", "bytes", "sha256"})
_SOURCE_FIELDS = frozenset(
    {
        "commit",
        "tree",
        "first_parent_commit",
        "first_parent_tree",
        "parent_count",
        "clean",
        "dirty_status_sha256",
    }
)
_HOST_FIELDS = frozenset({"platform", "machine", "python"})
_HARNESS_FIELDS = frozenset({"driver", "rust_probe"})
_BUILD_FIELDS = frozenset(
    {
        "command",
        "environment",
        "cargo_stdout_sha256",
        "cargo_stderr_sha256",
        "executable",
    }
)
_BUILD_ENVIRONMENT_FIELDS = frozenset({"CARGO_BUILD_JOBS", "CARGO_INCREMENTAL"})
_PROBE_REPORT_FIELDS = frozenset(
    {"command", "timeout_seconds", "returncode", "stdout_sha256", "stderr_sha256"}
)
_CONTROLS_REPORT_FIELDS = frozenset({"probe", "receipt"})
_EMPTY_GIT_STATUS_SHA256 = hashlib.sha256(b"").hexdigest()


class LifecycleContractError(ValueError):
    """The lifecycle evidence cannot satisfy its registered contract."""


def repo_root() -> Path:
    return Path(__file__).resolve().parents[2]


def _reject_duplicates(pairs: list[tuple[str, object]]) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result:
            raise LifecycleContractError(f"duplicate JSON key: {key}")
        result[key] = value
    return result


def _parse_float(token: str) -> float:
    value = float(token)
    if not math.isfinite(value):
        raise LifecycleContractError(f"non-finite JSON number: {token}")
    return value


def _reject_constant(token: str) -> None:
    raise LifecycleContractError(f"non-finite JSON number: {token}")


def strict_json_text(text: str, *, source: str) -> object:
    try:
        return json.loads(
            text,
            object_pairs_hook=_reject_duplicates,
            parse_float=_parse_float,
            parse_constant=_reject_constant,
        )
    except LifecycleContractError:
        raise
    except json.JSONDecodeError as error:
        raise LifecycleContractError(f"invalid JSON from {source}: {error}") from error


def strict_json_path(path: Path) -> object:
    try:
        text = path.read_text(encoding="utf-8")
    except (OSError, UnicodeError) as error:
        raise LifecycleContractError(f"cannot read JSON file {path}: {error}") from error
    return strict_json_text(text, source=str(path))


def _object(
    value: object,
    *,
    fields: frozenset[str],
    context: str,
) -> dict[str, object]:
    if not isinstance(value, dict):
        raise LifecycleContractError(f"{context} must be an object")
    actual = frozenset(value)
    if actual != fields:
        missing = sorted(fields - actual)
        unknown = sorted(actual - fields)
        raise LifecycleContractError(
            f"{context} fields differ: missing={missing}, unknown={unknown}"
        )
    return value


def _list(value: object, *, context: str) -> list[object]:
    if not isinstance(value, list):
        raise LifecycleContractError(f"{context} must be a list")
    return value


def _string(value: object, *, context: str) -> str:
    if not isinstance(value, str) or not value.strip() or value != value.strip():
        raise LifecycleContractError(f"{context} must be a trimmed non-empty string")
    return value


def _boolean(value: object, *, context: str) -> bool:
    if not isinstance(value, bool):
        raise LifecycleContractError(f"{context} must be a boolean")
    return value


def _nonnegative_int(value: object, *, context: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        raise LifecycleContractError(f"{context} must be a non-negative integer")
    return value


def _positive_int(value: object, *, context: str) -> int:
    result = _nonnegative_int(value, context=context)
    if result == 0:
        raise LifecycleContractError(f"{context} must be a positive integer")
    return result


def _finite_number(value: object, *, context: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise LifecycleContractError(f"{context} must be numeric")
    result = float(value)
    if not math.isfinite(result):
        raise LifecycleContractError(f"{context} must be finite")
    return result


def _string_list(
    value: object,
    *,
    context: str,
    allow_empty: bool,
) -> tuple[str, ...]:
    result = tuple(
        _string(item, context=f"{context}[{index}]")
        for index, item in enumerate(_list(value, context=context))
    )
    if not allow_empty and not result:
        raise LifecycleContractError(f"{context} must not be empty")
    if len(result) != len(set(result)):
        raise LifecycleContractError(f"{context} contains duplicate values")
    return result


def _positive_int_list(value: object, *, context: str) -> tuple[int, ...]:
    result = tuple(
        _positive_int(item, context=f"{context}[{index}]")
        for index, item in enumerate(_list(value, context=context))
    )
    if not result:
        raise LifecycleContractError(f"{context} must not be empty")
    if len(result) != len(set(result)) or tuple(sorted(result)) != result:
        raise LifecycleContractError(f"{context} must be unique and increasing")
    return result


def _repo_relative_path(value: object, *, context: str) -> str:
    result = _string(value, context=context)
    pure = PurePosixPath(result)
    if (
        pure.is_absolute()
        or pure.as_posix() != result
        or "\\" in result
        or any(part in ("", ".", "..") for part in pure.parts)
    ):
        raise LifecycleContractError(f"{context} must be a normalized repo-relative path")
    return result


def _sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _sha256_path(path: Path) -> str:
    digest = hashlib.sha256()
    try:
        with path.open("rb") as source:
            for chunk in iter(lambda: source.read(1024 * 1024), b""):
                digest.update(chunk)
    except OSError as error:
        raise LifecycleContractError(f"cannot hash {path}: {error}") from error
    return digest.hexdigest()


def _sha256(value: object, *, context: str) -> str:
    digest = _string(value, context=context)
    if len(digest) != 64 or any(
        character not in "0123456789abcdef" for character in digest
    ):
        raise LifecycleContractError(f"{context} must be a lowercase SHA-256 digest")
    return digest


def _git_object_id(value: object, *, context: str) -> str:
    revision = _string(value, context=context)
    if len(revision) not in (40, 64) or any(
        character not in "0123456789abcdef" for character in revision
    ):
        raise LifecycleContractError(f"{context} must be a lowercase Git object id")
    return revision


def _describe_file(
    path: Path, *, root: Path, absolute_path: bool = False
) -> dict[str, object]:
    try:
        resolved = path.resolve(strict=True)
    except OSError as error:
        raise LifecycleContractError(f"file does not exist: {path}") from error
    if not resolved.is_file():
        raise LifecycleContractError(f"path is not a file: {resolved}")
    if absolute_path:
        display = str(resolved)
    else:
        try:
            display = resolved.relative_to(root.resolve(strict=True)).as_posix()
        except ValueError:
            display = str(resolved)
    return {
        "path": display,
        "bytes": resolved.stat().st_size,
        "sha256": _sha256_path(resolved),
    }


def _validate_file_descriptor(value: object, *, context: str) -> dict[str, object]:
    descriptor = _object(value, fields=_FILE_DESCRIPTOR_FIELDS, context=context)
    _string(descriptor["path"], context=f"{context}.path")
    _nonnegative_int(descriptor["bytes"], context=f"{context}.bytes")
    _sha256(descriptor["sha256"], context=f"{context}.sha256")
    return descriptor


def _lane_report(lane: LaneMetadata) -> dict[str, object]:
    return {
        "id": lane.id,
        "owner": lane.owner,
        "public_operation": lane.public_operation,
        "process_lifecycle": lane.process_lifecycle,
        "engine_lifecycle": lane.engine_lifecycle,
        "logical_operations_per_estimate": lane.logical_operations_per_estimate,
        "transport": lane.transport,
        "workload": lane.workload,
        "size_vector": list(lane.size_vector),
        "measurement_metrics": list(lane.measurement_metrics),
        "semantic_output_dimensions": list(lane.semantic_output_dimensions),
    }


def _harness_report(root: Path) -> dict[str, object]:
    return {
        "driver": _describe_file(
            root / "tools" / "bench" / "run_state_rough_lifecycle.py", root=root
        ),
        "rust_probe": _describe_file(
            root
            / "crates"
            / "merman-render"
            / "src"
            / "svg"
            / "parity"
            / "state"
            / "rough_lifecycle_probe.rs",
            root=root,
        ),
    }


def load_owner_contract(
    path: Path,
    *,
    lane: LaneMetadata,
    root: Path,
) -> dict[str, object]:
    contract = _object(
        strict_json_path(path), fields=_CONTRACT_FIELDS, context="owner contract"
    )
    if contract["schema_version"] != 1:
        raise LifecycleContractError("unsupported owner contract schema_version")
    if contract["lane_id"] != lane.id:
        raise LifecycleContractError("owner contract lane_id differs from corpus lane")
    if contract["workload"] != lane.workload:
        raise LifecycleContractError("owner contract workload differs from corpus lane")
    if contract["evidence_class"] != "candidate-bound":
        raise LifecycleContractError("owner contract must be candidate-bound")
    if contract["candidate_admission"] is not True:
        raise LifecycleContractError("owner contract must enable candidate admission")

    baseline_provenance = _object(
        contract["baseline_provenance"],
        fields=_CONTRACT_BASELINE_PROVENANCE_FIELDS,
        context="owner contract.baseline_provenance",
    )
    if baseline_provenance["relation"] != "candidate_head_first_parent":
        raise LifecycleContractError(
            "owner contract baseline provenance relation drifted"
        )
    if (
        _positive_int(
            baseline_provenance["candidate_parent_count"],
            context="owner contract.baseline_provenance.candidate_parent_count",
        )
        != 1
    ):
        raise LifecycleContractError(
            "owner contract baseline provenance requires exactly one candidate parent"
        )
    if (
        _boolean(
            baseline_provenance["baseline_source_clean"],
            context="owner contract.baseline_provenance.baseline_source_clean",
        )
        is not True
    ):
        raise LifecycleContractError(
            "owner contract baseline provenance requires a clean baseline source"
        )

    receipt = _object(
        contract["receipt"],
        fields=_CONTRACT_RECEIPT_FIELDS,
        context="owner contract.receipt",
    )
    expected_receipt_strings = {
        "schema": "merman.state_rough_lifecycle.v1",
        "marker": "MERMAN_STATE_ROUGH_LIFECYCLE_RECEIPT_V1=",
        "owned_bytes": "sum_of_cached_string_capacities",
        "configured_seed_zero": (
            "configured_hand_drawn_seed_zero_resolves_to_operation_seed_before_cache_bypass"
        ),
    }
    for field, expected in expected_receipt_strings.items():
        if _string(receipt[field], context=f"owner contract.receipt.{field}") != expected:
            raise LifecycleContractError(f"owner contract.receipt.{field} drifted")
    fallback_seeds = tuple(
        _finite_number(value, context=f"owner contract.receipt.fallback[{index}]")
        for index, value in enumerate(
            _list(
                receipt["fallback_capable_configured_seeds"],
                context="owner contract.receipt.fallback_capable_configured_seeds",
            )
        )
    )
    if fallback_seeds != (4_294_967_296.0, -1.0):
        raise LifecycleContractError("owner contract fallback seeds drifted")

    probe = _object(
        contract["probe"], fields=_CONTRACT_PROBE_FIELDS, context="owner contract.probe"
    )
    if probe["package"] != lane.owner or probe["package"] != "merman-render":
        raise LifecycleContractError("owner contract probe package differs from lane owner")
    manifest = _repo_relative_path(
        probe["package_manifest"], context="owner contract.probe.package_manifest"
    )
    manifest_path = (root / manifest).resolve()
    try:
        manifest_path.relative_to(root.resolve(strict=True))
    except ValueError as error:
        raise LifecycleContractError("probe manifest escapes the repository") from error
    if not manifest_path.is_file():
        raise LifecycleContractError("probe manifest does not exist")
    if probe["target"] != "lib":
        raise LifecycleContractError("State Rough probe must use the library test target")
    test_name = _string(probe["test_name"], context="owner contract.probe.test_name")
    if test_name != (
        "svg::parity::state::rough_lifecycle_probe::"
        "state_rough_lifecycle_probe_receipt"
    ):
        raise LifecycleContractError("State Rough probe exact test name drifted")
    if not isinstance(probe["default_features"], bool):
        raise LifecycleContractError("owner contract.probe.default_features must be boolean")
    features = _string_list(
        probe["features"], context="owner contract.probe.features", allow_empty=True
    )
    if features != lane.required_features:
        raise LifecycleContractError("owner contract probe features differ from corpus lane")

    controls = _object(
        contract["controls"],
        fields=_CONTRACT_CONTROLS_FIELDS,
        context="owner contract.controls",
    )
    expected_control_strings = {
        "schema": "merman.state_rough_lifecycle_controls.v1",
        "marker": "MERMAN_STATE_ROUGH_LIFECYCLE_CONTROLS_V1=",
        "test_name": (
            "svg::parity::state::rough_lifecycle_probe::"
            "state_rough_lifecycle_release_controls"
        ),
        "error_sentinel": "State Rough lifecycle control error after root render",
        "unwind_sentinel": "State Rough lifecycle control unwind after root render",
    }
    for field, expected in expected_control_strings.items():
        if _string(
            controls[field], context=f"owner contract.controls.{field}"
        ) != expected:
            label = "test name" if field == "test_name" else field.replace("_", " ")
            raise LifecycleContractError(
                f"owner contract controls {label} drifted"
            )
    scenarios = _string_list(
        controls["scenarios"],
        context="owner contract.controls.scenarios",
        allow_empty=False,
    )
    if scenarios != ("error", "unwind", "concurrency"):
        raise LifecycleContractError("owner contract controls scenarios drifted")
    if _finite_number(
        controls["configured_seed"],
        context="owner contract.controls.configured_seed",
    ) != 23.0:
        raise LifecycleContractError("owner contract controls configured seed drifted")
    if _positive_int(
        controls["workers"], context="owner contract.controls.workers"
    ) != 2:
        raise LifecycleContractError("owner contract controls workers drifted")

    schedule = _object(
        contract["schedule"],
        fields=_CONTRACT_SCHEDULE_FIELDS,
        context="owner contract.schedule",
    )
    detailed_count = _positive_int(
        schedule["detailed_requests"], context="owner contract.schedule.detailed_requests"
    )
    long_lived_count = _positive_int(
        schedule["long_lived_requests"],
        context="owner contract.schedule.long_lived_requests",
    )
    if detailed_count + long_lived_count != lane.logical_operations_per_estimate:
        raise LifecycleContractError(
            "owner schedule request count differs from lane logical operation count"
        )
    detailed_specs = []
    for index, raw_spec in enumerate(
        _list(schedule["detailed_specs"], context="owner contract.schedule.detailed_specs")
    ):
        spec = _object(
            raw_spec,
            fields=_DETAILED_SPEC_FIELDS,
            context=f"owner contract.schedule.detailed_specs[{index}]",
        )
        case = _string(
            spec["case"],
            context=f"owner contract.schedule.detailed_specs[{index}].case",
        )
        configured_seed = _finite_number(
            spec["configured_seed"],
            context=f"owner contract.schedule.detailed_specs[{index}].configured_seed",
        )
        render_thread = _string(
            spec["render_thread"],
            context=f"owner contract.schedule.detailed_specs[{index}].render_thread",
        )
        if render_thread not in {"primary", "fresh"}:
            raise LifecycleContractError(
                f"owner detailed spec {index} uses an unknown render thread"
            )
        geometry_label_bytes = _positive_int(
            spec["geometry_label_bytes"],
            context=(
                f"owner contract.schedule.detailed_specs[{index}]."
                "geometry_label_bytes"
            ),
        )
        ordinary_nodes = _positive_int(
            spec["ordinary_nodes"],
            context=f"owner contract.schedule.detailed_specs[{index}].ordinary_nodes",
        )
        detailed_specs.append(
            (case, configured_seed, render_thread, geometry_label_bytes, ordinary_nodes)
        )
    expected_detailed_specs = (
        ("seed-7-cold", 7.0, "primary", 4, 6),
        ("seed-7-tls-warm", 7.0, "primary", 4, 6),
        ("seed-7-global-warm", 7.0, "fresh", 4, 6),
        ("seed-11-width-16", 11.0, "primary", 16, 6),
        ("seed-12-width-32", 12.0, "primary", 32, 6),
        ("seed-13-width-64", 13.0, "primary", 64, 6),
        ("configured-zero-operation-seed", 0.0, "primary", 16, 6),
        ("fallback-u32-wrap", 4_294_967_296.0, "primary", 4, 6),
        ("fallback-second-stroke-wrap", -1.0, "primary", 4, 6),
    )
    if tuple(detailed_specs) != expected_detailed_specs:
        raise LifecycleContractError("owner detailed request specs drifted")
    if len(detailed_specs) != detailed_count:
        raise LifecycleContractError("owner detailed request spec cardinality drifted")
    schedule_vectors = {
        field: _positive_int_list(
            schedule[field], context=f"owner contract.schedule.{field}"
        )
        for field in (
            "same_seed_request_ordinals",
            "distinct_seed_request_ordinals",
            "fallback_bypass_request_ordinals",
            "geometry_label_byte_checkpoints",
            "long_lived_checkpoints",
            "long_lived_geometry_label_bytes",
        )
    }
    if schedule_vectors["long_lived_checkpoints"] != lane.size_vector:
        raise LifecycleContractError(
            "owner long-lived checkpoints differ from corpus size_vector"
        )
    for field in (
        "same_seed_request_ordinals",
        "distinct_seed_request_ordinals",
        "fallback_bypass_request_ordinals",
    ):
        if schedule_vectors[field][-1] > detailed_count:
            raise LifecycleContractError(f"owner contract.schedule.{field} is out of range")
    if schedule_vectors["long_lived_checkpoints"][-1] != long_lived_count:
        raise LifecycleContractError("final long-lived checkpoint must equal request count")
    expected_vectors = {
        "same_seed_request_ordinals": (1, 2, 3),
        "distinct_seed_request_ordinals": (4, 5, 6),
        "fallback_bypass_request_ordinals": (8, 9),
        "geometry_label_byte_checkpoints": (4, 16, 32, 64),
        "long_lived_checkpoints": (1, 16, 64, 256, 1024, 2048),
        "long_lived_geometry_label_bytes": (1, 2, 4, 8, 16, 32, 64, 128),
    }
    if detailed_count != 9 or long_lived_count != 2_048:
        raise LifecycleContractError("owner lifecycle request cardinality drifted")
    for field, expected in expected_vectors.items():
        if schedule_vectors[field] != expected:
            raise LifecycleContractError(f"owner contract.schedule.{field} drifted")
    if _finite_number(
        schedule["long_lived_seed_base"],
        context="owner contract.schedule.long_lived_seed_base",
    ) != 10_000.0:
        raise LifecycleContractError("owner long-lived seed base drifted")
    if _positive_int(
        schedule["long_lived_ordinary_nodes_min"],
        context="owner contract.schedule.long_lived_ordinary_nodes_min",
    ) != 2:
        raise LifecycleContractError("owner long-lived ordinary-node minimum drifted")
    if _positive_int(
        schedule["long_lived_ordinary_nodes_cycle"],
        context="owner contract.schedule.long_lived_ordinary_nodes_cycle",
    ) != 5:
        raise LifecycleContractError("owner long-lived ordinary-node cycle drifted")

    if lane.transport != "native-library-test-probe":
        raise LifecycleContractError("State Rough lane uses the wrong transport")
    if lane.process_lifecycle != "reused-process" or lane.engine_lifecycle != "reused-engine":
        raise LifecycleContractError("State Rough lane lifecycle drifted")
    if lane.evidence_contract is None:
        raise LifecycleContractError("State Rough lane has no evidence contract")
    return contract


def _validate_sha_identity(value: object, *, context: str) -> str:
    identity = _string(value, context=context)
    prefix = "sha256:"
    digest = identity.removeprefix(prefix)
    if (
        not identity.startswith(prefix)
        or len(digest) != 64
        or any(character not in "0123456789abcdef" for character in digest)
    ):
        raise LifecycleContractError(
            f"{context} must be sha256:<64 lowercase hexadecimal characters>"
        )
    return identity


def _validate_svg(value: object, *, context: str) -> dict[str, object]:
    svg = _object(value, fields=_SVG_FIELDS, context=context)
    _positive_int(svg["bytes"], context=f"{context}.bytes")
    _positive_int(svg["elements"], context=f"{context}.elements")
    _validate_sha_identity(svg["identity"], context=f"{context}.identity")
    return svg


def _validate_kind_counters(value: object, *, context: str) -> dict[str, object]:
    counters = _object(value, fields=_KIND_COUNTER_FIELDS, context=context)
    parsed = {
        field: _nonnegative_int(counters[field], context=f"{context}.{field}")
        for field in _KIND_COUNTER_FIELDS
    }
    if parsed["draw_requests"] != (
        parsed["operation_lookups"] + parsed["bypass_builds"]
    ):
        raise LifecycleContractError(f"{context} draw-request identity failed")
    if parsed["operation_lookups"] != (
        parsed["operation_hits"] + parsed["operation_misses"]
    ):
        raise LifecycleContractError(f"{context} operation lookup identity failed")
    if parsed["operation_misses"] != (
        parsed["tls_hits"]
        + parsed["global_hits"]
        + parsed["operation_builds"]
    ):
        raise LifecycleContractError(f"{context} operation miss-source identity failed")
    return counters


def _validate_counters(value: object, *, context: str) -> dict[str, object]:
    counters = _object(value, fields=_COUNTER_FIELDS, context=context)
    for kind in _COUNTER_KINDS:
        _validate_kind_counters(counters[kind], context=f"{context}.{kind}")
    return counters


def _validate_footprint(value: object, *, context: str) -> dict[str, object]:
    footprint = _object(value, fields=_FOOTPRINT_FIELDS, context=context)
    _nonnegative_int(footprint["entries"], context=f"{context}.entries")
    _nonnegative_int(footprint["owned_bytes"], context=f"{context}.owned_bytes")
    return footprint


def _validate_retained(value: object, *, context: str) -> dict[str, object]:
    retained = _object(value, fields=_RETAINED_FIELDS, context=context)
    _validate_footprint(retained["global"], context=f"{context}.global")
    _validate_footprint(retained["tls"], context=f"{context}.tls")
    return retained


def _validate_operation(value: object, *, context: str) -> dict[str, object]:
    operation = _object(value, fields=_OPERATION_FIELDS, context=context)
    _finite_number(operation["configured_seed"], context=f"{context}.configured_seed")
    _finite_number(operation["resolved_seed"], context=f"{context}.resolved_seed")
    seed_resolution = _string(
        operation["seed_resolution"], context=f"{context}.seed_resolution"
    )
    if seed_resolution not in {
        "configured_deterministic",
        "configured_fallback_capable",
        "operation_resolved",
    }:
        raise LifecycleContractError(f"{context}.seed_resolution is unknown")
    cache_allowed = _boolean(
        operation["cache_allowed"], context=f"{context}.cache_allowed"
    )
    outcome = _string(operation["outcome"], context=f"{context}.outcome")
    if outcome not in {"success", "error", "unwind"}:
        raise LifecycleContractError(f"{context}.outcome is unknown")
    counters = _validate_counters(
        operation["counters"], context=f"{context}.counters"
    )
    for kind in _COUNTER_KINDS:
        kind_counters = counters[kind]
        assert isinstance(kind_counters, Mapping)
        if int(kind_counters["draw_requests"]) <= 0:
            raise LifecycleContractError(
                f"{context}.{kind} did not exercise a Rough draw"
            )
        if cache_allowed and int(kind_counters["bypass_builds"]) != 0:
            raise LifecycleContractError(
                f"{context}.{kind} bypassed an eligible deterministic cache"
            )
        if not cache_allowed and any(
            int(kind_counters[field]) != 0
            for field in (
                "operation_lookups",
                "operation_hits",
                "operation_misses",
                "operation_builds",
                "tls_hits",
                "global_hits",
            )
        ):
            raise LifecycleContractError(
                f"{context}.{kind} entered a cache while bypass was required"
            )
    _validate_footprint(operation["operation_peak"], context=f"{context}.operation_peak")
    _validate_retained(
        operation["post_operation_retained"],
        context=f"{context}.post_operation_retained",
    )
    return operation


def _validate_control_operation(
    value: object,
    *,
    context: str,
    expected_outcome: str,
    configured_seed: float,
) -> dict[str, object]:
    operation = _validate_operation(value, context=context)
    if operation["outcome"] != expected_outcome:
        raise LifecycleContractError(
            f"{context}.outcome must be {expected_outcome!r}"
        )
    if float(operation["configured_seed"]) != configured_seed:
        raise LifecycleContractError(f"{context}.configured_seed drifted")
    if float(operation["resolved_seed"]) != configured_seed:
        raise LifecycleContractError(f"{context}.resolved_seed drifted")
    if operation["seed_resolution"] != "configured_deterministic":
        raise LifecycleContractError(f"{context}.seed_resolution drifted")
    if operation["cache_allowed"] is not True:
        raise LifecycleContractError(f"{context} must exercise seeded caching")
    counters = operation["counters"]
    assert isinstance(counters, Mapping)
    for kind in _COUNTER_KINDS:
        kind_counters = counters[kind]
        assert isinstance(kind_counters, Mapping)
        if int(kind_counters["operation_lookups"]) <= 0:
            raise LifecycleContractError(
                f"{context}.{kind} did not exercise an operation-owned lookup"
            )
        if int(kind_counters["operation_hits"]) <= 0:
            raise LifecycleContractError(
                f"{context}.{kind} did not exercise operation-owned reuse"
            )
        if int(kind_counters["bypass_builds"]) != 0:
            raise LifecycleContractError(
                f"{context}.{kind} bypassed deterministic caching"
            )
    peak = operation["operation_peak"]
    assert isinstance(peak, Mapping)
    if int(peak["entries"]) <= 0 or int(peak["owned_bytes"]) <= 0:
        raise LifecycleContractError(
            f"{context} did not populate the operation-owned cache"
        )
    return operation


def _control_operations(
    receipt: Mapping[str, object],
) -> tuple[Mapping[str, object], ...]:
    error = receipt["error"]
    unwind = receipt["unwind"]
    concurrency = receipt["concurrency"]
    assert isinstance(error, Mapping)
    assert isinstance(unwind, Mapping)
    assert isinstance(concurrency, Mapping)
    operations = concurrency["operations"]
    assert isinstance(operations, list)
    return (
        error["operation"],
        unwind["operation"],
        *operations,
    )


def _validate_controls_receipt(
    value: object,
    *,
    contract: Mapping[str, object],
) -> dict[str, object]:
    receipt = _object(
        value,
        fields=_CONTROLS_RECEIPT_FIELDS,
        context="controls receipt",
    )
    controls_contract = contract["controls"]
    assert isinstance(controls_contract, Mapping)
    if receipt["schema"] != controls_contract["schema"]:
        raise LifecycleContractError(
            "controls receipt schema differs from owner contract"
        )
    configured_seed = float(controls_contract["configured_seed"])

    for scenario, expected_outcome, sentinel_field in (
        ("error", "error", "error_sentinel"),
        ("unwind", "unwind", "unwind_sentinel"),
    ):
        control = _object(
            receipt[scenario],
            fields=_FAILURE_CONTROL_FIELDS,
            context=f"controls receipt.{scenario}",
        )
        sentinel = _string(
            control["sentinel"],
            context=f"controls receipt.{scenario}.sentinel",
        )
        if sentinel != controls_contract[sentinel_field]:
            raise LifecycleContractError(
                f"controls receipt {scenario} sentinel drifted"
            )
        _validate_control_operation(
            control["operation"],
            context=f"controls receipt.{scenario}.operation",
            expected_outcome=expected_outcome,
            configured_seed=configured_seed,
        )

    concurrency = _object(
        receipt["concurrency"],
        fields=_CONCURRENCY_CONTROL_FIELDS,
        context="controls receipt.concurrency",
    )
    workers = _positive_int(
        concurrency["workers"], context="controls receipt.concurrency.workers"
    )
    if workers != controls_contract["workers"]:
        raise LifecycleContractError("controls receipt concurrency workers drifted")
    if not _boolean(
        concurrency["overlap_observed"],
        context="controls receipt.concurrency.overlap_observed",
    ):
        raise LifecycleContractError(
            "controls receipt did not observe overlapping operations"
        )
    serial_svg = _validate_svg(
        concurrency["serial_svg"], context="controls receipt.concurrency.serial_svg"
    )
    worker_svgs = [
        _validate_svg(
            svg, context=f"controls receipt.concurrency.worker_svgs[{index}]"
        )
        for index, svg in enumerate(
            _list(
                concurrency["worker_svgs"],
                context="controls receipt.concurrency.worker_svgs",
            )
        )
    ]
    if len(worker_svgs) != workers:
        raise LifecycleContractError(
            "controls receipt worker SVG cardinality drifted"
        )
    if any(svg != serial_svg for svg in worker_svgs):
        raise LifecycleContractError(
            "controls receipt worker output differs from the serial SVG"
        )
    operations = [
        _validate_control_operation(
            operation,
            context=f"controls receipt.concurrency.operations[{index}]",
            expected_outcome="success",
            configured_seed=configured_seed,
        )
        for index, operation in enumerate(
            _list(
                concurrency["operations"],
                context="controls receipt.concurrency.operations",
            )
        )
    ]
    if len(operations) != workers:
        raise LifecycleContractError(
            "controls receipt concurrent operation cardinality drifted"
        )
    return receipt


def validate_controls_mode(
    receipt: Mapping[str, object],
    *,
    mode: str,
) -> tuple[str, ...]:
    operations = _control_operations(receipt)
    checks = [
        "strict_release_controls_schema",
        "release_control_semantics",
        "release_control_circle_paths_population",
        "release_control_operation_peak",
        "concurrent_operation_overlap",
        "concurrent_serial_svg_identity",
    ]
    retained = []
    for operation in operations:
        snapshot = operation["post_operation_retained"]
        assert isinstance(snapshot, Mapping)
        retained.append(snapshot)

    if mode == "baseline":
        if not all(_retained_has_state(snapshot) for snapshot in retained):
            raise LifecycleContractError(
                "baseline release controls did not retain legacy cache state"
            )
        checks.append("release_controls_legacy_retention_observed")
    elif mode == "candidate":
        if not all(_retained_is_zero(snapshot) for snapshot in retained):
            raise LifecycleContractError(
                "candidate release controls retained State Rough state"
            )
        if any(
            int(operation["counters"][kind][field]) != 0
            for operation in operations
            for kind in _COUNTER_KINDS
            for field in ("tls_hits", "global_hits")
        ):
            raise LifecycleContractError(
                "candidate release controls entered a cross-operation cache"
            )
        checks.extend(
            (
                "release_controls_zero_cross_operation_hits",
                "release_controls_zero_post_operation_retention",
            )
        )
    else:
        raise LifecycleContractError(f"unsupported lifecycle mode: {mode}")
    return tuple(checks)


def _validate_request(value: object, *, context: str) -> dict[str, object]:
    request = _object(value, fields=_REQUEST_FIELDS, context=context)
    _positive_int(request["ordinal"], context=f"{context}.ordinal")
    _string(request["case"], context=f"{context}.case")
    thread = _string(request["render_thread"], context=f"{context}.render_thread")
    if thread not in {"primary", "fresh"}:
        raise LifecycleContractError(f"{context}.render_thread is unknown")
    _positive_int(
        request["geometry_label_bytes"], context=f"{context}.geometry_label_bytes"
    )
    _positive_int(request["ordinary_nodes"], context=f"{context}.ordinary_nodes")
    _validate_svg(request["svg"], context=f"{context}.svg")
    _validate_operation(request["operation"], context=f"{context}.operation")
    return request


def _validate_checkpoint(value: object, *, context: str) -> dict[str, object]:
    checkpoint = _object(value, fields=_CHECKPOINT_FIELDS, context=context)
    _positive_int(checkpoint["request_count"], context=f"{context}.request_count")
    _positive_int(
        checkpoint["geometry_label_bytes"],
        context=f"{context}.geometry_label_bytes",
    )
    _finite_number(checkpoint["configured_seed"], context=f"{context}.configured_seed")
    _validate_retained(checkpoint["retained"], context=f"{context}.retained")
    return checkpoint


def _counter_sum(
    counters: Sequence[Mapping[str, object]],
) -> dict[str, dict[str, int]]:
    return {
        kind: {
            field: sum(int(counter[kind][field]) for counter in counters)
            for field in _KIND_COUNTER_FIELDS
        }
        for kind in _COUNTER_KINDS
    }


def _footprint_is_zero(value: Mapping[str, object]) -> bool:
    return value["entries"] == 0 and value["owned_bytes"] == 0


def _retained_is_zero(value: Mapping[str, object]) -> bool:
    return _footprint_is_zero(value["global"]) and _footprint_is_zero(value["tls"])


def _retained_has_state(value: Mapping[str, object]) -> bool:
    return not _retained_is_zero(value)


def _footprint_growth(
    final: Mapping[str, object], initial: Mapping[str, object]
) -> dict[str, int]:
    if int(final["entries"]) < int(initial["entries"]) or int(
        final["owned_bytes"]
    ) < int(initial["owned_bytes"]):
        raise LifecycleContractError(
            "retained footprint decreased below the probe's initial snapshot"
        )
    return {
        "entries": int(final["entries"]) - int(initial["entries"]),
        "owned_bytes": int(final["owned_bytes"]) - int(initial["owned_bytes"]),
    }


def _validate_receipt(
    value: object,
    *,
    contract: Mapping[str, object],
) -> dict[str, object]:
    receipt = _object(value, fields=_RECEIPT_FIELDS, context="receipt")
    contract_receipt = contract["receipt"]
    contract_schedule = contract["schedule"]
    assert isinstance(contract_receipt, Mapping)
    assert isinstance(contract_schedule, Mapping)

    if receipt["schema"] != contract_receipt["schema"]:
        raise LifecycleContractError("receipt schema differs from owner contract")
    contracts = _object(
        receipt["contracts"], fields=_RECEIPT_CONTRACT_FIELDS, context="receipt.contracts"
    )
    if contracts["owned_bytes"] != contract_receipt["owned_bytes"]:
        raise LifecycleContractError("receipt owned-byte definition drifted")
    if contracts["configured_seed_zero"] != contract_receipt["configured_seed_zero"]:
        raise LifecycleContractError("receipt configured-zero contract drifted")
    fallback_seeds = tuple(
        _finite_number(value, context=f"receipt.contracts.fallback[{index}]")
        for index, value in enumerate(
            _list(
                contracts["fallback_capable_configured_seeds"],
                context="receipt.contracts.fallback_capable_configured_seeds",
            )
        )
    )
    if fallback_seeds != tuple(contract_receipt["fallback_capable_configured_seeds"]):
        raise LifecycleContractError("receipt fallback seed contract drifted")

    detailed_count = int(contract_schedule["detailed_requests"])
    long_lived_count = int(contract_schedule["long_lived_requests"])
    engine = _object(
        receipt["engine_lifecycle"], fields=_ENGINE_FIELDS, context="receipt.engine_lifecycle"
    )
    if _positive_int(engine["engine_instances"], context="receipt.engine_lifecycle.engine_instances") != 1:
        raise LifecycleContractError("receipt must use exactly one Engine instance")
    if not _boolean(
        engine["engine_reused_across_requests"],
        context="receipt.engine_lifecycle.engine_reused_across_requests",
    ):
        raise LifecycleContractError("receipt must reuse its Engine across requests")
    if _positive_int(
        engine["detailed_request_count"],
        context="receipt.engine_lifecycle.detailed_request_count",
    ) != detailed_count:
        raise LifecycleContractError("receipt detailed request count drifted")
    if _positive_int(
        engine["long_lived_request_count"],
        context="receipt.engine_lifecycle.long_lived_request_count",
    ) != long_lived_count:
        raise LifecycleContractError("receipt long-lived request count drifted")
    if _positive_int(
        engine["request_count"], context="receipt.engine_lifecycle.request_count"
    ) != detailed_count + long_lived_count:
        raise LifecycleContractError("receipt total request count drifted")
    if _positive_int(
        engine["render_threads"], context="receipt.engine_lifecycle.render_threads"
    ) != 2:
        raise LifecycleContractError(
            "receipt must exercise exactly one primary and one fresh render thread"
        )

    schedule = _object(receipt["schedule"], fields=_SCHEDULE_FIELDS, context="receipt.schedule")
    schedule_pairs = (
        ("same_seed_request_ordinals", "same_seed_request_ordinals"),
        ("distinct_seed_request_ordinals", "distinct_seed_request_ordinals"),
        ("fallback_bypass_request_ordinals", "fallback_bypass_request_ordinals"),
        ("geometry_label_byte_checkpoints", "geometry_label_byte_checkpoints"),
        ("request_count_checkpoints", "long_lived_checkpoints"),
    )
    for receipt_field, contract_field in schedule_pairs:
        actual = _positive_int_list(
            schedule[receipt_field], context=f"receipt.schedule.{receipt_field}"
        )
        if actual != tuple(contract_schedule[contract_field]):
            raise LifecycleContractError(f"receipt schedule {receipt_field} drifted")

    requests = [
        _validate_request(request, context=f"receipt.requests[{index}]")
        for index, request in enumerate(_list(receipt["requests"], context="receipt.requests"))
    ]
    if len(requests) != detailed_count:
        raise LifecycleContractError("receipt detailed request cardinality drifted")
    if tuple(int(request["ordinal"]) for request in requests) != tuple(
        range(1, detailed_count + 1)
    ):
        raise LifecycleContractError("receipt request ordinals must be contiguous")
    detailed_specs = contract_schedule["detailed_specs"]
    assert isinstance(detailed_specs, list)
    for request, spec in zip(requests, detailed_specs, strict=True):
        assert isinstance(spec, Mapping)
        operation = request["operation"]
        assert isinstance(operation, Mapping)
        if (
            request["case"] != spec["case"]
            or operation["configured_seed"] != spec["configured_seed"]
            or request["render_thread"] != spec["render_thread"]
            or request["geometry_label_bytes"] != spec["geometry_label_bytes"]
            or request["ordinary_nodes"] != spec["ordinary_nodes"]
        ):
            raise LifecycleContractError(
                f"receipt detailed request {request['ordinal']} differs from owner spec"
            )

    checkpoints = [
        _validate_checkpoint(checkpoint, context=f"receipt.checkpoints[{index}]")
        for index, checkpoint in enumerate(
            _list(receipt["checkpoints"], context="receipt.checkpoints")
        )
    ]
    if len(checkpoints) != detailed_count:
        raise LifecycleContractError("receipt detailed checkpoint cardinality drifted")
    for request, checkpoint in zip(requests, checkpoints, strict=True):
        if checkpoint["request_count"] != request["ordinal"]:
            raise LifecycleContractError("detailed checkpoint request count drifted")
        if checkpoint["geometry_label_bytes"] != request["geometry_label_bytes"]:
            raise LifecycleContractError("detailed checkpoint geometry size drifted")
        operation = request["operation"]
        assert isinstance(operation, Mapping)
        if checkpoint["configured_seed"] != operation["configured_seed"]:
            raise LifecycleContractError("detailed checkpoint configured seed drifted")
        if checkpoint["retained"] != operation["post_operation_retained"]:
            raise LifecycleContractError("detailed checkpoint retained snapshot drifted")

    long_lived = _object(
        receipt["long_lived"], fields=_LONG_LIVED_FIELDS, context="receipt.long_lived"
    )
    if _positive_int(
        long_lived["request_count"], context="receipt.long_lived.request_count"
    ) != long_lived_count:
        raise LifecycleContractError("long-lived request count drifted")
    long_checkpoint_counts = _positive_int_list(
        long_lived["request_count_checkpoints"],
        context="receipt.long_lived.request_count_checkpoints",
    )
    if long_checkpoint_counts != tuple(contract_schedule["long_lived_checkpoints"]):
        raise LifecycleContractError("long-lived request checkpoints drifted")
    long_checkpoints = [
        _validate_checkpoint(checkpoint, context=f"receipt.long_lived.checkpoints[{index}]")
        for index, checkpoint in enumerate(
            _list(
                long_lived["checkpoints"], context="receipt.long_lived.checkpoints"
            )
        )
    ]
    if tuple(int(checkpoint["request_count"]) for checkpoint in long_checkpoints) != long_checkpoint_counts:
        raise LifecycleContractError("long-lived checkpoint payloads drifted")
    long_label_cycle = tuple(contract_schedule["long_lived_geometry_label_bytes"])
    long_seed_base = float(contract_schedule["long_lived_seed_base"])
    for checkpoint in long_checkpoints:
        request_count = int(checkpoint["request_count"])
        expected_label_bytes = int(
            long_label_cycle[(request_count - 1) % len(long_label_cycle)]
        )
        if checkpoint["geometry_label_bytes"] != expected_label_bytes:
            raise LifecycleContractError(
                "long-lived checkpoint geometry-label schedule drifted"
            )
        if float(checkpoint["configured_seed"]) != long_seed_base + request_count:
            raise LifecycleContractError("long-lived checkpoint seed schedule drifted")
    _validate_svg(long_lived["svg"], context="receipt.long_lived.svg")
    _validate_counters(long_lived["counters"], context="receipt.long_lived.counters")
    _validate_footprint(
        long_lived["max_operation_peak"],
        context="receipt.long_lived.max_operation_peak",
    )
    max_post_operation_retained = _validate_retained(
        long_lived["max_post_operation_retained"],
        context="receipt.long_lived.max_post_operation_retained",
    )
    _validate_retained(
        long_lived["final_retained"], context="receipt.long_lived.final_retained"
    )
    if long_checkpoints[-1]["retained"] != long_lived["final_retained"]:
        raise LifecycleContractError("long-lived final retained snapshot drifted")
    observed_retained = [
        checkpoint["retained"] for checkpoint in long_checkpoints
    ]
    observed_retained.append(long_lived["final_retained"])
    for snapshot in observed_retained:
        for scope in ("global", "tls"):
            if (
                int(max_post_operation_retained[scope]["entries"])
                < int(snapshot[scope]["entries"])
                or int(max_post_operation_retained[scope]["owned_bytes"])
                < int(snapshot[scope]["owned_bytes"])
            ):
                raise LifecycleContractError(
                    "long-lived maximum post-operation retention is below an observed snapshot"
                )

    rollup = _object(receipt["rollup"], fields=_ROLLUP_FIELDS, context="receipt.rollup")
    _validate_svg(rollup["svg"], context="receipt.rollup.svg")
    rollup_counters = _validate_counters(
        rollup["counters"], context="receipt.rollup.counters"
    )
    _validate_footprint(
        rollup["max_operation_peak"], context="receipt.rollup.max_operation_peak"
    )
    initial_retained = _validate_retained(
        rollup["initial_retained"], context="receipt.rollup.initial_retained"
    )
    final_retained = _validate_retained(
        rollup["final_retained"], context="receipt.rollup.final_retained"
    )
    retained_growth = _validate_retained(
        rollup["retained_growth"], context="receipt.rollup.retained_growth"
    )
    for field in (
        "operation_cache_reuse_observed",
        "legacy_cross_operation_cache_observed",
        "configured_zero_operation_resolution_observed",
        "fallback_bypass_observed",
    ):
        _boolean(rollup[field], context=f"receipt.rollup.{field}")

    request_counters = []
    for request in requests:
        operation = request["operation"]
        assert isinstance(operation, Mapping)
        counters = operation["counters"]
        assert isinstance(counters, Mapping)
        request_counters.append(counters)
    expected_counters = _counter_sum(request_counters)
    long_counters = long_lived["counters"]
    assert isinstance(long_counters, Mapping)
    expected_counters = {
        kind: {
            field: int(expected_counters[kind][field])
            + int(long_counters[kind][field])
            for field in _KIND_COUNTER_FIELDS
        }
        for kind in _COUNTER_KINDS
    }
    if dict(rollup_counters) != expected_counters:
        raise LifecycleContractError(
            "receipt rollup counters differ from detailed and long-lived requests"
        )
    request_svgs = [request["svg"] for request in requests]
    if rollup["svg"]["bytes"] != int(long_lived["svg"]["bytes"]) + sum(
        int(svg["bytes"]) for svg in request_svgs
    ):
        raise LifecycleContractError(
            "receipt rollup SVG bytes differ from detailed and long-lived requests"
        )
    if rollup["svg"]["elements"] != sum(
        int(svg["elements"]) for svg in request_svgs
    ) + int(long_lived["svg"]["elements"]):
        raise LifecycleContractError(
            "receipt rollup SVG elements differ from detailed and long-lived requests"
        )
    expected_peak = {
        "entries": max(
            int(long_lived["max_operation_peak"]["entries"]),
            *(int(request["operation"]["operation_peak"]["entries"]) for request in requests),
        ),
        "owned_bytes": max(
            int(long_lived["max_operation_peak"]["owned_bytes"]),
            *(
                int(request["operation"]["operation_peak"]["owned_bytes"])
                for request in requests
            ),
        ),
    }
    if rollup["max_operation_peak"] != expected_peak:
        raise LifecycleContractError("receipt maximum operation peak drifted")
    if final_retained != long_lived["final_retained"]:
        raise LifecycleContractError("receipt final retained snapshot drifted")
    expected_growth = {
        scope: _footprint_growth(final_retained[scope], initial_retained[scope])
        for scope in ("global", "tls")
    }
    if retained_growth != expected_growth:
        raise LifecycleContractError("receipt retained growth calculation drifted")

    same_ordinals = tuple(schedule["same_seed_request_ordinals"])
    same_requests = [requests[int(ordinal) - 1] for ordinal in same_ordinals]
    if len({float(request["operation"]["configured_seed"]) for request in same_requests}) != 1:
        raise LifecycleContractError("same-seed controls do not use one configured seed")
    if len({json.dumps(request["svg"], sort_keys=True) for request in same_requests}) != 1:
        raise LifecycleContractError("same-seed controls do not have identical SVG output")
    distinct_ordinals = tuple(schedule["distinct_seed_request_ordinals"])
    distinct_requests = [requests[int(ordinal) - 1] for ordinal in distinct_ordinals]
    if len({float(request["operation"]["configured_seed"]) for request in distinct_requests}) != len(distinct_requests):
        raise LifecycleContractError("distinct-seed controls do not use distinct seeds")

    configured_zero_observed = any(
        float(request["operation"]["configured_seed"]) == 0.0
        and float(request["operation"]["resolved_seed"]) != 0.0
        and request["operation"]["seed_resolution"] == "operation_resolved"
        for request in requests
    )
    if rollup["configured_zero_operation_resolution_observed"] != configured_zero_observed:
        raise LifecycleContractError("configured-zero rollup flag drifted")

    fallback_ordinals = tuple(schedule["fallback_bypass_request_ordinals"])
    fallback_requests = [requests[int(ordinal) - 1] for ordinal in fallback_ordinals]
    expected_fallback_seeds = tuple(contract_receipt["fallback_capable_configured_seeds"])
    fallback_observed = (
        tuple(
            float(request["operation"]["configured_seed"])
            for request in fallback_requests
        )
        == expected_fallback_seeds
        and all(
            request["operation"]["cache_allowed"] is False
            and request["operation"]["seed_resolution"]
            == "configured_fallback_capable"
            and all(
                request["operation"]["counters"][kind]["operation_lookups"] == 0
                and request["operation"]["counters"][kind]["operation_hits"] == 0
                and request["operation"]["counters"][kind]["operation_misses"] == 0
                and request["operation"]["counters"][kind]["operation_builds"] == 0
                and request["operation"]["counters"][kind]["tls_hits"] == 0
                and request["operation"]["counters"][kind]["global_hits"] == 0
                and request["operation"]["counters"][kind]["bypass_builds"] > 0
                for kind in _COUNTER_KINDS
            )
            for request in fallback_requests
        )
    )
    if rollup["fallback_bypass_observed"] != fallback_observed:
        raise LifecycleContractError("fallback-bypass rollup flag drifted")

    operation_reuse_observed = all(
        int(rollup_counters[kind]["operation_hits"]) > 0
        for kind in _COUNTER_KINDS
    )
    if rollup["operation_cache_reuse_observed"] != operation_reuse_observed:
        raise LifecycleContractError("operation-cache reuse rollup flag drifted")
    retained_growth_observed = _retained_has_state(retained_growth)
    legacy_observed = (
        all(
            int(rollup_counters[kind]["tls_hits"]) > 0
            and int(rollup_counters[kind]["global_hits"]) > 0
            for kind in _COUNTER_KINDS
        )
        and retained_growth_observed
    )
    if rollup["legacy_cross_operation_cache_observed"] != legacy_observed:
        raise LifecycleContractError("legacy cross-operation rollup flag drifted")

    if not _retained_is_zero(initial_retained):
        raise LifecycleContractError("probe must start with zero retained State Rough state")
    if any(request["operation"]["outcome"] != "success" for request in requests):
        raise LifecycleContractError("decision receipt detailed requests must all succeed")
    return receipt


def validate_mode(
    receipt: Mapping[str, object],
    *,
    mode: str,
) -> tuple[str, ...]:
    rollup = receipt["rollup"]
    long_lived = receipt["long_lived"]
    requests = receipt["requests"]
    checkpoints = receipt["checkpoints"]
    assert isinstance(rollup, Mapping)
    assert isinstance(long_lived, Mapping)
    assert isinstance(requests, list)
    assert isinstance(checkpoints, list)
    rollup_counters = rollup["counters"]
    long_counters = long_lived["counters"]
    assert isinstance(rollup_counters, Mapping)
    assert isinstance(long_counters, Mapping)

    checks = [
        "strict_receipt_schema",
        "counter_identities",
        "single_reused_engine",
        "primary_and_fresh_thread_coverage",
        "same_seed_output_identity",
        "registered_detailed_request_specs",
    ]
    detailed_counters = _counter_sum(
        [request["operation"]["counters"] for request in requests]
    )
    for kind in _COUNTER_KINDS:
        if int(detailed_counters[kind]["operation_hits"]) <= 0:
            raise LifecycleContractError(
                f"detailed probe did not observe {kind} operation-cache reuse"
            )
        if int(long_counters[kind]["operation_hits"]) <= 0:
            raise LifecycleContractError(
                f"long-lived probe did not observe {kind} operation-cache reuse"
            )
    if not rollup["configured_zero_operation_resolution_observed"]:
        raise LifecycleContractError("configured-zero resolution contract was not observed")
    if not rollup["fallback_bypass_observed"]:
        raise LifecycleContractError("fallback-capable seeds did not bypass caching")
    checks.extend(
        [
            "operation_local_reuse",
            "configured_zero_resolution_exact_harness_assertion",
            "fallback_cache_bypass",
        ]
    )

    if mode == "baseline":
        if not rollup["legacy_cross_operation_cache_observed"]:
            raise LifecycleContractError(
                "baseline did not observe the legacy cross-operation cache path"
            )
        tls_request = requests[1]["operation"]["counters"]
        global_request = requests[2]["operation"]["counters"]
        for kind in _COUNTER_KINDS:
            if int(tls_request[kind]["tls_hits"]) <= 0:
                raise LifecycleContractError(
                    f"baseline TLS-warm request did not hit the {kind} TLS cache"
                )
            if int(global_request[kind]["global_hits"]) <= 0:
                raise LifecycleContractError(
                    f"baseline global-warm request did not hit the {kind} global cache"
                )
        retained_snapshots = [checkpoint["retained"] for checkpoint in checkpoints]
        retained_snapshots.extend(
            checkpoint["retained"] for checkpoint in long_lived["checkpoints"]
        )
        if not any(_retained_has_state(snapshot) for snapshot in retained_snapshots):
            raise LifecycleContractError("baseline did not retain legacy cache state")
        if not _retained_has_state(long_lived["final_retained"]):
            raise LifecycleContractError(
                "baseline long-lived process ended without retained legacy cache state"
            )
        if not _retained_has_state(long_lived["max_post_operation_retained"]):
            raise LifecycleContractError(
                "baseline long-lived operations never observed retained legacy cache state"
            )
        checks.extend(
            [
                "legacy_tls_and_global_hits",
                "legacy_retained_state_growth",
                "legacy_max_post_operation_retention",
            ]
        )
    elif mode == "candidate":
        if rollup["legacy_cross_operation_cache_observed"]:
            raise LifecycleContractError("candidate still reports a legacy cache path")
        if any(
            int(counters[kind][field]) != 0
            for counters in (rollup_counters, long_counters)
            for kind in _COUNTER_KINDS
            for field in ("tls_hits", "global_hits")
        ):
            raise LifecycleContractError("candidate still records cross-operation cache hits")
        for request in requests:
            counters = request["operation"]["counters"]
            if any(
                counters[kind]["tls_hits"] != 0
                or counters[kind]["global_hits"] != 0
                for kind in _COUNTER_KINDS
            ):
                raise LifecycleContractError(
                    "candidate detailed request entered a cross-operation cache"
                )
        retained_snapshots = [checkpoint["retained"] for checkpoint in checkpoints]
        retained_snapshots.extend(
            checkpoint["retained"] for checkpoint in long_lived["checkpoints"]
        )
        retained_snapshots.extend(
            [
                rollup["initial_retained"],
                rollup["final_retained"],
                rollup["retained_growth"],
                long_lived["max_post_operation_retained"],
                long_lived["final_retained"],
            ]
        )
        if not all(_retained_is_zero(snapshot) for snapshot in retained_snapshots):
            raise LifecycleContractError(
                "candidate retained State Rough entries or owned bytes after an operation"
            )
        checks.extend(
            [
                "zero_cross_operation_hits",
                "zero_post_operation_retained_state",
                "zero_max_post_operation_retained_state",
                "request_count_independent_retention",
            ]
        )
    else:
        raise LifecycleContractError(f"unsupported lifecycle mode: {mode}")
    return tuple(checks)


def parse_probe_output(
    stdout: str,
    stderr: str,
    *,
    contract: Mapping[str, object],
) -> dict[str, object]:
    receipt_contract = contract["receipt"]
    assert isinstance(receipt_contract, Mapping)
    marker = str(receipt_contract["marker"])
    combined = f"{stdout}\n{stderr}"
    if combined.count(marker) != 1:
        raise LifecycleContractError("probe output must contain exactly one lifecycle marker")
    payload = combined.split(marker, 1)[1].splitlines()[0].strip()
    if not payload:
        raise LifecycleContractError("lifecycle marker has no JSON payload")
    return _validate_receipt(
        strict_json_text(payload, source="State Rough lifecycle marker"),
        contract=contract,
    )


def parse_controls_output(
    stdout: str,
    stderr: str,
    *,
    contract: Mapping[str, object],
) -> dict[str, object]:
    controls_contract = contract["controls"]
    assert isinstance(controls_contract, Mapping)
    marker = str(controls_contract["marker"])
    combined = f"{stdout}\n{stderr}"
    if combined.count(marker) != 1:
        raise LifecycleContractError(
            "controls output must contain exactly one lifecycle marker"
        )
    payload = combined.split(marker, 1)[1].splitlines()[0].strip()
    if not payload:
        raise LifecycleContractError("controls lifecycle marker has no JSON payload")
    return _validate_controls_receipt(
        strict_json_text(payload, source="State Rough lifecycle controls marker"),
        contract=contract,
    )


def _toolchain_command(toolchain: str | None, executable: str, *args: str) -> list[str]:
    if toolchain:
        return ["rustup", "run", toolchain, executable, *args]
    return [executable, *args]


def build_test_executable(
    root: Path,
    *,
    contract: Mapping[str, object],
    target_dir: Path,
    toolchain: str | None,
    timeout_seconds: int,
) -> tuple[Path, dict[str, object]]:
    probe = contract["probe"]
    assert isinstance(probe, Mapping)
    command = _toolchain_command(
        toolchain,
        "cargo",
        "test",
        "--locked",
        "-p",
        str(probe["package"]),
        "--lib",
        "--no-run",
        "--message-format=json-render-diagnostics",
        "--target-dir",
        str(target_dir),
    )
    if probe["default_features"] is False:
        command.append("--no-default-features")
    features = tuple(probe["features"])
    if features:
        command.extend(("--features", ",".join(features)))

    environment = os.environ.copy()
    environment.update({"CARGO_BUILD_JOBS": "1", "CARGO_INCREMENTAL": "0"})
    try:
        completed = subprocess.run(
            command,
            cwd=root,
            env=environment,
            capture_output=True,
            text=True,
            check=False,
            timeout=timeout_seconds,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise LifecycleContractError(f"cannot build lifecycle probe: {error}") from error
    if completed.returncode != 0:
        raise LifecycleContractError(
            "lifecycle probe build failed: " + completed.stderr[-2_000:]
        )

    target_name = str(probe["package"]).replace("-", "_")
    executables: set[Path] = set()
    for index, line in enumerate(completed.stdout.splitlines()):
        if not line.strip():
            continue
        message = strict_json_text(line, source=f"cargo message line {index + 1}")
        if not isinstance(message, Mapping) or message.get("reason") != "compiler-artifact":
            continue
        target = message.get("target")
        profile = message.get("profile")
        executable = message.get("executable")
        if not isinstance(target, Mapping) or not isinstance(profile, Mapping):
            continue
        if (
            target.get("name") == target_name
            and target.get("kind") == ["lib"]
            and profile.get("test") is True
            and isinstance(executable, str)
            and executable
        ):
            executables.add(Path(executable).resolve())
    if len(executables) != 1:
        raise LifecycleContractError(
            f"cargo produced {len(executables)} matching merman-render lib test executables"
        )
    executable = next(iter(executables))
    descriptor = _describe_file(executable, root=root, absolute_path=True)
    return executable, {
        "command": command,
        "environment": {"CARGO_BUILD_JOBS": "1", "CARGO_INCREMENTAL": "0"},
        "cargo_stdout_sha256": _sha256_bytes(completed.stdout.encode("utf-8")),
        "cargo_stderr_sha256": _sha256_bytes(completed.stderr.encode("utf-8")),
        "executable": descriptor,
    }


def run_probe(
    executable: Path,
    *,
    contract: Mapping[str, object],
    timeout_seconds: int,
) -> tuple[dict[str, object], dict[str, object]]:
    probe = contract["probe"]
    assert isinstance(probe, Mapping)
    command = [
        str(executable),
        str(probe["test_name"]),
        "--exact",
        "--ignored",
        "--nocapture",
        "--test-threads=1",
    ]
    try:
        completed = subprocess.run(
            command,
            capture_output=True,
            text=True,
            check=False,
            timeout=timeout_seconds,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise LifecycleContractError(f"cannot run lifecycle probe: {error}") from error
    if completed.returncode != 0:
        raise LifecycleContractError(
            "lifecycle probe failed: " + (completed.stdout + completed.stderr)[-2_000:]
        )
    receipt = parse_probe_output(completed.stdout, completed.stderr, contract=contract)
    return receipt, {
        "command": command,
        "timeout_seconds": timeout_seconds,
        "returncode": completed.returncode,
        "stdout_sha256": _sha256_bytes(completed.stdout.encode("utf-8")),
        "stderr_sha256": _sha256_bytes(completed.stderr.encode("utf-8")),
    }


def run_controls(
    executable: Path,
    *,
    contract: Mapping[str, object],
    timeout_seconds: int,
) -> tuple[dict[str, object], dict[str, object]]:
    controls = contract["controls"]
    assert isinstance(controls, Mapping)
    command = [
        str(executable),
        str(controls["test_name"]),
        "--exact",
        "--ignored",
        "--nocapture",
        "--test-threads=1",
    ]
    try:
        completed = subprocess.run(
            command,
            capture_output=True,
            text=True,
            check=False,
            timeout=timeout_seconds,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise LifecycleContractError(
            f"cannot run lifecycle controls: {error}"
        ) from error
    if completed.returncode != 0:
        raise LifecycleContractError(
            "lifecycle controls failed: "
            + (completed.stdout + completed.stderr)[-2_000:]
        )
    receipt = parse_controls_output(
        completed.stdout, completed.stderr, contract=contract
    )
    return receipt, {
        "command": command,
        "timeout_seconds": timeout_seconds,
        "returncode": completed.returncode,
        "stdout_sha256": _sha256_bytes(completed.stdout.encode("utf-8")),
        "stderr_sha256": _sha256_bytes(completed.stderr.encode("utf-8")),
    }


def _output_projection(receipt: Mapping[str, object]) -> dict[str, object]:
    requests = receipt["requests"]
    long_lived = receipt["long_lived"]
    rollup = receipt["rollup"]
    assert isinstance(requests, list)
    assert isinstance(long_lived, Mapping)
    assert isinstance(rollup, Mapping)
    return {
        "detailed": [
            {
                "ordinal": request["ordinal"],
                "svg": request["svg"],
            }
            for request in requests
        ],
        "total_rollup_svg": rollup["svg"],
        "long_lived_svg": long_lived["svg"],
    }


def _schedule_projection(receipt: Mapping[str, object]) -> dict[str, object]:
    long_lived = receipt["long_lived"]
    assert isinstance(long_lived, Mapping)
    return {
        "schedule": receipt["schedule"],
        "detailed_request_inputs": [
            {
                "ordinal": request["ordinal"],
                "case": request["case"],
                "render_thread": request["render_thread"],
                "geometry_label_bytes": request["geometry_label_bytes"],
                "ordinary_nodes": request["ordinary_nodes"],
                "configured_seed": request["operation"]["configured_seed"],
            }
            for request in receipt["requests"]
        ],
        "long_lived_request_count": long_lived["request_count"],
        "long_lived_request_count_checkpoints": long_lived[
            "request_count_checkpoints"
        ],
        "long_lived_checkpoint_inputs": [
            {
                "request_count": checkpoint["request_count"],
                "geometry_label_bytes": checkpoint["geometry_label_bytes"],
                "configured_seed": checkpoint["configured_seed"],
            }
            for checkpoint in long_lived["checkpoints"]
        ],
    }


def _control_operation_semantics(
    operation: Mapping[str, object],
) -> dict[str, object]:
    return {
        "configured_seed": operation["configured_seed"],
        "resolved_seed": operation["resolved_seed"],
        "seed_resolution": operation["seed_resolution"],
        "cache_allowed": operation["cache_allowed"],
        "outcome": operation["outcome"],
    }


def _controls_semantic_projection(
    receipt: Mapping[str, object],
) -> dict[str, object]:
    error = receipt["error"]
    unwind = receipt["unwind"]
    concurrency = receipt["concurrency"]
    assert isinstance(error, Mapping)
    assert isinstance(unwind, Mapping)
    assert isinstance(concurrency, Mapping)
    operations = concurrency["operations"]
    assert isinstance(operations, list)
    return {
        "error": {
            "sentinel": error["sentinel"],
            "operation": _control_operation_semantics(error["operation"]),
        },
        "unwind": {
            "sentinel": unwind["sentinel"],
            "operation": _control_operation_semantics(unwind["operation"]),
        },
        "concurrency": {
            "workers": concurrency["workers"],
            "overlap_observed": concurrency["overlap_observed"],
            "serial_svg": concurrency["serial_svg"],
            "worker_svgs": concurrency["worker_svgs"],
            "operations": [
                _control_operation_semantics(operation) for operation in operations
            ],
        },
    }


def _validate_success_report(
    value: object,
    *,
    lane: LaneMetadata,
    owner_contract: Mapping[str, object],
    expected_contract: Mapping[str, object],
    expected_harness: Mapping[str, object],
    expected_host: Mapping[str, object],
) -> dict[str, object]:
    report = _object(value, fields=_REPORT_FIELDS, context="baseline driver report")
    if report["schema"] != DRIVER_SCHEMA:
        raise LifecycleContractError("baseline driver report schema drifted")
    if report["mode"] != "baseline" or report["outcome"] != "pass" or report["exit_code"] != 0:
        raise LifecycleContractError("baseline driver report did not pass baseline mode")
    _string(report["generated_at_utc"], context="baseline driver report.generated_at_utc")
    report_lane = _object(
        report["lane"], fields=_REPORT_LANE_FIELDS, context="baseline driver report.lane"
    )
    if report_lane != _lane_report(lane):
        raise LifecycleContractError("baseline driver report lane provenance drifted")
    report_contract = _validate_file_descriptor(
        report["contract"], context="baseline driver report.contract"
    )
    if report_contract != expected_contract:
        raise LifecycleContractError("baseline owner-contract provenance drifted")
    source = _object(
        report["source"], fields=_SOURCE_FIELDS, context="baseline driver report.source"
    )
    for field in ("commit", "tree", "first_parent_commit", "first_parent_tree"):
        _git_object_id(
            source[field], context=f"baseline driver report.source.{field}"
        )
    _positive_int(
        source["parent_count"],
        context="baseline driver report.source.parent_count",
    )
    if _boolean(source["clean"], context="baseline driver report.source.clean") is not True:
        raise LifecycleContractError("baseline lifecycle evidence must come from a clean worktree")
    dirty_status_sha256 = _sha256(
        source["dirty_status_sha256"],
        context="baseline driver report.source.dirty_status_sha256",
    )
    if dirty_status_sha256 != _EMPTY_GIT_STATUS_SHA256:
        raise LifecycleContractError(
            "clean baseline source must hash an empty git-status payload"
        )
    host = _object(
        report["host"], fields=_HOST_FIELDS, context="baseline driver report.host"
    )
    for field in _HOST_FIELDS:
        _string(host[field], context=f"baseline driver report.host.{field}")
    if host != expected_host:
        raise LifecycleContractError("baseline host provenance differs from candidate host")
    harness = _object(
        report["harness"],
        fields=_HARNESS_FIELDS,
        context="baseline driver report.harness",
    )
    for field in _HARNESS_FIELDS:
        _validate_file_descriptor(
            harness[field], context=f"baseline driver report.harness.{field}"
        )
    if harness != expected_harness:
        raise LifecycleContractError("baseline lifecycle harness provenance drifted")
    build = _object(
        report["build"], fields=_BUILD_FIELDS, context="baseline driver report.build"
    )
    build_command = _string_list(
        build["command"],
        context="baseline driver report.build.command",
        allow_empty=False,
    )
    probe_contract = owner_contract["probe"]
    assert isinstance(probe_contract, Mapping)
    for required in (
        "cargo",
        "test",
        "--locked",
        "-p",
        str(probe_contract["package"]),
        "--lib",
        "--no-run",
        "--message-format=json-render-diagnostics",
    ):
        if required not in build_command:
            raise LifecycleContractError(
                f"baseline build command is missing registered argument {required!r}"
            )
    build_environment = _object(
        build["environment"],
        fields=_BUILD_ENVIRONMENT_FIELDS,
        context="baseline driver report.build.environment",
    )
    if build_environment != {"CARGO_BUILD_JOBS": "1", "CARGO_INCREMENTAL": "0"}:
        raise LifecycleContractError("baseline build environment drifted")
    _sha256(
        build["cargo_stdout_sha256"],
        context="baseline driver report.build.cargo_stdout_sha256",
    )
    _sha256(
        build["cargo_stderr_sha256"],
        context="baseline driver report.build.cargo_stderr_sha256",
    )
    build_executable = _validate_file_descriptor(
        build["executable"], context="baseline driver report.build.executable"
    )
    if not Path(str(build_executable["path"])).is_absolute():
        raise LifecycleContractError(
            "baseline build executable path must be absolute across worktrees"
        )
    executable_path = Path(str(build_executable["path"]))
    if not executable_path.is_absolute():
        raise LifecycleContractError(
            "baseline built test executable path must be absolute"
        )
    probe = _object(
        report["probe"],
        fields=_PROBE_REPORT_FIELDS,
        context="baseline driver report.probe",
    )
    probe_command = _string_list(
        probe["command"],
        context="baseline driver report.probe.command",
        allow_empty=False,
    )
    if probe_command[1:] != (
        str(probe_contract["test_name"]),
        "--exact",
        "--ignored",
        "--nocapture",
        "--test-threads=1",
    ):
        raise LifecycleContractError("baseline probe command drifted from the exact ignored test")
    _positive_int(
        probe["timeout_seconds"], context="baseline driver report.probe.timeout_seconds"
    )
    if probe["returncode"] != 0:
        raise LifecycleContractError("baseline lifecycle probe did not exit successfully")
    _sha256(probe["stdout_sha256"], context="baseline driver report.probe.stdout_sha256")
    _sha256(probe["stderr_sha256"], context="baseline driver report.probe.stderr_sha256")
    controls = _object(
        report["controls"],
        fields=_CONTROLS_REPORT_FIELDS,
        context="baseline driver report.controls",
    )
    controls_probe = _object(
        controls["probe"],
        fields=_PROBE_REPORT_FIELDS,
        context="baseline driver report.controls.probe",
    )
    controls_command = _string_list(
        controls_probe["command"],
        context="baseline driver report.controls.probe.command",
        allow_empty=False,
    )
    controls_contract = owner_contract["controls"]
    assert isinstance(controls_contract, Mapping)
    if controls_command[1:] != (
        str(controls_contract["test_name"]),
        "--exact",
        "--ignored",
        "--nocapture",
        "--test-threads=1",
    ):
        raise LifecycleContractError(
            "baseline controls command drifted from the exact ignored test"
        )
    if controls_command[0] != probe_command[0]:
        raise LifecycleContractError(
            "baseline controls and decision receipt used different test executables"
        )
    probe_executable_path = Path(probe_command[0])
    if not probe_executable_path.is_absolute():
        raise LifecycleContractError(
            "baseline probe command executable path must be absolute"
        )
    if probe_executable_path.resolve() != executable_path.resolve():
        raise LifecycleContractError(
            "baseline probe command differs from the built test executable"
        )
    _positive_int(
        controls_probe["timeout_seconds"],
        context="baseline driver report.controls.probe.timeout_seconds",
    )
    if controls_probe["returncode"] != 0:
        raise LifecycleContractError("baseline lifecycle controls did not exit successfully")
    _sha256(
        controls_probe["stdout_sha256"],
        context="baseline driver report.controls.probe.stdout_sha256",
    )
    _sha256(
        controls_probe["stderr_sha256"],
        context="baseline driver report.controls.probe.stderr_sha256",
    )
    controls_receipt = _validate_controls_receipt(
        controls["receipt"], contract=owner_contract
    )
    validate_controls_mode(controls_receipt, mode="baseline")
    if report["baseline_comparison"] is not None:
        raise LifecycleContractError("baseline report cannot contain a baseline comparison")
    _string_list(report["checks"], context="baseline driver report.checks", allow_empty=False)
    if not isinstance(report["receipt"], dict):
        raise LifecycleContractError("baseline driver report has no receipt")
    return report


def compare_with_baseline(
    candidate: Mapping[str, object],
    candidate_controls: Mapping[str, object],
    baseline_path: Path,
    *,
    contract: Mapping[str, object],
    lane: LaneMetadata,
    expected_contract: Mapping[str, object],
    expected_harness: Mapping[str, object],
    expected_host: Mapping[str, object],
    expected_baseline_commit: str,
    expected_baseline_tree: str,
) -> dict[str, object]:
    raw = strict_json_path(baseline_path)
    baseline_report = _validate_success_report(
        raw,
        lane=lane,
        owner_contract=contract,
        expected_contract=expected_contract,
        expected_harness=expected_harness,
        expected_host=expected_host,
    )
    baseline_source = baseline_report["source"]
    assert isinstance(baseline_source, Mapping)
    actual_baseline_commit = str(baseline_source["commit"])
    actual_baseline_tree = str(baseline_source["tree"])
    if actual_baseline_commit != expected_baseline_commit:
        raise LifecycleContractError(
            "baseline source commit is not the candidate HEAD first parent"
        )
    if actual_baseline_tree != expected_baseline_tree:
        raise LifecycleContractError(
            "baseline source tree is not the candidate HEAD first-parent tree"
        )
    baseline_receipt = _validate_receipt(
        baseline_report["receipt"], contract=contract
    )
    validate_mode(baseline_receipt, mode="baseline")
    baseline_controls_report = baseline_report["controls"]
    assert isinstance(baseline_controls_report, Mapping)
    baseline_controls = _validate_controls_receipt(
        baseline_controls_report["receipt"], contract=contract
    )
    validate_controls_mode(baseline_controls, mode="baseline")
    schedule_equal = _schedule_projection(candidate) == _schedule_projection(
        baseline_receipt
    )
    output_equal = _output_projection(candidate) == _output_projection(baseline_receipt)
    release_control_semantics_equal = _controls_semantic_projection(
        candidate_controls
    ) == _controls_semantic_projection(baseline_controls)
    if not schedule_equal:
        raise LifecycleContractError("candidate lifecycle schedule differs from baseline")
    if not output_equal:
        raise LifecycleContractError(
            "candidate SVG SHA-256/bytes/elements differ from baseline"
        )
    if not release_control_semantics_equal:
        raise LifecycleContractError(
            "candidate release-control semantics differ from baseline"
        )
    return {
        "status": "passed",
        "baseline_path": str(baseline_path.resolve()),
        "baseline_json_sha256": _sha256_path(baseline_path),
        "contract_sha256_equal": True,
        "harness_sha256_equal": True,
        "host_equal": True,
        "revision_relation": "candidate_head_first_parent",
        "expected_baseline_commit": expected_baseline_commit,
        "actual_baseline_commit": actual_baseline_commit,
        "expected_baseline_tree": expected_baseline_tree,
        "actual_baseline_tree": actual_baseline_tree,
        "revision_equal": True,
        "schedule_equal": True,
        "svg_outputs_equal": True,
        "release_control_semantics_equal": True,
    }


def _git_provenance(root: Path) -> dict[str, object]:
    def git(*arguments: str) -> str:
        completed = subprocess.run(
            ["git", *arguments],
            cwd=root,
            capture_output=True,
            text=True,
            check=False,
        )
        if completed.returncode != 0:
            raise LifecycleContractError(
                f"git {' '.join(arguments)} failed: {completed.stderr[-1_000:]}"
            )
        return completed.stdout

    status = git("status", "--porcelain=v1", "--untracked-files=all")
    revision_line = git("rev-list", "--parents", "-n", "1", "HEAD").split()
    if len(revision_line) < 2:
        raise LifecycleContractError(
            "candidate-bound lifecycle evidence requires HEAD to have a first parent"
        )
    first_parent_commit = revision_line[1]
    return {
        "commit": revision_line[0],
        "tree": git("rev-parse", "HEAD^{tree}").strip(),
        "first_parent_commit": first_parent_commit,
        "first_parent_tree": git(
            "rev-parse", f"{first_parent_commit}^{{tree}}"
        ).strip(),
        "parent_count": len(revision_line) - 1,
        "clean": not status,
        "dirty_status_sha256": _sha256_bytes(status.encode("utf-8")),
    }


def _verify_source_unchanged(
    initial: Mapping[str, object], current: Mapping[str, object]
) -> None:
    for field in _SOURCE_FIELDS:
        if initial[field] != current[field]:
            raise LifecycleContractError(
                f"repository source disposition changed during probe: {field}"
            )


def execute(args: argparse.Namespace) -> dict[str, object]:
    root = repo_root()
    if args.mode == "candidate" and not args.baseline_json:
        raise LifecycleContractError(
            "candidate mode requires --baseline-json to prove output equivalence"
        )
    if args.mode == "baseline" and args.baseline_json:
        raise LifecycleContractError("--baseline-json is valid only in candidate mode")
    corpus_path = (root / args.corpus).resolve()
    corpus = load_corpus(corpus_path)
    lane = resolve_lane_selector(corpus, args.lane)
    contract_path = (
        (root / args.contract).resolve()
        if args.contract
        else (root / str(lane.evidence_contract or "")).resolve()
    )
    contract = load_owner_contract(contract_path, lane=lane, root=root)
    contract_descriptor = _describe_file(contract_path, root=root)
    harness_descriptor = _harness_report(root)
    host_descriptor = {
        "platform": platform.platform(),
        "machine": platform.machine(),
        "python": platform.python_version(),
    }
    source_before = _git_provenance(root)
    baseline_provenance = contract["baseline_provenance"]
    assert isinstance(baseline_provenance, Mapping)
    if args.mode == "candidate" and source_before["parent_count"] != baseline_provenance[
        "candidate_parent_count"
    ]:
        raise LifecycleContractError(
            "candidate HEAD must have exactly one parent for adjacent baseline evidence"
        )
    if not source_before["clean"]:
        if args.allow_dirty:
            raise LifecycleContractError(
                "dirty-source exploratory evidence is non-admissible; commit the source before running this contract"
            )
        raise LifecycleContractError(
            "repository is dirty; commit the source before running lifecycle evidence"
        )
    if source_before["dirty_status_sha256"] != _EMPTY_GIT_STATUS_SHA256:
        raise LifecycleContractError(
            "clean source provenance did not hash an empty git-status payload"
        )

    target_dir = Path(args.target_dir)
    if not target_dir.is_absolute():
        target_dir = root / target_dir
    executable, build = build_test_executable(
        root,
        contract=contract,
        target_dir=target_dir.resolve(),
        toolchain=args.toolchain,
        timeout_seconds=args.timeout_seconds,
    )
    controls_receipt, controls_probe = run_controls(
        executable,
        contract=contract,
        timeout_seconds=args.timeout_seconds,
    )
    checks = list(validate_controls_mode(controls_receipt, mode=args.mode))
    receipt, probe = run_probe(
        executable,
        contract=contract,
        timeout_seconds=args.timeout_seconds,
    )
    checks.extend(validate_mode(receipt, mode=args.mode))

    baseline_comparison = None
    if args.baseline_json:
        baseline_comparison = compare_with_baseline(
            receipt,
            controls_receipt,
            Path(args.baseline_json),
            contract=contract,
            lane=lane,
            expected_contract=contract_descriptor,
            expected_harness=harness_descriptor,
            expected_host=host_descriptor,
            expected_baseline_commit=str(source_before["first_parent_commit"]),
            expected_baseline_tree=str(source_before["first_parent_tree"]),
        )
        checks.extend(
            (
                "baseline_adjacent_first_parent_identity",
                "baseline_schedule_identity",
                "baseline_svg_output_identity",
                "baseline_release_control_semantic_identity",
            )
        )

    source_after = _git_provenance(root)
    _verify_source_unchanged(source_before, source_after)
    return {
        "schema": DRIVER_SCHEMA,
        "generated_at_utc": dt.datetime.now(dt.timezone.utc)
        .replace(microsecond=0)
        .isoformat()
        .replace("+00:00", "Z"),
        "mode": args.mode,
        "lane": _lane_report(lane),
        "contract": contract_descriptor,
        "source": source_before,
        "host": host_descriptor,
        "harness": harness_descriptor,
        "build": build,
        "probe": probe,
        "controls": {
            "probe": controls_probe,
            "receipt": controls_receipt,
        },
        "baseline_comparison": baseline_comparison,
        "receipt": receipt,
        "checks": checks,
        "outcome": "pass",
        "exit_code": 0,
    }


def _write_json(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Run the State Rough retained-lifecycle admission probe."
    )
    parser.add_argument("--mode", choices=("baseline", "candidate"), required=True)
    parser.add_argument("--corpus", default=str(DEFAULT_CORPUS))
    parser.add_argument("--lane", default=DEFAULT_LANE)
    parser.add_argument("--contract", default=None)
    parser.add_argument("--target-dir", default="target")
    parser.add_argument("--toolchain", default=None)
    parser.add_argument("--baseline-json", default=None)
    parser.add_argument("--json-out", default=None)
    parser.add_argument("--timeout-seconds", type=int, default=DEFAULT_TIMEOUT_SECONDS)
    parser.add_argument("--allow-dirty", action="store_true")
    args = parser.parse_args(argv)
    if args.timeout_seconds <= 0:
        parser.error("--timeout-seconds must be positive")
    return args


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv)
    output = (
        Path(args.json_out)
        if args.json_out
        else DEFAULT_REPORT_ROOT / f"state_rough_lifecycle_{args.mode}.json"
    )
    try:
        report = execute(args)
    except (LifecycleContractError, ValueError) as error:
        failure = {
            "schema": DRIVER_SCHEMA,
            "generated_at_utc": dt.datetime.now(dt.timezone.utc)
            .replace(microsecond=0)
            .isoformat()
            .replace("+00:00", "Z"),
            "mode": args.mode,
            "outcome": "fail",
            "exit_code": 2,
            "error": str(error),
        }
        _write_json(output, failure)
        print(f"State Rough lifecycle probe failed: {error}", file=sys.stderr)
        print(f"failure report: {output}", file=sys.stderr)
        return 2

    _write_json(output, report)
    print(f"State Rough lifecycle {args.mode} admission passed: {output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
