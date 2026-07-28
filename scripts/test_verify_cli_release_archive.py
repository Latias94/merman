#!/usr/bin/env python3
"""Synthetic tests for the merman-cli release archive verifier."""

from __future__ import annotations

import copy
import hashlib
import io
import json
import os
from pathlib import Path
import stat
import subprocess
import sys
import tarfile
import tempfile
import time
import unittest
from unittest import mock
import uuid
import warnings
import zipfile

sys.path.insert(0, str(Path(__file__).resolve().parent))

import verify_cli_release_archive as verifier


VERSION = "0.8.0-alpha.4"
LINUX_TARGET = "x86_64-unknown-linux-gnu"
WINDOWS_TARGET = "x86_64-pc-windows-msvc"
PROJECT_ROOT = Path(__file__).resolve().parents[1]
CONTRACT_PATHS = (
    "capabilities/artifact-profiles-v1.json",
    "capabilities/feature-surface-v1.json",
    "tools/upstreams/REPOS.lock.json",
    "tools/upstreams/MERMAID_REFERENCE_BUNDLE.json",
)
CLI_RELEASE_COMMANDS = [
    "batch",
    "capabilities",
    "completion",
    "detect",
    "fix",
    "layout",
    "lint",
    "lint-rules",
    "mmdc",
    "parse",
    "render",
]
SOURCE_ASSET_PATHS = (
    "assets/completions/_merman-cli",
    "assets/completions/merman-cli.bash",
    "assets/completions/merman-cli.elv",
    "assets/completions/merman-cli.fish",
    "assets/completions/merman-cli.ps1",
    "assets/man/merman-cli-render.1",
    "assets/man/merman-cli.1",
)
RELEASE_WORKFLOW = PROJECT_ROOT / ".github/workflows/release.yml"


