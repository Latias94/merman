#!/usr/bin/env python3
"""Report declared and observed release surface status."""

from __future__ import annotations

import argparse
from concurrent.futures import ThreadPoolExecutor
import fnmatch
import json
import os
import re
import shutil
import subprocess
import sys
import tomllib
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path
from typing import Any

try:
    from scripts.release_version import ReleaseVersion, parse_release_version
except ModuleNotFoundError:
    from release_version import ReleaseVersion, parse_release_version


ROOT = Path(__file__).resolve().parents[1]
SURFACES_PATH = ROOT / "docs" / "release" / "SURFACES.json"
STATE_ORDER = {
    "published": 0,
    "artifact-only": 1,
    "manual-registry": 2,
    "credential-blocked": 3,
    "registry-blocked": 4,
    "not-built": 5,
    "not-applicable": 6,
}
RELEASE_KINDS = {"stable", "prerelease"}
NPM_DIST_TAGS = {
    "stable": "latest",
    "alpha": "alpha",
    "beta": "beta",
    "rc": "rc",
}
PACKAGE_KINDS = {"android", "crate", "flutter", "npm", "python", "swiftpm", "typst", "vscode"}
CHANNEL_KINDS = {
    "crates.io",
    "github-actions-artifact",
    "github-release-assets",
    "homebrew",
    "maven-central",
    "npm",
    "pub.dev",
    "pypi",
    "scoop",
    "swiftpm",
    "typst-registry",
    "vs-marketplace",
    "winget",
}
PROTECTED_PUBLICATION_KINDS = {
    "crates.io",
    "github-release-assets",
    "npm",
    "pypi",
    "pub.dev",
}
ENVIRONMENT_IDENTIFIER_RE = re.compile(r"[A-Za-z0-9][A-Za-z0-9._-]*")
TOP_LEVEL_KEYS = {"schema_version", "states", "release_kinds", "feature_contract", "surfaces"}
FEATURE_CONTRACT_KEYS = {
    "web_descriptor",
    "web_package_group_surface",
}
SURFACE_KEYS = {
    "id",
    "name",
    "audience",
    "public",
    "entry_point",
    "install",
    "support_level",
    "dependency_weight",
    "capabilities",
    "docs",
    "packages",
    "channels",
    "gates",
    "public_channel",
}
PACKAGE_KEYS = {"kind", "name", "manifest", "version_source"}
CHANNEL_KEYS = {
    "id",
    "kind",
    "declared_state",
    "release_kinds",
    "workflow",
    "workflow_job",
    "environment",
    "credential",
    "blocker",
    "not_applicable_reason",
    "asset_patterns",
    "artifact_patterns",
    "dist_tags",
}
PATTERN_KEYS = {"glob", "min_matches", "max_matches"}


class SurfaceError(Exception):
    pass


def load_contract(path: Path = SURFACES_PATH) -> dict[str, Any]:
    try:
        data = json.loads(
            path.read_text(encoding="utf-8"),
            object_pairs_hook=reject_duplicate_json_keys,
        )
    except SurfaceError:
        raise
    except (OSError, UnicodeError, json.JSONDecodeError) as exc:
        raise SurfaceError(f"invalid JSON in {path}: {exc}") from exc
    validate_contract(data)
    return data


def reject_duplicate_json_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise SurfaceError(f"duplicate JSON object key {key!r}")
        result[key] = value
    return result


