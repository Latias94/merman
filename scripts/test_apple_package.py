#!/usr/bin/env python3
"""Build-free behavior tests for the complete Apple package handoff."""

from __future__ import annotations

from contextlib import redirect_stdout
import hashlib
import io
import json
import os
from pathlib import Path
import plistlib
import subprocess
import tempfile
import unittest
from unittest import mock
import zipfile

from scripts import apple_package, sdk_archive
from scripts.artifact_profile_recipe import load_artifact_profiles
from scripts.release_archive import ArchiveVerificationError


class ApplePackageTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.repo = self.root / "repo"
        self.repo.mkdir()
        self.xcf = self.root / "built/Merman.xcframework"
        self.output = self.root / "output"
        self.version = "1.2.3"
        self.commit = "a" * 40
        source = mock.patch.object(apple_package, "resolve_source_commit", return_value=self.commit)
        self.source = source.start()
        self.addCleanup(source.stop)
        clean = mock.patch.object(apple_package, "require_clean_source_tree")
        clean.start()
        self.addCleanup(clean.stop)
        self.write(self.repo / "Cargo.toml", '[workspace.package]\nversion = "1.2.3"\n')
        descriptor = json.loads((sdk_archive.REPO_ROOT / "capabilities/artifact-profiles-v2.json").read_text())
        self.profile = next(p for p in descriptor["profiles"] if p["id"] == "apple-uniffi-native")
        self.descriptor = self.repo / "capabilities/artifact-profiles-v2.json"
        self.write(self.descriptor, json.dumps({"schema_version": 2, "profiles": [self.profile]}))
        self.generated = self.repo / "platforms/apple/Sources/Merman/Generated"
        for name, text in {
            "Merman.swift": "public struct Merman {}\n",
            "MermanResourceContract.swift": "public enum ResourceContract {}\n",
            "MermanFFI.h": "void merman(void);\n",
            "MermanFFI.modulemap": 'module MermanFFI { header "MermanFFI.h" export * }\n',
            "Nested/Additional.swift": "public enum Additional {}\n",
        }.items():
            self.write(self.generated / name, text)
        self.legal = self.repo / "platforms/apple"
        for name in ("LICENSE", "THIRD_PARTY_NOTICES.md", "THIRD_PARTY_LICENSES/ratex/LICENSE",
                     "THIRD_PARTY_LICENSES/eclipse-elk/LICENSE.md"):
            self.write(self.legal / name, name + "\n")
        self.write(self.legal / "THIRD_PARTY_LICENSES/rust-cargo-dependencies.json",
                   json.dumps({"artifact_bundle": {"artifact_profiles": [
                       load_artifact_profiles(self.descriptor)[0].report_projection()]}}))
        self.entries = [
            {"LibraryIdentifier": "ios-arm64", "SupportedPlatform": "ios",
             "SupportedArchitectures": ["arm64"]},
            {"LibraryIdentifier": "ios-arm64_x86_64-simulator", "SupportedPlatform": "ios",
             "SupportedPlatformVariant": "simulator", "SupportedArchitectures": ["arm64", "x86_64"]},
            {"LibraryIdentifier": "macos-arm64_x86_64", "SupportedPlatform": "macos",
             "SupportedArchitectures": ["arm64", "x86_64"]},
        ]
        for entry in self.entries:
            entry.update(LibraryPath="libmerman_uniffi.a", HeadersPath="Headers")
            base = self.xcf / entry["LibraryIdentifier"]
            self.write(base / entry["LibraryPath"], "tiny native library\n")
            for generated, installed in (("MermanFFI.h", "MermanFFI.h"),
                                         ("MermanFFI.modulemap", "module.modulemap")):
                self.write(base / "Headers" / installed, (self.generated / generated).read_text())
        self.save_plist()

    def write(self, path, text):
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")
        return path

    def save_plist(self, entries=None):
        self.xcf.mkdir(parents=True, exist_ok=True)
        with (self.xcf / "Info.plist").open("wb") as stream:
            plistlib.dump({"XCFrameworkFormatVersion": "1.0", "AvailableLibraries":
                           self.entries if entries is None else entries}, stream)

    def package(self, **changes):
        args = dict(repo_root=self.repo, xcframework=self.xcf, output_dir=self.output,
                    version=self.version, source_commit=self.commit)
        args.update(changes)
        return apple_package.package_apple(**args)

    def remote_url(self):
        return ("https://github.com/Latias94/merman/releases/download/"
                f"v{self.version}/Merman.xcframework-v{self.version}.zip")

    def members(self, archive):
        with zipfile.ZipFile(archive) as stream:
            return {name: stream.read(name) for name in stream.namelist()}

    def test_complete_local_package_and_binary_have_the_same_inputs(self):
        result = self.package()
        self.assertEqual(set(result), {"package_dir", "package_archive", "binary_archive"})
        name = f"Merman-swiftpm-v{self.version}"
        self.assertEqual(result["package_dir"], (self.output / name).resolve())
        self.assertEqual(result["package_archive"].name, name + ".zip")
        self.assertEqual(result["binary_archive"].name, f"Merman.xcframework-v{self.version}.zip")
        local = self.members(result["package_archive"])
        local = {key.removeprefix(name + "/"): value for key, value in local.items()}
        staged = {path.relative_to(result["package_dir"]).as_posix(): path.read_bytes()
                  for path in result["package_dir"].rglob("*") if path.is_file()}
        self.assertEqual(local, staged)
        for path in self.generated.rglob("*"):
            if path.is_file():
                self.assertEqual(local["Sources/Merman/Generated/" + path.relative_to(self.generated).as_posix()],
                                 path.read_bytes())
        binary = self.members(result["binary_archive"])
        for path in self.xcf.rglob("*"):
            if path.is_file():
                name = "Merman.xcframework/" + path.relative_to(self.xcf).as_posix()
                self.assertEqual(binary[name], path.read_bytes())
        for path in self.legal.rglob("*"):
            if path.is_file() and "Sources" not in path.relative_to(self.legal).parts:
                name = path.relative_to(self.legal).as_posix()
                self.assertEqual(binary[name], path.read_bytes())
        self.assertEqual(binary, {name: data for name, data in local.items()
                                 if name.startswith(("Merman.xcframework/", "THIRD_PARTY_LICENSES/"))
                                 or name in {"LICENSE", "THIRD_PARTY_NOTICES.md", "SOURCE_AVAILABILITY.txt"}})
        self.assertIn("README.md", local)
        availability = local["SOURCE_AVAILABILITY.txt"].decode()
        self.assertIn(self.version, availability)
        self.assertIn(self.commit, availability)
        manifest = local["Package.swift"].decode()
        for required in ('// swift-tools-version: 5.9', '.iOS(.v14)', '.macOS(.v12)',
                         'name: "MermanFFI"', 'path: "Merman.xcframework"',
                         'path: "Sources/Merman"', 'dependencies: ["MermanFFI"]'):
            self.assertIn(required, manifest)
        self.assertNotIn("platforms/apple", manifest)
        for key in ("package_archive", "binary_archive"):
            path = result[key]
            digest = hashlib.sha256(path.read_bytes()).hexdigest()
            self.assertEqual(Path(str(path) + ".checksum").read_text(), f"{digest}  {path.name}\n")

    def test_remote_manifest_uses_only_the_binary_archive_digest(self):
        result = self.package(artifact_url=self.remote_url())
        remote = result["remote_manifest"]
        self.assertEqual(remote.name, f"Merman-swiftpm-v{self.version}.Package.swift")
        manifest = remote.read_text()
        binary_digest = hashlib.sha256(result["binary_archive"].read_bytes()).hexdigest()
        package_digest = hashlib.sha256(result["package_archive"].read_bytes()).hexdigest()
        self.assertIn(f'url: "{self.remote_url()}"', manifest)
        self.assertIn(f'checksum: "{binary_digest}"', manifest)
        self.assertNotIn(package_digest, manifest)
        self.assertNotIn('path: "Merman.xcframework"', manifest)
        self.assertIn('path: "Sources/Merman"', manifest)
        accepted = self.package(output_dir=self.root / "accepted", artifact_url=self.remote_url(),
                                expected_checksum=binary_digest)
        self.assertEqual(accepted["binary_archive"].read_bytes(), result["binary_archive"].read_bytes())
        with self.assertRaisesRegex(ArchiveVerificationError, "checksum"):
            self.package(output_dir=self.root / "wrong-archive", artifact_url=self.remote_url(),
                         expected_checksum=package_digest)

    def test_checksum_mismatch_fails_without_emitting_a_handoff(self):
        for checksum in ("0" * 64, "not-a-sha256"):
            with self.subTest(checksum=checksum):
                with self.assertRaisesRegex(ArchiveVerificationError, "checksum"):
                    self.package(artifact_url=self.remote_url(), expected_checksum=checksum)
                self.assertFalse((self.output / f"Merman-swiftpm-v{self.version}.Package.swift").exists())
                self.assertFalse((self.output / f"Merman-swiftpm-v{self.version}.zip").exists())

    def test_version_specific_https_artifact_url_is_required(self):
        valid = self.remote_url()
        invalid = (valid.replace("https:", "http:"), valid.replace("github.com", "user:secret@github.com"),
                   valid.replace("github.com", "user@github.com"), valid + "?token=secret", valid + "#part",
                   valid + "?", valid + "#", valid.replace("Merman.xcframework", "Merman-swiftpm"),
                   valid.replace("/v1.2.3/", "/latest/"), valid.replace("/v1.2.3/", "/v1.2.2/"),
                   valid.replace("https://github.com", "https://"), valid + "\n")
        for url in invalid:
            with self.subTest(url=url):
                with self.assertRaisesRegex(ArchiveVerificationError, "URL"):
                    self.package(artifact_url=url)

    def test_archives_are_deflated_and_deterministic_across_input_metadata(self):
        first = self.package()
        for directory in (self.generated, self.xcf, self.legal / "THIRD_PARTY_LICENSES"):
            for path in directory.rglob("*"):
                if path.is_file():
                    os.utime(path, (1760000000, 1760000000))
                    path.chmod(0o600)
        second = self.package(output_dir=self.root / "repeat")
        for key in ("package_archive", "binary_archive"):
            self.assertEqual(first[key].read_bytes(), second[key].read_bytes())
            with zipfile.ZipFile(second[key]) as archive:
                self.assertTrue(all(info.compress_type == zipfile.ZIP_DEFLATED for info in archive.infolist()))
                self.assertEqual({info.date_time for info in archive.infolist()}, {(1980, 1, 1, 0, 0, 0)})

    def test_partial_ios_or_macos_xcframework_is_rejected(self):
        for entries in (self.entries[:2], self.entries[2:]):
            with self.subTest(platforms=[entry["SupportedPlatform"] for entry in entries]):
                self.save_plist(entries)
                with self.assertRaisesRegex(ArchiveVerificationError, "targets"):
                    self.package()

    def test_required_targets_follow_the_descriptor(self):
        self.profile["cargo"]["build_target"]["triples"].remove("x86_64-apple-darwin")
        self.write(self.descriptor, json.dumps({"schema_version": 2, "profiles": [self.profile]}))
        with self.assertRaisesRegex(ArchiveVerificationError, "targets"):
            self.package()
        self.entries[2]["SupportedArchitectures"] = ["arm64"]
        self.save_plist()
        self.assertTrue(self.package()["package_archive"].is_file())

    def test_overlapping_slices_are_rejected_even_with_a_complete_union(self):
        duplicate = dict(self.entries[0], LibraryIdentifier="duplicate-device")
        for name in ("libmerman_uniffi.a", "Headers/MermanFFI.h", "Headers/module.modulemap"):
            self.write(self.xcf / "duplicate-device" / name, (self.xcf / "ios-arm64" / name).read_text())
        self.save_plist(self.entries + [duplicate])
        with self.assertRaisesRegex(ArchiveVerificationError, "duplicate|overlap"):
            self.package()

    def test_missing_declared_files_and_mismatched_generated_headers_are_rejected(self):
        base = self.xcf / self.entries[0]["LibraryIdentifier"]
        for relative in ("libmerman_uniffi.a", "Headers/MermanFFI.h", "Headers/module.modulemap"):
            path = base / relative
            content = path.read_text()
            with self.subTest(missing=relative):
                path.unlink()
                with self.assertRaises(ArchiveVerificationError):
                    self.package()
                self.write(path, content)
            if relative.startswith("Headers/"):
                with self.subTest(mismatch=relative):
                    self.write(path, "different build\n")
                    with self.assertRaisesRegex(ArchiveVerificationError, "match|differ"):
                        self.package()
                    self.write(path, content)

    def test_required_facade_resource_contract_and_legal_inputs_cannot_be_omitted(self):
        for path in (self.generated / "Merman.swift", self.generated / "MermanResourceContract.swift",
                     self.legal / "LICENSE", self.legal / "THIRD_PARTY_NOTICES.md"):
            content = path.read_text()
            with self.subTest(path=path.name):
                path.unlink()
                with self.assertRaises(ArchiveVerificationError):
                    self.package()
                self.write(path, content)

    def test_wrong_version_or_source_commit_is_rejected(self):
        with self.assertRaisesRegex(ArchiveVerificationError, "version"):
            self.package(version="1.2.4")
        self.source.side_effect = sdk_archive.resolve_source_commit
        head = subprocess.CompletedProcess([], 0, self.commit + "\n", "")
        with mock.patch.object(sdk_archive.subprocess, "run", return_value=head):
            for commit in ("b" * 40, "not-a-commit"):
                with self.subTest(commit=commit):
                    with self.assertRaisesRegex(ArchiveVerificationError, "source_commit"):
                        self.package(source_commit=commit)

    def test_source_commit_can_be_resolved_from_head(self):
        result = self.package(source_commit=None)
        self.assertIn(self.commit, (result["package_dir"] / "SOURCE_AVAILABILITY.txt").read_text())

    def test_dirty_source_tree_is_rejected_without_outputs(self):
        status = subprocess.CompletedProcess(
            [], 0, " M platforms/apple/Sources/Merman/Generated/Merman.swift\n", "")
        with (
            mock.patch.object(apple_package, "require_clean_source_tree",
                              sdk_archive.require_clean_source_tree),
            mock.patch.object(sdk_archive.subprocess, "run", return_value=status),
        ):
            with self.assertRaisesRegex(ArchiveVerificationError, "uncommitted changes"):
                self.package()
        self.assertFalse(self.output.exists())

    def test_existing_handoff_is_preserved(self):
        result = self.package(artifact_url=self.remote_url())
        before = {path: path.read_bytes() for path in self.output.rglob("*") if path.is_file()}
        with self.assertRaisesRegex(ArchiveVerificationError, "already exists"):
            self.package(artifact_url=self.remote_url())
        self.assertEqual(before, {path: path.read_bytes() for path in self.output.rglob("*") if path.is_file()})
        self.assertTrue(result["package_dir"].is_dir())

    def test_declared_paths_cannot_escape_the_staged_xcframework(self):
        for field in ("LibraryIdentifier", "LibraryPath", "HeadersPath"):
            original = self.entries[0][field]
            with self.subTest(field=field):
                self.entries[0][field] = "../" + original
                self.save_plist()
                with self.assertRaises(ArchiveVerificationError):
                    self.package()
                self.entries[0][field] = original

    def test_existing_file_outside_framework_cannot_be_declared_as_a_library(self):
        self.entries[0]["LibraryPath"] = "../../Sources/Merman/Generated/MermanFFI.h"
        self.save_plist()
        with self.assertRaisesRegex(ArchiveVerificationError, "path|traversal"):
            self.package()

    def test_invalid_plist_is_rejected_as_an_artifact_error(self):
        for content in ("not a plist", "<?xml version='1.0'?><plist><dict>"):
            with self.subTest(content=content):
                self.write(self.xcf / "Info.plist", content)
                with self.assertRaisesRegex(ArchiveVerificationError, "Info.plist"):
                    self.package()

    def test_cli_prints_returned_paths_and_resolves_default_xcframework_from_repo(self):
        with tempfile.TemporaryDirectory() as temporary:
            default_xcf = self.repo / "platforms/apple/Merman.xcframework"
            default_xcf.parent.mkdir(parents=True, exist_ok=True)
            self.xcf.rename(default_xcf)
            output = io.StringIO()
            with redirect_stdout(output):
                status = apple_package.main(["--repo-root", str(self.repo), "--output-dir", temporary,
                                             "--version", self.version, "--source-commit", self.commit,
                                             "--artifact-url", self.remote_url()])
            self.assertEqual(status, 0)
            result = json.loads(output.getvalue())
            self.assertEqual(set(result), {"package_dir", "package_archive", "binary_archive", "remote_manifest"})
            self.assertTrue(all(Path(path).exists() for path in result.values()))


