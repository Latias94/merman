#!/usr/bin/env python3
"""Prove that job history authorizes only the first actual upload."""

from __future__ import annotations

import copy
import io
import json
import subprocess
import unittest
from contextlib import redirect_stdout
from unittest import mock

from scripts import release_attempt_history as history


class PublishHistoryTests(unittest.TestCase):
    def page(self, *, job_conclusion="failure", step_conclusion="skipped") -> list[dict]:
        return [{"total_count": 1, "jobs": [{
            "id": 11, "name": "publish", "status": "completed", "conclusion": job_conclusion,
            "steps": [
                {"name": "Install pinned npm", "status": "completed", "conclusion": "failure"},
                {"name": "Publish npm package group", "status": "completed", "conclusion": step_conclusion},
            ],
        }]}]

    def cancelled_page(self) -> list[dict]:
        page = self.page(job_conclusion="cancelled")
        page[0]["jobs"][0].update(runner_id=None, runner_name=None, steps=[])
        return page

    def state(self, *attempts) -> str:
        with mock.patch.object(history, "read_api", side_effect=[
            {"path": ".github/workflows/release-web.yml"}, *attempts,
        ]) as read:
            result = history.prior_publish_state("example/project", 42, len(attempts) + 1, "web")
        self.assertTrue(all(call.kwargs.get("paginate") for call in read.call_args_list[1:]))
        return result

    def test_initial_attempt_requires_no_history(self) -> None:
        with mock.patch.object(history, "read_api") as read:
            self.assertEqual(history.prior_publish_state("example/project", 42, 1, "web"), "never-attempted")
        read.assert_not_called()

    def test_failed_setup_with_explicitly_skipped_publish_allows_first_upload(self) -> None:
        self.assertEqual(self.state(self.page()), "never-attempted")

    def test_skipped_job_allows_first_upload(self) -> None:
        page = self.page(job_conclusion="skipped")
        page[0]["jobs"][0].pop("steps")
        self.assertEqual(self.state(page), "never-attempted")

    def test_cancelled_job_without_runner_or_steps_allows_first_upload(self) -> None:
        self.assertEqual(self.state(self.cancelled_page()), "never-attempted")

    def test_cancelled_job_requires_explicit_runner_and_steps_fields(self) -> None:
        for field in ("runner_id", "runner_name", "steps"):
            with self.subTest(field=field):
                page = self.cancelled_page()
                page[0]["jobs"][0].pop(field)
                self.assertEqual(self.state(page), "unknown")

    def test_cancelled_job_rejects_assigned_or_malformed_runner_fields(self) -> None:
        for field, value in (
            ("runner_id", 11), ("runner_name", "GitHub Actions 11"),
            ("runner_id", 0), ("runner_id", False), ("runner_id", True),
            ("runner_name", ""), ("runner_name", False), ("runner_name", 0),
        ):
            with self.subTest(field=field, value=value):
                page = self.cancelled_page()
                page[0]["jobs"][0][field] = value
                self.assertEqual(self.state(page), "unknown")

    def test_cancelled_job_rejects_nonempty_or_malformed_steps(self) -> None:
        for steps in (self.page()[0]["jobs"][0]["steps"], None, {}, False):
            with self.subTest(steps=steps):
                page = self.cancelled_page()
                page[0]["jobs"][0]["steps"] = steps
                self.assertEqual(self.state(page), "unknown")

    def test_cancelled_job_must_be_completed(self) -> None:
        for status in ("queued", "in_progress", "waiting", None):
            with self.subTest(status=status):
                page = self.cancelled_page()
                page[0]["jobs"][0]["status"] = status
                self.assertEqual(self.state(page), "unknown")

    def test_cancelled_job_cannot_hide_incomplete_or_unknown_history(self) -> None:
        page = self.cancelled_page()
        page[0]["total_count"] = 2
        self.assertEqual(self.state(page), "unknown")
        self.assertEqual(self.state(
            self.cancelled_page(), self.page(step_conclusion="failure"),
        ), "unknown")

    def test_started_failed_cancelled_or_successful_upload_is_unknown(self) -> None:
        for conclusion in ["success", "failure", "cancelled", None]:
            with self.subTest(conclusion=conclusion):
                self.assertEqual(self.state(self.page(step_conclusion=conclusion)), "unknown")
        page = self.page(job_conclusion="cancelled")
        self.assertEqual(self.state(page), "unknown")

    def test_missing_publish_step_is_not_proof_of_a_skipped_upload(self) -> None:
        page = self.page()
        page[0]["jobs"][0]["steps"] = []
        self.assertEqual(self.state(page), "unknown")

    def test_earlier_upload_cannot_be_hidden_by_a_later_skipped_attempt(self) -> None:
        self.assertEqual(self.state(self.page(step_conclusion="failure"), self.page()), "unknown")

    def test_every_earlier_attempt_must_prove_no_upload(self) -> None:
        self.assertEqual(self.state(self.page(), self.page()), "never-attempted")

    def test_incomplete_paginated_history_is_unknown(self) -> None:
        page = self.page()
        page[0]["total_count"] = 2
        self.assertEqual(self.state(page), "unknown")
        page = self.page()
        page[0]["jobs"].append(copy.deepcopy(page[0]["jobs"][0]))
        page[0]["total_count"] = 2
        self.assertEqual(self.state(page), "unknown")

    def test_complete_paginated_history_finds_the_publish_job(self) -> None:
        pages = self.page()
        pages[0]["total_count"] = 2
        pages.insert(0, {"total_count": 2, "jobs": [{"id": 10, "name": "build"}]})
        self.assertEqual(self.state(pages), "never-attempted")

    def test_wrong_workflow_or_failed_api_cannot_authorize_upload(self) -> None:
        with mock.patch.object(history, "read_api", return_value={"path": ".github/workflows/ci.yml"}):
            self.assertEqual(history.prior_publish_state("example/project", 42, 2, "web"), "unknown")
        with mock.patch.object(history, "read_api", side_effect=subprocess.CalledProcessError(1, "gh api")):
            self.assertEqual(history.prior_publish_state("example/project", 42, 2, "web"), "unknown")

    def test_grammar_inspection_failure_does_not_count_as_an_upload(self) -> None:
        for owner in ("grammar-npm", "grammar-crates", "crates", "flutter"):
            workflow, job, step = history.OWNERS[owner]
            pages = [{"total_count": 1, "jobs": [{
                "id": 11, "name": job, "status": "completed", "conclusion": "failure",
                "steps": [
                    {"name": "Inspect publication state", "status": "completed", "conclusion": "failure"},
                    {"name": step, "status": "completed", "conclusion": "skipped"},
                ],
            }]}]
            with self.subTest(owner=owner), mock.patch.object(history, "read_api", side_effect=[
                {"path": f".github/workflows/{workflow}"}, pages,
            ]):
                self.assertEqual(history.prior_publish_state("example/project", 42, 2, owner), "never-attempted")

    def test_api_client_is_read_only_and_collects_every_page(self) -> None:
        with mock.patch.object(history.subprocess, "run", return_value=subprocess.CompletedProcess([], 0, "[]")) as run:
            self.assertEqual(history.read_api("repos/example/project/actions/runs/42/jobs", paginate=True), [])
        self.assertEqual(run.call_args.args[0][:2], ["gh", "api"])
        self.assertIn("--paginate", run.call_args.args[0])
        self.assertIn("--slurp", run.call_args.args[0])
        self.assertNotIn("--method", run.call_args.args[0])


