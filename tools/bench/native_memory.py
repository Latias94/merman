#!/usr/bin/env python3
"""Fail-closed contracts and statistics for native allocator evidence."""

from __future__ import annotations

import hashlib
import json
import math
import random
import statistics
from collections.abc import Mapping, Sequence
from typing import Any


MEMORY_SCALES = (1, 2, 4, 10, 32, 100)
MIN_REPEATS = 5
MAX_BOOTSTRAP_RESAMPLES = 100_000

_COMMON_PROTOCOL_FIELDS = frozenset(
    {
        "schema_version",
        "lane_id",
        "public_operation",
        "process_lifecycle",
        "engine_lifecycle",
        "logical_operations_per_estimate",
        "mode",
        "scale",
        "seed",
        "repeat",
        "pid",
        "executable_sha256",
        "invocation_id",
        "nonce",
        "output_sha256",
        "snapshot_live_bytes",
        "allocation_count",
        "allocated_bytes",
        "live_bytes_after",
        "peak_live_bytes",
        "peak_growth_bytes",
        "counter_overflowed",
        "counter_underflowed",
    }
)
_PROTOCOL_FIELDS_BY_SCHEMA = {
    1: _COMMON_PROTOCOL_FIELDS
    | frozenset(
        {
            "output_width",
            "output_height",
            "input_nodes",
            "input_edges",
        }
    ),
    2: _COMMON_PROTOCOL_FIELDS | frozenset({"workload_units", "semantic_output"}),
}
_ECHO_FIELDS = (
    "lane_id",
    "public_operation",
    "process_lifecycle",
    "engine_lifecycle",
    "logical_operations_per_estimate",
    "mode",
    "scale",
    "seed",
    "repeat",
    "executable_sha256",
    "invocation_id",
    "nonce",
)
_COUNTER_FIELDS = (
    "snapshot_live_bytes",
    "allocation_count",
    "allocated_bytes",
    "live_bytes_after",
    "peak_live_bytes",
    "peak_growth_bytes",
)
_ADJUSTED_METRICS = frozenset(
    {"allocation_count", "allocated_bytes", "peak_growth_bytes"}
)
_OUTCOMES = frozenset(
    {"pass", "inconclusive", "failed_bound", "contract_failure"}
)


class MemoryContractError(ValueError):
    """The native-memory evidence cannot be trusted."""


class _IncompleteEvidence(ValueError):
    """The evidence is valid but cannot support a decision."""


def _reject_duplicate_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise MemoryContractError(f"duplicate JSON key: {key}")
        result[key] = value
    return result


def _reject_non_finite(token: str) -> None:
    raise MemoryContractError(f"non-finite JSON number: {token}")


def strict_json_line(stdout: str) -> dict[str, object]:
    """Parse exactly one newline-terminated JSON object."""

    if not isinstance(stdout, str):
        raise MemoryContractError("stdout must be text")
    if not stdout.endswith("\n") or stdout.count("\n") != 1:
        raise MemoryContractError("stdout must contain exactly one JSON line")

    line = stdout[:-1]
    if not line or "\r" in line:
        raise MemoryContractError("stdout must contain exactly one JSON line")
    try:
        value = json.loads(
            line,
            object_pairs_hook=_reject_duplicate_keys,
            parse_constant=_reject_non_finite,
        )
    except MemoryContractError:
        raise
    except (json.JSONDecodeError, TypeError, ValueError) as exc:
        raise MemoryContractError(f"invalid JSON response: {exc}") from exc
    if not isinstance(value, dict):
        raise MemoryContractError("native-memory response must be a JSON object")
    return value


def _require_int(
    payload: Mapping[str, object],
    field: str,
    *,
    minimum: int,
) -> int:
    value = payload[field]
    if isinstance(value, bool) or not isinstance(value, int):
        raise MemoryContractError(f"{field} must be an integer")
    if value < minimum:
        raise MemoryContractError(f"{field} must be >= {minimum}")
    return value


def _require_finite_nonnegative_number(
    payload: Mapping[str, object],
    field: str,
) -> float:
    value = payload[field]
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise MemoryContractError(f"{field} must be numeric")
    number = float(value)
    if not math.isfinite(number) or number < 0:
        raise MemoryContractError(f"{field} must be finite and non-negative")
    return number


