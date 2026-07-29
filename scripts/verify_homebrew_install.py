#!/usr/bin/env python3
"""Verify the version-gated contract of an installed Homebrew formula."""

from __future__ import annotations

import argparse
from collections.abc import Callable
from datetime import date
import json
from pathlib import Path
import re
import shlex
import subprocess
import sys
from typing import TypeAlias

from release_version import ReleaseVersion, parse_release_version


ROOT = Path(__file__).resolve().parents[1]
PROFILE_DESCRIPTOR = Path("capabilities/artifact-profiles-v1.json")
CLI_RELEASE_PROFILE = "cli-release"
CLI_CONTRACT_VERSION = 3
CAPABILITIES_SCHEMA_VERSION = 2
COMMANDS = (
    "batch",
    "capabilities",
    "completion",
    "detect",
    "fix",
    "layout",
    "lint",
    "lint-rules",
    "mmdc",
    "parse",
    "render",
)
COMPLETION_PATHS = {
    "bash": Path("etc/bash_completion.d/merman-cli"),
    "zsh": Path("share/zsh/site-functions/_merman-cli"),
    "fish": Path("share/fish/vendor_completions.d/merman-cli.fish"),
    "powershell": Path("share/pwsh/completions/_merman-cli.ps1"),
}
MANPAGE_NAMES = (
    "merman-cli-batch.1",
    "merman-cli-capabilities.1",
    "merman-cli-completion.1",
    "merman-cli-detect.1",
    "merman-cli-fix.1",
    "merman-cli-layout.1",
    "merman-cli-lint-rules.1",
    "merman-cli-lint.1",
    "merman-cli-mmdc.1",
    "merman-cli-parse.1",
    "merman-cli-render.1",
    "merman-cli.1",
)
RUNTIME_TIMEOUT_SECONDS = 30
RUNTIME_OUTPUT_MAX_BYTES = 4 * 1024 * 1024
SUPPORT_ASSET_MAX_BYTES = 4 * 1024 * 1024

CommandRunner: TypeAlias = Callable[..., subprocess.CompletedProcess[bytes]]


class HomebrewVerificationError(RuntimeError):
    """Raised when an installed formula violates the Homebrew contract."""


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


def _read_release_contract(root: Path) -> tuple[dict, dict]:
    path = root / PROFILE_DESCRIPTOR
    try:
        descriptor = json.loads(path.read_text(encoding="utf-8"))
        profiles = descriptor["profiles"]
        authority = descriptor["capability_authority"]
    except (OSError, UnicodeError, json.JSONDecodeError, KeyError, TypeError) as error:
        raise HomebrewVerificationError(
            f"cannot read CLI artifact profile {path}: {error}"
        ) from error
    matches = [
        profile
        for profile in profiles
        if isinstance(profile, dict) and profile.get("id") == CLI_RELEASE_PROFILE
    ]
    if len(matches) != 1:
        raise HomebrewVerificationError(
            f"expected exactly one {CLI_RELEASE_PROFILE!r} artifact profile"
        )
    if not isinstance(authority, dict):
        raise HomebrewVerificationError("capability authority must be an object")
    if not isinstance(authority.get("schema_version"), int):
        raise HomebrewVerificationError("capability authority schema version is invalid")
    digest = authority.get("digest")
    if not isinstance(digest, str) or re.fullmatch(r"sha256:[0-9a-f]{64}", digest) is None:
        raise HomebrewVerificationError("capability authority digest is invalid")
    return matches[0], authority


def _read_profile(root: Path) -> dict:
    return _read_release_contract(root)[0]


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


