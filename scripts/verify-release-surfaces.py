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
    "platforms/web/pkg/full/package.json",
}
REQUIRED_SURFACE_DOCS = [
    "docs/release/PACKAGE_SURFACES.md",
    "docs/release/RELEASING.md",
    "docs/release/ADDING_SURFACE.md",
    "docs/release/MERMAID_UPGRADE_PLAYBOOK.md",
    "docs/security/RENDERING_SECURITY.md",
]
REQUIRED_WEB_DOC_SUBPATHS = [
    "@mermanjs/web/core",
    "@mermanjs/web/render",
    "@mermanjs/web/render-only",
    "@mermanjs/web/ascii",
    "@mermanjs/web/full",
]
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

    presets = extract_browser_presets(read_text(root, "platforms/web/scripts/build-wasm.mjs"))
    expected_presets = set(feature_contract["browser_presets"])
    if presets != expected_presets:
        fail(
            "platforms/web/scripts/build-wasm.mjs",
            "browser preset mismatch: expected "
            + ", ".join(sorted(expected_presets))
            + "; found "
            + ", ".join(sorted(presets)),
        )

    wrappers = extract_wrapper_surfaces(read_text(root, "platforms/web/scripts/surface-manifest.mjs"))
    wrapper_subpaths = {"."} | {f"./{entry}" for entry, _preset in wrappers}
    if wrapper_subpaths != expected_subpaths:
        fail(
            "platforms/web/scripts/surface-manifest.mjs",
            "wrapper subpaths do not match package surface contract: "
            + ", ".join(sorted(wrapper_subpaths)),
        )
    wrapper_presets = {preset for _entry, preset in wrappers}
    required_wrapped_presets = expected_presets - {"browser-full-no-elk", "browser-ratex-math"}
    if wrapper_presets != required_wrapped_presets:
        fail(
            "platforms/web/scripts/surface-manifest.mjs",
            "wrapper presets should cover shipped subpaths only: "
            + ", ".join(sorted(wrapper_presets)),
        )

    wasm_features = cargo_features(root, "crates/merman-wasm/Cargo.toml")
    for feature in ["core-full", "core-host", "analysis", "ascii", "render", "cytoscape-layout", "elk-layout", "editor-language", "ratex-math"]:
        if feature not in wasm_features:
            fail("crates/merman-wasm/Cargo.toml", f"missing wasm feature {feature}")

    web_docs = "\n".join(
        [
            read_text(root, "README.md"),
            read_text(root, "platforms/web/README.md"),
            read_text(root, "docs/release/PACKAGE_SURFACES.md"),
        ]
    )
    for term in REQUIRED_WEB_DOC_SUBPATHS:
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


def extract_browser_presets(text: str) -> set[str]:
    return set(re.findall(r'^\s+"(browser-[^"]+)":\s+\{', text, flags=re.MULTILINE))


def extract_wrapper_surfaces(text: str) -> set[tuple[str, str]]:
    return set(re.findall(r'entry:\s*"([^"]+)".*?preset:\s*"([^"]+)"', text, flags=re.DOTALL))


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
