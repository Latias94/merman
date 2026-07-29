#!/usr/bin/env python3
"""Verify that CLI installers resolve the canonical cargo-dist artifacts."""

from __future__ import annotations

from dataclasses import dataclass
import json
from pathlib import Path, PurePosixPath
import re
import sys
import tomllib
from urllib.parse import urlparse

from verify_cli_release_archive import (
    release_archive_name,
    release_binary_archive_path,
)


ROOT = Path(__file__).resolve().parents[1]
CLI_MANIFEST = Path("crates/merman-cli/Cargo.toml")
DIST_CONFIG = Path("dist-workspace.toml")
ARTIFACT_PROFILES = Path("capabilities/artifact-profiles-v1.json")
CLI_RELEASE_PROFILE = "cli-release"

_ARCHIVE_SUFFIXES = {"txz": ".tar.xz", "zip": ".zip"}
_TEMPLATE_VARIABLE = re.compile(r"\{\s*([a-z][a-z0-9-]*)\s*\}")


class InstallationContractError(RuntimeError):
    """Raised when installation metadata drifts from release artifacts."""


@dataclass(frozen=True)
class ResolvedBinstallArtifact:
    target: str
    package_format: str
    url: str
    archive_name: str
    binary_path: str


def _read_toml(root: Path, relative: Path) -> dict:
    path = root / relative
    try:
        with path.open("rb") as handle:
            document = tomllib.load(handle)
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise InstallationContractError(f"cannot read {relative}: {error}") from error
    if not isinstance(document, dict):
        raise InstallationContractError(f"{relative} must contain a TOML table")
    return document


def _read_json(root: Path, relative: Path) -> dict:
    path = root / relative
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise InstallationContractError(f"cannot read {relative}: {error}") from error
    if not isinstance(document, dict):
        raise InstallationContractError(f"{relative} must contain a JSON object")
    return document


def _require_string(value: object, label: str) -> str:
    if not isinstance(value, str) or not value:
        raise InstallationContractError(f"{label} must be a non-empty string")
    return value


def _require_string_list(value: object, label: str) -> list[str]:
    if not isinstance(value, list) or not value:
        raise InstallationContractError(f"{label} must be a non-empty string list")
    if any(not isinstance(item, str) or not item for item in value):
        raise InstallationContractError(f"{label} must contain only non-empty strings")
    if len(value) != len(set(value)):
        raise InstallationContractError(f"{label} must not contain duplicates")
    return value


def _render_template(template: object, values: dict[str, str], label: str) -> str:
    source = _require_string(template, label)
    used: set[str] = set()

    def replace(match: re.Match[str]) -> str:
        variable = match.group(1)
        if variable not in values:
            raise InstallationContractError(
                f"{label} uses unsupported template variable {variable!r}"
            )
        used.add(variable)
        return values[variable]

    rendered = _TEMPLATE_VARIABLE.sub(replace, source)
    if "{" in rendered or "}" in rendered:
        raise InstallationContractError(f"{label} contains malformed template syntax")
    if not used:
        raise InstallationContractError(f"{label} must be a cargo-binstall template")
    return rendered


def _cli_release_profile(root: Path) -> dict:
    descriptor = _read_json(root, ARTIFACT_PROFILES)
    profiles = descriptor.get("profiles")
    if not isinstance(profiles, list):
        raise InstallationContractError("artifact profile descriptor has no profiles list")
    matches = [
        profile
        for profile in profiles
        if isinstance(profile, dict) and profile.get("id") == CLI_RELEASE_PROFILE
    ]
    if len(matches) != 1:
        raise InstallationContractError(
            f"expected exactly one {CLI_RELEASE_PROFILE!r} artifact profile"
        )
    return matches[0]


def resolve_binstall_artifact(
    target: str,
    *,
    root: Path = ROOT,
    version: str | None = None,
) -> ResolvedBinstallArtifact:
    workspace = _read_toml(root, Path("Cargo.toml"))
    manifest = _read_toml(root, CLI_MANIFEST)
    try:
        workspace_package = workspace["workspace"]["package"]
        package = manifest["package"]
        metadata = package["metadata"]["binstall"]
    except (KeyError, TypeError) as error:
        raise InstallationContractError("CLI cargo-binstall metadata is incomplete") from error

    if not isinstance(metadata, dict):
        raise InstallationContractError("package.metadata.binstall must be a table")
    overrides = metadata.get("overrides", {})
    if not isinstance(overrides, dict):
        raise InstallationContractError("cargo-binstall overrides must be a table")
    override = overrides.get(target, {})
    if not isinstance(override, dict):
        raise InstallationContractError(f"cargo-binstall override for {target} must be a table")

    package_name = _require_string(package.get("name"), "CLI package name")
    repository = _require_string(
        workspace_package.get("repository"), "workspace repository"
    ).rstrip("/")
    package_version = version or _require_string(
        workspace_package.get("version"), "workspace version"
    )
    package_format = _require_string(
        override.get("pkg-fmt", metadata.get("pkg-fmt")),
        f"cargo-binstall pkg-fmt for {target}",
    )
    try:
        archive_suffix = _ARCHIVE_SUFFIXES[package_format]
    except KeyError as error:
        raise InstallationContractError(
            f"unsupported cargo-binstall package format {package_format!r}"
        ) from error

    values = {
        "repo": repository,
        "version": package_version,
        "name": package_name,
        "bin": package_name,
        "target": target,
        "archive-suffix": archive_suffix,
        "binary-ext": ".exe" if "windows" in target.split("-") else "",
    }
    url = _render_template(
        override.get("pkg-url", metadata.get("pkg-url")),
        values,
        f"cargo-binstall pkg-url for {target}",
    )
    binary_path = _render_template(
        override.get("bin-dir", metadata.get("bin-dir")),
        values,
        f"cargo-binstall bin-dir for {target}",
    )
    parsed = urlparse(url)
    if parsed.scheme != "https" or not parsed.netloc or parsed.query or parsed.fragment:
        raise InstallationContractError(
            f"cargo-binstall URL for {target} must be an immutable HTTPS URL"
        )
    archive_name = PurePosixPath(parsed.path).name
    return ResolvedBinstallArtifact(
        target=target,
        package_format=package_format,
        url=url,
        archive_name=archive_name,
        binary_path=binary_path,
    )


