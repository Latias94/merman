#!/usr/bin/env python3
"""Unit tests for platform binding verification helpers."""

from __future__ import annotations

from dataclasses import replace
import importlib.util
import hashlib
import io
import json
import tempfile
import unittest
from unittest import mock
import zipfile
from pathlib import Path

MODULE_PATH = Path(__file__).with_name("verify-platform-bindings.py")
SPEC = importlib.util.spec_from_file_location("verify_platform_bindings", MODULE_PATH)
assert SPEC is not None
verify_platform_bindings = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(verify_platform_bindings)

FLUTTER_ANDROID_SMOKE_PATH = (
    MODULE_PATH.parents[1] / "platforms" / "flutter" / "tool" / "android-smoke.py"
)
FLUTTER_ANDROID_SMOKE_SPEC = importlib.util.spec_from_file_location(
    "flutter_android_smoke",
    FLUTTER_ANDROID_SMOKE_PATH,
)
assert FLUTTER_ANDROID_SMOKE_SPEC is not None
flutter_android_smoke = importlib.util.module_from_spec(FLUTTER_ANDROID_SMOKE_SPEC)
assert FLUTTER_ANDROID_SMOKE_SPEC.loader is not None
FLUTTER_ANDROID_SMOKE_SPEC.loader.exec_module(flutter_android_smoke)

EXPECTED_ANDROID_NATIVE_LIBRARIES = [
    "jni/arm64-v8a/libmerman_android_jni.so",
    "jni/x86_64/libmerman_android_jni.so",
]
ANDROID_CONSUMER_RULES = (
    MODULE_PATH.parents[1] / "platforms" / "android" / "consumer-rules.pro"
)


