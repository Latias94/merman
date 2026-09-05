#!/usr/bin/env python3
"""Unit tests for prerelease fresh-resolution compatibility checks."""

from __future__ import annotations

from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest import mock

from scripts import verify_prerelease_compatibility as verify


class PrereleaseCompatibilityTests(unittest.TestCase):
    def test_render_manifest_uses_an_exact_consumer_requirement_and_patches(self) -> None:
        packages = (
            verify.CandidatePackage("merman", "0.8.1-alpha.1", Path("C:/candidate/merman")),
            verify.CandidatePackage(
                "merman-core", "0.8.1-alpha.1", Path("C:/candidate/merman-core")
            ),
        )

        manifest = verify.render_manifest("0.9.0-alpha.1", packages)

        self.assertIn(
            'merman = { version = "=0.9.0-alpha.1", default-features = false, '
            'features = ["ascii"] }',
            manifest,
        )
        self.assertIn(
            f'"merman-core" = {{ path = {verify.toml_string(str(packages[1].path))} }}',
            manifest,
        )
        self.assertIn('[patch.crates-io]', manifest)

    def test_verify_runs_candidate_and_previous_facade_lanes_without_lockfiles(self) -> None:
        calls: list[tuple[Path, str]] = []

        def fake_check(command, *, cwd, env):
            calls.append((cwd, (cwd / "Cargo.toml").read_text(encoding="utf-8")))
            self.assertFalse((cwd / "Cargo.lock").exists())
            return subprocess.CompletedProcess(command, 0, "", "")

        packages = (
            verify.CandidatePackage("merman", "0.9.0-alpha.1", Path("C:/candidate/merman")),
            verify.CandidatePackage(
                "merman-core", "0.9.0-alpha.1", Path("C:/candidate/merman-core")
            ),
        )
        with mock.patch.object(verify, "candidate_packages", return_value=packages):
            with tempfile.TemporaryDirectory() as temp_dir:
                verify.verify(
                    Path(temp_dir),
                    "0.8.1-alpha.1",
                    "0.8.0-alpha.6",
                    target_directory=Path(temp_dir) / "target",
                    run_check=fake_check,
                )

        self.assertEqual(len(calls), 2)
        candidate_manifest = calls[0][1]
        previous_manifest = calls[1][1]
        merman_path = verify.toml_string(str(packages[0].path))
        core_path = verify.toml_string(str(packages[1].path))
        self.assertIn(f'"merman" = {{ path = {merman_path} }}', candidate_manifest)
        self.assertNotIn(f'"merman" = {{ path = {merman_path} }}', previous_manifest)
        self.assertIn(f'"merman-core" = {{ path = {core_path} }}', previous_manifest)

    def test_stable_releases_skip_without_loading_candidate_packages(self) -> None:
        with mock.patch.object(verify, "candidate_packages") as packages:
            verify.verify(Path("."), "0.9.0")
        packages.assert_not_called()

    def test_new_prerelease_line_does_not_require_old_line_compatibility(self) -> None:
        first = verify.parse_release_version("0.8.0-alpha.6")
        second = verify.parse_release_version("0.9.0-alpha.1")
        self.assertFalse(verify.same_compatibility_line(first, second))

        packages = (
            verify.CandidatePackage("merman", "0.9.0-alpha.1", Path("C:/candidate/merman")),
        )
        with mock.patch.object(verify, "candidate_packages", return_value=packages):
            with mock.patch.object(verify, "_run_lane") as run_lane:
                verify.verify(
                    Path("."),
                    "0.9.0-alpha.1",
                    "0.8.0-alpha.6",
                )
        run_lane.assert_called_once()

    def test_prerelease_requires_a_previous_lane_unless_explicitly_first(self) -> None:
        with self.assertRaisesRegex(verify.PrereleaseCompatibilityError, "previous-version"):
            verify.verify(Path("."), "0.9.0-alpha.1")

        with mock.patch.object(verify, "candidate_packages", return_value=()):
            with mock.patch.object(verify, "_run_lane") as run_lane:
                verify.verify(
                    Path("."),
                    "0.9.0-alpha.1",
                    allow_missing_previous=True,
                )
        run_lane.assert_called_once()

    def test_candidate_packages_rejects_a_mixed_workspace_version(self) -> None:
        metadata = {
            "workspace_members": ["candidate"],
            "packages": [
                {
                    "id": "candidate",
                    "name": "merman",
                    "version": "0.9.0-alpha.2",
                    "publish": None,
                    "manifest_path": "C:/repo/crates/merman/Cargo.toml",
                }
            ],
        }
        with mock.patch.object(verify, "cargo_metadata", return_value=metadata):
            with self.assertRaisesRegex(verify.PrereleaseCompatibilityError, "expected"):
                verify.candidate_packages(
                    Path("C:/repo"),
                    verify.parse_release_version("0.9.0-alpha.1"),
                )


if __name__ == "__main__":
    unittest.main()
