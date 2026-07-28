#!/usr/bin/env python3
"""Verify a cargo-dist merman-cli release archive without trusting its paths."""

from __future__ import annotations

import argparse
from collections.abc import Callable, Iterable
from dataclasses import dataclass
import hashlib
import json
import lzma
import os
from pathlib import Path, PurePosixPath
import platform
import re
import signal
import stat
import subprocess
import sys
import tarfile
import tempfile
import threading
import time
from typing import BinaryIO, TypeAlias
import unicodedata
import xml.etree.ElementTree as ElementTree
import zipfile


PACKAGE_NAME = "merman-cli"
CAPABILITIES_SCHEMA_VERSION = 2
CLI_CONTRACT_VERSION = 2
SVG_SMOKE_SOURCE = b"flowchart LR\nA --> B\n"
CHECKSUM_MAX_BYTES = 4096
REPOSITORY_CONTRACT_MAX_BYTES = 4 * 1024 * 1024
RUNTIME_OUTPUT_MAX_BYTES = 16 * 1024 * 1024
RUNTIME_TIMEOUT_SECONDS = 30
XZ_MEMORY_MAX_BYTES = 128 * 1024 * 1024

ARTIFACT_PROFILES_PATH = "capabilities/artifact-profiles-v1.json"
CAPABILITY_SURFACE_PATH = "capabilities/feature-surface-v1.json"
UPSTREAM_REPOS_PATH = "tools/upstreams/REPOS.lock.json"
MERMAID_REFERENCE_BUNDLE_PATH = "tools/upstreams/MERMAID_REFERENCE_BUNDLE.json"
ASSET_SOURCE_ROOTS = ("assets/completions", "assets/man")
ASSET_ARCHIVE_ROOTS = ("completions", "man")
NOTICE_PATH = "THIRD_PARTY_NOTICES.md"
LICENSE_ROOT = "THIRD_PARTY_LICENSES"

_TARGET_RE = re.compile(r"[A-Za-z0-9_][A-Za-z0-9_.-]{0,127}\Z")
_VERSION_RE = re.compile(r"[0-9A-Za-z][0-9A-Za-z.+-]{0,127}\Z")
_SHA256_RE = re.compile(r"(?:sha256:)?([0-9A-Fa-f]{64})\Z")
_CAPABILITY_KINDS = {"adapter", "api", "engine", "output", "tool"}
_WINDOWS_DEVICE_NAMES = {
    "CON",
    "PRN",
    "AUX",
    "NUL",
    *(f"COM{index}" for index in range(1, 10)),
    *(f"LPT{index}" for index in range(1, 10)),
}

CommandRunner: TypeAlias = Callable[..., subprocess.CompletedProcess[bytes]]
HostTargetChecker: TypeAlias = Callable[[str], bool]


class ArchiveVerificationError(RuntimeError):
    """Raised when a release archive violates the distribution contract."""


@dataclass(frozen=True)
class ExtractionLimits:
    max_archive_size: int = 512 * 1024 * 1024
    max_member_size: int = 256 * 1024 * 1024
    max_total_size: int = 512 * 1024 * 1024
    max_members: int = 10_000
    max_path_bytes: int = 1024
    max_path_components: int = 64

    def __post_init__(self) -> None:
        for field_name, value in vars(self).items():
            if not isinstance(value, int) or isinstance(value, bool) or value <= 0:
                raise ValueError(f"{field_name} must be a positive integer")


DEFAULT_LIMITS = ExtractionLimits()


@dataclass(frozen=True)
class ArchiveMember:
    archive_name: str
    logical_path: str
    is_directory: bool
    size: int
    mode: int
    source: object


@dataclass(frozen=True)
class VerificationReport:
    archive: Path
    digest: str
    target: str
    member_count: int
    total_uncompressed_bytes: int
    binary_path: str


def _portable_path_key(value: str) -> str:
    return "/".join(
        unicodedata.normalize(
            "NFC",
            unicodedata.normalize("NFC", part).casefold(),
        )
        for part in value.split("/")
    )


def _validate_target_and_version(target: str, version: str) -> None:
    if not _TARGET_RE.fullmatch(target):
        raise ArchiveVerificationError(f"invalid Rust target triple: {target!r}")
    if not _VERSION_RE.fullmatch(version):
        raise ArchiveVerificationError(f"invalid package version: {version!r}")


def _archive_extension_for_target(target: str) -> str:
    return ".zip" if "windows" in target.split("-") else ".tar.xz"


def _archive_stem(archive: Path, target: str) -> str:
    extension = _archive_extension_for_target(target)
    expected_name = f"{PACKAGE_NAME}-{target}{extension}"
    if archive.name != expected_name:
        raise ArchiveVerificationError(
            f"archive name must be {expected_name!r}, got {archive.name!r}"
        )
    return expected_name[: -len(extension)]


def _require_regular_input(path: Path, label: str) -> None:
    if path.is_symlink():
        raise ArchiveVerificationError(f"{label} must not be a symlink: {path}")
    try:
        mode = path.stat().st_mode
    except OSError as error:
        raise ArchiveVerificationError(f"cannot inspect {label} {path}: {error}") from error
    if not stat.S_ISREG(mode):
        raise ArchiveVerificationError(f"{label} is not a regular file: {path}")


def _read_checksum(checksum: Path, archive_name: str) -> str:
    try:
        with checksum.open("rb") as stream:
            encoded = stream.read(CHECKSUM_MAX_BYTES + 1)
        if len(encoded) > CHECKSUM_MAX_BYTES:
            raise ArchiveVerificationError(
                f"checksum file exceeds {CHECKSUM_MAX_BYTES} bytes: {checksum}"
            )
        text = encoded.decode("ascii")
    except (OSError, UnicodeDecodeError) as error:
        raise ArchiveVerificationError(f"cannot read checksum file {checksum}: {error}") from error
    lines = text.splitlines()
    if len(lines) != 1 or not lines[0] or lines[0].strip() != lines[0]:
        raise ArchiveVerificationError("checksum file must contain exactly one canonical line")
    fields = lines[0].split()
    if len(fields) not in {1, 2}:
        raise ArchiveVerificationError("checksum line must contain a digest and optional filename")
    match = _SHA256_RE.fullmatch(fields[0])
    if match is None:
        raise ArchiveVerificationError("checksum file does not contain a valid SHA-256 digest")
    if len(fields) == 2 and fields[1].removeprefix("*") != archive_name:
        raise ArchiveVerificationError(
            f"checksum filename does not match archive {archive_name!r}"
        )
    return match.group(1).lower()


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def verify_checksum(
    archive: Path,
    checksum: Path,
    *,
    limits: ExtractionLimits = DEFAULT_LIMITS,
) -> str:
    """Verify the adjacent SHA-256 file before parsing any archive bytes."""
    _require_regular_input(archive, "archive")
    _require_regular_input(checksum, "checksum")
    expected_checksum = archive.with_name(f"{archive.name}.sha256")
    if checksum.resolve() != expected_checksum.resolve():
        raise ArchiveVerificationError(
            f"checksum must be adjacent and named {expected_checksum.name!r}"
        )
    archive_size = archive.stat().st_size
    if archive_size > limits.max_archive_size:
        raise ArchiveVerificationError(
            f"archive size {archive_size} exceeds budget {limits.max_archive_size}"
        )
    expected = _read_checksum(checksum, archive.name)
    observed = _sha256(archive)
    if observed != expected:
        raise ArchiveVerificationError(
            f"SHA-256 mismatch for {archive.name}: expected {expected}, got {observed}"
        )
    return expected


