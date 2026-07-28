#!/usr/bin/env python3
"""Contract tests for the corpus-driven performance helper scripts."""

from __future__ import annotations

import copy
import io
import json
import math
import os
import stat
import sys
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path
from unittest import mock

import perf_runner
import compare_self
import compare_mermaid_renderers
import render_perf_comment
import run_native_memory
import stage_spotcheck
import verify_pipeline_bench_list
from corpus_utils import (
    compare_mmdr_fixture_inputs,
    fixture_names_for_suite,
    lane_selector_group,
    load_corpus,
    resolve_lane_group,
    select_corpus_fixtures,
)


ROOT = Path(__file__).resolve().parents[2]
CORPUS_PATH = ROOT / "tools" / "bench" / "corpus.json"


class CorpusContractsTest(unittest.TestCase):
    def test_pipeline_groups_are_owned_by_registered_lanes(self) -> None:
        corpus = load_corpus(CORPUS_PATH)
        expected_groups = {
            "parse",
            "compatibility_json_parse",
            "parse_cold_engine",
            "frontmatter_preprocess",
            "layout",
            "render",
            "end_to_end",
        }

        self.assertEqual(
            {group: resolve_lane_group(corpus, group).id for group in expected_groups},
            {
                "parse": "typed-render-model-parse",
                "compatibility_json_parse": "compatibility-json-parse",
                "parse_cold_engine": "typed-render-model-parse-cold",
                "frontmatter_preprocess": "frontmatter-preprocess-known-type",
                "layout": "prepare-layout",
                "render": "emit-svg",
                "end_to_end": "render-svg",
            },
        )

        with self.assertRaisesRegex(compare_self.ContractViolation, "no registered lane"):
            compare_self._optional_lane_for_group(corpus, "unregistered_group")

    def test_compiled_pipeline_list_requires_current_groups_and_rejects_history(self) -> None:
        corpus = load_corpus(CORPUS_PATH)
        current_groups = sorted(
            lane_selector_group(lane.selector)
            for lane in corpus.lanes
            if lane.transport == "native-criterion"
            and set(lane.required_features).issubset({"svg"})
        )
        fixture = corpus.fixtures[0].name
        output = "\n".join(
            f"{group}/{fixture}: benchmark" for group in current_groups
        )

        result = verify_pipeline_bench_list.validate_pipeline_bench_list(corpus, output)

        self.assertEqual(result["groups"], tuple(current_groups))
        historical = output.replace(
            f"compatibility_json_parse/{fixture}",
            f"parse_known_type/{fixture}",
        )
        with self.assertRaisesRegex(
            verify_pipeline_bench_list.PipelineBenchListError,
            "historical lane aliases",
        ):
            verify_pipeline_bench_list.validate_pipeline_bench_list(corpus, historical)

        with self.assertRaisesRegex(
            verify_pipeline_bench_list.PipelineBenchListError,
            r"unknown=\['unregistered'\]",
        ):
            verify_pipeline_bench_list.validate_pipeline_bench_list(
                corpus,
                output + f"\nunregistered/{fixture}: benchmark\n",
            )

    def test_canary_suite_is_standard_hotspot_set(self) -> None:
        corpus = load_corpus(CORPUS_PATH)

        self.assertEqual(
            fixture_names_for_suite(corpus, "canary"),
            (
                "flowchart_medium",
                "class_medium",
                "mindmap_medium",
                "architecture_medium",
            ),
        )

    def test_frontmatter_suite_covers_preprocess_fixtures(self) -> None:
        corpus = load_corpus(CORPUS_PATH)

        self.assertEqual(
            fixture_names_for_suite(corpus, "frontmatter"),
            (
                "frontmatter_basic",
                "frontmatter_indented",
                "frontmatter_deep_config",
            ),
        )

    def test_full_suite_uses_all_fixtures_in_corpus_order(self) -> None:
        corpus = load_corpus(CORPUS_PATH)

        self.assertEqual(select_corpus_fixtures(corpus, "full"), list(corpus.fixtures))

    def test_flowchart_adapter_memory_lanes_match_candidate_contract_identities(self) -> None:
        corpus = load_corpus(CORPUS_PATH)
        lanes = {lane.id: lane for lane in corpus.lanes}
        expected = {
            "flowchart-adapter-low-cluster-memory": {
                "selector": "memory/adapter_low_clusters/{fixture}",
                "workload": "flowchart-adapter-low-cluster-generator-v1",
                "contract": (
                    "docs/performance/contracts/"
                    "flowchart-u4-adapter-low-cluster-memory-v1.json"
                ),
            },
            "flowchart-adapter-high-cluster-memory": {
                "selector": "memory/adapter_high_clusters/{fixture}",
                "workload": "flowchart-adapter-high-cluster-generator-v1",
                "contract": (
                    "docs/performance/contracts/"
                    "flowchart-u4-adapter-high-cluster-memory-v1.json"
                ),
            },
        }
        metrics = ("allocation_count", "allocated_bytes", "peak_growth_bytes")
        semantic_dimensions = (
            "input_nodes",
            "input_edges",
            "svg_sha256",
            "svg_viewbox_width",
            "svg_viewbox_height",
        )

        for lane_id, identity in expected.items():
            with self.subTest(lane=lane_id):
                lane = lanes[lane_id]
                self.assertEqual(lane.kind, "public")
                self.assertEqual(lane.public_operation, "render-svg")
                self.assertEqual(lane.process_lifecycle, "fresh-process")
                self.assertEqual(lane.engine_lifecycle, "reused-engine")
                self.assertEqual(lane.transport, "native-system-allocator-subprocess")
                self.assertEqual(lane.logical_operations_per_estimate, 1)
                self.assertEqual(lane.selector, identity["selector"])
                self.assertEqual(lane.size_vector, (1, 2, 4, 10, 32, 100))
                self.assertEqual(lane.workload, identity["workload"])
                self.assertEqual(lane.evidence_contract, identity["contract"])
                self.assertEqual(lane.measurement_metrics, metrics)
                self.assertEqual(lane.semantic_output_dimensions, semantic_dimensions)

                contract_path = ROOT / str(lane.evidence_contract)
                contract = json.loads(contract_path.read_text(encoding="utf-8"))
                self.assertEqual(contract["schema_version"], 1)
                self.assertEqual(contract["lane_id"], lane.id)
                self.assertEqual(contract["workload"], lane.workload)
                self.assertEqual(contract["evidence_class"], "candidate-bound")
                self.assertIs(contract["candidate_admission"], True)
                self.assertEqual(
                    contract["generator"],
                    {
                        "id": lane.workload,
                        "nodes_per_scale": 4,
                        "edges_per_scale": 4,
                    },
                )
                self.assertEqual(tuple(contract["metrics"]), metrics)
                for metric in metrics:
                    self.assertEqual(
                        set(contract["metrics"][metric]),
                        {"slope_cap", "max_scale_cap"},
                    )
                    self.assertGreater(contract["metrics"][metric]["slope_cap"], 0)
                    self.assertGreater(contract["metrics"][metric]["max_scale_cap"], 0)


class PerfRunnerContractsTest(unittest.TestCase):
    def test_canary_dry_run_uses_corpus_suite_for_comparison(self) -> None:
        buf = io.StringIO()

        with redirect_stdout(buf):
            result = perf_runner.main(["--profile", "canary", "--dry-run"])

        self.assertEqual(result, 0)
        out = buf.getvalue().replace("\\", "/")
        self.assertIn(
            "stage spotcheck (flowchart_medium,class_medium,mindmap_medium,architecture_medium)",
            out,
        )
        self.assertIn("compare_mermaid_renderers.py", out)
        self.assertIn("--preset long --suite canary", out)
        self.assertIn("--skip-mermaid-js", out)

    def test_triage_dry_run_includes_cold_parse_steps(self) -> None:
        buf = io.StringIO()

        with redirect_stdout(buf):
            result = perf_runner.main(
                ["--profile", "triage", "--include-cold-parse", "--dry-run"]
            )

        self.assertEqual(result, 0)
        out = buf.getvalue().replace("\\", "/")
        self.assertIn("cold parse (flowchart_medium)", out)
        self.assertIn("parse_cold_engine/flowchart_medium", out)
        self.assertIn("cold parse (architecture_medium)", out)

    def test_frontmatter_profile_dry_run_adds_preprocess_steps(self) -> None:
        buf = io.StringIO()

        with redirect_stdout(buf):
            result = perf_runner.main(
                [
                    "--profile",
                    "triage",
                    "--stage-fixtures",
                    "frontmatter_basic,frontmatter_indented,frontmatter_deep_config",
                    "--dry-run",
                ]
            )

        self.assertEqual(result, 0)
        out = buf.getvalue().replace("\\", "/")
        self.assertIn(
            "stage spotcheck (frontmatter_basic,frontmatter_indented,frontmatter_deep_config)",
            out,
        )

    def test_full_write_docs_dry_run_writes_suite_report_to_docs(self) -> None:
        buf = io.StringIO()

        with redirect_stdout(buf):
            result = perf_runner.main(
                ["--profile", "full", "--write-docs", "--dry-run"]
            )

        self.assertEqual(result, 0)
        out = buf.getvalue().replace("\\", "/")
        self.assertIn(
            "Output mode: docs/performance (Markdown), target/bench/perf-runner (JSON)",
            out,
        )
        self.assertIn("broader compare suite (standard)", out)
        self.assertIn(
            "docs/performance/renderer_comparison_"
            f"{perf_runner.today_stamp()}_perf-runner_full_suite_standard.md",
            out,
        )
        self.assertIn(
            f"target/bench/perf-runner/{perf_runner.today_stamp()}_full_suite_standard.json",
            out,
        )
        self.assertNotIn("run_native_memory.py", out)

    def test_native_memory_is_explicit_and_uses_the_isolated_driver(self) -> None:
        buf = io.StringIO()

        with redirect_stdout(buf):
            result = perf_runner.main(
                ["--profile", "triage", "--include-native-memory", "--dry-run"]
            )

        self.assertEqual(result, 0)
        out = buf.getvalue().replace("\\", "/")
        self.assertIn("native memory (flowchart-end-to-end-memory)", out)
        self.assertIn("run_native_memory.py", out)
        self.assertIn("--repeats 5", out)
        self.assertIn("--bootstrap-resamples 10000", out)
        self.assertIn(
            f"target/bench/perf-runner/{perf_runner.today_stamp()}_triage_native_memory.json",
            out,
        )


