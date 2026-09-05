#!/usr/bin/env python3
"""Tests for fresh registry-dependent compilation checks."""

from __future__ import annotations

from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest import mock

from scripts import release_registry_dependents as release


class RegistryDependentTests(unittest.TestCase):
    def test_parse_spec_requires_exact_package_and_version(self) -> None:
        self.assertEqual(
            release.parse_spec("roughr-merman=0.12.3", option="--dependency"),
            release.PackageSpec("roughr-merman", "0.12.3"),
        )
        for value in ("roughr-merman", "roughr-merman=0.12", "bad name=1.0.0"):
            with self.subTest(value=value), self.assertRaises(release.RegistryDependentError):
                release.parse_spec(value, option="--dependent")

    def test_render_manifest_patches_only_candidate_dependency(self) -> None:
        manifest = release.render_manifest(
            release.PackageSpec("roughr-merman", "0.12.3"),
            release.PackageSpec("merman-render", "0.8.0-alpha.5"),
            candidate_path=Path("/tmp/candidate"),
        )
        self.assertIn('"merman-render" = { version = "=0.8.0-alpha.5" }', manifest)
        self.assertNotIn('"roughr-merman" = { version = "=0.12.3" }', manifest)
        self.assertIn('[patch.crates-io]', manifest)
        candidate_path = release.toml_string(str(Path("/tmp/candidate")))
        self.assertIn(
            f'"roughr-merman" = {{ path = {candidate_path} }}',
            manifest,
        )

    def test_verify_runs_both_lanes_without_reusing_a_lockfile(self) -> None:
        calls: list[tuple[list[str], Path, str, str]] = []

        def fake_check(command, *, cwd, env):
            calls.append(
                (
                    command,
                    cwd,
                    (cwd / "Cargo.toml").read_text(),
                    env["CARGO_TARGET_DIR"],
                )
            )
            return subprocess.CompletedProcess(command, 0, "", "")

        with tempfile.TemporaryDirectory() as temp_dir:
            candidate = Path(temp_dir) / "candidate"
            candidate.mkdir()
            (candidate / "Cargo.toml").write_text(
                '[package]\nname = "roughr-merman"\nversion = "0.12.3"\n',
                encoding="utf-8",
            )
            release.verify(
                release.PackageSpec("roughr-merman", "0.12.3"),
                (
                    release.PackageSpec("merman-render", "0.7.0"),
                    release.PackageSpec("merman-render", "0.8.0-alpha.5"),
                ),
                candidate_path=candidate,
                target_directory=Path(temp_dir) / "shared-target",
                run_check=fake_check,
            )

        self.assertEqual(len(calls), 4)
        self.assertEqual(sum("[patch.crates-io]" in text for _, _, text, _ in calls), 2)
        self.assertEqual(sum("[patch.crates-io]" not in text for _, _, text, _ in calls), 2)
        self.assertEqual(
            sum(
                '"merman-render" = { version = "=0.7.0" }' in text
                for _, _, text, _ in calls
            ),
            2,
        )
        self.assertEqual(
            sum(
                '"merman-render" = { version = "=0.8.0-alpha.5" }' in text
                for _, _, text, _ in calls
            ),
            2,
        )
        self.assertTrue(
            all("--manifest-path" in command for command, _, _, _ in calls)
        )
        self.assertEqual(len({target for _, _, _, target in calls}), 1)

    def test_verify_rejects_candidate_version_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            candidate = Path(temp_dir)
            (candidate / "Cargo.toml").write_text(
                '[package]\nname = "roughr-merman"\nversion = "0.12.4"\n',
                encoding="utf-8",
            )
            with self.assertRaisesRegex(release.RegistryDependentError, "does not match"):
                release.verify(
                    release.PackageSpec("roughr-merman", "0.12.3"),
                    (release.PackageSpec("merman-render", "0.7.0"),),
                    candidate_path=candidate,
                    run_check=lambda **_kwargs: subprocess.CompletedProcess([], 0),
                )

    def test_verify_reports_compile_failure_with_lane(self) -> None:
        def failed(command, *, cwd, env):
            return subprocess.CompletedProcess(command, 101, "", "dependency conflict")

        with self.assertRaisesRegex(
            release.RegistryDependentError,
            r"registry registry-dependent check failed for merman-render 0\.7\.0",
        ):
            release.verify(
                release.PackageSpec("roughr-merman", "0.12.3"),
                (release.PackageSpec("merman-render", "0.7.0"),),
                candidate_path=None,
                run_check=failed,
            )


if __name__ == "__main__":
    unittest.main()