class NpmRecoverySelectionTests(unittest.TestCase):
    repository = "example/project"

    def run_record(self, run_id=42, attempt=3, *, owner="node", status="in_progress") -> dict:
        return {
            "id": run_id, "path": f".github/workflows/release-{owner}.yml",
            "repository": {"full_name": self.repository}, "event": "workflow_dispatch",
            "run_attempt": attempt, "status": status,
        }

    def report(self, attempt=1, *, artifact_id=101, owner="node", expired=False) -> dict:
        return {
            "id": artifact_id, "name": f"merman-{owner}-npm-reconciliation-report-attempt-{attempt}",
            "expired": expired,
        }

    def jobs(self, attempt, *, conclusion="skipped", owner="node") -> list[dict]:
        return [{"total_count": 1, "jobs": [{
            "id": 200 + attempt, "name": history.OWNERS[owner][1],
            "status": "completed", "conclusion": "failure",
            "steps": [{"name": history.OWNERS[owner][2], "status": "completed",
                       "conclusion": conclusion}],
        }]}]

    def select(self, responses, *, attempt=3, ancestor=None, owner="node"):
        prefix = f"repos/{self.repository}/actions/runs/"

        def read(path, *, paginate=False):
            self.assertTrue(path.startswith(prefix), path)
            self.assertEqual(paginate, path.endswith("?per_page=100"), path)
            return responses[path.removeprefix(prefix)]

        with mock.patch.object(history, "read_api", side_effect=read) as api:
            selected = history.select_npm_recovery(
                self.repository, 42, attempt, owner, ancestor_run_id=ancestor,
            )
        return selected, api

    def test_first_attempt_needs_no_history(self) -> None:
        selected, api = self.select({}, attempt=1)
        self.assertEqual(selected, history.NpmRecovery(observe_only=False))
        api.assert_not_called()

    def test_skipped_setup_gap_selects_original_report_identity_for_both_owners(self) -> None:
        for owner in ("node", "web"):
            with self.subTest(owner=owner):
                selected, api = self.select({
                    "42": self.run_record(owner=owner),
                    "42/artifacts?per_page=100": [{"total_count": 1, "artifacts": [self.report(owner=owner)]}],
                    "42/attempts/2/jobs?per_page=100": self.jobs(2, owner=owner),
                }, owner=owner)
                self.assertEqual(selected, history.NpmRecovery(101, 42, 1, observe_only=False))
                self.assertEqual(api.call_count, 3)

    def test_nearest_report_covers_earlier_upload_history(self) -> None:
        selected, api = self.select({
            "42": self.run_record(),
            "42/artifacts?per_page=100": [{"total_count": 2, "artifacts": [
                self.report(), self.report(2, artifact_id=102),
            ]}],
        })
        self.assertEqual(selected, history.NpmRecovery(102, 42, 2, observe_only=False))
        self.assertEqual(api.call_count, 2)

    def test_unknown_upload_gap_cannot_select_older_or_ancestor_report(self) -> None:
        for conclusion in ("success", "failure", "cancelled", None):
            with self.subTest(conclusion=conclusion):
                selected, api = self.select({
                    "42": self.run_record(),
                    "42/artifacts?per_page=100": [{"total_count": 1, "artifacts": [self.report()]}],
                    "42/attempts/2/jobs?per_page=100": self.jobs(2, conclusion=conclusion),
                }, ancestor=41)
                self.assertEqual(selected, history.NpmRecovery())
                self.assertEqual(api.call_count, 3)

    def test_missing_job_or_upload_step_cannot_bridge_a_gap(self) -> None:
        missing_step = self.jobs(2)
        missing_step[0]["jobs"][0]["steps"] = []
        for pages in ([{"total_count": 0, "jobs": []}], missing_step):
            with self.subTest(pages=pages):
                selected, _ = self.select({
                    "42": self.run_record(),
                    "42/artifacts?per_page=100": [{"total_count": 1, "artifacts": [self.report()]}],
                    "42/attempts/2/jobs?per_page=100": pages,
                })
                self.assertEqual(selected, history.NpmRecovery())

    def test_all_prior_uploads_skipped_allow_first_send_in_same_run(self) -> None:
        selected, _ = self.select({
            "42": self.run_record(),
            "42/artifacts?per_page=100": [{"total_count": 0, "artifacts": []}],
            "42/attempts/2/jobs?per_page=100": self.jobs(2),
            "42/attempts/1/jobs?per_page=100": self.jobs(1),
        })
        self.assertEqual(selected, history.NpmRecovery(observe_only=False))

    def test_ancestor_report_requires_every_current_upload_skipped(self) -> None:
        selected, api = self.select({
            "42": self.run_record(),
            "42/artifacts?per_page=100": [{"total_count": 0, "artifacts": []}],
            "42/attempts/2/jobs?per_page=100": self.jobs(2),
            "42/attempts/1/jobs?per_page=100": self.jobs(1),
            "41": self.run_record(41, 2, status="completed"),
            "41/artifacts?per_page=100": [{"total_count": 1, "artifacts": [self.report()]}],
            "41/attempts/2/jobs?per_page=100": self.jobs(2),
        }, ancestor=41)
        self.assertEqual(selected, history.NpmRecovery(101, 41, 1, observe_only=False))
        self.assertEqual(api.call_count, 7)

    def test_first_recovery_attempt_selects_ancestor_report(self) -> None:
        selected, _ = self.select({
            "41": self.run_record(41, 1, status="completed"),
            "41/artifacts?per_page=100": [{"total_count": 1, "artifacts": [self.report()]}],
        }, attempt=1, ancestor=41)
        self.assertEqual(selected, history.NpmRecovery(101, 41, 1, observe_only=False))

    def test_reportless_ancestor_is_observation_only_even_when_all_uploads_skipped(self) -> None:
        selected, _ = self.select({
            "41": self.run_record(41, 1, status="completed"),
            "41/artifacts?per_page=100": [{"total_count": 0, "artifacts": []}],
            "41/attempts/1/jobs?per_page=100": self.jobs(1),
        }, attempt=1, ancestor=41)
        self.assertEqual(selected, history.NpmRecovery())

    def test_complete_pagination_finds_report_and_skipped_job_on_later_pages(self) -> None:
        jobs = self.jobs(2)
        jobs[0]["total_count"] = 2
        jobs.insert(0, {"total_count": 2, "jobs": [{"id": 7, "name": "build"}]})
        selected, _ = self.select({
            "42": self.run_record(),
            "42/artifacts?per_page=100": [
                {"total_count": 2, "artifacts": [{"id": 9, "name": "package-group"}]},
                {"total_count": 2, "artifacts": [self.report()]},
            ],
            "42/attempts/2/jobs?per_page=100": jobs,
        })
        self.assertEqual(selected, history.NpmRecovery(101, 42, 1, observe_only=False))

    def test_duplicate_report_names_do_not_fall_back_to_older_report(self) -> None:
        selected, api = self.select({
            "42": self.run_record(),
            "42/artifacts?per_page=100": [
                {"total_count": 3, "artifacts": [self.report(), self.report(2, artifact_id=102)]},
                {"total_count": 3, "artifacts": [self.report(2, artifact_id=103)]},
            ],
        })
        self.assertEqual(selected, history.NpmRecovery())
        self.assertEqual(api.call_count, 2)

    def test_expired_report_cannot_hide_an_unobserved_upload(self) -> None:
        selected, _ = self.select({
            "42": self.run_record(),
            "42/artifacts?per_page=100": [{"total_count": 2, "artifacts": [
                self.report(), self.report(2, artifact_id=102, expired=True),
            ]}],
            "42/attempts/2/jobs?per_page=100": self.jobs(2, conclusion="failure"),
        })
        self.assertEqual(selected, history.NpmRecovery())

    def test_malformed_incomplete_or_duplicate_listing_is_observation_only(self) -> None:
        for field, records in (("artifacts", [self.report()]), ("jobs", self.jobs(2)[0]["jobs"])):
            for pages in (
                [], [{"total_count": 2, field: records}],
                [{"total_count": 2, field: records}, {"total_count": 2, field: records}],
                [{"total_count": 1, field: records}, {"total_count": 2, field: []}],
                [{"total_count": True, field: records}],
            ):
                with self.subTest(field=field, pages=pages):
                    responses = {
                        "42": self.run_record(),
                        "42/artifacts?per_page=100": [{"total_count": 1, "artifacts": [self.report()]}],
                        "42/attempts/2/jobs?per_page=100": self.jobs(2),
                    }
                    path = "42/artifacts?per_page=100" if field == "artifacts" else "42/attempts/2/jobs?per_page=100"
                    responses[path] = pages
                    selected, _ = self.select(responses)
                    self.assertEqual(selected, history.NpmRecovery())

    def test_wrong_run_identity_or_incomplete_ancestor_cannot_select_a_report(self) -> None:
        ancestor = self.run_record(41, 1, status="completed")
        for field, value in (
            ("id", 40), ("path", ".github/workflows/release-web.yml"),
            ("repository", None), ("repository", {"full_name": "other/project"}),
            ("event", "push"), ("status", "in_progress"), ("run_attempt", 0),
        ):
            with self.subTest(field=field, value=value):
                selected, _ = self.select({"41": dict(ancestor, **{field: value})}, attempt=1, ancestor=41)
                self.assertEqual(selected, history.NpmRecovery())

    def test_failed_api_and_current_attempt_race_are_observation_only(self) -> None:
        with mock.patch.object(history, "read_api", side_effect=subprocess.CalledProcessError(1, "gh api")):
            self.assertEqual(history.select_npm_recovery(self.repository, 42, 3, "node"), history.NpmRecovery())
        selected, _ = self.select({"42": self.run_record(attempt=4)})
        self.assertEqual(selected, history.NpmRecovery())

    def test_cli_emits_numeric_identity_and_explicit_observation_default(self) -> None:
        for selected in (history.NpmRecovery(101, 41, 1, False), history.NpmRecovery()):
            with self.subTest(selected=selected), mock.patch.object(
                history, "select_npm_recovery", return_value=selected,
            ) as select, redirect_stdout(io.StringIO()) as output:
                self.assertEqual(history.main([
                    "--owner", "node", "--repository", self.repository,
                    "--run-id", "42", "--before-attempt", "3",
                    "--select-npm-report", "--ancestor-run-id", "41",
                ]), 0)
            values = dict(line.split("=", 1) for line in output.getvalue().splitlines())
            self.assertEqual(values, {
                "artifact_id": str(selected.artifact_id or ""), "run_id": str(selected.run_id or ""),
                "attempt": str(selected.attempt or ""), "observe_only": str(selected.observe_only).lower(),
            })
            select.assert_called_once_with(self.repository, 42, 3, "node", ancestor_run_id=41)