def _require_lowercase_hex(value: object, *, field: str, length: int) -> str:
    if (
        not isinstance(value, str)
        or len(value) != length
        or any(character not in "0123456789abcdef" for character in value)
    ):
        raise MemoryContractError(
            f"{field} must be {length}-character lowercase hexadecimal"
        )
    return value


def _type_sensitive_json_equal(actual: object, expected: object) -> bool:
    if isinstance(actual, Mapping) and isinstance(expected, Mapping):
        return set(actual) == set(expected) and all(
            _type_sensitive_json_equal(actual[key], expected[key]) for key in actual
        )
    if isinstance(actual, list) and isinstance(expected, list):
        return len(actual) == len(expected) and all(
            _type_sensitive_json_equal(left, right)
            for left, right in zip(actual, expected, strict=True)
        )
    return type(actual) is type(expected) and actual == expected


def _validate_json_contract_value(value: object, *, context: str) -> None:
    if value is None or isinstance(value, (str, bool, int)):
        return
    if isinstance(value, float):
        if not math.isfinite(value):
            raise MemoryContractError(f"{context} must not contain non-finite numbers")
        return
    if isinstance(value, Mapping):
        for key, nested in value.items():
            if not isinstance(key, str) or not key:
                raise MemoryContractError(f"{context} object keys must be non-empty strings")
            _validate_json_contract_value(nested, context=f"{context}.{key}")
        return
    if isinstance(value, list):
        for index, nested in enumerate(value):
            _validate_json_contract_value(nested, context=f"{context}[{index}]")
        return
    raise MemoryContractError(f"{context} contains a non-JSON value")


def validate_semantic_response_contract(
    semantic_contract: Mapping[str, object],
    *,
    semantic_output_dimensions: Sequence[str] | None = None,
) -> str:
    """Validate the declarative schema-v2 semantic response contract."""

    contract_fields = frozenset(
        {
            "scale_field",
            "operation_output_sha256",
            "zero_output_sha256",
            "operation",
            "zero",
        }
    )
    if frozenset(semantic_contract) != contract_fields:
        raise MemoryContractError("semantic response contract fields differ")

    scale_field = semantic_contract["scale_field"]
    if not isinstance(scale_field, str) or not scale_field.strip():
        raise MemoryContractError("semantic response scale_field must be non-empty")
    operation = semantic_contract["operation"]
    zero = semantic_contract["zero"]
    if not isinstance(operation, Mapping) or not isinstance(zero, Mapping):
        raise MemoryContractError("semantic response operation/zero values must be objects")
    if not operation or set(operation) != set(zero) or scale_field in operation:
        raise MemoryContractError(
            "semantic response operation/zero fields must match and exclude scale_field"
        )
    for mode, fixed in (("operation", operation), ("zero", zero)):
        _validate_json_contract_value(fixed, context=f"semantic_response.{mode}")

    _require_lowercase_hex(
        semantic_contract["operation_output_sha256"],
        field="semantic_response.operation_output_sha256",
        length=64,
    )
    zero_digest = _require_lowercase_hex(
        semantic_contract["zero_output_sha256"],
        field="semantic_response.zero_output_sha256",
        length=64,
    )
    if zero_digest != hashlib.sha256(b"").hexdigest():
        raise MemoryContractError("zero semantic response digest must identify empty output")

    if semantic_output_dimensions is not None:
        expected_dimensions = set(operation) | {scale_field}
        if set(semantic_output_dimensions) != expected_dimensions:
            raise MemoryContractError(
                "semantic response fields differ from lane semantic_output_dimensions"
            )
    return scale_field


