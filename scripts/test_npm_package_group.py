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
