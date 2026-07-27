#!/usr/bin/env python3
"""Release version helpers used by GitHub Actions."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

try:
    from scripts import release_readme
    from scripts.release_projection import (
        apply_readme_install_mode,
        apply_version_update,
        format_verification_failures,
        verify_repository,
    )
    from scripts.release_version import parse_release_version
except ModuleNotFoundError:
    import release_readme
    from release_projection import (
        apply_readme_install_mode,
        apply_version_update,
        format_verification_failures,
        verify_repository,
    )
    from release_version import parse_release_version


ROOT = Path(__file__).resolve().parents[1]


def semver_to_pep440(version: str) -> str:
    return parse_release_version(version).to_pep440()


def canonical_release_version(version: str) -> str:
    return parse_release_version(version).canonical


def npm_dist_tag(version: str) -> str:
    return parse_release_version(version).to_npm_dist_tag()


def cargo_workspace_version() -> str:
    return verify_repository(ROOT).authority.canonical


def check_versions(
    version: str | None = None,
    *,
    required_readme_mode: str | None = None,
) -> int:
    result = verify_repository(
        ROOT,
        expected_version=version,
        required_readme_mode=required_readme_mode,
    )
    for observation in result.observations:
        if observation.matches:
            print(f"{observation.label}: {observation.actual}")
    failures = format_verification_failures(result)
    for failure in failures:
        print(f"::error::{failure}", file=sys.stderr)
    return 1 if failures else 0


def main() -> int:
    parser = argparse.ArgumentParser(
        description=(
            "Verify or project the root Cargo workspace release version across every "
            "workspace-coupled package surface."
        )
    )
    parser.add_argument(
        "command",
        nargs="?",
        default="verify",
        choices=[
            "canonical",
            "check",
            "npm-dist-tag",
            "pep440",
            "set",
            "set-readme-mode",
            "verify",
        ],
    )
    parser.add_argument("--version")
    parser.add_argument("--mode", choices=sorted(release_readme.MODES))
    args = parser.parse_args()

    try:
        if args.command == "verify":
            if args.version is not None:
                parser.error("verify reads the authority from Cargo.toml; do not pass --version")
            if args.mode is not None:
                parser.error("verify reads the README mode from Cargo.toml; do not pass --mode")
            return check_versions()
        if args.command == "set-readme-mode":
            if args.version is None:
                parser.error("set-readme-mode requires --version")
            if args.mode is None:
                parser.error("set-readme-mode requires --mode")
            changed = apply_readme_install_mode(ROOT, args.version, args.mode)
            for path in changed:
                print(f"updated {path}")
            if not changed:
                print(
                    f"README installation mode already matches {args.mode} "
                    f"for {canonical_release_version(args.version)}"
                )
            return 0
        if args.mode is not None:
            parser.error(f"{args.command} does not accept --mode")
        if args.version is None:
            parser.error(f"{args.command} requires --version")
        if args.command == "canonical":
            print(canonical_release_version(args.version))
            return 0
        if args.command == "npm-dist-tag":
            print(npm_dist_tag(args.version))
            return 0
        if args.command == "pep440":
            print(semver_to_pep440(args.version))
            return 0
        if args.command == "check":
            return check_versions(
                args.version,
                required_readme_mode=release_readme.REGISTRY_MODE,
            )
        if args.command == "set":
            changed = apply_version_update(ROOT, args.version)
            for path in changed:
                print(f"updated {path}")
            if not changed:
                print(f"release projections already match {args.version}")
            return 0
    except (OSError, KeyError, ValueError) as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 2
    raise AssertionError(args.command)


if __name__ == "__main__":
    raise SystemExit(main())
