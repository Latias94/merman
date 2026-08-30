#!/usr/bin/env python3
"""Tests for release changelog projection validation."""

from __future__ import annotations

from pathlib import Path
import tempfile
import unittest

from scripts import verify_release_changelog as verify


class ReleaseChangelogTests(unittest.TestCase):
    def write_projection(self, root: Path, version: str = "0.8.0-alpha.6") -> None:
        for relative in verify.CHANGELOG_PATHS:
            path = root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            shown = "0.8.0a6" if relative.name == "CHANGELOG.md" and "python" in str(relative) else version
            path.write_text(f"# Changelog\n\n## [{shown}] - Unreleased\n", encoding="utf-8")

    def test_current_repository_projections_match_alpha6(self) -> None:
        verify.verify_repository(Path(__file__).resolve().parents[1], "0.8.0-alpha.6")

    def test_python_uses_pep440_projection(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.write_projection(root)
            verify.verify_repository(root, "0.8.0-alpha.6")

    def test_unversioned_first_heading_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.write_projection(root)
            path = root / "CHANGELOG.md"
            path.write_text(
                "# Changelog\n\n## [Unreleased]\n\n## [0.8.0-alpha.6] - Unreleased\n",
                encoding="utf-8",
            )
            with self.assertRaisesRegex(
                verify.ReleaseChangelogError,
                "first release heading has no date/status",
            ):
                verify.verify_repository(root, "0.8.0-alpha.6")

    def test_invalid_status_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.write_projection(root)
            path = root / "CHANGELOG.md"
            path.write_text("# Changelog\n\n## [0.8.0-alpha.6] - pending\n", encoding="utf-8")
            with self.assertRaisesRegex(verify.ReleaseChangelogError, "invalid release date/status"):
                verify.verify_repository(root, "0.8.0-alpha.6")

    def test_impossible_calendar_date_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.write_projection(root)
            path = root / "CHANGELOG.md"
            path.write_text(
                "# Changelog\n\n## [0.8.0-alpha.6] - 2026-02-30\n",
                encoding="utf-8",
            )
            with self.assertRaisesRegex(
                verify.ReleaseChangelogError,
                "invalid release date/status",
            ):
                verify.verify_repository(root, "0.8.0-alpha.6")


if __name__ == "__main__":
    unittest.main()
