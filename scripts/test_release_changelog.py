#!/usr/bin/env python3
"""Tests for release changelog projection validation."""

from __future__ import annotations

from pathlib import Path
import tempfile
import tomllib
import unittest

from scripts import verify_release_changelog as verify


class ReleaseChangelogTests(unittest.TestCase):
    def write_projection(self, root: Path, version: str = "0.8.0-alpha.6") -> None:
        for relative in verify.CHANGELOG_PATHS:
            path = root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            shown = "0.8.0a6" if relative.name == "CHANGELOG.md" and "python" in str(relative) else version
            path.write_text(f"# Changelog\n\n## [{shown}] - Unreleased\n", encoding="utf-8")

    def test_current_repository_projections_match_workspace_version(self) -> None:
        root = Path(__file__).resolve().parents[1]
        with (root / "Cargo.toml").open("rb") as manifest:
            workspace = tomllib.load(manifest)
        verify.verify_repository(root, workspace["workspace"]["package"]["version"])

    def test_python_uses_pep440_projection(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.write_projection(root)
            verify.verify_repository(root, "0.8.0-alpha.6")

    def test_release_preflight_requires_dated_projections(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.write_projection(root)
            with self.assertRaisesRegex(
                verify.ReleaseChangelogError,
                "must be dated",
            ):
                verify.verify_repository(
                    root,
                    "0.8.0-alpha.6",
                    require_date=True,
                )

    def test_development_allows_pending_notes_before_the_latest_version(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.write_projection(root)
            path = root / "CHANGELOG.md"
            path.write_text(
                "# Changelog\n\n## [Unreleased]\n\n- Pending change.\n\n"
                "## [0.8.0-alpha.6] - 2026-09-02\n",
                encoding="utf-8",
            )
            verify.verify_repository(root, "0.8.0-alpha.6")

    def test_immutable_preflight_rejects_pending_notes_in_any_position(self) -> None:
        released = "## [0.8.0-alpha.6] - 2026-09-02\n"
        pending = "## [Unreleased]\n\n- API 8 has not shipped.\n"
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.write_projection(root)
            for relative in verify.CHANGELOG_PATHS:
                path = root / relative
                path.write_text(path.read_text().replace(" - Unreleased", " - 2026-09-02"))
            verify.verify_repository(root, "0.8.0-alpha.6", require_date=True)
            for text in [pending + released, released + pending]:
                (root / "CHANGELOG.md").write_text(text, encoding="utf-8")
                with self.subTest(text=text), self.assertRaisesRegex(
                    verify.ReleaseChangelogError, "Unreleased section",
                ):
                    verify.verify_repository(root, "0.8.0-alpha.6", require_date=True)

    def test_pending_notes_do_not_hide_a_wrong_latest_release(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.write_projection(root)
            path = root / "CHANGELOG.md"
            path.write_text(
                "## [Unreleased]\n\n## [0.8.0-alpha.5] - 2026-08-01\n",
                encoding="utf-8",
            )
            with self.assertRaisesRegex(verify.ReleaseChangelogError, "expected"):
                verify.verify_repository(root, "0.8.0-alpha.6")

    def test_pending_notes_do_not_replace_a_version_or_date(self) -> None:
        for text, message in [
            ("## [Unreleased]\n", "no release heading"),
            ("## [Unreleased]\n\n## [0.8.0-alpha.6]\n", "no date/status"),
        ]:
            with self.subTest(text=text), self.assertRaisesRegex(
                verify.ReleaseChangelogError, message,
            ):
                verify.first_release_heading(text, Path("CHANGELOG.md"))

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
