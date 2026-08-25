#!/usr/bin/env python3
"""Project canonical legal material into every binary release surface."""

from __future__ import annotations

import argparse
import json
import shutil
import sys
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
CANONICAL_LICENSE_DIRECTORY = "THIRD_PARTY_LICENSES"
CANONICAL_NOTICE = "THIRD_PARTY_NOTICES.md"
PROJECT_LICENSE_TARGETS = (
    "platforms/web/LICENSE",
    "platforms/python/merman/LICENSE",
    "platforms/flutter/LICENSE",
    "platforms/android/LICENSE",
    "platforms/apple/LICENSE",
    "distribution/typst/merman/LICENSE",
    "tools/vscode-extension/LICENSE",
    "playground/public/LICENSE",
)
RELEASE_BUNDLE_ROOTS = (
    "platforms/web",
    "platforms/python/merman",
    "platforms/flutter",
    "platforms/android",
    "platforms/apple",
    "distribution/typst/merman",
    "tools/vscode-extension",
    "playground/public",
)
ARTIFACT_SCOPE_BY_BUNDLE = {
    "distribution/typst/merman": "typst-publish",
}
ANDROID_META_INF = "platforms/android/src/main/resources/META-INF"
RUST_DEPENDENCY_REPORT = Path("rust-cargo-dependencies.json")
NATIVE_RUST_REPORTS = {
    "platforms/python/merman": Path(
        "platforms/python/merman/THIRD_PARTY_LICENSES/rust-cargo-dependencies.json"
    ),
    "platforms/flutter": Path(
        "platforms/flutter/THIRD_PARTY_LICENSES/rust-cargo-dependencies.json"
    ),
    "platforms/android": Path(
        "platforms/android/THIRD_PARTY_LICENSES/rust-cargo-dependencies.json"
    ),
    "platforms/apple": Path(
        "platforms/apple/THIRD_PARTY_LICENSES/rust-cargo-dependencies.json"
    ),
}
EXTERNALLY_MANAGED_PROJECTIONS = (
    "playground/public/THIRD_PARTY_LICENSES/npm-production-dependencies.txt",
    "tools/vscode-extension/THIRD_PARTY_LICENSES/npm-production-dependencies.txt",
)
DUAL_LICENSE_CRATE_ROOTS = (
    "crates/dugong",
    "crates/dugong-graphlib",
    "crates/manatee",
    "crates/merman",
    "crates/merman-analysis",
    "crates/merman-ascii",
    "crates/merman-bindings-core",
    "crates/merman-cli",
    "crates/merman-core",
    "crates/merman-editor-core",
    "crates/merman-export",
    "crates/merman-ffi",
    "crates/merman-layout-elk",
    "crates/merman-lsp",
    "crates/merman-render",
    "crates/merman-rustdoc",
    "crates/merman-typst-plugin",
    "crates/merman-uniffi",
    "crates/merman-wasm",
)
CRATE_COMPONENTS = {
    "crates/dugong": ("dagre",),
    "crates/dugong-graphlib": ("graphlib",),
    "crates/manatee": (
        "cose-base-v1",
        "cose-base-v2",
        "cytoscape",
        "cytoscape-cose-bilkent",
        "cytoscape-fcose",
        "layout-base-v1",
        "layout-base-v2",
    ),
    "crates/merman-ascii": (
        "beautiful-mermaid",
        "mermaid",
        "mermaid-ascii",
        "mermaid-rs-renderer",
    ),
    "crates/merman-core": ("dompurify", "mermaid", "sanitize-url", "zenuml-core"),
    "crates/merman-render": (
        "d3-shape",
        "fmin",
        "mermaid",
        "rough-rs",
        "roughjs",
        "venn-js",
        "zenuml-core",
    ),
}


class LegalMaterialError(Exception):
    pass


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--check", action="store_true", help="Fail if a projection is stale.")
    mode.add_argument("--write", action="store_true", help="Refresh every owned projection.")
    return parser.parse_args(argv)


def combined_project_license(root: Path) -> bytes:
    header = (
        "This package is dual-licensed under either of:\n\n"
        "- Apache License, Version 2.0\n"
        "- MIT License\n\n"
        "Full license texts follow.\n\n"
    ).encode()
    return (
        header
        + read_required_file(root / "LICENSE-MIT")
        + b"\n\n"
        + read_required_file(root / "LICENSE-APACHE")
    )


