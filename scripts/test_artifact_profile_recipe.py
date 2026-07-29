#!/usr/bin/env python3
"""Tests for exact artifact recipe projection."""

from __future__ import annotations

from contextlib import nullcontext
from dataclasses import replace
import importlib.util
import json
import os
from pathlib import Path
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
    cargo_run_example_args,
    load_artifact_profile,
)
from github_workflow_contract import load_workflow_contract, workflow_job, workflow_step


WHEEL_BUILDER_PATH = Path(__file__).resolve().parent / "build-python-uniffi-wheel.py"
WHEEL_BUILDER_SPEC = importlib.util.spec_from_file_location(
    "build_python_uniffi_wheel", WHEEL_BUILDER_PATH
)
assert WHEEL_BUILDER_SPEC is not None
wheel_builder = importlib.util.module_from_spec(WHEEL_BUILDER_SPEC)
assert WHEEL_BUILDER_SPEC.loader is not None
WHEEL_BUILDER_SPEC.loader.exec_module(wheel_builder)


class ArtifactProfileRecipeTests(unittest.TestCase):
    def test_python_wheel_smoke_uses_exact_profile_capabilities_and_outputs(self) -> None:
        script = wheel_builder.python_wheel_smoke_script("python-uniffi-native")

        expected = json.loads(
            (artifact_profile_recipe.DEFAULT_DESCRIPTOR).read_text(encoding="utf-8")
        )
        profile = next(
            item for item in expected["profiles"] if item["id"] == "python-uniffi-native"
        )
        self.assertIn(
            f"EXPECTED_CAPABILITY_IDS = {profile['expected']['capabilities']!r}",
            script,
        )
        self.assertIn(
            f"EXPECTED_OUTPUT_IDS = {profile['expected']['outputs']!r}",
            script,
        )
        self.assertIn(
            "EXPECTED_OPERATION_IDS = ['analysis-facts-json', 'analysis-json', "
            "'ascii', 'document-analysis-facts-json', 'document-analysis-json', "
            "'jpeg', 'layout-json', 'pdf', 'png', 'semantic-json', 'svg', "
            "'svg-plan-json', 'validation-json']",
            script,
        )
        self.assertNotIn("required_capabilities", script)
        self.assertIn("assert_shared_semantic_operation_fixtures(engine)", script)

    def test_python_wheel_smoke_receives_the_shared_fixture_path(self) -> None:
        environment = wheel_builder.wheel_smoke_environment()

        self.assertEqual(
            environment["MERMAN_SEMANTIC_OPERATION_FIXTURES"],
            str(wheel_builder.SEMANTIC_OPERATION_FIXTURES),
        )
        self.assertTrue(wheel_builder.SEMANTIC_OPERATION_FIXTURES.is_file())

    def test_native_sdk_profile_owns_the_release_optimization_policy(self) -> None:
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

    def test_committed_native_recipes_have_owner_specific_structure(self) -> None:
        expected_packages = {
            "android-native": "merman-android-jni",
            "apple-uniffi-native": "merman-uniffi",
            "c-abi-native": "merman-ffi",
            "flutter-android-native": "merman-ffi",
            "flutter-desktop-native": "merman-ffi",
            "flutter-ios-native": "merman-ffi",
            "python-uniffi-native": "merman-uniffi",
        }
        for profile_id, package in expected_packages.items():
            with self.subTest(profile_id=profile_id):
                recipe = load_artifact_profile(profile_id)
                self.assertEqual(recipe.package, package)
                self.assertEqual(recipe.cargo_profile, "native-sdk")
                self.assertFalse(recipe.default_features)
                self.assertTrue(recipe.features)

    def test_native_sdk_commands_reject_profile_environment_overrides(self) -> None:
        recipe = load_artifact_profile("c-abi-native")
        override = {"CARGO_PROFILE_NATIVE_SDK_OPT_LEVEL": "z"}

        with (
            mock.patch.dict(os.environ, override, clear=False),
            self.assertRaisesRegex(
                RuntimeError,
                "CARGO_PROFILE_NATIVE_SDK_OPT_LEVEL",
            ),
        ):
            cargo_build_args(recipe)

        with (
            mock.patch.dict(os.environ, override, clear=False),
            self.assertRaisesRegex(
                RuntimeError,
                "CARGO_PROFILE_NATIVE_SDK_OPT_LEVEL",
            ),
        ):
            cargo_run_example_args(recipe, "generate_python_package")

    def test_release_workflows_do_not_override_native_optimization_policy(self) -> None:
        repo_root = Path(__file__).resolve().parents[1]
        forbidden = (
            "CARGO_PROFILE_RELEASE_OPT_LEVEL",
            "CARGO_PROFILE_RELEASE_CODEGEN_UNITS",
            "CARGO_PROFILE_RELEASE_STRIP",
            "CARGO_PROFILE_NATIVE_SDK_",
        )
        for relative_path in (
            ".github/workflows/release-flutter.yml",
            ".github/workflows/release-preflight.yml",
        ):
            text = (repo_root / relative_path).read_text(encoding="utf-8")
            for variable in forbidden:
                with self.subTest(path=relative_path, variable=variable):
                    self.assertNotIn(variable, text)

    def test_flutter_recipes_own_exact_cross_platform_target_sets(self) -> None:
        expected_targets = {
            "flutter-android-native": (
                "aarch64-linux-android",
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
                self.assertEqual(recipe.cargo_profile, "native-sdk")
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
        self.assertEqual(flutter.features, c_abi.features)

    def test_native_shell_consumers_validate_the_committed_recipe_at_runtime(self) -> None:
        repo_root = Path(__file__).resolve().parents[1]
        environment = os.environ.copy()
        environment["MERMAN_CHECK_RECIPE_ONLY"] = "true"
        for relative_path in (
            "scripts/build-apple-xcframework.sh",
            "platforms/flutter/build-ios.sh",
            "platforms/flutter/build-desktop.sh",
        ):
            with self.subTest(path=relative_path):
                subprocess.run(
                    ["bash", str(repo_root / relative_path)],
                    cwd=repo_root,
                    env=environment,
                    check=True,
                    capture_output=True,
                    text=True,
                )

    def test_native_shell_consumers_delegate_cargo_projection_to_helper(self) -> None:
        repo_root = Path(__file__).resolve().parents[1]
        profiles = {
            "scripts/build-apple-xcframework.sh": "apple-uniffi-native",
            "platforms/flutter/build-ios.sh": "flutter-ios-native",
            "platforms/flutter/build-desktop.sh": "flutter-desktop-native",
        }
        for relative_path, profile_id in profiles.items():
            with self.subTest(path=relative_path):
                source = (repo_root / relative_path).read_text(encoding="utf-8")
                self.assertIn(f'RECIPE_PROFILE="{profile_id}"', source)
                self.assertNotIn("NATIVE_SDK_FEATURES", source)
                self.assertNotIn("cargo build", source)
                self.assertNotIn("cargo zigbuild", source)
                self.assertIn("artifact_profile_recipe.py", source)
                self.assertIn("--build --locked", source)

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
        cdylib = wheel_builder.production_cdylib_path(recipe, target)
        generator = wheel_builder.python_generator_args(
            recipe,
            cdylib,
            Path("/tmp/merman-python-package"),
        )
        self.assertEqual(
            production[production.index("--features") + 1], recipe.feature_argument
        )
        self.assertNotIn("bindgen-smoke", production)
        self.assertEqual(production[production.index("--target") + 1], target)
        self.assertEqual(
            cdylib,
            wheel_builder.REPO_ROOT
            / "target"
            / target
            / recipe.cargo_profile
            / "libmerman_uniffi.dylib",
        )
        expected_generator_features = ",".join(
            sorted((*recipe.features, "bindgen-smoke"))
        )
        self.assertEqual(
            generator[generator.index("--features") + 1],
            expected_generator_features,
        )
        self.assertNotEqual(
            wheel_builder.python_generator_environment()["CARGO_TARGET_DIR"],
            str(wheel_builder.REPO_ROOT / "target"),
        )
        compile(wheel_builder.WHEEL_SMOKE, "<wheel-smoke>", "exec")

    def test_python_builder_validates_identity_without_duplicating_capabilities(self) -> None:
        recipe = replace(
            load_artifact_profile("python-uniffi-native"),
            features=("svg",),
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
            with (
                mock.patch.object(wheel_builder, "REPO_ROOT", repo_root),
                mock.patch.object(wheel_builder, "run") as run,
            ):
                wheel_builder.require_tracked_python_support_files(package_dir)

        self.assertEqual(
            [call.args[0] for call in run.call_args_list],
            [
                [
                    "git",
                    "ls-files",
                    "--error-unmatch",
                    "--",
                    f"platforms/python/merman/{relative}",
                ]
                for relative in wheel_builder.PYTHON_GENERATED_SUPPORT_FILES
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

            with self.assertRaisesRegex(RuntimeError, "stale generated Python support file"):
                wheel_builder.verify_staged_python_support_files(source, staged)

    def test_python_builder_generates_and_packages_only_from_staging(self) -> None:
        recipe = load_artifact_profile("python-uniffi-native")
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            source = root / "source-package"
            staged = root / "staged-package"
            wheel_dir = root / "wheels"
            cdylib = root / "libmerman_uniffi.so"
            wheel = wheel_dir / "merman-0.0.0-py3-none-linux_x86_64.whl"
            source.mkdir()
            example = source / "examples" / "smoke.py"
            example.parent.mkdir()
            example.write_text("print('smoke')\n", encoding="utf-8")
            staged.mkdir()
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
                mock.patch.object(wheel_builder, "production_cdylib_path", return_value=cdylib),
                mock.patch.object(
                    wheel_builder,
                    "staged_python_package",
                    return_value=nullcontext(staged),
                ),
                mock.patch.object(
                    wheel_builder,
                    "require_tracked_python_support_files",
                ),
                mock.patch.object(
                    wheel_builder,
                    "verify_staged_python_support_files",
                ),
                mock.patch.object(wheel_builder, "install_target_report"),
                mock.patch.object(wheel_builder, "newest_wheel", return_value=wheel),
                mock.patch.object(wheel_builder, "require_platform_wheel"),
                mock.patch.object(wheel_builder, "require_native_platlib_layout"),
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
        self.assertIn(str(staged), wheel_command)
        self.assertNotIn(str(source), wheel_command)
        self.assertEqual(
            run.call_args_list[-1].args[0],
            [str(venv_python), str(example.resolve())],
        )

    def test_python_ci_and_release_do_not_depend_on_generated_source_bindings(
        self,
    ) -> None:
        repo_root = Path(__file__).resolve().parents[1]
        for relative in (
            ".github/workflows/ci.yml",
            ".github/workflows/release-python.yml",
        ):
            with self.subTest(workflow=relative):
                workflow = (repo_root / relative).read_text(encoding="utf-8")
                self.assertNotIn(
                    "PYTHONPATH=platforms/python/merman/src",
                    workflow,
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

    def test_example_projection_adds_only_maintenance_features(self) -> None:
        recipe = load_artifact_profile("python-uniffi-native")

        command = cargo_run_example_args(
            recipe,
            "generate_python_package",
            locked=True,
            extra_features=("bindgen-smoke",),
            example_args=("--package-dir", "/tmp/package"),
        )

        self.assertEqual(command[:2], ["cargo", "run"])
        self.assertEqual(
            command[command.index("--features") + 1],
            ",".join(sorted((*recipe.features, "bindgen-smoke"))),
        )
        self.assertEqual(command[-3:], ["--", "--package-dir", "/tmp/package"])

    def test_example_cli_executes_the_projected_command(self) -> None:
        recipe = load_artifact_profile("python-uniffi-native")
        argv = [
            "artifact_profile_recipe.py",
            recipe.profile_id,
            "--run-example",
            "generate_python_package",
            "--locked",
            "--extra-feature",
            "bindgen-smoke",
            "--example-argument=--package-dir",
            "--example-argument=/tmp/package",
        ]
        with (
            mock.patch.object(sys, "argv", argv),
            mock.patch.object(artifact_profile_recipe.subprocess, "run") as run,
        ):
            self.assertEqual(artifact_profile_recipe.main(), 0)

        command = run.call_args.args[0]
        self.assertEqual(command[:2], ["cargo", "run"])
        self.assertEqual(command[-3:], ["--", "--package-dir", "/tmp/package"])
        self.assertEqual(run.call_args.kwargs["cwd"], artifact_profile_recipe.REPO_ROOT)
        self.assertTrue(run.call_args.kwargs["check"])

    def test_ci_runs_the_formal_strict_feature_matrix_command(self) -> None:
        repo_root = Path(__file__).resolve().parents[1]
        workflow = load_workflow_contract(repo_root / ".github/workflows/ci.yml")
        job = workflow_job(workflow, "build-test")
        step = workflow_step(
            job,
            name="Verify generated architecture contracts",
        )
        commands = [line.strip() for line in step["run"].splitlines() if line.strip()]
        self.assertIn(
            "cargo run --locked -p xtask -- verify-feature-matrix --strict",
            commands,
        )
        toolchain = workflow_step(job, name="Install Rust toolchain")
        self.assertEqual(
            set(toolchain["with"]["targets"].split(",")),
            {"aarch64-linux-android", "wasm32-unknown-unknown"},
        )

    def test_cli_profiles_use_binary_process_contracts(self) -> None:
        repo_root = Path(__file__).resolve().parents[1]
        workflow = load_workflow_contract(repo_root / ".github/workflows/ci.yml")
        owner_step = workflow_step(
            workflow_job(workflow, "build-test"),
            name="Test exact artifact owner APIs",
        )
        self.assertNotIn("run_owner_test cli-analysis", owner_step["run"])

        process_step = workflow_step(
            workflow_job(workflow, "cli-contracts"),
            name="Test exact CLI feature process matrix",
        )
        self.assertEqual(
            process_step["run"],
            "python3 scripts/verify_cli_process_matrix.py --locked",
        )
        validation_step = workflow_step(
            workflow_job(workflow, "cli-contracts"),
            name="Validate CLI distribution assets",
        )
        self.assertEqual(
            validation_step["run"],
            "python3 scripts/verify_cli_assets.py "
            "--require bash,zsh,fish,elvish,mandoc",
        )

    def test_cli_artifact_profiles_build_on_every_descriptor_host(self) -> None:
        repo_root = Path(__file__).resolve().parents[1]
        workflow = load_workflow_contract(repo_root / ".github/workflows/ci.yml")
        job = workflow_job(workflow, "cli-artifact-profiles")
        matrix = job["matrix_include"]
        self.assertEqual(
            {(row["os"], row["target"]) for row in matrix},
            {
                ("ubuntu-24.04", "x86_64-unknown-linux-gnu"),
                ("macos-15", "aarch64-apple-darwin"),
                ("macos-15-intel", "x86_64-apple-darwin"),
                ("windows-2025-vs2026", "x86_64-pc-windows-msvc"),
            },
        )
        step = workflow_step(job, name="Build exact CLI artifact profiles")
        self.assertEqual(
            [line.strip() for line in step["run"].splitlines() if line.strip()],
            [
                "python3 scripts/artifact_profile_recipe.py "
                "cli-analysis --build-host --locked",
                "python3 scripts/artifact_profile_recipe.py "
                "cli-release --build-host --locked",
            ],
        )
        powershell = workflow_step(job, name="Validate PowerShell completion")
        self.assertEqual(powershell["if"], "runner.os == 'Windows'")
        self.assertEqual(
            powershell["run"],
            "python3 scripts/verify_cli_assets.py --require powershell",
        )

    def test_homebrew_checks_binary_and_version_gated_asset_contracts(self) -> None:
        repo_root = Path(__file__).resolve().parents[1]
        workflow = load_workflow_contract(repo_root / ".github/workflows/homebrew.yml")
        job = workflow_job(workflow, "formula-health")
        self.assertEqual(
            {row["os"] for row in job["matrix_include"]},
            {"macos-15", "ubuntu-24.04"},
        )
        self.assertEqual(job["env"]["SUPPORT_ASSETS_SINCE"], "0.8.0")
        step = workflow_step(
            job,
            name="Smoke installed merman-cli",
        )
        command = step["run"]
        self.assertEqual(
            step["env"]["FORMULA_VERSION"],
            "${{ steps.metadata.outputs.version }}",
        )
        self.assertNotIn("brew info", command)
        self.assertNotIn("formula_version", command)
        self.assertIn("merman-cli --version", command)
        self.assertIn("merman-cli render", command)
        support_assets = workflow_step(
            job,
            name="Verify version-gated support assets",
        )
        self.assertIn("scripts/verify_homebrew_install.py", support_assets["run"])
        linkage = workflow_step(job, name="Audit installed formula")
        self.assertIn("brew linkage --test merman-cli", linkage["run"])

    def test_c_ffi_ci_smoke_resolves_the_recipe_owned_output_directory(self) -> None:
        repo_root = Path(__file__).resolve().parents[1]
        workflow = load_workflow_contract(repo_root / ".github/workflows/ci.yml")
        step = workflow_step(
            workflow_job(workflow, "c-ffi-example"),
            name="Build and run C example",
        )
        command = step["run"]
        self.assertIn(
            "artifact_profile_recipe.py c-abi-native --field profile",
            command,
        )
        self.assertNotIn("target/release", command)

    def test_uniffi_bindgen_is_generator_only(self) -> None:
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
            crate["features"]["bindgen-smoke"],
            ["uniffi/bindgen", "uniffi/cargo-metadata"],
        )
        self.assertEqual(
            {example["name"]: example["required-features"] for example in crate["example"]},
            {
                "generate_python_package": ["bindgen-smoke"],
                "generate_swift_bindings": ["bindgen-smoke"],
            },
        )
        for profile_id in ("apple-uniffi-native", "python-uniffi-native"):
            with self.subTest(profile_id=profile_id):
                self.assertNotIn("bindgen-smoke", load_artifact_profile(profile_id).features)

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
