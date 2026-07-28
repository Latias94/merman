#!/usr/bin/env python3
"""Contracts for freezing and verifying performance baseline manifests."""

from __future__ import annotations

import copy
import hashlib
import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock


BENCH_DIR = Path(__file__).resolve().parent
if str(BENCH_DIR) not in sys.path:
    sys.path.insert(0, str(BENCH_DIR))

import freeze_perf_baseline as baseline
import run_native_memory


COMMIT = "1" * 40
TREE = "2" * 40
MEMORY_SCALES = (1, 2, 4, 10, 32, 100)
EXECUTABLE_BYTES = b"native-memory-executable"


class FakeGitProbe:
    def __init__(self, snapshot: baseline.GitSnapshot) -> None:
        self.current = snapshot

    def snapshot(self) -> baseline.GitSnapshot:
        return self.current

    def is_tracked(self, _path: str, _commit: str) -> bool:
        return True


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def file_record(path: Path) -> dict[str, object]:
    value = path.read_bytes()
    return {
        "path": str(path),
        "bytes": len(value),
        "sha256": sha256_bytes(value),
    }


def lane(
    lane_id: str,
    *,
    selector: str,
    process_lifecycle: str,
    engine_lifecycle: str,
    transport: str,
    size_vector: list[int],
    workload: str,
    evidence_contract: str | None,
    measurement_metrics: list[str],
    semantic_output_dimensions: list[str],
) -> dict[str, object]:
    return {
        "id": lane_id,
        "kind": "public",
        "owner": "merman",
        "public_operation": "render-svg",
        "diagnostic_stage": None,
        "parent_public_lane": None,
        "process_lifecycle": process_lifecycle,
        "engine_lifecycle": engine_lifecycle,
        "logical_operations_per_estimate": 1,
        "transport": transport,
        "required_features": ["svg"],
        "selector": selector,
        "history_aliases": [],
        "size_vector": size_vector,
        "workload": workload,
        "evidence_contract": evidence_contract,
        "measurement_metrics": measurement_metrics,
        "semantic_output_dimensions": semantic_output_dimensions,
    }


