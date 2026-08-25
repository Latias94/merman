#!/usr/bin/env python3
"""Contract tests for the corpus-driven performance helper scripts."""

from __future__ import annotations

import copy
import io
import json
import math
import os
import subprocess
import sys
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

import compare_self
import perf_runner
from corpus_utils import (
    load_corpus,
    resolve_lane_group,
)
from perf_contract_test_support import minimal_corpus, preflight_receipt


ROOT = Path(__file__).resolve().parents[2]
CORPUS_PATH = ROOT / "tools" / "bench" / "corpus.json"


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
        return minimal_corpus(
            schema_version=schema_version,
            default_group=default_group,
        )

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
        return preflight_receipt(
            benchmark,
            output_sha256=output_sha256,
            output_bytes=output_bytes,
            svg_elements=svg_elements,
        )

    def test_native_preflight_receipts_reject_malformed_or_duplicate_entries(self) -> None:
        receipt = self._preflight_receipt()
        line = "[bench][preflight] " + json.dumps(receipt, separators=(",", ":"))

        self.assertEqual(
            compare_self.parse_preflight_receipts(line),
            {receipt["benchmark"]: receipt},
        )

        ascii_receipt = {
            "schema_version": 1,
            "benchmark": "ascii_end_to_end/sequence_medium",
            "output_kind": "plain_ascii",
            "output_bytes": 321,
            "output_sha256": "b" * 64,
            "svg_elements": None,
        }
        self.assertEqual(
            compare_self.parse_preflight_receipts(
                "[bench][preflight] "
                + json.dumps(ascii_receipt, separators=(",", ":"))
            ),
            {ascii_receipt["benchmark"]: ascii_receipt},
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
        contracts = {
            "native-criterion-preflight-v1.json": {
                "id": "native-criterion-preflight-v1",
                "group": "render",
                "wrong_kind": "prepared_layout",
            },
            "native-ascii-criterion-preflight-v1.json": {
                "id": "native-ascii-criterion-preflight-v1",
                "group": "ascii_end_to_end",
                "wrong_kind": "svg",
            },
        }
        for filename, expected in contracts.items():
            with self.subTest(contract=filename):
                contract = ROOT / "docs/performance/contracts" / filename
                description = compare_self._describe_preflight_contract(contract)
                self.assertEqual(description["id"], expected["id"])

                with tempfile.TemporaryDirectory() as temp_dir:
                    changed = Path(temp_dir) / "docs/performance/contracts" / filename
                    changed.parent.mkdir(parents=True)
                    value = json.loads(contract.read_text(encoding="utf-8"))
                    value["output_kinds"][expected["group"]] = expected["wrong_kind"]
                    changed.write_text(json.dumps(value), encoding="utf-8")
                    with self.assertRaisesRegex(compare_self.ContractViolation, "differs"):
                        compare_self._describe_preflight_contract(changed)

    def test_ascii_contract_does_not_expand_the_legacy_pipeline_contract(self) -> None:
        pipeline_contract = ROOT / compare_self._NATIVE_CRITERION_PREFLIGHT_CONTRACT
        ascii_contract = ROOT / compare_self._NATIVE_ASCII_CRITERION_PREFLIGHT_CONTRACT
        pipeline = json.loads(pipeline_contract.read_text(encoding="utf-8"))
        ascii_only = json.loads(ascii_contract.read_text(encoding="utf-8"))

        self.assertNotIn("ascii_end_to_end", pipeline["output_kinds"])
        self.assertEqual(
            ascii_only["output_kinds"],
            {"ascii_end_to_end": "plain_ascii"},
        )

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
                    "bench_target": {"sha256": "3" * 64},
                    "bench_source": {"sha256": "4" * 64},
                    "corpus": {
                        "sha256": "5" * 64,
                        "preflight_receipts_required": True,
                        "preflight_contract": {"sha256": "6" * 64},
                    },
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

if __name__ == "__main__":
    sys.exit(unittest.main())