def expected_projections(root: Path) -> dict[Path, bytes]:
    licenses = root / CANONICAL_LICENSE_DIRECTORY
    if not licenses.is_dir():
        raise LegalMaterialError(f"missing canonical license directory: {licenses}")

    notice = read_required_file(root / CANONICAL_NOTICE)
    project_license = combined_project_license(root)
    contract = load_component_contract(root)
    expected: dict[Path, bytes] = {}
    for relative in PROJECT_LICENSE_TARGETS:
        expected[root / relative] = project_license
    for bundle_root in RELEASE_BUNDLE_ROOTS:
        destination = root / bundle_root
        scope_id = ARTIFACT_SCOPE_BY_BUNDLE.get(bundle_root)
        if scope_id is None:
            expected[destination / CANONICAL_NOTICE] = notice
            add_directory_projection(
                licenses,
                destination / CANONICAL_LICENSE_DIRECTORY,
                expected,
            )
        else:
            add_artifact_scope_projection(
                root,
                destination,
                contract,
                scope_id,
                expected,
            )
        if bundle_root == "distribution/typst/merman":
            expected.pop(
                destination / CANONICAL_LICENSE_DIRECTORY / RUST_DEPENDENCY_REPORT,
                None,
            )
        native_report = NATIVE_RUST_REPORTS.get(bundle_root)
        if native_report is not None:
            expected[
                destination / CANONICAL_LICENSE_DIRECTORY / RUST_DEPENDENCY_REPORT
            ] = read_required_file(root / native_report)

    android = root / ANDROID_META_INF
    expected[android / "LICENSE"] = project_license
    expected[android / CANONICAL_NOTICE] = notice
    add_directory_projection(
        licenses,
        android / CANONICAL_LICENSE_DIRECTORY,
        expected,
    )
    expected[android / CANONICAL_LICENSE_DIRECTORY / RUST_DEPENDENCY_REPORT] = (
        read_required_file(root / NATIVE_RUST_REPORTS["platforms/android"])
    )
    for crate_root in DUAL_LICENSE_CRATE_ROOTS:
        destination = root / crate_root
        expected[destination / "LICENSE-MIT"] = read_required_file(root / "LICENSE-MIT")
        expected[destination / "LICENSE-APACHE"] = read_required_file(root / "LICENSE-APACHE")
    add_crate_component_projections(root, expected, contract)
    return expected


def load_component_contract(root: Path) -> dict[str, object]:
    contract_path = root / "docs/release/THIRD_PARTY_COMPONENTS.json"
    try:
        contract = json.loads(read_required_file(contract_path))
    except (UnicodeError, json.JSONDecodeError) as error:
        raise LegalMaterialError(f"invalid third-party component contract: {error}") from error
    if not isinstance(contract, dict):
        raise LegalMaterialError("third-party component contract must be an object")
    return contract


def component_index(contract: dict[str, object]) -> dict[str, dict[str, object]]:
    raw_components = contract.get("components", [])
    if not isinstance(raw_components, list):
        raise LegalMaterialError("third-party component contract components must be an array")
    components = {
        component["id"]: component
        for component in raw_components
        if isinstance(component, dict) and isinstance(component.get("id"), str)
    }
    return components


def artifact_scope_index(contract: dict[str, object]) -> dict[str, dict[str, object]]:
    raw_scopes = contract.get("artifact_scopes", [])
    if not isinstance(raw_scopes, list):
        raise LegalMaterialError("third-party artifact scopes must be an array")
    return {
        scope["id"]: scope
        for scope in raw_scopes
        if isinstance(scope, dict) and isinstance(scope.get("id"), str)
    }


def resolve_artifact_components(
    contract: dict[str, object], scope_id: str
) -> tuple[dict[str, object], list[dict[str, object]]]:
    scopes = artifact_scope_index(contract)
    components = component_index(contract)
    selected_ids: list[str] = []
    selected = set()
    visiting: set[str] = set()

    def visit(current_id: str) -> None:
        scope = scopes.get(current_id)
        if scope is None:
            raise LegalMaterialError(f"artifact scope references unknown scope: {current_id}")
        if current_id in visiting:
            raise LegalMaterialError(f"artifact scope cycle detected at: {current_id}")
        if current_id in selected:
            return
        visiting.add(current_id)
        parents = scope.get("extends", [])
        if not isinstance(parents, list):
            raise LegalMaterialError(f"artifact scope extends must be an array: {current_id}")
        for parent in parents:
            if not isinstance(parent, str):
                raise LegalMaterialError(f"artifact scope parent must be a string: {current_id}")
            visit(parent)
        own_components = scope.get("components", [])
        if not isinstance(own_components, list):
            raise LegalMaterialError(f"artifact scope components must be an array: {current_id}")
        for component_id in own_components:
            if not isinstance(component_id, str):
                raise LegalMaterialError(
                    f"artifact scope component must be a string: {current_id}"
                )
            if component_id not in components:
                raise LegalMaterialError(
                    f"artifact scope references unknown component: {component_id}"
                )
            if component_id not in selected:
                selected.add(component_id)
                selected_ids.append(component_id)
        visiting.remove(current_id)

    visit(scope_id)
    return scopes[scope_id], [components[component_id] for component_id in selected_ids]