class FlutterPublicationHistoryTests(unittest.TestCase):
    repository = "example/project"
    tag = "flutter-v0.10.0-alpha.2"
    source = "a" * 40

    def run_record(self, run_id=42, attempt=1, status="in_progress") -> dict:
        return {
            "id": run_id, "run_attempt": attempt, "status": status,
            "path": ".github/workflows/release-flutter.yml", "head_branch": self.tag,
            "head_sha": self.source, "event": "workflow_dispatch",
            "repository": {"full_name": self.repository},
        }

    def jobs(self, conclusion="skipped") -> list[dict]:
        return [{"total_count": 1, "jobs": [{
            "id": 101, "name": "publish", "status": "completed", "conclusion": "failure",
            "steps": [{"name": "Publish to pub.dev", "status": "completed", "conclusion": conclusion}],
        }]}]

    def state(self, runs, jobs=(), *, attempt=1) -> str:
        with mock.patch.object(history, "read_api", side_effect=[
            [{"total_count": len(runs), "workflow_runs": runs}], *jobs,
        ]) as api:
            state = history.flutter_publish_state(self.repository, 42, attempt, self.tag, self.source)
        self.assertTrue(all(call.kwargs.get("paginate") for call in api.call_args_list))
        return state

    def test_first_run_checks_tag_history_before_authorizing_upload(self) -> None:
        self.assertEqual(self.state([self.run_record()]), "never-attempted")

    def test_failed_upload_in_previous_run_cannot_be_hidden_by_new_dispatch(self) -> None:
        for outcome in ("success", "failure", "cancelled", None):
            with self.subTest(outcome=outcome):
                runs = [self.run_record(), self.run_record(41, status="completed")]
                self.assertEqual(self.state(runs, [self.jobs(outcome)]), "unknown")

    def test_only_explicit_skips_allow_facade_repair_in_a_new_run(self) -> None:
        runs = [self.run_record(), self.run_record(41, attempt=2, status="completed")]
        self.assertEqual(self.state(runs, [self.jobs(), self.jobs()]), "never-attempted")

    def test_cancelled_job_without_runner_allows_facade_repair_in_a_new_run(self) -> None:
        jobs = self.jobs()
        jobs[0]["jobs"][0].update(
            conclusion="cancelled", runner_id=None, runner_name=None, steps=[],
        )
        runs = [self.run_record(), self.run_record(41, attempt=2, status="completed")]
        self.assertEqual(self.state(runs, [self.jobs(), jobs]), "never-attempted")

    def test_same_run_upload_and_incomplete_other_run_remain_unknown(self) -> None:
        self.assertEqual(self.state([self.run_record(attempt=2)], [self.jobs("failure")], attempt=2), "unknown")
        self.assertEqual(self.state([self.run_record(), self.run_record(41)]), "unknown")

    def test_missing_current_wrong_source_and_incomplete_pages_fail_closed(self) -> None:
        self.assertEqual(self.state([]), "unknown")
        self.assertEqual(self.state([dict(self.run_record(), head_sha="b" * 40)]), "unknown")
        with mock.patch.object(history, "read_api", return_value=[{
            "total_count": 2, "workflow_runs": [self.run_record()],
        }]):
            self.assertEqual(history.flutter_publish_state(self.repository, 42, 1, self.tag, self.source), "unknown")


if __name__ == "__main__":
    unittest.main()
