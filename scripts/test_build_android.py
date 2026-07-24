#!/usr/bin/env python3
"""Unit tests for the reproducible Android native build toolchain."""

from __future__ import annotations

import importlib.util
from dataclasses import replace
import tempfile
import unittest
from pathlib import Path
from unittest import mock


MODULE_PATH = Path(__file__).parents[1] / "platforms" / "android" / "build-android.py"
SPEC = importlib.util.spec_from_file_location("build_android", MODULE_PATH)
assert SPEC is not None
build_android = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(build_android)


class AndroidToolchainTests(unittest.TestCase):
    def test_catalog_defines_exact_ndk_version(self) -> None:
        self.assertRegex(build_android.PINNED_NDK_VERSION, r"^\d+\.\d+\.\d+$")

    def test_catalog_pins_android_builds_to_java_17(self) -> None:
        self.assertEqual(build_android.PINNED_JAVA_MAJOR, 17)

    def test_java_major_parser_accepts_modern_and_legacy_output(self) -> None:
        self.assertEqual(
            build_android.parse_java_major('openjdk version "17.0.19" 2026-04-21'),
            17,
        )
        self.assertEqual(build_android.parse_java_major('java version "1.8.0_412"'), 8)

    def test_java_major_parser_rejects_unknown_output(self) -> None:
        with self.assertRaisesRegex(RuntimeError, "could not parse Java version"):
            build_android.parse_java_major("not a Java version")

    def test_explicit_java_home_must_match_pinned_major(self) -> None:
        with (
            tempfile.TemporaryDirectory() as temp_dir,
            mock.patch.object(build_android, "java_major", return_value=26),
            self.assertRaisesRegex(RuntimeError, "does not point to a JDK 17"),
        ):
            build_android.resolve_pinned_java_home(temp_dir)

    def test_java_discovery_skips_incompatible_jdks(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            wrong = root / "jdk-26"
            pinned = root / "jdk-17"
            with (
                mock.patch.object(
                    build_android,
                    "java_home_candidates",
                    return_value=[wrong, pinned],
                ),
                mock.patch.object(build_android, "java_major", side_effect=[26, 17]),
            ):
                self.assertEqual(build_android.resolve_pinned_java_home(), pinned)

    def test_gradle_environment_is_scoped_to_resolved_java_home(self) -> None:
        java_home = Path("/resolved/jdk-17")
        with mock.patch.dict(build_android.os.environ, {"PATH": "/usr/bin"}, clear=True):
            env = build_android.gradle_environment(java_home)

        self.assertEqual(env["JAVA_HOME"], str(java_home))
        self.assertEqual(env["PATH"].split(build_android.os.pathsep)[0], str(java_home / "bin"))

    def test_explicit_ndk_must_match_pinned_revision(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            ndk = Path(temp_dir)
            (ndk / "source.properties").write_text(
                "Pkg.Revision = 1.0.0\n",
                encoding="utf-8",
            )

            with self.assertRaisesRegex(RuntimeError, "does not match pinned"):
                build_android.default_ndk_home(str(ndk))

    def test_explicit_pinned_ndk_is_accepted(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            ndk = Path(temp_dir)
            (ndk / "source.properties").write_text(
                f"Pkg.Revision = {build_android.PINNED_NDK_VERSION}\n",
                encoding="utf-8",
            )

            self.assertEqual(build_android.default_ndk_home(str(ndk)), ndk.resolve())

    def test_repository_carries_a_cross_platform_gradle_wrapper(self) -> None:
        self.assertTrue((build_android.ANDROID_ROOT / "gradlew").is_file())
        self.assertTrue((build_android.ANDROID_ROOT / "gradlew.bat").is_file())
        command = build_android.gradle_wrapper_command()
        self.assertIn("gradlew", command[-1])

    def test_default_targets_are_owned_by_the_artifact_recipe(self) -> None:
        with mock.patch.object(build_android.sys, "argv", ["build-android.py"]):
            args = build_android.parse_args()

        self.assertEqual(
            args.targets,
            list(build_android.ANDROID_NATIVE_RECIPE.build_targets),
        )
        self.assertFalse(hasattr(args, "profile"))

    def test_builder_does_not_duplicate_the_profile_capability_tuple(self) -> None:
        recipe = replace(build_android.ANDROID_NATIVE_RECIPE, features=("svg",))

        build_android.validate_android_native_recipe(recipe)

    def test_native_sdk_build_arguments_are_fully_recipe_owned(self) -> None:
        target = "aarch64-linux-android"
        args = build_android.cargo_build_args(target)

        self.assertEqual(args[:4], ["cargo", "build", "--profile", "native-sdk"])
        self.assertIn("--package", args)
        self.assertEqual(
            args[args.index("--package") + 1], build_android.ANDROID_NATIVE_RECIPE.package
        )
        self.assertIn("--lib", args)
        self.assertIn("--no-default-features", args)
        self.assertEqual(
            args[args.index("--features") + 1],
            build_android.ANDROID_NATIVE_RECIPE.feature_argument,
        )
        self.assertEqual(args[args.index("--target") + 1], target)
        self.assertEqual(
            args[args.index("--manifest-path") + 1],
            str(
                build_android.REPO_ROOT
                / build_android.ANDROID_NATIVE_RECIPE.manifest
            ),
        )

    def test_android_build_rejects_a_target_outside_the_recipe(self) -> None:
        with self.assertRaisesRegex(RuntimeError, "does not declare target"):
            build_android.cargo_build_args("armv7-linux-androideabi")


if __name__ == "__main__":
    unittest.main()