def _copy_verified_snapshot(
    archive: Path,
    destination: Path,
    *,
    expected_digest: str,
    limits: ExtractionLimits,
) -> None:
    """Copy a checksum-bound snapshot so parsing cannot race a path replacement."""
    digest = hashlib.sha256()
    observed_size = 0
    created_destination = False
    try:
        with archive.open("rb") as source:
            try:
                output = destination.open("xb")
            except FileExistsError as error:
                raise ArchiveVerificationError(
                    f"snapshot destination must not already exist: {destination}"
                ) from error
            created_destination = True
            with output:
                while True:
                    chunk = source.read(1024 * 1024)
                    if not chunk:
                        break
                    observed_size += len(chunk)
                    if observed_size > limits.max_archive_size:
                        raise ArchiveVerificationError(
                            f"archive size exceeds budget {limits.max_archive_size}"
                        )
                    digest.update(chunk)
                    output.write(chunk)
                output.flush()
                os.fsync(output.fileno())
    except BaseException:
        if created_destination:
            destination.unlink(missing_ok=True)
        raise
    observed_digest = digest.hexdigest()
    if observed_digest != expected_digest:
        destination.unlink(missing_ok=True)
        raise ArchiveVerificationError(
            "archive changed after checksum verification; refusing to parse an unbound snapshot"
        )


def _normalize_member_name(
    raw_name: str,
    *,
    is_directory: bool,
    limits: ExtractionLimits = DEFAULT_LIMITS,
) -> str:
    if "\x00" in raw_name:
        raise ArchiveVerificationError("archive member path contains NUL")
    if "\\" in raw_name:
        raise ArchiveVerificationError(
            f"archive member path contains a backslash: {raw_name!r}"
        )
    if not raw_name or raw_name.startswith("/") or raw_name.startswith("//"):
        raise ArchiveVerificationError(f"archive member path is absolute or empty: {raw_name!r}")
    name = raw_name[:-1] if is_directory and raw_name.endswith("/") else raw_name
    if not name or name.endswith("/"):
        raise ArchiveVerificationError(f"archive member path is malformed: {raw_name!r}")
    if len(name.encode("utf-8")) > limits.max_path_bytes:
        raise ArchiveVerificationError(f"archive member path exceeds budget: {raw_name!r}")

    parts = name.split("/")
    if len(parts) > limits.max_path_components:
        raise ArchiveVerificationError(f"archive member path is too deep: {raw_name!r}")
    for part in parts:
        if part in {"", ".", ".."}:
            raise ArchiveVerificationError(
                f"archive member path contains traversal or empty components: {raw_name!r}"
            )
        if part.endswith((" ", ".")):
            raise ArchiveVerificationError(
                f"archive member path is not portable to Windows: {raw_name!r}"
            )
        if any(ord(character) < 32 or ord(character) == 127 for character in part):
            raise ArchiveVerificationError(
                f"archive member path contains a control character: {raw_name!r}"
            )
        if any(character in '<>:"|?*' for character in part):
            raise ArchiveVerificationError(
                f"archive member path contains a Windows-special character: {raw_name!r}"
            )
        device_stem = part.split(".", maxsplit=1)[0].upper()
        if device_stem in _WINDOWS_DEVICE_NAMES:
            raise ArchiveVerificationError(
                f"archive member path contains a Windows device name: {raw_name!r}"
            )
    return name


def _logical_tar_path(member_name: str, wrapper: str) -> str:
    if member_name == wrapper:
        return ""
    prefix = f"{wrapper}/"
    if not member_name.startswith(prefix):
        raise ArchiveVerificationError(
            f"tar member is outside required top-level directory {wrapper!r}: {member_name!r}"
        )
    logical = member_name[len(prefix) :]
    if not logical:
        raise ArchiveVerificationError(f"tar member has an empty logical path: {member_name!r}")
    return logical


def _validate_zip_member_type(info: zipfile.ZipInfo, is_directory: bool) -> int:
    unix_mode = (info.external_attr >> 16) & 0xFFFF
    dos_attributes = info.external_attr & 0xFFFF
    if info.create_system == 0:
        allowed_attributes = 0x01 | 0x02 | 0x04 | 0x20
        if is_directory:
            allowed_attributes |= 0x10
        if dos_attributes & ~allowed_attributes:
            raise ArchiveVerificationError(
                f"ZIP member uses a non-ordinary Windows attribute: {info.filename!r}"
            )
        marked_directory = bool(dos_attributes & 0x10)
        if marked_directory != is_directory:
            raise ArchiveVerificationError(
                f"ZIP member has inconsistent Windows directory attributes: {info.filename!r}"
            )
        return 0o755 if is_directory else 0o600

    file_type = stat.S_IFMT(unix_mode)
    allowed_type = stat.S_IFDIR if is_directory else stat.S_IFREG
    if file_type not in {0, allowed_type}:
        raise ArchiveVerificationError(
            f"ZIP member is not a regular file or directory: {info.filename!r}"
        )
    if file_type == stat.S_IFDIR and not is_directory:
        raise ArchiveVerificationError(
            f"ZIP member has inconsistent Unix directory attributes: {info.filename!r}"
        )
    if unix_mode & 0o7000:
        raise ArchiveVerificationError(
            f"ZIP member uses special permission bits: {info.filename!r}"
        )
    return unix_mode & 0o777


def _validate_member_set(
    members: list[ArchiveMember],
    *,
    limits: ExtractionLimits,
) -> None:
    if len(members) > limits.max_members:
        raise ArchiveVerificationError(
            f"archive member count {len(members)} exceeds budget {limits.max_members}"
        )

    exact: set[str] = set()
    portable: dict[str, str] = {}
    by_path: dict[str, ArchiveMember] = {}
    by_portable_path: dict[str, ArchiveMember] = {}
    total_size = 0
    for member in members:
        path = member.logical_path
        if path in exact:
            raise ArchiveVerificationError(f"archive contains duplicate path {path!r}")
        exact.add(path)
        portable_path = _portable_path_key(path)
        previous = portable.get(portable_path)
        if previous is not None:
            raise ArchiveVerificationError(
                f"archive contains a portable path collision: "
                f"{previous!r} and {path!r}"
            )
        portable[portable_path] = path
        by_path[path] = member
        by_portable_path[portable_path] = member
        if member.size < 0:
            raise ArchiveVerificationError(
                f"archive member {path!r} declares a negative size"
            )
        if member.size > limits.max_member_size:
            raise ArchiveVerificationError(
                f"archive member {path!r} size {member.size} exceeds budget "
                f"{limits.max_member_size}"
            )
        total_size += member.size
        if total_size > limits.max_total_size:
            raise ArchiveVerificationError(
                f"archive uncompressed size exceeds budget {limits.max_total_size}"
            )

    for path, member in by_path.items():
        parts = PurePosixPath(path).parts
        for end in range(1, len(parts)):
            parent = "/".join(parts[:end])
            parent_member = by_portable_path.get(_portable_path_key(parent))
            if parent_member is not None and not parent_member.is_directory:
                raise ArchiveVerificationError(
                    f"archive file {parent_member.logical_path!r} is a portable ancestor "
                    f"of {member.logical_path!r}"
                )


