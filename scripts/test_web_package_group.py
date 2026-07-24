#!/usr/bin/env python3
"""Tests for the Web package-group artifact and promotion state machine."""

from __future__ import annotations

import io
import hashlib
import json
import tarfile
import tempfile
import unittest
from pathlib import Path

from scripts import web_package_group


VERSION = "0.8.0-alpha.4"
SOURCE_SHA = "0123456789abcdef0123456789abcdef01234567"


def descriptor() -> dict:
    return {
        "schema_version": 1,
        "default_package": "full",
        "packages": [
            {
                "id": "full",
                "name": "@mermanjs/web",
                "package_dir": "packages/full",
                "artifact_profile": "web-full",
                "runtime_profile": "full",
                "visibility": "public",
            },
            {
                "id": "editor",
                "name": "@mermanjs/web-editor",
                "package_dir": "packages/editor",
                "artifact_profile": "web-editor",
                "runtime_profile": "editor",
                "visibility": "public",
            },
            {
                "id": "render",
                "name": "@mermanjs/web-render",
                "package_dir": "packages/render",
                "artifact_profile": "web-render",
                "runtime_profile": "render",
                "visibility": "candidate",
            },
        ],
    }


def write_json(path: Path, value: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value), encoding="utf-8")


def package_manifest(
    entry: dict,
    *,
    private: bool = False,
    entrypoint_id: str | None = None,
    extra_file_root: bool = False,
    lifecycle_script: bool = False,
    publish_registry: bool = False,
    bundled_dependency: bool = False,
) -> dict:
    package_id = entrypoint_id or entry["id"]
    entrypoint = f"./dist/package-entries/{package_id}"
    result = {
        "name": entry["name"],
        "version": VERSION,
        "main": entrypoint + ".js",
        "types": entrypoint + ".d.ts",
        "exports": {".": {"import": entrypoint + ".js", "types": entrypoint + ".d.ts"}},
        "merman": {"artifact_profile": entry["artifact_profile"]},
        "files": ["artifacts", "dist", "README.md", "LICENSE", "THIRD_PARTY_LICENSES", "THIRD_PARTY_NOTICES.md"],
    }
    if private:
        result["private"] = True
    else:
        result["publishConfig"] = {"access": "public"}
    if extra_file_root:
        result["files"].append("secret.txt")
    if lifecycle_script:
        result["scripts"] = {"postpack": "node postinstall.js"}
    if publish_registry:
        result["publishConfig"] = {
            "access": "public",
            "registry": "https://attacker.invalid",
        }
    if bundled_dependency:
        result["bundleDependencies"] = ["unexpected"]
    return result


def populate_workspace(root: Path, data: dict) -> Path:
    write_json(root / "platforms/web/package.json", {"name": "@mermanjs/web-workspace", "private": True})
    descriptor_path = root / "platforms/web/web-surface-descriptor.json"
    write_json(descriptor_path, data)
    for entry in data["packages"]:
        write_json(
            root / "platforms/web" / entry["package_dir"] / "package.json",
            package_manifest(entry, private=entry["visibility"] == "candidate"),
        )
    return descriptor_path


