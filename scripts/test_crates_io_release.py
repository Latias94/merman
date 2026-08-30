#!/usr/bin/env python3
"""Unit tests for receipt-bound crates.io publication and recovery."""

from __future__ import annotations

from contextlib import contextmanager, ExitStack
import json
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
            yield

    def invoke(
        self,
        root: Path,
        *,
        recovery: Path | None = None,
    ) -> None:
        release.publish_receipted_release(
            root,
            {"target_directory": str(root / "target")},
            source_sha=self.SOURCE_SHA,
            source_tree=self.SOURCE_TREE,
            receipts_dir=root / "receipts",
            registry_token="test-token",
            recovery_receipts_dir=recovery,
            visibility_attempts=2,
            visibility_delay=0,
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
                barrier = release.reconcile_registry_barrier(
                    [item],
                    registry_api=release.CRATES_IO_API,
                    attempts=2,
                    delay_seconds=0,
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
                barrier = release.reconcile_registry_barrier(
                    [item],
                    registry_api=release.CRATES_IO_API,
                    attempts=2,
                    delay_seconds=0,
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
        self.assertEqual(set(receipt), set(schema["required"]))
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
            release._write_receipt(
                prior / "batch-000-prepared.json",
                self.receipt(root, [alpha, beta], "prepared"),
            )
            release._write_receipt(
                prior / "batch-000-result.json",
                self.receipt(root, [alpha, beta], "pending_recovery"),
            )
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
                self.invoke(root, recovery=root / "recovery")
            self.assertEqual(
                [command[3] for command, _env in commands],
                ["beta", "beta"],
            )
            self.assertNotIn(
                "CARGO_REGISTRY_TOKEN",
                commands[0][1] or {},
            )
            self.assertEqual(commands[1][1]["CARGO_REGISTRY_TOKEN"], "test-token")

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


if __name__ == "__main__":
    unittest.main()
