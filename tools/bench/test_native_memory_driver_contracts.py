#!/usr/bin/env python3
"""Contracts for the native-memory subprocess driver."""

from __future__ import annotations

import argparse
import hashlib
import io
import json
import subprocess
import sys
import tempfile
import unittest
from collections.abc import Mapping
from contextlib import redirect_stdout
from pathlib import Path
from types import SimpleNamespace
from unittest import mock


BENCH_DIR = Path(__file__).resolve().parent
ROOT = BENCH_DIR.parents[1]
if str(BENCH_DIR) not in sys.path:
    sys.path.insert(0, str(BENCH_DIR))

import run_native_memory
from corpus_utils import load_corpus, resolve_lane_selector


CORPUS_PATH = ROOT / "tools" / "bench" / "corpus.json"
CONTRACT_PATH = (
    ROOT
    / "docs"
    / "performance"
    / "contracts"
    / "flowchart-end-to-end-memory-v1.json"
)
BINDING_CORPUS_PATH = ROOT / "tools" / "bench" / "binding_request_corpus.json"
BINDING_CONTRACT_PATH = (
    ROOT
    / "docs"
    / "performance"
    / "contracts"
    / "binding-request-version-only-memory-v2.json"
)
EXECUTABLE_SHA256 = "a" * 64
EMPTY_SHA256 = hashlib.sha256(b"").hexdigest()
BINDING_DATA_SHA256 = "471b942eb8877b2ee7c38b86567b13f79fba101df1e5b4767b3421a200e0ad3f"
BINDING_METADATA_SHA256 = "5662eba44530c1a9bf01f352dd84496fb368c9522757c479f735526d200a9e29"
BINDING_OUTPUT_SHA256 = "5c5a3cdbe1f692c630006b4c365f348d93d8afd6ea99ecf45d8d2e10524acf53"


def response_for(request: dict[str, object]) -> dict[str, object]:
    scale = int(request["scale"])
    zero = request["mode"] == "zero"
    return {
        "schema_version": 1,
        "lane_id": request["lane_id"],
        "public_operation": "render-svg",
        "process_lifecycle": "fresh-process",
        "engine_lifecycle": "reused-engine",
        "logical_operations_per_estimate": 1,
        "mode": request["mode"],
        "scale": scale,
        "seed": request["seed"],
        "repeat": request["repeat"],
        "pid": 1234,
        "executable_sha256": EXECUTABLE_SHA256,
        "invocation_id": request["invocation_id"],
        "nonce": request["nonce"],
        "output_sha256": (
            hashlib.sha256(b"").hexdigest() if zero else f"{scale:064x}"
        ),
        "output_width": 0 if zero else scale * 10,
        "output_height": 0 if zero else scale * 5,
        "input_nodes": scale * 3,
        "input_edges": scale * 4,
        "snapshot_live_bytes": 100,
        "allocation_count": 0 if zero else scale * 10,
        "allocated_bytes": 0 if zero else scale * 1_000,
        "live_bytes_after": 100,
        "peak_live_bytes": 100 if zero else 100 + scale * 100,
        "peak_growth_bytes": 0 if zero else scale * 100,
        "counter_overflowed": False,
        "counter_underflowed": False,
    }


def binding_response_for(request: dict[str, object]) -> dict[str, object]:
    scale = int(request["scale"])
    zero = request["mode"] == "zero"
    return {
        "schema_version": 2,
        "lane_id": request["lane_id"],
        "public_operation": "binding-execute-operation-semantic-json",
        "process_lifecycle": "fresh-process",
        "engine_lifecycle": "reused-engine",
        "logical_operations_per_estimate": 1,
        "mode": request["mode"],
        "scale": scale,
        "seed": request["seed"],
        "repeat": request["repeat"],
        "pid": 1234,
        "executable_sha256": EXECUTABLE_SHA256,
        "invocation_id": request["invocation_id"],
        "nonce": request["nonce"],
        "output_sha256": EMPTY_SHA256 if zero else BINDING_OUTPUT_SHA256,
        "workload_units": scale,
        "semantic_output": {
            "kind": "binding-operation-result-v1",
            "operation_id": "semantic-json",
            "media_type": "application/json",
            "result_data_bytes": 0 if zero else 31,
            "result_metadata_bytes": 0 if zero else 126,
            "result_data_sha256": EMPTY_SHA256 if zero else BINDING_DATA_SHA256,
            "result_metadata_sha256": (
                EMPTY_SHA256 if zero else BINDING_METADATA_SHA256
            ),
            "operation_calls": 0 if zero else scale,
        },
        "snapshot_live_bytes": 100,
        "allocation_count": 0 if zero else scale * 10,
        "allocated_bytes": 0 if zero else scale * 1_000,
        "live_bytes_after": 100,
        "peak_live_bytes": 100 if zero else 100 + scale * 100,
        "peak_growth_bytes": 0 if zero else scale * 100,
        "counter_overflowed": False,
        "counter_underflowed": False,
    }