def _tar_members(
    archive: tarfile.TarFile,
    *,
    wrapper: str,
    binary_name: str,
    limits: ExtractionLimits,
) -> list[ArchiveMember]:
    members: list[ArchiveMember] = []
    root_seen = False
    raw_paths: set[str] = set()
    total_size = 0
    for index, info in enumerate(archive, start=1):
        if index > limits.max_members:
            raise ArchiveVerificationError(
                f"archive member count exceeds budget {limits.max_members}"
            )
        if not (info.isdir() or info.isreg()):
            raise ArchiveVerificationError(
                f"tar member is not a regular file or directory: {info.name!r}"
            )
        if info.mode & 0o7000:
            raise ArchiveVerificationError(
                f"tar member uses special permission bits: {info.name!r}"
            )
        if info.isdir() and info.size != 0:
            raise ArchiveVerificationError(
                f"tar directory member declares payload bytes: {info.name!r}"
            )
        normalized = _normalize_member_name(
            info.name,
            is_directory=info.isdir(),
            limits=limits,
        )
        if normalized in raw_paths:
            raise ArchiveVerificationError(f"archive contains duplicate path {normalized!r}")
        raw_paths.add(normalized)
        logical = _logical_tar_path(normalized, wrapper)
        if not logical:
            if not info.isdir() or root_seen:
                raise ArchiveVerificationError(
                    f"tar top-level entry {wrapper!r} must be one directory"
                )
            root_seen = True
            continue
        if info.isreg() and logical != binary_name and info.mode & 0o111:
            raise ArchiveVerificationError(
                f"archive resource file is executable: {logical!r}"
            )
        member_size = 0 if info.isdir() else info.size
        if member_size < 0 or member_size > limits.max_member_size:
            raise ArchiveVerificationError(
                f"archive member {logical!r} size {member_size} exceeds budget "
                f"{limits.max_member_size}"
            )
        total_size += member_size
        if total_size > limits.max_total_size:
            raise ArchiveVerificationError(
                f"archive uncompressed size exceeds budget {limits.max_total_size}"
            )
        members.append(
            ArchiveMember(
                archive_name=info.name,
                logical_path=logical,
                is_directory=info.isdir(),
                size=member_size,
                mode=info.mode & 0o777,
                source=info,
            )
        )
    _validate_member_set(members, limits=limits)
    return members


def _zip_members(
    archive: zipfile.ZipFile,
    *,
    wrapper: str,
    binary_name: str,
    limits: ExtractionLimits,
) -> list[ArchiveMember]:
    infos = archive.infolist()
    if len(infos) > limits.max_members:
        raise ArchiveVerificationError(
            f"archive member count {len(infos)} exceeds budget {limits.max_members}"
        )
    members: list[ArchiveMember] = []
    for info in infos:
        is_directory = info.is_dir()
        if is_directory and (info.file_size != 0 or info.compress_size != 0):
            raise ArchiveVerificationError(
                f"ZIP directory member declares payload bytes: {info.filename!r}"
            )
        normalized = _normalize_member_name(
            info.filename,
            is_directory=is_directory,
            limits=limits,
        )
        if normalized == wrapper or normalized.startswith(f"{wrapper}/"):
            raise ArchiveVerificationError(
                f"ZIP archive must be flat rather than wrapped in {wrapper!r}"
            )
        if info.flag_bits & 0x1:
            raise ArchiveVerificationError(f"ZIP member is encrypted: {info.filename!r}")
        mode = _validate_zip_member_type(info, is_directory)
        if not is_directory and normalized != binary_name and mode & 0o111:
            raise ArchiveVerificationError(
                f"archive resource file is executable: {normalized!r}"
            )
        members.append(
            ArchiveMember(
                archive_name=info.filename,
                logical_path=normalized,
                is_directory=is_directory,
                size=0 if is_directory else info.file_size,
                mode=mode,
                source=info,
            )
        )
    _validate_member_set(members, limits=limits)
    return members


def _destination(root: Path, logical_path: str) -> Path:
    destination = root.joinpath(*PurePosixPath(logical_path).parts)
    try:
        destination.relative_to(root)
    except ValueError as error:
        raise ArchiveVerificationError(
            f"archive path escapes extraction root: {logical_path!r}"
        ) from error
    return destination


def _write_member(
    source: BinaryIO,
    destination: Path,
    *,
    expected_size: int,
) -> None:
    destination.parent.mkdir(parents=True, exist_ok=True)
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
    if hasattr(os, "O_BINARY"):
        flags |= os.O_BINARY
    descriptor = os.open(destination, flags, 0o600)
    observed_size = 0
    try:
        with os.fdopen(descriptor, "wb") as output:
            while True:
                chunk = source.read(min(1024 * 1024, expected_size - observed_size + 1))
                if not chunk:
                    break
                observed_size += len(chunk)
                if observed_size > expected_size:
                    raise ArchiveVerificationError(
                        f"archive member expanded beyond declared size for {destination.name!r}"
                    )
                output.write(chunk)
    except BaseException:
        destination.unlink(missing_ok=True)
        raise
    if observed_size != expected_size:
        destination.unlink(missing_ok=True)
        raise ArchiveVerificationError(
            f"archive member size mismatch for {destination.name!r}: "
            f"expected {expected_size}, got {observed_size}"
        )


def _tar_stream_budget(limits: ExtractionLimits) -> int:
    per_member_metadata = limits.max_path_bytes + 2048
    return (
        limits.max_total_size
        + limits.max_members * per_member_metadata
        + tarfile.RECORDSIZE
    )


def _decompress_tar_xz(
    archive_path: Path,
    destination: Path,
    *,
    limits: ExtractionLimits,
) -> None:
    max_stream_size = _tar_stream_budget(limits)
    observed_size = 0
    try:
        decompressor = lzma.LZMADecompressor(
            format=lzma.FORMAT_XZ,
            memlimit=XZ_MEMORY_MAX_BYTES,
        )
        with archive_path.open("rb") as source, destination.open("xb") as output:
            while compressed := source.read(1024 * 1024):
                pending = compressed
                while pending or not decompressor.needs_input:
                    remaining = max_stream_size - observed_size
                    chunk = decompressor.decompress(
                        pending,
                        max_length=min(1024 * 1024, remaining + 1),
                    )
                    pending = b""
                    observed_size += len(chunk)
                    if observed_size > max_stream_size:
                        raise ArchiveVerificationError(
                            f"decompressed tar stream exceeds budget {max_stream_size}"
                        )
                    output.write(chunk)
                    if decompressor.eof:
                        if decompressor.unused_data or source.read(1):
                            raise ArchiveVerificationError(
                                "tar.xz archive contains trailing or concatenated data"
                            )
                        return
                    if decompressor.needs_input:
                        break
            raise EOFError("tar.xz archive ended before the XZ stream footer")
    except BaseException:
        destination.unlink(missing_ok=True)
        raise


def _extract_tar(
    archive_path: Path,
    destination: Path,
    *,
    wrapper: str,
    target: str,
    limits: ExtractionLimits,
) -> list[ArchiveMember]:
    try:
        with tarfile.open(archive_path, mode="r:") as archive:
            members = _tar_members(
                archive,
                wrapper=wrapper,
                binary_name=_binary_name(target),
                limits=limits,
            )
            for member in members:
                output_path = _destination(destination, member.logical_path)
                if member.is_directory:
                    output_path.mkdir(parents=True, exist_ok=True)
                    continue
                source = archive.extractfile(member.source)
                if source is None:
                    raise ArchiveVerificationError(
                        f"cannot read tar member {member.archive_name!r}"
                    )
                with source:
                    _write_member(source, output_path, expected_size=member.size)
                if member.logical_path == _binary_name(target):
                    if member.mode & 0o111 == 0:
                        raise ArchiveVerificationError(
                            "Unix CLI binary is missing executable permission bits"
                        )
                    output_path.chmod(0o700)
            return members
    except (tarfile.TarError, lzma.LZMAError, EOFError, OSError) as error:
        if isinstance(error, ArchiveVerificationError):
            raise
        raise ArchiveVerificationError(f"cannot parse tar.xz archive: {error}") from error


