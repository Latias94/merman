#!/usr/bin/env python3
"""Generate a deterministic Rust dependency license report with cargo-about."""

from __future__ import annotations

import argparse
from dataclasses import dataclass
import hashlib
import json
import subprocess
import sys
import tempfile
import tomllib
from pathlib import Path, PurePosixPath, PureWindowsPath
from typing import Any

try:
    from scripts import strict_json
except ModuleNotFoundError:
    import strict_json


ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "THIRD_PARTY_LICENSES" / "rust-cargo-dependencies.json"
ARTIFACT_PROFILES_PATH = Path("capabilities/artifact-profiles-v1.json")
WEB_REPORT_ROOT = Path("platforms/web/legal/rust-cargo-dependencies")
WEB_ARTIFACT_PROFILE_IDS = (
    "web-analysis",
    "web-ascii",
    "web-editor",
    "web-full",
    "web-render",
)
WEB_TARGET = "wasm32-unknown-unknown"
PYTHON_ARTIFACT_PROFILE_ID = "python-uniffi-native"
PYTHON_TARGET_REPORT_ROOT = Path("platforms/python/legal/rust-cargo-dependencies")
CARGO_ABOUT_VERSION = "0.9.1"
SCHEMA_VERSION = 1
WEB_SCHEMA_VERSION = 2
NATIVE_SCHEMA_VERSION = 3


@dataclass(frozen=True)
class NativeReportSpec:
    bundle_id: str
    output: Path
    profile_ids: tuple[str, ...]
    target_selections: tuple[tuple[str, tuple[str, ...]], ...] = ()


NATIVE_REPORT_SPECS = (
    NativeReportSpec(
        bundle_id="android-native-sdk",
        output=Path(
            "platforms/android/THIRD_PARTY_LICENSES/rust-cargo-dependencies.json"
        ),
        profile_ids=("android-native",),
    ),
    NativeReportSpec(
        bundle_id="apple-native-sdk",
        output=Path(
            "platforms/apple/THIRD_PARTY_LICENSES/rust-cargo-dependencies.json"
        ),
        profile_ids=("apple-uniffi-native",),
    ),
    NativeReportSpec(
        bundle_id="flutter-native-sdk",
        output=Path(
            "platforms/flutter/THIRD_PARTY_LICENSES/rust-cargo-dependencies.json"
        ),
        profile_ids=(
            "flutter-android-native",
            "flutter-desktop-native",
            "flutter-ios-native",
        ),
    ),
    NativeReportSpec(
        bundle_id="python-native-sdk",
        output=Path(
            "platforms/python/merman/THIRD_PARTY_LICENSES/rust-cargo-dependencies.json"
        ),
        profile_ids=(PYTHON_ARTIFACT_PROFILE_ID,),
    ),
)


class RustLicenseReportError(Exception):
    pass


STRICT_JSON = strict_json.StrictJsonContract(
    RustLicenseReportError,
    read_error_prefix="could not read JSON",
)
load_json_strict = STRICT_JSON.load
require_object = STRICT_JSON.object
require_array = STRICT_JSON.array
require_exact_fields = STRICT_JSON.exact_fields
expect_string = STRICT_JSON.string
sha256_json = strict_json.canonical_sha256


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--check", action="store_true")
    mode.add_argument("--write", action="store_true")
    return parser.parse_args(argv)


def generate_report(root: Path) -> bytes:
    verify_cargo_about_version(root)
    return generate_report_for_profile(root, None)


def generate_report_for_profile(
    root: Path,
    artifact_profile: dict[str, Any] | None,
    *,
    target: str | None = None,
) -> bytes:
    normalized = generate_normalized_report_for_profile(
        root,
        artifact_profile,
        target=target,
    )
    return (json.dumps(normalized, indent=2, ensure_ascii=True) + "\n").encode()


