#!/usr/bin/env python3
"""Generate a deterministic Rust dependency license report with cargo-about."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "THIRD_PARTY_LICENSES" / "rust-cargo-dependencies.json"
CARGO_ABOUT_VERSION = "0.9.1"
SCHEMA_VERSION = 1


class RustLicenseReportError(Exception):
    pass


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--check", action="store_true")
    mode.add_argument("--write", action="store_true")
    return parser.parse_args(argv)


def generate_report(root: Path) -> bytes:
    verify_cargo_about_version(root)
    with tempfile.TemporaryDirectory(prefix="merman-cargo-about-") as temporary:
        raw_path = Path(temporary) / "cargo-about.json"
        command = [
            "cargo",
            "about",
            "generate",
            "--config",
            "about.toml",
            "--workspace",
            "--all-features",
            "--locked",
            "--offline",
            "--fail",
            "--format",
            "json",
            "--output-file",
            str(raw_path),
        ]
        result = subprocess.run(command, cwd=root, text=True, capture_output=True)
        if result.returncode != 0:
            raise RustLicenseReportError(
                "cargo-about failed:\n" + (result.stderr or result.stdout).strip()
            )
        try:
            raw = json.loads(raw_path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as error:
            raise RustLicenseReportError(f"could not read cargo-about output: {error}") from error
    normalized = normalize_report(raw, root)
    return (json.dumps(normalized, indent=2, ensure_ascii=True) + "\n").encode()


def verify_cargo_about_version(root: Path) -> None:
    try:
        result = subprocess.run(
            ["cargo", "about", "--version"],
            cwd=root,
            text=True,
            capture_output=True,
            check=True,
        )
    except (OSError, subprocess.CalledProcessError) as error:
        raise RustLicenseReportError(
            f"cargo-about {CARGO_ABOUT_VERSION} is required"
        ) from error
    actual = result.stdout.strip()
    if actual != f"cargo-about {CARGO_ABOUT_VERSION}":
        raise RustLicenseReportError(
            f"cargo-about {CARGO_ABOUT_VERSION} is required, found {actual!r}"
        )


def normalize_report(raw: dict[str, Any], root: Path) -> dict[str, Any]:
    licenses: list[dict[str, Any]] = []
    for license_entry in require_list(raw, "licenses"):
        packages = normalized_packages(license_entry)
        if not packages:
            continue
        text = require_string(license_entry, "text")
        licenses.append(
            {
                "id": require_string(license_entry, "id"),
                "name": require_string(license_entry, "name"),
                "text_sha256": hashlib.sha256(text.encode()).hexdigest(),
                "text": text,
                "packages": packages,
            }
        )
    licenses.sort(key=lambda item: (item["id"], item["text_sha256"]))
    if not licenses:
        raise RustLicenseReportError("cargo-about returned no third-party licenses")

    return {
        "schema_version": SCHEMA_VERSION,
        "generator": {
            "name": "cargo-about",
            "version": CARGO_ABOUT_VERSION,
            "command_profile": "workspace-all-features-runtime",
            "offline": True,
            "cargo_lock_sha256": sha256_file(root / "Cargo.lock"),
            "configuration_sha256": sha256_file(root / "about.toml"),
        },
        "licenses": licenses,
    }


def normalized_packages(license_entry: dict[str, Any]) -> list[dict[str, Any]]:
    packages: dict[tuple[str, str, str], dict[str, Any]] = {}
    for use in require_list(license_entry, "used_by"):
        package = use.get("crate")
        if not isinstance(package, dict):
            raise RustLicenseReportError("cargo-about used_by entry has no crate metadata")
        source = package.get("source")
        if not isinstance(source, str):
            continue
        name = require_string(package, "name")
        version = require_string(package, "version")
        key = (name, version, source)
        packages[key] = {
            "name": name,
            "version": version,
            "source": source,
            "license_expression": package.get("license"),
            "authors": sorted(
                author for author in package.get("authors", []) if isinstance(author, str)
            ),
            "repository": package.get("repository"),
        }
    return [packages[key] for key in sorted(packages)]


def require_list(value: dict[str, Any], key: str) -> list[Any]:
    result = value.get(key)
    if not isinstance(result, list):
        raise RustLicenseReportError(f"cargo-about output field {key!r} is not a list")
    return result


def require_string(value: dict[str, Any], key: str) -> str:
    result = value.get(key)
    if not isinstance(result, str) or not result:
        raise RustLicenseReportError(f"cargo-about output field {key!r} is not a string")
    return result


def sha256_file(path: Path) -> str:
    if not path.is_file():
        raise RustLicenseReportError(f"missing report input: {path}")
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv or sys.argv[1:])
    try:
        generated = generate_report(ROOT)
        if args.write:
            OUTPUT.parent.mkdir(parents=True, exist_ok=True)
            OUTPUT.write_bytes(generated)
        if not OUTPUT.is_file():
            raise RustLicenseReportError(f"missing generated report: {OUTPUT}")
        if OUTPUT.read_bytes() != generated:
            raise RustLicenseReportError(
                "Rust dependency license report is stale; run "
                "`python3 scripts/generate-rust-license-report.py --write`"
            )
    except (OSError, RustLicenseReportError) as error:
        print(f"Rust dependency license report failed: {error}", file=sys.stderr)
        return 1
    print(f"Rust dependency license report: ok ({len(generated)} bytes)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
