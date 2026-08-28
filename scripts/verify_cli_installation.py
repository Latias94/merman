#!/usr/bin/env python3
"""Verify an installed complete-profile Merman CLI and its support assets."""

from __future__ import annotations

import argparse
from collections.abc import Callable
from datetime import date
import gzip
import io
import json
from pathlib import Path
import re
import shlex
import subprocess
import sys
from typing import Literal, TypeAlias

if __package__:
    from .ascii_capability_contract import (
        AsciiCapabilityContractError,
        validate_ascii_capabilities,
    )
else:
    from ascii_capability_contract import (
        AsciiCapabilityContractError,
        validate_ascii_capabilities,
    )


ROOT = Path(__file__).resolve().parents[1]
PROFILE_DESCRIPTOR = Path("capabilities/artifact-profiles-v1.json")
CLI_RELEASE_PROFILE = "cli-release"
CLI_CONTRACT_VERSION = 5
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
    "rustdoc",
)
HOMEBREW_COMPLETION_PATHS = {
    "bash": Path("etc/bash_completion.d/merman-cli"),
    "zsh": Path("share/zsh/site-functions/_merman-cli"),
    "fish": Path("share/fish/vendor_completions.d/merman-cli.fish"),
    "powershell": Path("share/pwsh/completions/_merman-cli.ps1"),
}
NIX_COMPLETION_PATHS = {
    "bash": Path("share/bash-completion/completions/merman-cli"),
    "zsh": Path("share/zsh/site-functions/_merman-cli"),
    "fish": Path("share/fish/vendor_completions.d/merman-cli.fish"),
    "powershell": Path("share/pwsh/completions/_merman-cli.ps1"),
    "elvish": Path("share/elvish/lib/merman-cli.elv"),
}
COMPLETION_LAYOUTS = {
    "homebrew": HOMEBREW_COMPLETION_PATHS,
    "nix": NIX_COMPLETION_PATHS,
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
    "merman-cli-rustdoc-build.1",
    "merman-cli-rustdoc-check.1",
    "merman-cli-rustdoc.1",
    "merman-cli.1",
)
RUNTIME_TIMEOUT_SECONDS = 30
RUNTIME_OUTPUT_MAX_BYTES = 4 * 1024 * 1024
SUPPORT_ASSET_MAX_BYTES = 4 * 1024 * 1024

CommandRunner: TypeAlias = Callable[..., subprocess.CompletedProcess[bytes]]
CompletionLayout: TypeAlias = Literal["homebrew", "nix"]


class CliInstallationError(RuntimeError):
    """Raised when an installed CLI violates the complete-profile contract."""


def read_release_contract(root: Path) -> tuple[dict, dict]:
    path = root / PROFILE_DESCRIPTOR
    try:
        descriptor = json.loads(path.read_text(encoding="utf-8"))
        profiles = descriptor["profiles"]
        authority = descriptor["capability_authority"]
    except (OSError, UnicodeError, json.JSONDecodeError, KeyError, TypeError) as error:
        raise CliInstallationError(
            f"cannot read CLI artifact profile {path}: {error}"
        ) from error
    matches = [
        profile
        for profile in profiles
        if isinstance(profile, dict) and profile.get("id") == CLI_RELEASE_PROFILE
    ]
    if len(matches) != 1:
        raise CliInstallationError(
            f"expected exactly one {CLI_RELEASE_PROFILE!r} artifact profile"
        )
    if not isinstance(authority, dict):
        raise CliInstallationError("capability authority must be an object")
    if not isinstance(authority.get("schema_version"), int):
        raise CliInstallationError("capability authority schema version is invalid")
    digest = authority.get("digest")
    if not isinstance(digest, str) or re.fullmatch(r"sha256:[0-9a-f]{64}", digest) is None:
        raise CliInstallationError("capability authority digest is invalid")
    return matches[0], authority


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
        raise CliInstallationError(
            f"cannot execute {binary} {' '.join(arguments)}: {error}"
        ) from error
    if completed.returncode != 0:
        stderr = completed.stderr[:4096].decode("utf-8", errors="replace").strip()
        raise CliInstallationError(
            f"{binary.name} {' '.join(arguments)} failed with "
            f"exit code {completed.returncode}: {stderr}"
        )
    if len(completed.stdout) > RUNTIME_OUTPUT_MAX_BYTES:
        raise CliInstallationError(
            f"{binary.name} {' '.join(arguments)} exceeded its output budget"
        )
    return completed.stdout


def _require_regular_path(path: Path, description: str) -> None:
    if path.is_symlink() or not path.is_file():
        raise CliInstallationError(f"missing {description}: {path}")


def _read_regular_file(path: Path, description: str) -> bytes:
    _require_regular_path(path, description)
    try:
        with path.open("rb") as handle:
            contents = handle.read(SUPPORT_ASSET_MAX_BYTES + 1)
    except OSError as error:
        raise CliInstallationError(f"cannot read {description}: {error}") from error
    if not contents:
        raise CliInstallationError(f"{description} is empty: {path}")
    if len(contents) > SUPPORT_ASSET_MAX_BYTES:
        raise CliInstallationError(
            f"{description} exceeds {SUPPORT_ASSET_MAX_BYTES} bytes: {path}"
        )
    return contents


