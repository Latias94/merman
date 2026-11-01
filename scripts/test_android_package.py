#!/usr/bin/env python3
"""Focused tests of the selected-version Maven delivery command."""

from __future__ import annotations

import contextlib
import hashlib
import importlib.util
import io
import json
import os
import subprocess
import sys
import tempfile
import unittest
import xml.etree.ElementTree as ET
import zipfile
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from scripts.test_verify_platform_bindings import (
    verify_platform_bindings as verifier,
    write_aar,
    write_android_maven_publication,
    write_sha256,
)


class AndroidPackageTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.repository = self.root / "repo"
        self.module = self.repository / "org" / "example" / "merman-android"
        self.selected = write_android_maven_publication(
            self.module, "0.9.3", group_id="org.example",
        )
        write_android_maven_publication(self.module, "0.9.2", group_id="org.example")
        (self.module / "maven-metadata.xml").write_text(
            """<metadata><groupId>org.example</groupId><artifactId>merman-android</artifactId>
<versioning><latest>0.9.3</latest><release>0.9.3</release><versions>
<version>0.9.2</version><version>0.9.3</version></versions>
<lastUpdated>20261101000000</lastUpdated></versioning></metadata>""",
            encoding="utf-8",
        )
        self.output = self.root / "out"
        self.command = [
            "package", "--repository", str(self.repository),
            "--group-id", "org.example", "--version", "0.9.3",
            "--output-dir", str(self.output),
        ]

    def run_package(self, *extra: str) -> int:
        return self.run_command([*self.command, *extra])

    def run_command(self, command: list[str]) -> int:
        path = Path(__file__).with_name("android_package.py")
        spec = importlib.util.spec_from_file_location("android_package", path)
        assert spec is not None and spec.loader is not None
        package = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(package)
        with (
            mock.patch.object(package, "verifier", verifier),
            mock.patch.object(verifier, "android_library_symbols", return_value={"JNI_OnLoad"}),
            mock.patch.object(
                verifier, "android_class_api",
                return_value="public io.merman.MermanIconPackSet getIconPackSet();",
            ),
        ):
            return package.main(command)

    def write_selected_facade(self) -> Path:
        android = self.root / "selected-source" / "platforms" / "android"
        resource = android / "src" / "main" / "resources" / "selected-facade.txt"
        source = android / "src" / "main" / "kotlin" / "selected" / "Facade.kt"
        for path, content in ((resource, "selected facade data"),
                              (source, "package selected\nclass Facade\n")):
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(content, encoding="utf-8")
        write_aar(
            self.selected / "merman-android-0.9.3.aar",
            verifier.ANDROID_PACKAGING_SENTINEL_CLASSES,
            resource_names=[resource.name],
        )
        with zipfile.ZipFile(self.selected / "merman-android-0.9.3-sources.jar", "w") as archive:
            archive.writestr("selected/Facade.kt", source.read_bytes())
        module_path = self.selected / "merman-android-0.9.3.module"
        module = json.loads(module_path.read_text(encoding="utf-8"))
        for variant in module["variants"]:
            for entry in variant["files"]:
                artifact = self.selected / entry["name"]
                entry["size"] = artifact.stat().st_size
                entry["sha256"] = hashlib.sha256(artifact.read_bytes()).hexdigest()
        module_path.write_text(json.dumps(module), encoding="utf-8")
        for artifact in self.selected.iterdir():
            if artifact.suffix in {".aar", ".pom", ".module", ".jar"}:
                write_sha256(artifact)
        return android

    def test_repository_zip_retains_selected_publication_and_metadata(self) -> None:
        aar = self.selected / "merman-android-0.9.3.aar"
        signature = aar.with_name(aar.name + ".asc")
        signature.write_bytes(b"retained detached signing sidecar")
        signature.with_name(signature.name + ".sha512").write_text(
            hashlib.sha512(signature.read_bytes()).hexdigest(), encoding="ascii",
        )
        self.assertEqual(self.run_package(), 0)
        archive = self.output / "merman-android-0.9.3-maven.zip"
        with zipfile.ZipFile(archive) as bundle:
            names = set(bundle.namelist())
            prefix = "org/example/merman-android/0.9.3/"
            for path in self.selected.iterdir():
                self.assertEqual(bundle.read(prefix + path.name), path.read_bytes())
            self.assertFalse(any("/0.9.2/" in name for name in names))
            metadata = ET.fromstring(bundle.read("org/example/merman-android/maven-metadata.xml"))
            self.assertEqual(metadata.findtext("groupId"), "org.example")
            self.assertEqual(
                [entry.text for entry in metadata.findall("versioning/versions/version")],
                ["0.9.3"],
            )
            self.assertEqual(metadata.findtext("versioning/latest"), "0.9.3")
            for algorithm in ("md5", "sha1", "sha256", "sha512"):
                entry = "org/example/merman-android/maven-metadata.xml"
                self.assertEqual(
                    bundle.read(entry + "." + algorithm).decode().strip(),
                    hashlib.new(algorithm, bundle.read(entry)).hexdigest(),
                )
        self.assertEqual(
            archive.with_name(archive.name + ".sha256").read_text().strip(),
            hashlib.sha256(archive.read_bytes()).hexdigest(),
        )

    def test_missing_selected_version_does_not_fall_back_to_another_version(self) -> None:
        self.command[self.command.index("0.9.3")] = "0.9.4"
        self.assertEqual(self.run_package(), 1)
        self.assertFalse(self.output.exists())

    def test_corrupt_sidecar_is_rejected_before_an_archive_is_written(self) -> None:
        aar = self.selected / "merman-android-0.9.3.aar"
        aar.with_name(aar.name + ".sha1").write_text("0" * 40, encoding="ascii")
        self.assertEqual(self.run_package(), 1)
        self.assertFalse(self.output.exists())

    def test_central_bundle_requires_signed_artifacts(self) -> None:
        self.assertEqual(self.run_package("--central-bundle"), 1)
        self.assertFalse(self.output.exists())

    def test_central_bundle_uses_only_the_official_version_directory_layout(self) -> None:
        for path in list(self.selected.iterdir()):
            if path.suffix not in {".aar", ".pom", ".module", ".jar"}:
                continue
            path.with_name(path.name + ".asc").write_bytes(b"signature checked by GPG")
            for algorithm in ("md5", "sha1"):
                path.with_name(path.name + "." + algorithm).write_text(
                    hashlib.new(algorithm, path.read_bytes()).hexdigest(), encoding="ascii",
                )
        with mock.patch.object(
            verifier.subprocess, "run", return_value=subprocess.CompletedProcess([], 0),
        ):
            self.assertEqual(self.run_package("--central-bundle"), 0)
        with zipfile.ZipFile(self.output / "merman-android-0.9.3-central.zip") as bundle:
            self.assertTrue(all(
                name.startswith("org/example/merman-android/0.9.3/")
                for name in bundle.namelist()
            ))
            self.assertTrue(any(name.endswith(".module.asc") for name in bundle.namelist()))

    def test_invalid_gpg_signature_prevents_central_bundle(self) -> None:
        for path in list(self.selected.iterdir()):
            if path.suffix not in {".aar", ".pom", ".module", ".jar"}:
                continue
            path.with_name(path.name + ".asc").write_bytes(b"bad signature")
            for algorithm in ("md5", "sha1"):
                path.with_name(path.name + "." + algorithm).write_text(
                    hashlib.new(algorithm, path.read_bytes()).hexdigest(), encoding="ascii",
                )
        with mock.patch.object(
            verifier.subprocess, "run", side_effect=subprocess.CalledProcessError(1, ["gpg"]),
        ):
            self.assertEqual(self.run_package("--central-bundle"), 1)
        self.assertFalse(self.output.exists())

    def test_signing_preserves_primary_bytes_and_refreshes_signature_checksums(self) -> None:
        before = {
            path.name: path.read_bytes() for path in self.selected.iterdir()
            if path.suffix in {".aar", ".pom", ".module", ".jar"}
        }
        signature = self.selected / "merman-android-0.9.3.aar.asc"
        signature.write_bytes(b"previous detached signature")
        signature.with_name(signature.name + ".sha512").write_text(
            hashlib.sha512(signature.read_bytes()).hexdigest(), encoding="ascii",
        )
        seen_signatures: list[str] = []

        def gpg(command: list[str], **kwargs: object) -> subprocess.CompletedProcess:
            self.assertEqual(command[0], "gpg")
            self.assertNotIn("fixture-passphrase", command)
            if "--detach-sign" in command:
                self.assertEqual(kwargs["input"], "fixture-passphrase\n")
                Path(command[command.index("--output") + 1]).write_bytes(b"new detached signature")
            elif "--verify" in command:
                seen_signatures.append(command[-1])
            return subprocess.CompletedProcess(command, 0)

        with (
            mock.patch.dict(os.environ, {"MERMAN_ANDROID_SIGNING_PASSWORD": "fixture-passphrase"}),
            mock.patch.object(verifier, "resolve_android_llvm_nm", return_value=Path("llvm-nm")),
            mock.patch.object(verifier.subprocess, "run", side_effect=gpg),
        ):
            self.assertEqual(self.run_command([
                "sign", "--repository", str(self.repository), "--group-id", "org.example",
                "--version", "0.9.3", "--signing-key-id", "fixture-fingerprint",
                "--android-ndk-home", "/selected/ndk",
            ]), 0)
        self.assertEqual(len(seen_signatures), 5)
        for name, content in before.items():
            self.assertEqual((self.selected / name).read_bytes(), content)
            self.assertEqual((self.selected / (name + ".asc")).read_bytes(), b"new detached signature")
            for algorithm in ("md5", "sha1", "sha256", "sha512"):
                self.assertEqual(
                    (self.selected / (name + "." + algorithm)).read_text().strip(),
                    hashlib.new(algorithm, content).hexdigest(),
                )
        self.assertEqual(
            signature.with_name(signature.name + ".sha512").read_text().strip(),
            hashlib.sha512(b"new detached signature").hexdigest(),
        )

    def test_public_package_android_root_validates_selected_resource_and_source_data(self) -> None:
        android = self.write_selected_facade()
        with mock.patch.object(verifier.subprocess, "run", side_effect=AssertionError("unexpected process")):
            error = io.StringIO()
            with contextlib.redirect_stderr(error):
                self.assertEqual(self.run_package(), 1)
            self.assertIn("missing projected resources", error.getvalue())
            self.assertFalse(self.output.exists())
            self.assertEqual(self.run_package("--android-root", str(android)), 0)
            with zipfile.ZipFile(self.output / "merman-android-0.9.3-maven.zip") as bundle:
                name = "org/example/merman-android/0.9.3/merman-android-0.9.3-sources.jar"
                self.assertEqual(bundle.read(name), (self.selected / Path(name).name).read_bytes())
            (android / "src" / "main" / "kotlin" / "Unpublished.kt").write_text(
                "class Unpublished\n", encoding="utf-8",
            )
            rejected = self.root / "rejected-package"
            error = io.StringIO()
            with contextlib.redirect_stderr(error):
                self.assertEqual(self.run_package(
                    "--android-root", str(android), "--output-dir", str(rejected),
                ), 1)
            self.assertIn("sources JAR does not match", error.getvalue())
            self.assertFalse(rejected.exists())

    def test_public_sign_android_root_validates_selected_data_before_any_gpg_call(self) -> None:
        android = self.write_selected_facade()
        command = [
            "sign", "--repository", str(self.repository), "--group-id", "org.example",
            "--version", "0.9.3", "--signing-key-id", "fixture-fingerprint",
            "--android-ndk-home", "/selected/ndk",
        ]
        signed: list[str] = []

        def gpg(arguments: list[str], **kwargs: object) -> subprocess.CompletedProcess:
            self.assertEqual(arguments[0], "gpg")
            if "--detach-sign" in arguments:
                signature = Path(arguments[arguments.index("--output") + 1])
                signature.write_bytes(b"fixture signature; no cryptographic operation")
                signed.append(arguments[-1])
            else:
                self.assertIn("--verify", arguments)
            return subprocess.CompletedProcess(arguments, 0)

        with (
            mock.patch.object(verifier, "resolve_android_llvm_nm", return_value=Path("llvm-nm")),
            mock.patch.object(verifier.subprocess, "run", side_effect=gpg) as processes,
        ):
            error = io.StringIO()
            with contextlib.redirect_stderr(error):
                self.assertEqual(self.run_command(command), 1)
            self.assertIn("missing projected resources", error.getvalue())
            processes.assert_not_called()
            self.assertEqual(self.run_command([*command, "--android-root", str(android)]), 0)
            self.assertEqual(len(signed), 5)
            self.assertEqual(processes.call_count, 10)
            (android / "src" / "main" / "resources" / "unpublished-resource.txt").write_text(
                "resource absent from the AAR", encoding="utf-8",
            )
            processes.reset_mock()
            error = io.StringIO()
            with contextlib.redirect_stderr(error):
                self.assertEqual(self.run_command([*command, "--android-root", str(android)]), 1)
            self.assertIn("missing projected resources: unpublished-resource.txt", error.getvalue())
            processes.assert_not_called()


if __name__ == "__main__":
    unittest.main()
