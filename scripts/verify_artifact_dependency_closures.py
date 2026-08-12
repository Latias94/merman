#!/usr/bin/env python3
"""Verify runtime dependency closures for exact Cargo artifact profiles."""

from __future__ import annotations

import argparse
from collections.abc import Callable, Iterable, Mapping, Sequence
from dataclasses import dataclass
from functools import lru_cache
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import tomllib

from artifact_profile_recipe import (
    DEFAULT_DESCRIPTOR,
    REPO_ROOT,
    ArtifactProfileError,
    CargoArtifactRecipe,
    load_artifact_profiles,
    rustc_host_target,
)


HOST_CLOSURE_REFERENCE_TARGET = "x86_64-unknown-linux-gnu"
LINUX_REFERENCE_SCOPE = "linux-reference"
PROFILE_TARGET_SCOPE = "profile-target"
RESOLVED_REPO_ROOT = REPO_ROOT.resolve()
NATIVE_BINDING_PROFILE_IDS = frozenset(
    {
        "android-native",
        "apple-uniffi-native",
        "c-abi-native",
        "flutter-android-native",
        "flutter-desktop-native",
        "flutter-ios-native",
        "python-uniffi-native",
        "rust-bindings-core-native-sdk",
    }
)
NATIVE_BINDING_FORBIDDEN_PACKAGES = (
    "cargo_metadata",
    "clap",
    "clap_complete",
    "hickory-resolver",
    "merman-cli",
    "rayon",
    "reqwest",
    "tokio",
    "uniffi_bindgen",
)


@dataclass(frozen=True)
class PackageFeatureExclusion:
    package: str
    features: tuple[str, ...]


@dataclass(frozen=True)
class ClosureClaim:
    claim_id: str
    profile_id: str
    required_packages: tuple[str, ...]
    forbidden_packages: tuple[str, ...]
    forbidden_features: tuple[PackageFeatureExclusion, ...] = ()


@dataclass(frozen=True)
class VerificationCase:
    recipe: CargoArtifactRecipe
    claim: ClosureClaim
    target: str

    @property
    def closure_scope(self) -> str:
        if self.recipe.build_target_kind == "host":
            return LINUX_REFERENCE_SCOPE
        return PROFILE_TARGET_SCOPE


@dataclass(frozen=True)
class DependencyClosure:
    features_by_package_identity: Mapping[
        tuple[str, str, str], frozenset[str]
    ]

    @property
    def packages(self) -> frozenset[str]:
        return frozenset(name for name, _version, _source in self.features_by_package_identity)

    @property
    def features_by_package(self) -> Mapping[str, frozenset[str]]:
        merged: dict[str, set[str]] = {}
        for (name, _version, _source), package_features in self.features_by_package_identity.items():
            merged.setdefault(name, set()).update(package_features)
        return {
            name: frozenset(package_features)
            for name, package_features in sorted(merged.items())
        }


@dataclass(frozen=True)
class ClosureObservation:
    profile_id: str
    build_target_kind: str
    closure_scope: str
    closure_target: str
    package_count: int


class ClosureVerificationError(RuntimeError):
    """One or more exact artifact closure claims failed."""


CommandRunner = Callable[[Sequence[str]], subprocess.CompletedProcess[str]]
ProbePreparer = Callable[[VerificationCase, Path], Path]


