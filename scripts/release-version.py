#!/usr/bin/env python3
"""Release version helpers used by GitHub Actions."""

from __future__ import annotations

import argparse
import json
import re
import sys
import tomllib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def semver_to_pep440(version: str) -> str:
    match = re.fullmatch(r"(\d+\.\d+\.\d+)(?:-([0-9A-Za-z.-]+))?", version)
    if not match:
        raise ValueError(f"unsupported SemVer release version: {version!r}")

    base, prerelease = match.groups()
    if prerelease is None:
        return base

    pre_match = re.fullmatch(r"(alpha|beta|rc)\.(\d+)", prerelease)
    if not pre_match:
        raise ValueError(f"unsupported prerelease for PyPI version: {version!r}")

    label, number = pre_match.groups()
    pep440_label = {"alpha": "a", "beta": "b", "rc": "rc"}[label]
    return f"{base}{pep440_label}{number}"


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


def android_version() -> str:
    text = (ROOT / "platforms/android/build.gradle.kts").read_text()
    match = re.search(r'^version\s*=\s*"([^"]+)"', text, flags=re.MULTILINE)
    if not match:
        raise ValueError("platforms/android/build.gradle.kts does not contain a version assignment")
    return match.group(1)


def check_versions(version: str) -> int:
    expected = {
        "Cargo workspace": version,
        "Flutter pubspec": version,
        "Web package": version,
        "Android package": version,
        "Python package": semver_to_pep440(version),
    }
    actual = {
        "Cargo workspace": cargo_workspace_version(),
        "Flutter pubspec": flutter_version(),
        "Web package": web_version(),
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
    parser.add_argument("command", choices=["check", "pep440"])
    parser.add_argument("--version", required=True)
    args = parser.parse_args()

    if args.command == "pep440":
        print(semver_to_pep440(args.version))
        return 0
    if args.command == "check":
        return check_versions(args.version)
    raise AssertionError(args.command)


if __name__ == "__main__":
    raise SystemExit(main())
