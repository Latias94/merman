#!/usr/bin/env python3
"""Contract tests for the corpus-driven performance helper scripts."""

from __future__ import annotations

import io
import sys
import unittest
from contextlib import redirect_stdout
from pathlib import Path

import perf_runner
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
        out = buf.getvalue()
        self.assertIn(
            "stage spotcheck (flowchart_medium,class_medium,mindmap_medium,architecture_medium)",
            out,
        )
        self.assertIn("compare_mermaid_renderers.py --preset long --suite canary", out)
        self.assertIn("--skip-mermaid-js", out)

    def test_triage_dry_run_includes_cold_parse_steps(self) -> None:
        buf = io.StringIO()

        with redirect_stdout(buf):
            result = perf_runner.main(
                ["--profile", "triage", "--include-cold-parse", "--dry-run"]
            )

        self.assertEqual(result, 0)
        out = buf.getvalue()
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
        out = buf.getvalue()
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


if __name__ == "__main__":
    sys.exit(unittest.main())
