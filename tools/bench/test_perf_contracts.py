#!/usr/bin/env python3
"""Contract tests for the corpus-driven performance helper scripts."""

from __future__ import annotations

import copy
import io
import json
import math
import os
import stat
import subprocess
import sys
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from dataclasses import replace
from pathlib import Path
from types import SimpleNamespace
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
BINDING_REQUEST_CORPUS_PATH = ROOT / "tools" / "bench" / "binding_request_corpus.json"
PIPELINE_EXECUTABLE = (
    "pipeline-deadbeef.exe" if os.name == "nt" else "pipeline-deadbeef"
)


class CorpusContractsTest(unittest.TestCase):
    def test_cross_family_has_one_fixture_per_declared_family(self) -> None:
        corpus = load_corpus(CORPUS_PATH)
        fixtures = select_corpus_fixtures(corpus, "cross_family")

        self.assertEqual(len(fixtures), len({fixture.family for fixture in fixtures}))
        self.assertTrue(all((ROOT / fixture.source).is_file() for fixture in fixtures))

    def test_branch_start_controls_use_a_revision_stable_exact_filter(self) -> None:
        corpus = load_corpus(CORPUS_PATH)
        expected = {
            "flowchart_medium",
            "flowchart_large",
            "flowchart_ports_heavy",
            "class_medium",
            "mindmap_medium",
            "requirement_medium",
            "architecture_medium",
        }

        self.assertNotIn("branch_start", corpus.suites)
        exact = compare_self.expand_filter_to_exact_benches(
            "end_to_end/(flowchart_medium|flowchart_large|flowchart_ports_heavy|"
            "class_medium|mindmap_medium|requirement_medium|architecture_medium)"
        )
        self.assertEqual(
            {compare_self.split_exact_bench(item)[1] for item in exact}, expected
        )

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
        current = dict(
            verify_pipeline_bench_list._pipeline_lane_groups(
                corpus,
                enabled_features=frozenset({"svg"}),
            )[0]
        )
        current_groups = sorted(current)
        expected_benches = sorted(
            verify_pipeline_bench_list._expected_pipeline_benches(
                corpus,
                current_groups=current,
            )
        )
        expected_set = set(expected_benches)
        self.assertIn("frontmatter_preprocess/frontmatter_basic", expected_set)
        self.assertNotIn("end_to_end/frontmatter_basic", expected_set)
        self.assertIn("end_to_end/error_basic", expected_set)
        self.assertNotIn("frontmatter_preprocess/error_basic", expected_set)
        fixture = corpus.fixtures[0].name
        list_output = "\n".join(f"{bench}: benchmark" for bench in expected_benches)
        receipt_output = "\n".join(
            "[bench][preflight] "
            + json.dumps(
                {
                    "schema_version": 1,
                    "benchmark": bench,
                    "output_kind": compare_self._PREFLIGHT_OUTPUT_KIND_BY_GROUP[group],
                    "output_bytes": 123,
                    "output_sha256": "a" * 64,
                    "svg_elements": 7 if group in {"render", "end_to_end"} else None,
                },
                separators=(",", ":"),
            )
            for bench in expected_benches
            for group, _fixture in (bench.rsplit("/", 1),)
        )
        output = f"{list_output}\n{receipt_output}"

        result = verify_pipeline_bench_list.validate_pipeline_bench_list(corpus, output)

        self.assertEqual(result["groups"], tuple(current_groups))
        self.assertEqual(result["receipt_count"], len(expected_benches))
        historical = output.replace(
            f"compatibility_json_parse/{fixture}",
            f"parse_known_type/{fixture}",
            1,
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

        missing_benchmark = "end_to_end/error_basic"
        without_benchmark = "\n".join(
            line
            for line in output.splitlines()
            if line != f"{missing_benchmark}: benchmark"
            and f'"benchmark":"{missing_benchmark}"' not in line
        )
        with self.assertRaisesRegex(
            verify_pipeline_bench_list.PipelineBenchListError,
            "corpus/lane product.*missing",
        ):
            verify_pipeline_bench_list.validate_pipeline_bench_list(
                corpus, without_benchmark
            )

        without_receipt = output.replace(
            next(
                line
                for line in output.splitlines()
                if line.startswith("[bench][preflight]")
            ),
            "",
            1,
        )
        with self.assertRaisesRegex(
            verify_pipeline_bench_list.PipelineBenchListError,
            "preflight receipts differ",
        ):
            verify_pipeline_bench_list.validate_pipeline_bench_list(
                corpus, without_receipt
            )

        for contract in (None, "docs/performance/contracts/unknown.json"):
            mixed = replace(
                corpus,
                lanes=tuple(
                    replace(lane, evidence_contract=contract)
                    if lane.id == "render-svg"
                    else lane
                    for lane in corpus.lanes
                ),
            )
            with self.subTest(contract=contract), self.assertRaisesRegex(
                verify_pipeline_bench_list.PipelineBenchListError,
                "uniformly declare",
            ):
                verify_pipeline_bench_list.validate_pipeline_bench_list(
                    mixed, output
                )

    def test_binding_request_corpus_owns_one_complete_benchmark_list(self) -> None:
        corpus = load_corpus(BINDING_REQUEST_CORPUS_PATH)
        fixture = corpus.fixtures[0]
        expected_benches = (
            "binding_request_empty_analysis_ascii_svg/info_fixed_cost",
            "binding_request_resource_override_analysis_ascii_svg/info_fixed_cost",
            "binding_request_version_only_analysis_ascii_svg/info_fixed_cost",
        )
        output = "\n".join(f"{bench}: benchmark" for bench in expected_benches)

        result = verify_pipeline_bench_list.validate_pipeline_bench_list(
            corpus,
            output,
            enabled_features=("analysis", "ascii", "svg"),
        )

        self.assertEqual(result["bench_count"], 3)
        self.assertEqual(
            result["lane_ids"],
            (
                "binding-analysis-ascii-svg-request-empty",
                "binding-analysis-ascii-svg-request-resource-override",
                "binding-analysis-ascii-svg-request-version-only",
            ),
        )
        self.assertEqual(
            corpus.default_group,
            "binding_request_version_only_analysis_ascii_svg",
        )
        self.assertEqual(
            resolve_lane_group(corpus, corpus.default_group).id,
            "binding-analysis-ascii-svg-request-version-only",
        )
        self.assertEqual(
            fixture.source,
            "crates/merman-bindings-core/benches/fixtures/info_fixed_cost.mmd",
        )
        lanes = {lane.id: lane for lane in corpus.lanes}
        self.assertEqual(
            {
                lane_id: (
                    lane.kind,
                    lane.owner,
                    lane.public_operation,
                    lane.transport,
                    lane.process_lifecycle,
                    lane.engine_lifecycle,
                    lane.required_features,
                    lane.logical_operations_per_estimate,
                    lane.measurement_metrics,
                    lane.size_vector,
                    lane.workload,
                )
                for lane_id, lane in lanes.items()
            },
            {
                "binding-analysis-ascii-svg-request-empty": (
                    "public",
                    "merman-bindings-core",
                    "binding-execute-operation-semantic-json",
                    "native-criterion",
                    "reused-process",
                    "reused-engine",
                    ("analysis", "ascii", "svg"),
                    1,
                    ("latency_ns",),
                    (),
                    "binding-semantic-info-analysis-ascii-svg-empty-trusted-native-v1",
                ),
                "binding-analysis-ascii-svg-request-version-only": (
                    "public",
                    "merman-bindings-core",
                    "binding-execute-operation-semantic-json",
                    "native-criterion",
                    "reused-process",
                    "reused-engine",
                    ("analysis", "ascii", "svg"),
                    1,
                    ("latency_ns",),
                    (),
                    "binding-semantic-info-analysis-ascii-svg-version-only-trusted-native-v1",
                ),
                "binding-request-version-only-memory": (
                    "public",
                    "merman-bindings-core",
                    "binding-execute-operation-semantic-json",
                    "native-system-allocator-subprocess",
                    "fresh-process",
                    "reused-engine",
                    ("analysis", "ascii", "svg"),
                    1,
                    ("allocation_count", "allocated_bytes", "peak_growth_bytes"),
                    (1, 2, 4, 10, 32, 100),
                    "binding-semantic-info-analysis-ascii-svg-version-only-operation-calls-v1",
                ),
                "binding-analysis-ascii-svg-request-resource-override": (
                    "public",
                    "merman-bindings-core",
                    "binding-execute-operation-semantic-json",
                    "native-criterion",
                    "reused-process",
                    "reused-engine",
                    ("analysis", "ascii", "svg"),
                    1,
                    ("latency_ns",),
                    (),
                    "binding-semantic-info-analysis-ascii-svg-resource-max-source-4096-trusted-native-v1",
                ),
            },
        )
        with self.assertRaisesRegex(
            verify_pipeline_bench_list.PipelineBenchListError,
            "missing=",
        ):
            verify_pipeline_bench_list.validate_pipeline_bench_list(
                corpus,
                "\n".join(
                    f"{bench}: benchmark" for bench in expected_benches[:-1]
                ),
                enabled_features=("analysis", "ascii", "svg"),
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

    def test_full_write_docs_dry_run_defers_suite_report_publication(self) -> None:
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
            f"target/bench/perf-runner/{perf_runner.today_stamp()}_full_suite_standard.md",
            out,
        )
        self.assertIn(
            f"target/bench/perf-runner/{perf_runner.today_stamp()}_full_suite_standard.json",
            out,
        )
        command, publication = out.split("==> publish Markdown reports", maxsplit=1)
        self.assertNotIn("docs/performance/", command)
        self.assertIn("docs/performance/", publication)
        self.assertNotIn("run_native_memory.py", out)

    def test_write_docs_publishes_only_after_every_measurement_step(self) -> None:
        calls: list[str] = []

        def record_step(step: perf_runner.Step, *, dry_run: bool) -> None:
            self.assertFalse(dry_run)
            calls.append(step.label)

        def record_publication(
            publications: list[perf_runner.ReportPublication], *, dry_run: bool
        ) -> None:
            self.assertFalse(dry_run)
            self.assertGreater(len(publications), 0)
            calls.append("publish")

        with (
            mock.patch.object(perf_runner, "run_step", side_effect=record_step),
            mock.patch.object(
                perf_runner, "publish_reports", side_effect=record_publication
            ),
            redirect_stdout(io.StringIO()),
        ):
            result = perf_runner.main(["--profile", "canary", "--write-docs"])

        self.assertEqual(result, 0)
        self.assertEqual(calls[-1], "publish")
        self.assertGreater(len(calls), 1)

    def test_failed_measurement_never_publishes_docs(self) -> None:
        with (
            mock.patch.object(perf_runner, "run_step", side_effect=SystemExit(9)),
            mock.patch.object(perf_runner, "publish_reports") as publish,
            redirect_stdout(io.StringIO()),
            self.assertRaisesRegex(SystemExit, "9"),
        ):
            perf_runner.main(["--profile", "canary", "--write-docs"])

        publish.assert_not_called()

    def test_report_publication_copies_only_prevalidated_sources(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            source = root / "target" / "report.md"
            destination = root / "docs" / "report.md"
            source.parent.mkdir(parents=True)
            source.write_text("measured\n", encoding="utf-8")

            with redirect_stdout(io.StringIO()):
                perf_runner.publish_reports(
                    [perf_runner.ReportPublication(source, destination)],
                    dry_run=False,
                )

            self.assertEqual(destination.read_text(encoding="utf-8"), "measured\n")

    def test_report_root_cannot_dirty_the_repository_before_comparison(self) -> None:
        stderr = io.StringIO()

        with redirect_stderr(stderr), self.assertRaises(SystemExit) as raised:
            perf_runner.main(
                [
                    "--profile",
                    "canary",
                    "--write-docs",
                    "--report-root",
                    "docs/performance",
                    "--dry-run",
                ]
            )

        self.assertEqual(raised.exception.code, 2)
        self.assertIn("must remain under target/bench", stderr.getvalue())

    def test_report_root_may_live_outside_the_repository(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir, redirect_stdout(io.StringIO()):
            result = perf_runner.main(
                [
                    "--profile",
                    "canary",
                    "--report-root",
                    temp_dir,
                    "--dry-run",
                ]
            )

        self.assertEqual(result, 0)

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
        if corpus.get("schema_version") == 2:
            contract_source = (
                ROOT
                / "docs/performance/contracts/native-criterion-preflight-v1.json"
            )
            contract_target = checkout / contract_source.relative_to(ROOT)
            contract_target.parent.mkdir(parents=True, exist_ok=True)
            contract_target.write_bytes(contract_source.read_bytes())
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

    @staticmethod
    def _preflight_receipt(
        benchmark: str = "end_to_end/flowchart_medium",
        *,
        output_sha256: str = "a" * 64,
        output_bytes: int = 123,
        svg_elements: int | None = 7,
    ) -> dict[str, object]:
        return {
            "schema_version": 1,
            "benchmark": benchmark,
            "output_kind": "svg",
            "output_bytes": output_bytes,
            "output_sha256": output_sha256,
            "svg_elements": svg_elements,
        }

    def test_native_preflight_receipts_reject_malformed_or_duplicate_entries(self) -> None:
        receipt = self._preflight_receipt()
        line = "[bench][preflight] " + json.dumps(receipt, separators=(",", ":"))

        self.assertEqual(
            compare_self.parse_preflight_receipts(line),
            {receipt["benchmark"]: receipt},
        )

        with self.assertRaisesRegex(compare_self.ContractViolation, "duplicate"):
            compare_self.parse_preflight_receipts(f"{line}\n{line}")

        malformed = dict(receipt)
        malformed["output_sha256"] = "short"
        with self.assertRaisesRegex(compare_self.ContractViolation, "output_sha256"):
            compare_self.parse_preflight_receipts(
                "[bench][preflight] "
                + json.dumps(malformed, separators=(",", ":"))
            )

        wrong_kind = dict(receipt)
        wrong_kind["output_kind"] = "typed_render_model"
        with self.assertRaisesRegex(compare_self.ContractViolation, "output_kind"):
            compare_self.parse_preflight_receipts(
                "[bench][preflight] "
                + json.dumps(wrong_kind, separators=(",", ":"))
            )

        for field, value in (("output_bytes", 0), ("svg_elements", 0)):
            invalid = dict(receipt)
            invalid[field] = value
            with self.subTest(field=field), self.assertRaisesRegex(
                compare_self.ContractViolation, field
            ):
                compare_self.parse_preflight_receipts(
                    "[bench][preflight] "
                    + json.dumps(invalid, separators=(",", ":"))
                )

        wrong_schema = dict(receipt)
        wrong_schema["schema_version"] = 2
        with self.assertRaisesRegex(compare_self.ContractViolation, "schema_version"):
            compare_self.parse_preflight_receipts(
                "[bench][preflight] "
                + json.dumps(wrong_schema, separators=(",", ":"))
            )

        no_group = dict(receipt)
        no_group["benchmark"] = "flowchart_medium"
        with self.assertRaisesRegex(compare_self.ContractViolation, "group boundary"):
            compare_self.parse_preflight_receipts(
                "[bench][preflight] "
                + json.dumps(no_group, separators=(",", ":"))
            )

        non_svg = dict(receipt)
        non_svg["benchmark"] = "parse/flowchart_medium"
        non_svg["output_kind"] = "typed_render_model"
        with self.assertRaisesRegex(compare_self.ContractViolation, "null for non-SVG"):
            compare_self.parse_preflight_receipts(
                "[bench][preflight] "
                + json.dumps(non_svg, separators=(",", ":"))
            )

        missing = dict(receipt)
        del missing["output_bytes"]
        with self.assertRaisesRegex(compare_self.ContractViolation, "fields differ"):
            compare_self.parse_preflight_receipts(
                "[bench][preflight] "
                + json.dumps(missing, separators=(",", ":"))
            )

        with self.assertRaisesRegex(compare_self.ContractViolation, "invalid benchmark preflight"):
            compare_self.parse_preflight_receipts("[bench][preflight] {")

        self.assertEqual(
            compare_self.parse_postflight_receipts(
                "[bench][postflight] end_to_end/flowchart_medium"
            ),
            {"end_to_end/flowchart_medium"},
        )
        with self.assertRaisesRegex(compare_self.ContractViolation, "duplicate"):
            compare_self.parse_postflight_receipts(
                "[bench][postflight] end_to_end/flowchart_medium\n"
                "[bench][postflight] end_to_end/flowchart_medium"
            )

    def test_direct_measurement_requires_the_discovery_output_receipt(self) -> None:
        benchmark = "end_to_end/flowchart_medium"
        receipt = self._preflight_receipt(benchmark)
        runner = SimpleNamespace(
            executable=Path("/tmp/pipeline"),
            recipe=SimpleNamespace(
                label="head",
                checkout=ROOT,
                logical_operations=1,
            ),
            env={},
            provenance={
                "corpus": {"preflight_receipts_required": True},
                "discovery": {"preflight_receipts": {benchmark: receipt}},
            },
        )
        timing = f"{benchmark}\ntime:   [100.0 ns 200.0 ns 300.0 ns]\n"
        missing = subprocess.CompletedProcess([], 0, stdout=timing, stderr="")
        with (
            mock.patch.object(compare_self, "_run_process", return_value=missing),
            self.assertRaisesRegex(compare_self.ContractViolation, "did not repeat"),
        ):
            compare_self._measure_once(
                runner,
                exact_bench=benchmark,
                sample_size=10,
                warm_up_seconds=1,
                measurement_seconds=1,
                timeout_seconds=30,
                sequence_index=1,
            )

        receipt_line = "[bench][preflight] " + json.dumps(
            receipt, separators=(",", ":")
        )
        completed = subprocess.CompletedProcess(
            [],
            0,
            stdout=timing,
            stderr=receipt_line
            + "\n[bench][postflight] end_to_end/flowchart_medium",
        )
        with mock.patch.object(compare_self, "_run_process", return_value=completed):
            measured = compare_self._measure_once(
                runner,
                exact_bench=benchmark,
                sample_size=10,
                warm_up_seconds=1,
                measurement_seconds=1,
                timeout_seconds=30,
                sequence_index=2,
            )
        self.assertEqual(measured["output_identity"], receipt)
        self.assertTrue(measured["postflight_verified"])

        extra_benchmark = "end_to_end/class_medium"
        extra_receipt = self._preflight_receipt(
            extra_benchmark, output_sha256="c" * 64
        )
        extra_preflight = subprocess.CompletedProcess(
            [],
            0,
            stdout=timing,
            stderr=receipt_line
            + "\n[bench][preflight] "
            + json.dumps(extra_receipt, separators=(",", ":"))
            + "\n[bench][postflight] end_to_end/flowchart_medium",
        )
        with (
            mock.patch.object(
                compare_self, "_run_process", return_value=extra_preflight
            ),
            self.assertRaisesRegex(
                compare_self.ContractViolation, "preflight receipts differ"
            ),
        ):
            compare_self._measure_once(
                runner,
                exact_bench=benchmark,
                sample_size=10,
                warm_up_seconds=1,
                measurement_seconds=1,
                timeout_seconds=30,
                sequence_index=3,
            )

        preflight_only = subprocess.CompletedProcess(
            [], 0, stdout=timing, stderr=receipt_line
        )
        with (
            mock.patch.object(
                compare_self, "_run_process", return_value=preflight_only
            ),
            self.assertRaisesRegex(compare_self.ContractViolation, "postflight receipts differ"),
        ):
            compare_self._measure_once(
                runner,
                exact_bench=benchmark,
                sample_size=10,
                warm_up_seconds=1,
                measurement_seconds=1,
                timeout_seconds=30,
                sequence_index=3,
            )

        changed = self._preflight_receipt(benchmark, output_sha256="b" * 64)
        changed_result = subprocess.CompletedProcess(
            [],
            0,
            stdout=timing,
            stderr="[bench][preflight] "
            + json.dumps(changed, separators=(",", ":"))
            + "\n[bench][postflight] end_to_end/flowchart_medium",
        )
        with (
            mock.patch.object(
                compare_self, "_run_process", return_value=changed_result
            ),
            self.assertRaisesRegex(compare_self.ContractViolation, "changed after discovery"),
        ):
            compare_self._measure_once(
                runner,
                exact_bench=benchmark,
                sample_size=10,
                warm_up_seconds=1,
                measurement_seconds=1,
                timeout_seconds=30,
                sequence_index=4,
            )

    def test_native_preflight_contract_content_must_match_the_harness(self) -> None:
        contract = (
            ROOT / "docs/performance/contracts/native-criterion-preflight-v1.json"
        )
        description = compare_self._describe_preflight_contract(contract)
        self.assertEqual(description["id"], "native-criterion-preflight-v1")

        with tempfile.TemporaryDirectory() as temp_dir:
            changed = Path(temp_dir) / contract.name
            value = json.loads(contract.read_text(encoding="utf-8"))
            value["output_kinds"]["render"] = "prepared_layout"
            changed.write_text(json.dumps(value), encoding="utf-8")
            with self.assertRaisesRegex(compare_self.ContractViolation, "differs"):
                compare_self._describe_preflight_contract(changed)

    def test_post_sampling_failure_revokes_every_timing_claim(self) -> None:
        report = {
            "rows": [
                {
                    "outcome": "confirmed_non_regression",
                    "improvement_outcome": "confirmed_improvement",
                    "base_ns": 100.0,
                    "head_ns": 80.0,
                    "bounds": {"relative_percent": {"upper": -10.0}},
                    "reason": None,
                }
            ]
        }

        compare_self._revoke_timing_claims(report, reason="verification failed")

        row = report["rows"][0]
        self.assertEqual(row["outcome"], "contract_failure")
        self.assertIsNone(row["improvement_outcome"])
        self.assertIsNone(row["base_ns"])
        self.assertIsNone(row["head_ns"])
        self.assertIsNone(row["bounds"])
        self.assertEqual(row["reason"], "verification failed")

    def test_finalize_revokes_timing_claims_when_any_contract_error_exists(self) -> None:
        report = {
            "method": {"discovery_only": False, "evidence_mode": "confirmation"},
            "fixtures": [{"coverage_status": "comparable"}],
            "contract_errors": [{"stage": "projection", "message": "failed"}],
            "rows": [
                {
                    "outcome": "confirmed_non_regression",
                    "improvement_outcome": "confirmed_improvement",
                    "base_ns": 100.0,
                    "head_ns": 80.0,
                    "bounds": {"relative_percent": {"upper": -10.0}},
                    "reason": None,
                }
            ],
        }

        exit_code = compare_self._finalize_summary(report)

        self.assertEqual(exit_code, 2)
        self.assertEqual(report["summary"]["confirmed_improvements"], 0)
        self.assertEqual(report["rows"][0]["outcome"], "contract_failure")
        self.assertIsNone(report["rows"][0]["improvement_outcome"])

    def test_current_only_classification_does_not_hide_head_execution_failure(self) -> None:
        benchmark = "end_to_end/error_basic"
        contract = {
            "name": "error_basic",
            "family": "error",
            "base_benchmark": None,
            "head_benchmark": benchmark,
            "selected": {"base": False, "head": True},
            "bytes": {"status": "missing_base"},
        }
        base = SimpleNamespace(
            recipe=SimpleNamespace(label="base"),
            benches=set(),
            skipped={},
            provenance={
                "corpus": {"preflight_receipts_required": False},
                "discovery": {"preflight_receipts": {}},
            },
        )
        head = SimpleNamespace(
            recipe=SimpleNamespace(label="head"),
            benches=set(),
            skipped={"end_to_end": ["error_basic: render failed"]},
            provenance={
                "corpus": {"preflight_receipts_required": True},
                "discovery": {"preflight_receipts": {}},
            },
        )

        completed = compare_self._complete_fixture_contracts(
            [contract], base=base, head=head
        )[0]

        self.assertEqual(completed["coverage_status"], "coverage_only")
        self.assertEqual(completed["coverage_class"], "execution_failure")

    def test_historical_only_classification_does_not_hide_base_execution_failure(self) -> None:
        benchmark = "end_to_end/error_basic"
        contract = {
            "name": "error_basic",
            "family": "error",
            "base_benchmark": benchmark,
            "head_benchmark": None,
            "selected": {"base": True, "head": False},
            "bytes": {"status": "missing_head"},
        }
        base = SimpleNamespace(
            recipe=SimpleNamespace(label="base"),
            benches=set(),
            skipped={"end_to_end": ["error_basic: render failed"]},
            provenance={
                "corpus": {"preflight_receipts_required": True},
                "discovery": {"preflight_receipts": {}},
            },
        )
        head = SimpleNamespace(
            recipe=SimpleNamespace(label="head"),
            benches=set(),
            skipped={},
            provenance={
                "corpus": {"preflight_receipts_required": False},
                "discovery": {"preflight_receipts": {}},
            },
        )

        completed = compare_self._complete_fixture_contracts(
            [contract], base=base, head=head
        )[0]

        self.assertEqual(completed["coverage_status"], "coverage_only")
        self.assertEqual(completed["coverage_class"], "execution_failure")

    def test_current_pipeline_rows_require_matching_output_identity(self) -> None:
        benchmark = "end_to_end/flowchart_medium"
        receipt = self._preflight_receipt(benchmark)
        contract = {
            "name": "flowchart_medium",
            "family": "flowchart",
            "base_benchmark": benchmark,
            "head_benchmark": benchmark,
            "selected": {"base": True, "head": True},
            "bytes": {"status": "identical"},
        }

        def runner(label: str, value: dict[str, object]) -> SimpleNamespace:
            return SimpleNamespace(
                recipe=SimpleNamespace(label=label),
                benches={benchmark},
                skipped={},
                provenance={
                    "corpus": {"preflight_receipts_required": True},
                    "discovery": {"preflight_receipts": {benchmark: value}},
                },
            )

        matched = compare_self._complete_fixture_contracts(
            [copy.deepcopy(contract)],
            base=runner("base", receipt),
            head=runner("head", receipt),
        )[0]
        self.assertEqual(matched["coverage_status"], "comparable")
        self.assertEqual(matched["coverage_class"], "common")
        self.assertEqual(matched["output_identity"]["status"], "matched")

        changed = self._preflight_receipt(output_sha256="b" * 64)
        mismatched = compare_self._complete_fixture_contracts(
            [copy.deepcopy(contract)],
            base=runner("base", receipt),
            head=runner("head", changed),
        )[0]
        self.assertEqual(mismatched["coverage_status"], "coverage_only")
        self.assertEqual(mismatched["coverage_class"], "output_mismatch")
        self.assertTrue(
            any("output identity differs" in reason for reason in mismatched["coverage_reasons"])
        )

    def test_legacy_pipeline_rows_record_identity_gap_without_claiming_a_match(self) -> None:
        benchmark = "end_to_end/flowchart_medium"
        receipt = self._preflight_receipt(benchmark)
        contract = {
            "name": "flowchart_medium",
            "family": "flowchart",
            "base_benchmark": benchmark,
            "head_benchmark": benchmark,
            "selected": {"base": True, "head": True},
            "bytes": {"status": "identical"},
        }
        base = SimpleNamespace(
            recipe=SimpleNamespace(label="base"),
            benches={benchmark},
            skipped={},
            provenance={
                "corpus": {"preflight_receipts_required": False},
                "discovery": {"preflight_receipts": {}},
            },
        )
        head = SimpleNamespace(
            recipe=SimpleNamespace(label="head"),
            benches={benchmark},
            skipped={},
            provenance={
                "corpus": {"preflight_receipts_required": True},
                "discovery": {"preflight_receipts": {benchmark: receipt}},
            },
        )

        completed = compare_self._complete_fixture_contracts(
            [contract], base=base, head=head
        )[0]

        self.assertEqual(completed["coverage_status"], "coverage_only")
        self.assertEqual(completed["coverage_class"], "unverified_output")
        self.assertEqual(
            completed["output_identity"]["status"], "legacy_unavailable"
        )
        self.assertIsNone(completed["output_identity"]["base"])
        self.assertEqual(completed["output_identity"]["head"], receipt)

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

        relative_only = compare_self._comparison_row(
            common.pop("contract"),
            pairs=self._pairs(100.0, 112.0),
            **common,
        )
        absolute_only = compare_self._comparison_row(
            self._contract(),
            pairs=self._pairs(1_000_000.0, 1_080_000.0),
            **common,
        )
        both_bounds = compare_self._comparison_row(
            self._contract(),
            pairs=self._pairs(1_000_000.0, 1_040_000.0),
            **common,
        )
        regression = compare_self._comparison_row(
            self._contract(),
            pairs=self._pairs(1_000_000.0, 1_120_000.0),
            **common,
        )

        self.assertEqual(relative_only["outcome"], "inconclusive")
        self.assertEqual(absolute_only["outcome"], "inconclusive")
        self.assertEqual(both_bounds["outcome"], "confirmed_non_regression")
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

    def test_discovery_reuse_rejects_conflicting_freeze_mode(self) -> None:
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

    def test_legal_discovery_reuse_never_enters_the_cargo_build_path(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            base_dir = root / "base"
            head_dir = root / "head"
            base_dir.mkdir()
            head_dir.mkdir()
            base_executable = root / "base-bench"
            head_executable = root / "head-bench"
            base_executable.write_bytes(b"base")
            head_executable.write_bytes(b"head")
            args = SimpleNamespace(
                base_dir=str(base_dir),
                head_dir=str(head_dir),
                base_label="base",
                head_label="head",
                base_target_dir=str(root / "target"),
                head_target_dir=str(root / "target"),
                freeze_shared_target=False,
                reuse_discovery_json=str(root / "discovery.json"),
                reuse_discovery_sha256="a" * 64,
                base_package="merman",
                head_package="merman",
                base_bench="pipeline",
                head_bench="pipeline",
                base_features="svg",
                head_features="svg",
                base_default_features=False,
                head_default_features=False,
                base_toolchain="1.95.0",
                head_toolchain="1.95.0",
                base_target="",
                head_target="",
                base_corpus="tools/bench/corpus.json",
                head_corpus="tools/bench/corpus.json",
                suite="canary",
                group=None,
                base_group=None,
                head_group=None,
                filter="end_to_end/flowchart_medium",
                preset="long",
                sample_size=None,
                warm_up=None,
                measurement=None,
                evidence_mode="confirmation",
                pairs=2,
                calibration_pairs=8,
                max_pairs=32,
                start_side="base",
                relative_threshold_percent=10.0,
                absolute_threshold_ns=None,
                absolute_threshold_us=None,
                confidence_level=0.95,
                bootstrap_seed=0,
                bootstrap_resamples=10_000,
                base_logical_operations=None,
                head_logical_operations=None,
                timeout_seconds=30,
                allow_dirty=False,
                discovery_only=False,
            )
            report = compare_self._empty_report(args, 50_000.0)
            contract = {
                "name": "flowchart_medium",
                "family": "flowchart",
                "base_benchmark": "end_to_end/flowchart_medium",
                "head_benchmark": "end_to_end/flowchart_medium",
                "selected": {"base": True, "head": True},
                "metadata": {"base": None, "head": None},
                "bytes": {"status": "identical", "base": {}, "head": {}},
            }
            source = {
                "generated_at": "2026-07-29T00:00:00Z",
                "harness": {"sha256": "b" * 64},
                "method": {
                    "shared_target_freeze": {"enabled": True},
                    "confidence_contract": {
                        "simultaneous_confidence_level": 0.95,
                        "component_confidence_level": 0.975,
                        "family_size": 2,
                        "multiplicity_adjustment": "bonferroni",
                        "components": "two metrics per comparable benchmark",
                    },
                },
                "runners": {"base": {}, "head": {}},
            }

            def prepare_reused(recipe, **_kwargs):
                executable = base_executable if recipe.label == "base" else head_executable
                digest = "1" * 64 if recipe.label == "base" else "2" * 64
                receipt = self._preflight_receipt(
                    "end_to_end/flowchart_medium"
                )
                provenance = {
                    "git": {
                        "revision": recipe.label,
                        "tree": recipe.label,
                        "dirty": False,
                    },
                    "corpus": {"preflight_receipts_required": True},
                    "discovery": {
                        "preflight_receipts": {
                            "end_to_end/flowchart_medium": receipt
                        }
                    },
                }
                return (
                    compare_self.PreparedRunner(
                        recipe=recipe,
                        executable=executable,
                        executable_sha256=digest,
                        benches={"end_to_end/flowchart_medium"},
                        skipped={},
                        provenance=provenance,
                        env={},
                        frozen=True,
                    ),
                    provenance,
                    [],
                )

            def aa_schedule(_runner, *, contracts, pair_count, **_kwargs):
                pairs = [
                    {
                        "a": {"normalized_ns": 1_000_000.0},
                        "b": {"normalized_ns": 1_000_000.0},
                        "first": {"normalized_ns": 1_000_000.0},
                        "second": {"normalized_ns": 1_000_000.0},
                    }
                    for _ in range(pair_count)
                ]
                return {
                    "rounds": [],
                    "rows": {item["name"]: pairs for item in contracts},
                    "errors": {},
                }

            def ab_schedule(*, contracts, pair_count, **_kwargs):
                pairs = [
                    {
                        "base": {"normalized_ns": 1_000_000.0},
                        "head": {"normalized_ns": 1_000_000.0},
                    }
                    for _ in range(pair_count)
                ]
                return {
                    "rounds": [],
                    "rows": {item["name"]: pairs for item in contracts},
                    "errors": {},
                }

            with (
                mock.patch.object(
                    compare_self,
                    "_load_reusable_discovery_report",
                    return_value=(source, {"path": str(root / "discovery.json")}),
                ),
                mock.patch.object(compare_self, "_validate_reusable_discovery_report"),
                mock.patch.object(
                    compare_self,
                    "_load_fixture_contracts",
                    return_value=([contract], {"kind": "filter"}, (None, None)),
                ),
                mock.patch.object(compare_self, "_validate_reuse_comparison_contract"),
                mock.patch.object(
                    compare_self,
                    "_prepare_runner",
                    side_effect=AssertionError("Cargo build path entered"),
                ) as cargo_prepare,
                mock.patch.object(
                    compare_self,
                    "_prepare_reused_runner",
                    side_effect=prepare_reused,
                ) as reuse_prepare,
                mock.patch.object(
                    compare_self, "_run_aa_schedule", side_effect=aa_schedule
                ) as aa,
                mock.patch.object(
                    compare_self, "_run_ab_schedule", side_effect=ab_schedule
                ) as ab,
                mock.patch.object(compare_self, "_verification_errors", return_value=[]),
                mock.patch.object(
                    compare_self, "_fixture_verification_errors", return_value=[]
                ),
                mock.patch.object(
                    compare_self,
                    "_discovery_reuse_verification_errors",
                    return_value=[],
                ),
            ):
                compare_self._execute_comparison(args, report)

        cargo_prepare.assert_not_called()
        self.assertEqual(reuse_prepare.call_count, 2)
        self.assertEqual(aa.call_count, 2)
        ab.assert_called_once()
        self.assertEqual(report["method"]["discovery_reuse"]["status"], "verified")
        self.assertEqual(report["rows"][0]["outcome"], "confirmed_non_regression")

        invalid_reuse_modes = {
            "preregistered lowercase": lambda value: setattr(
                value, "reuse_discovery_sha256", "bad"
            ),
            "requires --reuse-discovery-json": lambda value: setattr(
                value, "reuse_discovery_json", ""
            ),
            "cannot be combined": lambda value: setattr(
                value, "discovery_only", True
            ),
            "only valid": lambda value: setattr(
                value, "evidence_mode", "diagnostic"
            ),
            "requires clean": lambda value: setattr(value, "allow_dirty", True),
        }
        for message, mutate in invalid_reuse_modes.items():
            changed = copy.copy(args)
            mutate(changed)
            with self.subTest(message=message), self.assertRaisesRegex(
                compare_self.ContractViolation, message
            ):
                compare_self._validate_args(changed)

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

    def test_alias_detected_after_json_write_preserves_json(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            base = root / "base"
            head = root / "head"
            base.mkdir()
            head.mkdir()
            markdown = root / "evidence.md"
            structured = root / "evidence.json"
            with (
                mock.patch.object(compare_self, "_execute_comparison"),
                mock.patch.object(
                    compare_self,
                    "_same_existing_file",
                    side_effect=[False, True],
                ),
            ):
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
        benchmark = "end_to_end/flowchart_medium"
        receipt = {
            "schema_version": 1,
            "benchmark": benchmark,
            "output_kind": "svg",
            "output_bytes": 123,
            "output_sha256": "a" * 64,
            "svg_elements": 7,
        }
        runner = {
            "recipe": {},
            "git": {},
            "manifest": {},
            "workspace_manifest": {},
            "lockfile": {},
            "corpus": {
                "preflight_receipts_required": True,
                "preflight_contract": {
                    "path": "/tmp/native-criterion-preflight-v1.json",
                    "bytes": 100,
                    "sha256": "b" * 64,
                },
            },
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
            "discovery": {"preflight_receipts": {benchmark: receipt}},
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
                    "output_identity": {
                        "status": "matched",
                        "base": copy.deepcopy(receipt),
                        "head": copy.deepcopy(receipt),
                    },
                    "post_sampling_verification": {"status": "verified"},
                }
            ],
            "rows": [
                {
                    "base_benchmark": "end_to_end/flowchart_medium",
                    "head_benchmark": "end_to_end/flowchart_medium",
                    "outcome": "diagnostic_advisory",
                    "output_identity": {
                        "status": "matched",
                        "base": copy.deepcopy(receipt),
                        "head": copy.deepcopy(receipt),
                    },
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

    def test_reusable_discovery_ignores_unselected_output_drift(self) -> None:
        selected = "end_to_end/flowchart_medium"
        unselected = "end_to_end/state_medium"
        origin = {
            "bench_count": 2,
            "benches": [selected, unselected],
            "skipped": {},
            "preflight_receipts": {
                selected: CompareSelfContractsTest._preflight_receipt(selected),
                unselected: CompareSelfContractsTest._preflight_receipt(
                    unselected, output_sha256="b" * 64
                ),
            },
            "output_sha256": "c" * 64,
        }
        current = copy.deepcopy(origin)
        current["preflight_receipts"][unselected]["output_sha256"] = "d" * 64
        current["output_sha256"] = "e" * 64

        compare_self._require_reusable_discovery_match(
            label="base",
            current=current,
            origin=origin,
            required_benchmarks=frozenset({selected}),
        )

        current["preflight_receipts"][selected]["output_sha256"] = "f" * 64
        with self.assertRaisesRegex(
            compare_self.ContractViolation, "selected preflight receipt"
        ):
            compare_self._require_reusable_discovery_match(
                label="base",
                current=current,
                origin=origin,
                required_benchmarks=frozenset({selected}),
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

        missing_receipts = copy.deepcopy(valid)
        del missing_receipts["fixtures"][0]["output_identity"]
        with self.assertRaisesRegex(compare_self.ContractViolation, "output identity"):
            compare_self._validate_reusable_discovery_report(missing_receipts)

        wrong_order = copy.deepcopy(valid)
        wrong_order["runners"]["base"]["shared_target_freeze"]["build_sequence"] = 2
        with self.assertRaisesRegex(compare_self.ContractViolation, "build sequence"):
            compare_self._validate_reusable_discovery_report(wrong_order)

        type_confusions = (
            ("schema_version", lambda report: report.__setitem__("schema_version", 2.0)),
            (
                "discovery_only",
                lambda report: report["method"].__setitem__("discovery_only", 1),
            ),
            (
                "complete successfully",
                lambda report: report["summary"].__setitem__("exit_code", False),
            ),
            (
                "contract failures",
                lambda report: report["summary"].__setitem__(
                    "contract_failures", False
                ),
            ),
            (
                "comparable count",
                lambda report: report["summary"].__setitem__("comparable", 1.0),
            ),
        )
        for message, mutate in type_confusions:
            confused = copy.deepcopy(valid)
            mutate(confused)
            with self.subTest(message=message), self.assertRaisesRegex(
                compare_self.ContractViolation, message
            ):
                compare_self._validate_reusable_discovery_report(confused)

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
            corpus.write_text(
                json.dumps(
                    CompareSelfContractsTest._minimal_corpus(
                        schema_version=2,
                        default_group="end_to_end",
                    )
                )
                + "\n",
                encoding="utf-8",
            )
            contract_source = (
                ROOT
                / "docs/performance/contracts/native-criterion-preflight-v1.json"
            )
            contract_target = checkout / contract_source.relative_to(ROOT)
            contract_target.parent.mkdir(parents=True)
            contract_target.write_bytes(contract_source.read_bytes())
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
                / PIPELINE_EXECUTABLE
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
                "corpus": compare_self._describe_corpus(corpus, recipe=recipe),
                "bench_source": compare_self._describe_required_file(bench_source),
            }
            executable_description = compare_self._describe_required_file(executable)
            frozen_description = {
                **executable_description,
                "executable": True,
                "mode": "0555",
            }
            receipt = CompareSelfContractsTest._preflight_receipt()
            receipt_line = "[bench][preflight] " + json.dumps(
                receipt, separators=(",", ":")
            )
            discovery_stdout = "end_to_end/flowchart_medium: benchmark\n"
            combined = "\n".join((discovery_stdout, receipt_line))
            discovery = {
                "bench_count": 1,
                "benches": ["end_to_end/flowchart_medium"],
                "skipped": {},
                "preflight_receipts": {
                    "end_to_end/flowchart_medium": receipt,
                },
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
                "discovery_command": compare_self.criterion_list_command(
                    executable,
                    exact_benchmark="end_to_end/flowchart_medium",
                ),
                "discovery": discovery,
                "post_sampling_verification": {
                    "status": "verified",
                    "git": git,
                    "files": {key: value["sha256"] for key, value in files.items()},
                    "executable_sha256": executable_description["sha256"],
                },
            }
            listed = mock.Mock(
                returncode=0,
                stdout=discovery_stdout,
                stderr=receipt_line,
            )
            package_id = f"path+file://{checkout}#merman@0.0.0"
            metadata = mock.Mock(
                returncode=0,
                stdout=json.dumps(
                    {
                        "workspace_members": [package_id],
                        "packages": [
                            {
                                "id": package_id,
                                "name": "merman",
                                "manifest_path": str(manifest),
                            }
                        ],
                    }
                ),
                stderr="",
            )

            with (
                mock.patch.object(compare_self, "_git_provenance", return_value=git),
                mock.patch.object(
                    compare_self, "_toolchain_version", return_value="rustc test"
                ),
                mock.patch.object(
                    compare_self, "_cargo_version", return_value="cargo test"
                ),
                mock.patch.object(
                    compare_self,
                    "_run_process",
                    side_effect=[metadata, listed],
                ) as run,
            ):
                runner, provenance, errors = compare_self._prepare_reused_runner(
                    recipe,
                    origin=origin,
                    source_report={"path": "/tmp/discovery.json", "sha256": "c" * 64},
                    required_benchmarks=frozenset(
                        {"end_to_end/flowchart_medium"}
                    ),
                    timeout_seconds=1,
                )

            self.assertFalse(errors)
            self.assertIsNotNone(runner)
            assert runner is not None
            self.assertTrue(runner.frozen)
            self.assertEqual(runner.executable, executable.resolve())
            self.assertEqual(provenance["discovery_reuse"]["status"], "verified")
            self.assertEqual(run.call_count, 2)
            self.assertEqual(
                run.call_args_list[0].args[0][0:3],
                ["cargo", "+1.95.0", "metadata"],
            )
            self.assertEqual(run.call_args_list[1].args[0][0], str(executable))
            self.assertNotIn("build", run.call_args_list[0].args[0])

            invalid_rediscoveries = {
                "missing": mock.Mock(
                    returncode=0,
                    stdout=discovery_stdout,
                    stderr="",
                ),
                "extra": mock.Mock(
                    returncode=0,
                    stdout=discovery_stdout,
                    stderr=receipt_line
                    + "\n[bench][preflight] "
                    + json.dumps(
                        CompareSelfContractsTest._preflight_receipt(
                            "end_to_end/class_medium", output_sha256="c" * 64
                        ),
                        separators=(",", ":"),
                    ),
                ),
            }
            for label, invalid_rediscovery in invalid_rediscoveries.items():
                with (
                    self.subTest(label=label),
                    mock.patch.object(
                        compare_self, "_git_provenance", return_value=git
                    ),
                    mock.patch.object(
                        compare_self, "_toolchain_version", return_value="rustc test"
                    ),
                    mock.patch.object(
                        compare_self, "_cargo_version", return_value="cargo test"
                    ),
                    mock.patch.object(
                        compare_self,
                        "_run_process",
                        side_effect=[metadata, invalid_rediscovery],
                    ),
                ):
                    invalid_runner, _invalid_provenance, invalid_errors = (
                        compare_self._prepare_reused_runner(
                            recipe,
                            origin=origin,
                            source_report={
                                "path": "/tmp/discovery.json",
                                "sha256": "c" * 64,
                            },
                            required_benchmarks=frozenset(
                                {"end_to_end/flowchart_medium"}
                            ),
                            timeout_seconds=1,
                        )
                    )
                self.assertIsNone(invalid_runner)
                self.assertTrue(
                    any("preflight receipts differ" in error for error in invalid_errors)
                )

            runner_mutations = {
                "Git revision": lambda value: value["git"].__setitem__(
                    "revision", "c" * 40
                ),
                "lockfile digest": lambda value: value["lockfile"].__setitem__(
                    "sha256", "0" * 64
                ),
                "toolchain": lambda value: value["toolchain"].__setitem__(
                    "cargo_verbose", "cargo other"
                ),
                "prebuild command": lambda value: value["prebuild_command"].append(
                    "--forged"
                ),
                "frozen mode": lambda value: (
                    value["frozen_executable"].__setitem__("mode", "0755"),
                    value["shared_target_freeze"]["frozen_executable"].__setitem__(
                        "mode", "0755"
                    ),
                ),
                "origin verification": lambda value: value[
                    "post_sampling_verification"
                ]["files"].__setitem__("lockfile", "0" * 64),
                "selected preflight receipt": lambda value: value["discovery"][
                    "preflight_receipts"
                ]["end_to_end/flowchart_medium"].__setitem__(
                    "output_sha256", "0" * 64
                ),
            }
            for label, mutate in runner_mutations.items():
                changed = copy.deepcopy(origin)
                mutate(changed)
                with (
                    self.subTest(label=label),
                    mock.patch.object(
                        compare_self, "_git_provenance", return_value=git
                    ),
                    mock.patch.object(
                        compare_self, "_toolchain_version", return_value="rustc test"
                    ),
                    mock.patch.object(
                        compare_self, "_cargo_version", return_value="cargo test"
                    ),
                    mock.patch.object(
                        compare_self,
                        "_run_process",
                        side_effect=[metadata, listed],
                    ),
                ):
                    changed_runner, _changed_provenance, changed_errors = (
                        compare_self._prepare_reused_runner(
                            recipe,
                            origin=changed,
                            source_report={
                                "path": "/tmp/discovery.json",
                                "sha256": "c" * 64,
                            },
                            required_benchmarks=frozenset(
                                {"end_to_end/flowchart_medium"}
                            ),
                            timeout_seconds=1,
                        )
                    )

                self.assertIsNone(changed_runner)
                self.assertTrue(changed_errors)

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
                mock.patch.object(
                    compare_self,
                    "_run_process",
                    return_value=metadata,
                ) as swapped_run,
            ):
                swapped_runner, _provenance, swapped_errors = (
                    compare_self._prepare_reused_runner(
                        recipe,
                        origin=swapped,
                        source_report={
                            "path": "/tmp/discovery.json",
                            "sha256": "c" * 64,
                        },
                        required_benchmarks=frozenset(
                            {"end_to_end/flowchart_medium"}
                        ),
                        timeout_seconds=1,
                    )
                )

            self.assertIsNone(swapped_runner)
            self.assertIn("destination identity differs", swapped_errors[0])
            swapped_run.assert_called_once()
            self.assertEqual(
                swapped_run.call_args.args[0][0:3],
                ["cargo", "+1.95.0", "metadata"],
            )

    def test_reuse_comparison_contract_allows_new_selection_but_rejects_runner_drift(
        self,
    ) -> None:
        root = Path("/tmp/reuse-contract")
        recipes = {
            side: self._recipe(
                label=side,
                checkout=root / side,
                package="merman",
                bench="pipeline",
                features=("svg",),
                default_features=False,
                toolchain="1.95.0",
                target_dir=root / f"target-{side}",
                corpus=Path("tools/bench/corpus.json"),
            )
            for side in ("base", "head")
        }
        method = {
            "preset": "long",
            "sample_size": 30,
            "warm_up_seconds": 2,
            "measurement_seconds": 3,
            "diagnostic_pairs": 2,
            "calibration_pairs": 8,
            "max_pairs": 32,
            "start_side": "base",
            "relative_threshold_percent": 10.0,
            "relative_threshold_log": math.log1p(0.10),
            "absolute_threshold_ns": 50_000.0,
            "absolute_threshold_us": 50.0,
            "confidence_level": 0.95,
            "bootstrap_seed": 0,
            "bootstrap_resamples": 10_000,
            "interval_contract": {"confirmation": "paired"},
        }
        report = {
            "comparison": {"base_label": "base", "head_label": "head"},
            "environment": {"machine": "test"},
            "method": method,
        }
        selection = {
            "kind": "filter",
            "groups": {"base": "end_to_end", "head": "end_to_end"},
            "lane_contracts": {"effective": ("render-svg",)},
        }
        contracts = [
            {
                "name": "flowchart_medium",
                "family": "flowchart",
                "base_benchmark": "end_to_end/flowchart_medium",
                "head_benchmark": "end_to_end/flowchart_medium",
                "selected": {"base": True, "head": True},
                "metadata": {"base": None, "head": None},
                "bytes": {
                    "status": "identical",
                    "base": {"sha256": "a" * 64},
                    "head": {"sha256": "a" * 64},
                },
            }
        ]
        source = json.loads(
            json.dumps(
                {
                    "comparison": report["comparison"],
                    "environment": report["environment"],
                    "method": method,
                    "recipes": {
                        side: compare_self._recipe_report(recipe)
                        for side, recipe in recipes.items()
                    },
                    "selection": selection,
                    "fixtures": contracts,
                }
            )
        )

        compare_self._validate_reuse_comparison_contract(
            source=source,
            report=report,
            recipes=recipes,
        )

        changed_selection = copy.deepcopy(source)
        changed_selection["selection"]["groups"]["head"] = "render"
        changed_selection["fixtures"][0]["bytes"]["head"]["sha256"] = "b" * 64
        changed_selection["method"]["bootstrap_seed"] = 20260806
        changed_selection["method"]["bootstrap_resamples"] = 20_000
        compare_self._validate_reuse_comparison_contract(
            source=changed_selection,
            report=report,
            recipes=recipes,
        )

        mutations = {
            "comparison labels": lambda value: value["comparison"].__setitem__(
                "head_label", "other"
            ),
            "environment": lambda value: value["environment"].__setitem__(
                "machine", "other"
            ),
            "method": lambda value: value["method"].__setitem__("sample_size", 31),
            "recipes": lambda value: value["recipes"]["head"].__setitem__(
                "bench", "other"
            ),
        }
        for label, mutate in mutations.items():
            changed = copy.deepcopy(source)
            mutate(changed)
            with self.subTest(label=label), self.assertRaisesRegex(
                compare_self.ContractViolation, "differs"
            ):
                compare_self._validate_reuse_comparison_contract(
                    source=changed,
                    report=report,
                    recipes=recipes,
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
            artifact = target_dir / "debug" / "deps" / PIPELINE_EXECUTABLE
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
            corpus.write_text(
                json.dumps(
                    CompareSelfContractsTest._minimal_corpus(
                        schema_version=2,
                        default_group="end_to_end",
                    )
                )
                + "\n",
                encoding="utf-8",
            )
            contract_source = (
                ROOT
                / "docs/performance/contracts/native-criterion-preflight-v1.json"
            )
            contract_target = checkout / contract_source.relative_to(ROOT)
            contract_target.parent.mkdir(parents=True)
            contract_target.write_bytes(contract_source.read_bytes())
            bench_source = checkout / "benches" / "pipeline.rs"
            bench_source.parent.mkdir()
            bench_source.write_text("fn main() {}\n", encoding="utf-8")
            target_dir = root / "target"
            cargo_artifact = target_dir / "debug" / "deps" / PIPELINE_EXECUTABLE
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
            package_id = f"path+file://{checkout}#merman@0.0.0"
            metadata_result = mock.Mock(
                returncode=0,
                stdout=json.dumps(
                    {
                        "workspace_members": [package_id],
                        "packages": [
                            {
                                "id": package_id,
                                "name": "merman",
                                "manifest_path": str(checkout / "Cargo.toml"),
                            }
                        ],
                    }
                ),
                stderr="",
            )
            clean_result = mock.Mock(
                returncode=0,
                stdout="",
                stderr="Removed 42 files, 12.3MiB total",
            )
            receipt = CompareSelfContractsTest._preflight_receipt()
            receipt_line = "[bench][preflight] " + json.dumps(
                receipt, separators=(",", ":")
            )
            discovery_result = mock.Mock(
                returncode=0,
                stdout="end_to_end/flowchart_medium: benchmark\n",
                stderr=receipt_line,
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
                    side_effect=[
                        metadata_result,
                        clean_result,
                        cargo_result,
                        discovery_result,
                    ],
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
                run_process.call_args_list[1].args[0][1:4],
                ["clean", "--locked", "--profile"],
            )
            self.assertEqual(
                run_process.call_args_list[1].kwargs["env"]["CARGO_BUILD_JOBS"],
                "1",
            )
            self.assertEqual(
                run_process.call_args_list[2].kwargs["env"]["CARGO_BUILD_JOBS"],
                "1",
            )
            self.assertEqual(
                provenance["discovery"]["preflight_receipts"],
                {"end_to_end/flowchart_medium": receipt},
            )

            invalid_discoveries = {
                "missing": mock.Mock(
                    returncode=0,
                    stdout="end_to_end/flowchart_medium: benchmark\n",
                    stderr="",
                ),
                "extra": mock.Mock(
                    returncode=0,
                    stdout="end_to_end/flowchart_medium: benchmark\n",
                    stderr=receipt_line
                    + "\n[bench][preflight] "
                    + json.dumps(
                        CompareSelfContractsTest._preflight_receipt(
                            "end_to_end/class_medium", output_sha256="c" * 64
                        ),
                        separators=(",", ":"),
                    ),
                ),
            }
            for label, invalid_discovery in invalid_discoveries.items():
                with (
                    self.subTest(label=label),
                    mock.patch.object(
                        compare_self, "_git_provenance", return_value=git
                    ),
                    mock.patch.object(
                        compare_self, "_toolchain_version", return_value="rustc"
                    ),
                    mock.patch.object(
                        compare_self, "_cargo_version", return_value="cargo"
                    ),
                    mock.patch.object(
                        compare_self,
                        "_run_process",
                        side_effect=[
                            metadata_result,
                            clean_result,
                            cargo_result,
                            invalid_discovery,
                        ],
                    ),
                ):
                    invalid_runner, _invalid_provenance, invalid_errors = (
                        compare_self._prepare_runner(
                            recipe,
                            allow_dirty=False,
                            timeout_seconds=1,
                            freeze_plan=compare_self.SharedTargetFreezePlan(
                                target_dir=target_dir,
                                context=f"prepare-{label}",
                            ),
                            build_sequence=1,
                        )
                    )
                self.assertIsNone(invalid_runner)
                self.assertTrue(
                    any("preflight receipts differ" in error for error in invalid_errors)
                )

    def test_shared_target_freeze_rejects_source_digest_drift_during_copy(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            target_dir = root / "target"
            artifact = target_dir / "debug" / "deps" / PIPELINE_EXECUTABLE
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

            frozen_files = list((target_dir / "perf-frozen").rglob(PIPELINE_EXECUTABLE))
            self.assertEqual(frozen_files, [])

    def test_shared_target_freeze_cleans_failed_read_only_publication(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            target_dir = root / "target"
            artifact = target_dir / "debug" / "deps" / PIPELINE_EXECUTABLE
            artifact.parent.mkdir(parents=True)
            artifact.write_bytes(b"bench executable")
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
                context="failed-publication-test",
            )
            digest = compare_self.hashlib.sha256(artifact.read_bytes()).hexdigest()

            with (
                mock.patch.object(
                    compare_self,
                    "_path_sha256",
                    side_effect=[digest, digest, digest, "0" * 64],
                ),
                self.assertRaisesRegex(
                    compare_self.ContractViolation,
                    "published frozen executable digest differs",
                ),
            ):
                compare_self._freeze_bench_executable(
                    artifact,
                    recipe=recipe,
                    git={"revision": "a" * 40, "tree": "b" * 40},
                    plan=plan,
                    build_sequence=1,
                )

            frozen_files = list((target_dir / "perf-frozen").rglob(PIPELINE_EXECUTABLE))
            self.assertEqual(frozen_files, [])
            freeze_context = target_dir / "perf-frozen" / "failed-publication-test"
            self.assertEqual(list(freeze_context.iterdir()), [])

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
            executable = root / "target" / "release" / "deps" / PIPELINE_EXECUTABLE
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
        executable = Path("target") / "release" / "deps" / PIPELINE_EXECUTABLE
        command = compare_self.criterion_command(
            executable=executable,
            exact_bench="end_to_end/flowchart_medium",
            sample_size=30,
            warm_up_seconds=2,
            measurement_seconds=3,
        )

        self.assertEqual(command[0], str(executable))
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
                    absolute=(35_000.0, 25_000.0, 45_000.0),
                ),
            ),
            (
                "inconclusive",
                self._bounds(
                    relative=(0.08, 0.07, 0.09),
                    absolute=(65_000.0, 55_000.0, 75_000.0),
                ),
            ),
            (
                "inconclusive",
                self._bounds(
                    relative=(0.12, 0.11, 0.13),
                    absolute=(35_000.0, 25_000.0, 45_000.0),
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

        threshold = math.log1p(0.10)
        cases = [
            (
                "confirmed_non_improvement",
                self._bounds(
                    relative=(-0.08, -0.09, -0.07),
                    absolute=(-35_000.0, -45_000.0, -25_000.0),
                ),
            ),
            (
                "inconclusive",
                self._bounds(
                    relative=(-0.08, -0.09, -0.07),
                    absolute=(-65_000.0, -75_000.0, -55_000.0),
                ),
            ),
            (
                "inconclusive",
                self._bounds(
                    relative=(-0.12, -0.13, -0.11),
                    absolute=(-35_000.0, -45_000.0, -25_000.0),
                ),
            ),
        ]
        for expected, case_bounds in cases:
            with self.subTest(expected=expected, bounds=case_bounds):
                self.assertEqual(
                    compare_self.classify_confirmation(
                        case_bounds,
                        relative_threshold=threshold,
                        absolute_threshold_ns=50_000.0,
                        direction="improvement",
                        evidence_mode="confirmation",
                        pair_count=8,
                        required_pairs=8,
                    ),
                    expected,
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

    def test_renderer_subprocess_timeout_fails_with_command_context(self) -> None:
        timeout = subprocess.TimeoutExpired(["renderer"], 7, output="partial output")
        with mock.patch.object(subprocess, "run", side_effect=timeout):
            with self.assertRaisesRegex(RuntimeError, r"timed out after 7s") as raised:
                compare_mermaid_renderers.run(
                    ["renderer", "--bench"], ROOT, timeout_seconds=7
                )
        self.assertIn("partial output", str(raised.exception))
        self.assertIn("renderer --bench", str(raised.exception))

    def test_native_runner_prebuild_records_unique_executable_digest(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            checkout = Path(temp_dir)
            (checkout / "Cargo.lock").write_text("", encoding="utf-8")
            executable = checkout / "target" / "release" / "deps" / "renderer-deadbeef"
            executable.parent.mkdir(parents=True)
            executable.write_bytes(b"criterion runner")
            executable.chmod(executable.stat().st_mode | stat.S_IXUSR)
            artifact = {
                "reason": "compiler-artifact",
                "target": {"name": "renderer", "kind": ["bench"]},
                "executable": str(executable),
            }
            bench_env = {"MMDR_RUN_CRITERION_BENCHES": "1"}

            with mock.patch.object(
                compare_mermaid_renderers,
                "run",
                return_value=json.dumps(artifact),
            ) as run_mock:
                runner = compare_mermaid_renderers.prepare_criterion_runner(
                    label="mmdr",
                    cwd=checkout,
                    bench_bin="renderer",
                    package=None,
                    features="benchmark",
                    env=bench_env,
                    toolchain="1.92.0",
                )

            command = run_mock.call_args.args[0]
            self.assertEqual(run_mock.call_count, 1)
            self.assertEqual(command[:4], ["cargo", "+1.92.0", "bench", "--locked"])
            self.assertIn("--no-run", command)
            self.assertIn("--message-format=json-render-diagnostics", command)
            self.assertEqual(run_mock.call_args.kwargs["env"], bench_env)
            self.assertEqual(runner.executable, executable.resolve())
            self.assertEqual(
                runner.sha256,
                compare_mermaid_renderers._sha256_file(executable),
            )

            duplicate = {
                **artifact,
                "executable": str(executable.with_name("renderer-cafebabe")),
            }
            with self.assertRaisesRegex(RuntimeError, "multiple executables"):
                compare_mermaid_renderers.parse_bench_executable(
                    "\n".join((json.dumps(artifact), json.dumps(duplicate))),
                    cwd=checkout,
                    bench_bin="renderer",
                )

    def test_native_runner_uses_prebuilt_binary_and_isolates_exact_failures(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            checkout = Path(temp_dir)
            executable = checkout / "renderer"
            executable.write_bytes(b"criterion runner")
            executable.chmod(executable.stat().st_mode | stat.S_IXUSR)
            prepared = compare_mermaid_renderers.PreparedCriterionRunner(
                executable=executable.resolve(),
                sha256=compare_mermaid_renderers._sha256_file(executable),
            )
            bench_env = {"MMDR_RUN_CRITERION_BENCHES": "1"}
            list_output = (
                "end_to_end/first: benchmark\n"
                "end_to_end/second: benchmark\n"
            )
            second_output = (
                "end_to_end/second\n"
                "time:   [100.0 ns 200.0 ns 300.0 ns]\n"
            )

            with mock.patch.object(
                compare_mermaid_renderers,
                "run",
                side_effect=[list_output, RuntimeError("first failed"), second_output],
            ) as run_mock, redirect_stdout(io.StringIO()):
                bench_list = compare_mermaid_renderers.list_criterion_benches(
                    cwd=checkout,
                    runner=prepared,
                    env=bench_env,
                )
                result = compare_mermaid_renderers.run_native_runner(
                    label="mmdr",
                    cwd=checkout,
                    runner=prepared,
                    exact_benches=["end_to_end/first", "end_to_end/second"],
                    bench_list=bench_list,
                    sample_size=20,
                    warm_up=1,
                    measurement=1,
                    env=bench_env,
                )

            commands = [call.args[0] for call in run_mock.call_args_list]
            self.assertEqual(run_mock.call_count, 3)
            self.assertTrue(
                all(
                    command[:2] == [str(executable.resolve()), "--bench"]
                    for command in commands
                )
            )
            self.assertNotIn("cargo", {part for command in commands for part in command})
            self.assertTrue(
                all(
                    call.kwargs["env"] == bench_env
                    for call in run_mock.call_args_list
                )
            )
            self.assertIn("end_to_end/first", result["errors"])
            self.assertEqual(result["times_ns"]["end_to_end/second"], 200.0)
            self.assertEqual(result["executable"]["status"], "verified")

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
        family_summary = compare_mermaid_renderers.build_family_summary(comparable_rows)
        family_coverage = compare_mermaid_renderers.build_family_coverage(family_summary)
        self.assertEqual(family_coverage["requested_count"], 1)
        self.assertEqual(family_coverage["native_same_byte_comparable_count"], 1)

    def test_git_provenance_rejects_dirty_tree_and_fingerprints_allowed_changes(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            checkout = Path(temp_dir)
            subprocess.run(["git", "init", "-q"], cwd=checkout, check=True)
            subprocess.run(
                ["git", "config", "user.email", "bench@example.com"],
                cwd=checkout,
                check=True,
            )
            subprocess.run(
                ["git", "config", "user.name", "Benchmark Contract"],
                cwd=checkout,
                check=True,
            )
            tracked = checkout / "tracked.txt"
            tracked.write_text("one\n", encoding="utf-8")
            subprocess.run(["git", "add", "tracked.txt"], cwd=checkout, check=True)
            subprocess.run(["git", "commit", "-qm", "fixture"], cwd=checkout, check=True)

            clean = compare_mermaid_renderers.capture_git_provenance(
                checkout,
                allow_dirty=False,
                expected_revision=None,
            )
            self.assertFalse(clean["dirty"])

            tracked.write_text("two\n", encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "--allow-dirty"):
                compare_mermaid_renderers.capture_git_provenance(
                    checkout,
                    allow_dirty=False,
                    expected_revision=None,
                )

            first = compare_mermaid_renderers.capture_git_provenance(
                checkout,
                allow_dirty=True,
                expected_revision=clean["revision"],
            )
            tracked.write_text("three\n", encoding="utf-8")
            second = compare_mermaid_renderers.capture_git_provenance(
                checkout,
                allow_dirty=True,
                expected_revision=clean["revision"],
            )
            self.assertNotEqual(first["worktree_sha256"], second["worktree_sha256"])

    def test_provenance_verification_reports_repo_input_and_fixture_drift(self) -> None:
        errors = compare_mermaid_renderers.provenance_verification_errors(
            before=compare_mermaid_renderers.ComparisonSnapshot(
                repositories={
                    "merman": {"revision": "a", "worktree_sha256": "1"}
                },
                files={"corpus": {"sha256": "3", "bytes": 10}},
                fixture_inputs={"flowchart_tiny": {"status": "identical"}},
            ),
            after=compare_mermaid_renderers.ComparisonSnapshot(
                repositories={
                    "merman": {"revision": "b", "worktree_sha256": "2"}
                },
                files={"corpus": {"sha256": "4", "bytes": 10}},
                fixture_inputs={"flowchart_tiny": {"status": "different"}},
            ),
        )

        self.assertEqual(len(errors), 3)
        self.assertTrue(any("revision changed" in error for error in errors))
        self.assertTrue(any("corpus changed" in error for error in errors))
        self.assertTrue(any("fixture inputs changed" in error for error in errors))

    def test_mermaid_js_output_retains_valid_raw_samples_and_recomputes_summary(self) -> None:
        method = {
            "measurement_stop_conditions": {
                "measure_ms": 1_000,
                "max_samples": 10,
            },
            "watchdogs": {
                "navigation_timeout_ms": 30_000,
                "fixture_timeout_ms": 62_000,
            },
        }
        runner = compare_mermaid_renderers.parse_mermaid_js_output(
            {
                "schema_version": 3,
                "meta": {"mermaid": "11.16.0"},
                "method": method,
                "results": {
                    "flowchart_tiny": {
                        "median_ns": 999,
                        "times_ns": [300.0, 100.0, 200.0],
                        "stop_reason": "measurement_time",
                        "sample_cap": 10,
                        "samples_truncated": False,
                        "preflight": {
                            "svg_chars": 123,
                            "svg_bytes": 123,
                            "svg_sha256": "a" * 64,
                            "view_box": [0, 0, 100, 50],
                        },
                    }
                },
            }
        )

        self.assertEqual(runner["times_ns"]["end_to_end/flowchart_tiny"], 200.0)
        self.assertEqual(runner["samples"]["flowchart_tiny"], 3)
        self.assertEqual(
            runner["raw_samples_ns"]["flowchart_tiny"],
            [300.0, 100.0, 200.0],
        )
        self.assertEqual(runner["sample_stats_ns"]["flowchart_tiny"]["p95"], 300.0)

        invalid = compare_mermaid_renderers.parse_mermaid_js_output(
            {
                "schema_version": 3,
                "method": method,
                "results": {
                    "flowchart_tiny": {
                        "times_ns": [0, float("nan")],
                        "stop_reason": "measurement_time",
                        "sample_cap": 10,
                        "samples_truncated": False,
                        "preflight": {
                            "svg_chars": 123,
                            "svg_bytes": 123,
                            "svg_sha256": "a" * 64,
                            "view_box": [0, 0, 100, 50],
                        },
                    }
                },
            }
        )
        self.assertIn("end_to_end/flowchart_tiny", invalid["errors"])
        self.assertNotIn("end_to_end/flowchart_tiny", invalid["times_ns"])

        mismatched_cap = compare_mermaid_renderers.parse_mermaid_js_output(
            {
                "schema_version": 3,
                "method": method,
                "results": {
                    "flowchart_tiny": {
                        "times_ns": [100.0],
                        "stop_reason": "measurement_time",
                        "sample_cap": 11,
                        "samples_truncated": False,
                        "preflight": {
                            "svg_chars": 123,
                            "svg_bytes": 123,
                            "svg_sha256": "a" * 64,
                            "view_box": [0, 0, 100, 50],
                        },
                    }
                },
            }
        )
        self.assertIn("end_to_end/flowchart_tiny", mismatched_cap["errors"])

    def test_comparison_contract_fails_closed_on_required_errors_and_empty_common_set(self) -> None:
        errors = compare_mermaid_renderers.comparison_contract_errors(
            exact_benches=["end_to_end/example"],
            merman={"errors": {"end_to_end/example": "failed"}, "times_ns": {}},
            mmdr={"errors": {}, "times_ns": {}},
            mermaid_js={"errors": {}, "times_ns": {}},
            rows=[],
            require_mermaid_js=True,
            provenance_errors=["corpus changed during sampling"],
        )

        self.assertTrue(any("merman benchmark errors" in error for error in errors))
        self.assertTrue(any("Mermaid JS measured no fixtures" in error for error in errors))
        self.assertTrue(any("no byte-identical" in error for error in errors))
        self.assertIn("corpus changed during sampling", errors)

    def test_mermaid_js_is_not_required_for_native_only_groups(self) -> None:
        self.assertFalse(
            compare_mermaid_renderers.requires_mermaid_js(
                ["parse/flowchart_tiny"],
                skip=False,
            )
        )
        self.assertTrue(
            compare_mermaid_renderers.requires_mermaid_js(
                ["end_to_end/flowchart_tiny"],
                skip=False,
            )
        )
        self.assertFalse(
            compare_mermaid_renderers.requires_mermaid_js(
                ["end_to_end/flowchart_tiny"],
                skip=True,
            )
        )

        errors = compare_mermaid_renderers.comparison_contract_errors(
            exact_benches=["parse/flowchart_tiny"],
            merman={"errors": {}, "times_ns": {"parse/flowchart_tiny": 10.0}},
            mmdr={"errors": {}, "times_ns": {"parse/flowchart_tiny": 20.0}},
            mermaid_js={"errors": {}, "times_ns": {}},
            rows=[
                {
                    "ratios": {
                        "merman_over_mermaid_rs_renderer": 0.5,
                    }
                }
            ],
            require_mermaid_js=False,
            provenance_errors=[],
        )
        self.assertFalse(any("Mermaid JS" in error for error in errors))

    def test_contract_ignores_unrequested_criterion_skip_lines(self) -> None:
        common = {
            "errors": {},
            "times_ns": {"end_to_end/flowchart_tiny": 10.0},
            "skipped": {"parse": ["unrequested_fixture"]},
        }
        rows = [
            {
                "ratios": {
                    "merman_over_mermaid_rs_renderer": 0.5,
                }
            }
        ]

        errors = compare_mermaid_renderers.comparison_contract_errors(
            exact_benches=["end_to_end/flowchart_tiny"],
            merman=common,
            mmdr={"errors": {}, "times_ns": {"end_to_end/flowchart_tiny": 20.0}},
            mermaid_js={"errors": {}, "times_ns": {}},
            rows=rows,
            require_mermaid_js=False,
            provenance_errors=[],
        )
        self.assertFalse(any("skipped" in error for error in errors))

        requested = {
            **common,
            "skipped": {"end_to_end": ["flowchart_tiny"]},
        }
        errors = compare_mermaid_renderers.comparison_contract_errors(
            exact_benches=["end_to_end/flowchart_tiny"],
            merman=requested,
            mmdr={"errors": {}, "times_ns": {"end_to_end/flowchart_tiny": 20.0}},
            mermaid_js={"errors": {}, "times_ns": {}},
            rows=rows,
            require_mermaid_js=False,
            provenance_errors=[],
        )
        self.assertIn("merman skipped one or more requested benchmarks", errors)


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

    def test_measurement_workflow_enforces_comparison_and_receipt_results(self) -> None:
        workflow = (ROOT / ".github" / "workflows" / "performance.yml").read_text(
            encoding="utf-8"
        )

        self.assertIn("comparison_exit=$?", workflow)
        self.assertIn("render_exit=$?", workflow)
        self.assertIn("--process-exit-code", workflow)
        self.assertIn("name: Enforce measurement result", workflow)
        self.assertIn('if [[ "$RENDER_EXIT" -ne 0 ]]', workflow)
        self.assertIn('case "$COMPARISON_EXIT" in', workflow)

    def test_perf_label_runs_compiled_contracts_without_repeating_python_contracts(
        self,
    ) -> None:
        workflow = (ROOT / ".github" / "workflows" / "performance.yml").read_text(
            encoding="utf-8"
        )

        self.assertIn(
            "if: github.event_name != 'pull_request' || contains(github.event.pull_request.labels.*.name, 'perf')",
            workflow,
        )
        self.assertEqual(workflow.count("if: github.event_name != 'pull_request'\n"), 2)
        self.assertIn("Verify compiled pipeline benchmark list", workflow)
        self.assertIn("Build native memory probe contract", workflow)

    def test_manual_regression_uses_the_requested_corpus_suite(self) -> None:
        workflow = (ROOT / ".github" / "workflows" / "performance.yml").read_text(
            encoding="utf-8"
        )

        self.assertIn(
            "SUITE: ${{ github.event_name == 'workflow_dispatch' && matrix.id == 'regression' && inputs.suite || matrix.suite }}",
            workflow,
        )


if __name__ == "__main__":
    sys.exit(unittest.main())