def validate_contract(data: dict[str, Any]) -> None:
    if not isinstance(data, dict):
        raise SurfaceError("SURFACES.json root must be an object")
    reject_unknown_keys(data, TOP_LEVEL_KEYS, "SURFACES.json")
    if data.get("schema_version") != 2:
        raise SurfaceError("SURFACES.json schema_version must be 2")
    states = require_string_set(data, "states", "SURFACES.json")
    if states != set(STATE_ORDER):
        raise SurfaceError("SURFACES.json states must exactly match the supported state catalog")
    release_kinds = require_string_set(data, "release_kinds", "SURFACES.json")
    if release_kinds != RELEASE_KINDS:
        raise SurfaceError("SURFACES.json release_kinds must be exactly stable and prerelease")

    feature_contract = data.get("feature_contract")
    if not isinstance(feature_contract, dict):
        raise SurfaceError("SURFACES.json feature_contract must be an object")
    reject_unknown_keys(feature_contract, FEATURE_CONTRACT_KEYS, "feature_contract")
    require_str(feature_contract, "web_descriptor", "feature_contract")
    require_str(feature_contract, "web_package_group_surface", "feature_contract")

    surfaces = require_array(data, "surfaces", "SURFACES.json", allow_empty=False)
    seen_ids: set[str] = set()
    for surface in surfaces:
        if not isinstance(surface, dict):
            raise SurfaceError("SURFACES.json surfaces must contain only objects")
        surface_id = require_str(surface, "id", "surface")
        if surface_id in seen_ids:
            raise SurfaceError(f"duplicate surface id: {surface_id}")
        seen_ids.add(surface_id)
        reject_unknown_keys(surface, SURFACE_KEYS, surface_id)
        require_str(surface, "name", surface_id)
        require_str(surface, "audience", surface_id)
        if not isinstance(surface.get("public"), bool):
            raise SurfaceError(f"{surface_id}: public must be a boolean")
        require_str(surface, "entry_point", surface_id)
        require_str(surface, "install", surface_id)
        require_str(surface, "support_level", surface_id)
        require_str(surface, "dependency_weight", surface_id)
        require_string_list(surface, "capabilities", surface_id)
        require_string_list(surface, "docs", surface_id)
        require_string_list(surface, "gates", surface_id)

        packages = require_array(surface, "packages", surface_id, allow_empty=True)
        for package in packages:
            validate_package(package, surface_id)

        channels = require_array(surface, "channels", surface_id, allow_empty=False)
        seen_channel_ids: set[str] = set()
        for channel in channels:
            if not isinstance(channel, dict):
                raise SurfaceError(f"{surface_id}: channels must contain only objects")
            channel_id = require_str(channel, "id", surface_id)
            if channel_id in seen_channel_ids:
                raise SurfaceError(f"{surface_id}: duplicate channel id {channel_id!r}")
            seen_channel_ids.add(channel_id)
            owner = f"{surface_id}/{channel_id}"
            reject_unknown_keys(channel, CHANNEL_KEYS, owner)
            kind = require_str(channel, "kind", owner)
            if kind not in CHANNEL_KINDS:
                raise SurfaceError(f"{owner}: unsupported channel kind {kind!r}")
            state = require_str(channel, "declared_state", f"{surface_id}/{channel_id}")
            if state not in states:
                raise SurfaceError(f"{surface_id}/{channel_id}: unknown declared_state {state!r}")
            channel_release_kinds = require_string_set(channel, "release_kinds", owner)
            if not channel_release_kinds <= RELEASE_KINDS:
                raise SurfaceError(f"{owner}: release_kinds contains unsupported values")
            if (
                channel_release_kinds != RELEASE_KINDS
                or state == "not-applicable"
            ) and (
                not isinstance(channel.get("not_applicable_reason"), str)
                or not channel["not_applicable_reason"].strip()
            ):
                raise SurfaceError(
                    f"{owner}: conditionally not-applicable channels require not_applicable_reason"
                )
            require_str(channel, "workflow", owner)
            if state in {"published", "artifact-only"}:
                require_str(channel, "workflow_job", owner)
            elif channel.get("workflow_job") is not None:
                require_str(channel, "workflow_job", owner)
            environment = channel.get("environment")
            if kind in PROTECTED_PUBLICATION_KINDS and state in {"published", "artifact-only"}:
                environment = require_str(channel, "environment", owner)
            if environment is not None:
                if not isinstance(environment, str) or not ENVIRONMENT_IDENTIFIER_RE.fullmatch(environment):
                    raise SurfaceError(f"{owner}: environment must be a literal identifier")
            credential = channel.get("credential")
            if credential is not None and (not isinstance(credential, str) or not credential.strip()):
                raise SurfaceError(f"{owner}: credential must be null or a non-empty string")
            if state == "credential-blocked" and credential is None:
                raise SurfaceError(
                    f"{owner}: credential-blocked channels must name the missing credential"
                )
            blocker = channel.get("blocker")
            if blocker is not None and (
                not isinstance(blocker, str) or not blocker.strip()
            ):
                raise SurfaceError(f"{owner}: blocker must be a non-empty string")
            if state in {
                "credential-blocked",
                "registry-blocked",
                "manual-registry",
            } and blocker is None:
                raise SurfaceError(f"{owner}: {state} channels must explain the blocker")
            validate_probe_contract(channel, kind, owner)

        public_channel = surface.get("public_channel")
        if surface["public"]:
            if not isinstance(public_channel, str) or not public_channel.strip():
                raise SurfaceError(f"{surface_id}: public surfaces require public_channel")
            if public_channel not in seen_channel_ids:
                raise SurfaceError(
                    f"{surface_id}: public_channel references unknown channel {public_channel!r}"
                )
        elif public_channel is not None:
            raise SurfaceError(f"{surface_id}: non-public surfaces must not declare public_channel")


def require_str(item: dict[str, Any], key: str, owner: str) -> str:
    value = item.get(key)
    if not isinstance(value, str) or not value.strip():
        raise SurfaceError(f"{owner}: missing string field {key}")
    return value


def require_array(item: dict[str, Any], key: str, owner: str, *, allow_empty: bool) -> list[Any]:
    value = item.get(key)
    if not isinstance(value, list) or (not allow_empty and not value):
        qualifier = "list" if allow_empty else "non-empty list"
        raise SurfaceError(f"{owner}: missing {qualifier} field {key}")
    return value


def require_string_list(item: dict[str, Any], key: str, owner: str) -> list[str]:
    values = require_array(item, key, owner, allow_empty=False)
    if not all(isinstance(value, str) and value.strip() for value in values):
        raise SurfaceError(f"{owner}: {key} must contain only non-empty strings")
    if len(values) != len(set(values)):
        raise SurfaceError(f"{owner}: {key} must not contain duplicates")
    return values


def require_string_set(item: dict[str, Any], key: str, owner: str) -> set[str]:
    return set(require_string_list(item, key, owner))


def reject_unknown_keys(item: dict[str, Any], allowed: set[str], owner: str) -> None:
    unknown = set(item) - allowed
    if unknown:
        raise SurfaceError(f"{owner}: unknown fields: {', '.join(sorted(unknown))}")


