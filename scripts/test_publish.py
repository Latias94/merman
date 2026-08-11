#!/usr/bin/env python3
"""Unit tests for crates.io publish helper metadata handling."""

import contextlib
import importlib.util
import io
import sys
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
MODULE_PATH = ROOT / "tools" / "publish.py"
SPEC = importlib.util.spec_from_file_location("publish_tool", MODULE_PATH)
assert SPEC is not None
publish_tool = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
sys.modules[SPEC.name] = publish_tool
SPEC.loader.exec_module(publish_tool)


class PublishMetadataTests(unittest.TestCase):
    def test_publish_field_allows_default_and_crates_io_registry(self) -> None:
        self.assertTrue(publish_tool.publish_field_allows_crates_io(None))
        self.assertTrue(publish_tool.publish_field_allows_crates_io(True))
        self.assertTrue(publish_tool.publish_field_allows_crates_io(["crates-io"]))

    def test_publish_field_rejects_publish_false_and_other_registries(self) -> None:
        self.assertFalse(publish_tool.publish_field_allows_crates_io([]))
        self.assertFalse(publish_tool.publish_field_allows_crates_io(False))
        self.assertFalse(publish_tool.publish_field_allows_crates_io(["internal"]))

    def test_workspace_packages_exclude_publish_false_metadata(self) -> None:
        metadata = workspace_metadata(
            package("xtask", publish=[]),
            package("merman-core"),
        )

        packages = publish_tool.workspace_package_infos(metadata)

        self.assertNotIn("xtask", packages)
        self.assertEqual(packages["merman-core"].version, "1.0.0")

    def test_crates_io_package_order_rejects_internal_registry_packages(self) -> None:
        metadata = workspace_metadata(
            package("default-publish"),
            package("explicit-crates-io", publish=["crates-io"]),
            package("internal-only", publish=["internal"]),
            package("publish-false", publish=[]),
        )

        self.assertEqual(
            publish_tool.crates_io_publish_order(metadata),
            ("default-publish", "explicit-crates-io"),
        )

    def test_no_verify_does_not_apply_to_preflight_dry_run(self) -> None:
        commands: list[list[str]] = []
        original_argv = sys.argv
        original_cargo_metadata = publish_tool.cargo_metadata
        original_git_is_clean = publish_tool.git_is_clean
        original_require_tool = publish_tool.require_tool
        original_run_command = publish_tool.run_command
        try:
            sys.argv = [
                "publish.py",
                "--crates",
                "merman-core",
                "--skip-xtask-verify",
                "--allow-dirty",
                "--yes",
                "--preflight-publish-dry-run",
                "--no-verify",
                "--no-check-published",
                "--wait",
                "0",
            ]
            publish_tool.cargo_metadata = lambda _repo_root, **_kwargs: workspace_metadata(
                package("merman-core")
            )
            publish_tool.git_is_clean = lambda _repo_root: True
            publish_tool.require_tool = lambda _name: None

            def run_command(cmd, **_kwargs):
                commands.append(list(cmd))
                return publish_tool.subprocess.CompletedProcess(args=cmd, returncode=0)

            publish_tool.run_command = run_command

            stdout = io.StringIO()
            stderr = io.StringIO()
            with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
                self.assertEqual(publish_tool.main(), 0)
        finally:
            sys.argv = original_argv
            publish_tool.cargo_metadata = original_cargo_metadata
            publish_tool.git_is_clean = original_git_is_clean
            publish_tool.require_tool = original_require_tool
            publish_tool.run_command = original_run_command

        self.assertIn(["cargo", "publish", "-p", "merman-core", "--dry-run"], commands)
        self.assertIn(["cargo", "publish", "-p", "merman-core", "--no-verify"], commands)
        preflight = commands[0]
        upload = commands[1]
        self.assertEqual(preflight, ["cargo", "publish", "-p", "merman-core", "--dry-run"])
        self.assertEqual(upload, ["cargo", "publish", "-p", "merman-core", "--no-verify"])

    def test_preflight_only_skips_crates_with_unpublished_internal_deps(self) -> None:
        commands: list[list[str]] = []
        published_checks: list[tuple[str, str]] = []
        original_argv = sys.argv
        original_cargo_metadata = publish_tool.cargo_metadata
        original_git_is_clean = publish_tool.git_is_clean
        original_require_tool = publish_tool.require_tool
        original_run_command = publish_tool.run_command
        original_check_crate_published = publish_tool.check_crate_published
        try:
            sys.argv = [
                "publish.py",
                "--crates",
                "merman",
                "--skip-xtask-verify",
                "--allow-dirty",
                "--yes",
                "--preflight-only",
                "--preflight-publish-dry-run",
                "--wait",
                "0",
            ]
            publish_tool.cargo_metadata = lambda _repo_root, **_kwargs: workspace_metadata(
                package("merman-core"),
                package("merman", dependency("merman-core")),
            )
            publish_tool.git_is_clean = lambda _repo_root: True
            publish_tool.require_tool = lambda _name: None

            def run_command(cmd, **_kwargs):
                commands.append(list(cmd))
                return publish_tool.subprocess.CompletedProcess(args=cmd, returncode=0)

            def check_crate_published(crate_name: str, version: str) -> bool:
                published_checks.append((crate_name, version))
                return False

            publish_tool.run_command = run_command
            publish_tool.check_crate_published = check_crate_published

            stdout = io.StringIO()
            stderr = io.StringIO()
            with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
                self.assertEqual(publish_tool.main(), 0)
        finally:
            sys.argv = original_argv
            publish_tool.cargo_metadata = original_cargo_metadata
            publish_tool.git_is_clean = original_git_is_clean
            publish_tool.require_tool = original_require_tool
            publish_tool.run_command = original_run_command
            publish_tool.check_crate_published = original_check_crate_published

        self.assertEqual(published_checks, [("merman-core", "1.0.0")])
        self.assertEqual(commands, [])
        self.assertIn(
            "Skipping preflight: internal workspace dependencies are not published yet: merman-core v1.0.0",
            stdout.getvalue(),
        )
        self.assertIn("Skipped 1 crate(s): merman", stdout.getvalue())


