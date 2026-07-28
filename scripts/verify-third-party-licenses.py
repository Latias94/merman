#!/usr/bin/env python3
"""Verify the offline third-party source and embedded-resource contract."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import sys
import tempfile
import tomllib
from collections.abc import Iterable
from pathlib import Path, PurePosixPath, PureWindowsPath
from typing import Any

try:
    from scripts.artifact_profile_recipe import (
        ArtifactProfileError,
        load_artifact_profiles,
        validate_artifact_profile_manifest,
    )
    from scripts import strict_json
except ModuleNotFoundError:
    from artifact_profile_recipe import (
        ArtifactProfileError,
        load_artifact_profiles,
        validate_artifact_profile_manifest,
    )
    import strict_json


CONTRACT_PATH = Path("docs/release/THIRD_PARTY_COMPONENTS.json")
NOTICE_PATH = Path("THIRD_PARTY_NOTICES.md")
LICENSE_ROOT = Path("THIRD_PARTY_LICENSES")
ARTIFACT_PROFILES_PATH = Path("capabilities/artifact-profiles-v1.json")
WEB_REPORT_ROOT = Path("platforms/web/legal/rust-cargo-dependencies")
WEB_REPORT_PROJECTION = Path("THIRD_PARTY_LICENSES/rust-cargo-dependencies.json")
SCHEMA_VERSION = 3
WEB_REPORT_SCHEMA_VERSION = 2
CARGO_ABOUT_VERSION = "0.9.1"
HEX_40 = re.compile(r"[0-9a-f]{40}\Z")
HEX_64 = re.compile(r"[0-9a-f]{64}\Z")
SLUG = re.compile(r"[a-z0-9]+(?:-[a-z0-9]+)*\Z")
RELATIONSHIPS = {
    "behavior-reference",
    "copied",
    "embedded",
    "fixtures",
    "generated",
    "linked",
    "modified",
    "translated",
}


class ContractError(RuntimeError):
    """A fail-closed contract violation."""


STRICT_JSON = strict_json.StrictJsonContract(
    ContractError,
    read_error_prefix="cannot read JSON",
    include_path_in_duplicate_key=False,
)
load_json = STRICT_JSON.load
require_object = STRICT_JSON.object
require_list = STRICT_JSON.array
sha256_json = strict_json.canonical_sha256


def require_exact_keys(
    value: dict[str, Any], required: set[str], optional: set[str], context: str
) -> None:
    STRICT_JSON.exact_fields(
        value,
        required,
        context,
        optional=optional,
    )


def require_string(value: Any, context: str) -> str:
    value = STRICT_JSON.string(value, context)
    if any(ord(char) < 0x20 for char in value):
        raise ContractError(f"{context} must not contain control characters")
    return value


def require_string_list(value: Any, context: str) -> list[str]:
    items = require_list(value, context)
    result = [require_string(item, f"{context}[{index}]") for index, item in enumerate(items)]
    if len(result) != len(set(result)):
        raise ContractError(f"{context} must not contain duplicates")
    return result


def require_repo_path(value: Any, context: str) -> Path:
    raw = require_string(value, context)
    pure = PurePosixPath(raw)
    if (
        pure.is_absolute()
        or bool(PureWindowsPath(raw).drive)
        or "\\" in raw
        or pure.as_posix() != raw
        or not pure.parts
        or any(part in {"", ".", ".."} for part in pure.parts)
    ):
        raise ContractError(f"{context} must be a normalized repository-relative path")
    return Path(*pure.parts)


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    try:
        with path.open("rb") as source:
            for chunk in iter(lambda: source.read(1024 * 1024), b""):
                digest.update(chunk)
    except OSError as error:
        raise ContractError(f"cannot hash {path}: {error}") from error
    return digest.hexdigest()


def require_regular_file(root: Path, relative: Path, context: str) -> Path:
    path = root / relative
    try:
        if path.is_symlink() or not path.is_file():
            raise ContractError(f"{context} must be a regular, non-symlink file: {relative}")
    except OSError as error:
        raise ContractError(f"cannot inspect {relative}: {error}") from error
    return path


def require_existing_path(root: Path, relative: Path, context: str) -> None:
    path = root / relative
    try:
        if path.is_symlink() or not path.exists():
            raise ContractError(f"{context} does not exist or is a symlink: {relative}")
    except OSError as error:
        raise ContractError(f"cannot inspect {relative}: {error}") from error


def load_repo_lock(root: Path, descriptor: dict[str, Any]) -> dict[str, Any]:
    require_exact_keys(descriptor, {"path", "schema_version"}, set(), "repository_lock")
    relative = require_repo_path(descriptor["path"], "repository_lock.path")
    expected_schema = descriptor["schema_version"]
    if expected_schema != 1:
        raise ContractError("repository_lock.schema_version must be 1")
    lock = require_object(load_json(require_regular_file(root, relative, "repository lock")), "repository lock")
    require_exact_keys(lock, {"schemaVersion", "repos"}, set(), "repository lock")
    if lock["schemaVersion"] != expected_schema:
        raise ContractError("repository lock schema does not match the contract")
    return require_object(lock["repos"], "repository lock repos")


def load_cargo_packages(root: Path, relative: Path, cache: dict[Path, list[dict[str, Any]]]) -> list[dict[str, Any]]:
    if relative in cache:
        return cache[relative]
    path = require_regular_file(root, relative, "Cargo lock")
    try:
        document = tomllib.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, tomllib.TOMLDecodeError) as error:
        raise ContractError(f"cannot read Cargo lock {relative}: {error}") from error
    packages = document.get("package")
    if not isinstance(packages, list) or not all(isinstance(item, dict) for item in packages):
        raise ContractError(f"Cargo lock {relative} has no package array")
    cache[relative] = packages
    return packages


def validate_lock(
    root: Path,
    component: dict[str, Any],
    lock: dict[str, Any],
    repo_lock: dict[str, Any],
    cargo_cache: dict[Path, list[dict[str, Any]]],
    npm_cache: dict[Path, dict[str, Any]],
    context: str,
) -> None:
    lock_type = require_string(lock.get("type"), f"{context}.type")
    source = component["source"]
    if lock_type == "repository":
        require_exact_keys(lock, {"type", "repository_id"}, set(), context)
        repository_id = require_string(lock["repository_id"], f"{context}.repository_id")
        entry = require_object(repo_lock.get(repository_id), f"repository lock entry {repository_id}")
        require_exact_keys(entry, {"path", "url", "ref", "commit"}, set(), f"repository lock entry {repository_id}")
        expected = {
            "repository": entry["url"],
            "ref": entry["ref"],
            "commit": entry["commit"],
        }
        observed = {key: source[key] for key in expected}
        if observed != expected:
            raise ContractError(f"{context} does not match repository lock entry {repository_id}")
        return
    if lock_type == "cargo":
        require_exact_keys(lock, {"type", "path", "package", "version", "checksum"}, set(), context)
        relative = require_repo_path(lock["path"], f"{context}.path")
        package = require_string(lock["package"], f"{context}.package")
        version = require_string(lock["version"], f"{context}.version")
        checksum = require_string(lock["checksum"], f"{context}.checksum")
        if not HEX_64.fullmatch(checksum):
            raise ContractError(f"{context}.checksum must be a lowercase SHA-256")
        matches = [
            item
            for item in load_cargo_packages(root, relative, cargo_cache)
            if item.get("name") == package and item.get("version") == version
        ]
        if len(matches) != 1 or matches[0].get("checksum") != checksum:
            raise ContractError(f"{context} does not match {relative}: {package}@{version}")
        return
    if lock_type == "npm":
        require_exact_keys(lock, {"type", "path", "package_path", "version", "integrity"}, set(), context)
        relative = require_repo_path(lock["path"], f"{context}.path")
        package_path = require_string(lock["package_path"], f"{context}.package_path")
        version = require_string(lock["version"], f"{context}.version")
        integrity = require_string(lock["integrity"], f"{context}.integrity")
        if relative not in npm_cache:
            document = require_object(
                load_json(require_regular_file(root, relative, "npm lock")), f"npm lock {relative}"
            )
            npm_cache[relative] = require_object(document.get("packages"), f"npm lock {relative}.packages")
        entry = require_object(npm_cache[relative].get(package_path), f"npm lock entry {package_path}")
        if entry.get("version") != version or entry.get("integrity") != integrity:
            raise ContractError(f"{context} does not match {relative}: {package_path}")
        return
    if lock_type == "pinned-source":
        require_exact_keys(lock, {"type", "evidence"}, set(), context)
        require_string(lock["evidence"], f"{context}.evidence")
        return
    raise ContractError(f"{context}.type is unsupported: {lock_type}")


def validate_external_materials(root: Path, values: Any) -> set[Path]:
    paths: set[Path] = set()
    for index, raw in enumerate(require_list(values, "externally_managed_files")):
        context = f"externally_managed_files[{index}]"
        value = require_object(raw, context)
        require_exact_keys(value, {"path", "owner", "required", "format"}, set(), context)
        relative = require_repo_path(value["path"], f"{context}.path")
        if relative in paths or relative.parent != LICENSE_ROOT:
            raise ContractError(f"{context}.path must be a unique direct child of {LICENSE_ROOT}")
        require_string(value["owner"], f"{context}.owner")
        if not isinstance(value["required"], bool):
            raise ContractError(f"{context}.required must be a boolean")
        if value["format"] != "json":
            raise ContractError(f"{context}.format must be json")
        path = root / relative
        if path.exists() or path.is_symlink():
            load_json(require_regular_file(root, relative, context))
        elif value["required"]:
            raise ContractError(f"required external material is missing: {relative}")
        paths.add(relative)
    return paths


def validate_scoped_external_materials(
    root: Path,
    values: Any,
    scopes: dict[str, dict[str, Any]],
) -> list[dict[str, Any]]:
    materials: list[dict[str, Any]] = []
    source_paths: set[Path] = set()
    mapped_scopes: set[str] = set()
    raw_values = require_list(values, "scoped_external_materials")
    for index, raw in enumerate(raw_values):
        context = f"scoped_external_materials[{index}]"
        value = require_object(raw, context)
        require_exact_keys(
            value,
            {
                "artifact_scope",
                "path",
                "projection_path",
                "owner",
                "required",
                "format",
            },
            set(),
            context,
        )
        scope_id = require_string(value["artifact_scope"], f"{context}.artifact_scope")
        if scope_id not in scopes:
            raise ContractError(f"{context} references unknown artifact scope {scope_id}")
        if scope_id in mapped_scopes:
            raise ContractError(f"{context} repeats artifact scope {scope_id}")
        relative = require_repo_path(value["path"], f"{context}.path")
        if (
            relative.parent != WEB_REPORT_ROOT
            or relative.name != f"{scope_id}.json"
            or relative in source_paths
        ):
            raise ContractError(
                f"{context}.path must uniquely bind {scope_id} under {WEB_REPORT_ROOT}"
            )
        projection = require_repo_path(
            value["projection_path"], f"{context}.projection_path"
        )
        if projection != WEB_REPORT_PROJECTION:
            raise ContractError(
                f"{context}.projection_path must be {WEB_REPORT_PROJECTION}"
            )
        require_string(value["owner"], f"{context}.owner")
        if value["required"] is not True:
            raise ContractError(f"{context}.required must be true")
        if value["format"] != "json":
            raise ContractError(f"{context}.format must be json")
        source_paths.add(relative)
        mapped_scopes.add(scope_id)
        materials.append(value)

    profile_path = root / ARTIFACT_PROFILES_PATH
    if not raw_values and not profile_path.is_file():
        return materials
    profiles = load_web_artifact_profiles(root)
    expected_scopes = set(profiles)
    if mapped_scopes != expected_scopes:
        missing = sorted(expected_scopes - mapped_scopes)
        unexpected = sorted(mapped_scopes - expected_scopes)
        details = []
        if missing:
            details.append("missing " + ", ".join(missing))
        if unexpected:
            details.append("unexpected " + ", ".join(unexpected))
        raise ContractError(
            "scoped_external_materials must exactly cover Web artifact profiles: "
            + "; ".join(details)
        )
    for material in materials:
        scope_id = material["artifact_scope"]
        report_path = require_regular_file(
            root,
            require_repo_path(material["path"], f"scoped material {scope_id} path"),
            f"scoped material {scope_id}",
        )
        validate_web_rust_report(root, load_json(report_path), profiles[scope_id], scope_id)
    return materials


def load_web_artifact_profiles(root: Path) -> dict[str, dict[str, Any]]:
    descriptor_path = require_regular_file(
        root, ARTIFACT_PROFILES_PATH, "artifact profiles"
    )
    try:
        authority_profiles = load_artifact_profiles(descriptor_path)
    except ArtifactProfileError as error:
        raise ContractError(str(error)) from error
    profiles: dict[str, dict[str, Any]] = {}
    for profile in authority_profiles:
        if profile.semantic_target != "web":
            continue
        profile_id = profile.profile_id
        if profile.cargo.default_features:
            raise ContractError(
                f"artifact profile {profile_id} must disable default features"
            )
        if not profile.cargo.features:
            raise ContractError(
                f"artifact profile {profile_id} must select at least one Cargo feature"
            )
        if (
            profile.cargo.build_target_kind != "target-set"
            or profile.cargo.build_targets != ("wasm32-unknown-unknown",)
        ):
            raise ContractError(
                f"artifact profile {profile_id} must target wasm32-unknown-unknown"
            )
        try:
            validate_artifact_profile_manifest(root, profile)
        except ArtifactProfileError as error:
            raise ContractError(str(error)) from error
        profiles[profile_id] = profile.report_projection()
    if not profiles:
        raise ContractError("artifact profile authority defines no Web profiles")
    return profiles


def validate_web_rust_report(
    root: Path,
    raw: Any,
    expected_profile: dict[str, Any],
    scope_id: str,
) -> None:
    report = require_object(raw, f"Web Rust report {scope_id}")
    require_exact_keys(
        report,
        {"schema_version", "artifact_profile", "generator", "dependency_closure", "licenses"},
        set(),
        f"Web Rust report {scope_id}",
    )
    if report["schema_version"] != WEB_REPORT_SCHEMA_VERSION:
        raise ContractError(f"Web Rust report {scope_id} has an unsupported schema")
    if report["artifact_profile"] != expected_profile:
        raise ContractError(f"Web Rust report {scope_id} does not match its artifact profile recipe")
    generator = require_object(report["generator"], f"Web Rust report {scope_id} generator")
    require_exact_keys(
        generator,
        {
            "name",
            "version",
            "command_profile",
            "offline",
            "cargo_lock_sha256",
            "configuration_sha256",
            "artifact_profile_sha256",
        },
        set(),
        f"Web Rust report {scope_id} generator",
    )
    expected_generator = {
        "name": "cargo-about",
        "version": CARGO_ABOUT_VERSION,
        "command_profile": "artifact-profile-runtime",
        "offline": True,
        "cargo_lock_sha256": sha256(root / "Cargo.lock"),
        "configuration_sha256": sha256(root / "about.toml"),
        "artifact_profile_sha256": sha256_json(expected_profile),
    }
    if generator != expected_generator:
        raise ContractError(f"Web Rust report {scope_id} generator inputs have drifted")

    licenses = require_list(report["licenses"], f"Web Rust report {scope_id} licenses")
    if not licenses:
        raise ContractError(f"Web Rust report {scope_id} has no licenses")
    packages: dict[tuple[str, str, str], dict[str, Any]] = {}
    license_order: list[tuple[str, str]] = []
    for license_index, raw_license in enumerate(licenses):
        context = f"Web Rust report {scope_id} licenses[{license_index}]"
        license_entry = require_object(raw_license, context)
        require_exact_keys(
            license_entry,
            {"id", "name", "text_sha256", "text", "packages"},
            set(),
            context,
        )
        license_id = require_string(license_entry["id"], f"{context}.id")
        require_string(license_entry["name"], f"{context}.name")
        text = license_entry["text"]
        if not isinstance(text, str) or not text:
            raise ContractError(f"{context}.text must be a non-empty string")
        text_digest = require_string(license_entry["text_sha256"], f"{context}.text_sha256")
        if text_digest != hashlib.sha256(text.encode()).hexdigest():
            raise ContractError(f"{context}.text_sha256 does not match its license text")
        license_order.append((license_id, text_digest))
        package_order: list[tuple[str, str, str]] = []
        for package_index, raw_package in enumerate(
            require_list(license_entry["packages"], f"{context}.packages")
        ):
            package_context = f"{context}.packages[{package_index}]"
            package = require_object(raw_package, package_context)
            require_exact_keys(
                package,
                {
                    "name",
                    "version",
                    "source",
                    "license_expression",
                    "authors",
                    "repository",
                },
                set(),
                package_context,
            )
            key = (
                require_string(package["name"], f"{package_context}.name"),
                require_string(package["version"], f"{package_context}.version"),
                require_string(package["source"], f"{package_context}.source"),
            )
            authors = [
                require_string(author, f"{package_context}.authors[{author_index}]")
                for author_index, author in enumerate(
                    require_list(package["authors"], f"{package_context}.authors")
                )
            ]
            if authors != sorted(authors):
                raise ContractError(f"{package_context}.authors must be sorted")
            for optional in ("license_expression", "repository"):
                if package[optional] is not None:
                    require_string(package[optional], f"{package_context}.{optional}")
            existing = packages.get(key)
            if existing is not None and existing != package:
                raise ContractError(f"{package_context} conflicts with another package record")
            packages[key] = package
            package_order.append(key)
        if (
            not package_order
            or len(package_order) != len(set(package_order))
            or package_order != sorted(package_order)
        ):
            raise ContractError(f"{context}.packages must be non-empty, unique, and sorted")
    if license_order != sorted(license_order) or len(license_order) != len(set(license_order)):
        raise ContractError(f"Web Rust report {scope_id} licenses must be unique and sorted")

    ordered_packages = [packages[key] for key in sorted(packages)]
    encoded = json.dumps(
        ordered_packages,
        ensure_ascii=True,
        separators=(",", ":"),
        sort_keys=True,
    ).encode()
    expected_closure = {
        "package_count": len(ordered_packages),
        "packages_sha256": hashlib.sha256(encoded).hexdigest(),
    }
    closure = require_object(
        report["dependency_closure"], f"Web Rust report {scope_id} dependency closure"
    )
    require_exact_keys(
        closure,
        {"package_count", "packages_sha256"},
        set(),
        f"Web Rust report {scope_id} dependency closure",
    )
    if closure != expected_closure:
        raise ContractError(f"Web Rust report {scope_id} dependency closure is incomplete or stale")


def validate_components(
    root: Path, values: Any, repo_lock: dict[str, Any]
) -> tuple[dict[str, dict[str, Any]], set[Path]]:
    components: dict[str, dict[str, Any]] = {}
    licensed_paths: set[Path] = set()
    cargo_cache: dict[Path, list[dict[str, Any]]] = {}
    npm_cache: dict[Path, dict[str, Any]] = {}
    required = {
        "id",
        "name",
        "version",
        "source",
        "relationships",
        "local_paths",
        "license_expression",
        "license_files",
        "locks",
        "notice",
    }
    for index, raw in enumerate(require_list(values, "components")):
        context = f"components[{index}]"
        component = require_object(raw, context)
        require_exact_keys(component, required, {"selected_license"}, context)
        component_id = require_string(component["id"], f"{context}.id")
        if not SLUG.fullmatch(component_id) or component_id in components:
            raise ContractError(f"{context}.id must be a unique lowercase slug")
        require_string(component["name"], f"{context}.name")
        require_string(component["version"], f"{context}.version")
        source = require_object(component["source"], f"{context}.source")
        require_exact_keys(source, {"repository", "ref", "commit", "path"}, set(), f"{context}.source")
        repository = require_string(source["repository"], f"{context}.source.repository")
        if not repository.startswith("https://"):
            raise ContractError(f"{context}.source.repository must use https")
        require_string(source["ref"], f"{context}.source.ref")
        commit = require_string(source["commit"], f"{context}.source.commit")
        if not HEX_40.fullmatch(commit):
            raise ContractError(f"{context}.source.commit must be a full lowercase Git commit")
        require_string(source["path"], f"{context}.source.path")
        relationships = require_string_list(component["relationships"], f"{context}.relationships")
        if not relationships:
            raise ContractError(f"{context}.relationships must not be empty")
        unknown = set(relationships) - RELATIONSHIPS
        if unknown:
            raise ContractError(f"{context}.relationships are unsupported: {', '.join(sorted(unknown))}")
        local_paths = require_string_list(component["local_paths"], f"{context}.local_paths")
        if not local_paths:
            raise ContractError(f"{context}.local_paths must not be empty")
        for local_index, local_path in enumerate(local_paths):
            require_existing_path(
                root,
                require_repo_path(local_path, f"{context}.local_paths[{local_index}]"),
                f"{context}.local_paths[{local_index}]",
            )
        expression = require_string(component["license_expression"], f"{context}.license_expression")
        if expression in {"UNKNOWN", "NOASSERTION"} or "SEE LICENSE" in expression:
            raise ContractError(f"{context}.license_expression is unresolved")
        selected = component.get("selected_license")
        if " OR " in expression:
            selected = require_string(selected, f"{context}.selected_license")
            if selected not in expression.replace("(", "").replace(")", "").split(" OR "):
                raise ContractError(f"{context}.selected_license is not offered by the expression")
        elif selected is not None:
            raise ContractError(f"{context}.selected_license is only valid for OR expressions")
        license_files = require_list(component["license_files"], f"{context}.license_files")
        if not license_files:
            raise ContractError(f"{context}.license_files must not be empty")
        component_paths: set[Path] = set()
        for file_index, raw_file in enumerate(license_files):
            file_context = f"{context}.license_files[{file_index}]"
            descriptor = require_object(raw_file, file_context)
            require_exact_keys(
                descriptor, {"path", "sha256", "source_url", "source_path", "role"}, set(), file_context
            )
            relative = require_repo_path(descriptor["path"], f"{file_context}.path")
            try:
                relative.relative_to(LICENSE_ROOT)
            except ValueError as error:
                raise ContractError(f"{file_context}.path must be below {LICENSE_ROOT}") from error
            if relative in component_paths:
                raise ContractError(f"{file_context}.path is duplicated in component {component_id}")
            expected_hash = require_string(descriptor["sha256"], f"{file_context}.sha256")
            if not HEX_64.fullmatch(expected_hash):
                raise ContractError(f"{file_context}.sha256 must be a lowercase SHA-256")
            source_url = require_string(descriptor["source_url"], f"{file_context}.source_url")
            if not source_url.startswith("https://"):
                raise ContractError(f"{file_context}.source_url must use https")
            require_string(descriptor["source_path"], f"{file_context}.source_path")
            if descriptor["role"] not in {"license", "notice"}:
                raise ContractError(f"{file_context}.role must be license or notice")
            path = require_regular_file(root, relative, file_context)
            observed_hash = sha256(path)
            if observed_hash != expected_hash:
                raise ContractError(
                    f"license hash mismatch for {relative}: expected {expected_hash}, got {observed_hash}"
                )
            component_paths.add(relative)
            licensed_paths.add(relative)
        locks = require_list(component["locks"], f"{context}.locks")
        if not locks:
            raise ContractError(f"{context}.locks must not be empty")
        for lock_index, raw_lock in enumerate(locks):
            lock_context = f"{context}.locks[{lock_index}]"
            validate_lock(
                root,
                component,
                require_object(raw_lock, lock_context),
                repo_lock,
                cargo_cache,
                npm_cache,
                lock_context,
            )
        require_string(component["notice"], f"{context}.notice")
        components[component_id] = component
    if not components:
        raise ContractError("components must not be empty")
    return components, licensed_paths


def validate_scopes(values: Any, components: dict[str, dict[str, Any]]) -> dict[str, dict[str, Any]]:
    scopes: dict[str, dict[str, Any]] = {}
    for index, raw in enumerate(require_list(values, "artifact_scopes")):
        context = f"artifact_scopes[{index}]"
        scope = require_object(raw, context)
        require_exact_keys(scope, {"id", "description", "extends", "components"}, set(), context)
        scope_id = require_string(scope["id"], f"{context}.id")
        if not SLUG.fullmatch(scope_id) or scope_id in scopes:
            raise ContractError(f"{context}.id must be a unique lowercase slug")
        require_string(scope["description"], f"{context}.description")
        scope["extends"] = require_string_list(scope["extends"], f"{context}.extends")
        scope["components"] = require_string_list(scope["components"], f"{context}.components")
        unknown = set(scope["components"]) - components.keys()
        if unknown:
            raise ContractError(f"{context} references unknown components: {', '.join(sorted(unknown))}")
        scopes[scope_id] = scope
    if not scopes:
        raise ContractError("artifact_scopes must not be empty")
    for scope_id, scope in scopes.items():
        unknown = set(scope["extends"]) - scopes.keys()
        if unknown or scope_id in scope["extends"]:
            raise ContractError(f"artifact scope {scope_id} has invalid parents: {', '.join(sorted(unknown))}")

    visiting: set[str] = set()
    resolved: dict[str, set[str]] = {}

    def resolve(scope_id: str) -> set[str]:
        if scope_id in resolved:
            return resolved[scope_id]
        if scope_id in visiting:
            raise ContractError(f"artifact scope inheritance cycle at {scope_id}")
        visiting.add(scope_id)
        component_ids = set(scopes[scope_id]["components"])
        for parent in scopes[scope_id]["extends"]:
            component_ids.update(resolve(parent))
        visiting.remove(scope_id)
        if not component_ids:
            raise ContractError(f"artifact scope {scope_id} resolves to no components")
        resolved[scope_id] = component_ids
        scopes[scope_id]["resolved_components"] = sorted(component_ids)
        return component_ids

    for scope_id in scopes:
        resolve(scope_id)
    used = set().union(*resolved.values())
    unused = components.keys() - used
    if unused:
        raise ContractError(f"components are not assigned to any artifact scope: {', '.join(sorted(unused))}")
    return scopes


def validate_license_directory(root: Path, licensed: set[Path], external: set[Path]) -> None:
    directory = root / LICENSE_ROOT
    if directory.is_symlink() or not directory.is_dir():
        raise ContractError(f"{LICENSE_ROOT} must be a regular directory")
    observed: set[Path] = set()
    for path in directory.rglob("*"):
        if path.is_dir():
            if path.is_symlink():
                raise ContractError(f"third-party license directory contains a symlink: {path.relative_to(root)}")
            continue
        relative = path.relative_to(root)
        if path.is_symlink() or not path.is_file():
            raise ContractError(f"third-party license material is not a regular file: {relative}")
        observed.add(relative)
    unknown = observed - licensed - external
    missing = licensed - observed
    if unknown:
        raise ContractError(f"unregistered files in {LICENSE_ROOT}: {', '.join(map(str, sorted(unknown)))}")
    if missing:
        raise ContractError(f"registered license files are missing: {', '.join(map(str, sorted(missing)))}")


def render_notice(
    contract_path: Path,
    components: dict[str, dict[str, Any]],
    scopes: dict[str, dict[str, Any]],
    external_materials: list[dict[str, Any]],
) -> str:
    reverse_scopes: dict[str, list[str]] = {component_id: [] for component_id in components}
    for scope_id, scope in scopes.items():
        for component_id in scope["resolved_components"]:
            reverse_scopes[component_id].append(scope_id)
    lines = [
        "# Third-Party Notices",
        "",
        "<!-- This file is generated. Run `python3 scripts/verify-third-party-licenses.py --write`. -->",
        "",
        "Merman is an independent, headless Rust implementation of Mermaid-compatible behavior. It",
        "is not affiliated with or endorsed by Mermaid or the projects listed below.",
        "",
        f"The machine-readable source of truth is [`{contract_path.as_posix()}`]({contract_path.as_posix()}).",
        "It records exact source revisions, local relationships, artifact scopes, and SHA-256-bound",
        "license or notice files. This inventory is engineering evidence, not legal advice.",
        "",
        "## Artifact Scopes",
        "",
    ]
    for scope_id in sorted(scopes):
        scope = scopes[scope_id]
        lines.extend(
            [
                f"### `{scope_id}`",
                "",
                scope["description"],
                "",
                "Components: " + ", ".join(f"`{item}`" for item in scope["resolved_components"]) + ".",
                "",
            ]
        )
    lines.extend(["## Components", ""])
    for component_id in sorted(components):
        component = components[component_id]
        source = component["source"]
        lines.extend(
            [
                f"### {component['name']} (`{component_id}`)",
                "",
                component["notice"],
                "",
                f"- Version: `{component['version']}`",
                f"- Source: <{source['repository']}>",
                f"- Source ref: `{source['ref']}`",
                f"- Source commit: `{source['commit']}`",
                f"- Source path: `{source['path']}`",
                "- Relationship: "
                + ", ".join(f"`{item}`" for item in sorted(component["relationships"])),
                f"- License expression: `{component['license_expression']}`",
            ]
        )
        if "selected_license" in component:
            lines.append(f"- Selected license path: `{component['selected_license']}`")
        lines.extend(
            [
                "- Artifact scopes: "
                + ", ".join(f"`{item}`" for item in sorted(reverse_scopes[component_id])),
                "- Local evidence: "
                + ", ".join(f"`{item}`" for item in sorted(component["local_paths"])),
                "- Legal files:",
            ]
        )
        for descriptor in sorted(component["license_files"], key=lambda item: item["path"]):
            path = descriptor["path"]
            lines.append(
                f"  - [`{path}`]({path}) ({descriptor['role']}, SHA-256 `{descriptor['sha256']}`)"
            )
        lines.append("")
    if external_materials:
        lines.extend(["## Additional Generated Inventories", ""])
        for material in sorted(external_materials, key=lambda item: item["path"]):
            state = "required" if material["required"] else "owned by its dedicated release gate"
            lines.append(f"- `{material['path']}`: {material['owner']} ({state}).")
        lines.append("")
    lines.extend(
        [
            "## Verification",
            "",
            "The verifier is offline and fails closed on unknown schema fields, lock drift, missing or",
            "unregistered files, SHA-256 mismatches, invalid artifact scopes, and notice drift:",
            "",
            "```bash",
            "python3 scripts/verify-third-party-licenses.py",
            "```",
            "",
        ]
    )
    return "\n".join(lines)


def load_and_validate(root: Path, contract_relative: Path = CONTRACT_PATH) -> tuple[str, Path]:
    root = root.resolve()
    contract_path = require_regular_file(root, contract_relative, "third-party contract")
    contract = require_object(load_json(contract_path), "third-party contract")
    required = {
        "schema_version",
        "generated_notice",
        "license_root",
        "repository_lock",
        "externally_managed_files",
        "scoped_external_materials",
        "artifact_scopes",
        "components",
    }
    require_exact_keys(contract, required, set(), "third-party contract")
    if contract["schema_version"] != SCHEMA_VERSION:
        raise ContractError(
            f"unsupported third-party contract schema {contract['schema_version']}; expected {SCHEMA_VERSION}"
        )
    notice_relative = require_repo_path(contract["generated_notice"], "generated_notice")
    if notice_relative != NOTICE_PATH:
        raise ContractError(f"generated_notice must be {NOTICE_PATH}")
    license_root = require_repo_path(contract["license_root"], "license_root")
    if license_root != LICENSE_ROOT:
        raise ContractError(f"license_root must be {LICENSE_ROOT}")
    repo_lock = load_repo_lock(root, require_object(contract["repository_lock"], "repository_lock"))
    external = validate_external_materials(root, contract["externally_managed_files"])
    components, licensed = validate_components(root, contract["components"], repo_lock)
    scopes = validate_scopes(contract["artifact_scopes"], components)
    validate_scoped_external_materials(
        root, contract["scoped_external_materials"], scopes
    )
    validate_license_directory(root, licensed, external)
    notice = render_notice(
        contract_relative,
        components,
        scopes,
        require_list(contract["externally_managed_files"], "externally_managed_files"),
    )
    return notice, notice_relative


def write_atomic(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    encoded = content.encode("utf-8")
    temporary_name: str | None = None
    try:
        with tempfile.NamedTemporaryFile(dir=path.parent, prefix=f".{path.name}.", delete=False) as temporary:
            temporary_name = temporary.name
            temporary.write(encoded)
            temporary.flush()
            os.fsync(temporary.fileno())
        os.replace(temporary_name, path)
        temporary_name = None
        if os.name != "nt":
            directory_fd = os.open(path.parent, os.O_RDONLY)
            try:
                os.fsync(directory_fd)
            finally:
                os.close(directory_fd)
    finally:
        if temporary_name is not None:
            try:
                os.unlink(temporary_name)
            except FileNotFoundError:
                pass


def verify_repository(root: Path, contract_relative: Path = CONTRACT_PATH, write: bool = False) -> str:
    expected, notice_relative = load_and_validate(root, contract_relative)
    notice_path = root.resolve() / notice_relative
    if write:
        write_atomic(notice_path, expected)
        return expected
    try:
        observed = notice_path.read_text(encoding="utf-8")
    except (OSError, UnicodeError) as error:
        raise ContractError(f"cannot read generated notice {notice_relative}: {error}") from error
    if observed != expected:
        raise ContractError(
            f"{notice_relative} is stale; run `python3 scripts/verify-third-party-licenses.py --write`"
        )
    return expected


def parse_args(argv: Iterable[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--write", action="store_true", help="atomically regenerate THIRD_PARTY_NOTICES.md")
    parser.add_argument("--root", type=Path, help=argparse.SUPPRESS)
    parser.add_argument("--contract", type=Path, default=CONTRACT_PATH, help=argparse.SUPPRESS)
    return parser.parse_args(argv)


def main(argv: Iterable[str] | None = None) -> int:
    args = parse_args(argv)
    root = args.root or Path(__file__).resolve().parent.parent
    try:
        verify_repository(root, args.contract, args.write)
    except ContractError as error:
        print(f"third-party license verification failed: {error}", file=sys.stderr)
        return 1
    action = "generated" if args.write else "verified"
    print(f"{action} third-party license contract: {args.contract}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
