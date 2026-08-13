#!/usr/bin/env python3
"""Tests for the Flutter pub package size gate."""

from __future__ import annotations

import argparse
from contextlib import redirect_stderr, redirect_stdout
import io
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))

from verify_flutter_pub_package import (
    FlutterPackageSizeError,
    compressed_archive_megabytes,
    main,
    positive_float,
)


class FlutterPubPackageSizeTests(unittest.TestCase):
    def test_reads_the_total_compressed_archive_size(self) -> None:
        self.assertEqual(
            compressed_archive_megabytes("Total compressed archive size: 94 MB.\n"),
            94.0,
        )

    def test_prefers_the_more_precise_large_package_hint(self) -> None:
        output = """
Total compressed archive size: 149 MB.
Your package is 149.5 MB.
"""
        self.assertEqual(compressed_archive_megabytes(output), 149.5)

    def test_normalizes_other_reported_units(self) -> None:
        self.assertEqual(
            compressed_archive_megabytes(
                "Total compressed archive size: 99000 KB.\n"
            ),
            99.0,
        )

    def test_rejects_output_without_a_size_report(self) -> None:
        with self.assertRaisesRegex(FlutterPackageSizeError, "did not report"):
            compressed_archive_megabytes("Package has 0 warnings.\n")

    def test_rejects_non_finite_budget(self) -> None:
        with self.assertRaises(argparse.ArgumentTypeError):
            positive_float("nan")

    def test_main_rejects_a_package_above_budget(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            package_root = Path(temp_dir)
            (package_root / "pubspec.yaml").write_text(
                "name: fixture\nversion: 1.0.0\n",
                encoding="utf-8",
            )
            completed = subprocess.CompletedProcess(
                args=["dart", "pub", "publish", "--dry-run"],
                returncode=0,
                stdout="Total compressed archive size: 100 MB.\n",
            )
            with (
                mock.patch(
                    "verify_flutter_pub_package.subprocess.run",
                    return_value=completed,
                ),
                redirect_stdout(io.StringIO()),
                redirect_stderr(io.StringIO()),
            ):
                result = main(
                    [
                        "--package-root",
                        str(package_root),
                        "--max-compressed-mb",
                        "99",
                    ]
                )

        self.assertEqual(result, 1)

    def test_main_preserves_dart_failure_status(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            package_root = Path(temp_dir)
            (package_root / "pubspec.yaml").write_text(
                "name: fixture\nversion: 1.0.0\n",
                encoding="utf-8",
            )
            completed = subprocess.CompletedProcess(
                args=["dart", "pub", "publish", "--dry-run"],
                returncode=7,
                stdout="validation failed\n",
            )
            with (
                mock.patch(
                    "verify_flutter_pub_package.subprocess.run",
                    return_value=completed,
                ),
                redirect_stdout(io.StringIO()),
            ):
                result = main(["--package-root", str(package_root)])

        self.assertEqual(result, 7)


if __name__ == "__main__":
    unittest.main()
