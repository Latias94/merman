#!/usr/bin/env python3
"""Build and safely extract the Flutter package artifact used by release CI."""

from __future__ import annotations

import argparse
from dataclasses import dataclass
import gzip
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import stat
import sys
import tarfile
from typing import BinaryIO
import unicodedata


ARCHIVE_NAME = "merman-flutter-package.tar.gz"
RECEIPT_NAME = "merman-flutter-package.receipt.json"
PACKAGE_NAME = "merman"
SCHEMA_VERSION = 1
COPY_BYTES = 1024 * 1024
REQUIRED_FILES = {"LICENSE", "THIRD_PARTY_NOTICES.md", "pubspec.yaml"}
IGNORED_DIRECTORIES = {".dart_tool", ".git", "__pycache__", "build"}
IGNORED_FILES = IGNORED_DIRECTORIES | {".DS_Store", ".gitignore", ".pubignore"}
IGNORED_ROOT_FILES = {"build-native.py", "ffigen.yaml"}
SHA_RE = re.compile(r"[0-9a-f]{40}\Z")
VERSION_RE = re.compile(
    r"(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)"
    r"(?:-(?:alpha|beta|rc)\.(?:0|[1-9][0-9]*))?"
    r"(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?\Z"
)
WINDOWS_DEVICES = {
    "CON",
    "PRN",
    "AUX",
    "NUL",
    *(f"COM{i}" for i in range(1, 10)),
    *(f"LPT{i}" for i in range(1, 10)),
}


class FlutterReleaseArchiveError(RuntimeError):
    """The Flutter release archive violates its owner-local contract."""


@dataclass(frozen=True)
class ArchiveLimits:
    max_archive_bytes: int = 128 * 1024 * 1024
    max_member_bytes: int = 256 * 1024 * 1024
    max_total_bytes: int = 512 * 1024 * 1024
    max_metadata_bytes: int = 32 * 1024 * 1024
    max_members: int = 10_000
    max_path_bytes: int = 1024
    max_path_components: int = 64

    def __post_init__(self) -> None:
        if any(
            not isinstance(value, int) or isinstance(value, bool) or value <= 0
            for value in vars(self).values()
        ):
            raise ValueError("archive limits must be positive integers")


DEFAULT_LIMITS = ArchiveLimits()


def _receipt_identity(source_sha: str, source_tree: str, version: str) -> dict[str, object]:
    for value, pattern, label in (
        (source_sha, SHA_RE, "source SHA"),
        (source_tree, SHA_RE, "source tree"),
        (version, VERSION_RE, "version"),
    ):
        if pattern.fullmatch(value) is None:
            raise FlutterReleaseArchiveError(f"invalid {label}: {value!r}")
    return {
        "schema_version": SCHEMA_VERSION,
        "package": PACKAGE_NAME,
        "version": version,
        "source_sha": source_sha,
        "source_tree": source_tree,
    }


def _portable_key(path: str) -> str:
    # Case folding can change normalization, so normalize both before and after it.
    return "/".join(
        unicodedata.normalize("NFC", unicodedata.normalize("NFC", part).casefold())
        for part in path.split("/")
    )


def _validate_path(path: str, limits: ArchiveLimits) -> str:
    if not path or path.startswith("/") or "\\" in path or "\x00" in path:
        raise FlutterReleaseArchiveError(f"archive path is absolute or malformed: {path!r}")
    if len(path.encode()) > limits.max_path_bytes:
        raise FlutterReleaseArchiveError(f"archive path exceeds the byte budget: {path!r}")
    parts = path.split("/")
    if len(parts) > limits.max_path_components:
        raise FlutterReleaseArchiveError(f"archive path is too deep: {path!r}")
    for part in parts:
        if part in {"", ".", ".."}:
            raise FlutterReleaseArchiveError(f"archive path contains traversal: {path!r}")
        if part.endswith((" ", ".")) or any(character in '<>:"|?*' for character in part):
            raise FlutterReleaseArchiveError(f"archive path is not portable: {path!r}")
        if any(ord(character) < 32 or ord(character) == 127 for character in part):
            raise FlutterReleaseArchiveError(f"archive path contains a control byte: {path!r}")
        if part.split(".", 1)[0].upper() in WINDOWS_DEVICES:
            raise FlutterReleaseArchiveError(f"archive path contains a device name: {path!r}")
    return path