def _extract_zip(
    archive_path: Path,
    destination: Path,
    *,
    wrapper: str,
    target: str,
    limits: ExtractionLimits,
) -> list[ArchiveMember]:
    try:
        with zipfile.ZipFile(archive_path, mode="r") as archive:
            members = _zip_members(
                archive,
                wrapper=wrapper,
                binary_name=_binary_name(target),
                limits=limits,
            )
            for member in members:
                output_path = _destination(destination, member.logical_path)
                if member.is_directory:
                    output_path.mkdir(parents=True, exist_ok=True)
                    continue
                with archive.open(member.source, mode="r") as source:
                    _write_member(source, output_path, expected_size=member.size)
            return members
    except (zipfile.BadZipFile, NotImplementedError, RuntimeError, OSError) as error:
        if isinstance(error, ArchiveVerificationError):
            raise
        raise ArchiveVerificationError(f"cannot parse ZIP archive: {error}") from error


def _binary_name(target: str) -> str:
    return f"{PACKAGE_NAME}.exe" if "windows" in target.split("-") else PACKAGE_NAME


def _require_repository_root(repo_root: Path) -> Path:
    if repo_root.is_symlink() or not repo_root.is_dir():
        raise ArchiveVerificationError(f"repository root is missing or unsafe: {repo_root}")
    return repo_root.resolve()


def _repository_tree_files(repo_root: Path, relative_root: str) -> dict[str, Path]:
    root = repo_root / relative_root
    if root.is_symlink() or not root.is_dir():
        raise ArchiveVerificationError(
            f"repository comparison directory is missing or unsafe: {root}"
        )
    files: dict[str, Path] = {}
    for path in root.rglob("*"):
        if path.is_symlink():
            raise ArchiveVerificationError(
                f"repository comparison path must not be a symlink: {path}"
            )
        if path.is_file():
            relative = path.relative_to(repo_root).as_posix()
            _normalize_member_name(relative, is_directory=False)
            files[relative] = path
        elif not path.is_dir():
            raise ArchiveVerificationError(
                f"repository comparison path is not a regular file: {path}"
            )
    return files


def _repository_asset_files(repo_root: Path) -> dict[str, Path]:
    package_root = repo_root / "crates/merman-cli"
    result: dict[str, Path] = {}
    for source_root in ASSET_SOURCE_ROOTS:
        files = _repository_tree_files(package_root, source_root)
        if not files:
            raise ArchiveVerificationError(
                f"repository asset directory is empty: {package_root / source_root}"
            )
        for source_relative, source in files.items():
            archive_relative = source_relative.removeprefix("assets/")
            result[archive_relative] = source
    return result


def _run_git(repo_root: Path, *arguments: str) -> bytes:
    command = [
        "git",
        "-c",
        "core.quotepath=false",
        "-C",
        str(repo_root),
        *arguments,
    ]
    try:
        completed = subprocess.run(
            command,
            capture_output=True,
            check=False,
        )
    except OSError as error:
        raise ArchiveVerificationError(
            f"cannot inspect repository with Git: {error}"
        ) from error
    if completed.returncode != 0:
        detail = completed.stderr.decode("utf-8", errors="replace").strip()
        raise ArchiveVerificationError(
            f"cannot inspect repository with Git: {detail or 'git command failed'}"
        )
    return completed.stdout


def _git_tracked_legal_files(repo_root: Path) -> dict[str, Path]:
    try:
        top_level = _run_git(repo_root, "rev-parse", "--show-toplevel").decode(
            "utf-8"
        ).strip()
        tracked = _run_git(
            repo_root,
            "ls-files",
            "-z",
            "--",
            NOTICE_PATH,
            LICENSE_ROOT,
        ).decode("utf-8").split("\0")
    except UnicodeDecodeError as error:
        raise ArchiveVerificationError(
            f"repository path reported by Git is not valid UTF-8: {error}"
        ) from error
    if Path(top_level).resolve() != repo_root:
        raise ArchiveVerificationError(
            f"repository root must be the Git top-level directory: {repo_root}"
        )

    result: dict[str, Path] = {}
    for relative in filter(None, tracked):
        if relative != NOTICE_PATH and not relative.startswith(f"{LICENSE_ROOT}/"):
            raise ArchiveVerificationError(
                f"git returned an unexpected legal path: {relative!r}"
            )
        _normalize_member_name(relative, is_directory=False)
        path = repo_root / Path(relative)
        if path.is_symlink() or not path.is_file():
            raise ArchiveVerificationError(
                f"tracked legal file is missing or unsafe: {path}"
            )
        result[relative] = path
    if NOTICE_PATH not in result:
        raise ArchiveVerificationError(f"{NOTICE_PATH!r} must be tracked by Git")
    if not any(path.startswith(f"{LICENSE_ROOT}/") for path in result):
        raise ArchiveVerificationError(
            f"at least one file under {LICENSE_ROOT!r} must be tracked by Git"
        )
    return result


def _is_archive_asset_path(path: str) -> bool:
    return path.startswith(
        (
            f"{ASSET_ARCHIVE_ROOTS[0]}/",
            f"{ASSET_ARCHIVE_ROOTS[1]}/",
        )
    )


def _uses_assets_prefixed_layout(path: str) -> bool:
    return path == "assets" or any(
        path == f"assets/{root}" or path.startswith(f"assets/{root}/")
        for root in ASSET_ARCHIVE_ROOTS
    )


def _set_mismatch(label: str, expected: set[str], observed: set[str]) -> str:
    details = []
    missing = sorted(expected - observed)
    extra = sorted(observed - expected)
    if missing:
        details.append("missing " + ", ".join(missing))
    if extra:
        details.append("unexpected " + ", ".join(extra))
    return f"archive {label} set differs from repository: " + "; ".join(details)


def _require_distribution_contents(
    root: Path,
    members: Iterable[ArchiveMember],
    *,
    target: str,
    asset_files: dict[str, Path],
    legal_files: dict[str, Path],
) -> None:
    old_layout = sorted(
        member.logical_path
        for member in members
        if _uses_assets_prefixed_layout(member.logical_path)
    )
    if old_layout:
        raise ArchiveVerificationError(
            "archive uses the unsupported assets-prefixed CLI layout: "
            + ", ".join(old_layout)
        )

    regular = {
        member.logical_path: member
        for member in members
        if not member.is_directory
    }
    binary_name = _binary_name(target)
    binary_candidates = [
        path for path in regular if PurePosixPath(path).name == binary_name
    ]
    if binary_candidates != [binary_name]:
        raise ArchiveVerificationError(
            f"archive must contain exactly one root {binary_name!r} binary; "
            f"found {sorted(binary_candidates)!r}"
        )

    archived_assets = {path for path in regular if _is_archive_asset_path(path)}
    expected_assets = set(asset_files)
    if archived_assets != expected_assets:
        raise ArchiveVerificationError(
            _set_mismatch("CLI asset", expected_assets, archived_assets)
        )

    archived_legal = {
        path
        for path in regular
        if path == NOTICE_PATH or path.startswith(f"{LICENSE_ROOT}/")
    }
    expected_legal = set(legal_files)
    if archived_legal != expected_legal:
        raise ArchiveVerificationError(
            _set_mismatch("tracked legal file", expected_legal, archived_legal)
        )

    required_nonempty = (*sorted(expected_assets), *sorted(expected_legal), binary_name)
    for path in required_nonempty:
        member = regular.get(path)
        if member is None:
            raise ArchiveVerificationError(f"archive is missing required file {path!r}")
        if member.size == 0 or _destination(root, path).stat().st_size == 0:
            raise ArchiveVerificationError(f"required archive file is empty: {path!r}")

    for archive_relative, source in {**asset_files, **legal_files}.items():
        archived = _destination(root, archive_relative)
        if archived.read_bytes() != source.read_bytes():
            raise ArchiveVerificationError(
                f"archive content differs from repository file {archive_relative!r}"
            )