def validate_package(package: Any, surface_id: str) -> None:
    if not isinstance(package, dict):
        raise SurfaceError(f"{surface_id}: packages must contain only objects")
    name = require_str(package, "name", surface_id)
    owner = f"{surface_id}/{name}"
    reject_unknown_keys(package, PACKAGE_KEYS, owner)
    kind = require_str(package, "kind", owner)
    if kind not in PACKAGE_KINDS:
        raise SurfaceError(f"{owner}: unsupported package kind {kind!r}")
    require_str(package, "manifest", owner)
    version_source = package.get("version_source", "target")
    if version_source not in {"target", "manifest"}:
        raise SurfaceError(f"{owner}: version_source must be target or manifest")


def validate_probe_contract(channel: dict[str, Any], kind: str, owner: str) -> None:
    if kind == "npm":
        dist_tags = channel.get("dist_tags")
        if dist_tags != NPM_DIST_TAGS:
            raise SurfaceError(
                f"{owner}: dist_tags must exactly match the canonical stable/alpha/beta/rc mapping"
            )
    required_pattern_field = {
        "github-release-assets": "asset_patterns",
        "github-actions-artifact": "artifact_patterns",
    }.get(kind)
    if required_pattern_field is not None and required_pattern_field not in channel:
        raise SurfaceError(f"{owner}: {kind} requires {required_pattern_field}")
    for field in ["asset_patterns", "artifact_patterns"]:
        if field not in channel:
            continue
        patterns = require_array(channel, field, owner, allow_empty=False)
        for index, pattern in enumerate(patterns):
            pattern_owner = f"{owner}/{field}[{index}]"
            if not isinstance(pattern, dict):
                raise SurfaceError(f"{pattern_owner}: expected an object")
            reject_unknown_keys(pattern, PATTERN_KEYS, pattern_owner)
            glob = require_str(pattern, "glob", pattern_owner)
            placeholders = set(re.findall(r"\{([^{}]+)\}", glob))
            unknown_placeholders = placeholders - {
                "version",
                "python_version",
                "package_version",
                "tag",
                "channel",
                "source_sha",
            }
            if unknown_placeholders:
                raise SurfaceError(
                    f"{pattern_owner}: unsupported placeholders: "
                    + ", ".join(sorted(unknown_placeholders))
                )
            minimum = pattern.get("min_matches")
            maximum = pattern.get("max_matches")
            if not isinstance(minimum, int) or isinstance(minimum, bool) or minimum < 0:
                raise SurfaceError(f"{pattern_owner}: min_matches must be a non-negative integer")
            if not isinstance(maximum, int) or isinstance(maximum, bool) or maximum < minimum:
                raise SurfaceError(f"{pattern_owner}: max_matches must be an integer >= min_matches")
            if kind in {"github-release-assets", "github-actions-artifact"} and minimum < 1:
                raise SurfaceError(
                    f"{pattern_owner}: release artifact patterns must require at least one asset"
                )
    if kind == "github-actions-artifact":
        patterns = channel["artifact_patterns"]
        if not all(
            ("{version}" in pattern["glob"] or "{tag}" in pattern["glob"])
            and "{channel}" in pattern["glob"]
            and pattern["glob"].count("{source_sha}") == 1
            for pattern in patterns
        ):
            raise SurfaceError(
                f"{owner}: GitHub Actions artifacts must bind target version, release channel, and source SHA"
            )


def release_kind(version: str | None) -> str | None:
    if not version:
        return None
    return parse_release_version(version).kind


def effective_declared_state(channel: dict[str, Any], version: str | None) -> str:
    kind = release_kind(version)
    if kind and kind not in channel.get("release_kinds", []):
        return "not-applicable"
    return channel["declared_state"]


def summarize_surface_state(surface: dict[str, Any], version: str | None) -> str:
    public_channel = surface.get("public_channel")
    if isinstance(public_channel, str):
        return effective_declared_state(
            next(channel for channel in surface["channels"] if channel["id"] == public_channel),
            version,
        )
    states = [effective_declared_state(channel, version) for channel in surface["channels"]]
    return min(states, key=lambda state: STATE_ORDER[state])


def channel_probe(channel: dict[str, Any], surface: dict[str, Any], version: str) -> dict[str, str]:
    target = parse_release_version(version)
    kind = channel.get("kind")
    if kind == "npm":
        packages = package_records(surface, "npm")
        if packages:
            return probe_many(
                [(package["name"], package_registry_version(package, target)) for package in packages],
                lambda package, package_version: probe_npm(package, package_version, channel["dist_tags"]),
            )
    if kind == "pub.dev":
        package = first_package_name(surface, "flutter")
        if package:
            return probe_pub_dev(package, target.canonical)
    if kind == "pypi":
        package = first_package_name(surface, "python")
        if package:
            return probe_pypi(package, target.to_pep440())
    if kind == "crates.io":
        packages = package_records(surface, "crate")
        if packages:
            targets = [
                (package["name"], package_registry_version(package, target)) for package in packages
            ]
            return probe_many(
                targets,
                lambda package, package_version: probe_crates_io(package, package_version),
            )
    if kind == "github-release-assets":
        return probe_github_release(channel, target)
    if kind == "github-actions-artifact":
        return probe_github_actions_artifacts(
            channel,
            target,
            package_version=artifact_package_version(surface, target),
        )
    return {"state": "unknown", "reason": f"no probe implemented for {kind}"}


def first_package_name(surface: dict[str, Any], kind: str) -> str | None:
    names = package_names(surface, kind)
    return names[0] if names else None


def package_names(surface: dict[str, Any], kind: str) -> list[str]:
    return [package["name"] for package in package_records(surface, kind)]


