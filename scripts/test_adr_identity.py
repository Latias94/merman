#!/usr/bin/env python3
"""Tests for the ADR filename/title identity contract."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

try:
    from scripts.adr_identity import identity_failures
except ModuleNotFoundError:
    from adr_identity import identity_failures


REPOSITORY_ROOT = Path(__file__).resolve().parents[1]


class AdrIdentityTests(unittest.TestCase):
    def test_accepts_existing_title_spellings(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            adr_root = Path(temp_dir)
            (adr_root / "0001-hyphen.md").write_text(
                "# ADR-0001: Hyphen\n", encoding="utf-8"
            )
            (adr_root / "0002-space.md").write_text(
                "# ADR 0002: Space\n", encoding="utf-8"
            )
            (adr_root / "0003-bare.md").write_text(
                "# 0003: Bare\n", encoding="utf-8"
            )

            self.assertEqual(identity_failures(adr_root), [])

    def test_rejects_duplicate_filename_ids(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            adr_root = Path(temp_dir)
            (adr_root / "0041-first.md").write_text(
                "# ADR 0041: First\n", encoding="utf-8"
            )
            (adr_root / "0041-second.md").write_text(
                "# ADR-0041: Second\n", encoding="utf-8"
            )

            self.assertEqual(
                identity_failures(adr_root),
                [
                    "ADR id 0041 is used by multiple files: "
                    "0041-first.md, 0041-second.md"
                ],
            )

    def test_ignores_ordinary_prose_after_the_title(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            adr_root = Path(temp_dir)
            (adr_root / "0080-example.md").write_text(
                "# ADR 0080: Example\n\nHistorical prose mentions ADR 0041.\n",
                encoding="utf-8",
            )

            self.assertEqual(identity_failures(adr_root), [])

    def test_rejects_filename_title_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            adr_root = Path(temp_dir)
            (adr_root / "0080-example.md").write_text(
                "# ADR 0081: Example\n", encoding="utf-8"
            )

            self.assertEqual(
                identity_failures(adr_root),
                [
                    "0080-example.md: title ADR id 0081 does not match filename id 0080"
                ],
            )

    def test_rejects_adr_prefix_without_a_separator(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            adr_root = Path(temp_dir)
            (adr_root / "0080-example.md").write_text(
                "# ADR0080: Example\n", encoding="utf-8"
            )

            self.assertEqual(
                identity_failures(adr_root),
                ["0080-example.md: first line must contain a structured ADR id"],
            )

    def test_repository_adr_identities_are_unique_and_match_titles(self) -> None:
        self.assertEqual(identity_failures(REPOSITORY_ROOT / "docs" / "adr"), [])


if __name__ == "__main__":
    unittest.main()