def target_matches_host(target: str) -> bool:
    system = platform.system().lower()
    machine = platform.machine().lower()
    architecture = {
        "amd64": "x86_64",
        "x86_64": "x86_64",
        "arm64": "aarch64",
        "aarch64": "aarch64",
    }.get(machine)
    if architecture is None or not target.startswith(f"{architecture}-"):
        return False
    if system == "darwin":
        return target.endswith("-apple-darwin")
    if system == "linux":
        return "-linux-" in target
    if system == "windows":
        return "-windows-" in target
    return False


def _kill_process_tree(process: subprocess.Popen[bytes]) -> None:
    if os.name == "posix":
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except (PermissionError, ProcessLookupError):
            if process.poll() is None:
                process.kill()
    elif process.poll() is None:
        process.kill()


def _read_bounded_stream(
    stream: BinaryIO,
    output: bytearray,
    exceeded: threading.Event,
) -> None:
    try:
        while True:
            chunk = stream.read(64 * 1024)
            if not chunk:
                return
            remaining = RUNTIME_OUTPUT_MAX_BYTES - len(output)
            if len(chunk) > remaining:
                output.extend(chunk[:remaining])
                exceeded.set()
                return
            output.extend(chunk)
    finally:
        stream.close()


def _run_subprocess_bounded(
    command: list[str],
    *,
    stdin: bytes,
    cwd: Path,
    environment: dict[str, str],
) -> subprocess.CompletedProcess[bytes]:
    process = subprocess.Popen(
        command,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        cwd=cwd,
        env=environment,
        start_new_session=os.name == "posix",
    )
    if process.stdin is None or process.stdout is None or process.stderr is None:
        _kill_process_tree(process)
        process.wait()
        raise ArchiveVerificationError("runtime process pipes were not created")

    stdout = bytearray()
    stderr = bytearray()
    exceeded = threading.Event()
    readers = [
        threading.Thread(
            target=_read_bounded_stream,
            args=(process.stdout, stdout, exceeded),
            daemon=True,
        ),
        threading.Thread(
            target=_read_bounded_stream,
            args=(process.stderr, stderr, exceeded),
            daemon=True,
        ),
    ]
    for reader in readers:
        reader.start()

    try:
        try:
            process.stdin.write(stdin)
            process.stdin.flush()
        except BrokenPipeError:
            pass
        finally:
            try:
                process.stdin.close()
            except BrokenPipeError:
                pass

        deadline = time.monotonic() + RUNTIME_TIMEOUT_SECONDS
        timed_out = False
        while True:
            if exceeded.is_set():
                _kill_process_tree(process)
                break
            if process.poll() is not None and all(not reader.is_alive() for reader in readers):
                break
            if time.monotonic() >= deadline:
                timed_out = True
                _kill_process_tree(process)
                break
            time.sleep(0.01)

        process.wait()
        for reader in readers:
            reader.join()
        if timed_out:
            raise subprocess.TimeoutExpired(
                command,
                RUNTIME_TIMEOUT_SECONDS,
                output=bytes(stdout),
                stderr=bytes(stderr),
            )
        if exceeded.is_set():
            raise ArchiveVerificationError("runtime command output exceeds verification budget")
        return subprocess.CompletedProcess(
            command,
            process.returncode,
            stdout=bytes(stdout),
            stderr=bytes(stderr),
        )
    except BaseException:
        _kill_process_tree(process)
        process.wait()
        for reader in readers:
            reader.join()
        raise


def _run_checked(
    command: list[str],
    *,
    stdin: bytes,
    cwd: Path,
    runner: CommandRunner,
) -> subprocess.CompletedProcess[bytes]:
    environment = os.environ.copy()
    environment["NO_COLOR"] = "1"
    if runner is subprocess.run:
        completed = _run_subprocess_bounded(
            command,
            stdin=stdin,
            cwd=cwd,
            environment=environment,
        )
    else:
        completed = runner(
            command,
            input=stdin,
            cwd=cwd,
            env=environment,
            capture_output=True,
            check=False,
            timeout=RUNTIME_TIMEOUT_SECONDS,
        )
    if not isinstance(completed.stdout, bytes) or not isinstance(completed.stderr, bytes):
        raise ArchiveVerificationError("runtime command runner must return byte output")
    if (
        len(completed.stdout) > RUNTIME_OUTPUT_MAX_BYTES
        or len(completed.stderr) > RUNTIME_OUTPUT_MAX_BYTES
    ):
        raise ArchiveVerificationError("runtime command output exceeds verification budget")
    if completed.returncode != 0:
        detail = completed.stderr.decode("utf-8", errors="replace").strip() or "no diagnostic"
        raise ArchiveVerificationError(
            f"runtime command failed with exit {completed.returncode}: {detail}"
        )
    return completed


def _strict_json_object(
    data: bytes,
    *,
    label: str = "capabilities output",
) -> dict[str, object]:
    try:
        text = data.decode("utf-8")
        value = json.loads(
            text,
            object_pairs_hook=lambda pairs: _json_object_without_duplicates(
                pairs,
                label=label,
            ),
        )
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ArchiveVerificationError(
            f"{label} is not valid UTF-8 JSON: {error}"
        ) from error
    if not isinstance(value, dict):
        raise ArchiveVerificationError(f"{label} must be one JSON object")
    return value


def _json_object_without_duplicates(
    pairs: list[tuple[str, object]],
    *,
    label: str,
) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result:
            raise ArchiveVerificationError(
                f"{label} contains duplicate JSON key {key!r}"
            )
        result[key] = value
    return result


def _read_repository_json(repo_root: Path, relative: str) -> dict[str, object]:
    path = repo_root / relative
    _require_regular_input(path, f"repository contract {relative!r}")
    try:
        with path.open("rb") as stream:
            data = stream.read(REPOSITORY_CONTRACT_MAX_BYTES + 1)
    except OSError as error:
        raise ArchiveVerificationError(
            f"cannot read repository contract {path}: {error}"
        ) from error
    if len(data) > REPOSITORY_CONTRACT_MAX_BYTES:
        raise ArchiveVerificationError(
            f"repository contract exceeds {REPOSITORY_CONTRACT_MAX_BYTES} bytes: {path}"
        )
    return _strict_json_object(data, label=f"repository contract {relative!r}")


def _require_json_object(
    value: object,
    *,
    label: str,
    fields: set[str] | None = None,
) -> dict[str, object]:
    if type(value) is not dict:
        raise ArchiveVerificationError(f"{label} must be a JSON object")
    result = value
    if fields is not None:
        observed = set(result)
        if observed != fields:
            missing = sorted(fields - observed)
            extra = sorted(observed - fields)
            details = []
            if missing:
                details.append("missing fields " + ", ".join(missing))
            if extra:
                details.append("extra fields " + ", ".join(extra))
            raise ArchiveVerificationError(f"{label} has " + "; ".join(details))
    return result


def _require_json_array(value: object, *, label: str) -> list[object]:
    if type(value) is not list:
        raise ArchiveVerificationError(f"{label} must be a JSON array")
    return value


def _require_json_string(value: object, *, label: str) -> str:
    if type(value) is not str or not value:
        raise ArchiveVerificationError(f"{label} must be a non-empty JSON string")
    return value


def _require_json_integer(value: object, *, label: str) -> int:
    if type(value) is not int:
        raise ArchiveVerificationError(f"{label} must be a JSON integer")
    return value


def _require_json_boolean(value: object, *, label: str) -> bool:
    if type(value) is not bool:
        raise ArchiveVerificationError(f"{label} must be a JSON boolean")
    return value


