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

try:
    from scripts.github_workflow_contract import (
        load_workflow_contract,
        workflow_job,
        workflow_step,
    )
except ModuleNotFoundError:
    from github_workflow_contract import (
        load_workflow_contract,
        workflow_job,
        workflow_step,
    )

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

EXPECTED_ANDROID_WRAPPER_CLASSES = [
    "io/merman/MermanEngine.class",
    "io/merman/MermanErrorKind.class",
    "io/merman/MermanOperationResult.class",
    "io/merman/MermanReusableEngine.class",
    "io/merman/MermanException.class",
    "io/merman/MermanResourceLimitId.class",
    "io/merman/MermanResourceOptions.class",
    "io/merman/MermanResourceOptionsBuilder.class",
    "io/merman/MermanResourceProfile.class",
    "io/merman/MermanTextMeasureRequest.class",
    "io/merman/MermanTextMeasureResult.class",
    "io/merman/MermanTextDirection.class",
    "io/merman/MermanTextMeasurementPhase.class",
    "io/merman/MermanTextMeasurementOperation.class",
    "io/merman/MermanTextMeasurementResultKind.class",
    "io/merman/MermanTextMeasurer.class",
    "io/merman/MermanTextWhiteSpace.class",
    "io/merman/MermanTextWrapMode.class",
]
EXPECTED_ANDROID_NATIVE_LIBRARIES = [
    "jni/arm64-v8a/libmerman_android_jni.so",
    "jni/x86_64/libmerman_android_jni.so",
]
ANDROID_CONSUMER_RULES = (
    MODULE_PATH.parents[1] / "platforms" / "android" / "consumer-rules.pro"
)


class SemanticOperationFixtureTests(unittest.TestCase):
    def test_canonical_fixture_uses_the_strict_semantic_schema(self) -> None:
        cases = verify_platform_bindings.load_semantic_operation_fixtures()

        self.assertEqual(len(cases), 6)
        self.assertEqual(cases[0]["operation_id"], "semantic-json")
        self.assertTrue(all(case["operation_id"] != "not-an-operation" for case in cases))

    def test_duplicate_json_keys_are_rejected_at_the_shared_gate(self) -> None:
        canonical = verify_platform_bindings.SEMANTIC_OPERATION_FIXTURES.read_text(
            encoding="utf-8"
        )
        malformed = canonical.replace(
            '"max_svg_bytes": 1024',
            '"max_svg_bytes": 1024,\n            "max_svg_bytes": 2048',
            1,
        )

        with tempfile.TemporaryDirectory() as temp_dir:
            path = Path(temp_dir) / "fixtures.json"
            path.write_text(malformed, encoding="utf-8")
            with self.assertRaisesRegex(RuntimeError, "duplicate JSON key"):
                verify_platform_bindings.load_semantic_operation_fixtures(path)

    def test_transport_wire_fields_are_rejected(self) -> None:
        root = json.loads(
            verify_platform_bindings.SEMANTIC_OPERATION_FIXTURES.read_text(
                encoding="utf-8"
            )
        )
        root["cases"][0]["native_operation_code"] = 6

        with tempfile.TemporaryDirectory() as temp_dir:
            path = Path(temp_dir) / "fixtures.json"
            path.write_text(json.dumps(root), encoding="utf-8")
            with self.assertRaisesRegex(RuntimeError, "unknown fields"):
                verify_platform_bindings.load_semantic_operation_fixtures(path)

    def test_each_case_declares_exactly_one_outcome(self) -> None:
        root = json.loads(
            verify_platform_bindings.SEMANTIC_OPERATION_FIXTURES.read_text(
                encoding="utf-8"
            )
        )
        root["cases"][0]["expected_error_kind"] = "generic"

        with tempfile.TemporaryDirectory() as temp_dir:
            path = Path(temp_dir) / "fixtures.json"
            path.write_text(json.dumps(root), encoding="utf-8")
            with self.assertRaisesRegex(RuntimeError, "exactly one expected outcome"):
                verify_platform_bindings.load_semantic_operation_fixtures(path)