class PerfBaselineManifestTest(unittest.TestCase):
    def setUp(self) -> None:
        self._temporary = tempfile.TemporaryDirectory()
        self.root = Path(self._temporary.name)
        (self.root / "tools" / "bench").mkdir(parents=True)
        (self.root / "fixtures").mkdir()
        (self.root / "contracts").mkdir()
        (self.root / "target").mkdir()
        (self.root / "crates" / "merman").mkdir(parents=True)

        (self.root / "bench.py").write_text("print('bench')\n", encoding="utf-8")
        (self.root / "Cargo.toml").write_text("[workspace]\n", encoding="utf-8")
        (self.root / "crates" / "merman" / "Cargo.toml").write_text(
            '[package]\nname = "merman"\nversion = "0.0.0"\n',
            encoding="utf-8",
        )
        (self.root / "Cargo.lock").write_text("version = 4\n", encoding="utf-8")
        self.fixture = self.root / "fixtures" / "flowchart.mmd"
        self.fixture.write_text("flowchart LR\n  A --> B\n", encoding="utf-8")

        self.contract_path = self.root / "contracts" / "memory.json"
        self.owner_contract = {
            "schema_version": 1,
            "lane_id": "flowchart-end-to-end-memory",
            "workload": "flowchart-modular-generator-v1",
            "evidence_class": "infrastructure-smoke",
            "candidate_admission": False,
            "generator": {
                "id": "flowchart-modular-generator-v1",
                "nodes_per_scale": 3,
                "edges_per_scale": 4,
            },
            "metrics": {
                metric: {"slope_cap": 2.0, "max_scale_cap": 1_000_000_000}
                for metric in (
                    "allocation_count",
                    "allocated_bytes",
                    "peak_growth_bytes",
                )
            },
        }
        self._write_json(self.contract_path, self.owner_contract)

        self.corpus_path = self.root / "tools" / "bench" / "corpus.json"
        self.corpus = {
            "schema_version": 2,
            "default_group": "end_to_end",
            "suites": {"canary": "test", "full": "test"},
            "lanes": [
                lane(
                    "render-svg",
                    selector="end_to_end/{fixture}",
                    process_lifecycle="reused-process",
                    engine_lifecycle="reused-engine",
                    transport="native-criterion",
                    size_vector=[],
                    workload="corpus-fixture",
                    evidence_contract=None,
                    measurement_metrics=["latency_ns"],
                    semantic_output_dimensions=["svg_bytes"],
                ),
                lane(
                    "flowchart-end-to-end-memory",
                    selector="memory/end_to_end/{fixture}",
                    process_lifecycle="fresh-process",
                    engine_lifecycle="reused-engine",
                    transport="native-system-allocator-subprocess",
                    size_vector=list(MEMORY_SCALES),
                    workload="flowchart-modular-generator-v1",
                    evidence_contract="contracts/memory.json",
                    measurement_metrics=[
                        "allocation_count",
                        "allocated_bytes",
                        "peak_growth_bytes",
                    ],
                    semantic_output_dimensions=[
                        "input_nodes",
                        "input_edges",
                        "svg_sha256",
                        "svg_viewbox_width",
                        "svg_viewbox_height",
                    ],
                ),
            ],
            "fixtures": [
                {
                    "name": "flowchart",
                    "family": "flowchart",
                    "size": "small",
                    "category": "canary",
                    "source": "fixtures/flowchart.mmd",
                    "suites": ["canary"],
                    "features": ["basic_nodes"],
                    "quality": ["svg_sanity"],
                }
            ],
        }
        self._write_json(self.corpus_path, self.corpus)

        self.executable = self.root / "target" / "native_memory"
        self.executable.write_bytes(EXECUTABLE_BYTES)
        self.executable.chmod(0o755)
        self.report_path = self.root / "target" / "native-memory.json"
        self.host = {
            "rustc": "rustc 1.90.0\nbinary: rustc",
            "cargo": "cargo 1.90.0",
            "os": "TestOS-1",
            "cpu": "Test CPU",
            "architecture": "test64",
        }
        self._write_native_memory_report()

        self.clean = baseline.GitSnapshot(commit=COMMIT, tree=TREE, status=())
        self.git = FakeGitProbe(self.clean)

    def tearDown(self) -> None:
        self._temporary.cleanup()

    @staticmethod
    def _write_json(path: Path, value: object) -> None:
        path.write_text(
            json.dumps(value, indent=2, sort_keys=True, allow_nan=False) + "\n",
            encoding="utf-8",
        )

    def _response(
        self,
        *,
        scale: int,
        repeat: int,
        mode: str,
        pair_index: int,
        executable_sha256: str,
    ) -> dict[str, object]:
        zero = mode == "zero"
        invocation_id = f"test:{scale}:{repeat}:{mode}"
        nonce = f"{pair_index * 2 + (1 if zero else 0) + 1:032x}"
        growth = 10 if zero else scale * 100
        return {
            "schema_version": 1,
            "lane_id": "flowchart-end-to-end-memory",
            "public_operation": "render-svg",
            "process_lifecycle": "fresh-process",
            "engine_lifecycle": "reused-engine",
            "logical_operations_per_estimate": 1,
            "mode": mode,
            "scale": scale,
            "seed": 101,
            "repeat": repeat,
            "pid": 1000 + pair_index * 2 + (1 if zero else 0),
            "executable_sha256": executable_sha256,
            "invocation_id": invocation_id,
            "nonce": nonce,
            "output_sha256": (
                sha256_bytes(b"") if zero else sha256_bytes(f"svg:{scale}".encode())
            ),
            "output_width": 0 if zero else scale * 10,
            "output_height": 0 if zero else scale * 5,
            "input_nodes": scale * 3,
            "input_edges": scale * 4,
            "snapshot_live_bytes": 100,
            "allocation_count": 1 if zero else scale * 10,
            "allocated_bytes": 10 if zero else scale * 1000,
            "live_bytes_after": 100,
            "peak_live_bytes": 100 + growth,
            "peak_growth_bytes": growth,
            "counter_overflowed": False,
            "counter_underflowed": False,
        }

    def _write_native_memory_report(self) -> None:
        executable_sha256 = sha256_bytes(EXECUTABLE_BYTES)
        native_recipe = run_native_memory.memory_recipe(
            self.root,
            target_dir=self.root / "target",
            toolchain=None,
        )
        build_command = run_native_memory.cargo_prebuild_command(native_recipe)
        schedule: list[dict[str, object]] = []
        responses: list[dict[str, object]] = []
        pair_index = 0
        for scale in MEMORY_SCALES:
            for repeat in range(5):
                modes = (
                    ("operation", "zero")
                    if pair_index % 2 == 0
                    else ("zero", "operation")
                )
                for position, mode in enumerate(modes):
                    response = self._response(
                        scale=scale,
                        repeat=repeat,
                        mode=mode,
                        pair_index=pair_index,
                        executable_sha256=executable_sha256,
                    )
                    request = {
                        field: response[field]
                        for field in (
                            "schema_version",
                            "lane_id",
                            "mode",
                            "scale",
                            "seed",
                            "repeat",
                            "invocation_id",
                            "nonce",
                        )
                    }
                    schedule.append(
                        {
                            "pair_index": pair_index,
                            "position": position,
                            "request": request,
                            "response": response,
                        }
                    )
                    responses.append(response)
                pair_index += 1

        analysis, outcomes = run_native_memory.analyze_samples(
            responses,
            contract=self.owner_contract,
            bootstrap_resamples=10_000,
            seed_material="flowchart-end-to-end-memory:101:5",
        )
        exit_code = run_native_memory.suite_exit_code(outcomes)
        outcome = (
            "failed_bound"
            if exit_code == 1
            else "inconclusive" if exit_code == 3 else "pass"
        )

        report = {
            "schema_version": 1,
            "generated_at": "2026-07-28T08:00:00+00:00",
            "outcome": outcome,
            "exit_code": exit_code,
            "output": str(self.report_path),
            "method": {
                "scales": list(MEMORY_SCALES),
                "repeats": 5,
                "seed": 101,
                "bootstrap_resamples": 10_000,
                "subprocess_isolation": "fresh-process-per-sample",
                "pair_order": "alternating-operation-zero",
                "evidence_class": "infrastructure-smoke",
            },
            "candidate_admission": False,
            "environment": {
                "os": self.host["os"],
                "machine": self.host["architecture"],
                "processor": "Test Processor",
                "cpu": self.host["cpu"],
                "python": "3.13.5",
                "rustc": self.host["rustc"],
                "cargo": self.host["cargo"],
            },
            "contract_errors": [],
            "schedule": schedule,
            "analysis": analysis,
            "lane": {
                "id": "flowchart-end-to-end-memory",
                "public_operation": "render-svg",
                "process_lifecycle": "fresh-process",
                "engine_lifecycle": "reused-engine",
                "logical_operations_per_estimate": 1,
                "transport": "native-system-allocator-subprocess",
                "workload": "flowchart-modular-generator-v1",
                "size_vector": list(MEMORY_SCALES),
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
            "inputs": {
                "workspace_manifest": file_record(self.root / "Cargo.toml"),
                "package_manifest": file_record(
                    self.root / "crates" / "merman" / "Cargo.toml"
                ),
                "cargo_lock": file_record(self.root / "Cargo.lock"),
                "corpus": file_record(self.corpus_path),
                "owner_contract": {
                    **file_record(self.contract_path),
                    "value": self.owner_contract,
                },
            },
            "recipe": {
                "package": native_recipe.package,
                "bench": native_recipe.bench,
                "features": list(native_recipe.features),
                "default_features": native_recipe.default_features,
                "locked": native_recipe.locked,
                "target_dir": str(native_recipe.target_dir),
                "build_command": build_command,
                "build_environment": {
                    "CARGO_BUILD_JOBS": "1",
                    "CARGO_INCREMENTAL": "0",
                    "CARGO_PROFILE_BENCH_DEBUG": "0",
                    "CARGO_PROFILE_BENCH_LTO": None,
                    "CARGO_PROFILE_BENCH_CODEGEN_UNITS": None,
                    "CARGO_PROFILE_BENCH_OPT_LEVEL": None,
                    "RUSTFLAGS": None,
                    "CARGO_ENCODED_RUSTFLAGS": None,
                    "RUSTUP_TOOLCHAIN": None,
                    "RUSTC_WRAPPER": None,
                    "RUSTC_WORKSPACE_WRAPPER": None,
                },
                "requested_toolchain": None,
            },
            "executable": {
                **file_record(self.executable),
                "build": {
                    "command": build_command,
                    "stdout_sha256": sha256_bytes(b"cargo stdout"),
                    "stderr_sha256": sha256_bytes(b"cargo stderr"),
                },
            },
            "run_id": "test-run",
            "source": {
                "commit": COMMIT,
                "tree": TREE,
                "clean": True,
                "dirty_status_sha256": sha256_bytes(b""),
                "dirty_disposition": "clean",
            },
        }
        self._write_json(self.report_path, report)

    def freeze(self) -> dict[str, object]:
        return baseline.freeze_baseline(
            self.root,
            self.report_path,
            source_paths=("bench.py",),
            manifest_paths=("Cargo.toml", "crates/merman/Cargo.toml"),
            lock_path="Cargo.lock",
            corpus_path="tools/bench/corpus.json",
            git_probe=self.git,
            host_probe=lambda _root: self.host,
            frozen_at="2026-07-28T09:00:00+00:00",
        )

    def write_manifest(self, value: dict[str, object] | None = None) -> Path:
        path = self.root / "baseline.json"
        self._write_json(path, value if value is not None else self.freeze())
        return path

    def test_freeze_records_committed_tree_recipes_and_every_fixture(self) -> None:
        manifest = self.freeze()

        self.assertIn(
            "tools/bench/compare_mermaid_renderers.py",
            baseline.DEFAULT_SOURCE_PATHS,
        )
        self.assertEqual(
            manifest["repository"],
            {
                "commit": COMMIT,
                "tree": TREE,
                "result_tree": TREE,
                "patch_stack": [],
            },
        )
        self.assertEqual(
            manifest["artifacts"]["fixtures"],
            [
                {
                    "name": "flowchart",
                    "path": "fixtures/flowchart.mmd",
                    "bytes": self.fixture.stat().st_size,
                    "sha256": sha256_bytes(self.fixture.read_bytes()),
                }
            ],
        )
        latency = manifest["recipes"]["latency_common_aa"]
        self.assertEqual(latency["calibration_pairs"], 8)
        self.assertEqual(latency["process_lifecycle"], "reused-process")
        memory = manifest["recipes"]["native_memory"]
        self.assertEqual(memory["scales"], list(MEMORY_SCALES))
        self.assertEqual(memory["repeats"], 5)
        self.assertEqual(memory["seed"], 101)
        self.assertEqual(memory["process_lifecycle"], "fresh-process")
        self.assertEqual(manifest["host"], self.host)

    def test_freeze_collects_host_with_reported_toolchain(self) -> None:
        report = baseline.load_strict_json(self.report_path)
        report["recipe"]["requested_toolchain"] = "nightly-2026-07-01"
        recipe = run_native_memory.memory_recipe(
            self.root,
            target_dir=self.root / "target",
            toolchain="nightly-2026-07-01",
        )
        command = run_native_memory.cargo_prebuild_command(recipe)
        report["recipe"]["build_command"] = command
        report["executable"]["build"]["command"] = command
        self._write_json(self.report_path, report)
        observed: list[str | None] = []

        manifest = baseline.freeze_baseline(
            self.root,
            self.report_path,
            source_paths=("bench.py",),
            manifest_paths=("Cargo.toml", "crates/merman/Cargo.toml"),
            lock_path="Cargo.lock",
            corpus_path="tools/bench/corpus.json",
            git_probe=self.git,
            host_probe=lambda _root, toolchain: (
                observed.append(toolchain) or self.host
            ),
            frozen_at="2026-07-28T09:00:00+00:00",
        )

        self.assertEqual(observed, ["nightly-2026-07-01"])
        self.assertEqual(manifest["host"], self.host)

    def test_freezer_accepts_exactly_the_driver_build_environment_fields(self) -> None:
        self.assertEqual(
            baseline._BUILD_ENVIRONMENT_FIELDS,
            frozenset(run_native_memory._build_environment()),
        )

    def test_collect_host_runs_rust_tools_through_requested_toolchain(self) -> None:
        commands: list[list[str]] = []
        cpu_probe = mock.Mock(return_value="Test CPU")

        def command_output(
            command: list[str], *, root: Path, context: str
        ) -> str:
            self.assertEqual(root, self.root)
            commands.append(command)
            return "rustc test" if context == "rustc" else "cargo test"

        with (
            mock.patch.object(baseline, "_command_output", side_effect=command_output),
            mock.patch.object(
                baseline,
                "best_effort_cpu_model",
                cpu_probe,
            ),
            mock.patch.object(baseline.platform, "platform", return_value="TestOS-1"),
            mock.patch.object(baseline.platform, "machine", return_value="test64"),
        ):
            host = baseline.collect_host(self.root, "nightly-2026-07-01")

        self.assertEqual(
            commands,
            [
                [
                    "rustup",
                    "run",
                    "nightly-2026-07-01",
                    "rustc",
                    "-Vv",
                ],
                [
                    "rustup",
                    "run",
                    "nightly-2026-07-01",
                    "cargo",
                    "-V",
                ],
            ],
        )
        self.assertEqual(
            host,
            {
                "rustc": "rustc test",
                "cargo": "cargo test",
                "os": "TestOS-1",
                "cpu": "Test CPU",
                "architecture": "test64",
            },
        )
        cpu_probe.assert_called_once_with()

    def test_freeze_rejects_noncanonical_native_recipe_and_build_commands(self) -> None:
        original = baseline.load_strict_json(self.report_path)
        cases = (
            ("package", ("recipe", "package"), "other-package"),
            ("bench", ("recipe", "bench"), "pipeline"),
            ("features", ("recipe", "features"), ["svg", "layout"]),
            ("default_features", ("recipe", "default_features"), True),
            ("locked", ("recipe", "locked"), False),
            (
                "requested_toolchain",
                ("recipe", "requested_toolchain"),
                "nightly-2026-07-01",
            ),
            (
                "target_dir",
                ("recipe", "target_dir"),
                str(self.root / "other-target"),
            ),
            (
                "recipe.build_command",
                ("recipe", "build_command"),
                [*original["recipe"]["build_command"], "--quiet"],
            ),
            (
                "executable.build.command",
                ("executable", "build", "command"),
                [*original["executable"]["build"]["command"], "--quiet"],
            ),
        )

        for label, path, value in cases:
            with self.subTest(field=label):
                damaged = copy.deepcopy(original)
                target = damaged
                for segment in path[:-1]:
                    target = target[segment]
                target[path[-1]] = value
                self._write_json(self.report_path, damaged)
                with self.assertRaisesRegex(
                    baseline.ManifestError,
                    "canonical native-memory recipe|build command|locked prebuild",
                ):
                    self.freeze()

    def test_freeze_rejects_skipped_executable_build(self) -> None:
        report = baseline.load_strict_json(self.report_path)
        report["executable"]["build"] = {"skipped": "explicit executable"}
        self._write_json(self.report_path, report)

        with self.assertRaisesRegex(
            baseline.ManifestError,
            "persistent baseline requires a Cargo-built executable",
        ):
            self.freeze()

    def test_freeze_rejects_report_source_commit_tree_and_dirty_state(self) -> None:
        original = baseline.load_strict_json(self.report_path)
        cases = (
            ("commit", {"commit": "3" * 40}, "source commit/tree differs"),
            ("tree", {"tree": "3" * 40}, "source commit/tree differs"),
            (
                "dirty",
                {
                    "clean": False,
                    "dirty_status_sha256": sha256_bytes(b" M source.rs"),
                    "dirty_disposition": "allowed-diagnostic",
                },
                "not collected from a clean source tree",
            ),
        )

        for label, changes, expected_error in cases:
            with self.subTest(source=label):
                damaged = copy.deepcopy(original)
                damaged["source"].update(changes)
                self._write_json(self.report_path, damaged)
                with self.assertRaisesRegex(baseline.ManifestError, expected_error):
                    self.freeze()

    def test_freeze_rejects_a_dirty_tree_before_recording_inputs(self) -> None:
        self.git.current = baseline.GitSnapshot(
            commit=COMMIT,
            tree=TREE,
            status=("?? scratch.txt",),
        )

        with self.assertRaisesRegex(baseline.ManifestError, "clean committed tree"):
            self.freeze()

    def test_verify_rejects_commit_tree_and_result_tree_mismatches(self) -> None:
        path = self.write_manifest()

        self.git.current = baseline.GitSnapshot(commit="3" * 40, tree=TREE, status=())
        with self.assertRaisesRegex(baseline.ManifestError, "commit mismatch"):
            baseline.verify_baseline(self.root, path, git_probe=self.git)

        self.git.current = baseline.GitSnapshot(commit=COMMIT, tree="3" * 40, status=())
        with self.assertRaisesRegex(baseline.ManifestError, "tree mismatch"):
            baseline.verify_baseline(self.root, path, git_probe=self.git)

        self.git.current = self.clean
        damaged = self.freeze()
        damaged["repository"]["result_tree"] = "3" * 40
        self._write_json(path, damaged)
        with self.assertRaisesRegex(baseline.ManifestError, "result_tree mismatch"):
            baseline.verify_baseline(self.root, path, git_probe=self.git)

    def test_verify_checks_nonempty_patch_entry_digest(self) -> None:
        patch_path = self.root / "patches" / "0001.patch"
        patch_path.parent.mkdir()
        patch_path.write_bytes(b"diff --git a/a b/a\n")
        manifest = self.freeze()
        manifest["repository"]["patch_stack"] = [
            {
                "order": 1,
                "path": "patches/0001.patch",
                "bytes": patch_path.stat().st_size,
                "sha256": "0" * 64,
            }
        ]
        path = self.write_manifest(manifest)

        with self.assertRaisesRegex(
            baseline.ManifestError, r"patch_stack\[0\].*sha256 mismatch"
        ):
            baseline.verify_baseline(self.root, path, git_probe=self.git)

    def test_verify_rejects_fixture_digest_mismatch(self) -> None:
        path = self.write_manifest()
        self.fixture.write_text("flowchart LR\n  A --> C\n", encoding="utf-8")

        with self.assertRaisesRegex(
            baseline.ManifestError, r"fixtures\[0\].*sha256 mismatch"
        ):
            baseline.verify_baseline(self.root, path, git_probe=self.git)

    def test_verify_rejects_report_and_executable_digest_mismatches(self) -> None:
        path = self.write_manifest()
        original_report = self.report_path.read_bytes()
        self.report_path.write_bytes(original_report + b" ")
        with self.assertRaisesRegex(
            baseline.ManifestError, "native_memory_report.*bytes mismatch"
        ):
            baseline.verify_baseline(self.root, path, git_probe=self.git)

        self.report_path.write_bytes(original_report)
        self.executable.write_bytes(b"changed executable")
        with self.assertRaisesRegex(
            baseline.ManifestError, "native_memory_executable.*(bytes|sha256) mismatch"
        ):
            baseline.verify_baseline(self.root, path, git_probe=self.git)

    def test_freeze_rejects_tampered_counters_analysis_and_aggregate_outcome(self) -> None:
        original = json.loads(self.report_path.read_text(encoding="utf-8"))

        counter_tamper = copy.deepcopy(original)
        counter_tamper["schedule"][0]["response"]["peak_growth_bytes"] += 1
        self._write_json(self.report_path, counter_tamper)
        with self.assertRaisesRegex(
            baseline.ManifestError, "response protocol is invalid"
        ):
            self.freeze()

        analysis_tamper = copy.deepcopy(original)
        analysis_tamper["analysis"]["metrics"]["allocated_bytes"][
            "outcome"
        ] = "inconclusive"
        self._write_json(self.report_path, analysis_tamper)
        with self.assertRaisesRegex(
            baseline.ManifestError, "analysis differs from recomputed"
        ):
            self.freeze()

        aggregate_tamper = copy.deepcopy(original)
        aggregate_tamper["outcome"] = "inconclusive"
        aggregate_tamper["exit_code"] = 3
        self._write_json(self.report_path, aggregate_tamper)
        with self.assertRaisesRegex(
            baseline.ManifestError, "outcome/exit_code differs from recomputed"
        ):
            self.freeze()

    def test_manifest_schema_and_json_parser_fail_closed(self) -> None:
        manifest = self.freeze()
        damaged = copy.deepcopy(manifest)
        damaged["unknown"] = True
        with self.assertRaisesRegex(baseline.ManifestError, "fields differ"):
            baseline.validate_manifest(damaged)

        duplicate = self.root / "duplicate.json"
        duplicate.write_text('{"schema_version":1,"schema_version":1}\n')
        with self.assertRaisesRegex(baseline.ManifestError, "duplicate JSON key"):
            baseline.load_strict_json(duplicate)

        for token in ("NaN", "1e400"):
            invalid = self.root / f"non-finite-{token}.json"
            invalid.write_text(f'{{"value":{token}}}\n', encoding="utf-8")
            with self.subTest(token=token), self.assertRaisesRegex(
                baseline.ManifestError, "non-finite JSON number"
            ):
                baseline.load_strict_json(invalid)

    def test_atomic_freeze_output_round_trips_through_verify(self) -> None:
        output = self.root / "out" / "baseline.json"
        baseline.freeze_to_file(
            self.root,
            self.report_path,
            output,
            source_paths=("bench.py",),
            manifest_paths=("Cargo.toml", "crates/merman/Cargo.toml"),
            lock_path="Cargo.lock",
            corpus_path="tools/bench/corpus.json",
            git_probe=self.git,
            host_probe=lambda _root: self.host,
            frozen_at="2026-07-28T09:00:00+00:00",
        )

        baseline.verify_baseline(self.root, output, git_probe=self.git)
        self.assertEqual(list(output.parent.glob("*.tmp")), [])


if __name__ == "__main__":
    unittest.main()
