#!/usr/bin/env python3
"""Unit tests for platform binding verification helpers."""

from __future__ import annotations

import importlib.util
import io
import tempfile
import unittest
import zipfile
from pathlib import Path


MODULE_PATH = Path(__file__).with_name("verify-platform-bindings.py")
SPEC = importlib.util.spec_from_file_location("verify_platform_bindings", MODULE_PATH)
assert SPEC is not None
verify_platform_bindings = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(verify_platform_bindings)

EXPECTED_ANDROID_WRAPPER_CLASSES = [
    "io/merman/MermanEngine.class",
    "io/merman/MermanReusableEngine.class",
    "io/merman/MermanException.class",
    "io/merman/MermanTextMeasureRequest.class",
    "io/merman/MermanTextMeasureResult.class",
    "io/merman/MermanTextMeasurer.class",
]


class AndroidAarVerificationTests(unittest.TestCase):
    def test_android_wrapper_class_manifest_matches_public_kotlin_types(self) -> None:
        self.assertEqual(
            verify_platform_bindings.ANDROID_WRAPPER_CLASSES,
            EXPECTED_ANDROID_WRAPPER_CLASSES,
        )

    def test_android_wrapper_class_manifest_covers_kotlin_source_files(self) -> None:
        kotlin_root = (
            MODULE_PATH.parents[1]
            / "platforms"
            / "android"
            / "src"
            / "main"
            / "kotlin"
            / "io"
            / "merman"
        )
        source_classes = sorted(
            f"io/merman/{source_path.stem}.class"
            for source_path in kotlin_root.glob("*.kt")
        )

        self.assertEqual(
            sorted(verify_platform_bindings.ANDROID_WRAPPER_CLASSES),
            source_classes,
        )

    def test_android_aar_contains_all_public_kotlin_wrapper_classes(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            aar_path = Path(temp_dir) / "merman-android-release.aar"
            write_aar(aar_path, EXPECTED_ANDROID_WRAPPER_CLASSES)

            verify_platform_bindings.assert_android_aar_contains_kotlin_wrappers(aar_path)

    def test_android_aar_reports_missing_public_wrapper_classes(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            aar_path = Path(temp_dir) / "merman-android-release.aar"
            classes = [
                class_name
                for class_name in EXPECTED_ANDROID_WRAPPER_CLASSES
                if class_name != "io/merman/MermanTextMeasureRequest.class"
            ]
            write_aar(aar_path, classes)

            with self.assertRaisesRegex(
                RuntimeError,
                "MermanTextMeasureRequest.class",
            ):
                verify_platform_bindings.assert_android_aar_contains_kotlin_wrappers(aar_path)

    def test_android_aar_reports_missing_text_measure_result_wrapper(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            aar_path = Path(temp_dir) / "merman-android-release.aar"
            classes = [
                class_name
                for class_name in EXPECTED_ANDROID_WRAPPER_CLASSES
                if class_name != "io/merman/MermanTextMeasureResult.class"
            ]
            write_aar(aar_path, classes)

            with self.assertRaisesRegex(
                RuntimeError,
                "MermanTextMeasureResult.class",
            ):
                verify_platform_bindings.assert_android_aar_contains_kotlin_wrappers(aar_path)


class AndroidInstrumentationReportTests(unittest.TestCase):
    def test_android_instrumentation_report_accepts_smoke_test_result(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            results_root = Path(temp_dir)
            report = results_root / "connected" / "TEST-smoke.xml"
            report.parent.mkdir(parents=True)
            report.write_text(
                """
                <testsuite name="io.merman.MermanInstrumentedSmokeTest">
                  <testcase name="runsPublicSmokeIncludingThrowingTextMeasurerFallback" />
                </testsuite>
                """,
                encoding="utf-8",
            )

            verify_platform_bindings.assert_android_instrumentation_smoke_report(results_root)

    def test_android_instrumentation_report_requires_smoke_test_result(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            results_root = Path(temp_dir)
            report = results_root / "connected" / "TEST-other.xml"
            report.parent.mkdir(parents=True)
            report.write_text("<testsuite name=\"OtherTest\" />", encoding="utf-8")

            with self.assertRaisesRegex(RuntimeError, "MermanInstrumentedSmokeTest"):
                verify_platform_bindings.assert_android_instrumentation_smoke_report(results_root)


def write_aar(aar_path: Path, class_names: list[str]) -> None:
    classes_jar = io.BytesIO()
    with zipfile.ZipFile(classes_jar, "w") as jar:
        for class_name in class_names:
            jar.writestr(class_name, b"")

    with zipfile.ZipFile(aar_path, "w") as aar:
        aar.writestr("classes.jar", classes_jar.getvalue())


if __name__ == "__main__":
    unittest.main()