def _open_regular(path: Path, label: str) -> BinaryIO:
    if path.is_symlink():
        raise FlutterReleaseArchiveError(f"{label} must not be a symlink: {path}")
    flags = os.O_RDONLY | getattr(os, "O_BINARY", 0) | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise FlutterReleaseArchiveError(f"cannot open {label} {path}: {error}") from error
    if not stat.S_ISREG(os.fstat(descriptor).st_mode):
        os.close(descriptor)
        raise FlutterReleaseArchiveError(f"{label} must be a regular file: {path}")
    return os.fdopen(descriptor, "rb")


class _LimitedReader:
    def __init__(self, source: BinaryIO, limit: int) -> None:
        self.source = source
        self.limit = limit
        self.consumed = 0

    def read(self, size: int = -1) -> bytes:
        remaining = self.limit - self.consumed
        requested = remaining + 1 if size < 0 else min(size, remaining + 1)
        data = self.source.read(requested)
        self.consumed += len(data)
        if self.consumed > self.limit:
            raise FlutterReleaseArchiveError("decompressed tar exceeds its byte budget")
        return data


def _pubspec_scalar(data: bytes, key: str) -> str:
    try:
        lines = data.decode().splitlines()
    except UnicodeDecodeError as error:
        raise FlutterReleaseArchiveError("pubspec.yaml must be UTF-8") from error
    prefix = f"{key}:"
    values = [line[len(prefix) :].split("#", 1)[0].strip() for line in lines if line.startswith(prefix)]
    if len(values) != 1:
        raise FlutterReleaseArchiveError(f"pubspec.yaml must define one top-level {key}")
    value = values[0]
    if len(value) >= 2 and value[0] == value[-1] and value[0] in {'"', "'"}:
        value = value[1:-1]
    if not value or any(character.isspace() for character in value):
        raise FlutterReleaseArchiveError(f"unsupported pubspec.yaml {key} value")
    return value


def _verify_pubspec(data: bytes, version: str) -> None:
    if (name := _pubspec_scalar(data, "name")) != PACKAGE_NAME:
        raise FlutterReleaseArchiveError(f"Flutter package name is {name!r}, not {PACKAGE_NAME!r}")
    if (observed := _pubspec_scalar(data, "version")) != version:
        raise FlutterReleaseArchiveError(
            f"Flutter package version does not match: expected {version!r}, got {observed!r}"
        )


def _collect_files(root: Path, limits: ArchiveLimits) -> list[tuple[Path, str, os.stat_result]]:
    if root.is_symlink() or not root.is_dir():
        raise FlutterReleaseArchiveError(f"Flutter package root is missing or unsafe: {root}")
    root = root.resolve()
    files: list[tuple[Path, str, os.stat_result]] = []
    total = 0
    def walk_error(error: OSError) -> None:
        raise FlutterReleaseArchiveError(f"cannot walk Flutter package: {error}") from error

    for directory, dirnames, filenames in os.walk(
        root, topdown=True, onerror=walk_error, followlinks=False
    ):
        current = Path(directory)
        kept_directories = []
        for name in dirnames:
            if name in IGNORED_DIRECTORIES:
                continue
            path = current / name
            if path.is_symlink():
                raise FlutterReleaseArchiveError(
                    f"Flutter package contains a link: {path.relative_to(root)}"
                )
            kept_directories.append(name)
        dirnames[:] = sorted(kept_directories)

        for name in sorted(filenames):
            if (
                name in IGNORED_FILES
                or (current == root and name in IGNORED_ROOT_FILES)
                or Path(name).suffix in {".pyc", ".pyo"}
            ):
                continue
            path = current / name
            relative = path.relative_to(root)
            if path.is_symlink():
                raise FlutterReleaseArchiveError(f"Flutter package contains a link: {relative}")
            metadata = path.stat()
            if not stat.S_ISREG(metadata.st_mode):
                raise FlutterReleaseArchiveError(
                    f"Flutter package contains a special file: {relative}"
                )
            logical = _validate_path(relative.as_posix(), limits)
            if metadata.st_size > limits.max_member_bytes:
                raise FlutterReleaseArchiveError(
                    f"package member exceeds the size budget: {logical!r}"
                )
            total += metadata.st_size
            if total > limits.max_total_bytes:
                raise FlutterReleaseArchiveError("package exceeds the uncompressed size budget")
            files.append((path, logical, metadata))
            if len(files) > limits.max_members:
                raise FlutterReleaseArchiveError("package exceeds the member-count budget")
    files.sort(key=lambda item: item[1])
    return files


