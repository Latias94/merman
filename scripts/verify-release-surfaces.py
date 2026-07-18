#!/usr/bin/env python3
"""Verify release surface metadata against repository facts."""

from __future__ import annotations

import argparse
import importlib.util
import json
import os
import re
import sys
import tomllib
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
SURFACES_PATH = ROOT / "docs" / "release" / "SURFACES.json"
REQUIRED_NON_SURFACE_PACKAGE_MANIFESTS = {
    "playground/package.json",
    "tools/mermaid-cli/package.json",
}
OPTIONAL_NON_SURFACE_PACKAGE_MANIFESTS = {
    "package.json",
}
NON_SURFACE_PACKAGE_MANIFESTS = REQUIRED_NON_SURFACE_PACKAGE_MANIFESTS | OPTIONAL_NON_SURFACE_PACKAGE_MANIFESTS
WEB_GENERATED_PACKAGE_MANIFESTS = {
    "platforms/web/pkg/package.json",
    "platforms/web/pkg/core/package.json",
    "platforms/web/pkg/render/package.json",
    "platforms/web/pkg/render-only/package.json",
    "platforms/web/pkg/ascii/package.json",
    "platforms/web/pkg/editor/package.json",
    "platforms/web/pkg/full/package.json",
    "platforms/web/pkg/full-no-elk/package.json",
    "platforms/web/pkg/ratex-math/package.json",
}
REQUIRED_SURFACE_DOCS = [
    "docs/release/PACKAGE_SURFACES.md",
    "docs/release/RELEASING.md",
    "docs/release/ADDING_SURFACE.md",
    "docs/release/MERMAID_UPGRADE_PLAYBOOK.md",
    "docs/security/RENDERING_SECURITY.md",
]
WEB_SURFACE_DESCRIPTOR_SCHEMA_VERSION = 1
WEB_SURFACE_DESCRIPTOR_PATH = "platforms/web/web-surface-descriptor.json"
WEB_CAPABILITY_NAMES = {
    "render",
    "analysis",
    "ascii",
    "core_full",
    "core_host",
    "elk_layout",
    "ratex_math",
    "editor_language",
}
WEB_RUNTIME_PROFILES = {"core", "render", "render-only", "ascii", "editor", "full"}
EVIDENCE_ONLY_WEB_PRESETS = {"browser-full-no-elk", "browser-ratex-math"}
FORBIDDEN_WEB_SUBPATHS = [
    "@mermanjs/web/analysis",
    '"./analysis"',
]
REQUIRED_FEATURE_DOC_TERMS = [
    "editor-language",
    "ratex-math",
    "cytoscape-layout",
    "browser-core",
    "browser-render",
    "browser-render-only",
    "browser-ascii",
    "browser-editor",
    "browser-full",
    "browser-full-no-elk",
    "browser-ratex-math",
]


class CheckFailure(Exception):
    pass


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--contract", type=Path, default=SURFACES_PATH)
    parser.add_argument(
        "--check-ci-self",
        action=argparse.BooleanOptionalAction,
        default=True,
        help="Require CI to run this verifier and its unit tests.",
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv or sys.argv[1:])
    root = ROOT
    failures: list[str] = []

    try:
        contract = load_contract(args.contract)
    except CheckFailure as error:
        print(f"::error file={rel(args.contract, root)}::{error}", file=sys.stderr)
        return 1

    checks = [
        ("surface contract paths", lambda: check_surface_paths(root, contract)),
        ("package manifest names", lambda: check_package_manifest_names(root, contract)),
        ("package manifest inventory", lambda: check_package_inventory(root, contract)),
        ("web package contract", lambda: check_web_contract(root, contract)),
        ("release docs contract", lambda: check_release_docs(root, contract)),
        ("host text measurement docs", lambda: check_host_text_measurement_docs(root)),
        ("blocked channel metadata", lambda: check_blocked_channel_metadata(contract)),
    ]
    if args.check_ci_self:
        checks.append(("CI wiring", lambda: check_ci_wiring(root)))

    for label, check in checks:
        try:
            check()
            print(f"{label}: ok")
        except CheckFailure as error:
            failures.append(str(error))

    if failures:
        for failure in failures:
            print(failure, file=sys.stderr)
        return 1
    return 0


def load_contract(path: Path) -> dict[str, Any]:
    if not path.exists():
        raise CheckFailure(f"missing surface contract: {path}")

    data = json.loads(path.read_text(encoding="utf-8"))
    release_status = load_release_status_module()
    try:
        release_status.validate_contract(data)
    except release_status.SurfaceError as error:
        raise CheckFailure(str(error)) from error
    return data


