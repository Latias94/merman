#!/usr/bin/env python3
"""Build, verify, and reconcile the lockstep browser npm package group.

The Web workspace owns package layout. This helper intentionally owns only the
release artifact boundary: it reads the workspace's closed package descriptor,
packs public packages, verifies their tarballs, and reconciles npm publication
without assuming that a group publish is transactional.
"""

from __future__ import annotations

import argparse
import base64
import functools
import hashlib
import json
import os
import re
import subprocess
import sys
import tarfile
from dataclasses import dataclass, field
from pathlib import Path, PurePosixPath
from typing import Any, Protocol


GROUP_MANIFEST_SCHEMA_VERSION = 2
PROVENANCE_SCHEMA_VERSION = 2
DEFAULT_DESCRIPTOR = Path("platforms/web/web-surface-descriptor.json")
DEFAULT_DESCRIPTOR_SCHEMA = (
    Path(__file__).resolve().parents[1]
    / "platforms"
    / "web"
    / "web-surface-descriptor.schema.json"
)
DEFAULT_MANIFEST_NAME = "web-package-group.json"
PACKAGE_ID_RE = re.compile(r"[a-z0-9][a-z0-9-]*\Z")
SHA256_RE = re.compile(r"sha256:[0-9a-f]{64}\Z")
INTEGRITY_RE = re.compile(r"sha512-[A-Za-z0-9+/]+={0,2}\Z")
SOURCE_SHA_RE = re.compile(r"[0-9a-f]{7,64}\Z")
NPM_DIST_TAG_RE = re.compile(r"[a-z][a-z0-9-]*\Z")
NPMJS_REGISTRY_URL = "https://registry.npmjs.org"
REQUIRED_TARBALL_FILES = {
    "package/package.json",
    "package/README.md",
    "package/LICENSE",
    "package/THIRD_PARTY_NOTICES.md",
    "package/artifacts/provenance.json",
}
WASM_RUNTIME_TARBALL_FILES = {
    "package/artifacts/wasm/merman_wasm.js",
    "package/artifacts/wasm/merman_wasm.d.ts",
    "package/artifacts/wasm/merman_wasm_bg.wasm",
    "package/artifacts/wasm/merman_wasm_bg.wasm.d.ts",
}
REQUIRED_TARBALL_FILES |= WASM_RUNTIME_TARBALL_FILES
PACKAGE_TARBALL_FIXED_FILES = {
    "package/package.json",
    "package/README.md",
    "package/LICENSE",
    "package/THIRD_PARTY_NOTICES.md",
    "package/artifacts/provenance.json",
}
EXPECTED_PACKAGE_FILE_ROOTS = {
    "artifacts",
    "dist",
    "LICENSE",
    "README.md",
    "THIRD_PARTY_LICENSES",
    "THIRD_PARTY_NOTICES.md",
}
WASM_MEMBER = "package/artifacts/wasm/merman_wasm_bg.wasm"
ARTIFACT_TARBALL_PREFIX = "package/artifacts/"
WASM_TARBALL_PREFIX = "package/artifacts/wasm/"
LEGAL_TARBALL_PREFIX = "package/THIRD_PARTY_LICENSES/"
LEGACY_TARBALL_PREFIX = "package/pkg/"
# The package artifact crosses from an unprivileged build job into the npm OIDC
# publisher. These ceilings protect the verifier itself without constraining the
# current package group (the largest current WASM payload is under 10 MiB).
MAX_TARBALL_PACKED_BYTES = 64 * 1024 * 1024
MAX_TARBALL_UNPACKED_BYTES = 128 * 1024 * 1024
MAX_METADATA_MEMBER_BYTES = 1024 * 1024
MAX_LEGAL_MEMBER_BYTES = 8 * 1024 * 1024
FULL_PACKAGE_ID = "full"
COMPLETE_SVG_PACKAGE_ID = "render"
MIN_PUBLIC_SLIM_UNPACKED_SIZE_SAVINGS_PERCENT = 15


class PackageGroupError(ValueError):
    """The package group descriptor, artifact, or registry state is invalid."""


class ReconciliationError(PackageGroupError):
    """A registry mutation failed after a reportable reconciliation state existed."""

    def __init__(self, message: str, report: dict[str, Any]) -> None:
        super().__init__(message)
        self.report = report


def reject_duplicate_json_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise PackageGroupError(f"duplicate JSON object key {key!r}")
        result[key] = value
    return result


def load_json(path: Path) -> dict[str, Any]:
    try:
        data = json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=reject_duplicate_json_keys)
    except OSError as exc:
        raise PackageGroupError(f"cannot read {path}: {exc}") from exc
    except (json.JSONDecodeError, PackageGroupError) as exc:
        raise PackageGroupError(f"invalid JSON in {path}: {exc}") from exc
    if not isinstance(data, dict):
        raise PackageGroupError(f"{path}: expected a JSON object")
    return data


def require_exact_keys(value: dict[str, Any], expected: set[str], owner: str) -> None:
    actual = set(value)
    if actual != expected:
        missing = sorted(expected - actual)
        extra = sorted(actual - expected)
        details = []
        if missing:
            details.append("missing " + ", ".join(missing))
        if extra:
            details.append("unknown " + ", ".join(extra))
        raise PackageGroupError(f"{owner}: fields must be exact ({'; '.join(details)})")


def require_string(value: dict[str, Any], key: str, owner: str) -> str:
    item = value.get(key)
    if not isinstance(item, str) or not item.strip():
        raise PackageGroupError(f"{owner}: {key} must be a non-empty string")
    return item


def descriptor_package_path(entry: dict[str, Any]) -> Path:
    package_dir = require_string(entry, "package_dir", "Web package")
    path = PurePosixPath(package_dir)
    if path.is_absolute() or ".." in path.parts or path.parts[:1] != ("packages",) or len(path.parts) != 2:
        raise PackageGroupError(
            f"Web package {entry.get('id')!r}: package_dir must be packages/<id>"
        )
    return Path(*path.parts)


def load_descriptor_schema(path: Path = DEFAULT_DESCRIPTOR_SCHEMA) -> dict[str, Any]:
    schema = load_json(path)
    _validate_descriptor_schema(schema)
    return schema


@functools.cache
def descriptor_package_name_pattern() -> re.Pattern[str]:
    schema = load_descriptor_schema()
    pattern = schema["$defs"]["package"]["properties"]["name"].get("pattern")
    if not isinstance(pattern, str):
        raise PackageGroupError("Web package descriptor schema package name must define a pattern")
    try:
        return re.compile(pattern)
    except re.error as exc:
        raise PackageGroupError(f"Web package descriptor schema package name pattern is invalid: {exc}") from exc


def validate_descriptor(
    data: dict[str, Any], *, schema: dict[str, Any] | None = None
) -> dict[str, Any]:
    schema = load_descriptor_schema() if schema is None else schema
    _validate_descriptor_schema(schema)
    _validate_json_schema(data, schema, schema, "Web package descriptor")
    _validate_descriptor_invariants(data, schema["x-merman-invariants"])
    return data


