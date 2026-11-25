#!/usr/bin/env python3
"""Contracts for the structured performance workflow descriptor."""

from __future__ import annotations

import io
import json
import os
import subprocess
import sys
import tempfile
import tomllib
import time
import unittest
from contextlib import chdir, redirect_stderr
from hashlib import sha256
from pathlib import Path
from unittest import mock
from unittest.mock import patch
from urllib.error import HTTPError
from urllib.parse import parse_qs, urlsplit

from merman_bench import ci as performance_workflow
from merman_bench import layout_calibration as calibration
from merman_bench import weekly as weekly


ROOT = Path(__file__).resolve().parents[2]


class LayoutCalibrationProcessContractsTest(unittest.TestCase):
    def test_timeout_terminates_the_managed_process_group(self) -> None:
        if not sys.platform.startswith(("darwin", "linux", "win32")):
            self.skipTest("supported hosts only")
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            started = root / "grandchild-started"
            orphan = root / "grandchild-survived"
            grandchild = (
                "import signal, time; from pathlib import Path; "
                "signal.signal(signal.SIGTERM, signal.SIG_IGN); "
                f"Path({str(started)!r}).write_text('started'); "
                "time.sleep(1.5); "
                f"Path({str(orphan)!r}).write_text('orphan'); "
                "time.sleep(60)"
            )
            parent = (
                "import subprocess, sys, time; "
                f"subprocess.Popen([sys.executable, '-c', {grandchild!r}]); "
                "time.sleep(60)"
            )

            result = calibration.run_managed_process(
                [sys.executable, "-c", parent],
                cwd=calibration.ROOT,
                timeout_seconds=1.0,
                termination_grace_seconds=0.1,
            )

            self.assertTrue(result["timed_out"])
            self.assertTrue(started.is_file(), "grandchild never started")
            time.sleep(1.7)
            self.assertFalse(orphan.exists(), "grandchild survived the timeout")

    def test_timeout_terminates_descendants_after_group_leader_exits(self) -> None:
        if not sys.platform.startswith(("darwin", "linux", "win32")):
            self.skipTest("supported hosts only")
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            started = root / "grandchild-started"
            orphan = root / "grandchild-survived"
            grandchild = (
                "import signal, time; from pathlib import Path; "
                "signal.signal(signal.SIGTERM, signal.SIG_IGN); "
                f"Path({str(started)!r}).write_text('started'); "
                "time.sleep(1.5); "
                f"Path({str(orphan)!r}).write_text('orphan'); "
                "time.sleep(60)"
            )
            parent = (
                "import subprocess, sys; "
                f"subprocess.Popen([sys.executable, '-c', {grandchild!r}])"
            )

            result = calibration.run_managed_process(
                [sys.executable, "-c", parent],
                cwd=calibration.ROOT,
                timeout_seconds=1.0,
                termination_grace_seconds=0.1,
            )

            self.assertTrue(result["timed_out"])
            self.assertEqual(result["returncode"], 0)
            self.assertTrue(started.is_file(), "grandchild never started")
            time.sleep(1.7)
            self.assertFalse(orphan.exists(), "grandchild survived the timeout")

    def test_timeout_terminates_descendants_that_close_output_pipes(self) -> None:
        if not sys.platform.startswith(("darwin", "linux", "win32")):
            self.skipTest("supported hosts only")
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            started = root / "grandchild-started"
            orphan = root / "grandchild-survived"
            grandchild = (
                "import os, signal, time; from pathlib import Path; "
                "sink = os.open(os.devnull, os.O_WRONLY); "
                "os.dup2(sink, 1); os.dup2(sink, 2); os.close(sink); "
                "signal.signal(signal.SIGTERM, signal.SIG_IGN); "
                f"Path({str(started)!r}).write_text('started'); "
                "time.sleep(1.5); "
                f"Path({str(orphan)!r}).write_text('orphan'); "
                "time.sleep(60)"
            )
            parent = (
                "import subprocess, sys, time; "
                f"subprocess.Popen([sys.executable, '-c', {grandchild!r}]); "
                "time.sleep(60)"
            )

            result = calibration.run_managed_process(
                [sys.executable, "-c", parent],
                cwd=calibration.ROOT,
                timeout_seconds=1.0,
                termination_grace_seconds=0.1,
            )

            self.assertTrue(result["timed_out"])
            self.assertTrue(started.is_file(), "grandchild never started")
            time.sleep(1.7)
            self.assertFalse(orphan.exists(), "grandchild survived the timeout")

    def test_windows_reads_peak_working_set_after_target_releases_memory(self) -> None:
        if sys.platform != "win32":
            self.skipTest("Windows only")
        result = calibration.run_managed_process(
            [sys.executable, "-c", "memory = bytearray(64 * 1024 * 1024); del memory; print('done')"],
            cwd=calibration.ROOT,
            timeout_seconds=10,
        )
        self.assertEqual(result["returncode"], 0)
        self.assertFalse(result["timed_out"])
        self.assertEqual(result["stdout"].strip(), "done")
        self.assertGreaterEqual(result["maximum_resident_set_size_bytes"], 64 * 1024 * 1024)

    def test_windows_preserves_target_exit_code_and_stderr(self) -> None:
        if sys.platform != "win32":
            self.skipTest("Windows only")
        result = calibration.run_managed_process(
            [sys.executable, "-c", "import sys; print('failed', file=sys.stderr); sys.exit(7)"],
            cwd=calibration.ROOT,
            timeout_seconds=10,
        )
        self.assertEqual(result["returncode"], 7)
        self.assertFalse(result["timed_out"])
        self.assertEqual(result["stderr"].strip(), "failed")

    def test_windows_launch_gate_prevents_execution_when_job_assignment_fails(self) -> None:
        if sys.platform != "win32":
            self.skipTest("Windows only")
        with tempfile.TemporaryDirectory() as directory:
            marker = Path(directory) / "must-not-run"
            command = [
                sys.executable,
                "-c",
                f"from pathlib import Path; Path({str(marker)!r}).write_text('ran')",
            ]
            with mock.patch.object(calibration.WindowsJob, "assign", side_effect=OSError("assignment failed")):
                with self.assertRaisesRegex(OSError, "assignment failed"):
                    calibration.run_managed_process(
                        command, cwd=calibration.ROOT, timeout_seconds=10
                    )
            self.assertFalse(marker.exists())

    def test_successful_windows_leader_cannot_leave_background_descendants(self) -> None:
        if sys.platform != "win32":
            self.skipTest("Windows only")
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            started = root / "grandchild-started"
            orphan = root / "grandchild-survived"
            grandchild = (
                "import time; from pathlib import Path; "
                f"Path({str(started)!r}).write_text('started'); "
                "time.sleep(1.0); "
                f"Path({str(orphan)!r}).write_text('orphan'); "
                "time.sleep(60)"
            )
            parent = (
                "import subprocess, sys, time; from pathlib import Path; "
                f"subprocess.Popen([sys.executable, '-c', {grandchild!r}], "
                "stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL); "
                f"marker = Path({str(started)!r}); "
                "\nwhile not marker.exists(): time.sleep(0.01)"
            )
            result = calibration.run_managed_process(
                [sys.executable, "-c", parent],
                cwd=calibration.ROOT,
                timeout_seconds=10,
            )
            self.assertEqual(result["returncode"], 0)
            self.assertFalse(result["timed_out"])
            self.assertTrue(started.is_file(), "grandchild never started")
            time.sleep(1.2)
            self.assertFalse(orphan.exists(), "grandchild survived successful completion")