def load_release_status_module() -> Any:
    module_path = ROOT / "scripts" / "release-status.py"
    spec = importlib.util.spec_from_file_location("release_status", module_path)
    if spec is None or spec.loader is None:
        raise CheckFailure("could not load scripts/release-status.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def check_surface_paths(root: Path, contract: dict[str, Any]) -> None:
    require_path(root, "docs/release/SURFACES.json")
    for doc in REQUIRED_SURFACE_DOCS:
        require_path(root, doc)

    for doc in contract.get("feature_contract", {}).get("docs", []):
        require_path(root, doc)

    for surface in contract["surfaces"]:
        for doc in surface.get("docs", []):
            require_path(root, doc)
        for package in surface.get("packages", []):
            require_path(root, package["manifest"])
        for channel in surface.get("channels", []):
            workflow = channel.get("workflow")
            if workflow:
                require_path(root, workflow)


def check_package_manifest_names(root: Path, contract: dict[str, Any]) -> None:
    for surface in contract["surfaces"]:
        for package in surface.get("packages", []):
            kind = package["kind"]
            name = package["name"]
            manifest = package["manifest"]
            actual = package_manifest_name(root, kind, manifest)
            if actual != name:
                fail(manifest, f"{kind} package name is {actual!r}, expected {name!r}")


def check_package_inventory(root: Path, contract: dict[str, Any]) -> None:
    declared_manifests = {
        normalize_rel(package["manifest"])
        for surface in contract["surfaces"]
        for package in surface.get("packages", [])
    }
    package_jsons = {normalize_rel(path.relative_to(root)) for path in iter_package_jsons(root)}
    undeclared = sorted(
        package_jsons
        - declared_manifests
        - NON_SURFACE_PACKAGE_MANIFESTS
        - WEB_GENERATED_PACKAGE_MANIFESTS
    )
    if undeclared:
        fail(
            "docs/release/SURFACES.json",
            "package.json manifests are neither release surfaces nor allowlisted non-surfaces: "
            + ", ".join(undeclared),
        )

    for rel_path in sorted(REQUIRED_NON_SURFACE_PACKAGE_MANIFESTS):
        manifest = root / rel_path
        if not manifest.exists():
            fail(rel_path, "allowlisted non-surface package manifest is missing")

    for rel_path in sorted(NON_SURFACE_PACKAGE_MANIFESTS):
        manifest = root / rel_path
        if manifest.exists() and rel_path != "package.json":
            data = json.loads(manifest.read_text(encoding="utf-8"))
            if data.get("private") is not True:
                fail(rel_path, "non-surface package manifest must set private: true")


def iter_package_jsons(root: Path) -> list[Path]:
    ignored_dirs = {
        ".git",
        ".github",
        ".gradle",
        ".pytest_cache",
        "coverage",
        "dist",
        "node_modules",
        "repo-ref",
        "target",
    }
    manifests: list[Path] = []
    for current, dirs, files in os.walk(root):
        dirs[:] = [name for name in dirs if name not in ignored_dirs]
        if "package.json" in files:
            manifests.append(Path(current) / "package.json")
    return manifests


def check_web_contract(root: Path, contract: dict[str, Any]) -> None:
    feature_contract = contract["feature_contract"]
    descriptor = load_web_surface_descriptor(root, feature_contract)
    web_package = read_json(root, "platforms/web/package.json")
    exports = set(web_package.get("exports", {}))
    expected_subpaths = set(feature_contract["web_subpaths"])
    if expected_subpaths - exports:
        fail(
            "platforms/web/package.json",
            "missing public web exports: " + ", ".join(sorted(expected_subpaths - exports)),
        )
    if "./analysis" in exports:
        fail("platforms/web/package.json", "@mermanjs/web/analysis is not a supported export")

    expected_presets = set(feature_contract["browser_presets"])
    descriptor_presets = {preset["name"] for preset in descriptor["presets"]}
    if descriptor_presets != expected_presets:
        fail(
            feature_contract["web_descriptor"],
            "browser preset mismatch: expected "
            + ", ".join(sorted(expected_presets))
            + "; found "
            + ", ".join(sorted(descriptor_presets)),
        )

    public_surfaces = descriptor["public_surfaces"]
    descriptor_subpaths = {"."} | {f"./{surface['entry']}" for surface in public_surfaces}
    if descriptor_subpaths != expected_subpaths:
        fail(
            feature_contract["web_descriptor"],
            "descriptor subpaths do not match package surface contract: "
            + ", ".join(sorted(descriptor_subpaths)),
        )
    public_presets = {surface["preset"] for surface in public_surfaces}
    required_public_presets = expected_presets - EVIDENCE_ONLY_WEB_PRESETS
    if public_presets != required_public_presets:
        fail(
            feature_contract["web_descriptor"],
            "public surfaces should cover shipped presets only: "
            + ", ".join(sorted(public_presets)),
        )
    expected_default = feature_contract["web_default_preset"]
    if descriptor["default_preset"] != expected_default:
        fail(
            feature_contract["web_descriptor"],
            f"default preset is {descriptor['default_preset']!r}, expected {expected_default!r}",
        )

    wasm_features = cargo_features(root, "crates/merman-wasm/Cargo.toml")
    for feature in ["core-full", "core-host", "analysis", "ascii", "render", "cytoscape-layout", "elk-layout", "editor-language", "ratex-math"]:
        if feature not in wasm_features:
            fail("crates/merman-wasm/Cargo.toml", f"missing wasm feature {feature}")
    for preset in descriptor["presets"]:
        for feature in preset["features"]:
            if feature not in wasm_features:
                fail(
                    feature_contract["web_descriptor"],
                    f"preset {preset['name']} references missing wasm feature {feature}",
                )

    web_docs = "\n".join(
        [
            read_text(root, "README.md"),
            read_text(root, "platforms/web/README.md"),
            read_text(root, "docs/release/PACKAGE_SURFACES.md"),
        ]
    )
    for surface in public_surfaces:
        term = f"@mermanjs/web/{surface['entry']}"
        if term not in web_docs:
            fail("docs/release/PACKAGE_SURFACES.md", f"missing web subpath docs for {term}")
    if "@mermanjs/web/analysis" in web_docs and "no `@mermanjs/web/analysis`" not in web_docs:
        fail("docs/release/PACKAGE_SURFACES.md", "analysis must be documented as absent, not as a package")

    for forbidden in FORBIDDEN_WEB_SUBPATHS:
        if forbidden in read_text(root, "platforms/web/package.json"):
            fail("platforms/web/package.json", f"forbidden web subpath appears: {forbidden}")


def check_release_docs(root: Path, contract: dict[str, Any]) -> None:
    package_surfaces = read_text(root, "docs/release/PACKAGE_SURFACES.md")
    releasing = read_text(root, "docs/release/RELEASING.md")
    features = read_text(root, "docs/FEATURES.md")
    readme = read_text(root, "README.md")

    for state in contract["states"]:
        if state not in package_surfaces + releasing:
            fail("docs/release/PACKAGE_SURFACES.md", f"release state {state} is not documented")

    for surface in contract["surfaces"]:
        if surface["entry_point"] not in package_surfaces + readme:
            fail("docs/release/PACKAGE_SURFACES.md", f"missing entry point {surface['entry_point']}")

    for term in REQUIRED_FEATURE_DOC_TERMS:
        if term not in features + readme + package_surfaces:
            fail("docs/FEATURES.md", f"missing feature or preset name {term}")

    for command in [
        "scripts/release-status.py",
        "scripts/verify-release-surfaces.py",
    ]:
        if command not in releasing + package_surfaces:
            fail("docs/release/RELEASING.md", f"missing release helper command {command}")


def check_host_text_measurement_docs(root: Path) -> None:
    readme = read_text(root, "README.md")
    stale = "This surface does not expose host text-measurement callbacks yet"
    if stale in readme:
        fail("README.md", "Python row still says host text measurement is not exposed")

    for rel_path in [
        "README.md",
        "docs/bindings/HOST_TEXT_MEASUREMENT.md",
        "docs/bindings/PYTHON_UNIFFI.md",
        "platforms/python/merman/README.md",
    ]:
        text = read_text(root, rel_path)
        for token in ["MermanTextMeasurer", "reusable_engine_with_text_measurer"]:
            if token not in text:
                fail(rel_path, f"missing host text measurement token {token}")


def check_blocked_channel_metadata(contract: dict[str, Any]) -> None:
    for surface in contract["surfaces"]:
        for channel in surface.get("channels", []):
            state = channel["declared_state"]
            owner = f"docs/release/SURFACES.json:{surface['id']}/{channel['id']}"
            if state == "credential-blocked" and not channel.get("credential"):
                fail(owner, "credential-blocked channels must name the missing credential")
            if state in {"credential-blocked", "registry-blocked", "manual-registry"} and not channel.get("blocker"):
                fail(owner, f"{state} channels must explain the blocker")
            if state == "not-applicable" and not channel.get("not_applicable_reason"):
                fail(owner, "not-applicable channels must explain why")


def check_ci_wiring(root: Path) -> None:
    ci = read_text(root, ".github/workflows/ci.yml")
    for token in [
        "scripts/test_release_status.py",
        "scripts/test_verify_release_surfaces.py",
        "scripts/verify-release-surfaces.py",
    ]:
        if token not in ci:
            fail(".github/workflows/ci.yml", f"CI does not run {token}")


def package_manifest_name(root: Path, kind: str, manifest: str) -> str:
    path = root / manifest
    if kind in {"npm", "vscode"}:
        return read_json(root, manifest)["name"]
    if kind == "crate":
        data = tomllib.loads(path.read_text(encoding="utf-8"))
        return data["package"]["name"]
    if kind == "python":
        data = tomllib.loads(path.read_text(encoding="utf-8"))
        return data["project"]["name"]
    if kind == "flutter":
        return require_regex(manifest, path.read_text(encoding="utf-8"), r"^name:\s*([^\s#]+)")
    if kind == "typst":
        data = tomllib.loads(path.read_text(encoding="utf-8"))
        return data["package"]["name"]
    if kind == "android":
        text = path.read_text(encoding="utf-8")
        group = require_regex(manifest, text, r"\bgroup\s*=\s*\"([^\"]+)\"")
        artifact = require_regex(manifest, text, r"\bartifactId\s*=\s*\"([^\"]+)\"")
        return f"{group}:{artifact}"
    if kind == "swiftpm":
        text = path.read_text(encoding="utf-8")
        return require_regex(manifest, text, r"name:\s*\"([^\"]+)\"")
    raise CheckFailure(f"unsupported package kind {kind!r} in {manifest}")


def cargo_features(root: Path, manifest: str) -> set[str]:
    data = tomllib.loads((root / manifest).read_text(encoding="utf-8"))
    return set(data.get("features", {}))


def load_web_surface_descriptor(
    root: Path,
    feature_contract: dict[str, Any],
) -> dict[str, Any]:
    rel_path = feature_contract.get("web_descriptor")
    if not isinstance(rel_path, str) or not rel_path:
        fail("docs/release/SURFACES.json", "feature_contract.web_descriptor is required")
    if rel_path != WEB_SURFACE_DESCRIPTOR_PATH:
        fail(
            "docs/release/SURFACES.json",
            f"web_descriptor must be {WEB_SURFACE_DESCRIPTOR_PATH}",
        )
    require_path(root, rel_path)
    return validate_web_surface_descriptor(read_json(root, rel_path), rel_path)


def validate_web_surface_descriptor(
    descriptor: dict[str, Any],
    rel_path: str = WEB_SURFACE_DESCRIPTOR_PATH,
) -> dict[str, Any]:
    require_exact_keys(
        descriptor,
        {"schema_version", "default_preset", "presets", "public_surfaces"},
        rel_path,
        "Web surface descriptor",
    )
    if descriptor["schema_version"] != WEB_SURFACE_DESCRIPTOR_SCHEMA_VERSION:
        fail(
            rel_path,
            f"Web surface descriptor schema must be {WEB_SURFACE_DESCRIPTOR_SCHEMA_VERSION}",
        )

    presets = descriptor["presets"]
    if not isinstance(presets, list) or not presets:
        fail(rel_path, "Web surface descriptor presets must be a non-empty array")
    preset_names: set[str] = set()
    for index, preset in enumerate(presets):
        label = f"presets[{index}]"
        require_exact_keys(
            preset,
            {"name", "surface", "default_features", "features", "capabilities"},
            rel_path,
            label,
        )
        name = require_web_name(preset["name"], rel_path, f"{label}.name")
        if name in preset_names:
            fail(rel_path, f"duplicate Web preset name: {name}")
        preset_names.add(name)
        if preset["surface"] != "browser":
            fail(rel_path, f"preset {name} must declare surface browser")
        if not isinstance(preset["default_features"], bool):
            fail(rel_path, f"preset {name} default_features must be boolean")
        features = preset["features"]
        if not isinstance(features, list):
            fail(rel_path, f"preset {name} features must be an array")
        normalized_features = [
            require_web_name(feature, rel_path, f"preset {name} feature")
            for feature in features
        ]
        if len(set(normalized_features)) != len(normalized_features):
            fail(rel_path, f"preset {name} contains duplicate features")
        capabilities = preset["capabilities"]
        require_exact_keys(
            capabilities,
            WEB_CAPABILITY_NAMES,
            rel_path,
            f"preset {name} capabilities",
        )
        for capability, enabled in capabilities.items():
            if not isinstance(enabled, bool):
                fail(rel_path, f"preset {name} capability {capability} must be boolean")

    default_preset = require_web_name(
        descriptor["default_preset"],
        rel_path,
        "default_preset",
    )
    if default_preset not in preset_names:
        fail(rel_path, f"default_preset references unknown preset {default_preset}")

    public_surfaces = descriptor["public_surfaces"]
    if not isinstance(public_surfaces, list) or not public_surfaces:
        fail(rel_path, "public_surfaces must be a non-empty array")
    entries: set[str] = set()
    public_presets: set[str] = set()
    package_dirs: set[str] = set()
    for index, surface in enumerate(public_surfaces):
        label = f"public_surfaces[{index}]"
        require_exact_keys(
            surface,
            {"entry", "preset", "pkg_dir_rel", "runtime_profile"},
            rel_path,
            label,
        )
        entry = require_web_name(surface["entry"], rel_path, f"{label}.entry")
        preset = require_web_name(surface["preset"], rel_path, f"surface {entry} preset")
        package_dir = surface["pkg_dir_rel"]
        if not isinstance(package_dir, str) or not re.fullmatch(
            r"pkg/[a-z0-9][a-z0-9-]*",
            package_dir,
        ):
            fail(rel_path, f"surface {entry} pkg_dir_rel must be a package-relative directory")
        runtime_profile = require_web_name(
            surface["runtime_profile"],
            rel_path,
            f"surface {entry} runtime_profile",
        )
        if entry in entries:
            fail(rel_path, f"duplicate public Web surface entry: {entry}")
        if preset in public_presets:
            fail(rel_path, f"duplicate public Web surface preset: {preset}")
        if package_dir in package_dirs:
            fail(rel_path, f"duplicate public Web package directory: {package_dir}")
        entries.add(entry)
        public_presets.add(preset)
        package_dirs.add(package_dir)
        if preset not in preset_names:
            fail(rel_path, f"public surface {entry} references unknown preset {preset}")
        if package_dir != f"pkg/{entry}":
            fail(rel_path, f"public surface {entry} pkg_dir_rel must be pkg/{entry}")
        if runtime_profile not in WEB_RUNTIME_PROFILES:
            fail(
                rel_path,
                f"public surface {entry} has unknown runtime profile {runtime_profile}",
            )

    return descriptor


def require_exact_keys(
    value: Any,
    expected: set[str],
    rel_path: str,
    label: str,
) -> None:
    if not isinstance(value, dict):
        fail(rel_path, f"{label} must be an object")
    actual = set(value)
    if actual != expected:
        fail(
            rel_path,
            f"{label} keys must be exactly: {', '.join(sorted(expected))}",
        )


def require_web_name(value: Any, rel_path: str, label: str) -> str:
    if not isinstance(value, str) or not re.fullmatch(r"[a-z0-9][a-z0-9-]*", value):
        fail(rel_path, f"{label} must be a lowercase kebab-case name")
    return value


def require_path(root: Path, rel_path: str) -> None:
    if not (root / rel_path).exists():
        fail(rel_path, "required release surface path is missing")


def read_text(root: Path, rel_path: str) -> str:
    return (root / rel_path).read_text(encoding="utf-8")


def read_json(root: Path, rel_path: str) -> dict[str, Any]:
    return json.loads(read_text(root, rel_path))


def require_regex(rel_path: str, text: str, pattern: str) -> str:
    match = re.search(pattern, text, flags=re.MULTILINE)
    if not match:
        fail(rel_path, f"missing pattern {pattern}")
    return match.group(1)


def fail(path: str | Path, message: str) -> None:
    normalized = normalize_rel(path)
    raise CheckFailure(f"::error file={normalized}::{message}")


def rel(path: Path, root: Path) -> str:
    try:
        return normalize_rel(path.relative_to(root))
    except ValueError:
        return normalize_rel(path)


def normalize_rel(path: str | Path) -> str:
    return str(path).replace("\\", "/")


if __name__ == "__main__":
    raise SystemExit(main())
