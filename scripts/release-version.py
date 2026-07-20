#!/usr/bin/env python3
"""Release version helpers used by GitHub Actions."""

from __future__ import annotations

import argparse
import json
import re
import sys
import tomllib
from pathlib import Path

try:
    from scripts.release_version import parse_release_version
except ModuleNotFoundError:
    from release_version import parse_release_version


ROOT = Path(__file__).resolve().parents[1]


def semver_to_pep440(version: str) -> str:
    return parse_release_version(version).to_pep440()


def semver_to_vscode_manifest_version(version: str) -> str:
    return parse_release_version(version).to_vscode_manifest()


def cargo_workspace_version() -> str:
    with (ROOT / "Cargo.toml").open("rb") as handle:
        return tomllib.load(handle)["workspace"]["package"]["version"]


def python_project_version() -> str:
    with (ROOT / "platforms/python/merman/pyproject.toml").open("rb") as handle:
        return tomllib.load(handle)["project"]["version"]


def flutter_version() -> str:
    for line in (ROOT / "platforms/flutter/pubspec.yaml").read_text().splitlines():
        if line.startswith("version:"):
            return line.split(":", 1)[1].strip()
    raise ValueError("platforms/flutter/pubspec.yaml does not contain a version field")


def web_version() -> str:
    data = json.loads((ROOT / "platforms/web/package.json").read_text())
    return str(data["version"])


def vscode_extension_version() -> str:
    data = json.loads((ROOT / "tools/vscode-extension/package.json").read_text())
    return str(data["version"])


def android_version() -> str:
    text = (ROOT / "platforms/android/build.gradle.kts").read_text()
    match = re.search(r'^version\s*=\s*"([^"]+)"', text, flags=re.MULTILINE)
    if not match:
        raise ValueError("platforms/android/build.gradle.kts does not contain a version assignment")
    return match.group(1)


def check_versions(version: str) -> int:
    release = parse_release_version(version)
    expected = {
        "Cargo workspace": release.canonical,
        "Flutter pubspec": release.canonical,
        "Web package": release.canonical,
        "VS Code extension": release.to_vscode_manifest(),
        "Android package": release.canonical,
        "Python package": release.to_pep440(),
    }
    actual = {
        "Cargo workspace": cargo_workspace_version(),
        "Flutter pubspec": flutter_version(),
        "Web package": web_version(),
        "VS Code extension": vscode_extension_version(),
        "Android package": android_version(),
        "Python package": python_project_version(),
    }

    failed = False
    for name, expected_version in expected.items():
        actual_version = actual[name]
        if actual_version == expected_version:
            print(f"{name}: {actual_version}")
            continue
        failed = True
        print(
            f"::error::{name} version {actual_version!r} does not match expected {expected_version!r}",
            file=sys.stderr,
        )

    return 1 if failed else 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=["check", "pep440", "vscode"])
    parser.add_argument("--version", required=True)
    args = parser.parse_args()

    try:
        if args.command == "pep440":
            print(semver_to_pep440(args.version))
            return 0
        if args.command == "vscode":
            print(semver_to_vscode_manifest_version(args.version))
            return 0
        if args.command == "check":
            return check_versions(args.version)
    except (OSError, KeyError, ValueError, json.JSONDecodeError, tomllib.TOMLDecodeError) as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 2
    raise AssertionError(args.command)


if __name__ == "__main__":
    raise SystemExit(main())