def write_tarball(
    path: Path,
    entry: dict,
    *,
    second_wasm: bool = False,
    legacy: bool = False,
    unsafe_member: str | None = None,
    duplicate_member: bool = False,
    symlink: bool = False,
    include_legal: bool = True,
    provenance_id: str | None = None,
    entrypoint_id: str | None = None,
    artifact_entrypoint_id: str | None = None,
    tamper_provenance_sha: bool = False,
    omit_provenance_file: bool = False,
    unlisted_dist_file: bool = False,
    foreign_package_entry: bool = False,
    full_sized_dist: bool = False,
    unexpected_wasm_top_level: bool = False,
    unexpected_wasm_nested: bool = False,
    extra_top_level_file: bool = False,
    extra_manifest_file_root: bool = False,
    lifecycle_script: bool = False,
    publish_registry: bool = False,
    bundled_dependency: bool = False,
) -> None:
    artifact_id = artifact_entrypoint_id or entry["id"]
    artifact_files = [
        ("package/artifacts/wasm/merman_wasm.js", b"wasm glue"),
        ("package/artifacts/wasm/merman_wasm.d.ts", b"wasm declarations"),
        ("package/artifacts/wasm/merman_wasm_bg.wasm", b"wasm bytes"),
        ("package/artifacts/wasm/merman_wasm_bg.wasm.d.ts", b"wasm declarations"),
        (f"package/dist/package-entries/{artifact_id}.d.ts", b"entry declarations"),
        (f"package/dist/package-entries/{artifact_id}.d.ts.map", b"{}"),
        (f"package/dist/package-entries/{artifact_id}.js", b"entry module"),
        (f"package/dist/package-entries/{artifact_id}.js.map", b"{}"),
        ("package/dist/index.d.ts", b"export {};"),
        ("package/dist/index.d.ts.map", b"{}"),
        ("package/dist/index.js", b"export {};"),
        ("package/dist/index.js.map", b"{}"),
        ("package/dist/shared/runtime.js", b"export const runtime = true;"),
    ]
    if entry["id"] == "full" or full_sized_dist:
        artifact_files.append(("package/dist/full-runtime.js", b"x" * 4_096))
    provenance_files = [
        {
            "path": name.removeprefix("package/"),
            "bytes": len(contents),
            "sha256": "sha256:" + hashlib.sha256(contents).hexdigest(),
        }
        for name, contents in sorted(artifact_files)
    ]
    if tamper_provenance_sha:
        provenance_files[0]["sha256"] = "sha256:" + "0" * 64
    if omit_provenance_file:
        provenance_files.pop()
    provenance = {
        "schema_version": 2,
        "package": {
            "id": provenance_id or entry["id"],
            "name": entry["name"],
            "version": VERSION,
        },
        "artifact_profile": entry["artifact_profile"],
        "artifact_files": provenance_files,
    }
    files = [
        (
            "package/package.json",
            json.dumps(
                package_manifest(
                    entry,
                    entrypoint_id=entrypoint_id,
                    extra_file_root=extra_manifest_file_root,
                    lifecycle_script=lifecycle_script,
                    publish_registry=publish_registry,
                    bundled_dependency=bundled_dependency,
                )
            ).encode("utf-8"),
        ),
        ("package/README.md", b"# package\n"),
        ("package/LICENSE", b"Apache-2.0 OR MIT\n"),
        ("package/THIRD_PARTY_NOTICES.md", b"notice text\n"),
        ("package/artifacts/provenance.json", json.dumps(provenance).encode("utf-8")),
        *artifact_files,
    ]
    if unlisted_dist_file:
        files.append(("package/dist/unlisted-static-import.js", b"export {};"))
    if foreign_package_entry:
        files.append(("package/dist/package-entries/editor.js", b"export {};"))
    if unexpected_wasm_top_level:
        files.append(("package/artifacts/wasm/unexpected.js", b"export {};"))
    if unexpected_wasm_nested:
        files.append(("package/artifacts/wasm/not-snippets/nested.js", b"export {};"))
    if extra_top_level_file:
        files.append(("package/secret.txt", b"not part of the published surface"))
    if include_legal:
        files.append(("package/THIRD_PARTY_LICENSES/demo/LICENSE", b"license text\n"))
    if second_wasm:
        files.append(("package/other.wasm", b"duplicate"))
    if legacy:
        files.append(("package/pkg/merman_wasm_bg.wasm", b"legacy"))
    if unsafe_member is not None:
        files.append((unsafe_member, b"unsafe"))
    if duplicate_member:
        files.append(("package/README.md", b"duplicate"))
    with tarfile.open(path, "w:gz") as archive:
        for name, contents in files:
            info = tarfile.TarInfo(name)
            info.size = len(contents)
            archive.addfile(info, io.BytesIO(contents))
        if symlink:
            link = tarfile.TarInfo("package/link")
            link.type = tarfile.SYMTYPE
            link.linkname = "README.md"
            archive.addfile(link)


