"""Behavior checks for installed Apple consumer evidence."""

import json
from pathlib import Path
import plistlib
import random
import subprocess
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest import mock
import zipfile

from scripts import apple_qualify
from scripts.artifact_profile_recipe import load_artifact_profiles


def consumer_payload():
    profile = next(p for p in load_artifact_profiles() if p.profile_id == "apple-uniffi-native")
    def output(body):
        svg = f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 20">{body}</svg>'
        return {"svg": svg, "metadata": {
            "version": 1, "operation_id": "svg", "media_type": "image/svg+xml",
            "runtime_policy": "deterministic", "byte_length": len(svg.encode()),
            "svg_viewport": {"min_x": 0, "min_y": 0, "width": 10, "height": 20},
        }}
    return {
        "catalog": {"package_version": "0.9.3", "capabilities": {
            "capability_ids": list(profile.capabilities), "output_ids": list(profile.outputs)},
            "registry": {"diagram_families": list(profile.diagram_families)}},
        "basic": output('<text>Hello</text><text>World</text>'),
        "math": output('<g class="merman-prepared-math-native"><path d="M0 0L1 1"/></g>'),
        "rejected_capability_ids": ["jpeg", "math", "pdf", "png"],
    }


class AppleQualificationTests(unittest.TestCase):
    def test_matched_compression_requires_at_least_half_the_original_payload(self):
        self.assertEqual(apple_qualify.compare_sizes(1000, 499)["reduction_fraction"], 0.501)
        with self.assertRaisesRegex(ValueError, "50%"):
            apple_qualify.compare_sizes(1000, 501)

    def test_installed_consumer_requires_formula_paths_and_exact_catalog(self):
        svg = ('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 20">'
               '<g class="merman-prepared-math-native"><path d="M0 0L1 1"/></g></svg>')
        metadata = {"version": 1, "operation_id": "svg", "media_type": "image/svg+xml",
                    "byte_length": len(svg.encode()), "runtime_policy": "deterministic",
                    "svg_viewport": {"min_x": 0, "min_y": 0, "width": 10, "height": 20}}
        catalog = {"capabilities": {"capability_ids": ["layout-elk", "math", "svg"],
                                    "output_ids": ["ascii", "svg"]},
                   "registry": {"diagram_families": ["flowchart"]},
                   "package_version": "1.2.3"}
        basic_svg = svg.replace('</svg>', '<text>Hello</text><text>World</text></svg>')
        payload = {"catalog": catalog, "basic": {"svg": basic_svg, "metadata": dict(metadata, byte_length=len(basic_svg.encode()))},
                   "math": {"svg": svg, "metadata": metadata},
                   "rejected_capability_ids": ["jpeg", "math", "pdf", "png"]}
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "consumer.json"
            path.write_text(json.dumps(payload))
            report = apple_qualify.validate_consumer(
                path, capabilities=("layout-elk", "math", "svg"),
                outputs=("ascii", "svg"), diagram_families=("flowchart",), version="1.2.3",
            )
            self.assertEqual(report["catalog"], catalog)
            payload["math"]["svg"] = svg.replace('d="M0 0L1 1"', 'd=""')
            payload["math"]["metadata"] = dict(metadata, byte_length=len(payload["math"]["svg"].encode()))
            path.write_text(json.dumps(payload))
            with self.assertRaisesRegex(ValueError, "glyph paths"):
                apple_qualify.validate_consumer(path, capabilities=("layout-elk", "math", "svg"),
                                                outputs=("ascii", "svg"), diagram_families=("flowchart",), version="1.2.3")

    def test_missing_families_and_blank_basic_diagram_are_rejected(self):
        profile = next(p for p in load_artifact_profiles() if p.profile_id == "apple-uniffi-native")
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "result.json"
            for missing in ("registry", "one-family", "all-but-one", "basic-content"):
                payload = consumer_payload()
                if missing == "registry":
                    del payload["catalog"]["registry"]
                elif missing == "basic-content":
                    payload["basic"]["svg"] = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 20"/>'
                    payload["basic"]["metadata"]["byte_length"] = len(payload["basic"]["svg"].encode())
                else:
                    payload["catalog"]["registry"]["diagram_families"] = (
                        ["flowchart"] if missing == "one-family" else list(profile.diagram_families[:-1]))
                path.write_text(json.dumps(payload))
                with self.subTest(missing=missing), self.assertRaises(ValueError):
                    apple_qualify.validate_consumer(path, capabilities=profile.capabilities,
                        outputs=profile.outputs, diagram_families=profile.diagram_families, version="0.9.3")


class InstalledPackageFlowTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        self.package = self.root / "安装 包" / "Merman-swiftpm-v0.9.3"
        self.package.mkdir(parents=True)
        (self.package / "Package.swift").write_text('// fixture\n')
        source = self.root / "source.swift"
        source.write_text('import Merman\n')
        self.source = source
        self.calls = []
        self.failure = None
        self.different_original = False
        self.enterContext(mock.patch.object(apple_qualify, "run", side_effect=self.process))

    def process(self, command, *, cwd, **options):
        command = [str(x) for x in command]
        self.calls.append((command, options))
        if command[:2] == ["swift", "build"]:
            project = Path(command[command.index("--package-path") + 1])
            manifest = (project / "Package.swift").read_text()
            expected_package = (project.parent.parent / "original-package"
                                if project.parent.name == "original-consumer" else self.package)
            self.assertIn(str(expected_package), manifest)
            binary_dir = project.parent / "bin"
            binary_dir.mkdir(exist_ok=True)
            (binary_dir / "InstalledConsumer").write_text("consumer binary")
            return SimpleNamespace(stdout=str(binary_dir))
        if command[0] == "uname":
            return SimpleNamespace(stdout="arm64\n")
        if command[:2] == ["xcrun", "--sdk"]:
            return SimpleNamespace(stdout="/SDK/" + command[2])
        if command[0] == "xcrun" and command[2:4] == ["list", "devices"]:
            return SimpleNamespace(stdout=json.dumps({"devices": {"com.apple.CoreSimulator.SimRuntime.iOS-26-0": [
                {"udid": "task-simulator", "isAvailable": True, "state": "Shutdown"}]}}))
        if command[0] == "xcrun" and command[2] in {"boot", "bootstatus", "shutdown"}:
            self.assertGreater(options.get("timeout", 0), 0)
            if command[2] == self.failure:
                raise subprocess.TimeoutExpired(command, options["timeout"])
            return SimpleNamespace(stdout="")
        if command[0].endswith("InstalledConsumer") or (command[0] == "xcrun" and command[2] == "spawn"):
            self.assertGreater(options.get("timeout", 0), 0)
            if self.failure == "consumer" or (self.failure == "spawn" and command[0] == "xcrun"):
                raise subprocess.TimeoutExpired(command, options["timeout"])
            payload = consumer_payload()
            if self.different_original and "original-consumer" in command[0]:
                payload["basic"]["svg"] = payload["basic"]["svg"].replace("Hello", "Hello!")
                payload["basic"]["metadata"]["byte_length"] += 1
            json.dump(payload, options["stdout"])
            return SimpleNamespace(stdout="")
        self.fail(f"unexpected external command: {command}")

    def test_unicode_package_manifest_and_device_simulator_builds(self):
        binary = apple_qualify.compile_consumer(self.package, self.source, self.root / "unicode")
        self.assertTrue(binary.is_file())
        manifest = (self.root / "unicode/consumer/Package.swift").read_text()
        self.assertNotIn(r"\u", manifest)
        report, _ = apple_qualify.qualify_package(repo_root=apple_qualify.REPO_ROOT,
            package_dir=self.package, work_dir=self.root / "installed", version="0.9.3", simulator=True)
        self.assertEqual(report["simulator_runtime"], "passed")
        builds = [command for command, _ in self.calls if command[:2] == ["swift", "build"]]
        self.assertTrue(any("arm64-apple-ios14.0" in command for command in builds))
        self.assertTrue(any("arm64-apple-ios14.0-simulator" in command for command in builds))
        self.assertTrue(any(command[2:4] == ["spawn", "task-simulator"] for command, _ in self.calls))

    def test_simulator_timeout_attempts_bounded_owned_cleanup(self):
        for failure in ("bootstatus", "spawn"):
            with self.subTest(failure=failure):
                self.failure = failure
                self.calls.clear()
                with self.assertRaises(subprocess.TimeoutExpired):
                    apple_qualify.qualify_package(repo_root=apple_qualify.REPO_ROOT,
                        package_dir=self.package, work_dir=self.root / failure, version="0.9.3", simulator=True)
                self.assertTrue(any(command[2:4] == ["shutdown", "task-simulator"]
                                    for command, _ in self.calls))

    def test_main_extracts_package_and_never_records_success_after_consumer_failure(self):
        archive = self.root / "package.zip"
        name = self.package.name
        with zipfile.ZipFile(archive, "w") as zipped:
            zipped.writestr(name + "/Package.swift", '// fixture\n')
        work = self.root / "work"
        self.package = work / "extracted" / name
        self.failure = "consumer"
        arguments = ["apple_qualify.py", "--package-archive", str(archive),
                     "--work-dir", str(work), "--version", "0.9.3"]
        with mock.patch.object(sys, "argv", arguments), self.assertRaises(subprocess.TimeoutExpired):
            apple_qualify.main()
        self.assertTrue((self.package / "Package.swift").is_file())
        self.assertFalse((work / "qualification.json").exists())

    def test_original_comparison_uses_original_libraries_and_rejects_changed_output(self):
        repo = self.root / "repo"
        (repo / "capabilities").mkdir(parents=True)
        (repo / "capabilities/artifact-profiles-v2.json").write_bytes(
            (apple_qualify.REPO_ROOT / "capabilities/artifact-profiles-v2.json").read_bytes())
        (repo / "platforms/apple/tests").mkdir(parents=True)
        (repo / "platforms/apple/tests/InstalledConsumer.swift").write_bytes(self.source.read_bytes())
        framework = self.package / "Merman.xcframework"
        library = framework / "macos-arm64/libmerman_uniffi.a"
        library.parent.mkdir(parents=True)
        library.write_bytes(b"cleaned native")
        (framework / "Info.plist").write_bytes(plistlib.dumps({"AvailableLibraries": [{
            "LibraryIdentifier": "macos-arm64", "LibraryPath": library.name,
            "SupportedPlatform": "macos", "SupportedArchitectures": ["arm64"],
        }]}))
        original = repo / "target/aarch64-apple-darwin/native-sdk/libmerman_uniffi.a"
        original.parent.mkdir(parents=True)
        original.write_bytes(random.Random(1).randbytes(16384))
        for mismatch in (False, True):
            self.different_original = mismatch
            work = self.root / ("different" if mismatch else "comparison")
            def compare():
                return apple_qualify.compare_original(repo_root=repo, package_dir=self.package,
                    work_dir=work, staged_result=consumer_payload(), version="0.9.3")
            if mismatch:
                with self.assertRaisesRegex(ValueError, "outputs differ"):
                    compare()
            else:
                report = compare()
                self.assertGreaterEqual(report["reduction_fraction"], 0.5)
            self.assertEqual((work / "original-package/Merman.xcframework/macos-arm64" / library.name).read_bytes(),
                             original.read_bytes())
        self.assertEqual(library.read_bytes(), b"cleaned native")


if __name__ == "__main__":
    unittest.main()
