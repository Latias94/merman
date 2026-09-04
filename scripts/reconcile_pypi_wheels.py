#!/usr/bin/env python3
"""Verify existing PyPI wheels before an idempotent publication retry."""

from __future__ import annotations

import argparse
from email.parser import Parser
import hashlib
import json
from pathlib import Path
import re
import sys
import urllib.error
import urllib.parse
import urllib.request
from zipfile import ZipFile


class PyPIReconciliationError(RuntimeError):
    """The local wheel set cannot be reconciled with PyPI safely."""


SEMVER_RE = re.compile(
    r"^(?P<base>(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*))"
    r"(?:-(?P<channel>alpha|beta|rc)\.(?P<number>0|[1-9][0-9]*))?"
    r"(?:\+(?P<build>[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?$"
)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def normalize_project(value: str) -> str:
    return value.replace("-", "_").replace(".", "_").lower()


def normalize_version(value: str) -> str:
    """Accept the workspace SemVer spelling as well as an existing PEP 440 value."""
    match = SEMVER_RE.fullmatch(value)
    if match is None:
        return value
    version = match.group("base")
    channel = match.group("channel")
    if channel is not None:
        version += {"alpha": "a", "beta": "b", "rc": "rc"}[channel]
        version += match.group("number")
    build = match.group("build")
    if build is not None:
        version += "+" + build.replace("-", ".").lower()
    return version


def wheel_identity(path: Path) -> tuple[str, str]:
    with ZipFile(path) as archive:
        metadata_names = [
            name for name in archive.namelist() if name.endswith(".dist-info/METADATA")
        ]
    if len(metadata_names) != 1:
        raise PyPIReconciliationError(
            f"wheel must contain exactly one dist-info/METADATA file: {path.name}"
        )
    with ZipFile(path) as archive:
        metadata = Parser().parsestr(
            archive.read(metadata_names[0]).decode("utf-8", errors="strict")
        )
    name = metadata.get("Name")
    version = metadata.get("Version")
    if not name or not version:
        raise PyPIReconciliationError(f"wheel metadata is missing Name or Version: {path.name}")
    return name, version


def fetch_release(project: str, *, opener=urllib.request.urlopen) -> dict | None:
    encoded = urllib.parse.quote(project, safe="")
    request = urllib.request.Request(
        f"https://pypi.org/pypi/{encoded}/json",
        headers={"User-Agent": "merman-release-operator/1"},
    )
    try:
        with opener(request, timeout=30) as response:
            payload = json.load(response)
    except urllib.error.HTTPError as error:
        if error.code == 404:
            return None
        raise PyPIReconciliationError(
            f"PyPI returned HTTP {error.code} for project {project}"
        ) from error
    except (OSError, json.JSONDecodeError) as error:
        raise PyPIReconciliationError(f"cannot observe PyPI project {project}: {error}") from error
    if not isinstance(payload, dict):
        raise PyPIReconciliationError("PyPI response must be a JSON object")
    return payload


def reconcile(directory: Path, project: str, version: str, *, opener=urllib.request.urlopen) -> bool:
    directory = directory.resolve()
    version = normalize_version(version)
    wheels = sorted(path for path in directory.glob("*.whl") if path.is_file())
    if not wheels:
        raise PyPIReconciliationError(f"no wheel artifacts found in {directory}")

    for wheel in wheels:
        name, observed_version = wheel_identity(wheel)
        if normalize_project(name) != normalize_project(project) or observed_version != version:
            raise PyPIReconciliationError(
                f"wheel identity mismatch for {wheel.name}: "
                f"expected {project} {version}, found {name} {observed_version}"
            )

    payload = fetch_release(project, opener=opener)
    if payload is None:
        print(f"PyPI project {project} is not published; all local wheels are missing")
        return False
    releases = payload.get("releases")
    if not isinstance(releases, dict):
        raise PyPIReconciliationError("PyPI response has no releases map")
    existing = releases.get(version, [])
    if not isinstance(existing, list):
        raise PyPIReconciliationError(f"PyPI release metadata is invalid for {project} {version}")
    local_names = {wheel.name for wheel in wheels}
    by_filename: dict[str, set[str]] = {}
    for item in existing:
        if not isinstance(item, dict):
            continue
        filename = item.get("filename")
        if not isinstance(filename, str) or not filename.endswith(".whl"):
            continue
        digests = item.get("digests")
        sha256 = digests.get("sha256") if isinstance(digests, dict) else None
        if not isinstance(sha256, str) or re.fullmatch(r"[0-9a-fA-F]{64}", sha256) is None:
            raise PyPIReconciliationError(
                f"PyPI wheel has no valid SHA-256 metadata: {filename}"
            )
        by_filename.setdefault(filename, set()).add(sha256.lower())

    extra_wheels = sorted(set(by_filename) - local_names)
    if extra_wheels:
        raise PyPIReconciliationError(
            f"PyPI contains wheels outside the local release set: {extra_wheels}"
        )

    all_exact = True
    for wheel in wheels:
        digest = sha256_file(wheel)
        known = by_filename.get(wheel.name)
        if known is None:
            all_exact = False
            print(f"PyPI missing {wheel.name}; it will be uploaded")
        elif known == {digest}:
            print(f"PyPI contains the exact wheel {wheel.name}")
        else:
            raise PyPIReconciliationError(
                f"PyPI wheel checksum mismatch for {wheel.name}: "
                f"local {digest}, registry {sorted(known)}"
            )
    return all_exact


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--directory", type=Path, required=True)
    parser.add_argument("--project", required=True)
    parser.add_argument(
        "--version",
        required=True,
        help="workspace SemVer (for example 0.8.0-alpha.6) or PEP 440 spelling",
    )
    parser.add_argument(
        "--require-exact",
        action="store_true",
        help="fail when any local wheel is not yet visible with the exact SHA-256",
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        exact = reconcile(args.directory, args.project, args.version)
        if args.require_exact and not exact:
            print(
                "reconcile_pypi_wheels.py: PyPI has not exposed the exact local wheel set yet",
                file=sys.stderr,
            )
            return 3
    except (OSError, PyPIReconciliationError) as error:
        print(f"reconcile_pypi_wheels.py: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