def generate_normalized_report_for_profile(
    root: Path,
    artifact_profile: dict[str, Any] | None,
    *,
    target: str | None = None,
) -> dict[str, Any]:
    with tempfile.TemporaryDirectory(prefix="merman-cargo-about-") as temporary:
        raw_path = Path(temporary) / "cargo-about.json"
        command = cargo_about_command(raw_path, artifact_profile, target=target)
        result = subprocess.run(command, cwd=root, text=True, capture_output=True)
        if result.returncode != 0:
            raise RustLicenseReportError(
                "cargo-about failed:\n" + (result.stderr or result.stdout).strip()
            )
        try:
            raw = json.loads(raw_path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as error:
            raise RustLicenseReportError(f"could not read cargo-about output: {error}") from error
    return normalize_report(raw, root, artifact_profile=artifact_profile)


def cargo_about_command(
    raw_path: Path,
    artifact_profile: dict[str, Any] | None,
    *,
    target: str | None = None,
) -> list[str]:
    command = [
        "cargo",
        "about",
        "generate",
        "--config",
        "about.toml",
    ]
    if artifact_profile is None:
        if target is not None:
            raise RustLicenseReportError(
                "workspace-all-features report does not accept an artifact target"
            )
        command.extend(["--workspace", "--all-features"])
    else:
        cargo = artifact_profile["cargo"]
        command.extend(
            [
                "--manifest-path",
                cargo["manifest"],
                "--no-default-features",
                "--features",
                " ".join(cargo["features"]),
            ]
        )
        resolved_target = resolve_report_target(artifact_profile, target)
        if resolved_target is not None:
            command.extend(["--target", resolved_target])
    command.extend(
        [
            "--locked",
            "--offline",
            "--fail",
            "--format",
            "json",
            "--output-file",
            str(raw_path),
        ]
    )
    return command


def resolve_report_target(
    artifact_profile: dict[str, Any],
    requested_target: str | None,
) -> str | None:
    profile_id = expect_string(artifact_profile.get("id"), "artifact profile id")
    cargo = require_object(artifact_profile.get("cargo"), f"artifact profile {profile_id} cargo")
    build_target = require_object(
        cargo.get("build_target"), f"artifact profile {profile_id} build target"
    )
    kind = build_target.get("kind")
    if requested_target is not None:
        target = expect_string(requested_target, f"artifact profile {profile_id} report target")
        if kind == "target-set" and target not in build_target.get("triples", []):
            raise RustLicenseReportError(
                f"artifact profile {profile_id} does not declare report target {target}"
            )
        if kind not in {"host", "target-set"}:
            raise RustLicenseReportError(
                f"artifact profile {profile_id} has unsupported build target kind {kind!r}"
            )
        return target
    if kind == "host":
        return None
    if kind == "target-set":
        triples = build_target.get("triples")
        if isinstance(triples, list) and len(triples) == 1:
            return expect_string(triples[0], f"artifact profile {profile_id} report target")
        raise RustLicenseReportError(
            f"multi-target artifact profile {profile_id} requires an explicit report target"
        )
    raise RustLicenseReportError(
        f"artifact profile {profile_id} has unsupported build target kind {kind!r}"
    )


def load_web_profile_recipes(root: Path) -> dict[str, dict[str, Any]]:
    recipes = load_selected_profile_recipes(
        root,
        WEB_ARTIFACT_PROFILE_IDS,
        semantic_target="web",
    )
    for profile_id, recipe in recipes.items():
        build_target = recipe["cargo"]["build_target"]
        if build_target != {"kind": "target-set", "triples": [WEB_TARGET]}:
            raise RustLicenseReportError(
                f"artifact profile {profile_id} must target only {WEB_TARGET}"
            )
    return recipes


def load_native_profile_recipes(root: Path) -> dict[str, dict[str, Any]]:
    profile_ids = tuple(
        profile_id
        for spec in NATIVE_REPORT_SPECS
        for profile_id in spec.profile_ids
    )
    if len(profile_ids) != len(set(profile_ids)):
        raise RustLicenseReportError("native report specs repeat an artifact profile")
    return load_selected_profile_recipes(
        root,
        profile_ids,
        semantic_target="native",
    )


def load_selected_profile_recipes(
    root: Path,
    profile_ids: tuple[str, ...],
    *,
    semantic_target: str,
) -> dict[str, dict[str, Any]]:
    document = require_object(
        load_json_strict(root / ARTIFACT_PROFILES_PATH),
        "artifact profile authority",
    )
    if document.get("schema_version") != 1:
        raise RustLicenseReportError("artifact profile authority schema_version must be 1")
    profiles = require_array(document.get("profiles"), "artifact profile authority profiles")
    selected: dict[str, dict[str, Any]] = {}
    for index, raw in enumerate(profiles):
        profile = require_object(raw, f"artifact profile[{index}]")
        profile_id = profile.get("id")
        if profile_id not in profile_ids:
            continue
        if profile_id in selected:
            raise RustLicenseReportError(f"artifact profile authority repeats {profile_id}")
        selected[profile_id] = normalize_artifact_profile(
            root,
            profile,
            semantic_target=semantic_target,
        )
    missing = sorted(set(profile_ids) - selected.keys())
    if missing:
        raise RustLicenseReportError(
            "artifact profile authority is missing selected profiles: " + ", ".join(missing)
        )
    return {profile_id: selected[profile_id] for profile_id in profile_ids}


def normalize_artifact_profile(
    root: Path,
    profile: dict[str, Any],
    *,
    semantic_target: str,
) -> dict[str, Any]:
    profile_id = expect_string(profile.get("id"), "artifact profile id")
    if profile.get("semantic_target") != semantic_target:
        raise RustLicenseReportError(
            f"artifact profile {profile_id} must target {semantic_target}"
        )
    cargo = require_object(profile.get("cargo"), f"artifact profile {profile_id} cargo")
    require_exact_fields(
        cargo,
        {
            "package",
            "manifest",
            "profile",
            "default_features",
            "features",
            "target",
            "build_target",
        },
        f"artifact profile {profile_id} cargo",
    )
    package = expect_string(cargo.get("package"), f"artifact profile {profile_id} package")
    manifest = expect_repo_path(cargo.get("manifest"), f"artifact profile {profile_id} manifest")
    cargo_profile = expect_string(cargo.get("profile"), f"artifact profile {profile_id} profile")
    if cargo.get("default_features") is not False:
        raise RustLicenseReportError(
            f"artifact profile {profile_id} must declare default_features=false"
        )
    features = expect_string_array(
        cargo.get("features"), f"artifact profile {profile_id} features"
    )
    if not features or features != sorted(set(features)):
        raise RustLicenseReportError(
            f"artifact profile {profile_id} features must be non-empty, unique, and sorted"
        )
    target = require_object(cargo.get("target"), f"artifact profile {profile_id} target")
    require_exact_fields(
        target,
        {"name", "kinds", "crate_types", "required_features"},
        f"artifact profile {profile_id} target",
    )
    normalized_target = {
        "name": expect_string(target.get("name"), f"artifact profile {profile_id} target name"),
        "kinds": expect_string_array(target.get("kinds"), f"artifact profile {profile_id} target kinds"),
        "crate_types": expect_string_array(
            target.get("crate_types"), f"artifact profile {profile_id} target crate types"
        ),
        "required_features": expect_string_array(
            target.get("required_features"),
            f"artifact profile {profile_id} target required features",
        ),
    }
    build_target = require_object(
        cargo.get("build_target"), f"artifact profile {profile_id} build target"
    )
    kind = build_target.get("kind")
    if kind == "host":
        require_exact_fields(
            build_target,
            {"kind"},
            f"artifact profile {profile_id} build target",
        )
        normalized_build_target = {"kind": "host"}
    elif kind == "target-set":
        require_exact_fields(
            build_target,
            {"kind", "triples"},
            f"artifact profile {profile_id} build target",
        )
        triples = expect_string_array(
            build_target.get("triples"),
            f"artifact profile {profile_id} build target triples",
        )
        if not triples or triples != sorted(set(triples)):
            raise RustLicenseReportError(
                f"artifact profile {profile_id} build target triples must be non-empty, "
                "unique, and sorted"
            )
        normalized_build_target = {"kind": "target-set", "triples": triples}
    else:
        raise RustLicenseReportError(
            f"artifact profile {profile_id} has unsupported build target kind {kind!r}"
        )
    validate_manifest_recipe(root, manifest, package, features, profile_id)
    return {
        "id": profile_id,
        "semantic_target": semantic_target,
        "cargo": {
            "package": package,
            "manifest": manifest,
            "profile": cargo_profile,
            "default_features": False,
            "features": features,
            "target": normalized_target,
            "build_target": normalized_build_target,
        },
    }


def validate_manifest_recipe(
    root: Path,
    manifest: str,
    package: str,
    features: list[str],
    profile_id: str,
) -> None:
    manifest_path = root / manifest
    try:
        document = tomllib.loads(manifest_path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, tomllib.TOMLDecodeError) as error:
        raise RustLicenseReportError(
            f"could not read artifact profile {profile_id} manifest {manifest}: {error}"
        ) from error
    package_table = require_object(document.get("package"), f"manifest {manifest} package")
    if package_table.get("name") != package:
        raise RustLicenseReportError(
            f"artifact profile {profile_id} package does not match {manifest}"
        )
    manifest_features = require_object(document.get("features"), f"manifest {manifest} features")
    missing = sorted(set(features) - manifest_features.keys())
    if missing:
        raise RustLicenseReportError(
            f"artifact profile {profile_id} references missing Cargo features: {', '.join(missing)}"
        )


def verify_cargo_about_version(root: Path) -> None:
    try:
        result = subprocess.run(
            ["cargo", "about", "--version"],
            cwd=root,
            text=True,
            capture_output=True,
            check=True,
        )
    except (OSError, subprocess.CalledProcessError) as error:
        raise RustLicenseReportError(
            f"cargo-about {CARGO_ABOUT_VERSION} is required"
        ) from error
    actual = result.stdout.strip()
    if actual != f"cargo-about {CARGO_ABOUT_VERSION}":
        raise RustLicenseReportError(
            f"cargo-about {CARGO_ABOUT_VERSION} is required, found {actual!r}"
        )


def normalize_report(
    raw: dict[str, Any],
    root: Path,
    *,
    artifact_profile: dict[str, Any] | None = None,
) -> dict[str, Any]:
    licenses: list[dict[str, Any]] = []
    for license_entry in require_list(raw, "licenses"):
        packages = normalized_packages(license_entry)
        if not packages:
            continue
        text = require_string(license_entry, "text")
        licenses.append(
            {
                "id": require_string(license_entry, "id"),
                "name": require_string(license_entry, "name"),
                "text_sha256": hashlib.sha256(text.encode()).hexdigest(),
                "text": text,
                "packages": packages,
            }
        )
    licenses.sort(key=lambda item: (item["id"], item["text_sha256"]))
    if not licenses:
        raise RustLicenseReportError("cargo-about returned no third-party licenses")

    generator = {
        "name": "cargo-about",
        "version": CARGO_ABOUT_VERSION,
        "command_profile": "workspace-all-features-runtime",
        "offline": True,
        "cargo_lock_sha256": sha256_file(root / "Cargo.lock"),
        "configuration_sha256": sha256_file(root / "about.toml"),
    }
    if artifact_profile is None:
        return {
            "schema_version": SCHEMA_VERSION,
            "generator": generator,
            "licenses": licenses,
        }
    generator["command_profile"] = "artifact-profile-runtime"
    generator["artifact_profile_sha256"] = sha256_json(artifact_profile)
    closure = dependency_closure(licenses)
    return {
        "schema_version": WEB_SCHEMA_VERSION,
        "artifact_profile": artifact_profile,
        "generator": generator,
        "dependency_closure": closure,
        "licenses": licenses,
    }


def generate_native_report(
    root: Path,
    spec: NativeReportSpec,
    all_recipes: dict[str, dict[str, Any]],
    *,
    normalized_report_cache: dict[tuple[str, str], dict[str, Any]] | None = None,
) -> bytes:
    recipes = {profile_id: all_recipes[profile_id] for profile_id in spec.profile_ids}
    observations = native_target_observations(spec, recipes)
    cache = {} if normalized_report_cache is None else normalized_report_cache
    observation_licenses: dict[tuple[str, str], list[dict[str, Any]]] = {}
    for observation in observations:
        profile_id = observation["artifact_profile_id"]
        target = observation["target"]
        key = (profile_id, target)
        if key not in cache:
            cache[key] = generate_normalized_report_for_profile(
                root,
                recipes[profile_id],
                target=target,
            )
        normalized = cache[key]
        observation_licenses[(profile_id, target)] = normalized["licenses"]
    report = build_native_report(
        root,
        spec,
        recipes,
        observation_licenses,
    )
    return (json.dumps(report, indent=2, ensure_ascii=True) + "\n").encode()


def generate_native_outputs(
    root: Path,
    specs: tuple[NativeReportSpec, ...],
    all_recipes: dict[str, dict[str, Any]],
) -> dict[Path, bytes]:
    normalized_report_cache: dict[tuple[str, str], dict[str, Any]] = {}
    outputs: dict[Path, bytes] = {}
    for spec in specs:
        outputs[root / spec.output] = generate_native_report(
            root,
            spec,
            all_recipes,
            normalized_report_cache=normalized_report_cache,
        )
    return outputs


def native_target_observations(
    spec: NativeReportSpec,
    recipes: dict[str, dict[str, Any]],
) -> tuple[dict[str, str], ...]:
    selection_pairs = spec.target_selections
    selection_ids = tuple(profile_id for profile_id, _ in selection_pairs)
    if len(selection_ids) != len(set(selection_ids)):
        raise RustLicenseReportError(
            f"native report {spec.bundle_id} repeats a target selection"
        )
    unknown_selections = sorted(set(selection_ids) - set(spec.profile_ids))
    if unknown_selections:
        raise RustLicenseReportError(
            f"native report {spec.bundle_id} has target selections for unknown profiles: "
            + ", ".join(unknown_selections)
        )
    selections = dict(selection_pairs)
    observations: list[dict[str, str]] = []
    for profile_id in spec.profile_ids:
        recipe = recipes[profile_id]
        build_target = recipe["cargo"]["build_target"]
        kind = build_target["kind"]
        if kind != "target-set":
            raise RustLicenseReportError(
                f"native report {spec.bundle_id} requires descriptor-owned target-set "
                f"profile {profile_id}; found {kind!r}"
            )
        descriptor_targets = tuple(build_target["triples"])
        targets = selections.get(profile_id, descriptor_targets)
        if targets != tuple(sorted(set(targets))):
            raise RustLicenseReportError(
                f"native report {spec.bundle_id} targets for {profile_id} must be unique "
                "and sorted"
            )
        if not targets or not set(targets).issubset(descriptor_targets):
            raise RustLicenseReportError(
                f"native report {spec.bundle_id} targets for {profile_id} must be a "
                "non-empty subset of its descriptor-owned targets"
            )
        observations.extend(
            {"artifact_profile_id": profile_id, "target": target}
            for target in targets
        )
    return tuple(observations)


def python_target_report_specs(
    recipes: dict[str, dict[str, Any]],
) -> tuple[NativeReportSpec, ...]:
    profile = recipes[PYTHON_ARTIFACT_PROFILE_ID]
    build_target = profile["cargo"]["build_target"]
    if build_target.get("kind") != "target-set":
        raise RustLicenseReportError(
            f"{PYTHON_ARTIFACT_PROFILE_ID} must own the finite published wheel target set"
        )
    return tuple(
        NativeReportSpec(
            bundle_id=f"python-wheel-{target}",
            output=PYTHON_TARGET_REPORT_ROOT / f"{target}.json",
            profile_ids=(PYTHON_ARTIFACT_PROFILE_ID,),
            target_selections=((PYTHON_ARTIFACT_PROFILE_ID, (target,)),),
        )
        for target in build_target["triples"]
    )


def build_native_report(
    root: Path,
    spec: NativeReportSpec,
    recipes: dict[str, dict[str, Any]],
    observation_licenses: dict[tuple[str, str], list[dict[str, Any]]],
) -> dict[str, Any]:
    observations = native_target_observations(spec, recipes)
    observation_keys = tuple(
        (observation["artifact_profile_id"], observation["target"])
        for observation in observations
    )
    if set(observation_licenses) != set(observation_keys):
        missing = sorted(set(observation_keys) - set(observation_licenses))
        unexpected = sorted(set(observation_licenses) - set(observation_keys))
        details: list[str] = []
        if missing:
            details.append(f"missing={missing!r}")
        if unexpected:
            details.append(f"unexpected={unexpected!r}")
        raise RustLicenseReportError(
            f"native report {spec.bundle_id} observation reports do not match its targets: "
            + " ".join(details)
        )

    artifact_bundle = {
        "id": spec.bundle_id,
        "artifact_profiles": [recipes[profile_id] for profile_id in spec.profile_ids],
        "target_observations": list(observations),
    }
    target_closures = []
    for profile_id, target in observation_keys:
        target_closures.append(
            {
                "artifact_profile_id": profile_id,
                "target": target,
                **dependency_closure(observation_licenses[(profile_id, target)]),
            }
        )
    licenses = merge_license_entries(
        [observation_licenses[key] for key in observation_keys]
    )
    generator = {
        "name": "cargo-about",
        "version": CARGO_ABOUT_VERSION,
        "command_profile": (
            "artifact-profile-target"
            if len(observations) == 1
            else "artifact-profile-target-union"
        ),
        "offline": True,
        "cargo_lock_sha256": sha256_file(root / "Cargo.lock"),
        "configuration_sha256": sha256_file(root / "about.toml"),
        "artifact_bundle_sha256": sha256_json(artifact_bundle),
    }
    return {
        "schema_version": NATIVE_SCHEMA_VERSION,
        "artifact_bundle": artifact_bundle,
        "generator": generator,
        "target_dependency_closures": target_closures,
        "dependency_closure": dependency_closure(licenses),
        "licenses": licenses,
    }


def merge_license_entries(
    reports: list[list[dict[str, Any]]],
) -> list[dict[str, Any]]:
    merged: dict[tuple[str, str, str, str], dict[str, Any]] = {}
    packages_by_license: dict[
        tuple[str, str, str, str],
        dict[tuple[str, str, str], dict[str, Any]],
    ] = {}
    for licenses in reports:
        for entry in licenses:
            key = (
                entry["id"],
                entry["name"],
                entry["text_sha256"],
                entry["text"],
            )
            if key not in merged:
                merged[key] = {
                    "id": entry["id"],
                    "name": entry["name"],
                    "text_sha256": entry["text_sha256"],
                    "text": entry["text"],
                    "packages": [],
                }
                packages_by_license[key] = {}
            packages = packages_by_license[key]
            for package in entry["packages"]:
                package_key = (
                    package["name"],
                    package["version"],
                    package["source"],
                )
                existing = packages.get(package_key)
                if existing is not None and existing != package:
                    raise RustLicenseReportError(
                        "cargo-about returned conflicting metadata for "
                        f"{package['name']}@{package['version']}"
                    )
                packages[package_key] = package
    for key, entry in merged.items():
        packages = packages_by_license[key]
        entry["packages"] = [packages[package_key] for package_key in sorted(packages)]
    ordered_keys = sorted(merged, key=lambda key: (key[0], key[2], key[1], key[3]))
    result = [merged[key] for key in ordered_keys]
    if not result:
        raise RustLicenseReportError("native cargo-about reports contain no licenses")
    return result


def dependency_closure(licenses: list[dict[str, Any]]) -> dict[str, Any]:
    packages: dict[tuple[str, str, str], dict[str, Any]] = {}
    for license_entry in licenses:
        for package in license_entry["packages"]:
            key = (package["name"], package["version"], package["source"])
            existing = packages.get(key)
            if existing is not None and existing != package:
                raise RustLicenseReportError(
                    "cargo-about returned conflicting metadata for "
                    f"{package['name']}@{package['version']}"
                )
            packages[key] = package
    ordered = [packages[key] for key in sorted(packages)]
    encoded = json.dumps(
        ordered,
        ensure_ascii=True,
        separators=(",", ":"),
        sort_keys=True,
    ).encode()
    return {
        "package_count": len(ordered),
        "packages_sha256": hashlib.sha256(encoded).hexdigest(),
    }


def normalized_packages(license_entry: dict[str, Any]) -> list[dict[str, Any]]:
    packages: dict[tuple[str, str, str], dict[str, Any]] = {}
    for use in require_list(license_entry, "used_by"):
        package = use.get("crate")
        if not isinstance(package, dict):
            raise RustLicenseReportError("cargo-about used_by entry has no crate metadata")
        source = package.get("source")
        if not isinstance(source, str):
            continue
        name = require_string(package, "name")
        version = require_string(package, "version")
        key = (name, version, source)
        packages[key] = {
            "name": name,
            "version": version,
            "source": source,
            "license_expression": package.get("license"),
            "authors": sorted(
                author for author in package.get("authors", []) if isinstance(author, str)
            ),
            "repository": package.get("repository"),
        }
    return [packages[key] for key in sorted(packages)]


def require_list(value: dict[str, Any], key: str) -> list[Any]:
    result = value.get(key)
    if not isinstance(result, list):
        raise RustLicenseReportError(f"cargo-about output field {key!r} is not a list")
    return result


def require_string(value: dict[str, Any], key: str) -> str:
    result = value.get(key)
    if not isinstance(result, str) or not result:
        raise RustLicenseReportError(f"cargo-about output field {key!r} is not a string")
    return result


def expect_string_array(value: Any, context: str) -> list[str]:
    return [
        expect_string(item, f"{context}[{index}]")
        for index, item in enumerate(require_array(value, context))
    ]


def expect_repo_path(value: Any, context: str) -> str:
    raw = expect_string(value, context)
    path = PurePosixPath(raw)
    if (
        path.is_absolute()
        or bool(PureWindowsPath(raw).drive)
        or "\\" in raw
        or path.as_posix() != raw
        or not path.parts
        or any(part in {"", ".", ".."} for part in path.parts)
    ):
        raise RustLicenseReportError(f"{context} must be a normalized repository-relative path")
    return raw


def sha256_file(path: Path) -> str:
    if not path.is_file():
        raise RustLicenseReportError(f"missing report input: {path}")
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv or sys.argv[1:])
    try:
        verify_cargo_about_version(ROOT)
        web_recipes = load_web_profile_recipes(ROOT)
        native_recipes = load_native_profile_recipes(ROOT)
        native_specs = (*NATIVE_REPORT_SPECS, *python_target_report_specs(native_recipes))
        generated_outputs = {
            OUTPUT: generate_report_for_profile(ROOT, None),
            **{
                ROOT / WEB_REPORT_ROOT / f"{profile_id}.json": generate_report_for_profile(
                    ROOT, recipe
                )
                for profile_id, recipe in web_recipes.items()
            },
        }
        generated_outputs.update(
            generate_native_outputs(
                ROOT,
                native_specs,
                native_recipes,
            )
        )
        if args.write:
            for output, generated in generated_outputs.items():
                output.parent.mkdir(parents=True, exist_ok=True)
                output.write_bytes(generated)
        stale = [
            output.relative_to(ROOT).as_posix()
            for output, generated in generated_outputs.items()
            if not output.is_file() or output.read_bytes() != generated
        ]
        if stale:
            raise RustLicenseReportError(
                "Rust dependency license reports are stale or missing: "
                + ", ".join(stale)
                + "; run "
                "`python3 scripts/generate-rust-license-report.py --write`"
            )
    except (OSError, RustLicenseReportError) as error:
        print(f"Rust dependency license report failed: {error}", file=sys.stderr)
        return 1
    total_bytes = sum(len(generated) for generated in generated_outputs.values())
    print(
        "Rust dependency license reports: ok "
        f"({len(generated_outputs)} reports, {total_bytes} bytes)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
