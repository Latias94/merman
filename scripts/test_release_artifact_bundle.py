#!/usr/bin/env python3
"""Tests for Merman's immutable release artifact bundle contract."""

from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest
from unittest import mock

try:
    from scripts import release_artifact_bundle as bundle
except ModuleNotFoundError:
    import release_artifact_bundle as bundle


SOURCE_SHA = "a" * 40
VERSION = "0.8.0-alpha.4"
TAG = f"v{VERSION}"
TARGETS = (
    "aarch64-apple-darwin",
    "x86_64-apple-darwin",
    "x86_64-pc-windows-msvc",
    "x86_64-unknown-linux-gnu",
)


def archive_name(package: str, target: str = TARGETS[-1]) -> str:
    extension = "zip" if "windows" in target else "tar.xz"
    return f"{package}-{target}.{extension}"


def archive_names() -> set[str]:
    return {
        archive_name(package, target)
        for package in bundle.PACKAGES
        for target in TARGETS
    }


def release_names() -> set[str]:
    archives = archive_names()
    return {
        *archives,
        *(f"{name}.sha256" for name in archives),
        *(
            f"{package}-installer.{extension}"
            for package in bundle.PACKAGES
            for extension in ("sh", "ps1")
        ),
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
    artifacts: dict[str, dict[str, object]] = {}
    for name in sorted(release_names()):
        if name.endswith((".tar.xz", ".zip")):
            kind = "executable-zip"
            target_triples = [bundle._asset_identity(name)[1]["target"]]
        elif name.endswith(".sha256"):
            kind = "checksum"
            target_triples = [
                bundle._asset_identity(name.removesuffix(".sha256"))[1]["target"]
            ]
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
    verified.mkdir()
    digests: dict[str, str] = {}
    for target in TARGETS:
        producer = root / f"artifacts-build-local-{target}"
        producer.mkdir(parents=True)
        local_artifacts: dict[str, dict[str, object]] = {}
        for package in bundle.PACKAGES:
            name = archive_name(package, target)
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
        (producer / f"{target}-dist-manifest.json").write_text(
            json.dumps({"announcement_tag": TAG, "artifacts": local_artifacts}),
            encoding="utf-8",
        )
    plan_path.write_text(json.dumps(plan_document()), encoding="utf-8")
    return digests


def raw_shell_installer(package: str, digests: dict[str, str]) -> str:
    url = f"https://github.com/Latias94/merman/releases/download/{TAG}"
    cases = []
    for name in sorted(name for name in digests if name.startswith(f"{package}-")):
        cases.append(
            f'        "{name}")\n'
            f'            _checksum_value="{digests[name]}"\n'
            "            ;;\n"
        )
    return (
        f"# {package} {VERSION}\n# {url}\n"
        + "".join(cases)
        + "verify_checksum() {\n"
        + "    case \"$_checksum_style\" in\n"
        + bundle._SHELL_SHA256_FAIL_OPEN
        + "    esac\n}\n"
    )


def raw_powershell_installer(package: str, digests: dict[str, str]) -> str:
    windows_archive = archive_name(package, "x86_64-pc-windows-msvc")
    url = f"https://github.com/Latias94/merman/releases/download/{TAG}"
    return (
        f"$app_name = '{package}'\n"
        f"$app_version = '{VERSION}'\n"
        f"# {url}/{windows_archive}\n"
        + bundle._POWERSHELL_DOWNLOAD_ANCHOR
    )


def checksum_probe_script() -> str:
    return (
        "set -eu\n"
        "check_cmd() { command -v \"$1\" >/dev/null 2>&1; }\n"
        "err() { printf '%s\\n' \"$1\" >&2; return 1; }\n"
        "verify_checksum() {\n"
        "    local _file=\"$1\"\n"
        "    local _checksum_style=\"$2\"\n"
        "    local _checksum_value=\"$3\"\n"
        "    local _calculated_checksum\n"
        "    case \"$_checksum_style\" in\n"
        + bundle._SHELL_SHA256_FAIL_CLOSED
        + "    esac\n"
        "    if [ \"$_calculated_checksum\" != \"$_checksum_value\" ]; then\n"
        "        err \"checksum mismatch\"\n"
        "    fi\n"
        "}\n"
        "verify_checksum \"$1\" sha256 \"$2\"\n"
    )


def complete_global_generation(
    root: Path,
    digests: dict[str, str],
    surfaces: Path,
    *,
    harden: bool = True,
) -> None:
    plan = json.loads((root / bundle.DIST_INPUT_MANIFEST).read_text(encoding="utf-8"))
    (root / bundle.DIST_OUTPUT_MANIFEST).write_text(json.dumps(plan), encoding="utf-8")
    (root / "sha256.sum").write_text(
        "".join(f"{digests[name]} *{name}\n" for name in sorted(digests)) + "\n",
        encoding="ascii",
    )
    for package in bundle.PACKAGES:
        (root / f"{package}-installer.sh").write_text(
            raw_shell_installer(package, digests),
            encoding="utf-8",
        )
        (root / f"{package}-installer.ps1").write_text(
            raw_powershell_installer(package, digests),
            encoding="utf-8",
        )
    if harden:
        bundle.harden_installers(root, surfaces, version=VERSION)


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
                "harden_installers",
                "prepare_global_inputs",
                "verify_bundle",
            },
        )

    def test_prepare_harden_assemble_and_verify_close_end_to_end(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            temp = Path(temp_dir)
            _, verified, generated, surfaces, digests = self.prepare(temp)
            complete_global_generation(generated, digests, surfaces)
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
            shell = (destination / "merman-cli-installer.sh").read_text(encoding="utf-8")
            powershell = (destination / "merman-cli-installer.ps1").read_text(
                encoding="utf-8"
            )
            self.assertNotIn("skipping sha256 checksum verification", shell)
            self.assertIn("shasum -a 256 -b", shell)
            self.assertIn("openssl dgst -sha256", shell)
            self.assertIn("Get-FileHash -LiteralPath", powershell)

    def test_prepare_copies_only_verified_snapshot_paths(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            temp = Path(temp_dir)
            local = temp / "local"
            verified = temp / "verified"
            surfaces = temp / "surfaces.json"
            plan = temp / "plan.json"
            local.mkdir()
            write_surface(surfaces)
            write_local_inputs(local, verified, plan)
            with mock.patch.object(
                bundle.shutil,
                "copyfile",
                wraps=shutil.copyfile,
            ) as copyfile:
                bundle.prepare_global_inputs(
                    local,
                    verified,
                    plan,
                    surfaces,
                    temp / "generated",
                    tag=TAG,
                    version=VERSION,
                )
            self.assertEqual(len(copyfile.call_args_list), len(archive_names()))
            self.assertTrue(
                all(Path(call.args[0]).parent == verified for call in copyfile.call_args_list)
            )

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

    def test_prepare_rejects_archive_moved_between_producers(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            temp = Path(temp_dir)
            local = temp / "local"
            verified = temp / "verified"
            surfaces = temp / "surfaces.json"
            plan = temp / "plan.json"
            local.mkdir()
            write_surface(surfaces)
            write_local_inputs(local, verified, plan)
            source_target, destination_target = TARGETS[:2]
            name = archive_name("merman-cli", source_target)
            shutil.move(
                local / f"artifacts-build-local-{source_target}" / name,
                local / f"artifacts-build-local-{destination_target}" / name,
            )
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

    def test_prepare_rejects_each_local_self_consistency_break(self) -> None:
        for mutation in ("tag", "manifest-digest", "checksum", "raw", "verified"):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as temp_dir:
                temp = Path(temp_dir)
                local = temp / "local"
                verified = temp / "verified"
                surfaces = temp / "surfaces.json"
                plan = temp / "plan.json"
                local.mkdir()
                write_surface(surfaces)
                write_local_inputs(local, verified, plan)
                target = TARGETS[-1]
                name = archive_name("merman-cli", target)
                producer = local / f"artifacts-build-local-{target}"
                manifest_path = producer / f"{target}-dist-manifest.json"
                manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
                if mutation == "tag":
                    manifest["announcement_tag"] = "v9.9.9"
                    manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
                elif mutation == "manifest-digest":
                    manifest["artifacts"][name]["checksums"]["sha256"] = "0" * 64
                    manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
                elif mutation == "checksum":
                    (producer / f"{name}.sha256").write_text(
                        f"{'0' * 64} *{name}\n",
                        encoding="ascii",
                    )
                elif mutation == "raw":
                    (producer / name).write_bytes(b"tampered raw")
                else:
                    (verified / name).write_bytes(b"tampered snapshot")
                destination = temp / "generated"
                with self.assertRaises(bundle.ReleaseArtifactError):
                    bundle.prepare_global_inputs(
                        local,
                        verified,
                        plan,
                        surfaces,
                        destination,
                        tag=TAG,
                        version=VERSION,
                    )
                self.assertFalse(destination.exists())

    def test_hardening_rejects_cargo_dist_template_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            temp = Path(temp_dir)
            _, _, generated, surfaces, digests = self.prepare(temp)
            complete_global_generation(generated, digests, surfaces, harden=False)
            path = generated / "merman-cli-installer.sh"
            path.write_text(
                path.read_text(encoding="utf-8").replace(
                    "skipping sha256 checksum verification",
                    "changed upstream behavior",
                ),
                encoding="utf-8",
            )
            with self.assertRaises(bundle.ReleaseArtifactError):
                bundle.harden_installers(generated, surfaces, version=VERSION)

    def test_installer_archive_digest_mapping_is_exact(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            temp = Path(temp_dir)
            _, _, generated, surfaces, digests = self.prepare(temp)
            complete_global_generation(generated, digests, surfaces)
            first, second = sorted(
                name for name in digests if name.startswith("merman-cli-")
            )[:2]
            path = generated / "merman-cli-installer.sh"
            text = path.read_text(encoding="utf-8")
            text = text.replace(digests[first], "x" * 64, 1)
            text = text.replace(digests[second], digests[first], 1)
            text = text.replace("x" * 64, digests[second], 1)
            path.write_text(text, encoding="utf-8")
            with self.assertRaises(bundle.ReleaseArtifactError):
                bundle._validate_installers(
                    generated,
                    tuple(sorted(archive_names())),
                    version=VERSION,
                )

    @unittest.skipUnless(os.name == "posix", "shell installer probe requires POSIX sh")
    def test_shell_checksum_hardening_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            temp = Path(temp_dir)
            payload = temp / "archive"
            payload.write_bytes(b"verified bytes")
            script = temp / "probe.sh"
            script.write_text(checksum_probe_script(), encoding="utf-8")
            digest = hashlib.sha256(payload.read_bytes()).hexdigest()

            success = subprocess.run(
                ["/bin/sh", str(script), str(payload), digest],
                text=True,
                capture_output=True,
                timeout=10,
                check=False,
            )
            self.assertEqual(success.returncode, 0, msg=success.stderr)

            mismatch = subprocess.run(
                ["/bin/sh", str(script), str(payload), "0" * 64],
                text=True,
                capture_output=True,
                timeout=10,
                check=False,
            )
            self.assertNotEqual(mismatch.returncode, 0)
            self.assertIn("checksum mismatch", mismatch.stderr)

            empty_path = temp / "empty-path"
            empty_path.mkdir()
            unavailable = subprocess.run(
                ["/bin/sh", str(script), str(payload), digest],
                env={"PATH": str(empty_path)},
                text=True,
                capture_output=True,
                timeout=10,
                check=False,
            )
            self.assertNotEqual(unavailable.returncode, 0)
            self.assertIn("cannot verify sha256 checksum", unavailable.stderr)

    def test_assemble_rejects_unhardened_installers(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            temp = Path(temp_dir)
            _, verified, generated, surfaces, digests = self.prepare(temp)
            complete_global_generation(generated, digests, surfaces, harden=False)
            with self.assertRaises(bundle.ReleaseArtifactError):
                bundle.assemble_bundle(
                    generated,
                    verified,
                    temp / "bundle",
                    surfaces,
                    version=VERSION,
                    source_sha=SOURCE_SHA,
                )

    def test_bundle_tampering_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            temp = Path(temp_dir)
            _, verified, generated, surfaces, digests = self.prepare(temp)
            complete_global_generation(generated, digests, surfaces)
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