class SliceTargetsTests(unittest.TestCase):
    def test_platform_variant_and_architecture_map_to_rust_targets(self):
        for entry, expected in (
            ({"SupportedPlatform": "ios", "SupportedArchitectures": ["arm64"]}, ("aarch64-apple-ios",)),
            ({"SupportedPlatform": "ios", "SupportedPlatformVariant": "simulator",
              "SupportedArchitectures": ["x86_64", "arm64"]}, ("x86_64-apple-ios", "aarch64-apple-ios-sim")),
            ({"SupportedPlatform": "macos", "SupportedArchitectures": ["arm64", "x86_64"]},
             ("aarch64-apple-darwin", "x86_64-apple-darwin")),
        ):
            with self.subTest(entry=entry):
                self.assertEqual(apple_package.slice_targets(entry), expected)

    def test_unsupported_platform_variants_and_architectures_are_rejected(self):
        for entry in ({}, {"SupportedPlatform": "tvos", "SupportedArchitectures": ["arm64"]},
                      {"SupportedPlatform": "ios", "SupportedArchitectures": ["x86_64"]},
                      {"SupportedPlatform": "ios", "SupportedPlatformVariant": "maccatalyst",
                       "SupportedArchitectures": ["arm64"]},
                      {"SupportedPlatform": "macos", "SupportedArchitectures": ["arm64e"]},
                      {"SupportedPlatform": "macos", "SupportedArchitectures": ["arm64", "arm64"]}):
            with self.subTest(entry=entry):
                with self.assertRaises(ArchiveVerificationError):
                    apple_package.slice_targets(entry)


if __name__ == "__main__":
    unittest.main()
