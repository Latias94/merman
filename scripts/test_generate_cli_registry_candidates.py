#!/usr/bin/env python3
"""Contract tests for stable Scoop and WinGet draft generation."""

from __future__ import annotations

import contextlib
import io
import json
from pathlib import Path
import shutil
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))

import generate_cli_registry_candidates as generator
from verify_cli_release_archive import ArchiveVerificationError, VerificationReport


VERSION = "0.8.0"
DIGEST = "0123456789abcdef" * 4
ARCHIVE_NAME = "merman-cli-x86_64-pc-windows-msvc.zip"
REPOSITORY_URL = "https://github.com/Latias94/merman"


def write_workspace(
    root: Path,
    *,
    version: str = VERSION,
    repository: str = REPOSITORY_URL,
) -> None:
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
    del archive, checksum, version, repo_root
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
        repository: str = REPOSITORY_URL,
        verifier=fake_verifier,
        template_root: Path = generator.TEMPLATE_ROOT,
    ) -> generator.CandidateSet:
        write_workspace(root, version=version, repository=repository)
        return generator.generate_candidates(
            write_bundle(root),
            root / "candidates",
            version=version,
            repo_root=root,
            template_root=template_root,
            verifier=verifier,
        )

    def test_generates_deterministic_archive_bound_drafts(self) -> None:
        with tempfile.TemporaryDirectory() as first_dir, tempfile.TemporaryDirectory() as second_dir:
            first = self.generate(Path(first_dir))
            second = self.generate(Path(second_dir))
            first_files = generated_files(first.output_dir)

            self.assertEqual(first_files, generated_files(second.output_dir))
            self.assertEqual(
                set(first_files),
                {
                    "scoop/merman-cli.json",
                    "winget/manifests/l/Latias94/MermanCLI/0.8.0/Latias94.MermanCLI.yaml",
                    "winget/manifests/l/Latias94/MermanCLI/0.8.0/Latias94.MermanCLI.installer.yaml",
                    "winget/manifests/l/Latias94/MermanCLI/0.8.0/Latias94.MermanCLI.locale.en-US.yaml",
                },
            )
            self.assertEqual(len(first.manifests), 4)
            self.assertTrue(all(path.is_file() for path in first.manifests))

            scoop = json.loads(first_files["scoop/merman-cli.json"])
            download = scoop["architecture"]["64bit"]
            expected_url = f"{REPOSITORY_URL}/releases/download/v{VERSION}/{ARCHIVE_NAME}"
            self.assertEqual(download, {"url": expected_url, "hash": DIGEST})
            self.assertEqual(scoop["bin"], "merman-cli.exe")
            self.assertEqual(scoop["checkver"], {"github": REPOSITORY_URL})

            installer = first_files[
                "winget/manifests/l/Latias94/MermanCLI/0.8.0/Latias94.MermanCLI.installer.yaml"
            ].decode()
            self.assertIn(f"InstallerUrl: {expected_url}", installer)
            self.assertIn(f"InstallerSha256: {DIGEST.upper()}", installer)
            self.assertIn("NestedInstallerType: portable", installer)
            self.assertIn("RelativeFilePath: merman-cli.exe", installer)

    def test_calls_the_archive_verifier_for_the_exact_windows_asset(self) -> None:
        calls: list[tuple[tuple, dict]] = []

        def recording_verifier(*args, **kwargs):
            calls.append((args, kwargs))
            return fake_verifier(*args, **kwargs)

        with tempfile.TemporaryDirectory() as directory:
            self.generate(Path(directory), verifier=recording_verifier)

        self.assertEqual(len(calls), 1)
        args, kwargs = calls[0]
        self.assertEqual(args[0].name, ARCHIVE_NAME)
        self.assertEqual(args[1].name, f"{ARCHIVE_NAME}.sha256")
        self.assertEqual(kwargs["target"], generator.WINDOWS_TARGET)
        self.assertEqual(kwargs["version"], VERSION)
        self.assertEqual(kwargs["verified_output"].name, ARCHIVE_NAME)

    def test_rejects_nonstable_versions_and_workspace_mismatch(self) -> None:
        for version in ["0.8.0-alpha.1", "0.8.0+build.1", "v0.8.0", "0.8", "latest"]:
            with self.subTest(version=version), tempfile.TemporaryDirectory() as directory:
                with self.assertRaises(generator.CandidateGenerationError):
                    self.generate(Path(directory), version=version)

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            write_workspace(root, version="0.8.1")
            with self.assertRaisesRegex(
                generator.CandidateGenerationError,
                "requested version must exactly match",
            ):
                generator.generate_candidates(
                    write_bundle(root),
                    root / "candidates",
                    version=VERSION,
                    repo_root=root,
                    verifier=fake_verifier,
                )

    def test_rejects_noncanonical_repository_urls(self) -> None:
        for repository in [
            "http://github.com/Latias94/merman",
            "https://github.com/Latias94/merman/",
            "https://github.com/Latias94/merman?ref=main",
        ]:
            with self.subTest(repository=repository), tempfile.TemporaryDirectory() as directory:
                with self.assertRaises(generator.CandidateGenerationError):
                    self.generate(Path(directory), repository=repository)

    def test_rejects_missing_or_unverified_archive_inputs(self) -> None:
        for missing_name in [ARCHIVE_NAME, f"{ARCHIVE_NAME}.sha256"]:
            with self.subTest(missing=missing_name), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                write_workspace(root)
                bundle = write_bundle(root)
                (bundle / missing_name).unlink()
                with self.assertRaises(generator.CandidateGenerationError):
                    generator.generate_candidates(
                        bundle,
                        root / "candidates",
                        version=VERSION,
                        repo_root=root,
                        verifier=fake_verifier,
                    )

        def rejecting_verifier(*_args, **_kwargs):
            raise ArchiveVerificationError("checksum mismatch")

        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaisesRegex(ArchiveVerificationError, "checksum mismatch"):
                self.generate(Path(directory), verifier=rejecting_verifier)

    def test_does_not_overwrite_an_existing_output_directory(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            write_workspace(root)
            bundle = write_bundle(root)
            output = root / "candidates"
            output.mkdir()
            marker = output / "owned-by-user"
            marker.write_text("keep", encoding="utf-8")

            with self.assertRaisesRegex(generator.CandidateGenerationError, "must not exist"):
                generator.generate_candidates(
                    bundle,
                    output,
                    version=VERSION,
                    repo_root=root,
                    verifier=fake_verifier,
                )
            self.assertEqual(marker.read_text(encoding="utf-8"), "keep")

    def test_rejects_unknown_template_placeholders(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            templates = root / "templates"
            shutil.copytree(generator.TEMPLATE_ROOT, templates)
            installer = templates / "winget.installer.template.yaml"
            installer.write_text(
                installer.read_text(encoding="utf-8") + "Unknown: ${UNKNOWN}\n",
                encoding="utf-8",
            )
            with self.assertRaisesRegex(generator.CandidateGenerationError, "unknown placeholder"):
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
