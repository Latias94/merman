#!/usr/bin/env python3
"""Contract tests for the corpus-driven performance helper scripts."""

from __future__ import annotations

import copy
import io
import json
import math
import os
import sys
import tempfile
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from unittest import mock

import perf_runner
import compare_self
import compare_mermaid_renderers
import render_perf_comment
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
            locked=locked,
            corpus=corpus,
        )

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
