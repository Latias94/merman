#!/usr/bin/env python3
"""Contract tests for stable Scoop and WinGet candidate generation."""

from __future__ import annotations

import contextlib
import hashlib
import io
import json
from pathlib import Path
import shutil
import sys
import tempfile
import unittest
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))

import generate_cli_registry_candidates as generator
from verify_cli_release_archive import ArchiveVerificationError, VerificationReport


VERSION = "0.8.0"
DIGEST = "0123456789abcdef" * 4
ARCHIVE_NAME = "merman-cli-x86_64-pc-windows-msvc.zip"
REPOSITORY_URL = "https://github.com/Latias94/merman"
FIXTURE_CHECKSUMS = (
    Path(__file__).resolve().parent
    / "fixtures"
    / "cli_registry_candidates"
    / "v0.8.0.sha256.json"
)


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write_workspace(root: Path, *, version: str = VERSION, repository: str = REPOSITORY_URL) -> None:
    (root / "Cargo.toml").write_text(
        "[workspace]\n"
        "[workspace.package]\n"
        f'version = "{version}"\n'
        f'repository = "{repository}"\n',
        encoding="utf-8",
    )


def write_bundle(root: Path) -> Path:
    bundle = root / "verified"
    bundle.mkdir()
    (bundle / ARCHIVE_NAME).write_bytes(b"verified archive fixture")
    (bundle / f"{ARCHIVE_NAME}.sha256").write_text(
        f"{DIGEST}  {ARCHIVE_NAME}\n",
        encoding="ascii",
    )
    return bundle


def fake_verifier(
    archive: Path,
    checksum: Path,
    *,
    target: str,
    version: str,
    repo_root: Path,
    verified_output: Path,
) -> VerificationReport:
    del checksum, version, repo_root
    return VerificationReport(
        archive=verified_output,
        digest=DIGEST,
        target=target,
        member_count=7,
        total_uncompressed_bytes=1024,
        binary_path="merman-cli.exe",
    )


def generated_files(root: Path) -> dict[str, bytes]:
    return {
        path.relative_to(root).as_posix(): path.read_bytes()
        for path in sorted(root.rglob("*"))
        if path.is_file()
    }


