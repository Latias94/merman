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

    def test_workspace_packages_mark_publish_false_metadata_as_not_publishable(self) -> None:
        original_cargo_metadata = publish_tool.cargo_metadata
        try:
            publish_tool.cargo_metadata = lambda _repo_root: {
                "packages": [
                    {
                        "name": "xtask",
                        "version": "1.0.0",
                        "publish": [],
                        "manifest_path": str(ROOT / "crates" / "xtask" / "Cargo.toml"),
                        "dependencies": [],
                    },
                    {
                        "name": "merman-core",
                        "version": "1.0.0",
                        "publish": None,
                        "manifest_path": str(ROOT / "crates" / "merman-core" / "Cargo.toml"),
                        "dependencies": [],
                    },
                ],
            }

            packages = publish_tool.get_workspace_packages(ROOT)
        finally:
            publish_tool.cargo_metadata = original_cargo_metadata

        self.assertFalse(packages["xtask"].publish)
        self.assertTrue(packages["merman-core"].publish)

    def test_crates_io_package_list_rejects_internal_registry_packages(self) -> None:
        metadata = {
            "packages": [
                {"name": "default-publish", "publish": None},
                {"name": "explicit-crates-io", "publish": ["crates-io"]},
                {"name": "internal-only", "publish": ["internal"]},
                {"name": "publish-false", "publish": []},
            ]
        }

        self.assertEqual(
            publish_tool.crates_io_publishable_package_names(metadata),
            ["default-publish", "explicit-crates-io"],
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
            publish_tool.cargo_metadata = lambda _repo_root: {
                "packages": [
                    {
                        "name": "merman-core",
                        "version": "1.0.0",
                        "publish": None,
                        "manifest_path": str(ROOT / "crates" / "merman-core" / "Cargo.toml"),
                        "dependencies": [],
                    }
                ],
            }
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


if __name__ == "__main__":
    unittest.main()