class PublishGraphTests(unittest.TestCase):
    def test_independent_packages_share_lexically_sorted_batches(self) -> None:
        metadata = workspace_metadata(
            package("beta"),
            package("alpha"),
            package("gamma", dependency("alpha")),
            package("delta", dependency("beta")),
        )

        self.assertEqual(
            publish_tool.crates_io_publish_batches(metadata),
            (("alpha", "beta"), ("delta", "gamma")),
        )

    def test_renamed_optional_target_and_build_dependencies_precede_dependents(self) -> None:
        metadata = workspace_metadata(
            package("core"),
            package(
                "app",
                dependency("core", rename="renamed_core", optional=True),
                dependency("core", kind="build", target="cfg(unix)"),
            ),
        )

        self.assertEqual(
            publish_tool.crates_io_publish_batches(metadata),
            (("core",), ("app",)),
        )

    def test_dev_dependencies_do_not_constrain_publish_order(self) -> None:
        metadata = workspace_metadata(
            package("alpha", dependency("zeta", kind="dev")),
            package("zeta"),
        )

        self.assertEqual(
            publish_tool.crates_io_publish_batches(metadata),
            (("alpha", "zeta"),),
        )

    def test_publishable_package_cannot_depend_on_private_workspace_member(self) -> None:
        metadata = workspace_metadata(
            package("internal", publish=[]),
            package("public", dependency("internal")),
        )

        with self.assertRaisesRegex(
            publish_tool.PublishGraphError,
            "public.*non-publishable workspace package internal",
        ):
            publish_tool.crates_io_publish_batches(metadata)

    def test_dependency_cycle_is_rejected(self) -> None:
        metadata = workspace_metadata(
            package("alpha", dependency("beta")),
            package("beta", dependency("alpha")),
        )

        with self.assertRaisesRegex(
            publish_tool.PublishGraphError,
            "dependency cycle.*alpha.*beta",
        ):
            publish_tool.crates_io_publish_batches(metadata)


def workspace_metadata(*packages: dict[str, object]) -> dict[str, object]:
    return {
        "workspace_members": [package["id"] for package in packages],
        "packages": list(packages),
    }


def package(
    name: str,
    *dependencies: dict[str, object],
    publish: object = None,
) -> dict[str, object]:
    manifest_path = ROOT / "crates" / name / "Cargo.toml"
    return {
        "id": f"path+file://{manifest_path.parent}#1.0.0",
        "name": name,
        "version": "1.0.0",
        "publish": publish,
        "manifest_path": str(manifest_path),
        "dependencies": list(dependencies),
    }


def dependency(
    name: str,
    *,
    kind: str | None = None,
    rename: str | None = None,
    optional: bool = False,
    target: str | None = None,
) -> dict[str, object]:
    return {
        "name": name,
        "kind": kind,
        "rename": rename,
        "optional": optional,
        "target": target,
        "path": str(ROOT / "crates" / name),
    }


if __name__ == "__main__":
    unittest.main()