def package_records(surface: dict[str, Any], kind: str) -> list[dict[str, Any]]:
    return [package for package in surface.get("packages", []) if package.get("kind") == kind]


def package_registry_version(package: dict[str, Any], target: ReleaseVersion) -> str:
    if package.get("version_source", "target") == "target":
        return target.canonical

    manifest = (ROOT / package["manifest"]).resolve()
    try:
        manifest.relative_to(ROOT.resolve())
    except ValueError as exc:
        raise SurfaceError(f"{package['name']}: manifest escapes the repository") from exc
    try:
        if manifest.suffix == ".json":
            package_version = json.loads(manifest.read_text(encoding="utf-8"))["version"]
        else:
            with manifest.open("rb") as handle:
                package_version = tomllib.load(handle)["package"]["version"]
    except (OSError, KeyError, json.JSONDecodeError, tomllib.TOMLDecodeError) as exc:
        raise SurfaceError(f"{package['name']}: cannot read package version from {package['manifest']}") from exc
    if not isinstance(package_version, str):
        raise SurfaceError(f"{package['name']}: manifest version is not a string")
    return package_version


def artifact_package_version(surface: dict[str, Any], target: ReleaseVersion) -> str:
    manifest_owned = [
        package
        for package in surface.get("packages", [])
        if package.get("version_source", "target") == "manifest"
    ]
    if not manifest_owned:
        return target.canonical
    versions = {package_registry_version(package, target) for package in manifest_owned}
    if len(versions) != 1:
        raise SurfaceError(
            f"{surface['id']}: independently versioned artifact packages disagree: "
            + ", ".join(sorted(versions))
        )
    return versions.pop()


def probe_many(packages: list[tuple[str, str]], probe: Any) -> dict[str, str]:
    if len(packages) < 2:
        results = [(package, version, probe(package, version)) for package, version in packages]
    else:
        with ThreadPoolExecutor(max_workers=min(4, len(packages))) as executor:
            probe_results = executor.map(
                lambda item: probe(item[0], item[1]),
                packages,
            )
            results = [
                (package, version, result)
                for (package, version), result in zip(packages, probe_results, strict=True)
            ]
    invalid = [
        (package, result)
        for package, _version, result in results
        if result.get("state") not in {"found", "missing", "unknown"}
    ]
    if invalid:
        return {
            "state": "unknown",
            "reason": "; ".join(f"{package}: invalid probe response" for package, _result in invalid),
        }
    missing = [
        (package, result) for package, _version, result in results if result["state"] == "missing"
    ]
    unknown = [
        (package, result) for package, _version, result in results if result["state"] == "unknown"
    ]
    if missing:
        return {
            "state": "missing",
            "reason": "; ".join(f"{package}: {result['reason']}" for package, result in missing),
        }
    if unknown:
        return {
            "state": "unknown",
            "reason": "; ".join(f"{package}: {result['reason']}" for package, result in unknown),
        }
    return {
        "state": "found",
        "reason": "all package versions exist: "
        + ", ".join(f"{package}@{version}" for package, version, _result in results),
    }


def python_version(version: str) -> str:
    return parse_release_version(version).to_pep440()


def probe_npm(
    package: str,
    version: ReleaseVersion | str,
    dist_tags: dict[str, str],
) -> dict[str, str]:
    target = parse_release_version(version) if isinstance(version, str) else version
    encoded_package = urllib.parse.quote(package, safe="")
    url = f"https://registry.npmjs.org/{encoded_package}"
    request = urllib.request.Request(url, headers={"User-Agent": "merman-release-status"})
    try:
        with urllib.request.urlopen(request, timeout=10) as response:
            data = json.load(response)
    except urllib.error.HTTPError as exc:
        if exc.code == 404:
            return {"state": "missing", "reason": "npm package version not found"}
        return {"state": "unknown", "reason": f"npm HTTP {exc.code}"}
    except (OSError, UnicodeError, json.JSONDecodeError) as exc:
        return {"state": "unknown", "reason": str(exc)}
    if not isinstance(data, dict):
        return {"state": "unknown", "reason": "npm response is not an object"}
    versions = data.get("versions")
    if not isinstance(versions, dict):
        return {"state": "unknown", "reason": "npm response did not contain a versions object"}
    if target.canonical not in versions:
        return {"state": "missing", "reason": f"npm version {target.canonical} not found"}
    expected_dist_tag = dist_tags[target.channel]
    observed_dist_tags = data.get("dist-tags")
    if not isinstance(observed_dist_tags, dict):
        return {"state": "unknown", "reason": "npm response did not contain dist-tags"}
    observed_version = observed_dist_tags.get(expected_dist_tag)
    if observed_version is not None and not isinstance(observed_version, str):
        return {
            "state": "unknown",
            "reason": f"npm dist-tag {expected_dist_tag!r} has an invalid version value",
        }
    if observed_version != target.canonical:
        return {
            "state": "missing",
            "reason": (
                f"npm dist-tag {expected_dist_tag!r} points to {observed_version!r}, "
                f"expected {target.canonical!r}"
            ),
        }
    return {
        "state": "found",
        "reason": f"npm version exists and dist-tag {expected_dist_tag!r} points to it",
    }