def _run(
    binary: Path,
    arguments: list[str],
    *,
    runner: CommandRunner,
) -> bytes:
    try:
        completed = runner(
            [str(binary), *arguments],
            check=False,
            capture_output=True,
            timeout=RUNTIME_TIMEOUT_SECONDS,
        )
    except (OSError, subprocess.SubprocessError) as error:
        raise HomebrewVerificationError(
            f"cannot execute {binary} {' '.join(arguments)}: {error}"
        ) from error
    if completed.returncode != 0:
        stderr = completed.stderr[:4096].decode("utf-8", errors="replace").strip()
        raise HomebrewVerificationError(
            f"{binary.name} {' '.join(arguments)} failed with "
            f"exit code {completed.returncode}: {stderr}"
        )
    if len(completed.stdout) > RUNTIME_OUTPUT_MAX_BYTES:
        raise HomebrewVerificationError(
            f"{binary.name} {' '.join(arguments)} exceeded its output budget"
        )
    return completed.stdout


def _require_regular_path(path: Path, label: str) -> None:
    if path.is_symlink() or not path.is_file():
        raise HomebrewVerificationError(f"missing installed {label}: {path}")


def _read_regular_file(path: Path, label: str) -> bytes:
    _require_regular_path(path, label)
    try:
        with path.open("rb") as handle:
            contents = handle.read(SUPPORT_ASSET_MAX_BYTES + 1)
    except OSError as error:
        raise HomebrewVerificationError(f"cannot read installed {label}: {error}") from error
    if not contents:
        raise HomebrewVerificationError(f"installed {label} is empty: {path}")
    if len(contents) > SUPPORT_ASSET_MAX_BYTES:
        raise HomebrewVerificationError(
            f"installed {label} exceeds {SUPPORT_ASSET_MAX_BYTES} bytes: {path}"
        )
    return contents


def _string_set(value: object, label: str) -> set[str]:
    if not isinstance(value, list) or any(not isinstance(item, str) for item in value):
        raise HomebrewVerificationError(f"{label} must be a string list")
    if len(value) != len(set(value)):
        raise HomebrewVerificationError(f"{label} must not contain duplicates")
    return set(value)


def _id_set(value: object, label: str) -> set[str]:
    if not isinstance(value, list):
        raise HomebrewVerificationError(f"{label} must be a list")
    identifiers: list[str] = []
    for item in value:
        if not isinstance(item, dict) or not isinstance(item.get("id"), str):
            raise HomebrewVerificationError(f"{label} entries must have string ids")
        identifiers.append(item["id"])
    return _string_set(identifiers, label)


def _verify_manpage(contents: bytes, *, name: str, formula_version: str) -> None:
    try:
        text = contents.decode("utf-8")
    except UnicodeError as error:
        raise HomebrewVerificationError(
            f"installed man page is not UTF-8: {name}"
        ) from error

    headers = [line for line in text.splitlines() if line.startswith(".TH ")]
    if len(headers) != 1:
        raise HomebrewVerificationError(
            f"installed man page must contain exactly one .TH header: {name}"
        )
    try:
        fields = shlex.split(headers[0], posix=True)
    except ValueError as error:
        raise HomebrewVerificationError(
            f"installed man page has an invalid .TH header: {name}"
        ) from error

    expected_title = name.removesuffix(".1").upper()
    if len(fields) != 6 or fields[:3] != [".TH", expected_title, "1"]:
        raise HomebrewVerificationError(
            f"installed man page title or section does not match {name}"
        )
    if re.fullmatch(r"[0-9]{4}-[0-9]{2}-[0-9]{2}", fields[3]) is None:
        raise HomebrewVerificationError(f"installed man page date is invalid: {name}")
    try:
        date.fromisoformat(fields[3])
    except ValueError as error:
        raise HomebrewVerificationError(
            f"installed man page date is invalid: {name}"
        ) from error
    if fields[4:] != [f"Merman {formula_version}", "Merman CLI Manual"]:
        raise HomebrewVerificationError(
            f"installed man page version or manual name does not match {name}"
        )
    if "\n.SH NAME\n" not in text:
        raise HomebrewVerificationError(
            f"installed man page is missing its NAME section: {name}"
        )


