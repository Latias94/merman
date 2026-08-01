#!/usr/bin/env python3
"""Shared safety primitives for checksum-bound cargo-dist archives."""

from __future__ import annotations

from contextlib import contextmanager
from dataclasses import dataclass
import hashlib
import lzma
import os
from pathlib import Path, PurePosixPath
import re
import stat
import subprocess
import tarfile
import tempfile
from typing import BinaryIO, Iterator
import unicodedata
import zipfile


__all__ = (
    "ArchiveMember",
    "ArchiveVerificationError",
    "DEFAULT_LIMITS",
    "ExtractionLimits",
    "VerificationReport",
    "archive_member_path",
    "binary_name_for",
    "format_set_mismatch",
    "git_tracked_legal_files",
    "persist_verified_archive",
    "read_checksum",
    "regular_files_equal",
    "release_archive_name_for",
    "repository_tree_files",
    "require_regular_input",
    "require_repository_root",
    "sha256_file",
    "verified_archive_contents",
)


CHECKSUM_MAX_BYTES = 4096
XZ_MEMORY_MAX_BYTES = 128 * 1024 * 1024
NOTICE_PATH = "THIRD_PARTY_NOTICES.md"
LICENSE_ROOT = "THIRD_PARTY_LICENSES"

_TARGET_RE = re.compile(r"[A-Za-z0-9_][A-Za-z0-9_.-]{0,127}\Z")
_VERSION_RE = re.compile(r"[0-9A-Za-z][0-9A-Za-z.+-]{0,127}\Z")
_PACKAGE_RE = re.compile(r"[a-z0-9][a-z0-9-]{0,63}\Z")
_SHA256_RE = re.compile(r"(?:sha256:)?([0-9A-Fa-f]{64})\Z")
_WINDOWS_DEVICE_NAMES = {
    "CON",
    "PRN",
    "AUX",
    "NUL",
    *(f"COM{index}" for index in range(1, 10)),
    *(f"LPT{index}" for index in range(1, 10)),
}


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


@dataclass(frozen=True)
class ExtractedReleaseArchive:
    """Checksum-bound archive contents valid only inside their context manager."""

    archive: Path
    snapshot: Path
    root: Path
    digest: str
    binary_path: str
    members: tuple[ArchiveMember, ...]


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


def release_archive_name_for(package_name: str, target: str) -> str:
    """Return the canonical cargo-dist archive name for one package and target."""
    if _PACKAGE_RE.fullmatch(package_name) is None:
        raise ArchiveVerificationError(f"invalid package name: {package_name!r}")
    if _TARGET_RE.fullmatch(target) is None:
        raise ArchiveVerificationError(f"invalid Rust target triple: {target!r}")
    return f"{package_name}-{target}{_archive_extension_for_target(target)}"


def _archive_stem(
    archive: Path,
    target: str,
    *,
    package_name: str,
) -> str:
    extension = _archive_extension_for_target(target)
    expected_name = release_archive_name_for(package_name, target)
    if archive.name != expected_name:
        raise ArchiveVerificationError(
            f"archive name must be {expected_name!r}, got {archive.name!r}"
        )
    return expected_name[: -len(extension)]


def require_regular_input(path: Path, label: str) -> None:
    if path.is_symlink():
        raise ArchiveVerificationError(f"{label} must not be a symlink: {path}")
    try:
        mode = path.stat().st_mode
    except OSError as error:
        raise ArchiveVerificationError(f"cannot inspect {label} {path}: {error}") from error
    if not stat.S_ISREG(mode):
        raise ArchiveVerificationError(f"{label} is not a regular file: {path}")


def read_checksum(checksum: Path, archive_name: str) -> str:
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
    if lines and not lines[-1]:
        lines.pop()
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


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def regular_files_equal(left: Path, right: Path) -> bool:
    """Compare two regular files exactly with bounded memory."""
    left = Path(left)
    right = Path(right)
    require_regular_input(left, "comparison file")
    require_regular_input(right, "comparison file")
    with left.open("rb") as left_stream, right.open("rb") as right_stream:
        while True:
            left_chunk = left_stream.read(1024 * 1024)
            right_chunk = right_stream.read(1024 * 1024)
            if left_chunk != right_chunk:
                return False
            if not left_chunk:
                return True


