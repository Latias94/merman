#!/usr/bin/env python3
"""Focused public-contract tests for shared release process primitives."""

from __future__ import annotations

from pathlib import Path
import sys
import unittest
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))

import release_process


EXPECTED_PUBLIC_API = {
    "CommandRunner",
    "HostTargetChecker",
    "drain_bounded_stream",
    "run_checked",
    "target_matches_host",
    "terminate_process_tree",
}


class PublicContractTests(unittest.TestCase):
    def test_public_api_is_explicit_and_bounded(self) -> None:
        self.assertEqual(set(release_process.__all__), EXPECTED_PUBLIC_API)
        self.assertEqual(len(release_process.__all__), len(EXPECTED_PUBLIC_API))

    def test_linux_match_is_gnu_specific(self) -> None:
        with (
            mock.patch.object(release_process.platform, "system", return_value="Linux"),
            mock.patch.object(release_process.platform, "machine", return_value="x86_64"),
            mock.patch.object(
                release_process.platform,
                "libc_ver",
                return_value=("glibc", "2.39"),
            ),
        ):
            self.assertTrue(
                release_process.target_matches_host("x86_64-unknown-linux-gnu")
            )
            self.assertFalse(
                release_process.target_matches_host("x86_64-unknown-linux-musl")
            )

    def test_linux_match_rejects_a_different_host_libc(self) -> None:
        with (
            mock.patch.object(release_process.platform, "system", return_value="Linux"),
            mock.patch.object(release_process.platform, "machine", return_value="x86_64"),
            mock.patch.object(
                release_process.platform,
                "libc_ver",
                return_value=("musl", "1.2.5"),
            ),
        ):
            self.assertFalse(
                release_process.target_matches_host("x86_64-unknown-linux-gnu")
            )

    def test_windows_match_is_msvc_specific(self) -> None:
        with (
            mock.patch.object(release_process.platform, "system", return_value="Windows"),
            mock.patch.object(release_process.platform, "machine", return_value="AMD64"),
        ):
            self.assertTrue(
                release_process.target_matches_host("x86_64-pc-windows-msvc")
            )
            self.assertFalse(
                release_process.target_matches_host("x86_64-pc-windows-gnu")
            )

    def test_only_admitted_architectures_match(self) -> None:
        with (
            mock.patch.object(release_process.platform, "system", return_value="Linux"),
            mock.patch.object(release_process.platform, "machine", return_value="aarch64"),
            mock.patch.object(
                release_process.platform,
                "libc_ver",
                return_value=("glibc", "2.39"),
            ),
        ):
            self.assertFalse(
                release_process.target_matches_host("aarch64-unknown-linux-gnu")
            )


if __name__ == "__main__":
    unittest.main()
