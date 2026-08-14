#!/usr/bin/env python3
"""Verify that every publishable Cargo package contains its legal materials."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]


class PackageLegalMaterialError(RuntimeError):
    """A governed Cargo package omitted required legal material."""


def requires_third_party_bundle(package: dict[str, Any]) -> bool:
    metadata = package.get("metadata") or {}
    legal = metadata.get("merman-legal")
    if legal is None:
        return False
    if legal != {"third-party-bundle": True}:
        raise PackageLegalMaterialError(
            f"{package['name']} has invalid merman-legal package metadata"
        )
    return True


def required_legal_paths(package: dict[str, Any]) -> set[str]:
    manifest = Path(package["manifest_path"])
    crate_root = manifest.parent
    required: set[str] = set()
    license_expression = package.get("license")
    if license_expression == "MIT OR Apache-2.0":
        required.update({"LICENSE-MIT", "LICENSE-APACHE"})
    elif license_expression == "EPL-2.0":
        required.add("LICENSES/EPL-2.0.txt")
    elif license_expression == "MIT":
        required.add("LICENSE")
    else:
        raise PackageLegalMaterialError(
            f"{package['name']} has an unsupported or missing license expression: "
            f"{license_expression!r}"
        )

    notice = crate_root / "THIRD_PARTY_NOTICES.md"
    license_root = crate_root / "THIRD_PARTY_LICENSES"
    if requires_third_party_bundle(package) or notice.exists() or license_root.exists():
        if not notice.is_file() or not license_root.is_dir():
            raise PackageLegalMaterialError(
                f"{package['name']} has an incomplete third-party legal bundle"
            )
        required.add("THIRD_PARTY_NOTICES.md")
        third_party_files = sorted(path for path in license_root.rglob("*") if path.is_file())
        if not third_party_files:
            raise PackageLegalMaterialError(
                f"{package['name']} has an empty THIRD_PARTY_LICENSES directory"
            )
        required.update(path.relative_to(crate_root).as_posix() for path in third_party_files)
    return required


def verify_package_listing(package: dict[str, Any], listing: set[str]) -> None:
    missing = required_legal_paths(package) - listing
    if missing:
        raise PackageLegalMaterialError(
            f"{package['name']} package omits legal files: {', '.join(sorted(missing))}"
        )


def cargo_json(*args: str) -> Any:
    result = subprocess.run(
        ["cargo", *args],
        cwd=ROOT,
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    return json.loads(result.stdout)


def governed_packages(metadata: dict[str, Any]) -> list[dict[str, Any]]:
    independent = set(
        metadata.get("metadata", {})
        .get("merman-release", {})
        .get("independent-packages", [])
    )
    return sorted(
        (
            package
            for package in metadata["packages"]
            if package.get("publish") != [] or package.get("name") in independent
        ),
        key=lambda package: package["name"],
    )


def package_listing(name: str) -> set[str]:
    result = subprocess.run(
        ["cargo", "package", "--list", "--allow-dirty", "-p", name],
        cwd=ROOT,
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    return {line.strip() for line in result.stdout.splitlines() if line.strip()}


def verify_repository() -> int:
    metadata = cargo_json("metadata", "--locked", "--no-deps", "--format-version", "1")
    packages = governed_packages(metadata)
    for package in packages:
        verify_package_listing(package, package_listing(package["name"]))
    return len(packages)


def main() -> int:
    try:
        count = verify_repository()
    except (PackageLegalMaterialError, KeyError, json.JSONDecodeError, subprocess.CalledProcessError) as error:
        print(f"Cargo package legal-material verification failed: {error}", file=sys.stderr)
        return 1
    print(f"verified legal materials in {count} governed Cargo packages")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
