#!/usr/bin/env python3
"""Small, build-free fixtures for the SDK distribution boundary."""

from __future__ import annotations

import hashlib
import io
import json
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import unittest
from unittest import mock
import zipfile

from scripts import sdk_archive


class SdkArchiveTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.repo = self.root / "repo"
        self.repo.mkdir()
        self.artifacts = self.root / "artifacts"
        self.artifacts.mkdir()
        self.output = self.root / "output"
        self.target = "x86_64-unknown-linux-gnu"
        self.profile = "sdk-native-standard"
        self.commit = "a" * 40
        source_mock = mock.patch.object(sdk_archive, "resolve_source_commit", return_value=self.commit)
        source_mock.start()
        self.addCleanup(source_mock.stop)
        clean_mock = mock.patch.object(sdk_archive, "require_clean_source_tree")
        clean_mock.start()
        self.addCleanup(clean_mock.stop)
        self.version = "1.2.3"
        self.write("Cargo.toml", '[workspace.package]\nversion = "1.2.3"\n')
        for name, macro, version in (
            ("merman.h", "MERMAN_NATIVE_ABI_VERSION", 3),
            ("merman_text_measurement_abi.h", "MERMAN_TEXT_MEASUREMENT_PROTOCOL_VERSION", 1),
            ("merman_resource_contract.h", "MERMAN_BINDING_OPTIONS_SCHEMA_VERSION", 3),
        ):
            self.write(f"crates/merman-ffi/include/{name}", f"#define {macro} {version}u\n")
        self.write("contracts/abi/merman-v3.json", '{"abi_version":3}\n')
        self.write("crates/merman-wasi/README.md", "# WASI scalar ABI\n")
        self.write("contracts/abi/text-measurement-v1.json", '{"protocol_version":1}\n')
        for name in ("LICENSE-MIT", "LICENSE-APACHE"):
            self.write(name, "license\n")
        for name in ("README.md", "CMakeLists.txt", "examples/render.c", "cmake/MermanConfig.cmake.in"):
            source = sdk_archive.REPO_ROOT / "distribution/sdk" / name
            self.write(f"distribution/sdk/{name}", source.read_text())
        self.write("distribution/sdk/examples/probe.c", "int main(void) { return 0; }\n")
        self.write("distribution/sdk/examples/render.cpp", "int main() { return 0; }\n")
        for name in ("go.mod", "go.sum", "client.go", "client_test.go", "cmd/render/main.go", "README.md"):
            self.write("distribution/sdk/examples/go-wasi/" + name, "Go example\n")
        for name in ("client.py", "render.py", "test_client.py", "requirements.txt", "README.md"):
            self.write("distribution/sdk/examples/python-wasi/" + name, "Python example\n")
        self.write("crates/merman-export/assets/fonts/OFL.txt", "font license\n")
        self.write("crates/merman-export/assets/fonts/Cousine-Regular.ttf", "font\n")
        self.write("crates/merman-export/assets/fonts/manifest.json", json.dumps({
            "family": "Cousine", "files": [
                {"path": name, "sha256": sdk_archive.sha256_file(
                    self.repo / "crates/merman-export/assets/fonts" / name)}
                for name in ("OFL.txt", "Cousine-Regular.ttf")
            ],
        }))
        self.descriptor = self.repo / "capabilities/artifact-profiles-v2.json"
        self.configure()

    def write(self, relative, text):
        path = self.repo / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")
        return path

    def configure(self, *, wasi=False, flavor="standard", target=None):
        self.target = target or ("wasm32-wasip1" if wasi else self.target)
        self.profile = f"sdk-{'wasi' if wasi else 'native'}-{flavor}"
        caps = ["native-fonts", "png", "svg"]
        if flavor != "standard":
            caps.append("math")
        if flavor == "full":
            caps.append("layout-elk")
        self.expected = {"capabilities": sorted(caps), "runtime_ids": sorted(caps),
                         "outputs": ["png", "svg"], "diagram_families": ["flowchart"]}
        self.write("capabilities/artifact-profiles-v2.json", json.dumps({
            "schema_version": 2, "profiles": [{
                "id": self.profile, "semantic_target": "native",
                "cargo": {
                    "package": "merman-wasi" if wasi else "merman-ffi",
                    "manifest": "crates/merman-wasi/Cargo.toml" if wasi else "crates/merman-ffi/Cargo.toml",
                    "profile": "native-sdk", "default_features": False,
                    "features": ["native-fonts", "png", "svg"],
                    "target": {"name": "merman_wasi" if wasi else "merman_ffi",
                               "kinds": ["cdylib"] if wasi else ["cdylib", "staticlib"],
                               "crate_types": ["cdylib"] if wasi else ["cdylib", "staticlib"],
                               "required_features": []},
                    "build_target": {"kind": "target-set", "triples": [self.target]},
                }, "expected": self.expected,
            }],
        }))
        components = []
        for name in ("sdk-cousine", "roxmltree-dom", "ratex", "eclipse-elk", "mermaid-ascii"):
            path = self.write(f"THIRD_PARTY_LICENSES/{name}/{'OFL.txt' if name == 'sdk-cousine' else 'LICENSE'}", name + " license\n")
            components.append({"id": name, "name": name, "notice": name, "version": "1.0",
                               "source": {"repository": "https://example.org/source", "commit": self.commit, "path": "src"},
                               "relationships": ["linked"], "license_expression": "MPL-2.0",
                               "license_files": [{"path": path.relative_to(self.repo).as_posix(),
                                                  "sha256": sdk_archive.sha256_file(path)}]})
        self.write("docs/release/THIRD_PARTY_COMPONENTS.json", json.dumps({
            "components": components,
            "artifact_scopes": [
                {"id": "sdk-base", "extends": [], "components": ["sdk-cousine", "roxmltree-dom"]},
                {"id": "sdk-standard", "extends": ["sdk-base"], "components": []},
                {"id": "sdk-math", "extends": ["sdk-standard"], "components": ["ratex"]},
                {"id": "sdk-full", "extends": ["sdk-math"], "components": ["eclipse-elk"]},
                {"id": "ascii-render", "extends": [], "components": ["mermaid-ascii"]},
                *[{"id": f"sdk-native-{variant}", "extends": [f"sdk-{variant}", "ascii-render"], "components": []}
                  for variant in ("standard", "math", "full")],
            ],
        }))
        projection = sdk_archive.load_artifact_profiles(self.descriptor)[0].report_projection()
        self.report = self.artifacts / "rust-cargo-dependencies.json"
        self.report.write_text(json.dumps({"artifact_profile": projection,
                                          "licenses": [{"id": "MPL-2.0", "text": "Full MPL text"}]}))
        for name in sdk_archive.library_names(self.target).values():
            (self.artifacts / name).write_bytes(b"small built artifact\n")
        self.catalog = {
            "schema_version": 1, "profile": self.profile, "target": self.target,
            "source_commit": self.commit,
            "artifacts": {name: sdk_archive.sha256_file(self.artifacts / name)
                          for name in sdk_archive.library_names(self.target).values()},
            "runtime_catalog": {
                "schema_version": 1, "transport_api_version": 1 if wasi else 3,
                "package_version": self.version, "options_schema_versions": [3],
                "capabilities": {"capability_ids": sorted(caps), "output_ids": ["png", "svg"],
                                 "text_measurement": {"provider_ids": ["native-fonts"]}},
                "constructor_service_ids": ["native-font-context"],
                "registry": {"diagram_families": ["flowchart"]},
            },
        }
        self.save_catalog()

    def save_catalog(self):
        self.catalog_path = self.artifacts / "runtime-catalog.json"
        self.catalog_path.write_text(json.dumps(self.catalog), encoding="utf-8")

    def package(self, **changes):
        args = dict(repo_root=self.repo, profile=self.profile, target=self.target,
                    artifact_dir=self.artifacts, output_dir=self.output,
                    runtime_catalog=self.catalog_path, source_commit=self.commit,
                    descriptor=self.descriptor)
        args.update(changes)
        return sdk_archive.package_sdk(**args)

    def members(self, path):
        if path.suffix == ".zip":
            with zipfile.ZipFile(path) as archive:
                return {name: archive.read(name) for name in archive.namelist()}
        with tarfile.open(path) as archive:
            return {member.name.split("/", 1)[1]: archive.extractfile(member).read()
                    for member in archive if member.isfile()}

    def test_manifest_checksums_and_explicit_public_payload(self):
        (self.artifacts / "test_fixture.txt").write_text("do not ship")
        archive = self.package()
        members = self.members(archive)
        manifest = json.loads(members["manifest.json"])
        self.assertEqual(manifest["version"], self.version)
        self.assertEqual(manifest["profile"], self.profile)
        self.assertEqual(manifest["source_commit"], self.commit)
        self.assertEqual(manifest["abi_version"], 3)
        self.assertEqual(manifest["capabilities"], self.expected["runtime_ids"])
        self.assertEqual(set(manifest["files"]), set(members) - {"manifest.json"})
        for name, details in manifest["files"].items():
            self.assertEqual(details["sha256"], hashlib.sha256(members[name]).hexdigest())
            self.assertEqual(details["size"], len(members[name]))
        self.assertNotIn("test_fixture.txt", members)
        self.assertIn("contracts/abi/merman-v3.json", members)
        self.assertIn("contracts/abi/text-measurement-v1.json", members)
        self.assertIn("include/merman_text_measurement_abi.h", members)
        self.assertIn("examples/render.cpp", members)
        self.assertIn("lib/cmake/Merman/MermanConfig.cmake", members)
        self.assertIn("lib/pkgconfig/merman.pc", members)
        self.assertIn("THIRD_PARTY_LICENSES/roxmltree-dom/LICENSE", members)
        self.assertIn(b"Full MPL text", members["THIRD_PARTY_LICENSES/rust-cargo-dependencies.json"])
        self.assertNotIn("THIRD_PARTY_LICENSES/eclipse-elk/LICENSE", members)
        self.assertEqual(sdk_archive.read_checksum(Path(str(archive) + ".sha256"), archive.name),
                         sdk_archive.sha256_file(archive))

    def test_wasi_module_protocol_fonts_and_no_native_payload(self):
        self.configure(wasi=True)
        archive = self.package()
        members = self.members(archive)
        self.assertEqual(members["lib/merman_wasi.wasm"],
                         (self.artifacts / "merman_wasi.wasm").read_bytes())
        self.assertEqual(members["docs/WASI_ABI.md"],
                         (self.repo / "crates/merman-wasi/README.md").read_bytes())
        self.assertEqual(json.loads(members["runtime-catalog.json"]), self.catalog)
        self.assertEqual(json.loads(members["manifest.json"])["abi_version"], 1)
        self.assertFalse(any(name.startswith(("contracts/abi/", "include/")) for name in members))
        self.assertIn("fonts/manifest.json", members)
        self.assertNotIn("examples/render.c", members)
        self.assertIn("examples/go-wasi/client.go", members)
        self.assertIn("examples/go-wasi/cmd/render/main.go", members)
        self.assertIn("examples/python-wasi/client.py", members)
        self.assertIn("examples/python-wasi/render.py", members)
        self.assertEqual(sdk_archive.verify_sdk_archive(
            repo_root=self.repo, profile=self.profile, target=self.target,
            archive=archive, checksum=Path(str(archive) + ".sha256"),
            source_commit=self.commit, version=self.version, descriptor=self.descriptor,
        ), archive.resolve())

    def test_wasi_catalog_version_and_module_digest_are_required(self):
        self.configure(wasi=True)
        for version in (None, True, 3):
            with self.subTest(version=version):
                self.catalog["runtime_catalog"]["transport_api_version"] = version
                self.save_catalog()
                with self.assertRaisesRegex(sdk_archive.ArchiveVerificationError, "transport_api_version"):
                    self.package()
        self.catalog["runtime_catalog"]["transport_api_version"] = 1
        self.save_catalog()
        (self.artifacts / "merman_wasi.wasm").write_bytes(b"replaced module")
        with self.assertRaisesRegex(sdk_archive.ArchiveVerificationError, "SHA-256 digests mismatch"):
            self.package()

    def test_native_staging_and_archives_share_integration_files_and_library_layout(self):
        for target in sdk_archive.NATIVE_TARGETS:
            with self.subTest(target=target):
                self.configure(target=target)
                members = self.members(self.package(output_dir=self.output / target))
                files = sdk_archive.native_consumer_files(self.repo, target, self.version)
                self.assertTrue(files)
                for name, data in files.items():
                    self.assertEqual(members[name], data, name)
                names = sdk_archive.library_names(target)
                for kind, relative in sdk_archive.library_layout(target).items():
                    self.assertEqual(members[relative], (self.artifacts / names[kind]).read_bytes())
        with self.assertRaisesRegex(sdk_archive.ArchiveVerificationError, "native SDK target"):
            sdk_archive.native_consumer_files(self.repo, sdk_archive.WASI_TARGET, self.version)

    def test_all_eighteen_profile_target_combinations(self):
        for target in (*sdk_archive.NATIVE_TARGETS, sdk_archive.WASI_TARGET):
            for flavor in ("standard", "math", "full"):
                with self.subTest(target=target, flavor=flavor):
                    self.configure(wasi=target == sdk_archive.WASI_TARGET, flavor=flavor, target=target)
                    archive = self.package(output_dir=self.output / self.profile / target)
                    members = self.members(archive)
                    manifest = json.loads(members["manifest.json"])
                    self.assertEqual(manifest["target"], target)
                    self.assertEqual(manifest["profile"], self.profile)
                    self.assertEqual("math" in manifest["capabilities"], flavor != "standard")
                    self.assertEqual("layout-elk" in manifest["capabilities"], flavor == "full")
                    self.assertEqual("THIRD_PARTY_LICENSES/eclipse-elk/LICENSE" in members, flavor == "full")
                    self.assertEqual("THIRD_PARTY_LICENSES/ratex/LICENSE" in members, flavor != "standard")
                    self.assertIn("THIRD_PARTY_LICENSES/roxmltree-dom/LICENSE", members)
                    self.assertEqual("THIRD_PARTY_LICENSES/mermaid-ascii/LICENSE" in members,
                                     target != sdk_archive.WASI_TARGET)
                    if "windows" in target:
                        self.assertEqual(archive.suffix, ".zip")
                        self.assertIn("bin/merman_ffi.dll", members)
                        self.assertIn("lib/merman_ffi.dll.lib", members)
                        self.assertIn(b"-lmerman_ffi.dll", members["lib/pkgconfig/merman.pc"])
                        self.assertNotIn(b"-lmerman_ffi.dll", members["lib/pkgconfig/merman-static.pc"])
                    if flavor == "full":
                        self.assertIn(self.commit.encode(), members["SOURCE_AVAILABILITY.md"])
                        self.assertIn(b"crates/merman-layout-elk", members["SOURCE_AVAILABILITY.md"])

    def test_archives_follow_shared_safe_extraction_conventions(self):
        from scripts.release_archive import verified_archive_contents
        for target in (self.target, "x86_64-pc-windows-msvc"):
            with self.subTest(target=target):
                self.configure(target=target)
                archive = self.package(output_dir=self.output / target)
                with verified_archive_contents(
                    archive, Path(str(archive) + ".sha256"), package_name=f"merman-{self.profile}",
                    target=target, version=self.version,
                ) as extracted:
                    self.assertEqual(json.loads((extracted.root / "manifest.json").read_bytes())["profile"], self.profile)
                    self.assertTrue((extracted.root / "include/merman.h").is_file())

    def test_archive_bytes_are_reproducible_and_outputs_are_not_replaced(self):
        first = self.package()
        second = self.package(output_dir=self.root / "second-output")
        self.assertEqual(first.read_bytes(), second.read_bytes())
        with self.assertRaisesRegex(sdk_archive.ArchiveVerificationError, "already exists"):
            self.package()

    def test_report_wrong_profile_or_missing_license_text_is_rejected(self):
        document = json.loads(self.report.read_text())
        for field, value in (("artifact_profile", {}), ("licenses", []),
                             ("licenses", [{"id": "MPL-2.0"}])):
            with self.subTest(field=field, value=value):
                self.report.write_text(json.dumps({**document, field: value}))
                with self.assertRaises(sdk_archive.ArchiveVerificationError):
                    self.package()

    def test_catalog_missing_outputs_families_and_schema_are_rejected(self):
        raw = json.loads(json.dumps(self.catalog))
        for branch, field, value in (
            ("capabilities", "output_ids", []), ("registry", "diagram_families", []),
            (None, "transport_api_version", True), (None, "options_schema_versions", []),
        ):
            with self.subTest(field=field):
                self.catalog = json.loads(json.dumps(raw))
                destination = self.catalog["runtime_catalog"]
                if branch:
                    destination = destination[branch]
                destination[field] = value
                self.save_catalog()
                with self.assertRaises(sdk_archive.ArchiveVerificationError):
                    self.package()

    def test_catalog_outside_input_root_and_symlinked_parent_are_rejected(self):
        outside = self.root / "runtime-catalog.json"
        outside.write_bytes(self.catalog_path.read_bytes())
        with self.assertRaisesRegex(sdk_archive.ArchiveVerificationError, "inside artifact-dir"):
            self.package(runtime_catalog=outside)
        (self.artifacts / "linked").symlink_to(self.root, target_is_directory=True)
        with self.assertRaisesRegex(sdk_archive.ArchiveVerificationError, "symlink"):
            self.package(runtime_catalog=self.artifacts / "linked/runtime-catalog.json")

    def test_catalog_and_scope_reject_duplicate_keys_and_unsafe_members(self):
        self.catalog_path.write_text('{"schema_version":1,"schema_version":1}')
        with self.assertRaisesRegex(sdk_archive.ArchiveVerificationError, "duplicate"):
            self.package()
        self.save_catalog()
        contract = self.repo / "docs/release/THIRD_PARTY_COMPONENTS.json"
        document = json.loads(contract.read_text())
        document["components"][0]["license_files"][0]["path"] = "THIRD_PARTY_LICENSES/../secret"
        contract.write_text(json.dumps(document))
        with self.assertRaisesRegex(sdk_archive.ArchiveVerificationError, "traversal"):
            self.package()

    def test_abi_contract_and_font_checksums_are_required(self):
        abi = self.repo / "contracts/abi/merman-v3.json"
        abi.write_text('{"abi_version":99}')
        with self.assertRaisesRegex(sdk_archive.ArchiveVerificationError, "ABI contract"):
            self.package()
        abi.write_text('{"abi_version":3}')
        font = self.repo / "crates/merman-export/assets/fonts/Cousine-Regular.ttf"
        font.write_text("changed")
        with self.assertRaisesRegex(sdk_archive.ArchiveVerificationError, "font checksum"):
            self.package()

    def test_public_cli_build_command_does_not_execute_cargo(self):
        result = subprocess.run([
            sys.executable, str(sdk_archive.REPO_ROOT / "scripts/sdk_archive.py"),
            "build-command", "--profile", self.profile, "--target", self.target,
            "--repo-root", str(self.repo), "--descriptor", str(self.descriptor),
        ], check=True, text=True, capture_output=True)
        command = json.loads(result.stdout)
        self.assertEqual(command[:2], ["cargo", "build"])
        self.assertEqual(command[command.index("--target") + 1], self.target)

    def test_catalog_helper_binds_raw_runtime_to_source_and_product_bytes(self):
        raw = self.artifacts / "raw-catalog.json"
        raw.write_text(json.dumps(self.catalog["runtime_catalog"]))
        self.catalog_path.unlink()
        output = sdk_archive.write_catalog_sidecar(
            repo_root=self.repo, profile=self.profile, target=self.target,
            artifact_dir=self.artifacts, catalog_in=raw, descriptor=self.descriptor,
            source_commit=self.commit,
        )
        sidecar = json.loads(output.read_bytes())
        self.assertEqual(sidecar, self.catalog)
        self.assertIn("manifest.json", self.members(self.package()))
        with self.assertRaisesRegex(sdk_archive.ArchiveVerificationError, "already exists"):
            sdk_archive.write_catalog_sidecar(
                repo_root=self.repo, profile=self.profile, target=self.target,
                artifact_dir=self.artifacts, catalog_in=raw, descriptor=self.descriptor,
            )

    def test_catalog_helper_rejects_feature_guesses_and_escaped_output(self):
        raw = self.artifacts / "raw-catalog.json"
        raw.write_text(json.dumps(self.catalog["runtime_catalog"]))
        with self.assertRaisesRegex(sdk_archive.ArchiveVerificationError, "inside artifact-dir"):
            sdk_archive.write_catalog_sidecar(
                repo_root=self.repo, profile=self.profile, target=self.target,
                artifact_dir=self.artifacts, catalog_in=raw, descriptor=self.descriptor,
                output=self.root / "escaped.json",
            )
        raw.write_text(json.dumps({"features": ["png", "svg"]}))
        with self.assertRaisesRegex(sdk_archive.ArchiveVerificationError, "runtime catalog"):
            sdk_archive.write_catalog_sidecar(
                repo_root=self.repo, profile=self.profile, target=self.target,
                artifact_dir=self.artifacts, catalog_in=raw, descriptor=self.descriptor,
                output=self.artifacts / "sidecar.json",
            )

    def test_header_version_accepts_lf_and_crlf(self):
        for newline in (b"\n", b"\r\n"):
            with self.subTest(newline=newline):
                header = newline.join((
                    b"/* Generated header */",
                    b"#define MERMAN_NATIVE_ABI_VERSION 3u",
                    b"",
                ))
                self.assertEqual(
                    sdk_archive._header_version(header, "MERMAN_NATIVE_ABI_VERSION"), 3,
                )

    def test_header_version_mismatch_is_rejected(self):
        self.write("crates/merman-ffi/include/merman.h", "#define MERMAN_NATIVE_ABI_VERSION 99u\n")
        with self.assertRaisesRegex(sdk_archive.ArchiveVerificationError, "header ABI"):
            self.package()

    def test_font_capability_still_requires_its_provider_and_constructor(self):
        self.assertIn("native-fonts", self.catalog["runtime_catalog"]["capabilities"]["capability_ids"])
        for field in ("provider", "constructor"):
            with self.subTest(field=field):
                self.configure()
                if field == "provider":
                    self.catalog["runtime_catalog"]["capabilities"]["text_measurement"]["provider_ids"] = []
                else:
                    self.catalog["runtime_catalog"]["constructor_service_ids"] = []
                self.save_catalog()
                with self.assertRaisesRegex(sdk_archive.ArchiveVerificationError, "must expose"):
                    self.package()

    def test_verifier_persists_exact_checksum_bound_snapshot(self):
        archive = self.package()
        verified = self.root / "verified"
        verified.mkdir()
        destination = sdk_archive.verify_sdk_archive(
            repo_root=self.repo, profile=self.profile, target=self.target,
            archive=archive, checksum=Path(str(archive) + ".sha256"),
            source_commit=self.commit, version=self.version, descriptor=self.descriptor,
            verified_output_dir=verified,
        )
        self.assertEqual(destination.read_bytes(), archive.read_bytes())
        self.assertEqual(sdk_archive.read_checksum(Path(str(destination) + ".sha256"), destination.name),
                         sdk_archive.sha256_file(destination))

    def test_verifier_rejects_tag_version_and_source_mismatch(self):
        archive = self.package()
        for version, commit in (("9.9.9", self.commit), (self.version, "b" * 40)):
            with self.subTest(version=version, commit=commit):
                with mock.patch.object(sdk_archive, "resolve_source_commit", return_value=commit), self.assertRaises(sdk_archive.ArchiveVerificationError):
                    sdk_archive.verify_sdk_archive(
                        repo_root=self.repo, profile=self.profile, target=self.target,
                        archive=archive, checksum=Path(str(archive) + ".sha256"),
                        source_commit=commit, version=version, descriptor=self.descriptor,
                    )

    def test_verifier_checks_payload_hashes_even_if_archive_checksum_is_replaced(self):
        archive = self.package()
        with tarfile.open(archive) as source:
            members = [(info, source.extractfile(info).read()) for info in source if info.isfile()]
        with tarfile.open(archive, "w:xz") as output:
            for info, data in members:
                if info.name.endswith("/README.md"):
                    data = b"tampered payload"
                    info.size = len(data)
                output.addfile(info, io.BytesIO(data))
        checksum = Path(str(archive) + ".sha256")
        checksum.write_text(f"{sdk_archive.sha256_file(archive)}  {archive.name}\n")
        with self.assertRaisesRegex(sdk_archive.ArchiveVerificationError, "payload checksum"):
            sdk_archive.verify_sdk_archive(
                repo_root=self.repo, profile=self.profile, target=self.target,
                archive=archive, checksum=checksum,
                source_commit=self.commit, version=self.version, descriptor=self.descriptor,
            )

    def test_missing_required_inputs_fail_closed(self):
        for path in (self.artifacts / "libmerman_ffi.a", self.catalog_path,
                     self.repo / "crates/merman-ffi/include/merman.h",
                     self.report):
            with self.subTest(path=path):
                original = path.read_bytes()
                path.unlink()
                with self.assertRaises(sdk_archive.ArchiveVerificationError):
                    self.package()
                path.write_bytes(original)
        self.assertFalse(self.output.exists())

    def test_catalog_identity_and_actual_capabilities_are_required(self):
        for field, wrong in (("profile", "sdk-native-full"), ("target", "aarch64-apple-darwin"),
                             ("source_commit", "b" * 40)):
            with self.subTest(field=field):
                original = self.catalog[field]
                self.catalog[field] = wrong
                self.save_catalog()
                with self.assertRaises(sdk_archive.ArchiveVerificationError):
                    self.package()
                self.catalog[field] = original
        self.catalog["runtime_catalog"]["capabilities"]["capability_ids"].append("math")
        self.save_catalog()
        with self.assertRaises(sdk_archive.ArchiveVerificationError):
            self.package()

    def test_version_and_library_digest_mismatch(self):
        self.catalog["runtime_catalog"]["package_version"] = "9.9.9"
        self.save_catalog()
        with self.assertRaises(sdk_archive.ArchiveVerificationError):
            self.package()
        self.catalog["runtime_catalog"]["package_version"] = self.version
        self.save_catalog()
        (self.artifacts / "libmerman_ffi.a").write_bytes(b"replaced")
        with self.assertRaises(sdk_archive.ArchiveVerificationError):
            self.package()

    def test_unsafe_members_and_symlinks(self):
        font_manifest = self.repo / "crates/merman-export/assets/fonts/manifest.json"
        for name in ("../escape.ttf", "/escape", "C:/escape", "a\\b", "NUL", "tests/fixture.ttf"):
            with self.subTest(name=name):
                font_manifest.write_text(json.dumps({"files": [{"path": name, "sha256": "0" * 64}]}))
                with self.assertRaises(sdk_archive.ArchiveVerificationError):
                    self.package()
        self.configure()
        library = self.artifacts / "libmerman_ffi.a"
        library.unlink()
        library.symlink_to(self.catalog_path)
        with self.assertRaises(sdk_archive.ArchiveVerificationError):
            self.package()

    def test_build_command_only_constructs_exact_recipe(self):
        command = sdk_archive.build_command(self.profile, self.target, self.descriptor, self.repo)
        self.assertIn("--locked", command)
        self.assertIn("--no-default-features", command)
        self.assertEqual(command[command.index("--features") + 1], "native-fonts,png,svg")
        with self.assertRaises(sdk_archive.ArchiveVerificationError):
            sdk_archive.build_command(self.profile, "../target", self.descriptor, self.repo)