def add_artifact_scope_projection(
    root: Path,
    destination: Path,
    contract: dict[str, object],
    scope_id: str,
    expected: dict[Path, bytes],
) -> None:
    scope, selected = resolve_artifact_components(contract, scope_id)
    expected[destination / CANONICAL_NOTICE] = artifact_scope_notice(scope, selected)
    for component in selected:
        for license_file in component.get("license_files", []):
            source_relative = Path(license_file["path"])
            relative = source_relative.relative_to(CANONICAL_LICENSE_DIRECTORY)
            expected[destination / CANONICAL_LICENSE_DIRECTORY / relative] = (
                read_required_file(root / source_relative)
            )


def add_crate_component_projections(
    root: Path, expected: dict[Path, bytes], contract: dict[str, object]
) -> None:
    components = component_index(contract)
    for crate_root, component_ids in CRATE_COMPONENTS.items():
        destination = root / crate_root
        selected = []
        for component_id in component_ids:
            component = components.get(component_id)
            if component is None:
                raise LegalMaterialError(
                    f"crate projection references unknown component: {component_id}"
                )
            selected.append(component)
            for license_file in component.get("license_files", []):
                source_relative = Path(license_file["path"])
                relative = source_relative.relative_to(CANONICAL_LICENSE_DIRECTORY)
                expected[destination / CANONICAL_LICENSE_DIRECTORY / relative] = (
                    read_required_file(root / source_relative)
                )
        expected[destination / CANONICAL_NOTICE] = crate_notice(crate_root, selected)


def artifact_scope_notice(
    scope: dict[str, object], components: list[dict[str, object]]
) -> bytes:
    scope_id = scope["id"]
    description = scope.get("description", "")
    component_ids = ", ".join(f"`{component['id']}`" for component in components)
    lines = [
        "# Third-Party Notices",
        "",
        f"This file records the third-party material in the `{scope_id}` artifact scope.",
        "It is generated from the release component contract and is intentionally limited to this artifact.",
        "The wrapper and Merman-authored code remain under the project license; each component below keeps its own terms.",
        "",
        "## Artifact scope",
        "",
        f"- ID: `{scope_id}`",
        f"- Description: {description}",
        f"- Components: {component_ids}",
        "",
        "## Components",
        "",
    ]
    for component in components:
        source = component["source"]
        assert isinstance(source, dict)
        lines.extend(
            [
                f"### {component['name']}",
                "",
                str(component["notice"]),
                "",
                f"- Version: `{component['version']}`",
                f"- Source: {source['repository']} @ `{source['commit']}`",
                f"- Source path: `{source['path']}`",
                f"- Relationship: {', '.join(f'`{value}`' for value in component['relationships'])}",
                f"- License: `{component['license_expression']}`",
            ]
        )
        local_paths = component.get("local_paths", [])
        if local_paths:
            lines.append(f"- Local evidence: {', '.join(f'`{value}`' for value in local_paths)}")
        for license_file in component["license_files"]:
            relative = Path(license_file["path"]).relative_to(CANONICAL_LICENSE_DIRECTORY)
            lines.append(f"- Legal file: `THIRD_PARTY_LICENSES/{relative.as_posix()}`")
        lines.append("")
    return ("\n".join(lines).rstrip() + "\n").encode()


def crate_notice(crate_root: str, components: list[dict[str, object]]) -> bytes:
    lines = [
        "# Third-Party Notices",
        "",
        f"This file records source translated, copied, generated, or embedded in `{crate_root}`.",
        "It is generated from `docs/release/THIRD_PARTY_COMPONENTS.json`.",
        "",
    ]
    for component in components:
        source = component["source"]
        assert isinstance(source, dict)
        lines.extend(
            [
                f"## {component['name']}",
                "",
                str(component["notice"]),
                "",
                f"- Version: `{component['version']}`",
                f"- Source: {source['repository']} @ `{source['commit']}`",
                f"- Relationship: {', '.join(f'`{value}`' for value in component['relationships'])}",
                f"- License: `{component['license_expression']}`",
            ]
        )
        for license_file in component["license_files"]:
            relative = Path(license_file["path"]).relative_to(CANONICAL_LICENSE_DIRECTORY)
            lines.append(f"- Legal file: `THIRD_PARTY_LICENSES/{relative.as_posix()}`")
        lines.append("")
    return ("\n".join(lines).rstrip() + "\n").encode()