class FakeNpm:
    def __init__(
        self,
        *,
        fail_tag_for: str | None = None,
        fail_observe_after_add_for: str | None = None,
    ) -> None:
        self.versions: dict[tuple[str, str], str] = {}
        self.tags: dict[tuple[str, str], str] = {}
        self.published: list[tuple[str, str]] = []
        self.fail_tag_for = fail_tag_for
        self.fail_observe_after_add_for = fail_observe_after_add_for
        self.added_tags: set[tuple[str, str]] = set()
        self.did_fail_observe = False

    def version_integrity(self, package: str, version: str) -> str | None:
        return self.versions.get((package, version))

    def dist_tag(self, package: str, tag: str) -> str | None:
        if (
            package == self.fail_observe_after_add_for
            and (package, tag) in self.added_tags
            and not self.did_fail_observe
        ):
            self.did_fail_observe = True
            raise web_package_group.PackageGroupError("simulated post-promotion lookup failure")
        return self.tags.get((package, tag))

    def publish(self, tarball: Path, tag: str) -> None:
        del tag
        record = next(item for item in self.manifest["packages"] if item["tarball"] == tarball.name)
        self.versions[(record["name"], self.manifest["version"])] = record["integrity"]
        self.published.append((record["name"], self.manifest["version"]))

    def add_tag(self, package: str, version: str, tag: str) -> None:
        if package == self.fail_tag_for:
            raise web_package_group.PackageGroupError("simulated tag failure")
        self.tags[(package, tag)] = version
        self.added_tags.add((package, tag))

    def remove_tag(self, package: str, tag: str) -> None:
        self.tags.pop((package, tag), None)