def _read_manpage(path: Path, name: str) -> bytes:
    contents = _read_regular_file(path, f"installed man page {name}")
    if path.suffix != ".gz":
        return contents
    try:
        with gzip.GzipFile(fileobj=io.BytesIO(contents)) as archive:
            decompressed = archive.read(SUPPORT_ASSET_MAX_BYTES + 1)
    except (EOFError, OSError) as error:
        raise CliInstallationError(
            f"cannot decompress installed man page {name}: {error}"
        ) from error
    if not decompressed:
        raise CliInstallationError(f"installed man page is empty: {path}")
    if len(decompressed) > SUPPORT_ASSET_MAX_BYTES:
        raise CliInstallationError(
            f"installed man page exceeds {SUPPORT_ASSET_MAX_BYTES} bytes: {path}"
        )
    return decompressed


def _string_set(value: object, label: str) -> set[str]:
    if not isinstance(value, list) or any(not isinstance(item, str) for item in value):
        raise CliInstallationError(f"{label} must be a string list")
    if len(value) != len(set(value)):
        raise CliInstallationError(f"{label} must not contain duplicates")
    return set(value)


def _id_set(value: object, label: str) -> set[str]:
    if not isinstance(value, list):
        raise CliInstallationError(f"{label} must be a list")
    identifiers: list[str] = []
    for item in value:
        if not isinstance(item, dict) or not isinstance(item.get("id"), str):
            raise CliInstallationError(f"{label} entries must have string ids")
        identifiers.append(item["id"])
    return _string_set(identifiers, label)


def _verify_manpage(contents: bytes, *, name: str, package_version: str) -> None:
    try:
        text = contents.decode("utf-8")
    except UnicodeError as error:
        raise CliInstallationError(
            f"installed man page is not UTF-8: {name}"
        ) from error

    headers = [line for line in text.splitlines() if line.startswith(".TH ")]
    if len(headers) != 1:
        raise CliInstallationError(
            f"installed man page must contain exactly one .TH header: {name}"
        )
    try:
        fields = shlex.split(headers[0], posix=True)
    except ValueError as error:
        raise CliInstallationError(
            f"installed man page has an invalid .TH header: {name}"
        ) from error

    expected_title = name.removesuffix(".1").upper()
    if len(fields) != 6 or fields[:3] != [".TH", expected_title, "1"]:
        raise CliInstallationError(
            f"installed man page title or section does not match {name}"
        )
    if re.fullmatch(r"[0-9]{4}-[0-9]{2}-[0-9]{2}", fields[3]) is None:
        raise CliInstallationError(f"installed man page date is invalid: {name}")
    try:
        date.fromisoformat(fields[3])
    except ValueError as error:
        raise CliInstallationError(
            f"installed man page date is invalid: {name}"
        ) from error
    if fields[4:] != [f"Merman {package_version}", "Merman CLI Manual"]:
        raise CliInstallationError(
            f"installed man page version or manual name does not match {name}"
        )
    if "\n.SH NAME\n" not in text:
        raise CliInstallationError(
            f"installed man page is missing its NAME section: {name}"
        )


def _verify_capabilities(
    payload: bytes,
    *,
    package_version: str,
    profile: dict,
    authority: dict,
) -> None:
    try:
        document = json.loads(payload.decode("utf-8"))
    except (UnicodeError, json.JSONDecodeError) as error:
        raise CliInstallationError(f"capabilities output is not JSON: {error}") from error
    if not isinstance(document, dict):
        raise CliInstallationError("capabilities output must be a JSON object")
    if document.get("schema_version") != CAPABILITIES_SCHEMA_VERSION:
        raise CliInstallationError("unexpected capabilities schema version")
    if document.get("cli_contract_version") != CLI_CONTRACT_VERSION:
        raise CliInstallationError("unexpected CLI contract version")
    package = document.get("package")
    if package != {"name": "merman-cli", "version": package_version}:
        raise CliInstallationError("capabilities package identity does not match installation")
    expected_descriptor = {
        "schema_version": authority["schema_version"],
        "digest": authority["digest"],
    }
    if document.get("descriptor") != expected_descriptor:
        raise CliInstallationError(
            "capabilities descriptor does not match the release capability authority"
        )
    compatibility = document.get("compatibility")
    if not isinstance(compatibility, dict) or any(
        not isinstance(compatibility.get(key), str) or not compatibility[key]
        for key in ("mermaid", "mmdc")
    ):
        raise CliInstallationError("capabilities compatibility metadata is incomplete")

    try:
        expected_capabilities = _string_set(
            profile["expected"]["capabilities"], "cli-release capabilities"
        )
        expected_outputs = _string_set(
            profile["expected"]["outputs"], "cli-release outputs"
        )
    except (KeyError, TypeError) as error:
        raise CliInstallationError("cli-release expected surface is incomplete") from error
    if _string_set(document.get("commands"), "installed commands") != set(COMMANDS):
        raise CliInstallationError(
            f"installed command set differs from CLI contract {CLI_CONTRACT_VERSION}"
        )
    if _id_set(document.get("capabilities"), "installed capabilities") != expected_capabilities:
        raise CliInstallationError(
            "installed capability set differs from cli-release"
        )
    if _id_set(document.get("outputs"), "installed outputs") != expected_outputs:
        raise CliInstallationError("installed output set differs from cli-release")
    if "ascii" in expected_capabilities:
        _verify_ascii_capabilities(document.get("ascii"))
    elif "ascii" in document:
        raise CliInstallationError(
            "installed capabilities expose an ASCII subcontract without ASCII support"
        )