class SourceIdentityTests(unittest.TestCase):
    def test_source_is_derived_from_head_not_caller_labels(self):
        commit = "a" * 40
        with mock.patch.object(sdk_archive.subprocess, "run", return_value=mock.Mock(
            returncode=0, stdout=commit + "\n", stderr="",
        )) as run:
            self.assertEqual(sdk_archive.resolve_source_commit(sdk_archive.REPO_ROOT, commit), commit)
            self.assertEqual(run.call_args.args[0][-1], "HEAD^{commit}")
            with self.assertRaisesRegex(sdk_archive.ArchiveVerificationError, "does not match"):
                sdk_archive.resolve_source_commit(sdk_archive.REPO_ROOT, "b" * 40)
        with self.assertRaisesRegex(sdk_archive.ArchiveVerificationError, "40-character"):
            sdk_archive.resolve_source_commit(sdk_archive.REPO_ROOT, "v1.2.3")

    def test_unavailable_git_identity_fails_closed(self):
        with mock.patch.object(sdk_archive.subprocess, "run", return_value=mock.Mock(
            returncode=128, stdout="", stderr="not a repository",
        )):
            with self.assertRaisesRegex(sdk_archive.ArchiveVerificationError, "cannot resolve"):
                sdk_archive.resolve_source_commit(sdk_archive.REPO_ROOT, None)

    def test_source_tree_check_rejects_tracked_and_untracked_changes(self):
        for status in (" M crates/merman/src/lib.rs\n", "?? crates/merman/src/new.rs\n"):
            with self.subTest(status=status), mock.patch.object(
                sdk_archive.subprocess, "run", return_value=mock.Mock(
                    returncode=0, stdout=status, stderr="",
                ),
            ) as run:
                with self.assertRaisesRegex(sdk_archive.ArchiveVerificationError, "uncommitted changes"):
                    sdk_archive.require_clean_source_tree(sdk_archive.REPO_ROOT)
                self.assertIn("--untracked-files=all", run.call_args.args[0])

    def test_source_tree_check_fails_closed_when_git_is_unavailable(self):
        with mock.patch.object(sdk_archive.subprocess, "run", return_value=mock.Mock(
            returncode=0, stdout="", stderr="",
        )):
            sdk_archive.require_clean_source_tree(sdk_archive.REPO_ROOT)
        with mock.patch.object(sdk_archive.subprocess, "run", return_value=mock.Mock(
            returncode=128, stdout="", stderr="not a repository",
        )):
            with self.assertRaisesRegex(sdk_archive.ArchiveVerificationError, "cannot verify"):
                sdk_archive.require_clean_source_tree(sdk_archive.REPO_ROOT)


if __name__ == "__main__":
    unittest.main()