def _expected_checksum(
    archive: Path,
    checksum: Path,
    *,
    limits: ExtractionLimits = DEFAULT_LIMITS,
) -> str:
    """Validate checksum metadata before copying any archive bytes."""
    require_regular_input(archive, "archive")
    require_regular_input(checksum, "checksum")
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
    return read_checksum(checksum, archive.name)


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
            f"SHA-256 mismatch for {archive.name}: "
            f"expected {expected_digest}, got {observed_digest}"
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


def archive_member_path(root: Path, logical_path: str) -> Path:
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
    binary_name: str,
    limits: ExtractionLimits,
) -> list[ArchiveMember]:
    try:
        with tarfile.open(archive_path, mode="r:") as archive:
            members = _tar_members(
                archive,
                wrapper=wrapper,
                binary_name=binary_name,
                limits=limits,
            )
            for member in members:
                output_path = archive_member_path(destination, member.logical_path)
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
                if member.logical_path == binary_name:
                    if member.mode & 0o111 == 0:
                        raise ArchiveVerificationError(
                            "Unix release binary is missing executable permission bits"
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
    binary_name: str,
    limits: ExtractionLimits,
) -> list[ArchiveMember]:
    try:
        with zipfile.ZipFile(archive_path, mode="r") as archive:
            members = _zip_members(
                archive,
                wrapper=wrapper,
                binary_name=binary_name,
                limits=limits,
            )
            for member in members:
                output_path = archive_member_path(destination, member.logical_path)
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


def binary_name_for(package_name: str, target: str) -> str:
    if _PACKAGE_RE.fullmatch(package_name) is None:
        raise ArchiveVerificationError(f"invalid package name: {package_name!r}")
    return f"{package_name}.exe" if "windows" in target.split("-") else package_name


def format_set_mismatch(label: str, expected: set[str], observed: set[str]) -> str:
    details = []
    if missing := sorted(expected - observed):
        details.append("missing " + ", ".join(missing))
    if extra := sorted(observed - expected):
        details.append("unexpected " + ", ".join(extra))
    return f"archive {label} set differs from repository: " + "; ".join(details)


@contextmanager
def verified_archive_contents(
    archive: Path,
    checksum: Path,
    *,
    package_name: str,
    target: str,
    version: str,
    limits: ExtractionLimits = DEFAULT_LIMITS,
) -> Iterator[ExtractedReleaseArchive]:
    """Yield safely extracted, checksum-bound bytes for one cargo-dist archive."""
    archive = Path(archive)
    checksum = Path(checksum)
    _validate_target_and_version(target, version)
    wrapper = _archive_stem(archive, target, package_name=package_name)
    binary_name = binary_name_for(package_name, target)
    expected_digest = _expected_checksum(archive, checksum, limits=limits)

    with tempfile.TemporaryDirectory(prefix=f"{package_name}-release-verify-") as temp_dir:
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
                binary_name=binary_name,
                limits=limits,
            )
        else:
            members = _extract_zip(
                snapshot,
                extraction_root,
                wrapper=wrapper,
                binary_name=binary_name,
                limits=limits,
            )
        yield ExtractedReleaseArchive(
            archive=archive,
            snapshot=snapshot,
            root=extraction_root,
            digest=expected_digest,
            binary_path=binary_name,
            members=tuple(members),
        )


def persist_verified_archive(
    extracted: ExtractedReleaseArchive,
    verified_output: Path,
    *,
    limits: ExtractionLimits = DEFAULT_LIMITS,
) -> Path:
    """Persist the exact checksum-bound snapshot without repacking it."""
    verified_output = Path(verified_output)
    if verified_output.name != extracted.archive.name:
        raise ArchiveVerificationError(
            f"verified output must retain archive name {extracted.archive.name!r}"
        )
    output_parent = verified_output.parent
    if output_parent.is_symlink() or not output_parent.is_dir():
        raise ArchiveVerificationError(
            f"verified output directory is missing or unsafe: {output_parent}"
        )
    destination = output_parent.resolve() / verified_output.name
    _copy_verified_snapshot(
        extracted.snapshot,
        destination,
        expected_digest=extracted.digest,
        limits=limits,
    )
    try:
        if os.name == "posix":
            destination.chmod(0o400)
    except BaseException:
        destination.unlink(missing_ok=True)
        raise
    return destination


def require_repository_root(repo_root: Path) -> Path:
    if repo_root.is_symlink() or not repo_root.is_dir():
        raise ArchiveVerificationError(f"repository root is missing or unsafe: {repo_root}")
    return repo_root.resolve()


def repository_tree_files(repo_root: Path, relative_root: str) -> dict[str, Path]:
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


def git_tracked_legal_files(repo_root: Path) -> dict[str, Path]:
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
