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
    from .theme_preset_catalog_contract import validate_unqualified_catalog
    from .ascii_capability_contract import (
        AsciiCapabilityContractError,
        validate_ascii_capabilities,
    )
else:
    from theme_preset_catalog_contract import validate_unqualified_catalog
    from ascii_capability_contract import (
        AsciiCapabilityContractError,
        validate_ascii_capabilities,
    )


ROOT = Path(__file__).resolve().parents[1]
PROFILE_DESCRIPTOR = Path("capabilities/artifact-profiles-v2.json")
CLI_RELEASE_PROFILE = "cli-release"
CLI_CONTRACT_VERSION = 6
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


def _release_output_contract(output: dict[str, object]) -> dict[str, object]:
    """Project the runtime metadata emitted by ``merman-cli capabilities``.

    Native raster/PDF exporters disclose their host-font and embedded-image
    policies at runtime.  Keep the release verifiers in sync with that public
    release contract; SVG and ASCII do not own those resources and therefore emit
    explicit ``null`` values through serde.
    """

    projected = {
        "id": output["id"],
        "description": output["description"],
        "media_type": output["media_type"],
        "system_fonts": None,
        "embedded_images": None,
    }
    if output["id"] in {"jpeg", "pdf", "png"}:
        projected["system_fonts"] = {
            "source_id": "host-system",
            "discovery": "first-use",
            "cache_scope": "process-global",
            "host_dependent": True,
            "caller_configurable": True,
            "resource_bounded": False,
        }
        projected["embedded_images"] = {
            "source_ids": ["data-url"],
            "filesystem_access": False,
            "network_access": False,
            "caller_configurable": True,
            "limits": {
                "max_bytes_per_image": 16 * 1024 * 1024,
                "max_total_bytes": 32 * 1024 * 1024,
                "max_pixels_per_image": 16 * 1024 * 1024,
                "max_total_pixels": 32 * 1024 * 1024,
            },
        }
    return projected


def release_capabilities_subcontract(
    surface: dict[str, object],
    *,
    capability_ids: set[str],
) -> dict[str, object]:
    """Project the release metadata shared by archive and installation checks."""
    capabilities = [
        {
            "id": capability["id"],
            "kind": capability["kind"],
            "description": capability["description"],
            "implications": sorted(capability["implications"]),
        }
        for capability in sorted(surface["capabilities"], key=lambda value: value["id"])
        if capability["id"] in capability_ids
    ]
    outputs = [
        _release_output_contract(output)
        for output in sorted(surface["outputs"], key=lambda value: value["id"])
        if output["capability"] in capability_ids
    ]
    return {
        "capabilities": capabilities,
        "outputs": outputs,
        "native_fonts": {
            "default_policy_scope": "mermaid-graphical-input",
            "default_measurement": "deterministic",
            "default_font_source": "none",
            "default_system_discovery": "disabled",
            "system_resources_bounded": True,
            "selected_font_bytes_bounded": True,
            "legacy_export_paths": (
                "raw-svg-without-font-options-and-deterministic-mermaid-use-output-environment-contract"
            ),
            "explicit_font_sources": "closed-set-unless-font-system-on",
            "measurement_operations": [
                "measure",
                "computed-length",
                "bbox-x",
                "bbox-x-with-ascii-overhang",
                "title-bbox-x",
                "simple-bbox-width",
                "raw-bbox-width",
                "tspan-bbox-width",
                "tspan-bbox-height",
                "wrap-probe-bbox-width",
                "simple-bbox-height",
                "wrapped",
                "wrapped-with-raw-width",
                "bounding-client-rect-width",
                "create-text-bbox-y-offset",
                "mermaid-calculate-text-dimensions",
                "canvas-measure-text-width",
                "create-text-middle-bbox-y-offset",
                "raw-bbox-height",
                "raw-bbox-y-offset",
                "raw-middle-bbox-y-offset",
                "raw-hanging-bbox-y-offset",
            ],
            "measurement_phases": ["layout", "wrap", "svg-bbox", "computed-length"],
            "unsupported_operations": "deterministic-fallback",
            "missing_glyphs": "deterministic-fallback",
            "browser_pixel_parity": False,
            "system_discovery_compiled": True,
            "system_discovery_runtime_available": True,
            "embeds_fonts_in_svg": False,
            "context_scope": "operation-owned",
        },
    }