def validate_repository_contract(root: Path = ROOT) -> list[ResolvedBinstallArtifact]:
    workspace = _read_toml(root, Path("Cargo.toml"))
    manifest = _read_toml(root, CLI_MANIFEST)
    dist_config = _read_toml(root, DIST_CONFIG)
    profile = _cli_release_profile(root)
    try:
        workspace_package = workspace["workspace"]["package"]
        package = manifest["package"]
        metadata = package["metadata"]
        binstall = metadata["binstall"]
        dist = metadata["dist"]
        cargo_profile = profile["cargo"]
        profile_target = cargo_profile["build_target"]
    except (KeyError, TypeError) as error:
        raise InstallationContractError("CLI installation contract is incomplete") from error

    package_name = _require_string(package.get("name"), "CLI package name")
    version = _require_string(workspace_package.get("version"), "workspace version")
    repository = _require_string(
        workspace_package.get("repository"), "workspace repository"
    ).rstrip("/")
    if dist.get("default-features") is not False:
        raise InstallationContractError("cargo-dist must disable CLI default features")
    if cargo_profile.get("default_features") is not False:
        raise InstallationContractError("cli-release must disable CLI default features")
    if cargo_profile.get("profile") != "dist":
        raise InstallationContractError(
            "cli-release must use Cargo's dist profile used by cargo-dist"
        )

    default_features = _require_string_list(
        manifest.get("features", {}).get("default"), "CLI default features"
    )
    dist_features = _require_string_list(dist.get("features"), "cargo-dist CLI features")
    profile_features = _require_string_list(
        cargo_profile.get("features"), "cli-release features"
    )
    feature_sets = {
        tuple(sorted(values))
        for values in (default_features, dist_features, profile_features)
    }
    if len(feature_sets) != 1:
        raise InstallationContractError(
            "CLI defaults, cargo-dist, and cli-release must select the same complete feature set"
        )

    dist_table = dist_config.get("dist", {})
    if not isinstance(dist_table, dict):
        raise InstallationContractError("cargo-dist configuration must be a table")
    if dist_table.get("unix-archive") != ".tar.xz":
        raise InstallationContractError("cargo-dist Unix archive must be .tar.xz")
    if dist_table.get("windows-archive") != ".zip":
        raise InstallationContractError("cargo-dist Windows archive must be .zip")
    dist_targets = _require_string_list(dist_table.get("targets"), "cargo-dist targets")
    if profile_target.get("kind") != "target-set":
        raise InstallationContractError("cli-release must use a target-set build target")
    profile_targets = _require_string_list(
        profile_target.get("triples"), "cli-release targets"
    )
    if set(dist_targets) != set(profile_targets):
        raise InstallationContractError(
            "cargo-dist and cli-release must advertise the same target set"
        )

    disabled_strategies = binstall.get("disabled-strategies", [])
    if not isinstance(disabled_strategies, list) or any(
        not isinstance(item, str) for item in disabled_strategies
    ):
        raise InstallationContractError(
            "cargo-binstall disabled-strategies must be a string list"
        )
    if disabled_strategies != ["quick-install"]:
        raise InstallationContractError(
            "cargo-binstall must disable third-party quick-install artifacts only"
        )
    if "compile" in disabled_strategies:
        raise InstallationContractError(
            "cargo-binstall must preserve its cargo-install source fallback"
        )

    overrides = binstall.get("overrides", {})
    if not isinstance(overrides, dict) or not set(overrides).issubset(dist_targets):
        raise InstallationContractError(
            "cargo-binstall overrides must refer only to published targets"
        )

    resolved: list[ResolvedBinstallArtifact] = []
    for target in profile_targets:
        artifact = resolve_binstall_artifact(target, root=root, version=version)
        expected_archive = release_archive_name(target)
        expected_url = f"{repository}/releases/download/v{version}/{expected_archive}"
        expected_format = "zip" if expected_archive.endswith(".zip") else "txz"
        expected_binary_path = release_binary_archive_path(target)
        if artifact.url != expected_url:
            raise InstallationContractError(
                f"cargo-binstall URL for {target} must resolve to {expected_url}"
            )
        if artifact.archive_name != expected_archive:
            raise InstallationContractError(
                f"cargo-binstall archive for {target} must be {expected_archive}"
            )
        if artifact.package_format != expected_format:
            raise InstallationContractError(
                f"cargo-binstall format for {target} must be {expected_format}"
            )
        if artifact.binary_path != expected_binary_path:
            raise InstallationContractError(
                f"cargo-binstall binary for {target} must be {expected_binary_path}"
            )
        resolved.append(artifact)

    if package_name != "merman-cli":
        raise InstallationContractError("CLI installation contract requires package merman-cli")
    return resolved


def main() -> int:
    try:
        artifacts = validate_repository_contract()
    except InstallationContractError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    for artifact in artifacts:
        print(
            f"{artifact.target}: {artifact.package_format} "
            f"{artifact.archive_name} -> {artifact.binary_path}"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
