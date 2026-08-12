#!/usr/bin/env python3
"""Require independent crates to change version when their owned tree changes."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import subprocess
import sys
import tomllib


ROOT = Path(__file__).resolve().parents[1]


def run_git(root: Path, *args: str, check: bool = True) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["git", *args],
        cwd=root,
        check=check,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )


def independent_package_names(root: Path) -> tuple[str, ...]:
    manifest = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))
    metadata = manifest.get("workspace", {}).get("metadata", {}).get("merman-release", {})
    names = metadata.get("independent-packages", [])
    if not isinstance(names, list) or not all(isinstance(name, str) for name in names):
        raise ValueError(
            "Cargo.toml workspace.metadata.merman-release.independent-packages "
            "must be an array of package names"
        )
    if len(names) != len(set(names)):
        raise ValueError("independent package declarations must be unique")
    return tuple(names)


def current_workspace_packages(root: Path) -> dict[str, tuple[Path, str]]:
    result = subprocess.run(
        ["cargo", "metadata", "--no-deps", "--format-version", "1"],
        cwd=root,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
    )
    metadata = json.loads(result.stdout)
    packages: dict[str, tuple[Path, str]] = {}
    for package in metadata["packages"]:
        manifest = Path(package["manifest_path"]).resolve()
        packages[package["name"]] = (manifest.relative_to(root.resolve()), package["version"])
    return packages


def manifest_version_at(root: Path, ref: str, manifest: Path) -> str | None:
    result = run_git(root, "show", f"{ref}:{manifest.as_posix()}", check=False)
    if result.returncode != 0:
        return None
    package = tomllib.loads(result.stdout).get("package", {})
    version = package.get("version")
    if not isinstance(version, str):
        raise ValueError(f"{ref}:{manifest} has no literal package.version")
    return version


def changed_paths(root: Path, base_ref: str, target_ref: str, package_root: Path) -> tuple[str, ...]:
    result = run_git(
        root,
        "diff",
        "--name-only",
        f"{base_ref}..{target_ref}",
        "--",
        package_root.as_posix(),
    )
    return tuple(line for line in result.stdout.splitlines() if line)


def verify(root: Path, base_ref: str, target_ref: str) -> tuple[str, ...]:
    run_git(root, "rev-parse", "--verify", f"{base_ref}^{{commit}}")
    run_git(root, "rev-parse", "--verify", f"{target_ref}^{{commit}}")
    if run_git(root, "merge-base", "--is-ancestor", base_ref, target_ref, check=False).returncode:
        raise ValueError(f"base ref {base_ref!r} is not an ancestor of {target_ref!r}")

    packages = current_workspace_packages(root)
    failures: list[str] = []
    for name in independent_package_names(root):
        package = packages.get(name)
        if package is None:
            failures.append(f"independent package {name!r} is not a workspace package")
            continue
        manifest, current_version = package
        package_root = manifest.parent
        changes = changed_paths(root, base_ref, target_ref, package_root)
        if not changes:
            print(f"{name}: unchanged at {current_version}")
            continue
        base_version = manifest_version_at(root, base_ref, manifest)
        if base_version is None:
            print(f"{name}: new independent package at {current_version}")
            continue
        if current_version == base_version:
            failures.append(
                f"{name} changed since {base_ref} but still uses {current_version}: "
                + ", ".join(changes)
            )
            continue
        print(f"{name}: {base_version} -> {current_version} ({len(changes)} changed paths)")
    return tuple(failures)


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base-ref", required=True)
    parser.add_argument("--target-ref", default="HEAD")
    parser.add_argument("--repo-root", type=Path, default=ROOT)
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    failures = verify(args.repo_root.resolve(), args.base_ref, args.target_ref)
    for failure in failures:
        print(f"::error::{failure}", file=sys.stderr)
    return 1 if failures else 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, subprocess.CalledProcessError, ValueError, json.JSONDecodeError) as error:
        print(f"verify-independent-crate-version-bumps.py: {error}", file=sys.stderr)
        raise SystemExit(2) from error