class RegistryCandidateGenerationTests(unittest.TestCase):
    def generate(
        self,
        root: Path,
        *,
        version: str = VERSION,
        verifier=fake_verifier,
        template_root: Path = generator.TEMPLATE_ROOT,
        output_name: str = "candidates",
    ) -> generator.CandidateSet:
        write_workspace(root, version=version)
        bundle = write_bundle(root)
        return generator.generate_candidates(
            bundle,
            root / output_name,
            version=version,
            repo_root=root,
            template_root=template_root,
            verifier=verifier,
        )

    def test_generates_deterministic_registry_ready_candidates(self) -> None:
        with tempfile.TemporaryDirectory() as first_dir, tempfile.TemporaryDirectory() as second_dir:
            first_root = Path(first_dir)
            second_root = Path(second_dir)
            first = self.generate(first_root)
            second = self.generate(second_root)

            first_files = generated_files(first.output_dir)
            self.assertEqual(first_files, generated_files(second.output_dir))
            self.assertEqual(
                set(first_files),
                {
                    "candidate-receipt.json",
                    "scoop/merman-cli.json",
                    "winget/manifests/l/Latias94/MermanCLI/0.8.0/Latias94.MermanCLI.yaml",
                    "winget/manifests/l/Latias94/MermanCLI/0.8.0/Latias94.MermanCLI.installer.yaml",
                    "winget/manifests/l/Latias94/MermanCLI/0.8.0/Latias94.MermanCLI.locale.en-US.yaml",
                },
            )
            expected_hashes = json.loads(FIXTURE_CHECKSUMS.read_text(encoding="utf-8"))
            self.assertEqual(
                {path: hashlib.sha256(contents).hexdigest() for path, contents in first_files.items()},
                expected_hashes,
            )

    def test_receipt_binds_every_manifest_to_the_verified_archive(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            result = self.generate(root)
            receipt = json.loads(result.receipt.read_text(encoding="utf-8"))

            self.assertEqual(receipt["version"], VERSION)
            self.assertEqual(receipt["release_tag"], f"v{VERSION}")
            self.assertEqual(receipt["target"], generator.WINDOWS_TARGET)
            self.assertEqual(
                receipt["source"],
                {
                    "archive_name": ARCHIVE_NAME,
                    "archive_url": (
                        f"{REPOSITORY_URL}/releases/download/v{VERSION}/{ARCHIVE_NAME}"
                    ),
                    "sha256": DIGEST,
                    "binary_path": "merman-cli.exe",
                },
            )
            self.assertEqual(len(receipt["manifests"]), 4)
            for manifest in receipt["manifests"]:
                self.assertEqual(
                    manifest["sha256"],
                    sha256(result.output_dir / manifest["path"]),
                )

    def test_scoop_candidate_uses_only_x86_64_and_an_immutable_download(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            result = self.generate(Path(directory))
            manifest = json.loads((result.output_dir / generator.SCOOP_OUTPUT).read_text())

            self.assertEqual(set(manifest["architecture"]), {"64bit"})
            download = manifest["architecture"]["64bit"]
            self.assertEqual(download["hash"], DIGEST)
            self.assertEqual(
                download["url"],
                f"{REPOSITORY_URL}/releases/download/v{VERSION}/{ARCHIVE_NAME}",
            )
            self.assertEqual(manifest["bin"], "merman-cli.exe")
            self.assertEqual(manifest["license"], "MIT|Apache-2.0")
            self.assertEqual(manifest["checkver"], {"github": REPOSITORY_URL})
            self.assertEqual(
                manifest["autoupdate"]["architecture"]["64bit"]["url"],
                f"{REPOSITORY_URL}/releases/download/v$version/{ARCHIVE_NAME}",
            )

    def test_winget_candidate_uses_multifile_zip_portable_contract(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            result = self.generate(Path(directory))
            winget_root = result.output_dir / generator.WINGET_ROOT / VERSION
            version_text = (winget_root / "Latias94.MermanCLI.yaml").read_text()
            installer_text = (winget_root / "Latias94.MermanCLI.installer.yaml").read_text()
            locale_text = (winget_root / "Latias94.MermanCLI.locale.en-US.yaml").read_text()

            self.assertIn("ManifestType: \"version\"", version_text)
            self.assertIn("DefaultLocale: \"en-US\"", version_text)
            self.assertIn("InstallerType: \"zip\"", installer_text)
            self.assertIn("NestedInstallerType: \"portable\"", installer_text)
            self.assertIn("Architecture: \"x64\"", installer_text)
            self.assertIn(f"InstallerSha256: \"{DIGEST.upper()}\"", installer_text)
            self.assertIn("RelativeFilePath: \"merman-cli.exe\"", installer_text)
            self.assertIn("PortableCommandAlias: \"merman-cli\"", installer_text)
            self.assertIn("Microsoft.VCRedist.2015+.x64", installer_text)
            self.assertIn("ManifestType: \"defaultLocale\"", locale_text)
            self.assertIn("Moniker: \"merman-cli\"", locale_text)
            self.assertIn(
                f'ReleaseNotesUrl: "{REPOSITORY_URL}/releases/tag/v{VERSION}"',
                locale_text,
            )

    def test_verifier_receives_only_the_exact_windows_archive_contract(self) -> None:
        calls: list[tuple[tuple, dict]] = []

        def recording_verifier(*args, **kwargs):
            calls.append((args, kwargs))
            return fake_verifier(*args, **kwargs)

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            result = self.generate(root, verifier=recording_verifier)

            self.assertEqual(len(calls), 1)
            args, kwargs = calls[0]
            self.assertEqual(args[0].name, ARCHIVE_NAME)
            self.assertEqual(args[1].name, f"{ARCHIVE_NAME}.sha256")
            self.assertEqual(kwargs["target"], generator.WINDOWS_TARGET)
            self.assertEqual(kwargs["version"], VERSION)
            self.assertEqual(kwargs["verified_output"].name, ARCHIVE_NAME)
            self.assertFalse(kwargs["verified_output"].exists())
            self.assertTrue(result.output_dir.is_dir())

    def test_rejects_prerelease_build_metadata_prefix_and_malformed_versions(self) -> None:
        invalid_versions = [
            "0.8.0-alpha.1",
            "0.8.0+build.1",
            "v0.8.0",
            "0.8",
            "01.2.3",
            "latest",
        ]
        for version in invalid_versions:
            with self.subTest(version=version), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                with self.assertRaises(generator.CandidateGenerationError):
                    self.generate(root, version=version)

    def test_rejects_workspace_version_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            write_workspace(root, version="0.8.1")
            bundle = write_bundle(root)
            with self.assertRaisesRegex(
                generator.CandidateGenerationError,
                "requested version must exactly match",
            ):
                generator.generate_candidates(
                    bundle,
                    root / "candidates",
                    version=VERSION,
                    repo_root=root,
                    verifier=fake_verifier,
                )

    def test_rejects_noncanonical_or_moving_repository_urls(self) -> None:
        invalid_urls = [
            "http://github.com/Latias94/merman",
            "https://github.com/Latias94/merman/",
            "https://github.com/Latias94/merman.git",
            "https://github.com/Latias94/merman/releases/latest",
            "https://github.com/Latias94/merman?ref=main",
            "https://user@github.com/Latias94/merman",
            "https://example.com/Latias94/merman",
            "https://[github.com/Latias94/merman",
        ]
        for repository in invalid_urls:
            with self.subTest(repository=repository), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                write_workspace(root, repository=repository)
                bundle = write_bundle(root)
                with self.assertRaises(generator.CandidateGenerationError):
                    generator.generate_candidates(
                        bundle,
                        root / "candidates",
                        version=VERSION,
                        repo_root=root,
                        verifier=fake_verifier,
                    )

    def test_rejects_missing_or_unsafe_bundle_inputs(self) -> None:
        cases = ["bundle", "archive", "checksum"]
        for missing in cases:
            with self.subTest(missing=missing), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                write_workspace(root)
                bundle = write_bundle(root)
                if missing == "bundle":
                    shutil.rmtree(bundle)
                elif missing == "archive":
                    (bundle / ARCHIVE_NAME).unlink()
                else:
                    (bundle / f"{ARCHIVE_NAME}.sha256").unlink()
                with self.assertRaises(generator.CandidateGenerationError):
                    generator.generate_candidates(
                        bundle,
                        root / "candidates",
                        version=VERSION,
                        repo_root=root,
                        verifier=fake_verifier,
                    )

    def test_rejects_unverified_archive(self) -> None:
        def rejecting_verifier(*_args, **_kwargs):
            raise ArchiveVerificationError("checksum mismatch")

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with self.assertRaisesRegex(ArchiveVerificationError, "checksum mismatch"):
                self.generate(root, verifier=rejecting_verifier)
            self.assertFalse((root / "candidates").exists())

    def test_rejects_malformed_verifier_reports(self) -> None:
        invalid_reports = {
            "target": {"target": "aarch64-pc-windows-msvc"},
            "archive": {"archive": Path("different.zip")},
            "binary": {"binary_path": "nested/merman-cli.exe"},
            "digest-case": {"digest": DIGEST.upper()},
            "digest-length": {"digest": "0" * 63},
            "digest-type": {"digest": None},
            "archive-type": {"archive": ARCHIVE_NAME},
        }
        for name, changes in invalid_reports.items():
            def invalid_verifier(*args, **kwargs):
                report = fake_verifier(*args, **kwargs)
                values = {
                    "archive": report.archive,
                    "digest": report.digest,
                    "target": report.target,
                    "member_count": report.member_count,
                    "total_uncompressed_bytes": report.total_uncompressed_bytes,
                    "binary_path": report.binary_path,
                }
                values.update(changes)
                return VerificationReport(**values)

            with self.subTest(name=name), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                with self.assertRaises(generator.CandidateGenerationError):
                    self.generate(root, verifier=invalid_verifier)

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with self.assertRaisesRegex(
                generator.CandidateGenerationError,
                "invalid report",
            ):
                self.generate(root, verifier=lambda *_args, **_kwargs: None)

    def test_rejects_existing_output_without_modifying_it(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            write_workspace(root)
            bundle = write_bundle(root)
            output = root / "candidates"
            output.mkdir()
            marker = output / "owned-by-user"
            marker.write_text("keep", encoding="utf-8")

            with self.assertRaisesRegex(
                generator.CandidateGenerationError,
                "must not exist",
            ):
                generator.generate_candidates(
                    bundle,
                    output,
                    version=VERSION,
                    repo_root=root,
                    verifier=fake_verifier,
                )
            self.assertEqual(marker.read_text(encoding="utf-8"), "keep")

    def test_output_is_transactional_when_a_write_fails(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            write_workspace(root)
            bundle = write_bundle(root)
            real_write = generator._write_new_file
            call_count = 0

            def fail_second_write(*args, **kwargs):
                nonlocal call_count
                call_count += 1
                if call_count == 2:
                    raise OSError("simulated disk failure")
                return real_write(*args, **kwargs)

            with mock.patch.object(generator, "_write_new_file", side_effect=fail_second_write):
                with self.assertRaisesRegex(OSError, "simulated disk failure"):
                    generator.generate_candidates(
                        bundle,
                        root / "candidates",
                        version=VERSION,
                        repo_root=root,
                        verifier=fake_verifier,
                    )
            self.assertFalse((root / "candidates").exists())
            self.assertEqual(
                {path.name for path in root.iterdir()},
                {"Cargo.toml", "verified"},
            )

    def test_rejects_unknown_embedded_or_structurally_invalid_templates(self) -> None:
        mutations = {
            "unknown": lambda template: template["manifest"].update(
                {"description": "${UNKNOWN}"}
            ),
            "embedded": lambda template: template["manifest"].update(
                {"homepage": "prefix-${REPOSITORY_URL}"}
            ),
            "extra": lambda template: template["manifest"].update({"notes": "unexpected"}),
        }
        for name, mutate in mutations.items():
            with self.subTest(name=name), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                templates = root / "templates"
                shutil.copytree(generator.TEMPLATE_ROOT, templates)
                scoop_path = templates / "scoop.template.json"
                scoop = json.loads(scoop_path.read_text(encoding="utf-8"))
                mutate(scoop)
                scoop_path.write_text(json.dumps(scoop), encoding="utf-8")
                with self.assertRaises(generator.CandidateGenerationError):
                    self.generate(root, template_root=templates)

    def test_cli_reports_contract_errors_without_a_traceback(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            write_workspace(root, version="0.8.0-alpha.1")
            bundle = write_bundle(root)
            stderr = io.StringIO()
            with contextlib.redirect_stderr(stderr):
                exit_code = generator.main(
                    [
                        str(bundle),
                        "--version",
                        "0.8.0-alpha.1",
                        "--output-dir",
                        str(root / "candidates"),
                        "--repo-root",
                        str(root),
                    ]
                )
            self.assertEqual(exit_code, 1)
            self.assertIn("require a stable", stderr.getvalue())
            self.assertNotIn("Traceback", stderr.getvalue())


if __name__ == "__main__":
    unittest.main()