def probe_pub_dev(package: str, version: str) -> dict[str, str]:
    encoded_package = urllib.parse.quote(package, safe="")
    url = f"https://pub.dev/api/packages/{encoded_package}"
    request = urllib.request.Request(url, headers={"User-Agent": "merman-release-status"})
    try:
        with urllib.request.urlopen(request, timeout=10) as response:
            data = json.load(response)
    except urllib.error.HTTPError as exc:
        if exc.code == 404:
            return {"state": "missing", "reason": "pub.dev package not found"}
        return {"state": "unknown", "reason": f"pub.dev HTTP {exc.code}"}
    except (OSError, UnicodeError, json.JSONDecodeError) as exc:
        return {"state": "unknown", "reason": str(exc)}
    if not isinstance(data, dict):
        return {"state": "unknown", "reason": "pub.dev response is not an object"}
    versions = data.get("versions")
    if not isinstance(versions, list) or not all(isinstance(item, dict) for item in versions):
        return {"state": "unknown", "reason": "pub.dev response did not contain a valid versions list"}
    if any(item.get("version") == version for item in versions):
        return {"state": "found", "reason": "pub.dev package version exists"}
    return {"state": "missing", "reason": "pub.dev package version not found"}


def probe_pypi(package: str, version: str) -> dict[str, str]:
    url = f"https://pypi.org/pypi/{package}/{version}/json"
    try:
        with urllib.request.urlopen(url, timeout=10) as response:
            if response.status == 200:
                return {"state": "found", "reason": "PyPI package version exists"}
    except urllib.error.HTTPError as exc:
        if exc.code == 404:
            return {"state": "missing", "reason": "PyPI package version not found"}
        return {"state": "unknown", "reason": f"PyPI HTTP {exc.code}"}
    except OSError as exc:
        return {"state": "unknown", "reason": str(exc)}
    return {"state": "unknown", "reason": "PyPI response did not confirm version"}


def probe_crates_io(package: str, version: str) -> dict[str, str]:
    url = f"https://crates.io/api/v1/crates/{package}/{version}"
    request = urllib.request.Request(url, headers={"User-Agent": "merman-release-status"})
    try:
        with urllib.request.urlopen(request, timeout=10) as response:
            if response.status == 200:
                return {"state": "found", "reason": "crates.io package version exists"}
    except urllib.error.HTTPError as exc:
        if exc.code == 404:
            return {"state": "missing", "reason": "crates.io package version not found"}
        return {"state": "unknown", "reason": f"crates.io HTTP {exc.code}"}
    except OSError as exc:
        return {"state": "unknown", "reason": str(exc)}
    return {"state": "unknown", "reason": "crates.io response did not confirm version"}


def probe_github_release(
    channel: dict[str, Any],
    version: ReleaseVersion | str,
) -> dict[str, str]:
    target = parse_release_version(version) if isinstance(version, str) else version
    gh = shutil.which("gh")
    if not gh:
        return {"state": "unknown", "reason": "gh not found"}
    try:
        result = subprocess.run(
            [
                gh,
                "release",
                "view",
                target.tag,
                "--json",
                "tagName,isDraft,isPrerelease,assets",
            ],
            cwd=ROOT,
            text=True,
            capture_output=True,
            timeout=20,
            check=False,
        )
    except subprocess.TimeoutExpired:
        return {"state": "unknown", "reason": "gh release view timed out"}
    except OSError as exc:
        return {"state": "unknown", "reason": f"could not run gh: {exc}"}

    if result.returncode != 0:
        reason = stderr_or_stdout(result)
        if github_release_not_found(reason):
            return {"state": "missing", "reason": f"GitHub Release not found: {reason}"}
        return {"state": "unknown", "reason": f"gh release view failed: {reason}"}

    try:
        data = json.loads(result.stdout)
    except (TypeError, json.JSONDecodeError) as exc:
        return {"state": "unknown", "reason": f"gh returned invalid release JSON: {exc}"}
    if not isinstance(data, dict):
        return {"state": "unknown", "reason": "gh release response is not an object"}
    if data.get("tagName") != target.tag:
        return {"state": "unknown", "reason": "GitHub response did not confirm the target tag"}
    if not isinstance(data.get("isDraft"), bool) or not isinstance(data.get("isPrerelease"), bool):
        return {"state": "unknown", "reason": "GitHub response omitted release marker fields"}
    if data["isDraft"]:
        return {"state": "missing", "reason": "GitHub Release is still a draft"}
    expected_prerelease = target.kind == "prerelease"
    if data["isPrerelease"] is not expected_prerelease:
        expected = "prerelease" if expected_prerelease else "stable"
        return {"state": "missing", "reason": f"GitHub Release is not marked as {expected}"}

    assets = data.get("assets")
    if not isinstance(assets, list) or not all(isinstance(asset, dict) for asset in assets):
        return {"state": "unknown", "reason": "GitHub response did not contain an asset list"}
    names: list[str] = []
    expected_assets: list[tuple[str, dict[str, Any]]] = []
    expanded_patterns = [expand_pattern(pattern["glob"], target) for pattern in channel["asset_patterns"]]
    for asset in assets:
        name = asset.get("name")
        if not isinstance(name, str):
            return {"state": "unknown", "reason": "GitHub asset omitted its name"}
        names.append(name)
        if any(fnmatch.fnmatchcase(name, glob) for glob in expanded_patterns):
            expected_assets.append((name, asset))
    for name, asset in expected_assets:
        state = asset.get("state")
        size = asset.get("size")
        if not isinstance(state, str) or not isinstance(size, int) or isinstance(size, bool):
            return {
                "state": "unknown",
                "reason": f"GitHub asset {name!r} omitted uploaded state and size metadata",
            }
        if state != "uploaded":
            return {"state": "missing", "reason": f"GitHub asset {name!r} is not uploaded (state={state!r})"}
        if size <= 0:
            return {"state": "missing", "reason": f"GitHub asset {name!r} is empty"}
    pattern_result = probe_name_patterns(
        names,
        channel["asset_patterns"],
        target,
        noun="GitHub Release asset",
    )
    if pattern_result is not None:
        return pattern_result
    return {
        "state": "found",
        "reason": f"non-draft GitHub Release has all {len(channel['asset_patterns'])} asset groups",
    }