def _verify_ascii_capabilities(value: object) -> None:
    try:
        validate_ascii_capabilities(value)
    except AsciiCapabilityContractError as error:
        raise CliInstallationError(f"installed {error}") from error


def _verify_completion_paths(
    *,
    prefix: Path,
    binary: Path,
    completion_paths: dict[str, Path],
    runner: CommandRunner,
) -> None:
    for shell, relative in completion_paths.items():
        installed = _read_regular_file(
            prefix / relative,
            f"installed {shell} completion",
        )
        generated = _run(binary, ["completion", shell], runner=runner)
        if installed != generated:
            raise CliInstallationError(
                f"installed {shell} completion differs from runtime generation"
            )


def verify_cli_installation(
    *,
    package_version: str,
    prefix: Path,
    binary: Path,
    contract_root: Path = ROOT,
    completion_layout: CompletionLayout,
    runner: CommandRunner = subprocess.run,
) -> None:
    """Verify complete-profile capabilities, completions, and man pages."""
    try:
        completion_paths = COMPLETION_LAYOUTS[completion_layout]
    except KeyError as error:
        raise CliInstallationError(
            f"unsupported completion layout: {completion_layout}"
        ) from error
    try:
        installed_prefix = prefix.resolve(strict=True)
    except OSError as error:
        raise CliInstallationError(
            f"cannot resolve CLI installation prefix {prefix}: {error}"
        ) from error
    if not installed_prefix.is_dir():
        raise CliInstallationError(f"invalid CLI installation prefix: {prefix}")
    _require_regular_path(binary, "installed merman-cli binary")

    profile, authority = read_release_contract(contract_root)
    capabilities = _run(binary, ["capabilities", "--json"], runner=runner)
    _verify_capabilities(
        capabilities,
        package_version=package_version,
        profile=profile,
        authority=authority,
    )
    _verify_completion_paths(
        prefix=installed_prefix,
        binary=binary,
        completion_paths=completion_paths,
        runner=runner,
    )
    man_root = installed_prefix / "share/man/man1"
    installed_manpages: dict[str, Path] = {}
    for pattern in ("merman-cli*.1", "merman-cli*.1.gz"):
        for path in man_root.glob(pattern):
            if not path.is_file() or path.is_symlink():
                continue
            name = path.name.removesuffix(".gz")
            if name in installed_manpages:
                raise CliInstallationError(
                    f"installed man page has compressed and uncompressed copies: {name}"
                )
            installed_manpages[name] = path
    if set(installed_manpages) != set(MANPAGE_NAMES):
        raise CliInstallationError(
            f"installed man page set differs from CLI contract {CLI_CONTRACT_VERSION}: "
            f"expected {sorted(MANPAGE_NAMES)}, got {sorted(installed_manpages)}"
        )
    for name in MANPAGE_NAMES:
        contents = _read_manpage(installed_manpages[name], name)
        _verify_manpage(contents, name=name, package_version=package_version)
        source = _read_regular_file(
            contract_root / "crates/merman-cli/assets/man" / name,
            f"release source man page {name}",
        )
        if contents != source:
            raise CliInstallationError(
                f"installed man page differs from release source asset: {name}"
            )


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package-version", required=True)
    parser.add_argument("--prefix", type=Path, required=True)
    parser.add_argument("--binary", type=Path)
    parser.add_argument(
        "--contract-root",
        type=Path,
        default=ROOT,
        help="source tree that owns the installed version's CLI contract",
    )
    parser.add_argument(
        "--completion-layout",
        choices=tuple(COMPLETION_LAYOUTS),
        required=True,
        help="package-manager integration layout to verify",
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    binary = args.binary or args.prefix / "bin/merman-cli"
    try:
        verify_cli_installation(
            package_version=args.package_version,
            prefix=args.prefix,
            binary=binary,
            contract_root=args.contract_root,
            completion_layout=args.completion_layout,
        )
    except CliInstallationError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    print(
        f"verified CLI contract {CLI_CONTRACT_VERSION}, complete-profile capabilities, "
        f"and support assets for {args.package_version}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
