"""Read Cargo.lock identities for generated Rust license reports."""

from __future__ import annotations

import argparse
from pathlib import Path
import sys
import tomllib
from typing import Any

try:
    from scripts.strict_json import canonical_sha256
except ModuleNotFoundError:
    from strict_json import canonical_sha256


class CargoLockIdentityError(ValueError):
    """The lockfile does not describe Cargo package identities."""


def load_cargo_packages(path: Path) -> list[dict[str, Any]]:
    try:
        packages = tomllib.loads(path.read_text(encoding="utf-8")).get("package")
    except (OSError, UnicodeError, tomllib.TOMLDecodeError) as error:
        raise CargoLockIdentityError(f"cannot read Cargo lock {path}: {error}") from error
    if not isinstance(packages, list) or not all(isinstance(item, dict) for item in packages):
        raise CargoLockIdentityError(f"Cargo lock {path} has no package array")
    return packages


def third_party_cargo_lock_sha256(path: Path) -> str:
    """Hash ordered (name, version, source[, checksum]) rows, excluding path packages."""
    rows = []
    for package in load_cargo_packages(path):
        if "source" not in package:
            continue
        fields = ("name", "version", "source")
        if "checksum" in package:
            fields += ("checksum",)
        row = tuple(package.get(field) for field in fields)
        if not all(isinstance(value, str) and value for value in row):
            raise CargoLockIdentityError(f"Cargo lock {path} has an invalid third-party identity")
        rows.append(row)
    return canonical_sha256(sorted(rows))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("lockfile", type=Path)
    args = parser.parse_args()
    try:
        print(third_party_cargo_lock_sha256(args.lockfile))
    except CargoLockIdentityError as error:
        print(error, file=sys.stderr)
        raise SystemExit(1) from None