def _validate_semantic_output(
    payload: Mapping[str, object],
    *,
    semantic_contract: Mapping[str, object],
    workload_units_per_scale: int,
) -> None:
    if (
        isinstance(workload_units_per_scale, bool)
        or not isinstance(workload_units_per_scale, int)
        or workload_units_per_scale <= 0
    ):
        raise MemoryContractError("workload_units_per_scale must be a positive integer")

    scale_field = validate_semantic_response_contract(semantic_contract)
    operation = semantic_contract["operation"]
    zero = semantic_contract["zero"]
    assert isinstance(operation, Mapping)
    assert isinstance(zero, Mapping)

    operation_digest = _require_lowercase_hex(
        semantic_contract["operation_output_sha256"],
        field="semantic_response.operation_output_sha256",
        length=64,
    )
    zero_digest = str(semantic_contract["zero_output_sha256"])

    scale = int(payload["scale"])
    expected_workload_units = scale * workload_units_per_scale
    if payload["workload_units"] != expected_workload_units:
        raise MemoryContractError("workload_units differ from the registered scale contract")
    mode = str(payload["mode"])
    fixed = operation if mode == "operation" else zero
    expected_semantic = {
        **fixed,
        scale_field: expected_workload_units if mode == "operation" else 0,
    }
    if not _type_sensitive_json_equal(payload["semantic_output"], expected_semantic):
        raise MemoryContractError("semantic output differs from the owner contract")
    expected_digest = operation_digest if mode == "operation" else zero_digest
    if payload["output_sha256"] != expected_digest:
        raise MemoryContractError("output_sha256 differs from the owner contract")


def _validate_payload(
    payload: Mapping[str, object],
    *,
    expected: Mapping[str, object] | None,
    validate_peak_invariants: bool = True,
    semantic_contract: Mapping[str, object] | None = None,
    workload_units_per_scale: int | None = None,
    require_semantic_contract: bool = False,
) -> dict[str, object]:
    raw_schema_version = payload.get("schema_version")
    if isinstance(raw_schema_version, bool) or not isinstance(raw_schema_version, int):
        raise MemoryContractError("schema_version must be an integer")
    protocol_fields = _PROTOCOL_FIELDS_BY_SCHEMA.get(raw_schema_version)
    if protocol_fields is None:
        raise MemoryContractError("unsupported native-memory schema_version")
    fields = frozenset(payload)
    if fields != protocol_fields:
        missing = sorted(protocol_fields - fields)
        unknown = sorted(fields - protocol_fields)
        raise MemoryContractError(
            f"protocol fields differ: missing={missing}, unknown={unknown}"
        )

    schema_version = _require_int(payload, "schema_version", minimum=1)

    lane_id = payload["lane_id"]
    if not isinstance(lane_id, str) or not lane_id.strip():
        raise MemoryContractError("lane_id must be a non-empty string")
    for field in ("public_operation", "process_lifecycle", "engine_lifecycle"):
        value = payload[field]
        if not isinstance(value, str) or not value.strip():
            raise MemoryContractError(f"{field} must be a non-empty string")
    _require_int(payload, "logical_operations_per_estimate", minimum=1)
    mode = payload["mode"]
    if mode not in ("operation", "zero"):
        raise MemoryContractError("mode must be 'operation' or 'zero'")

    _require_int(payload, "scale", minimum=1)
    _require_int(payload, "seed", minimum=0)
    _require_int(payload, "repeat", minimum=0)
    _require_int(payload, "pid", minimum=1)
    if schema_version == 1:
        _require_int(payload, "input_nodes", minimum=0)
        _require_int(payload, "input_edges", minimum=0)
        _require_finite_nonnegative_number(payload, "output_width")
        _require_finite_nonnegative_number(payload, "output_height")
        if semantic_contract is not None or workload_units_per_scale is not None:
            raise MemoryContractError("schema version 1 cannot use a semantic response contract")
    else:
        _require_int(payload, "workload_units", minimum=1)
        semantic_output = payload["semantic_output"]
        if not isinstance(semantic_output, Mapping) or not semantic_output:
            raise MemoryContractError("semantic_output must be a non-empty object")
        if semantic_contract is None or workload_units_per_scale is None:
            if require_semantic_contract:
                raise MemoryContractError(
                    "schema version 2 requires an owner semantic response contract"
                )
        else:
            _validate_semantic_output(
                payload,
                semantic_contract=semantic_contract,
                workload_units_per_scale=workload_units_per_scale,
            )
    for field in _COUNTER_FIELDS:
        _require_int(payload, field, minimum=0)

    _require_lowercase_hex(
        payload["executable_sha256"],
        field="executable_sha256",
        length=64,
    )
    _require_lowercase_hex(
        payload["output_sha256"],
        field="output_sha256",
        length=64,
    )
    invocation_id = payload["invocation_id"]
    if (
        not isinstance(invocation_id, str)
        or not invocation_id
        or invocation_id != invocation_id.strip()
        or len(invocation_id) > 256
    ):
        raise MemoryContractError(
            "invocation_id must be a trimmed non-empty string of at most 256 characters"
        )
    _require_lowercase_hex(payload["nonce"], field="nonce", length=32)

    for field in ("counter_overflowed", "counter_underflowed"):
        value = payload[field]
        if not isinstance(value, bool):
            raise MemoryContractError(f"{field} must be a boolean")
        if value:
            raise MemoryContractError(f"allocator reported {field}")

    if validate_peak_invariants:
        snapshot = int(payload["snapshot_live_bytes"])
        live_after = int(payload["live_bytes_after"])
        peak = int(payload["peak_live_bytes"])
        growth = int(payload["peak_growth_bytes"])
        if peak < snapshot:
            raise MemoryContractError("peak_live_bytes precedes the live-byte snapshot")
        if peak < live_after:
            raise MemoryContractError("peak_live_bytes is below live_bytes_after")
        if growth != peak - snapshot:
            raise MemoryContractError(
                "peak_growth_bytes does not match the live-byte snapshot"
            )

    if expected is not None:
        expected_fields = frozenset(expected)
        if expected_fields != frozenset(_ECHO_FIELDS):
            raise MemoryContractError("expected echo must contain the complete echo contract")
        for field in _ECHO_FIELDS:
            if payload[field] != expected[field]:
                raise MemoryContractError(f"protocol echo drift for {field}")

    return dict(payload)


