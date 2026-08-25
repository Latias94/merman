#!/usr/bin/env python3

from __future__ import annotations

import io
import json
import tarfile
import tempfile
import unittest
from pathlib import Path

from scripts import node_package_group
from scripts.npm_package_group import DryRunNpmClient, reconcile_group


ROOT = Path(__file__).resolve().parents[1]
DESCRIPTOR = ROOT / "platforms" / "node" / "package-surfaces.json"


class NodePackageGroupTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.artifacts = Path(self.temporary.name)
        self.descriptor = json.loads(DESCRIPTOR.read_text(encoding="utf-8"))
        self.version = self.descriptor["version"]
        for target in self.descriptor["targets"]:
            self._write_package(target["name"], role="platform", target=target)
        self._write_package(self.descriptor["wasm"]["name"], role="wasm")
        self._write_package(self.descriptor["root"]["name"], role="loader")

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def _write_package(
        self,
        name: str,
        *,
        role: str,
        target: dict | None = None,
    ) -> None:
        manifest = {
            "name": name,
            "version": self.version,
            "engines": {"node": self.descriptor["node_engine"]},
            "repository": node_package_group.REPOSITORY,
            "homepage": node_package_group.HOMEPAGE,
            "publishConfig": {"access": "public"},
            "license": "MIT OR Apache-2.0",
        }
        files = {
            "package/package.json": json.dumps(manifest).encode(),
            "package/README.md": b"readme",
            "package/LICENSE-APACHE": b"apache",
            "package/LICENSE-MIT": b"mit",
            "package/THIRD_PARTY_NOTICES.md": b"notices",
        }
        if role == "loader":
            manifest["optionalDependencies"] = {
                package["name"]: self.version
                for package in self.descriptor["targets"]
            }
            for required in node_package_group.REQUIRED_LOADER_FILES:
                files[required] = b"loader"
        elif role == "wasm":
            manifest["main"] = "./dist/index.mjs"
            manifest["types"] = "./dist/index.d.ts"
            for required in node_package_group.REQUIRED_WASM_FILES:
                files[required] = b"wasm"
        else:
            assert target is not None
            manifest["main"] = f"./{target['node_artifact']}"
            for field in ("os", "cpu", "libc"):
                if value := target.get(field):
                    manifest[field] = [value]
            files[f"package/{target['node_artifact']}"] = b"native"
        files["package/package.json"] = json.dumps(manifest).encode()
        filename = name.removeprefix("@").replace("/", "-") + f"-{self.version}.tgz"
        with tarfile.open(self.artifacts / filename, "w:gz") as archive:
            for path, data in sorted(files.items()):
                info = tarfile.TarInfo(path)
                info.size = len(data)
                archive.addfile(info, io.BytesIO(data))

    def test_create_verify_and_publish_lockstep_group(self) -> None:
        manifest_path = node_package_group.create_manifest(
            self.artifacts,
            DESCRIPTOR,
            version=self.version,
            source_sha="a" * 40,
            target_dist_tag="alpha",
        )
        manifest = node_package_group.verify_artifact(
            manifest_path,
            self.artifacts,
            descriptor_path=DESCRIPTOR,
            expected_version=self.version,
            expected_source_sha="a" * 40,
            expected_target_dist_tag="alpha",
        )
        self.assertEqual(
            [record["name"] for record in manifest["packages"]],
            [target["name"] for target in self.descriptor["targets"]]
            + [self.descriptor["wasm"]["name"]]
            + [self.descriptor["root"]["name"]],
        )

        client = DryRunNpmClient(manifest)
        report = reconcile_group(manifest, self.artifacts, client)
        self.assertEqual(report["status"], "released")
        self.assertEqual(report["already_published"], [])
        self.assertEqual(report["published"][-1], "@mermanjs/node")
        self.assertEqual(
            client.operations,
            [
                f"publish {record['name']}@{self.version} --tag alpha"
                for record in manifest["packages"]
            ],
        )

        retry = reconcile_group(manifest, self.artifacts, client)
        self.assertEqual(retry["status"], "released")
        self.assertEqual(retry["published"], [])
        self.assertEqual(retry["already_published"][-1], "@mermanjs/node")

    def test_tampered_tarball_is_rejected(self) -> None:
        manifest_path = node_package_group.create_manifest(
            self.artifacts,
            DESCRIPTOR,
            version=self.version,
            source_sha="b" * 40,
            target_dist_tag="alpha",
        )
        first = next(self.artifacts.glob("*.tgz"))
        first.write_bytes(first.read_bytes() + b"tampered")
        with self.assertRaisesRegex(
            node_package_group.PackageGroupError,
            "no longer matches",
        ):
            node_package_group.verify_artifact(
                manifest_path,
                self.artifacts,
                descriptor_path=DESCRIPTOR,
            )

    def test_descriptor_cannot_redirect_the_publisher_to_another_package(self) -> None:
        descriptor_path = self.artifacts / "package-surfaces.json"
        descriptor = dict(self.descriptor)
        descriptor["root"] = {
            "name": "@attacker/node",
            "directory": "packages/node",
        }
        descriptor_path.write_text(json.dumps(descriptor), encoding="utf-8")
        with self.assertRaisesRegex(
            node_package_group.PackageGroupError,
            "loader package does not match",
        ):
            node_package_group.load_descriptor(descriptor_path)

    def test_loader_must_pin_every_platform_package(self) -> None:
        loader = self.descriptor["root"]["name"]
        loader_tarball = next(
            path for path in self.artifacts.glob("*.tgz") if "mermanjs-node-0" in path.name
        )
        loader_tarball.unlink()
        self._write_package(loader, role="loader")
        loader_tarball = next(
            path for path in self.artifacts.glob("*.tgz") if "mermanjs-node-0" in path.name
        )
        with tarfile.open(loader_tarball, "r:gz") as archive:
            members = {
                member.name: archive.extractfile(member).read()
                for member in archive.getmembers()
                if member.isfile()
            }
        manifest = json.loads(members["package/package.json"])
        manifest["optionalDependencies"].pop("@mermanjs/node-win32-x64-msvc")
        members["package/package.json"] = json.dumps(manifest).encode()
        with tarfile.open(loader_tarball, "w:gz") as archive:
            for path, data in sorted(members.items()):
                info = tarfile.TarInfo(path)
                info.size = len(data)
                archive.addfile(info, io.BytesIO(data))
        with self.assertRaisesRegex(
            node_package_group.PackageGroupError,
            "depend on every platform package",
        ):
            node_package_group.create_manifest(
                self.artifacts,
                DESCRIPTOR,
                version=self.version,
                source_sha="c" * 40,
                target_dist_tag="alpha",
            )


if __name__ == "__main__":
    unittest.main()
