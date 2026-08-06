#!/usr/bin/env python3
"""Tests for fixed native FFI artifact provenance and size budgets."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest import mock


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from ffi_contract_dependency_probes import (  # noqa: E402
    BASELINE_COMMIT,
    BASELINE_TREE,
    load_dependency_probes,
)
from ffi_contract_baseline_contract import (  # noqa: E402
    BASELINE_INPUT_PATHS,
    BASELINE_LOCK_SCHEMA_VERSION,
    file_sha256,
)
from verify_native_artifact_sizes import (  # noqa: E402
    AppleToolchain,
    NativeBuildBindings,
    NativeArtifactSizeError,
    NativeArtifactSizeApproval,
    ToolIdentity,
    artifact_growth_budget,
    capture_native_artifact_measurements,
    compare_native_artifact_sizes,
    evaluate_native_artifact_sizes,
    load_native_artifact_baseline,
    load_native_artifact_profiles,
    load_native_artifact_size_approvals,
    native_artifact_report,
    native_build_command,
    native_profile_configuration,
    reject_cargo_configuration,
    reject_native_measurement_environment,
    select_compiler_artifacts,
)
import capture_ffi_contract_baseline as baseline_capture  # noqa: E402
import verify_native_artifact_sizes as artifact_verifier  # noqa: E402
from verify_artifact_dependency_closures import (  # noqa: E402
    BASELINE_SCHEMA_VERSION as DEPENDENCY_BASELINE_SCHEMA_VERSION,
)


class NativeArtifactRecipeTests(unittest.TestCase):
    def test_profiles_are_exact_semantic_and_full_c_abi_recipes(self) -> None:
        profiles = load_native_artifact_profiles()
        self.assertEqual(
            tuple(profile.label for profile in profiles),
            ("ffi-full-native", "ffi-semantic"),
        )
        self.assertTrue(profiles[0].recipe.features)
        self.assertEqual(profiles[1].recipe.features, ())
        for profile in profiles:
            self.assertEqual(profile.recipe.package, "merman-ffi")
            self.assertEqual(profile.recipe.cargo_profile, "native-sdk")
            self.assertFalse(profile.recipe.default_features)

    def test_build_command_requests_json_and_an_exact_target_directory(self) -> None:
        profile = load_native_artifact_profiles()[1]
        target_dir = SCRIPT_DIR.parent / "target" / "fixture-artifacts"
        command = native_build_command(
            profile,
            repo_root=SCRIPT_DIR.parent,
            target_dir=target_dir,
            target="aarch64-apple-darwin",
            bindings=NativeBuildBindings(
                cargo="/toolchain/bin/cargo",
                rustc="/toolchain/bin/rustc",
                linker_driver="/Xcode/bin/clang",
                developer_dir="/Xcode/Developer",
                sdk="/Xcode/SDKs/MacOSX.sdk",
            ),
        )
        self.assertEqual(command[:2], ["/toolchain/bin/cargo", "build"])
        self.assertEqual(command[command.index("--package") + 1], "merman-ffi")
        self.assertIn("--locked", command)
        self.assertIn("--no-default-features", command)
        self.assertNotIn("--features", command)
        self.assertEqual(
            command[command.index("--message-format") + 1],
            "json-render-diagnostics",
        )
        self.assertEqual(command[command.index("--target-dir") + 1], str(target_dir))
        self.assertNotIn("--target", command)
        self.assertEqual(
            command[command.index("--config") + 1],
            'build.rustc="/toolchain/bin/rustc"',
        )
        self.assertIn(
            'target.aarch64-apple-darwin.linker="/Xcode/bin/clang"',
            command,
        )
        self.assertIn(
            'target.aarch64-apple-darwin.rustflags=["-C","link-arg=-isysroot",'
            '"-C","link-arg=/Xcode/SDKs/MacOSX.sdk"]',
            command,
        )
        self.assertEqual(
            native_profile_configuration()["native-sdk"]["strip"],
            "debuginfo",
        )

    def test_measurement_rejects_environment_overrides(self) -> None:
        for environment in (
            {"RUSTFLAGS": "-C opt-level=0"},
            {"RUSTC": "/tmp/rustc"},
            {"CARGO_BUILD_RUSTFLAGS": "-C opt-level=0"},
            {"CARGO_PROFILE_NATIVE_SDK_OPT_LEVEL": "0"},
            {"CARGO_BUILD_TARGET": "x86_64-apple-darwin"},
            {"CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER": "/tmp/clang"},
            {"CC_aarch64_apple_darwin": "/tmp/clang"},
            {"MACOSX_DEPLOYMENT_TARGET": "14.0"},
            {"CARGO_INCREMENTAL": "1"},
            {"LIBRARY_PATH": "/tmp/lib"},
            {"CPATH": "/tmp/include"},
            {"COMPILER_PATH": "/tmp/bin"},
            {"ZERO_AR_DATE": "0"},
            {"DYLD_INSERT_LIBRARIES": "/tmp/inject.dylib"},
            {"ARFLAGS": "-x"},
            {"RANLIB": "/tmp/ranlib"},
            {"STRIP": "/tmp/strip"},
            {"PKG_CONFIG_PATH": "/tmp/pkgconfig"},
        ):
            with self.subTest(environment=environment), self.assertRaisesRegex(
                NativeArtifactSizeError,
                "environment overrides",
            ):
                reject_native_measurement_environment(environment)

    def test_measurement_rejects_ancestor_and_user_cargo_configuration(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            repo = root / "parent" / "repo"
            repo.mkdir(parents=True)
            user_home = root / "home"

            ancestor_config = root / "parent" / ".cargo" / "config.toml"
            ancestor_config.parent.mkdir()
            ancestor_config.write_text("[build]\n", encoding="utf-8")
            with self.assertRaisesRegex(
                NativeArtifactSizeError,
                "Cargo configuration",
            ):
                reject_cargo_configuration(
                    repo,
                    environment={},
                    user_home=user_home,
                )

            ancestor_config.unlink()
            user_config = user_home / ".cargo" / "config"
            user_config.parent.mkdir(parents=True)
            user_config.write_text("[build]\n", encoding="utf-8")
            with self.assertRaisesRegex(
                NativeArtifactSizeError,
                "Cargo configuration",
            ):
                reject_cargo_configuration(
                    repo,
                    environment={},
                    user_home=user_home,
                )

            user_config.unlink()
            configured_home = root / "configured-cargo-home"
            configured_config = configured_home / "config.toml"
            configured_home.mkdir()
            configured_config.write_text("[build]\n", encoding="utf-8")
            with self.assertRaisesRegex(
                NativeArtifactSizeError,
                "Cargo configuration",
            ):
                reject_cargo_configuration(
                    repo,
                    environment={"CARGO_HOME": str(configured_home)},
                    user_home=user_home,
                )


class CargoArtifactSelectionTests(unittest.TestCase):
    def test_only_current_matching_compiler_artifacts_are_selected(self) -> None:
        profile = load_native_artifact_profiles()[1]
        with tempfile.TemporaryDirectory() as temporary_directory:
            repo = SCRIPT_DIR.parent
            target = Path(temporary_directory) / "target"
            artifact_dir = target / "native-sdk"
            artifact_dir.mkdir(parents=True)
            dylib = artifact_dir / "libmerman_ffi.dylib"
            staticlib = artifact_dir / "libmerman_ffi.a"
            dylib.write_bytes(b"dylib")
            staticlib.write_bytes(b"archive")
            output = "\n".join(
                (
                    json.dumps({"reason": "compiler-message"}),
                    json.dumps(
                        {
                            "reason": "compiler-artifact",
                            "manifest_path": str(repo / profile.recipe.manifest),
                            "target": {"name": "other"},
                            "filenames": [str(target / "stale.dylib")],
                        }
                    ),
                    json.dumps(self._compiler_event(profile, repo, dylib, staticlib)),
                )
            )

            selected = select_compiler_artifacts(
                output,
                profile=profile,
                repo_root=repo,
                target_dir=target,
                host_target="aarch64-apple-darwin",
            )
            self.assertEqual(
                selected,
                (("cdylib", dylib), ("staticlib", staticlib)),
            )

    def test_package_id_accepts_only_the_cargo_lexical_symlink_path(self) -> None:
        profile = load_native_artifact_profiles()[1]
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            real_root = root / "real-root"
            alias_root = root / "alias-root"
            other_alias_root = root / "other-alias-root"
            real_repo = real_root / "repo"
            alias_repo = alias_root / "repo"
            other_alias_repo = other_alias_root / "repo"
            manifest = real_repo / profile.recipe.manifest
            manifest.parent.mkdir(parents=True)
            (real_repo / "Cargo.toml").write_bytes(
                (SCRIPT_DIR.parent / "Cargo.toml").read_bytes()
            )
            manifest.write_bytes(
                (SCRIPT_DIR.parent / profile.recipe.manifest).read_bytes()
            )
            source = manifest.parent / "src" / "lib.rs"
            source.parent.mkdir()
            source.write_text("", encoding="utf-8")
            alias_root.symlink_to(real_root, target_is_directory=True)
            other_alias_root.symlink_to(real_root, target_is_directory=True)

            target = root / "target"
            artifact_dir = target / "native-sdk"
            artifact_dir.mkdir(parents=True)
            dylib = artifact_dir / "libmerman_ffi.dylib"
            staticlib = artifact_dir / "libmerman_ffi.a"
            dylib.write_bytes(b"dylib")
            staticlib.write_bytes(b"archive")

            event = self._compiler_event(profile, alias_repo, dylib, staticlib)
            version = artifact_verifier._workspace_package_version(
                alias_repo / profile.recipe.manifest,
                alias_repo / "Cargo.toml",
            )
            event["package_id"] = (
                f"path+{(alias_repo / profile.recipe.manifest).parent.absolute().as_uri()}#"
                f"{profile.recipe.package}@{version}"
            )

            self.assertEqual(
                select_compiler_artifacts(
                    json.dumps(event),
                    profile=profile,
                    repo_root=alias_repo,
                    target_dir=target,
                    host_target="aarch64-apple-darwin",
                ),
                (("cdylib", dylib), ("staticlib", staticlib)),
            )

            other_alias_package_id = (
                f"path+{(other_alias_repo / profile.recipe.manifest).parent.absolute().as_uri()}#"
                f"{profile.recipe.package}@{version}"
            )
            self.assertFalse(
                artifact_verifier._matches_workspace_package_id(
                    other_alias_package_id,
                    profile.recipe,
                    alias_repo,
                )
            )

    def test_artifact_outside_exact_target_directory_is_rejected(self) -> None:
        profile = load_native_artifact_profiles()[1]
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            repo = SCRIPT_DIR.parent
            target = root / "target"
            outside = root / "outside"
            artifact_dir = target / "native-sdk"
            artifact_dir.mkdir(parents=True)
            outside.mkdir()
            dylib = outside / "libmerman_ffi.dylib"
            staticlib = artifact_dir / "libmerman_ffi.a"
            dylib.write_bytes(b"dylib")
            staticlib.write_bytes(b"archive")
            output = json.dumps(self._compiler_event(profile, repo, dylib, staticlib))
            with self.assertRaisesRegex(NativeArtifactSizeError, "escaped"):
                select_compiler_artifacts(
                    output,
                    profile=profile,
                    repo_root=repo,
                    target_dir=target,
                    host_target="aarch64-apple-darwin",
                )

    def test_relative_compiler_artifact_path_is_rejected(self) -> None:
        profile = load_native_artifact_profiles()[1]
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            repo = SCRIPT_DIR.parent
            target = root / "target"
            target.mkdir()
            output = json.dumps(
                self._compiler_event(
                    profile,
                    repo,
                    Path("target/libmerman_ffi.dylib"),
                    Path("target/libmerman_ffi.a"),
                )
            )
            with self.assertRaisesRegex(NativeArtifactSizeError, "absolute path"):
                select_compiler_artifacts(
                    output,
                    profile=profile,
                    repo_root=repo,
                    target_dir=target,
                    host_target="aarch64-apple-darwin",
                )

    def test_forged_target_contract_and_duplicate_root_events_are_rejected(self) -> None:
        profile = load_native_artifact_profiles()[1]
        with tempfile.TemporaryDirectory() as temporary_directory:
            target = Path(temporary_directory) / "target"
            artifact_dir = target / "native-sdk"
            artifact_dir.mkdir(parents=True)
            dylib = artifact_dir / "libmerman_ffi.dylib"
            staticlib = artifact_dir / "libmerman_ffi.a"
            dylib.write_bytes(b"dylib")
            staticlib.write_bytes(b"archive")
            event = self._compiler_event(
                profile,
                SCRIPT_DIR.parent,
                dylib,
                staticlib,
            )
            forged = json.loads(json.dumps(event))
            forged["target"]["kind"] = ["bin"]
            with self.assertRaisesRegex(NativeArtifactSizeError, "target kind"):
                select_compiler_artifacts(
                    json.dumps(forged),
                    profile=profile,
                    repo_root=SCRIPT_DIR.parent,
                    target_dir=target,
                    host_target="aarch64-apple-darwin",
                )
            with self.assertRaisesRegex(NativeArtifactSizeError, "exactly one root"):
                select_compiler_artifacts(
                    "\n".join((json.dumps(event), json.dumps(event))),
                    profile=profile,
                    repo_root=SCRIPT_DIR.parent,
                    target_dir=target,
                    host_target="aarch64-apple-darwin",
                )

    @staticmethod
    def _compiler_event(
        profile: object,
        repo: Path,
        dylib: Path,
        staticlib: Path,
    ) -> dict[str, object]:
        manifest = repo / profile.recipe.manifest
        version = artifact_verifier._workspace_package_version(
            manifest,
            repo / "Cargo.toml",
        )
        configuration = native_profile_configuration(repo)["native-sdk"]
        return {
            "reason": "compiler-artifact",
            "package_id": (
                f"path+{manifest.parent.resolve().as_uri()}#"
                f"{profile.recipe.package}@{version}"
            ),
            "manifest_path": str(manifest.resolve()),
            "target": {
                "name": profile.recipe.target_name,
                "kind": list(profile.recipe.target_kinds),
                "crate_types": list(profile.recipe.crate_types),
                "src_path": str((manifest.parent / "src" / "lib.rs").resolve()),
            },
            "profile": {
                "opt_level": str(configuration["opt-level"]),
                "debug_assertions": configuration["debug-assertions"],
                "overflow_checks": configuration["overflow-checks"],
                "test": False,
            },
            "filenames": [str(dylib), str(staticlib)],
            "fresh": False,
        }


class NativeArtifactCaptureTests(unittest.TestCase):
    def test_capture_rejects_nonempty_target_directory(self) -> None:
        profile = load_native_artifact_profiles()[1]
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            target = root / "build" / profile.label
            target.mkdir(parents=True)
            (target / "stale").write_text("stale", encoding="utf-8")
            with (
                mock.patch.object(
                    artifact_verifier,
                    "_resolve_apple_toolchain",
                    return_value=self._apple_toolchain(),
                ),
                self.assertRaisesRegex(NativeArtifactSizeError, "must be empty"),
            ):
                capture_native_artifact_measurements(
                    (profile,),
                    repo_root=SCRIPT_DIR.parent,
                    output_root=root,
                    rust_toolchain=self._toolchain(),
                    runner=lambda command, cwd: subprocess.CompletedProcess(
                        command, 1, "", "must not run"
                    ),
                )

    def test_xcrun_cannot_substitute_an_unrelated_successful_program(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            developer = (
                Path(temporary_directory).resolve()
                / "Xcode.app"
                / "Contents"
                / "Developer"
            )
            tools = developer / "Toolchains" / "Fixture.xctoolchain" / "usr" / "bin"
            sdk = (
                developer
                / "Platforms"
                / "MacOSX.platform"
                / "Developer"
                / "SDKs"
                / "MacOSX.sdk"
            )
            tools.mkdir(parents=True)
            sdk.mkdir(parents=True)
            (sdk / "SDKSettings.json").write_text("{}", encoding="utf-8")
            clang = tools / "clang"
            clang.write_bytes(b"clang")
            clang.chmod(0o755)

            def runner(
                command: list[str] | tuple[str, ...],
                cwd: Path,
            ) -> subprocess.CompletedProcess[str]:
                outputs = {
                    ("/usr/bin/xcode-select", "--print-path"): str(developer),
                    ("/usr/bin/xcodebuild", "-version"): "Xcode fixture",
                    ("/usr/bin/xcrun", "--sdk", "macosx", "--show-sdk-path"): str(
                        sdk
                    ),
                    ("/usr/bin/xcrun", "--sdk", "macosx", "--show-sdk-version"): "1.0",
                    (
                        "/toolchain/bin/rustc",
                        "--print",
                        "deployment-target",
                        "--target",
                        "aarch64-apple-darwin",
                    ): "MACOSX_DEPLOYMENT_TARGET=11.0",
                    ("/usr/bin/xcrun", "--sdk", "macosx", "--find", "clang"): str(clang),
                    ("/usr/bin/xcrun", "--sdk", "macosx", "--find", "ld"): "/usr/bin/true",
                    ("/usr/bin/xcrun", "--sdk", "macosx", "--find", "strip"): "/usr/bin/true",
                }
                output = outputs.get(tuple(command))
                return subprocess.CompletedProcess(
                    command,
                    0 if output is not None else 1,
                    (output + "\n") if output is not None else "",
                    "" if output is not None else "unexpected command",
                )

            with self.assertRaisesRegex(NativeArtifactSizeError, "non-canonical"):
                artifact_verifier._resolve_apple_toolchain(
                    "aarch64-apple-darwin",
                    "/toolchain/bin/rustc",
                    SCRIPT_DIR.parent,
                    runner,
                )

    def test_xcrun_versioned_sdk_symlink_is_canonicalized(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            developer = (
                Path(temporary_directory).resolve()
                / "Xcode.app"
                / "Contents"
                / "Developer"
            )
            tools = developer / "Toolchains" / "Fixture.xctoolchain" / "usr" / "bin"
            sdk_root = (
                developer
                / "Platforms"
                / "MacOSX.platform"
                / "Developer"
                / "SDKs"
            )
            tools.mkdir(parents=True)
            sdk_root.mkdir(parents=True)
            canonical_sdk = sdk_root / "MacOSX.sdk"
            canonical_sdk.mkdir()
            (canonical_sdk / "SDKSettings.json").write_text("{}", encoding="utf-8")
            versioned_sdk = sdk_root / "MacOSX1.0.sdk"
            versioned_sdk.symlink_to(canonical_sdk.name)
            clang = tools / "clang"
            linker = tools / "ld"
            strip = tools / "strip"
            for tool in (clang, linker, strip):
                tool.write_bytes(tool.name.encode())
                tool.chmod(0o755)

            outputs = {
                ("/usr/bin/xcode-select", "--print-path"): str(developer),
                ("/usr/bin/xcodebuild", "-version"): "Xcode fixture",
                ("/usr/bin/xcrun", "--sdk", "macosx", "--show-sdk-path"): str(
                    versioned_sdk
                ),
                ("/usr/bin/xcrun", "--sdk", "macosx", "--show-sdk-version"): "1.0",
                (
                    "/toolchain/bin/rustc",
                    "--print",
                    "deployment-target",
                    "--target",
                    "aarch64-apple-darwin",
                ): "MACOSX_DEPLOYMENT_TARGET=11.0",
                ("/usr/bin/xcrun", "--sdk", "macosx", "--find", "clang"): str(clang),
                ("/usr/bin/xcrun", "--sdk", "macosx", "--find", "ld"): str(linker),
                ("/usr/bin/xcrun", "--sdk", "macosx", "--find", "strip"): str(strip),
                (str(clang), "-print-prog-name=ld"): str(linker),
            }

            def runner(
                command: list[str] | tuple[str, ...],
                cwd: Path,
            ) -> subprocess.CompletedProcess[str]:
                output = outputs.get(tuple(command))
                return subprocess.CompletedProcess(
                    command,
                    0 if output is not None else 1,
                    (output + "\n") if output is not None else "",
                    "" if output is not None else "unexpected command",
                )

            toolchain = artifact_verifier._resolve_apple_toolchain(
                "aarch64-apple-darwin",
                "/toolchain/bin/rustc",
                SCRIPT_DIR.parent,
                runner,
            )
            self.assertEqual(toolchain.sdk_path, str(canonical_sdk.resolve()))

    def test_clang_driver_must_resolve_the_recorded_linker(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            developer = (
                Path(temporary_directory).resolve()
                / "Xcode.app"
                / "Contents"
                / "Developer"
            )
            tools = developer / "Toolchains" / "Fixture.xctoolchain" / "usr" / "bin"
            sdk = (
                developer
                / "Platforms"
                / "MacOSX.platform"
                / "Developer"
                / "SDKs"
                / "MacOSX.sdk"
            )
            tools.mkdir(parents=True)
            sdk.mkdir(parents=True)
            (sdk / "SDKSettings.json").write_text("{}", encoding="utf-8")
            clang = tools / "clang"
            linker = tools / "ld"
            strip = tools / "strip"
            other_linker = tools / "ld.other"
            for tool in (clang, linker, strip, other_linker):
                tool.write_bytes(tool.name.encode())
                tool.chmod(0o755)

            outputs = {
                ("/usr/bin/xcode-select", "--print-path"): str(developer),
                ("/usr/bin/xcodebuild", "-version"): "Xcode fixture",
                ("/usr/bin/xcrun", "--sdk", "macosx", "--show-sdk-path"): str(sdk),
                ("/usr/bin/xcrun", "--sdk", "macosx", "--show-sdk-version"): "1.0",
                (
                    "/toolchain/bin/rustc",
                    "--print",
                    "deployment-target",
                    "--target",
                    "aarch64-apple-darwin",
                ): "MACOSX_DEPLOYMENT_TARGET=11.0",
                ("/usr/bin/xcrun", "--sdk", "macosx", "--find", "clang"): str(clang),
                ("/usr/bin/xcrun", "--sdk", "macosx", "--find", "ld"): str(linker),
                ("/usr/bin/xcrun", "--sdk", "macosx", "--find", "strip"): str(strip),
                (str(clang), "-print-prog-name=ld"): str(other_linker),
            }

            def runner(
                command: list[str] | tuple[str, ...],
                cwd: Path,
            ) -> subprocess.CompletedProcess[str]:
                output = outputs.get(tuple(command))
                return subprocess.CompletedProcess(
                    command,
                    0 if output is not None else 1,
                    (output + "\n") if output is not None else "",
                    "" if output is not None else "unexpected command",
                )

            with self.assertRaisesRegex(NativeArtifactSizeError, "different linker"):
                artifact_verifier._resolve_apple_toolchain(
                    "aarch64-apple-darwin",
                    "/toolchain/bin/rustc",
                    SCRIPT_DIR.parent,
                    runner,
                )

    def test_strip_failure_is_fail_closed(self) -> None:
        profile = load_native_artifact_profiles()[1]
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)

            def runner(
                command: list[str] | tuple[str, ...],
                cwd: Path,
            ) -> subprocess.CompletedProcess[str]:
                if len(command) > 1 and command[1] == "build":
                    target = Path(command[command.index("--target-dir") + 1])
                    artifact_dir = target / "native-sdk"
                    artifact_dir.mkdir(parents=True, exist_ok=True)
                    dylib = artifact_dir / "libmerman_ffi.dylib"
                    staticlib = artifact_dir / "libmerman_ffi.a"
                    dylib.write_bytes(b"dylib")
                    staticlib.write_bytes(b"archive")
                    stdout = json.dumps(
                        CargoArtifactSelectionTests._compiler_event(
                            profile,
                            cwd,
                            dylib,
                            staticlib,
                        )
                    )
                    return subprocess.CompletedProcess(command, 0, stdout, "")
                return subprocess.CompletedProcess(command, 1, "", "strip failed")

            with (
                mock.patch.object(
                    artifact_verifier,
                    "_resolve_apple_toolchain",
                    return_value=self._apple_toolchain(),
                ),
                self.assertRaisesRegex(NativeArtifactSizeError, "strip failed"),
            ):
                capture_native_artifact_measurements(
                    (profile,),
                    repo_root=SCRIPT_DIR.parent,
                    output_root=root,
                    rust_toolchain=self._toolchain(),
                    runner=runner,
                )

    def test_report_is_whole_file_locked_and_size_budgets_are_exact(self) -> None:
        profiles = load_native_artifact_profiles()
        measurements = tuple(self._measurement(profile) for profile in profiles)
        report = native_artifact_report(
            measurements,
            repo_root=SCRIPT_DIR.parent,
            toolchain=self._toolchain(),
            source_snapshot_sha256=self._snapshot_sha256(),
        )
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            baseline = root / "native-artifact-sizes.json"
            baseline.write_text(
                json.dumps(report, indent=2, sort_keys=True) + "\n",
                encoding="utf-8",
            )
            digest = "sha256:" + hashlib.sha256(baseline.read_bytes()).hexdigest()
            lock = root / "lock.json"
            lock.write_text(
                json.dumps(
                    {
                        "schema_version": BASELINE_LOCK_SCHEMA_VERSION,
                        "baseline_commit": BASELINE_COMMIT,
                        "baseline_input_sha256": {
                            path.as_posix(): file_sha256(SCRIPT_DIR.parent / path)
                            for path in BASELINE_INPUT_PATHS
                        },
                        "source_snapshot_sha256": self._snapshot_sha256(),
                        "baseline_tree": BASELINE_TREE,
                        "dependency_report_schema_version": (
                            DEPENDENCY_BASELINE_SCHEMA_VERSION
                        ),
                        "dependency_report_file_sha256": "sha256:" + "5" * 64,
                        "native_artifact_report_schema_version": 3,
                        "native_artifact_report_file_sha256": digest,
                        "probe_registry_sha256": "sha256:" + "6" * 64,
                    }
                ),
                encoding="utf-8",
            )
            loaded = load_native_artifact_baseline(
                baseline,
                lock_path=lock,
                repo_root=SCRIPT_DIR.parent,
            )
            self.assertEqual(compare_native_artifact_sizes(loaded, measurements), [])
            changed_toolchain = json.loads(json.dumps(measurements))
            changed_toolchain[0]["build"]["apple_toolchain"]["xcode_version"] = (
                "Xcode changed"
            )
            self.assertTrue(
                any(
                    "Apple toolchain changed" in failure
                    for failure in compare_native_artifact_sizes(
                        loaded,
                        changed_toolchain,
                    )
                )
            )

            tampered = json.loads(json.dumps(report))
            tampered["toolchain"]["cargo_version"] = "cargo changed"
            tampered["report_sha256"] = artifact_verifier.embedded_report_sha256(
                tampered
            )
            baseline.write_text(json.dumps(tampered), encoding="utf-8")
            with self.assertRaisesRegex(NativeArtifactSizeError, "whole-file digest"):
                load_native_artifact_baseline(
                    baseline,
                    lock_path=lock,
                    repo_root=SCRIPT_DIR.parent,
                )

        self.assertEqual(artifact_growth_budget(5_828_032, "semantic"), 65_536)
        self.assertEqual(artifact_growth_budget(44_205_776, "semantic"), 442_058)
        self.assertEqual(artifact_growth_budget(24_528_944, "full"), 524_288)
        self.assertEqual(artifact_growth_budget(169_231_248, "full"), 3_384_625)

    def test_other_report_zero_digest_is_not_an_atomic_lock(self) -> None:
        profiles = load_native_artifact_profiles()
        report = native_artifact_report(
            tuple(self._measurement(profile) for profile in profiles),
            repo_root=SCRIPT_DIR.parent,
            toolchain=self._toolchain(),
            source_snapshot_sha256=self._snapshot_sha256(),
        )
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            baseline = root / "native-artifact-sizes.json"
            baseline.write_text(json.dumps(report), encoding="utf-8")
            lock = root / "lock.json"
            lock.write_text(
                json.dumps(
                    {
                        "schema_version": BASELINE_LOCK_SCHEMA_VERSION,
                        "baseline_commit": BASELINE_COMMIT,
                        "baseline_input_sha256": {
                            path.as_posix(): file_sha256(SCRIPT_DIR.parent / path)
                            for path in BASELINE_INPUT_PATHS
                        },
                        "source_snapshot_sha256": self._snapshot_sha256(),
                        "baseline_tree": BASELINE_TREE,
                        "dependency_report_schema_version": (
                            DEPENDENCY_BASELINE_SCHEMA_VERSION
                        ),
                        "dependency_report_file_sha256": "sha256:" + "0" * 64,
                        "native_artifact_report_schema_version": 3,
                        "native_artifact_report_file_sha256": (
                            "sha256:" + hashlib.sha256(baseline.read_bytes()).hexdigest()
                        ),
                        "probe_registry_sha256": "sha256:" + "6" * 64,
                    }
                ),
                encoding="utf-8",
            )
            with self.assertRaisesRegex(NativeArtifactSizeError, "not finalized"):
                load_native_artifact_baseline(
                    baseline,
                    lock_path=lock,
                    repo_root=SCRIPT_DIR.parent,
                )

    def test_known_incremental_shared_target_report_is_rejected_before_decode(
        self,
    ) -> None:
        rejected = next(iter(artifact_verifier.REJECTED_BASELINE_FILE_SHA256))
        with (
            mock.patch.object(artifact_verifier, "bytes_sha256", return_value=rejected),
            mock.patch.object(
                type(artifact_verifier.STRICT_JSON),
                "load_bytes",
                return_value=(b"rejected", None),
            ),
            self.assertRaisesRegex(NativeArtifactSizeError, "incremental shared-target"),
        ):
            load_native_artifact_baseline(Path("does-not-need-to-exist.json"))

    def test_full_growth_requires_an_exact_self_invalidating_approval(self) -> None:
        profiles = load_native_artifact_profiles()
        measurements = tuple(self._measurement(profile) for profile in profiles)
        report = native_artifact_report(
            measurements,
            repo_root=SCRIPT_DIR.parent,
            toolchain=self._toolchain(),
            source_snapshot_sha256=self._snapshot_sha256(),
        )
        current = json.loads(json.dumps(measurements))
        full = next(
            profile
            for profile in current
            if profile["profile"]["label"] == "ffi-full-native"
        )
        artifact = next(
            row for row in full["artifacts"] if row["artifact_kind"] == "cdylib"
        )
        baseline_size = artifact["stripped_size_bytes"]
        budget = artifact_growth_budget(baseline_size, "full")
        artifact["stripped_size_bytes"] = baseline_size + budget + 1
        artifact["stripped_sha256"] = "sha256:" + "7" * 64

        missing = evaluate_native_artifact_sizes(report, current)
        self.assertTrue(any("exact approval required" in item for item in missing.failures))
        approval = NativeArtifactSizeApproval(
            profile="ffi-full-native",
            artifact_kind="cdylib",
            baseline_report_sha256=report["report_sha256"],
            baseline_stripped_sha256=next(
                row
                for profile in report["profiles"]
                if profile["profile"]["label"] == "ffi-full-native"
                for row in profile["artifacts"]
                if row["artifact_kind"] == "cdylib"
            )["stripped_sha256"],
            current_stripped_sha256=artifact["stripped_sha256"],
            baseline_size_bytes=baseline_size,
            current_size_bytes=artifact["stripped_size_bytes"],
            budget_bytes=budget,
            reason="Reviewed fixture growth.",
        )
        approved = evaluate_native_artifact_sizes(report, current, (approval,))
        self.assertEqual(approved.failures, ())
        self.assertTrue(any(delta.status == "approved" for delta in approved.deltas))

        artifact["stripped_size_bytes"] = baseline_size
        artifact["stripped_sha256"] = "sha256:" + "2" * 64
        stale = evaluate_native_artifact_sizes(report, current, (approval,))
        self.assertTrue(any("stale or unmatched" in item for item in stale.failures))

    def test_approval_loader_rejects_semantic_and_boolean_size_records(self) -> None:
        base = {
            "profile": "ffi-semantic",
            "artifact_kind": "cdylib",
            "baseline_report_sha256": "sha256:" + "1" * 64,
            "baseline_stripped_sha256": "sha256:" + "2" * 64,
            "current_stripped_sha256": "sha256:" + "3" * 64,
            "baseline_size_bytes": 1_000_000,
            "current_size_bytes": 1_100_000,
            "budget_bytes": 65_536,
            "reason": "Reviewed fixture growth.",
        }
        with tempfile.TemporaryDirectory() as temporary_directory:
            path = Path(temporary_directory) / "approvals.json"
            for record, message in (
                (base, "only the full"),
                ({**base, "profile": "ffi-full-native", "budget_bytes": True}, "integer"),
                (
                    {
                        **base,
                        "profile": "ffi-full-native",
                        "current_size_bytes": 1_600_000,
                        "budget_bytes": 524_288,
                        "reason": (
                            "Explain the reviewed source of this full-artifact growth."
                        ),
                    },
                    "reviewed",
                ),
            ):
                path.write_text(
                    json.dumps(
                        {
                            "schema_version": 1,
                            "registry_id": (
                                "merman-ffi-contract-native-size-approvals"
                            ),
                            "approvals": [record],
                        }
                    ),
                    encoding="utf-8",
                )
                with self.subTest(message=message), self.assertRaisesRegex(
                    NativeArtifactSizeError,
                    message,
                ):
                    load_native_artifact_size_approvals(path)

    def test_capture_refuses_a_wrong_repository_or_existing_destination(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            source = root / "source"
            source.mkdir()
            with (
                mock.patch.object(
                    baseline_capture,
                    "checked_output",
                    return_value=str(root),
                ),
                self.assertRaisesRegex(
                    baseline_capture.BaselineCaptureError,
                    "Git top-level",
                ),
            ):
                baseline_capture.validate_source_repository(source)

            with (
                mock.patch.object(
                    baseline_capture,
                    "checked_output",
                    side_effect=(str(source), "not-the-fixed-commit"),
                ),
                self.assertRaisesRegex(
                    baseline_capture.BaselineCaptureError,
                    "does not resolve",
                ),
            ):
                baseline_capture.validate_source_repository(source)

            output = root / "output"
            output.mkdir()
            (output / "existing.json").write_text("{}", encoding="utf-8")
            with self.assertRaisesRegex(
                baseline_capture.BaselineCaptureError,
                "must not already exist",
            ):
                baseline_capture.prepare_output_destination(output)

    @classmethod
    def _measurement(cls, profile: object) -> dict[str, object]:
        sizes = (
            ("cdylib", "libmerman_ffi.dylib", 1_000_000),
            ("staticlib", "libmerman_ffi.a", 2_000_000),
        )
        toolchain = cls._apple_toolchain()
        command = native_build_command(
            profile,
            repo_root=Path("$REPO"),
            target_dir=Path(f"$EVIDENCE/build/{profile.label}"),
            target="aarch64-apple-darwin",
            bindings=NativeBuildBindings(
                cargo="$CARGO",
                rustc="$RUSTC",
                linker_driver="$LINKER_DRIVER",
                developer_dir="$DEVELOPER_DIR",
                sdk="$SDK",
            ),
        )
        return {
            "profile": profile.projection("aarch64-apple-darwin"),
            "build": {
                "command": command,
                "target_dir": f"$EVIDENCE/build/{profile.label}",
                "target": "aarch64-apple-darwin",
                "cargo_message_format": "json-render-diagnostics",
                "artifact_selection": "current compiler-artifact event",
                "apple_toolchain": toolchain.projection(),
            },
            "artifacts": [
                {
                    "artifact_kind": kind,
                    "file_name": name,
                    "raw_size_bytes": size + 100,
                    "raw_sha256": "sha256:" + "1" * 64,
                    "stripped_size_bytes": size,
                    "stripped_sha256": "sha256:" + "2" * 64,
                    "strip": {
                        "recipe_id": "apple-strip-local-symbols-v1",
                        "tool": toolchain.strip.path,
                        "tool_sha256": toolchain.strip.sha256,
                        "command": ["$STRIP", "-x", "$ARTIFACT"],
                    },
                }
                for kind, name, size in sizes
            ],
        }

    @staticmethod
    def _apple_toolchain() -> AppleToolchain:
        developer = "/Applications/Xcode.app/Contents/Developer"
        tools = f"{developer}/Toolchains/XcodeDefault.xctoolchain/usr/bin"
        return AppleToolchain(
            developer_dir=developer,
            xcode_version="Xcode 26.5\nBuild version 17F42",
            sdk_path=(
                f"{developer}/Platforms/MacOSX.platform/Developer/SDKs/"
                "MacOSX26.5.sdk"
            ),
            sdk_version="26.5",
            sdk_settings=ToolIdentity(
                (
                    f"{developer}/Platforms/MacOSX.platform/Developer/SDKs/"
                    "MacOSX26.5.sdk/SDKSettings.json"
                ),
                "sha256:" + "4" * 64,
            ),
            deployment_target="MACOSX_DEPLOYMENT_TARGET=11.0",
            linker_driver=ToolIdentity(
                f"{tools}/clang",
                "sha256:" + "2" * 64,
            ),
            linker=ToolIdentity(f"{tools}/ld", "sha256:" + "5" * 64),
            strip=ToolIdentity(f"{tools}/strip", "sha256:" + "3" * 64),
        )

    @staticmethod
    def _toolchain() -> dict[str, object]:
        return {
            "cargo": {
                "path": "/toolchain/bin/cargo",
                "sha256": "sha256:" + "1" * 64,
            },
            "rustc": {
                "path": "/toolchain/bin/rustc",
                "sha256": "sha256:" + "2" * 64,
            },
            "cargo_version": "cargo 1.95.0",
            "rustc_verbose": "rustc 1.95.0\nhost: aarch64-apple-darwin",
            "host_target": "aarch64-apple-darwin",
        }

    @staticmethod
    def _snapshot_sha256() -> str:
        return "sha256:" + "3" * 64


if __name__ == "__main__":
    unittest.main()
