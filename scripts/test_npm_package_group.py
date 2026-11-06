#!/usr/bin/env python3
"""Exercise delayed npm replicas at the publisher/clock boundary."""

from __future__ import annotations

import copy
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock

from scripts import npm_package_group as npm
from scripts import release_attempt_history as history


class Clock:
    def __init__(self) -> None:
        self.now = 0.0
        self.delays: list[float] = []

    def sleep(self, seconds: float) -> None:
        self.delays.append(seconds)
        self.now += seconds


class DelayedNpm:
    def __init__(self, manifest: dict, clock: Clock, *, delay: float = 0) -> None:
        self.manifest = manifest
        self.clock = clock
        self.delay = delay
        self.accepted: dict[str, float] = {}
        self.calls: list[str] = []
        self.response_loss = False
        self.interrupt = False
        self.conflict = False
        self.tag_delay = 0.0

    def version_integrity(self, package: str, version: str) -> str | None:
        if package not in self.accepted or self.clock.now < self.accepted[package] + self.delay:
            return None
        if self.conflict:
            return "sha512-b3RoZXI="
        return next(item["integrity"] for item in self.manifest["packages"] if item["name"] == package)

    def dist_tag(self, package: str, tag: str) -> str | None:
        if self.clock.now < self.accepted[package] + self.tag_delay:
            return "0.9.0"
        return self.manifest["version"]

    def publish(self, tarball: Path, tag: str) -> None:
        name = next(item["name"] for item in self.manifest["packages"] if item["tarball"] == tarball.name)
        self.calls.append(name)
        self.accepted[name] = self.clock.now
        if self.interrupt:
            raise KeyboardInterrupt("process interrupted after registry acceptance")
        if self.response_loss:
            raise npm.PackageGroupError("connection closed after upload")


class NpmRecoveryTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.report = self.root / "reconciliation-report.json"
        self.manifest = {
            "version": "1.0.0", "target_dist_tag": "latest", "source_sha": "a" * 40,
            "packages": [
                {"name": "@test/dependency", "tarball": "dependency.tgz", "integrity": "sha512-ZGVw"},
                {"name": "@test/root", "tarball": "root.tgz", "integrity": "sha512-cm9vdA=="},
            ],
        }
        self.clock = Clock()
        self.client = DelayedNpm(self.manifest, self.clock)

    def run_group(self, **kwargs) -> dict:
        kwargs.setdefault("publication_run_id", "42")
        kwargs.setdefault("publication_attempt", 1)
        if isinstance(kwargs.get("recovery_report"), dict):
            prior = kwargs["recovery_report"].get("publication")
            if prior:
                kwargs.setdefault("recovery_run_id", prior["run_id"])
                kwargs.setdefault("recovery_attempt", prior["attempt"])
        with (mock.patch.object(npm.time, "sleep", side_effect=self.clock.sleep),
              mock.patch.object(npm.time, "monotonic", side_effect=lambda: self.clock.now)):
            return npm.reconcile_group(
                self.manifest, self.root, self.client, report_path=self.report,
                observation_attempts=4, observation_delay_seconds=5, **kwargs,
            )

    def saved(self) -> dict:
        return json.loads(self.report.read_text(encoding="utf-8"))

    def test_48_minute_delay_is_recoverable_without_reupload(self) -> None:
        self.client.delay = 48 * 60
        with self.assertRaises(npm.ReconciliationError) as raised:
            self.run_group()
        report = raised.exception.report
        self.assertEqual(report["status"], "pending-registry-visibility")
        self.assertEqual(report["package_states"]["@test/dependency"], "accepted_pending_visibility")
        self.assertEqual(report["package_states"]["@test/root"], "not_attempted")
        with self.assertRaises(npm.ReconciliationError):
            self.run_group(recovery_report=self.saved())
        self.assertEqual(self.client.calls, ["@test/dependency"])
        self.clock.now = 48 * 60
        self.client.delay = 0
        result = self.run_group(recovery_report=self.saved())
        self.assertEqual(result["status"], "released")
        self.assertEqual(self.client.calls, ["@test/dependency", "@test/root"])
        self.assertEqual(result["already_published"], ["@test/dependency"])
        self.assertEqual(self.clock.delays, [5, 10, 20] * 2)

    def test_missing_attempt_report_cannot_replay_an_older_not_attempted_state(self) -> None:
        self.client.delay = 3600
        with self.assertRaises(npm.ReconciliationError):
            self.run_group(publication_run_id="42", publication_attempt=1)
        old_report = self.saved()
        self.assertEqual(old_report["package_states"]["@test/root"], "not_attempted")
        self.clock.now = 3600
        self.client.delay = 0
        self.client.interrupt = True
        with self.assertRaises(KeyboardInterrupt):
            self.run_group(recovery_report=old_report, publication_run_id="42", publication_attempt=2,
                           recovery_run_id="42", recovery_attempt=1)
        # Simulate runner loss: attempt 2's on-disk report never became an artifact.
        self.client.interrupt = False
        self.client.delay = 3600
        with self.assertRaises(npm.ReconciliationError):
            self.run_group(recovery_report=old_report, publication_run_id="42", publication_attempt=3,
                           recovery_run_id="42", recovery_attempt=2)
        self.assertEqual(self.client.calls, ["@test/dependency", "@test/root"])
        self.assertEqual(self.saved()["publication"], {"run_id": "42", "attempt": 3})
        self.assertEqual(self.saved()["package_states"]["@test/root"], "upload_outcome_unknown")

    def test_partial_upload_then_skipped_setup_resumes_from_actual_report_attempt(self) -> None:
        self.client.delay = 3600
        with self.assertRaises(npm.ReconciliationError):
            self.run_group()
        first_report = self.saved()
        self.assertEqual(first_report["package_states"]["@test/root"], "not_attempted")
        prefix = "repos/example/project/actions/runs/42"
        responses = {
            prefix: {
                "id": 42, "path": ".github/workflows/release-node.yml",
                "repository": {"full_name": "example/project"}, "event": "workflow_dispatch",
                "run_attempt": 3,
            },
            prefix + "/artifacts?per_page=100": [{"total_count": 1, "artifacts": [{
                "id": 101, "name": "merman-node-npm-reconciliation-report-attempt-1", "expired": False,
            }]}],
            prefix + "/attempts/2/jobs?per_page=100": [{"total_count": 1, "jobs": [{
                "id": 202, "name": "publish npm package group",
                "status": "completed", "conclusion": "failure",
                "steps": [{"name": "Publish npm package group", "status": "completed", "conclusion": "skipped"}],
            }]}],
        }
        with mock.patch.object(history, "read_api", side_effect=lambda path, **_: responses[path]):
            selected = history.select_npm_recovery("example/project", 42, 3, "node")
        self.assertEqual((selected.run_id, selected.attempt, selected.observe_only), (42, 1, False))
        self.clock.now = 3600
        self.client.delay = 0
        result = self.run_group(
            recovery_report=first_report, publication_attempt=3,
            recovery_run_id=str(selected.run_id), recovery_attempt=selected.attempt,
            observe_only=selected.observe_only,
        )
        self.assertEqual(result["status"], "released")
        self.assertEqual(self.client.calls, ["@test/dependency", "@test/root"])
        self.assertEqual(result["already_published"], ["@test/dependency"])
        self.assertEqual(self.saved()["publication"], {"run_id": "42", "attempt": 3})

    def test_unknown_member_in_selected_older_report_still_cannot_be_resent(self) -> None:
        self.client.delay = 3600
        self.client.response_loss = True
        with self.assertRaises(npm.ReconciliationError):
            self.run_group()
        report = self.saved()
        with self.assertRaises(npm.ReconciliationError):
            self.run_group(
                recovery_report=report, publication_attempt=3,
                recovery_run_id="42", recovery_attempt=1,
            )
        self.assertEqual(self.client.calls, ["@test/dependency"])
        self.assertEqual(self.saved()["package_states"]["@test/dependency"], "upload_outcome_unknown")
        self.assertEqual(self.saved()["package_states"]["@test/root"], "not_attempted")

    def test_interrupted_atomic_report_replace_preserves_prior_complete_report(self) -> None:
        previous = {"status": "pending", "package_states": {"@test/root": "upload_started"}}
        npm.write_reconciliation_report(self.report, previous)
        with mock.patch.object(npm.os, "replace", side_effect=OSError("interrupted replace")):
            with self.assertRaisesRegex(OSError, "interrupted replace"):
                npm.write_reconciliation_report(self.report, {"status": "released"})
        self.assertEqual(self.saved(), previous)

    def test_lookup_failure_cannot_launder_unknown_history_into_attempt_four(self) -> None:
        self.client.delay = 3600
        with self.assertRaises(npm.ReconciliationError):
            self.run_group(publication_attempt=1)
        first_report = self.saved()
        self.clock.now = 3600
        self.client.delay = 0
        self.client.interrupt = True
        with self.assertRaises(KeyboardInterrupt):
            self.run_group(recovery_report=first_report, publication_attempt=2)
        # The runner loses attempt 2's artifact after actually sending root.
        self.client.interrupt = False
        self.client.delay = 3600
        with mock.patch.object(self.client, "version_integrity", side_effect=npm.PackageGroupError("HTTP 503")):
            with self.assertRaisesRegex(npm.ReconciliationError, "HTTP 503"):
                self.run_group(recovery_report=first_report, publication_attempt=3,
                               recovery_run_id="42", recovery_attempt=2)
        third_report = self.saved()
        self.assertEqual(third_report["publication"], {"run_id": "42", "attempt": 3})
        self.assertIs(third_report["observe_only"], True)
        self.assertEqual(third_report["package_states"]["@test/root"], "upload_outcome_unknown")
        with self.assertRaises(npm.ReconciliationError):
            self.run_group(recovery_report=third_report, publication_attempt=4,
                           recovery_run_id="42", recovery_attempt=3)
        self.assertEqual(self.client.calls, ["@test/dependency", "@test/root"])
        self.assertIs(self.saved()["observe_only"], True)

    def test_declared_recovery_without_a_report_cannot_publish(self) -> None:
        with self.assertRaises(npm.ReconciliationError):
            self.run_group(publication_attempt=2, recovery_run_id="42", recovery_attempt=1)
        self.assertEqual(self.client.calls, [])
        self.assertIs(self.saved()["observe_only"], True)
        self.assertEqual(set(self.saved()["package_states"].values()), {"upload_outcome_unknown"})

    def test_unknown_response_and_repeated_404_never_reupload(self) -> None:
        self.client.response_loss = True
        self.client.delay = 3600
        with self.assertRaises(npm.ReconciliationError) as raised:
            self.run_group()
        self.assertEqual(raised.exception.report["status"], "publish-outcome-unknown")
        self.assertEqual(self.saved()["package_states"]["@test/dependency"], "upload_outcome_unknown")
        with self.assertRaises(npm.ReconciliationError):
            self.run_group(recovery_report=self.saved())
        self.assertEqual(self.client.calls, ["@test/dependency"])

    def rejected_diagnostic(
        self, *, status: int = 404, endpoint: str | None = None, legacy: bool = False,
    ) -> str:
        target = endpoint or "https://registry.npmjs.org/@test%2fdependency"
        retry_option = "" if legacy else " --fetch-retries=0"
        return (
            f"npm publish {self.root / 'dependency.tgz'} --ignore-scripts --access public "
            f"--tag latest{retry_option} failed: npm notice Publishing to https://registry.npmjs.org/\n"
            f"npm error code E{status}\n"
            f"npm error {status} Not Found - PUT {target} - Not found\n"
            "npm error The requested resource could not be found or you do not have permission."
        )

    def test_explicit_permission_rejection_can_retry_after_configuration_repair(self) -> None:
        with mock.patch.object(
            self.client, "publish", side_effect=npm.PackageGroupError(self.rejected_diagnostic()),
        ) as publish:
            with self.assertRaises(npm.ReconciliationError):
                self.run_group()
        self.assertEqual(publish.call_count, 1)
        rejected = self.saved()
        self.assertEqual(rejected["status"], "publish-rejected")
        self.assertEqual(rejected["package_states"]["@test/dependency"], "upload_rejected")
        self.assertEqual(rejected["package_states"]["@test/root"], "not_attempted")
        self.assertEqual(self.clock.delays, [])
        result = self.run_group(recovery_report=rejected, publication_attempt=2)
        self.assertEqual(result["status"], "released")
        self.assertEqual(self.client.calls, ["@test/dependency", "@test/root"])

    def test_legacy_unknown_report_retains_evidence_of_explicit_package_put_rejection(self) -> None:
        self.client.response_loss = True
        self.client.delay = 3600
        with self.assertRaises(npm.ReconciliationError):
            self.run_group()
        prior = self.saved()
        prior["error"] = self.rejected_diagnostic(legacy=True) + "; registry observation could not confirm acceptance"
        frozen_prior = copy.deepcopy(prior)
        self.client.accepted.clear()
        self.client.calls.clear()
        self.client.response_loss = False
        self.client.delay = 0
        result = self.run_group(recovery_report=prior, publication_run_id="43", publication_attempt=1,
                                retry_rejected_package="@test/dependency")
        self.assertEqual(result["status"], "released")
        self.assertEqual(self.client.calls, ["@test/dependency", "@test/root"])
        self.assertEqual(result["recovered_rejections"], [{
            "name": "@test/dependency", "http_status": 404,
            "publication": {"run_id": "42", "attempt": 1},
            "explicit_retry": True,
        }])
        self.assertEqual(prior, frozen_prior)

    def test_only_explicit_permission_rejections_of_the_exact_package_put_qualify(self) -> None:
        record = self.manifest["packages"][0]
        for status in (401, 403, 404):
            with self.subTest(status=status):
                self.assertEqual(npm.publish_rejection_status(
                    self.rejected_diagnostic(status=status), record, self.root, "latest",
                ), status)
        diagnostic = self.rejected_diagnostic()
        for rejected in (
            diagnostic.replace(" - PUT ", " - GET "),
            diagnostic.replace(" - PUT ", " - POST "),
            diagnostic.replace("E404", "ECONNRESET"),
            diagnostic.replace("dependency.tgz", "root.tgz"),
            diagnostic.replace("--tag latest", "--tag alpha"),
            self.rejected_diagnostic(legacy=True),
            self.rejected_diagnostic(status=500),
            self.rejected_diagnostic(status=408),
            self.rejected_diagnostic(status=429),
            self.rejected_diagnostic(endpoint="https://registry.npmjs.org/@test%2froot"),
            self.rejected_diagnostic(endpoint="https://registry.npmjs.org/-/npm/v1/oidc/token"),
            self.rejected_diagnostic(endpoint="https://registry.npmjs.org/-/npm/v1/attestations"),
            self.rejected_diagnostic(endpoint="https://other.example/@test%2fdependency"),
            self.rejected_diagnostic(endpoint="https://registry.npmjs.org/@test%2fdependency?write=1"),
            diagnostic + "\nnpm error 404 Not Found - PUT https://registry.npmjs.org/@test%2fdependency - Not found",
            "connection closed after upload; npm view returned E404",
        ):
            with self.subTest(diagnostic=rejected):
                self.assertIsNone(npm.publish_rejection_status(rejected, record, self.root, "latest"))

    def test_final_404_after_possible_internal_retry_remains_unknown_without_explicit_decision(self) -> None:
        diagnostic = self.rejected_diagnostic(legacy=True)
        with mock.patch.object(self.client, "publish", side_effect=npm.PackageGroupError(diagnostic)):
            with self.assertRaises(npm.ReconciliationError):
                self.run_group()
        unknown = self.saved()
        self.assertEqual(unknown["package_states"]["@test/dependency"], "upload_outcome_unknown")
        with self.assertRaises(npm.ReconciliationError):
            self.run_group(recovery_report=unknown, publication_attempt=2)
        self.assertEqual(self.client.calls, [])
        self.assertNotIn("recovered_rejections", self.saved())

    def test_publish_command_disables_transport_retries(self) -> None:
        with mock.patch.object(npm.subprocess, "run") as run:
            run.return_value.returncode = 0
            npm.NpmCli().publish(self.root / "dependency.tgz", "latest")
        self.assertIn("--fetch-retries=0", run.call_args.args[0])

    def test_explicit_retry_input_does_not_block_later_pending_member_recovery(self) -> None:
        with mock.patch.object(
            self.client, "publish",
            side_effect=npm.PackageGroupError(self.rejected_diagnostic(legacy=True)),
        ):
            with self.assertRaises(npm.ReconciliationError):
                self.run_group()
        original = self.saved()
        lookup = self.client.version_integrity

        def delayed_root(package: str, version: str) -> str | None:
            return None if package == "@test/root" else lookup(package, version)

        with mock.patch.object(self.client, "version_integrity", side_effect=delayed_root):
            with self.assertRaises(npm.ReconciliationError):
                self.run_group(recovery_report=original, publication_attempt=2,
                               retry_rejected_package="@test/dependency")
        pending = self.saved()
        self.assertEqual(pending["status"], "pending-registry-visibility")
        self.assertEqual(pending["package_states"]["@test/dependency"], "verified")
        self.assertEqual(pending["package_states"]["@test/root"], "accepted_pending_visibility")
        result = self.run_group(recovery_report=pending, publication_attempt=3,
                                retry_rejected_package="@test/dependency")
        self.assertEqual(result["status"], "released")
        self.assertEqual(self.client.calls, ["@test/dependency", "@test/root"])

    def test_explicit_retry_cannot_select_an_unrelated_or_ambiguous_upload(self) -> None:
        self.client.response_loss = True
        self.client.delay = 3600
        with self.assertRaises(npm.ReconciliationError):
            self.run_group()
        unknown = self.saved()
        for name in ("@test/root", "@test/dependency", "@other/package"):
            with self.subTest(name=name), self.assertRaises(npm.PackageGroupError):
                self.run_group(recovery_report=unknown, retry_rejected_package=name)
        self.assertEqual(self.client.calls, ["@test/dependency"])

    def test_permission_rejection_cannot_bypass_missing_attempt_history(self) -> None:
        with mock.patch.object(
            self.client, "publish", side_effect=npm.PackageGroupError(self.rejected_diagnostic()),
        ):
            with self.assertRaises(npm.ReconciliationError):
                self.run_group()
        with self.assertRaises(npm.ReconciliationError):
            self.run_group(recovery_report=self.saved(), publication_attempt=3,
                           recovery_run_id="42", recovery_attempt=2)
        self.assertEqual(self.client.calls, [])
        self.assertTrue(self.saved()["observe_only"])
        self.assertEqual(self.saved()["package_states"]["@test/dependency"], "upload_outcome_unknown")

    def test_interruption_retains_attempt_before_publish_returns(self) -> None:
        self.client.interrupt = True
        self.client.delay = 3600
        with self.assertRaises(KeyboardInterrupt):
            self.run_group()
        self.assertEqual(self.saved()["package_states"]["@test/dependency"], "upload_started")
        self.client.interrupt = False
        with self.assertRaises(npm.ReconciliationError):
            self.run_group(recovery_report=self.saved())
        self.assertEqual(self.client.calls, ["@test/dependency"])

    def test_existing_member_is_verified_before_any_missing_upload(self) -> None:
        self.client.accepted["@test/root"] = 0
        result = self.run_group()
        self.assertEqual(result["already_published"], ["@test/root"])
        self.assertEqual(result["package_states"]["@test/root"], "already_published")
        self.assertEqual(self.client.calls, ["@test/dependency"])

    def test_existing_tag_conflict_stops_before_any_upload(self) -> None:
        self.client.accepted["@test/root"] = 0
        self.client.tag_delay = 100
        with self.assertRaisesRegex(npm.ReconciliationError, "cannot repair dist-tags"):
            self.run_group()
        self.assertEqual(self.client.calls, [])
        self.assertEqual(self.clock.delays, [])

    def test_recovery_state_cannot_bypass_registry_integrity_conflict(self) -> None:
        report = self.run_group()
        self.client.conflict = True
        with self.assertRaisesRegex(npm.ReconciliationError, "integrity differs"):
            self.run_group(recovery_report=report)
        self.assertEqual(len(self.client.calls), 2)
        self.assertEqual(self.clock.delays, [])

    def test_manifest_drift_rejects_report_before_registry_mutation(self) -> None:
        report = self.run_group()
        for field, value in [("source_sha", "b" * 40), ("version", "2.0.0")]:
            modified = copy.deepcopy(report)
            modified[field] = value
            with self.subTest(field=field), self.assertRaisesRegex(npm.PackageGroupError, "does not match"):
                self.run_group(recovery_report=modified)
        self.manifest["packages"][0]["integrity"] = "sha512-b3RoZXI="
        with self.assertRaisesRegex(npm.PackageGroupError, "does not match"):
            self.run_group(recovery_report=report)
        self.assertEqual(len(self.client.calls), 2)

    def test_delayed_tags_after_an_accepted_upload_are_observed(self) -> None:
        self.client.tag_delay = 12
        result = self.run_group()
        self.assertEqual(result["status"], "released")
        self.assertEqual(self.client.calls, ["@test/dependency", "@test/root"])
        self.assertEqual(self.clock.delays, [5, 10, 5, 10])

    def test_missing_report_observation_mode_cannot_publish(self) -> None:
        with self.assertRaises(npm.ReconciliationError):
            self.run_group(observe_only=True)
        self.assertEqual(self.client.calls, [])
        with self.assertRaises(npm.ReconciliationError):
            self.run_group(recovery_report=self.saved())
        self.assertEqual(self.client.calls, [])

    def test_slow_metadata_lookups_consume_the_wall_clock_budget(self) -> None:
        self.client.delay = 3600
        original = self.client.version_integrity

        def slow_lookup(*args):
            self.clock.now += 25
            return original(*args)

        with mock.patch.object(self.client, "version_integrity", side_effect=slow_lookup), \
                mock.patch.object(npm, "REGISTRY_OBSERVATION_SECONDS", 50):
            with self.assertRaises(npm.ReconciliationError):
                self.run_group()
        self.assertLessEqual(self.clock.now, 130)
        self.assertEqual(self.client.calls, ["@test/dependency"])

    def test_previous_setup_failure_allows_the_first_actual_upload(self) -> None:
        skipped = [{"total_count": 1, "jobs": [{
            "id": 11, "name": "publish", "status": "completed", "conclusion": "failure",
            "steps": [{"name": "Publish npm package group", "status": "completed", "conclusion": "skipped"}],
        }]}]
        with mock.patch.object(history, "read_api", side_effect=[
            {"path": ".github/workflows/release-web.yml"}, skipped,
        ]):
            state = history.prior_publish_state("example/project", 42, 2, "web")
        report = self.run_group(observe_only=state != "never-attempted")
        self.assertEqual(report["status"], "released")
        self.assertEqual(self.client.calls, ["@test/dependency", "@test/root"])

    def test_legacy_report_can_only_observe_the_original_artifacts(self) -> None:
        legacy = {"schema_version": 2, "version": "1.0.0", "target_dist_tag": "latest",
                  "published": ["@test/dependency"], "status": "failed-during-publish"}
        with self.assertRaises(npm.ReconciliationError):
            self.run_group(recovery_report=legacy)
        self.assertEqual(self.client.calls, [])
        self.client.accepted = {record["name"]: 0 for record in self.manifest["packages"]}
        report = self.run_group(recovery_report=self.saved())
        self.assertEqual(report["status"], "released")
        self.assertEqual(self.client.calls, [])

    def test_standalone_observer_binds_registry_integrity_to_candidate_bytes(self) -> None:
        candidate = self.root / "grammar.tgz"
        candidate.write_bytes(b"the original packed candidate")
        integrity = "sha512-" + npm.base64.b64encode(npm.hashlib.sha512(candidate.read_bytes()).digest()).decode()
        client = mock.Mock()
        client.version_integrity.return_value = integrity
        client.dist_tag.return_value = "1.0.0"
        with mock.patch.object(npm, "NpmCli", return_value=client):
            status = npm.main(["observe", "--package", "@test/grammar", "--version", "1.0.0",
                               "--tag", "latest", "--tarball", str(candidate)])
        self.assertEqual(status, 0)
        client.publish.assert_not_called()
        client.version_integrity.assert_called_once_with("@test/grammar", "1.0.0")

    def test_standalone_inspection_cannot_hide_registry_failures_as_missing(self) -> None:
        candidate = self.root / "grammar.tgz"
        candidate.write_bytes(b"the original packed candidate")
        client = mock.Mock()
        client.version_integrity.side_effect = npm.PackageGroupError("metadata HTTP 503")
        with mock.patch.object(npm, "NpmCli", return_value=client):
            status = npm.main(["inspect", "--package", "@test/grammar", "--version", "1.0.0",
                               "--tag", "latest", "--tarball", str(candidate)])
        self.assertEqual(status, 1)
        client.publish.assert_not_called()

    def test_lookup_error_containing_a_404_digit_sequence_is_not_missing(self) -> None:
        result = npm.subprocess.CompletedProcess([], 1, "", "ECONNRESET to proxy port 4040")
        with mock.patch.object(npm.subprocess, "run", return_value=result):
            with self.assertRaises(npm.PackageGroupError):
                npm.NpmCli().version_integrity("@test/root", "1.0.0")

    def test_metadata_process_timeout_is_an_observation_failure(self) -> None:
        with mock.patch.object(npm.subprocess, "run", side_effect=npm.subprocess.TimeoutExpired("npm", 30)) as run:
            with self.assertRaisesRegex(npm.PackageGroupError, "30-second"):
                npm.NpmCli().version_integrity("@test/root", "1.0.0")
        self.assertIn("--fetch-retries=0", run.call_args.args[0])


if __name__ == "__main__":
    unittest.main()
