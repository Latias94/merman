#!/usr/bin/env python3
"""Tests for Merman's immutable release artifact bundle contract."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import shutil
import tempfile
import unittest

try:
    from scripts import release_artifact_bundle as bundle
except ModuleNotFoundError:
    import release_artifact_bundle as bundle


SOURCE_SHA = "a" * 40
VERSION = "0.8.0-alpha.4"
TAG = f"v{VERSION}"
TARGET = "x86_64-unknown-linux-gnu"


def archive_name(package: str) -> str:
    return f"{package}-{TARGET}.tar.xz"


def release_names() -> set[str]:
    archives = {archive_name(package) for package in bundle.PACKAGES}
    return {
        *archives,
        *(f"{name}.sha256" for name in archives),
        *(f"{package}-installer.{extension}" for package in bundle.PACKAGES for extension in ("sh", "ps1")),
        "sha256.sum",
    }


def write_surface(path: Path, names: set[str] | None = None) -> None:
    names = release_names() if names is None else names
    path.write_text(
        json.dumps(
            {
                "surfaces": [
                    {
                        "channels": [
                            {
                                "workflow": bundle.RELEASE_WORKFLOW,
                                "workflow_job": bundle.RELEASE_JOB,
                                "asset_patterns": [
                                    {"glob": name}
                                    for name in sorted({*names, bundle.MANIFEST_NAME})
                                ],
                            }
                        ]
                    }
                ]
            }
        ),
        encoding="utf-8",
    )


def plan_document() -> dict[str, object]:
    artifacts = {}
    for name in sorted(release_names()):
        if name.endswith((".tar.xz", ".zip")):
            kind = "executable-zip"
            target_triples = [TARGET]
        elif name.endswith(".sha256"):
            kind = "checksum"
            target_triples = [TARGET]
        elif "-installer." in name:
            kind = "installer"
            target_triples = []
        else:
            kind = "unified-checksum"
            target_triples = []
        artifacts[name] = {"kind": kind, "target_triples": target_triples}
    return {
        "announcement_tag": TAG,
        "announcement_is_prerelease": True,
        "artifacts": artifacts,
        "releases": [
            {
                "app_name": package,
                "app_version": VERSION,
                "artifacts": sorted(
                    name
                    for name in release_names()
                    if name.startswith(f"{package}-") or name == "sha256.sum"
                ),
            }
            for package in bundle.PACKAGES
        ],
        "upload_files": [],
    }


def write_local_inputs(root: Path, verified: Path, plan_path: Path) -> dict[str, str]:
    producer = root / f"artifacts-build-local-{TARGET}"
    producer.mkdir(parents=True)
    verified.mkdir()
    digests = {}
    local_artifacts = {}
    for package in bundle.PACKAGES:
        name = archive_name(package)
        contents = f"verified:{name}\n".encode()
        digest = hashlib.sha256(contents).hexdigest()
        digests[name] = digest
        (producer / name).write_bytes(contents)
        (producer / f"{name}.sha256").write_text(
            f"{digest} *{name}\n",
            encoding="ascii",
        )
        (verified / name).write_bytes(contents)
        local_artifacts[name] = {
            "kind": "executable-zip",
            "checksums": {"sha256": digest},
        }
    (producer / f"{TARGET}-dist-manifest.json").write_text(
        json.dumps({"announcement_tag": TAG, "artifacts": local_artifacts}),
        encoding="utf-8",
    )
    plan_path.write_text(json.dumps(plan_document()), encoding="utf-8")
    return digests


def complete_global_generation(root: Path, digests: dict[str, str]) -> None:
    plan = json.loads((root / bundle.DIST_INPUT_MANIFEST).read_text(encoding="utf-8"))
    (root / bundle.DIST_OUTPUT_MANIFEST).write_text(json.dumps(plan), encoding="utf-8")
    (root / "sha256.sum").write_text(
        "".join(f"{digests[name]} *{name}\n" for name in sorted(digests)) + "\n",
        encoding="ascii",
    )
    url = f"https://github.com/Latias94/merman/releases/download/{TAG}"
    for package in bundle.PACKAGES:
        required = [
            package,
            VERSION,
            url,
            archive_name(package),
            digests[archive_name(package)],
        ]
        text = "\n".join(required) + "\n"
        (root / f"{package}-installer.sh").write_text(text, encoding="utf-8")
        (root / f"{package}-installer.ps1").write_text(text, encoding="utf-8")


class ReleaseArtifactBundleTests(unittest.TestCase):
    def prepare(self, temp: Path) -> tuple[Path, Path, Path, Path, dict[str, str]]:
        local = temp / "local"
        verified = temp / "verified"
        generated = temp / "generated"
        surfaces = temp / "surfaces.json"
        plan = temp / "plan-dist-manifest.json"
        local.mkdir()
        write_surface(surfaces)
        digests = write_local_inputs(local, verified, plan)
        bundle.prepare_global_inputs(
            local,
            verified,
            plan,
            surfaces,
            generated,
            tag=TAG,
            version=VERSION,
        )
        return local, verified, generated, surfaces, digests

    def test_public_api_is_explicit_and_product_bounded(self) -> None:
        self.assertEqual(
            set(bundle.__all__),
            {
                "ReleaseArtifactError",
                "assemble_bundle",
                "prepare_global_inputs",
                "verify_bundle",
                "verify_native_receipts",
                "write_native_receipt",
            },
        )

    def test_prepare_bundle_and_receipts_close_end_to_end(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            temp = Path(temp_dir)
            _, verified, generated, surfaces, digests = self.prepare(temp)
            complete_global_generation(generated, digests)
            destination = temp / "bundle"
            bundle.assemble_bundle(
                generated,
                verified,
                destination,
                surfaces,
                version=VERSION,
                source_sha=SOURCE_SHA,
            )
            bundle.verify_bundle(destination, version=VERSION, source_sha=SOURCE_SHA)

            receipts = temp / "receipts"
            receipts.mkdir()
            receipt = receipts / f"native-release-verification-{TARGET}.json"
            bundle.write_native_receipt(
                destination,
                receipt,
                target=TARGET,
                source_sha=SOURCE_SHA,
            )
            bundle.verify_native_receipts(destination, receipts, source_sha=SOURCE_SHA)

    def test_prepare_rejects_missing_verified_archive(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            temp = Path(temp_dir)
            local = temp / "local"
            verified = temp / "verified"
            surfaces = temp / "surfaces.json"
            plan = temp / "plan.json"
            local.mkdir()
            write_surface(surfaces)
            write_local_inputs(local, verified, plan)
            (verified / archive_name("merman-lsp")).unlink()
            with self.assertRaises(bundle.ReleaseArtifactError):
                bundle.prepare_global_inputs(
                    local,
                    verified,
                    plan,
                    surfaces,
                    temp / "generated",
                    tag=TAG,
                    version=VERSION,
                )

    def test_prepare_rejects_cross_producer_or_extra_payload(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            temp = Path(temp_dir)
            local = temp / "local"
            verified = temp / "verified"
            surfaces = temp / "surfaces.json"
            plan = temp / "plan.json"
            local.mkdir()
            write_surface(surfaces)
            write_local_inputs(local, verified, plan)
            producer = local / f"artifacts-build-local-{TARGET}"
            (producer / "foreign-installer.sh").write_text("unexpected", encoding="utf-8")
            with self.assertRaises(bundle.ReleaseArtifactError):
                bundle.prepare_global_inputs(
                    local,
                    verified,
                    plan,
                    surfaces,
                    temp / "generated",
                    tag=TAG,
                    version=VERSION,
                )

    def test_assemble_rejects_installer_without_archive_digest(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            temp = Path(temp_dir)
            _, verified, generated, surfaces, digests = self.prepare(temp)
            complete_global_generation(generated, digests)
            installer = generated / "merman-cli-installer.sh"
            installer.write_text(
                installer.read_text(encoding="utf-8").replace(
                    digests[archive_name("merman-cli")],
                    "missing-digest",
                ),
                encoding="utf-8",
            )
            with self.assertRaises(bundle.ReleaseArtifactError):
                bundle.assemble_bundle(
                    generated,
                    verified,
                    temp / "bundle",
                    surfaces,
                    version=VERSION,
                    source_sha=SOURCE_SHA,
                )

    def test_installer_contract_matches_cargo_dist_platform_split(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            archives: list[str] = []
            digests: dict[str, str] = {}
            for package in bundle.PACKAGES:
                for target, extension in (
                    ("x86_64-unknown-linux-gnu", "tar.xz"),
                    ("x86_64-pc-windows-msvc", "zip"),
                ):
                    name = f"{package}-{target}.{extension}"
                    digest = hashlib.sha256(name.encode()).hexdigest()
                    archives.append(name)
                    digests[name] = digest
                    (root / f"{name}.sha256").write_text(
                        f"{digest} *{name}\n",
                        encoding="ascii",
                    )

            url = f"https://github.com/Latias94/merman/releases/download/{TAG}"
            for package in bundle.PACKAGES:
                package_archives = sorted(
                    name for name in archives if name.startswith(f"{package}-")
                )
                shell_contract = [
                    package,
                    VERSION,
                    url,
                    *package_archives,
                    *(digests[name] for name in package_archives),
                ]
                windows_archive = next(
                    name for name in package_archives if name.endswith(".zip")
                )
                powershell_contract = [package, VERSION, url, windows_archive]
                (root / f"{package}-installer.sh").write_text(
                    "\n".join(shell_contract) + "\n",
                    encoding="utf-8",
                )
                (root / f"{package}-installer.ps1").write_text(
                    "\n".join(powershell_contract) + "\n",
                    encoding="utf-8",
                )

            names = tuple(sorted(archives))
            bundle._validate_installers(root, names, version=VERSION)

            installer = root / "merman-cli-installer.ps1"
            installer.write_text(
                installer.read_text(encoding="utf-8").replace(
                    "merman-cli-x86_64-pc-windows-msvc.zip",
                    "missing-windows-archive",
                ),
                encoding="utf-8",
            )
            with self.assertRaises(bundle.ReleaseArtifactError):
                bundle._validate_installers(root, names, version=VERSION)

    def test_bundle_and_receipt_tampering_fail_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            temp = Path(temp_dir)
            _, verified, generated, surfaces, digests = self.prepare(temp)
            complete_global_generation(generated, digests)
            destination = temp / "bundle"
            bundle.assemble_bundle(
                generated,
                verified,
                destination,
                surfaces,
                version=VERSION,
                source_sha=SOURCE_SHA,
            )
            (destination / archive_name("merman-cli")).write_bytes(b"tampered")
            with self.assertRaises(bundle.ReleaseArtifactError):
                bundle.verify_bundle(destination, version=VERSION, source_sha=SOURCE_SHA)

            shutil.copyfile(verified / archive_name("merman-cli"), destination / archive_name("merman-cli"))
            receipts = temp / "receipts"
            receipts.mkdir()
            receipt = receipts / f"native-release-verification-{TARGET}.json"
            bundle.write_native_receipt(
                destination,
                receipt,
                target=TARGET,
                source_sha=SOURCE_SHA,
            )
            document = json.loads(receipt.read_text(encoding="utf-8"))
            document["manifest_sha256"] = "0" * 64
            receipt.write_text(json.dumps(document), encoding="utf-8")
            with self.assertRaises(bundle.ReleaseArtifactError):
                bundle.verify_native_receipts(destination, receipts, source_sha=SOURCE_SHA)

    def test_surface_contract_rejects_unknown_source_or_glob_assets(self) -> None:
        for unexpected in ("source.tar.gz", "*.zip"):
            with self.subTest(unexpected=unexpected), tempfile.TemporaryDirectory() as temp_dir:
                temp = Path(temp_dir)
                surfaces = temp / "surfaces.json"
                write_surface(surfaces, {*release_names(), unexpected})
                with self.assertRaises(bundle.ReleaseArtifactError):
                    bundle.prepare_global_inputs(
                        temp / "missing-local",
                        temp / "missing-verified",
                        temp / "missing-plan",
                        surfaces,
                        temp / "generated",
                        tag=TAG,
                        version=VERSION,
                    )


if __name__ == "__main__":
    unittest.main()
