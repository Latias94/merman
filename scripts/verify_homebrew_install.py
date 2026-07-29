#!/usr/bin/env python3
"""Verify the version-gated contract of an installed Homebrew formula."""

from __future__ import annotations

import argparse
from pathlib import Path
import subprocess
import sys

from release_version import ReleaseVersion, parse_release_version
from verify_cli_installation import (
    CAPABILITIES_SCHEMA_VERSION,
    CLI_CONTRACT_VERSION,
    COMMANDS,
    HOMEBREW_COMPLETION_PATHS,
    MANPAGE_NAMES,
    CliInstallationError,
    CommandRunner,
    read_release_contract,
    verify_cli_installation,
)


ROOT = Path(__file__).resolve().parents[1]
COMPLETION_PATHS = HOMEBREW_COMPLETION_PATHS
HomebrewVerificationError = CliInstallationError
_read_release_contract = read_release_contract


def _stable_version(value: str, label: str) -> ReleaseVersion:
    try:
        version = parse_release_version(value, allow_v_prefix=False)
    except ValueError as error:
        raise HomebrewVerificationError(f"invalid {label}: {error}") from error
    if version.kind != "stable" or version.build_metadata is not None:
        raise HomebrewVerificationError(f"{label} must be a stable X.Y.Z version")
    return version


def requires_support_assets(formula_version: str, threshold: str) -> bool:
    current = _stable_version(formula_version, "Homebrew formula version")
    minimum = _stable_version(threshold, "support-assets threshold")
    return (current.major, current.minor, current.patch) >= (
        minimum.major,
        minimum.minor,
        minimum.patch,
    )


def select_version_verifier(
    *,
    formula_version: str,
    support_assets_since: str,
    contract_root: Path,
    fallback: Path,
) -> Path:
    candidate = contract_root / "scripts/verify_homebrew_install.py"
    if candidate.is_file() and not candidate.is_symlink():
        return candidate
    if requires_support_assets(formula_version, support_assets_since):
        raise HomebrewVerificationError(
            f"release {formula_version} does not contain its Homebrew verifier: {candidate}"
        )
    return fallback


def verify_homebrew_install(
    *,
    formula_version: str,
    support_assets_since: str,
    prefix: Path,
    binary: Path,
    contract_root: Path = ROOT,
    runner: CommandRunner = subprocess.run,
) -> bool:
    """Verify support assets when the installed formula reaches the contract version."""
    if not requires_support_assets(formula_version, support_assets_since):
        return False
    verify_cli_installation(
        package_version=formula_version,
        prefix=prefix,
        binary=binary,
        contract_root=contract_root,
        completion_layout="homebrew",
        runner=runner,
    )
    return True


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--formula-version", required=True)
    parser.add_argument("--support-assets-since", required=True)
    parser.add_argument("--prefix", type=Path)
    parser.add_argument("--binary", type=Path)
    parser.add_argument(
        "--contract-root",
        type=Path,
        default=ROOT,
        help="release source tree that owns the installed version's CLI contract",
    )
    parser.add_argument(
        "--select-version-verifier",
        action="store_true",
        help="print the verifier owned by the formula version and exit",
    )
    args = parser.parse_args(argv)
    if not args.select_version_verifier and args.prefix is None:
        parser.error("--prefix is required unless --select-version-verifier is used")
    return args


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    if args.select_version_verifier:
        try:
            selected = select_version_verifier(
                formula_version=args.formula_version,
                support_assets_since=args.support_assets_since,
                contract_root=args.contract_root,
                fallback=Path(__file__).resolve(),
            )
        except HomebrewVerificationError as error:
            print(f"error: {error}", file=sys.stderr)
            return 1
        print(selected)
        return 0
    assert args.prefix is not None
    binary = args.binary or args.prefix / "bin/merman-cli"
    try:
        verified_assets = verify_homebrew_install(
            formula_version=args.formula_version,
            support_assets_since=args.support_assets_since,
            prefix=args.prefix,
            binary=binary,
            contract_root=args.contract_root,
        )
    except HomebrewVerificationError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    if verified_assets:
        print(
            f"verified Homebrew CLI contract {CLI_CONTRACT_VERSION} and support assets "
            f"for {args.formula_version}"
        )
    else:
        print(
            f"legacy Homebrew version {args.formula_version} does not require support assets; "
            f"support assets become mandatory at {args.support_assets_since}"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