def _require_string_array(value: object, *, label: str) -> list[str]:
    values = _require_json_array(value, label=label)
    result = [
        _require_json_string(item, label=f"{label}[{index}]")
        for index, item in enumerate(values)
    ]
    if len(set(result)) != len(result):
        raise ArchiveVerificationError(f"{label} contains duplicate values")
    return result


def _require_unique_ids(values: list[dict[str, object]], *, label: str) -> None:
    ids = [
        _require_json_string(value.get("id"), label=f"{label}[{index}].id")
        for index, value in enumerate(values)
    ]
    if len(set(ids)) != len(ids):
        raise ArchiveVerificationError(f"{label} contains duplicate ids")


def _canonical_capability_surface(
    value: dict[str, object],
) -> dict[str, object]:
    surface = _require_json_object(
        value,
        label="capability surface",
        fields={
            "schema_version",
            "descriptor_id",
            "targets",
            "capabilities",
            "outputs",
            "binding_operations",
        },
    )
    schema_version = _require_json_integer(
        surface["schema_version"],
        label="capability surface schema_version",
    )
    descriptor_id = _require_json_string(
        surface["descriptor_id"],
        label="capability surface descriptor_id",
    )

    targets = []
    for index, item in enumerate(
        _require_json_array(surface["targets"], label="capability surface targets")
    ):
        target = _require_json_object(
            item,
            label=f"capability surface targets[{index}]",
            fields={"id", "description"},
        )
        targets.append(
            {
                "id": _require_json_string(
                    target["id"],
                    label=f"capability surface targets[{index}].id",
                ),
                "description": _require_json_string(
                    target["description"],
                    label=f"capability surface targets[{index}].description",
                ),
            }
        )
    _require_unique_ids(targets, label="capability surface targets")

    capabilities = []
    for index, item in enumerate(
        _require_json_array(
            surface["capabilities"],
            label="capability surface capabilities",
        )
    ):
        capability = _require_json_object(
            item,
            label=f"capability surface capabilities[{index}]",
            fields={
                "id",
                "kind",
                "description",
                "targets",
                "implications",
                "absence",
            },
        )
        kind = _require_json_string(
            capability["kind"],
            label=f"capability surface capabilities[{index}].kind",
        )
        if kind not in _CAPABILITY_KINDS:
            raise ArchiveVerificationError(
                f"capability surface capabilities[{index}].kind is unknown: {kind!r}"
            )
        absence = _require_json_object(
            capability["absence"],
            label=f"capability surface capabilities[{index}].absence",
            fields={"error_id", "contract"},
        )
        capabilities.append(
            {
                "id": _require_json_string(
                    capability["id"],
                    label=f"capability surface capabilities[{index}].id",
                ),
                "kind": kind,
                "description": _require_json_string(
                    capability["description"],
                    label=f"capability surface capabilities[{index}].description",
                ),
                "targets": sorted(
                    _require_string_array(
                        capability["targets"],
                        label=f"capability surface capabilities[{index}].targets",
                    )
                ),
                "implications": sorted(
                    _require_string_array(
                        capability["implications"],
                        label=f"capability surface capabilities[{index}].implications",
                    )
                ),
                "absence": {
                    "error_id": _require_json_string(
                        absence["error_id"],
                        label=(
                            f"capability surface capabilities[{index}]"
                            ".absence.error_id"
                        ),
                    ),
                    "contract": _require_json_string(
                        absence["contract"],
                        label=(
                            f"capability surface capabilities[{index}]"
                            ".absence.contract"
                        ),
                    ),
                },
            }
        )
    _require_unique_ids(capabilities, label="capability surface capabilities")

    outputs = []
    for index, item in enumerate(
        _require_json_array(surface["outputs"], label="capability surface outputs")
    ):
        output = _require_json_object(
            item,
            label=f"capability surface outputs[{index}]",
            fields={"id", "capability", "description", "media_type", "targets"},
        )
        outputs.append(
            {
                "id": _require_json_string(
                    output["id"],
                    label=f"capability surface outputs[{index}].id",
                ),
                "capability": _require_json_string(
                    output["capability"],
                    label=f"capability surface outputs[{index}].capability",
                ),
                "description": _require_json_string(
                    output["description"],
                    label=f"capability surface outputs[{index}].description",
                ),
                "media_type": _require_json_string(
                    output["media_type"],
                    label=f"capability surface outputs[{index}].media_type",
                ),
                "targets": sorted(
                    _require_string_array(
                        output["targets"],
                        label=f"capability surface outputs[{index}].targets",
                    )
                ),
            }
        )
    _require_unique_ids(outputs, label="capability surface outputs")

    operations = []
    for index, item in enumerate(
        _require_json_array(
            surface["binding_operations"],
            label="capability surface binding_operations",
        )
    ):
        operation = _require_json_object(
            item,
            label=f"capability surface binding_operations[{index}]",
            fields={
                "id",
                "capability",
                "description",
                "media_type",
                "requires_uri",
                "targets",
            },
        )
        capability_id = operation["capability"]
        if capability_id is not None:
            capability_id = _require_json_string(
                capability_id,
                label=(
                    f"capability surface binding_operations[{index}].capability"
                ),
            )
        operations.append(
            {
                "id": _require_json_string(
                    operation["id"],
                    label=f"capability surface binding_operations[{index}].id",
                ),
                "capability": capability_id,
                "description": _require_json_string(
                    operation["description"],
                    label=(
                        f"capability surface binding_operations[{index}].description"
                    ),
                ),
                "media_type": _require_json_string(
                    operation["media_type"],
                    label=(
                        f"capability surface binding_operations[{index}].media_type"
                    ),
                ),
                "requires_uri": _require_json_boolean(
                    operation["requires_uri"],
                    label=(
                        f"capability surface binding_operations[{index}].requires_uri"
                    ),
                ),
                "targets": sorted(
                    _require_string_array(
                        operation["targets"],
                        label=(
                            f"capability surface binding_operations[{index}].targets"
                        ),
                    )
                ),
            }
        )
    _require_unique_ids(
        operations,
        label="capability surface binding_operations",
    )

    return {
        "schema_version": schema_version,
        "descriptor_id": descriptor_id,
        "targets": sorted(targets, key=lambda target: target["id"]),
        "capabilities": sorted(
            capabilities,
            key=lambda capability: capability["id"],
        ),
        "outputs": sorted(outputs, key=lambda output: output["id"]),
        "binding_operations": sorted(
            operations,
            key=lambda operation: operation["id"],
        ),
    }


def _capability_surface_digest(surface: dict[str, object]) -> str:
    encoded = json.dumps(
        surface,
        ensure_ascii=False,
        separators=(",", ":"),
    ).encode("utf-8")
    return f"sha256:{hashlib.sha256(encoded).hexdigest()}"