def _validate_descriptor_schema(schema: dict[str, Any]) -> None:
    expected_root = {
        "$schema",
        "$id",
        "title",
        "type",
        "additionalProperties",
        "required",
        "properties",
        "$defs",
        "x-merman-invariants",
    }
    require_exact_keys(schema, expected_root, "Web package descriptor schema")
    if schema["$schema"] != "https://json-schema.org/draft/2020-12/schema":
        raise PackageGroupError("Web package descriptor schema must use JSON Schema draft 2020-12")
    if schema["type"] != "object":
        raise PackageGroupError("Web package descriptor schema root must be an object")
    _validate_schema_node(schema, "Web package descriptor schema", root=True)
    invariants = schema["x-merman-invariants"]
    if not isinstance(invariants, dict):
        raise PackageGroupError("Web package descriptor schema invariants must be an object")
    require_exact_keys(
        invariants,
        {"uniqueBy", "derivedFields", "conditionalPackages", "defaultPackage"},
        "Web package descriptor schema invariants",
    )
    if not isinstance(invariants["uniqueBy"], list) or not all(
        isinstance(field, str) for field in invariants["uniqueBy"]
    ):
        raise PackageGroupError("Web package descriptor schema uniqueBy must be a string array")
    if not isinstance(invariants["derivedFields"], dict) or not all(
        isinstance(field, str) and isinstance(template, str)
        for field, template in invariants["derivedFields"].items()
    ):
        raise PackageGroupError("Web package descriptor schema derivedFields must be string templates")
    conditional_rules = invariants["conditionalPackages"]
    if not isinstance(conditional_rules, list):
        raise PackageGroupError("Web package descriptor schema conditionalPackages must be an array")
    for index, rule in enumerate(conditional_rules):
        if not isinstance(rule, dict):
            raise PackageGroupError(f"Web package descriptor conditionalPackages[{index}] must be an object")
        require_exact_keys(
            rule,
            {"when", "allowed"},
            f"Web package descriptor conditionalPackages[{index}]",
        )
        if not isinstance(rule["when"], dict) or not rule["when"]:
            raise PackageGroupError(f"Web package descriptor conditionalPackages[{index}].when must be an object")
        if not isinstance(rule["allowed"], list) or not all(
            isinstance(candidate, dict) and candidate for candidate in rule["allowed"]
        ):
            raise PackageGroupError(
                f"Web package descriptor conditionalPackages[{index}].allowed "
                "must be an object array"
            )
    default_rule = invariants["defaultPackage"]
    if not isinstance(default_rule, dict):
        raise PackageGroupError("Web package descriptor schema defaultPackage must be an object")
    require_exact_keys(
        default_rule,
        {"referenceField", "targetField", "requiredFields"},
        "Web package descriptor schema defaultPackage",
    )
    if (
        not isinstance(default_rule["referenceField"], str)
        or not isinstance(default_rule["targetField"], str)
        or not isinstance(default_rule["requiredFields"], dict)
    ):
        raise PackageGroupError("Web package descriptor schema defaultPackage is invalid")


def _validate_schema_node(
    schema: dict[str, Any], owner: str, *, root: bool = False
) -> None:
    supported = {
        "$ref",
        "type",
        "const",
        "enum",
        "pattern",
        "additionalProperties",
        "required",
        "properties",
        "minItems",
        "items",
    }
    ignored_root = {"$schema", "$id", "title", "$defs", "x-merman-invariants"}
    unknown = set(schema) - supported - (ignored_root if root else set())
    if unknown:
        raise PackageGroupError(f"{owner}: unsupported schema keywords: {', '.join(sorted(unknown))}")
    properties = schema.get("properties")
    if properties is not None:
        if not isinstance(properties, dict):
            raise PackageGroupError(f"{owner}: properties must be an object")
        for field, child in properties.items():
            if not isinstance(field, str) or not isinstance(child, dict):
                raise PackageGroupError(f"{owner}: property schemas must be objects")
            _validate_schema_node(child, f"{owner}.properties.{field}")
    items = schema.get("items")
    if items is not None:
        if not isinstance(items, dict):
            raise PackageGroupError(f"{owner}: items must be an object")
        _validate_schema_node(items, f"{owner}.items")
    definitions = schema.get("$defs", {})
    if not isinstance(definitions, dict):
        raise PackageGroupError(f"{owner}: $defs must be an object")
    for name, child in definitions.items():
        if not isinstance(child, dict):
            raise PackageGroupError(f"{owner}: definition {name} must be an object")
        _validate_schema_node(child, f"{owner}.$defs.{name}")


def _validate_json_schema(
    value: Any, schema: dict[str, Any], root: dict[str, Any], owner: str
) -> None:
    reference = schema.get("$ref")
    if reference is not None:
        prefix = "#/$defs/"
        if not isinstance(reference, str) or not reference.startswith(prefix):
            raise PackageGroupError(f"{owner}: unsupported schema reference {reference!r}")
        name = reference.removeprefix(prefix)
        definition = root["$defs"].get(name)
        if not isinstance(definition, dict):
            raise PackageGroupError(f"{owner}: unknown schema reference {reference}")
        _validate_json_schema(value, definition, root, owner)
        return

    if "const" in schema and not _json_equal(value, schema["const"]):
        raise PackageGroupError(f"{owner} must be {schema['const']!r}")
    if "enum" in schema:
        choices = schema["enum"]
        if not isinstance(choices, list) or not any(_json_equal(value, item) for item in choices):
            raise PackageGroupError(f"{owner} must be one of {choices!r}")

    expected_type = schema.get("type")
    if expected_type == "object":
        if not isinstance(value, dict):
            raise PackageGroupError(f"{owner} must be an object")
        properties = schema.get("properties", {})
        required = schema.get("required", [])
        if not isinstance(required, list) or not all(isinstance(item, str) for item in required):
            raise PackageGroupError(f"{owner}: schema required must be a string array")
        missing = [field for field in required if field not in value]
        unknown = [field for field in value if field not in properties]
        if missing or (schema.get("additionalProperties") is False and unknown):
            details = []
            if missing:
                details.append("missing " + ", ".join(sorted(missing)))
            if unknown:
                details.append("unknown " + ", ".join(sorted(unknown)))
            raise PackageGroupError(f"{owner}: fields must be exact ({'; '.join(details)})")
        for field, child in properties.items():
            if field in value:
                _validate_json_schema(value[field], child, root, f"{owner}.{field}")
    elif expected_type == "array":
        if not isinstance(value, list):
            raise PackageGroupError(f"{owner} must be an array")
        minimum = schema.get("minItems", 0)
        if not isinstance(minimum, int) or len(value) < minimum:
            raise PackageGroupError(f"{owner} must contain at least {minimum} items")
        item_schema = schema.get("items")
        if isinstance(item_schema, dict):
            for index, item in enumerate(value):
                _validate_json_schema(item, item_schema, root, f"{owner}[{index}]")
    elif expected_type == "string":
        if not isinstance(value, str):
            raise PackageGroupError(f"{owner} must be a string")
        pattern = schema.get("pattern")
        if pattern is not None and (not isinstance(pattern, str) or re.search(pattern, value) is None):
            raise PackageGroupError(f"{owner} does not match {pattern!r}")
    elif expected_type is not None:
        raise PackageGroupError(f"{owner}: unsupported schema type {expected_type!r}")


def _validate_descriptor_invariants(
    data: dict[str, Any], invariants: dict[str, Any]
) -> None:
    packages = data["packages"]
    for index, entry in enumerate(packages):
        owner = f"Web package descriptor packages[{index}]"
        for field, template in invariants["derivedFields"].items():
            try:
                expected = template.format_map(entry)
            except KeyError as exc:
                raise PackageGroupError(
                    f"Web package descriptor schema derivedFields references unknown field {exc.args[0]!r}"
                ) from exc
            if entry[field] != expected:
                raise PackageGroupError(f"{owner}: {field} must be {expected}")
        for rule in invariants["conditionalPackages"]:
            if all(entry.get(field) == value for field, value in rule["when"].items()) and not any(
                all(entry.get(field) == value for field, value in candidate.items())
                for candidate in rule["allowed"]
            ):
                raise PackageGroupError(f"{owner}: package mapping is not admitted by the schema")

    for field in invariants["uniqueBy"]:
        seen: set[Any] = set()
        for entry in packages:
            value = entry[field]
            if value in seen:
                raise PackageGroupError(f"Duplicate package {field}: {value!r}")
            seen.add(value)

    default_rule = invariants["defaultPackage"]
    reference = data[default_rule["referenceField"]]
    target_field = default_rule["targetField"]
    default_entry = next((entry for entry in packages if entry[target_field] == reference), None)
    if default_entry is None:
        raise PackageGroupError(f"default_package references unknown package {reference}")
    for field, expected in default_rule["requiredFields"].items():
        if default_entry[field] != expected:
            raise PackageGroupError(f"Web package descriptor: default package {field} must be {expected!r}")


