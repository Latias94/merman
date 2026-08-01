#!/usr/bin/env python3
"""Focused public-contract tests for shared release archive primitives."""

from __future__ import annotations

from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))

import release_archive


EXPECTED_PUBLIC_API = {
    "ArchiveMember",
    "ArchiveVerificationError",
    "DEFAULT_LIMITS",
    "ExtractionLimits",
    "VerificationReport",
    "archive_member_path",
    "binary_name_for",
    "format_set_mismatch",
    "git_tracked_legal_files",
    "persist_verified_archive",
    "read_checksum",
    "regular_files_equal",
    "release_archive_name_for",
    "repository_tree_files",
    "require_regular_input",
    "require_repository_root",
    "sha256_file",
    "verified_archive_contents",
}


class PublicContractTests(unittest.TestCase):
    def test_public_api_is_explicit_and_bounded(self) -> None:
        self.assertEqual(set(release_archive.__all__), EXPECTED_PUBLIC_API)
        self.assertEqual(len(release_archive.__all__), len(EXPECTED_PUBLIC_API))

    def test_names_are_package_neutral_and_target_specific(self) -> None:
        self.assertEqual(
            release_archive.release_archive_name_for(
                "merman-cli", "x86_64-unknown-linux-gnu"
            ),
            "merman-cli-x86_64-unknown-linux-gnu.tar.xz",
        )
        self.assertEqual(
            release_archive.release_archive_name_for(
                "merman-lsp", "x86_64-pc-windows-msvc"
            ),
            "merman-lsp-x86_64-pc-windows-msvc.zip",
        )
        self.assertEqual(
            release_archive.binary_name_for(
                "merman-lsp", "x86_64-pc-windows-msvc"
            ),
            "merman-lsp.exe",
        )

    def test_invalid_package_name_is_rejected(self) -> None:
        with self.assertRaises(release_archive.ArchiveVerificationError):
            release_archive.release_archive_name_for(
                "Merman LSP", "x86_64-unknown-linux-gnu"
            )

    def test_set_mismatch_is_shared_by_archive_verifiers(self) -> None:
        self.assertEqual(
            release_archive.format_set_mismatch(
                "payload",
                {"binary", "license"},
                {"binary", "unexpected"},
            ),
            "archive payload set differs from repository: "
            "missing license; unexpected unexpected",
        )

    def test_regular_file_comparison_is_exact(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            left = root / "left"
            right = root / "right"
            left.write_bytes(b"same" * 300_000)
            right.write_bytes(left.read_bytes())
            self.assertTrue(release_archive.regular_files_equal(left, right))

            right.write_bytes(b"same" * 299_999 + b"diff")
            self.assertFalse(release_archive.regular_files_equal(left, right))

    def test_checksum_parser_accepts_cargo_dist_trailing_separator(self) -> None:
        digest = "a" * 64
        archive_name = "merman-cli-x86_64-unknown-linux-gnu.tar.xz"
        with tempfile.TemporaryDirectory() as temp_dir:
            checksum = Path(temp_dir) / f"{archive_name}.sha256"
            checksum.write_text(f"{digest} *{archive_name}\n\n", encoding="ascii")
            self.assertEqual(release_archive.read_checksum(checksum, archive_name), digest)

            for suffix in ("\n\n\n", "\nsecond-line\n"):
                with self.subTest(suffix=suffix):
                    checksum.write_text(
                        f"{digest} *{archive_name}{suffix}",
                        encoding="ascii",
                    )
                    with self.assertRaises(release_archive.ArchiveVerificationError):
                        release_archive.read_checksum(checksum, archive_name)


if __name__ == "__main__":
    unittest.main()
