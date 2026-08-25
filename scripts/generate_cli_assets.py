#!/usr/bin/env python3
"""Check or update tracked merman-cli completions and the manual page."""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import subprocess
import sys

from artifact_profile_recipe import load_artifact_profile


REPO_ROOT = Path(__file__).resolve().parents[1]
PACKAGE = "merman-cli"
PROFILE = "cli-release"
CHECK_TEST = "app::distribution_assets::tracked_distribution_assets_are_current"
WRITE_TEST = "app::distribution_assets::write_distribution_assets"
ASSET_ROOTS = ("assets/completions", "assets/man")
TEMPORAL_CHECK_ENVIRONMENTS = (
    {"TZ": "UTC", "SOURCE_DATE_EPOCH": "0"},
    {"TZ": "Pacific/Kiritimati", "SOURCE_DATE_EPOCH": "4102444800"},
)


def cargo_feature_args() -> list[str]:
    recipe = load_artifact_profile(PROFILE)
    if recipe.package != PACKAGE:
        raise RuntimeError(
            f"artifact profile {PROFILE!r} belongs to {recipe.package!r}, not {PACKAGE!r}"
        )
    arguments: list[str] = []
    if not recipe.default_features:
        arguments.append("--no-default-features")
    if recipe.features:
        arguments.extend(("--features", recipe.feature_argument))
    return arguments


def cargo_test(test_name: str, *, ignored: bool, environment: dict[str, str]) -> None:
    command = [
        "cargo",
        "test",
        "--locked",
        "-p",
        PACKAGE,
        *cargo_feature_args(),
        "--bin",
        PACKAGE,
        test_name,
    ]
    discovery_arguments = ["--", "--list", "--format", "terse"]
    if ignored:
        discovery_arguments.append("--ignored")
    discovery = subprocess.run(
        [*command, *discovery_arguments],
        cwd=REPO_ROOT,
        env=environment,
        check=True,
        stdout=subprocess.PIPE,
        text=True,
    )
    expected = f"{test_name}: test"
    if expected not in discovery.stdout.splitlines():
        raise RuntimeError(
            f"Cargo did not discover required CLI asset test {test_name!r} "
            f"for artifact profile {PROFILE!r}"
        )
    command.extend(("--", "--exact"))
    if ignored:
        command.append("--ignored")
    subprocess.run(command, cwd=REPO_ROOT, env=environment, check=True)


def package_paths() -> set[str]:
    result = subprocess.run(
        [
            "cargo",
            "package",
            "--locked",
            "-p",
            PACKAGE,
            "--allow-dirty",
            "--list",
        ],
        cwd=REPO_ROOT,
        check=True,
        stdout=subprocess.PIPE,
        text=True,
    )
    return {line.strip() for line in result.stdout.splitlines() if line.strip()}


def verify_source_package() -> None:
    expected = {"src/app/distribution_assets.rs"}
    for relative_root in ASSET_ROOTS:
        root = REPO_ROOT / "crates" / PACKAGE / relative_root
        expected.update(
            path.relative_to(REPO_ROOT / "crates" / PACKAGE).as_posix()
            for path in root.rglob("*")
            if path.is_file()
        )
    missing = sorted(expected - package_paths())
    if missing:
        formatted = "\n".join(f"  - {path}" for path in missing)
        raise RuntimeError(f"merman-cli source package omits generated assets:\n{formatted}")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--check", action="store_true", help="fail when tracked assets are stale")
    mode.add_argument("--write", action="store_true", help="rewrite tracked assets")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    environment = os.environ.copy()
    if args.write:
        environment["MERMAN_UPDATE_CLI_ASSETS"] = "1"
        cargo_test(WRITE_TEST, ignored=True, environment=environment)
    for temporal_environment in TEMPORAL_CHECK_ENVIRONMENTS:
        check_environment = environment | temporal_environment
        cargo_test(CHECK_TEST, ignored=False, environment=check_environment)
    verify_source_package()
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, subprocess.CalledProcessError, RuntimeError) as error:
        print(f"generate_cli_assets.py: {error}", file=sys.stderr)
        raise SystemExit(1) from error
