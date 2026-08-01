#!/usr/bin/env python3
"""Proof-first contracts for native memory evidence and benchmark lane metadata."""

from __future__ import annotations

import hashlib
import json
import math
import sys
import tempfile
import unittest
from pathlib import Path
from typing import Any


BENCH_DIR = Path(__file__).resolve().parent
if str(BENCH_DIR) not in sys.path:
    sys.path.insert(0, str(BENCH_DIR))

import corpus_utils

try:
    import native_memory
except ModuleNotFoundError as exc:
    native_memory = None  # type: ignore[assignment]
    NATIVE_MEMORY_IMPORT_ERROR: ModuleNotFoundError | None = exc
else:
    NATIVE_MEMORY_IMPORT_ERROR = None


MEMORY_SCALES = (1, 2, 4, 10, 32, 100)
EXECUTABLE_SHA256 = "a" * 64
OUTPUT_SHA256 = "c" * 64
EMPTY_SHA256 = hashlib.sha256(b"").hexdigest()
BINDING_DATA_SHA256 = "471b942eb8877b2ee7c38b86567b13f79fba101df1e5b4767b3421a200e0ad3f"
BINDING_METADATA_SHA256 = "5662eba44530c1a9bf01f352dd84496fb368c9522757c479f735526d200a9e29"
BINDING_OUTPUT_SHA256 = "5c5a3cdbe1f692c630006b4c365f348d93d8afd6ea99ecf45d8d2e10524acf53"


def protocol_response(
    *,
    mode: str = "operation",
    scale: int = 1,
    seed: int = 101,
    repeat: int = 0,
    pid: int = 10_000,
    executable_sha256: str = EXECUTABLE_SHA256,
    invocation_id: str | None = None,
    nonce: str | None = None,
    output_sha256: str | None = None,
    output_width: int | float | None = None,
    output_height: int | float | None = None,
    input_nodes: int = 1,
    input_edges: int = 1,
    allocation_count: int = 10,
    allocated_bytes: int = 1_000,
    snapshot_live_bytes: int = 400,
    live_bytes_after: int = 450,
    peak_live_bytes: int = 600,
    peak_growth_bytes: int = 200,
) -> dict[str, object]:
    effective_invocation_id = invocation_id or (
        f"flowchart-end-to-end-memory:{mode}:{scale}:{seed}:{repeat}"
    )
    effective_nonce = nonce or hashlib.sha256(
        effective_invocation_id.encode("utf-8")
    ).hexdigest()[:32]
    effective_output_sha256 = (
        hashlib.sha256(b"").hexdigest()
        if mode == "zero" and output_sha256 is None
        else output_sha256 or OUTPUT_SHA256
    )
    effective_output_width = (
        0 if mode == "zero" and output_width is None else scale * 10
    )
    effective_output_height = (
        0 if mode == "zero" and output_height is None else scale * 5
    )
    return {
        "schema_version": 1,
        "lane_id": "flowchart-end-to-end-memory",
        "public_operation": "render-svg",
        "process_lifecycle": "fresh-process",
        "engine_lifecycle": "reused-engine",
        "logical_operations_per_estimate": 1,
        "mode": mode,
        "scale": scale,
        "seed": seed,
        "repeat": repeat,
        "pid": pid,
        "executable_sha256": executable_sha256,
        "invocation_id": effective_invocation_id,
        "nonce": effective_nonce,
        "output_sha256": effective_output_sha256,
        "output_width": (
            effective_output_width if output_width is None else output_width
        ),
        "output_height": (
            effective_output_height if output_height is None else output_height
        ),
        "input_nodes": input_nodes,
        "input_edges": input_edges,
        "snapshot_live_bytes": snapshot_live_bytes,
        "allocation_count": allocation_count,
        "allocated_bytes": allocated_bytes,
        "live_bytes_after": live_bytes_after,
        "peak_live_bytes": peak_live_bytes,
        "peak_growth_bytes": peak_growth_bytes,
        "counter_overflowed": False,
        "counter_underflowed": False,
    }