class NativeSdkRecipeTests(unittest.TestCase):
    def test_android_transport_checks_use_each_exact_descriptor_recipe(self) -> None:
        target = "aarch64-linux-android"
        for recipe in (
            verify_platform_bindings.ANDROID_NATIVE_RECIPE,
            verify_platform_bindings.FLUTTER_ANDROID_NATIVE_RECIPE,
        ):
            with self.subTest(profile=recipe.profile_id):
                args = verify_platform_bindings.cargo_android_check_args(
                    recipe,
                    "check",
                    target,
                )
                self.assertEqual(args[:3], ["cargo", "check", "--locked"])
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
            verify_platform_bindings.cargo_android_check_args(
                verify_platform_bindings.ANDROID_NATIVE_RECIPE,
                "check",
                "armv7-linux-androideabi",
            )

    def test_dart_ffi_smoke_consumes_the_exact_flutter_desktop_recipe(self) -> None:
        recipe = verify_platform_bindings.FLUTTER_DESKTOP_NATIVE_RECIPE
        target = "x86_64-unknown-linux-gnu"
        with mock.patch.object(verify_platform_bindings, "run") as run:
            verify_platform_bindings.run_dart_ffi_native_smoke(
                "dart",
                target=target,
                host_system="Linux",
            )

        build = run.call_args_list[0]
        self.assertEqual(
            build.args[0][:4],
            ["cargo", "build", "--profile", "native-sdk"],
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
        self.assertEqual(
            run.call_args_list[1].args[0],
            [
                "dart",
                "run",
                "example/smoke.dart",
                str(
                    MODULE_PATH.parents[1]
                    / "target"
                    / target
                    / "native-sdk"
                    / "libmerman_ffi.so"
                ),
            ],
        )
        self.assertEqual(
            run.call_args_list[2].args[0],
            [
                "dart",
                "run",
                "tool/semantic_operation_fixtures_test.dart",
                str(
                    MODULE_PATH.parents[1]
                    / "target"
                    / target
                    / "native-sdk"
                    / "libmerman_ffi.so"
                ),
            ],
        )

    def test_native_library_path_is_derived_from_recipe_identity(self) -> None:
        recipe = replace(
            verify_platform_bindings.C_ABI_NATIVE_RECIPE,
            target_name="custom-ffi",
            cargo_profile="custom-profile",
        )
        self.assertEqual(
            verify_platform_bindings.host_dynamic_library(
                recipe,
                target="aarch64-apple-darwin",
                host_system="Darwin",
            ),
            MODULE_PATH.parents[1]
            / "target"
            / "aarch64-apple-darwin"
            / "custom-profile"
            / "libcustom_ffi.dylib",
        )

    def test_dart_ffi_smoke_rejects_target_outside_the_recipe(self) -> None:
        recipe = replace(
            verify_platform_bindings.FLUTTER_DESKTOP_NATIVE_RECIPE,
            build_targets=("aarch64-apple-darwin",),
        )
        with (
            mock.patch.object(verify_platform_bindings, "run") as run,
            self.assertRaisesRegex(RuntimeError, "does not declare target"),
        ):
            verify_platform_bindings.run_dart_ffi_native_smoke(
                "dart",
                recipe,
                target="x86_64-unknown-linux-gnu",
                host_system="Linux",
            )

        run.assert_not_called()

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

    def test_instrumentation_gate_uses_one_explicit_ndk_for_build_and_symbols(self) -> None:
        llvm_nm = Path("/opt/android-ndk/bin/llvm-nm")
        with (
            mock.patch.object(
                verify_platform_bindings,
                "resolve_android_llvm_nm",
                return_value=llvm_nm,
            ) as resolve_nm,
            mock.patch.object(
                verify_platform_bindings,
                "ensure_android_native_slices",
            ) as ensure_slices,
            mock.patch.object(
                verify_platform_bindings,
                "resolve_gradle_command",
                return_value="gradle",
            ),
            mock.patch.object(verify_platform_bindings, "run"),
            mock.patch.object(
                verify_platform_bindings,
                "assert_android_aar_contract",
            ) as assert_aar,
            mock.patch.object(
                verify_platform_bindings,
                "assert_android_instrumentation_smoke_report",
            ),
        ):
            verify_platform_bindings.run_android_instrumentation_smoke(
                None,
                "/opt/android-ndk",
            )

        resolve_nm.assert_called_once_with("/opt/android-ndk")
        ensure_slices.assert_called_once_with("/opt/android-ndk")
        assert_aar.assert_called_once_with(llvm_nm=llvm_nm)


class FlutterAndroidSmokeTests(unittest.TestCase):
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

    def test_flutter_ci_uses_the_fail_closed_generated_binding_gate(self) -> None:
        document = load_workflow_contract(
            MODULE_PATH.parents[1] / ".github" / "workflows" / "ci.yml"
        )
        job = workflow_job(document, "platform-bindings")
        step = workflow_step(job, name="Verify platform bindings")
        self.assertEqual(step["run"], "python3 scripts/verify-platform-bindings.py")


class AndroidAarVerificationTests(unittest.TestCase):
    def setUp(self) -> None:
        symbol_reader = mock.patch.object(
            verify_platform_bindings,
            "android_library_symbols",
            return_value={"JNI_OnLoad"},
        )
        symbol_reader.start()
        self.addCleanup(symbol_reader.stop)

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
        multi_type_sources = {
            "MermanResourceOptions": (
                "MermanResourceLimitId",
                "MermanResourceOptions",
                "MermanResourceOptionsBuilder",
                "MermanResourceProfile",
            ),
            "MermanTextMeasurementVocabulary": (
                "MermanTextDirection",
                "MermanTextMeasurementPhase",
                "MermanTextWhiteSpace",
                "MermanTextWrapMode",
            ),
        }
        source_classes = sorted(
            f"io/merman/{class_name}.class"
            for source_path in kotlin_root.glob("*.kt")
            for class_name in multi_type_sources.get(
                source_path.stem,
                (source_path.stem,),
            )
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
                if name != "jni/x86_64/libmerman_android_jni.so"
            ]
            write_aar(
                aar_path,
                EXPECTED_ANDROID_WRAPPER_CLASSES,
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
            write_aar(aar_path, EXPECTED_ANDROID_WRAPPER_CLASSES)

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
                EXPECTED_ANDROID_WRAPPER_CLASSES,
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
            "io.merman.MermanEngine",
            "io.merman.MermanReusableEngine",
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
                <testsuites>
                  <testsuite name="io.merman.MermanInstrumentedSmokeTest">
                    <testcase name="runsPublicSmokeIncludingThrowingTextMeasurerFallback" />
                  </testsuite>
                  <testsuite name="io.merman.MermanSemanticOperationFixtureTest">
                    <testcase name="consumesSharedSemanticOperationFixtures" />
                  </testsuite>
                </testsuites>
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

    def test_android_instrumentation_report_requires_shared_fixture_result(self) -> None:
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

            with self.assertRaisesRegex(
                RuntimeError, "MermanSemanticOperationFixtureTest"
            ):
                verify_platform_bindings.assert_android_instrumentation_smoke_report(
                    results_root
                )


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