def probe_github_actions_artifacts(
    channel: dict[str, Any],
    version: ReleaseVersion | str,
    *,
    package_version: str | None = None,
) -> dict[str, str]:
    target = parse_release_version(version) if isinstance(version, str) else version
    package_version = package_version or target.canonical
    patterns = channel["artifact_patterns"]
    if not all(
        ("{version}" in pattern["glob"] or "{tag}" in pattern["glob"])
        and "{channel}" in pattern["glob"]
        and pattern["glob"].count("{source_sha}") == 1
        for pattern in patterns
    ):
        return {
            "state": "unknown",
            "reason": (
                "artifact contract does not bind every artifact name to the target release "
                "version, channel, and source SHA"
            ),
        }

    gh = shutil.which("gh")
    if not gh:
        return {"state": "unknown", "reason": "gh not found"}
    repository = github_repository()
    if repository is None:
        return {"state": "unknown", "reason": "could not determine the GitHub repository"}

    source_sha, source_failure = resolve_github_tag_sha(gh, repository, target.tag)
    if source_failure is not None:
        return source_failure

    workflow = Path(channel["workflow"]).name
    encoded_workflow = urllib.parse.quote(workflow, safe="")
    encoded_source_sha = urllib.parse.quote(source_sha, safe="")
    runs, failure = run_gh_paginated_json(
        gh,
        (
            f"repos/{repository}/actions/workflows/{encoded_workflow}/runs"
            f"?status=success&head_sha={encoded_source_sha}&per_page=100"
        ),
        collection="workflow_runs",
    )
    if failure is not None:
        return failure
    run_ids: list[int] = []
    malformed_successful_run = False
    for run in runs:
        if not isinstance(run, dict):
            malformed_successful_run = True
            continue
        if run.get("conclusion") != "success" or run.get("status") != "completed":
            malformed_successful_run = True
            continue
        if not isinstance(run.get("id"), int) or isinstance(run.get("id"), bool):
            malformed_successful_run = True
            continue
        event = run.get("event")
        head_sha = run.get("head_sha")
        if not isinstance(event, str) or not isinstance(head_sha, str) or not head_sha:
            malformed_successful_run = True
            continue
        if event == "pull_request" or event not in {"push", "workflow_dispatch"}:
            continue
        if head_sha.lower() != source_sha:
            continue
        run_ids.append(run["id"])
    if not run_ids:
        if malformed_successful_run:
            return {
                "state": "unknown",
                "reason": f"successful {workflow} run metadata omitted event or head SHA",
            }
        return {"state": "missing", "reason": f"no successful runs found for {workflow}"}

    legacy_provenance = False
    malformed_target_artifact = False
    for run_id in run_ids:
        artifacts, failure = run_gh_paginated_json(
            gh,
            f"repos/{repository}/actions/runs/{run_id}/artifacts?per_page=100",
            collection="artifacts",
        )
        if failure is not None:
            return failure
        artifacts_for_run: list[dict[str, Any]] = []
        malformed_artifact = False
        for artifact in artifacts:
            if not isinstance(artifact, dict):
                malformed_artifact = True
                continue
            if artifact.get("expired") is True:
                continue
            if (
                not isinstance(artifact.get("name"), str)
                or artifact.get("expired") is not False
            ):
                malformed_artifact = True
                continue
            artifacts_for_run.append(artifact)
        all_names = [artifact["name"] for artifact in artifacts_for_run]
        valid_names: list[str] = []
        invalid_names: list[str] = []
        for artifact in artifacts_for_run:
            size = artifact.get("size_in_bytes")
            if not isinstance(size, int) or isinstance(size, bool):
                invalid_names.append(artifact["name"])
                continue
            if size <= 0:
                invalid_names.append(artifact["name"])
                continue
            valid_names.append(artifact["name"])
        pattern_result = probe_name_patterns(
            valid_names,
            patterns,
            target,
            source_sha=source_sha,
            package_version=package_version,
            noun="GitHub Actions artifact",
        )
        if pattern_result is None:
            return {
                "state": "found",
                "reason": (
                    f"successful {workflow} run {run_id} has all {len(patterns)} "
                    "version-bound artifact groups"
                ),
            }
        legacy_patterns = [
            {
                **pattern,
                "glob": legacy_artifact_pattern(pattern["glob"]),
            }
            for pattern in patterns
        ]
        if probe_name_patterns(
            all_names,
            legacy_patterns,
            target,
            package_version=package_version,
            noun="GitHub Actions artifact",
        ) is None:
            legacy_provenance = True
        expanded_target_patterns = [
            expand_pattern(
                pattern["glob"],
                target,
                source_sha=source_sha,
                package_version=package_version,
            )
            for pattern in patterns
        ]
        if any(
            any(fnmatch.fnmatchcase(name, glob) for glob in expanded_target_patterns)
            for name in invalid_names
        ):
            malformed_target_artifact = True
        if malformed_artifact:
            return {
                "state": "unknown",
                "reason": "GitHub Actions artifact response contained malformed metadata",
            }
    if legacy_provenance:
        return {
            "state": "unknown",
            "reason": "target artifact set exists but has no source provenance",
        }
    if malformed_target_artifact:
        return {
            "state": "unknown",
            "reason": "target artifacts omitted a positive size_in_bytes",
        }
    if malformed_successful_run:
        return {
            "state": "unknown",
            "reason": f"successful {workflow} run response contained malformed metadata",
        }
    return {
        "state": "missing",
        "reason": f"no single successful {workflow} run has the complete target artifact set",
    }


