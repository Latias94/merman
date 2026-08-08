#!/usr/bin/env python3
"""Contracts for the State Rough retained-lifecycle probe driver."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest import mock


BENCH_DIR = Path(__file__).resolve().parent
ROOT = BENCH_DIR.parents[1]
if str(BENCH_DIR) not in sys.path:
    sys.path.insert(0, str(BENCH_DIR))

import run_state_rough_lifecycle as lifecycle
from corpus_utils import load_corpus, resolve_lane_selector


CORPUS_PATH = ROOT / "tools" / "bench" / "corpus.json"
CONTRACT_PATH = (
    ROOT
    / "docs"
    / "performance"
    / "contracts"
    / "state-rough-lifecycle-v2.json"
)
SHA_A = "sha256:" + "a" * 64
BASELINE_COMMIT = "a" * 40
BASELINE_TREE = "b" * 40
EMPTY_STATUS_SHA256 = lifecycle._sha256_bytes(b"")


def footprint(entries: int = 0, owned_bytes: int = 0) -> dict[str, int]:
    return {"entries": entries, "owned_bytes": owned_bytes}


def retained(entries: int = 0, owned_bytes: int = 0) -> dict[str, object]:
    return {
        "global": footprint(entries, owned_bytes),
        "tls": footprint(entries, owned_bytes),
    }


def kind_counters(
    *,
    draw_requests: int | None = None,
    operation_lookups: int = 2,
    operation_hits: int = 1,
    operation_misses: int = 1,
    operation_builds: int = 1,
    tls_hits: int = 0,
    global_hits: int = 0,
    bypass_builds: int = 0,
) -> dict[str, int]:
    return {
        "draw_requests": (
            operation_lookups + bypass_builds
            if draw_requests is None
            else draw_requests
        ),
        "operation_lookups": operation_lookups,
        "operation_hits": operation_hits,
        "operation_misses": operation_misses,
        "operation_builds": operation_builds,
        "tls_hits": tls_hits,
        "global_hits": global_hits,
        "bypass_builds": bypass_builds,
    }


def counters(**overrides: int | None) -> dict[str, object]:
    return {
        "circle": kind_counters(**overrides),
        "paths": kind_counters(**overrides),
    }


def release_proof(
    request_counters: dict[str, object],
    peak: dict[str, int],
    *,
    mode: str,
    cache_allowed: bool,
) -> dict[str, object]:
    if cache_allowed:
        geometry_witnesses = sum(
            int(request_counters[kind]["operation_misses"])
            for kind in lifecycle._COUNTER_KINDS
        )
        allocation_witnesses = int(
            request_counters["circle"]["operation_misses"]
        ) + 2 * int(request_counters["paths"]["operation_misses"])
        witnessed_owned_bytes = peak["owned_bytes"]
    else:
        geometry_witnesses = 0
        allocation_witnesses = 0
        witnessed_owned_bytes = 0
    return {
        "cache_drop_observed": True,
        "geometry_witnesses": geometry_witnesses,
        "allocation_witnesses": allocation_witnesses,
        "witnessed_owned_bytes": witnessed_owned_bytes,
        "live_allocation_witnesses": (
            allocation_witnesses if mode == "baseline" else 0
        ),
        "live_owned_bytes": witnessed_owned_bytes if mode == "baseline" else 0,
    }


def expected_release_rollup(
    mode: str, *, long_lived_only: bool
) -> dict[str, object]:
    if long_lived_only:
        operation_count = 2_048
        total_geometry_witnesses = 4_096
        total_allocation_witnesses = 6_144
        total_witnessed_owned_bytes = 65_536
    else:
        operation_count = 2_057
        total_geometry_witnesses = 4_110
        total_allocation_witnesses = 6_165
        total_witnessed_owned_bytes = 65_760
    return {
        "operation_count": operation_count,
        "total_geometry_witnesses": total_geometry_witnesses,
        "total_allocation_witnesses": total_allocation_witnesses,
        "total_witnessed_owned_bytes": total_witnessed_owned_bytes,
        "max_live_allocation_witnesses_after_operation": (
            3 if mode == "baseline" else 0
        ),
        "max_live_owned_bytes_after_operation": 32 if mode == "baseline" else 0,
        "all_cache_drops_observed": True,
    }


def independent_release_rollup(
    proofs: list[dict[str, object]],
) -> dict[str, object]:
    return {
        "operation_count": len(proofs),
        "total_geometry_witnesses": sum(
            int(proof["geometry_witnesses"]) for proof in proofs
        ),
        "total_allocation_witnesses": sum(
            int(proof["allocation_witnesses"]) for proof in proofs
        ),
        "total_witnessed_owned_bytes": sum(
            int(proof["witnessed_owned_bytes"]) for proof in proofs
        ),
        "max_live_allocation_witnesses_after_operation": max(
            (int(proof["live_allocation_witnesses"]) for proof in proofs),
            default=0,
        ),
        "max_live_owned_bytes_after_operation": max(
            (int(proof["live_owned_bytes"]) for proof in proofs), default=0
        ),
        "all_cache_drops_observed": bool(proofs)
        and all(proof["cache_drop_observed"] is True for proof in proofs),
    }


def operation(
    ordinal: int,
    *,
    mode: str,
    configured_seed: float,
    fallback: bool = False,
) -> dict[str, object]:
    state = retained(1, 10) if mode == "baseline" else retained()
    if fallback:
        request_counters = counters(
            operation_lookups=0,
            operation_hits=0,
            operation_misses=0,
            operation_builds=0,
            bypass_builds=2,
        )
        peak = footprint()
        seed_resolution = "configured_fallback_capable"
    elif mode == "baseline" and ordinal == 2:
        request_counters = counters(operation_builds=0, tls_hits=1)
        peak = footprint(2, 32)
        seed_resolution = "configured_deterministic"
    elif mode == "baseline" and ordinal == 3:
        request_counters = counters(operation_builds=0, global_hits=1)
        peak = footprint(2, 32)
        seed_resolution = "configured_deterministic"
    else:
        request_counters = counters()
        peak = footprint(2, 32)
        seed_resolution = (
            "operation_resolved"
            if configured_seed == 0.0
            else "configured_deterministic"
        )
    return {
        "configured_seed": configured_seed,
        "resolved_seed": 91.0 if configured_seed == 0.0 else configured_seed,
        "seed_resolution": seed_resolution,
        "cache_allowed": not fallback,
        "outcome": "success",
        "counters": request_counters,
        "operation_peak": peak,
        "post_operation_retained": state,
        "release_proof": release_proof(
            request_counters,
            peak,
            mode=mode,
            cache_allowed=not fallback,
        ),
    }


def request(
    ordinal: int,
    *,
    mode: str,
    configured_seed: float,
    identity: str,
    render_thread: str = "primary",
    geometry_label_bytes: int = 4,
    fallback: bool = False,
) -> dict[str, object]:
    return {
        "ordinal": ordinal,
        "case": f"case-{ordinal}",
        "render_thread": render_thread,
        "geometry_label_bytes": geometry_label_bytes,
        "ordinary_nodes": 6,
        "svg": {"bytes": 100 + ordinal, "elements": 10 + ordinal, "identity": identity},
        "operation": operation(
            ordinal,
            mode=mode,
            configured_seed=configured_seed,
            fallback=fallback,
        ),
    }


def long_release_record(request_count: int, *, mode: str) -> dict[str, object]:
    request_counters = counters()
    peak = footprint(2, 32)
    return {
        "request_count": request_count,
        "cache_allowed": True,
        "counters": request_counters,
        "operation_peak": peak,
        "release_proof": release_proof(
            request_counters, peak, mode=mode, cache_allowed=True
        ),
    }


def valid_receipt(mode: str) -> dict[str, object]:
    shared_identity = "sha256:" + "1" * 64
    requests = [
        request(1, mode=mode, configured_seed=7.0, identity=shared_identity),
        request(2, mode=mode, configured_seed=7.0, identity=shared_identity),
        request(
            3,
            mode=mode,
            configured_seed=7.0,
            identity=shared_identity,
            render_thread="fresh",
        ),
        request(
            4,
            mode=mode,
            configured_seed=11.0,
            identity="sha256:" + "2" * 64,
            geometry_label_bytes=16,
        ),
        request(
            5,
            mode=mode,
            configured_seed=12.0,
            identity="sha256:" + "3" * 64,
            geometry_label_bytes=32,
        ),
        request(
            6,
            mode=mode,
            configured_seed=13.0,
            identity="sha256:" + "4" * 64,
            geometry_label_bytes=64,
        ),
        request(
            7,
            mode=mode,
            configured_seed=0.0,
            identity="sha256:" + "5" * 64,
            geometry_label_bytes=16,
        ),
        request(
            8,
            mode=mode,
            configured_seed=4_294_967_296.0,
            identity="sha256:" + "6" * 64,
            fallback=True,
        ),
        request(
            9,
            mode=mode,
            configured_seed=-1.0,
            identity="sha256:" + "7" * 64,
            fallback=True,
        ),
    ]
    cases = (
        "seed-7-cold",
        "seed-7-tls-warm",
        "seed-7-global-warm",
        "seed-11-width-16",
        "seed-12-width-32",
        "seed-13-width-64",
        "configured-zero-operation-seed",
        "fallback-u32-wrap",
        "fallback-second-stroke-wrap",
    )
    for entry, case in zip(requests, cases, strict=True):
        entry["case"] = case
    for entry in requests[1:3]:
        entry["svg"] = dict(requests[0]["svg"])
    request_counters = [entry["operation"]["counters"] for entry in requests]
    final_retained = retained(1, 10) if mode == "baseline" else retained()
    long_release_proofs = [
        long_release_record(request_count, mode=mode)
        for request_count in range(1, 2_049)
    ]
    long_counters = lifecycle._counter_sum(
        [proof["counters"] for proof in long_release_proofs]
    )
    checkpoints = [
        {
            "request_count": entry["ordinal"],
            "geometry_label_bytes": entry["geometry_label_bytes"],
            "configured_seed": entry["operation"]["configured_seed"],
            "retained": entry["operation"]["post_operation_retained"],
        }
        for entry in requests
    ]
    long_checkpoint_counts = [1, 16, 64, 256, 1024, 2048]
    long_label_cycle = [1, 2, 4, 8, 16, 32, 64, 128]
    long_checkpoints = [
        {
            "request_count": count,
            "geometry_label_bytes": long_label_cycle[(count - 1) % len(long_label_cycle)],
            "configured_seed": float(10_000 + count),
            "retained": final_retained,
        }
        for count in long_checkpoint_counts
    ]
    rollup_counters = {
        kind: {
            field: sum(int(value[kind][field]) for value in request_counters)
            + int(long_counters[kind][field])
            for field in lifecycle._KIND_COUNTER_FIELDS
        }
        for kind in lifecycle._COUNTER_KINDS
    }
    return {
        "schema": "merman.state_rough_lifecycle.v2",
        "contracts": {
            "owned_bytes": "sum_of_cached_string_capacities",
            "release_proof": (
                "weak_string_allocation_witnesses_sampled_after_operation_cache_drop"
            ),
            "render_cancellation": "not_applicable_no_render_control_or_checkpoint",
            "early_termination_proof": "result_error_after_nonempty_operation_cache",
            "configured_seed_zero": (
                "configured_hand_drawn_seed_zero_resolves_to_operation_seed_before_cache_bypass"
            ),
            "fallback_capable_configured_seeds": [4_294_967_296.0, -1.0],
        },
        "engine_lifecycle": {
            "engine_instances": 1,
            "engine_reused_across_requests": True,
            "request_count": 2057,
            "detailed_request_count": 9,
            "long_lived_request_count": 2048,
            "render_threads": 2,
        },
        "schedule": {
            "same_seed_request_ordinals": [1, 2, 3],
            "distinct_seed_request_ordinals": [4, 5, 6],
            "fallback_bypass_request_ordinals": [8, 9],
            "geometry_label_byte_checkpoints": [4, 16, 32, 64],
            "request_count_checkpoints": long_checkpoint_counts,
        },
        "requests": requests,
        "checkpoints": checkpoints,
        "long_lived": {
            "request_count": 2048,
            "request_count_checkpoints": long_checkpoint_counts,
            "checkpoints": long_checkpoints,
            "svg": {"bytes": 500_000, "elements": 50_000, "identity": SHA_A},
            "counters": long_counters,
            "max_operation_peak": footprint(2, 32),
            "max_post_operation_retained": final_retained,
            "final_retained": final_retained,
            "release_proofs": long_release_proofs,
            "release_rollup": expected_release_rollup(
                mode, long_lived_only=True
            ),
        },
        "rollup": {
            "svg": {
                "bytes": 500_000
                + sum(int(entry["svg"]["bytes"]) for entry in requests),
                "elements": 50_000
                + sum(int(entry["svg"]["elements"]) for entry in requests),
                "identity": "sha256:" + "b" * 64,
            },
            "counters": rollup_counters,
            "max_operation_peak": footprint(2, 32),
            "initial_retained": retained(),
            "final_retained": final_retained,
            "retained_growth": final_retained,
            "operation_cache_reuse_observed": True,
            "legacy_cross_operation_cache_observed": mode == "baseline",
            "configured_zero_operation_resolution_observed": True,
            "fallback_bypass_observed": True,
            "release": expected_release_rollup(mode, long_lived_only=False),
        },
    }


def valid_controls(mode: str) -> dict[str, object]:
    shared_svg = {
        "bytes": 1_024,
        "elements": 42,
        "identity": "sha256:" + "c" * 64,
    }

    def control_operation(outcome: str) -> dict[str, object]:
        result = operation(
            1,
            mode=mode,
            configured_seed=23.0,
        )
        result["outcome"] = outcome
        return result

    def failure_control(sentinel: str, outcome: str) -> dict[str, object]:
        return {
            "sentinel": sentinel,
            "engine_lifecycle": {
                "engine_instances": 1,
                "engine_reused_across_requests": True,
                "request_count": 3,
            },
            "reference_svg": dict(shared_svg),
            "failure_svg": dict(shared_svg),
            "recovery_svg": dict(shared_svg),
            "reference_operation": control_operation("success"),
            "operation": control_operation(outcome),
            "recovery_operation": control_operation("success"),
        }

    return {
        "schema": "merman.state_rough_lifecycle_controls.v2",
        "error": failure_control(
            "State Rough lifecycle control error after root render", "error"
        ),
        "unwind": failure_control(
            "State Rough lifecycle control unwind after root render", "unwind"
        ),
        "concurrency": {
            "engine_lifecycle": {
                "engine_instances": 1,
                "engine_reused_across_requests": True,
                "request_count": 4,
            },
            "workers": 2,
            "overlap_observed": True,
            "serial_svg": dict(shared_svg),
            "worker_svgs": [dict(shared_svg), dict(shared_svg)],
            "recovery_svg": dict(shared_svg),
            "serial_operation": control_operation("success"),
            "operations": [
                control_operation("success"),
                control_operation("success"),
            ],
            "recovery_operation": control_operation("success"),
        },
    }


def success_baseline_report(
    receipt: dict[str, object],
    controls_receipt: dict[str, object] | None = None,
) -> dict[str, object]:
    lane = resolve_lane_selector(load_corpus(CORPUS_PATH), lifecycle.DEFAULT_LANE)
    descriptor = lifecycle._describe_file(CONTRACT_PATH, root=ROOT)
    harness = lifecycle._harness_report(ROOT)
    executable_path = str((ROOT / "target" / "debug" / "test").resolve())
    file_descriptor = {"path": executable_path, "bytes": 1, "sha256": "d" * 64}
    controls_receipt = controls_receipt or valid_controls("baseline")
    return {
        "schema": lifecycle.DRIVER_SCHEMA,
        "generated_at_utc": "2026-08-02T00:00:00Z",
        "mode": "baseline",
        "lane": lifecycle._lane_report(lane),
        "contract": descriptor,
        "source": {
            "commit": BASELINE_COMMIT,
            "tree": BASELINE_TREE,
            "first_parent_commit": "9" * 40,
            "first_parent_tree": "8" * 40,
            "parent_count": 1,
            "clean": True,
            "dirty_status_sha256": EMPTY_STATUS_SHA256,
        },
        "host": {
            "platform": lifecycle.platform.platform(),
            "machine": lifecycle.platform.machine(),
            "python": lifecycle.platform.python_version(),
        },
        "harness": harness,
        "build": {
            "command": [
                "cargo",
                "test",
                "--locked",
                "-p",
                "merman-render",
                "--lib",
                "--no-run",
                "--message-format=json-render-diagnostics",
            ],
            "environment": {"CARGO_BUILD_JOBS": "1", "CARGO_INCREMENTAL": "0"},
            "cargo_stdout_sha256": "f" * 64,
            "cargo_stderr_sha256": "0" * 64,
            "executable": file_descriptor,
        },
        "probe": {
            "command": [
                executable_path,
                (
                    "svg::parity::state::rough_lifecycle_probe::"
                    "state_rough_lifecycle_probe_receipt"
                ),
                "--exact",
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ],
            "timeout_seconds": 30,
            "returncode": 0,
            "stdout_sha256": "1" * 64,
            "stderr_sha256": "2" * 64,
        },
        "controls": {
            "probe": {
                "command": [
                    executable_path,
                    (
                        "svg::parity::state::rough_lifecycle_probe::"
                        "state_rough_lifecycle_release_controls"
                    ),
                    "--exact",
                    "--ignored",
                    "--nocapture",
                    "--test-threads=1",
                ],
                "timeout_seconds": 30,
                "returncode": 0,
                "stdout_sha256": "3" * 64,
                "stderr_sha256": "4" * 64,
            },
            "receipt": controls_receipt,
        },
        "baseline_comparison": None,
        "receipt": receipt,
        "checks": ["strict_receipt_schema"],
        "outcome": "pass",
        "exit_code": 0,
    }


def refresh_rollup_counters(receipt: dict[str, object]) -> None:
    receipt["rollup"]["counters"] = {
        kind: {
            field: sum(
                int(entry["operation"]["counters"][kind][field])
                for entry in receipt["requests"]
            )
            + int(receipt["long_lived"]["counters"][kind][field])
            for field in lifecycle._KIND_COUNTER_FIELDS
        }
        for kind in lifecycle._COUNTER_KINDS
    }


def refresh_operation_release_proof(
    value: dict[str, object], *, mode: str
) -> None:
    value["release_proof"] = release_proof(
        value["counters"],
        value["operation_peak"],
        mode=mode,
        cache_allowed=bool(value["cache_allowed"]),
    )


def refresh_release_rollups(receipt: dict[str, object]) -> None:
    long_proofs = receipt["long_lived"]["release_proofs"]
    receipt["long_lived"]["release_rollup"] = independent_release_rollup(
        [proof["release_proof"] for proof in long_proofs]
    )
    receipt["rollup"]["release"] = independent_release_rollup(
        [
            *(
                entry["operation"]["release_proof"]
                for entry in receipt["requests"]
            ),
            *(proof["release_proof"] for proof in long_proofs),
        ]
    )


class StateRoughLifecycleContractsTest(unittest.TestCase):
    @staticmethod
    def lane():
        return resolve_lane_selector(load_corpus(CORPUS_PATH), lifecycle.DEFAULT_LANE)

    @classmethod
    def contract(cls) -> dict[str, object]:
        return lifecycle.load_owner_contract(
            CONTRACT_PATH, lane=cls.lane(), root=ROOT
        )

    def test_corpus_registers_one_reused_process_library_probe(self) -> None:
        lane = self.lane()

        self.assertEqual(lane.owner, "merman-render")
        self.assertEqual(lane.public_operation, "render-svg")
        self.assertEqual(lane.transport, "native-library-test-probe")
        self.assertEqual(lane.process_lifecycle, "reused-process")
        self.assertEqual(lane.engine_lifecycle, "reused-engine")
        self.assertEqual(lane.logical_operations_per_estimate, 2057)
        self.assertEqual(lane.size_vector, (1, 16, 64, 256, 1024, 2048))

        corpus = json.loads(CORPUS_PATH.read_text(encoding="utf-8"))
        registered = next(
            item
            for item in corpus["lanes"]
            if item["id"] == lifecycle.DEFAULT_LANE
        )
        with tempfile.TemporaryDirectory() as raw:
            path = Path(raw) / "corpus.json"
            for field, value in (
                ("process_lifecycle", "fresh-process"),
                ("engine_lifecycle", "cold-engine"),
                ("evidence_contract", None),
                ("size_vector", []),
            ):
                damaged = json.loads(json.dumps(corpus))
                target = next(
                    item
                    for item in damaged["lanes"]
                    if item["id"] == registered["id"]
                )
                target[field] = value
                path.write_text(json.dumps(damaged), encoding="utf-8")
                with self.subTest(field=field), self.assertRaises(ValueError):
                    load_corpus(path)

    def test_owner_contract_is_strict_and_matches_corpus(self) -> None:
        contract = self.contract()

        self.assertEqual(contract["lane_id"], lifecycle.DEFAULT_LANE)
        self.assertEqual(contract["schema_version"], 2)
        self.assertEqual(
            contract["receipt"]["release_proof"],
            "weak_string_allocation_witnesses_sampled_after_operation_cache_drop",
        )
        self.assertEqual(
            contract["receipt"]["render_cancellation"],
            "not_applicable_no_render_control_or_checkpoint",
        )
        self.assertEqual(
            contract["receipt"]["early_termination_proof"],
            "result_error_after_nonempty_operation_cache",
        )
        self.assertEqual(
            contract["probe"]["test_name"],
            "svg::parity::state::rough_lifecycle_probe::state_rough_lifecycle_probe_receipt",
        )
        self.assertEqual(
            contract["controls"]["test_name"],
            (
                "svg::parity::state::rough_lifecycle_probe::"
                "state_rough_lifecycle_release_controls"
            ),
        )
        self.assertEqual(
            contract["controls"]["scenarios"],
            ["error", "unwind", "concurrency"],
        )

        damaged = json.loads(CONTRACT_PATH.read_text(encoding="utf-8"))
        damaged["unknown"] = True
        with tempfile.TemporaryDirectory() as raw:
            path = Path(raw) / "unknown.json"
            path.write_text(json.dumps(damaged), encoding="utf-8")
            with self.assertRaisesRegex(lifecycle.LifecycleContractError, "unknown"):
                lifecycle.load_owner_contract(path, lane=self.lane(), root=ROOT)

            path.write_text(
                CONTRACT_PATH.read_text(encoding="utf-8").replace(
                    '"schema_version": 2',
                    '"schema_version": 2, "schema_version": 2',
                    1,
                ),
                encoding="utf-8",
            )
            with self.assertRaisesRegex(lifecycle.LifecycleContractError, "duplicate"):
                lifecycle.load_owner_contract(path, lane=self.lane(), root=ROOT)

            damaged = json.loads(CONTRACT_PATH.read_text(encoding="utf-8"))
            damaged["schedule"]["detailed_specs"][0]["case"] = "drifted"
            path.write_text(json.dumps(damaged), encoding="utf-8")
            with self.assertRaisesRegex(
                lifecycle.LifecycleContractError, "detailed request specs"
            ):
                lifecycle.load_owner_contract(path, lane=self.lane(), root=ROOT)

            for field in ("render_cancellation", "early_termination_proof"):
                damaged = json.loads(CONTRACT_PATH.read_text(encoding="utf-8"))
                damaged["receipt"][field] = "drifted"
                path.write_text(json.dumps(damaged), encoding="utf-8")
                with self.subTest(field=field), self.assertRaisesRegex(
                    lifecycle.LifecycleContractError, field
                ):
                    lifecycle.load_owner_contract(path, lane=self.lane(), root=ROOT)

            provenance_mutations = (
                ("relation", "candidate_ancestor", "relation"),
                ("candidate_parent_count", True, "integer"),
                ("candidate_parent_count", 2, "exactly one"),
                ("baseline_source_clean", False, "clean baseline"),
            )
            for field, value, error_pattern in provenance_mutations:
                damaged = json.loads(CONTRACT_PATH.read_text(encoding="utf-8"))
                damaged["baseline_provenance"][field] = value
                path.write_text(json.dumps(damaged), encoding="utf-8")
                with self.subTest(field=field, value=value), self.assertRaisesRegex(
                    lifecycle.LifecycleContractError, error_pattern
                ):
                    lifecycle.load_owner_contract(path, lane=self.lane(), root=ROOT)

            controls_mutations = (
                ("schema", "merman.state_rough_lifecycle_controls.v1", "schema"),
                ("marker", "MERMAN_STATE_ROUGH_LIFECYCLE_CONTROLS_V1=", "marker"),
                ("test_name", "drifted", "test name"),
                ("scenarios", ["error", "concurrency"], "scenarios"),
                ("configured_seed", 24.0, "configured seed"),
                ("workers", 3, "workers"),
            )
            for field, value, error_pattern in controls_mutations:
                damaged = json.loads(CONTRACT_PATH.read_text(encoding="utf-8"))
                damaged["controls"][field] = value
                path.write_text(json.dumps(damaged), encoding="utf-8")
                with self.subTest(field=field), self.assertRaisesRegex(
                    lifecycle.LifecycleContractError, error_pattern
                ):
                    lifecycle.load_owner_contract(path, lane=self.lane(), root=ROOT)

    def test_baseline_and_candidate_receipts_pass_their_distinct_gates(self) -> None:
        contract = self.contract()
        baseline = lifecycle._validate_receipt(
            valid_receipt("baseline"), contract=contract
        )
        candidate = lifecycle._validate_receipt(
            valid_receipt("candidate"), contract=contract
        )

        self.assertIn(
            "legacy_tls_and_global_hits",
            lifecycle.validate_mode(baseline, mode="baseline"),
        )
        self.assertIn(
            "zero_post_operation_retained_state",
            lifecycle.validate_mode(candidate, mode="candidate"),
        )
        self.assertEqual(
            baseline["long_lived"]["release_rollup"],
            {
                "operation_count": 2_048,
                "total_geometry_witnesses": 4_096,
                "total_allocation_witnesses": 6_144,
                "total_witnessed_owned_bytes": 65_536,
                "max_live_allocation_witnesses_after_operation": 3,
                "max_live_owned_bytes_after_operation": 32,
                "all_cache_drops_observed": True,
            },
        )
        self.assertEqual(
            candidate["rollup"]["release"],
            {
                "operation_count": 2_057,
                "total_geometry_witnesses": 4_110,
                "total_allocation_witnesses": 6_165,
                "total_witnessed_owned_bytes": 65_760,
                "max_live_allocation_witnesses_after_operation": 0,
                "max_live_owned_bytes_after_operation": 0,
                "all_cache_drops_observed": True,
            },
        )

    def test_detailed_seed_resolution_and_cache_policy_are_ordinal_bound(self) -> None:
        contract = self.contract()

        mutations = []

        wrong_deterministic_seed = valid_receipt("candidate")
        wrong_deterministic_seed["requests"][0]["operation"]["resolved_seed"] = 8.0
        mutations.append((wrong_deterministic_seed, "deterministic"))

        wrong_deterministic_resolution = valid_receipt("candidate")
        wrong_deterministic_resolution["requests"][3]["operation"][
            "seed_resolution"
        ] = "operation_resolved"
        mutations.append((wrong_deterministic_resolution, "deterministic"))

        deterministic_bypass = valid_receipt("candidate")
        deterministic_operation = deterministic_bypass["requests"][4]["operation"]
        deterministic_operation["cache_allowed"] = False
        deterministic_operation["counters"] = counters(
            operation_lookups=0,
            operation_hits=0,
            operation_misses=0,
            operation_builds=0,
            bypass_builds=2,
        )
        deterministic_operation["operation_peak"] = footprint()
        refresh_operation_release_proof(
            deterministic_operation, mode="candidate"
        )
        mutations.append((deterministic_bypass, "deterministic"))

        unresolved_zero = valid_receipt("candidate")
        unresolved_zero["requests"][6]["operation"]["resolved_seed"] = 0.0
        mutations.append((unresolved_zero, "configured-zero"))

        fallback_seed_drift = valid_receipt("candidate")
        fallback_seed_drift["requests"][7]["operation"]["resolved_seed"] = 1.0
        mutations.append((fallback_seed_drift, "fallback"))

        fallback_cache_entry = valid_receipt("candidate")
        fallback_operation = fallback_cache_entry["requests"][8]["operation"]
        fallback_operation["cache_allowed"] = True
        fallback_operation["counters"] = counters()
        fallback_operation["operation_peak"] = footprint(2, 32)
        refresh_operation_release_proof(fallback_operation, mode="candidate")
        mutations.append((fallback_cache_entry, "fallback"))

        for receipt, pattern in mutations:
            with self.subTest(pattern=pattern), self.assertRaisesRegex(
                lifecycle.LifecycleContractError, pattern
            ):
                lifecycle._validate_receipt(receipt, contract=contract)

    def test_cross_version_schedule_projection_includes_seed_cache_semantics(self) -> None:
        baseline = valid_receipt("baseline")
        candidate = valid_receipt("candidate")
        baseline_projection = lifecycle._schedule_projection(baseline)
        self.assertEqual(
            baseline_projection, lifecycle._schedule_projection(candidate)
        )

        mutations = (
            ("resolved_seed", 12.0),
            ("seed_resolution", "operation_resolved"),
            ("cache_allowed", False),
            ("outcome", "error"),
        )
        for field, value in mutations:
            drifted = valid_receipt("candidate")
            drifted["requests"][3]["operation"][field] = value
            with self.subTest(field=field):
                self.assertNotEqual(
                    baseline_projection, lifecycle._schedule_projection(drifted)
                )

    def test_release_controls_are_strict_and_mode_aware(self) -> None:
        contract = self.contract()
        baseline = lifecycle._validate_controls_receipt(
            valid_controls("baseline"), contract=contract
        )
        candidate = lifecycle._validate_controls_receipt(
            valid_controls("candidate"), contract=contract
        )

        self.assertIn(
            "release_control_semantics",
            lifecycle.validate_controls_mode(baseline, mode="baseline"),
        )
        self.assertIn(
            "release_controls_zero_post_operation_retention",
            lifecycle.validate_controls_mode(candidate, mode="candidate"),
        )

        unknown = valid_controls("candidate")
        unknown["concurrency"]["unexpected"] = True
        with self.assertRaisesRegex(lifecycle.LifecycleContractError, "unknown"):
            lifecycle._validate_controls_receipt(unknown, contract=contract)

        wrong_sentinel = valid_controls("candidate")
        wrong_sentinel["error"]["sentinel"] = "different"
        with self.assertRaisesRegex(lifecycle.LifecycleContractError, "sentinel"):
            lifecycle._validate_controls_receipt(wrong_sentinel, contract=contract)

        no_peak = valid_controls("candidate")
        no_peak["unwind"]["operation"]["operation_peak"] = footprint()
        with self.assertRaisesRegex(
            lifecycle.LifecycleContractError, "operation cache|release tracker"
        ):
            lifecycle._validate_controls_receipt(no_peak, contract=contract)

        no_reuse = valid_controls("candidate")
        no_reuse["error"]["operation"]["counters"] = counters(
            operation_hits=0,
            operation_misses=2,
            operation_builds=2,
        )
        no_reuse["error"]["operation"]["operation_peak"] = footprint(4, 64)
        refresh_operation_release_proof(
            no_reuse["error"]["operation"], mode="candidate"
        )
        with self.assertRaisesRegex(lifecycle.LifecycleContractError, "reuse"):
            lifecycle._validate_controls_receipt(no_reuse, contract=contract)

        output_drift = valid_controls("candidate")
        output_drift["concurrency"]["worker_svgs"][1]["identity"] = (
            "sha256:" + "d" * 64
        )
        with self.assertRaisesRegex(lifecycle.LifecycleContractError, "serial SVG"):
            lifecycle._validate_controls_receipt(output_drift, contract=contract)

        retained_candidate = valid_controls("candidate")
        retained_candidate["concurrency"]["operations"][0][
            "post_operation_retained"
        ] = retained(1, 1)
        parsed = lifecycle._validate_controls_receipt(
            retained_candidate, contract=contract
        )
        with self.assertRaisesRegex(lifecycle.LifecycleContractError, "retained"):
            lifecycle.validate_controls_mode(parsed, mode="candidate")

    def test_receipt_rejects_unknown_nested_fields_and_nonfinite_numbers(self) -> None:
        contract = self.contract()
        unknown = valid_receipt("candidate")
        unknown["long_lived"]["final_retained"]["global"]["unexpected"] = 1
        with self.assertRaisesRegex(lifecycle.LifecycleContractError, "unknown"):
            lifecycle._validate_receipt(unknown, contract=contract)

        payload = json.dumps(valid_receipt("candidate")).replace(
            '"configured_seed": 7.0', '"configured_seed": 1e999', 1
        )
        with self.assertRaisesRegex(lifecycle.LifecycleContractError, "non-finite"):
            lifecycle.strict_json_text(payload, source="test")

    def test_receipt_rejects_missing_fields_wrong_types_and_invalid_hashes(self) -> None:
        contract = self.contract()
        missing = valid_receipt("candidate")
        del missing["rollup"]["retained_growth"]
        with self.assertRaisesRegex(lifecycle.LifecycleContractError, "missing"):
            lifecycle._validate_receipt(missing, contract=contract)

        wrong_type = valid_receipt("candidate")
        wrong_type["engine_lifecycle"]["engine_instances"] = True
        with self.assertRaisesRegex(lifecycle.LifecycleContractError, "integer"):
            lifecycle._validate_receipt(wrong_type, contract=contract)

        invalid_hash = valid_receipt("candidate")
        invalid_hash["requests"][0]["svg"]["identity"] = "sha256:" + "A" * 64
        with self.assertRaisesRegex(lifecycle.LifecycleContractError, "lowercase"):
            lifecycle._validate_receipt(invalid_hash, contract=contract)

        underflow = valid_receipt("baseline")
        underflow["rollup"]["initial_retained"] = retained(2, 20)
        with self.assertRaisesRegex(lifecycle.LifecycleContractError, "decreased"):
            lifecycle._validate_receipt(underflow, contract=contract)

    def test_release_proof_schema_and_nonvacuous_witnesses_fail_closed(self) -> None:
        contract = self.contract()

        unknown = valid_receipt("candidate")
        unknown["requests"][0]["operation"]["release_proof"]["unexpected"] = 1
        with self.assertRaisesRegex(lifecycle.LifecycleContractError, "unknown"):
            lifecycle._validate_receipt(unknown, contract=contract)

        missing = valid_receipt("candidate")
        del missing["requests"][0]["operation"]["release_proof"][
            "cache_drop_observed"
        ]
        with self.assertRaisesRegex(lifecycle.LifecycleContractError, "missing"):
            lifecycle._validate_receipt(missing, contract=contract)

        vacuous = valid_receipt("candidate")
        target = vacuous["requests"][0]["operation"]
        target["counters"] = counters(
            operation_hits=2,
            operation_misses=0,
            operation_builds=0,
        )
        target["operation_peak"] = footprint()
        refresh_operation_release_proof(target, mode="candidate")
        refresh_rollup_counters(vacuous)
        refresh_release_rollups(vacuous)
        with self.assertRaisesRegex(lifecycle.LifecycleContractError, "vacuous"):
            lifecycle._validate_receipt(vacuous, contract=contract)

    def test_release_mode_rejects_candidate_leaks_and_baseline_underreporting(self) -> None:
        contract = self.contract()

        candidate = valid_receipt("candidate")
        candidate_proof = candidate["long_lived"]["release_proofs"][127][
            "release_proof"
        ]
        candidate_proof["live_allocation_witnesses"] = 1
        candidate_proof["live_owned_bytes"] = 1
        refresh_release_rollups(candidate)
        parsed_candidate = lifecycle._validate_receipt(candidate, contract=contract)
        with self.assertRaisesRegex(
            lifecycle.LifecycleContractError, "live witnessed allocations"
        ):
            lifecycle.validate_mode(parsed_candidate, mode="candidate")

        baseline = valid_receipt("baseline")
        baseline_proof = baseline["requests"][0]["operation"]["release_proof"]
        baseline_proof["live_allocation_witnesses"] = 2
        baseline_proof["live_owned_bytes"] = 16
        parsed_baseline = lifecycle._validate_receipt(baseline, contract=contract)
        with self.assertRaisesRegex(
            lifecycle.LifecycleContractError, "live witnesses differ"
        ):
            lifecycle.validate_mode(parsed_baseline, mode="baseline")

    def test_long_lived_release_proofs_require_all_ordinals_and_exact_rollups(self) -> None:
        contract = self.contract()

        short = valid_receipt("candidate")
        short["long_lived"]["release_proofs"].pop()
        with self.assertRaisesRegex(lifecycle.LifecycleContractError, "cardinality"):
            lifecycle._validate_receipt(short, contract=contract)

        ordinal_drift = valid_receipt("candidate")
        ordinal_drift["long_lived"]["release_proofs"][1023][
            "request_count"
        ] = 1023
        with self.assertRaisesRegex(lifecycle.LifecycleContractError, "ordinals"):
            lifecycle._validate_receipt(ordinal_drift, contract=contract)

        long_rollup_lie = valid_receipt("candidate")
        long_rollup_lie["long_lived"]["release_rollup"][
            "total_allocation_witnesses"
        ] += 1
        with self.assertRaisesRegex(lifecycle.LifecycleContractError, "release rollup"):
            lifecycle._validate_receipt(long_rollup_lie, contract=contract)

        main_rollup_lie = valid_receipt("candidate")
        main_rollup_lie["rollup"]["release"][
            "total_witnessed_owned_bytes"
        ] += 1
        with self.assertRaisesRegex(lifecycle.LifecycleContractError, "release rollup"):
            lifecycle._validate_receipt(main_rollup_lie, contract=contract)

    def test_control_recovery_fields_identity_and_release_gates_fail_closed(self) -> None:
        contract = self.contract()

        missing = valid_controls("candidate")
        del missing["error"]["reference_svg"]
        with self.assertRaisesRegex(lifecycle.LifecycleContractError, "missing"):
            lifecycle._validate_controls_receipt(missing, contract=contract)

        identity_drift = valid_controls("candidate")
        identity_drift["unwind"]["failure_svg"]["identity"] = (
            "sha256:" + "d" * 64
        )
        with self.assertRaisesRegex(lifecycle.LifecycleContractError, "SVG identity"):
            lifecycle._validate_controls_receipt(identity_drift, contract=contract)

        engine_drift = valid_controls("candidate")
        engine_drift["concurrency"]["engine_lifecycle"]["request_count"] = 3
        with self.assertRaisesRegex(lifecycle.LifecycleContractError, "request_count"):
            lifecycle._validate_controls_receipt(engine_drift, contract=contract)

        for operation_index in range(10):
            leaked = valid_controls("candidate")
            parsed = lifecycle._validate_controls_receipt(leaked, contract=contract)
            operations = lifecycle._control_operations(parsed)
            self.assertEqual(len(operations), 10)
            leaked_proof = operations[operation_index]["release_proof"]
            leaked_proof["live_allocation_witnesses"] = 1
            leaked_proof["live_owned_bytes"] = 1
            with self.subTest(operation_index=operation_index), self.assertRaisesRegex(
                lifecycle.LifecycleContractError, "live witnessed allocations"
            ):
                lifecycle.validate_controls_mode(parsed, mode="candidate")

    def test_counter_identities_fail_closed(self) -> None:
        receipt = valid_receipt("candidate")
        receipt["requests"][0]["operation"]["counters"]["circle"][
            "operation_lookups"
        ] = 99

        with self.assertRaisesRegex(lifecycle.LifecycleContractError, "identity"):
            lifecycle._validate_receipt(receipt, contract=self.contract())

    def test_baseline_requires_tls_global_hits_and_retained_state(self) -> None:
        receipt = valid_receipt("baseline")
        receipt["requests"][1]["operation"]["counters"] = counters()
        receipt["requests"][2]["operation"]["counters"] = counters()
        refresh_rollup_counters(receipt)
        receipt["rollup"]["legacy_cross_operation_cache_observed"] = False
        parsed = lifecycle._validate_receipt(receipt, contract=self.contract())

        with self.assertRaisesRegex(lifecycle.LifecycleContractError, "legacy"):
            lifecycle.validate_mode(parsed, mode="baseline")

    def test_candidate_rejects_any_cross_operation_hit_or_retained_checkpoint(self) -> None:
        hit = valid_receipt("candidate")
        hit["requests"][0]["operation"]["counters"] = counters(
            operation_builds=0, tls_hits=1
        )
        refresh_rollup_counters(hit)
        parsed_hit = lifecycle._validate_receipt(hit, contract=self.contract())
        with self.assertRaisesRegex(
            lifecycle.LifecycleContractError,
            "candidate operation-owned|cross-operation",
        ):
            lifecycle.validate_mode(parsed_hit, mode="candidate")

        retained_candidate = valid_receipt("candidate")
        retained_candidate["long_lived"]["checkpoints"][2]["retained"] = retained(1, 1)
        retained_candidate["long_lived"]["max_post_operation_retained"] = retained(1, 1)
        parsed_retained = lifecycle._validate_receipt(
            retained_candidate, contract=self.contract()
        )
        with self.assertRaisesRegex(lifecycle.LifecycleContractError, "retained"):
            lifecycle.validate_mode(parsed_retained, mode="candidate")

        hidden_retention = valid_receipt("candidate")
        hidden_retention["long_lived"]["max_post_operation_retained"] = retained(1, 1)
        parsed_hidden = lifecycle._validate_receipt(
            hidden_retention, contract=self.contract()
        )
        with self.assertRaisesRegex(lifecycle.LifecycleContractError, "retained"):
            lifecycle.validate_mode(parsed_hidden, mode="candidate")

    def test_marker_is_unique_and_payload_is_strict_json(self) -> None:
        contract = self.contract()
        marker = contract["receipt"]["marker"]
        payload = json.dumps(valid_receipt("candidate"), separators=(",", ":"))

        parsed = lifecycle.parse_probe_output(
            f"running 1 test\n{marker}{payload}\ntest result: ok\n",
            "",
            contract=contract,
        )
        self.assertEqual(parsed["schema"], "merman.state_rough_lifecycle.v2")

        with self.assertRaisesRegex(lifecycle.LifecycleContractError, "exactly one"):
            lifecycle.parse_probe_output(
                f"{marker}{payload}\n{marker}{payload}\n", "", contract=contract
            )
        with self.assertRaisesRegex(lifecycle.LifecycleContractError, "duplicate"):
            duplicate_payload = payload.replace(
                '{"schema":', '{"schema":"duplicate","schema":', 1
            )
            lifecycle.parse_probe_output(
                f"{marker}{duplicate_payload}\n",
                "",
                contract=contract,
            )

        controls_marker = contract["controls"]["marker"]
        controls_payload = json.dumps(
            valid_controls("candidate"), separators=(",", ":")
        )
        parsed_controls = lifecycle.parse_controls_output(
            f"running 1 test\n{controls_marker}{controls_payload}\ntest result: ok\n",
            "",
            contract=contract,
        )
        self.assertEqual(
            parsed_controls["schema"],
            "merman.state_rough_lifecycle_controls.v2",
        )
        with self.assertRaisesRegex(lifecycle.LifecycleContractError, "exactly one"):
            lifecycle.parse_controls_output(
                (
                    f"{controls_marker}{controls_payload}\n"
                    f"{controls_marker}{controls_payload}\n"
                ),
                "",
                contract=contract,
            )

    def test_candidate_comparison_requires_schedule_and_all_svg_outputs(self) -> None:
        contract = self.contract()
        baseline = valid_receipt("baseline")
        candidate = lifecycle._validate_receipt(
            valid_receipt("candidate"), contract=contract
        )
        lane = self.lane()
        expected_contract = lifecycle._describe_file(CONTRACT_PATH, root=ROOT)
        expected_harness = lifecycle._harness_report(ROOT)
        expected_host = success_baseline_report(baseline)["host"]
        with tempfile.TemporaryDirectory() as raw:
            path = Path(raw) / "baseline.json"
            path.write_text(
                json.dumps(success_baseline_report(baseline)), encoding="utf-8"
            )
            result = lifecycle.compare_with_baseline(
                candidate,
                lifecycle._validate_controls_receipt(
                    valid_controls("candidate"), contract=contract
                ),
                path,
                contract=contract,
                lane=lane,
                expected_contract=expected_contract,
                expected_harness=expected_harness,
                expected_host=expected_host,
                expected_baseline_commit=BASELINE_COMMIT,
                expected_baseline_tree=BASELINE_TREE,
            )
            self.assertTrue(result["schedule_equal"])
            self.assertTrue(result["svg_outputs_equal"])
            self.assertTrue(result["release_control_semantics_equal"])
            self.assertEqual(
                result["revision_relation"], "candidate_head_first_parent"
            )
            self.assertEqual(result["expected_baseline_commit"], BASELINE_COMMIT)
            self.assertEqual(result["actual_baseline_commit"], BASELINE_COMMIT)
            self.assertEqual(result["expected_baseline_tree"], BASELINE_TREE)
            self.assertEqual(result["actual_baseline_tree"], BASELINE_TREE)
            self.assertTrue(result["revision_equal"])

            drifted = valid_receipt("baseline")
            drifted["long_lived"]["svg"]["bytes"] += 1
            drifted["rollup"]["svg"]["bytes"] += 1
            path.write_text(
                json.dumps(success_baseline_report(drifted)), encoding="utf-8"
            )
            with self.assertRaisesRegex(lifecycle.LifecycleContractError, "SVG"):
                lifecycle.compare_with_baseline(
                    candidate,
                    lifecycle._validate_controls_receipt(
                        valid_controls("candidate"), contract=contract
                    ),
                    path,
                    contract=contract,
                    lane=lane,
                    expected_contract=expected_contract,
                    expected_harness=expected_harness,
                    expected_host=expected_host,
                    expected_baseline_commit=BASELINE_COMMIT,
                    expected_baseline_tree=BASELINE_TREE,
                )

            drifted_controls = valid_controls("baseline")
            different_svg = {
                "bytes": 1_024,
                "elements": 42,
                "identity": "sha256:" + "e" * 64,
            }
            drifted_controls["concurrency"]["serial_svg"] = dict(different_svg)
            drifted_controls["concurrency"]["worker_svgs"] = [
                dict(different_svg),
                dict(different_svg),
            ]
            drifted_controls["concurrency"]["recovery_svg"] = dict(different_svg)
            path.write_text(
                json.dumps(
                    success_baseline_report(baseline, drifted_controls)
                ),
                encoding="utf-8",
            )
            with self.assertRaisesRegex(
                lifecycle.LifecycleContractError, "release-control semantics"
            ):
                lifecycle.compare_with_baseline(
                    candidate,
                    lifecycle._validate_controls_receipt(
                        valid_controls("candidate"), contract=contract
                    ),
                    path,
                    contract=contract,
                    lane=lane,
                    expected_contract=expected_contract,
                    expected_harness=expected_harness,
                    expected_host=expected_host,
                    expected_baseline_commit=BASELINE_COMMIT,
                    expected_baseline_tree=BASELINE_TREE,
                )

    def test_baseline_comparison_rejects_provenance_drift(self) -> None:
        contract = self.contract()
        lane = self.lane()
        candidate = lifecycle._validate_receipt(
            valid_receipt("candidate"), contract=contract
        )
        candidate_controls = lifecycle._validate_controls_receipt(
            valid_controls("candidate"), contract=contract
        )
        expected_contract = lifecycle._describe_file(CONTRACT_PATH, root=ROOT)
        expected_harness = lifecycle._harness_report(ROOT)
        expected_host = success_baseline_report(valid_receipt("baseline"))["host"]
        mutations = (
            lambda report: report["source"].__setitem__("clean", False),
            lambda report: report["source"].__setitem__(
                "dirty_status_sha256", "e" * 64
            ),
            lambda report: report["contract"].__setitem__("sha256", "3" * 64),
            lambda report: report["harness"]["driver"].__setitem__(
                "sha256", "4" * 64
            ),
            lambda report: report["host"].__setitem__("machine", "different"),
            lambda report: report["controls"]["probe"]["command"].__setitem__(
                0, "target/debug/different-test"
            ),
            lambda report: report["build"]["executable"].__setitem__(
                "path", "target/debug/test"
            ),
        )
        with tempfile.TemporaryDirectory() as raw:
            path = Path(raw) / "baseline.json"
            for mutate in mutations:
                report = success_baseline_report(valid_receipt("baseline"))
                mutate(report)
                path.write_text(json.dumps(report), encoding="utf-8")
                with self.subTest(report=report), self.assertRaises(
                    lifecycle.LifecycleContractError
                ):
                    lifecycle.compare_with_baseline(
                        candidate,
                        candidate_controls,
                        path,
                        contract=contract,
                        lane=lane,
                        expected_contract=expected_contract,
                        expected_harness=expected_harness,
                        expected_host=expected_host,
                        expected_baseline_commit=BASELINE_COMMIT,
                        expected_baseline_tree=BASELINE_TREE,
                    )

            for field, expected, pattern in (
                ("commit", "7" * 40, "first parent"),
                ("tree", "6" * 40, "first-parent tree"),
            ):
                report = success_baseline_report(valid_receipt("baseline"))
                report["source"][field] = expected
                path.write_text(json.dumps(report), encoding="utf-8")
                with self.subTest(field=field), self.assertRaisesRegex(
                    lifecycle.LifecycleContractError, pattern
                ):
                    lifecycle.compare_with_baseline(
                        candidate,
                        candidate_controls,
                        path,
                        contract=contract,
                        lane=lane,
                        expected_contract=expected_contract,
                        expected_harness=expected_harness,
                        expected_host=expected_host,
                        expected_baseline_commit=BASELINE_COMMIT,
                        expected_baseline_tree=BASELINE_TREE,
                    )

            drifted = valid_receipt("baseline")
            drifted["requests"][0]["case"] = "changed-schedule-case"
            path.write_text(
                json.dumps(success_baseline_report(drifted)), encoding="utf-8"
            )
            with self.assertRaisesRegex(
                lifecycle.LifecycleContractError, "schedule|owner spec"
            ):
                lifecycle.compare_with_baseline(
                    candidate,
                    candidate_controls,
                    path,
                    contract=contract,
                    lane=lane,
                    expected_contract=expected_contract,
                    expected_harness=expected_harness,
                    expected_host=expected_host,
                    expected_baseline_commit=BASELINE_COMMIT,
                    expected_baseline_tree=BASELINE_TREE,
                )

    def test_build_discovers_exactly_one_lib_test_executable(self) -> None:
        contract = self.contract()
        with tempfile.TemporaryDirectory() as raw:
            fake_root = Path(raw)
            executable = fake_root / "target" / "merman-render-test"
            executable.parent.mkdir()
            executable.write_bytes(b"test executable")
            cargo_message = {
                "reason": "compiler-artifact",
                "target": {"name": "merman_render", "kind": ["lib"]},
                "profile": {"test": True},
                "executable": str(executable),
            }
            completed = SimpleNamespace(
                returncode=0,
                stdout=json.dumps(cargo_message) + "\n",
                stderr="",
            )
            with mock.patch.object(subprocess, "run", return_value=completed) as run:
                discovered, report = lifecycle.build_test_executable(
                    fake_root,
                    contract=contract,
                    target_dir=fake_root / "target",
                    toolchain=None,
                    timeout_seconds=30,
                )

            self.assertEqual(discovered, executable.resolve())
            self.assertEqual(report["executable"]["sha256"], lifecycle._sha256_path(executable))
            self.assertEqual(report["executable"]["path"], str(executable.resolve()))
            command = run.call_args.args[0]
            self.assertEqual(command[:6], ["cargo", "test", "--locked", "-p", "merman-render", "--lib"])
            self.assertEqual(run.call_args.kwargs["env"]["CARGO_BUILD_JOBS"], "1")

    def test_baseline_executable_provenance_is_cross_worktree_safe(self) -> None:
        contract = self.contract()
        lane = self.lane()
        candidate = lifecycle._validate_receipt(
            valid_receipt("candidate"), contract=contract
        )
        candidate_controls = lifecycle._validate_controls_receipt(
            valid_controls("candidate"), contract=contract
        )
        expected_contract = lifecycle._describe_file(CONTRACT_PATH, root=ROOT)
        expected_harness = lifecycle._harness_report(ROOT)
        expected_host = success_baseline_report(valid_receipt("baseline"))["host"]
        with (
            tempfile.TemporaryDirectory() as baseline_raw,
            tempfile.TemporaryDirectory() as candidate_raw,
            tempfile.TemporaryDirectory() as report_raw,
        ):
            report = success_baseline_report(valid_receipt("baseline"))
            baseline_executable = str(
                Path(baseline_raw) / "target" / "debug" / "test"
            )
            report["build"]["executable"]["path"] = baseline_executable
            report["probe"]["command"][0] = baseline_executable
            report["controls"]["probe"]["command"][0] = baseline_executable
            path = Path(report_raw) / "baseline.json"
            path.write_text(json.dumps(report), encoding="utf-8")
            with mock.patch.object(
                lifecycle,
                "repo_root",
                return_value=Path(candidate_raw),
            ):
                result = lifecycle.compare_with_baseline(
                    candidate,
                    candidate_controls,
                    path,
                    contract=contract,
                    lane=lane,
                    expected_contract=expected_contract,
                    expected_harness=expected_harness,
                    expected_host=expected_host,
                    expected_baseline_commit=BASELINE_COMMIT,
                    expected_baseline_tree=BASELINE_TREE,
                )

        self.assertTrue(result["revision_equal"])
        self.assertTrue(result["release_control_semantics_equal"])

    def test_probe_runs_only_the_exact_ignored_test(self) -> None:
        contract = self.contract()
        marker = contract["receipt"]["marker"]
        payload = json.dumps(valid_receipt("candidate"), separators=(",", ":"))
        completed = SimpleNamespace(
            returncode=0,
            stdout=f"{marker}{payload}\n",
            stderr="",
        )
        with mock.patch.object(subprocess, "run", return_value=completed) as run:
            receipt, report = lifecycle.run_probe(
                Path("/tmp/merman-render-test"),
                contract=contract,
                timeout_seconds=30,
            )

        self.assertEqual(receipt["engine_lifecycle"]["request_count"], 2057)
        command = run.call_args.args[0]
        self.assertEqual(command[1], contract["probe"]["test_name"])
        self.assertEqual(
            command[2:], ["--exact", "--ignored", "--nocapture", "--test-threads=1"]
        )
        self.assertEqual(report["returncode"], 0)

    def test_release_controls_run_before_decision_probe_on_the_same_executable(self) -> None:
        contract = self.contract()
        marker = contract["controls"]["marker"]
        payload = json.dumps(valid_controls("candidate"), separators=(",", ":"))
        completed = SimpleNamespace(
            returncode=0,
            stdout=f"{marker}{payload}\n",
            stderr="",
        )
        executable = Path("/tmp/merman-render-test")
        with mock.patch.object(subprocess, "run", return_value=completed) as run:
            receipt, report = lifecycle.run_controls(
                executable,
                contract=contract,
                timeout_seconds=30,
            )

        self.assertTrue(receipt["concurrency"]["overlap_observed"])
        command = run.call_args.args[0]
        self.assertEqual(command[0], str(executable))
        self.assertEqual(command[1], contract["controls"]["test_name"])
        self.assertEqual(
            command[2:], ["--exact", "--ignored", "--nocapture", "--test-threads=1"]
        )
        self.assertEqual(report["returncode"], 0)

    def test_execute_runs_controls_before_decision_receipt(self) -> None:
        args = argparse.Namespace(
            corpus=str(CORPUS_PATH),
            lane=lifecycle.DEFAULT_LANE,
            contract=str(CONTRACT_PATH),
            target_dir="target",
            toolchain=None,
            baseline_json=None,
            mode="baseline",
            timeout_seconds=30,
            allow_dirty=False,
        )
        executable = Path("/tmp/merman-render-test")
        source = {
            "commit": "a" * 40,
            "tree": "b" * 40,
            "first_parent_commit": "9" * 40,
            "first_parent_tree": "8" * 40,
            "parent_count": 1,
            "clean": True,
            "dirty_status_sha256": EMPTY_STATUS_SHA256,
        }
        build_report = {
            "command": ["cargo", "test"],
            "environment": {"CARGO_BUILD_JOBS": "1", "CARGO_INCREMENTAL": "0"},
            "cargo_stdout_sha256": "1" * 64,
            "cargo_stderr_sha256": "2" * 64,
            "executable": {
                "path": str(executable),
                "bytes": 1,
                "sha256": "3" * 64,
            },
        }
        controls_report = {
            "command": [str(executable), "controls"],
            "timeout_seconds": 30,
            "returncode": 0,
            "stdout_sha256": "4" * 64,
            "stderr_sha256": "5" * 64,
        }
        probe_report = {
            "command": [str(executable), "probe"],
            "timeout_seconds": 30,
            "returncode": 0,
            "stdout_sha256": "6" * 64,
            "stderr_sha256": "7" * 64,
        }
        order: list[str] = []

        def run_controls(*args, **kwargs):
            order.append("controls")
            self.assertEqual(args[0], executable)
            return valid_controls("baseline"), controls_report

        def run_probe(*args, **kwargs):
            order.append("probe")
            self.assertEqual(args[0], executable)
            return valid_receipt("baseline"), probe_report

        with mock.patch.object(
            lifecycle, "_git_provenance", side_effect=[source, source]
        ), mock.patch.object(
            lifecycle,
            "build_test_executable",
            return_value=(executable, build_report),
        ), mock.patch.object(
            lifecycle, "run_controls", side_effect=run_controls
        ), mock.patch.object(
            lifecycle, "run_probe", side_effect=run_probe
        ):
            report = lifecycle.execute(args)

        self.assertEqual(order, ["controls", "probe"])
        self.assertEqual(report["controls"]["probe"], controls_report)
        self.assertEqual(
            report["controls"]["receipt"]["schema"],
            valid_controls("baseline")["schema"],
        )

    def test_candidate_baseline_option_is_mode_scoped(self) -> None:
        args = argparse.Namespace(
            corpus=str(CORPUS_PATH),
            lane=lifecycle.DEFAULT_LANE,
            contract=str(CONTRACT_PATH),
            target_dir="target",
            toolchain=None,
            baseline_json="baseline.json",
            mode="baseline",
            timeout_seconds=30,
            allow_dirty=True,
        )
        with mock.patch.object(lifecycle, "build_test_executable") as build, mock.patch.object(
            lifecycle, "run_probe"
        ) as probe, mock.patch.object(lifecycle, "_git_provenance") as provenance:
            build.return_value = (Path("/tmp/test"), {})
            probe.return_value = (
                lifecycle._validate_receipt(
                    valid_receipt("baseline"), contract=self.contract()
                ),
                {},
            )
            provenance.return_value = {
                "commit": "a" * 40,
                "tree": "b" * 40,
                "first_parent_commit": "9" * 40,
                "first_parent_tree": "8" * 40,
                "parent_count": 1,
                "clean": True,
                "dirty_status_sha256": "c" * 64,
            }
            with self.assertRaisesRegex(lifecycle.LifecycleContractError, "candidate mode"):
                lifecycle.execute(args)

        args.mode = "candidate"
        args.baseline_json = None
        with self.assertRaisesRegex(lifecycle.LifecycleContractError, "requires"):
            lifecycle.execute(args)

        with self.assertRaisesRegex(lifecycle.LifecycleContractError, "unsupported"):
            lifecycle.validate_mode(
                lifecycle._validate_receipt(
                    valid_receipt("candidate"), contract=self.contract()
                ),
                mode="unknown",
            )

    def test_candidate_requires_a_single_parent_before_running_probe(self) -> None:
        args = argparse.Namespace(
            corpus=str(CORPUS_PATH),
            lane=lifecycle.DEFAULT_LANE,
            contract=str(CONTRACT_PATH),
            target_dir="target",
            toolchain=None,
            baseline_json="baseline.json",
            mode="candidate",
            timeout_seconds=30,
            allow_dirty=True,
        )
        source = {
            "commit": "a" * 40,
            "tree": "b" * 40,
            "first_parent_commit": "9" * 40,
            "first_parent_tree": "8" * 40,
            "parent_count": 2,
            "clean": True,
            "dirty_status_sha256": "c" * 64,
        }
        with mock.patch.object(
            lifecycle, "_git_provenance", return_value=source
        ), mock.patch.object(lifecycle, "build_test_executable") as build:
            with self.assertRaisesRegex(
                lifecycle.LifecycleContractError, "exactly one parent"
            ):
                lifecycle.execute(args)
        build.assert_not_called()

    def test_dirty_candidate_is_rejected_even_with_allow_dirty(self) -> None:
        args = argparse.Namespace(
            corpus=str(CORPUS_PATH),
            lane=lifecycle.DEFAULT_LANE,
            contract=str(CONTRACT_PATH),
            target_dir="target",
            toolchain=None,
            baseline_json="baseline.json",
            mode="candidate",
            timeout_seconds=30,
            allow_dirty=True,
        )
        source = {
            "commit": "a" * 40,
            "tree": "b" * 40,
            "first_parent_commit": "9" * 40,
            "first_parent_tree": "8" * 40,
            "parent_count": 1,
            "clean": False,
            "dirty_status_sha256": "c" * 64,
        }
        with mock.patch.object(
            lifecycle, "_git_provenance", return_value=source
        ), mock.patch.object(lifecycle, "build_test_executable") as build:
            with self.assertRaisesRegex(
                lifecycle.LifecycleContractError, "dirty-source exploratory"
            ):
                lifecycle.execute(args)
        build.assert_not_called()


if __name__ == "__main__":
    unittest.main()
