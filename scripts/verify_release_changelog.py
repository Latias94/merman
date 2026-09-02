#!/usr/bin/env python3
"""Verify that release changelog projections name the requested version."""

from __future__ import annotations

import argparse
from datetime import date
import re
import sys
from pathlib import Path

try:
    from scripts.release_version import parse_release_version
except ModuleNotFoundError:
    from release_version import parse_release_version


CHANGELOG_PATHS = (
    Path("CHANGELOG.md"),
    Path("platforms/node/CHANGELOG.md"),
    Path("platforms/flutter/CHANGELOG.md"),
    Path("platforms/python/merman/CHANGELOG.md"),
    Path("platforms/android/CHANGELOG.md"),
    Path("platforms/apple/CHANGELOG.md"),
)
RELEASE_HEADING = re.compile(
    r"^##\s+\[(?P<version>[^]]+)\](?:\s+-\s+(?P<date>.+?))?\s*$"
)


class ReleaseChangelogError(ValueError):
    """A changelog projection cannot be associated with the requested release."""


def expected_heading_version(path: Path, version: str) -> str:
    parsed = parse_release_version(version, allow_v_prefix=False)
    if path == Path("platforms/python/merman/CHANGELOG.md"):
        return parsed.to_pep440()
    return parsed.canonical


def first_release_heading(text: str, path: Path) -> tuple[str, str]:
    for line_number, line in enumerate(text.splitlines(), start=1):
        match = RELEASE_HEADING.match(line)
        if match is not None:
            heading_date = match.group("date")
            if heading_date is None:
                raise ReleaseChangelogError(
                    f"{path}:{line_number} first release heading has no date/status"
                )
            return match.group("version"), heading_date
    raise ReleaseChangelogError(f"{path} has no release heading")


def verify_repository(
    root: Path,
    version: str,
    *,
    require_date: bool = False,
) -> None:
    for relative_path in CHANGELOG_PATHS:
        path = root / relative_path
        try:
            text = path.read_text(encoding="utf-8")
        except OSError as exc:
            raise ReleaseChangelogError(f"cannot read {relative_path}: {exc}") from exc
        actual_version, release_date = first_release_heading(text, relative_path)
        expected_version = expected_heading_version(relative_path, version)
        if actual_version != expected_version:
            raise ReleaseChangelogError(
                f"{relative_path} starts with {actual_version!r}, expected {expected_version!r}"
            )
        if release_date == "Unreleased":
            if require_date:
                raise ReleaseChangelogError(
                    f"{relative_path} must be dated before immutable release preflight"
                )
            continue
        try:
            if re.fullmatch(r"\d{4}-\d{2}-\d{2}", release_date) is None:
                raise ValueError("release date must use YYYY-MM-DD")
            date.fromisoformat(release_date)
        except ValueError as exc:
            raise ReleaseChangelogError(
                f"{relative_path} has invalid release date/status {release_date!r}"
            ) from exc


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Verify root and platform changelog projections for one release version."
    )
    parser.add_argument("--version", required=True)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument(
        "--require-date",
        action="store_true",
        help="reject Unreleased projections before immutable preflight",
    )
    args = parser.parse_args()
    try:
        canonical = parse_release_version(args.version, allow_v_prefix=False).canonical
        verify_repository(
            args.root.resolve(),
            canonical,
            require_date=args.require_date,
        )
    except (OSError, ValueError) as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 2
    print(f"release changelogs match {canonical}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
