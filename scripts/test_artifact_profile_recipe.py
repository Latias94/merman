#!/usr/bin/env python3
"""Tests for exact artifact recipe projection."""

from __future__ import annotations

from collections.abc import Callable
from contextlib import nullcontext
from dataclasses import replace
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import tomllib
from types import SimpleNamespace
import unittest
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))

import artifact_profile_recipe
from artifact_profile_recipe import (
    cargo_build_args,
    cargo_host_build_args,
    load_artifact_profile,
)


WHEEL_BUILDER_PATH = Path(__file__).resolve().parent / "build-python-uniffi-wheel.py"
WHEEL_BUILDER_SPEC = importlib.util.spec_from_file_location(
    "build_python_uniffi_wheel", WHEEL_BUILDER_PATH
)
assert WHEEL_BUILDER_SPEC is not None
wheel_builder = importlib.util.module_from_spec(WHEEL_BUILDER_SPEC)
assert WHEEL_BUILDER_SPEC.loader is not None
WHEEL_BUILDER_SPEC.loader.exec_module(wheel_builder)


def posix_recipe_shell(
    *,
    os_name: str | None = None,
    which: Callable[[str], str | None] = shutil.which,
) -> str | None:
    """Return Bash only when this host owns POSIX recipe execution."""
    resolved_os_name = os.name if os_name is None else os_name
    if resolved_os_name != "posix":
        return None
    return which("bash")


