#!/usr/bin/env python3
"""Build-free tests for SDK qualification ordering and failure boundaries."""

from __future__ import annotations

import contextlib
import hashlib
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest import mock

from scripts import sdk_archive, sdk_qualify


class SdkQualificationTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        self.repo = self.root / "repo"
        self.work = self.root / "qualification"
        self.output = self.root / "output"
        self.installed = self.root / "extracted-sdk"
        self.installed.mkdir()
        self.commit = "a" * 40
        self.version = "1.2.3"
        self.events = []
        self.failure = None
        self.git_status = ""
        self.cargo_target = self.repo / "target/sdk-target"
        self.write("Cargo.toml", '[workspace.package]\nversion = "1.2.3"\n')
        for name in sdk_archive.HEADERS:
            self.write(f"crates/merman-ffi/include/{name}", "#pragma once\n")
        for relative in ("CMakeLists.txt", "examples/render.c", "examples/probe.c", "examples/render.cpp",
                         "cmake/MermanConfig.cmake.in"):
            self.write("distribution/sdk/" + relative,
                       (sdk_archive.REPO_ROOT / "distribution/sdk" / relative).read_text())
        self.configure()
        self.enterContext(mock.patch.dict(os.environ, {
            "PATH": "host-tools", "CARGO_TARGET_DIR": "sdk-target",
        }, clear=True))
        self.enterContext(contextlib.redirect_stderr(io.StringIO()))
        self.process = self.enterContext(mock.patch.object(
            sdk_qualify.subprocess, "run", side_effect=self.run_process))
        original_sidecar = sdk_archive.write_catalog_sidecar

        def sidecar(**options):
            self.checkpoint("catalog")
            return original_sidecar(**options)

        self.enterContext(mock.patch.object(
            sdk_archive, "write_catalog_sidecar", side_effect=sidecar))
        self.package = self.enterContext(mock.patch.object(
            sdk_archive, "package_sdk", side_effect=self.package_archive))
        self.verify = self.enterContext(mock.patch.object(
            sdk_archive, "verify_sdk_archive",
            side_effect=lambda **options: self.checkpoint("verify")))
        self.extract = self.enterContext(mock.patch.object(
            sdk_qualify, "verified_archive_contents"))
        self.extract.return_value.__enter__.return_value = SimpleNamespace(root=self.installed)
        self.real_wasi_examples = sdk_qualify.verify_installed_wasi_examples
        self.wasi_examples = self.enterContext(mock.patch.object(
            sdk_qualify, "verify_installed_wasi_examples",
            side_effect=lambda **options: self.checkpoint("installed-wasi-examples"),
            create=True))

    def write(self, relative, text):
        path = self.repo / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")
        return path

    def configure(self, *, target="x86_64-unknown-linux-gnu", flavor="standard"):
        self.target = target
        self.host = target
        wasi = target == sdk_archive.WASI_TARGET
        self.profile = f"sdk-{'wasi' if wasi else 'native'}-{flavor}"
        features = ["native-fonts", "png", "svg"]
        if flavor != "standard":
            features.append("math")
        if flavor == "full":
            features.append("layout-elk")
        features.sort()
        self.features = features
        self.cargo_profile = "qualification-fixture"
        self.descriptor = self.write("capabilities/artifact-profiles-v2.json", json.dumps({
            "schema_version": 2, "profiles": [{
                "id": self.profile, "semantic_target": "native",
                "cargo": {
                    "package": "merman-wasi" if wasi else "merman-ffi",
                    "manifest": f"crates/merman-{'wasi' if wasi else 'ffi'}/Cargo.toml",
                    "profile": self.cargo_profile, "default_features": False,
                    "features": features,
                    "target": {
                        "name": "merman_wasi" if wasi else "merman_ffi",
                        "kinds": ["cdylib"] if wasi else ["cdylib", "staticlib"],
                        "crate_types": ["cdylib"] if wasi else ["cdylib", "staticlib"],
                        "required_features": [],
                    },
                    "build_target": {"kind": "target-set", "triples": [target]},
                },
                "expected": {"runtime_ids": features, "outputs": ["png", "svg"],
                             "diagram_families": ["flowchart"]},
            }],
        }))
        self.catalog = {
            "schema_version": 1, "transport_api_version": 1 if wasi else 3,
            "package_version": self.version, "options_schema_versions": [2],
            "capabilities": {"capability_ids": features, "output_ids": ["png", "svg"],
                             "text_measurement": {"provider_ids": ["native-fonts"]}},
            "constructor_service_ids": ["native-font-context"],
            "registry": {"diagram_families": ["flowchart"]},
        }

    def qualify(self, **changes):
        options = dict(profile=self.profile, target=self.target, work_dir=self.work,
                       output_dir=self.output, repo_root=self.repo, source_commit=self.commit)
        options.update(changes)
        return sdk_qualify.qualify_sdk(**options)

    def assert_unqualified(self):
        self.assertFalse((self.work / "qualification.json").exists())
        self.assertFalse((self.work / "evidence").exists())

    def checkpoint(self, event):
        self.assert_unqualified()
        self.events.append(event)
        if self.failure == event:
            raise subprocess.CalledProcessError(1, [event], stderr="fixture failure")

    def run_process(self, command, **options):
        command = [str(item) for item in command]
        tool = Path(command[0]).name
        if tool == "git":
            if command[-1] == "HEAD^{commit}":
                return subprocess.CompletedProcess(command, 0, self.commit + "\n", "")
            self.assertIn("--untracked-files=all", command)
            return subprocess.CompletedProcess(command, 0, self.git_status, "")
        if tool == "rustc":
            self.assertEqual(command[1:], ["-vV"])
            return subprocess.CompletedProcess(command, 0, f"host: {self.host}\n", "")
        self.assertEqual(options["cwd"], self.repo)
        self.assertTrue(options["check"])
        if tool == "cargo":
            if command[1] == "metadata":
                self.assertTrue({"--locked", "--no-deps", "--format-version"}.issubset(command))
                return subprocess.CompletedProcess(
                    command, 0, json.dumps({"target_directory": str(self.cargo_target)}), "")
            (self.work / "build.log").write_text("build diagnostics\n", encoding="utf-8")
            self.checkpoint("build")
            built = self.cargo_target / self.target / self.cargo_profile
            built.mkdir(parents=True, exist_ok=True)
            for name in sdk_archive.library_names(self.target).values():
                (built / name).write_bytes(b"built " + name.encode())
        elif tool in {"install_name_tool", "codesign"}:
            self.checkpoint(tool)
            library = Path(command[-1])
            self.assertEqual(library, self.work / "libmerman_ffi.dylib")
            with library.open("ab") as stream:
                stream.write(b"\n" + tool.encode())
        elif tool == "cmake":
            if "-B" in command:
                build = Path(command[command.index("-B") + 1])
                (build / "Release" if "windows" in self.target else build).mkdir(
                    parents=True, exist_ok=True)
        elif tool == "ctest":
            self.assertEqual(Path(options["env"]["PATH"].split(os.pathsep)[0]),
                             self.installed / "bin")
            self.checkpoint("installed-consumer")
        elif tool == "otool":
            return subprocess.CompletedProcess(
                command, 0, "@rpath/libmerman_ffi.dylib\n", "")
        elif Path(command[0]).name == "merman-render":
            self.assertEqual(options["env"]["DYLD_PRINT_LIBRARIES"], "1")
            return subprocess.CompletedProcess(
                command, 0, "", str(self.installed / "lib/libmerman_ffi.dylib"))
        elif command[0] == sys.executable:
            script = Path(command[1]).name
            if script in {"smoke.py", "native_smoke.py"}:
                smoke = Path(command[command.index("--output-dir") + 1])
                smoke.mkdir()
                (smoke / "diagnostic.log").write_text("smoke diagnostics\n", encoding="utf-8")
                self.checkpoint("smoke")
                Path(command[command.index("--catalog-out") + 1]).write_text(
                    json.dumps(self.catalog), encoding="utf-8")
                (smoke / "renders").mkdir()
                (smoke / "renders/smoke.png").write_bytes(b"PNG fixture")
            elif script == "generate-rust-license-report.py":
                self.checkpoint("legal")
                Path(command[command.index("--output") + 1]).write_text(
                    '{"licenses": [{"text": "fixture license"}]}\n', encoding="utf-8")
            else:
                self.fail(f"unexpected Python script: {command}")
        else:
            self.fail(f"unexpected external command: {command}")
        return subprocess.CompletedProcess(command, 0, "", "")

    def package_archive(self, **options):
        self.checkpoint("package")
        for relative in ("runtime-catalog.json", "rust-cargo-dependencies.json"):
            self.assertTrue((self.work / relative).is_file(), relative)
        self.output.mkdir()
        archive = self.output / "sdk.tar.xz"
        archive.write_bytes(b"packaged SDK")
        Path(str(archive) + ".sha256").write_text(
            hashlib.sha256(archive.read_bytes()).hexdigest() + "  " + archive.name + "\n",
            encoding="utf-8")
        return archive

    def command(self, name):
        for call in self.process.call_args_list:
            arguments = call.args[0]
            if name == "cargo":
                if len(arguments) > 1 and arguments[1] == "build":
                    return arguments, call.kwargs
                continue
            if any(Path(str(item)).name == name for item in arguments[:2]):
                return arguments, call.kwargs
        self.fail(f"command was not run: {name}")

    def assert_qualified(self, report, smoke_name):
        self.assertEqual(json.loads((self.work / "qualification.json").read_text()), report)
        self.assertEqual(
            {key: report[key] for key in
             ("schema_version", "ok", "source_commit", "version", "profile", "target")},
            dict(schema_version=1, ok=True, source_commit=self.commit, version=self.version,
                 profile=self.profile, target=self.target))
        self.assertEqual(report["archive"], str(self.output / "sdk.tar.xz"))
        self.assertEqual(report["evidence_dir"], str(self.work / "evidence"))
        self.assertEqual((Path(report["evidence_dir"]) / smoke_name / "renders/smoke.png").read_bytes(),
                         (self.work / smoke_name / "renders/smoke.png").read_bytes())
        command, _ = self.command("generate-rust-license-report.py")
        self.assertIn("--write", command)
        self.assertEqual(command[command.index("--artifact-profile") + 1], self.profile)
        self.assertEqual(command[command.index("--target") + 1], self.target)

    def test_native_build_uses_descriptor_and_installed_consumer_before_publishing(self):
        self.configure(target="x86_64-pc-windows-msvc", flavor="math")
        parity = self.root / "wasi-comparison"
        report = self.qualify(wasi_dir=parity)
        self.assertEqual(self.events, [
            "build", "smoke", "catalog", "legal", "package", "verify", "installed-consumer",
        ])
        command, _ = self.command("cargo")
        for flag, value in (("--profile", self.cargo_profile), ("--package", "merman-ffi"),
                            ("--target", self.target), ("--features", ",".join(self.features)),
                            ("--manifest-path", str(self.repo / "crates/merman-ffi/Cargo.toml"))):
            self.assertEqual(command[command.index(flag) + 1], value)
        self.assertTrue({"--locked", "--lib", "--no-default-features"}.issubset(command))
        self.assertIn("-DMERMAN_BUILD_CPP_EXAMPLES=ON", self.command("cmake")[0])
        command, options = self.command("native_smoke.py")
        self.assertEqual(command[:2], [sys.executable, str(self.repo / "platforms/wasi/native_smoke.py")])
        self.assertEqual(command[command.index("--probe") + 1],
                         str(self.work / "consumer-build/Release/merman-probe.exe"))
        self.assertEqual(command[command.index("--wasi-dir") + 1], str(parity.resolve()))
        self.assertEqual(command[command.index("--flavor") + 1], "math")
        prefix = self.work / "consumer-prefix"
        self.assertEqual(Path(options["env"]["PATH"].split(os.pathsep)[0]), prefix / "bin")
        self.assertEqual((prefix / "bin/merman_ffi.dll").read_bytes(), b"built merman_ffi.dll")
        self.assertTrue((prefix / "lib/merman_ffi.lib").is_file())
        self.assertTrue((prefix / "lib/merman_ffi.dll.lib").is_file())
        self.extract.assert_called_once_with(
            self.output / "sdk.tar.xz", self.output / "sdk.tar.xz.sha256",
            package_name="merman-" + self.profile, target=self.target, version=self.version)
        self.assert_qualified(report, "native-smoke")

    def test_wasi_build_uses_wasi_harness_and_skips_native_consumer(self):
        self.configure(target=sdk_archive.WASI_TARGET, flavor="full")
        report = self.qualify()
        self.assertEqual(self.events, ["build", "smoke", "catalog", "legal", "package", "verify",
                                       "installed-wasi-examples"])
        command, _ = self.command("smoke.py")
        self.assertEqual(command[:2], [sys.executable, str(self.repo / "platforms/wasi/smoke.py")])
        self.assertEqual(command[command.index("--module") + 1], str(self.work / "merman_wasi.wasm"))
        self.assertEqual(command[command.index("--flavor") + 1], "full")
        self.assertNotIn("--wasi-dir", command)
        commands = [call.args[0] for call in self.process.call_args_list]
        self.assertFalse(any(command[0] in {"rustc", "cmake", "ctest"} for command in commands))
        self.extract.assert_not_called()
        self.wasi_examples.assert_called_once_with(
            archive=self.output / "sdk.tar.xz", profile=self.profile, version=self.version,
            repo_root=self.repo, output_dir=self.work / "wasi-smoke/examples")
        self.assert_qualified(report, "wasi-smoke")

    def test_mac_library_is_normalized_and_signed_before_catalog_hashing(self):
        self.configure(target="aarch64-apple-darwin")
        report = self.qualify()
        self.assertEqual(self.events, [
            "build", "install_name_tool", "codesign", "smoke", "catalog", "legal",
            "package", "verify", "installed-consumer",
        ])
        command, _ = self.command("install_name_tool")
        self.assertEqual(command[1:3], ["-id", "@rpath/libmerman_ffi.dylib"])
        command, _ = self.command("codesign")
        self.assertIn("--timestamp=none", command)
        self.assertEqual(command[command.index("--sign") + 1], "-")
        library = self.work / "libmerman_ffi.dylib"
        self.assertTrue(library.read_bytes().endswith(b"\ninstall_name_tool\ncodesign"))
        sidecar = json.loads((self.work / "runtime-catalog.json").read_text())
        self.assertEqual(sidecar["artifacts"][library.name],
                         hashlib.sha256(library.read_bytes()).hexdigest())
        original = self.cargo_target / self.target / self.cargo_profile / library.name
        self.assertEqual(original.read_bytes(), b"built libmerman_ffi.dylib")
        self.assert_qualified(report, "native-smoke")

    def test_profile_transport_and_undeclared_target_are_rejected_before_build(self):
        for target in (sdk_archive.WASI_TARGET, "aarch64-unknown-linux-gnu"):
            with self.subTest(target=target), self.assertRaises(sdk_archive.ArchiveVerificationError):
                self.qualify(target=target)
        self.assertEqual(self.events, [])
        self.assertFalse(self.work.exists())
        self.package.assert_not_called()
        self.verify.assert_not_called()

    def test_source_must_be_exact_checked_out_commit_before_build(self):
        for source in ("v1.2.3", "b" * 40):
            with self.subTest(source=source), self.assertRaises(sdk_archive.ArchiveVerificationError):
                self.qualify(source_commit=source)
        self.assertEqual(self.events, [])
        self.assertFalse(self.work.exists())
        self.package.assert_not_called()
        self.verify.assert_not_called()

    def test_dirty_source_tree_is_rejected_before_the_sdk_build(self):
        for status in (" M crates/merman/src/lib.rs\n", "?? crates/merman/src/new.rs\n"):
            with self.subTest(status=status):
                self.git_status = status
                with self.assertRaisesRegex(sdk_archive.ArchiveVerificationError, "uncommitted changes"):
                    self.qualify()
                self.assertFalse(self.work.exists())
                self.assertEqual(self.events, [])
                self.package.assert_not_called()
                self.verify.assert_not_called()

    def test_native_requires_rustc_host_even_when_environment_claims_target(self):
        self.host = "aarch64-apple-darwin"
        with mock.patch.dict(os.environ, {
            "HOST": self.target, "TARGET": self.target, "CARGO_BUILD_TARGET": self.target,
        }), self.assertRaisesRegex(RuntimeError, "actual Rust target host"):
            self.qualify()
        self.command("rustc")
        self.assertEqual(self.events, [])
        self.assertFalse(self.work.exists())

    def test_existing_work_directory_is_rejected_without_touching_diagnostics(self):
        self.work.mkdir()
        diagnostic = self.work / "previous-run.log"
        diagnostic.write_bytes(b"keep previous failure")
        with self.assertRaises(FileExistsError):
            self.qualify()
        self.assertEqual(list(self.work.iterdir()), [diagnostic])
        self.assertEqual(diagnostic.read_bytes(), b"keep previous failure")
        self.assertEqual(self.events, [])
        self.package.assert_not_called()
        self.verify.assert_not_called()

    def test_cargo_reported_target_directory_wins_over_environment_guesses(self):
        self.cargo_target = self.root / "configured-target"
        self.configure(target=sdk_archive.WASI_TARGET)
        report = self.qualify()
        self.assert_qualified(report, "wasi-smoke")
        self.assertTrue((self.work / "merman_wasi.wasm").is_file())

    def test_wasi_rejects_comparison_directory_before_build(self):
        self.configure(target=sdk_archive.WASI_TARGET)
        with self.assertRaisesRegex(RuntimeError, "--wasi-dir"):
            self.qualify(wasi_dir=self.root / "comparison")
        self.assertEqual(self.events, [])
        self.assertFalse(self.work.exists())
        self.package.assert_not_called()
        self.verify.assert_not_called()

    def test_build_smoke_and_legal_failures_retain_work_and_never_package(self):
        for phase in ("build", "smoke", "legal"):
            with self.subTest(phase=phase):
                self.failure = phase
                self.work = self.root / ("failed-" + phase)
                self.events.clear()
                with self.assertRaises(subprocess.CalledProcessError):
                    self.qualify()
                self.assertEqual(self.events[-1], phase)
                self.assertEqual((self.work / "build.log").read_text(), "build diagnostics\n")
                if phase != "build":
                    self.assertTrue((self.work / "native-smoke/diagnostic.log").is_file())
                    self.assertTrue((self.work / "libmerman_ffi.so").is_file())
                self.assert_unqualified()
                self.package.assert_not_called()
                self.verify.assert_not_called()
                self.extract.assert_not_called()

    def test_archive_verification_failure_prevents_installed_consumer_and_success_record(self):
        self.verify.side_effect = RuntimeError("archive verification failed")
        with self.assertRaisesRegex(RuntimeError, "archive verification failed"):
            self.qualify()
        self.package.assert_called_once()
        self.verify.assert_called_once()
        self.extract.assert_not_called()
        self.assertTrue((self.work / "runtime-catalog.json").is_file())
        self.assert_unqualified()

    def test_installed_consumer_failure_keeps_diagnostics_without_success_evidence(self):
        self.failure = "installed-consumer"
        with self.assertRaises(subprocess.CalledProcessError):
            self.qualify()
        self.package.assert_called_once()
        self.verify.assert_called_once()
        self.extract.assert_called_once()
        self.assertEqual(self.events[-1], "installed-consumer")
        self.assertTrue((self.work / "native-smoke/renders/smoke.png").is_file())
        self.assertTrue((self.work / "rust-cargo-dependencies.json").is_file())
        self.assert_unqualified()

    def test_installed_wasi_example_failure_prevents_success_record(self):
        self.configure(target=sdk_archive.WASI_TARGET)
        self.failure = "installed-wasi-examples"
        with self.assertRaises(subprocess.CalledProcessError):
            self.qualify()
        self.package.assert_called_once()
        self.verify.assert_called_once()
        self.wasi_examples.assert_called_once()
        self.assert_unqualified()

    def test_extracted_wasi_examples_run_both_languages_and_reject_output_disagreement(self):
        module = self.installed / "lib/merman_wasi.wasm"
        module.parent.mkdir()
        module.write_bytes(b"fixture module")
        calls = []
        mismatch = False

        def example(command, **options):
            command = [str(item) for item in command]
            calls.append(command)
            self.assertEqual(options["environment"]["MERMAN_WASI_MODULE"], str(module))
            self.assertEqual(options["environment"]["MERMAN_WASI_FONT"],
                             str(self.installed / "fonts/Cousine-Regular.ttf"))
            if "--output" in command:
                output = Path(command[command.index("--output") + 1])
                output.write_bytes(b"different" if mismatch and "python." in output.name else b"render")

        with mock.patch.object(sdk_qualify, "run", side_effect=example):
            self.real_wasi_examples(archive=self.output / "sdk.tar.xz", profile="sdk-wasi-standard",
                                    version=self.version, repo_root=self.repo,
                                    output_dir=self.root / "examples")
            self.assertEqual(len(calls), 6)
            self.assertEqual(calls[0][0], "go")
            self.assertIn("-count=1", calls[0])
            self.assertIn("unittest", calls[1])
            self.assertTrue((self.root / "examples/verified.json").is_file())
            mismatch = True
            with self.assertRaisesRegex(RuntimeError, "example output mismatch"):
                self.real_wasi_examples(archive=self.output / "sdk.tar.xz", profile="sdk-wasi-standard",
                                        version=self.version, repo_root=self.repo,
                                        output_dir=self.root / "mismatched-examples")
            self.assertFalse((self.root / "mismatched-examples/verified.json").exists())


if __name__ == "__main__":
    unittest.main()