class NativeMemoryDriverSelectionContractsTest(unittest.TestCase):
    @staticmethod
    def _dry_run_driver(*, lane: str, corpus: Path = CORPUS_PATH) -> tuple[int, str, str]:
        stdout = io.StringIO()
        stderr = io.StringIO()
        source = {
            "commit": "a" * 40,
            "tree": "b" * 40,
            "clean": True,
            "dirty_status_sha256": "c" * 64,
            "dirty_disposition": "clean",
        }
        with (
            mock.patch.object(run_native_memory, "_git_provenance", return_value=source),
            mock.patch.object(run_native_memory, "best_effort_cpu_model", return_value="test-cpu"),
            redirect_stdout(stdout),
            redirect_stderr(stderr),
        ):
            result = run_native_memory.main(
                ["--corpus", str(corpus), "--lane", lane, "--dry-run"]
            )
        return result, stdout.getvalue(), stderr.getvalue()

    def test_registered_adapter_memory_lanes_accept_only_their_registered_workloads(self) -> None:
        expected = {
            "flowchart-adapter-low-cluster-memory": "flowchart-adapter-low-cluster-generator-v1",
            "flowchart-adapter-high-cluster-memory": "flowchart-adapter-high-cluster-generator-v1",
        }
        self.assertEqual(
            {
                lane: run_native_memory._SUPPORTED_LANE_WORKLOADS[lane]
                for lane in expected
            },
            expected,
        )

        for lane in expected:
            with self.subTest(lane=lane):
                result, stdout, stderr = self._dry_run_driver(lane=lane)
                self.assertEqual(result, 0)
                self.assertIn("$ cargo bench", stdout)
                self.assertEqual(stderr, "")

    def test_native_memory_driver_rejects_unknown_or_mismatched_adapter_lane_workloads(self) -> None:
        unknown_result, _stdout, unknown_stderr = self._dry_run_driver(
            lane="flowchart-adapter-unregistered-memory"
        )
        self.assertEqual(unknown_result, 2)
        self.assertIn("unknown lane selector", unknown_stderr)

        source_corpus = json.loads(CORPUS_PATH.read_text(encoding="utf-8"))
        mismatches = {
            "flowchart-adapter-low-cluster-memory": (
                "flowchart-adapter-high-cluster-generator-v1"
            ),
            "flowchart-adapter-high-cluster-memory": (
                "flowchart-adapter-low-cluster-generator-v1"
            ),
        }
        for lane_id, wrong_workload in mismatches.items():
            with self.subTest(lane=lane_id, workload=wrong_workload):
                corpus = copy.deepcopy(source_corpus)
                lane = next(item for item in corpus["lanes"] if item["id"] == lane_id)
                lane["workload"] = wrong_workload
                with tempfile.TemporaryDirectory() as temp_dir:
                    mismatched_corpus = Path(temp_dir) / "corpus.json"
                    mismatched_corpus.write_text(json.dumps(corpus), encoding="utf-8")
                    result, _stdout, stderr = self._dry_run_driver(
                        lane=lane_id,
                        corpus=mismatched_corpus,
                    )

                self.assertEqual(result, 2)
                self.assertIn("selected lane semantics are unsupported", stderr)