def _verify_capabilities(
    payload: bytes,
    *,
    formula_version: str,
    profile: dict,
    authority: dict,
) -> None:
    try:
        document = json.loads(payload.decode("utf-8"))
    except (UnicodeError, json.JSONDecodeError) as error:
        raise HomebrewVerificationError(f"capabilities output is not JSON: {error}") from error
    if not isinstance(document, dict):
        raise HomebrewVerificationError("capabilities output must be a JSON object")
    if document.get("schema_version") != CAPABILITIES_SCHEMA_VERSION:
        raise HomebrewVerificationError("unexpected capabilities schema version")
    if document.get("cli_contract_version") != CLI_CONTRACT_VERSION:
        raise HomebrewVerificationError("unexpected CLI contract version")
    package = document.get("package")
    if package != {"name": "merman-cli", "version": formula_version}:
        raise HomebrewVerificationError("capabilities package identity does not match Homebrew")
    expected_descriptor = {
        "schema_version": authority["schema_version"],
        "digest": authority["digest"],
    }
    if document.get("descriptor") != expected_descriptor:
        raise HomebrewVerificationError(
            "capabilities descriptor does not match the release capability authority"
        )
    compatibility = document.get("compatibility")
    if not isinstance(compatibility, dict) or any(
        not isinstance(compatibility.get(key), str) or not compatibility[key]
        for key in ("mermaid", "mmdc")
    ):
        raise HomebrewVerificationError("capabilities compatibility metadata is incomplete")

    try:
        expected_capabilities = _string_set(
            profile["expected"]["capabilities"], "cli-release capabilities"
        )
        expected_outputs = _string_set(
            profile["expected"]["outputs"], "cli-release outputs"
        )
    except (KeyError, TypeError) as error:
        raise HomebrewVerificationError("cli-release expected surface is incomplete") from error
    if _string_set(document.get("commands"), "installed commands") != set(COMMANDS):
        raise HomebrewVerificationError("installed command set differs from CLI contract 3")
    if _id_set(document.get("capabilities"), "installed capabilities") != expected_capabilities:
        raise HomebrewVerificationError(
            "installed capability set differs from cli-release"
        )
    if _id_set(document.get("outputs"), "installed outputs") != expected_outputs:
        raise HomebrewVerificationError("installed output set differs from cli-release")


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
    try:
        installed_prefix = prefix.resolve(strict=True)
    except OSError as error:
        raise HomebrewVerificationError(
            f"cannot resolve Homebrew formula prefix {prefix}: {error}"
        ) from error
    if not installed_prefix.is_dir():
        raise HomebrewVerificationError(f"invalid Homebrew formula prefix: {prefix}")
    _require_regular_path(binary, "merman-cli binary")

    profile, authority = _read_release_contract(contract_root)
    capabilities = _run(binary, ["capabilities", "--json"], runner=runner)
    _verify_capabilities(
        capabilities,
        formula_version=formula_version,
        profile=profile,
        authority=authority,
    )

    for shell, relative in COMPLETION_PATHS.items():
        installed = _read_regular_file(
            installed_prefix / relative,
            f"{shell} completion",
        )
        generated = _run(binary, ["completion", shell], runner=runner)
        if installed != generated:
            raise HomebrewVerificationError(
                f"installed {shell} completion differs from runtime generation"
            )

    man_root = installed_prefix / "share/man/man1"
    observed_manpages = {
        path.name
        for path in man_root.glob("merman-cli*.1")
        if path.is_file() and not path.is_symlink()
    }
    if observed_manpages != set(MANPAGE_NAMES):
        raise HomebrewVerificationError(
            "installed man page set differs from CLI contract 3: "
            f"expected {sorted(MANPAGE_NAMES)}, got {sorted(observed_manpages)}"
        )
    for name in MANPAGE_NAMES:
        contents = _read_regular_file(man_root / name, f"man page {name}")
        _verify_manpage(contents, name=name, formula_version=formula_version)
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