def _json_equal(left: Any, right: Any) -> bool:
    return type(left) is type(right) and left == right


def load_descriptor(path: Path) -> dict[str, Any]:
    return validate_descriptor(load_json(path))


def public_packages(descriptor: dict[str, Any]) -> list[dict[str, Any]]:
    return [entry for entry in descriptor["packages"] if entry["visibility"] == "public"]


def all_package_paths(root: Path, descriptor: dict[str, Any]) -> list[tuple[dict[str, Any], Path]]:
    return [(entry, root / "platforms" / "web" / descriptor_package_path(entry)) for entry in descriptor["packages"]]


def validate_package_manifest(entry: dict[str, Any], path: Path, *, expected_version: str | None) -> dict[str, Any]:
    manifest_path = path / "package.json"
    manifest = load_json(manifest_path)
    owner = manifest_path.as_posix()
    if manifest.get("name") != entry["name"]:
        raise PackageGroupError(
            f"{owner}: name must be {entry['name']!r}, found {manifest.get('name')!r}"
        )
    if expected_version is not None and manifest.get("version") != expected_version:
        raise PackageGroupError(
            f"{owner}: version must be {expected_version!r}, found {manifest.get('version')!r}"
        )
    is_candidate = entry["visibility"] == "candidate"
    if is_candidate and manifest.get("private") is not True:
        raise PackageGroupError(f"{owner}: candidate packages must be private")
    if not is_candidate and manifest.get("private") is True:
        raise PackageGroupError(f"{owner}: public packages must not be private")

    validate_package_file_surface(manifest, owner)

    exports = manifest.get("exports")
    if not isinstance(exports, dict) or set(exports) != {"."}:
        raise PackageGroupError(f"{owner}: public API must export exactly '.'")
    if any("pkg/" in str(target) for target in exports.values()):
        raise PackageGroupError(f"{owner}: exports must not expose legacy pkg artifacts")
    return manifest


def validate_package_file_surface(manifest: dict[str, Any], owner: str) -> None:
    files = manifest.get("files")
    if not isinstance(files, list) or any(not isinstance(item, str) or not item for item in files):
        raise PackageGroupError(f"{owner}: files must be an array of non-empty paths")
    if len(set(files)) != len(files) or set(files) != EXPECTED_PACKAGE_FILE_ROOTS:
        raise PackageGroupError(
            f"{owner}: files must list exactly the closed package artifact roots"
        )
    if "scripts" in manifest:
        raise PackageGroupError(f"{owner}: packages must not declare npm lifecycle scripts")
    if "bundleDependencies" in manifest or "bundledDependencies" in manifest:
        raise PackageGroupError(f"{owner}: packages must not declare bundled dependencies")

    if manifest.get("private") is True:
        if "publishConfig" in manifest:
            raise PackageGroupError(f"{owner}: private packages must not declare publishConfig")
    elif manifest.get("publishConfig") != {"access": "public"}:
        raise PackageGroupError(
            f"{owner}: public packages must declare only publishConfig.access=public"
        )


def validate_workspace_manifest(root: Path) -> dict[str, Any]:
    manifest_path = root / "platforms" / "web" / "package.json"
    manifest = load_json(manifest_path)
    if manifest.get("private") is not True:
        raise PackageGroupError(f"{manifest_path}: Web workspace owner must be private")
    return manifest


def file_sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return "sha256:" + digest.hexdigest()


def file_integrity(path: Path) -> str:
    digest = hashlib.sha512()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return "sha512-" + base64.b64encode(digest.digest()).decode("ascii")


def normalized_member_name(member: tarfile.TarInfo) -> str:
    raw_name = member.name
    if not raw_name or "\\" in raw_name:
        raise PackageGroupError(f"tarball contains a non-canonical member path {raw_name!r}")
    path = PurePosixPath(raw_name)
    name = str(path)
    expected_raw_name = name + "/" if member.isdir() and name != "." else name
    if (
        path.is_absolute()
        or ".." in path.parts
        or name in {"", "."}
        or raw_name != expected_raw_name
    ):
        raise PackageGroupError(f"tarball contains unsafe member path {member.name!r}")
    return name


def read_tar_member(archive: tarfile.TarFile, name: str, *, max_bytes: int) -> bytes:
    try:
        member = archive.getmember(name)
    except KeyError as exc:
        raise PackageGroupError(f"tarball is missing member {name!r}") from exc
    if not member.isfile():
        raise PackageGroupError(f"tarball member {name!r} must be a regular file")
    if member.size > max_bytes:
        raise PackageGroupError(
            f"tarball member {name!r} exceeds the {max_bytes} byte verification budget"
        )
    handle = archive.extractfile(member)
    if handle is None:
        raise PackageGroupError(f"cannot read tarball member {name!r}")
    return handle.read()


def hash_tar_member(archive: tarfile.TarFile, name: str) -> tuple[int, str]:
    try:
        member = archive.getmember(name)
    except KeyError as exc:
        raise PackageGroupError(f"tarball is missing member {name!r}") from exc
    if not member.isfile():
        raise PackageGroupError(f"tarball member {name!r} must be a regular file")
    handle = archive.extractfile(member)
    if handle is None:
        raise PackageGroupError(f"cannot read tarball member {name!r}")
    digest = hashlib.sha256()
    for chunk in iter(lambda: handle.read(1024 * 1024), b""):
        digest.update(chunk)
    return member.size, "sha256:" + digest.hexdigest()


def validate_provenance_artifact_files(value: Any, owner: str) -> list[dict[str, Any]]:
    if not isinstance(value, list) or not value:
        raise PackageGroupError(f"{owner}: artifact_files must be a non-empty array")
    records: list[dict[str, Any]] = []
    paths: list[str] = []
    for index, record in enumerate(value):
        record_owner = f"{owner} artifact_files[{index}]"
        if not isinstance(record, dict):
            raise PackageGroupError(f"{record_owner}: expected an object")
        require_exact_keys(record, {"path", "bytes", "sha256"}, record_owner)
        artifact_path = require_string(record, "path", record_owner)
        parsed_path = PurePosixPath(artifact_path)
        is_wasm_artifact = (
            len(parsed_path.parts) >= 3 and parsed_path.parts[:2] == ("artifacts", "wasm")
        )
        is_dist_artifact = len(parsed_path.parts) >= 2 and parsed_path.parts[:1] == ("dist",)
        if (
            parsed_path.is_absolute()
            or ".." in parsed_path.parts
            or not (is_wasm_artifact or is_dist_artifact)
            or str(parsed_path) != artifact_path
        ):
            raise PackageGroupError(
                f"{record_owner}: path must be a canonical artifacts/wasm/** or dist/** path"
            )
        byte_count = record.get("bytes")
        if not isinstance(byte_count, int) or isinstance(byte_count, bool) or byte_count <= 0:
            raise PackageGroupError(f"{record_owner}: bytes must be a positive integer")
        digest = require_string(record, "sha256", record_owner)
        if not SHA256_RE.fullmatch(digest):
            raise PackageGroupError(f"{record_owner}: sha256 must be a lowercase sha256 digest")
        paths.append(artifact_path)
        records.append({"path": artifact_path, "bytes": byte_count, "sha256": digest})
    if len(set(paths)) != len(paths):
        raise PackageGroupError(f"{owner}: artifact_files paths must be unique")
    if paths != sorted(paths):
        raise PackageGroupError(f"{owner}: artifact_files must be sorted by path")
    return records