SEMANTIC_CLAIMS = (
    ClosureClaim(
        claim_id="static-svg-is-environment-and-export-free",
        profile_id="rust-static-svg",
        required_packages=("merman", "merman-core", "merman-render"),
        forbidden_packages=(
            "chrono",
            "getrandom",
            "image",
            "jiff",
            "krilla",
            "krilla-svg",
            "merman-export",
            "resvg",
            "tiny-skia",
            "usvg",
            "web-time",
        ),
        forbidden_features=(
            PackageFeatureExclusion(
                "merman",
                (
                    "system-clock",
                    "system-random",
                    "system-timezone",
                    "system-timing",
                ),
            ),
            PackageFeatureExclusion(
                "merman-core",
                (
                    "system-clock",
                    "system-random",
                    "system-timezone",
                    "system-timing",
                ),
            ),
            PackageFeatureExclusion(
                "merman-render",
                (
                    "system-clock",
                    "system-random",
                    "system-timezone",
                    "system-timing",
                ),
            ),
        ),
    ),
    ClosureClaim(
        claim_id="svg-basic-excludes-optional-engines-and-products",
        profile_id="rust-svg-basic",
        required_packages=("merman", "merman-core", "merman-render"),
        forbidden_packages=(
            "chrono",
            "getrandom",
            "image",
            "jiff",
            "krilla",
            "krilla-svg",
            "manatee",
            "merman-analysis",
            "merman-ascii",
            "merman-editor-core",
            "merman-export",
            "merman-layout-elk",
            "ratex-layout",
            "ratex-parser",
            "ratex-svg",
            "ratex-types",
            "resvg",
            "tiny-skia",
            "usvg",
            "web-time",
        ),
        forbidden_features=(
            PackageFeatureExclusion(
                "merman",
                (
                    "analysis",
                    "ascii",
                    "editor",
                    "jpeg",
                    "layout-cytoscape",
                    "layout-elk",
                    "math",
                    "pdf",
                    "png",
                    "system-clock",
                    "system-random",
                    "system-timezone",
                    "system-timing",
                ),
            ),
            PackageFeatureExclusion(
                "merman-core",
                (
                    "system-clock",
                    "system-random",
                    "system-timezone",
                    "system-timing",
                ),
            ),
            PackageFeatureExclusion(
                "merman-render",
                (
                    "layout-cytoscape",
                    "layout-elk",
                    "math",
                    "system-clock",
                    "system-random",
                    "system-timezone",
                    "system-timing",
                ),
            ),
        ),
    ),
    ClosureClaim(
        claim_id="cli-analysis-is-render-and-tool-free",
        profile_id="cli-analysis",
        required_packages=("merman-analysis", "merman-cli", "merman-core"),
        forbidden_packages=(
            "chrono",
            "clap_complete",
            "krilla",
            "krilla-svg",
            "merman-export",
            "merman-render",
            "rayon",
            "reqwest",
            "resvg",
            "tiny-skia",
            "usvg",
        ),
        forbidden_features=(
            PackageFeatureExclusion(
                "merman-cli",
                (
                    "ascii",
                    "icons",
                    "jpeg",
                    "markdown",
                    "network-icons",
                    "parallel-markdown",
                    "pdf",
                    "png",
                    "shell-completions",
                    "svg",
                ),
            ),
        ),
    ),
    ClosureClaim(
        claim_id="jpeg-excludes-pdf-backend",
        profile_id="rust-export-jpeg",
        required_packages=(
            "image",
            "merman-export",
            "merman-render",
            "resvg",
            "tiny-skia",
            "usvg",
        ),
        forbidden_packages=("chrono", "krilla", "krilla-svg"),
        forbidden_features=(
            PackageFeatureExclusion("merman-export", ("pdf", "png")),
        ),
    ),
    ClosureClaim(
        claim_id="png-excludes-pdf-backend",
        profile_id="rust-export-png",
        required_packages=(
            "merman-export",
            "merman-render",
            "resvg",
            "tiny-skia",
            "usvg",
        ),
        forbidden_packages=("chrono", "krilla", "krilla-svg"),
        forbidden_features=(
            PackageFeatureExclusion("merman-export", ("jpeg", "pdf")),
        ),
    ),
    ClosureClaim(
        claim_id="pdf-requires-krilla-and-excludes-raster-exports",
        profile_id="rust-export-pdf",
        required_packages=("krilla", "merman-export", "merman-render"),
        forbidden_packages=("chrono",),
        forbidden_features=(
            PackageFeatureExclusion("merman-export", ("jpeg", "png")),
        ),
    ),
    ClosureClaim(
        claim_id="typst-wasm-excludes-host-and-browser-adapters",
        profile_id="typst-wasm",
        required_packages=("merman-typst-plugin",),
        forbidden_packages=(
            "chrono",
            "console_error_panic_hook",
            "getrandom",
            "js-sys",
            "pest",
            "serde-wasm-bindgen",
            "serde_yaml",
            "unsafe-libyaml",
            "wasm-bindgen",
            "wasm-bindgen-futures",
            "web-time",
        ),
    ),
)


