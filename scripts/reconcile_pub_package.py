#!/usr/bin/env python3
"""Reconcile a Flutter package archive with an existing pub.dev release.

pub.dev publishes a tarball produced by Dart, while this repository transports
the package through a deterministic safety archive.  Compare validated member
paths, executable bits, sizes, and bytes instead of comparing tar metadata.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import sys
import tempfile
import tarfile
import urllib.error
import urllib.parse
import urllib.request

from scripts import flutter_release_archive as archive_contract


class PubReconciliationError(RuntimeError):
    """The local Flutter package cannot be reconciled with pub.dev."""


def _canonical_members(path: Path) -> dict[str, tuple[int, int, str]]:
    limits = archive_contract.DEFAULT_LIMITS
    try:
        if path.stat().st_size > limits.max_archive_bytes:
            raise PubReconciliationError(f"archive exceeds the compressed size budget: {path}")

        members: dict[str, tuple[int, int, str]] = {}
        portable: dict[str, str] = {}
        total = 0
        with tarfile.open(path, mode="r:gz") as package:
            for index, member in enumerate(package, start=1):
                if index > limits.max_members:
                    raise PubReconciliationError("archive exceeds the member-count budget")
                name = member.name.rstrip("/")
                if not name:
                    raise PubReconciliationError("archive contains an empty member path")
                name = archive_contract._validate_path(name, limits)
                key = archive_contract._portable_key(name)
                previous = portable.get(key)
                if previous is not None:
                    raise PubReconciliationError(
                        f"archive contains duplicate or colliding path {previous!r}/{name!r}"
                    )
                portable[key] = name

                if member.isdir():
                    continue
                if not member.isreg() or getattr(member, "sparse", None):
                    raise PubReconciliationError(
                        f"archive member is not a regular file: {member.name!r}"
                    )
                if member.mode & 0o7000:
                    raise PubReconciliationError(
                        f"archive member has special permissions: {member.name!r}"
                    )
                if member.size < 0 or member.size > limits.max_member_bytes:
                    raise PubReconciliationError(
                        f"archive member exceeds the size budget: {name!r}"
                    )
                total += member.size
                if total > limits.max_total_bytes:
                    raise PubReconciliationError("archive exceeds the uncompressed size budget")
                source = package.extractfile(member)
                if source is None:
                    raise PubReconciliationError(f"cannot read archive member {name!r}")
                digest = hashlib.sha256()
                remaining = member.size
                with source:
                    while remaining:
                        chunk = source.read(min(1024 * 1024, remaining))
                        if not chunk:
                            raise PubReconciliationError(
                                f"archive member ended early: {name!r}"
                            )
                        digest.update(chunk)
                        remaining -= len(chunk)
                members[name] = (
                    1 if member.mode & 0o111 else 0,
                    member.size,
                    digest.hexdigest(),
                )
        if not members:
            raise PubReconciliationError("archive contains no regular files")
        return members
    except PubReconciliationError:
        raise
    except (OSError, tarfile.TarError, EOFError) as error:
        raise PubReconciliationError(f"cannot inspect Flutter archive {path}: {error}") from error


def reconcile(
    archive_path: Path,
    package: str,
    version: str,
    *,
    opener=urllib.request.urlopen,
) -> str:
    """Return ``exact`` or ``missing``; reject an existing byte mismatch."""
    archive_path = archive_path.resolve()
    if not archive_path.is_file():
        raise PubReconciliationError(f"Flutter candidate archive is missing: {archive_path}")
    local_members = _canonical_members(archive_path)
    encoded = urllib.parse.quote(package, safe="")
    request = urllib.request.Request(
        f"https://pub.dev/api/packages/{encoded}",
        headers={"User-Agent": "merman-release-operator/1"},
    )
    try:
        with opener(request, timeout=30) as response:
            payload = json.load(response)
    except urllib.error.HTTPError as error:
        if error.code == 404:
            return "missing"
        raise PubReconciliationError(
            f"pub.dev returned HTTP {error.code} for package {package}"
        ) from error
    except (OSError, json.JSONDecodeError) as error:
        raise PubReconciliationError(f"cannot observe pub.dev package {package}: {error}") from error
    if not isinstance(payload, dict) or not isinstance(payload.get("versions"), list):
        raise PubReconciliationError("pub.dev response has no versions list")
    release = next(
        (item for item in payload["versions"] if isinstance(item, dict) and item.get("version") == version),
        None,
    )
    if release is None:
        return "missing"
    archive_url = release.get("archive_url")
    if not isinstance(archive_url, str) or not archive_url:
        raise PubReconciliationError("pub.dev release has no archive_url")
    with tempfile.TemporaryDirectory(prefix="merman-pub-registry-") as temp_dir:
        registry_archive = Path(temp_dir) / "registry.tar.gz"
        _download_with_opener(archive_url, registry_archive, opener)
        registry_members = _canonical_members(registry_archive)
    if local_members != registry_members:
        raise PubReconciliationError(
            f"pub.dev archive contents differ for {package} {version}"
        )
    return "exact"


def _download_with_opener(
    url: str,
    destination: Path,
    opener,
    *,
    max_bytes: int = archive_contract.DEFAULT_LIMITS.max_archive_bytes,
) -> None:
    request = urllib.request.Request(
        url,
        headers={"User-Agent": "merman-release-operator/1"},
    )
    try:
        with opener(request, timeout=60) as response, destination.open("wb") as output:
            total = 0
            for chunk in iter(lambda: response.read(1024 * 1024), b""):
                total += len(chunk)
                if total > max_bytes:
                    raise PubReconciliationError(
                        "pub.dev archive exceeds the compressed size budget"
                    )
                output.write(chunk)
    except PubReconciliationError:
        raise
    except (OSError, urllib.error.HTTPError) as error:
        raise PubReconciliationError(f"cannot download pub.dev archive: {error}") from error


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archive", type=Path, required=True)
    parser.add_argument("--package", required=True)
    parser.add_argument("--version", required=True)
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        state = reconcile(args.archive, args.package, args.version)
    except (OSError, PubReconciliationError) as error:
        print(f"reconcile_pub_package.py: {error}", file=sys.stderr)
        return 1
    print(state)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