def expected_echo(payload: dict[str, object]) -> dict[str, object]:
    return {
        key: payload[key]
        for key in (
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
    }


def binding_semantic_contract() -> dict[str, object]:
    return {
        "scale_field": "operation_calls",
        "operation_output_sha256": BINDING_OUTPUT_SHA256,
        "zero_output_sha256": EMPTY_SHA256,
        "operation": {
            "kind": "binding-operation-result-v1",
            "operation_id": "semantic-json",
            "media_type": "application/json",
            "result_data_bytes": 31,
            "result_metadata_bytes": 126,
            "result_data_sha256": BINDING_DATA_SHA256,
            "result_metadata_sha256": BINDING_METADATA_SHA256,
        },
        "zero": {
            "kind": "binding-operation-result-v1",
            "operation_id": "semantic-json",
            "media_type": "application/json",
            "result_data_bytes": 0,
            "result_metadata_bytes": 0,
            "result_data_sha256": EMPTY_SHA256,
            "result_metadata_sha256": EMPTY_SHA256,
        },
    }


def binding_protocol_response(
    *,
    mode: str = "operation",
    scale: int = 1,
    repeat: int = 0,
    pid: int = 30_000,
) -> dict[str, object]:
    zero = mode == "zero"
    contract = binding_semantic_contract()
    fixed = contract["zero" if zero else "operation"]
    assert isinstance(fixed, dict)
    invocation_id = f"binding-request-version-only-memory:{mode}:{scale}:101:{repeat}"
    return {
        "schema_version": 2,
        "lane_id": "binding-request-version-only-memory",
        "public_operation": "binding-execute-operation-semantic-json",
        "process_lifecycle": "fresh-process",
        "engine_lifecycle": "reused-engine",
        "logical_operations_per_estimate": 1,
        "mode": mode,
        "scale": scale,
        "seed": 101,
        "repeat": repeat,
        "pid": pid,
        "executable_sha256": EXECUTABLE_SHA256,
        "invocation_id": invocation_id,
        "nonce": hashlib.sha256(invocation_id.encode("utf-8")).hexdigest()[:32],
        "output_sha256": EMPTY_SHA256 if zero else BINDING_OUTPUT_SHA256,
        "workload_units": scale,
        "semantic_output": {
            **fixed,
            "operation_calls": 0 if zero else scale,
        },
        "snapshot_live_bytes": 400,
        "allocation_count": 0 if zero else scale * 10,
        "allocated_bytes": 0 if zero else scale * 1_000,
        "live_bytes_after": 400,
        "peak_live_bytes": 400 if zero else 400 + scale * 100,
        "peak_growth_bytes": 0 if zero else scale * 100,
        "counter_overflowed": False,
        "counter_underflowed": False,
    }


def complete_sample_matrix(*, repeats: int = 5) -> list[dict[str, object]]:
    samples: list[dict[str, object]] = []
    pid = 20_000
    for scale in MEMORY_SCALES:
        for repeat in range(repeats):
            seed = 101
            samples.append(
                protocol_response(
                    mode="operation",
                    scale=scale,
                    seed=seed,
                    repeat=repeat,
                    pid=pid,
                    allocation_count=scale * 20 + repeat,
                    allocated_bytes=scale * 2_000 + repeat,
                    peak_live_bytes=400 + scale * 200 + repeat,
                    peak_growth_bytes=scale * 200 + repeat,
                )
            )
            pid += 1
            samples.append(
                protocol_response(
                    mode="zero",
                    scale=scale,
                    seed=seed,
                    repeat=repeat,
                    pid=pid,
                    allocation_count=scale * 2,
                    allocated_bytes=scale * 100,
                    peak_live_bytes=400 + scale * 10,
                    peak_growth_bytes=scale * 10,
                )
            )
            pid += 1
    return samples


def power_adjustments(
    power: int,
    factors: tuple[int, ...] = (1, 2, 3, 4, 100),
) -> dict[int, tuple[float, ...]]:
    return {
        scale: tuple(float((scale**power) * factor) for factor in factors)
        for scale in MEMORY_SCALES
    }


def mixed_power_adjustments(
    powers: tuple[int, ...],
) -> dict[int, tuple[float, ...]]:
    return {
        scale: tuple(float(scale**power) for power in powers)
        for scale in MEMORY_SCALES
    }


class NativeMemoryProtocolContractsTest(unittest.TestCase):
    def api(self, name: str) -> Any:
        if native_memory is None:
            self.fail(f"native_memory.py is missing: {NATIVE_MEMORY_IMPORT_ERROR}")
        self.assertTrue(hasattr(native_memory, name), f"native_memory.{name} is missing")
        return getattr(native_memory, name)

    def contract_error(self) -> type[Exception]:
        return self.api("MemoryContractError")

    def test_memory_scales_and_minimum_repeats_are_fixed(self) -> None:
        self.assertEqual(self.api("MEMORY_SCALES"), MEMORY_SCALES)
        self.assertEqual(self.api("MIN_REPEATS"), 5)
        self.assertEqual(self.api("MAX_BOOTSTRAP_RESAMPLES"), 100_000)

    def test_strict_json_line_accepts_exactly_one_object(self) -> None:
        parse = self.api("strict_json_line")
        payload = protocol_response()

        self.assertEqual(parse(json.dumps(payload) + "\n"), payload)

    def test_strict_json_line_rejects_duplicate_keys_and_non_finite_numbers(self) -> None:
        parse = self.api("strict_json_line")
        error = self.contract_error()

        for stdout in (
            '{"schema_version":1,"scale":1,"scale":2}\n',
            '{"schema_version":1,"value":NaN}\n',
            '{"schema_version":1,"value":Infinity}\n',
            '{"schema_version":1,"value":-Infinity}\n',
        ):
            with self.subTest(stdout=stdout), self.assertRaises(error):
                parse(stdout)

    def test_strict_json_line_rejects_extra_stdout_and_non_objects(self) -> None:
        parse = self.api("strict_json_line")
        error = self.contract_error()

        for stdout in (
            '{"schema_version":1}\n{"extra":true}\n',
            "[]\n",
            "\n",
            "not-json\n",
        ):
            with self.subTest(stdout=stdout), self.assertRaises(error):
                parse(stdout)

    def test_validate_response_preserves_checked_allocator_values(self) -> None:
        validate = self.api("validate_response")
        payload = protocol_response()

        sample = validate(
            json.dumps(payload) + "\n",
            "",
            expected=expected_echo(payload),
        )

        self.assertEqual(sample["snapshot_live_bytes"], 400)
        self.assertEqual(sample["allocation_count"], 10)
        self.assertEqual(sample["allocated_bytes"], 1_000)
        self.assertEqual(sample["live_bytes_after"], 450)
        self.assertEqual(sample["peak_live_bytes"], 600)
        self.assertEqual(sample["peak_growth_bytes"], 200)
        self.assertEqual(sample["invocation_id"], payload["invocation_id"])
        self.assertEqual(sample["nonce"], payload["nonce"])
        self.assertEqual(sample["output_sha256"], OUTPUT_SHA256)
        self.assertEqual(sample["output_width"], 10)
        self.assertEqual(sample["output_height"], 5)
        self.assertEqual(sample["input_nodes"], 1)
        self.assertEqual(sample["input_edges"], 1)

        floating_dimensions = protocol_response(output_width=10.5, output_height=5.25)
        floating_sample = validate(
            json.dumps(floating_dimensions) + "\n",
            "",
            expected=expected_echo(floating_dimensions),
        )
        self.assertEqual(floating_sample["output_width"], 10.5)
        self.assertEqual(floating_sample["output_height"], 5.25)

    def test_schema_v2_validates_owner_locked_binding_semantics(self) -> None:
        validate = self.api("validate_response")
        payload = binding_protocol_response(scale=10)

        sample = validate(
            json.dumps(payload) + "\n",
            "",
            expected=expected_echo(payload),
            semantic_contract=binding_semantic_contract(),
            workload_units_per_scale=1,
        )

        self.assertEqual(sample["workload_units"], 10)
        self.assertEqual(sample["semantic_output"]["operation_calls"], 10)
        self.assertEqual(
            sample["semantic_output"]["result_data_sha256"],
            BINDING_DATA_SHA256,
        )

    def test_schema_v2_rejects_semantic_drift_and_type_confusion(self) -> None:
        validate = self.api("validate_response")
        error = self.contract_error()
        baseline = binding_protocol_response(scale=1)

        mutations = (
            lambda payload: payload["semantic_output"].__setitem__(  # type: ignore[union-attr]
                "media_type", "text/plain"
            ),
            lambda payload: payload["semantic_output"].__setitem__(  # type: ignore[union-attr]
                "operation_calls", True
            ),
            lambda payload: payload["semantic_output"].__setitem__(  # type: ignore[union-attr]
                "operation_calls", 2
            ),
            lambda payload: payload["semantic_output"].__setitem__(  # type: ignore[union-attr]
                "unexpected", "field"
            ),
            lambda payload: payload.__setitem__("workload_units", True),
            lambda payload: payload.__setitem__("output_sha256", "b" * 64),
        )
        for mutate in mutations:
            payload = json.loads(json.dumps(baseline))
            mutate(payload)
            with self.subTest(payload=payload), self.assertRaises(error):
                validate(
                    json.dumps(payload) + "\n",
                    "",
                    expected=expected_echo(baseline),
                    semantic_contract=binding_semantic_contract(),
                    workload_units_per_scale=1,
                )

    def test_schema_v2_pairs_generic_workload_evidence_without_svg_fields(self) -> None:
        validate = self.api("validate_response")
        adjust = self.api("paired_adjustments")
        operation = binding_protocol_response(mode="operation", scale=4, pid=31_000)
        zero = binding_protocol_response(mode="zero", scale=4, pid=31_001)
        samples = [
            validate(
                json.dumps(payload) + "\n",
                "",
                expected=expected_echo(payload),
                semantic_contract=binding_semantic_contract(),
                workload_units_per_scale=1,
            )
            for payload in (operation, zero)
        ]

        self.assertEqual(
            adjust(samples, metric="allocation_count"),
            {4: (40,)},
        )

        samples[1]["workload_units"] = 5
        with self.assertRaisesRegex(self.contract_error(), "workload_units"):
            adjust(samples, metric="allocation_count")

    def test_validate_response_rejects_any_stderr(self) -> None:
        validate = self.api("validate_response")
        error = self.contract_error()
        payload = protocol_response()

        with self.assertRaises(error):
            validate(
                json.dumps(payload) + "\n",
                "allocator warning\n",
                expected=expected_echo(payload),
            )

    def test_validate_response_rejects_boolean_integer_fields(self) -> None:
        validate = self.api("validate_response")
        error = self.contract_error()

        for field in (
            "scale",
            "seed",
            "repeat",
            "pid",
            "snapshot_live_bytes",
            "allocation_count",
            "allocated_bytes",
            "live_bytes_after",
            "peak_live_bytes",
            "peak_growth_bytes",
            "output_width",
            "output_height",
            "input_nodes",
            "input_edges",
        ):
            payload = protocol_response()
            payload[field] = True
            with self.subTest(field=field), self.assertRaises(error):
                validate(
                    json.dumps(payload) + "\n",
                    "",
                    expected=expected_echo(protocol_response()),
                )

    def test_validate_response_rejects_echo_drift(self) -> None:
        validate = self.api("validate_response")
        error = self.contract_error()
        baseline = protocol_response()

        replacements: dict[str, object] = {
            "lane_id": "different-lane",
            "mode": "zero",
            "scale": 2,
            "seed": 102,
            "repeat": 1,
            "executable_sha256": "b" * 64,
            "invocation_id": "different-invocation",
            "nonce": "d" * 32,
        }
        for field, replacement in replacements.items():
            payload = dict(baseline)
            payload[field] = replacement
            with self.subTest(field=field), self.assertRaises(error):
                validate(
                    json.dumps(payload) + "\n",
                    "",
                    expected=expected_echo(baseline),
                )

    def test_validate_response_rejects_invalid_invocation_and_output_evidence(self) -> None:
        validate = self.api("validate_response")
        error = self.contract_error()

        replacements: tuple[tuple[str, object], ...] = (
            ("invocation_id", ""),
            ("nonce", "short"),
            ("nonce", "A" * 32),
            ("output_sha256", "short"),
            ("output_sha256", "G" * 64),
            ("output_width", -1),
            ("output_height", float("inf")),
            ("input_nodes", -1),
            ("input_edges", 1.5),
        )
        for field, replacement in replacements:
            payload = protocol_response()
            payload[field] = replacement
            with self.subTest(field=field, replacement=replacement), self.assertRaises(
                error
            ):
                validate(
                    json.dumps(payload) + "\n",
                    "",
                    expected=expected_echo(protocol_response()),
                )

        payload = protocol_response(invocation_id=" ")
        with self.assertRaisesRegex(
            error,
            r"^invocation_id must be a trimmed non-empty string of at most 256 characters$",
        ):
            validate(
                json.dumps(payload) + "\n",
                "",
                expected=expected_echo(protocol_response()),
            )

    def test_validate_response_rejects_unknown_fields_and_protocol_versions(self) -> None:
        validate = self.api("validate_response")
        error = self.contract_error()

        extra = protocol_response()
        extra["diagnostic"] = "unexpected"
        wrong_version = protocol_response()
        wrong_version["schema_version"] = 2
        missing_payloads: list[dict[str, object]] = []
        for field in (
            "invocation_id",
            "nonce",
            "output_sha256",
            "output_width",
            "output_height",
            "input_nodes",
            "input_edges",
        ):
            payload = protocol_response()
            del payload[field]
            missing_payloads.append(payload)

        for payload in (extra, wrong_version, *missing_payloads):
            with self.subTest(payload=payload), self.assertRaises(error):
                validate(
                    json.dumps(payload) + "\n",
                    "",
                    expected=expected_echo(protocol_response()),
                )

    def test_validate_response_rejects_counter_damage_and_peak_drift(self) -> None:
        validate = self.api("validate_response")
        error = self.contract_error()
        payloads: list[dict[str, object]] = []

        for flag in ("counter_overflowed", "counter_underflowed"):
            payload = protocol_response()
            payload[flag] = True
            payloads.append(payload)

        negative = protocol_response()
        negative["allocated_bytes"] = -1
        payloads.append(negative)

        bad_growth = protocol_response()
        bad_growth["peak_growth_bytes"] = 199
        payloads.append(bad_growth)

        peak_before_live = protocol_response()
        peak_before_live["peak_live_bytes"] = 449
        peak_before_live["peak_growth_bytes"] = 49
        payloads.append(peak_before_live)

        for payload in payloads:
            with self.subTest(payload=payload), self.assertRaises(error):
                validate(
                    json.dumps(payload) + "\n",
                    "",
                    expected=expected_echo(protocol_response()),
                )


class NativeMemoryMatrixContractsTest(unittest.TestCase):
    def api(self, name: str) -> Any:
        if native_memory is None:
            self.fail(f"native_memory.py is missing: {NATIVE_MEMORY_IMPORT_ERROR}")
        self.assertTrue(hasattr(native_memory, name), f"native_memory.{name} is missing")
        return getattr(native_memory, name)

    def contract_error(self) -> type[Exception]:
        return self.api("MemoryContractError")

    def test_complete_matrix_has_exact_scales_repeats_and_matched_fresh_pairs(self) -> None:
        validate = self.api("validate_sample_matrix")

        matrix = validate(complete_sample_matrix())

        self.assertTrue(matrix["complete"])
        self.assertEqual(tuple(matrix["scales"]), MEMORY_SCALES)
        self.assertEqual(
            matrix["repeat_count_by_scale"],
            {scale: 5 for scale in MEMORY_SCALES},
        )
        self.assertEqual(matrix["incomplete_reasons"], ())

    def test_missing_scale_or_too_few_repeats_is_structurally_valid_but_incomplete(self) -> None:
        validate = self.api("validate_sample_matrix")

        without_100x = [
            sample for sample in complete_sample_matrix() if sample["scale"] != 100
        ]
        four_repeats = complete_sample_matrix(repeats=4)

        for samples in (without_100x, four_repeats):
            with self.subTest(sample_count=len(samples)):
                matrix = validate(samples)
                self.assertFalse(matrix["complete"])
                self.assertTrue(matrix["incomplete_reasons"])

    def test_unregistered_scale_is_contract_failure(self) -> None:
        validate = self.api("validate_sample_matrix")
        error = self.contract_error()
        samples = complete_sample_matrix()
        samples.extend(
            [
                protocol_response(mode="operation", scale=3, pid=90_000),
                protocol_response(mode="zero", scale=3, pid=90_001),
            ]
        )

        with self.assertRaises(error):
            validate(samples)

    def test_unmatched_or_duplicate_pair_is_contract_failure(self) -> None:
        validate = self.api("validate_sample_matrix")
        error = self.contract_error()
        samples = complete_sample_matrix()

        missing_control = samples[:-1]
        duplicate_operation = samples + [
            dict(
                samples[0],
                pid=99_999,
                invocation_id="duplicate-operation",
                nonce="e" * 32,
            )
        ]

        for damaged in (missing_control, duplicate_operation):
            with self.subTest(sample_count=len(damaged)), self.assertRaises(error):
                validate(damaged)

    def test_os_pid_reuse_is_allowed_for_distinct_invocations(self) -> None:
        validate = self.api("validate_sample_matrix")
        samples = complete_sample_matrix()
        samples[1]["pid"] = samples[0]["pid"]

        self.assertTrue(validate(samples)["complete"])

    def test_repeated_invocation_id_or_nonce_is_contract_failure(self) -> None:
        validate = self.api("validate_sample_matrix")
        for field in ("invocation_id", "nonce"):
            samples = complete_sample_matrix()
            samples[-1][field] = samples[0][field]
            with self.subTest(field=field), self.assertRaisesRegex(
                self.contract_error(), field
            ):
                validate(samples)

    def test_matrix_requires_one_seed_across_all_scales_and_repeats(self) -> None:
        validate = self.api("validate_sample_matrix")
        samples = complete_sample_matrix()
        samples[-1]["seed"] = 102

        with self.assertRaisesRegex(self.contract_error(), "fixed seed"):
            validate(samples)

    def test_operation_output_is_deterministic_and_zero_output_is_empty(self) -> None:
        validate = self.api("validate_sample_matrix")
        operation_drift = complete_sample_matrix()
        operation_drift[-2]["output_sha256"] = "b" * 64
        nonempty_zero = complete_sample_matrix()
        nonempty_zero[-1]["output_width"] = 1

        with self.assertRaisesRegex(self.contract_error(), "deterministic"):
            validate(operation_drift)
        with self.assertRaisesRegex(self.contract_error(), "zero-work"):
            validate(nonempty_zero)

    def test_executable_digest_change_is_contract_failure(self) -> None:
        validate = self.api("validate_sample_matrix")
        error = self.contract_error()
        samples = complete_sample_matrix()
        samples[-1]["executable_sha256"] = "b" * 64

        with self.assertRaises(error):
            validate(samples)

    def test_paired_subtraction_preserves_negative_adjustments(self) -> None:
        adjust = self.api("paired_adjustments")
        operation = protocol_response(
            mode="operation",
            allocation_count=5,
            allocated_bytes=50,
            peak_growth_bytes=7,
            pid=30_000,
        )
        control = protocol_response(
            mode="zero",
            allocation_count=8,
            allocated_bytes=80,
            peak_growth_bytes=9,
            pid=30_001,
        )

        self.assertEqual(
            adjust([operation, control], metric="allocation_count"),
            {1: (-3,)},
        )
        self.assertEqual(
            adjust([operation, control], metric="allocated_bytes"),
            {1: (-30,)},
        )
        self.assertEqual(
            adjust([operation, control], metric="peak_growth_bytes"),
            {1: (-2,)},
        )

    def test_median_by_scale_uses_adjusted_repeats(self) -> None:
        summarize = self.api("median_by_scale")

        self.assertEqual(
            summarize(
                {
                    1: (-5, -1, -3, -2, -4),
                    2: (10, 40, 20, 50, 30),
                }
            ),
            {1: -3.0, 2: 30.0},
        )

    def test_ols_log_slope_recovers_known_power(self) -> None:
        slope = self.api("ols_log_slope")
        medians = {scale: float(7 * scale**2) for scale in MEMORY_SCALES}

        self.assertAlmostEqual(slope(medians), 2.0, places=12)

    def test_bootstrap_resamples_matched_repeat_vectors_deterministically(self) -> None:
        bootstrap = self.api("bootstrap_memory_bounds")
        adjustments = power_adjustments(2)
        seed_material = "flowchart-end-to-end-memory:allocated_bytes"

        first = bootstrap(adjustments, seed_material=seed_material, resamples=512)
        second = bootstrap(adjustments, seed_material=seed_material, resamples=512)

        self.assertEqual(first, second)
        self.assertEqual(
            first["bootstrap_seed"],
            int.from_bytes(
                hashlib.sha256(seed_material.encode("utf-8")).digest()[:8],
                "big",
            ),
        )
        self.assertAlmostEqual(first["slope"]["estimate"], 2.0, places=12)
        self.assertAlmostEqual(first["slope"]["lower_95"], 2.0, places=12)
        self.assertAlmostEqual(first["slope"]["upper_95"], 2.0, places=12)
        self.assertEqual(first["max_scale"]["scale"], 100)
        self.assertEqual(first["max_scale"]["median"], 30_000.0)
        self.assertLessEqual(
            first["max_scale"]["lower_95"],
            first["max_scale"]["median"],
        )
        self.assertGreaterEqual(
            first["max_scale"]["upper_95"],
            first["max_scale"]["median"],
        )

    def test_bootstrap_rejects_an_unbounded_resample_count(self) -> None:
        bootstrap = self.api("bootstrap_memory_bounds")

        with self.assertRaisesRegex(self.contract_error(), "at most"):
            bootstrap(
                power_adjustments(1),
                seed_material="bounded",
                resamples=self.api("MAX_BOOTSTRAP_RESAMPLES") + 1,
            )

    def test_non_positive_or_incomplete_evidence_is_inconclusive(self) -> None:
        classify = self.api("classify_memory_metric")
        non_positive = power_adjustments(1)
        non_positive[10] = (-1.0, -1.0, -1.0, -1.0, -1.0)
        missing_scale = power_adjustments(1)
        del missing_scale[32]
        too_few_repeats = power_adjustments(1)
        too_few_repeats[4] = too_few_repeats[4][:-1]

        for adjustments in (non_positive, missing_scale, too_few_repeats):
            with self.subTest(scales=tuple(adjustments)):
                result = classify(
                    adjustments,
                    slope_cap=1.25,
                    max_scale_cap=100_000.0,
                    seed_material="incomplete",
                    resamples=128,
                )
                self.assertEqual(result["outcome"], "inconclusive")
                self.assertTrue(result["reasons"])

    def test_slope_and_absolute_cap_use_two_sided_bootstrap_bounds(self) -> None:
        classify = self.api("classify_memory_metric")

        passing = classify(
            power_adjustments(1, (1, 1, 1, 1, 1)),
            slope_cap=1.1,
            max_scale_cap=101.0,
            seed_material="pass",
            resamples=128,
        )
        slope_failure = classify(
            power_adjustments(2, (1, 1, 1, 1, 1)),
            slope_cap=1.5,
            max_scale_cap=1_000_000.0,
            seed_material="slope-fail",
            resamples=128,
        )
        cap_failure = classify(
            power_adjustments(1, (1, 1, 1, 1, 1)),
            slope_cap=1.1,
            max_scale_cap=99.0,
            seed_material="cap-fail",
            resamples=128,
        )

        self.assertEqual(passing["outcome"], "pass")
        self.assertEqual(slope_failure["outcome"], "failed_bound")
        self.assertEqual(cap_failure["outcome"], "failed_bound")
        self.assertGreater(slope_failure["bounds"]["slope"]["lower_95"], 1.5)
        self.assertGreater(cap_failure["bounds"]["max_scale"]["lower_95"], 99.0)

    def test_bound_equality_passes_and_crossing_intervals_are_inconclusive(self) -> None:
        classify = self.api("classify_memory_metric")

        equality = classify(
            power_adjustments(1, (1, 1, 1, 1, 1)),
            slope_cap=1.0,
            max_scale_cap=100.0,
            seed_material="equality",
            resamples=512,
        )
        slope_crossing = classify(
            mixed_power_adjustments((1, 1, 2, 2, 2)),
            slope_cap=1.5,
            max_scale_cap=20_000.0,
            seed_material="slope-crossing",
            resamples=1024,
        )
        max_crossing = classify(
            power_adjustments(1, (1, 1, 1, 2, 2)),
            slope_cap=1.1,
            max_scale_cap=150.0,
            seed_material="max-crossing",
            resamples=1024,
        )

        self.assertEqual(equality["outcome"], "pass")
        for result in (slope_crossing, max_crossing):
            with self.subTest(bounds=result["bounds"]):
                self.assertEqual(result["outcome"], "inconclusive")
                self.assertIsNotNone(result["bounds"])
                self.assertTrue(result["reasons"])

    def test_any_strict_lower_bound_excess_fails(self) -> None:
        classify = self.api("classify_memory_metric")

        slope_failure = classify(
            power_adjustments(1, (1, 1, 1, 1, 1)),
            slope_cap=math.nextafter(1.0, -math.inf),
            max_scale_cap=100.0,
            seed_material="strict-slope-bound",
            resamples=128,
        )
        max_failure = classify(
            power_adjustments(1, (1, 1, 1, 1, 1)),
            slope_cap=1.0,
            max_scale_cap=math.nextafter(100.0, -math.inf),
            seed_material="strict-max-bound",
            resamples=128,
        )

        self.assertEqual(slope_failure["outcome"], "failed_bound")
        self.assertEqual(max_failure["outcome"], "failed_bound")

    def test_suite_exit_code_has_fixed_priority(self) -> None:
        exit_code = self.api("suite_exit_code")

        cases = (
            (("pass",), 0),
            (("inconclusive",), 3),
            (("failed_bound", "inconclusive"), 1),
            (("contract_failure", "failed_bound", "inconclusive"), 2),
        )
        for outcomes, expected in cases:
            with self.subTest(outcomes=outcomes):
                self.assertEqual(exit_code(outcomes), expected)


class LaneRegistryContractsTest(unittest.TestCase):
    def api(self, name: str) -> Any:
        self.assertTrue(hasattr(corpus_utils, name), f"corpus_utils.{name} is missing")
        return getattr(corpus_utils, name)

    @staticmethod
    def registry() -> dict[str, object]:
        return {
            "schema_version": 2,
            "default_group": "end_to_end",
            "suites": {"full": "All fixtures in corpus order."},
            "fixtures": [],
            "lanes": [
                {
                    "id": "compatibility-json-parse",
                    "kind": "public",
                    "owner": "merman",
                    "public_operation": "compatibility-json-parse",
                    "diagnostic_stage": None,
                    "parent_public_lane": None,
                    "process_lifecycle": "reused-process",
                    "engine_lifecycle": "reused-engine",
                    "logical_operations_per_estimate": 1,
                    "transport": "native-criterion",
                    "required_features": ["svg"],
                    "selector": "compatibility_json_parse/{fixture}",
                    "history_aliases": ["parse_known_type/{fixture}"],
                    "size_vector": [],
                    "workload": "corpus-fixture",
                    "evidence_contract": None,
                    "measurement_metrics": ["latency_ns"],
                    "semantic_output_dimensions": ["compatibility_json_bytes"],
                },
                {
                    "id": "flowchart-end-to-end-memory",
                    "kind": "public",
                    "owner": "merman",
                    "public_operation": "render-svg",
                    "diagnostic_stage": None,
                    "parent_public_lane": None,
                    "process_lifecycle": "fresh-process",
                    "engine_lifecycle": "reused-engine",
                    "logical_operations_per_estimate": 1,
                    "transport": "native-system-allocator-subprocess",
                    "required_features": ["svg"],
                    "selector": "memory/end_to_end/{fixture}",
                    "history_aliases": [],
                    "size_vector": list(MEMORY_SCALES),
                    "workload": "flowchart-modular-generator-v1",
                    "evidence_contract": (
                        "docs/performance/contracts/"
                        "flowchart-end-to-end-memory-v1.json"
                    ),
                    "measurement_metrics": [
                        "allocation_count",
                        "allocated_bytes",
                        "peak_growth_bytes",
                    ],
                    "semantic_output_dimensions": [
                        "input_nodes",
                        "input_edges",
                        "svg_sha256",
                        "svg_viewbox_width",
                        "svg_viewbox_height",
                    ],
                },
            ],
        }

    def load(self, data: dict[str, object]) -> Any:
        load_corpus = self.api("load_corpus")
        with tempfile.TemporaryDirectory() as raw:
            path = Path(raw) / "corpus.json"
            path.write_text(json.dumps(data), encoding="utf-8")
            return load_corpus(path)

    @staticmethod
    def clone(data: dict[str, object]) -> dict[str, object]:
        return json.loads(json.dumps(data))

    def test_schema_v2_loads_typed_lane_metadata(self) -> None:
        lane_type = self.api("LaneMetadata")
        corpus = self.load(self.registry())

        self.assertEqual(corpus.schema_version, 2)
        self.assertEqual(len(corpus.lanes), 2)
        lane = corpus.lanes[0]
        self.assertIsInstance(lane, lane_type)
        self.assertEqual(lane.id, "compatibility-json-parse")
        self.assertEqual(lane.kind, "public")
        self.assertEqual(lane.owner, "merman")
        self.assertEqual(lane.public_operation, "compatibility-json-parse")
        self.assertIsNone(lane.diagnostic_stage)
        self.assertIsNone(lane.parent_public_lane)
        self.assertEqual(lane.process_lifecycle, "reused-process")
        self.assertEqual(lane.engine_lifecycle, "reused-engine")
        self.assertEqual(lane.logical_operations_per_estimate, 1)
        self.assertEqual(lane.transport, "native-criterion")
        self.assertEqual(lane.required_features, ("svg",))
        self.assertEqual(lane.selector, "compatibility_json_parse/{fixture}")
        self.assertEqual(lane.history_aliases, ("parse_known_type/{fixture}",))
        self.assertEqual(lane.size_vector, ())
        self.assertEqual(lane.measurement_metrics, ("latency_ns",))
        self.assertEqual(
            lane.semantic_output_dimensions, ("compatibility_json_bytes",)
        )

    def test_schema_v1_remains_read_compatible_without_lanes(self) -> None:
        corpus = self.load(
            {
                "schema_version": 1,
                "default_group": "end_to_end",
                "suites": {},
                "fixtures": [],
            }
        )

        self.assertEqual(corpus.schema_version, 1)
        self.assertEqual(corpus.lanes, ())

    def test_schema_v2_rejects_an_empty_registry_and_duplicate_json_keys(self) -> None:
        empty = self.registry()
        empty["lanes"] = []
        with self.assertRaisesRegex(ValueError, "must not be empty"):
            self.load(empty)

        load_corpus = self.api("load_corpus")
        with tempfile.TemporaryDirectory() as raw:
            path = Path(raw) / "duplicate.json"
            path.write_text(
                '{"schema_version":2,"schema_version":1,"lanes":[],"fixtures":[]}',
                encoding="utf-8",
            )
            with self.assertRaisesRegex(ValueError, "duplicate JSON key"):
                load_corpus(path)

    def test_unknown_schema_fails_closed(self) -> None:
        registry = self.registry()
        registry["schema_version"] = 3

        with self.assertRaises(ValueError):
            self.load(registry)

    def test_lane_kind_and_public_operation_are_strict(self) -> None:
        for field, value in (
            ("kind", "stage"),
            ("kind", ""),
            ("public_operation", ""),
            ("public_operation", None),
        ):
            registry = self.clone(self.registry())
            registry["lanes"][0][field] = value  # type: ignore[index]
            with self.subTest(field=field, value=value), self.assertRaises(ValueError):
                self.load(registry)

    def test_process_and_engine_lifecycles_use_closed_vocabulary(self) -> None:
        cases = (
            ("process_lifecycle", "worker", "fresh-process"),
            ("engine_lifecycle", "warm", "cold-engine"),
        )
        for field, invalid, valid in cases:
            registry = self.clone(self.registry())
            registry["lanes"][0][field] = invalid  # type: ignore[index]
            with self.subTest(field=field, value=invalid), self.assertRaises(ValueError):
                self.load(registry)

            registry = self.clone(self.registry())
            registry["lanes"][0][field] = valid  # type: ignore[index]
            self.assertEqual(getattr(self.load(registry).lanes[0], field), valid)

        registry = self.clone(self.registry())
        registry["lanes"][0]["engine_lifecycle"] = "not-applicable"  # type: ignore[index]
        self.assertEqual(
            self.load(registry).lanes[0].engine_lifecycle,
            "not-applicable",
        )

    def test_logical_operation_divisor_is_a_positive_integer_per_lane(self) -> None:
        for value in (0, -1, True, 1.5, "10"):
            registry = self.clone(self.registry())
            registry["lanes"][0]["logical_operations_per_estimate"] = value  # type: ignore[index]
            with self.subTest(value=value), self.assertRaises(ValueError):
                self.load(registry)

        registry = self.clone(self.registry())
        second = registry["lanes"][1]  # type: ignore[index]
        second["logical_operations_per_estimate"] = 100
        corpus = self.load(registry)
        self.assertEqual(
            tuple(lane.logical_operations_per_estimate for lane in corpus.lanes),
            (1, 100),
        )

    def test_transport_is_one_closed_vocabulary_value(self) -> None:
        for value in ("browser-wasm", "native", "", ["native-criterion"]):
            registry = self.clone(self.registry())
            registry["lanes"][0]["transport"] = value  # type: ignore[index]
            with self.subTest(value=value), self.assertRaises(ValueError):
                self.load(registry)

        registry = self.clone(self.registry())
        registry["lanes"][0]["transport"] = "node-napi"  # type: ignore[index]
        self.assertEqual(self.load(registry).lanes[0].transport, "node-napi")

    def test_diagnostic_lane_requires_same_owner_public_parent(self) -> None:
        valid = self.clone(self.registry())
        valid["lanes"].append(  # type: ignore[union-attr]
            {
                "id": "compatibility-decode-stage",
                "kind": "diagnostic",
                "owner": "merman",
                "public_operation": "compatibility-json-parse",
                "diagnostic_stage": "compatibility-json-decode",
                "parent_public_lane": "compatibility-json-parse",
                "process_lifecycle": "reused-process",
                "engine_lifecycle": "reused-engine",
                "logical_operations_per_estimate": 1,
                "transport": "native-criterion",
                "required_features": ["svg"],
                "selector": "compatibility_json_decode/{fixture}",
                "history_aliases": [],
                "size_vector": [],
                "workload": "corpus-fixture",
                "evidence_contract": None,
                "measurement_metrics": ["latency_ns"],
                "semantic_output_dimensions": ["model_nodes"],
            }
        )
        corpus = self.load(valid)
        self.assertEqual(corpus.lanes[-1].parent_public_lane, "compatibility-json-parse")

        invalid_cases: list[dict[str, object]] = []
        wrong_owner = self.clone(valid)
        wrong_owner["lanes"][-1]["owner"] = "merman-render"  # type: ignore[index]
        invalid_cases.append(wrong_owner)
        missing_parent = self.clone(valid)
        missing_parent["lanes"][-1]["parent_public_lane"] = "missing"  # type: ignore[index]
        invalid_cases.append(missing_parent)
        missing_stage = self.clone(valid)
        missing_stage["lanes"][-1]["diagnostic_stage"] = None  # type: ignore[index]
        invalid_cases.append(missing_stage)
        for field, value in (
            ("process_lifecycle", "fresh-process"),
            ("engine_lifecycle", "cold-engine"),
            ("transport", "node-napi"),
            ("logical_operations_per_estimate", 2),
            ("required_features", []),
            ("size_vector", [1]),
        ):
            drifted = self.clone(valid)
            drifted["lanes"][-1][field] = value  # type: ignore[index]
            invalid_cases.append(drifted)

        for registry in invalid_cases:
            with self.subTest(lane=registry["lanes"][-1]), self.assertRaises(ValueError):  # type: ignore[index]
                self.load(registry)

    def test_memory_lane_requires_exact_registered_scale_vector(self) -> None:
        for vector in (
            [1, 2, 4, 10, 32],
            [1, 2, 4, 8, 32, 100],
            [1, 2, 4, 10, 32, 100, 128],
            [100, 32, 10, 4, 2, 1],
        ):
            registry = self.clone(self.registry())
            registry["lanes"][1]["size_vector"] = vector  # type: ignore[index]
            with self.subTest(vector=vector), self.assertRaises(ValueError):
                self.load(registry)

        corpus = self.load(self.registry())
        self.assertEqual(corpus.lanes[1].size_vector, MEMORY_SCALES)

    def test_render_svg_memory_lane_requires_svg_semantic_evidence(self) -> None:
        required_dimensions = (
            "input_nodes",
            "input_edges",
            "svg_sha256",
            "svg_viewbox_width",
            "svg_viewbox_height",
        )
        for missing in required_dimensions:
            registry = self.clone(self.registry())
            lane = registry["lanes"][1]  # type: ignore[index]
            lane["semantic_output_dimensions"].remove(missing)
            with self.subTest(missing=missing), self.assertRaisesRegex(
                ValueError,
                "missing semantic output evidence",
            ):
                self.load(registry)

    def test_memory_lane_accepts_owner_specific_semantic_dimensions(self) -> None:
        registry = self.clone(self.registry())
        lane = registry["lanes"][1]  # type: ignore[index]
        lane.update(
            {
                "id": "binding-request-version-only-memory",
                "owner": "merman-bindings-core",
                "public_operation": "binding-execute-operation-semantic-json",
                "required_features": ["analysis", "ascii", "svg"],
                "selector": "memory/request_version_only/{fixture}",
                "workload": "binding-version-only-operation-calls-v1",
                "evidence_contract": (
                    "docs/performance/contracts/"
                    "binding-request-version-only-memory-v2.json"
                ),
                "semantic_output_dimensions": [
                    "operation_calls",
                    "operation_id",
                    "media_type",
                    "result_data_bytes",
                    "result_metadata_bytes",
                    "result_data_sha256",
                    "result_metadata_sha256",
                ],
            }
        )

        corpus = self.load(registry)
        self.assertEqual(
            corpus.lanes[1].semantic_output_dimensions[0],
            "operation_calls",
        )

        lane["semantic_output_dimensions"] = []
        with self.assertRaisesRegex(ValueError, "must not be empty"):
            self.load(registry)

    def test_id_selector_and_history_alias_share_one_global_namespace(self) -> None:
        collision_mutators = (
            lambda lanes: lanes[1].__setitem__("id", lanes[0]["selector"]),
            lambda lanes: lanes[1].__setitem__("selector", lanes[0]["id"]),
            lambda lanes: lanes[1].__setitem__(
                "history_aliases", [lanes[0]["selector"]]
            ),
            lambda lanes: lanes[0].__setitem__(
                "history_aliases", [lanes[0]["selector"]]
            ),
        )

        for mutate in collision_mutators:
            registry = self.clone(self.registry())
            lanes = registry["lanes"]  # type: ignore[assignment]
            mutate(lanes)
            with self.assertRaises(ValueError):
                self.load(registry)

    def test_history_alias_resolves_to_the_current_lane(self) -> None:
        resolve = self.api("resolve_lane_selector")
        resolve_group = self.api("resolve_lane_group")
        corpus = self.load(self.registry())

        current = resolve(corpus, "compatibility_json_parse/{fixture}")
        historical = resolve(corpus, "parse_known_type/{fixture}")

        self.assertIs(current, historical)
        self.assertEqual(historical.id, "compatibility-json-parse")
        self.assertIs(
            resolve_group(corpus, "compatibility_json_parse"),
            resolve_group(corpus, "parse_known_type"),
        )
        with self.assertRaises(ValueError):
            resolve(corpus, "unknown/{fixture}")

    def test_native_criterion_selector_has_one_unambiguous_group(self) -> None:
        registry = self.clone(self.registry())
        registry["lanes"][0]["selector"] = "nested/parse/{fixture}"  # type: ignore[index]
        with self.assertRaisesRegex(ValueError, "must not contain"):
            self.load(registry)


if __name__ == "__main__":
    unittest.main()