def validate_zero_work_smoke_report(report: Mapping[str, object]) -> None:
    if report.get("outcome") != "protocol_smoke_pass" or report.get("exit_code") != 0:
        raise ValueError("native-memory smoke report did not pass its protocol contract")
    schedule = report.get("schedule")
    if not isinstance(schedule, list):
        raise ValueError("native-memory smoke report has no schedule")
    responses: list[Mapping[str, object]] = []
    for entry in schedule:
        if not isinstance(entry, Mapping):
            raise ValueError("native-memory smoke schedule has no response")
        response = entry.get("response")
        if not isinstance(response, Mapping):
            raise ValueError("native-memory smoke schedule has no response")
        responses.append(response)

    if {response.get("scale") for response in responses} != {1, 100}:
        raise ValueError("native-memory smoke must cover the 1x and 100x boundary scales")

    zero_responses = [response for response in responses if response.get("mode") == "zero"]
    operation_responses = [
        response for response in responses if response.get("mode") == "operation"
    ]
    if len(zero_responses) != 2 or len(operation_responses) != 2:
        raise ValueError(
            "native-memory smoke must contain one operation/zero pair at each boundary scale"
        )
    for scale in (1, 100):
        modes = sorted(
            str(response.get("mode"))
            for response in responses
            if response.get("scale") == scale
        )
        if modes != ["operation", "zero"]:
            raise ValueError(
                "native-memory smoke must pair operation/zero at each boundary scale"
            )

    for zero in zero_responses:
        if any(
            zero.get(field) != 0
            for field in ("allocation_count", "allocated_bytes", "peak_growth_bytes")
        ):
            raise ValueError("zero-work measurement contains operation-owned allocations")
        snapshot = zero.get("snapshot_live_bytes")
        if zero.get("live_bytes_after") != snapshot or zero.get("peak_live_bytes") != snapshot:
            raise ValueError("zero-work live/peak counters differ from the setup snapshot")

    for operation in operation_responses:
        if any(
            not isinstance(operation.get(field), int) or operation[field] <= 0
            for field in ("allocation_count", "allocated_bytes", "peak_growth_bytes")
        ):
            raise ValueError("instrumented operation did not report positive allocation evidence")