def _digest(stream: BinaryIO, max_bytes: int) -> str:
    digest = hashlib.sha256()
    size = 0
    for chunk in iter(lambda: stream.read(COPY_BYTES), b""):
        size += len(chunk)
        if size > max_bytes:
            raise FlutterReleaseArchiveError("archive exceeds the compressed size budget")
        digest.update(chunk)
    return digest.hexdigest()


def create_archive(
    package_root: Path,
    archive_path: Path,
    receipt_path: Path,
    *,
    source_sha: str,
    source_tree: str,
    version: str,
    limits: ArchiveLimits = DEFAULT_LIMITS,
) -> dict[str, object]:
    receipt = _receipt_identity(source_sha, source_tree, version)
    package_root, archive_path, receipt_path = map(Path, (package_root, archive_path, receipt_path))
    if archive_path.name != ARCHIVE_NAME or receipt_path.name != RECEIPT_NAME:
        raise FlutterReleaseArchiveError("Flutter release artifact names are not canonical")
    for output in (archive_path, receipt_path):
        if output.exists() or output.is_symlink():
            raise FlutterReleaseArchiveError(f"output already exists: {output}")
        if package_root.resolve() in output.resolve().parents:
            raise FlutterReleaseArchiveError("release artifact outputs must be outside the package root")
        output.parent.mkdir(parents=True, exist_ok=True)

    files = _collect_files(package_root, limits)
    try:
        with archive_path.open("xb") as raw:
            with gzip.GzipFile(
                filename="", mode="wb", fileobj=raw, compresslevel=6, mtime=0
            ) as compressed:
                with tarfile.open(mode="w", fileobj=compressed, format=tarfile.PAX_FORMAT) as archive:
                    for path, logical, metadata in files:
                        info = tarfile.TarInfo(logical)
                        info.size = metadata.st_size
                        info.mode = 0o755 if metadata.st_mode & 0o111 else 0o644
                        info.mtime = info.uid = info.gid = 0
                        info.uname = info.gname = ""
                        with path.open("rb") as source:
                            archive.addfile(info, source)
        with archive_path.open("rb") as stream:
            sha256 = _digest(stream, limits.max_archive_bytes)
            stream.seek(0)
            _, pubspec = _scan_archive(stream, limits)
        if pubspec is None:
            raise FlutterReleaseArchiveError("archive is missing pubspec.yaml")
        _verify_pubspec(pubspec, version)
        receipt["archive_sha256"] = sha256
        with receipt_path.open("x", encoding="utf-8") as output:
            output.write(json.dumps(receipt, sort_keys=True, indent=2) + "\n")
        return receipt
    except BaseException:
        archive_path.unlink(missing_ok=True)
        receipt_path.unlink(missing_ok=True)
        raise


def _unique_object(pairs: list[tuple[str, object]]) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result:
            raise FlutterReleaseArchiveError(f"receipt contains duplicate key {key!r}")
        result[key] = value
    return result


def _load_receipt(path: Path) -> dict[str, object]:
    with _open_regular(path, "receipt") as stream:
        data = stream.read(32 * 1024 + 1)
    if len(data) > 32 * 1024:
        raise FlutterReleaseArchiveError("receipt exceeds the size budget")
    try:
        receipt = json.loads(data, object_pairs_hook=_unique_object)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise FlutterReleaseArchiveError(f"cannot parse release receipt: {error}") from error
    if not isinstance(receipt, dict):
        raise FlutterReleaseArchiveError("release receipt must be an object")
    return receipt


