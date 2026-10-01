#!/usr/bin/env python3
"""Prove that job history authorizes only the first actual upload."""

from __future__ import annotations

import copy
import json
import subprocess
import unittest
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
        for owner in ("grammar-npm", "grammar-crates", "crates"):
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


if __name__ == "__main__":
    unittest.main()