class NativeMemoryDriverContractsTest(unittest.TestCase):
    @staticmethod
    def lane():
        return resolve_lane_selector(load_corpus(CORPUS_PATH), run_native_memory.DEFAULT_LANE)

    @staticmethod
    def binding_lane():
        return resolve_lane_selector(
            load_corpus(BINDING_CORPUS_PATH),
            "binding-request-version-only-memory",
        )

    @staticmethod
    def args(**overrides: object) -> argparse.Namespace:
        values: dict[str, object] = {
            "corpus": str(CORPUS_PATH),
            "lane": run_native_memory.DEFAULT_LANE,
            "contract": str(CONTRACT_PATH),
            "executable": "",
            "target_dir": "target",
            "toolchain": None,
            "repeats": 5,
            "seed": run_native_memory.DEFAULT_SEED,
            "bootstrap_resamples": 10_000,
            "timeout_seconds": 30,
            "run_id": "test-run",
            "json_out": "target/bench/native-memory-test.json",
            "allow_dirty": True,
            "smoke": False,
            "dry_run": False,
        }
        values.update(overrides)
        return argparse.Namespace(**values)

    def test_owner_contract_is_strict_and_matches_lane(self) -> None:
        contract = run_native_memory.load_owner_contract(
            CONTRACT_PATH, lane=self.lane()
        )

        self.assertEqual(contract["lane_id"], run_native_memory.DEFAULT_LANE)
        self.assertEqual(
            set(contract["metrics"]),
            {"allocation_count", "allocated_bytes", "peak_growth_bytes"},
        )
        self.assertEqual(contract["evidence_class"], "infrastructure-smoke")
        self.assertIs(contract["candidate_admission"], False)

        with tempfile.TemporaryDirectory() as raw:
            path = Path(raw) / "invalid.json"
            damaged = json.loads(CONTRACT_PATH.read_text(encoding="utf-8"))
            damaged["metrics"]["allocated_bytes"]["slope_cap"] = 0
            path.write_text(json.dumps(damaged), encoding="utf-8")
            with self.assertRaisesRegex(
                run_native_memory.DriverContractError, "positive"
            ):
                run_native_memory.load_owner_contract(path, lane=self.lane())

            candidate = json.loads(CONTRACT_PATH.read_text(encoding="utf-8"))
            candidate["evidence_class"] = "candidate-bound"
            candidate["candidate_admission"] = True
            path.write_text(json.dumps(candidate), encoding="utf-8")
            loaded = run_native_memory.load_owner_contract(path, lane=self.lane())
            self.assertIs(loaded["candidate_admission"], True)

            candidate["candidate_admission"] = False
            path.write_text(json.dumps(candidate), encoding="utf-8")
            with self.assertRaisesRegex(
                run_native_memory.DriverContractError, "disagree"
            ):
                run_native_memory.load_owner_contract(path, lane=self.lane())

    def test_binding_owner_contract_selects_its_own_probe_recipe(self) -> None:
        lane = self.binding_lane()
        contract = run_native_memory.load_owner_contract(
            BINDING_CONTRACT_PATH,
            lane=lane,
        )
        recipe = run_native_memory.memory_recipe(
            ROOT,
            target_dir=ROOT / "target",
            toolchain="1.95.0",
            corpus=BINDING_CORPUS_PATH,
            contract=contract,
        )

        self.assertEqual(contract["schema_version"], 2)
        self.assertEqual(contract["scale"]["dimension"], "operation_calls")
        self.assertEqual(recipe.package, "merman-bindings-core")
        self.assertEqual(recipe.bench, "request_overlay_memory")
        self.assertEqual(recipe.features, ("analysis", "ascii", "svg"))
        self.assertEqual(recipe.corpus, BINDING_CORPUS_PATH)

    def test_binding_probe_is_locked_to_owner_semantics_and_call_scale(self) -> None:
        lane = self.binding_lane()
        contract = run_native_memory.load_owner_contract(
            BINDING_CONTRACT_PATH,
            lane=lane,
        )
        nonces = (f"{index:032x}" for index in range(60))
        request = run_native_memory.build_schedule(
            lane_id=lane.id,
            repeats=5,
            seed=1,
            run_id="binding-probe",
            nonce_factory=lambda: next(nonces),
        )[18]["request"]
        payload = binding_response_for(request)
        completed = SimpleNamespace(
            returncode=0,
            stdout=json.dumps(payload) + "\n",
            stderr="",
        )

        with mock.patch.object(subprocess, "run", return_value=completed):
            response = run_native_memory.run_probe(
                Path("/tmp/request-overlay-memory"),
                request,
                executable_sha256=EXECUTABLE_SHA256,
                lane=lane,
                contract=contract,
                timeout_seconds=30,
            )
        self.assertEqual(response["workload_units"], request["scale"])

        payload["semantic_output"]["result_data_bytes"] = True
        completed.stdout = json.dumps(payload) + "\n"
        with mock.patch.object(subprocess, "run", return_value=completed), self.assertRaisesRegex(
            run_native_memory.DriverContractError,
            "semantic output",
        ):
            run_native_memory.run_probe(
                Path("/tmp/request-overlay-memory"),
                request,
                executable_sha256=EXECUTABLE_SHA256,
                lane=lane,
                contract=contract,
                timeout_seconds=30,
            )

    def test_binding_owner_contract_rejects_recipe_and_semantic_drift(self) -> None:
        lane = self.binding_lane()
        baseline = json.loads(BINDING_CONTRACT_PATH.read_text(encoding="utf-8"))
        mutations = (
            lambda value: value["probe"].__setitem__("protocol_schema_version", 1),
            lambda value: value["probe"].__setitem__(
                "features", ["svg", "ascii", "analysis"]
            ),
            lambda value: value["scale"].__setitem__("units_per_scale", True),
            lambda value: value["semantic_response"]["operation"].__setitem__(
                "unexpected", 1
            ),
        )

        with tempfile.TemporaryDirectory() as raw:
            path = Path(raw) / "contract.json"
            for mutate in mutations:
                damaged = json.loads(json.dumps(baseline))
                mutate(damaged)
                path.write_text(json.dumps(damaged), encoding="utf-8")
                with self.subTest(contract=damaged), self.assertRaises(
                    run_native_memory.DriverContractError
                ):
                    run_native_memory.load_owner_contract(path, lane=lane)

    def test_binding_dry_run_records_recipe_without_source_hash_approvals(self) -> None:
        report, exit_code = run_native_memory.execute(
            self.args(
                corpus=str(BINDING_CORPUS_PATH),
                lane="binding-request-version-only-memory",
                contract=str(BINDING_CONTRACT_PATH),
                dry_run=True,
            )
        )

        self.assertEqual(exit_code, 0, report.get("contract_errors"))
        self.assertEqual(report["recipe"]["package"], "merman-bindings-core")
        self.assertEqual(
            report["inputs"]["package_manifest"]["path"],
            "crates/merman-bindings-core/Cargo.toml",
        )
        self.assertNotIn("probe_inputs", report["inputs"])

    def test_schedule_is_balanced_fixed_seed_and_has_fresh_identities(self) -> None:
        nonces = (f"{index:032x}" for index in range(60))
        schedule = run_native_memory.build_schedule(
            repeats=5,
            seed=99,
            run_id="fixed",
            nonce_factory=lambda: next(nonces),
        )

        self.assertEqual(len(schedule), 60)
        self.assertEqual(
            [entry["request"]["mode"] for entry in schedule[:6]],
            ["operation", "zero", "zero", "operation", "operation", "zero"],
        )
        requests = [entry["request"] for entry in schedule]
        self.assertEqual({request["seed"] for request in requests}, {99})
        self.assertEqual(len({request["invocation_id"] for request in requests}), 60)
        self.assertEqual(len({request["nonce"] for request in requests}), 60)

    def test_run_probe_validates_protocol_and_generator_dimensions(self) -> None:
        nonces = (f"{index:032x}" for index in range(60))
        request = run_native_memory.build_schedule(
            repeats=5,
            seed=1,
            run_id="probe",
            nonce_factory=lambda: next(nonces),
        )[0]["request"]
        payload = response_for(request)
        completed = SimpleNamespace(
            returncode=0,
            stdout=json.dumps(payload) + "\n",
            stderr="",
        )

        with mock.patch.object(subprocess, "run", return_value=completed):
            response = run_native_memory.run_probe(
                Path("/tmp/native-memory"),
                request,
                executable_sha256=EXECUTABLE_SHA256,
                lane=self.lane(),
                generator={"nodes_per_scale": 3, "edges_per_scale": 4},
                timeout_seconds=30,
            )
        self.assertEqual(response["input_nodes"], 3)

        payload["input_nodes"] = 4
        completed.stdout = json.dumps(payload) + "\n"
        with mock.patch.object(subprocess, "run", return_value=completed), self.assertRaisesRegex(
            run_native_memory.DriverContractError, "dimensions"
        ):
            run_native_memory.run_probe(
                Path("/tmp/native-memory"),
                request,
                executable_sha256=EXECUTABLE_SHA256,
                lane=self.lane(),
                generator={"nodes_per_scale": 3, "edges_per_scale": 4},
                timeout_seconds=30,
            )

        payload = response_for(request)
        payload["engine_lifecycle"] = "cold-engine"
        completed.stdout = json.dumps(payload) + "\n"
        with mock.patch.object(subprocess, "run", return_value=completed), self.assertRaisesRegex(
            run_native_memory.DriverContractError, "echo drift"
        ):
            run_native_memory.run_probe(
                Path("/tmp/native-memory"),
                request,
                executable_sha256=EXECUTABLE_SHA256,
                lane=self.lane(),
                generator={"nodes_per_scale": 3, "edges_per_scale": 4},
                timeout_seconds=30,
            )

    def test_run_probe_fails_closed_on_exit_stderr_multiline_and_timeout(self) -> None:
        nonces = (f"{index:032x}" for index in range(60))
        request = run_native_memory.build_schedule(
            repeats=5,
            seed=1,
            run_id="failure",
            nonce_factory=lambda: next(nonces),
        )[0]["request"]
        payload = response_for(request)
        cases = (
            SimpleNamespace(returncode=137, stdout="", stderr=""),
            SimpleNamespace(
                returncode=0,
                stdout=json.dumps(payload) + "\n",
                stderr="warning\n",
            ),
            SimpleNamespace(
                returncode=0,
                stdout=json.dumps(payload) + "\n{}\n",
                stderr="",
            ),
        )
        for completed in cases:
            with self.subTest(completed=completed), mock.patch.object(
                subprocess, "run", return_value=completed
            ), self.assertRaises(run_native_memory.DriverContractError):
                run_native_memory.run_probe(
                    Path("/tmp/native-memory"),
                    request,
                    executable_sha256=EXECUTABLE_SHA256,
                    lane=self.lane(),
                    generator={"nodes_per_scale": 3, "edges_per_scale": 4},
                    timeout_seconds=30,
                )

        with mock.patch.object(
            subprocess,
            "run",
            side_effect=subprocess.TimeoutExpired(["probe"], 30),
        ), self.assertRaisesRegex(run_native_memory.DriverContractError, "timed out"):
            run_native_memory.run_probe(
                Path("/tmp/native-memory"),
                request,
                executable_sha256=EXECUTABLE_SHA256,
                lane=self.lane(),
                generator={"nodes_per_scale": 3, "edges_per_scale": 4},
                timeout_seconds=30,
            )

    def test_dry_run_is_side_effect_free_and_plans_sixty_processes(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            output = Path(raw) / "report.json"
            stdout = io.StringIO()
            with redirect_stdout(stdout):
                result = run_native_memory.main(
                    ["--dry-run", "--json-out", str(output)]
                )

            self.assertEqual(result, 0)
            self.assertFalse(output.exists())
            self.assertIn("--locked", stdout.getvalue())
            self.assertIn("planned fresh subprocesses: 60", stdout.getvalue())

    def test_dry_run_rejects_an_unexecutable_sampling_contract(self) -> None:
        cases = (
            ("--repeats", "4", "at least 5 repeats"),
            ("--seed", "-1", "fit the native u64"),
            ("--bootstrap-resamples", "9999", "at least 10000"),
            ("--timeout-seconds", "0", "must be positive"),
        )
        for option, value, message in cases:
            with self.subTest(option=option), redirect_stdout(io.StringIO()):
                stderr = io.StringIO()
                with mock.patch("sys.stderr", stderr):
                    result = run_native_memory.main(["--dry-run", option, value])
            self.assertEqual(result, 2)
            self.assertIn(message, stderr.getvalue())

    def test_schedule_rejects_repeated_nonce_before_launch(self) -> None:
        with self.assertRaisesRegex(
            run_native_memory.DriverContractError, "repeated identity"
        ):
            run_native_memory.build_schedule(
                repeats=5,
                seed=1,
                run_id="duplicate",
                nonce_factory=lambda: "0" * 32,
            )

    def test_execute_launches_every_schedule_entry_in_a_fresh_call(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            executable = Path(raw) / "native_memory"
            executable.write_bytes(b"probe")
            executable.chmod(0o755)
            calls: list[dict[str, object]] = []

            def fake_probe(_executable, request, **_kwargs):
                calls.append(dict(request))
                return response_for(dict(request))

            with mock.patch.object(
                run_native_memory,
                "discover_executable",
                return_value=(executable, {"command": ["cargo"]}),
            ), mock.patch.object(
                run_native_memory, "run_probe", side_effect=fake_probe
            ), mock.patch.object(
                run_native_memory,
                "analyze_samples",
                return_value=({"matrix": {"complete": True}, "metrics": {}}, ["pass"]),
            ), mock.patch.object(
                run_native_memory, "_tool_output", return_value="tool-version"
            ):
                report, exit_code = run_native_memory.execute(self.args())

        self.assertEqual(exit_code, 0)
        self.assertEqual(report["outcome"], "pass")
        self.assertEqual(len(calls), 60)
        self.assertEqual(len({call["invocation_id"] for call in calls}), 60)
        self.assertIn("commit", report["source"])
        self.assertEqual(report["inputs"]["cargo_lock"]["path"], "Cargo.lock")
        self.assertEqual(report["recipe"]["build_environment"]["CARGO_BUILD_JOBS"], "1")
        self.assertIs(report["candidate_admission"], False)

    def test_execute_retains_completed_schedule_entries_after_probe_failure(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            executable = Path(raw) / "native_memory"
            executable.write_bytes(b"probe")
            executable.chmod(0o755)
            calls = 0

            def fail_after_one(_executable, request, **_kwargs):
                nonlocal calls
                calls += 1
                if calls == 2:
                    raise run_native_memory.DriverContractError("probe failed")
                return response_for(dict(request))

            with mock.patch.object(
                run_native_memory,
                "discover_executable",
                return_value=(executable, {"command": ["cargo"]}),
            ), mock.patch.object(
                run_native_memory, "run_probe", side_effect=fail_after_one
            ), mock.patch.object(
                run_native_memory, "_tool_output", return_value="tool-version"
            ):
                report, exit_code = run_native_memory.execute(self.args())

        self.assertEqual(exit_code, 2)
        self.assertEqual(report["outcome"], "contract_failure")
        self.assertEqual(len(report["schedule"]), 60)
        self.assertIn("response", report["schedule"][0])
        self.assertNotIn("response", report["schedule"][1])
        self.assertIn("probe failed", report["contract_errors"])

    def test_execute_maps_failed_and_inconclusive_outcomes_to_process_exit(self) -> None:
        cases = (
            ("failed_bound", "failed_bound", 1),
            ("inconclusive", "inconclusive", 3),
        )
        with tempfile.TemporaryDirectory() as raw:
            executable = Path(raw) / "native_memory"
            executable.write_bytes(b"probe")
            executable.chmod(0o755)

            for metric_outcome, expected_outcome, expected_exit in cases:
                with self.subTest(metric_outcome=metric_outcome), mock.patch.object(
                    run_native_memory,
                    "discover_executable",
                    return_value=(executable, {"command": ["cargo"]}),
                ), mock.patch.object(
                    run_native_memory,
                    "run_probe",
                    side_effect=lambda _executable, request, **_kwargs: response_for(
                        dict(request)
                    ),
                ), mock.patch.object(
                    run_native_memory,
                    "analyze_samples",
                    return_value=(
                        {"matrix": {"complete": True}, "metrics": {}},
                        [metric_outcome],
                    ),
                ), mock.patch.object(
                    run_native_memory,
                    "_tool_output",
                    return_value="tool-version",
                ):
                    report, exit_code = run_native_memory.execute(self.args())

                self.assertEqual(exit_code, expected_exit)
                self.assertEqual(report["exit_code"], expected_exit)
                self.assertEqual(report["outcome"], expected_outcome)

    def test_smoke_runs_boundary_protocol_pairs_without_decision_claim(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            executable = Path(raw) / "native_memory"
            executable.write_bytes(b"probe")
            executable.chmod(0o755)
            calls: list[dict[str, object]] = []

            def fake_probe(_executable, request, **_kwargs):
                calls.append(dict(request))
                return response_for(dict(request))

            with mock.patch.object(
                run_native_memory,
                "discover_executable",
                return_value=(executable, {"command": ["cargo"]}),
            ), mock.patch.object(
                run_native_memory, "run_probe", side_effect=fake_probe
            ), mock.patch.object(
                run_native_memory, "_tool_output", return_value="tool-version"
            ):
                report, exit_code = run_native_memory.execute(
                    self.args(smoke=True)
                )

        self.assertEqual(exit_code, 0)
        self.assertEqual(report["outcome"], "protocol_smoke_pass")
        self.assertIs(report["candidate_admission"], False)
        self.assertEqual(report["method"]["evidence_class"], "protocol-smoke")
        self.assertEqual(
            [call["mode"] for call in calls],
            ["operation", "zero", "zero", "operation"],
        )
        self.assertEqual({call["scale"] for call in calls}, {1, 100})
        self.assertFalse(report["analysis"]["matrix"]["complete"])
        validate_zero_work_smoke_report(report)

        leaked_setup = json.loads(json.dumps(report))
        zero = next(
            entry["response"]
            for entry in leaked_setup["schedule"]
            if entry["response"]["mode"] == "zero"
        )
        zero["allocation_count"] = 1
        with self.assertRaisesRegex(ValueError, "operation-owned allocations"):
            validate_zero_work_smoke_report(leaked_setup)

    def test_smoke_report_cli_enforces_the_zero_work_boundary(self) -> None:
        nonces = (f"{index:032x}" for index in range(60))
        schedule = run_native_memory.boundary_smoke_schedule(
            run_native_memory.build_schedule(
                repeats=5,
                seed=1,
                run_id="smoke-cli",
                nonce_factory=lambda: next(nonces),
            )
        )
        for entry in schedule:
            entry["response"] = response_for(dict(entry["request"]))
        report = {
            "outcome": "protocol_smoke_pass",
            "exit_code": 0,
            "schedule": schedule,
        }

        with tempfile.TemporaryDirectory() as raw:
            path = Path(raw) / "smoke.json"
            path.write_text(json.dumps(report), encoding="utf-8")
            command = [
                sys.executable,
                str(Path(__file__).resolve()),
                "--verify-smoke-report",
                str(path),
            ]
            valid = subprocess.run(command, capture_output=True, text=True, check=False)
            self.assertEqual(valid.returncode, 0, valid.stderr)

            zero = next(
                entry["response"]
                for entry in report["schedule"]
                if entry["response"]["mode"] == "zero"
            )
            zero["allocated_bytes"] = 1
            path.write_text(json.dumps(report), encoding="utf-8")
            invalid = subprocess.run(command, capture_output=True, text=True, check=False)

        self.assertEqual(invalid.returncode, 2)
        self.assertIn("zero-work measurement", invalid.stderr)

    def test_main_writes_contract_failure_report_before_returning_two(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            output = Path(raw) / "failure.json"
            result = run_native_memory.main(
                [
                    "--contract",
                    str(Path(raw) / "missing.json"),
                    "--allow-dirty",
                    "--json-out",
                    str(output),
                ]
            )
            report = json.loads(output.read_text(encoding="utf-8"))

        self.assertEqual(result, 2)
        self.assertEqual(report["exit_code"], 2)
        self.assertEqual(report["outcome"], "contract_failure")
        self.assertTrue(report["contract_errors"])

    def test_decision_evidence_rejects_dirty_source_by_default(self) -> None:
        dirty = {
            "commit": "a" * 40,
            "tree": "b" * 40,
            "clean": False,
            "dirty_status_sha256": "c" * 64,
            "dirty_disposition": "unapproved",
        }
        with mock.patch.object(
            run_native_memory, "_git_provenance", return_value=dirty
        ):
            report, exit_code = run_native_memory.execute(
                self.args(allow_dirty=False)
            )

        self.assertEqual(exit_code, 2)
        self.assertIn("clean Git worktree", report["contract_errors"][0])

    def test_tool_versions_follow_the_requested_rustup_toolchain(self) -> None:
        self.assertEqual(
            run_native_memory._toolchain_command("1.95.0", "rustc", "-Vv"),
            ["rustup", "run", "1.95.0", "rustc", "-Vv"],
        )

    def test_main_classifies_report_write_failure_as_contract_failure(self) -> None:
        with mock.patch.object(
            run_native_memory,
            "execute",
            return_value=({"outcome": "pass", "exit_code": 0}, 0),
        ), mock.patch.object(
            run_native_memory,
            "_atomic_json",
            side_effect=OSError("disk unavailable"),
        ), mock.patch("sys.stderr", io.StringIO()) as stderr:
            result = run_native_memory.main([])

        self.assertEqual(result, 2)
        self.assertIn("failed to write", stderr.getvalue())


if __name__ == "__main__":
    if len(sys.argv) == 3 and sys.argv[1] == "--verify-smoke-report":
        try:
            smoke_report = json.loads(Path(sys.argv[2]).read_text(encoding="utf-8"))
            if not isinstance(smoke_report, dict):
                raise ValueError("native-memory smoke report must be a JSON object")
            validate_zero_work_smoke_report(smoke_report)
        except (OSError, ValueError) as error:
            print(f"native-memory smoke boundary failed: {error}", file=sys.stderr)
            raise SystemExit(2) from error
        print("Verified native-memory zero-work measurement boundary")
        raise SystemExit(0)
    unittest.main()