class WebPackageDescriptorTests(unittest.TestCase):
    def test_schema_is_closed_and_candidates_cannot_be_default(self) -> None:
        data = descriptor()
        self.assertEqual(web_package_group.validate_descriptor(data), data)

        old = descriptor()
        old["presets"] = []
        with self.assertRaisesRegex(web_package_group.PackageGroupError, "fields must be exact"):
            web_package_group.validate_descriptor(old)

        candidate_default = descriptor()
        candidate_default["default_package"] = "render"
        with self.assertRaisesRegex(web_package_group.PackageGroupError, "must be public"):
            web_package_group.validate_descriptor(candidate_default)

    def test_candidate_manifest_must_be_private(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            data = descriptor()
            descriptor_path = populate_workspace(root, data)
            render = next(entry for entry in data["packages"] if entry["id"] == "render")
            write_json(root / "platforms/web" / render["package_dir"] / "package.json", package_manifest(render))
            with self.assertRaisesRegex(web_package_group.PackageGroupError, "candidate packages must be private"):
                web_package_group.validate_package_manifest(
                    render,
                    root / "platforms/web" / render["package_dir"],
                    expected_version=VERSION,
                )


class WebPackageArtifactTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.data = descriptor()
        self.descriptor_path = populate_workspace(self.root, self.data)
        self.artifacts = self.root / "artifacts"
        self.artifacts.mkdir()
        for entry in web_package_group.public_packages(self.data):
            filename = entry["name"].replace("@", "").replace("/", "-") + ".tgz"
            write_tarball(self.artifacts / filename, entry)

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def create_manifest(self) -> Path:
        return web_package_group.create_manifest(
            self.root,
            self.descriptor_path,
            self.artifacts,
            version=VERSION,
            source_sha=SOURCE_SHA,
            target_dist_tag="alpha",
        )

    def test_manifest_covers_only_public_packages_and_verifies_tarballs(self) -> None:
        path = self.create_manifest()
        manifest = web_package_group.verify_artifact(
            path,
            self.artifacts,
            expected_version=VERSION,
            descriptor=web_package_group.load_descriptor(self.descriptor_path),
        )
        self.assertEqual([item["id"] for item in manifest["packages"]], ["full", "editor"])
        self.assertNotIn("render", {item["id"] for item in manifest["packages"]})

    def test_tarball_rejects_second_wasm_and_legacy_pkg_artifact(self) -> None:
        entry = web_package_group.public_packages(self.data)[0]
        bad = self.artifacts / "bad.tgz"
        write_tarball(bad, entry, second_wasm=True)
        with self.assertRaisesRegex(web_package_group.PackageGroupError, "exactly"):
            web_package_group.inspect_tarball(bad)

        write_tarball(bad, entry, legacy=True)
        with self.assertRaisesRegex(web_package_group.PackageGroupError, "legacy pkg"):
            web_package_group.inspect_tarball(bad)

        write_tarball(bad, entry, include_legal=False)
        with self.assertRaisesRegex(web_package_group.PackageGroupError, "third-party license"):
            web_package_group.inspect_tarball(bad)

    def test_tarball_rejects_noncanonical_paths_links_and_duplicate_members(self) -> None:
        entry = web_package_group.public_packages(self.data)[0]
        bad = self.artifacts / "bad.tgz"

        write_tarball(bad, entry, unsafe_member="./package/extra")
        with self.assertRaisesRegex(web_package_group.PackageGroupError, "unsafe member path"):
            web_package_group.inspect_tarball(bad)

        write_tarball(bad, entry, duplicate_member=True)
        with self.assertRaisesRegex(web_package_group.PackageGroupError, "duplicate member"):
            web_package_group.inspect_tarball(bad)

        write_tarball(bad, entry, symlink=True)
        with self.assertRaisesRegex(web_package_group.PackageGroupError, "must not contain links"):
            web_package_group.inspect_tarball(bad)

    def test_tarball_rejects_tampered_provenance_and_entrypoint(self) -> None:
        entry = web_package_group.public_packages(self.data)[0]
        bad = self.artifacts / "bad.tgz"

        write_tarball(bad, entry, tamper_provenance_sha=True)
        with self.assertRaisesRegex(web_package_group.PackageGroupError, "provenance sha256"):
            web_package_group.inspect_tarball(bad)

        write_tarball(bad, entry, entrypoint_id="editor")
        with self.assertRaisesRegex(web_package_group.PackageGroupError, "main must point"):
            web_package_group.inspect_tarball(bad)

        write_tarball(bad, entry, omit_provenance_file=True)
        with self.assertRaisesRegex(web_package_group.PackageGroupError, "must exactly cover owned WASM and dist"):
            web_package_group.inspect_tarball(bad)

        write_tarball(bad, entry, unlisted_dist_file=True)
        with self.assertRaisesRegex(web_package_group.PackageGroupError, "must exactly cover owned WASM and dist"):
            web_package_group.inspect_tarball(bad)

        write_tarball(bad, entry, foreign_package_entry=True)
        with self.assertRaisesRegex(web_package_group.PackageGroupError, "exactly the owned wrapper"):
            web_package_group.inspect_tarball(bad)

    def test_tarball_rejects_wasm_files_or_directories_outside_the_runtime_allowlist(self) -> None:
        entry = web_package_group.public_packages(self.data)[0]
        bad = self.artifacts / "bad.tgz"

        write_tarball(bad, entry, unexpected_wasm_top_level=True)
        with self.assertRaisesRegex(web_package_group.PackageGroupError, "runtime files and snippets"):
            web_package_group.inspect_tarball(bad)

    def test_tarball_rejects_files_and_lifecycle_outside_the_closed_surface(self) -> None:
        entry = web_package_group.public_packages(self.data)[0]
        bad = self.artifacts / "bad.tgz"

        write_tarball(bad, entry, extra_top_level_file=True)
        with self.assertRaisesRegex(web_package_group.PackageGroupError, "outside the closed package surface"):
            web_package_group.inspect_tarball(bad)

        write_tarball(bad, entry, extra_manifest_file_root=True)
        with self.assertRaisesRegex(web_package_group.PackageGroupError, "files must list exactly"):
            web_package_group.inspect_tarball(bad)

        write_tarball(bad, entry, lifecycle_script=True)
        with self.assertRaisesRegex(web_package_group.PackageGroupError, "must not declare npm lifecycle scripts"):
            web_package_group.inspect_tarball(bad)

        write_tarball(bad, entry, publish_registry=True)
        with self.assertRaisesRegex(web_package_group.PackageGroupError, "must declare only publishConfig"):
            web_package_group.inspect_tarball(bad)

        write_tarball(bad, entry, bundled_dependency=True)
        with self.assertRaisesRegex(web_package_group.PackageGroupError, "must not declare bundled dependencies"):
            web_package_group.inspect_tarball(bad)

        write_tarball(bad, entry, unexpected_wasm_nested=True)
        with self.assertRaisesRegex(web_package_group.PackageGroupError, "runtime files and snippets"):
            web_package_group.inspect_tarball(bad)

    def test_manifest_rejects_provenance_identity_that_disagrees_with_descriptor(self) -> None:
        entry = web_package_group.public_packages(self.data)[0]
        tarball = self.artifacts / (entry["name"].replace("@", "").replace("/", "-") + ".tgz")
        write_tarball(
            tarball,
            entry,
            provenance_id="editor",
            entrypoint_id="editor",
            artifact_entrypoint_id="editor",
        )
        with self.assertRaisesRegex(web_package_group.PackageGroupError, "provenance id"):
            self.create_manifest()

    def test_artifact_binds_source_tag_and_exact_tarball_set(self) -> None:
        path = self.create_manifest()
        with self.assertRaisesRegex(web_package_group.PackageGroupError, "source_sha"):
            web_package_group.verify_artifact(
                path,
                self.artifacts,
                expected_source_sha="f" * 40,
            )
        with self.assertRaisesRegex(web_package_group.PackageGroupError, "target_dist_tag"):
            web_package_group.verify_artifact(
                path,
                self.artifacts,
                expected_target_dist_tag="beta",
            )

        entry = web_package_group.public_packages(self.data)[0]
        write_tarball(self.artifacts / "unexpected.tgz", entry)
        with self.assertRaisesRegex(web_package_group.PackageGroupError, "tarball set"):
            web_package_group.verify_artifact(path, self.artifacts)

    def test_public_slim_tarballs_must_have_a_measured_size_benefit(self) -> None:
        editor = next(entry for entry in web_package_group.public_packages(self.data) if entry["id"] == "editor")
        tarball = self.artifacts / (editor["name"].replace("@", "").replace("/", "-") + ".tgz")
        write_tarball(tarball, editor, full_sized_dist=True)
        with self.assertRaisesRegex(web_package_group.PackageGroupError, "at least 15% smaller"):
            self.create_manifest()

    def test_group_manifest_requires_the_full_package_for_size_admission(self) -> None:
        path = self.create_manifest()
        manifest = json.loads(path.read_text(encoding="utf-8"))
        manifest["packages"] = [
            package for package in manifest["packages"] if package["id"] != "full"
        ]
        with self.assertRaisesRegex(web_package_group.PackageGroupError, "must include the public 'full' package"):
            web_package_group.validate_group_manifest(manifest)

    def test_reconciliation_publishes_missing_versions_then_promotes_every_package(self) -> None:
        path = self.create_manifest()
        manifest = web_package_group.verify_artifact(path, self.artifacts)
        client = FakeNpm()
        client.manifest = manifest
        report = web_package_group.reconcile_group(manifest, self.artifacts, client)
        self.assertEqual({name for name, _version in client.published}, {"@mermanjs/web", "@mermanjs/web-editor"})
        self.assertEqual(set(report["promoted"]), {"@mermanjs/web", "@mermanjs/web-editor"})
        for record in manifest["packages"]:
            self.assertEqual(client.tags[(record["name"], "alpha")], VERSION)

    def test_reconciliation_recovers_prior_tags_when_promotion_fails(self) -> None:
        path = self.create_manifest()
        manifest = web_package_group.verify_artifact(path, self.artifacts)
        client = FakeNpm(fail_tag_for="@mermanjs/web-editor")
        client.manifest = manifest
        for record in manifest["packages"]:
            client.versions[(record["name"], VERSION)] = record["integrity"]
        client.tags[("@mermanjs/web", "alpha")] = "0.8.0-alpha.3"
        with self.assertRaisesRegex(web_package_group.PackageGroupError, "simulated tag failure"):
            web_package_group.reconcile_group(manifest, self.artifacts, client)
        self.assertEqual(client.tags[("@mermanjs/web", "alpha")], "0.8.0-alpha.3")
        self.assertNotIn(("@mermanjs/web-editor", "alpha"), client.tags)

    def test_reconciliation_rolls_back_a_tag_when_its_post_promotion_read_fails(self) -> None:
        path = self.create_manifest()
        manifest = web_package_group.verify_artifact(path, self.artifacts)
        client = FakeNpm(fail_observe_after_add_for="@mermanjs/web")
        client.manifest = manifest
        for record in manifest["packages"]:
            client.versions[(record["name"], VERSION)] = record["integrity"]
        client.tags[("@mermanjs/web", "alpha")] = "0.8.0-alpha.3"

        with self.assertRaisesRegex(web_package_group.PackageGroupError, "post-promotion lookup failure"):
            web_package_group.reconcile_group(manifest, self.artifacts, client)

        self.assertEqual(client.tags[("@mermanjs/web", "alpha")], "0.8.0-alpha.3")
        self.assertNotIn(("@mermanjs/web-editor", "alpha"), client.tags)

    def test_reconciliation_rejects_existing_version_with_different_integrity(self) -> None:
        path = self.create_manifest()
        manifest = web_package_group.verify_artifact(path, self.artifacts)
        client = FakeNpm()
        client.manifest = manifest
        first = manifest["packages"][0]
        client.versions[(first["name"], VERSION)] = "sha512-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=="
        with self.assertRaisesRegex(web_package_group.PackageGroupError, "integrity differs"):
            web_package_group.reconcile_group(manifest, self.artifacts, client)


if __name__ == "__main__":
    unittest.main()