def validate_response(
    stdout: str,
    stderr: str,
    *,
    expected: Mapping[str, object],
    semantic_contract: Mapping[str, object] | None = None,
    workload_units_per_scale: int | None = None,
) -> dict[str, object]:
    """Validate one completed subprocess response and its request echo."""

    if not isinstance(stderr, str) or stderr:
        raise MemoryContractError("native-memory subprocess wrote to stderr")
    return _validate_payload(
        strict_json_line(stdout),
        expected=expected,
        semantic_contract=semantic_contract,
        workload_units_per_scale=workload_units_per_scale,
        require_semantic_contract=True,
    )


def _paired_sample_index(
    samples: Sequence[Mapping[str, object]],
) -> tuple[
    dict[tuple[int, int, int], dict[str, dict[str, object]]],
    str | None,
    str | None,
]:
    if isinstance(samples, (str, bytes)) or not isinstance(samples, Sequence):
        raise MemoryContractError("samples must be a sequence")

    pairs: dict[tuple[int, int, int], dict[str, dict[str, object]]] = {}
    repeat_seeds: dict[tuple[int, int], int] = {}
    matrix_seed: int | None = None
    invocation_ids: set[str] = set()
    nonces: set[str] = set()
    lane_id: str | None = None
    executable_sha256: str | None = None
    protocol_schema_version: int | None = None
    lane_contract: tuple[object, ...] | None = None

    for raw_sample in samples:
        if not isinstance(raw_sample, Mapping):
            raise MemoryContractError("every sample must be a protocol object")
        # Matrix consumers receive samples that have already crossed validate_response().
        # Keeping peak arithmetic there lets deterministic synthetic matrices exercise
        # pairing and subtraction without pretending to be allocator transcripts.
        sample = _validate_payload(
            raw_sample,
            expected=None,
            validate_peak_invariants=False,
        )
        scale = int(sample["scale"])
        seed = int(sample["seed"])
        repeat = int(sample["repeat"])
        mode = str(sample["mode"])

        if scale not in MEMORY_SCALES:
            raise MemoryContractError(f"unregistered native-memory scale: {scale}")
        invocation_id = str(sample["invocation_id"])
        nonce = str(sample["nonce"])
        if invocation_id in invocation_ids:
            raise MemoryContractError("sample matrix contains a repeated invocation_id")
        if nonce in nonces:
            raise MemoryContractError("sample matrix contains a repeated nonce")
        invocation_ids.add(invocation_id)
        nonces.add(nonce)

        if matrix_seed is None:
            matrix_seed = seed
        elif seed != matrix_seed:
            raise MemoryContractError(
                "matched repeat vectors must use one fixed seed across every scale"
            )

        current_lane = str(sample["lane_id"])
        current_digest = str(sample["executable_sha256"])
        current_schema_version = int(sample["schema_version"])
        current_lane_contract = tuple(sample[field] for field in _ECHO_FIELDS[1:5])
        if lane_id is None:
            lane_id = current_lane
        elif lane_id != current_lane:
            raise MemoryContractError("sample matrix contains multiple lane ids")
        if executable_sha256 is None:
            executable_sha256 = current_digest
        elif executable_sha256 != current_digest:
            raise MemoryContractError("sample executable digest changed within the matrix")
        if protocol_schema_version is None:
            protocol_schema_version = current_schema_version
        elif protocol_schema_version != current_schema_version:
            raise MemoryContractError("sample matrix contains multiple protocol schemas")
        if lane_contract is None:
            lane_contract = current_lane_contract
        elif lane_contract != current_lane_contract:
            raise MemoryContractError("sample lane lifecycle contract changed within the matrix")

        repeat_slot = (scale, repeat)
        prior_seed = repeat_seeds.setdefault(repeat_slot, seed)
        if prior_seed != seed:
            raise MemoryContractError(
                f"scale {scale} repeat {repeat} has multiple seeds"
            )

        key = (scale, seed, repeat)
        pair = pairs.setdefault(key, {})
        if mode in pair:
            raise MemoryContractError(
                f"duplicate {mode} sample for scale={scale}, seed={seed}, repeat={repeat}"
            )
        pair[mode] = sample

    for (scale, seed, repeat), pair in pairs.items():
        if frozenset(pair) != frozenset({"operation", "zero"}):
            raise MemoryContractError(
                f"unmatched operation/zero pair for scale={scale}, seed={seed}, repeat={repeat}"
            )
        schema_version = int(pair["operation"]["schema_version"])
        if schema_version == 1:
            for field in ("input_nodes", "input_edges"):
                if pair["operation"][field] != pair["zero"][field]:
                    raise MemoryContractError(
                        f"paired operation/zero input evidence differs for {field}"
                    )
        elif pair["operation"]["workload_units"] != pair["zero"]["workload_units"]:
            raise MemoryContractError(
                "paired operation/zero workload_units evidence differs"
            )

        zero = pair["zero"]
        zero_output_is_empty = zero["output_sha256"] == hashlib.sha256(b"").hexdigest()
        if schema_version == 1:
            zero_output_is_empty = (
                zero_output_is_empty
                and float(zero["output_width"]) == 0.0
                and float(zero["output_height"]) == 0.0
            )
        if not zero_output_is_empty:
            raise MemoryContractError("zero-work output signature is not empty")

    operation_signatures: dict[int, tuple[object, ...]] = {}
    for (scale, _seed, _repeat), pair in sorted(pairs.items()):
        operation = pair["operation"]
        if int(operation["schema_version"]) == 1:
            signature = (
                operation["output_sha256"],
                operation["output_width"],
                operation["output_height"],
                operation["input_nodes"],
                operation["input_edges"],
            )
        else:
            signature = (
                operation["output_sha256"],
                operation["workload_units"],
                json.dumps(
                    operation["semantic_output"],
                    sort_keys=True,
                    separators=(",", ":"),
                    allow_nan=False,
                ),
            )
        previous = operation_signatures.setdefault(scale, signature)
        if previous != signature:
            raise MemoryContractError(
                f"operation output is not deterministic at scale {scale}"
            )

    return pairs, lane_id, executable_sha256


