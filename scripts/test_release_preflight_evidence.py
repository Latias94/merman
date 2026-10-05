#!/usr/bin/env python3
"""Trust and fallback contracts for immutable release preflight reuse."""

from __future__ import annotations

import copy
from dataclasses import replace
import hashlib
import io
import json
import os
import re
from pathlib import Path
import stat
import tempfile
import unittest
from unittest import mock
import urllib.error
import urllib.request
import zipfile

from scripts import release_attempt_history as history
from scripts import release_preflight_evidence as evidence

ROOT = Path(__file__).resolve().parents[1]


def archive_bytes(value, *, name=evidence.RECEIPT_NAME, extra=False, mode=None):
    output = io.BytesIO()
    with zipfile.ZipFile(output, "w", compression=zipfile.ZIP_DEFLATED) as archive:
        member = zipfile.ZipInfo(name)
        if mode is not None:
            member.external_attr = mode << 16
        archive.writestr(member, value if isinstance(value, bytes) else json.dumps(value))
        if extra:
            archive.writestr("unexpected.json", "{}")
    return output.getvalue()


class PreflightEvidenceTests(unittest.TestCase):
    def setUp(self):
        self.identity = evidence.ReleaseIdentity(
            "Latias94/merman", 42, "a" * 40, "b" * 40, "c" * 40, "0.8.0-alpha.7",
        )
        self.needs = {job: {"result": "success"} for job in evidence.REQUIRED_JOBS}
        self.run = {
            "id": 100, "run_attempt": 2, "workflow_id": 7,
            "path": evidence.WORKFLOW_PATH,
            "event": "workflow_dispatch", "status": "completed", "conclusion": "success",
            "head_sha": self.identity.workflow_sha,
            "repository": {"id": 42, "full_name": self.identity.repository},
            "head_repository": {"id": 42, "full_name": self.identity.repository},
        }
        self.document = evidence.receipt(self.identity, 100, 2)
        self.archive = archive_bytes(self.document)
        self.artifact = {
            "id": 300, "name": evidence.artifact_name(self.identity, 2),
            "expired": False, "size_in_bytes": len(self.archive),
            "digest": "sha256:" + hashlib.sha256(self.archive).hexdigest(),
            "workflow_run": {"id": 100, "repository_id": 42, "head_repository_id": 42,
                             "head_sha": self.identity.workflow_sha},
        }
        self.pages = {
            "/actions/workflows/release-preflight.yml": {"id": 7, "path": evidence.WORKFLOW_PATH},
            "/actions/workflows/7/runs?event=workflow_dispatch&per_page=20": {"workflow_runs": [self.run]},
            "/actions/runs/100/attempts/2": copy.deepcopy(self.run),
            "/actions/runs/100/artifacts?per_page=100": {"total_count": 1, "artifacts": [self.artifact]},
            "/actions/runs/100": copy.deepcopy(self.run),
        }
        self.api = mock.Mock(spec=evidence.GitHubApi)
        self.api.get.side_effect = lambda path: self.pages[path]
        self.api.download.side_effect = lambda artifact_id: self.archive
        self.log = mock.Mock()

    def find(self):
        return evidence.find_evidence(self.identity, self.api, self.log)

    def update_archive(self, data):
        self.archive = data
        self.artifact["size_in_bytes"] = len(data)
        self.artifact["digest"] = "sha256:" + hashlib.sha256(data).hexdigest()

    def test_versions_use_the_canonical_release_contract(self):
        for version in (
            "0.8.0", "0.8.0+build.01", "0.8.0-alpha.0+build.01",
            "0.8.0-beta.1+sha.Abc-123", "0.8.0-rc.2+001",
        ):
            with self.subTest(version=version):
                identity = replace(self.identity, version=version)
                document = evidence.create_receipt(identity, identity.source_sha, self.needs, 100, 2)
                self.assertEqual(document["version"], version)
        for version in (
            "00.8.0", "0.08.0", "0.8.00", "0.8.0-alpha.01", "0.8.0-beta.00",
            "0.8.0-rc.01+build", "v0.8.0", "0.8.0-preview.1", "0.8.0+", "0.8.0+bad..build",
        ):
            with self.subTest(version=version), self.assertRaises(evidence.EvidenceError):
                replace(self.identity, version=version)

    def test_create_requires_frozen_input_and_every_successful_owner(self):
        self.assertEqual(evidence.create_receipt(
            self.identity, self.identity.source_sha, self.needs, 100, 2,
        ), self.document)
        for source_ref in ("main", "refs/heads/main", "v0.8.0-alpha.7"):
            with self.subTest(source_ref=source_ref):
                self.assertIsNone(evidence.create_receipt(self.identity, source_ref, self.needs, 100, 2))
        for result in ("failure", "cancelled", "skipped", None):
            with self.subTest(result=result):
                needs = copy.deepcopy(self.needs)
                needs["web-npm-dry-run"]["result"] = result
                with self.assertRaises(evidence.EvidenceError):
                    evidence.create_receipt(self.identity, self.identity.source_sha, needs, 100, 2)
        needs = copy.deepcopy(self.needs)
        del needs["flutter-dry-run"]
        with self.assertRaises(evidence.EvidenceError):
            evidence.create_receipt(self.identity, self.identity.source_sha, needs, 100, 2)
        with self.assertRaises(evidence.EvidenceError):
            evidence.create_receipt(self.identity, "d" * 40, self.needs, 100, 2)

    def test_finds_source_input_independently_of_workflow_head(self):
        self.assertNotEqual(self.identity.source_sha, self.run["head_sha"])
        result = self.find()
        self.assertEqual(result, {
            "run_id": 100, "run_attempt": 2, "artifact_id": 300,
            "artifact_digest": self.artifact["digest"],
        })
        self.api.download.assert_called_once_with(300)
        self.assertTrue(all("head_sha=" not in call.args[0] for call in self.api.get.call_args_list))

    def test_rejects_wrong_run_identity_and_non_success(self):
        changes = {
            "head_sha": "d" * 40, "workflow_id": 8, "path": ".github/workflows/ci.yml",
            "event": "pull_request", "status": "in_progress", "conclusion": "failure",
            "repository": {"id": 999, "full_name": self.identity.repository},
            "head_repository": {"id": 42, "full_name": "attacker/merman"},
        }
        for key, value in changes.items():
            with self.subTest(key=key):
                old = self.run[key]
                self.run[key] = value
                self.assertIsNone(self.find())
                self.run[key] = old
        self.api.download.assert_not_called()

    def test_rejects_wrong_receipt_identity_and_types(self):
        changes = {
            "schema_version": 2, "repository": "attacker/merman", "repository_id": 999,
            "workflow_path": ".github/workflows/ci.yml", "workflow_sha": "d" * 40,
            "source_sha": "d" * 40, "source_tree": "d" * 40, "version": "0.8.0",
            "run_id": 101, "run_attempt": 1,
        }
        for key, value in changes.items():
            with self.subTest(key=key):
                document = {**self.document, key: value}
                self.update_archive(archive_bytes(document))
                self.assertIsNone(self.find())
        for document in ({**self.document, "schema_version": True}, {**self.document, "extra": True}, []):
            self.update_archive(archive_bytes(document))
            self.assertIsNone(self.find())

    def test_rejects_missing_expired_duplicate_and_unbounded_artifacts(self):
        listing = self.pages["/actions/runs/100/artifacts?per_page=100"]
        for value in ({"total_count": 0, "artifacts": []},
                      {"total_count": 2, "artifacts": [self.artifact, self.artifact]},
                      {"total_count": 101, "artifacts": [self.artifact]}):
            self.pages["/actions/runs/100/artifacts?per_page=100"] = value
            self.assertIsNone(self.find())
        self.pages["/actions/runs/100/artifacts?per_page=100"] = listing
        for key, value in (("expired", True), ("size_in_bytes", 65537),
                           ("name", evidence.artifact_name(self.identity, 1))):
            old = self.artifact[key]
            self.artifact[key] = value
            self.assertIsNone(self.find())
            self.artifact[key] = old
        self.api.download.assert_not_called()

    def test_rejects_artifact_from_another_producer_or_corrupted_bytes(self):
        for key, value in (("id", 99), ("repository_id", 99), ("head_repository_id", 99),
                           ("head_sha", "d" * 40)):
            with self.subTest(key=key):
                origin = self.artifact["workflow_run"]
                old = origin[key]
                origin[key] = value
                self.assertIsNone(self.find())
                origin[key] = old
        self.archive += b"changed"
        self.assertIsNone(self.find())

    def test_rejects_attempt_mixing_and_rerun_race(self):
        attempt = self.pages["/actions/runs/100/attempts/2"]
        attempt["conclusion"] = "failure"
        self.assertIsNone(self.find())
        attempt["conclusion"] = "success"
        latest = self.pages["/actions/runs/100"]
        latest["run_attempt"] = 3
        self.assertIsNone(self.find())
        latest["run_attempt"] = 2
        latest["status"] = "in_progress"
        self.assertIsNone(self.find())

    def test_rejects_unsafe_zip_without_extracting(self):
        payloads = [
            archive_bytes(self.document, name="../preflight-evidence.json"),
            archive_bytes(self.document, extra=True),
            archive_bytes(self.document, mode=stat.S_IFLNK | 0o777),
            archive_bytes(b"x" * (evidence.MAX_RECEIPT_BYTES + 1)),
            archive_bytes(b"not json"), b"not a ZIP",
        ]
        with mock.patch.object(zipfile.ZipFile, "extractall") as extract:
            for payload in payloads:
                self.update_archive(payload)
                self.assertIsNone(self.find())
        extract.assert_not_called()

    def test_search_is_bounded_and_ignores_a_later_matching_run(self):
        bad = {**self.run, "conclusion": "failure"}
        self.pages["/actions/workflows/7/runs?event=workflow_dispatch&per_page=20"] = {
            "workflow_runs": [bad] * evidence.MAX_RUNS + [self.run],
        }
        self.assertIsNone(self.find())
        self.api.download.assert_not_called()

    def test_network_failure_stops_search_and_reaches_the_fallback_boundary(self):
        self.api.download.side_effect = urllib.error.URLError("offline")
        with self.assertRaises(urllib.error.URLError):
            self.find()
        self.api.download.assert_called_once_with(300)

    def command_environment(self):
        return {
            "GITHUB_REPOSITORY": self.identity.repository, "GITHUB_REPOSITORY_ID": "42",
            "PREFLIGHT_WORKFLOW_SHA": self.identity.workflow_sha,
            "SOURCE_SHA": self.identity.source_sha, "SOURCE_TREE": self.identity.source_tree,
            "VERSION": self.identity.version, "GH_TOKEN": "read-token",
            "PREFLIGHT_SOURCE_REF": self.identity.source_sha,
            "PREFLIGHT_NEEDS": json.dumps(self.needs),
            "GITHUB_RUN_ID": "100", "GITHUB_RUN_ATTEMPT": "2",
        }

    def test_find_command_falls_back_for_metadata_or_unexpected_http_failure(self):
        environment = self.command_environment()
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "outputs"
            environment["GITHUB_OUTPUT"] = str(output)
            for failure in (urllib.error.URLError("offline"), RuntimeError("bad compression")):
                with mock.patch.dict(os.environ, environment), mock.patch.object(
                    evidence, "find_evidence", side_effect=failure,
                ), mock.patch("builtins.print") as log:
                    self.assertEqual(evidence.main(["find"]), 0)
                    self.assertIn("reusable=false", output.read_text())
                    self.assertTrue(any("complete release checks" in str(call) for call in log.call_args_list))

    def test_create_and_find_command_outputs_are_bound_to_verified_evidence(self):
        with tempfile.TemporaryDirectory() as directory:
            directory = Path(directory)
            output = directory / "outputs"
            environment = {**self.command_environment(), "GITHUB_OUTPUT": str(output)}
            with mock.patch.dict(os.environ, environment):
                path = directory / evidence.RECEIPT_NAME
                self.assertEqual(evidence.main(["create", "--output", str(path)]), 0)
                self.assertEqual(json.loads(path.read_text()), self.document)
                self.assertEqual(output.read_text(), "created=true\n")
                output.write_text("")
                with mock.patch.object(evidence, "GitHubApi", return_value=self.api):
                    self.assertEqual(evidence.main(["find"]), 0)
                values = dict(line.split("=", 1) for line in output.read_text().splitlines())
                self.assertEqual(values, {"reusable": "true", "run_id": "100", "run_attempt": "2",
                                         "artifact_id": "300", "artifact_digest": self.artifact["digest"]})
            output.write_text("")
            environment["PREFLIGHT_SOURCE_REF"] = "main"
            with mock.patch.dict(os.environ, environment):
                branch_output = directory / "branch.json"
                self.assertEqual(evidence.main(["create", "--output", str(branch_output)]), 0)
                self.assertFalse(branch_output.exists())
                self.assertEqual(output.read_text(), "created=false\n")

    def test_redirect_drops_credentials_and_rejects_plain_http(self):
        handler = evidence.SafeRedirectHandler()
        request = urllib.request.Request("https://api.github.com/artifact", headers={"Authorization": "secret"})
        redirected = handler.redirect_request(request, None, 302, "Found", {}, "https://storage.example/zip")
        self.assertIsNone(redirected.get_header("Authorization"))
        with self.assertRaises(evidence.EvidenceError):
            handler.redirect_request(request, None, 302, "Found", {}, "http://storage.example/zip")

    def test_http_reads_are_bounded(self):
        api = evidence.GitHubApi(self.identity.repository, "read-token")
        response = mock.MagicMock()
        response.__enter__.return_value.read.return_value = b"12345"
        api.opener = mock.Mock()
        api.opener.open.return_value = response
        with self.assertRaises(evidence.EvidenceError):
            api.read("/actions/workflows", 4)
        response.__enter__.return_value.read.assert_called_once_with(5)
        self.assertEqual(api.opener.open.call_args.kwargs["timeout"], 15)