def resolve_github_tag_sha(
    gh: str,
    repository: str,
    tag: str,
) -> tuple[str | None, dict[str, str] | None]:
    encoded_tag = urllib.parse.quote(tag, safe="")
    try:
        result = subprocess.run(
            [gh, "api", f"repos/{repository}/commits/{encoded_tag}", "--jq", ".sha"],
            cwd=ROOT,
            text=True,
            capture_output=True,
            timeout=20,
            check=False,
        )
    except subprocess.TimeoutExpired:
        return None, {"state": "unknown", "reason": "gh api timed out while resolving target tag SHA"}
    except OSError as exc:
        return None, {"state": "unknown", "reason": f"could not run gh: {exc}"}
    if result.returncode != 0:
        reason = stderr_or_stdout(result)
        if github_release_not_found(reason):
            return None, {"state": "missing", "reason": f"target tag {tag!r} was not found: {reason}"}
        return None, {"state": "unknown", "reason": f"could not resolve target tag SHA: {reason}"}
    sha = result.stdout.strip()
    if not re.fullmatch(r"[0-9a-fA-F]{40}", sha):
        return None, {"state": "unknown", "reason": "GitHub target tag response did not contain a commit SHA"}
    return sha.lower(), None


def run_gh_paginated_json(
    gh: str,
    endpoint: str,
    *,
    collection: str,
) -> tuple[list[Any], dict[str, str] | None]:
    try:
        result = subprocess.run(
            [gh, "api", "--paginate", "--slurp", endpoint],
            cwd=ROOT,
            text=True,
            capture_output=True,
            timeout=20,
            check=False,
        )
    except subprocess.TimeoutExpired:
        return [], {"state": "unknown", "reason": f"gh api timed out while reading {collection}"}
    except OSError as exc:
        return [], {"state": "unknown", "reason": f"could not run gh: {exc}"}

    if result.returncode != 0:
        return [], {
            "state": "unknown",
            "reason": f"gh api failed while reading {collection}: {stderr_or_stdout(result)}",
        }
    try:
        pages = json.loads(result.stdout)
    except (TypeError, json.JSONDecodeError) as exc:
        return [], {"state": "unknown", "reason": f"gh returned invalid {collection} JSON: {exc}"}
    if not isinstance(pages, list):
        return [], {"state": "unknown", "reason": f"gh {collection} response is not a page list"}

    items: list[Any] = []
    for page in pages:
        if not isinstance(page, dict) or not isinstance(page.get(collection), list):
            return [], {"state": "unknown", "reason": f"gh {collection} page has an invalid shape"}
        items.extend(page[collection])
    return items, None


def github_repository() -> str | None:
    configured = os.environ.get("GH_REPO") or os.environ.get("GITHUB_REPOSITORY")
    if configured:
        return configured if valid_github_repository(configured) else None
    try:
        with (ROOT / "Cargo.toml").open("rb") as handle:
            repository_url = tomllib.load(handle)["workspace"]["package"]["repository"]
    except (FileNotFoundError, KeyError, tomllib.TOMLDecodeError):
        return None
    if not isinstance(repository_url, str):
        return None
    match = re_github_repository(repository_url)
    return match if match and valid_github_repository(match) else None


def re_github_repository(repository_url: str) -> str | None:
    parsed = urllib.parse.urlparse(repository_url)
    if parsed.hostname == "github.com":
        return parsed.path.strip("/").removesuffix(".git")
    if repository_url.startswith("git@github.com:"):
        return repository_url.removeprefix("git@github.com:").removesuffix(".git")
    return None


def valid_github_repository(repository: str) -> bool:
    parts = repository.split("/")
    return len(parts) == 2 and all(part and part not in {".", ".."} for part in parts)


def probe_name_patterns(
    names: list[str],
    patterns: list[dict[str, Any]],
    version: ReleaseVersion,
    source_sha: str | None = None,
    package_version: str | None = None,
    *,
    noun: str,
) -> dict[str, str] | None:
    failures: list[str] = []
    for pattern in patterns:
        glob = expand_pattern(
            pattern["glob"],
            version,
            source_sha=source_sha,
            package_version=package_version,
        )
        count = sum(fnmatch.fnmatchcase(name, glob) for name in names)
        minimum = pattern["min_matches"]
        maximum = pattern["max_matches"]
        if count < minimum or count > maximum:
            failures.append(f"{glob!r} matched {count}, expected {minimum}..{maximum}")
    if failures:
        return {"state": "missing", "reason": f"{noun} contract failed: " + "; ".join(failures)}
    return None