def validate_sample_matrix(
    samples: Sequence[Mapping[str, object]],
) -> dict[str, object]:
    """Validate pair integrity and report whether the six-point matrix is complete."""

    pairs, lane_id, executable_sha256 = _paired_sample_index(samples)
    repeat_ids: dict[int, set[int]] = {scale: set() for scale in MEMORY_SCALES}
    for scale, _seed, repeat in pairs:
        repeat_ids[scale].add(repeat)

    repeat_count_by_scale = {
        scale: len(repeat_ids[scale]) for scale in MEMORY_SCALES
    }
    incomplete_reasons: list[str] = []
    for scale in MEMORY_SCALES:
        count = repeat_count_by_scale[scale]
        if count == 0:
            incomplete_reasons.append(f"missing registered scale {scale}x")
        elif count < MIN_REPEATS:
            incomplete_reasons.append(
                f"scale {scale}x has {count} repeats; at least {MIN_REPEATS} required"
            )

    populated_vectors = [repeat_ids[scale] for scale in MEMORY_SCALES if repeat_ids[scale]]
    if populated_vectors and any(
        vector != populated_vectors[0] for vector in populated_vectors[1:]
    ):
        incomplete_reasons.append("repeat ids do not form matched vectors across scales")

    present_scales = tuple(scale for scale in MEMORY_SCALES if repeat_ids[scale])
    return {
        "complete": not incomplete_reasons,
        "scales": present_scales,
        "repeat_count_by_scale": repeat_count_by_scale,
        "incomplete_reasons": tuple(incomplete_reasons),
        "pair_count": len(pairs),
        "lane_id": lane_id,
        "executable_sha256": executable_sha256,
    }


