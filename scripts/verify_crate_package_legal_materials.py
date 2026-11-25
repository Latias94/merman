#!/usr/bin/env python3
"""Verify that every publishable Cargo package contains its legal materials."""

from __future__ import annotations

import hashlib
import json
import re
import subprocess
import sys
import tarfile
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
PINNED_LICENSES = {
    "merman-resvg": ("LICENSE-BSD-2-Clause", "THIRD_PARTY_LICENSES/resvg-iir-blur/LICENSE"),
    "merman-krilla": ("icc/LICENSE", "THIRD_PARTY_LICENSES/krilla/LICENSE-ICC"),
}


class PackageLegalMaterialError(RuntimeError):
    """A governed Cargo package has missing or invalid legal material."""


def pinned_license_digests(package_name: str) -> dict[str, str]:
    if (pin := PINNED_LICENSES.get(package_name)) is None:
        return {}
    package_path, source_path = pin
    contract = json.loads((ROOT / "docs/release/THIRD_PARTY_COMPONENTS.json").read_text(encoding="utf-8"))
    digests = [
        entry["sha256"]
        for component in contract["components"]
        for entry in component["license_files"]
        if entry["path"] == source_path
    ]
    if len(digests) != 1 or not isinstance(digests[0], str) or not re.fullmatch(r"[0-9a-f]{64}", digests[0]):
        raise PackageLegalMaterialError(f"{package_name} is missing a unique upstream SHA-256 for {source_path}")
    return {package_path: digests[0]}


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
    elif license_expression == "(MIT OR Apache-2.0) AND BSD-2-Clause":
        required.update({"LICENSE-MIT", "LICENSE-APACHE", "LICENSE-BSD-2-Clause"})
    elif license_expression == "EPL-2.0":
        required.add("LICENSES/EPL-2.0.txt")
    elif license_expression in {"MIT", "Apache-2.0"}:
        required.add("LICENSE")
    else:
        raise PackageLegalMaterialError(
            f"{package['name']} has an unsupported or missing license expression: "
            f"{license_expression!r}"
        )

    if pin := PINNED_LICENSES.get(package["name"]):
        required.add(pin[0])

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
    listing = {path.replace("\\", "/") for path in listing}
    missing = required_legal_paths(package) - listing
    if missing:
        raise PackageLegalMaterialError(
            f"{package['name']} package omits legal files: {', '.join(sorted(missing))}"
        )


def verify_package_archive(package: dict[str, Any], archive: Path) -> None:
    prefix = f"{package['name']}-{package['version']}/"
    crate_root = Path(package["manifest_path"]).parent
    required = required_legal_paths(package)
    files: set[str] = set()
    try:
        pinned = pinned_license_digests(package["name"])
        with tarfile.open(archive, "r:gz") as crate:
            for member in crate:
                if not member.name.startswith(prefix):
                    continue
                name = member.name.removeprefix(prefix)
                if name not in required:
                    continue
                if name in files or not member.isfile():
                    raise PackageLegalMaterialError(f"{archive.name} has an invalid legal file entry: {name}")
                content = crate.extractfile(member)
                if content is None:
                    raise PackageLegalMaterialError(f"{archive.name} cannot read legal file: {name}")
                with content:
                    if digest := pinned.get(name):
                        if hashlib.file_digest(content, "sha256").hexdigest() != digest:
                            raise PackageLegalMaterialError(f"{archive.name} legal file differs from pinned upstream SHA-256: {name}")
                    else:
                        expected = (crate_root / name).read_bytes()
                        if not expected or member.size != len(expected) or content.read() != expected:
                            raise PackageLegalMaterialError(f"{archive.name} legal file differs from source: {name}")
                files.add(name)
    except (OSError, tarfile.TarError) as error:
        raise PackageLegalMaterialError(f"cannot inspect {archive.name}: {error}") from error
    verify_package_listing(package, files)


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
    # Cargo prints platform-native separators; normalize before comparing with
    # the portable package-relative paths derived from ``Path.as_posix()``.
    return {
        line.strip().replace("\\", "/")
        for line in result.stdout.splitlines()
        if line.strip()
    }


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