def _receipt_digest(
    receipt: dict[str, object],
    expected: dict[str, object],
) -> str:
    digest = receipt.get("archive_sha256")
    if receipt != {**expected, "archive_sha256": digest}:
        raise FlutterReleaseArchiveError("receipt does not match the expected release identity")
    if not isinstance(digest, str) or not re.fullmatch(r"[0-9a-f]{64}", digest):
        raise FlutterReleaseArchiveError("receipt archive digest is invalid")
    return digest


def _scan_archive(
    compressed: BinaryIO,
    limits: ArchiveLimits,
    *,
    destination: Path | None = None,
    expected: list[tuple[str, int, int]] | None = None,
) -> tuple[list[tuple[str, int, int]], bytes | None]:
    signatures: list[tuple[str, int, int]] = []
    portable: dict[str, str] = {}
    total = 0
    pubspec: bytes | None = None
    decompressed_limit = limits.max_total_bytes + limits.max_metadata_bytes
    try:
        with gzip.GzipFile(fileobj=compressed, mode="rb") as uncompressed:
            bounded = _LimitedReader(uncompressed, decompressed_limit)
            with tarfile.open(mode="r|", fileobj=bounded) as archive:
                for index, member in enumerate(archive, start=1):
                    if index > limits.max_members:
                        raise FlutterReleaseArchiveError("archive exceeds the member-count budget")
                    if not member.isreg() or getattr(member, "sparse", None):
                        raise FlutterReleaseArchiveError(
                            f"archive member is not a regular file: {member.name!r}"
                        )
                    if member.mode & 0o7000:
                        raise FlutterReleaseArchiveError(
                            f"archive member has special permissions: {member.name!r}"
                        )
                    path = _validate_path(member.name, limits)
                    key = _portable_key(path)
                    if previous := portable.get(key):
                        if previous == path:
                            raise FlutterReleaseArchiveError(
                                f"archive contains duplicate path {path!r}"
                            )
                        raise FlutterReleaseArchiveError(
                            f"portable path collision: {previous!r} and {path!r}"
                        )
                    portable[key] = path
                    if member.size < 0 or member.size > limits.max_member_bytes:
                        raise FlutterReleaseArchiveError(
                            f"archive member exceeds the size budget: {path!r}"
                        )
                    total += member.size
                    if total > limits.max_total_bytes:
                        raise FlutterReleaseArchiveError(
                            "archive exceeds the uncompressed size budget"
                        )
                    signature = (path, member.size, 1 if member.mode & 0o111 else 0)
                    signatures.append(signature)
                    if expected is not None:
                        if index > len(expected) or signature != expected[index - 1]:
                            raise FlutterReleaseArchiveError(
                                "archive changed between validation and extraction"
                            )
                    if destination is not None:
                        _extract_member(archive, member, destination)
                    elif path == "pubspec.yaml":
                        pubspec = _member_bytes(archive, member)
    except (tarfile.TarError, OSError, EOFError) as error:
        raise FlutterReleaseArchiveError(f"cannot read Flutter release archive: {error}") from error
    if not signatures:
        raise FlutterReleaseArchiveError("archive is empty")
    if expected is not None and len(signatures) != len(expected):
        raise FlutterReleaseArchiveError("archive changed between validation and extraction")
    paths = set(portable.values())
    for path in paths:
        parts = PurePosixPath(path).parts
        if any(_portable_key("/".join(parts[:end])) in portable for end in range(1, len(parts))):
            raise FlutterReleaseArchiveError(f"archive file is an ancestor of {path!r}")
    missing = sorted(REQUIRED_FILES - paths)
    if missing:
        raise FlutterReleaseArchiveError("archive is missing: " + ", ".join(missing))
    return signatures, pubspec