def paired_adjustments(
    samples: Sequence[Mapping[str, object]],
    *,
    metric: str,
) -> dict[int, tuple[int, ...]]:
    """Subtract the matched zero-work value from every operation value."""

    if metric not in _ADJUSTED_METRICS:
        raise MemoryContractError(f"unsupported adjusted memory metric: {metric}")
    pairs, _lane_id, _executable_sha256 = _paired_sample_index(samples)

    values: dict[int, list[tuple[int, int, int]]] = {}
    for (scale, seed, repeat), pair in pairs.items():
        operation = int(pair["operation"][metric])
        zero = int(pair["zero"][metric])
        values.setdefault(scale, []).append((repeat, seed, operation - zero))

    return {
        scale: tuple(value for _repeat, _seed, value in sorted(entries))
        for scale, entries in sorted(values.items())
    }


def _finite_number(value: object, *, context: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise MemoryContractError(f"{context} must be numeric")
    result = float(value)
    if not math.isfinite(result):
        raise MemoryContractError(f"{context} must be finite")
    return result


def median_by_scale(
    adjustments: Mapping[int, Sequence[int | float]],
) -> dict[int, float]:
    """Summarize each scale using the median of paired adjustments."""

    medians: dict[int, float] = {}
    for scale, raw_values in adjustments.items():
        if isinstance(scale, bool) or not isinstance(scale, int) or scale <= 0:
            raise MemoryContractError("adjustment scales must be positive integers")
        if isinstance(raw_values, (str, bytes)) or not isinstance(raw_values, Sequence):
            raise MemoryContractError(f"adjustments for scale {scale} must be a sequence")
        values = tuple(
            _finite_number(value, context=f"scale {scale} adjustment")
            for value in raw_values
        )
        if not values:
            raise MemoryContractError(f"scale {scale} has no adjustments")
        medians[scale] = float(statistics.median(values))
    return medians


def ols_log_slope(medians: Mapping[int, int | float]) -> float:
    """Fit ordinary least squares over log(scale) and log(median)."""

    if len(medians) < 2:
        raise _IncompleteEvidence("at least two positive scales are required for a slope")
    points: list[tuple[float, float]] = []
    for scale, raw_median in sorted(medians.items()):
        if isinstance(scale, bool) or not isinstance(scale, int) or scale <= 0:
            raise MemoryContractError("slope scales must be positive integers")
        median = _finite_number(raw_median, context=f"scale {scale} median")
        if median <= 0:
            raise _IncompleteEvidence(f"scale {scale} median is non-positive")
        points.append((math.log(float(scale)), math.log(median)))

    mean_x = statistics.fmean(point[0] for point in points)
    mean_y = statistics.fmean(point[1] for point in points)
    denominator = sum((x - mean_x) ** 2 for x, _y in points)
    if denominator == 0:
        raise _IncompleteEvidence("scale vector has no variance")
    numerator = sum((x - mean_x) * (y - mean_y) for x, y in points)
    return numerator / denominator


def _normalized_adjustments(
    adjustments: Mapping[int, Sequence[int | float]],
) -> dict[int, tuple[float, ...]]:
    if not isinstance(adjustments, Mapping):
        raise MemoryContractError("adjustments must be a mapping")
    if any(
        isinstance(scale, bool) or not isinstance(scale, int) or scale <= 0
        for scale in adjustments
    ):
        raise MemoryContractError("adjustment scales must be positive integers")
    raw_scales = set(adjustments)
    unknown = raw_scales - set(MEMORY_SCALES)
    if unknown:
        raise MemoryContractError(f"unregistered adjustment scales: {sorted(unknown)}")
    missing = set(MEMORY_SCALES) - raw_scales
    if missing:
        raise _IncompleteEvidence(f"missing registered scales: {sorted(missing)}")

    normalized: dict[int, tuple[float, ...]] = {}
    for scale in MEMORY_SCALES:
        raw_values = adjustments[scale]
        if isinstance(raw_values, (str, bytes)) or not isinstance(raw_values, Sequence):
            raise MemoryContractError(f"adjustments for scale {scale} must be a sequence")
        values = tuple(
            _finite_number(value, context=f"scale {scale} adjustment")
            for value in raw_values
        )
        if len(values) < MIN_REPEATS:
            raise _IncompleteEvidence(
                f"scale {scale} has {len(values)} repeats; at least {MIN_REPEATS} required"
            )
        normalized[scale] = values

    lengths = {len(values) for values in normalized.values()}
    if len(lengths) != 1:
        raise _IncompleteEvidence("repeat counts do not form matched vectors across scales")
    return normalized


def _percentile(values: Sequence[float], probability: float) -> float:
    ordered = sorted(values)
    if not ordered:
        raise MemoryContractError("bootstrap distribution is empty")
    if not 0.0 <= probability <= 1.0:
        raise MemoryContractError("bootstrap percentile must be between zero and one")
    index = max(0, math.ceil(probability * len(ordered)) - 1)
    return ordered[index]


def _validate_bootstrap_request(*, seed_material: str, resamples: int) -> None:
    if not isinstance(seed_material, str) or not seed_material:
        raise MemoryContractError("seed_material must be a non-empty string")
    if isinstance(resamples, bool) or not isinstance(resamples, int) or resamples <= 0:
        raise MemoryContractError("resamples must be a positive integer")
    if resamples > MAX_BOOTSTRAP_RESAMPLES:
        raise MemoryContractError(
            f"resamples must be at most {MAX_BOOTSTRAP_RESAMPLES}"
        )


def _bootstrap_memory_bounds_from_normalized(
    normalized: Mapping[int, Sequence[float]],
    medians: Mapping[int, float],
    *,
    seed_material: str,
    resamples: int,
) -> dict[str, object]:
    estimate = ols_log_slope(medians)
    max_median = medians[MEMORY_SCALES[-1]]
    if max_median <= 0:
        raise _IncompleteEvidence("100x median is non-positive")

    bootstrap_seed = int.from_bytes(
        hashlib.sha256(seed_material.encode("utf-8")).digest()[:8],
        "big",
    )
    rng = random.Random(bootstrap_seed)
    repeat_count = len(normalized[MEMORY_SCALES[0]])
    slope_distribution: list[float] = []
    max_distribution: list[float] = []

    for _ in range(resamples):
        indices = tuple(rng.randrange(repeat_count) for _ in range(repeat_count))
        sample_medians = {
            scale: float(statistics.median(values[index] for index in indices))
            for scale, values in normalized.items()
        }
        try:
            sample_slope = ols_log_slope(sample_medians)
        except _IncompleteEvidence as exc:
            raise _IncompleteEvidence(
                "a matched-vector bootstrap resample has a non-positive median"
            ) from exc
        sample_max = sample_medians[MEMORY_SCALES[-1]]
        if sample_max <= 0:
            raise _IncompleteEvidence(
                "a matched-vector bootstrap resample has a non-positive 100x median"
            )
        slope_distribution.append(sample_slope)
        max_distribution.append(sample_max)

    return {
        "bootstrap_seed": bootstrap_seed,
        "resamples": resamples,
        "slope": {
            "estimate": estimate,
            "lower_95": min(estimate, _percentile(slope_distribution, 0.05)),
            "upper_95": max(estimate, _percentile(slope_distribution, 0.95)),
        },
        "max_scale": {
            "scale": MEMORY_SCALES[-1],
            "median": max_median,
            "lower_95": min(max_median, _percentile(max_distribution, 0.05)),
            "upper_95": max(max_median, _percentile(max_distribution, 0.95)),
        },
    }


def bootstrap_memory_bounds(
    adjustments: Mapping[int, Sequence[int | float]],
    *,
    seed_material: str,
    resamples: int = 4096,
) -> dict[str, object]:
    """Bootstrap matched repeat vectors for slope and max-scale upper bounds."""

    _validate_bootstrap_request(
        seed_material=seed_material,
        resamples=resamples,
    )
    normalized = _normalized_adjustments(adjustments)
    medians = median_by_scale(normalized)
    return _bootstrap_memory_bounds_from_normalized(
        normalized,
        medians,
        seed_material=seed_material,
        resamples=resamples,
    )


def classify_memory_metric(
    adjustments: Mapping[int, Sequence[int | float]],
    *,
    slope_cap: float,
    max_scale_cap: float,
    seed_material: str,
    resamples: int = 4096,
) -> dict[str, object]:
    """Classify valid memory evidence against one-sided slope and absolute caps."""

    slope_limit = _finite_number(slope_cap, context="slope_cap")
    absolute_limit = _finite_number(max_scale_cap, context="max_scale_cap")
    if absolute_limit <= 0:
        raise MemoryContractError("max_scale_cap must be positive")

    try:
        normalized = _normalized_adjustments(adjustments)
        medians = median_by_scale(normalized)
        non_positive = [scale for scale, median in medians.items() if median <= 0]
        if non_positive:
            raise _IncompleteEvidence(
                f"non-positive adjusted medians at scales: {non_positive}"
            )
        _validate_bootstrap_request(
            seed_material=seed_material,
            resamples=resamples,
        )
        bounds = _bootstrap_memory_bounds_from_normalized(
            normalized,
            medians,
            seed_material=seed_material,
            resamples=resamples,
        )
    except _IncompleteEvidence as exc:
        return {
            "outcome": "inconclusive",
            "reasons": (str(exc),),
            "bounds": None,
            "caps": {
                "slope": slope_limit,
                "max_scale": absolute_limit,
            },
        }

    bound_inputs = (
        (
            "slope",
            float(bounds["slope"]["lower_95"]),  # type: ignore[index]
            float(bounds["slope"]["upper_95"]),  # type: ignore[index]
            slope_limit,
        ),
        (
            "100x",
            float(bounds["max_scale"]["lower_95"]),  # type: ignore[index]
            float(bounds["max_scale"]["upper_95"]),  # type: ignore[index]
            absolute_limit,
        ),
    )
    states: list[str] = []
    reasons: list[str] = []
    for label, lower, upper, limit in bound_inputs:
        if upper <= limit:
            states.append("pass")
        elif lower > limit:
            states.append("failed_bound")
            reasons.append(
                f"{label} lower bound {lower:.12g} exceeds cap {limit:.12g}"
            )
        else:
            states.append("inconclusive")
            reasons.append(
                f"{label} interval [{lower:.12g}, {upper:.12g}] crosses cap {limit:.12g}"
            )

    if "failed_bound" in states:
        outcome = "failed_bound"
    elif "inconclusive" in states:
        outcome = "inconclusive"
    else:
        outcome = "pass"

    return {
        "outcome": outcome,
        "reasons": tuple(reasons),
        "bounds": bounds,
        "caps": {
            "slope": slope_limit,
            "max_scale": absolute_limit,
        },
    }


def suite_exit_code(outcomes: Sequence[str]) -> int:
    """Return the fixed evidence-failure, bound-failure, inconclusive priority."""

    if isinstance(outcomes, (str, bytes)) or not isinstance(outcomes, Sequence):
        raise MemoryContractError("outcomes must be a sequence")
    unknown = set(outcomes) - _OUTCOMES
    if unknown:
        raise MemoryContractError(f"unknown memory outcomes: {sorted(unknown)}")
    if "contract_failure" in outcomes:
        return 2
    if "failed_bound" in outcomes:
        return 1
    if "inconclusive" in outcomes:
        return 3
    return 0