def load_verification_cases(
    *,
    descriptor_path: Path = DEFAULT_DESCRIPTOR,
    semantic_claims: Sequence[ClosureClaim] = SEMANTIC_CLAIMS,
) -> tuple[VerificationCase, ...]:
    """Join descriptor-owned recipes with semantic runtime-closure checks."""
    try:
        profiles = load_artifact_profiles(descriptor_path)
    except ArtifactProfileError as error:
        raise ClosureVerificationError(str(error)) from error
    profile_ids = tuple(profile.profile_id for profile in profiles)

    semantic_by_profile = {claim.profile_id: claim for claim in semantic_claims}
    if len(semantic_by_profile) != len(semantic_claims):
        raise ClosureVerificationError("semantic claims contain duplicate profiles")
    unknown_semantic = sorted(set(semantic_by_profile) - set(profile_ids))
    if unknown_semantic:
        raise ClosureVerificationError(
            f"semantic claims reference unknown profiles: {unknown_semantic!r}"
        )

    cases: list[VerificationCase] = []
    for profile in profiles:
        profile_id = profile.profile_id
        recipe = profile.cargo

        if recipe.build_target_kind == "host":
            expected_targets = (HOST_CLOSURE_REFERENCE_TARGET,)
        elif recipe.build_target_kind == "target-set":
            expected_targets = recipe.build_targets
        else:
            raise ClosureVerificationError(
                f"profile {profile_id!r} has unsupported build target kind "
                f"{recipe.build_target_kind!r}"
            )

        claim = semantic_by_profile.get(profile_id)
        if claim is None and profile_id in NATIVE_BINDING_PROFILE_IDS:
            claim = ClosureClaim(
                claim_id=f"{profile_id}-native-runtime-dependency-boundary",
                profile_id=profile_id,
                required_packages=(recipe.package,),
                forbidden_packages=NATIVE_BINDING_FORBIDDEN_PACKAGES,
            )
        if claim is None:
            continue
        if recipe.package not in claim.required_packages:
            raise ClosureVerificationError(
                f"semantic claim {claim.claim_id!r} must require descriptor root "
                f"package {recipe.package!r}"
            )
        cases.extend(
            VerificationCase(recipe, claim, target) for target in expected_targets
        )
    return tuple(cases)


def _validate_case_recipe(case: VerificationCase) -> None:
    """Validate the target selectors before querying Cargo metadata."""
    recipe = case.recipe
    if recipe.default_features:
        raise ClosureVerificationError(
            f"profile {recipe.profile_id!r} must set default_features=false"
        )
    if recipe.build_target_kind == "host":
        if case.target != HOST_CLOSURE_REFERENCE_TARGET:
            raise ClosureVerificationError(
                f"host profile {recipe.profile_id!r} must use Linux reference "
                f"target {HOST_CLOSURE_REFERENCE_TARGET!r}"
            )
    elif recipe.build_target_kind == "target-set":
        if case.target not in recipe.build_targets:
            raise ClosureVerificationError(
                f"profile {recipe.profile_id!r} does not declare target "
                f"{case.target!r}"
            )
    else:
        raise ClosureVerificationError(
            f"profile {recipe.profile_id!r} has unsupported build target kind "
            f"{recipe.build_target_kind!r}"
        )


@lru_cache(maxsize=1)
def _workspace_metadata_without_dependencies() -> Mapping[str, object]:
    return _run_cargo_metadata(
        ["cargo", "metadata", "--locked", "--format-version", "1", "--no-deps"],
        "workspace package declarations",
    )


@lru_cache(maxsize=1)
def _locked_workspace_external_identities() -> frozenset[tuple[str, str, str]]:
    return _lockfile_external_identities(REPO_ROOT / "Cargo.lock")