def _cli_release_runtime_ids(
    profiles: dict[str, object],
    *,
    surface: dict[str, object],
    digest: str,
) -> tuple[list[str], list[str]]:
    authority = _require_json_object(
        profiles.get("capability_authority"),
        label="artifact profiles capability_authority",
        fields={"path", "schema_version", "digest"},
    )
    _require_exact_json(
        "artifact profiles capability_authority.path",
        authority["path"],
        CAPABILITY_SURFACE_PATH,
    )
    _require_exact_json(
        "artifact profiles capability_authority.schema_version",
        authority["schema_version"],
        surface["schema_version"],
    )
    _require_exact_json(
        "artifact profiles capability_authority.digest",
        authority["digest"],
        digest,
    )

    candidates = []
    for index, value in enumerate(
        _require_json_array(profiles.get("profiles"), label="artifact profiles")
    ):
        profile = _require_json_object(
            value,
            label=f"artifact profiles[{index}]",
        )
        if profile.get("id") == "cli-release":
            candidates.append(profile)
    if len(candidates) != 1:
        raise ArchiveVerificationError(
            "artifact profiles must contain exactly one cli-release profile"
        )
    profile = candidates[0]
    _require_exact_json(
        "cli-release semantic_target",
        profile.get("semantic_target"),
        "native",
    )
    cargo = _require_json_object(
        profile.get("cargo"),
        label="cli-release cargo",
    )
    _require_exact_json(
        "cli-release cargo.package",
        cargo.get("package"),
        PACKAGE_NAME,
    )
    _require_exact_json(
        "cli-release cargo.default_features",
        cargo.get("default_features"),
        False,
    )
    cargo_features = _require_string_array(
        cargo.get("features"),
        label="cli-release cargo.features",
    )
    expected = _require_json_object(
        profile.get("expected"),
        label="cli-release expected",
        fields={"capabilities", "runtime_ids", "outputs"},
    )
    capability_ids = _require_string_array(
        expected["capabilities"],
        label="cli-release expected capabilities",
    )
    runtime_ids = _require_string_array(
        expected["runtime_ids"],
        label="cli-release expected runtime_ids",
    )
    output_ids = _require_string_array(
        expected["outputs"],
        label="cli-release expected outputs",
    )
    _require_exact_json(
        "cli-release expected capabilities",
        capability_ids,
        runtime_ids,
    )
    _require_exact_json(
        "cli-release cargo.features",
        cargo_features,
        runtime_ids,
    )
    for label, values in (
        ("cli-release expected runtime_ids", runtime_ids),
        ("cli-release expected outputs", output_ids),
    ):
        if values != sorted(values):
            raise ArchiveVerificationError(f"{label} must be sorted")
    declared_ids = {
        capability["id"]
        for capability in surface["capabilities"]
    }
    unknown = sorted(set(runtime_ids) - declared_ids)
    if unknown:
        raise ArchiveVerificationError(
            "cli-release references unknown capabilities: " + ", ".join(unknown)
        )
    return runtime_ids, output_ids


def _repository_compatibility(repo_root: Path) -> dict[str, str]:
    repos = _read_repository_json(repo_root, UPSTREAM_REPOS_PATH)
    bundle = _read_repository_json(repo_root, MERMAID_REFERENCE_BUNDLE_PATH)
    locked_repos = _require_json_object(
        repos.get("repos"),
        label="upstream repository lock repos",
    )
    locked_mermaid = _require_json_object(
        locked_repos.get("mermaid"),
        label="upstream repository lock mermaid",
    )
    locked_mermaid_cli = _require_json_object(
        locked_repos.get("mermaid-cli"),
        label="upstream repository lock mermaid-cli",
    )
    locked_ref = _require_json_string(
        locked_mermaid.get("ref"),
        label="upstream repository lock mermaid.ref",
    )
    if not locked_ref.startswith("mermaid@") or len(locked_ref) == len("mermaid@"):
        raise ArchiveVerificationError(
            "upstream repository lock mermaid.ref must use mermaid@VERSION"
        )
    mermaid_version = locked_ref.removeprefix("mermaid@")
    release = _require_json_object(
        bundle.get("release"),
        label="Mermaid reference bundle release",
    )
    _require_exact_json(
        "Mermaid reference bundle release.version",
        release.get("version"),
        mermaid_version,
    )
    release_source = _require_json_object(
        release.get("source"),
        label="Mermaid reference bundle release.source",
    )
    _require_exact_json(
        "Mermaid source commit",
        release_source.get("commit"),
        _require_json_string(
            locked_mermaid.get("commit"),
            label="upstream repository lock mermaid.commit",
        ),
    )

    reference_cli = _require_json_object(
        bundle.get("referenceCli"),
        label="Mermaid reference bundle referenceCli",
    )
    reference_package = _require_json_object(
        reference_cli.get("package"),
        label="Mermaid reference bundle referenceCli.package",
    )
    _require_exact_json(
        "Mermaid reference CLI package",
        reference_package.get("package"),
        "@mermaid-js/mermaid-cli",
    )
    reference_source = _require_json_object(
        reference_package.get("source"),
        label="Mermaid reference bundle referenceCli.package.source",
    )
    locked_url = _require_json_string(
        locked_mermaid_cli.get("url"),
        label="upstream repository lock mermaid-cli.url",
    ).removesuffix(".git")
    reference_url = _require_json_string(
        reference_source.get("repository"),
        label="Mermaid reference bundle referenceCli.package.source.repository",
    ).removesuffix(".git")
    _require_exact_json(
        "Mermaid reference CLI repository",
        reference_url,
        locked_url,
    )
    return {
        "mermaid": mermaid_version,
        "mmdc": _require_json_string(
            reference_package.get("version"),
            label="Mermaid reference bundle referenceCli.package.version",
        ),
    }


def _cli_release_commands(runtime_ids: list[str]) -> list[str]:
    enabled = set(runtime_ids)
    commands = {"capabilities", "detect", "help", "parse"}
    if enabled.intersection({"ascii", "svg"}):
        commands.add("render")
    for capability, gated_commands in (
        ("analysis", ("fix", "lint", "lint-rules")),
        ("markdown", ("batch",)),
        ("shell-completions", ("completion",)),
        ("svg", ("layout", "mmdc")),
    ):
        if capability in enabled:
            commands.update(gated_commands)
    return sorted(commands)


def _release_capabilities_contract(
    repo_root: Path,
    *,
    version: str,
) -> dict[str, object]:
    profiles = _read_repository_json(repo_root, ARTIFACT_PROFILES_PATH)
    surface = _canonical_capability_surface(
        _read_repository_json(repo_root, CAPABILITY_SURFACE_PATH)
    )
    digest = _capability_surface_digest(surface)
    runtime_ids, expected_output_ids = _cli_release_runtime_ids(
        profiles,
        surface=surface,
        digest=digest,
    )
    enabled = set(runtime_ids)
    capabilities = [
        {
            "id": capability["id"],
            "kind": capability["kind"],
            "description": capability["description"],
            "implications": capability["implications"],
        }
        for capability in surface["capabilities"]
        if capability["id"] in enabled
    ]
    outputs = [
        {
            "id": output["id"],
            "description": output["description"],
            "media_type": output["media_type"],
        }
        for output in surface["outputs"]
        if output["capability"] in enabled
    ]
    observed_output_ids = [output["id"] for output in outputs]
    _require_exact_json(
        "cli-release expected outputs",
        expected_output_ids,
        observed_output_ids,
    )
    return {
        "schema_version": CAPABILITIES_SCHEMA_VERSION,
        "cli_contract_version": CLI_CONTRACT_VERSION,
        "package": {"name": PACKAGE_NAME, "version": version},
        "compatibility": _repository_compatibility(repo_root),
        "descriptor": {
            "schema_version": surface["schema_version"],
            "digest": digest,
        },
        "commands": _cli_release_commands(runtime_ids),
        "capabilities": capabilities,
        "outputs": outputs,
    }


