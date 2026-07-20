#!/usr/bin/env python3
"""Unit tests for platform binding verification helpers."""

from __future__ import annotations

import importlib.util
import hashlib
import io
import json
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
    "io/merman/MermanTextMeasurementOperation.class",
    "io/merman/MermanTextMeasurementResultKind.class",
    "io/merman/MermanTextMeasurer.class",
]
EXPECTED_ANDROID_NATIVE_LIBRARIES = [
    "jni/arm64-v8a/libmerman_ffi.so",
    "jni/x86_64/libmerman_ffi.so",
]


class AndroidAarVerificationTests(unittest.TestCase):
    def test_android_compile_sources_follow_the_complete_main_source_set(self) -> None:
        sources = verify_platform_bindings.android_kotlin_compile_sources()
        main_sources = sources[:-1]

        self.assertEqual(
            main_sources,
            sorted(
                (
                    MODULE_PATH.parents[1]
                    / "platforms"
                    / "android"
                    / "src"
                    / "main"
                    / "kotlin"
                    / "io"
                    / "merman"
                ).glob("*.kt")
            ),
        )
        self.assertEqual(sources[-1].name, "MermanSmoke.kt")

    def test_android_wrapper_class_manifest_matches_public_kotlin_types(self) -> None:
        self.assertEqual(
            verify_platform_bindings.ANDROID_WRAPPER_CLASSES,
            EXPECTED_ANDROID_WRAPPER_CLASSES,
        )

    def test_android_native_library_manifest_covers_published_abis(self) -> None:
        self.assertEqual(
            verify_platform_bindings.ANDROID_NATIVE_LIBRARIES,
            EXPECTED_ANDROID_NATIVE_LIBRARIES,
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

    def test_android_aar_contains_complete_release_contract(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            aar_path = Path(temp_dir) / "merman-android-release.aar"
            write_aar(aar_path, EXPECTED_ANDROID_WRAPPER_CLASSES)

            verify_platform_bindings.assert_android_aar_contract(aar_path)

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
                verify_platform_bindings.assert_android_aar_contract(aar_path)

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
                verify_platform_bindings.assert_android_aar_contract(aar_path)

    def test_android_aar_reports_missing_projected_resource(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            aar_path = Path(temp_dir) / "merman-android-release.aar"
            resources = [
                name
                for name in verify_platform_bindings.android_expected_resource_entries()
                if name != "META-INF/LICENSE"
            ]
            write_aar(
                aar_path,
                EXPECTED_ANDROID_WRAPPER_CLASSES,
                resource_names=resources,
            )

            with self.assertRaisesRegex(RuntimeError, "META-INF/LICENSE"):
                verify_platform_bindings.assert_android_aar_contract(aar_path)

    def test_android_aar_reports_missing_native_library(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            aar_path = Path(temp_dir) / "merman-android-release.aar"
            native_libraries = [
                name
                for name in EXPECTED_ANDROID_NATIVE_LIBRARIES
                if name != "jni/x86_64/libmerman_ffi.so"
            ]
            write_aar(
                aar_path,
                EXPECTED_ANDROID_WRAPPER_CLASSES,
                native_libraries=native_libraries,
            )

            with self.assertRaisesRegex(RuntimeError, "jni/x86_64/libmerman_ffi.so"):
                verify_platform_bindings.assert_android_aar_contract(aar_path)


class AndroidMavenPublicationTests(unittest.TestCase):
    def test_android_maven_publication_contains_complete_release_contract(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            module_root = Path(temp_dir) / "merman-android"
            version_dir = write_android_maven_publication(module_root)

            self.assertEqual(
                verify_platform_bindings.assert_android_maven_publication(module_root),
                version_dir,
            )

    def test_android_maven_publication_rejects_missing_pom_metadata(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            module_root = Path(temp_dir) / "merman-android"
            version_dir = write_android_maven_publication(module_root)
            pom_path = next(version_dir.glob("*.pom"))
            pom_path.write_text(
                pom_path.read_text(encoding="utf-8").replace(
                    "<scm>", "<removed-scm>", 1
                ).replace("</scm>", "</removed-scm>", 1),
                encoding="utf-8",
            )
            write_sha256(pom_path)

            with self.assertRaisesRegex(RuntimeError, "missing project/scm"):
                verify_platform_bindings.assert_android_maven_publication(module_root)

    def test_android_maven_publication_rejects_invalid_checksum(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            module_root = Path(temp_dir) / "merman-android"
            version_dir = write_android_maven_publication(module_root)
            aar_path = next(version_dir.glob("*.aar"))
            aar_path.with_name(f"{aar_path.name}.sha256").write_text(
                "0" * 64,
                encoding="ascii",
            )

            with self.assertRaisesRegex(RuntimeError, "SHA-256 mismatch"):
                verify_platform_bindings.assert_android_maven_publication(module_root)

    def test_android_maven_publication_requires_javadoc_variant(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            module_root = Path(temp_dir) / "merman-android"
            version_dir = write_android_maven_publication(module_root)
            module_path = next(version_dir.glob("*.module"))
            module = json.loads(module_path.read_text(encoding="utf-8"))
            module["variants"] = [
                variant
                for variant in module["variants"]
                if variant["attributes"].get("org.gradle.docstype") != "javadoc"
            ]
            module_path.write_text(json.dumps(module), encoding="utf-8")
            write_sha256(module_path)

            with self.assertRaisesRegex(RuntimeError, "documentation variants: javadoc"):
                verify_platform_bindings.assert_android_maven_publication(module_root)


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


def write_aar(
    aar_path: Path,
    class_names: list[str],
    *,
    resource_names: list[str] | None = None,
    native_libraries: list[str] | None = None,
) -> None:
    if resource_names is None:
        resource_names = verify_platform_bindings.android_expected_resource_entries()
    if native_libraries is None:
        native_libraries = EXPECTED_ANDROID_NATIVE_LIBRARIES

    classes_jar = io.BytesIO()
    with zipfile.ZipFile(classes_jar, "w") as jar:
        for class_name in class_names:
            jar.writestr(class_name, b"")
        for resource_name in resource_names:
            jar.writestr(resource_name, b"")

    with zipfile.ZipFile(aar_path, "w") as aar:
        aar.writestr("classes.jar", classes_jar.getvalue())
        for native_library in native_libraries:
            aar.writestr(native_library, b"")


def write_android_maven_publication(
    module_root: Path,
    version: str = "0.8.0-alpha.3",
) -> Path:
    version_dir = module_root / version
    version_dir.mkdir(parents=True)
    base_name = f"merman-android-{version}"
    pom_path = version_dir / f"{base_name}.pom"
    aar_path = version_dir / f"{base_name}.aar"
    source_jar = version_dir / f"{base_name}-sources.jar"
    javadoc_jar = version_dir / f"{base_name}-javadoc.jar"
    module_path = version_dir / f"{base_name}.module"

    pom_path.write_text(
        f"""<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0">
  <modelVersion>4.0.0</modelVersion>
  <groupId>io.merman</groupId>
  <artifactId>merman-android</artifactId>
  <version>{version}</version>
  <packaging>aar</packaging>
  <name>merman-android</name>
  <description>Android JNI bindings for merman headless Mermaid rendering.</description>
  <url>https://github.com/Latias94/merman</url>
  <licenses>
    <license><name>MIT License</name><url>https://opensource.org/license/mit</url><distribution>repo</distribution></license>
    <license><name>Apache License, Version 2.0</name><url>https://www.apache.org/licenses/LICENSE-2.0</url><distribution>repo</distribution></license>
  </licenses>
  <developers>
    <developer><id>frankorz</id><name>Mingzhen Zhuang</name><email>superfrankie621@gmail.com</email></developer>
  </developers>
  <scm>
    <connection>scm:git:https://github.com/Latias94/merman.git</connection>
    <developerConnection>scm:git:ssh://git@github.com/Latias94/merman.git</developerConnection>
    <url>https://github.com/Latias94/merman</url>
  </scm>
  <dependencies>
    <dependency><groupId>org.jetbrains.kotlin</groupId><artifactId>kotlin-stdlib</artifactId><version>2.2.10</version><scope>compile</scope></dependency>
  </dependencies>
</project>
""",
        encoding="utf-8",
    )
    write_aar(aar_path, EXPECTED_ANDROID_WRAPPER_CLASSES)

    kotlin_root = MODULE_PATH.parents[1] / "platforms" / "android" / "src" / "main" / "kotlin"
    with zipfile.ZipFile(source_jar, "w") as archive:
        for source_path in sorted(kotlin_root.rglob("*.kt")):
            archive.writestr(source_path.relative_to(kotlin_root).as_posix(), b"")

    with zipfile.ZipFile(javadoc_jar, "w") as archive:
        for entry in [
            "index.html",
            "merman-android/package-list",
            "merman-android/io.merman/index.html",
            "merman-android/io.merman/-merman-engine/index.html",
            "merman-android/io.merman/-merman-reusable-engine/index.html",
        ]:
            archive.writestr(entry, b"")

    def variant_file(path: Path) -> dict[str, object]:
        return {
            "name": path.name,
            "url": path.name,
            "size": path.stat().st_size,
            "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
        }

    module = {
        "formatVersion": "1.1",
        "component": {
            "group": "io.merman",
            "module": "merman-android",
            "version": version,
        },
        "variants": [
            {
                "name": "releaseApi",
                "attributes": {"org.gradle.libraryelements": "aar"},
                "files": [variant_file(aar_path)],
            },
            {
                "name": "releaseSources",
                "attributes": {"org.gradle.docstype": "sources"},
                "files": [variant_file(source_jar)],
            },
            {
                "name": "releaseJavadoc",
                "attributes": {"org.gradle.docstype": "javadoc"},
                "files": [variant_file(javadoc_jar)],
            },
        ],
    }
    module_path.write_text(json.dumps(module), encoding="utf-8")
    for artifact in [pom_path, module_path, aar_path, source_jar, javadoc_jar]:
        write_sha256(artifact)
    return version_dir


def write_sha256(path: Path) -> None:
    path.with_name(f"{path.name}.sha256").write_text(
        hashlib.sha256(path.read_bytes()).hexdigest(),
        encoding="ascii",
    )


if __name__ == "__main__":
    unittest.main()