class PreflightWorkflowTests(unittest.TestCase):
    def job(self, text, job_id):
        start = text.index(f"  {job_id}:\n")
        following = re.search(r"\n  [a-z][a-z0-9-]*:\n", text[start + 1:])
        return text[start:] if following is None else text[start:start + 1 + following.start()]

    def test_issuer_requires_all_owners_and_trusted_tooling(self):
        workflow = (ROOT / ".github/workflows/release-preflight.yml").read_text()
        issuer = self.job(workflow, "release-preflight-evidence")
        jobs = set(re.findall(r"^  ([a-z][a-z0-9-]*):$", workflow.split("jobs:\n", 1)[1], re.MULTILINE))
        self.assertEqual(jobs - {"release-preflight-evidence"}, evidence.REQUIRED_JOBS)
        for job_id in evidence.REQUIRED_JOBS:
            self.assertIn(f"      - {job_id}\n", issuer)
        self.assertIn("ref: ${{ github.workflow_sha }}", issuer)
        self.assertIn("PREFLIGHT_NEEDS: ${{ toJSON(needs) }}", issuer)
        self.assertIn("PREFLIGHT_SOURCE_REF: ${{ inputs.source_ref }}", issuer)
        self.assertIn("-attempt-${{ github.run_attempt }}", issuer)
        self.assertNotIn("overwrite:", issuer)
        self.assertNotIn("always()", issuer)
        self.assertIn("timeout-minutes: 10", workflow)
        self.assertIn("Acquire::Retries=3", workflow)

    def test_only_expensive_checks_are_conditional(self):
        workflow = (ROOT / ".github/workflows/release-crates.yml").read_text()
        preflight = self.job(workflow, "preflight")
        web = self.job(workflow, "web-owner-preflight")
        self.assertIn("ref: ${{ github.workflow_sha }}", preflight)
        self.assertIn("actions: read", preflight)
        self.assertIn("if: ${{ steps.evidence.outputs.reusable != 'true' }}", preflight)
        self.assertEqual(preflight.count("        if:"), 1)
        for command in ("verify_prerelease_compatibility.py", "verify-independent-crate-version-bumps.py",
                        "release_surface_contract.py", "--list-crates-io-packages"):
            self.assertIn(command, preflight)
        self.assertNotIn("\n    if:", web)
        contracts = web.split("      - name: Verify generated Web contracts\n", 1)[1].split("      - name:", 1)[0]
        self.assertNotIn("if:", contracts)
        self.assertIn("release-version.py check", web)
        self.assertIn("SOURCE_TREE:", web)
        self.assertIn("if: ${{ needs.preflight.outputs.reused_evidence != 'true' }}", web)
        self.assertIn("typst-package-smoke", self.job(workflow, "typst-owner-preflight"))
        publish = self.job(workflow, "publish")
        self.assertIn("publish-receipted", publish)
        self.assertIn('test "$observed" = "$SOURCE_SHA"', publish)
        self.assertIn("needs: [validate-inputs, preflight, web-owner-preflight, typst-owner-preflight]", publish)

    def test_failed_readonly_tag_check_does_not_count_as_an_upload(self):
        workflow = (ROOT / ".github/workflows/release-crates.yml").read_text()
        publish = self.job(workflow, "publish")
        steps = dict((part.split("\n", 1)[0], part.split("\n", 1)[1])
                     for part in publish.split("      - name: ")[1:])
        tag_check = "Verify release tag immediately before publication"
        upload = history.OWNERS["crates"][2]
        names = list(steps)
        self.assertEqual(names[names.index(upload) - 1], tag_check)
        self.assertIn('git/ref/tags/$RELEASE_TAG', steps[tag_check])
        self.assertIn('commits/$RELEASE_TAG', steps[tag_check])
        self.assertIn('test "$observed" = "$SOURCE_SHA"', steps[tag_check])
        self.assertNotIn("continue-on-error:", steps[tag_check])
        self.assertNotIn("        if:", steps[upload])
        self.assertNotIn("gh api", steps[upload])
        self.assertIn("publish-receipted", steps[upload])
        pages = [{"total_count": 1, "jobs": [{
            "id": 11, "name": "publish", "status": "completed", "conclusion": "failure",
            "steps": [
                {"name": tag_check, "status": "completed", "conclusion": "failure"},
                {"name": upload, "status": "completed", "conclusion": "skipped"},
            ],
        }]}]
        # A 503 during the read-only tag lookup skips the actual publisher.
        for conclusion, expected in (("skipped", "never-attempted"), ("failure", "unknown")):
            pages[0]["jobs"][0]["steps"][1]["conclusion"] = conclusion
            with self.subTest(upload_conclusion=conclusion), mock.patch.object(
                history, "read_api", side_effect=[{"path": ".github/workflows/release-crates.yml"}, pages],
            ):
                self.assertEqual(history.prior_publish_state("Latias94/merman", 100, 2, "crates"), expected)

    def test_recovery_rerun_uses_its_own_previous_attempt_receipts(self):
        workflow = (ROOT / ".github/workflows/release-crates.yml").read_text()
        publish = self.job(workflow, "publish")
        self.assertIn('recovery_run_id="$REQUESTED_RECOVERY_RUN_ID"', publish)
        self.assertIn('--owner crates --repository "$GITHUB_REPOSITORY"', publish)
        self.assertIn('if [ "$history" != "never-attempted" ]; then', publish)
        self.assertIn('recovery_run_id="$GITHUB_RUN_ID"', publish)
        self.assertIn('recovery_attempt=$((GITHUB_RUN_ATTEMPT - 1))', publish)
        self.assertIn("name: crates-io-receipts-${{ needs.preflight.outputs.source_sha }}-attempt-${{ steps.recovery.outputs.attempt }}", publish)
        self.assertNotIn("pattern: crates-io-receipts-", publish)
        self.assertIn('recovery_args+=(--recovery-run-id "$PRIOR_RUN_ID" --recovery-attempt "$PRIOR_ATTEMPT")', publish)
        self.assertIn("compgen -G 'recovery-receipts/batch-*.json'", publish)
        self.assertIn('if [ "$RECOVERY_DOWNLOAD_OUTCOME" = "success" ]', publish)
        self.assertIn('recovery_args+=(--observe-only)', publish)
        self.assertIn('--publication-run-id "$GITHUB_RUN_ID" --publication-attempt "$GITHUB_RUN_ATTEMPT"', publish)
        self.assertIn("CARGO_REGISTRIES_CRATES_IO_PROTOCOL: sparse", workflow)


if __name__ == "__main__":
    unittest.main()
