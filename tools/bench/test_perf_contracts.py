#!/usr/bin/env python3
"""Contract tests for the corpus-driven performance helper scripts."""

from __future__ import annotations

import io
import sys
import unittest
from contextlib import redirect_stdout
from pathlib import Path

import perf_runner
import compare_self
import render_perf_comment
from corpus_utils import fixture_names_for_suite, load_corpus, select_corpus_fixtures


ROOT = Path(__file__).resolve().parents[2]
CORPUS_PATH = ROOT / "tools" / "bench" / "corpus.json"


class CorpusContractsTest(unittest.TestCase):
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
            "docs/performance/COMPARISON.perf-runner_"
            f"{perf_runner.today_stamp()}_full_suite_standard.md",
            out,
        )
        self.assertIn(
            f"target/bench/perf-runner/{perf_runner.today_stamp()}_full_suite_standard.json",
            out,
        )


class CompareSelfContractsTest(unittest.TestCase):
    def test_classifies_head_regression_at_fail_threshold(self) -> None:
        rows = compare_self.classify_rows(
            exact_benches=["end_to_end/flowchart_medium"],
            fixtures_by_name={"flowchart_medium": {"family": "flowchart"}},
            base={"times_ns": {"end_to_end/flowchart_medium": 100.0}},
            head={"times_ns": {"end_to_end/flowchart_medium": 112.0}},
            warn_threshold_percent=5.0,
            fail_threshold_percent=10.0,
        )

        self.assertEqual(rows[0].status, "fail")
        self.assertAlmostEqual(rows[0].change_percent or 0.0, 12.0)

    def test_classifies_head_missing_after_base_measured_as_failure(self) -> None:
        rows = compare_self.classify_rows(
            exact_benches=["end_to_end/mindmap_medium"],
            fixtures_by_name={"mindmap_medium": {"family": "mindmap"}},
            base={
                "times_ns": {"end_to_end/mindmap_medium": 100.0},
                "missing": [],
                "errors": {},
                "skipped": {},
            },
            head={
                "times_ns": {},
                "missing": ["end_to_end/mindmap_medium"],
                "errors": {},
                "skipped": {},
            },
            warn_threshold_percent=5.0,
            fail_threshold_percent=10.0,
        )

        self.assertEqual(rows[0].status, "fail")
        self.assertIn("head benchmark is missing", rows[0].reason)


class PerfCommentContractsTest(unittest.TestCase):
    def test_renders_warning_signal_rows(self) -> None:
        body = render_perf_comment.render_comment(
            {
                "summary": {
                    "gate_status": "pass",
                    "comparable": 2,
                    "failures": 0,
                    "warnings": 1,
                    "improvements": 1,
                    "geomean_change_percent": 1.23,
                },
                "selection": {"suite": "canary"},
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
        self.assertIn("`end_to_end/flowchart_medium`", body)
        self.assertIn("+6.20%", body)
        self.assertIn("https://example.test/run", body)

    def test_renders_missing_report_fallback(self) -> None:
        body = render_perf_comment.render_comment(
            None,
            run_url="https://example.test/run",
            artifact_name="perf-regression",
        )

        self.assertIn("Status: `report unavailable`", body)
        self.assertIn("workflow logs", body)


if __name__ == "__main__":
    sys.exit(unittest.main())