def _member_bytes(archive: tarfile.TarFile, member: tarfile.TarInfo) -> bytes:
    source = archive.extractfile(member)
    if source is None:
        raise FlutterReleaseArchiveError(f"cannot read archive member {member.name!r}")
    with source:
        data = source.read(member.size + 1)
    if len(data) != member.size:
        raise FlutterReleaseArchiveError(f"archive member size mismatch: {member.name!r}")
    return data


def _extract_member(archive: tarfile.TarFile, member: tarfile.TarInfo, destination: Path) -> None:
    source = archive.extractfile(member)
    if source is None:
        raise FlutterReleaseArchiveError(f"cannot read archive member {member.name!r}")
    output = destination.joinpath(*PurePosixPath(member.name).parts)
    output.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    try:
        target = output.open("xb")
    except FileExistsError as error:
        raise FlutterReleaseArchiveError(f"refusing to overwrite {output}") from error
    remaining = member.size
    try:
        with source, target:
            while remaining:
                chunk = source.read(min(COPY_BYTES, remaining))
                if not chunk:
                    raise FlutterReleaseArchiveError(f"archive member ended early: {member.name!r}")
                target.write(chunk)
                remaining -= len(chunk)
        output.chmod(0o755 if member.mode & 0o111 else 0o644)
    except BaseException:
        output.unlink(missing_ok=True)
        raise


def verify_and_extract(
    archive_path: Path,
    receipt_path: Path,
    destination: Path,
    *,
    expected_source_sha: str,
    expected_source_tree: str,
    expected_version: str,
    limits: ArchiveLimits = DEFAULT_LIMITS,
) -> dict[str, object]:
    expected = _receipt_identity(expected_source_sha, expected_source_tree, expected_version)
    archive_path, receipt_path, destination = map(Path, (archive_path, receipt_path, destination))
    if archive_path.name != ARCHIVE_NAME or receipt_path.name != RECEIPT_NAME:
        raise FlutterReleaseArchiveError("Flutter release artifact names are not canonical")
    if destination.exists() or destination.is_symlink():
        raise FlutterReleaseArchiveError("extraction destination must not already exist")
    if destination.parent.is_symlink() or not destination.parent.is_dir():
        raise FlutterReleaseArchiveError("extraction parent is missing or unsafe")
    receipt = _load_receipt(receipt_path)
    expected_digest = _receipt_digest(receipt, expected)
    with _open_regular(archive_path, "archive") as stream:
        if _digest(stream, limits.max_archive_bytes) != expected_digest:
            raise FlutterReleaseArchiveError("archive digest does not match the receipt")
        stream.seek(0)
        signatures, pubspec = _scan_archive(stream, limits)
        if pubspec is None:
            raise FlutterReleaseArchiveError("archive is missing pubspec.yaml")
        _verify_pubspec(pubspec, expected_version)
        destination.mkdir(mode=0o700)
        stream.seek(0)
        _scan_archive(stream, limits, destination=destination, expected=signatures)
    return receipt


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    pack = commands.add_parser("pack")
    pack.add_argument("--package-root", type=Path, required=True)
    extract = commands.add_parser("extract")
    extract.add_argument("--destination", type=Path, required=True)
    for command in (pack, extract):
        command.add_argument("--archive", type=Path, required=True)
        command.add_argument("--receipt", type=Path, required=True)
        command.add_argument("--source-sha", required=True)
        command.add_argument("--source-tree", required=True)
        command.add_argument("--version", required=True)
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        if args.command == "pack":
            receipt = create_archive(
                args.package_root,
                args.archive,
                args.receipt,
                source_sha=args.source_sha,
                source_tree=args.source_tree,
                version=args.version,
            )
            print(f"created {ARCHIVE_NAME} for {receipt['source_sha']}")
        else:
            receipt = verify_and_extract(
                args.archive,
                args.receipt,
                args.destination,
                expected_source_sha=args.source_sha,
                expected_source_tree=args.source_tree,
                expected_version=args.version,
            )
            print(f"verified {ARCHIVE_NAME} for {receipt['source_sha']}")
    except FlutterReleaseArchiveError as error:
        print(f"Flutter release archive verification failed: {error}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
