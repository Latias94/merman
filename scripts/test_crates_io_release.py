#!/usr/bin/env python3
"""Unit tests for receipt-bound crates.io publication and recovery."""

from __future__ import annotations

from contextlib import contextmanager, ExitStack
import json
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import threading
from pathlib import Path
import tempfile
import unittest
from unittest import mock

from scripts import crates_io_release as release
from tools import publish


ROOT = Path(__file__).resolve().parents[1]


def sequence(*values):
    iterator = iter(values)
    return lambda *_args, **_kwargs: next(iterator)


def package_metadata(*names: str) -> dict[str, object]:
    packages = []
    for name in names:
        manifest = ROOT / "crates" / name / "Cargo.toml"
        packages.append(
            {
                "id": f"path+file://{manifest.parent}#1.0.0",
                "name": name,
                "version": "1.0.0",
                "publish": None,
                "manifest_path": str(manifest),
                "dependencies": [],
            }
        )
    return {
        "workspace_members": [item["id"] for item in packages],
        "packages": packages,
        "target_directory": str(ROOT / "target"),
    }


class CratesIoReceiptTests(unittest.TestCase):
    SOURCE_SHA = "1" * 40
    SOURCE_TREE = "2" * 40
    DIGEST = "3" * 64
    PLAN_DIGEST = "6" * 64

    def prepared(
        self,
        root: Path,
        name: str = "alpha",
        dependencies: tuple[str, ...] = (),
    ) -> release.PreparedCrate:
        manifest = root / "crates" / name / "Cargo.toml"
        manifest.parent.mkdir(parents=True, exist_ok=True)
        manifest.write_text(
            f'[package]\nname = "{name}"\nversion = "1.0.0"\n',
            encoding="utf-8",
        )
        artifact = root / "target" / "package" / f"{name}-1.0.0.crate"
        artifact.parent.mkdir(parents=True, exist_ok=True)
        artifact.write_bytes(b"crate bytes")
        return release.PreparedCrate(
            publish.PackageInfo(name, "1.0.0", manifest, dependencies),
            artifact,
            self.DIGEST,
            artifact.stat().st_size,
            "4" * 64,
        )

    @contextmanager
    def mocked_release(
        self,
        plan: publish.PublishPlan,
        *,
        prepare,
        run,
        checksum,
    ):
        with ExitStack() as stack:
            stack.enter_context(
                mock.patch.object(release, "crates_io_publish_plan", return_value=plan)
            )
            stack.enter_context(
                mock.patch.object(release, "_plan_digest", return_value=self.PLAN_DIGEST)
            )
            stack.enter_context(
                mock.patch.object(release, "_prepare_crate", side_effect=prepare)
            )
            stack.enter_context(
                mock.patch.object(
                    release,
                    "_captured_command",
                    side_effect=["cargo", "rustc"],
                )
            )
            stack.enter_context(mock.patch.object(release, "assert_release_source"))
            stack.enter_context(mock.patch.object(release, "_assert_artifact_unchanged"))
            stack.enter_context(mock.patch.object(release, "run_command", side_effect=run))
            stack.enter_context(
                mock.patch.object(
                    release,
                    "fetch_crates_io_checksum",
                    side_effect=checksum,
                )
            )
            stack.enter_context(mock.patch.object(release, "fetch_crates_io_index_checksum", return_value=self.DIGEST))
            yield

    def invoke(
        self,
        root: Path,
        *,
        recovery: Path | None = None,
        **kwargs,
    ) -> None:
        receipts_dir = kwargs.pop("receipts_dir", root / "receipts")
        release.publish_receipted_release(
            root,
            {"target_directory": str(root / "target")},
            source_sha=self.SOURCE_SHA,
            source_tree=self.SOURCE_TREE,
            receipts_dir=receipts_dir,
            registry_token="test-token",
            recovery_receipts_dir=recovery,
            visibility_attempts=2,
            visibility_delay=0,
            **kwargs,
        )

    def receipt(
        self,
        root: Path,
        prepared: list[release.PreparedCrate],
        state: str,
    ) -> dict:
        return release._batch_receipt(
            root,
            prepared,
            state=state,
            source_sha=self.SOURCE_SHA,
            source_tree=self.SOURCE_TREE,
            cargo_version="cargo",
            rustc_version="rustc",
            plan_sha256=self.PLAN_DIGEST,
            batch_index=0,
        )

    def test_registry_barrier_accepts_delayed_exact_checksum(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            item = self.prepared(Path(temp_dir))
            with mock.patch.object(
                release,
                "fetch_crates_io_checksum",
                side_effect=[None, self.DIGEST],
            ):
                with mock.patch.object(release, "fetch_crates_io_index_checksum", return_value=self.DIGEST):
                    barrier = release.reconcile_registry_barrier(
                        [item], registry_api=release.CRATES_IO_API, attempts=2, delay_seconds=0,
                    )
        self.assertEqual(barrier.state, "complete")

    def test_registry_barrier_stops_on_different_bytes(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            item = self.prepared(Path(temp_dir))
            with mock.patch.object(
                release,
                "fetch_crates_io_checksum",
                return_value="5" * 64,
            ):
                with mock.patch.object(release, "fetch_crates_io_index_checksum", return_value=self.DIGEST):
                    barrier = release.reconcile_registry_barrier(
                        [item], registry_api=release.CRATES_IO_API, attempts=2, delay_seconds=0,
                    )
        self.assertEqual(barrier.state, "mismatch")

    def test_initial_preflight_dry_runs_only_missing_versions(self) -> None:
        commands: list[list[str]] = []
        prepared = release.PreparedCrate(
            publish.PackageInfo(
                "alpha",
                "1.0.0",
                ROOT / "crates" / "alpha" / "Cargo.toml",
                (),
            ),
            ROOT / "target" / "package" / "alpha-1.0.0.crate",
            self.DIGEST,
            1,
            "4" * 64,
        )

        def run(command, **_kwargs):
            commands.append(list(command))
            return publish.subprocess.CompletedProcess(command, 0)

        with (
            mock.patch.object(
                release,
                "fetch_crates_io_checksum",
                side_effect=[self.DIGEST, None],
            ),
            mock.patch.object(
                release,
                "_prepare_crate",
                return_value=prepared,
            ),
            mock.patch.object(release, "run_command", side_effect=run),
        ):
            release.preflight_initial_batch(
                ROOT,
                package_metadata("alpha", "beta"),
            )
        self.assertEqual([command[3] for command in commands], ["beta"])

    def test_initial_preflight_rejects_existing_version_with_different_bytes(self) -> None:
        prepared = release.PreparedCrate(
            publish.PackageInfo(
                "alpha",
                "1.0.0",
                ROOT / "crates" / "alpha" / "Cargo.toml",
                (),
            ),
            ROOT / "target" / "package" / "alpha-1.0.0.crate",
            self.DIGEST,
            1,
            "4" * 64,
        )
        with (
            mock.patch.object(
                release,
                "fetch_crates_io_checksum",
                return_value="5" * 64,
            ),
            mock.patch.object(
                release,
                "_prepare_crate",
                return_value=prepared,
            ),
        ):
            with self.assertRaisesRegex(
                release.CratesIoPublishError,
                "registry checksum mismatch for alpha 1.0.0",
            ):
                release.preflight_initial_batch(ROOT, package_metadata("alpha"))

    def test_independent_preflight_rejects_existing_version_with_different_bytes(self) -> None:
        prepared = self.prepared(Path(tempfile.mkdtemp()), "roughr-merman")
        metadata = package_metadata("roughr-merman")
        with (
            mock.patch.object(release, "_prepare_crate", return_value=prepared),
            mock.patch.object(
                release,
                "fetch_crates_io_checksum",
                return_value="5" * 64,
            ),
        ):
            with self.assertRaisesRegex(
                release.CratesIoPublishError,
                "registry checksum mismatch for roughr-merman 1.0.0",
            ):
                release.preflight_independent_crate(
                    ROOT,
                    metadata,
                    package_name="roughr-merman",
                    expected_version="1.0.0",
                )

    def test_independent_preflight_resolves_package_outside_coupled_graph(self) -> None:
        prepared = self.prepared(Path(tempfile.mkdtemp()), "roughr-merman")
        metadata = package_metadata("alpha", "roughr-merman")
        metadata["metadata"] = {
            "merman-release": {"independent-packages": ["roughr-merman"]}
        }
        with (
            mock.patch.object(release, "_prepare_crate", return_value=prepared),
            mock.patch.object(
                release,
                "fetch_crates_io_checksum",
                return_value=self.DIGEST,
            ),
        ):
            self.assertTrue(
                release.preflight_independent_crate(
                    ROOT,
                    metadata,
                    package_name="roughr-merman",
                    expected_version="1.0.0",
                )
            )

    def test_independent_preflight_dry_runs_missing_version(self) -> None:
        prepared = self.prepared(Path(tempfile.mkdtemp()), "roughr-merman")
        metadata = package_metadata("roughr-merman")
        commands: list[list[str]] = []

        def run(command, **_kwargs):
            commands.append(list(command))
            return publish.subprocess.CompletedProcess(command, 0)

        with (
            mock.patch.object(release, "_prepare_crate", return_value=prepared),
            mock.patch.object(release, "fetch_crates_io_checksum", return_value=None),
            mock.patch.object(release, "run_command", side_effect=run),
        ):
            self.assertFalse(
                release.preflight_independent_crate(
                    ROOT,
                    metadata,
                    package_name="roughr-merman",
                    expected_version="1.0.0",
                )
            )
        self.assertEqual(commands[0][0:4], ["cargo", "publish", "-p", "roughr-merman"])

    def test_receipt_validation_rejects_bad_artifact_identity(self) -> None:
        receipt = {
            "schema_version": 1,
            "schema": release.RECEIPT_SCHEMA.as_posix(),
            "channel": "crates.io",
            "kind": "topological-batch",
            "state": "prepared",
            "source": {"commit": self.SOURCE_SHA, "tree": self.SOURCE_TREE},
            "toolchain": {"cargo": "cargo", "rustc": "rustc"},
            "plan_sha256": self.PLAN_DIGEST,
            "batch_index": 0,
            "packages": [{"name": "alpha", "artifact": {"sha256": "bad", "size": 1}}],
        }
        with self.assertRaises(release.CratesIoPublishError):
            release.validate_crates_io_receipt(receipt)

    def test_generated_receipt_matches_owner_schema_shape(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            receipt = self.receipt(root, [self.prepared(root)], "prepared")
        schema = json.loads((ROOT / release.RECEIPT_SCHEMA).read_text(encoding="utf-8"))
        package_schema = schema["$defs"]["package"]
        artifact_schema = package_schema["properties"]["artifact"]
        registry_schema = package_schema["properties"]["registry"]
        self.assertTrue(set(schema["required"]).issubset(receipt))
        self.assertTrue(set(receipt).issubset(schema["properties"]))
        self.assertIs(receipt["observe_only"], False)
        self.assertEqual(set(receipt["packages"][0]), set(package_schema["required"]))
        self.assertEqual(
            set(receipt["packages"][0]["artifact"]),
            set(artifact_schema["required"]),
        )
        self.assertEqual(
            set(receipt["packages"][0]["registry"]),
            set(registry_schema["required"]),
        )

    def test_response_loss_reconciles_without_second_publish(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            item = self.prepared(root)
            plan = publish.PublishPlan((("alpha",),), ("alpha",), {"alpha": item.package})
            commands: list[list[str]] = []

            def run(command, **_kwargs):
                commands.append(list(command))
                return publish.subprocess.CompletedProcess(
                    command,
                    7 if "--no-verify" in command else 0,
                )

            with self.mocked_release(
                plan,
                prepare=lambda *_args: item,
                run=run,
                checksum=sequence(None, self.DIGEST, self.DIGEST),
            ):
                self.invoke(root)
            self.assertEqual(
                sum("--no-verify" in command for command in commands),
                1,
            )
            result = json.loads(
                (root / "receipts" / "batch-000-result.json").read_text()
            )
            self.assertEqual(
                result["packages"][0]["registry"]["status"],
                "published_after_response_loss",
            )

    def test_partial_recovery_skips_matching_version_without_publish(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            item = self.prepared(root)
            plan = publish.PublishPlan((("alpha",),), ("alpha",), {"alpha": item.package})
            with self.mocked_release(
                plan,
                prepare=lambda *_args: item,
                run=lambda *_args, **_kwargs: self.fail("publish command must not run"),
                checksum=lambda *_args, **_kwargs: self.DIGEST,
            ):
                self.invoke(root)
            result = json.loads(
                (root / "receipts" / "batch-000-result.json").read_text()
            )
            self.assertEqual(
                result["packages"][0]["registry"]["status"],
                "already_published",
            )

    def test_existing_different_bytes_stop_before_publish(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            item = self.prepared(root)
            plan = publish.PublishPlan((("alpha",),), ("alpha",), {"alpha": item.package})
            with (
                self.mocked_release(
                    plan,
                    prepare=lambda *_args: item,
                    run=lambda *_args, **_kwargs: self.fail("publish command must not run"),
                    checksum=lambda *_args, **_kwargs: "5" * 64,
                ),
                self.assertRaisesRegex(release.CratesIoPublishError, "checksum mismatch"),
            ):
                self.invoke(root)

    def test_all_dry_runs_finish_before_any_batch_publish(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            alpha = self.prepared(root, "alpha")
            beta = self.prepared(root, "beta")
            plan = publish.PublishPlan(
                (("alpha", "beta"),),
                ("alpha", "beta"),
                {"alpha": alpha.package, "beta": beta.package},
            )
            commands: list[list[str]] = []

            def run(command, **_kwargs):
                commands.append(list(command))
                return publish.subprocess.CompletedProcess(
                    command,
                    4 if "--dry-run" in command and "beta" in command else 0,
                )

            with (
                self.mocked_release(
                    plan,
                    prepare=sequence(alpha, beta),
                    run=run,
                    checksum=lambda *_args, **_kwargs: None,
                ),
                self.assertRaisesRegex(release.CratesIoPublishError, "dry-run failed"),
            ):
                self.invoke(root)
            self.assertEqual(
                [command[3] for command in commands if "--dry-run" in command],
                ["alpha", "beta"],
            )
            self.assertFalse(any("--no-verify" in command for command in commands))

    def test_recovery_reads_receipts_and_publishes_only_missing_members(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            alpha = self.prepared(root, "alpha")
            beta = self.prepared(root, "beta")
            prior = root / "recovery" / "attempt-1"
            for state, suffix in [("prepared", "prepared"), ("pending_recovery", "result")]:
                receipt = self.receipt(root, [alpha, beta], state)
                receipt["publication"] = {"run_id": "42", "attempt": 1}
                release._write_receipt(prior / f"batch-000-{suffix}.json", receipt)
            plan = publish.PublishPlan(
                (("alpha", "beta"),),
                ("alpha", "beta"),
                {"alpha": alpha.package, "beta": beta.package},
            )
            commands: list[tuple[list[str], dict[str, str] | None]] = []

            def run(command, **kwargs):
                commands.append((list(command), kwargs.get("env")))
                return publish.subprocess.CompletedProcess(command, 0)

            with self.mocked_release(
                plan,
                prepare=sequence(alpha, beta),
                run=run,
                checksum=sequence(self.DIGEST, None, self.DIGEST, self.DIGEST),
            ):
                self.invoke(root, recovery=root / "recovery", recovery_run_id="42", recovery_attempt=1)
            self.assertEqual(
                [command[3] for command, _env in commands],
                ["beta", "beta"],
            )
            self.assertNotIn(
                "CARGO_REGISTRY_TOKEN",
                commands[0][1] or {},
            )
            self.assertEqual(commands[1][1]["CARGO_REGISTRY_TOKEN"], "test-token")

    def test_api_acceptance_without_index_stops_before_preparing_next_batch(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            alpha = self.prepared(root, "alpha")
            beta = self.prepared(root, "beta", ("alpha",))
            plan = publish.PublishPlan((("alpha",), ("beta",)), ("alpha", "beta"),
                                       {"alpha": alpha.package, "beta": beta.package})
            prepared_names = []

            def prepare(_root, _target, package):
                prepared_names.append(package.name)
                return alpha if package.name == "alpha" else beta

            with self.mocked_release(
                plan, prepare=prepare,
                run=lambda command, **kwargs: publish.subprocess.CompletedProcess(command, 0),
                checksum=sequence(None, self.DIGEST, self.DIGEST),
            ), mock.patch.object(release, "fetch_crates_io_index_checksum", return_value=None):
                with self.assertRaisesRegex(release.CratesIoPublishError, "pending_recovery"):
                    self.invoke(root)
            self.assertEqual(prepared_names, ["alpha"])
            result = json.loads((root / "receipts/batch-000-result.json").read_text())
            self.assertEqual(result["packages"][0]["registry"]["status"], "accepted_pending_index")
            self.assertIsNone(result["packages"][0]["registry"]["observed_index_checksum"])

    def test_repeated_recovery_404_cannot_republish_an_attempted_upload(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            item = self.prepared(root)
            plan = publish.PublishPlan((("alpha",),), ("alpha",), {"alpha": item.package})
            recovery = root / "receipts"
            release._write_receipt(recovery / "batch-000-prepared.json", self.receipt(root, [item], "prepared"))
            prior = self.receipt(root, [item], "pending_recovery")
            prior["packages"][0]["registry"]["status"] = "publish_started"
            release._write_receipt(recovery / "batch-000-result.json", prior)
            for _ in range(2):
                with self.mocked_release(
                    plan, prepare=lambda *_args: item,
                    run=lambda *_args, **_kwargs: self.fail("an attempted upload cannot be sent twice"),
                    checksum=lambda *_args, **_kwargs: None,
                ), mock.patch.object(release, "fetch_crates_io_index_checksum", return_value=None):
                    with self.assertRaisesRegex(release.CratesIoPublishError, "do not republish"):
                        self.invoke(root, recovery=recovery)
                result = json.loads((recovery / "batch-000-result.json").read_text())
                self.assertEqual(result["packages"][0]["registry"]["status"], "publish_response_lost")

    def test_receipt_attempt_gap_makes_old_missing_members_observation_only(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            alpha = self.prepared(root, "alpha")
            beta = self.prepared(root, "beta")
            plan = publish.PublishPlan((("alpha", "beta"),), ("alpha", "beta"),
                                       {"alpha": alpha.package, "beta": beta.package})
            recovery = root / "old-receipts"
            for state, suffix in [("prepared", "prepared"), ("pending_recovery", "result")]:
                receipt = self.receipt(root, [alpha, beta], state)
                receipt["publication"] = {"run_id": "42", "attempt": 1}
                if state != "prepared":
                    receipt["packages"][0]["registry"]["status"] = "published"
                    receipt["packages"][1]["registry"]["status"] = "missing"
                release._write_receipt(recovery / f"batch-000-{suffix}.json", receipt)
            with self.mocked_release(
                plan, prepare=sequence(alpha, beta),
                run=lambda *_args, **_kwargs: self.fail("missing attempt 2 may already have uploaded beta"),
                checksum=lambda name, *_args, **_kwargs: self.DIGEST if name == "alpha" else None,
            ):
                with self.assertRaisesRegex(release.CratesIoPublishError, "do not republish"):
                    self.invoke(root, recovery=recovery, publication_run_id="42", publication_attempt=3,
                                recovery_run_id="42", recovery_attempt=2)
            result = json.loads((root / "receipts/batch-000-result.json").read_text())
            self.assertEqual(result["publication"], {"run_id": "42", "attempt": 3})
            self.assertEqual(result["packages"][1]["registry"]["status"], "publish_response_lost")

    def test_missing_recovery_artifact_never_downgrades_to_fresh_publication(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            item = self.prepared(root)
            plan = publish.PublishPlan((("alpha",),), ("alpha",), {"alpha": item.package})
            with self.mocked_release(
                plan, prepare=lambda *_args: item,
                run=lambda *_args, **_kwargs: self.fail("missing receipt evidence cannot authorize upload"),
                checksum=lambda *_args, **_kwargs: None,
            ):
                with self.assertRaisesRegex(release.CratesIoPublishError, "do not republish"):
                    self.invoke(root, publication_run_id="42", publication_attempt=2,
                                recovery_run_id="42", recovery_attempt=1)

    def test_complete_prefix_cannot_launder_unknown_later_batch_into_attempt_four(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            alpha = self.prepared(root, "alpha")
            beta = self.prepared(root, "beta", ("alpha",))
            plan = publish.PublishPlan((("alpha",), ("beta",)), ("alpha", "beta"),
                                       {"alpha": alpha.package, "beta": beta.package})
            uploads = []

            def prepare(_root, _target, package):
                return alpha if package.name == "alpha" else beta

            def prepare_failure(_root, _target, package):
                if package.name == "beta":
                    raise release.CratesIoPublishError("temporary beta preparation failure")
                return alpha

            def checksum(name, *_args, **_kwargs):
                return self.DIGEST if name == "alpha" else None

            def run(command, **_kwargs):
                if "--no-verify" in command:
                    uploads.append(command[3])
                    raise KeyboardInterrupt("runner lost after sending beta")
                return publish.subprocess.CompletedProcess(command, 0)

            # Attempt 1 leaves a complete alpha prefix, with beta never prepared.
            with self.mocked_release(plan, prepare=prepare_failure, run=run, checksum=checksum):
                with self.assertRaisesRegex(release.CratesIoPublishError, "preparation failure"):
                    self.invoke(root, receipts_dir=root / "attempt-1", publication_run_id="42", publication_attempt=1)
            # Attempt 2 really sends beta, but none of its receipts survive as artifacts.
            with self.mocked_release(plan, prepare=prepare, run=run, checksum=checksum):
                with self.assertRaises(KeyboardInterrupt):
                    self.invoke(root, recovery=root / "attempt-1", receipts_dir=root / "attempt-2",
                                publication_run_id="42", publication_attempt=2,
                                recovery_run_id="42", recovery_attempt=1)
            # Attempt 3 observes an exact alpha, then fails before preparing beta.
            with self.mocked_release(plan, prepare=prepare_failure, run=run, checksum=checksum):
                with self.assertRaisesRegex(release.CratesIoPublishError, "preparation failure"):
                    self.invoke(root, recovery=root / "attempt-1", receipts_dir=root / "attempt-3",
                                publication_run_id="42", publication_attempt=3,
                                recovery_run_id="42", recovery_attempt=2)
            third = json.loads((root / "attempt-3/batch-000-result.json").read_text())
            self.assertEqual(third["state"], "complete")
            self.assertIs(third["observe_only"], True)
            self.assertFalse((root / "attempt-3/batch-001-prepared.json").exists())
            # Matching provenance on attempt 4 must not erase the inherited uncertainty.
            with self.mocked_release(plan, prepare=prepare, run=run, checksum=checksum):
                with self.assertRaisesRegex(release.CratesIoPublishError, "do not republish"):
                    self.invoke(root, recovery=root / "attempt-3", receipts_dir=root / "attempt-4",
                                publication_run_id="42", publication_attempt=4,
                                recovery_run_id="42", recovery_attempt=3)
            self.assertEqual(uploads, ["beta"])
            fourth = json.loads((root / "attempt-4/batch-001-result.json").read_text())
            self.assertIs(fourth["observe_only"], True)

    def test_prepared_only_receipt_preserves_run_wide_observation_for_future_batches(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            alpha = self.prepared(root, "alpha")
            beta = self.prepared(root, "beta", ("alpha",))
            plan = publish.PublishPlan((("alpha",), ("beta",)), ("alpha", "beta"),
                                       {"alpha": alpha.package, "beta": beta.package})

            def prepare(_root, _target, package):
                return alpha if package.name == "alpha" else beta

            def interrupted(*_args, **_kwargs):
                raise KeyboardInterrupt()

            with self.mocked_release(plan, prepare=prepare, run=lambda *_args, **_kwargs: self.fail("no upload"),
                                     checksum=interrupted):
                with self.assertRaises(KeyboardInterrupt):
                    self.invoke(root, observe_only=True, receipts_dir=root / "attempt-3",
                                publication_run_id="42", publication_attempt=3)
            third = json.loads((root / "attempt-3/batch-000-prepared.json").read_text())
            self.assertIs(third["observe_only"], True)
            self.assertFalse((root / "attempt-3/batch-000-result.json").exists())
            with self.mocked_release(
                plan, prepare=prepare, run=lambda *_args, **_kwargs: self.fail("unknown beta cannot be sent"),
                checksum=lambda name, *_args, **_kwargs: self.DIGEST if name == "alpha" else None,
            ):
                with self.assertRaisesRegex(release.CratesIoPublishError, "do not republish"):
                    self.invoke(root, recovery=root / "attempt-3", receipts_dir=root / "attempt-4",
                                publication_run_id="42", publication_attempt=4,
                                recovery_run_id="42", recovery_attempt=3)

    def test_existing_version_evidence_survives_a_later_replica_404(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            item = self.prepared(root)
            plan = publish.PublishPlan((("alpha",),), ("alpha",), {"alpha": item.package})
            with self.mocked_release(
                plan, prepare=lambda *_args: item,
                run=lambda *_args, **_kwargs: self.fail("an existing version cannot be uploaded"),
                checksum=sequence(self.DIGEST, None, None),
            ):
                with self.assertRaisesRegex(release.CratesIoPublishError, "pending_recovery"):
                    self.invoke(root)
            result = json.loads((root / "receipts/batch-000-result.json").read_text())
            self.assertEqual(result["packages"][0]["registry"]["status"], "publish_response_lost")
            with self.mocked_release(
                plan, prepare=lambda *_args: item,
                run=lambda *_args, **_kwargs: self.fail("earlier existence remains observation-only"),
                checksum=lambda *_args, **_kwargs: None,
            ):
                with self.assertRaisesRegex(release.CratesIoPublishError, "do not republish"):
                    self.invoke(root, recovery=root / "receipts")

    def test_recovery_lookup_failure_preserves_prior_upload_attempt(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            item = self.prepared(root)
            plan = publish.PublishPlan((("alpha",),), ("alpha",), {"alpha": item.package})
            recovery = root / "receipts"
            release._write_receipt(recovery / "batch-000-prepared.json", self.receipt(root, [item], "prepared"))
            prior = self.receipt(root, [item], "pending_recovery")
            prior["packages"][0]["registry"]["status"] = "publish_started"
            release._write_receipt(recovery / "batch-000-result.json", prior)
            def unavailable(*_args, **_kwargs):
                raise release.CratesIoPublishError("HTTP 503")

            with self.mocked_release(
                plan, prepare=lambda *_args: item,
                run=lambda *_args, **_kwargs: self.fail("cannot upload during a registry outage"),
                checksum=unavailable,
            ):
                with self.assertRaisesRegex(release.CratesIoPublishError, "HTTP 503"):
                    self.invoke(root, recovery=recovery)
            result = json.loads((recovery / "batch-000-result.json").read_text())
            self.assertEqual(result["packages"][0]["registry"]["status"], "publish_started")

    def test_interruption_persists_upload_attempt_before_subprocess(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            item = self.prepared(root)
            plan = publish.PublishPlan((("alpha",),), ("alpha",), {"alpha": item.package})

            def run(command, **kwargs):
                if "--no-verify" in command:
                    raise KeyboardInterrupt()
                return publish.subprocess.CompletedProcess(command, 0)

            with self.mocked_release(plan, prepare=lambda *_args: item, run=run,
                                     checksum=lambda *_args, **_kwargs: None):
                with self.assertRaises(KeyboardInterrupt):
                    self.invoke(root)
            result = json.loads((root / "receipts/batch-000-result.json").read_text())
            self.assertEqual(result["packages"][0]["registry"]["status"], "publish_started")

    def test_recovery_rejects_toolchain_or_artifact_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            item = self.prepared(root)
            current = self.receipt(root, [item], "prepared")
            prior = json.loads(json.dumps(current))
            prior["toolchain"]["cargo"] = "cargo-old"
            prior["packages"][0]["artifact"]["sha256"] = "5" * 64
            with self.assertRaisesRegex(release.CratesIoPublishError, "identity differs"):
                release._require_recovery_identity(
                    current,
                    [(root / "batch-000-prepared.json", prior)],
                )

    def test_recovery_result_without_prepared_receipt_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            item = self.prepared(root)
            recovery = root / "recovery"
            release._write_receipt(
                recovery / "batch-000-result.json",
                self.receipt(root, [item], "pending_recovery"),
            )
            with self.assertRaisesRegex(
                release.CratesIoPublishError,
                "without a prepared receipt",
            ):
                release._load_recovery_receipts(recovery)

    def test_recovery_receipts_must_form_a_contiguous_batch_prefix(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            item = self.prepared(root)
            receipt = self.receipt(root, [item], "prepared")
            receipt["batch_index"] = 1
            recovery = root / "recovery"
            release._write_receipt(recovery / "batch-001-prepared.json", receipt)
            with self.assertRaisesRegex(
                release.CratesIoPublishError,
                "contiguous batch prefix",
            ):
                release._load_recovery_receipts(recovery)

    def test_pending_response_stops_before_next_batch(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            alpha = self.prepared(root, "alpha")
            beta = self.prepared(root, "beta", ("alpha",))
            plan = publish.PublishPlan(
                (("alpha",), ("beta",)),
                ("alpha", "beta"),
                {"alpha": alpha.package, "beta": beta.package},
            )
            prepared_names: list[str] = []

            def prepare(_root, _target, package):
                prepared_names.append(package.name)
                return alpha if package.name == "alpha" else beta

            def run(command, **_kwargs):
                return publish.subprocess.CompletedProcess(
                    command,
                    9 if "--no-verify" in command else 0,
                )

            with (
                self.mocked_release(
                    plan,
                    prepare=prepare,
                    run=run,
                    checksum=lambda *_args, **_kwargs: None,
                ),
                self.assertRaisesRegex(release.CratesIoPublishError, "pending_recovery"),
            ):
                self.invoke(root)
            self.assertEqual(prepared_names, ["alpha"])


class SparseIndexBoundaryTests(unittest.TestCase):
    @contextmanager
    def registry(self, *, api_delay=0, index_delay=0, index_checksum=None, yanked=False):
        state = {"now": 0, "paths": [], "sleeps": []}
        digest = CratesIoReceiptTests.DIGEST

        class Handler(BaseHTTPRequestHandler):
            def log_message(self, *_args):
                pass

            def do_GET(self):
                state["paths"].append(self.path)
                is_api = self.path.startswith("/api/")
                visible = state["now"] >= (api_delay if is_api else index_delay)
                if not visible:
                    self.send_response(404)
                    self.end_headers()
                    return
                if is_api:
                    payload = {"version": {"checksum": digest}}
                else:
                    payload = {"name": "alpha", "vers": "1.0.0", "cksum": index_checksum or digest, "yanked": yanked}
                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.end_headers()
                self.wfile.write((json.dumps(payload) + "\n").encode())

        server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        thread = threading.Thread(target=lambda: server.serve_forever(poll_interval=0.01), daemon=True)
        thread.start()

        def sleep(seconds):
            state["sleeps"].append(seconds)
            state["now"] += seconds

        try:
            with (tempfile.TemporaryDirectory() as temp_dir,
                  mock.patch.object(release.time, "sleep", side_effect=sleep),
                  mock.patch.object(release.time, "monotonic", side_effect=lambda: state["now"])):
                item = CratesIoReceiptTests().prepared(Path(temp_dir))
                endpoint = f"http://127.0.0.1:{server.server_port}"
                yield item, endpoint, state
        finally:
            server.shutdown()
            server.server_close()
            thread.join(timeout=2)

    def barrier(self, item, endpoint):
        return release.reconcile_registry_barrier(
            [item], registry_api=endpoint + "/api/v1", registry_index=endpoint,
            attempts=4, delay_seconds=5,
        )

    def test_api_checksum_does_not_complete_before_consumable_index(self) -> None:
        with self.registry(index_delay=30) as (item, endpoint, clock):
            barrier = self.barrier(item, endpoint)
        self.assertEqual(barrier.state, "complete")
        self.assertEqual(clock["sleeps"], [5, 10, 20])
        self.assertIn("/al/ph/alpha", clock["paths"])
        self.assertEqual(barrier.index_checksums["alpha"], item.artifact_sha256)

    def test_index_visibility_cannot_bypass_missing_api_checksum(self) -> None:
        with self.registry(api_delay=1000) as (item, endpoint, clock):
            barrier = self.barrier(item, endpoint)
        self.assertEqual(barrier.state, "pending_recovery")
        self.assertEqual(barrier.index_checksums["alpha"], item.artifact_sha256)
        self.assertIsNone(barrier.checksums["alpha"])
        self.assertIn("API propagation", barrier.errors["alpha"])

    def test_conflicting_index_bytes_stop_on_first_observation(self) -> None:
        with self.registry(index_checksum="f" * 64) as (item, endpoint, clock):
            barrier = self.barrier(item, endpoint)
        self.assertEqual(barrier.state, "mismatch")
        self.assertEqual(clock["sleeps"], [])

    def test_observation_budget_expires_before_attempt_limit(self) -> None:
        with self.registry(index_delay=1000) as (item, endpoint, clock), \
                mock.patch.object(release, "REGISTRY_OBSERVATION_SECONDS", 12):
            barrier = self.barrier(item, endpoint)
        self.assertEqual(barrier.state, "pending_recovery")
        self.assertEqual(clock["now"], 12)
        self.assertEqual(clock["sleeps"], [5, 7])

    def test_yanked_index_version_is_a_conflict(self) -> None:
        with self.registry(yanked=True) as (item, endpoint, clock):
            barrier = self.barrier(item, endpoint)
        self.assertEqual(barrier.state, "mismatch")
        self.assertEqual(clock["sleeps"], [])


if __name__ == "__main__":
    unittest.main()