def add_directory_projection(
    source: Path,
    destination: Path,
    expected: dict[Path, bytes],
) -> None:
    files = sorted(path for path in source.rglob("*") if path.is_file())
    if not files:
        raise LegalMaterialError(f"canonical license directory is empty: {source}")
    for path in files:
        if path.is_symlink():
            raise LegalMaterialError(f"canonical legal material must not be a symlink: {path}")
        expected[destination / path.relative_to(source)] = read_required_file(path)


def read_required_file(path: Path) -> bytes:
    if not path.is_file() or path.is_symlink():
        raise LegalMaterialError(f"required legal material is not a regular file: {path}")
    return path.read_bytes()


def check_projections(root: Path, expected: dict[Path, bytes]) -> list[str]:
    failures: list[str] = []
    for path, expected_bytes in sorted(expected.items()):
        if not path.is_file():
            failures.append(f"missing projection: {display_relative_path(root, path)}")
        elif path.read_bytes() != expected_bytes:
            failures.append(f"stale projection: {display_relative_path(root, path)}")

    for directory in owned_directories(root):
        if not directory.exists():
            continue
        expected_files = {path for path in expected if path.is_relative_to(directory)}
        actual_files = {path for path in directory.rglob("*") if path.is_file()}
        allowed_external = {
            root / relative
            for relative in EXTERNALLY_MANAGED_PROJECTIONS
            if (root / relative).is_relative_to(directory)
        }
        for path in sorted(actual_files - expected_files - allowed_external):
            failures.append(
                f"unexpected projection file: {display_relative_path(root, path)}"
            )
    return failures


def display_relative_path(root: Path, path: Path) -> str:
    """Return stable POSIX paths in diagnostics on every host."""

    return path.relative_to(root).as_posix()


def owned_directories(root: Path) -> tuple[Path, ...]:
    directories = [
        root / bundle_root / CANONICAL_LICENSE_DIRECTORY
        for bundle_root in RELEASE_BUNDLE_ROOTS
    ]
    directories.append(root / ANDROID_META_INF / CANONICAL_LICENSE_DIRECTORY)
    directories.extend(
        root / crate_root / CANONICAL_LICENSE_DIRECTORY for crate_root in CRATE_COMPONENTS
    )
    return tuple(directories)


def write_projections(root: Path, expected: dict[Path, bytes]) -> None:
    for directory in owned_directories(root):
        replace_owned_directory(root, directory, expected)

    owned_files = {
        path: contents
        for path, contents in expected.items()
        if not any(path.is_relative_to(directory) for directory in owned_directories(root))
    }
    for path, contents in sorted(owned_files.items()):
        atomic_write(path, contents)


def replace_owned_directory(
    root: Path,
    destination: Path,
    expected: dict[Path, bytes],
) -> None:
    destination.parent.mkdir(parents=True, exist_ok=True)
    temporary = Path(tempfile.mkdtemp(prefix=f".{destination.name}-", dir=destination.parent))
    try:
        for path, contents in sorted(expected.items()):
            if not path.is_relative_to(destination):
                continue
            relative = path.relative_to(destination)
            target = temporary / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(contents)
        for relative in EXTERNALLY_MANAGED_PROJECTIONS:
            external = root / relative
            if external.is_file() and external.is_relative_to(destination):
                target = temporary / external.relative_to(destination)
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(external.read_bytes())
        if destination.exists():
            shutil.rmtree(destination)
        temporary.replace(destination)
    except Exception:
        shutil.rmtree(temporary, ignore_errors=True)
        raise


def atomic_write(path: Path, contents: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(dir=path.parent, prefix=f".{path.name}-", delete=False) as file:
        temporary = Path(file.name)
        file.write(contents)
        file.flush()
    temporary.replace(path)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv or sys.argv[1:])
    try:
        expected = expected_projections(ROOT)
        if args.write:
            write_projections(ROOT, expected)
        failures = check_projections(ROOT, expected)
    except (LegalMaterialError, OSError) as error:
        print(f"legal material projection failed: {error}", file=sys.stderr)
        return 1

    if failures:
        print("\n".join(failures), file=sys.stderr)
        print(
            "run `python3 scripts/sync-release-legal-materials.py --write` to refresh projections",
            file=sys.stderr,
        )
        return 1
    print(f"release legal material projections: ok ({len(expected)} files)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