def _run_cargo_metadata(
    command: Sequence[str],
    description: str,
) -> Mapping[str, object]:
    completed = subprocess.run(
        command,
        cwd=REPO_ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    if completed.returncode != 0:
        detail = (completed.stderr or completed.stdout).strip() or "<empty output>"
        raise ClosureVerificationError(f"failed to read {description}: {detail}")
    try:
        document = json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        raise ClosureVerificationError(
            f"{description} was not valid Cargo metadata JSON: {error}"
        ) from error
    if not isinstance(document, Mapping):
        raise ClosureVerificationError(f"{description} must be a JSON object")
    return document


def _workspace_package_metadata(recipe: CargoArtifactRecipe) -> Mapping[str, object]:
    document = _workspace_metadata_without_dependencies()
    packages = document.get("packages")
    workspace_members = document.get("workspace_members")
    if not isinstance(packages, list) or not isinstance(workspace_members, list):
        raise ClosureVerificationError(
            "workspace cargo metadata lacks packages/workspace_members"
        )
    member_ids = set(workspace_members)
    matches = [
        package
        for package in packages
        if isinstance(package, Mapping)
        and package.get("id") in member_ids
        and package.get("name") == recipe.package
    ]
    if len(matches) != 1:
        raise ClosureVerificationError(
            f"workspace metadata must contain one package {recipe.package!r}; "
            f"found {len(matches)}"
        )
    manifest_path = matches[0].get("manifest_path")
    expected_manifest = (REPO_ROOT / recipe.manifest).resolve()
    if not isinstance(manifest_path, str) or Path(manifest_path).resolve() != expected_manifest:
        raise ClosureVerificationError(
            f"artifact profile {recipe.profile_id!r} manifest does not match workspace metadata"
        )
    return matches[0]


def write_metadata_probe(
    case: VerificationCase,
    probe_dir: Path,
    *,
    package_metadata: Mapping[str, object] | None = None,
) -> Path:
    """Create a standalone package projection for one exact artifact recipe."""
    _validate_case_recipe(case)
    recipe = case.recipe
    package = (
        _workspace_package_metadata(recipe)
        if package_metadata is None
        else package_metadata
    )
    if package.get("name") != recipe.package:
        raise ClosureVerificationError(
            f"structured package metadata does not match recipe root {recipe.package!r}"
        )
    version = package.get("version")
    edition = package.get("edition")
    features = package.get("features")
    dependencies = package.get("dependencies")
    if (
        not isinstance(version, str)
        or not isinstance(edition, str)
        or not isinstance(features, Mapping)
        or not isinstance(dependencies, list)
    ):
        raise ClosureVerificationError(
            f"structured package metadata for {recipe.package!r} is incomplete"
        )

    probe_dir.mkdir(parents=True, exist_ok=True)
    (probe_dir / "lib.rs").write_text("", encoding="utf-8")
    lines = [
        "[package]",
        f"name = {json.dumps(recipe.package)}",
        f"version = {json.dumps(version)}",
        f"edition = {json.dumps(edition)}",
        "publish = false",
        "",
        "[lib]",
        'path = "lib.rs"',
        "",
        "[workspace]",
        'resolver = "2"',
        "",
        "[features]",
    ]
    for feature, members in sorted(features.items()):
        if not isinstance(feature, str) or not isinstance(members, list) or not all(
            isinstance(member, str) for member in members
        ):
            raise ClosureVerificationError(
                f"structured feature metadata for {recipe.package!r} is invalid"
            )
        rendered_members = ", ".join(json.dumps(member) for member in members)
        lines.append(f"{json.dumps(feature)} = [{rendered_members}]")

    sections: dict[str, list[str]] = {}
    for dependency in dependencies:
        if not isinstance(dependency, Mapping):
            raise ClosureVerificationError(
                f"structured dependency metadata for {recipe.package!r} is invalid"
            )
        if dependency.get("kind") is not None:
            continue
        target = dependency.get("target")
        if target is not None and not isinstance(target, str):
            raise ClosureVerificationError(
                f"dependency target for {recipe.package!r} is invalid"
            )
        section = "dependencies" if target is None else f"target.{json.dumps(target)}.dependencies"
        sections.setdefault(section, []).append(_render_probe_dependency(dependency))
    for section, entries in sorted(sections.items()):
        lines.extend(("", f"[{section}]", *sorted(entries)))
    lines.append("")

    manifest = probe_dir / "Cargo.toml"
    manifest.write_text("\n".join(lines), encoding="utf-8")
    return manifest


def _render_probe_dependency(dependency: Mapping[str, object]) -> str:
    name = dependency.get("name")
    rename = dependency.get("rename")
    requirement = dependency.get("req")
    path = dependency.get("path")
    source = dependency.get("source")
    registry = dependency.get("registry")
    optional = dependency.get("optional")
    uses_default_features = dependency.get("uses_default_features")
    dependency_features = dependency.get("features")
    if (
        not isinstance(name, str)
        or (rename is not None and not isinstance(rename, str))
        or not isinstance(requirement, str)
        or not isinstance(optional, bool)
        or not isinstance(uses_default_features, bool)
        or not isinstance(dependency_features, list)
        or not all(isinstance(feature, str) for feature in dependency_features)
    ):
        raise ClosureVerificationError("structured normal dependency metadata is invalid")

    fields: list[str] = []
    if isinstance(path, str):
        fields.extend(
            (f"path = {json.dumps(path)}", f"version = {json.dumps(requirement)}")
        )
    elif isinstance(source, str) and source.startswith("registry+") and registry is None:
        fields.append(f"version = {json.dumps(requirement)}")
    else:
        raise ClosureVerificationError(
            f"normal dependency {name!r} uses an unsupported non-path/crates.io source"
        )

    alias = rename or name
    if alias != name:
        fields.append(f"package = {json.dumps(name)}")
    fields.append(
        f"default-features = {'true' if uses_default_features else 'false'}"
    )
    if optional:
        fields.append("optional = true")
    if dependency_features:
        rendered_features = ", ".join(
            json.dumps(feature) for feature in dependency_features
        )
        fields.append(f"features = [{rendered_features}]")
    return f"{json.dumps(alias)} = {{ {', '.join(fields)} }}"


def _lockfile_external_identities(
    lockfile: Path,
) -> frozenset[tuple[str, str, str]]:
    try:
        with lockfile.open("rb") as file:
            document = tomllib.load(file)
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise ClosureVerificationError(
            f"failed to read Cargo lockfile {lockfile}: {error}"
        ) from error

    packages = document.get("package")
    if not isinstance(packages, list):
        raise ClosureVerificationError(
            f"Cargo lockfile {lockfile} lacks a package array"
        )
    identities: set[tuple[str, str, str]] = set()
    for package in packages:
        if not isinstance(package, Mapping):
            raise ClosureVerificationError(
                f"Cargo lockfile {lockfile} contains a non-object package"
            )
        source = package.get("source")
        if source is None:
            continue
        name = package.get("name")
        version = package.get("version")
        if not isinstance(name, str) or not isinstance(version, str) or not isinstance(source, str):
            raise ClosureVerificationError(
                f"Cargo lockfile {lockfile} contains an invalid external package"
            )
        identities.add((name, version, source))
    return frozenset(identities)


def prepare_metadata_probe(case: VerificationCase, probe_dir: Path) -> Path:
    """Create one exact standalone probe seeded from the committed workspace lock."""
    manifest = write_metadata_probe(case, probe_dir)
    committed_lock = REPO_ROOT / "Cargo.lock"
    if not committed_lock.is_file():
        raise ClosureVerificationError(f"missing committed Cargo.lock at {committed_lock}")
    shutil.copyfile(committed_lock, probe_dir / "Cargo.lock")
    return manifest


def _validate_probe_lock(probe_manifest: Path) -> None:
    lockfile = probe_manifest.parent / "Cargo.lock"
    if not lockfile.is_file():
        raise ClosureVerificationError("cargo metadata did not create the probe lockfile")
    unexpected = sorted(
        _lockfile_external_identities(lockfile)
        - _locked_workspace_external_identities()
    )
    if unexpected:
        rendered = ", ".join(
            f"{name} {version} ({source})" for name, version, source in unexpected
        )
        raise ClosureVerificationError(
            "metadata probe resolved packages outside the committed workspace lock: "
            + rendered
        )


def cargo_metadata_command(
    case: VerificationCase,
    *,
    probe_manifest: Path,
    frozen: bool = True,
) -> list[str]:
    """Project one exact descriptor case into a structured Cargo metadata query."""
    _validate_case_recipe(case)
    recipe = case.recipe
    command = [
        "cargo",
        "metadata",
        "--format-version",
        "1",
        "--filter-platform",
        case.target,
        "--manifest-path",
        str(probe_manifest),
        "--no-default-features",
    ]
    if recipe.features:
        command.extend(("--features", recipe.feature_argument))
    command.append("--frozen" if frozen else "--offline")
    return command


def parse_cargo_metadata(
    document: Mapping[str, object] | str,
    *,
    root_package: str,
) -> DependencyClosure:
    """Traverse normal, target-filtered dependencies from structured Cargo metadata."""
    if isinstance(document, str):
        try:
            document = json.loads(document)
        except json.JSONDecodeError as error:
            raise ClosureVerificationError(
                f"cargo metadata output was not valid JSON: {error}"
            ) from error
    if not isinstance(document, Mapping):
        raise ClosureVerificationError("cargo metadata output must be a JSON object")

    packages_raw = document.get("packages")
    resolve = document.get("resolve")
    if not isinstance(packages_raw, list) or not isinstance(resolve, Mapping):
        raise ClosureVerificationError(
            "cargo metadata output must contain packages and resolve objects"
        )
    root_id = resolve.get("root")
    nodes_raw = resolve.get("nodes")
    if not isinstance(root_id, str) or not isinstance(nodes_raw, list):
        raise ClosureVerificationError(
            "cargo metadata resolve must contain a root id and nodes array"
        )

    packages: dict[str, Mapping[str, object]] = {}
    for package in packages_raw:
        if not isinstance(package, Mapping):
            raise ClosureVerificationError("cargo metadata package entries must be objects")
        package_id = package.get("id")
        if not isinstance(package_id, str) or not package_id:
            raise ClosureVerificationError("cargo metadata package is missing an id")
        if package_id in packages:
            raise ClosureVerificationError(f"cargo metadata repeats package id {package_id!r}")
        packages[package_id] = package

    nodes: dict[str, Mapping[str, object]] = {}
    for node in nodes_raw:
        if not isinstance(node, Mapping):
            raise ClosureVerificationError("cargo metadata resolve node entries must be objects")
        node_id = node.get("id")
        if not isinstance(node_id, str) or not node_id:
            raise ClosureVerificationError("cargo metadata resolve node is missing an id")
        if node_id in nodes:
            raise ClosureVerificationError(f"cargo metadata repeats resolve node {node_id!r}")
        nodes[node_id] = node
    if root_id not in nodes or root_id not in packages:
        raise ClosureVerificationError("cargo metadata resolve root is not a package node")

    def package_name(package_id: str) -> str:
        package = packages.get(package_id)
        name = package.get("name") if package is not None else None
        if not isinstance(name, str) or not name:
            raise ClosureVerificationError(
                f"cargo metadata package {package_id!r} is missing a name"
            )
        return name

    def is_normal_dependency(dep: Mapping[str, object]) -> bool:
        dep_kinds = dep.get("dep_kinds")
        if not isinstance(dep_kinds, list):
            raise ClosureVerificationError(
                "cargo metadata dependency is missing its dep_kinds array"
            )
        return any(
            isinstance(kind, Mapping) and kind.get("kind") is None
            for kind in dep_kinds
        )

    def is_proc_macro(package: Mapping[str, object]) -> bool:
        targets = package.get("targets")
        if not isinstance(targets, list):
            raise ClosureVerificationError("cargo metadata package is missing targets")
        return any(
            isinstance(target, Mapping)
            and isinstance(target.get("kind"), list)
            and "proc-macro" in target["kind"]
            for target in targets
        )

    def normal_dependencies(node_id: str) -> list[str]:
        node = nodes.get(node_id)
        if node is None:
            raise ClosureVerificationError(
                f"cargo metadata is missing resolve node {node_id!r}"
            )
        deps = node.get("deps")
        if not isinstance(deps, list):
            raise ClosureVerificationError(
                f"cargo metadata resolve node {node_id!r} is missing deps"
            )
        result = []
        for dep in deps:
            if not isinstance(dep, Mapping) or not is_normal_dependency(dep):
                continue
            dep_id = dep.get("pkg")
            if not isinstance(dep_id, str) or dep_id not in packages:
                raise ClosureVerificationError(
                    f"cargo metadata dependency from {node_id!r} references unknown package"
                )
            if not is_proc_macro(packages[dep_id]):
                result.append(dep_id)
        return result

    if package_name(root_id) != root_package:
        raise ClosureVerificationError(
            f"cargo metadata selected root {package_name(root_id)!r}; "
            f"expected {root_package!r}"
        )

    reachable: list[str] = []
    pending = [root_id]
    seen: set[str] = set()
    while pending:
        package_id = pending.pop()
        if package_id in seen:
            continue
        seen.add(package_id)
        reachable.append(package_id)
        pending.extend(normal_dependencies(package_id))

    identity_features: dict[tuple[str, str, str], set[str]] = {}
    for package_id in reachable:
        package = packages[package_id]
        name = package.get("name")
        version = package.get("version")
        source_value = package.get("source")
        manifest_path = package.get("manifest_path")
        if not isinstance(name, str) or not isinstance(version, str):
            raise ClosureVerificationError(
                f"cargo metadata package {package_id!r} is missing name/version"
            )
        if source_value is not None and not isinstance(source_value, str):
            raise ClosureVerificationError(
                f"cargo metadata package {package_id!r} has an invalid source"
            )
        if not isinstance(manifest_path, str) or not manifest_path:
            raise ClosureVerificationError(
                f"cargo metadata package {package_id!r} is missing manifest_path"
            )
        source = _metadata_source(source_value, Path(manifest_path))
        node = nodes[package_id]
        features = node.get("features", [])
        if not isinstance(features, list) or not all(
            isinstance(feature, str) for feature in features
        ):
            raise ClosureVerificationError(
                f"cargo metadata resolve node {package_id!r} has invalid features"
            )
        identity_features.setdefault((name, version, source), set()).update(features)

    if not identity_features:
        raise ClosureVerificationError("cargo metadata produced no dependency packages")

    return DependencyClosure(
        features_by_package_identity={
            package_id: frozenset(package_features)
            for package_id, package_features in sorted(identity_features.items())
        },
    )


def _metadata_source(source: str | None, manifest_path: Path) -> str:
    if source is not None:
        return source
    resolved_path = manifest_path.resolve()
    try:
        relative_path = resolved_path.parent.relative_to(RESOLVED_REPO_ROOT)
    except ValueError:
        return f"path+file://{resolved_path.parent.as_posix()}"
    return f"path+workspace://{relative_path.as_posix()}"


def check_case(
    case: VerificationCase,
    closure: DependencyClosure,
) -> tuple[list[str], ClosureObservation]:
    """Compare one observed runtime closure with its profile-owned contract."""
    claim = case.claim
    failures: list[str] = []
    required = set(claim.required_packages)
    forbidden = set(claim.forbidden_packages)

    overlaps = sorted(required & forbidden)
    if overlaps:
        failures.append(
            "claim lists packages as both required and forbidden: "
            + ", ".join(overlaps)
        )

    missing = sorted(required - closure.packages)
    if missing:
        failures.append("required packages missing: " + ", ".join(missing))

    present = sorted(forbidden & closure.packages)
    if present:
        failures.append("forbidden packages present: " + ", ".join(present))

    for exclusion in claim.forbidden_features:
        if exclusion.package not in required:
            failures.append(
                f"feature exclusion owner {exclusion.package!r} is not a required package"
            )
            continue
        actual = closure.features_by_package.get(exclusion.package)
        if actual is None:
            continue
        enabled = sorted(set(exclusion.features) & actual)
        if enabled:
            failures.append(
                f"package {exclusion.package!r} enables forbidden features: "
                + ", ".join(enabled)
            )

    observation = ClosureObservation(
        profile_id=case.recipe.profile_id,
        build_target_kind=case.recipe.build_target_kind,
        closure_scope=case.closure_scope,
        closure_target=case.target,
        package_count=len(closure.features_by_package_identity),
    )
    return failures, observation


def _default_runner(command: Sequence[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        command,
        cwd=REPO_ROOT,
        check=False,
        capture_output=True,
        text=True,
    )


def verify_cases(
    cases: Iterable[VerificationCase],
    *,
    runner: CommandRunner = _default_runner,
    probe_preparer: ProbePreparer = prepare_metadata_probe,
) -> tuple[ClosureObservation, ...]:
    """Run every selected profile-target metadata query and aggregate failures."""
    failures: list[str] = []
    observations: list[ClosureObservation] = []
    command_outcomes: dict[
        tuple[str, ...],
        DependencyClosure | RuntimeError,
    ] = {}

    case_list = tuple(cases)
    with tempfile.TemporaryDirectory(prefix="merman-closure-") as temporary_directory:
        probe_root = Path(temporary_directory)
        probe_paths: dict[tuple[object, ...], Path] = {}
        for index, case in enumerate(case_list):
            context = (
                f"{case.claim.claim_id} ({case.recipe.profile_id}, "
                f"build-target-kind={case.recipe.build_target_kind}, "
                f"closure-scope={case.closure_scope}, closure-target={case.target})"
            )
            try:
                probe_key = (
                    case.recipe.package,
                    case.recipe.manifest,
                    case.recipe.features,
                    case.target,
                )
                probe_manifest = probe_paths.get(probe_key)
                if probe_manifest is None:
                    probe_manifest = probe_preparer(
                        case,
                        probe_root / f"probe-{index}",
                    )
                    probe_paths[probe_key] = probe_manifest
                command = cargo_metadata_command(
                    case,
                    probe_manifest=probe_manifest,
                    frozen=False,
                )
                command_key = tuple(command)
                outcome = command_outcomes.get(command_key)
                if outcome is None:
                    try:
                        completed = runner(command)
                        if completed.returncode != 0:
                            stderr = (completed.stderr or "").strip() or "<empty stderr>"
                            raise ClosureVerificationError(
                                f"cargo metadata exited with {completed.returncode}: {stderr}"
                            )
                        if not isinstance(completed.stdout, (str, dict)):
                            raise ClosureVerificationError(
                                "cargo metadata stdout was neither JSON text nor an object"
                            )
                        _validate_probe_lock(probe_manifest)
                        outcome = parse_cargo_metadata(
                            completed.stdout,
                            root_package=case.recipe.package,
                        )
                    except RuntimeError as error:
                        outcome = error
                    command_outcomes[command_key] = outcome
                if isinstance(outcome, RuntimeError):
                    raise outcome
                closure = outcome
                case_failures, observation = check_case(case, closure)
                observations.append(observation)
                failures.extend(f"{context}: {failure}" for failure in case_failures)
            except RuntimeError as error:
                failures.append(f"{context}: {error}")

    if failures:
        raise ClosureVerificationError(
            "artifact dependency closure verification failed:\n- "
            + "\n- ".join(failures)
        )
    return tuple(observations)


def _select_cases(
    cases: Sequence[VerificationCase],
    profile_ids: Sequence[str],
) -> tuple[VerificationCase, ...]:
    if not profile_ids:
        return tuple(cases)
    requested = set(profile_ids)
    known = {case.recipe.profile_id for case in cases}
    unknown = sorted(requested - known)
    if unknown:
        raise ClosureVerificationError(
            "profiles have no dependency-closure recipe: " + ", ".join(unknown)
        )
    return tuple(case for case in cases if case.recipe.profile_id in requested)


def select_representative_cases(
    cases: Sequence[VerificationCase],
) -> tuple[VerificationCase, ...]:
    """Keep the descriptor's first target for each profile.

    The full target set remains the release/mainline contract. Pull requests only
    need one representative resolution per profile to catch feature leakage
    without turning a dependency-policy check into a cross-compilation matrix.
    """
    selected: list[VerificationCase] = []
    seen_profiles: set[str] = set()
    for case in cases:
        if case.recipe.profile_id in seen_profiles:
            continue
        selected.append(case)
        seen_profiles.add(case.recipe.profile_id)
    return tuple(selected)


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--descriptor",
        type=Path,
        default=DEFAULT_DESCRIPTOR,
        help="Path to the exact artifact profile descriptor.",
    )
    parser.add_argument(
        "--profile",
        action="append",
        default=[],
        help="Verify only this artifact profile; may be repeated.",
    )
    parser.add_argument(
        "--representative-targets",
        action="store_true",
        help="Verify only the first descriptor target for each selected profile.",
    )
    args = parser.parse_args(argv)

    try:
        cases = _select_cases(
            load_verification_cases(descriptor_path=args.descriptor),
            args.profile,
        )
        if args.representative_targets:
            cases = select_representative_cases(cases)
        try:
            running_host_target = rustc_host_target()
        except RuntimeError as error:
            raise ClosureVerificationError(str(error)) from error
        observations = verify_cases(cases)
    except ClosureVerificationError as error:
        print(error, file=sys.stderr)
        return 1

    for observation in observations:
        print(
            "artifact-closure OK "
            f"profile={observation.profile_id} "
            f"build-target-kind={observation.build_target_kind} "
            f"closure-scope={observation.closure_scope} "
            f"closure-target={observation.closure_target} "
            f"verifier-host={running_host_target} "
            "closure=runtime "
            f"packages={observation.package_count}"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