def _require_exact_json(label: str, observed: object, expected: object) -> None:
    if type(observed) is not type(expected):
        raise ArchiveVerificationError(
            f"{label} has the wrong JSON type: "
            f"expected {type(expected).__name__}, got {type(observed).__name__}"
        )
    if isinstance(expected, dict):
        observed_object = observed
        expected_fields = set(expected)
        observed_fields = set(observed_object)
        if observed_fields != expected_fields:
            missing = sorted(expected_fields - observed_fields)
            extra = sorted(observed_fields - expected_fields)
            details = []
            if missing:
                details.append("missing fields " + ", ".join(missing))
            if extra:
                details.append("extra fields " + ", ".join(extra))
            raise ArchiveVerificationError(f"{label} has " + "; ".join(details))
        for key, expected_value in expected.items():
            _require_exact_json(
                f"{label}.{key}",
                observed_object[key],
                expected_value,
            )
        return
    if isinstance(expected, list):
        observed_array = observed
        if len(observed_array) != len(expected):
            raise ArchiveVerificationError(
                f"{label} has {len(observed_array)} entries; expected {len(expected)}"
            )
        for index, expected_value in enumerate(expected):
            _require_exact_json(
                f"{label}[{index}]",
                observed_array[index],
                expected_value,
            )
        return
    if observed != expected:
        raise ArchiveVerificationError(
            f"{label} differs from the repository contract: "
            f"expected {expected!r}, got {observed!r}"
        )


def _validate_runtime_capabilities(
    observed: dict[str, object],
    expected: dict[str, object],
) -> None:
    _require_exact_json("capabilities document", observed, expected)


def verify_runtime_contract(
    binary: Path,
    *,
    target: str,
    version: str,
    repo_root: Path,
    runner: CommandRunner = subprocess.run,
    host_target_checker: HostTargetChecker = target_matches_host,
) -> None:
    """Execute an explicitly approved host binary and verify stable CLI behavior."""
    if not host_target_checker(target):
        raise ArchiveVerificationError(
            f"refusing to execute archive target {target!r} on this host"
        )
    repo_root = _require_repository_root(Path(repo_root))
    expected_capabilities = _release_capabilities_contract(
        repo_root,
        version=version,
    )
    command = str(binary)
    version_result = _run_checked(
        [command, "--version"],
        stdin=b"",
        cwd=binary.parent,
        runner=runner,
    )
    expected_version = f"{PACKAGE_NAME} {version}\n".encode()
    if version_result.stdout != expected_version or version_result.stderr:
        raise ArchiveVerificationError(
            "--version must emit exactly one stable line on stdout and no stderr"
        )

    capabilities_result = _run_checked(
        [command, "capabilities", "--json"],
        stdin=b"",
        cwd=binary.parent,
        runner=runner,
    )
    if capabilities_result.stderr:
        raise ArchiveVerificationError("capabilities --json emitted unexpected stderr")
    capabilities = _strict_json_object(capabilities_result.stdout)
    _validate_runtime_capabilities(capabilities, expected_capabilities)

    render_result = _run_checked(
        [command, "render", "--format", "svg", "-"],
        stdin=SVG_SMOKE_SOURCE,
        cwd=binary.parent,
        runner=runner,
    )
    if render_result.stderr:
        raise ArchiveVerificationError("minimal SVG render emitted unexpected stderr")
    try:
        root = ElementTree.fromstring(render_result.stdout)
    except ElementTree.ParseError as error:
        raise ArchiveVerificationError(f"minimal render is not valid XML: {error}") from error
    if root.tag.rsplit("}", maxsplit=1)[-1] != "svg":
        raise ArchiveVerificationError("minimal render root element is not SVG")


def verify_release_archive(
    archive: Path,
    checksum: Path,
    *,
    target: str,
    version: str,
    repo_root: Path,
    verified_output: Path,
    execute: bool = False,
    limits: ExtractionLimits = DEFAULT_LIMITS,
    runner: CommandRunner = subprocess.run,
    host_target_checker: HostTargetChecker = target_matches_host,
) -> VerificationReport:
    """Verify one archive and persist the checksum-bound bytes for downstream use."""
    archive = Path(archive)
    checksum = Path(checksum)
    repo_root = _require_repository_root(Path(repo_root))
    asset_files = _repository_asset_files(repo_root)
    legal_files = _git_tracked_legal_files(repo_root)
    verified_output = Path(verified_output)
    if verified_output.name != archive.name:
        raise ArchiveVerificationError(
            f"verified output must retain archive name {archive.name!r}"
        )
    output_parent = verified_output.parent
    if output_parent.is_symlink() or not output_parent.is_dir():
        raise ArchiveVerificationError(
            f"verified output directory is missing or unsafe: {output_parent}"
        )
    verified_output = output_parent.resolve() / verified_output.name

    _validate_target_and_version(target, version)
    wrapper = _archive_stem(archive, target)
    expected_digest = verify_checksum(archive, checksum, limits=limits)

    with tempfile.TemporaryDirectory(prefix="merman-cli-release-verify-") as temp_dir:
        temporary_root = Path(temp_dir)
        snapshot = temporary_root / archive.name
        _copy_verified_snapshot(
            archive,
            snapshot,
            expected_digest=expected_digest,
            limits=limits,
        )
        extraction_root = temporary_root / "contents"
        extraction_root.mkdir()
        if archive.name.endswith(".tar.xz"):
            uncompressed_tar = temporary_root / f"{archive.name}.tar"
            try:
                _decompress_tar_xz(
                    snapshot,
                    uncompressed_tar,
                    limits=limits,
                )
            except (lzma.LZMAError, EOFError, OSError) as error:
                raise ArchiveVerificationError(
                    f"cannot decompress tar.xz archive: {error}"
                ) from error
            members = _extract_tar(
                uncompressed_tar,
                extraction_root,
                wrapper=wrapper,
                target=target,
                limits=limits,
            )
        else:
            members = _extract_zip(
                snapshot,
                extraction_root,
                wrapper=wrapper,
                target=target,
                limits=limits,
            )
        _require_distribution_contents(
            extraction_root,
            members,
            target=target,
            asset_files=asset_files,
            legal_files=legal_files,
        )
        binary_name = _binary_name(target)
        if execute:
            verify_runtime_contract(
                _destination(extraction_root, binary_name),
                target=target,
                version=version,
                repo_root=repo_root,
                runner=runner,
                host_target_checker=host_target_checker,
            )
        _copy_verified_snapshot(
            snapshot,
            verified_output,
            expected_digest=expected_digest,
            limits=limits,
        )
        try:
            if os.name == "posix":
                verified_output.chmod(0o400)
        except BaseException:
            verified_output.unlink(missing_ok=True)
            raise
        return VerificationReport(
            archive=verified_output,
            digest=expected_digest,
            target=target,
            member_count=len(members),
            total_uncompressed_bytes=sum(member.size for member in members),
            binary_path=binary_name,
        )


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=Path, help="cargo-dist .tar.xz or .zip archive")
    parser.add_argument(
        "--checksum",
        type=Path,
        help="adjacent .sha256 file (defaults to ARCHIVE.sha256)",
    )
    parser.add_argument("--target", required=True, help="Rust target triple carried by the archive")
    parser.add_argument("--version", required=True, help="expected merman-cli package version")
    parser.add_argument(
        "--repo-root",
        type=Path,
        required=True,
        help="repository root containing the exact CLI and tracked legal assets",
    )
    parser.add_argument(
        "--verified-output",
        type=Path,
        required=True,
        help="new persistent path for the checksum-bound verified archive",
    )
    parser.add_argument(
        "--execute",
        action="store_true",
        help="execute the binary after structural verification when TARGET matches the host",
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    checksum = args.checksum or args.archive.with_name(f"{args.archive.name}.sha256")
    verify_release_archive(
        args.archive,
        checksum,
        target=args.target,
        version=args.version,
        repo_root=args.repo_root,
        verified_output=args.verified_output,
        execute=args.execute,
    )
    print(f"verified {args.verified_output}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (ArchiveVerificationError, OSError, subprocess.TimeoutExpired) as error:
        print(f"verify_cli_release_archive.py: {error}", file=sys.stderr)
        raise SystemExit(1) from error