def read_json(root: Path, relative: str) -> dict[str, object]:
    value = json.loads((root / relative).read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise AssertionError(f"{relative} must contain an object")
    return value


def semantic_surface_digest(surface: dict[str, object]) -> str:
    canonical = {
        "schema_version": surface["schema_version"],
        "descriptor_id": surface["descriptor_id"],
        "targets": sorted(
            (
                {
                    "id": target["id"],
                    "description": target["description"],
                }
                for target in surface["targets"]
            ),
            key=lambda target: target["id"],
        ),
        "capabilities": sorted(
            (
                {
                    "id": capability["id"],
                    "kind": capability["kind"],
                    "description": capability["description"],
                    "targets": sorted(capability["targets"]),
                    "implications": sorted(capability["implications"]),
                    "absence": {
                        "error_id": capability["absence"]["error_id"],
                        "contract": capability["absence"]["contract"],
                    },
                }
                for capability in surface["capabilities"]
            ),
            key=lambda capability: capability["id"],
        ),
        "outputs": sorted(
            (
                {
                    "id": output["id"],
                    "capability": output["capability"],
                    "description": output["description"],
                    "media_type": output["media_type"],
                    "targets": sorted(output["targets"]),
                }
                for output in surface["outputs"]
            ),
            key=lambda output: output["id"],
        ),
        "binding_operations": sorted(
            (
                {
                    "id": operation["id"],
                    "capability": operation["capability"],
                    "description": operation["description"],
                    "media_type": operation["media_type"],
                    "requires_uri": operation["requires_uri"],
                    "targets": sorted(operation["targets"]),
                }
                for operation in surface["binding_operations"]
            ),
            key=lambda operation: operation["id"],
        ),
    }
    encoded = json.dumps(
        canonical,
        ensure_ascii=False,
        separators=(",", ":"),
    ).encode()
    return f"sha256:{hashlib.sha256(encoded).hexdigest()}"


def valid_capabilities_payload(
    repo_root: Path = PROJECT_ROOT,
    *,
    version: str = VERSION,
) -> dict[str, object]:
    profiles = read_json(repo_root, "capabilities/artifact-profiles-v1.json")
    profile = next(
        profile
        for profile in profiles["profiles"]
        if profile["id"] == "cli-release"
    )
    runtime_ids = profile["expected"]["runtime_ids"]
    surface = read_json(repo_root, "capabilities/feature-surface-v1.json")
    bundle = read_json(repo_root, "tools/upstreams/MERMAID_REFERENCE_BUNDLE.json")
    capabilities = [
        {
            "id": capability["id"],
            "kind": capability["kind"],
            "description": capability["description"],
            "implications": sorted(capability["implications"]),
        }
        for capability in sorted(
            surface["capabilities"],
            key=lambda capability: capability["id"],
        )
        if capability["id"] in runtime_ids
    ]
    outputs = [
        {
            "id": output["id"],
            "description": output["description"],
            "media_type": output["media_type"],
        }
        for output in sorted(
            surface["outputs"],
            key=lambda output: output["id"],
        )
        if output["capability"] in runtime_ids
    ]
    return {
        "schema_version": 2,
        "cli_contract_version": 2,
        "package": {"name": "merman-cli", "version": version},
        "compatibility": {
            "mermaid": bundle["release"]["version"],
            "mmdc": bundle["referenceCli"]["package"]["version"],
        },
        "descriptor": {
            "schema_version": surface["schema_version"],
            "digest": semantic_surface_digest(surface),
        },
        "commands": CLI_RELEASE_COMMANDS,
        "capabilities": capabilities,
        "outputs": outputs,
    }


def required_files(target: str) -> dict[str, bytes]:
    binary = verifier._binary_name(target)
    files = {
        binary: b"synthetic executable\n",
        verifier.NOTICE_PATH: b"Third-party notices\n",
        f"{verifier.LICENSE_ROOT}/example/LICENSE": b"Example license\n",
    }
    for source_path in SOURCE_ASSET_PATHS:
        archive_path = source_path.removeprefix("assets/")
        files[archive_path] = f"asset for {source_path}\n".encode()
    return files


def archive_name(target: str) -> str:
    return (
        f"merman-cli-{target}.zip"
        if "windows" in target
        else f"merman-cli-{target}.tar.xz"
    )


def write_checksum(archive: Path, *, digest: str | None = None) -> Path:
    checksum = archive.with_name(f"{archive.name}.sha256")
    value = digest or hashlib.sha256(archive.read_bytes()).hexdigest()
    checksum.write_text(f"{value}  {archive.name}\n", encoding="ascii")
    return checksum


def tar_info(name: str, data: bytes, *, mode: int = 0o644) -> tarfile.TarInfo:
    info = tarfile.TarInfo(name)
    info.size = len(data)
    info.mode = mode
    return info


def write_tar(
    root: Path,
    *,
    target: str = LINUX_TARGET,
    files: dict[str, bytes] | None = None,
    additions: list[tuple[tarfile.TarInfo, bytes | None]] | None = None,
    wrapper: str | None = None,
) -> tuple[Path, Path]:
    archive = root / archive_name(target)
    prefix = wrapper or archive.name.removesuffix(".tar.xz")
    payloads = required_files(target) if files is None else files
    with tarfile.open(archive, "w:xz") as output:
        directory = tarfile.TarInfo(prefix)
        directory.type = tarfile.DIRTYPE
        directory.mode = 0o755
        output.addfile(directory)
        for relative, data in payloads.items():
            mode = 0o755 if relative == verifier._binary_name(target) else 0o644
            info = tar_info(f"{prefix}/{relative}", data, mode=mode)
            output.addfile(info, io.BytesIO(data))
        for info, data in additions or []:
            output.addfile(info, io.BytesIO(data) if data is not None else None)
    return archive, write_checksum(archive)


def zip_info(name: str, *, mode: int = stat.S_IFREG | 0o644) -> zipfile.ZipInfo:
    info = zipfile.ZipInfo(name)
    info.create_system = 3
    info.external_attr = mode << 16
    info.compress_type = zipfile.ZIP_DEFLATED
    return info


def write_zip(
    root: Path,
    *,
    target: str = WINDOWS_TARGET,
    files: dict[str, bytes] | None = None,
    additions: list[tuple[zipfile.ZipInfo, bytes]] | None = None,
    wrapper: str | None = None,
) -> tuple[Path, Path]:
    archive = root / archive_name(target)
    payloads = required_files(target) if files is None else files
    with zipfile.ZipFile(archive, "w") as output:
        for relative, data in payloads.items():
            name = f"{wrapper}/{relative}" if wrapper else relative
            output.writestr(zip_info(name), data)
        with warnings.catch_warnings():
            warnings.simplefilter("ignore", UserWarning)
            for info, data in additions or []:
                output.writestr(info, data)
    return archive, write_checksum(archive)


def write_repo_assets(
    root: Path,
    files: dict[str, bytes],
) -> None:
    for relative in CONTRACT_PATHS:
        path = root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes((PROJECT_ROOT / relative).read_bytes())
    for source_relative in SOURCE_ASSET_PATHS:
        archive_relative = source_relative.removeprefix("assets/")
        path = root / "crates/merman-cli" / source_relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(files[archive_relative])
    notice = root / verifier.NOTICE_PATH
    notice.write_bytes(files[verifier.NOTICE_PATH])
    for relative, data in files.items():
        if relative.startswith(f"{verifier.LICENSE_ROOT}/"):
            path = root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
    subprocess.run(["git", "init", "-q"], cwd=root, check=True)
    subprocess.run(
        [
            "git",
            "add",
            "--",
            verifier.NOTICE_PATH,
            verifier.LICENSE_ROOT,
        ],
        cwd=root,
        check=True,
    )


def verify_archive(
    archive: Path,
    checksum: Path,
    *,
    repo_root: Path | None = None,
    verified_output: Path | None = None,
    **kwargs: object,
) -> verifier.VerificationReport:
    target = kwargs.get("target")
    if not isinstance(target, str):
        raise AssertionError("test verification requires a target")
    if repo_root is None:
        repo_root = archive.parent / f"repo-{uuid.uuid4().hex}"
        write_repo_assets(repo_root, required_files(target))
    if verified_output is None:
        output_dir = archive.parent / f"verified-{uuid.uuid4().hex}"
        output_dir.mkdir()
        verified_output = output_dir / archive.name
    return verifier.verify_release_archive(
        archive,
        checksum,
        repo_root=repo_root,
        verified_output=verified_output,
        **kwargs,
    )


class SuccessfulArchiveTests(unittest.TestCase):
    def test_valid_tar_with_repository_comparison(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            files = required_files(LINUX_TARGET)
            archive, checksum = write_tar(root, files=files)
            repo = root / "repo"
            write_repo_assets(repo, files)

            report = verify_archive(
                archive,
                checksum,
                target=LINUX_TARGET,
                version=VERSION,
                repo_root=repo,
            )

            self.assertEqual(report.binary_path, "merman-cli")
            self.assertEqual(report.member_count, len(files))
            self.assertGreater(report.total_uncompressed_bytes, 0)
            self.assertEqual(report.digest, hashlib.sha256(report.archive.read_bytes()).hexdigest())

    def test_valid_windows_zip_with_windows_attributes(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            files = required_files(WINDOWS_TARGET)
            archive = root / archive_name(WINDOWS_TARGET)
            with zipfile.ZipFile(archive, "w") as output:
                for relative, data in files.items():
                    info = zipfile.ZipInfo(relative)
                    info.create_system = 0
                    info.external_attr = 0x20
                    output.writestr(info, data)
            checksum = write_checksum(archive)

            report = verify_archive(
                archive,
                checksum,
                target=WINDOWS_TARGET,
                version=VERSION,
            )

            self.assertEqual(report.binary_path, "merman-cli.exe")

    def test_checksum_accepts_digest_only_and_sha256_prefix(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            archive, checksum = write_tar(root)
            digest = hashlib.sha256(archive.read_bytes()).hexdigest()
            for text in (f"{digest}\n", f"sha256:{digest}\n"):
                with self.subTest(text=text.split(":")[0]):
                    checksum.write_text(text, encoding="ascii")
                    verify_archive(
                        archive,
                        checksum,
                        target=LINUX_TARGET,
                        version=VERSION,
                    )


class ChecksumAndNamingTests(unittest.TestCase):
    def test_checksum_is_verified_before_archive_parsing(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            archive = root / archive_name(LINUX_TARGET)
            archive.write_bytes(b"not an archive")
            checksum = write_checksum(archive, digest="0" * 64)

            with self.assertRaisesRegex(verifier.ArchiveVerificationError, "SHA-256 mismatch"):
                verify_archive(
                    archive,
                    checksum,
                    target=LINUX_TARGET,
                    version=VERSION,
                )

    def test_checksum_must_be_adjacent_and_name_the_archive(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            archive, checksum = write_tar(root)
            other = root / "other.sha256"
            other.write_bytes(checksum.read_bytes())
            with self.assertRaisesRegex(verifier.ArchiveVerificationError, "adjacent"):
                verify_archive(
                    archive,
                    other,
                    target=LINUX_TARGET,
                    version=VERSION,
                )

            checksum.write_text(
                f"{hashlib.sha256(archive.read_bytes()).hexdigest()}  other.tar.xz\n",
                encoding="ascii",
            )
            with self.assertRaisesRegex(verifier.ArchiveVerificationError, "does not match"):
                verify_archive(
                    archive,
                    checksum,
                    target=LINUX_TARGET,
                    version=VERSION,
                )

    def test_archive_name_and_extension_must_match_target(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            archive, checksum = write_tar(root)
            with self.assertRaisesRegex(verifier.ArchiveVerificationError, "archive name"):
                verify_archive(
                    archive,
                    checksum,
                    target="aarch64-apple-darwin",
                    version=VERSION,
                )


class PathSafetyTests(unittest.TestCase):
    def test_rejects_absolute_traversal_backslash_drive_and_nul_paths(self) -> None:
        cases = (
            "/absolute",
            "../escape",
            "safe/../../escape",
            "safe\\escape",
            "C:/escape",
            "safe/\x00escape",
        )
        for name in cases:
            with self.subTest(name=name), self.assertRaises(
                verifier.ArchiveVerificationError
            ):
                verifier._normalize_member_name(name, is_directory=False)

    def test_rejects_windows_devices_and_nonportable_components(self) -> None:
        for name in ("CON", "licenses/aux.txt", "trailing.", "trailing ", "a:b", "a?b"):
            with self.subTest(name=name), self.assertRaises(
                verifier.ArchiveVerificationError
            ):
                verifier._normalize_member_name(name, is_directory=False)

    def test_tar_rejects_member_outside_expected_wrapper(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            info = tar_info("other-root/extra", b"x")
            archive, checksum = write_tar(root, additions=[(info, b"x")])
            with self.assertRaisesRegex(verifier.ArchiveVerificationError, "top-level"):
                verify_archive(
                    archive,
                    checksum,
                    target=LINUX_TARGET,
                    version=VERSION,
                )

    def test_zip_rejects_tar_style_wrapper(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            wrapper = archive_name(WINDOWS_TARGET).removesuffix(".zip")
            archive, checksum = write_zip(root, wrapper=wrapper)
            with self.assertRaisesRegex(verifier.ArchiveVerificationError, "must be flat"):
                verify_archive(
                    archive,
                    checksum,
                    target=WINDOWS_TARGET,
                    version=VERSION,
                )

    def test_duplicate_and_portable_path_collisions_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            duplicate = zip_info(verifier.NOTICE_PATH)
            archive, checksum = write_zip(
                root,
                additions=[(duplicate, b"duplicate")],
            )
            with self.assertRaisesRegex(verifier.ArchiveVerificationError, "duplicate"):
                verify_archive(
                    archive,
                    checksum,
                    target=WINDOWS_TARGET,
                    version=VERSION,
                )

            case_collision = zip_info("third_party_notices.md")
            archive, checksum = write_zip(
                root,
                additions=[(case_collision, b"collision")],
            )
            with self.assertRaisesRegex(
                verifier.ArchiveVerificationError,
                "portable path collision",
            ):
                verify_archive(
                    archive,
                    checksum,
                    target=WINDOWS_TARGET,
                    version=VERSION,
                )

    def test_file_cannot_be_an_ancestor_of_another_member(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            archive, checksum = write_zip(
                root,
                additions=[
                    (zip_info("extra"), b"file"),
                    (zip_info("extra/child"), b"child"),
                ],
            )
            with self.assertRaisesRegex(verifier.ArchiveVerificationError, "ancestor"):
                verify_archive(
                    archive,
                    checksum,
                    target=WINDOWS_TARGET,
                    version=VERSION,
                )


class MemberTypeTests(unittest.TestCase):
    def test_tar_rejects_links_devices_and_fifo(self) -> None:
        type_cases = (
            tarfile.SYMTYPE,
            tarfile.LNKTYPE,
            tarfile.CHRTYPE,
            tarfile.BLKTYPE,
            tarfile.FIFOTYPE,
        )
        for member_type in type_cases:
            with self.subTest(member_type=member_type), tempfile.TemporaryDirectory() as temp_dir:
                root = Path(temp_dir)
                prefix = archive_name(LINUX_TARGET).removesuffix(".tar.xz")
                info = tarfile.TarInfo(f"{prefix}/unsafe")
                info.type = member_type
                info.linkname = f"{prefix}/merman-cli"
                archive, checksum = write_tar(root, additions=[(info, None)])
                with self.assertRaisesRegex(
                    verifier.ArchiveVerificationError,
                    "regular file or directory",
                ):
                    verify_archive(
                        archive,
                        checksum,
                        target=LINUX_TARGET,
                        version=VERSION,
                    )

    def test_zip_rejects_unix_symlink(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            link = zip_info("unsafe", mode=stat.S_IFLNK | 0o777)
            archive, checksum = write_zip(root, additions=[(link, b"merman-cli.exe")])
            with self.assertRaisesRegex(
                verifier.ArchiveVerificationError,
                "regular file or directory",
            ):
                verify_archive(
                    archive,
                    checksum,
                    target=WINDOWS_TARGET,
                    version=VERSION,
                )

    def test_zip_rejects_windows_device_volume_and_inconsistent_directory(self) -> None:
        attributes = (0x40, 0x08, 0x10, 0x400)
        for attribute in attributes:
            with self.subTest(attribute=attribute), tempfile.TemporaryDirectory() as temp_dir:
                root = Path(temp_dir)
                info = zipfile.ZipInfo("unsafe")
                info.create_system = 0
                info.external_attr = attribute
                archive, checksum = write_zip(root, additions=[(info, b"unsafe")])
                with self.assertRaises(verifier.ArchiveVerificationError):
                    verify_archive(
                        archive,
                        checksum,
                        target=WINDOWS_TARGET,
                        version=VERSION,
                    )


class BudgetTests(unittest.TestCase):
    def test_rejects_oversized_member(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            files = required_files(WINDOWS_TARGET)
            largest = max(len(data) for data in files.values())
            archive, checksum = write_zip(root, files=files)
            limits = verifier.ExtractionLimits(
                max_archive_size=1024 * 1024,
                max_member_size=largest - 1,
                max_total_size=1024 * 1024,
                max_members=100,
                max_path_bytes=1024,
                max_path_components=64,
            )
            with self.assertRaisesRegex(verifier.ArchiveVerificationError, "member .* exceeds"):
                verify_archive(
                    archive,
                    checksum,
                    target=WINDOWS_TARGET,
                    version=VERSION,
                    limits=limits,
                )

    def test_rejects_aggregate_expansion_budget(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            files = required_files(WINDOWS_TARGET)
            total = sum(len(data) for data in files.values())
            archive, checksum = write_zip(root, files=files)
            limits = verifier.ExtractionLimits(
                max_archive_size=1024 * 1024,
                max_member_size=1024 * 1024,
                max_total_size=total - 1,
                max_members=100,
                max_path_bytes=1024,
                max_path_components=64,
            )
            with self.assertRaisesRegex(verifier.ArchiveVerificationError, "uncompressed"):
                verify_archive(
                    archive,
                    checksum,
                    target=WINDOWS_TARGET,
                    version=VERSION,
                    limits=limits,
                )

    def test_rejects_member_count_and_archive_size_budgets(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            archive, checksum = write_zip(root)
            for field, value, message in (
                ("max_members", 1, "member count"),
                ("max_archive_size", 1, "archive size"),
            ):
                values = dict(vars(verifier.DEFAULT_LIMITS))
                values[field] = value
                with self.subTest(field=field), self.assertRaisesRegex(
                    verifier.ArchiveVerificationError,
                    message,
                ):
                    verify_archive(
                        archive,
                        checksum,
                        target=WINDOWS_TARGET,
                        version=VERSION,
                        limits=verifier.ExtractionLimits(**values),
                    )


class RequiredContentsTests(unittest.TestCase):
    def test_missing_required_file_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            files = required_files(WINDOWS_TARGET)
            del files["man/merman-cli-render.1"]
            archive, checksum = write_zip(root, files=files)
            with self.assertRaisesRegex(
                verifier.ArchiveVerificationError,
                "CLI asset set",
            ):
                verify_archive(
                    archive,
                    checksum,
                    target=WINDOWS_TARGET,
                    version=VERSION,
                )

    def test_assets_prefixed_layout_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            files = required_files(WINDOWS_TARGET)
            files["assets/completions/_merman-cli"] = b"duplicate layout entry\n"
            archive, checksum = write_zip(root, files=files)

            with self.assertRaisesRegex(
                verifier.ArchiveVerificationError,
                "assets-prefixed CLI layout",
            ):
                verify_archive(
                    archive,
                    checksum,
                    target=WINDOWS_TARGET,
                    version=VERSION,
                )

    def test_empty_assets_prefixed_layout_directory_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            directory = zip_info(
                "assets/completions/",
                mode=stat.S_IFDIR | 0o755,
            )
            directory.compress_type = zipfile.ZIP_STORED
            archive, checksum = write_zip(
                root,
                additions=[(directory, b"")],
            )

            with self.assertRaisesRegex(
                verifier.ArchiveVerificationError,
                "assets-prefixed CLI layout",
            ):
                verify_archive(
                    archive,
                    checksum,
                    target=WINDOWS_TARGET,
                    version=VERSION,
                )

    def test_unexpected_root_asset_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            files = required_files(WINDOWS_TARGET)
            files["man/unexpected.1"] = b"unexpected manpage\n"
            archive, checksum = write_zip(root, files=files)

            with self.assertRaisesRegex(verifier.ArchiveVerificationError, "unexpected"):
                verify_archive(
                    archive,
                    checksum,
                    target=WINDOWS_TARGET,
                    version=VERSION,
                )

    def test_nested_second_binary_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            archive, checksum = write_zip(
                root,
                additions=[(zip_info("other/merman-cli.exe"), b"duplicate binary")],
            )
            with self.assertRaisesRegex(verifier.ArchiveVerificationError, "exactly one"):
                verify_archive(
                    archive,
                    checksum,
                    target=WINDOWS_TARGET,
                    version=VERSION,
                )

    def test_license_set_must_be_nonempty_and_each_file_nonempty(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            no_licenses = required_files(WINDOWS_TARGET)
            del no_licenses[f"{verifier.LICENSE_ROOT}/example/LICENSE"]
            archive, checksum = write_zip(root, files=no_licenses)
            with self.assertRaisesRegex(verifier.ArchiveVerificationError, "legal file set"):
                verify_archive(
                    archive,
                    checksum,
                    target=WINDOWS_TARGET,
                    version=VERSION,
                )

            empty_license = required_files(WINDOWS_TARGET)
            empty_license[f"{verifier.LICENSE_ROOT}/example/LICENSE"] = b""
            archive, checksum = write_zip(root, files=empty_license)
            with self.assertRaisesRegex(verifier.ArchiveVerificationError, "empty"):
                verify_archive(
                    archive,
                    checksum,
                    target=WINDOWS_TARGET,
                    version=VERSION,
                )

    def test_repository_asset_mismatch_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            files = required_files(LINUX_TARGET)
            archive, checksum = write_tar(root, files=files)
            repo = root / "repo"
            write_repo_assets(repo, files)
            (
                repo
                / "crates/merman-cli/assets/man"
                / "merman-cli-render.1"
            ).write_bytes(b"changed\n")

            with self.assertRaisesRegex(verifier.ArchiveVerificationError, "differs"):
                verify_archive(
                    archive,
                    checksum,
                    target=LINUX_TARGET,
                    version=VERSION,
                    repo_root=repo,
                )

    def test_untracked_legal_files_are_ignored_but_not_accepted_in_archive(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            files = required_files(LINUX_TARGET)
            archive, checksum = write_tar(root, files=files)
            repo = root / "repo"
            write_repo_assets(repo, files)
            scratch = repo / verifier.LICENSE_ROOT / "scratch.txt"
            scratch.write_text("not part of the release\n", encoding="utf-8")

            verify_archive(
                archive,
                checksum,
                target=LINUX_TARGET,
                version=VERSION,
                repo_root=repo,
            )

            archive_files = dict(files)
            archive_files[f"{verifier.LICENSE_ROOT}/scratch.txt"] = scratch.read_bytes()
            archive, checksum = write_tar(root, files=archive_files)
            with self.assertRaisesRegex(
                verifier.ArchiveVerificationError,
                "unexpected",
            ):
                verify_archive(
                    archive,
                    checksum,
                    target=LINUX_TARGET,
                    version=VERSION,
                    repo_root=repo,
                )


class AdversarialRegressionTests(unittest.TestCase):
    def test_unicode_normalization_and_casefolded_ancestor_collisions_are_rejected(
        self,
    ) -> None:
        unicode_collision = [
            verifier.ArchiveMember("café", "café", False, 1, 0o644, None),
            verifier.ArchiveMember("cafe\u0301", "cafe\u0301", False, 1, 0o644, None),
        ]
        with self.assertRaisesRegex(
            verifier.ArchiveVerificationError,
            "portable path collision",
        ):
            verifier._validate_member_set(
                unicode_collision,
                limits=verifier.DEFAULT_LIMITS,
            )

        casefolded_ancestor = [
            verifier.ArchiveMember("Extra", "Extra", False, 1, 0o644, None),
            verifier.ArchiveMember(
                "extra/child",
                "extra/child",
                False,
                1,
                0o644,
                None,
            ),
        ]
        with self.assertRaisesRegex(
            verifier.ArchiveVerificationError,
            "portable ancestor",
        ):
            verifier._validate_member_set(
                casefolded_ancestor,
                limits=verifier.DEFAULT_LIMITS,
            )

    def test_tar_rejects_special_permission_bits(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            prefix = archive_name(LINUX_TARGET).removesuffix(".tar.xz")
            privileged = tar_info(
                f"{prefix}/privileged",
                b"privileged",
                mode=0o4755,
            )
            archive, checksum = write_tar(
                root,
                additions=[(privileged, b"privileged")],
            )

            with self.assertRaisesRegex(
                verifier.ArchiveVerificationError,
                "special permission",
            ):
                verify_archive(
                    archive,
                    checksum,
                    target=LINUX_TARGET,
                    version=VERSION,
                )

    def test_tar_and_zip_reject_executable_resource_files(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            prefix = archive_name(LINUX_TARGET).removesuffix(".tar.xz")
            executable_resource = tar_info(
                f"{prefix}/unexpected-tool",
                b"tool",
                mode=0o755,
            )
            archive, checksum = write_tar(
                root,
                additions=[(executable_resource, b"tool")],
            )
            with self.assertRaisesRegex(
                verifier.ArchiveVerificationError,
                "resource file is executable",
            ):
                verify_archive(
                    archive,
                    checksum,
                    target=LINUX_TARGET,
                    version=VERSION,
                )

            executable_resource_zip = zip_info(
                "unexpected-tool",
                mode=stat.S_IFREG | 0o755,
            )
            archive, checksum = write_zip(
                root,
                additions=[(executable_resource_zip, b"tool")],
            )
            with self.assertRaisesRegex(
                verifier.ArchiveVerificationError,
                "resource file is executable",
            ):
                verify_archive(
                    archive,
                    checksum,
                    target=WINDOWS_TARGET,
                    version=VERSION,
                )

    def test_pax_metadata_counts_toward_the_tar_stream_budget(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            archive = root / archive_name(LINUX_TARGET)
            prefix = archive.name.removesuffix(".tar.xz")
            files = required_files(LINUX_TARGET)
            with tarfile.open(archive, "w:xz", format=tarfile.PAX_FORMAT) as output:
                directory = tarfile.TarInfo(prefix)
                directory.type = tarfile.DIRTYPE
                directory.mode = 0o755
                output.addfile(directory)
                for relative, data in files.items():
                    mode = 0o755 if relative == verifier._binary_name(LINUX_TARGET) else 0o644
                    info = tar_info(f"{prefix}/{relative}", data, mode=mode)
                    if relative == verifier.NOTICE_PATH:
                        info.pax_headers = {"comment": "A" * (2 * 1024 * 1024)}
                    output.addfile(info, io.BytesIO(data))
            checksum = write_checksum(archive)
            logical_size = sum(len(data) for data in files.values())
            limits = verifier.ExtractionLimits(
                max_archive_size=1024 * 1024,
                max_member_size=1024 * 1024,
                max_total_size=logical_size,
                max_members=100,
                max_path_bytes=1024,
                max_path_components=64,
            )

            with self.assertRaisesRegex(
                verifier.ArchiveVerificationError,
                "tar stream",
            ):
                verify_archive(
                    archive,
                    checksum,
                    target=LINUX_TARGET,
                    version=VERSION,
                    limits=limits,
                )

    def test_repository_manpage_inventory_is_exact(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            files = required_files(LINUX_TARGET)
            archive_files = dict(files)
            del archive_files["man/merman-cli-render.1"]
            archive, checksum = write_tar(root, files=archive_files)
            repo = root / "repo"
            write_repo_assets(repo, files)

            with self.assertRaisesRegex(
                verifier.ArchiveVerificationError,
                "asset set",
            ):
                verify_archive(
                    archive,
                    checksum,
                    target=LINUX_TARGET,
                    version=VERSION,
                    repo_root=repo,
                )

    def test_verified_output_survives_source_replacement(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            archive, checksum = write_tar(root)
            verified_output = root / "verified" / archive.name
            verified_output.parent.mkdir()

            report = verify_archive(
                archive,
                checksum,
                target=LINUX_TARGET,
                version=VERSION,
                verified_output=verified_output,
            )
            expected = verified_output.read_bytes()
            archive.write_bytes(b"replaced after verification")

            self.assertEqual(report.archive, verified_output.resolve())
            self.assertEqual(verified_output.read_bytes(), expected)
            if os.name == "posix":
                self.assertEqual(stat.S_IMODE(report.archive.stat().st_mode), 0o400)

    def test_verified_output_never_overwrites_an_existing_file(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            archive, checksum = write_tar(root)
            verified_output = root / "verified" / archive.name
            verified_output.parent.mkdir()
            verified_output.write_bytes(b"keep me")

            with self.assertRaisesRegex(
                verifier.ArchiveVerificationError,
                "must not already exist",
            ):
                verify_archive(
                    archive,
                    checksum,
                    target=LINUX_TARGET,
                    version=VERSION,
                    verified_output=verified_output,
                )
            self.assertEqual(verified_output.read_bytes(), b"keep me")

    def test_cli_requires_repository_and_verified_output(self) -> None:
        arguments = [
            "merman-cli-x86_64-unknown-linux-gnu.tar.xz",
            "--target",
            LINUX_TARGET,
            "--version",
            VERSION,
        ]
        with (
            mock.patch.object(sys, "stderr", io.StringIO()),
            self.assertRaises(SystemExit),
        ):
            verifier.parse_args(arguments)


class RuntimeContractTests(unittest.TestCase):
    def assert_runtime_payload_rejected(
        self,
        repo_root: Path,
        payload: dict[str, object] | bytes,
        message: str,
    ) -> None:
        encoded = (
            payload
            if isinstance(payload, bytes)
            else json.dumps(payload).encode()
        )
        pending = [
            subprocess.CompletedProcess(
                [],
                0,
                stdout=f"merman-cli {VERSION}\n".encode(),
                stderr=b"",
            ),
            subprocess.CompletedProcess([], 0, stdout=encoded, stderr=b""),
        ]

        def runner(
            _command: list[str],
            **_kwargs: object,
        ) -> subprocess.CompletedProcess[bytes]:
            return pending.pop(0)

        with self.assertRaisesRegex(verifier.ArchiveVerificationError, message):
            verifier.verify_runtime_contract(
                Path("/synthetic/merman-cli"),
                target=LINUX_TARGET,
                version=VERSION,
                repo_root=repo_root,
                runner=runner,
                host_target_checker=lambda _target: True,
            )

    def test_runtime_contract_can_be_verified_with_an_injected_runner(self) -> None:
        calls: list[tuple[list[str], bytes]] = []

        def runner(command: list[str], **kwargs: object) -> subprocess.CompletedProcess[bytes]:
            stdin = kwargs["input"]
            self.assertIsInstance(stdin, bytes)
            calls.append((command, stdin))
            if command[-1] == "--version":
                stdout = f"merman-cli {VERSION}\n".encode()
            elif command[-2:] == ["capabilities", "--json"]:
                stdout = json.dumps(valid_capabilities_payload()).encode()
            else:
                stdout = b'<svg xmlns="http://www.w3.org/2000/svg"></svg>\n'
            return subprocess.CompletedProcess(command, 0, stdout=stdout, stderr=b"")

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            archive, checksum = write_tar(root)
            verify_archive(
                archive,
                checksum,
                target=LINUX_TARGET,
                version=VERSION,
                execute=True,
                runner=runner,
                host_target_checker=lambda _target: True,
            )

        self.assertEqual(len(calls), 3)
        self.assertEqual(calls[2][1], verifier.SVG_SMOKE_SOURCE)

    def test_runtime_rejects_version_schema_and_invalid_svg(self) -> None:
        binary = Path("/synthetic/merman-cli")
        outputs = (
            [
                subprocess.CompletedProcess([], 0, stdout=b"wrong\n", stderr=b""),
            ],
            [
                subprocess.CompletedProcess(
                    [],
                    0,
                    stdout=f"merman-cli {VERSION}\n".encode(),
                    stderr=b"",
                ),
                subprocess.CompletedProcess(
                    [],
                    0,
                    stdout=b'{"schema_version":999,"package":{"name":"merman-cli","version":"'
                    + VERSION.encode()
                    + b'"}}',
                    stderr=b"",
                ),
            ],
            [
                subprocess.CompletedProcess(
                    [],
                    0,
                    stdout=f"merman-cli {VERSION}\n".encode(),
                    stderr=b"",
                ),
                subprocess.CompletedProcess(
                    [],
                    0,
                    stdout=json.dumps(valid_capabilities_payload()).encode(),
                    stderr=b"",
                ),
                subprocess.CompletedProcess([], 0, stdout=b"not svg", stderr=b""),
            ],
        )
        with tempfile.TemporaryDirectory() as temp_dir:
            repo_root = Path(temp_dir)
            write_repo_assets(repo_root, required_files(LINUX_TARGET))
            for responses in outputs:
                with self.subTest(responses=len(responses)):
                    pending = list(responses)

                    def runner(
                        _command: list[str],
                        **_kwargs: object,
                    ) -> subprocess.CompletedProcess[bytes]:
                        return pending.pop(0)

                    with self.assertRaises(verifier.ArchiveVerificationError):
                        verifier.verify_runtime_contract(
                            binary,
                            target=LINUX_TARGET,
                            version=VERSION,
                            repo_root=repo_root,
                            runner=runner,
                            host_target_checker=lambda _target: True,
                        )

    def test_runtime_rejects_document_shape_and_scalar_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            repo_root = Path(temp_dir)
            write_repo_assets(repo_root, required_files(LINUX_TARGET))
            mutations = (
                ("missing", lambda value: value.pop("cli_contract_version")),
                ("extra", lambda value: value.__setitem__("unknown", None)),
                (
                    "type",
                    lambda value: value.__setitem__("cli_contract_version", "2"),
                ),
                (
                    "schema_version",
                    lambda value: value.__setitem__("schema_version", 999),
                ),
                (
                    "cli_contract_version",
                    lambda value: value.__setitem__("cli_contract_version", 999),
                ),
                (
                    "package",
                    lambda value: value["package"].__setitem__("unknown", None),
                ),
                (
                    "package.name",
                    lambda value: value["package"].__setitem__("name", "other"),
                ),
                (
                    "package.version",
                    lambda value: value["package"].__setitem__("version", "0.0.0"),
                ),
                (
                    "compatibility",
                    lambda value: value["compatibility"].__setitem__(
                        "mermaid",
                        "0.0.0",
                    ),
                ),
                (
                    "compatibility.mmdc",
                    lambda value: value["compatibility"].__setitem__(
                        "mmdc",
                        "0.0.0",
                    ),
                ),
                (
                    "descriptor",
                    lambda value: value["descriptor"].__setitem__(
                        "schema_version",
                        True,
                    ),
                ),
                (
                    "descriptor.digest",
                    lambda value: value["descriptor"].__setitem__(
                        "digest",
                        "sha256:" + "0" * 64,
                    ),
                ),
            )
            for label, mutate in mutations:
                with self.subTest(label=label):
                    payload = copy.deepcopy(valid_capabilities_payload())
                    mutate(payload)
                    self.assert_runtime_payload_rejected(
                        repo_root,
                        payload,
                        label,
                    )

            duplicate = json.dumps(valid_capabilities_payload()).encode().replace(
                b'{"schema_version": 2,',
                b'{"schema_version": 2, "schema_version": 2,',
                1,
            )
            self.assert_runtime_payload_rejected(repo_root, duplicate, "duplicate")

    def test_runtime_rejects_command_set_and_order_drift(self) -> None:
        mutations = (
            ("missing", lambda values: values.pop()),
            ("extra", lambda values: values.append("unknown")),
            ("duplicate", lambda values: values.append(values[-1])),
            ("type", lambda values: values.__setitem__(0, 7)),
            ("order", lambda values: values.reverse()),
        )
        with tempfile.TemporaryDirectory() as temp_dir:
            repo_root = Path(temp_dir)
            write_repo_assets(repo_root, required_files(LINUX_TARGET))
            for label, mutate in mutations:
                with self.subTest(label=label):
                    payload = copy.deepcopy(valid_capabilities_payload())
                    mutate(payload["commands"])
                    self.assert_runtime_payload_rejected(
                        repo_root,
                        payload,
                        "commands",
                    )

    def test_runtime_rejects_capability_contract_drift(self) -> None:
        mutations = (
            ("missing", lambda values: values.pop()),
            (
                "extra",
                lambda values: values.append(
                    {
                        "id": "unknown",
                        "kind": "tool",
                        "description": "Unknown.",
                        "implications": [],
                    }
                ),
            ),
            ("duplicate", lambda values: values.append(copy.deepcopy(values[-1]))),
            (
                "type",
                lambda values: values[0].__setitem__("description", 7),
            ),
            ("id", lambda values: values[0].__setitem__("id", "unknown")),
            ("kind", lambda values: values[0].__setitem__("kind", "unknown")),
            (
                "description",
                lambda values: values[0].__setitem__("description", "drifted"),
            ),
            (
                "implications",
                lambda values: values[0]["implications"].append("svg"),
            ),
            ("order", lambda values: values.reverse()),
        )
        with tempfile.TemporaryDirectory() as temp_dir:
            repo_root = Path(temp_dir)
            write_repo_assets(repo_root, required_files(LINUX_TARGET))
            for label, mutate in mutations:
                with self.subTest(label=label):
                    payload = copy.deepcopy(valid_capabilities_payload())
                    mutate(payload["capabilities"])
                    self.assert_runtime_payload_rejected(
                        repo_root,
                        payload,
                        "capabilities",
                    )

    def test_runtime_rejects_output_contract_drift(self) -> None:
        mutations = (
            ("missing", lambda values: values.pop()),
            (
                "extra",
                lambda values: values.append(
                    {
                        "id": "unknown",
                        "description": "Unknown.",
                        "media_type": "application/octet-stream",
                    }
                ),
            ),
            ("duplicate", lambda values: values.append(copy.deepcopy(values[-1]))),
            ("type", lambda values: values[0].__setitem__("media_type", 7)),
            ("id", lambda values: values[0].__setitem__("id", "unknown")),
            (
                "description",
                lambda values: values[0].__setitem__("description", "drifted"),
            ),
            (
                "media_type",
                lambda values: values[0].__setitem__(
                    "media_type",
                    "application/octet-stream",
                ),
            ),
            ("order", lambda values: values.reverse()),
        )
        with tempfile.TemporaryDirectory() as temp_dir:
            repo_root = Path(temp_dir)
            write_repo_assets(repo_root, required_files(LINUX_TARGET))
            for label, mutate in mutations:
                with self.subTest(label=label):
                    payload = copy.deepcopy(valid_capabilities_payload())
                    mutate(payload["outputs"])
                    self.assert_runtime_payload_rejected(
                        repo_root,
                        payload,
                        "outputs",
                    )

    def test_repository_sources_drive_versions_and_semantic_digest(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            repo_root = Path(temp_dir)
            write_repo_assets(repo_root, required_files(LINUX_TARGET))
            surface_path = repo_root / "capabilities/feature-surface-v1.json"
            profiles_path = repo_root / "capabilities/artifact-profiles-v1.json"
            bundle_path = (
                repo_root / "tools/upstreams/MERMAID_REFERENCE_BUNDLE.json"
            )

            surface = read_json(repo_root, "capabilities/feature-surface-v1.json")
            surface["capabilities"][0]["description"] = "Changed description."
            surface_path.write_text(
                json.dumps(surface, indent=2) + "\n",
                encoding="utf-8",
            )
            with self.assertRaisesRegex(
                verifier.ArchiveVerificationError,
                "capability_authority.digest",
            ):
                verifier._release_capabilities_contract(
                    repo_root,
                    version=VERSION,
                )

            digest = semantic_surface_digest(surface)
            profiles = read_json(
                repo_root,
                "capabilities/artifact-profiles-v1.json",
            )
            profiles["capability_authority"]["digest"] = digest
            profiles_path.write_text(
                json.dumps(profiles, indent=2) + "\n",
                encoding="utf-8",
            )
            expected = verifier._release_capabilities_contract(
                repo_root,
                version=VERSION,
            )
            self.assertEqual(expected["descriptor"]["digest"], digest)
            changed = next(
                capability
                for capability in expected["capabilities"]
                if capability["id"] == surface["capabilities"][0]["id"]
            )
            self.assertEqual(
                changed["description"],
                "Changed description.",
            )

            bundle = read_json(
                repo_root,
                "tools/upstreams/MERMAID_REFERENCE_BUNDLE.json",
            )
            bundle["referenceCli"]["package"]["version"] = "99.1.2"
            bundle_path.write_text(
                json.dumps(bundle, indent=2) + "\n",
                encoding="utf-8",
            )
            expected = verifier._release_capabilities_contract(
                repo_root,
                version=VERSION,
            )
            self.assertEqual(expected["compatibility"]["mmdc"], "99.1.2")

    def test_repository_rejects_mermaid_lock_and_bundle_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            repo_root = Path(temp_dir)
            write_repo_assets(repo_root, required_files(LINUX_TARGET))
            bundle_path = (
                repo_root / "tools/upstreams/MERMAID_REFERENCE_BUNDLE.json"
            )
            bundle = read_json(
                repo_root,
                "tools/upstreams/MERMAID_REFERENCE_BUNDLE.json",
            )
            bundle["release"]["version"] = "0.0.0"
            bundle_path.write_text(
                json.dumps(bundle, indent=2) + "\n",
                encoding="utf-8",
            )

            with self.assertRaisesRegex(
                verifier.ArchiveVerificationError,
                "release.version",
            ):
                verifier._release_capabilities_contract(
                    repo_root,
                    version=VERSION,
                )

    def test_runtime_refuses_cross_target_execution_before_spawning(self) -> None:
        def runner(
            _command: list[str],
            **_kwargs: object,
        ) -> subprocess.CompletedProcess[bytes]:
            self.fail("runner must not be called")

        with self.assertRaisesRegex(verifier.ArchiveVerificationError, "refusing"):
            verifier.verify_runtime_contract(
                Path("merman-cli.exe"),
                target=WINDOWS_TARGET,
                version=VERSION,
                repo_root=PROJECT_ROOT,
                runner=runner,
                host_target_checker=lambda _target: False,
            )

    def test_real_subprocess_output_is_bounded_while_running(self) -> None:
        with (
            tempfile.TemporaryDirectory() as temp_dir,
            mock.patch.object(verifier, "RUNTIME_OUTPUT_MAX_BYTES", 1024),
        ):
            with self.assertRaisesRegex(
                verifier.ArchiveVerificationError,
                "output exceeds",
            ):
                verifier._run_checked(
                    [
                        sys.executable,
                        "-c",
                        "import os; os.write(1, b'x' * 2048)",
                    ],
                    stdin=b"",
                    cwd=Path(temp_dir),
                    runner=subprocess.run,
                )

    def test_real_subprocess_timeout_uses_the_bounded_runner(self) -> None:
        with (
            tempfile.TemporaryDirectory() as temp_dir,
            mock.patch.object(verifier, "RUNTIME_TIMEOUT_SECONDS", 0.1),
        ):
            with self.assertRaises(subprocess.TimeoutExpired):
                verifier._run_checked(
                    [
                        sys.executable,
                        "-c",
                        "import time; time.sleep(30)",
                    ],
                    stdin=b"",
                    cwd=Path(temp_dir),
                    runner=subprocess.run,
                )

    @unittest.skipUnless(os.name == "posix", "POSIX process groups are required")
    def test_timeout_terminates_descendants_in_the_runtime_process_group(self) -> None:
        script = (
            "import subprocess, sys; "
            "child = subprocess.Popen([sys.executable, '-c', "
            "'import time; time.sleep(30)']); "
            "print(child.pid, flush=True)"
        )
        with (
            tempfile.TemporaryDirectory() as temp_dir,
            mock.patch.object(verifier, "RUNTIME_TIMEOUT_SECONDS", 0.1),
        ):
            with self.assertRaises(subprocess.TimeoutExpired) as raised:
                verifier._run_checked(
                    [sys.executable, "-c", script],
                    stdin=b"",
                    cwd=Path(temp_dir),
                    runner=subprocess.run,
                )

        child_pid = int(raised.exception.output.strip())
        for _attempt in range(100):
            try:
                os.kill(child_pid, 0)
            except ProcessLookupError:
                break
            time.sleep(0.01)
        else:
            self.fail(f"runtime descendant {child_pid} survived process-group timeout")


class ReleaseWorkflowWiringTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        workflow = RELEASE_WORKFLOW.read_text(encoding="utf-8")
        cls.verification_job, host_job = workflow.split(
            "\n  verify-cli-archives:\n",
            maxsplit=1,
        )[1].split("\n  host:\n", maxsplit=1)
        cls.host_job = host_job

    def test_all_cli_archives_are_structurally_verified(self) -> None:
        self.assertIn("-name 'merman-cli-*.tar.xz'", self.verification_job)
        self.assertIn("-name 'merman-cli-*.zip'", self.verification_job)
        self.assertIn('for archive in "${cli_archives[@]}"', self.verification_job)
        self.assertIn(
            "python3 scripts/verify_cli_release_archive.py",
            self.verification_job,
        )
        for argument in [
            '--checksum "$archive.sha256"',
            '--target "$target"',
            '--version "$release_version"',
            '--repo-root "$GITHUB_WORKSPACE"',
            '--verified-output "$VERIFIED_CLI_DIR/$archive_name"',
        ]:
            with self.subTest(argument=argument):
                self.assertIn(argument, self.verification_job)

    def test_cli_archive_targets_match_the_release_profile_exactly(self) -> None:
        self.assertIn(
            "artifact_profile_recipe.py cli-release --field triples",
            self.verification_job,
        )
        for diagnostic in [
            "Unexpected merman-cli archive target",
            "Duplicate merman-cli archive target",
            "Missing merman-cli archive target",
        ]:
            with self.subTest(diagnostic=diagnostic):
                self.assertIn(diagnostic, self.verification_job)

    def test_smoke_execution_is_limited_to_the_host_target(self) -> None:
        self.assertIn(
            """if [[ "$target" == "$host_target" ]]; then
              execute_args+=(--execute)
              host_archive_executed=true
            fi""",
            self.verification_job,
        )
        self.assertIn('"${execute_args[@]}"', self.verification_job)
        self.assertIn("host_archive_executed=true", self.verification_job)
        self.assertIn(
            "The host merman-cli archive was not executed",
            self.verification_job,
        )

    def test_release_authority_only_downloads_verified_snapshots(self) -> None:
        self.assertIn("name: verified-release-assets", self.verification_job)
        self.assertIn(
            "*-dist-manifest.json|merman-cli-*.tar.xz|merman-cli-*.zip",
            self.verification_job,
        )
        self.assertIn(
            'cp -- "$VERIFIED_CLI_DIR"/* "$RELEASE_ASSET_DIR/"',
            self.verification_job,
        )
        self.assertIn("needs.verify-cli-archives.result == 'success'", self.host_job)
        self.assertIn("name: verified-release-assets", self.host_job)
        self.assertNotIn("pattern: artifacts-*", self.host_job)
        self.assertNotIn("verify_cli_release_archive.py", self.host_job)


if __name__ == "__main__":
    unittest.main()