def _require_exact_json(label: str, observed: object, expected: object) -> None:
    if type(observed) is not type(expected):
        raise CliInstallationError(
            f"{label} has the wrong JSON type: "
            f"expected {type(expected).__name__}, got {type(observed).__name__}"
        )
    if isinstance(expected, dict):
        observed_object = observed
        expected_fields = set(expected)
        observed_fields = set(observed_object)
        if observed_fields != expected_fields:
            missing = sorted(expected_fields - observed_fields)
            extra = sorted(observed_fields - expected_fields)
            details = []
            if missing:
                details.append("missing fields " + ", ".join(missing))
            if extra:
                details.append("extra fields " + ", ".join(extra))
            raise CliInstallationError(f"{label} has " + "; ".join(details))
        for key, expected_value in expected.items():
            _require_exact_json(
                f"{label}.{key}",
                observed_object[key],
                expected_value,
            )
        return
    if isinstance(expected, list):
        observed_array = observed
        if len(observed_array) != len(expected):
            raise CliInstallationError(
                f"{label} has {len(observed_array)} entries; expected {len(expected)}"
            )
        for index, expected_value in enumerate(expected):
            _require_exact_json(
                f"{label}[{index}]",
                observed_array[index],
                expected_value,
            )
        return
    if observed != expected:
        raise CliInstallationError(
            f"{label} differs from the repository contract: "
            f"expected {expected!r}, got {observed!r}"
        )


def _strict_json_object(
    data: bytes,
    *,
    label: str = "capabilities output",
) -> dict[str, object]:
    try:
        text = data.decode("utf-8")
        value = json.loads(
            text,
            object_pairs_hook=lambda pairs: _json_object_without_duplicates(
                pairs,
                label=label,
            ),
        )
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise CliInstallationError(
            f"{label} is not valid UTF-8 JSON: {error}"
        ) from error
    if not isinstance(value, dict):
        raise CliInstallationError(f"{label} must be one JSON object")
    return value


def _json_object_without_duplicates(
    pairs: list[tuple[str, object]],
    *,
    label: str,
) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result:
            raise CliInstallationError(
                f"{label} contains duplicate JSON key {key!r}"
            )
        result[key] = value
    return result


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
    if descriptor.get("schema_version") != 2:
        raise CliInstallationError("artifact profiles schema_version must be 2")
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


def _publishes_theme_preset_catalog(contract_root: Path) -> bool:
    """Detect whether the version-owned CLI source publishes the catalog field.

    The Arch job verifies the stable 0.9.0 source package with the current verifier.
    That source predates the theme-preset capability field, while the current development
    contract requires it. Keep the version-owned capability surface authoritative without
    weakening the current source contract when the source marker is absent.
    """
    source = contract_root / "crates/merman-cli/src/capabilities.rs"
    if not source.is_file() or source.is_symlink():
        return True
    try:
        text = source.read_text(encoding="utf-8")
    except (OSError, UnicodeError):
        return True
    return "theme_preset_catalog_document" in text


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
    contract_root: Path,
) -> None:
    document = _strict_json_object(payload)
    schema_version = document.get("schema_version")
    if type(schema_version) is not int or schema_version != CAPABILITIES_SCHEMA_VERSION:
        raise CliInstallationError("unexpected capabilities schema version")
    contract_version = document.get("cli_contract_version")
    if type(contract_version) is not int or contract_version != CLI_CONTRACT_VERSION:
        raise CliInstallationError("unexpected CLI contract version")
    package = document.get("package")
    if package != {"name": "merman-cli", "version": package_version}:
        raise CliInstallationError("capabilities package identity does not match installation")
    expected_descriptor = {
        "schema_version": authority["schema_version"],
        "digest": authority["digest"],
    }
    try:
        _require_exact_json(
            "capabilities descriptor", document.get("descriptor"), expected_descriptor
        )
    except CliInstallationError as error:
        raise CliInstallationError(
            "capabilities descriptor does not match the release capability authority: "
            f"{error}"
        ) from error
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
        expected_diagram_families = profile["expected"]["diagram_families"]
        _string_set(expected_diagram_families, "cli-release diagram families")
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
    if document.get("diagram_families") != expected_diagram_families:
        raise CliInstallationError("installed diagram family set differs from cli-release")
    if _id_set(document.get("outputs"), "installed outputs") != expected_outputs:
        raise CliInstallationError("installed output set differs from cli-release")
    if "svg" in expected_capabilities and _publishes_theme_preset_catalog(contract_root):
        try:
            validate_unqualified_catalog(document.get("theme_presets"))
        except RuntimeError as error:
            raise CliInstallationError(f"installed {error}") from error
    elif "theme_presets" in document and _publishes_theme_preset_catalog(contract_root):
        raise CliInstallationError("installed capabilities expose a theme preset catalog without SVG support")
    surface = _strict_json_object(
        _read_regular_file(
            contract_root / "capabilities/feature-surface-v1.json", "release capability surface"
        ),
        label="release capability surface",
    )
    _require_exact_json(
        "release capability surface schema version",
        surface.get("schema_version"),
        authority["schema_version"],
    )
    try:
        expected_subcontract = release_capabilities_subcontract(
            surface, capability_ids=expected_capabilities
        )
    except (KeyError, TypeError) as error:
        raise CliInstallationError("release capability surface is incomplete") from error
    for field, expected in expected_subcontract.items():
        _require_exact_json(f"installed {field}", document.get(field), expected)
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
        contract_root=contract_root,
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
