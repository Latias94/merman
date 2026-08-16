#!/usr/bin/env python3
"""Contracts for the structured performance workflow descriptor."""

from __future__ import annotations

import io
import json
import tempfile
import unittest
from contextlib import redirect_stderr
from pathlib import Path

import performance_workflow


ROOT = Path(__file__).resolve().parents[2]


class PerformanceWorkflowContractsTest(unittest.TestCase):
    def setUp(self) -> None:
        self.registry = performance_workflow.load_registry()

    def test_registry_owns_the_measurement_recipes(self) -> None:
        lanes = self.registry.by_id

        self.assertEqual(tuple(lanes), ("regression", "ascii", "frontmatter"))
        self.assertEqual(
            {
                lane_id: (
                    lane["pull_request_label"],
                    lane["suite"],
                    lane["group"],
                    lane["bench"],
                    lane["corpus"],
                    lane["features"],
                    lane["default_features"],
                    lane["artifact"],
                )
                for lane_id, lane in lanes.items()
            },
            {
                "regression": (
                    "perf",
                    "canary",
                    "",
                    "pipeline",
                    "tools/bench/corpus.json",
                    "svg",
                    True,
                    "perf-regression",
                ),
                "ascii": (
                    "perf-ascii",
                    "comparable",
                    "ascii_end_to_end",
                    "ascii_pipeline",
                    "tools/bench/ascii_corpus.json",
                    "ascii",
                    False,
                    "perf-ascii",
                ),
                "frontmatter": (
                    "perf-frontmatter",
                    "frontmatter",
                    "frontmatter_preprocess",
                    "pipeline",
                    "tools/bench/corpus.json",
                    "svg",
                    True,
                    "perf-frontmatter",
                ),
            },
        )
        self.assertTrue(
            all((ROOT / str(lane["corpus"])).is_file() for lane in lanes.values())
        )

    def test_event_selection_is_owned_by_the_registry(self) -> None:
        select = performance_workflow.select_lane_ids

        self.assertEqual(
            select(
                self.registry,
                event_name="pull_request",
                labels=frozenset({"perf-frontmatter", "unrelated", "perf"}),
            ),
            ("regression", "frontmatter"),
        )
        self.assertEqual(
            select(self.registry, event_name="pull_request"),
            (),
        )
        self.assertEqual(
            select(self.registry, event_name="schedule"),
            ("regression", "frontmatter"),
        )
        for run, expected in {
            "contracts": (),
            "reference": (),
            "regression": ("regression",),
            "ascii": ("ascii",),
            "frontmatter": ("frontmatter",),
            "full": ("regression", "ascii", "frontmatter"),
        }.items():
            with self.subTest(run=run):
                self.assertEqual(
                    select(
                        self.registry,
                        event_name="workflow_dispatch",
                        dispatch_run=run,
                    ),
                    expected,
                )

        with self.assertRaisesRegex(
            performance_workflow.WorkflowContractError,
            "unsupported performance lane",
        ):
            select(
                self.registry,
                event_name="workflow_dispatch",
                dispatch_run="unknown",
            )
        with self.assertRaisesRegex(
            performance_workflow.WorkflowContractError,
            "unsupported performance event",
        ):
            select(self.registry, event_name="push")

    def test_select_cli_writes_the_matrix_and_selected_bit(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            output = Path(temp_dir) / "github-output"
            result = performance_workflow.main(
                [
                    "select",
                    "--event",
                    "pull_request",
                    "--labels-json",
                    '["perf-ascii"]',
                    "--github-output",
                    str(output),
                ]
            )
            entries = dict(
                line.split("=", 1)
                for line in output.read_text(encoding="utf-8").splitlines()
            )

        self.assertEqual(result, 0)
        self.assertEqual(entries["selected"], "true")
        matrix = json.loads(entries["matrix"])
        self.assertEqual([lane["id"] for lane in matrix["include"]], ["ascii"])
        self.assertNotIn("pull_request_label", matrix["include"][0])

    def test_registry_rejects_unknown_lane_references(self) -> None:
        payload = json.loads(
            performance_workflow.DEFAULT_REGISTRY.read_text(encoding="utf-8")
        )
        payload["scheduled_lanes"].append("unknown")
        with tempfile.TemporaryDirectory() as temp_dir:
            path = Path(temp_dir) / "lanes.json"
            path.write_text(json.dumps(payload), encoding="utf-8")
            with self.assertRaisesRegex(
                performance_workflow.WorkflowContractError,
                "unknown lanes",
            ):
                performance_workflow.load_registry(path)

    def test_result_enforcement_preserves_producer_exit_semantics(self) -> None:
        for comparison_exit in range(4):
            error = io.StringIO()
            with self.subTest(comparison_exit=comparison_exit):
                self.assertEqual(
                    performance_workflow.enforce_measurement_result(
                        comparison_exit=comparison_exit,
                        render_exit=0,
                        error=error,
                    ),
                    comparison_exit,
                )
        render_error = io.StringIO()
        self.assertEqual(
            performance_workflow.enforce_measurement_result(
                comparison_exit=0,
                render_exit=1,
                error=render_error,
            ),
            2,
        )
        self.assertIn("report consumer rejected", render_error.getvalue())
        unexpected = io.StringIO()
        self.assertEqual(
            performance_workflow.enforce_measurement_result(
                comparison_exit=9,
                render_exit=0,
                error=unexpected,
            ),
            2,
        )
        self.assertIn(
            "Unexpected performance comparison exit code",
            unexpected.getvalue(),
        )

        cli_error = io.StringIO()
        with redirect_stderr(cli_error):
            self.assertEqual(
                performance_workflow.main(
                    ["enforce", "--comparison-exit", "3", "--render-exit", "0"]
                ),
                3,
            )
        self.assertIn("statistically inconclusive", cli_error.getvalue())


if __name__ == "__main__":
    unittest.main()
