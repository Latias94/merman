#!/usr/bin/env python3
"""Contracts for the structured performance workflow descriptor."""

from __future__ import annotations

import io
import json
import os
import subprocess
import tempfile
import tomllib
import unittest
from contextlib import chdir, redirect_stderr
from hashlib import sha256
from pathlib import Path
from unittest.mock import patch
from urllib.error import HTTPError
from urllib.parse import parse_qs, urlsplit

import performance_workflow
import weekly_workload_baseline as weekly


ROOT = Path(__file__).resolve().parents[2]


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

    def test_current_ascii_lane_selects_full_diagram_coverage_without_defaults(self) -> None:
        lane = self.registry.by_id["ascii"]
        self.assertEqual(lane["features"], "all-diagrams,ascii")
        self.assertIs(lane["default_features"], False)

    def test_current_pipeline_lanes_explicitly_enable_the_full_bench(self) -> None:
        manifest = tomllib.loads(
            (ROOT / "crates/merman/Cargo.toml").read_text(encoding="utf-8")
        )
        required = next(
            set(bench["required-features"])
            for bench in manifest["bench"]
            if bench["name"] == "pipeline"
        )
        for lane in self.registry.lanes:
            if lane["bench"] == "pipeline":
                with self.subTest(lane=lane["id"]):
                    enabled = set(lane["features"].split(","))
                    self.assertIn("all-diagrams", enabled)
                    self.assertTrue(required.issubset(enabled))

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

    def test_only_scheduled_regression_uses_the_workload_receipt(self) -> None:
        workflow = (ROOT / ".github/workflows/performance.yml").read_text(encoding="utf-8")
        job = workflow.split("\n  measurement:\n", 1)[1].split("\n  weekly-baseline:\n", 1)[0]
        resolution = job.split("id: scheduled-base\n", 1)[1].split("\n      - name:", 1)[0]
        self.assertIn("if: github.event_name == 'schedule'", resolution)
        self.assertIn("LANE: ${{ matrix.id }}", resolution)
        measured, parent = resolution.split("          else\n", 1)
        self.assertIn('if [[ "$LANE" == regression ]]; then', measured)
        self.assertIn('sha="${MEASURED_BASE:?', measured)
        self.assertIn('sha="$(git rev-parse HEAD^)"', parent)
        self.assertIn("matrix.id != 'regression' && 2 || 1", job)
        self.assertIn("github.event.pull_request.base.sha || steps.scheduled-base.outputs.sha || inputs.base_ref", job)
        self.assertIn("github.event.pull_request.base.ref || steps.scheduled-base.outputs.sha || inputs.base_ref", job)
        self.assertNotIn("needs.weekly-baseline.result", job.split("    steps:", 1)[0])

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

    def test_last_measured_interval_skips_gaps_and_unrelated_check_identities(self):
        older = self.occurrence(8, "a" * 40, conclusion="failure")  # Another job can fail after measurement.
        archive = self.artifact(older)
        self.artifact(self.occurrence(10, "b" * 40), name="perf-public-workloads-incomplete-1")
        self.artifact(self.occurrence(11, "c" * 40), expired=True)
        self.artifact(self.occurrence(12, "d" * 40))
        self.detached.add("d" * 40)
        self.occurrence(13, "e" * 40)  # Green check, no measured artifact.
        for number, overrides in enumerate((dict(event="pull_request"), dict(event="workflow_dispatch"),
                dict(head_branch="topic"), dict(path=".github/workflows/ci.yml"), dict(status="in_progress"),
                dict(head_repository={"full_name": "fork/project"})), start=14):
            self.artifact(self.occurrence(number, self.env["GITHUB_SHA"], **overrides))
        self.artifact(self.occurrence(150, self.env["GITHUB_SHA"], attempt=2))
        self.artifact(self.occurrence(151, self.env["GITHUB_SHA"]))  # Future run on an old run's rerun.
        result = self.select()
        self.assertEqual((result["kind"], result["base_sha"], result["previous"]["artifact_id"]),
                         ("interval", older["head_sha"], archive["id"]))
        self.assertEqual(result["skipped"], {"no-receipt": 1, "non-ancestor": 1,
                                           "artifact-unavailable": 1, "incomplete-measurement": 1})

    def test_polling_on_the_same_sha_binds_the_exact_completed_attempt(self):
        run = self.occurrence(75, self.env["GITHUB_SHA"], attempt=3)
        archive = self.artifact(run, attempt=1)
        self.artifact(run, attempt=2)
        self.attempts[run["id"], 2]["run_attempt"] = 3  # Check name and SHA are identical.
        result = self.select()
        self.assertEqual(result["kind"], "same-head")
        self.assertEqual((result["previous"]["run_attempt"], result["previous"]["artifact_id"]), (1, archive["id"]))
        self.assertEqual(result["skipped"], {"attempt-mismatch": 1})
        self.assertEqual(self.select(), result)
        self.archive_metadata[archive["id"]]["workflow_run"]["id"] += 1
        self.assertEqual(self.select()["kind"], "bootstrap")

    def test_protocol_change_starts_a_new_series_even_if_an_older_protocol_matches(self):
        self.artifact(self.occurrence(1, "1" * 40))
        self.artifact(self.occurrence(2, "2" * 40), protocol="v2-" + "b" * 64)
        result = self.select()
        self.assertEqual((result["kind"], result["base_sha"]), ("protocol-change", self.env["GITHUB_SHA"]))
        self.assertEqual(result["previous"]["head_sha"], "2" * 40)
        self.plan["scenarios"].append({"id": "new-full-case"})
        self.assertEqual(weekly.protocol(self.plan, self.env), self.recipe)
        self.plan["scenarios"][0]["requests"].reverse()
        self.assertNotEqual(weekly.protocol(self.plan, self.env), self.recipe)
        self.plan["scenarios"][0]["requests"].reverse()
        for field in ("WORKLOAD_SOURCES", "WORKLOAD_RECIPE", "WORKLOAD_RUNNER", "RUNNER_ARCH", "WORKLOAD_TOOLCHAIN"):
            with self.subTest(field=field):
                self.assertNotEqual(weekly.protocol(self.plan, dict(self.env, **{field: "9" * 64})), self.recipe)

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
        from run_workloads import compare

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
                            from run_workloads import compare
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


if __name__ == "__main__":
    unittest.main()
