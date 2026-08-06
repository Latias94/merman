#!/usr/bin/env python3
"""Verify runtime dependency closures for exact Cargo artifact profiles."""

from __future__ import annotations

import argparse
from collections.abc import Callable, Iterable, Mapping, Sequence
from dataclasses import dataclass
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
from typing import Any

from artifact_profile_recipe import (
    DEFAULT_DESCRIPTOR,
    REPO_ROOT,
    ArtifactProfileError,
    CargoArtifactRecipe,
    load_artifact_profiles,
)
from ffi_contract_dependency_probes import (
    BASELINE_COMMIT,
    DependencyProbe,
    load_dependency_probes,
    probe_registry_sha256,
)
from ffi_contract_baseline_contract import (
    BASELINE_INPUT_PATHS,
    DEFAULT_BASELINE_LOCK,
    FINGERPRINT_RE,
    FfiBaselineContractError,
    file_sha256,
    input_records,
    load_baseline_lock,
    rust_toolchain_dependency_compatibility_projection,
    source_revision_projection,
    validate_input_records,
    validate_rust_toolchain,
    validate_source_revision,
)
from ffi_contract_reproducibility import (
    FfiContractReproducibilityError,
    ffi_contract_subprocess_environment,
    reject_cargo_configuration,
    reject_ffi_contract_environment,
    rust_toolchain_provenance,
)
from strict_json import StrictJsonContract, bytes_sha256, canonical_sha256
from verify_rustsec_exceptions import (
    RustSecExceptionError,
    load_exception_records,
    validate_profile_coverage,
)


PACKAGE_MARKER = "__MERMAN_CLOSURE_PACKAGE__"
FEATURE_MARKER = "__MERMAN_CLOSURE_FEATURES__"
PACKAGE_ID_RE = re.compile(
    r"^(?P<name>[A-Za-z0-9_-]+)\s+v(?P<version>[^\s]+)(?P<annotations>.*)$"
)
FINGERPRINT_DOMAIN = b"merman-artifact-dependency-closure-v2\0"
ATTRIBUTION_FINGERPRINT_DOMAIN = b"merman-ffi-attribution-closure-v1\0"
BASELINE_REPORT_ID = "merman-ffi-contract-dependency-baseline"
BASELINE_SCHEMA_VERSION = 4
REJECTED_BASELINE_FILE_SHA256 = frozenset(
    {"sha256:0b1ac1c061439158375bc64de03c9fdbd68c46eeead8a18f7d6f27331fd1aeca"}
)
DEFAULT_REGISTRY_SOURCE = "registry+https://github.com/rust-lang/crates.io-index"
CARGO_DEDUPLICATION_SUFFIX = " (*)"
CARGO_PROC_MACRO_ANNOTATION = " (proc-macro)"
HOST_CLOSURE_REFERENCE_TARGET = "x86_64-unknown-linux-gnu"
LINUX_REFERENCE_SCOPE = "linux-reference"
PROFILE_TARGET_SCOPE = "profile-target"


