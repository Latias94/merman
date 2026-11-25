#!/usr/bin/env python3
"""Unit tests for receipt-bound crates.io publication and recovery."""

from __future__ import annotations

from contextlib import contextmanager, ExitStack
from dataclasses import replace
import http.client
import io
import json
import os
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import struct
import tarfile
import threading
from pathlib import Path
import tempfile
import unittest
from unittest import mock

from scripts import crates_io_archive, crates_io_release as release
from scripts import renderer_fork_consumer
from scripts.verify_crate_package_legal_materials import PackageLegalMaterialError
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
            registry_token=kwargs.pop("registry_token", "test-token"),
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
            mock.patch.object(release, "verify_package_archive"),
            mock.patch.object(
                release,
                "fetch_crates_io_checksum",
                return_value="5" * 64,
            ),
        ):
            with self.assertRaisesRegex(
                release.CratesIoPublishError,
                "cannot confirm registry state for roughr-merman 1.0.0",
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
            mock.patch.object(release, "verify_package_archive"),
            mock.patch.object(release, "_assert_artifact_unchanged"),
            mock.patch.object(
                release,
                "fetch_crates_io_checksum",
                return_value=self.DIGEST,
            ),
            mock.patch.object(release, "fetch_crates_io_index_checksum", return_value=self.DIGEST),
        ):
            self.assertEqual(
                release.preflight_independent_crate(
                    ROOT,
                    metadata,
                    package_name="roughr-merman",
                    expected_version="1.0.0",
                ), (True, prepared),
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
            mock.patch.object(release, "verify_package_archive"),
            mock.patch.object(release, "_assert_artifact_unchanged"),
            mock.patch.object(release, "fetch_crates_io_checksum", return_value=None),
            mock.patch.object(release, "fetch_crates_io_index_checksum", return_value=None),
            mock.patch.object(release, "run_command", side_effect=run),
        ):
            self.assertEqual(
                release.preflight_independent_crate(
                    ROOT,
                    metadata,
                    package_name="roughr-merman",
                    expected_version="1.0.0",
                ), (False, prepared),
            )
        self.assertEqual(commands[0][0:4], ["cargo", "publish", "-p", "roughr-merman"])

    def test_renderer_fork_bootstrap_and_supplemental_preflight_share_the_prepared_archive(self) -> None:
        metadata = package_metadata("alpha", *renderer_fork_consumer.FORK_REQUIREMENTS)
        metadata["metadata"] = {
            "merman-release": {"independent-packages": list(renderer_fork_consumer.FORK_REQUIREMENTS)}
        }
        with tempfile.TemporaryDirectory() as temporary:
            for name in renderer_fork_consumer.FORK_REQUIREMENTS:
                prepared = self.prepared(Path(temporary), name)
                with (
                    self.subTest(package=name),
                    mock.patch.object(release, "_prepare_crate", return_value=prepared),
                    mock.patch.object(release, "verify_package_archive"),
                    mock.patch.object(release, "_assert_artifact_unchanged") as assert_unchanged,
                    mock.patch.object(release, "fetch_crates_io_checksum", return_value=None),
                    mock.patch.object(release, "fetch_crates_io_index_checksum", return_value=None),
                    mock.patch.object(release, "run_command", return_value=publish.subprocess.CompletedProcess([], 0)),
                    mock.patch.object(renderer_fork_consumer, "verify") as verify,
                ):
                    self.assertEqual(release.preflight_independent_crate(
                        ROOT, metadata, package_name=name, expected_version="1.0.0"
                    ), (False, prepared))
                    verify.assert_not_called()
                    self.assertEqual(release.preflight_independent_crate(
                        ROOT, metadata, package_name=name, expected_version="1.0.0", verify_fork_consumer=True
                    ), (False, prepared))
                    self.assertEqual(assert_unchanged.call_args_list, [mock.call(prepared)] * 2)
                    verify.assert_called_once_with(
                        prepared.artifact_path, candidate=name, version="1.0.0",
                        repo_root=ROOT,
                        target_directory=ROOT / "target",
                    )

    def test_independent_preflight_stops_if_the_verified_archive_changes_during_dry_run(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            prepared = self.prepared(Path(temporary), "merman-resvg")
            prepared = replace(prepared, artifact_sha256=release.sha256_file(prepared.artifact_path))

            def change_archive(command, **_kwargs):
                self.assertIn("--dry-run", command)
                prepared.artifact_path.write_bytes(b"different registry sibling lockfile")
                return publish.subprocess.CompletedProcess(command, 0)

            with (
                mock.patch.object(release, "_prepare_crate", return_value=prepared),
                mock.patch.object(release, "verify_package_archive"),
                mock.patch.object(release, "independent_version_exists", return_value=False),
                mock.patch.object(release, "run_command", side_effect=change_archive),
                self.assertRaisesRegex(release.CratesIoPublishError, "artifact changed after receipt"),
            ):
                release.preflight_independent_crate(
                    ROOT, package_metadata("merman-resvg"), package_name="merman-resvg", expected_version="1.0.0",
                )

    def test_independent_preflight_verifies_the_special_licenses_before_registry_access(self) -> None:
        for name, expression, extra in (
            ("merman-krilla", "MIT OR Apache-2.0", "icc/LICENSE"),
            ("merman-resvg", "(MIT OR Apache-2.0) AND BSD-2-Clause", "LICENSE-BSD-2-Clause"),
        ):
            for corruption in ("missing", "truncated", "same length", "unchanged"):
                with self.subTest(name=name, corruption=corruption), tempfile.TemporaryDirectory() as temporary:
                    root = Path(temporary)
                    prepared = self.prepared(root, name)
                    metadata = package_metadata(name)
                    [package] = metadata["packages"]
                    package.update(license=expression, manifest_path=str(prepared.package.manifest_path))
                    with tarfile.open(prepared.artifact_path, "w:gz") as crate:
                        for filename in ("LICENSE-MIT", "LICENSE-APACHE", extra):
                            vendored = "krilla" if name == "merman-krilla" else "resvg"
                            expected = ((ROOT / "vendor" / vendored / filename).read_bytes()
                                        if filename == extra else b"test license")
                            source = prepared.package.manifest_path.parent / filename
                            source.parent.mkdir(parents=True, exist_ok=True)
                            source.write_bytes(expected)
                            if filename == extra and corruption == "missing":
                                continue
                            text = ({"truncated": b"X", "same length": b"X" * len(expected)}.get(corruption, expected)
                                    if filename == extra else expected)
                            member = tarfile.TarInfo(f"{name}-1.0.0/{filename}")
                            member.size = len(text)
                            crate.addfile(member, io.BytesIO(text))
                    prepared = replace(prepared, artifact_sha256=release.sha256_file(prepared.artifact_path),
                                       artifact_size=prepared.artifact_path.stat().st_size)
                    with (
                        mock.patch.object(release, "_prepare_crate", return_value=prepared),
                        mock.patch.object(release, "independent_version_exists", return_value=True) as registry,
                    ):
                        if corruption == "unchanged":
                            self.assertEqual(release.preflight_independent_crate(
                                root, metadata, package_name=name, expected_version="1.0.0",
                            ), (True, prepared))
                            registry.assert_called_once()
                        else:
                            with self.assertRaisesRegex(PackageLegalMaterialError, extra):
                                release.preflight_independent_crate(
                                    root, metadata, package_name=name, expected_version="1.0.0",
                                )
                            registry.assert_not_called()

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

    def test_each_upload_gets_a_fresh_token_after_all_batch_dry_runs(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            items = [self.prepared(root, name) for name in ("alpha", "beta")]
            plan = publish.PublishPlan((("alpha", "beta"),), ("alpha", "beta"),
                                       {item.package.name: item.package for item in items})
            events = []
            token_count = 0

            @contextmanager
            def credential():
                nonlocal token_count
                token_count += 1
                token = f"temporary-{token_count}"
                events.append(("auth", token))
                try:
                    yield token
                finally:
                    events.append(("revoke", token))

            def run(command, **kwargs):
                environment = kwargs["env"]
                if "--dry-run" in command:
                    self.assertNotIn("CARGO_REGISTRY_TOKEN", environment)
                    events.append(("dry-run", command[3]))
                else:
                    events.append(("upload", environment["CARGO_REGISTRY_TOKEN"]))
                return publish.subprocess.CompletedProcess(command, 0)

            with self.mocked_release(plan, prepare=sequence(*items), run=run,
                                     checksum=sequence(None, None, self.DIGEST, self.DIGEST)):
                self.invoke(root, registry_token=None, token_provider=credential)
            self.assertEqual(events, [("dry-run", "alpha"), ("dry-run", "beta"),
                                      ("auth", "temporary-1"), ("upload", "temporary-1"),
                                      ("revoke", "temporary-1"), ("auth", "temporary-2"),
                                      ("upload", "temporary-2"), ("revoke", "temporary-2")])

    def test_auth_failure_does_not_mark_an_upload_as_attempted(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            item = self.prepared(root)
            plan = publish.PublishPlan((("alpha",),), ("alpha",), {"alpha": item.package})
            provider = mock.Mock(side_effect=RuntimeError("OIDC exchange rejected"))
            commands = []

            def run(command, **_kwargs):
                commands.append(command)
                return publish.subprocess.CompletedProcess(command, 0)

            with self.mocked_release(plan, prepare=sequence(item), run=run,
                                     checksum=lambda *_args, **_kwargs: None):
                with self.assertRaisesRegex(RuntimeError, "OIDC exchange rejected"):
                    self.invoke(root, registry_token=None, token_provider=provider)
            self.assertTrue(all("--dry-run" in command for command in commands))
            self.assertFalse((root / "receipts/batch-000-result.json").exists())

    def test_already_published_release_does_not_request_oidc_credentials(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            item = self.prepared(root)
            plan = publish.PublishPlan((("alpha",),), ("alpha",), {"alpha": item.package})
            provider = mock.Mock()
            with self.mocked_release(plan, prepare=sequence(item),
                                     run=mock.Mock(side_effect=AssertionError("No upload is needed")),
                                     checksum=lambda *_args, **_kwargs: self.DIGEST):
                self.invoke(root, registry_token=None, token_provider=provider)
            provider.assert_not_called()

    def test_cli_keeps_oidc_request_credentials_out_of_cargo_subprocesses(self) -> None:
        credentials = {
            "ACTIONS_ID_TOKEN_REQUEST_URL": "https://oidc.actions.githubusercontent.com/token",
            "ACTIONS_ID_TOKEN_REQUEST_TOKEN": "github-request-token",
        }

        def metadata(*_args, **_kwargs):
            for key in credentials:
                self.assertNotIn(key, os.environ)
            return {"target_directory": str(ROOT / "target")}

        with mock.patch.dict(os.environ, credentials), mock.patch.object(
            release.sys, "argv", ["release", "publish-receipted", "--trusted-publishing",
                                  "--source-sha", self.SOURCE_SHA, "--source-tree", self.SOURCE_TREE,
                                  "--receipts-dir", str(ROOT / "target/receipts")],
        ), mock.patch.object(release, "require_tool"), mock.patch.object(
            release, "cargo_metadata", side_effect=metadata,
        ), mock.patch.object(release, "publish_receipted_release") as publish_release:
            self.assertEqual(release.main(), 0)
            with mock.patch.object(release, "trusted_publishing_token") as exchange:
                publish_release.call_args.kwargs["token_provider"]()
            exchange.assert_called_once_with(request_url=credentials["ACTIONS_ID_TOKEN_REQUEST_URL"],
                                             request_token=credentials["ACTIONS_ID_TOKEN_REQUEST_TOKEN"])

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


class FrozenIndependentCrateTests(unittest.TestCase):
    name = "merman-resvg"
    version = "0.50.1"
    source = {"commit": "a" * 40, "tree": "b" * 40}
    publication = {"run_id": "42", "attempt": 1}
    repository = "example/project"

    def setUp(self) -> None:
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.original = self.root / f"{self.name}-{self.version}.crate"
        self.original.write_bytes(b"verified archive with the original dependency lockfile")
        self.directory = self.root / "frozen"
        self.candidate = self.freeze(self.directory, self.publication)
        self.shared = None
        self.enterContext(mock.patch.object(
            release.release_attempt_history, "read_independent_receipt", side_effect=lambda *_args: self.shared,
        ))
        self.shared_writes = self.enterContext(mock.patch.object(
            release.release_attempt_history, "record_independent_receipt", side_effect=self.record,
        ))

    def freeze(self, directory, publication):
        return crates_io_archive.freeze_candidate(
            directory, self.original, json.dumps({"name": self.name, "vers": self.version}).encode(),
            archive_sha256=release.sha256_file(self.original), name=self.name, version=self.version,
            source=self.source, publication=publication,
        )

    def record(self, repository, package, version, receipt):
        self.assertEqual((repository, package, version), (self.repository, self.name, self.version))
        self.shared = receipt

    def command(
        self, action: str, *, current_attempt: str = "1", manual: bool = False, share_receipts: bool = False,
    ) -> int:
        provenance = [] if manual else ["--publication-run-id", "42", "--publication-attempt", "1"]
        if share_receipts:
            provenance += ["--publication-repository", self.repository]
        with (
            mock.patch("sys.argv", [
                "release", action, "--candidate-dir", str(self.directory),
                "--package", self.name, "--version", self.version,
                "--source-sha", self.source["commit"], "--source-tree", self.source["tree"],
                "--sha256", self.candidate.archive_sha256, "--receipt-sha256", self.candidate.receipt_sha256,
                *provenance,
            ]),
            mock.patch.dict(os.environ, {
                "CARGO_REGISTRY_TOKEN": "dummy-token", "GITHUB_OUTPUT": str(self.root / "outputs"),
                "GITHUB_ACTIONS": "false" if manual else "true", "GITHUB_RUN_ID": "42",
                "GITHUB_RUN_ATTEMPT": current_attempt, "GITHUB_REPOSITORY": self.repository,
            }),
        ):
            return release.main()

    def test_publish_metadata_matches_cargo_feature_and_dependency_fields(self) -> None:
        manifest = b"""
[package]
name = "merman-resvg"
version = "0.50.1"

[features]
default = ["codec"]
codec = ["image-webp"]

[dependencies]
gif = { version = "0.14", optional = true }
image-webp = { package = "webp", version = "0.2", optional = true }
"""
        with tarfile.open(self.original, "w:gz") as crate:
            member = tarfile.TarInfo(f"{self.name}-{self.version}/Cargo.toml")
            member.size = len(manifest)
            crate.addfile(member, io.BytesIO(manifest))
        package = dict.fromkeys((
            "readme", "authors", "description", "documentation", "homepage", "keywords", "categories",
            "license", "license_file", "repository", "links", "rust_version",
        ))
        common = {
            "source": "registry+https://github.com/rust-lang/crates.io-index", "registry": None,
            "features": [], "optional": True, "uses_default_features": True, "target": None, "kind": None,
        }
        package.update(
            name=self.name, version=self.version, dependencies=[
                dict(common, name="gif", req="^0.14", rename=None),
                dict(common, name="webp", req="^0.2", rename="image-webp"),
            ],
            features={"default": ["codec"], "codec": ["image-webp"], "gif": ["dep:gif"],
                      "image-webp": ["dep:image-webp"]},
        )
        result = mock.Mock(returncode=0, stdout=json.dumps({"packages": [package]}))
        with mock.patch.object(crates_io_archive, "run_command", return_value=result):
            metadata = json.loads(crates_io_archive.publish_metadata(ROOT, self.original, self.name, self.version))
        self.assertEqual(metadata["features"], {"default": ["codec"], "codec": ["image-webp"]})
        expected = {"features": [], "optional": True, "default_features": True, "target": None, "kind": "normal"}
        self.assertEqual(metadata["deps"], [
            dict(expected, name="gif", version_req="^0.14"),
            dict(expected, name="webp", version_req="^0.2", explicit_name_in_toml="image-webp"),
        ])

    def test_candidate_or_receipt_digest_failure_prevents_any_registry_write(self) -> None:
        for filename in (self.original.name, "publish-metadata.json", "receipt.json"):
            with self.subTest(filename=filename):
                path = self.directory / filename
                original = path.read_bytes()
                path.write_bytes(original + b" ")
                try:
                    with (
                        mock.patch.object(release, "independent_version_exists") as registry,
                        mock.patch.object(release, "assert_release_source"),
                        mock.patch.object(crates_io_archive.http.client, "HTTPSConnection") as connection,
                    ):
                        self.assertEqual(self.command("publish-independent"), 1)
                    registry.assert_not_called()
                    connection.assert_not_called()
                    self.assertFalse((self.directory / "upload-attempted.json").exists())
                    self.assertFalse((self.root / "outputs").exists())
                finally:
                    path.write_bytes(original)

    def test_recovery_receipt_must_belong_to_original_package_source_and_attempt(self) -> None:
        origin = {"name": self.name, "version": self.version, "source": self.source, "publication": self.publication}
        self.assertEqual(crates_io_archive.load_candidate(self.directory, **origin), self.candidate)
        for mismatch in (
            {"name": "merman-usvg"}, {"version": "0.50.2"},
            {"source": {"commit": "f" * 40, "tree": self.source["tree"]}},
            {"publication": {"run_id": "41", "attempt": 1}},
            {"publication": {"run_id": "42", "attempt": 2}},
        ):
            with self.subTest(mismatch=mismatch), self.assertRaisesRegex(
                crates_io_archive.CrateArchiveError, "differs from its producing source or attempt",
            ):
                crates_io_archive.load_candidate(self.directory, **(origin | mismatch))

    def test_preflight_retains_the_exact_archive_for_an_already_published_version(self) -> None:
        self.directory = self.root / "confirmed"
        prepared = release.PreparedCrate(
            publish.PackageInfo(self.name, self.version, self.root / "Cargo.toml", ()),
            self.original, self.candidate.archive_sha256, len(self.candidate.archive), "a" * 64,
        )
        metadata = package_metadata(self.name)
        metadata["packages"][0]["version"] = self.version
        identity = dict(name=self.name, version=self.version, source=self.source, publication=self.publication)

        def confirm(*_args, **_kwargs):
            self.assertEqual(crates_io_archive.load_candidate(self.directory, **identity), self.candidate)
            return True

        with (
            mock.patch.object(release, "assert_release_source"),
            mock.patch.object(release, "require_tool"),
            mock.patch.object(release, "cargo_metadata", return_value=metadata),
            mock.patch.object(release, "_prepare_crate", return_value=prepared),
            mock.patch.object(release, "verify_package_archive"),
            mock.patch.object(release, "independent_version_exists", side_effect=confirm) as registry,
            mock.patch.object(crates_io_archive, "publish_metadata", side_effect=[
                crates_io_archive.CrateArchiveError("cannot freeze upload metadata"), self.candidate.metadata,
            ]),
        ):
            self.assertEqual(self.command("preflight-independent"), 1)
            self.assertFalse(self.directory.exists())
            registry.assert_not_called()
            self.assertEqual(self.command("preflight-independent"), 0)
            registry.assert_called_once()
        self.assertEqual(crates_io_archive.load_candidate(self.directory, **identity), self.candidate)
        self.assertIn("publish=false", (self.root / "outputs").read_text(encoding="utf-8").splitlines())
        self.assertEqual(self.shared.state, "registry-confirmed")

    def test_failed_registry_check_does_not_authorize_upload_and_can_be_retried(self) -> None:
        with (
            mock.patch.object(release, "assert_release_source"),
            mock.patch.object(release, "fetch_crates_io_checksum", side_effect=[
                release.CratesIoPublishError("HTTP 503"), None,
            ]) as api,
            mock.patch.object(release, "fetch_crates_io_index_checksum", return_value=None) as index,
            mock.patch.object(crates_io_archive, "upload") as upload,
        ):
            self.assertEqual(self.command("publish-independent"), 1)
            self.assertFalse((self.directory / "upload-attempted.json").exists())
            self.shared_writes.assert_not_called()
            api.assert_called_once()
            index.assert_not_called()
            upload.assert_not_called()

            self.assertEqual(self.command("publish-independent", current_attempt="2"), 1)
            upload.assert_not_called()
            self.assertEqual(self.command("publish-independent"), 0)
            self.assertEqual(api.call_count, 2)
            index.assert_called_once()
            upload.assert_called_once()

    def test_final_registry_check_that_confirms_archive_never_authorizes_upload(self) -> None:
        for manual in (False, True):
            with self.subTest(manual=manual):
                if manual:
                    self.shared = None
                    self.directory = self.root / "manual"
                    self.candidate = self.freeze(self.directory, None)
                with (
                    mock.patch.object(release, "assert_release_source"),
                    mock.patch.object(release, "independent_version_exists", side_effect=[True, False]),
                    mock.patch.object(crates_io_archive, "upload") as upload,
                ):
                    self.assertEqual(self.command("publish-independent", manual=manual), 0)
                    self.assertEqual((self.directory / "registry-confirmed.json").read_bytes(),
                                     self.candidate.receipt_bytes)
                    self.assertEqual(self.command("publish-independent", manual=manual), 1)
                    upload.assert_not_called()

    def test_failed_receipt_write_prevents_crates_io_put(self) -> None:
        with (
            mock.patch.object(release, "assert_release_source"),
            mock.patch.object(release, "independent_version_exists", return_value=False),
            mock.patch.object(crates_io_archive.http.client, "HTTPSConnection") as connect,
        ):
            self.shared_writes.side_effect = OSError("receipt store not reachable")
            self.assertEqual(self.command("publish-independent"), 1)
            connect.assert_not_called()
            self.assertFalse((self.directory / "upload-attempted.json").exists())

    def test_shared_manual_confirmation_blocks_a_later_workflow_send(self) -> None:
        workflow_directory, workflow_candidate = self.directory, self.candidate
        self.directory = self.root / "manual"
        self.candidate = self.freeze(self.directory, None)
        with (
            mock.patch.object(release, "assert_release_source"),
            mock.patch.object(release, "independent_version_exists", side_effect=[True, False]),
            mock.patch.object(crates_io_archive, "upload") as upload,
        ):
            self.assertEqual(self.command("publish-independent", manual=True, share_receipts=True), 0)
            self.directory, self.candidate = workflow_directory, workflow_candidate
            self.assertEqual(self.command("publish-independent"), 1)
            upload.assert_not_called()
        self.assertEqual(self.shared.state, "registry-confirmed")

    def test_registry_observation_confirms_the_intent_without_requiring_the_archive(self) -> None:
        self.shared = release.release_attempt_history.IndependentCrateReceipt(
            self.candidate.archive_sha256, self.source["commit"], "upload-intent",
        )
        args = ["release", "registry-independent", "--observe-only", "--package", self.name,
                "--version", self.version, "--sha256", self.shared.sha256,
                "--publication-repository", self.repository]
        with (
            mock.patch("sys.argv", args),
            mock.patch.object(release, "independent_version_exists", return_value=True),
            mock.patch.object(crates_io_archive, "upload") as upload,
        ):
            self.assertEqual(release.main(), 0)
            upload.assert_not_called()
        self.assertEqual(self.shared.state, "registry-confirmed")
        self.assertEqual(self.shared.source_sha, self.source["commit"])

    def test_lost_upload_response_retains_intent_and_never_repackages_or_resends(self) -> None:
        requests = []
        intent_path = self.directory / "upload-attempted.json"

        class Registry(BaseHTTPRequestHandler):
            def do_PUT(self):
                requests.append((
                    self.path, self.headers.get("Authorization"), self.headers.get("Content-Type"),
                    self.rfile.read(int(self.headers["Content-Length"])), intent_path.read_bytes(),
                    self.server.receipt_state(),
                ))
                # The registry can receive all the bytes and lose the reply.

        server = ThreadingHTTPServer(("127.0.0.1", 0), Registry)
        server.receipt_state = lambda: self.shared.state
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            self.original.write_bytes(b"a subsequent Cargo invocation resolved a newer compatible sibling")
            with (
                mock.patch.object(release, "assert_release_source"),
                mock.patch.object(release, "fetch_crates_io_checksum", return_value=None) as api,
                mock.patch.object(release, "fetch_crates_io_index_checksum", return_value=None) as index,
                mock.patch.object(release, "run_command") as cargo,
                mock.patch.object(
                    crates_io_archive.http.client, "HTTPSConnection",
                    side_effect=lambda *_args, **_kwargs: http.client.HTTPConnection(*server.server_address),
                ) as connect,
            ):
                self.assertEqual(self.command("publish-independent"), 1)
                self.directory = self.root / "new-run-local-state"
                self.directory.mkdir()
                for archived in intent_path.parent.iterdir():
                    if archived != intent_path:
                        (self.directory / archived.name).write_bytes(archived.read_bytes())
                self.assertEqual(self.command("publish-independent"), 1)
            self.assertEqual(api.call_count, 2)
            self.assertEqual(index.call_count, 2)
            self.shared_writes.assert_called_once()
            connect.assert_called_once()
            cargo.assert_not_called()
        finally:
            server.shutdown()
            server.server_close()
            thread.join(2)
        self.assertEqual(len(requests), 1)
        path, token, content_type, body, intent, shared = requests[0]
        self.assertEqual((path, token, content_type), ("/api/v1/crates/new", "dummy-token", "application/octet-stream"))
        stream = io.BytesIO(body)
        [metadata_size] = struct.unpack("<I", stream.read(4))
        self.assertEqual(stream.read(metadata_size), self.candidate.metadata)
        [archive_size] = struct.unpack("<I", stream.read(4))
        self.assertEqual(stream.read(archive_size), self.candidate.archive)
        self.assertFalse(stream.read())
        self.assertEqual(intent, self.candidate.receipt_bytes)
        self.assertEqual(shared, "upload-intent")

    def test_cargo_error_in_success_response_fails_without_resending(self) -> None:
        connection = mock.MagicMock()
        response = connection.getresponse.return_value.__enter__.return_value
        response.status = 200
        response.read.return_value = b'{"errors":[{"detail":"rejected"}]}'
        intent = self.directory / "upload-attempted.json"
        with mock.patch.object(crates_io_archive.http.client, "HTTPSConnection", return_value=connection):
            with self.assertRaisesRegex(crates_io_archive.CrateArchiveError, "registry reported crate upload errors"):
                crates_io_archive.upload(self.candidate, token="dummy-token", intent_path=intent)
            with self.assertRaisesRegex(crates_io_archive.CrateArchiveError, "has an upload intent"):
                crates_io_archive.upload(self.candidate, token="dummy-token", intent_path=intent)
        response.read.assert_called_once()
        connection.request.assert_called_once()
        self.assertEqual(intent.read_bytes(), self.candidate.receipt_bytes)


class SparseIndexBoundaryTests(unittest.TestCase):
    @contextmanager
    def registry(self, *, name="alpha", api_delay=0, index_delay=0, index_checksum=None,
                 yanked=False, api_yanked=False):
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
                    payload = {"version": {"checksum": digest, "yanked": api_yanked}}
                else:
                    payload = {"name": name, "vers": "1.0.0", "cksum": index_checksum or digest, "yanked": yanked}
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

    def test_independent_upload_state_requires_two_consistent_observations(self) -> None:
        for options, expected in (
            ({}, True),
            ({"api_delay": 10, "index_delay": 10}, False),
            ({"index_delay": 10}, "propagation"),
            ({"api_delay": 10}, "propagation"),
            ({"index_checksum": "f" * 64}, "cannot confirm"),
            ({"api_yanked": True}, "yanked"),
            ({"yanked": True}, "yanked"),
        ):
            with self.subTest(options=options), self.registry(**options) as (item, url, _):
                def inspect():
                    return release.independent_version_exists(
                        "alpha", "1.0.0", item.artifact_sha256,
                        registry_api=url + "/api/v1", registry_index=url,
                    )
                if isinstance(expected, str):
                    with self.assertRaisesRegex(release.CratesIoPublishError, expected):
                        inspect()
                else:
                    self.assertIs(inspect(), expected)

    def test_registry_cli_only_compiles_after_both_checksums_match(self) -> None:
        for delay, expected in ((1000, 1), (5, 0)):
            with self.subTest(delay=delay), self.registry(name="merman-usvg", api_delay=delay, index_delay=delay) as (item, url, clock):
                output = item.artifact_path.parent / "outputs"
                args = ["release", "registry-independent", "--observe-only", "--verify-fork-consumer",
                        "--package", "merman-usvg", "--version", "1.0.0", "--sha256", item.artifact_sha256,
                        "--registry-api", url + "/api/v1", "--registry-index", url,
                        "--visibility-attempts", "2", "--visibility-delay", "5"]
                with (mock.patch("sys.argv", args),
                      mock.patch.dict(os.environ, {"GITHUB_OUTPUT": str(output), "CARGO_TARGET_DIR": str(output.parent)}),
                      mock.patch.object(release, "run_command") as run,
                      mock.patch.object(renderer_fork_consumer, "verify") as verify):
                    self.assertEqual(release.main(), expected)
                run.assert_not_called()
                self.assertEqual(clock["sleeps"], [5])
                if expected:
                    verify.assert_not_called()
                    self.assertFalse(output.exists())
                else:
                    verify.assert_called_once_with(None, candidate="merman-usvg", version="1.0.0",
                                                   repo_root=ROOT, target_directory=output.parent)
                    self.assertIn("publish=false", output.read_text(encoding="utf-8").splitlines())

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