def expand_pattern(
    pattern: str,
    version: ReleaseVersion,
    *,
    source_sha: str | None = None,
    package_version: str | None = None,
) -> str:
    expanded = (
        pattern.replace("{version}", version.canonical)
        .replace("{python_version}", version.to_pep440())
        .replace("{package_version}", package_version or version.canonical)
        .replace("{tag}", version.tag)
        .replace("{channel}", version.channel)
    )
    if "{source_sha}" in expanded:
        if source_sha is None:
            return expanded
        expanded = expanded.replace("{source_sha}", source_sha)
    return expanded


def legacy_artifact_pattern(pattern: str) -> str:
    for token in ("-{source_sha}", "{source_sha}-", "{source_sha}"):
        if token in pattern:
            return pattern.replace(token, "", 1)
    return pattern


def github_release_not_found(reason: str) -> bool:
    lowered = reason.lower()
    return "http 404" in lowered or "release not found" in lowered


def stderr_or_stdout(result: subprocess.CompletedProcess[str]) -> str:
    output = result.stderr or result.stdout or ""
    if not isinstance(output, str):
        return "command failed"
    lines = output.strip().splitlines()
    return lines[0] if lines else "command failed without diagnostic output"


def build_rows(data: dict[str, Any], *, version: str | None, probe: bool) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for surface in data["surfaces"]:
        surface_state = summarize_surface_state(surface, version)
        channel_rows: list[dict[str, Any]] = []
        for channel in surface["channels"]:
            channel_state = effective_declared_state(channel, version)
            row: dict[str, Any] = {
                "id": channel["id"],
                "kind": channel["kind"],
                "declared_state": channel_state,
                "workflow": channel.get("workflow"),
                "environment": channel.get("environment"),
                "credential": channel.get("credential"),
                "blocker": channel.get("blocker"),
            }
            if probe:
                if channel_state == "not-applicable":
                    row["observed_status"] = {
                        "state": "not-applicable",
                        "reason": channel.get("not_applicable_reason", "channel does not apply"),
                    }
                else:
                    row["observed_status"] = channel_probe(channel, surface, version or "")
            channel_rows.append(row)
        rows.append(
            {
                "id": surface["id"],
                "name": surface["name"],
                "audience": surface["audience"],
                "public": surface["public"],
                "entry_point": surface["entry_point"],
                "support_level": surface["support_level"],
                "dependency_weight": surface["dependency_weight"],
                "capabilities": surface["capabilities"],
                "declared_state": surface_state,
                "availability_channel": surface.get("public_channel"),
                "install": surface["install"],
                "docs": surface["docs"],
                "gates": surface["gates"],
                "channels": channel_rows,
            }
        )
    return rows


def render_public(rows: list[dict[str, Any]]) -> str:
    lines = [
        "Surface | Entry point | Install | Support | Availability | Weight | Capabilities",
        "--- | --- | --- | --- | --- | --- | ---",
    ]
    for row in rows:
        if not row["public"]:
            continue
        lines.append(
            " | ".join(
                [
                    row["name"],
                    f"`{row['entry_point']}`",
                    row["install"],
                    row["support_level"],
                    row["declared_state"],
                    row["dependency_weight"],
                    ", ".join(row["capabilities"]),
                ]
            )
        )
    return "\n".join(lines)


def render_maintainer(rows: list[dict[str, Any]]) -> str:
    lines = [
        "Surface | Channel | State | Workflow | Environment | Credential | Blocker | Observation",
        "--- | --- | --- | --- | --- | --- | --- | ---",
    ]
    for row in rows:
        for channel in row["channels"]:
            observed = channel.get("observed_status")
            state = channel["declared_state"]
            if observed:
                state = f"{state} ({observed['state']})"
            lines.append(
                " | ".join(
                    [
                        row["id"],
                        channel["id"],
                        state,
                        channel.get("workflow") or "",
                        channel.get("environment") or "",
                        channel.get("credential") or "",
                        channel.get("blocker") or "",
                        observed.get("reason", "") if observed else "",
                    ]
                )
            )
    return "\n".join(lines)


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--contract", type=Path, default=SURFACES_PATH)
    parser.add_argument("--version", help="Target release version, for example 0.8.0-alpha.4")
    parser.add_argument("--probe", action="store_true", help="Best-effort network/tool probes for the target version.")
    parser.add_argument("--view", choices=["maintainer", "public"], default="maintainer")
    parser.add_argument("--format", choices=["table", "json"], default="table")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv or sys.argv[1:])
    if args.probe and not args.version:
        print("error: --probe requires --version", file=sys.stderr)
        return 2
    try:
        target = parse_release_version(args.version) if args.version else None
        version = target.canonical if target else None
        data = load_contract(args.contract)
        rows = build_rows(data, version=version, probe=args.probe)
    except (SurfaceError, ValueError) as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1

    if args.format == "json":
        payload = {
            "schema_version": data["schema_version"],
            "version": version,
            "release_kind": target.kind if target else None,
            "view": args.view,
            "surfaces": public_projection(rows) if args.view == "public" else rows,
        }
        print(json.dumps(payload, indent=2, sort_keys=True))
        return 0

    if args.view == "public":
        print(render_public(rows))
    else:
        print(render_maintainer(rows))
    return 0


def public_projection(rows: list[dict[str, Any]]) -> list[dict[str, Any]]:
    public_keys = {
        "id",
        "name",
        "audience",
        "entry_point",
        "install",
        "support_level",
        "dependency_weight",
        "capabilities",
        "declared_state",
        "availability_channel",
    }
    return [
        {key: value for key, value in row.items() if key in public_keys}
        for row in rows
        if row["public"]
    ]


if __name__ == "__main__":
    raise SystemExit(main())