class CompareSelfContractsTest(unittest.TestCase):
    @staticmethod
    def _contract(name: str = "flowchart_medium") -> dict[str, object]:
        return {
            "name": name,
            "family": "flowchart",
            "base_benchmark": f"end_to_end/{name}",
            "head_benchmark": f"end_to_end/{name}",
            "coverage_status": "comparable",
        }

    @staticmethod
    def _minimal_corpus(*, schema_version: int, default_group: str) -> dict[str, object]:
        corpus = json.loads(CORPUS_PATH.read_text(encoding="utf-8"))
        fixture = next(
            item
            for item in corpus["fixtures"]
            if item["name"] == "flowchart_medium"
        )
        corpus["schema_version"] = schema_version
        corpus["default_group"] = default_group
        corpus["fixtures"] = [fixture]
        if schema_version == 1:
            corpus.pop("lanes", None)
        return corpus

    @staticmethod
    def _write_comparison_checkout(
        checkout: Path,
        corpus: dict[str, object],
    ) -> compare_self.RunnerRecipe:
        corpus_path = checkout / "tools" / "bench" / "corpus.json"
        corpus_path.parent.mkdir(parents=True)
        corpus_path.write_text(json.dumps(corpus), encoding="utf-8")
        fixture = corpus["fixtures"][0]
        assert isinstance(fixture, dict)
        fixture_source = Path(str(fixture["source"]))
        source = ROOT / fixture_source
        target = checkout / fixture_source
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(source.read_bytes())
        return compare_self.RunnerRecipe(
            label=checkout.name,
            checkout=checkout,
            package="merman",
            bench="pipeline",
            features=("svg",),
            default_features=False,
            toolchain=None,
            target_dir=checkout / "target",
            locked=True,
            corpus=Path("tools/bench/corpus.json"),
        )

    @staticmethod
    def _pairs(
        base_ns: float,
        head_ns: float,
        *,
        count: int = 8,
    ) -> list[dict[str, object]]:
        return [
            {
                "pair_index": pair_index,
                "order": "AB" if pair_index % 2 == 0 else "BA",
                "base": {"normalized_ns": base_ns},
                "head": {"normalized_ns": head_ns},
            }
            for pair_index in range(count)
        ]

    def test_v2_rows_require_both_relative_and_absolute_regression_bounds(self) -> None:
        common = {
            "contract": self._contract(),
            "evidence_mode": "confirmation",
            "required_pairs": 8,
            "relative_threshold": math.log1p(0.10),
            "absolute_threshold_ns": 50_000.0,
            "confidence_level": 0.95,
            "bootstrap_seed": 7,
            "bootstrap_resamples": 100,
        }

        ratio_only = compare_self._comparison_row(
            common.pop("contract"),
            pairs=self._pairs(100.0, 112.0),
            **common,
        )
        regression = compare_self._comparison_row(
            self._contract(),
            pairs=self._pairs(1_000_000.0, 1_120_000.0),
            **common,
        )

        self.assertEqual(ratio_only["outcome"], "confirmed_non_regression")
        self.assertEqual(regression["outcome"], "confirmed_regression")
        self.assertEqual(regression["pair_count"], 8)
        self.assertGreater(regression["bounds"]["relative_percent"]["lower"], 10.0)
        self.assertGreater(regression["bounds"]["absolute_ns"]["lower"], 50_000.0)

    def test_paired_bounds_use_suite_level_bonferroni_confidence(self) -> None:
        marginal_bounds = {"estimate": 0.0, "lower": -0.1, "upper": 0.1}
        with mock.patch.object(
            compare_self,
            "_bootstrap_mean_bounds",
            return_value=marginal_bounds,
        ) as bootstrap:
            bounds = compare_self.paired_bounds(
                base_ns=[100.0, 101.0],
                head_ns=[101.0, 102.0],
                confidence_level=0.95,
                bootstrap_resamples=100,
                family_size=8,
            )

        self.assertEqual(bootstrap.call_count, 2)
        for call in bootstrap.call_args_list:
            self.assertAlmostEqual(call.kwargs["confidence_level"], 0.99375)
        self.assertEqual(
            bounds["confidence_contract"],
            {
                "simultaneous_confidence_level": 0.95,
                "component_confidence_level": 0.99375,
                "family_size": 8,
                "multiplicity_adjustment": "bonferroni",
            },
        )

    def test_aa_calibration_uses_equivalence_margins_not_exact_zero(self) -> None:
        pairs = [
            {
                "a": {"normalized_ns": 1_000_000.0},
                "b": {"normalized_ns": 1_001_000.0},
                "first": {"normalized_ns": 1_000_000.0},
                "second": {"normalized_ns": 1_001_000.0},
            }
            for _ in range(8)
        ]

        calibration = compare_self._summarize_calibration(
            pairs,
            expected_pairs=8,
            relative_mde=math.log1p(0.10),
            absolute_mde_ns=50_000.0,
            max_pairs=32,
            confidence_level=0.95,
            bootstrap_seed=7,
            bootstrap_resamples=10_000,
        )

        self.assertEqual(calibration["status"], "stable")
        self.assertTrue(calibration["stable"])
        self.assertTrue(calibration["checks"]["identity_within_equivalence_margin"])
        self.assertTrue(calibration["checks"]["order_effect_within_equivalence_margin"])

    def test_incomplete_comparison_pairs_are_an_evidence_contract_failure(self) -> None:
        row = compare_self._comparison_row(
            self._contract("mindmap_medium"),
            pairs=[],
            evidence_mode="confirmation",
            required_pairs=8,
            relative_threshold=math.log1p(0.10),
            absolute_threshold_ns=50_000.0,
            confidence_level=0.95,
            bootstrap_seed=0,
            bootstrap_resamples=100,
        )

        self.assertEqual(row["outcome"], "contract_failure")
        self.assertIn("complete pairs", row["reason"])

    def test_markdown_includes_manual_comparison_labels_and_preset(self) -> None:
        report = {
            "schema_version": 2,
            "summary": {
                "outcome": "confirmed_non_regression",
                "exit_code": 0,
                "comparable": 1,
            },
            "method": {
                "preset": "long",
                "evidence_mode": "confirmation",
                "evidence_quality": "decision_grade",
                "relative_threshold_percent": 10.0,
                "absolute_threshold_ns": 50_000.0,
                "confidence_level": 0.95,
                "confidence_contract": {
                    "simultaneous_confidence_level": 0.95,
                    "component_confidence_level": 0.975,
                    "family_size": 2,
                    "multiplicity_adjustment": "bonferroni",
                },
            },
            "comparison": {
                "base_label": "Latias94/merman@main",
                "head_label": "Latias94/merman@perf-branch",
            },
            "recipes": {
                "base": {
                    "package": "merman",
                    "bench": "pipeline",
                    "features": ["svg"],
                    "logical_operations": 1,
                },
                "head": {
                    "package": "merman",
                    "bench": "pipeline",
                    "features": ["svg"],
                    "logical_operations": 1,
                },
            },
            "runners": {
                "base": {
                    "git": {"revision": "base-sha"},
                    "executable": {"sha256": "base-executable"},
                },
                "head": {
                    "git": {"revision": "head-sha"},
                    "executable": {"sha256": "head-executable"},
                },
            },
            "rows": [
                {
                    "benchmark": "end_to_end/flowchart_medium",
                    "base_benchmark": "end_to_end/flowchart_medium",
                    "head_benchmark": "end_to_end/flowchart_medium",
                    "base_ns": 1_000_000.0,
                    "head_ns": 990_000.0,
                    "bounds": {
                        "relative_percent": {"lower": -2.0, "upper": 0.0},
                        "absolute_ns": {"lower": -20_000.0, "upper": 0.0},
                    },
                    "outcome": "confirmed_non_regression",
                    "improvement_outcome": "confirmed_non_improvement",
                }
            ],
            "contract_errors": [],
        }

        with tempfile.TemporaryDirectory() as temp_dir:
            output = Path(temp_dir) / "report.md"
            compare_self.write_decision_markdown(output, report)
            body = output.read_text(encoding="utf-8")

        self.assertIn("schema v2", body)
        self.assertIn("- Preset: `long`", body)
        self.assertIn("- Base label: `Latias94/merman@main`", body)
        self.assertIn("- Head label: `Latias94/merman@perf-branch`", body)
        self.assertIn("logical operations `1`", body)
        self.assertIn("- Simultaneous confidence: `95%`", body)
        self.assertIn("- Multiplicity adjustment: `bonferroni`", body)
        self.assertIn("- Confidence family: `2` components at `97.5%` each", body)
        self.assertIn("| `end_to_end/flowchart_medium` | `end_to_end/flowchart_medium` |", body)
        self.assertIn("relative bounds", body)
        self.assertNotIn("geomean", body.lower())

    def test_balanced_ab_schedule_alternates_runner_order(self) -> None:
        base = mock.Mock()
        base.recipe.label = "base"
        head = mock.Mock()
        head.recipe.label = "head"
        calls: list[tuple[str, int]] = []

        def fake_measure(runner, **kwargs):
            calls.append((runner.recipe.label, kwargs["sequence_index"]))
            return {
                "runner": runner.recipe.label,
                "normalized_ns": 100.0 if runner.recipe.label == "base" else 101.0,
            }

        with mock.patch.object(compare_self, "_measure_once", side_effect=fake_measure):
            schedule = compare_self._run_ab_schedule(
                base=base,
                head=head,
                contracts=[self._contract()],
                pair_count=4,
                start_side="base",
                sample_size=10,
                warm_up_seconds=1,
                measurement_seconds=1,
                timeout_seconds=30,
            )

        self.assertEqual(
            [side for side, _ in calls],
            ["base", "head", "head", "base", "base", "head", "head", "base"],
        )
        self.assertEqual([index for _, index in calls], list(range(1, 9)))
        self.assertEqual([row["order"] for row in schedule["rows"]["flowchart_medium"]], ["AB", "BA", "AB", "BA"])

    def test_invalid_numeric_contract_still_writes_schema_v2_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            base = root / "base"
            head = root / "head"
            base.mkdir()
            head.mkdir()
            markdown = root / "invalid.md"
            structured = root / "invalid.json"

            result = compare_self.main(
                [
                    "--base-dir",
                    str(base),
                    "--head-dir",
                    str(head),
                    "--relative-threshold-percent",
                    "-200",
                    "--out",
                    str(markdown),
                    "--json-out",
                    str(structured),
                ]
            )

            report = json.loads(structured.read_text(encoding="utf-8"))
            body = markdown.read_text(encoding="utf-8")

        self.assertEqual(result, 2)
        self.assertEqual(report["schema_version"], 2)
        self.assertEqual(report["summary"]["outcome"], "contract_failure")
        self.assertEqual(report["summary"]["exit_code"], 2)
        self.assertIn("relative-threshold-percent", report["contract_errors"][0]["message"])
        self.assertIn("Outcome: `contract_failure`", body)

    def test_confirmation_rejects_non_decision_grade_bootstrap_count(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            base = root / "base"
            head = root / "head"
            base.mkdir()
            head.mkdir()
            markdown = root / "invalid.md"
            structured = root / "invalid.json"

            result = compare_self.main(
                [
                    "--base-dir",
                    str(base),
                    "--head-dir",
                    str(head),
                    "--evidence-mode",
                    "confirmation",
                    "--bootstrap-resamples",
                    str(compare_self.DECISION_GRADE_BOOTSTRAP_RESAMPLES - 1),
                    "--out",
                    str(markdown),
                    "--json-out",
                    str(structured),
                ]
            )

            report = json.loads(structured.read_text(encoding="utf-8"))

        self.assertEqual(result, 2)
        self.assertEqual(report["summary"]["outcome"], "contract_failure")
        self.assertIn("bootstrap-resamples", report["contract_errors"][0]["message"])
        self.assertIn("confirmation", report["contract_errors"][0]["message"])

    def test_discovery_reuse_is_confirmation_only_and_does_not_rebuild(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            base = root / "base"
            head = root / "head"
            base.mkdir()
            head.mkdir()
            source = root / "discovery.json"
            source.write_text("{}\n", encoding="utf-8")
            markdown = root / "invalid.md"
            structured = root / "invalid.json"

            with mock.patch.object(compare_self, "_execute_comparison") as execute:
                result = compare_self.main(
                    [
                        "--base-dir",
                        str(base),
                        "--head-dir",
                        str(head),
                        "--reuse-discovery-json",
                        str(source),
                        "--reuse-discovery-sha256",
                        compare_self.hashlib.sha256(b"{}\n").hexdigest(),
                        "--freeze-shared-target",
                        "--evidence-mode",
                        "confirmation",
                        "--out",
                        str(markdown),
                        "--json-out",
                        str(structured),
                    ]
                )

            report = json.loads(structured.read_text(encoding="utf-8"))

        self.assertEqual(result, 2)
        execute.assert_not_called()
        self.assertIn("mutually exclusive", report["contract_errors"][0]["message"])

    def test_confirmation_rejects_an_unbounded_bootstrap_count(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            base = root / "base"
            head = root / "head"
            base.mkdir()
            head.mkdir()
            markdown = root / "invalid.md"
            structured = root / "invalid.json"

            result = compare_self.main(
                [
                    "--base-dir",
                    str(base),
                    "--head-dir",
                    str(head),
                    "--evidence-mode",
                    "confirmation",
                    "--bootstrap-resamples",
                    str(compare_self.MAX_BOOTSTRAP_RESAMPLES + 1),
                    "--out",
                    str(markdown),
                    "--json-out",
                    str(structured),
                ]
            )

            report = json.loads(structured.read_text(encoding="utf-8"))

        self.assertEqual(result, 2)
        self.assertEqual(report["summary"]["outcome"], "contract_failure")
        self.assertIn("at most", report["contract_errors"][0]["message"])

    def test_confirmation_rejects_mismatched_operations_and_manual_divisors(self) -> None:
        mismatched = self._contract()
        mismatched["base_benchmark"] = "parse_only/flowchart_medium"

        with self.assertRaisesRegex(compare_self.ContractViolation, "same public operation"):
            compare_self._validate_confirmation_operation_contract(
                [mismatched],
                base_logical_operations=1,
                head_logical_operations=1,
            )
        with self.assertRaisesRegex(compare_self.ContractViolation, "logical-operation"):
            compare_self._validate_confirmation_operation_contract(
                [self._contract()],
                base_logical_operations=1,
                head_logical_operations=100,
            )
        with self.assertRaisesRegex(compare_self.ContractViolation, "both be one"):
            compare_self._validate_confirmation_operation_contract(
                [self._contract()],
                base_logical_operations=100,
                head_logical_operations=100,
            )

    def test_confirmation_uses_lane_history_and_corpus_owned_divisor(self) -> None:
        corpus = load_corpus(CORPUS_PATH)
        lane = resolve_lane_group(corpus, "compatibility_json_parse")
        historical = self._contract()
        historical["base_benchmark"] = "parse_known_type/flowchart_medium"
        historical["head_benchmark"] = (
            "compatibility_json_parse/flowchart_medium"
        )

        compare_self._validate_confirmation_operation_contract(
            [historical],
            base_logical_operations=1,
            head_logical_operations=1,
            base_lane=lane,
            head_lane=lane,
        )

        recipe = compare_self.RunnerRecipe(
            label="head",
            checkout=ROOT,
            package="merman",
            bench="pipeline",
            features=("svg",),
            default_features=True,
            toolchain=None,
            target_dir=ROOT / "target",
            locked=True,
            corpus=Path("tools/bench/corpus.json"),
        )
        normalized = compare_self._recipe_with_lane_divisor(
            recipe,
            lane=lane,
            explicit_divisor=None,
        )
        self.assertEqual(normalized.logical_operations, 1)
        with self.assertRaisesRegex(compare_self.ContractViolation, "conflicts"):
            compare_self._recipe_with_lane_divisor(
                recipe,
                lane=lane,
                explicit_divisor=100,
            )

    def test_v1_to_v2_fixture_loading_applies_the_registered_history_lane(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            base_recipe = self._write_comparison_checkout(
                root / "base",
                self._minimal_corpus(
                    schema_version=1,
                    default_group="parse_known_type",
                ),
            )
            head_recipe = self._write_comparison_checkout(
                root / "head",
                self._minimal_corpus(
                    schema_version=2,
                    default_group="compatibility_json_parse",
                ),
            )

            contracts, selection, lanes = compare_self._load_fixture_contracts(
                base_recipe=base_recipe,
                head_recipe=head_recipe,
                suite="full",
                common_group=None,
                base_group_override=None,
                head_group_override=None,
                filter_expr=None,
            )

        self.assertEqual(
            contracts[0]["base_benchmark"],
            "parse_known_type/flowchart_medium",
        )
        self.assertEqual(
            contracts[0]["head_benchmark"],
            "compatibility_json_parse/flowchart_medium",
        )
        self.assertIsNone(selection["lane_contracts"]["declared"]["base"])
        self.assertEqual(lanes[0].id, "compatibility-json-parse")
        self.assertEqual(lanes[1].id, "compatibility-json-parse")
        compare_self._validate_confirmation_operation_contract(
            contracts,
            base_logical_operations=1,
            head_logical_operations=1,
            base_lane=lanes[0],
            head_lane=lanes[1],
        )

    def test_loaded_v2_lanes_reject_semantic_mixing(self) -> None:
        cases = (
            ("process_lifecycle", "fresh-process"),
            ("engine_lifecycle", "cold-engine"),
            ("transport", "node-napi"),
            ("public_operation", "different-public-operation"),
        )
        for field, value in cases:
            with self.subTest(field=field), tempfile.TemporaryDirectory() as raw:
                root = Path(raw)
                base_corpus = self._minimal_corpus(
                    schema_version=2,
                    default_group="compatibility_json_parse",
                )
                head_corpus = copy.deepcopy(base_corpus)
                lane = next(
                    item
                    for item in head_corpus["lanes"]
                    if item["id"] == "compatibility-json-parse"
                )
                lane[field] = value
                base_recipe = self._write_comparison_checkout(
                    root / "base",
                    base_corpus,
                )
                head_recipe = self._write_comparison_checkout(
                    root / "head",
                    head_corpus,
                )
                contracts, _selection, lanes = compare_self._load_fixture_contracts(
                    base_recipe=base_recipe,
                    head_recipe=head_recipe,
                    suite="full",
                    common_group=None,
                    base_group_override=None,
                    head_group_override=None,
                    filter_expr=None,
                )

                with self.assertRaisesRegex(compare_self.ContractViolation, field):
                    compare_self._validate_confirmation_operation_contract(
                        contracts,
                        base_logical_operations=1,
                        head_logical_operations=1,
                        base_lane=lanes[0],
                        head_lane=lanes[1],
                    )

    def test_output_aliases_are_rejected_before_sampling(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            base = root / "base"
            head = root / "head"
            base.mkdir()
            head.mkdir()
            output = root / "evidence"

            with mock.patch.object(compare_self, "_execute_comparison") as execute:
                result = compare_self.main(
                    [
                        "--base-dir",
                        str(base),
                        "--head-dir",
                        str(head),
                        "--out",
                        str(root / "nested" / ".." / "evidence"),
                        "--json-out",
                        str(output),
                    ]
                )

            self.assertEqual(result, 2)
            execute.assert_not_called()
            self.assertFalse(output.exists())

    def test_output_symlink_aliases_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            target = root / "evidence"
            target.write_text("existing", encoding="utf-8")
            alias = root / "alias"
            try:
                alias.symlink_to(target)
            except OSError as error:
                self.skipTest(f"symlinks are unavailable: {error}")

            with self.assertRaisesRegex(compare_self.ContractViolation, "distinct"):
                compare_self._resolve_output_paths(
                    head_dir=root,
                    markdown_value=str(alias),
                    json_value=str(target),
                )

    def test_output_hardlink_aliases_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            target = root / "evidence.json"
            target.write_text("existing", encoding="utf-8")
            alias = root / "evidence.md"
            try:
                os.link(target, alias)
            except OSError as error:
                self.skipTest(f"hard links are unavailable: {error}")

            with self.assertRaisesRegex(compare_self.ContractViolation, "distinct"):
                compare_self._resolve_output_paths(
                    head_dir=root,
                    markdown_value=str(alias),
                    json_value=str(target),
                )

    def test_case_alias_detected_after_json_write_preserves_json(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            probe = root / "CaseProbe"
            probe.write_text("probe", encoding="utf-8")
            case_insensitive = (root / "caseprobe").exists()
            probe.unlink()
            if not case_insensitive:
                self.skipTest("filesystem is case-sensitive")

            base = root / "base"
            head = root / "head"
            base.mkdir()
            head.mkdir()
            markdown = root / "Evidence"
            structured = root / "evidence"
            with mock.patch.object(compare_self, "_execute_comparison"):
                result = compare_self.main(
                    [
                        "--base-dir",
                        str(base),
                        "--head-dir",
                        str(head),
                        "--out",
                        str(markdown),
                        "--json-out",
                        str(structured),
                    ]
                )

            report = json.loads(structured.read_text(encoding="utf-8"))
            self.assertEqual(result, 2)
            self.assertEqual(report["summary"]["outcome"], "contract_failure")
            self.assertFalse(structured.read_text(encoding="utf-8").startswith("#"))


class CompareSelfRecipeContractsTest(unittest.TestCase):
    def _recipe(
        self,
        *,
        label: str,
        checkout: Path,
        package: str,
        bench: str,
        features: tuple[str, ...],
        default_features: bool,
        toolchain: str | None,
        target_dir: Path,
        target: str | None = None,
        locked: bool = True,
        corpus: Path,
    ):
        return compare_self.RunnerRecipe(
            label=label,
            checkout=checkout,
            package=package,
            bench=bench,
            features=features,
            default_features=default_features,
            toolchain=toolchain,
            target_dir=target_dir,
            target=target,
            locked=locked,
            corpus=corpus,
        )

    @staticmethod
    def _minimal_reusable_discovery() -> dict[str, object]:
        runner = {
            "recipe": {},
            "git": {},
            "manifest": {},
            "workspace_manifest": {},
            "lockfile": {},
            "corpus": {},
            "bench_source": {},
            "toolchain": {},
            "build_environment": {},
            "shared_target_profile_reset": {},
            "prebuild_command": [],
            "prebuild_stderr_tail": "",
            "source_executable": {},
            "frozen_executable": {},
            "executable": {},
            "discovery_command": [],
            "discovery": {},
            "post_sampling_verification": {"status": "verified"},
            "shared_target_freeze": {
                "enabled": True,
                "context": "reuse-test",
                "target_dir": "/tmp/target",
            },
        }
        return {
            "schema_version": 2,
            "harness": {
                "schema": "compare-self-v2",
                "path": "/tmp/compare_self.py",
                "bytes": 100,
                "sha256": "a" * 64,
            },
            "method": {
                "evidence_mode": "confirmation",
                "evidence_quality": "discovery_only",
                "discovery_only": True,
                "shared_target_freeze": {
                    "enabled": True,
                    "context": "reuse-test",
                    "target_dir": "/tmp/target",
                    "build_order": ["base", "head"],
                    "cargo_build_jobs": "1",
                    "profile_reset": "cargo-clean-bench-profile-before-each-side",
                },
            },
            "summary": {
                "exit_code": 0,
                "outcome": "diagnostic_advisory",
                "contract_failures": 0,
                "comparable": 1,
            },
            "contract_errors": [],
            "calibration": None,
            "raw_rounds": [],
            "fixtures": [
                {
                    "base_benchmark": "end_to_end/flowchart_medium",
                    "head_benchmark": "end_to_end/flowchart_medium",
                    "coverage_status": "comparable",
                    "post_sampling_verification": {"status": "verified"},
                }
            ],
            "rows": [
                {
                    "base_benchmark": "end_to_end/flowchart_medium",
                    "head_benchmark": "end_to_end/flowchart_medium",
                    "outcome": "diagnostic_advisory",
                }
            ],
            "runners": {
                "base": {
                    **copy.deepcopy(runner),
                    "shared_target_freeze": {
                        **runner["shared_target_freeze"],
                        "build_sequence": 1,
                    },
                },
                "head": {
                    **copy.deepcopy(runner),
                    "shared_target_freeze": {
                        **runner["shared_target_freeze"],
                        "build_sequence": 2,
                    },
                },
            },
        }

    def test_reusable_discovery_loader_rejects_digest_drift_and_invalid_json(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            duplicate = root / "duplicate.json"
            duplicate.write_text('{"schema_version": 2, "schema_version": 2}\n')
            nonfinite = root / "nonfinite.json"
            nonfinite.write_text('{"value": NaN}\n')

            with self.assertRaisesRegex(compare_self.ContractViolation, "duplicate"):
                compare_self._load_reusable_discovery_report(
                    duplicate, expected_sha256="0" * 64
                )
            with self.assertRaisesRegex(compare_self.ContractViolation, "non-finite"):
                compare_self._load_reusable_discovery_report(
                    nonfinite, expected_sha256="0" * 64
                )

            valid = root / "valid.json"
            valid.write_text("{}\n", encoding="utf-8")
            with self.assertRaisesRegex(compare_self.ContractViolation, "digest differs"):
                compare_self._load_reusable_discovery_report(
                    valid, expected_sha256="0" * 64
                )

            crlf = root / "crlf.json"
            crlf_bytes = b"{}\r\n"
            crlf.write_bytes(crlf_bytes)
            _value, description = compare_self._load_reusable_discovery_report(
                crlf,
                expected_sha256=compare_self.hashlib.sha256(crlf_bytes).hexdigest(),
            )
            self.assertEqual(description["bytes"], len(crlf_bytes))
            self.assertEqual(
                description["sha256"],
                compare_self.hashlib.sha256(crlf_bytes).hexdigest(),
            )
            method = {
                "discovery_reuse": {
                    "enabled": True,
                    "source_report": description,
                }
            }
            self.assertEqual(
                compare_self._discovery_reuse_verification_errors(method), []
            )
            crlf.write_bytes(b"{}\n")
            self.assertIn(
                "digest changed",
                " ".join(compare_self._discovery_reuse_verification_errors(method)),
            )

    def test_reusable_discovery_requires_successful_post_verified_frozen_evidence(
        self,
    ) -> None:
        valid = self._minimal_reusable_discovery()
        compare_self._validate_reusable_discovery_report(valid)

        sampled = copy.deepcopy(valid)
        sampled["raw_rounds"] = [{"pair": 1}]
        with self.assertRaisesRegex(compare_self.ContractViolation, "sampling observations"):
            compare_self._validate_reusable_discovery_report(sampled)

        unverified = copy.deepcopy(valid)
        unverified["runners"]["head"]["post_sampling_verification"]["status"] = "failed"
        with self.assertRaisesRegex(compare_self.ContractViolation, "post-verified"):
            compare_self._validate_reusable_discovery_report(unverified)

        wrong_order = copy.deepcopy(valid)
        wrong_order["runners"]["base"]["shared_target_freeze"]["build_sequence"] = 2
        with self.assertRaisesRegex(compare_self.ContractViolation, "build sequence"):
            compare_self._validate_reusable_discovery_report(wrong_order)

    def test_prepare_reused_runner_revalidates_every_frozen_input_without_cargo_build(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            checkout = root / "checkout"
            checkout.mkdir()
            manifest = checkout / "Cargo.toml"
            manifest.write_text(
                '[package]\nname = "merman"\nversion = "0.0.0"\n',
                encoding="utf-8",
            )
            lockfile = checkout / "Cargo.lock"
            lockfile.write_text("# lock\n", encoding="utf-8")
            corpus = checkout / "tools" / "bench" / "corpus.json"
            corpus.parent.mkdir(parents=True)
            corpus.write_text("{}\n", encoding="utf-8")
            bench_source = checkout / "benches" / "pipeline.rs"
            bench_source.parent.mkdir()
            bench_source.write_text("fn main() {}\n", encoding="utf-8")
            target_dir = (root / "target").resolve()
            executable_bytes = b"frozen executable"
            executable_sha256 = compare_self.hashlib.sha256(executable_bytes).hexdigest()
            executable = (
                target_dir
                / "perf-frozen"
                / "reuse-test"
                / ("base-" + "a" * 40 + f"-{executable_sha256}")
                / "pipeline-deadbeef"
            )
            executable.parent.mkdir(parents=True)
            executable.write_bytes(executable_bytes)
            executable.chmod(0o555)
            executable = executable.resolve()
            recipe = self._recipe(
                label="base",
                checkout=checkout,
                package="merman",
                bench="pipeline",
                features=("svg",),
                default_features=False,
                toolchain="1.95.0",
                target_dir=target_dir,
                corpus=Path("tools/bench/corpus.json"),
            )
            git = {
                "revision": "a" * 40,
                "tree": "b" * 40,
                "dirty": False,
                "dirty_disposition": "clean",
                "dirty_entries": [],
                "dirty_entries_truncated": False,
            }
            files = {
                "manifest": compare_self._describe_required_file(manifest),
                "workspace_manifest": compare_self._describe_required_file(manifest),
                "lockfile": compare_self._describe_required_file(lockfile),
                "corpus": compare_self._describe_required_file(corpus),
                "bench_source": compare_self._describe_required_file(bench_source),
            }
            executable_description = compare_self._describe_required_file(executable)
            frozen_description = {
                **executable_description,
                "executable": True,
                "mode": "0555",
            }
            discovery_stdout = "end_to_end/flowchart_medium: benchmark\n"
            combined = discovery_stdout + "\n"
            discovery = {
                "bench_count": 1,
                "benches": ["end_to_end/flowchart_medium"],
                "skipped": {},
                "output_sha256": compare_self.hashlib.sha256(
                    combined.encode("utf-8")
                ).hexdigest(),
            }
            toolchain = {
                "requested": "1.95.0",
                "rustc_verbose": "rustc test",
                "cargo_verbose": "cargo test",
            }
            source_executable = {
                **executable_description,
                "path": str(target_dir / "debug" / "deps" / executable.name),
                "executable": True,
            }
            build_environment = {
                "RUSTFLAGS": None,
                "CARGO_ENCODED_RUSTFLAGS": None,
                "CARGO_BUILD_JOBS": "1",
                "CARGO_PROFILE_BENCH_LTO": None,
                "CARGO_PROFILE_BENCH_CODEGEN_UNITS": None,
                "CARGO_PROFILE_BENCH_OPT_LEVEL": None,
            }
            origin = {
                "recipe": compare_self._recipe_report(recipe),
                "git": git,
                **files,
                "toolchain": toolchain,
                "build_environment": build_environment,
                "shared_target_profile_reset": {
                    "strategy": "cargo-clean-bench-profile-before-each-side",
                    "command": compare_self.cargo_clean_bench_profile_command(recipe),
                    "stdout_tail": "",
                    "stderr_tail": "Removed bench profile",
                },
                "prebuild_command": compare_self.cargo_prebuild_command(recipe),
                "prebuild_stderr_tail": "",
                "source_executable": source_executable,
                "frozen_executable": frozen_description,
                "executable": {
                    **executable_description,
                    "executable": True,
                    "role": "frozen",
                },
                "shared_target_freeze": {
                    "enabled": True,
                    "context": "reuse-test",
                    "target_dir": str(target_dir),
                    "build_sequence": 1,
                    "commit": git["revision"],
                    "tree": git["tree"],
                    "source_executable": source_executable,
                    "frozen_executable": frozen_description,
                },
                "discovery_command": compare_self.criterion_list_command(executable),
                "discovery": discovery,
                "post_sampling_verification": {
                    "status": "verified",
                    "git": git,
                    "files": {key: value["sha256"] for key, value in files.items()},
                    "executable_sha256": executable_description["sha256"],
                },
            }
            listed = mock.Mock(returncode=0, stdout=discovery_stdout, stderr="")

            with (
                mock.patch.object(compare_self, "_git_provenance", return_value=git),
                mock.patch.object(
                    compare_self, "_toolchain_version", return_value="rustc test"
                ),
                mock.patch.object(
                    compare_self, "_cargo_version", return_value="cargo test"
                ),
                mock.patch.object(compare_self, "_run_process", return_value=listed) as run,
            ):
                runner, provenance, errors = compare_self._prepare_reused_runner(
                    recipe,
                    origin=origin,
                    source_report={"path": "/tmp/discovery.json", "sha256": "c" * 64},
                    timeout_seconds=1,
                )

            self.assertFalse(errors)
            self.assertIsNotNone(runner)
            assert runner is not None
            self.assertTrue(runner.frozen)
            self.assertEqual(runner.executable, executable.resolve())
            self.assertEqual(provenance["discovery_reuse"]["status"], "verified")
            self.assertEqual(run.call_count, 1)
            self.assertEqual(run.call_args.args[0][0], str(executable))
            self.assertNotIn("cargo", run.call_args.args[0])

            swapped_executable = (
                target_dir
                / "perf-frozen"
                / "reuse-test"
                / ("head-" + "a" * 40 + f"-{executable_sha256}")
                / executable.name
            )
            swapped_executable.parent.mkdir(parents=True)
            swapped_executable.write_bytes(executable_bytes)
            swapped_executable.chmod(0o555)
            swapped = copy.deepcopy(origin)
            for description in (
                swapped["executable"],
                swapped["frozen_executable"],
                swapped["shared_target_freeze"]["frozen_executable"],
            ):
                description["path"] = str(swapped_executable)
            swapped["discovery_command"][0] = str(swapped_executable)

            with (
                mock.patch.object(compare_self, "_git_provenance", return_value=git),
                mock.patch.object(
                    compare_self, "_toolchain_version", return_value="rustc test"
                ),
                mock.patch.object(
                    compare_self, "_cargo_version", return_value="cargo test"
                ),
                mock.patch.object(compare_self, "_run_process") as swapped_run,
            ):
                swapped_runner, _provenance, swapped_errors = (
                    compare_self._prepare_reused_runner(
                        recipe,
                        origin=swapped,
                        source_report={
                            "path": "/tmp/discovery.json",
                            "sha256": "c" * 64,
                        },
                        timeout_seconds=1,
                    )
                )

            self.assertIsNone(swapped_runner)
            self.assertIn("destination identity differs", swapped_errors[0])
            swapped_run.assert_not_called()

    def test_runner_recipes_build_each_side_with_its_own_cargo_contract(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            base_checkout = root / "base"
            head_checkout = root / "head"
            base_checkout.mkdir()
            head_checkout.mkdir()
            (base_checkout / "Cargo.lock").write_text("# base lock\n", encoding="utf-8")
            (head_checkout / "Cargo.lock").write_text("# head lock\n", encoding="utf-8")

            base = self._recipe(
                label="base",
                checkout=base_checkout,
                package="merman-alpha",
                bench="render",
                features=("render",),
                default_features=False,
                toolchain="1.92.0",
                target_dir=root / "targets" / "base",
                corpus=Path("tools/bench/corpus-alpha.json"),
            )
            head = self._recipe(
                label="head",
                checkout=head_checkout,
                package="merman",
                bench="pipeline",
                features=("svg", "raster"),
                default_features=True,
                toolchain=None,
                target_dir=root / "targets" / "head",
                corpus=Path("tools/bench/corpus.json"),
            )

            base_command = compare_self.cargo_prebuild_command(base)
            head_command = compare_self.cargo_prebuild_command(head)

        self.assertEqual(base_command[:3], ["cargo", "+1.92.0", "bench"])
        self.assertEqual(head_command[:2], ["cargo", "bench"])
        self.assertIn("merman-alpha", base_command)
        self.assertIn("render", base_command)
        self.assertIn("merman", head_command)
        self.assertIn("pipeline", head_command)
        self.assertEqual(base_command[base_command.index("--features") + 1], "render")
        self.assertEqual(
            head_command[head_command.index("--features") + 1],
            "svg,raster",
        )
        self.assertIn("--no-default-features", base_command)
        self.assertNotIn("--no-default-features", head_command)
        self.assertIn("--locked", base_command)
        self.assertIn("--locked", head_command)
        self.assertIn("--no-run", base_command)
        self.assertIn("--no-run", head_command)
        self.assertIn("--message-format=json-render-diagnostics", base_command)
        self.assertEqual(base.corpus, Path("tools/bench/corpus-alpha.json"))
        self.assertEqual(head.corpus, Path("tools/bench/corpus.json"))
        self.assertEqual(
            base_command[base_command.index("--target-dir") + 1],
            str(root / "targets" / "base"),
        )
        self.assertEqual(
            head_command[head_command.index("--target-dir") + 1],
            str(root / "targets" / "head"),
        )

    def test_prebuild_refuses_unlocked_or_missing_lockfile_recipes(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            checkout = root / "checkout"
            checkout.mkdir()
            unlocked = self._recipe(
                label="head",
                checkout=checkout,
                package="merman",
                bench="pipeline",
                features=("svg",),
                default_features=False,
                toolchain=None,
                target_dir=root / "target",
                locked=False,
                corpus=Path("tools/bench/corpus.json"),
            )
            locked_without_file = self._recipe(
                label="head",
                checkout=checkout,
                package="merman",
                bench="pipeline",
                features=("svg",),
                default_features=False,
                toolchain=None,
                target_dir=root / "target",
                locked=True,
                corpus=Path("tools/bench/corpus.json"),
            )

            with self.assertRaisesRegex(ValueError, "locked"):
                compare_self.cargo_prebuild_command(unlocked)
            with self.assertRaisesRegex(FileNotFoundError, "Cargo.lock"):
                compare_self.cargo_prebuild_command(locked_without_file)

    def test_shared_target_clean_resets_only_the_selected_bench_profile(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            checkout = root / "checkout"
            checkout.mkdir()
            (checkout / "Cargo.lock").write_text("# lock\n", encoding="utf-8")
            recipe = self._recipe(
                label="head",
                checkout=checkout,
                package="merman",
                bench="pipeline",
                features=("svg",),
                default_features=False,
                toolchain="1.95.0",
                target_dir=root / "target",
                target="aarch64-apple-darwin",
                corpus=Path("tools/bench/corpus.json"),
            )

            command = compare_self.cargo_clean_bench_profile_command(recipe)

            self.assertEqual(
                command,
                [
                    "cargo",
                    "+1.95.0",
                    "clean",
                    "--locked",
                    "--profile",
                    "bench",
                    "--target-dir",
                    str(root / "target"),
                    "--target",
                    "aarch64-apple-darwin",
                ],
            )
            self.assertNotIn("--workspace", command)
            self.assertNotIn("--release", command)

    def test_shared_target_requires_explicit_freeze_mode(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            shared_target = root / "target"
            base = self._recipe(
                label="base",
                checkout=root / "base",
                package="merman",
                bench="pipeline",
                features=("svg",),
                default_features=False,
                toolchain=None,
                target_dir=shared_target,
                corpus=Path("tools/bench/corpus.json"),
            )
            head = self._recipe(
                label="head",
                checkout=root / "head",
                package="merman",
                bench="pipeline",
                features=("svg",),
                default_features=False,
                toolchain=None,
                target_dir=shared_target,
                corpus=Path("tools/bench/corpus.json"),
            )

            with self.assertRaisesRegex(compare_self.ContractViolation, "distinct Cargo target"):
                compare_self._shared_target_freeze_plan(base, head, enabled=False)

            plan = compare_self._shared_target_freeze_plan(
                base,
                head,
                enabled=True,
                context="u4-shared-target-test",
            )
            self.assertIsNotNone(plan)
            assert plan is not None
            self.assertEqual(plan.target_dir, shared_target.resolve())

            distinct_head = compare_self.RunnerRecipe(
                **{
                    **head.__dict__,
                    "target_dir": root / "other-target",
                }
            )
            with self.assertRaisesRegex(compare_self.ContractViolation, "same Cargo target"):
                compare_self._shared_target_freeze_plan(
                    base,
                    distinct_head,
                    enabled=True,
                    context="u4-shared-target-test",
                )

    def test_shared_target_freeze_survives_cargo_artifact_overwrite_and_rejects_collision(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            target_dir = root / "target"
            artifact = target_dir / "debug" / "deps" / "pipeline-deadbeef"
            artifact.parent.mkdir(parents=True)
            artifact.write_bytes(b"base executable")
            artifact.chmod(0o755)
            base = self._recipe(
                label="base",
                checkout=root / "base",
                package="merman",
                bench="pipeline",
                features=("svg",),
                default_features=False,
                toolchain=None,
                target_dir=target_dir,
                corpus=Path("tools/bench/corpus.json"),
            )
            head = compare_self.RunnerRecipe(**{**base.__dict__, "label": "head"})
            plan = compare_self.SharedTargetFreezePlan(
                target_dir=target_dir,
                context="overwrite-test",
            )
            base_git = {"revision": "a" * 40, "tree": "b" * 40}
            head_git = {"revision": "c" * 40, "tree": "d" * 40}

            frozen_base, base_freeze = compare_self._freeze_bench_executable(
                artifact,
                recipe=base,
                git=base_git,
                plan=plan,
                build_sequence=1,
            )
            artifact.write_bytes(b"head executable")
            frozen_head, head_freeze = compare_self._freeze_bench_executable(
                artifact,
                recipe=head,
                git=head_git,
                plan=plan,
                build_sequence=2,
            )

            self.assertEqual(frozen_base.read_bytes(), b"base executable")
            self.assertEqual(frozen_head.read_bytes(), b"head executable")
            self.assertNotEqual(frozen_base, frozen_head)
            self.assertEqual(stat.S_IMODE(frozen_base.stat().st_mode), 0o555)
            self.assertEqual(base_freeze["build_sequence"], 1)
            self.assertEqual(head_freeze["build_sequence"], 2)
            self.assertEqual(base_freeze["commit"], "a" * 40)
            self.assertEqual(base_freeze["tree"], "b" * 40)

            with self.assertRaisesRegex(compare_self.ContractViolation, "already exists"):
                compare_self._freeze_bench_executable(
                    artifact,
                    recipe=head,
                    git=head_git,
                    plan=plan,
                    build_sequence=2,
                )

    def test_prepare_shared_target_uses_only_the_frozen_executable(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            checkout = root / "checkout"
            checkout.mkdir()
            (checkout / "Cargo.toml").write_text(
                '[package]\nname = "merman"\nversion = "0.0.0"\n',
                encoding="utf-8",
            )
            (checkout / "Cargo.lock").write_text("# lock\n", encoding="utf-8")
            corpus = checkout / "tools" / "bench" / "corpus.json"
            corpus.parent.mkdir(parents=True)
            corpus.write_text("{}\n", encoding="utf-8")
            bench_source = checkout / "benches" / "pipeline.rs"
            bench_source.parent.mkdir()
            bench_source.write_text("fn main() {}\n", encoding="utf-8")
            target_dir = root / "target"
            cargo_artifact = target_dir / "debug" / "deps" / "pipeline-deadbeef"
            cargo_artifact.parent.mkdir(parents=True)
            cargo_artifact.write_bytes(b"bench executable")
            cargo_artifact.chmod(0o755)
            recipe = self._recipe(
                label="base",
                checkout=checkout,
                package="merman",
                bench="pipeline",
                features=("svg",),
                default_features=False,
                toolchain=None,
                target_dir=target_dir,
                corpus=Path("tools/bench/corpus.json"),
            )
            cargo_stdout = json.dumps(
                {
                    "reason": "compiler-artifact",
                    "target": {"kind": ["bench"], "name": "pipeline"},
                    "executable": str(cargo_artifact),
                }
            )
            cargo_result = mock.Mock(returncode=0, stdout=cargo_stdout, stderr="")
            clean_result = mock.Mock(
                returncode=0,
                stdout="",
                stderr="Removed 42 files, 12.3MiB total",
            )
            discovery_result = mock.Mock(
                returncode=0,
                stdout="end_to_end/flowchart_medium: benchmark\n",
                stderr="",
            )
            git = {
                "revision": "a" * 40,
                "tree": "b" * 40,
                "dirty": False,
                "dirty_disposition": "clean",
                "dirty_entries": [],
                "dirty_entries_truncated": False,
            }
            plan = compare_self.SharedTargetFreezePlan(
                target_dir=target_dir,
                context="prepare-test",
            )

            with (
                mock.patch.object(compare_self, "_git_provenance", return_value=git),
                mock.patch.object(compare_self, "_toolchain_version", return_value="rustc"),
                mock.patch.object(compare_self, "_cargo_version", return_value="cargo"),
                mock.patch.object(
                    compare_self,
                    "_run_process",
                    side_effect=[clean_result, cargo_result, discovery_result],
                ) as run_process,
            ):
                runner, provenance, errors = compare_self._prepare_runner(
                    recipe,
                    allow_dirty=False,
                    timeout_seconds=1,
                    freeze_plan=plan,
                    build_sequence=1,
                )

            self.assertFalse(errors)
            self.assertIsNotNone(runner)
            assert runner is not None
            self.assertTrue(runner.frozen)
            self.assertTrue(
                runner.executable.is_relative_to(
                    (target_dir / "perf-frozen" / "prepare-test").resolve()
                )
            )
            self.assertEqual(provenance["build_environment"]["CARGO_BUILD_JOBS"], "1")
            self.assertEqual(provenance["shared_target_freeze"]["build_sequence"], 1)
            self.assertEqual(
                provenance["shared_target_profile_reset"]["strategy"],
                "cargo-clean-bench-profile-before-each-side",
            )
            self.assertIn(
                "Removed 42 files",
                provenance["shared_target_profile_reset"]["stderr_tail"],
            )
            self.assertEqual(
                Path(provenance["source_executable"]["path"]).resolve(),
                cargo_artifact.resolve(),
            )
            self.assertEqual(provenance["discovery_command"][0], str(runner.executable))
            self.assertEqual(
                compare_self.criterion_command(
                    executable=runner.executable,
                    exact_bench="end_to_end/flowchart_medium",
                    sample_size=10,
                    warm_up_seconds=1,
                    measurement_seconds=1,
                )[0],
                str(runner.executable),
            )
            self.assertEqual(
                run_process.call_args_list[0].args[0][1:4],
                ["clean", "--locked", "--profile"],
            )
            self.assertEqual(run_process.call_args_list[0].kwargs["env"]["CARGO_BUILD_JOBS"], "1")
            self.assertEqual(run_process.call_args_list[1].kwargs["env"]["CARGO_BUILD_JOBS"], "1")

    def test_shared_target_freeze_rejects_source_digest_drift_during_copy(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            target_dir = root / "target"
            artifact = target_dir / "debug" / "deps" / "pipeline-deadbeef"
            artifact.parent.mkdir(parents=True)
            artifact.write_bytes(b"original executable")
            artifact.chmod(0o755)
            recipe = self._recipe(
                label="base",
                checkout=root / "base",
                package="merman",
                bench="pipeline",
                features=("svg",),
                default_features=False,
                toolchain=None,
                target_dir=target_dir,
                corpus=Path("tools/bench/corpus.json"),
            )
            plan = compare_self.SharedTargetFreezePlan(
                target_dir=target_dir,
                context="source-drift-test",
            )
            copyfileobj = compare_self.shutil.copyfileobj

            def copy_then_mutate(source, destination, *, length):
                copyfileobj(source, destination, length=length)
                artifact.write_bytes(b"mutated executable")

            with (
                mock.patch.object(
                    compare_self.shutil,
                    "copyfileobj",
                    side_effect=copy_then_mutate,
                ),
                self.assertRaisesRegex(compare_self.ContractViolation, "changed while freezing"),
            ):
                compare_self._freeze_bench_executable(
                    artifact,
                    recipe=recipe,
                    git={"revision": "a" * 40, "tree": "b" * 40},
                    plan=plan,
                    build_sequence=1,
                )

            frozen_files = list((target_dir / "perf-frozen").rglob("pipeline-deadbeef"))
            self.assertEqual(frozen_files, [])

    def test_frozen_digest_drift_fails_before_round_sampling(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            executable = root / "pipeline"
            executable.write_bytes(b"frozen")
            executable.chmod(0o555)
            recipe = self._recipe(
                label="base",
                checkout=root,
                package="merman",
                bench="pipeline",
                features=("svg",),
                default_features=False,
                toolchain=None,
                target_dir=root / "target",
                corpus=Path("tools/bench/corpus.json"),
            )
            runner = compare_self.PreparedRunner(
                recipe=recipe,
                executable=executable,
                executable_sha256=compare_self._path_sha256(executable),
                benches={"end_to_end/flowchart_medium"},
                skipped={},
                provenance={},
                env={},
                frozen=True,
            )
            executable.chmod(0o755)
            executable.write_bytes(b"drifted")

            with mock.patch.object(compare_self, "_measure_once") as measure:
                schedule = compare_self._run_aa_schedule(
                    runner,
                    contracts=[
                        {
                            "name": "flowchart_medium",
                            "base_benchmark": "end_to_end/flowchart_medium",
                        }
                    ],
                    pair_count=1,
                    sample_size=10,
                    warm_up_seconds=1,
                    measurement_seconds=1,
                    timeout_seconds=1,
                )

            measure.assert_not_called()
            self.assertIn("digest changed", schedule["errors"]["flowchart_medium"])
            self.assertIn("error", schedule["rounds"][0]["executable_verification"])

    def test_different_trees_require_distinct_frozen_executables(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            executable = root / "pipeline"
            executable.write_bytes(b"same executable")
            executable.chmod(0o555)
            recipe = self._recipe(
                label="base",
                checkout=root,
                package="merman",
                bench="pipeline",
                features=("svg",),
                default_features=False,
                toolchain=None,
                target_dir=root / "target",
                corpus=Path("tools/bench/corpus.json"),
            )
            digest = compare_self._path_sha256(executable)
            base = compare_self.PreparedRunner(
                recipe=recipe,
                executable=executable,
                executable_sha256=digest,
                benches=set(),
                skipped={},
                provenance={"git": {"tree": "a" * 40}},
                env={},
                frozen=True,
            )
            head = compare_self.PreparedRunner(
                recipe=compare_self.RunnerRecipe(
                    **{**recipe.__dict__, "label": "head"}
                ),
                executable=executable,
                executable_sha256=digest,
                benches=set(),
                skipped={},
                provenance={"git": {"tree": "b" * 40}},
                env={},
                frozen=True,
            )

            error = compare_self._binary_independence_error(base, head)

            self.assertIsNotNone(error)
            self.assertIn("byte-identical", error)
            head.provenance["git"]["tree"] = "a" * 40
            self.assertIsNone(compare_self._binary_independence_error(base, head))

    def test_prepare_checks_git_before_creating_an_in_checkout_target_dir(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            checkout = Path(temp_dir) / "checkout"
            checkout.mkdir()
            target_dir = checkout / "unignored-target"
            recipe = self._recipe(
                label="head",
                checkout=checkout,
                package="merman",
                bench="pipeline",
                features=("svg",),
                default_features=False,
                toolchain=None,
                target_dir=target_dir,
                corpus=Path("tools/bench/corpus.json"),
            )

            def capture_clean_tree(*_args, **_kwargs):
                self.assertFalse(target_dir.exists())
                return {
                    "revision": "a" * 40,
                    "dirty": False,
                    "dirty_disposition": "clean",
                    "dirty_entries": [],
                    "dirty_entries_truncated": False,
                }

            with mock.patch.object(
                compare_self,
                "_git_provenance",
                side_effect=capture_clean_tree,
            ):
                prepared, _provenance, errors = compare_self._prepare_runner(
                    recipe,
                    allow_dirty=False,
                    timeout_seconds=1,
                )

            self.assertIsNone(prepared)
            self.assertTrue(errors)
            self.assertTrue(target_dir.exists())

    def test_describe_required_file_reuses_a_precomputed_digest(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            path = Path(temp_dir) / "artifact"
            path.write_bytes(b"artifact")
            digest = "a" * 64
            with mock.patch.object(compare_self, "_path_sha256") as path_sha256:
                description = compare_self._describe_required_file(
                    path,
                    sha256=digest,
                )

        path_sha256.assert_not_called()
        self.assertEqual(description["sha256"], digest)

    def test_parses_the_unique_matching_bench_executable_from_cargo_json(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            checkout = root / "checkout"
            checkout.mkdir()
            (checkout / "Cargo.lock").write_text("# lock\n", encoding="utf-8")
            recipe = self._recipe(
                label="head",
                checkout=checkout,
                package="merman",
                bench="pipeline",
                features=("svg",),
                default_features=False,
                toolchain=None,
                target_dir=root / "target",
                corpus=Path("tools/bench/corpus.json"),
            )
            executable = root / "target" / "release" / "deps" / "pipeline-deadbeef"
            unrelated = {
                "reason": "compiler-artifact",
                "package_id": "path+file:///repo#merman@0.9.0",
                "target": {"kind": ["lib"], "name": "merman"},
                "executable": None,
            }
            matching = {
                "reason": "compiler-artifact",
                "package_id": "path+file:///repo#merman@0.9.0",
                "target": {"kind": ["bench"], "name": "pipeline"},
                "profile": {"test": True},
                "executable": str(executable),
            }
            cargo_stdout = "\n".join(
                ["Compiling merman", json.dumps(unrelated), json.dumps(matching)]
            )

            parsed = compare_self.parse_bench_executable(cargo_stdout, recipe=recipe)

            self.assertEqual(parsed, executable)
            duplicate_stdout = "\n".join(
                [cargo_stdout, json.dumps({**matching, "executable": str(executable) + "-2"})]
            )
            with self.assertRaisesRegex(ValueError, "unique|multiple"):
                compare_self.parse_bench_executable(duplicate_stdout, recipe=recipe)
            with self.assertRaisesRegex(ValueError, "unique|missing"):
                compare_self.parse_bench_executable(json.dumps(unrelated), recipe=recipe)

    def test_direct_criterion_command_uses_hidden_benchmark_mode_and_exact_filter(self) -> None:
        command = compare_self.criterion_command(
            executable=Path("target/release/deps/pipeline-deadbeef"),
            exact_bench="end_to_end/flowchart_medium",
            sample_size=30,
            warm_up_seconds=2,
            measurement_seconds=3,
        )

        self.assertEqual(command[0], "target/release/deps/pipeline-deadbeef")
        self.assertIn("--bench", command)
        self.assertEqual(command[command.index("--color") + 1], "never")
        self.assertEqual(
            command[command.index("--exact") + 1],
            "end_to_end/flowchart_medium",
        )
        self.assertEqual(command[command.index("--sample-size") + 1], "30")
        self.assertEqual(command[command.index("--warm-up-time") + 1], "2")
        self.assertEqual(command[command.index("--measurement-time") + 1], "3")


class CompareSelfFixtureContractsTest(unittest.TestCase):
    def test_fixture_byte_comparison_classifies_identical_different_and_missing(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            base = root / "base.mmd"
            head = root / "head.mmd"
            base.write_bytes(b"flowchart LR\nA-->B\n")
            head.write_bytes(b"flowchart LR\nA-->B\n")

            identical = compare_self.fixture_byte_comparison(base, head)
            self.assertEqual(identical["status"], "identical")
            self.assertEqual(identical["base"]["sha256"], identical["head"]["sha256"])

            head.write_bytes(b"flowchart LR\nA-->C\n")
            different = compare_self.fixture_byte_comparison(base, head)
            self.assertEqual(different["status"], "different")
            self.assertNotEqual(different["base"]["sha256"], different["head"]["sha256"])

            head.unlink()
            missing_head = compare_self.fixture_byte_comparison(base, head)
            self.assertEqual(missing_head["status"], "missing")
            self.assertIsNotNone(missing_head["base"]["sha256"])
            self.assertIsNone(missing_head["head"]["sha256"])

            base.unlink()
            head.write_bytes(b"flowchart LR\nA-->C\n")
            missing_base = compare_self.fixture_byte_comparison(base, head)
            self.assertEqual(missing_base["status"], "missing")
            self.assertIsNone(missing_base["base"]["sha256"])
            self.assertIsNotNone(missing_base["head"]["sha256"])

            head.unlink()
            missing_both = compare_self.fixture_byte_comparison(base, head)
            self.assertEqual(missing_both["status"], "missing")
            self.assertIsNone(missing_both["base"]["sha256"])
            self.assertIsNone(missing_both["head"]["sha256"])


class CompareSelfStatisticsContractsTest(unittest.TestCase):
    @staticmethod
    def _bounds(
        *,
        relative: tuple[float, float, float],
        absolute: tuple[float, float, float],
    ) -> dict[str, dict[str, float]]:
        return {
            "log_ratio": dict(zip(("estimate", "lower", "upper"), relative)),
            "absolute_ns": dict(zip(("estimate", "lower", "upper"), absolute)),
        }

    def test_paired_bounds_use_canonical_head_over_base_direction_and_mirror_improvement(self) -> None:
        base_ns = [1_000_000.0, 2_000_000.0, 4_000_000.0, 8_000_000.0]
        head_ns = [1_200_000.0, 2_200_000.0, 4_200_000.0, 8_200_000.0]

        bounds = compare_self.paired_bounds(base_ns=base_ns, head_ns=head_ns)

        expected_log_ratio = sum(
            math.log(head / base) for base, head in zip(base_ns, head_ns)
        ) / len(base_ns)
        self.assertAlmostEqual(bounds["log_ratio"]["estimate"], expected_log_ratio)
        self.assertAlmostEqual(bounds["absolute_ns"]["estimate"], 200_000.0)
        self.assertGreater(bounds["log_ratio"]["estimate"], 0.0)
        self.assertGreater(bounds["absolute_ns"]["estimate"], 0.0)
        self.assertAlmostEqual(
            bounds["improvement_log_ratio"]["estimate"],
            -bounds["log_ratio"]["estimate"],
        )
        self.assertAlmostEqual(
            bounds["improvement_log_ratio"]["lower"],
            -bounds["log_ratio"]["upper"],
        )
        self.assertAlmostEqual(
            bounds["improvement_log_ratio"]["upper"],
            -bounds["log_ratio"]["lower"],
        )
        self.assertAlmostEqual(
            bounds["improvement_absolute_ns"]["lower"],
            -bounds["absolute_ns"]["upper"],
        )
        self.assertAlmostEqual(
            bounds["improvement_absolute_ns"]["upper"],
            -bounds["absolute_ns"]["lower"],
        )

    def test_confirmation_distinguishes_regression_non_regression_and_inconclusive(self) -> None:
        relative_threshold = math.log1p(0.10)
        cases = [
            (
                "confirmed_regression",
                self._bounds(
                    relative=(0.12, 0.11, 0.13),
                    absolute=(65_000.0, 55_000.0, 75_000.0),
                ),
            ),
            (
                "confirmed_non_regression",
                self._bounds(
                    relative=(0.08, 0.07, 0.09),
                    absolute=(65_000.0, 55_000.0, 75_000.0),
                ),
            ),
            (
                "inconclusive",
                self._bounds(
                    relative=(0.10, 0.08, 0.12),
                    absolute=(50_000.0, 40_000.0, 60_000.0),
                ),
            ),
        ]
        for expected, bounds in cases:
            with self.subTest(expected=expected):
                self.assertEqual(
                    compare_self.classify_confirmation(
                        bounds,
                        relative_threshold=relative_threshold,
                        absolute_threshold_ns=50_000.0,
                        direction="regression",
                        evidence_mode="confirmation",
                        pair_count=8,
                        required_pairs=8,
                    ),
                    expected,
                )

    def test_confirmation_supports_the_mirrored_improvement_test(self) -> None:
        bounds = self._bounds(
            relative=(-0.13, -0.15, -0.11),
            absolute=(-70_000.0, -80_000.0, -60_000.0),
        )

        self.assertEqual(
            compare_self.classify_confirmation(
                bounds,
                relative_threshold=math.log1p(0.10),
                absolute_threshold_ns=50_000.0,
                direction="improvement",
                evidence_mode="confirmation",
                pair_count=8,
                required_pairs=8,
            ),
            "confirmed_improvement",
        )

    def test_power_derived_pair_count_has_floor_even_rounding_and_cap_signal(self) -> None:
        floor = compare_self.required_pair_count(
            sigma=0.0,
            minimum_detectable_effect=0.10,
            max_pairs=32,
        )
        rounded = compare_self.required_pair_count(
            sigma=0.02,
            minimum_detectable_effect=0.01,
            max_pairs=32,
        )
        over_cap = compare_self.required_pair_count(
            sigma=0.02,
            minimum_detectable_effect=0.01,
            max_pairs=24,
        )

        self.assertEqual(floor.required_pairs, 8)
        self.assertEqual(floor.scheduled_pairs, 8)
        self.assertFalse(floor.exceeds_cap)
        self.assertEqual(rounded.required_pairs, 26)
        self.assertEqual(rounded.required_pairs % 2, 0)
        self.assertFalse(rounded.exceeds_cap)
        self.assertEqual(over_cap.required_pairs, 26)
        self.assertEqual(over_cap.scheduled_pairs, 24)
        self.assertTrue(over_cap.exceeds_cap)

    def test_suite_exit_code_uses_contract_regression_inconclusive_priority(self) -> None:
        cases = [
            (["diagnostic_advisory"], 0),
            (["confirmed_non_regression"], 0),
            (["inconclusive", "diagnostic_advisory"], 3),
            (["confirmed_regression", "inconclusive"], 1),
            (["contract_failure", "confirmed_regression", "inconclusive"], 2),
        ]
        for outcomes, expected in cases:
            with self.subTest(outcomes=outcomes):
                self.assertEqual(compare_self.suite_exit_code(outcomes), expected)

    def test_diagnostic_timing_is_advisory_even_when_bounds_look_regressive(self) -> None:
        bounds = self._bounds(
            relative=(0.15, 0.14, 0.16),
            absolute=(80_000.0, 70_000.0, 90_000.0),
        )

        outcome = compare_self.classify_confirmation(
            bounds,
            relative_threshold=math.log1p(0.10),
            absolute_threshold_ns=50_000.0,
            direction="regression",
            evidence_mode="diagnostic",
            pair_count=4,
            required_pairs=8,
        )

        self.assertEqual(outcome, "diagnostic_advisory")
        self.assertEqual(compare_self.suite_exit_code([outcome]), 0)


class RendererComparisonContractsTest(unittest.TestCase):
    def test_default_markdown_report_stays_outside_docs_tree(self) -> None:
        self.assertEqual(
            compare_mermaid_renderers.DEFAULT_MARKDOWN_OUT,
            "target/bench/renderer_comparison.md",
        )

    def test_formats_tiny_ratios_as_less_than_one_percent(self) -> None:
        self.assertEqual(compare_mermaid_renderers.fmt_ratio(0.0025), "<0.01x")
        self.assertEqual(compare_mermaid_renderers.fmt_ratio(0.025), "0.03x")

    def test_excludes_nonidentical_fixture_inputs_from_mmdr_ratios(self) -> None:
        runner = {
            "times_ns": {"end_to_end/example": 200.0},
            "errors": {},
            "missing": [],
            "skipped": {},
        }
        mmdr = {
            "times_ns": {"end_to_end/example": 100.0},
            "errors": {},
            "missing": [],
            "skipped": {},
        }
        mermaid_js = {
            "kind": "browser_warm",
            "times_ns": {},
            "errors": {},
            "missing": [],
            "skipped": {},
        }

        rows = compare_mermaid_renderers.build_rows(
            exact_benches=["end_to_end/example"],
            fixtures_by_name={},
            merman=runner,
            mmdr=mmdr,
            mermaid_js=mermaid_js,
            fixture_inputs={"example": {"status": "different"}},
        )

        self.assertIsNone(rows[0]["ratios"]["merman_over_mermaid_rs_renderer"])
        family_summary = compare_mermaid_renderers.build_family_summary(rows)[0]
        self.assertEqual(family_summary["measured"]["mermaid_rs_renderer"], 1)
        self.assertIsNone(
            family_summary["geomean_ratios"]["merman_over_mermaid_rs_renderer"]
        )

        comparable_rows = compare_mermaid_renderers.build_rows(
            exact_benches=["end_to_end/example"],
            fixtures_by_name={},
            merman=runner,
            mmdr=mmdr,
            mermaid_js=mermaid_js,
            fixture_inputs={"example": {"status": "identical"}},
        )
        self.assertEqual(
            comparable_rows[0]["ratios"]["merman_over_mermaid_rs_renderer"],
            2.0,
        )


class StageSpotcheckContractsTest(unittest.TestCase):
    def test_mmdr_command_enables_benchmark_feature(self) -> None:
        command = stage_spotcheck.mmdr_bench_cmd(
            sample_size=30,
            warm_up=2,
            measurement=3,
            exact="parse/requirement_medium",
            locked=True,
            toolchain=None,
        )

        self.assertIn("--features", command)
        feature_index = command.index("--features")
        self.assertEqual(command[feature_index + 1], "benchmark")
        self.assertIn("renderer", command)

    def test_rejects_nonidentical_fixture_inputs_before_benchmarking(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            mmdr_dir = root / "mmdr"
            merman_fixture = root / "crates" / "merman" / "benches" / "fixtures"
            mmdr_fixture = mmdr_dir / "benches" / "fixtures"
            merman_fixture.mkdir(parents=True)
            mmdr_fixture.mkdir(parents=True)
            (merman_fixture / "example.mmd").write_text("flowchart LR\nA-->B\n", encoding="utf-8")
            (mmdr_fixture / "example.mmd").write_text("flowchart LR\nA-->C\n", encoding="utf-8")

            with self.assertRaisesRegex(ValueError, r"example \(different\)"):
                stage_spotcheck.validate_fixture_inputs(root, mmdr_dir, ["example"])

    def test_fixture_comparison_records_identical_bytes(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            mmdr_dir = root / "mmdr"
            merman_fixture = root / "crates" / "merman" / "benches" / "fixtures"
            mmdr_fixture = mmdr_dir / "benches" / "fixtures"
            merman_fixture.mkdir(parents=True)
            mmdr_fixture.mkdir(parents=True)
            source = "mindmap\n  root((Merman))\n"
            (merman_fixture / "example.mmd").write_text(source, encoding="utf-8")
            (mmdr_fixture / "example.mmd").write_text(source, encoding="utf-8")

            comparisons = compare_mmdr_fixture_inputs(
                repo_root=root,
                mmdr_dir=mmdr_dir,
                fixture_names=["example"],
            )

        self.assertEqual(comparisons["example"]["status"], "identical")
        self.assertEqual(
            comparisons["example"]["merman"]["sha256"],
            comparisons["example"]["mermaid_rs_renderer"]["sha256"],
        )


class PerfCommentContractsTest(unittest.TestCase):
    def test_renders_warning_signal_rows(self) -> None:
        body = render_perf_comment.render_comment(
            {
                "schema_version": 1,
                "summary": {
                    "gate_status": "pass",
                    "comparable": 2,
                    "failures": 0,
                    "warnings": 1,
                    "improvements": 1,
                    "geomean_change_percent": 1.23,
                },
                "selection": {"suite": "canary"},
                "comparison": {
                    "base_label": "Latias94/merman@main",
                    "head_label": "Latias94/merman@perf-branch",
                },
                "method": {
                    "preset": "quick",
                    "warn_threshold_percent": 5.0,
                    "fail_threshold_percent": 10.0,
                },
                "rows": [
                    {
                        "benchmark": "end_to_end/flowchart_medium",
                        "base_ns": 100.0,
                        "head_ns": 106.2,
                        "change_percent": 6.2,
                        "status": "warn",
                    },
                    {
                        "benchmark": "end_to_end/class_medium",
                        "base_ns": 100.0,
                        "head_ns": 90.0,
                        "change_percent": -10.0,
                        "status": "improved",
                    },
                ],
            },
            run_url="https://example.test/run",
            artifact_name="perf-regression",
        )

        self.assertIn(render_perf_comment.MARKER, body)
        self.assertIn("Status: `passed with warnings`", body)
        self.assertIn("`Latias94/merman@main` -> `Latias94/merman@perf-branch`", body)
        self.assertIn("`end_to_end/flowchart_medium`", body)
        self.assertIn("+6.20%", body)
        self.assertIn("https://example.test/run", body)

    def test_renders_custom_marker_and_title(self) -> None:
        body = render_perf_comment.render_comment(
            {
                "schema_version": 1,
                "summary": {
                    "gate_status": "pass",
                    "comparable": 0,
                    "failures": 0,
                    "warnings": 0,
                    "improvements": 0,
                    "geomean_change_percent": 0.0,
                },
                "selection": {"suite": "frontmatter"},
                "comparison": {
                    "base_label": "base",
                    "head_label": "head",
                },
                "method": {
                    "preset": "quick",
                    "warn_threshold_percent": 5.0,
                    "fail_threshold_percent": 10.0,
                },
                "rows": [],
            },
            run_url="https://example.test/run",
            artifact_name="perf-frontmatter",
            marker="<!-- merman-perf-frontmatter -->",
            title="Merman Frontmatter Performance Regression",
        )

        self.assertIn("<!-- merman-perf-frontmatter -->", body)
        self.assertIn("## Merman Frontmatter Performance Regression", body)

    def test_renders_missing_report_fallback(self) -> None:
        body = render_perf_comment.render_comment(
            None,
            run_url="https://example.test/run",
            artifact_name="perf-regression",
        )

        self.assertIn("Status: `report unavailable`", body)
        self.assertIn("workflow logs", body)

    @staticmethod
    def _v2_report(outcome: str) -> dict[str, object]:
        exit_codes = {
            "diagnostic_advisory": 0,
            "confirmed_regression": 1,
            "inconclusive": 3,
            "contract_failure": 2,
        }
        return {
            "schema_version": 2,
            "summary": {
                "outcome": outcome,
                "exit_code": exit_codes[outcome],
                "comparable": 1,
            },
            "selection": {
                "suite": "canary",
                "group": "end_to_end",
                "groups": {"base": "end_to_end", "head": "end_to_end"},
            },
            "comparison": {
                "base_label": "base@abc123",
                "head_label": "head@def456",
            },
            "method": {
                "preset": "long",
                "evidence_mode": "diagnostic"
                if outcome == "diagnostic_advisory"
                else "confirmation",
                "relative_threshold_percent": 10.0,
                "absolute_threshold_ns": 50_000.0,
                "confidence_level": 0.95,
                "confidence_contract": {
                    "simultaneous_confidence_level": 0.95,
                    "component_confidence_level": 0.99375,
                    "family_size": 8,
                    "multiplicity_adjustment": "bonferroni",
                },
                "pair_count": 8,
                "required_pairs": 8,
            },
            "recipes": {
                "base": {"logical_operations": 1},
                "head": {"logical_operations": 1},
            },
            "rows": [
                {
                    "benchmark": "end_to_end/flowchart_medium",
                    "base_benchmark": "end_to_end/flowchart_medium",
                    "head_benchmark": "end_to_end/flowchart_medium",
                    "outcome": outcome,
                    "base_ns": 1_000_000.0,
                    "head_ns": 1_120_000.0,
                    "bounds": {
                        "relative_percent": {
                            "estimate": 12.0,
                            "lower": 10.5,
                            "upper": 13.5,
                        },
                        "absolute_ns": {
                            "estimate": 120_000.0,
                            "lower": 105_000.0,
                            "upper": 135_000.0,
                        },
                    },
                }
            ],
        }

    def test_schema_v2_renders_every_evidence_outcome_and_both_bounds(self) -> None:
        expected_statuses = {
            "diagnostic_advisory": "diagnostic advisory",
            "confirmed_regression": "confirmed regression",
            "inconclusive": "inconclusive",
            "contract_failure": "contract failure",
        }
        for outcome, status in expected_statuses.items():
            with self.subTest(outcome=outcome):
                body = render_perf_comment.render_comment(
                    self._v2_report(outcome),
                    run_url="https://example.test/run",
                    artifact_name="perf-regression",
                )
                lower_body = body.lower()

                self.assertIn(f"Status: `{status}`", body)
                self.assertIn("relative 99.375% bounds", lower_body)
                self.assertIn("absolute 99.375% bounds", lower_body)
                self.assertIn("+10.50%", body)
                self.assertIn("+13.50%", body)
                self.assertIn("105.00 us", body)
                self.assertIn("135.00 us", body)
                self.assertIn("Public operation groups: `end_to_end` -> `end_to_end`", body)
                self.assertIn("Logical operations per estimate: base `1`, head `1`", body)
                self.assertIn("via `bonferroni`, family `8` at component `99.375%`", body)

    def test_schema_v2_fails_closed_when_report_and_process_exit_disagree(self) -> None:
        report = self._v2_report("confirmed_regression")
        report["summary"]["exit_code"] = 0

        body = render_perf_comment.render_comment(
            report,
            run_url="https://example.test/run",
            artifact_name="perf-regression",
            process_exit_code=1,
        )

        self.assertIn("Status: `contract failure`", body)
        self.assertIn("exit", body.lower())
        self.assertNotIn("Status: `confirmed regression`", body)

    def test_schema_v2_process_exit_is_part_of_the_consumer_contract(self) -> None:
        body = render_perf_comment.render_comment(
            self._v2_report("inconclusive"),
            run_url="https://example.test/run",
            artifact_name="perf-regression",
            process_exit_code=0,
        )

        self.assertIn("Status: `contract failure`", body)
        self.assertIn("process exit", body.lower())

    def test_schema_v2_consumer_rederives_summary_from_rows(self) -> None:
        report = self._v2_report("confirmed_regression")
        report["summary"]["outcome"] = "confirmed_non_regression"
        report["summary"]["exit_code"] = 0

        body = render_perf_comment.render_comment(
            report,
            run_url="https://example.test/run",
            artifact_name="perf-regression",
            process_exit_code=0,
        )

        self.assertIn("Status: `contract failure`", body)
        self.assertIn("aggregate", body.lower())

    def test_comment_cli_writes_contract_failure_then_returns_nonzero(self) -> None:
        report = self._v2_report("confirmed_regression")
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            report_path = root / "report.json"
            output_path = root / "comment.md"
            report_path.write_text(json.dumps(report), encoding="utf-8")

            result = render_perf_comment.main(
                [
                    "--json",
                    str(report_path),
                    "--out",
                    str(output_path),
                    "--run-url",
                    "https://example.test/run",
                    "--process-exit-code",
                    "0",
                ]
            )
            body = output_path.read_text(encoding="utf-8")

        self.assertEqual(result, 2)
        self.assertIn("Status: `contract failure`", body)

    def test_comment_cli_accepts_signal_exit_and_still_writes_fallback(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            output_path = root / "comment.md"
            result = render_perf_comment.main(
                [
                    "--json",
                    str(root / "missing.json"),
                    "--out",
                    str(output_path),
                    "--run-url",
                    "https://example.test/run",
                    "--process-exit-code",
                    "137",
                ]
            )
            body = output_path.read_text(encoding="utf-8")

        self.assertEqual(result, 2)
        self.assertIn("report unavailable", body)

    def test_comment_cli_returns_zero_for_valid_nonzero_producer_outcome(self) -> None:
        report = self._v2_report("confirmed_regression")
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            report_path = root / "report.json"
            output_path = root / "comment.md"
            report_path.write_text(json.dumps(report), encoding="utf-8")

            result = render_perf_comment.main(
                [
                    "--json",
                    str(report_path),
                    "--out",
                    str(output_path),
                    "--run-url",
                    "https://example.test/run",
                    "--process-exit-code",
                    "1",
                ]
            )

        self.assertEqual(result, 0)

    def test_schema_v1_never_hides_failure_behind_warning_count(self) -> None:
        self.assertEqual(
            render_perf_comment.status_label(
                {"gate_status": "pass", "failures": 1, "warnings": 1}
            ),
            "failed",
        )

    def test_unknown_report_schema_is_explicitly_unsupported_and_never_passed(self) -> None:
        body = render_perf_comment.render_comment(
            {
                "schema_version": 999,
                "summary": {
                    "gate_status": "pass",
                    "outcome": "confirmed_non_regression",
                },
            },
            run_url="https://example.test/run",
            artifact_name="perf-regression",
        )

        self.assertIn("unsupported report schema", body.lower())
        self.assertNotIn("Status: `passed", body)
        self.assertNotIn("Status: `confirmed non-regression`", body)

    def test_schema_v1_remains_available_as_legacy_diagnostic_only(self) -> None:
        body = render_perf_comment.render_comment(
            {
                "schema_version": 1,
                "summary": {
                    "gate_status": "pass",
                    "comparable": 1,
                    "failures": 0,
                    "warnings": 0,
                    "improvements": 0,
                    "geomean_change_percent": 0.0,
                },
                "selection": {"suite": "canary"},
                "comparison": {"base_label": "base", "head_label": "head"},
                "method": {
                    "preset": "quick",
                    "warn_threshold_percent": 5.0,
                    "fail_threshold_percent": 10.0,
                },
                "rows": [],
            },
            run_url="https://example.test/run",
            artifact_name="perf-regression",
        )

        self.assertIn("legacy diagnostic", body.lower())
        self.assertIn("Status: `passed`", body)


class PerformanceWorkflowContractsTest(unittest.TestCase):
    def test_regression_workflow_exposes_decision_mode_and_absolute_threshold(self) -> None:
        workflow = (ROOT / ".github" / "workflows" / "performance.yml").read_text(
            encoding="utf-8"
        )

        self.assertIn("evidence_mode:", workflow)
        self.assertIn("absolute_threshold_ns:", workflow)
        self.assertIn("--evidence-mode", workflow)
        self.assertIn("--absolute-threshold-ns", workflow)

    def test_regression_workflow_captures_then_enforces_comparison_exit(self) -> None:
        workflow = (ROOT / ".github" / "workflows" / "performance.yml").read_text(
            encoding="utf-8"
        )

        self.assertRegex(
            workflow,
            r"(?s)set \+e\s+python3 tools/bench/compare_self\.py.*?comparison_exit=\$\?\s+set -e",
        )
        self.assertIn("comparison_exit", workflow)
        self.assertIn("--process-exit-code", workflow)
        self.assertIn("comment_render_exit", workflow)
        self.assertRegex(workflow, r"(?i)name: enforce .*performance.*result")
        self.assertRegex(workflow, r'exit "\$[A-Z_]*COMPARISON_EXIT"')
        self.assertRegex(workflow, r'if \[ "\$[A-Z_]*COMMENT_RENDER_EXIT" -ne 0 \]')


if __name__ == "__main__":
    sys.exit(unittest.main())