def validate_provenance_summary(value: Any, owner: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise PackageGroupError(f"{owner}: provenance must be a JSON object")
    require_exact_keys(value, {"id", "name", "version", "artifact_profile", "artifact_files"}, owner)
    package_id = require_string(value, "id", owner)
    if not PACKAGE_ID_RE.fullmatch(package_id):
        raise PackageGroupError(f"{owner}: provenance package id must be a kebab identifier")
    package_name = require_string(value, "name", owner)
    if descriptor_package_name_pattern().search(package_name) is None:
        raise PackageGroupError(f"{owner}: provenance package name is invalid")
    package_version = require_string(value, "version", owner)
    artifact_profile = require_string(value, "artifact_profile", owner)
    if not PACKAGE_ID_RE.fullmatch(artifact_profile):
        raise PackageGroupError(f"{owner}: artifact_profile must be a kebab identifier")
    artifact_files = validate_provenance_artifact_files(value.get("artifact_files"), owner)
    package_entry_paths = {
        record["path"]
        for record in artifact_files
        if record["path"].startswith("dist/package-entries/")
    }
    expected_package_entry_paths = {
        f"dist/package-entries/{package_id}{suffix}"
        for suffix in [".d.ts", ".d.ts.map", ".js", ".js.map"]
    }
    if package_entry_paths != expected_package_entry_paths:
        raise PackageGroupError(
            f"{owner}: artifact_files must contain exactly the owned package entry files"
        )
    return {
        "id": package_id,
        "name": package_name,
        "version": package_version,
        "artifact_profile": artifact_profile,
        "artifact_files": artifact_files,
    }


def validate_provenance_record(value: Any, owner: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise PackageGroupError(f"{owner}: provenance must be a JSON object")
    if value.get("schema_version") != PROVENANCE_SCHEMA_VERSION:
        raise PackageGroupError(f"{owner}: provenance schema_version must be {PROVENANCE_SCHEMA_VERSION}")
    package = value.get("package")
    if not isinstance(package, dict):
        raise PackageGroupError(f"{owner}: provenance package must be an object")
    return validate_provenance_summary(
        {
            "id": package.get("id"),
            "name": package.get("name"),
            "version": package.get("version"),
            "artifact_profile": value.get("artifact_profile"),
            "artifact_files": value.get("artifact_files"),
        },
        owner,
    )


def verify_tarball_provenance(
    archive: tarfile.TarFile,
    tarball_name: str,
    package_manifest: dict[str, Any],
    member_names: set[str],
    directory_names: set[str],
) -> dict[str, Any]:
    raw_provenance = json.loads(
        read_tar_member(
            archive,
            "package/artifacts/provenance.json",
            max_bytes=MAX_METADATA_MEMBER_BYTES,
        ).decode("utf-8"),
        object_pairs_hook=reject_duplicate_json_keys,
    )
    provenance = validate_provenance_record(
        raw_provenance,
        f"{tarball_name}: provenance",
    )
    if provenance["name"] != package_manifest.get("name"):
        raise PackageGroupError(f"{tarball_name}: provenance package name differs from package.json")
    if provenance["version"] != package_manifest.get("version"):
        raise PackageGroupError(f"{tarball_name}: provenance package version differs from package.json")
    merman = package_manifest.get("merman")
    if not isinstance(merman, dict) or merman.get("artifact_profile") != provenance["artifact_profile"]:
        raise PackageGroupError(f"{tarball_name}: provenance artifact_profile differs from package.json")

    wasm_members = sorted(name for name in member_names if name.startswith(WASM_TARBALL_PREFIX))
    wasm_directories = sorted(name for name in directory_names if name.startswith(WASM_TARBALL_PREFIX))
    unexpected_wasm_members = [
        name
        for name in wasm_members
        if name not in WASM_RUNTIME_TARBALL_FILES
        and not name.startswith(WASM_TARBALL_PREFIX + "snippets/")
    ]
    unexpected_wasm_directories = [
        name
        for name in wasm_directories
        if name != WASM_TARBALL_PREFIX.removesuffix("/")
        and name != WASM_TARBALL_PREFIX + "snippets"
        and not name.startswith(WASM_TARBALL_PREFIX + "snippets/")
    ]
    if unexpected_wasm_members or unexpected_wasm_directories:
        unexpected = sorted([*unexpected_wasm_members, *unexpected_wasm_directories])
        raise PackageGroupError(
            f"{tarball_name}: WASM artifacts may contain only the runtime files and snippets/**, found "
            + ", ".join(unexpected)
        )
    if not WASM_RUNTIME_TARBALL_FILES.issubset(wasm_members):
        missing_runtime = sorted(WASM_RUNTIME_TARBALL_FILES - set(wasm_members))
        raise PackageGroupError(
            f"{tarball_name}: WASM artifacts are missing required runtime files: "
            + ", ".join(missing_runtime)
        )
    dist_prefix = "package/dist/"
    dist_members = sorted(name for name in member_names if name.startswith(dist_prefix))
    entry_prefix = "package/dist/package-entries/"
    entry_members = sorted(name for name in dist_members if name.startswith(entry_prefix))
    expected_entry_members = sorted(
        f"{entry_prefix}{provenance['id']}{suffix}"
        for suffix in [".d.ts", ".d.ts.map", ".js", ".js.map"]
    )
    if entry_members != expected_entry_members:
        raise PackageGroupError(
            f"{tarball_name}: package entry artifacts must contain exactly the owned wrapper files"
        )
    artifact_members = {name for name in member_names if name.startswith(ARTIFACT_TARBALL_PREFIX)}
    allowed_artifact_members = {"package/artifacts/provenance.json", *wasm_members}
    unexpected_artifacts = sorted(artifact_members - allowed_artifact_members)
    if unexpected_artifacts:
        raise PackageGroupError(
            f"{tarball_name}: artifacts may contain only provenance.json and wasm/**, found "
            + ", ".join(unexpected_artifacts)
        )
    expected_paths = [name.removeprefix("package/") for name in sorted([*wasm_members, *dist_members])]
    observed_paths = [record["path"] for record in provenance["artifact_files"]]
    if observed_paths != expected_paths:
        raise PackageGroupError(
            f"{tarball_name}: provenance artifact_files must exactly cover owned WASM and dist files"
        )
    for record in provenance["artifact_files"]:
        tarball_member = "package/" + record["path"]
        byte_count, digest = hash_tar_member(archive, tarball_member)
        if byte_count != record["bytes"]:
            raise PackageGroupError(
                f"{tarball_name}: provenance bytes differ for {record['path']}"
            )
        if digest != record["sha256"]:
            raise PackageGroupError(
                f"{tarball_name}: provenance sha256 differs for {record['path']}"
            )
    return provenance


def verify_tarball_entrypoints(
    package_manifest: dict[str, Any],
    provenance: dict[str, Any],
    tarball_name: str,
    member_names: set[str],
) -> None:
    entry_prefix = f"./dist/package-entries/{provenance['id']}"
    expected_main = entry_prefix + ".js"
    expected_types = entry_prefix + ".d.ts"
    if package_manifest.get("main") != expected_main:
        raise PackageGroupError(f"{tarball_name}: main must point to {expected_main}")
    if package_manifest.get("types") != expected_types:
        raise PackageGroupError(f"{tarball_name}: types must point to {expected_types}")
    expected_exports = {".": {"import": expected_main, "types": expected_types}}
    if package_manifest.get("exports") != expected_exports:
        raise PackageGroupError(f"{tarball_name}: exports must expose exactly the owned package entrypoint")
    expected_members = {
        "package/" + expected_main.removeprefix("./"),
        "package/" + expected_types.removeprefix("./"),
    }
    missing = sorted(expected_members - member_names)
    if missing:
        raise PackageGroupError(
            f"{tarball_name}: package entrypoint files are missing: {', '.join(missing)}"
        )


def verify_tarball_file_closure(
    tarball_name: str,
    member_names: set[str],
    directory_names: set[str],
    legal_paths: list[str],
    provenance: dict[str, Any],
) -> None:
    allowed_files = {
        *PACKAGE_TARBALL_FIXED_FILES,
        *legal_paths,
        *("package/" + record["path"] for record in provenance["artifact_files"]),
    }
    unexpected_files = sorted(member_names - allowed_files)
    if unexpected_files:
        raise PackageGroupError(
            f"{tarball_name}: tarball contains files outside the closed package surface: "
            + ", ".join(unexpected_files)
        )

    allowed_directories: set[str] = set()
    for filename in allowed_files:
        parent = PurePosixPath(filename).parent
        while str(parent) not in {"", "."}:
            allowed_directories.add(str(parent))
            parent = parent.parent
    unexpected_directories = sorted(directory_names - allowed_directories)
    if unexpected_directories:
        raise PackageGroupError(
            f"{tarball_name}: tarball contains directories outside the closed package surface: "
            + ", ".join(unexpected_directories)
        )


def inspect_tarball(path: Path) -> dict[str, Any]:
    if path.is_symlink() or not path.is_file():
        raise PackageGroupError(f"tarball must be a regular file: {path}")
    if path.stat().st_size > MAX_TARBALL_PACKED_BYTES:
        raise PackageGroupError(
            f"{path.name}: packed size exceeds the {MAX_TARBALL_PACKED_BYTES} byte verification budget"
        )
    try:
        with tarfile.open(path, mode="r:gz") as archive:
            names: set[str] = set()
            directories: set[str] = set()
            regular_members: list[tarfile.TarInfo] = []
            unpacked_bytes = 0
            for member in archive.getmembers():
                name = normalized_member_name(member)
                if member.issym() or member.islnk():
                    raise PackageGroupError(
                        f"{path.name}: tarball must not contain links: {member.name!r}"
                    )
                if member.isdir():
                    directories.add(name)
                    continue
                if not member.isfile():
                    raise PackageGroupError(
                        f"{path.name}: tarball contains an unsupported member type: {member.name!r}"
                    )
                if name in names:
                    raise PackageGroupError(f"{path.name}: tarball contains duplicate member {name!r}")
                names.add(name)
                regular_members.append(member)
                unpacked_bytes += member.size
                if unpacked_bytes > MAX_TARBALL_UNPACKED_BYTES:
                    raise PackageGroupError(
                        f"{path.name}: unpacked size exceeds the "
                        f"{MAX_TARBALL_UNPACKED_BYTES} byte verification budget"
                    )
            missing = sorted(REQUIRED_TARBALL_FILES - names)
            if missing:
                raise PackageGroupError(
                    f"{path.name}: missing required package members: {', '.join(missing)}"
                )
            if "package/THIRD_PARTY_LICENSES" in names:
                raise PackageGroupError(
                    f"{path.name}: THIRD_PARTY_LICENSES must be a directory containing legal files"
                )
            legal_paths = sorted(name for name in names if name.startswith(LEGAL_TARBALL_PREFIX))
            if not legal_paths:
                raise PackageGroupError(
                    f"{path.name}: missing projected third-party license material"
                )
            if any(name.startswith(LEGACY_TARBALL_PREFIX) for name in names):
                raise PackageGroupError(f"{path.name}: contains legacy pkg artifacts")
            wasm_members = sorted(name for name in names if name.endswith(".wasm"))
            if wasm_members != [WASM_MEMBER]:
                raise PackageGroupError(
                    f"{path.name}: must contain exactly {WASM_MEMBER}, found {', '.join(wasm_members) or 'none'}"
                )
            manifest_bytes = read_tar_member(
                archive,
                "package/package.json",
                max_bytes=MAX_METADATA_MEMBER_BYTES,
            )
            manifest = json.loads(manifest_bytes.decode("utf-8"), object_pairs_hook=reject_duplicate_json_keys)
            if not isinstance(manifest, dict):
                raise PackageGroupError(f"{path.name}: package.json is not an object")
            validate_package_file_surface(manifest, f"{path.name}: package.json")
            provenance = verify_tarball_provenance(archive, path.name, manifest, names, directories)
            verify_tarball_entrypoints(manifest, provenance, path.name, names)
            legal_members = {
                name: hashlib.sha256(
                    read_tar_member(archive, name, max_bytes=MAX_LEGAL_MEMBER_BYTES)
                ).hexdigest()
                for name in legal_paths
            }
            verify_tarball_file_closure(path.name, names, directories, legal_paths, provenance)
    except (OSError, tarfile.TarError, UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise PackageGroupError(f"cannot inspect {path}: {exc}") from exc

    name = manifest.get("name")
    version = manifest.get("version")
    if not isinstance(name, str) or descriptor_package_name_pattern().search(name) is None:
        raise PackageGroupError(f"{path.name}: tarball package name is invalid")
    if not isinstance(version, str) or not version:
        raise PackageGroupError(f"{path.name}: tarball package version is invalid")
    return {
        "name": name,
        "version": version,
        "sha256": file_sha256(path),
        "integrity": file_integrity(path),
        "packed_bytes": path.stat().st_size,
        "unpacked_bytes": unpacked_bytes,
        "wasm_path": "artifacts/wasm/merman_wasm_bg.wasm",
        "provenance": provenance,
        "legal_members": legal_members,
    }


def legal_digest(records: list[dict[str, Any]]) -> str:
    payload = [
        {"name": record["name"], "legal_members": record["legal_members"]}
        for record in sorted(records, key=lambda record: record["name"])
    ]
    encoded = json.dumps(payload, sort_keys=True, separators=(",", ":")).encode("utf-8")
    return "sha256:" + hashlib.sha256(encoded).hexdigest()


def validate_public_package_size_admission(packages: list[dict[str, Any]], owner: str) -> None:
    """Require a measured unpacked-size benefit for published slim workflow packages."""

    full = next((package for package in packages if package.get("id") == FULL_PACKAGE_ID), None)
    if full is None:
        raise PackageGroupError(f"{owner}: must include the public {FULL_PACKAGE_ID!r} package")
    full_bytes = full.get("unpacked_bytes")
    if not isinstance(full_bytes, int) or isinstance(full_bytes, bool) or full_bytes <= 0:
        raise PackageGroupError(f"{owner}: full package unpacked_bytes must be a positive integer")
    for package in packages:
        if package is full or package.get("id") == COMPLETE_SVG_PACKAGE_ID:
            continue
        unpacked_bytes = package.get("unpacked_bytes")
        if not isinstance(unpacked_bytes, int) or isinstance(unpacked_bytes, bool) or unpacked_bytes <= 0:
            raise PackageGroupError(
                f"{owner}: {package.get('id')!r} package unpacked_bytes must be a positive integer"
            )
        # Keep this integer-only so the boundary is deterministic across Python versions.
        if unpacked_bytes * 100 > full_bytes * (100 - MIN_PUBLIC_SLIM_UNPACKED_SIZE_SAVINGS_PERCENT):
            saving_percent = (1 - unpacked_bytes / full_bytes) * 100
            raise PackageGroupError(
                f"{owner}: public slim package {package.get('id')!r} must be at least "
                f"{MIN_PUBLIC_SLIM_UNPACKED_SIZE_SAVINGS_PERCENT}% smaller than {FULL_PACKAGE_ID!r} "
                f"by actual unpacked tarball bytes (observed {saving_percent:.1f}%)"
            )


def validate_source_sha(value: str) -> None:
    if not SOURCE_SHA_RE.fullmatch(value):
        raise PackageGroupError("source_sha must be a lowercase Git object id")


def safe_tarball_name(value: str) -> str:
    path = PurePosixPath(value)
    if path.name != value or path.suffix != ".tgz" or value.startswith("."):
        raise PackageGroupError(f"tarball must be a simple .tgz filename, found {value!r}")
    return value


def build_manifest(
    root: Path,
    descriptor: dict[str, Any],
    artifact_dir: Path,
    *,
    version: str,
    source_sha: str,
    target_dist_tag: str,
) -> dict[str, Any]:
    validate_source_sha(source_sha)
    if not isinstance(version, str) or not version:
        raise PackageGroupError("version must be a non-empty string")
    if not isinstance(target_dist_tag, str) or not NPM_DIST_TAG_RE.fullmatch(target_dist_tag):
        raise PackageGroupError("target_dist_tag must be a lowercase npm dist-tag")
    validate_workspace_manifest(root)
    public_entries = public_packages(descriptor)
    records_by_name: dict[str, tuple[Path, dict[str, Any]]] = {}
    for tarball in sorted(artifact_dir.glob("*.tgz")):
        record = inspect_tarball(tarball)
        name = record["name"]
        if name in records_by_name:
            raise PackageGroupError(f"artifact directory contains duplicate package tarballs for {name}")
        records_by_name[name] = (tarball, record)

    expected_names = {entry["name"] for entry in public_entries}
    actual_names = set(records_by_name)
    if actual_names != expected_names:
        missing = sorted(expected_names - actual_names)
        extra = sorted(actual_names - expected_names)
        details = []
        if missing:
            details.append("missing " + ", ".join(missing))
        if extra:
            details.append("unexpected " + ", ".join(extra))
        raise PackageGroupError("artifact directory package group mismatch: " + "; ".join(details))

    package_records: list[dict[str, Any]] = []
    inspected_records: list[dict[str, Any]] = []
    for entry in public_entries:
        package_dir = root / "platforms" / "web" / descriptor_package_path(entry)
        validate_package_manifest(entry, package_dir, expected_version=version)
        tarball, record = records_by_name[entry["name"]]
        if record["version"] != version:
            raise PackageGroupError(
                f"{tarball.name}: version must be {version!r}, found {record['version']!r}"
            )
        provenance = record["provenance"]
        for key, expected in {
            "id": entry["id"],
            "name": entry["name"],
            "version": version,
            "artifact_profile": entry["artifact_profile"],
        }.items():
            if provenance[key] != expected:
                raise PackageGroupError(
                    f"{tarball.name}: provenance {key} must be {expected!r}, found {provenance[key]!r}"
                )
        inspected_records.append(record)
        package_records.append(
            {
                "id": entry["id"],
                "name": entry["name"],
                "package_dir": entry["package_dir"],
                "artifact_profile": entry["artifact_profile"],
                "runtime_profile": entry["runtime_profile"],
                "tarball": tarball.name,
                "sha256": record["sha256"],
                "integrity": record["integrity"],
                "wasm_path": record["wasm_path"],
                "provenance": provenance,
                "packed_bytes": record["packed_bytes"],
                "unpacked_bytes": record["unpacked_bytes"],
            }
        )
    validate_public_package_size_admission(package_records, "Web package group artifact")
    return {
        "schema_version": GROUP_MANIFEST_SCHEMA_VERSION,
        "version": version,
        "source_sha": source_sha,
        "target_dist_tag": target_dist_tag,
        "packages": package_records,
        "legal_digest": legal_digest(inspected_records),
    }


def validate_group_manifest(data: dict[str, Any]) -> dict[str, Any]:
    require_exact_keys(
        data,
        {"schema_version", "version", "source_sha", "target_dist_tag", "packages", "legal_digest"},
        "Web package group manifest",
    )
    if data.get("schema_version") != GROUP_MANIFEST_SCHEMA_VERSION:
        raise PackageGroupError(
            f"Web package group manifest schema_version must be {GROUP_MANIFEST_SCHEMA_VERSION}"
        )
    version = require_string(data, "version", "Web package group manifest")
    del version
    validate_source_sha(require_string(data, "source_sha", "Web package group manifest"))
    target_dist_tag = require_string(data, "target_dist_tag", "Web package group manifest")
    if not NPM_DIST_TAG_RE.fullmatch(target_dist_tag):
        raise PackageGroupError("Web package group manifest: target_dist_tag must be a lowercase npm dist-tag")
    legal = require_string(data, "legal_digest", "Web package group manifest")
    if not SHA256_RE.fullmatch(legal):
        raise PackageGroupError("Web package group manifest: legal_digest must be a sha256 digest")
    packages = data.get("packages")
    if not isinstance(packages, list) or not packages:
        raise PackageGroupError("Web package group manifest: packages must be a non-empty array")
    seen_ids: set[str] = set()
    seen_names: set[str] = set()
    seen_tarballs: set[str] = set()
    for index, package in enumerate(packages):
        owner = f"Web package group manifest packages[{index}]"
        if not isinstance(package, dict):
            raise PackageGroupError(f"{owner}: expected an object")
        require_exact_keys(
            package,
            {
                "id", "name", "package_dir", "artifact_profile", "runtime_profile", "tarball",
                "sha256", "integrity", "wasm_path", "provenance", "packed_bytes", "unpacked_bytes",
            },
            owner,
        )
        package_id = require_string(package, "id", owner)
        if not PACKAGE_ID_RE.fullmatch(package_id) or package_id in seen_ids:
            raise PackageGroupError(f"{owner}: id must be a unique kebab identifier")
        seen_ids.add(package_id)
        name = require_string(package, "name", owner)
        if descriptor_package_name_pattern().search(name) is None or name in seen_names:
            raise PackageGroupError(f"{owner}: name must be a unique @mermanjs/web package")
        seen_names.add(name)
        descriptor_package_path(package)
        if require_string(package, "artifact_profile", owner) == "":
            raise PackageGroupError(f"{owner}: artifact_profile must be non-empty")
        if require_string(package, "runtime_profile", owner) == "":
            raise PackageGroupError(f"{owner}: runtime_profile must be non-empty")
        tarball = safe_tarball_name(require_string(package, "tarball", owner))
        if tarball in seen_tarballs:
            raise PackageGroupError(f"{owner}: tarball must be unique")
        seen_tarballs.add(tarball)
        if not SHA256_RE.fullmatch(require_string(package, "sha256", owner)):
            raise PackageGroupError(f"{owner}: sha256 must be a sha256 digest")
        if not INTEGRITY_RE.fullmatch(require_string(package, "integrity", owner)):
            raise PackageGroupError(f"{owner}: integrity must be an npm sha512 integrity")
        if package.get("wasm_path") != "artifacts/wasm/merman_wasm_bg.wasm":
            raise PackageGroupError(f"{owner}: wasm_path is not the owned package artifact")
        provenance = validate_provenance_summary(package.get("provenance"), f"{owner} provenance")
        for key, expected in {
            "id": package_id,
            "name": name,
            "version": data["version"],
            "artifact_profile": package["artifact_profile"],
        }.items():
            if provenance[key] != expected:
                raise PackageGroupError(
                    f"{owner}: provenance {key} must be {expected!r}, found {provenance[key]!r}"
                )
        for key in ["packed_bytes", "unpacked_bytes"]:
            value = package.get(key)
            if not isinstance(value, int) or isinstance(value, bool) or value <= 0:
                raise PackageGroupError(f"{owner}: {key} must be a positive integer")
    validate_public_package_size_admission(packages, "Web package group manifest")
    return data


def write_json(path: Path, data: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(f".{path.name}.{os.getpid()}.tmp")
    temporary.write_text(json.dumps(data, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    os.replace(temporary, path)


def create_manifest(
    root: Path,
    descriptor_path: Path,
    artifact_dir: Path,
    *,
    version: str,
    source_sha: str,
    target_dist_tag: str,
    output: Path | None = None,
) -> Path:
    descriptor = load_descriptor(descriptor_path)
    manifest = build_manifest(
        root,
        descriptor,
        artifact_dir,
        version=version,
        source_sha=source_sha,
        target_dist_tag=target_dist_tag,
    )
    path = output or artifact_dir / DEFAULT_MANIFEST_NAME
    write_json(path, manifest)
    return path


def verify_artifact(
    manifest_path: Path,
    artifact_dir: Path,
    *,
    expected_version: str | None = None,
    expected_source_sha: str | None = None,
    expected_target_dist_tag: str | None = None,
    descriptor: dict[str, Any] | None = None,
) -> dict[str, Any]:
    manifest = validate_group_manifest(load_json(manifest_path))
    if expected_version is not None and manifest["version"] != expected_version:
        raise PackageGroupError(
            f"{manifest_path}: version must be {expected_version!r}, found {manifest['version']!r}"
        )
    if expected_source_sha is not None:
        validate_source_sha(expected_source_sha)
        if manifest["source_sha"] != expected_source_sha:
            raise PackageGroupError(
                f"{manifest_path}: source_sha must be {expected_source_sha!r}, "
                f"found {manifest['source_sha']!r}"
            )
    if expected_target_dist_tag is not None:
        if not NPM_DIST_TAG_RE.fullmatch(expected_target_dist_tag):
            raise PackageGroupError("expected_target_dist_tag must be a lowercase npm dist-tag")
        if manifest["target_dist_tag"] != expected_target_dist_tag:
            raise PackageGroupError(
                f"{manifest_path}: target_dist_tag must be {expected_target_dist_tag!r}, "
                f"found {manifest['target_dist_tag']!r}"
            )
    if descriptor is not None:
        expected_entries = {entry["id"]: entry for entry in public_packages(descriptor)}
        actual_entries = {entry["id"]: entry for entry in manifest["packages"]}
        if set(actual_entries) != set(expected_entries):
            raise PackageGroupError("package group manifest does not exactly cover public descriptor packages")
        for package_id, expected in expected_entries.items():
            actual = actual_entries[package_id]
            for key in ["name", "package_dir", "artifact_profile", "runtime_profile"]:
                if actual[key] != expected[key]:
                    raise PackageGroupError(
                        f"package group manifest {package_id}: {key} differs from descriptor"
                    )

    expected_tarballs = {record["tarball"] for record in manifest["packages"]}
    actual_tarballs = {path.name for path in artifact_dir.glob("*.tgz")}
    if actual_tarballs != expected_tarballs:
        missing = sorted(expected_tarballs - actual_tarballs)
        unexpected = sorted(actual_tarballs - expected_tarballs)
        details = []
        if missing:
            details.append("missing " + ", ".join(missing))
        if unexpected:
            details.append("unexpected " + ", ".join(unexpected))
        raise PackageGroupError("artifact directory tarball set differs from group manifest: " + "; ".join(details))

    inspected: list[dict[str, Any]] = []
    for record in manifest["packages"]:
        tarball = artifact_dir / record["tarball"]
        if not tarball.is_file():
            raise PackageGroupError(f"missing package tarball {tarball}")
        actual = inspect_tarball(tarball)
        for key in [
            "name",
            "sha256",
            "integrity",
            "wasm_path",
            "provenance",
            "packed_bytes",
            "unpacked_bytes",
        ]:
            if actual[key] != record[key]:
                raise PackageGroupError(
                    f"{tarball.name}: manifest {key} does not match packed artifact"
                )
        if actual["version"] != manifest["version"]:
            raise PackageGroupError(f"{tarball.name}: package version differs from group manifest")
        inspected.append(actual)
    if legal_digest(inspected) != manifest["legal_digest"]:
        raise PackageGroupError("package group legal_digest does not match packed artifacts")
    return manifest


def pack_group(
    root: Path,
    descriptor_path: Path,
    artifact_dir: Path,
    *,
    version: str,
    source_sha: str,
    target_dist_tag: str,
) -> Path:
    descriptor = load_descriptor(descriptor_path)
    artifact_dir.mkdir(parents=True, exist_ok=True)
    if any(artifact_dir.iterdir()):
        raise PackageGroupError(f"artifact directory must be empty: {artifact_dir}")
    validate_workspace_manifest(root)
    for entry in public_packages(descriptor):
        package_dir = root / "platforms" / "web" / descriptor_package_path(entry)
        validate_package_manifest(entry, package_dir, expected_version=version)
        result = subprocess.run(
            ["npm", "pack", "--ignore-scripts", "--json", "--pack-destination", str(artifact_dir)],
            cwd=package_dir,
            check=False,
            capture_output=True,
            text=True,
        )
        if result.returncode != 0:
            diagnostic = result.stderr.strip() or result.stdout.strip() or "npm pack failed"
            raise PackageGroupError(f"{entry['name']}: {diagnostic}")
        try:
            output = json.loads(result.stdout)
        except json.JSONDecodeError as exc:
            raise PackageGroupError(f"{entry['name']}: npm pack emitted invalid JSON") from exc
        if not isinstance(output, list) or len(output) != 1 or not isinstance(output[0], dict):
            raise PackageGroupError(f"{entry['name']}: npm pack must report exactly one tarball")
        filename = output[0].get("filename")
        if not isinstance(filename, str) or not (artifact_dir / filename).is_file():
            raise PackageGroupError(f"{entry['name']}: npm pack did not create its reported tarball")
    manifest = create_manifest(
        root,
        descriptor_path,
        artifact_dir,
        version=version,
        source_sha=source_sha,
        target_dist_tag=target_dist_tag,
    )
    verify_artifact(manifest, artifact_dir, expected_version=version, descriptor=descriptor)
    return manifest


class NpmClient(Protocol):
    def version_integrity(self, package: str, version: str) -> str | None: ...

    def dist_tag(self, package: str, tag: str) -> str | None: ...

    def publish(self, tarball: Path, tag: str) -> None: ...

    def add_tag(self, package: str, version: str, tag: str) -> None: ...

    def remove_tag(self, package: str, tag: str) -> None: ...


@dataclass
class NpmCli:
    registry: str | None = None

    def _command(self, *args: str) -> list[str]:
        command = ["npm", *args]
        if self.registry:
            command.extend(["--registry", self.registry])
        return command

    def _run(self, *args: str) -> subprocess.CompletedProcess[str]:
        result = subprocess.run(
            self._command(*args),
            check=False,
            capture_output=True,
            text=True,
        )
        if result.returncode != 0:
            diagnostic = result.stderr.strip() or result.stdout.strip() or "npm command failed"
            raise PackageGroupError(f"npm {' '.join(args)} failed: {diagnostic}")
        return result

    def version_integrity(self, package: str, version: str) -> str | None:
        result = subprocess.run(
            self._command("view", f"{package}@{version}", "dist.integrity", "--json"),
            check=False,
            capture_output=True,
            text=True,
        )
        if result.returncode != 0:
            diagnostic = result.stderr.strip() or result.stdout.strip()
            if "E404" in diagnostic or "404" in diagnostic:
                return None
            raise PackageGroupError(f"npm view {package}@{version} failed: {diagnostic or 'unknown error'}")
        try:
            value = json.loads(result.stdout)
        except json.JSONDecodeError as exc:
            raise PackageGroupError(f"npm view {package}@{version} returned invalid JSON") from exc
        if not isinstance(value, str) or not INTEGRITY_RE.fullmatch(value):
            raise PackageGroupError(f"npm view {package}@{version} returned invalid dist.integrity")
        return value

    def dist_tag(self, package: str, tag: str) -> str | None:
        result = subprocess.run(
            self._command("view", package, "dist-tags", "--json"),
            check=False,
            capture_output=True,
            text=True,
        )
        if result.returncode != 0:
            diagnostic = result.stderr.strip() or result.stdout.strip()
            if "E404" in diagnostic or "404" in diagnostic:
                return None
            raise PackageGroupError(f"npm view {package} dist-tags failed: {diagnostic or 'unknown error'}")
        try:
            value = json.loads(result.stdout)
        except json.JSONDecodeError as exc:
            raise PackageGroupError(f"npm view {package} dist-tags returned invalid JSON") from exc
        if not isinstance(value, dict):
            raise PackageGroupError(f"npm view {package} dist-tags returned invalid JSON")
        observed = value.get(tag)
        if observed is not None and not isinstance(observed, str):
            raise PackageGroupError(f"npm view {package} dist-tag {tag!r} is not a version string")
        return observed

    def publish(self, tarball: Path, tag: str) -> None:
        self._run("publish", str(tarball), "--ignore-scripts", "--access", "public", "--tag", tag)

    def add_tag(self, package: str, version: str, tag: str) -> None:
        self._run("dist-tag", "add", f"{package}@{version}", tag)

    def remove_tag(self, package: str, tag: str) -> None:
        self._run("dist-tag", "rm", package, tag)


@dataclass
class DryRunNpmClient:
    manifest: dict[str, Any]
    versions: dict[tuple[str, str], str] = field(default_factory=dict)
    tags: dict[tuple[str, str], str] = field(default_factory=dict)
    operations: list[str] = field(default_factory=list)

    def version_integrity(self, package: str, version: str) -> str | None:
        return self.versions.get((package, version))

    def dist_tag(self, package: str, tag: str) -> str | None:
        return self.tags.get((package, tag))

    def publish(self, tarball: Path, tag: str) -> None:
        record = next(item for item in self.manifest["packages"] if item["tarball"] == tarball.name)
        self.operations.append(f"publish {record['name']}@{self.manifest['version']} --tag {tag}")
        self.versions[(record["name"], self.manifest["version"])] = record["integrity"]

    def add_tag(self, package: str, version: str, tag: str) -> None:
        self.operations.append(f"dist-tag add {package}@{version} {tag}")
        self.tags[(package, tag)] = version

    def remove_tag(self, package: str, tag: str) -> None:
        self.operations.append(f"dist-tag rm {package} {tag}")
        self.tags.pop((package, tag), None)


def staging_tag(version: str) -> str:
    return "staging-v" + re.sub(r"[^a-z0-9]+", "-", version.lower()).strip("-")


def restore_tags(
    client: NpmClient,
    changed: list[dict[str, Any]],
    previous: dict[str, str | None],
    tag: str,
) -> list[str]:
    failures: list[str] = []
    for record in reversed(changed):
        name = record["name"]
        prior = previous[name]
        try:
            if prior is None:
                client.remove_tag(name, tag)
                observed = client.dist_tag(name, tag)
                if observed is not None:
                    raise PackageGroupError(f"tag still points to {observed!r}")
            else:
                client.add_tag(name, prior, tag)
                observed = client.dist_tag(name, tag)
                if observed != prior:
                    raise PackageGroupError(f"tag points to {observed!r}, expected {prior!r}")
        except PackageGroupError as exc:
            failures.append(f"{name}: {exc}")
    return failures


def reconcile_group(manifest: dict[str, Any], artifact_dir: Path, client: NpmClient) -> dict[str, Any]:
    manifest = validate_group_manifest(manifest)
    version = manifest["version"]
    target_tag = manifest["target_dist_tag"]
    stage = staging_tag(version)
    report: dict[str, Any] = {
        "schema_version": 1,
        "version": version,
        "target_dist_tag": target_tag,
        "staging_dist_tag": stage,
        "published": [],
        "promoted": [],
        "previous_tags": {},
        "status": "running",
    }
    for record in manifest["packages"]:
        try:
            observed_integrity = client.version_integrity(record["name"], version)
        except PackageGroupError as exc:
            report["status"] = "failed-before-promotion"
            report["error"] = str(exc)
            raise ReconciliationError(str(exc), report) from exc
        if observed_integrity is None:
            try:
                client.publish(artifact_dir / record["tarball"], stage)
            except PackageGroupError as exc:
                report["status"] = "failed-before-promotion"
                report["error"] = str(exc)
                raise ReconciliationError(str(exc), report) from exc
            report["published"].append(record["name"])
            try:
                observed_integrity = client.version_integrity(record["name"], version)
            except PackageGroupError as exc:
                report["status"] = "failed-before-promotion"
                report["error"] = str(exc)
                raise ReconciliationError(str(exc), report) from exc
        if observed_integrity != record["integrity"]:
            message = f"{record['name']}@{version}: registry integrity differs from the verified tarball"
            report["status"] = "failed-before-promotion"
            report["error"] = message
            raise ReconciliationError(message, report)

    try:
        previous = {
            record["name"]: client.dist_tag(record["name"], target_tag)
            for record in manifest["packages"]
        }
    except PackageGroupError as exc:
        report["status"] = "failed-before-promotion"
        report["error"] = str(exc)
        raise ReconciliationError(str(exc), report) from exc
    report["previous_tags"] = previous
    changed: list[dict[str, Any]] = []
    try:
        for record in manifest["packages"]:
            if previous[record["name"]] == version:
                continue
            # Treat a completed request as potentially visible even if the
            # client reports an error. Compensation must cover it.
            changed.append(record)
            client.add_tag(record["name"], version, target_tag)
            observed_tag = client.dist_tag(record["name"], target_tag)
            if observed_tag != version:
                raise PackageGroupError(
                    f"{record['name']}: dist-tag {target_tag!r} points to {observed_tag!r} after promotion"
                )
    except PackageGroupError as exc:
        rollback_failures = restore_tags(client, changed, previous, target_tag)
        detail = str(exc)
        if rollback_failures:
            detail += "; rollback failed: " + "; ".join(rollback_failures)
        report["status"] = "failed-during-promotion"
        report["error"] = detail
        report["promoted"] = [record["name"] for record in changed]
        report["rollback_failures"] = rollback_failures
        raise ReconciliationError(detail, report) from exc
    report["status"] = "reconciled"
    report["promoted"] = [record["name"] for record in changed]
    return report


def cli() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)

    validate = subparsers.add_parser("validate-descriptor")
    validate.add_argument("--descriptor", type=Path, default=DEFAULT_DESCRIPTOR)

    for name in ["pack", "create-manifest"]:
        command = subparsers.add_parser(name)
        command.add_argument("--root", type=Path, default=Path("."))
        command.add_argument("--descriptor", type=Path, default=DEFAULT_DESCRIPTOR)
        command.add_argument("--artifact-dir", type=Path, required=True)
        command.add_argument("--version", required=True)
        command.add_argument("--source-sha", required=True)
        command.add_argument("--target-dist-tag", required=True)
        command.add_argument("--output", type=Path)

    verify = subparsers.add_parser("verify-artifact")
    verify.add_argument("--manifest", type=Path, required=True)
    verify.add_argument("--artifact-dir", type=Path, required=True)
    verify.add_argument("--version")
    verify.add_argument("--source-sha")
    verify.add_argument("--target-dist-tag")
    verify.add_argument("--descriptor", type=Path)

    reconcile = subparsers.add_parser("reconcile")
    reconcile.add_argument("--manifest", type=Path, required=True)
    reconcile.add_argument("--artifact-dir", type=Path, required=True)
    reconcile.add_argument("--registry", default=NPMJS_REGISTRY_URL)
    reconcile.add_argument("--report", type=Path, required=True)
    reconcile.add_argument("--dry-run", action="store_true")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = cli().parse_args(argv)
    try:
        if args.command == "validate-descriptor":
            descriptor = load_descriptor(args.descriptor)
            print(f"validated {len(descriptor['packages'])} Web package descriptor entries")
        elif args.command == "pack":
            manifest = pack_group(
                args.root.resolve(),
                args.descriptor,
                args.artifact_dir,
                version=args.version,
                source_sha=args.source_sha,
                target_dist_tag=args.target_dist_tag,
            )
            print(manifest)
        elif args.command == "create-manifest":
            manifest = create_manifest(
                args.root.resolve(),
                args.descriptor,
                args.artifact_dir,
                version=args.version,
                source_sha=args.source_sha,
                target_dist_tag=args.target_dist_tag,
                output=args.output,
            )
            print(manifest)
        elif args.command == "verify-artifact":
            descriptor = load_descriptor(args.descriptor) if args.descriptor else None
            manifest = verify_artifact(
                args.manifest,
                args.artifact_dir,
                expected_version=args.version,
                expected_source_sha=args.source_sha,
                expected_target_dist_tag=args.target_dist_tag,
                descriptor=descriptor,
            )
            print(f"validated {len(manifest['packages'])} packed Web package(s)")
        elif args.command == "reconcile":
            manifest = verify_artifact(args.manifest, args.artifact_dir)
            client: NpmClient
            if args.dry_run:
                client = DryRunNpmClient(manifest)
            else:
                client = NpmCli(args.registry)
            report = reconcile_group(manifest, args.artifact_dir, client)
            if isinstance(client, DryRunNpmClient):
                report["dry_run_operations"] = client.operations
            write_json(args.report, report)
            print(args.report)
    except ReconciliationError as exc:
        write_json(args.report, exc.report)
        print(f"error: {exc}", file=sys.stderr)
        return 1
    except PackageGroupError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
