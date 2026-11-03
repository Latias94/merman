#!/usr/bin/env python3
"""Tests for the Flutter pub package layout and size gates."""

from __future__ import annotations

import argparse
from contextlib import redirect_stderr, redirect_stdout
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))

from flutter_native_assets import NativeAssetsError, TARGET_LIBRARIES, stage
from flutter_release_archive import _pubspec_scalar
from verify_flutter_pub_package import (
    FlutterPackageSizeError,
    compressed_archive_megabytes,
    main,
    positive_float,
    verify_thin_package,
)


class FlutterThinPackageTests(unittest.TestCase):
    def setUp(self) -> None:
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        self.package = root / "package"
        source = Path(__file__).resolve().parents[1] / "platforms/flutter"
        for relative in (
            "pubspec.yaml",
            "hook/build.dart",
            "lib/src/native_assets.dart",
            "lib/src/generated/package_version.dart",
        ):
            path = self.package / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes((source / relative).read_bytes())
        native = root / "native"
        for target, library in TARGET_LIBRARIES.items():
            path = native / target / library
            path.parent.mkdir(parents=True)
            path.write_bytes(b"tiny native fixture")
        version = _pubspec_scalar((self.package / "pubspec.yaml").read_bytes(), "version")
        stage(
            native, root / "assets", version=version, source_sha="1" * 40,
            source_tree="2" * 40, base_url=f"https://example.org/flutter-v{version}/",
        )
        self.manifest = self.package / "hook/native-assets.json"
        self.manifest.write_bytes((root / "assets/native-assets.json").read_bytes())

    def test_real_legacy_layout_fails_and_moved_helper_passes(self) -> None:
        helper = self.package / "lib/src/native_assets.dart"
        build = self.package / "hook/build.dart"
        helper_bytes, build_bytes = helper.read_bytes(), build.read_bytes()
        manifest_bytes = self.manifest.read_bytes()
        legacy = helper.rename(self.package / "hook/native_assets.dart")
        legacy.write_bytes(helper_bytes.replace(
            b"import 'generated/package_version.dart';",
            b"import '../lib/src/generated/package_version.dart';",
        ))
        build.write_bytes(build_bytes.replace(
            b"import '../lib/src/native_assets.dart';", b"import 'native_assets.dart';",
        ))
        errors = io.StringIO()
        with (
            mock.patch("verify_flutter_pub_package.subprocess.run") as run,
            redirect_stderr(errors),
        ):
            result = main(["--package-root", str(self.package), "--require-thin"])
        self.assertEqual(result, 1)
        self.assertIn("hook/native_assets.dart", errors.getvalue())
        run.assert_not_called()

        legacy.rename(helper)
        helper.write_bytes(helper_bytes)
        build.write_bytes(build_bytes)
        verify_thin_package(self.package)
        self.assertEqual(self.manifest.read_bytes(), manifest_bytes)

    def test_only_the_official_hook_entrypoints_are_allowed(self) -> None:
        (self.package / "hook/link.dart").write_text("void main() {}\n", encoding="utf-8")
        verify_thin_package(self.package)
        for relative in (
            "hook/helpers/native_assets.dart",
            "hook/nested/build.dart",
            "hook/nested/link.dart",
        ):
            with self.subTest(relative=relative):
                path = self.package / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text("void main() {}\n", encoding="utf-8")
                with self.assertRaisesRegex(
                    FlutterPackageSizeError, r"only allows hook/build\.dart and hook/link\.dart",
                ) as raised:
                    verify_thin_package(self.package)
                self.assertIn(relative, str(raised.exception))
                path.unlink()

    def test_thin_package_checks_actual_files(self) -> None:
        verify_thin_package(self.package)
        (self.package / "native").mkdir()
        (self.package / ".pubignore").write_text("/native/\n", encoding="utf-8")
        with self.assertRaisesRegex(FlutterPackageSizeError, "must not contain native"):
            verify_thin_package(self.package)
        (self.package / "native").rmdir()
        (self.package / "hidden.so").write_bytes(b"binary outside native")
        with self.assertRaisesRegex(FlutterPackageSizeError, "native archive/library"):
            verify_thin_package(self.package)

    def test_native_manifest_version_remains_pinned(self) -> None:
        manifest = json.loads(self.manifest.read_bytes())
        manifest["version"] = "0.0.0"
        self.manifest.write_text(json.dumps(manifest), encoding="utf-8")
        with self.assertRaisesRegex(NativeAssetsError, "version identity differs"):
            verify_thin_package(self.package)


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