STRICT_BASELINE_JSON = StrictJsonContract(
    error_factory=lambda message: ClosureVerificationError(message),
    read_error_prefix="cannot read FFI dependency baseline",
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
        for (
            name,
            _version,
            _source,
        ), package_features in self.features_by_package_identity.items():
            merged.setdefault(name, set()).update(package_features)
        return {
            name: frozenset(package_features)
            for name, package_features in sorted(merged.items())
        }

    def projection(self) -> list[dict[str, Any]]:
        return [
            {
                "package": package,
                "version": version,
                "source": source,
                "features": sorted(features),
            }
            for (package, version, source), features in sorted(
                self.features_by_package_identity.items()
            )
        ]


@dataclass(frozen=True)
class AttributionPackage:
    package: str
    version: str
    source: str
    features: tuple[str, ...]
    roles: tuple[str, ...]
    role_features: Mapping[str, tuple[str, ...]]

    def projection(self) -> dict[str, Any]:
        return {
            "package": self.package,
            "version": self.version,
            "source": self.source,
            "features": list(self.features),
            "roles": list(self.roles),
            "role_features": {
                role: list(features)
                for role, features in sorted(self.role_features.items())
            },
        }


@dataclass(frozen=True)
class AttributionClosure:
    packages: tuple[AttributionPackage, ...]

    @property
    def package_names(self) -> frozenset[str]:
        return frozenset(package.package for package in self.packages)

    def projection(self) -> list[dict[str, Any]]:
        return [package.projection() for package in self.packages]


@dataclass(frozen=True)
class ProbeClosureObservation:
    probe: DependencyProbe
    runtime: DependencyClosure
    attribution: AttributionClosure


@dataclass(frozen=True)
class ClosureObservation:
    profile_id: str
    build_target_kind: str
    closure_scope: str
    closure_target: str
    package_count: int
    package_versions: frozenset[tuple[str, str]]


class ClosureVerificationError(RuntimeError):
    """One or more exact artifact closure claims failed."""


CommandRunner = Callable[[Sequence[str]], subprocess.CompletedProcess[str]]


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
        if claim is None:
            claim = ClosureClaim(
                claim_id=f"{profile_id}-exact-runtime-dependency-closure",
                profile_id=profile_id,
                required_packages=(recipe.package,),
                forbidden_packages=(),
            )
        elif recipe.package not in claim.required_packages:
            raise ClosureVerificationError(
                f"semantic claim {claim.claim_id!r} must require descriptor root "
                f"package {recipe.package!r}"
            )
        cases.extend(
            VerificationCase(recipe, claim, target) for target in expected_targets
        )
    return tuple(cases)


def cargo_tree_command(
    case: VerificationCase,
    *,
    repo_root: Path = REPO_ROOT,
    cargo_path: str = "cargo",
    rustc_path: str = "rustc",
) -> list[str]:
    """Project one descriptor-owned case into a runtime-only Cargo tree."""
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

    command = [
        cargo_path,
        "tree",
        "--color",
        "never",
        "--locked",
        "--package",
        recipe.package,
        "--manifest-path",
        str(repo_root / recipe.manifest),
        "--edges",
        "normal,no-proc-macro",
        "--config",
        f"build.rustc={json.dumps(rustc_path)}",
        "--config",
        "build.incremental=false",
        "--prefix",
        "none",
        "--charset",
        "ascii",
        "--format",
        f"{PACKAGE_MARKER}{{p}}\t{FEATURE_MARKER}{{f}}",
        "--no-default-features",
    ]
    if recipe.features:
        command.extend(("--features", recipe.feature_argument))
    command.extend(("--target", case.target))
    return command


def probe_runtime_command(
    probe: DependencyProbe,
    *,
    repo_root: Path = REPO_ROOT,
    cargo_path: str = "cargo",
    rustc_path: str = "rustc",
) -> list[str]:
    """Project one fixed probe into an isolated runtime/legal Cargo tree."""
    return _probe_cargo_tree_command(
        probe,
        repo_root=repo_root,
        cargo_path=cargo_path,
        rustc_path=rustc_path,
        edges="normal,no-proc-macro",
        preserve_hierarchy=False,
    )


def probe_attribution_command(
    probe: DependencyProbe,
    *,
    repo_root: Path = REPO_ROOT,
    cargo_path: str = "cargo",
    rustc_path: str = "rustc",
) -> list[str]:
    """Project one fixed probe into a role-preserving compiler closure."""
    return _probe_cargo_tree_command(
        probe,
        repo_root=repo_root,
        cargo_path=cargo_path,
        rustc_path=rustc_path,
        edges="normal,build",
        preserve_hierarchy=True,
    )


def _probe_cargo_tree_command(
    probe: DependencyProbe,
    *,
    repo_root: Path,
    cargo_path: str,
    rustc_path: str,
    edges: str,
    preserve_hierarchy: bool,
) -> list[str]:
    recipe = probe.recipe
    if recipe.default_features:
        raise ClosureVerificationError(
            f"dependency probe {probe.probe_id!r} must disable Cargo defaults"
        )
    command = [
        cargo_path,
        "tree",
        "--color",
        "never",
        "--locked",
        "--package",
        recipe.package,
        "--manifest-path",
        str(repo_root / recipe.manifest),
        "--edges",
        edges,
        "--config",
        f"build.rustc={json.dumps(rustc_path)}",
        "--config",
        "build.incremental=false",
    ]
    if preserve_hierarchy:
        command.append("--no-dedupe")
    else:
        command.extend(("--prefix", "none"))
    command.extend(
        (
            "--charset",
            "ascii",
            "--format",
            f"{PACKAGE_MARKER}{{p}}\t{FEATURE_MARKER}{{f}}",
            "--no-default-features",
        )
    )
    if recipe.features:
        command.extend(("--features", recipe.feature_argument))
    command.extend(("--target", probe.target))
    return command


def parse_cargo_tree(
    output: str,
    *,
    repo_root: Path = REPO_ROOT,
) -> DependencyClosure:
    """Parse the marker-delimited Cargo tree emitted by this verifier."""
    identity_features: dict[tuple[str, str, str], set[str]] = {}
    malformed: list[str] = []

    for line_number, line in enumerate(output.splitlines(), start=1):
        if not line:
            continue
        if not line.startswith(PACKAGE_MARKER):
            malformed.append(f"line {line_number} lacks the package marker")
            continue
        package_and_features = line[len(PACKAGE_MARKER) :].split(
            f"\t{FEATURE_MARKER}", maxsplit=1
        )
        if len(package_and_features) != 2:
            malformed.append(f"line {line_number} lacks the feature marker")
            continue
        package_display, raw_features = package_and_features
        try:
            package, version, source, _proc_macro = _parse_package_display(
                package_display,
                repo_root=repo_root,
            )
        except ValueError as error:
            malformed.append(f"line {line_number} {error}")
            continue
        parsed_features = {
            feature.strip()
            for feature in raw_features.removesuffix(
                CARGO_DEDUPLICATION_SUFFIX
            ).split(",")
            if feature.strip()
        }
        identity_features.setdefault((package, version, source), set()).update(
            parsed_features
        )

    if malformed:
        raise ClosureVerificationError("; ".join(malformed))
    if not identity_features:
        raise ClosureVerificationError("cargo tree produced no dependency packages")

    return DependencyClosure(
        features_by_package_identity={
            package_id: frozenset(package_features)
            for package_id, package_features in sorted(identity_features.items())
        },
    )


def _parse_package_display(
    package_display: str,
    *,
    repo_root: Path = REPO_ROOT,
) -> tuple[str, str, str, bool]:
    match = PACKAGE_ID_RE.match(package_display)
    if match is None:
        raise ValueError("has an invalid Cargo package display")
    annotations = match.group("annotations")
    proc_macro = CARGO_PROC_MACRO_ANNOTATION.strip() in annotations
    return (
        match.group("name"),
        match.group("version"),
        _normalize_cargo_source(annotations, repo_root=repo_root),
        proc_macro,
    )


def _normalize_cargo_source(
    annotations: str,
    *,
    repo_root: Path = REPO_ROOT,
) -> str:
    source_annotation = annotations.strip()
    proc_macro_annotation = CARGO_PROC_MACRO_ANNOTATION.strip()
    if source_annotation.startswith(proc_macro_annotation):
        source_annotation = source_annotation.removeprefix(
            proc_macro_annotation
        ).strip()
    elif source_annotation.endswith(proc_macro_annotation):
        source_annotation = source_annotation.removesuffix(
            proc_macro_annotation
        ).strip()
    if proc_macro_annotation in source_annotation:
        raise ValueError("has invalid Cargo proc-macro annotations")
    if not source_annotation:
        return DEFAULT_REGISTRY_SOURCE
    if not (source_annotation.startswith("(") and source_annotation.endswith(")")):
        raise ValueError("has invalid Cargo source annotations")

    source = source_annotation[1:-1]
    path = Path(source)
    if path.is_absolute():
        resolved_path = path.resolve()
        try:
            relative_path = resolved_path.relative_to(repo_root.resolve())
        except ValueError:
            return f"path+file://{resolved_path.as_posix()}"
        return f"path+workspace://{relative_path.as_posix()}"
    if source.startswith(("http://", "https://", "ssh://", "git://", "file://")):
        return f"git+{source}"
    return source


def parse_attribution_cargo_tree(
    output: str,
    *,
    repo_root: Path = REPO_ROOT,
) -> AttributionClosure:
    """Parse a non-deduplicated Cargo tree while preserving compiler roles."""
    rows: dict[tuple[str, str, str], dict[str, Any]] = {}
    context_by_depth: dict[int, str] = {}
    build_sections: set[int] = set()
    root_seen = False

    for line_number, line in enumerate(output.splitlines(), start=1):
        if not line:
            continue
        if line.rstrip().endswith("[build-dependencies]"):
            prefix = line[: -len("[build-dependencies]")]
            parent_depth = _tree_continuation_depth(prefix, line_number)
            if parent_depth not in context_by_depth:
                raise ClosureVerificationError(
                    f"line {line_number} has a build section without a package parent"
                )
            build_sections = {
                depth for depth in build_sections if depth <= parent_depth
            }
            build_sections.add(parent_depth)
            continue
        marker_index = line.find(PACKAGE_MARKER)
        if marker_index < 0:
            raise ClosureVerificationError(
                f"line {line_number} is not a package or build-dependencies section"
            )
        prefix = line[:marker_index]
        depth = _tree_package_depth(prefix, line_number)
        if depth == 0:
            if root_seen:
                raise ClosureVerificationError("attribution tree contains multiple roots")
            root_seen = True
            incoming_context = "normal"
        else:
            parent_depth = depth - 1
            parent_context = context_by_depth.get(parent_depth)
            if parent_context is None:
                raise ClosureVerificationError(
                    f"line {line_number} skips its package parent"
                )
            incoming_context = (
                "build" if parent_depth in build_sections else parent_context
            )

        payload = line[marker_index + len(PACKAGE_MARKER) :].split(
            f"\t{FEATURE_MARKER}", maxsplit=1
        )
        if len(payload) != 2:
            raise ClosureVerificationError(
                f"line {line_number} lacks the attribution feature marker"
            )
        package_display, raw_features = payload
        if raw_features.endswith(CARGO_DEDUPLICATION_SUFFIX):
            raise ClosureVerificationError(
                "attribution cargo tree must be collected with --no-dedupe"
            )
        try:
            package, version, source, proc_macro = _parse_package_display(
                package_display,
                repo_root=repo_root,
            )
        except ValueError as error:
            raise ClosureVerificationError(f"line {line_number} {error}") from error
        features = _parse_feature_list(raw_features)

        roles = {incoming_context}
        traversal_context = incoming_context
        if proc_macro:
            roles.add("proc-macro")
            if incoming_context != "build":
                roles.discard("normal")
                traversal_context = "proc-macro"

        identity = (package, version, source)
        row = rows.setdefault(
            identity,
            {
                "features": set(),
                "roles": set(),
                "role_features": {},
            },
        )
        row["features"].update(features)
        row["roles"].update(roles)
        for role in roles:
            row["role_features"].setdefault(role, set()).update(features)

        context_by_depth = {
            existing_depth: context
            for existing_depth, context in context_by_depth.items()
            if existing_depth < depth
        }
        context_by_depth[depth] = traversal_context
        build_sections = {value for value in build_sections if value < depth}

    if not root_seen or not rows:
        raise ClosureVerificationError(
            "cargo tree produced no attribution dependency packages"
        )
    packages = tuple(
        AttributionPackage(
            package=identity[0],
            version=identity[1],
            source=identity[2],
            features=tuple(sorted(row["features"])),
            roles=tuple(sorted(row["roles"])),
            role_features={
                role: tuple(sorted(role_features))
                for role, role_features in sorted(row["role_features"].items())
            },
        )
        for identity, row in sorted(rows.items())
    )
    return AttributionClosure(packages)


def _tree_package_depth(prefix: str, line_number: int) -> int:
    if not prefix:
        return 0
    if len(prefix) % 4 != 0:
        raise ClosureVerificationError(
            f"line {line_number} has malformed Cargo tree indentation"
        )
    groups = tuple(prefix[index : index + 4] for index in range(0, len(prefix), 4))
    if groups[-1] not in {"|-- ", "`-- "} or any(
        group not in {"|   ", "    "} for group in groups[:-1]
    ):
        raise ClosureVerificationError(
            f"line {line_number} has malformed Cargo tree branches"
        )
    return len(groups)


def _tree_continuation_depth(prefix: str, line_number: int) -> int:
    if len(prefix) % 4 != 0:
        raise ClosureVerificationError(
            f"line {line_number} has malformed Cargo tree section indentation"
        )
    groups = tuple(prefix[index : index + 4] for index in range(0, len(prefix), 4))
    if any(group not in {"|   ", "    "} for group in groups):
        raise ClosureVerificationError(
            f"line {line_number} has malformed Cargo tree section branches"
        )
    return len(groups)


def _parse_feature_list(raw_features: str) -> set[str]:
    return {feature.strip() for feature in raw_features.split(",") if feature.strip()}


def closure_fingerprint(closure: DependencyClosure) -> str:
    """Hash one exact source-aware runtime dependency and feature closure."""
    digest = hashlib.sha256()
    digest.update(FINGERPRINT_DOMAIN)
    for (package, version, source), features in sorted(
        closure.features_by_package_identity.items()
    ):
        digest.update(package.encode("utf-8"))
        digest.update(b"\0")
        digest.update(version.encode("utf-8"))
        digest.update(b"\0")
        digest.update(source.encode("utf-8"))
        digest.update(b"\0")
        digest.update(",".join(sorted(features)).encode("utf-8"))
        digest.update(b"\n")
    return f"sha256:{digest.hexdigest()}"


def attribution_fingerprint(closure: AttributionClosure) -> str:
    digest = hashlib.sha256()
    digest.update(ATTRIBUTION_FINGERPRINT_DOMAIN)
    digest.update(
        json.dumps(
            closure.projection(),
            ensure_ascii=True,
            separators=(",", ":"),
            sort_keys=True,
        ).encode()
    )
    return f"sha256:{digest.hexdigest()}"


def check_probe_policy(observation: ProbeClosureObservation) -> list[str]:
    probe = observation.probe
    failures: list[str] = []
    runtime_packages = observation.runtime.packages
    attribution_packages = observation.attribution.package_names
    missing_runtime = sorted(
        set(probe.required_runtime_packages) - runtime_packages
    )
    missing_attribution = sorted(
        set(probe.required_attribution_packages) - attribution_packages
    )
    forbidden = set(probe.forbidden_packages)
    forbidden_attribution = sorted(forbidden & attribution_packages)
    if missing_runtime:
        failures.append("runtime required packages missing: " + ", ".join(missing_runtime))
    if missing_attribution:
        failures.append(
            "attribution required packages missing: " + ", ".join(missing_attribution)
        )
    if forbidden_attribution:
        failures.append(
            "forbidden packages present in compiler closure: "
            + ", ".join(forbidden_attribution)
        )
    return failures


def capture_probe_observations(
    probes: Sequence[DependencyProbe],
    *,
    repo_root: Path = REPO_ROOT,
    cargo_path: str = "cargo",
    rustc_path: str = "rustc",
    runner: CommandRunner,
) -> tuple[ProbeClosureObservation, ...]:
    observations: list[ProbeClosureObservation] = []
    probe_failures: list[tuple[str, str, str]] = []
    for probe in probes:
        try:
            runtime_command = probe_runtime_command(
                probe,
                repo_root=repo_root,
                cargo_path=cargo_path,
                rustc_path=rustc_path,
            )
            runtime_result = runner(runtime_command)
            if runtime_result.returncode != 0:
                raise ClosureVerificationError(
                    _command_failure(probe.probe_id, "runtime", runtime_result)
                )
            if not isinstance(runtime_result.stdout, str):
                raise ClosureVerificationError("runtime cargo tree stdout was not text")
            attribution_command = probe_attribution_command(
                probe,
                repo_root=repo_root,
                cargo_path=cargo_path,
                rustc_path=rustc_path,
            )
            attribution_result = runner(attribution_command)
            if attribution_result.returncode != 0:
                raise ClosureVerificationError(
                    _command_failure(probe.probe_id, "attribution", attribution_result)
                )
            if not isinstance(attribution_result.stdout, str):
                raise ClosureVerificationError(
                    "attribution cargo tree stdout was not text"
                )
            observation = ProbeClosureObservation(
                probe=probe,
                runtime=parse_cargo_tree(runtime_result.stdout, repo_root=repo_root),
                attribution=parse_attribution_cargo_tree(
                    attribution_result.stdout,
                    repo_root=repo_root,
                ),
            )
            failures = check_probe_policy(observation)
            if failures:
                raise ClosureVerificationError(
                    "policy violations: " + "; ".join(failures)
                )
            observations.append(observation)
        except RuntimeError as error:
            probe_failures.append((probe.lane, probe.probe_id, str(error)))
    if probe_failures:
        raise ClosureVerificationError(
            _format_probe_matrix_failures(
                "fixed FFI dependency probe capture failed",
                probes,
                probe_failures,
            )
        )
    return tuple(observations)


def _format_probe_matrix_failures(
    title: str,
    probes: Sequence[DependencyProbe],
    failures: Sequence[tuple[str, str, str]],
) -> str:
    failed_lanes = {lane for lane, _probe_id, _message in failures}
    lines = [title]
    for lane in sorted({probe.lane for probe in probes}):
        lane_count = sum(1 for probe in probes if probe.lane == lane)
        status = "failed" if lane in failed_lanes else "ok"
        lines.append(
            f"ffi-contract-readiness lane={lane} status={status} probes={lane_count}"
        )
    lines.extend(
        f"- lane={lane} probe={probe_id}: {message}"
        for lane, probe_id, message in failures
    )
    return "\n".join(lines)


def _command_failure(
    probe_id: str,
    closure_kind: str,
    completed: subprocess.CompletedProcess[str],
) -> str:
    detail = (completed.stderr or completed.stdout or "").strip() or "<empty output>"
    return (
        f"dependency probe {probe_id!r} {closure_kind} cargo tree failed with "
        f"{completed.returncode}: {detail}"
    )


def dependency_baseline_report(
    observations: Sequence[ProbeClosureObservation],
    *,
    repo_root: Path,
    toolchain: Mapping[str, Any],
    source_snapshot_sha256: str,
) -> dict[str, Any]:
    probes = tuple(observation.probe for observation in observations)
    expected = load_dependency_probes(repo_root)
    if tuple(probe.probe_id for probe in probes) != tuple(
        probe.probe_id for probe in expected
    ):
        raise ClosureVerificationError(
            "captured dependency observations do not match the fixed probe registry"
        )
    runtime_records, runtime_record_indexes = _package_record_table(
        observation.runtime.projection() for observation in observations
    )
    attribution_records, attribution_record_indexes = _package_record_table(
        observation.attribution.projection() for observation in observations
    )
    report: dict[str, Any] = {
        "schema_version": BASELINE_SCHEMA_VERSION,
        "report_id": BASELINE_REPORT_ID,
        "baseline_commit": BASELINE_COMMIT,
        "source_revision": source_revision_projection(source_snapshot_sha256),
        "inputs": baseline_input_records(repo_root, probes),
        "toolchain": dict(toolchain),
        "closure_model": {
            "runtime_legal_edges": "normal,no-proc-macro",
            "attribution_edges": "normal,build",
            "roles": ["build", "normal", "proc-macro"],
            "cargo_tree_deduplication": {
                "runtime_legal": "identity-feature union",
                "attribution": "disabled; hierarchy preserved",
            },
        },
        "probe_registry_sha256": probe_registry_sha256(expected),
        "package_records": {
            "runtime_legal": runtime_records,
            "attribution": attribution_records,
        },
        "probes": [
            _compact_probe_observation_projection(
                observation,
                runtime_record_indexes=runtime_record_indexes,
                attribution_record_indexes=attribution_record_indexes,
            )
            for observation in observations
        ],
    }
    report["report_sha256"] = _embedded_report_sha256(report)
    return report


def baseline_input_records(
    repo_root: Path,
    probes: Sequence[DependencyProbe],
) -> list[dict[str, str]]:
    _validate_baseline_probe_manifests(probes)
    return input_records(repo_root, BASELINE_INPUT_PATHS)


def baseline_input_paths(
    probes: Sequence[DependencyProbe],
) -> tuple[Path, ...]:
    _validate_baseline_probe_manifests(probes)
    return BASELINE_INPUT_PATHS


def _validate_baseline_probe_manifests(
    probes: Sequence[DependencyProbe],
) -> None:
    expected_manifests = {
        path
        for path in BASELINE_INPUT_PATHS
        if path.name == "Cargo.toml" and path != Path("Cargo.toml")
    }
    observed_manifests = {Path(probe.recipe.manifest) for probe in probes}
    if observed_manifests != expected_manifests:
        raise ClosureVerificationError(
            "fixed dependency probe manifest set drifted from the baseline contract"
        )


def _probe_observation_projection(
    observation: ProbeClosureObservation,
) -> dict[str, Any]:
    runtime_packages = observation.runtime.projection()
    attribution_packages = observation.attribution.projection()
    value: dict[str, Any] = {
        "probe": observation.probe.projection(),
        "runtime_legal": {
            "package_count": len(runtime_packages),
            "sha256": closure_fingerprint(observation.runtime),
            "packages": runtime_packages,
        },
        "attribution": {
            "package_count": len(attribution_packages),
            "sha256": attribution_fingerprint(observation.attribution),
            "packages": attribution_packages,
        },
    }
    value["probe_sha256"] = f"sha256:{canonical_sha256(value)}"
    return value


def _compact_probe_observation_projection(
    observation: ProbeClosureObservation,
    *,
    runtime_record_indexes: Mapping[str, int],
    attribution_record_indexes: Mapping[str, int],
) -> dict[str, Any]:
    runtime_packages = observation.runtime.projection()
    attribution_packages = observation.attribution.projection()
    value: dict[str, Any] = {
        "probe": observation.probe.projection(),
        "runtime_legal": {
            "package_count": len(runtime_packages),
            "sha256": closure_fingerprint(observation.runtime),
            "package_refs": [
                runtime_record_indexes[_package_record_key(package)]
                for package in runtime_packages
            ],
        },
        "attribution": {
            "package_count": len(attribution_packages),
            "sha256": attribution_fingerprint(observation.attribution),
            "package_refs": [
                attribution_record_indexes[_package_record_key(package)]
                for package in attribution_packages
            ],
        },
    }
    value["probe_sha256"] = f"sha256:{canonical_sha256(value)}"
    return value


def _package_record_table(
    package_groups: Iterable[Sequence[Mapping[str, Any]]],
) -> tuple[list[dict[str, Any]], dict[str, int]]:
    records_by_key: dict[str, dict[str, Any]] = {}
    for packages in package_groups:
        for package in packages:
            key = _package_record_key(package)
            records_by_key.setdefault(key, dict(package))
    keys = sorted(
        records_by_key,
        key=lambda key: _package_record_sort_key(records_by_key[key]),
    )
    return (
        [records_by_key[key] for key in keys],
        {key: index for index, key in enumerate(keys)},
    )


def _package_record_key(package: Mapping[str, Any]) -> str:
    return json.dumps(
        package,
        ensure_ascii=False,
        separators=(",", ":"),
        sort_keys=True,
    )


def _package_record_sort_key(package: Mapping[str, Any]) -> tuple[str, str, str, str]:
    return (
        str(package["package"]),
        str(package["version"]),
        str(package["source"]),
        _package_record_key(package),
    )


def _embedded_report_sha256(report: Mapping[str, Any]) -> str:
    unsigned = dict(report)
    unsigned.pop("report_sha256", None)
    return f"sha256:{canonical_sha256(unsigned)}"


def load_dependency_baseline(
    baseline_path: Path,
    *,
    lock_path: Path = DEFAULT_BASELINE_LOCK,
    repo_root: Path = REPO_ROOT,
) -> dict[str, Any]:
    raw, parsed = STRICT_BASELINE_JSON.load_bytes(baseline_path)
    file_sha256 = bytes_sha256(raw)
    if file_sha256 in REJECTED_BASELINE_FILE_SHA256:
        raise ClosureVerificationError(
            "dependency baseline is the rejected workspace-feature-unified cargo metadata report"
        )
    report = STRICT_BASELINE_JSON.object(
        parsed,
        "FFI dependency baseline",
    )
    expanded_report = _validate_dependency_report(report, repo_root=repo_root)
    try:
        lock = load_baseline_lock(lock_path)
    except FfiBaselineContractError as error:
        raise ClosureVerificationError(str(error)) from error
    if lock["dependency_report_schema_version"] != BASELINE_SCHEMA_VERSION:
        raise ClosureVerificationError("FFI baseline lock dependency schema is stale")
    if (
        report["source_revision"]["snapshot_sha256"]
        != lock["source_snapshot_sha256"]
    ):
        raise ClosureVerificationError(
            "FFI dependency baseline source snapshot does not match the lock"
        )
    expected_file_sha256 = lock["dependency_report_file_sha256"]
    if file_sha256 != expected_file_sha256:
        raise ClosureVerificationError(
            "FFI dependency baseline whole-file digest does not match the checked-in lock"
        )
    if lock["probe_registry_sha256"] != report["probe_registry_sha256"]:
        raise ClosureVerificationError(
            "FFI dependency baseline probe registry digest does not match its lock"
        )
    expected_inputs = [
        {"path": path, "sha256": digest}
        for path, digest in sorted(lock["baseline_input_sha256"].items())
    ]
    if report["inputs"] != expected_inputs:
        raise ClosureVerificationError(
            "FFI dependency baseline inputs do not match the checked-in source lock"
        )
    return expanded_report


def _validate_dependency_report(
    report: dict[str, Any],
    *,
    repo_root: Path,
) -> dict[str, Any]:
    STRICT_BASELINE_JSON.exact_fields(
        report,
        {
            "schema_version",
            "report_id",
            "baseline_commit",
            "source_revision",
            "inputs",
            "toolchain",
            "closure_model",
            "probe_registry_sha256",
            "package_records",
            "probes",
            "report_sha256",
        },
        "FFI dependency baseline",
    )
    if report.get("schema_version") != BASELINE_SCHEMA_VERSION:
        raise ClosureVerificationError(
            f"FFI dependency baseline schema_version must be {BASELINE_SCHEMA_VERSION}"
        )
    if report.get("report_id") != BASELINE_REPORT_ID:
        raise ClosureVerificationError("FFI dependency baseline report_id is invalid")
    if report.get("baseline_commit") != BASELINE_COMMIT:
        raise ClosureVerificationError("FFI dependency baseline commit is not canonical")
    try:
        validate_source_revision(report.get("source_revision"))
    except FfiBaselineContractError as error:
        raise ClosureVerificationError(str(error)) from error
    if report.get("report_sha256") != _embedded_report_sha256(report):
        raise ClosureVerificationError("FFI dependency baseline embedded digest is stale")
    expected_probes = load_dependency_probes(repo_root)
    expected_registry = probe_registry_sha256(expected_probes)
    if report.get("probe_registry_sha256") != expected_registry:
        raise ClosureVerificationError("FFI dependency baseline probe registry drifted")
    try:
        validate_input_records(
            report.get("inputs"),
            expected_paths=baseline_input_paths(expected_probes),
        )
        validate_rust_toolchain(report.get("toolchain"))
    except FfiBaselineContractError as error:
        raise ClosureVerificationError(str(error)) from error
    _validate_closure_model(report.get("closure_model"))
    package_records = STRICT_BASELINE_JSON.object(
        report.get("package_records"),
        "baseline package records",
    )
    STRICT_BASELINE_JSON.exact_fields(
        package_records,
        {"runtime_legal", "attribution"},
        "baseline package records",
    )
    runtime_records = STRICT_BASELINE_JSON.array(
        package_records.get("runtime_legal"),
        "baseline runtime package records",
    )
    attribution_records = STRICT_BASELINE_JSON.array(
        package_records.get("attribution"),
        "baseline attribution package records",
    )
    raw_probes = STRICT_BASELINE_JSON.array(report.get("probes"), "baseline probes")
    if len(raw_probes) != len(expected_probes):
        raise ClosureVerificationError("FFI dependency baseline probe count drifted")
    runtime_references: set[int] = set()
    attribution_references: set[int] = set()
    expanded_probes: list[dict[str, Any]] = []
    for raw, expected in zip(raw_probes, expected_probes, strict=True):
        expanded, references = _expand_compact_probe_record(
            raw,
            expected,
            runtime_records=runtime_records,
            attribution_records=attribution_records,
        )
        expanded_probes.append(expanded)
        runtime_references.update(references["runtime_legal"])
        attribution_references.update(references["attribution"])
    _validate_package_record_table(
        runtime_records,
        runtime_references,
        "runtime",
    )
    _validate_package_record_table(
        attribution_records,
        attribution_references,
        "attribution",
    )
    expanded_report = dict(report)
    expanded_report["probes"] = expanded_probes
    return expanded_report


def _validate_closure_model(value: Any) -> None:
    model = STRICT_BASELINE_JSON.object(value, "baseline closure model")
    expected = {
        "runtime_legal_edges": "normal,no-proc-macro",
        "attribution_edges": "normal,build",
        "roles": ["build", "normal", "proc-macro"],
        "cargo_tree_deduplication": {
            "runtime_legal": "identity-feature union",
            "attribution": "disabled; hierarchy preserved",
        },
    }
    if model != expected:
        raise ClosureVerificationError("FFI dependency baseline closure model drifted")


def _validate_package_record_table(
    records: Sequence[Mapping[str, Any]],
    references: set[int],
    kind: str,
) -> None:
    if references != set(range(len(records))):
        raise ClosureVerificationError(
            f"baseline {kind} package records must all be referenced"
        )
    sort_keys = [_package_record_sort_key(record) for record in records]
    if sort_keys != sorted(set(sort_keys)):
        raise ClosureVerificationError(
            f"baseline {kind} package records must be sorted and unique"
        )


def _expand_compact_probe_record(
    value: Any,
    expected: DependencyProbe,
    *,
    runtime_records: Sequence[Mapping[str, Any]],
    attribution_records: Sequence[Mapping[str, Any]],
) -> tuple[dict[str, Any], dict[str, list[int]]]:
    record = _validate_probe_envelope(value, expected)
    expanded = dict(record)
    references: dict[str, list[int]] = {}
    for section_name, records, kind in (
        ("runtime_legal", runtime_records, "runtime"),
        ("attribution", attribution_records, "attribution"),
    ):
        section = STRICT_BASELINE_JSON.object(
            record.get(section_name),
            f"{expected.probe_id} {kind} closure",
        )
        STRICT_BASELINE_JSON.exact_fields(
            section,
            {"package_count", "sha256", "package_refs"},
            f"{expected.probe_id} {kind} closure",
        )
        refs = _validate_package_references(
            section.get("package_refs"),
            len(records),
            f"{expected.probe_id} {kind}",
        )
        expanded_section = dict(section)
        expanded_section.pop("package_refs")
        expanded_section["packages"] = [dict(records[index]) for index in refs]
        expanded[section_name] = expanded_section
        references[section_name] = refs
    _validate_runtime_section(expanded["runtime_legal"], expected.probe_id)
    _validate_attribution_section(expanded["attribution"], expected.probe_id)
    return expanded, references


def _validate_probe_envelope(
    value: Any,
    expected: DependencyProbe,
) -> dict[str, Any]:
    record = STRICT_BASELINE_JSON.object(value, f"probe {expected.probe_id}")
    STRICT_BASELINE_JSON.exact_fields(
        record,
        {"probe", "runtime_legal", "attribution", "probe_sha256"},
        f"probe {expected.probe_id}",
    )
    if record.get("probe") != expected.projection():
        raise ClosureVerificationError(
            f"dependency baseline probe recipe drifted: {expected.probe_id}"
        )
    unsigned = dict(record)
    observed_probe_digest = unsigned.pop("probe_sha256", None)
    if observed_probe_digest != f"sha256:{canonical_sha256(unsigned)}":
        raise ClosureVerificationError(
            f"dependency baseline probe digest is stale: {expected.probe_id}"
        )
    return record


def _validate_runtime_section(
    value: Any,
    probe_id: str,
) -> None:
    section = STRICT_BASELINE_JSON.object(value, f"{probe_id} runtime closure")
    STRICT_BASELINE_JSON.exact_fields(
        section,
        {"package_count", "sha256", "packages"},
        f"{probe_id} runtime closure",
    )
    packages = _validate_runtime_packages(section.get("packages"), probe_id)
    if section.get("package_count") != len(packages):
        raise ClosureVerificationError(f"{probe_id} runtime package count is stale")
    closure = DependencyClosure(
        {
            (row["package"], row["version"], row["source"]): frozenset(
                row["features"]
            )
            for row in packages
        }
    )
    if section.get("sha256") != closure_fingerprint(closure):
        raise ClosureVerificationError(f"{probe_id} runtime closure digest is stale")


def _validate_runtime_packages(value: Any, probe_id: str) -> list[dict[str, Any]]:
    packages = STRICT_BASELINE_JSON.array(value, f"{probe_id} runtime packages")
    identities: list[tuple[str, str, str]] = []
    for index, raw in enumerate(packages):
        row = STRICT_BASELINE_JSON.object(raw, f"{probe_id} runtime package[{index}]")
        STRICT_BASELINE_JSON.exact_fields(
            row,
            {"package", "version", "source", "features"},
            f"{probe_id} runtime package[{index}]",
        )
        identity = _validate_package_row(row, f"{probe_id} runtime package[{index}]")
        identities.append(identity)
    if identities != sorted(set(identities)):
        raise ClosureVerificationError(f"{probe_id} runtime packages are not sorted unique")
    return packages


def _validate_attribution_section(
    value: Any,
    probe_id: str,
) -> None:
    section = STRICT_BASELINE_JSON.object(value, f"{probe_id} attribution closure")
    STRICT_BASELINE_JSON.exact_fields(
        section,
        {"package_count", "sha256", "packages"},
        f"{probe_id} attribution closure",
    )
    raw_packages = STRICT_BASELINE_JSON.array(
        section.get("packages"),
        f"{probe_id} attribution packages",
    )
    packages: list[AttributionPackage] = []
    identities: list[tuple[str, str, str]] = []
    for index, raw in enumerate(raw_packages):
        row = STRICT_BASELINE_JSON.object(
            raw,
            f"{probe_id} attribution package[{index}]",
        )
        STRICT_BASELINE_JSON.exact_fields(
            row,
            {"package", "version", "source", "features", "roles", "role_features"},
            f"{probe_id} attribution package[{index}]",
        )
        identity = _validate_package_row(
            row,
            f"{probe_id} attribution package[{index}]",
        )
        roles = _sorted_unique_strings(row.get("roles"), f"{probe_id} roles")
        if not set(roles) <= {"build", "normal", "proc-macro"}:
            raise ClosureVerificationError(f"{probe_id} attribution roles are invalid")
        role_features = STRICT_BASELINE_JSON.object(
            row.get("role_features"),
            f"{probe_id} role_features",
        )
        if tuple(sorted(role_features)) != roles:
            raise ClosureVerificationError(f"{probe_id} role_features keys drifted")
        parsed_role_features = {
            role: tuple(
                _sorted_unique_strings(
                    role_features[role],
                    f"{probe_id} role_features.{role}",
                )
            )
            for role in roles
        }
        features = tuple(row["features"])
        if set(features) != {
            feature
            for role_values in parsed_role_features.values()
            for feature in role_values
        }:
            raise ClosureVerificationError(
                f"{probe_id} attribution feature union drifted"
            )
        packages.append(
            AttributionPackage(
                identity[0],
                identity[1],
                identity[2],
                features,
                roles,
                parsed_role_features,
            )
        )
        identities.append(identity)
    if identities != sorted(set(identities)):
        raise ClosureVerificationError(
            f"{probe_id} attribution packages are not sorted unique"
        )
    closure = AttributionClosure(tuple(packages))
    if section.get("package_count") != len(packages):
        raise ClosureVerificationError(f"{probe_id} attribution package count is stale")
    if section.get("sha256") != attribution_fingerprint(closure):
        raise ClosureVerificationError(f"{probe_id} attribution closure digest is stale")


def _validate_package_references(
    value: Any,
    record_count: int,
    context: str,
) -> list[int]:
    raw_refs = STRICT_BASELINE_JSON.array(value, f"{context} package refs")
    refs: list[int] = []
    for raw in raw_refs:
        if not isinstance(raw, int) or isinstance(raw, bool) or raw < 0:
            raise ClosureVerificationError(
                f"{context} package refs must contain non-negative integers"
            )
        refs.append(raw)
    if refs != sorted(set(refs)):
        raise ClosureVerificationError(
            f"{context} package refs must be sorted and unique"
        )
    if refs and refs[-1] >= record_count:
        raise ClosureVerificationError(f"{context} package ref is out of range")
    return refs


def _validate_package_row(
    row: dict[str, Any],
    context: str,
) -> tuple[str, str, str]:
    package = STRICT_BASELINE_JSON.string(row.get("package"), f"{context}.package")
    version = STRICT_BASELINE_JSON.string(row.get("version"), f"{context}.version")
    source = STRICT_BASELINE_JSON.string(row.get("source"), f"{context}.source")
    _sorted_unique_strings(row.get("features"), f"{context}.features")
    return package, version, source


def _sorted_unique_strings(value: Any, context: str) -> tuple[str, ...]:
    raw = STRICT_BASELINE_JSON.array(value, context)
    parsed = tuple(STRICT_BASELINE_JSON.string(item, context) for item in raw)
    if parsed != tuple(sorted(set(parsed))):
        raise ClosureVerificationError(f"{context} must be sorted and unique")
    return parsed


def verify_dependency_baseline(
    baseline_path: Path,
    *,
    lock_path: Path = DEFAULT_BASELINE_LOCK,
    repo_root: Path = REPO_ROOT,
    runner: CommandRunner,
    rust_toolchain: Mapping[str, Any] | None = None,
) -> tuple[ProbeClosureObservation, ...]:
    try:
        reject_ffi_contract_environment()
        reject_cargo_configuration(repo_root)
        current_toolchain = (
            validate_rust_toolchain(rust_toolchain)
            if rust_toolchain is not None
            else rust_toolchain_provenance(runner)
        )
    except FfiBaselineContractError as error:
        raise ClosureVerificationError(str(error)) from error
    except FfiContractReproducibilityError as error:
        raise ClosureVerificationError(str(error)) from error
    baseline = load_dependency_baseline(
        baseline_path,
        lock_path=lock_path,
        repo_root=repo_root,
    )
    probes = load_dependency_probes(repo_root)
    if rust_toolchain_dependency_compatibility_projection(
        current_toolchain
    ) != rust_toolchain_dependency_compatibility_projection(baseline["toolchain"]):
        raise ClosureVerificationError(
            _format_probe_matrix_failures(
                "fixed FFI dependency baseline toolchain differs from the current "
                f"probe toolchain: baseline={baseline['toolchain']!r} "
                f"current={current_toolchain!r}",
                probes,
                tuple(
                    (lane, "toolchain", "toolchain provenance mismatch")
                    for lane in sorted({probe.lane for probe in probes})
                ),
            )
        )
    observations = capture_probe_observations(
        probes,
        repo_root=repo_root,
        cargo_path=current_toolchain["cargo"]["path"],
        rustc_path=current_toolchain["rustc"]["path"],
        runner=runner,
    )
    failures: list[tuple[str, str, str]] = []
    for baseline_probe, current in zip(baseline["probes"], observations, strict=True):
        current_projection = _probe_observation_projection(current)
        failures.extend(
            (current.probe.lane, current.probe.probe_id, failure)
            for failure in _closure_expansion_failures(
                baseline_probe,
                current_projection,
                current.probe.probe_id,
            )
        )
    if failures:
        raise ClosureVerificationError(
            _format_probe_matrix_failures(
                "fixed FFI dependency baseline verification failed",
                probes,
                failures,
            )
        )
    return observations


def _closure_expansion_failures(
    baseline_probe: Mapping[str, Any],
    current_probe: Mapping[str, Any],
    probe_id: str,
) -> list[str]:
    """Reject dependency growth while allowing removals and feature/role narrowing."""
    failures = _package_expansion_failures(
        baseline_probe["runtime_legal"]["packages"],
        current_probe["runtime_legal"]["packages"],
        context=f"{probe_id}: runtime/legal",
        compare_roles=False,
    )
    failures.extend(
        _package_expansion_failures(
            baseline_probe["attribution"]["packages"],
            current_probe["attribution"]["packages"],
            context=f"{probe_id}: attribution",
            compare_roles=True,
        )
    )
    return failures


def _package_expansion_failures(
    baseline_packages: Sequence[Mapping[str, Any]],
    current_packages: Sequence[Mapping[str, Any]],
    *,
    context: str,
    compare_roles: bool,
) -> list[str]:
    baseline = _packages_by_comparison_identity(baseline_packages, context)
    current = _packages_by_comparison_identity(current_packages, context)
    failures: list[str] = []
    for identity, current_row in current.items():
        baseline_row = baseline.get(identity)
        display = _package_display(current_row)
        if baseline_row is None:
            failures.append(f"{context} closure gained package {display}")
            continue
        added_features = sorted(
            set(current_row["features"]) - set(baseline_row["features"])
        )
        if added_features:
            failures.append(
                f"{context} package {display} gained features: "
                + ", ".join(added_features)
            )
        if not compare_roles:
            continue
        added_roles = sorted(
            set(current_row["roles"]) - set(baseline_row["roles"])
        )
        if added_roles:
            failures.append(
                f"{context} package {display} gained roles: "
                + ", ".join(added_roles)
            )
        for role, current_features in current_row["role_features"].items():
            baseline_features = baseline_row["role_features"].get(role, ())
            added_role_features = sorted(
                set(current_features) - set(baseline_features)
            )
            if added_role_features:
                failures.append(
                    f"{context} package {display} gained {role} features: "
                    + ", ".join(added_role_features)
                )
    return failures


def _packages_by_comparison_identity(
    packages: Sequence[Mapping[str, Any]],
    context: str,
) -> dict[tuple[str, ...], Mapping[str, Any]]:
    indexed: dict[tuple[str, ...], Mapping[str, Any]] = {}
    for row in packages:
        identity = (
            "exact",
            row["package"],
            row["version"],
            row["source"],
        )
        if identity in indexed:
            raise ClosureVerificationError(
                f"{context} has ambiguous comparison identity {identity!r}"
            )
        indexed[identity] = row
    return indexed


def _package_display(package: Mapping[str, Any]) -> str:
    return (
        f"{package['package']} v{package['version']} "
        f"({package['source']})"
    )


def _closure_reduction_summary(
    baseline_probe: Mapping[str, Any],
    current_probe: Mapping[str, Any],
) -> dict[str, int]:
    baseline_runtime = _packages_by_comparison_identity(
        baseline_probe["runtime_legal"]["packages"],
        "baseline runtime reduction summary",
    )
    current_runtime = _packages_by_comparison_identity(
        current_probe["runtime_legal"]["packages"],
        "current runtime reduction summary",
    )
    baseline_attribution = _packages_by_comparison_identity(
        baseline_probe["attribution"]["packages"],
        "baseline attribution reduction summary",
    )
    current_attribution = _packages_by_comparison_identity(
        current_probe["attribution"]["packages"],
        "current attribution reduction summary",
    )
    feature_narrowings = 0
    for baseline, current in (
        (baseline_runtime, current_runtime),
        (baseline_attribution, current_attribution),
    ):
        for identity in baseline.keys() & current.keys():
            feature_narrowings += len(
                set(baseline[identity]["features"])
                - set(current[identity]["features"])
            )
    role_narrowings = 0
    role_feature_narrowings = 0
    for identity in baseline_attribution.keys() & current_attribution.keys():
        baseline_row = baseline_attribution[identity]
        current_row = current_attribution[identity]
        role_narrowings += len(
            set(baseline_row["roles"]) - set(current_row["roles"])
        )
        for role, baseline_features in baseline_row["role_features"].items():
            current_features = current_row["role_features"].get(role, ())
            role_feature_narrowings += len(
                set(baseline_features) - set(current_features)
            )
    return {
        "runtime_removed_packages": len(baseline_runtime.keys() - current_runtime.keys()),
        "attribution_removed_packages": len(
            baseline_attribution.keys() - current_attribution.keys()
        ),
        "feature_narrowings": feature_narrowings,
        "role_narrowings": role_narrowings,
        "role_feature_narrowings": role_feature_narrowings,
    }


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
        package_versions=frozenset(
            (name, version)
            for name, version, _source in closure.features_by_package_identity
        ),
    )
    return failures, observation


