#!/usr/bin/env python3
"""Contract tests for the corpus-driven performance helper scripts."""

from __future__ import annotations

import io
import json
import stat
import subprocess
import sys
import tempfile
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from unittest import mock

import compare_mermaid_renderers
import render_perf_comment
import stage_spotcheck
from corpus_utils import compare_mmdr_fixture_inputs


ROOT = Path(__file__).resolve().parents[2]


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

if __name__ == "__main__":
    sys.exit(unittest.main())