class NativeSdkRecipeTests(unittest.TestCase):
    def test_flutter_format_contract_excludes_generator_owned_sources(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            handwritten = (
                root / "lib" / "merman.dart",
                root / "example" / "smoke.dart",
                root / "tool" / "abi3_contract_test.dart",
            )
            generated = (
                root / "lib" / "src" / "generated" / "binding_contract.dart",
                root / "lib" / "src" / "generated" / "native_abi.dart",
            )
            for path in (*handwritten, *generated):
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text("void main() {}\n", encoding="utf-8")

            paths = verify_platform_bindings.flutter_format_paths(root)

        self.assertEqual(
            paths,
            [
                "lib/merman.dart",
                "example/smoke.dart",
                "tool/abi3_contract_test.dart",
            ],
        )
        self.assertTrue(all("/generated/" not in path for path in paths))

    def test_android_transport_checks_use_each_exact_descriptor_recipe(self) -> None:
        target = "aarch64-linux-android"
        for recipe in (
            verify_platform_bindings.ANDROID_NATIVE_RECIPE,
            verify_platform_bindings.FLUTTER_ANDROID_NATIVE_RECIPE,
        ):
            with self.subTest(profile=recipe.profile_id):
                args = verify_platform_bindings.cargo_android_clippy_args(
                    recipe,
                    target,
                )
                self.assertEqual(args[:3], ["cargo", "clippy", "--locked"])
                self.assertIn("--no-deps", args)
                self.assertEqual(args[args.index("--package") + 1], recipe.package)
                self.assertEqual(
                    args[args.index("--manifest-path") + 1],
                    str(MODULE_PATH.parents[1] / recipe.manifest),
                )
                self.assertEqual(
                    args[args.index("--features") + 1],
                    recipe.feature_argument,
                )
                self.assertEqual(args[args.index("--target") + 1], target)

    def test_android_transport_checks_reject_non_descriptor_target(self) -> None:
        with self.assertRaisesRegex(RuntimeError, "does not declare Android target"):
            verify_platform_bindings.cargo_android_clippy_args(
                verify_platform_bindings.ANDROID_NATIVE_RECIPE,
                "armv7-linux-androideabi",
            )

    def test_dart_ffi_smoke_consumes_the_exact_flutter_desktop_recipe(self) -> None:
        recipe = verify_platform_bindings.FLUTTER_DESKTOP_NATIVE_RECIPE
        target = "x86_64-unknown-linux-gnu"
        with tempfile.TemporaryDirectory() as temp_dir:
            library = Path(temp_dir) / "libmerman_ffi.so"
            library.write_bytes(b"native")
            cargo_stdout = json.dumps(
                {
                    "reason": "compiler-artifact",
                    "target": {
                        "name": recipe.target_name,
                        "crate_types": ["cdylib", "rlib"],
                    },
                    "filenames": [str(library)],
                }
            )
            with (
                mock.patch.object(
                    verify_platform_bindings,
                    "run_capture",
                    return_value=mock.Mock(stdout=cargo_stdout),
                ) as run_capture,
                mock.patch.object(verify_platform_bindings, "run") as run,
            ):
                verify_platform_bindings.run_dart_ffi_native_smoke(
                    "dart",
                    target=target,
                )

        build = run_capture.call_args
        self.assertEqual(
            build.args[0][:4],
            ["cargo", "build", "--profile", "native-distribution"],
        )
        self.assertIn("--locked", build.args[0])
        self.assertEqual(
            build.args[0][build.args[0].index("--features") + 1],
            recipe.feature_argument,
        )
        self.assertEqual(
            build.args[0][build.args[0].index("--target") + 1],
            target,
        )
        self.assertIn("--message-format=json-render-diagnostics", build.args[0])
        self.assertEqual(
            run.call_args_list[0].args[0],
            [
                "dart",
                "run",
                "example/smoke.dart",
                str(library),
            ],
        )
        self.assertEqual(len(run.call_args_list), 1)

    def test_native_library_path_comes_from_the_matching_cargo_artifact(self) -> None:
        recipe = verify_platform_bindings.FLUTTER_DESKTOP_NATIVE_RECIPE
        with tempfile.TemporaryDirectory() as temp_dir:
            library = Path(temp_dir) / "renamed-output.dylib"
            library.write_bytes(b"native")
            stdout = "\n".join(
                [
                    json.dumps(
                        {
                            "reason": "compiler-artifact",
                            "target": {
                                "name": "other",
                                "crate_types": ["cdylib"],
                            },
                            "filenames": [str(Path(temp_dir) / "other.dylib")],
                        }
                    ),
                    json.dumps(
                        {
                            "reason": "compiler-artifact",
                            "target": {
                                "name": recipe.target_name,
                                "crate_types": ["cdylib", "rlib"],
                            },
                            "filenames": [str(library), str(Path(temp_dir) / "lib.rlib")],
                        }
                    ),
                ]
            )

            self.assertEqual(
                verify_platform_bindings.cargo_dynamic_library(stdout, recipe),
                library,
            )

    def test_dart_ffi_smoke_rejects_target_outside_the_recipe(self) -> None:
        recipe = replace(
            verify_platform_bindings.FLUTTER_DESKTOP_NATIVE_RECIPE,
            build_targets=("aarch64-apple-darwin",),
        )
        with (
            mock.patch.object(verify_platform_bindings, "run_capture") as run_capture,
            self.assertRaisesRegex(RuntimeError, "does not declare target"),
        ):
            verify_platform_bindings.run_dart_ffi_native_smoke(
                "dart",
                recipe,
                target="x86_64-unknown-linux-gnu",
            )

        run_capture.assert_not_called()

    def test_explicit_ndk_path_is_forwarded_to_android_slice_builds(self) -> None:
        with (
            mock.patch.object(
                verify_platform_bindings,
                "android_jni_libs_ready",
                return_value=False,
            ),
            mock.patch.object(verify_platform_bindings, "run") as run,
        ):
            verify_platform_bindings.ensure_android_native_slices("/opt/android-ndk")

        command = run.call_args.args[0]
        self.assertEqual(
            command[command.index("--ndk-home") + 1],
            str(Path("/opt/android-ndk").resolve()),
        )

    def test_instrumentation_gate_builds_slices_then_runs_connected_tests(self) -> None:
        with (
            mock.patch.object(
                verify_platform_bindings,
                "ensure_android_native_slices",
            ) as ensure_slices,
            mock.patch.object(
                verify_platform_bindings,
                "resolve_gradle_command",
                return_value="gradle",
            ),
            mock.patch.object(verify_platform_bindings, "run") as run,
        ):
            verify_platform_bindings.run_android_instrumentation_smoke(
                None,
                "/opt/android-ndk",
            )

        ensure_slices.assert_called_once_with("/opt/android-ndk")
        run.assert_called_once_with(
            [
                "gradle",
                "-p",
                str(verify_platform_bindings.ANDROID_ROOT),
                "connectedAndroidTest",
                "--stacktrace",
            ]
        )


class FlutterAndroidSmokeTests(unittest.TestCase):
    def test_generated_consumer_uses_the_plugin_min_sdk(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            project = Path(temp_dir)
            build_file = project / "android" / "app" / "build.gradle.kts"
            build_file.parent.mkdir(parents=True)
            build_file.write_text(
                """android {
    defaultConfig {
        minSdk = flutter.minSdkVersion
    }
}
""",
                encoding="utf-8",
            )

            flutter_android_smoke.configure_android_consumer(project)

            self.assertEqual(
                build_file.read_text(encoding="utf-8"),
                f"""android {{
    defaultConfig {{
        minSdk = {flutter_android_smoke.android_plugin_min_sdk()}
    }}
}}
""",
            )

    def test_requested_abis_only_require_requested_target_outputs(self) -> None:
        self.assertEqual(
            flutter_android_smoke.requested_abis(["aarch64-linux-android"]),
            ("arm64-v8a",),
        )

    def test_requested_abis_reject_unknown_targets(self) -> None:
        with self.assertRaisesRegex(RuntimeError, "unsupported Flutter Android Rust targets"):
            flutter_android_smoke.requested_abis(["armv7-linux-androideabi"])

    def test_clean_output_with_only_the_requested_abi_is_accepted(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            jni_libs = Path(temp_dir)
            library = jni_libs / "arm64-v8a" / "libmerman_ffi.so"
            library.parent.mkdir(parents=True)
            library.touch()

            flutter_android_smoke.verify_requested_native_libraries(
                ["aarch64-linux-android"],
                jni_libs,
            )

    def test_stale_other_abi_cannot_mask_a_missing_requested_output(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            jni_libs = Path(temp_dir)
            stale = jni_libs / "x86_64" / "libmerman_ffi.so"
            stale.parent.mkdir(parents=True)
            stale.touch()

            with self.assertRaises(RuntimeError) as context:
                flutter_android_smoke.verify_requested_native_libraries(
                    ["aarch64-linux-android"],
                    jni_libs,
                )
            self.assertIn(
                str(Path("arm64-v8a") / "libmerman_ffi.so"),
                str(context.exception),
            )


class GeneratedBindingFreshnessTests(unittest.TestCase):
    def test_flutter_binding_must_be_tracked_before_freshness_comparison(
        self,
    ) -> None:
        generated = (
            MODULE_PATH.parents[1]
            / "platforms"
            / "flutter"
            / "lib"
            / "src"
            / "generated"
            / "native_abi.dart"
        )
        with mock.patch.object(verify_platform_bindings, "run") as run:
            verify_platform_bindings.verify_tracked_generated_file(generated)

        relative = "platforms/flutter/lib/src/generated/native_abi.dart"
        self.assertEqual(
            [call.args[0] for call in run.call_args_list],
            [
                ["git", "ls-files", "--error-unmatch", "--", relative],
                ["git", "diff", "--exit-code", "--", relative],
            ],
        )


class AndroidAarVerificationTests(unittest.TestCase):
    def setUp(self) -> None:
        symbol_reader = mock.patch.object(
            verify_platform_bindings,
            "android_library_symbols",
            return_value={"JNI_OnLoad"},
        )
        symbol_reader.start()
        self.addCleanup(symbol_reader.stop)
        class_api_reader = mock.patch.object(
            verify_platform_bindings,
            "android_class_api",
            create=True,
            return_value=(
                "public final io.merman.MermanIconPackSet getIconPackSet();\n"
                "public final io.merman.MermanTextMeasurer getTextMeasurer();\n"
            ),
        )
        class_api_reader.start()
        self.addCleanup(class_api_reader.stop)

    def test_ndk_symbol_tool_can_be_resolved_from_environment(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            ndk = Path(temp_dir)
            executable = (
                "llvm-nm.exe"
                if verify_platform_bindings.os.name == "nt"
                else "llvm-nm"
            )
            tool = (
                ndk
                / "toolchains"
                / "llvm"
                / "prebuilt"
                / "test-host"
                / "bin"
                / executable
            )
            tool.parent.mkdir(parents=True)
            tool.touch()
            with mock.patch.dict(
                verify_platform_bindings.os.environ,
                {"ANDROID_NDK_HOME": str(ndk)},
                clear=True,
            ):
                self.assertEqual(
                    verify_platform_bindings.resolve_android_llvm_nm(),
                    tool.resolve(),
                )

    def test_android_native_library_manifest_covers_published_abis(self) -> None:
        self.assertEqual(
            verify_platform_bindings.ANDROID_NATIVE_LIBRARIES,
            EXPECTED_ANDROID_NATIVE_LIBRARIES,
        )

    def test_android_aar_contains_packaging_sentinels(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            aar_path = Path(temp_dir) / "merman-android-release.aar"
            write_aar(
                aar_path,
                verify_platform_bindings.ANDROID_PACKAGING_SENTINEL_CLASSES,
            )

            verify_platform_bindings.assert_android_aar_contract(aar_path)

    def test_android_aar_reports_missing_packaging_sentinel(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            aar_path = Path(temp_dir) / "merman-android-release.aar"
            sentinels = verify_platform_bindings.ANDROID_PACKAGING_SENTINEL_CLASSES
            for missing in sentinels:
                classes = [
                    class_name
                    for class_name in sentinels
                    if class_name != missing
                ]
                write_aar(aar_path, classes)

                with self.subTest(missing=missing), self.assertRaisesRegex(
                    RuntimeError,
                    missing,
                ):
                    verify_platform_bindings.assert_android_aar_contract(aar_path)

    def test_android_aar_rejects_removed_packaging_class(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            aar_path = Path(temp_dir) / "merman-android-release.aar"
            classes = [
                *verify_platform_bindings.ANDROID_PACKAGING_SENTINEL_CLASSES,
                *verify_platform_bindings.ANDROID_FORBIDDEN_PACKAGING_CLASSES,
            ]
            write_aar(aar_path, classes)

            with self.assertRaisesRegex(RuntimeError, "MermanIconRegistry.class"):
                verify_platform_bindings.assert_android_aar_contract(aar_path)

    def test_android_aar_requires_icon_pack_set_getter(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            aar_path = Path(temp_dir) / "merman-android-release.aar"
            write_aar(
                aar_path,
                verify_platform_bindings.ANDROID_PACKAGING_SENTINEL_CLASSES,
            )
            with (
                mock.patch.object(
                    verify_platform_bindings,
                    "android_class_api",
                    return_value="public final io.merman.MermanTextMeasurer getTextMeasurer();\n",
                ),
                self.assertRaisesRegex(RuntimeError, "getIconPackSet"),
            ):
                verify_platform_bindings.assert_android_aar_contract(aar_path)

    def test_android_aar_rejects_removed_icon_registry_getter(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            aar_path = Path(temp_dir) / "merman-android-release.aar"
            write_aar(
                aar_path,
                verify_platform_bindings.ANDROID_PACKAGING_SENTINEL_CLASSES,
            )
            with (
                mock.patch.object(
                    verify_platform_bindings,
                    "android_class_api",
                    return_value=(
                        "public final io.merman.MermanIconPackSet getIconPackSet();\n"
                        "public final io.merman.MermanIconRegistry getIconRegistry();\n"
                    ),
                ),
                self.assertRaisesRegex(RuntimeError, "getIconRegistry"),
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
                verify_platform_bindings.ANDROID_PACKAGING_SENTINEL_CLASSES,
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
                if name != "jni/x86_64/libmerman_android_jni.so"
            ]
            write_aar(
                aar_path,
                verify_platform_bindings.ANDROID_PACKAGING_SENTINEL_CLASSES,
                native_libraries=native_libraries,
            )

            with self.assertRaisesRegex(
                RuntimeError,
                "jni/x86_64/libmerman_android_jni.so",
            ):
                verify_platform_bindings.assert_android_aar_contract(aar_path)

    def test_android_aar_rejects_c_abi_export_in_jni_library(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            aar_path = Path(temp_dir) / "merman-android-release.aar"
            write_aar(
                aar_path,
                verify_platform_bindings.ANDROID_PACKAGING_SENTINEL_CLASSES,
            )

            with (
                mock.patch.object(
                    verify_platform_bindings,
                    "android_library_symbols",
                    return_value={"JNI_OnLoad", "merman_get_native_api"},
                ),
                self.assertRaisesRegex(RuntimeError, "forbidden.*merman_get_native_api"),
            ):
                verify_platform_bindings.assert_android_aar_contract(aar_path)

    def test_android_aar_rejects_an_extra_c_abi_transport_library(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            aar_path = Path(temp_dir) / "merman-android-release.aar"
            write_aar(
                aar_path,
                verify_platform_bindings.ANDROID_PACKAGING_SENTINEL_CLASSES,
                native_libraries=[
                    *EXPECTED_ANDROID_NATIVE_LIBRARIES,
                    "jni/arm64-v8a/libmerman_ffi.so",
                ],
            )

            with self.assertRaisesRegex(
                RuntimeError,
                "unexpected Merman native libraries.*libmerman_ffi.so",
            ):
                verify_platform_bindings.assert_android_aar_contract(aar_path)


class AndroidConsumerRulesTests(unittest.TestCase):
    def test_jni_registration_and_result_construction_classes_are_kept(self) -> None:
        rules = ANDROID_CONSUMER_RULES.read_text(encoding="utf-8")

        for class_name in (
            "io.merman.Merman",
            "io.merman.MermanEngine",
            "io.merman.MermanOperationResult",
        ):
            with self.subTest(class_name=class_name):
                self.assertIn(f"-keep class {class_name} {{ *; }}", rules)

    def test_jni_host_text_measurement_reflection_members_are_kept(self) -> None:
        rules = ANDROID_CONSUMER_RULES.read_text(encoding="utf-8")

        self.assertIn(
            "-keep,allowoptimization interface io.merman.MermanTextMeasurer {",
            rules,
        )
        self.assertIn(
            "io.merman.MermanTextMeasureResult measure(io.merman.MermanTextMeasureRequest);",
            rules,
        )
        self.assertIn(
            "-keepclassmembers,allowoptimization class * implements io.merman.MermanTextMeasurer {",
            rules,
        )
        self.assertIn(
            "-keep,allowoptimization class io.merman.MermanTextMeasureRequest {",
            rules,
        )
        self.assertIn(
            "<init>(java.lang.String,java.lang.String,double,java.lang.String,java.lang.String,java.lang.Double,double,double,double,int,int,int,int,int);",
            rules,
        )
        self.assertIn(
            "-keep,allowoptimization class io.merman.MermanTextMeasureResult {",
            rules,
        )

        expected_fields = {
            "resultKind": "int",
            "width": "double",
            "height": "double",
            "length": "double",
            "lineCount": "long",
            "bboxLeft": "double",
            "bboxRight": "double",
            "rawWidth": "double",
            "hasRawWidth": "boolean",
        }
        for field, field_type in expected_fields.items():
            with self.subTest(field=field):
                self.assertIn(f"    {field_type} {field};", rules)


class AndroidMavenPublicationTests(unittest.TestCase):
    def setUp(self) -> None:
        symbol_reader = mock.patch.object(
            verify_platform_bindings,
            "android_library_symbols",
            return_value={"JNI_OnLoad"},
        )
        symbol_reader.start()
        self.addCleanup(symbol_reader.stop)
        class_api_reader = mock.patch.object(
            verify_platform_bindings,
            "android_class_api",
            create=True,
            return_value=(
                "public final io.merman.MermanIconPackSet getIconPackSet();\n"
                "public final io.merman.MermanTextMeasurer getTextMeasurer();\n"
            ),
        )
        class_api_reader.start()
        self.addCleanup(class_api_reader.stop)

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

    def test_android_maven_publication_rejects_removed_javadoc_entry(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            module_root = Path(temp_dir) / "merman-android"
            version_dir = write_android_maven_publication(module_root)
            javadoc_jar = next(version_dir.glob("*-javadoc.jar"))
            with zipfile.ZipFile(javadoc_jar, "a") as archive:
                archive.writestr(
                    "merman-android/io.merman/-merman-icon-registry/index.html",
                    b"",
                )

            with self.assertRaisesRegex(RuntimeError, "removed.*merman-icon-registry"):
                verify_platform_bindings._assert_android_javadoc_jar(javadoc_jar)

    def test_android_maven_publication_rejects_removed_service_property(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            module_root = Path(temp_dir) / "merman-android"
            version_dir = write_android_maven_publication(module_root)
            javadoc_jar = next(version_dir.glob("*-javadoc.jar"))
            with zipfile.ZipFile(javadoc_jar, "a") as archive:
                archive.writestr(
                    "merman-android/io.merman/-merman-engine-services/icon-registry.html",
                    b"",
                )

            with self.assertRaisesRegex(RuntimeError, "removed.*icon-registry"):
                verify_platform_bindings._assert_android_javadoc_jar(javadoc_jar)


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
    write_aar(
        aar_path,
        verify_platform_bindings.ANDROID_PACKAGING_SENTINEL_CLASSES,
    )

    kotlin_root = MODULE_PATH.parents[1] / "platforms" / "android" / "src" / "main" / "kotlin"
    with zipfile.ZipFile(source_jar, "w") as archive:
        for source_path in sorted(kotlin_root.rglob("*.kt")):
            archive.writestr(source_path.relative_to(kotlin_root).as_posix(), b"")

    with zipfile.ZipFile(javadoc_jar, "w") as archive:
        for entry in [
            "index.html",
            "merman-android/package-list",
            "merman-android/io.merman/index.html",
            "merman-android/io.merman/-merman/index.html",
            "merman-android/io.merman/-merman-engine/index.html",
            "merman-android/io.merman/-merman-engine-services/index.html",
            "merman-android/io.merman/-merman-engine-services/icon-pack-set.html",
            "merman-android/io.merman/-merman-icon-pack/index.html",
            "merman-android/io.merman/-merman-icon-pack-set/index.html",
            "merman-android/io.merman/-merman-operation-metadata/index.html",
            "merman-android/io.merman/-merman-operation-result/index.html",
            "merman-android/io.merman/-merman-output-plan/index.html",
            "merman-android/io.merman/-merman-raster-output-plan/index.html",
            "merman-android/io.merman/-merman-pdf-filter-images-output-plan/index.html",
            "merman-android/io.merman/-merman-unknown-output-plan/index.html",
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