def _default_runner(command: Sequence[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        command,
        cwd=REPO_ROOT,
        check=False,
        capture_output=True,
        env=ffi_contract_subprocess_environment(),
        text=True,
    )


def verify_cases(
    cases: Iterable[VerificationCase],
    *,
    runner: CommandRunner = _default_runner,
    repo_root: Path = REPO_ROOT,
    cargo_path: str = "cargo",
    rustc_path: str = "rustc",
) -> tuple[ClosureObservation, ...]:
    """Run every selected profile-target case and aggregate all failures."""
    failures: list[str] = []
    observations: list[ClosureObservation] = []
    command_results: dict[
        tuple[str, ...],
        subprocess.CompletedProcess[str],
    ] = {}
    command_closures: dict[tuple[str, ...], DependencyClosure] = {}

    for case in cases:
        context = (
            f"{case.claim.claim_id} ({case.recipe.profile_id}, "
            f"build-target-kind={case.recipe.build_target_kind}, "
            f"closure-scope={case.closure_scope}, closure-target={case.target})"
        )
        try:
            command = cargo_tree_command(
                case,
                repo_root=repo_root,
                cargo_path=cargo_path,
                rustc_path=rustc_path,
            )
            command_key = tuple(command)
            completed = command_results.get(command_key)
            if completed is None:
                completed = runner(command)
                command_results[command_key] = completed
            closure = command_closures.get(command_key)
            if closure is None:
                if completed.returncode != 0:
                    stderr = (completed.stderr or "").strip() or "<empty stderr>"
                    raise ClosureVerificationError(
                        f"cargo tree exited with {completed.returncode}: {stderr}"
                    )
                if not isinstance(completed.stdout, str):
                    raise ClosureVerificationError("cargo tree stdout was not text")
                closure = parse_cargo_tree(completed.stdout)
                command_closures[command_key] = closure
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


def authoritative_rustsec_profile_ids(
    observations: Iterable[ClosureObservation],
    *,
    running_host_target: str,
) -> frozenset[str]:
    """Select profiles whose package observations are authoritative on this host."""
    return frozenset(
        observation.profile_id
        for observation in observations
        if observation.build_target_kind == "target-set"
        or (
            observation.build_target_kind == "host"
            and observation.closure_target == running_host_target
        )
    )


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
        "--baseline",
        type=Path,
        help="Compare the fixed FFI probe matrix with this immutable baseline report.",
    )
    parser.add_argument(
        "--baseline-lock",
        type=Path,
        default=DEFAULT_BASELINE_LOCK,
        help="Checked-in whole-report digest lock for --baseline.",
    )
    args = parser.parse_args(argv)

    baseline_reductions: dict[str, dict[str, int]] = {}
    try:
        try:
            current_toolchain = rust_toolchain_provenance(_default_runner)
        except FfiContractReproducibilityError as error:
            raise ClosureVerificationError(str(error)) from error
        baseline_observations: tuple[ProbeClosureObservation, ...] = ()
        if args.baseline is not None:
            if args.profile:
                raise ClosureVerificationError(
                    "--baseline cannot be combined with --profile"
                )
            if args.descriptor.resolve() != DEFAULT_DESCRIPTOR.resolve():
                raise ClosureVerificationError(
                    "--baseline requires the canonical artifact profile descriptor"
                )
            baseline_observations = verify_dependency_baseline(
                args.baseline,
                lock_path=args.baseline_lock,
                runner=_default_runner,
                rust_toolchain=current_toolchain,
            )
            loaded_baseline = load_dependency_baseline(
                args.baseline,
                lock_path=args.baseline_lock,
            )
            baseline_reductions = {
                observation.probe.probe_id: _closure_reduction_summary(
                    baseline_probe,
                    _probe_observation_projection(observation),
                )
                for baseline_probe, observation in zip(
                    loaded_baseline["probes"],
                    baseline_observations,
                    strict=True,
                )
            }
        cases = _select_cases(
            load_verification_cases(descriptor_path=args.descriptor),
            args.profile,
        )
        running_host_target = current_toolchain["host_target"]
        observations = verify_cases(
            cases,
            cargo_path=current_toolchain["cargo"]["path"],
            rustc_path=current_toolchain["rustc"]["path"],
        )
        if not args.profile and args.descriptor.resolve() == DEFAULT_DESCRIPTOR.resolve():
            profile_packages: dict[str, frozenset[tuple[str, str]]] = {}
            for observation in observations:
                profile_packages[observation.profile_id] = (
                    profile_packages.get(observation.profile_id, frozenset())
                    | observation.package_versions
                )
            validate_profile_coverage(
                load_exception_records(REPO_ROOT),
                profile_packages,
                authoritative_profile_ids=authoritative_rustsec_profile_ids(
                    observations,
                    running_host_target=running_host_target,
                ),
            )
    except (ClosureVerificationError, RustSecExceptionError) as error:
        print(error, file=sys.stderr)
        return 1

    for lane in sorted(
        {observation.probe.lane for observation in baseline_observations}
    ):
        lane_count = sum(
            1
            for observation in baseline_observations
            if observation.probe.lane == lane
        )
        print(
            f"ffi-contract-readiness lane={lane} status=ok probes={lane_count}"
        )
    for observation in baseline_observations:
        reductions = baseline_reductions[observation.probe.probe_id]
        print(
            "ffi-contract-baseline OK "
            f"probe={observation.probe.probe_id} "
            f"lane={observation.probe.lane} "
            f"target={observation.probe.target} "
            f"runtime-packages={len(observation.runtime.features_by_package_identity)} "
            f"runtime-sha256={closure_fingerprint(observation.runtime)} "
            f"attribution-packages={len(observation.attribution.packages)} "
            f"attribution-sha256={attribution_fingerprint(observation.attribution)} "
            f"runtime-removed-packages={reductions['runtime_removed_packages']} "
            "attribution-removed-packages="
            f"{reductions['attribution_removed_packages']} "
            f"feature-narrowings={reductions['feature_narrowings']} "
            f"role-narrowings={reductions['role_narrowings']} "
            "role-feature-narrowings="
            f"{reductions['role_feature_narrowings']}"
        )
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