class ArtifactProfileRecipeTests(unittest.TestCase):
    def test_native_distribution_profiles_own_their_release_optimization_policies(self) -> None:
        repo_root = Path(__file__).resolve().parents[1]
        with (repo_root / "Cargo.toml").open("rb") as handle:
            workspace = tomllib.load(handle)

        self.assertEqual(
            workspace["profile"]["native-sdk"],
            {
                "inherits": "release",
                "opt-level": 3,
                "lto": "thin",
                "codegen-units": 1,
                "debug": False,
                "strip": "debuginfo",
                "panic": "unwind",
                "incremental": False,
                "debug-assertions": False,
                "overflow-checks": False,
                "rpath": False,
            },
        )
        self.assertEqual(
            workspace["profile"]["native-distribution"],
            {
                "inherits": "release",
                "opt-level": "s",
                "lto": True,
                "codegen-units": 1,
                "debug": False,
                "strip": "symbols",
                "panic": "unwind",
                "incremental": False,
                "debug-assertions": False,
                "overflow-checks": False,
                "rpath": False,
                "build-override": {"strip": "none"},
            },
        )

    def test_committed_native_recipes_have_owner_specific_structure(self) -> None:
        expected_recipes = {
            "android-native": ("merman-android-jni", "native-distribution"),
            "apple-uniffi-native": ("merman-uniffi", "native-sdk"),
            "c-abi-native": ("merman-ffi", "native-sdk"),
            "flutter-android-native": ("merman-ffi", "native-distribution"),
            "flutter-desktop-native": ("merman-ffi", "native-distribution"),
            "flutter-ios-native": ("merman-ffi", "native-distribution"),
            "python-uniffi-native": ("merman-uniffi", "native-distribution"),
        }
        for profile_id, (package, cargo_profile) in expected_recipes.items():
            with self.subTest(profile_id=profile_id):
                recipe = load_artifact_profile(profile_id)
                self.assertEqual(recipe.package, package)
                self.assertEqual(recipe.cargo_profile, cargo_profile)
                self.assertFalse(recipe.default_features)
                self.assertTrue(recipe.features)

    def test_prebuilt_native_profiles_share_one_default_sku(self) -> None:
        profile_ids = (
            "android-native",
            "apple-uniffi-native",
            "flutter-android-native",
            "flutter-desktop-native",
            "flutter-ios-native",
            "python-uniffi-native",
        )
        descriptor = json.loads(
            artifact_profile_recipe.DEFAULT_DESCRIPTOR.read_text(encoding="utf-8")
        )
        profiles = {profile["id"]: profile for profile in descriptor["profiles"]}
        baseline = profiles[profile_ids[0]]

        self.assertEqual(
            baseline["cargo"]["features"],
            ["analysis", "ascii", "layout-cytoscape", "layout-elk", "svg"],
        )
        self.assertEqual(
            baseline["expected"]["capabilities"],
            ["analysis", "ascii", "layout-cytoscape", "layout-elk", "svg"],
        )
        self.assertEqual(baseline["expected"]["outputs"], ["ascii", "svg"])

        for profile_id in profile_ids[1:]:
            with self.subTest(profile_id=profile_id):
                candidate = profiles[profile_id]
                self.assertEqual(
                    candidate["cargo"]["features"],
                    baseline["cargo"]["features"],
                )
                self.assertEqual(candidate["expected"], baseline["expected"])

    def test_c_abi_reference_profile_remains_complete(self) -> None:
        recipe = load_artifact_profile("c-abi-native")
        self.assertEqual(
            recipe.features,
            (
                "analysis",
                "ascii",
                "jpeg",
                "layout-cytoscape",
                "layout-elk",
                "math",
                "native-runtime",
                "pdf",
                "png",
                "svg",
            ),
        )

    def test_repository_native_commands_reject_profile_environment_overrides(self) -> None:
        cases = (
            ("c-abi-native", "CARGO_PROFILE_NATIVE_SDK_OPT_LEVEL"),
            (
                "flutter-desktop-native",
                "CARGO_PROFILE_NATIVE_DISTRIBUTION_OPT_LEVEL",
            ),
        )
        for profile_id, variable in cases:
            recipe = load_artifact_profile(profile_id)
            with (
                self.subTest(profile_id=profile_id),
                mock.patch.dict(os.environ, {variable: "z"}, clear=False),
                self.assertRaisesRegex(RuntimeError, variable),
            ):
                cargo_build_args(recipe)

    def test_flutter_recipes_own_exact_cross_platform_target_sets(self) -> None:
        expected_targets = {
            "flutter-android-native": (
                "aarch64-linux-android",
                "armv7-linux-androideabi",
                "x86_64-linux-android",
            ),
            "flutter-ios-native": (
                "aarch64-apple-ios",
                "aarch64-apple-ios-sim",
                "x86_64-apple-ios",
            ),
            "flutter-desktop-native": (
                "aarch64-apple-darwin",
                "aarch64-unknown-linux-gnu",
                "x86_64-apple-darwin",
                "x86_64-pc-windows-gnu",
                "x86_64-unknown-linux-gnu",
            ),
        }
        for profile_id, triples in expected_targets.items():
            with self.subTest(profile_id=profile_id):
                recipe = load_artifact_profile(profile_id)
                self.assertEqual(recipe.package, "merman-ffi")
                self.assertEqual(recipe.manifest, "crates/merman-ffi/Cargo.toml")
                self.assertEqual(recipe.cargo_profile, "native-distribution")
                self.assertFalse(recipe.default_features)
                self.assertEqual(recipe.build_target_kind, "target-set")
                self.assertEqual(recipe.build_targets, triples)
                self.assertEqual(recipe.target_name, "merman_ffi")
                self.assertEqual(
                    recipe.crate_types,
                    ("cdylib", "rlib", "staticlib"),
                )

    def test_android_jni_transport_uses_an_independent_internal_crate(self) -> None:
        android = load_artifact_profile("android-native")
        flutter = load_artifact_profile("flutter-android-native")
        c_abi = load_artifact_profile("c-abi-native")

        self.assertEqual(android.package, "merman-android-jni")
        self.assertEqual(android.manifest, "crates/merman-android-jni/Cargo.toml")
        self.assertEqual(android.target_name, "merman_android_jni")
        self.assertEqual(android.crate_types, ("cdylib",))
        self.assertEqual(android.features, flutter.features)
        self.assertNotEqual(android.features, c_abi.features)
        self.assertIn("analysis", flutter.features)
        self.assertIn("ascii", flutter.features)
        self.assertNotIn("jpeg", flutter.features)
        self.assertNotIn("math", flutter.features)
        self.assertNotIn("native-runtime", flutter.features)
        self.assertNotIn("pdf", flutter.features)
        self.assertNotIn("png", flutter.features)

    def test_posix_recipe_shell_requires_both_owner_host_and_bash(self) -> None:
        lookups: list[str] = []

        def available(command: str) -> str:
            lookups.append(command)
            return "/usr/bin/bash"

        self.assertEqual(
            posix_recipe_shell(os_name="posix", which=available),
            "/usr/bin/bash",
        )
        self.assertEqual(lookups, ["bash"])
        self.assertIsNone(
            posix_recipe_shell(os_name="posix", which=lambda _command: None)
        )
        lookups.clear()
        self.assertIsNone(posix_recipe_shell(os_name="nt", which=available))
        self.assertEqual(lookups, [])

    def test_native_shell_consumers_validate_the_committed_recipe_at_runtime(self) -> None:
        repo_root = Path(__file__).resolve().parents[1]
        bash = posix_recipe_shell()
        if bash is None:
            if os.name != "posix":
                self.skipTest("POSIX recipe execution belongs to POSIX owner hosts")
            self.skipTest("Bash is unavailable for the POSIX owner recipe smoke")
        environment = os.environ.copy()
        environment["MERMAN_CHECK_RECIPE_ONLY"] = "true"
        for relative_path in ("scripts/build-apple-xcframework.sh",):
            with self.subTest(path=relative_path):
                subprocess.run(
                    [bash, str(repo_root / relative_path)],
                    cwd=repo_root,
                    env=environment,
                    check=True,
                    capture_output=True,
                    text=True,
                )

    def test_apple_binding_generator_uses_a_fresh_isolated_target(self) -> None:
        repo_root = Path(__file__).resolve().parents[1]
        script = (repo_root / "scripts/build-apple-xcframework.sh").read_text(
            encoding="utf-8"
        )

        self.assertIn(
            'BINDING_GENERATOR_TARGET_DIR="$OUT_DIR/binding-generator-target"',
            script,
        )
        self.assertIn(
            'CARGO_TARGET_DIR="$BINDING_GENERATOR_TARGET_DIR" cargo run',
            script,
        )
        self.assertIn("trap cleanup_binding_generator_target EXIT", script)

    def test_capability_bearing_workspace_crates_have_exact_profiles(self) -> None:
        repo_root = Path(__file__).resolve().parents[1]
        with (repo_root / "Cargo.toml").open("rb") as handle:
            workspace = tomllib.load(handle)
        descriptor = json.loads(
            (repo_root / "capabilities/artifact-profiles-v1.json").read_text(
                encoding="utf-8"
            )
        )
        required_packages = set()
        for member in workspace["workspace"]["members"]:
            manifest = repo_root / member / "Cargo.toml"
            with manifest.open("rb") as handle:
                package = tomllib.load(handle)
            marker = (
                package["package"]
                .get("metadata", {})
                .get("merman", {})
                .get("artifact-profile-required")
            )
            if marker is not None:
                self.assertIs(marker, True, f"invalid artifact profile marker: {manifest}")
                required_packages.add(
                    (package["package"]["name"], f"{member}/Cargo.toml")
                )

        covered_packages = {
            (profile["cargo"]["package"], profile["cargo"]["manifest"])
            for profile in descriptor["profiles"]
        }
        self.assertSetEqual(covered_packages, required_packages)

    def test_c_abi_build_command_is_fully_recipe_owned(self) -> None:
        repo_root = Path(__file__).resolve().parents[1]
        recipe = load_artifact_profile("c-abi-native")
        command = cargo_build_args(recipe, locked=True)
        self.assertEqual(command[:4], ["cargo", "build", "--profile", "native-sdk"])
        self.assertEqual(command[command.index("--package") + 1], recipe.package)
        self.assertEqual(
            command[command.index("--manifest-path") + 1],
            str(repo_root / recipe.manifest),
        )
        self.assertIn("--lib", command)
        self.assertIn("--locked", command)
        self.assertIn("--no-default-features", command)
        self.assertEqual(
            command[command.index("--features") + 1], recipe.feature_argument
        )

    def test_host_build_command_validates_the_host_without_changing_output_layout(
        self,
    ) -> None:
        recipe = load_artifact_profile("cli-release")
        command = cargo_host_build_args(
            recipe,
            "x86_64-unknown-linux-gnu",
            locked=True,
        )

        self.assertEqual(command[:4], ["cargo", "build", "--profile", "dist"])
        self.assertIn("--locked", command)
        self.assertNotIn("--target", command)
        self.assertEqual(
            command[command.index("--features") + 1],
            recipe.feature_argument,
        )
        with self.assertRaisesRegex(RuntimeError, "does not declare host target"):
            cargo_host_build_args(recipe, "aarch64-unknown-linux-gnu")

    def test_host_build_cli_uses_the_detected_rust_target(self) -> None:
        argv = [
            "artifact_profile_recipe.py",
            "cli-release",
            "--build-host",
            "--locked",
        ]
        with (
            mock.patch.object(sys, "argv", argv),
            mock.patch.object(
                artifact_profile_recipe,
                "rustc_host_target",
                return_value="x86_64-unknown-linux-gnu",
            ),
            mock.patch.object(artifact_profile_recipe.subprocess, "run") as run,
        ):
            self.assertEqual(artifact_profile_recipe.main(), 0)

        command = run.call_args.args[0]
        self.assertEqual(command[:4], ["cargo", "build", "--profile", "dist"])
        self.assertNotIn("--target", command)
        self.assertEqual(run.call_args.kwargs["cwd"], artifact_profile_recipe.REPO_ROOT)
        self.assertTrue(run.call_args.kwargs["check"])

    def test_rustc_host_target_fails_closed_on_command_or_output_errors(self) -> None:
        cases = (
            (
                SimpleNamespace(returncode=1, stdout="", stderr="rustc failed"),
                "could not detect",
            ),
            (
                SimpleNamespace(returncode=0, stdout="host: \n", stderr=""),
                "did not report",
            ),
        )
        for completed, message in cases:
            with (
                self.subTest(completed=completed),
                mock.patch.object(
                    artifact_profile_recipe.subprocess,
                    "run",
                    return_value=completed,
                ),
                self.assertRaisesRegex(RuntimeError, message),
            ):
                artifact_profile_recipe.rustc_host_target()

    def test_python_production_and_generator_commands_have_separate_features(self) -> None:
        recipe = load_artifact_profile("python-uniffi-native")
        wheel_builder.validate_python_native_recipe(recipe)
        target = "aarch64-apple-darwin"
        production = cargo_build_args(recipe, target=target)
        metadata_library = wheel_builder.production_metadata_library_path(recipe, target)
        cdylib = wheel_builder.production_cdylib_path(recipe, target)
        generator = wheel_builder.python_generator_args(
            recipe,
            metadata_library,
            cdylib,
            Path("/tmp/merman-python-package"),
        )
        self.assertEqual(
            production[production.index("--features") + 1], recipe.feature_argument
        )
        self.assertNotIn("binding-generation", production)
        self.assertEqual(production[production.index("--target") + 1], target)
        self.assertEqual(
            cdylib,
            wheel_builder.REPO_ROOT
            / "target"
            / target
            / recipe.cargo_profile
            / "libmerman_uniffi.dylib",
        )
        self.assertEqual(
            metadata_library,
            wheel_builder.REPO_ROOT
            / "target"
            / target
            / recipe.cargo_profile
            / "libmerman_uniffi.rlib",
        )
        self.assertEqual(
            generator[generator.index("--features") + 1],
            "binding-generation",
        )
        self.assertEqual(
            generator[generator.index("--metadata-library") + 1],
            str(metadata_library),
        )
        self.assertEqual(
            generator[generator.index("--cdylib") + 1],
            str(cdylib),
        )
        self.assertNotIn("--profile", generator)

    def test_python_builder_validates_identity_without_duplicating_capabilities(self) -> None:
        recipe = replace(
            load_artifact_profile("python-uniffi-native"),
            features=("svg",),
            cargo_profile="release",
            default_features=True,
            crate_types=("cdylib", "rlib"),
        )

        wheel_builder.validate_python_native_recipe(recipe)

    def test_python_builder_requires_generated_support_files_to_be_tracked(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            repo_root = Path(temp_dir)
            package_dir = repo_root / "platforms" / "python" / "merman"
            for relative in wheel_builder.PYTHON_GENERATED_SUPPORT_FILES:
                generated = package_dir / relative
                generated.parent.mkdir(parents=True, exist_ok=True)
                generated.write_text("generated\n", encoding="utf-8")
            staged = repo_root / "staged"
            shutil.copytree(package_dir, staged)
            with (
                mock.patch.object(wheel_builder, "REPO_ROOT", repo_root),
                mock.patch.object(wheel_builder, "run") as run,
            ):
                wheel_builder.verify_generated_python_support_files(package_dir, staged)

        self.assertEqual(
            run.call_args.args[0],
            [
                "git",
                "ls-files",
                "--error-unmatch",
                "--",
                *(
                    f"platforms/python/merman/{relative}"
                    for relative in wheel_builder.PYTHON_GENERATED_SUPPORT_FILES
                ),
            ],
        )

    def test_python_builder_rejects_stale_generated_support_files(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            source = root / "source"
            staged = root / "staged"
            for relative in wheel_builder.PYTHON_GENERATED_SUPPORT_FILES:
                source_file = source / relative
                staged_file = staged / relative
                source_file.parent.mkdir(parents=True, exist_ok=True)
                staged_file.parent.mkdir(parents=True, exist_ok=True)
                source_file.write_text("current\n", encoding="utf-8")
                staged_file.write_text("current\n", encoding="utf-8")

            (staged / wheel_builder.PYTHON_GENERATED_SUPPORT_FILES[0]).write_text(
                "regenerated\n",
                encoding="utf-8",
            )

            with (
                mock.patch.object(wheel_builder, "REPO_ROOT", root),
                mock.patch.object(wheel_builder, "run"),
                self.assertRaisesRegex(RuntimeError, "stale generated Python support file"),
            ):
                wheel_builder.verify_generated_python_support_files(source, staged)

    def test_python_builder_generates_and_packages_only_from_staging(self) -> None:
        recipe = load_artifact_profile("python-uniffi-native")
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            source = root / "source-package"
            staged = root / "staged-package"
            wheel_dir = root / "wheels"
            metadata_library = root / "libmerman_uniffi.rlib"
            cdylib = root / "libmerman_uniffi.so"
            wheel = wheel_dir / "merman-0.0.0-py3-none-linux_x86_64.whl"
            source.mkdir()
            example = source / "examples" / "smoke.py"
            example.parent.mkdir()
            example.write_text("print('smoke')\n", encoding="utf-8")
            staged.mkdir()
            metadata_library.write_bytes(b"metadata")
            cdylib.write_bytes(b"native")
            venv_python = root / "venv" / "python"
            args = SimpleNamespace(
                package_dir=str(source),
                wheel_dir=str(wheel_dir),
                python=sys.executable,
                run_smoke=True,
            )

            with (
                mock.patch.object(wheel_builder, "REPO_ROOT", root),
                mock.patch.object(wheel_builder, "parse_args", return_value=args),
                mock.patch.object(
                    wheel_builder,
                    "load_artifact_profile",
                    return_value=recipe,
                ),
                mock.patch.object(wheel_builder, "validate_python_native_recipe"),
                mock.patch.object(
                    wheel_builder,
                    "select_python_wheel_target",
                    return_value="x86_64-unknown-linux-gnu",
                ),
                mock.patch.object(
                    wheel_builder,
                    "production_metadata_library_path",
                    return_value=metadata_library,
                ),
                mock.patch.object(wheel_builder, "production_cdylib_path", return_value=cdylib),
                mock.patch.object(
                    wheel_builder,
                    "staged_python_package",
                    return_value=nullcontext(staged),
                ),
                mock.patch.object(
                    wheel_builder,
                    "verify_generated_python_support_files",
                ),
                mock.patch.object(wheel_builder, "install_target_report"),
                mock.patch.object(wheel_builder, "newest_wheel", return_value=wheel),
                mock.patch.object(wheel_builder, "require_native_platform_wheel"),
                mock.patch.object(wheel_builder, "verify_wheel_license_report"),
                mock.patch.object(
                    wheel_builder,
                    "venv_python",
                    return_value=venv_python,
                ),
                mock.patch.object(wheel_builder, "run") as run,
            ):
                self.assertEqual(wheel_builder.main(), 0)

        generator_command = run.call_args_list[1].args[0]
        wheel_command = run.call_args_list[2].args[0]
        build_command = run.call_args_list[0].args[0]
        self.assertEqual(
            build_command[build_command.index("--target") + 1],
            "x86_64-unknown-linux-gnu",
        )
        self.assertIn(str(staged), generator_command)
        self.assertNotIn(str(source), generator_command)
        self.assertEqual(
            generator_command[generator_command.index("--metadata-library") + 1],
            str(metadata_library),
        )
        self.assertEqual(
            generator_command[generator_command.index("--cdylib") + 1],
            str(cdylib),
        )
        self.assertIn(str(staged), wheel_command)
        self.assertNotIn(str(source), wheel_command)
        self.assertEqual(
            run.call_args_list[-1].args[0],
            [str(venv_python), str(example.resolve())],
        )

    def test_zigbuild_uses_the_same_exact_recipe_projection(self) -> None:
        recipe = load_artifact_profile("flutter-desktop-native")

        command = cargo_build_args(
            recipe,
            locked=True,
            target="x86_64-unknown-linux-gnu",
            build_tool="cargo-zigbuild",
        )

        self.assertEqual(command[:2], ["cargo", "zigbuild"])
        self.assertEqual(
            command[command.index("--features") + 1],
            recipe.feature_argument,
        )
        self.assertEqual(
            command[command.index("--target") + 1],
            "x86_64-unknown-linux-gnu",
        )

    def test_uniffi_binding_generation_is_generator_only(self) -> None:
        repo_root = Path(__file__).resolve().parents[1]
        with (repo_root / "Cargo.toml").open("rb") as handle:
            workspace = tomllib.load(handle)
        with (repo_root / "crates/merman-uniffi/Cargo.toml").open("rb") as handle:
            crate = tomllib.load(handle)

        self.assertFalse(
            workspace["workspace"]["dependencies"]["uniffi"]["default-features"]
        )
        self.assertFalse(crate["dependencies"]["uniffi"]["default-features"])
        self.assertEqual(
            crate["features"]["binding-generation"],
            ["uniffi/bindgen", "uniffi/cargo-metadata"],
        )
        self.assertEqual(
            {example["name"]: example["required-features"] for example in crate["example"]},
            {
                "generate_python_package": ["binding-generation"],
                "generate_swift_bindings": ["binding-generation"],
            },
        )
        for profile_id in ("apple-uniffi-native", "python-uniffi-native"):
            with self.subTest(profile_id=profile_id):
                self.assertNotIn(
                    "binding-generation", load_artifact_profile(profile_id).features
                )

    def test_rejects_duplicate_or_unsorted_features(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            descriptor = Path(temp_dir) / "profiles.json"
            descriptor.write_text(
                json.dumps(
                    {
                        "schema_version": 1,
                        "profiles": [
                            {
                                "id": "native",
                                "semantic_target": "native",
                                "cargo": {
                                    "package": "merman-ffi",
                                    "manifest": "crates/merman-ffi/Cargo.toml",
                                    "profile": "release",
                                    "default_features": False,
                                    "features": ["svg", "analysis", "svg"],
                                    "target": {
                                        "name": "merman_ffi",
                                        "kinds": ["cdylib"],
                                        "crate_types": ["cdylib"],
                                        "required_features": [],
                                    },
                                    "build_target": {"kind": "host"},
                                },
                            }
                        ]
                    }
                ),
                encoding="utf-8",
            )
            with self.assertRaisesRegex(RuntimeError, "sorted and unique"):
                load_artifact_profile("native", descriptor)

    def test_rejects_recipes_without_a_complete_target_contract(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            descriptor = Path(temp_dir) / "profiles.json"
            descriptor.write_text(
                json.dumps(
                    {
                        "schema_version": 1,
                        "profiles": [
                            {
                                "id": "native",
                                "semantic_target": "native",
                                "cargo": {
                                    "package": "merman-ffi",
                                    "manifest": "crates/merman-ffi/Cargo.toml",
                                    "profile": "release",
                                    "default_features": False,
                                    "features": ["analysis"],
                                },
                            }
                        ]
                    }
                ),
                encoding="utf-8",
            )
            with self.assertRaisesRegex(RuntimeError, "target"):
                load_artifact_profile("native", descriptor)


if __name__ == "__main__":
    unittest.main()