class PerformanceWorkflowContractsTest(unittest.TestCase):


    def setUp(self) -> None:
        self.registry = performance_workflow.load_registry()

    def test_checked_in_registry_is_unique_and_references_existing_corpora(
        self,
    ) -> None:
        lanes = self.registry.lanes
        lane_ids = [str(lane["id"]) for lane in lanes]
        labels = [
            str(lane["pull_request_label"])
            for lane in lanes
            if lane["pull_request_label"] is not None
        ]

        self.assertTrue(lanes)
        self.assertEqual(len(lane_ids), len(set(lane_ids)))
        self.assertEqual(len(labels), len(set(labels)))
        for lane in lanes:
            with self.subTest(lane=lane["id"]):
                self.assertTrue((ROOT / str(lane["corpus"])).is_file())

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

    def test_select_cli_projects_the_registry_descriptor_to_the_matrix(self) -> None:
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
        self.assertEqual(entries["selected"], "false")
        matrix = json.loads(entries["matrix"])
        self.assertEqual(matrix, {"include": []})

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


class WeeklyWorkloadBaselineTest(unittest.TestCase):


    def setUp(self):
        self.env = dict(GITHUB_REPOSITORY="owner/project", GITHUB_REF_NAME="main", GITHUB_SHA="f" * 40,
                        GITHUB_RUN_NUMBER="150", GITHUB_RUN_ID="1150", GITHUB_RUN_ATTEMPT="1",
                        WORKLOAD_SOURCES="a" * 64, WORKLOAD_RECIPE="collect paired workloads",
                        WORKLOAD_RUNNER="ubuntu-24.04", RUNNER_ARCH="X64", WORKLOAD_TOOLCHAIN="rust@1.100.0")
        self.plan = {"schema_version": 1, "suite": "full", "scenarios": [
            {"id": weekly.SENTINEL, "requests": [{"diagram_id": "one"}, {"diagram_id": "two"}]},
            {"id": "retained-configs", "requests": [{"diagram_id": "retained"}]},
            {"id": "scc-100", "requests": [{"diagram_id": "limited"}]},
            {"id": "scc-75-declarations-2", "requests": [{"diagram_id": "newly-limited"}]},
            {"id": "incomplete-flow", "requests": [{"diagram_id": "broken", "expected_outcome": "reject"}]},
            {"id": "editor-recovery", "requests": [{"diagram_id": "before"},
             {"diagram_id": "edit", "expected_outcome": "reject"}, {"diagram_id": "after"}]},
        ], "topologies": [{"id": "scc-100"}]}
        self.recipe = weekly.protocol(self.plan, self.env)
        self.runs = []
        self.artifacts = {}
        self.archive_metadata = {}
        self.attempts = {}
        self.requests = []
        self.detached = set()

    def occurrence(self, number, sha, *, attempt=1, **changes):
        run = dict(id=1000 + number, run_number=number, run_attempt=attempt, workflow_id=42,
                   path=weekly.WORKFLOW, head_sha=sha, head_branch="main", event="schedule",
                   status="completed", conclusion="success", repository={"full_name": "owner/project"},
                   head_repository={"full_name": "owner/project"})
        run.update(changes)
        self.runs.append(run)
        self.artifacts[run["id"]] = []
        self.attempts[run["id"], attempt] = dict(run)
        return run

    def artifact(self, run, *, attempt=None, protocol=None, **changes):
        attempt = attempt or run["run_attempt"]
        artifact = dict(id=run["id"] * 100 + attempt, expired=False, size_in_bytes=1234,
                        digest="sha256:" + "e" * 64,
                        name=f"perf-public-workloads-weekly-{attempt}-{protocol or self.recipe['protocol']}")
        artifact.update(changes)
        self.artifacts[run["id"]].append(artifact)
        self.archive_metadata[artifact["id"]] = dict(artifact, workflow_run={
            k: run[k] for k in ("id", "head_sha", "head_branch")})
        self.attempts[run["id"], attempt] = dict(run, run_attempt=attempt)
        return artifact

    def get(self, url):
        self.requests.append(url)
        parsed = urlsplit(url)
        path = parsed.path.split("/")
        if path[-1] == "runs":
            query = parse_qs(parsed.query)
            self.assertEqual((query["branch"], query["event"], query["status"]),
                             (["main"], ["schedule"], ["completed"]))
            offset = (int(query["page"][0]) - 1) * 100
            return {"workflow_runs": self.runs[offset:offset + 100]}
        if path[2] == "artifacts":
            if int(path[3]) not in self.archive_metadata:
                raise HTTPError(url, 410, "expired", {}, None)
            return self.archive_metadata[int(path[3])]
        run_id = int(path[3])
        if path[-1] == "artifacts":
            return {"artifacts": self.artifacts[run_id]}
        self.assertEqual(path[4], "attempts")
        return self.attempts[run_id, int(path[5])]

    def select(self):
        return weekly.select(self.env, self.recipe, self.get, lambda base, head: base not in self.detached)

    def test_first_run_and_all_expired_reseed_at_the_current_head(self):
        first = self.select()
        self.assertEqual((first["kind"], first["base_sha"], first["previous"]), ("bootstrap", self.env["GITHUB_SHA"], None))
        self.artifact(self.occurrence(5, "5" * 40), expired=True)
        expired = self.select()
        self.assertEqual(expired["base_sha"], first["base_sha"])
        self.assertEqual(expired["skipped"], {"artifact-unavailable": 1})
        archive = self.artifact(self.occurrence(6, "6" * 40))
        del self.archive_metadata[archive["id"]]  # Disappeared after the run's listing was read.
        self.assertEqual(self.select()["skipped"], {"artifact-unavailable": 2})

    def test_history_pages_and_service_failures_do_not_masquerade_as_a_first_run(self):
        for number in range(110, 10, -1):
            self.occurrence(number, "d" * 40)
        self.artifact(self.occurrence(1, "1" * 40))
        self.assertEqual(self.select()["base_sha"], "1" * 40)
        self.assertTrue(any("page=2" in url for url in self.requests))

        def outage(path):
            raise HTTPError(path, 503, "service unavailable", {}, None)
        with self.assertRaises(HTTPError):
            weekly.select(self.env, self.recipe, outage)

    def test_git_ancestry_uses_the_measured_endpoint_across_multiple_commits(self):
        fixture = subprocess.run(["git", "-C", str(ROOT), "rev-list", "--first-parent", "--max-count=5", "HEAD"],
                                 text=True, capture_output=True, check=True).stdout.splitlines()
        if len(fixture) < 5:
            self.skipTest("requires five local commits for a multi-commit interval")
        head, parent, older = fixture[1], fixture[2], fixture[-1]
        self.env["GITHUB_SHA"] = head
        self.artifact(self.occurrence(1, older))
        self.artifact(self.occurrence(2, fixture[0]))  # Present locally but not an ancestor of the selected head.
        with chdir(ROOT):
            interval = weekly.select(self.env, self.recipe, self.get)
        self.assertEqual((interval["base_sha"], interval["head_sha"]), (older, head))
        self.assertNotEqual(interval["base_sha"], parent)
        self.assertEqual(interval["skipped"], {"non-ancestor": 1})

    def write_pairs(self, root, interval):
        from merman_bench.workloads import compare

        def write(name, value):
            (root / name).write_text(json.dumps(value), encoding="utf-8")
        write("plan.json", self.plan)
        plan_sha = sha256((root / "plan.json").read_bytes()).hexdigest()
        for metric in ("memory", "work", "retained"):
            sides = []
            for side in ("base", "head"):
                work = metric == "work"
                operation = {"work": "layout-work", "memory": "svg-data", "retained": "prepare-retained"}[metric]
                options = dict(metric="work" if work else "memory", operation=operation, samples=1 if work else 3)
                rows = {}
                for case in self.plan["topologies" if work else "scenarios"]:
                    if work:
                        rejected = dict(status="layout_work_rejected", id="max_layout_work_units",
                                        phase="layout_model", actual=15, maximum=10)
                        accepted = dict(status="accepted", bytes=80, sha256="a" * 64)
                        rows[case["id"]] = dict(default_budget=10, high_budget=100, default=rejected, high=accepted,
                                                svg=dict(default=rejected, high=accepted), boundary=dict(minimum_passing_budget=15))
                    elif metric == "memory" or case["id"] == "retained-configs":
                        expectations = [r.get("expected_outcome", "draw") for r in case["requests"]]
                        success = dict(bytes=32, observed_size=32, sha256="d" * 64)
                        rejection = dict(status="expected-rejection", message="unexpected end of input")
                        outputs = [rejection if outcome == "reject" else success for outcome in expectations]
                        samples = [dict(allocated_bytes=128 if side == "base" else 256,
                                        live_delta_bytes=64, after_release_delta_bytes=0)] * 3
                        row = dict(completed=len(outputs), outputs=outputs, expected_outcomes=expectations, samples=samples)
                        if "reject" in expectations:
                            row.update(status="expected-rejection", samples=[], completed=expectations.count("draw"),
                                       expected_rejections=expectations.count("reject"))
                        if case["id"] == "scc-100" or (case["id"] == "scc-75-declarations-2" and side == "head"):
                            row.update(status="failed", samples=[], completed=0,
                                       outputs=[dict(status="render-error", message="layout work exceeded")])
                        rows[case["id"]] = row
                report = dict(schema_version=1, surface="rust", plan_sha256=plan_sha, options=options,
                              revision=interval[side + "_sha"], host=["Linux", "x86_64", "runner"],
                              harness_sha256="a" * 64, runner_sha256="b" * 64, rows=rows)
                write(f"{side}-{metric}.json", report)
                sides.append(report)
            write(f"compare-{metric}.json", compare(*sides))
        return plan_sha

    def test_published_receipt_requires_the_complete_paired_measurement(self):
        self.artifact(self.occurrence(4, "4" * 40))
        interval = self.select()
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            output = root / "github-output"
            env = dict(self.env, WEEKLY_INTERVAL=json.dumps(interval), WORKLOAD_RESULT="success",
                       GITHUB_OUTPUT=str(output), GITHUB_STEP_SUMMARY=str(root / "summary"))
            with patch.dict(os.environ, env), redirect_stderr(io.StringIO()):
                for problem in (None, "missing-side", "skipped-step", "wrong-revision", "partial-samples",
                                "partial-coverage", "harness-changed", "uncomparable-sentinel", "missing-outcome",
                                "unrecognized-error", "measurement-mismatch", "missing-work-outcome", "missing-retention",
                                "reordered-rejection"):
                    with self.subTest(problem=problem):
                        plan_sha = self.write_pairs(root, interval)
                        os.environ["WORKLOAD_RESULT"] = "skipped" if problem == "skipped-step" else "success"
                        output.write_text("", encoding="utf-8")
                        if problem == "missing-side":
                            (root / "base-work.json").unlink()
                        if problem in ("wrong-revision", "partial-samples", "partial-coverage"):
                            path = root / "base-memory.json"
                            report = json.loads(path.read_bytes())
                            if problem == "wrong-revision":
                                report["revision"] = interval["head_sha"]
                            elif problem == "partial-samples":
                                report["rows"][weekly.SENTINEL]["samples"].pop()
                            else:
                                report["rows"].pop("retained-configs")
                            path.write_text(json.dumps(report), encoding="utf-8")
                        if problem in ("harness-changed", "uncomparable-sentinel"):
                            path = root / "compare-memory.json"
                            report = json.loads(path.read_bytes())
                            if problem == "harness-changed":
                                report.update(classification="harness-changed", rows={})
                            else:
                                report["rows"][weekly.SENTINEL] = {"status": "output-changed"}
                            path.write_text(json.dumps(report), encoding="utf-8")
                        if problem in ("missing-outcome", "unrecognized-error", "measurement-mismatch"):
                            path = root / "head-memory.json"
                            report = json.loads(path.read_bytes())
                            row = report["rows"]["scc-100"]
                            if problem == "missing-outcome":
                                row["outputs"].clear()
                            elif problem == "unrecognized-error":
                                row["outputs"][0]["status"] = "unobserved"
                            else:
                                row["status"] = "measurement-mismatch"
                            path.write_text(json.dumps(report), encoding="utf-8")
                        if problem == "missing-work-outcome":
                            path = root / "head-work.json"
                            report = json.loads(path.read_bytes())
                            del report["rows"]["scc-100"]["svg"]["default"]["status"]
                            path.write_text(json.dumps(report), encoding="utf-8")
                        if problem == "reordered-rejection":
                            path = root / "head-memory.json"
                            report = json.loads(path.read_bytes())
                            outputs = report["rows"]["editor-recovery"]["outputs"]
                            outputs[0], outputs[1] = outputs[1], outputs[0]
                            path.write_text(json.dumps(report), encoding="utf-8")
                        if problem == "missing-retention":
                            from merman_bench.workloads import compare
                            head = json.loads((root / "head-retained.json").read_bytes())
                            head["rows"]["retained-configs"]["outputs"][0]["sha256"] = "e" * 64
                            (root / "head-retained.json").write_text(json.dumps(head))
                            base = json.loads((root / "base-retained.json").read_bytes())
                            (root / "compare-retained.json").write_text(json.dumps(compare(base, head)))
                        self.assertEqual(weekly.main(["record", temp]), int(problem is not None))
                        receipt = json.loads((root / "weekly-interval.json").read_bytes())
                        if problem:
                            self.assertEqual((receipt["outcome"], output.read_text()), ("incomplete", ""))
                        else:
                            self.assertEqual((receipt["plan_sha256"], receipt["outcome"]), (plan_sha, "complete"))
                            self.assertEqual(receipt["base_sha"], "4" * 40)
                            memory = receipt["coverage"]["memory"]
                            self.assertEqual(memory["comparisons"]["failed"], 1)
                            self.assertEqual(memory["comparisons"]["outcome-changed"], 1)
                            self.assertNotIn("scc-75-declarations-2", memory["base"]["exceptions"])
                            self.assertIn("scc-75-declarations-2", (root / "summary").read_text())
                            self.assertEqual(memory["head"]["exceptions"]["scc-100"], {"render-error": 1})
                            self.assertEqual(memory["head"]["exceptions"]["incomplete-flow"], {"expected-rejection": 1})
                            self.assertEqual(memory["head"]["exceptions"]["editor-recovery"], {"ok": 2, "expected-rejection": 1})
                            self.assertEqual(receipt["coverage"]["work"]["head"]["exceptions"]["scc-100"]["svg_default"],
                                             "layout_work_rejected")
                            self.assertEqual(receipt["sentinel_sha256"], sha256((root / "weekly-sentinel.json").read_bytes()).hexdigest())
                            self.assertIn("artifact=perf-public-workloads-weekly-1-" + self.recipe["protocol"], output.read_text())
