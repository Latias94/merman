#!/usr/bin/env python3
"""Verify runtime dependency closures for exact Cargo artifact profiles."""

from __future__ import annotations

import argparse
from collections.abc import Callable, Iterable, Mapping, Sequence
from dataclasses import dataclass
import hashlib
from pathlib import Path
import re
import subprocess
import sys

from artifact_dependency_approvals import (
    ARTIFACT_DEPENDENCY_APPROVALS,
    HOST_CLOSURE_REFERENCE_TARGET,
    ApprovalCatalog,
)
from artifact_profile_recipe import (
    DEFAULT_DESCRIPTOR,
    REPO_ROOT,
    ArtifactProfileError,
    CargoArtifactRecipe,
    load_artifact_profiles,
    rustc_host_target,
)
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
FINGERPRINT_RE = re.compile(r"^sha256:[0-9a-f]{64}$")
FINGERPRINT_DOMAIN = b"merman-artifact-dependency-closure-v2\0"
DEFAULT_REGISTRY_SOURCE = "registry+https://github.com/rust-lang/crates.io-index"
CARGO_DEDUPLICATION_SUFFIX = " (*)"
CARGO_PROC_MACRO_ANNOTATION = " (proc-macro)"
LINUX_REFERENCE_SCOPE = "linux-reference"
PROFILE_TARGET_SCOPE = "profile-target"
RESOLVED_REPO_ROOT = REPO_ROOT.resolve()


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
    observed_residual_packages: tuple[str, ...] = ()


@dataclass(frozen=True)
class VerificationCase:
    recipe: CargoArtifactRecipe
    claim: ClosureClaim
    target: str
    approved_fingerprint: str

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
    package_versions: frozenset[tuple[str, str]]
    observed_residual_packages: tuple[str, ...]
    fingerprint: str
    fingerprint_enforced: bool


class ClosureVerificationError(RuntimeError):
    """One or more exact artifact closure claims failed."""


CommandRunner = Callable[[Sequence[str]], subprocess.CompletedProcess[str]]


SEMANTIC_CLAIMS = (
    ClosureClaim(
        claim_id="static-svg-is-environment-and-export-free",
        profile_id="rust-static-svg",
        required_packages=("merman", "merman-core", "merman-render"),
        forbidden_packages=(
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
        forbidden_packages=("krilla", "krilla-svg"),
        forbidden_features=(
            PackageFeatureExclusion("merman-export", ("pdf", "png")),
        ),
        observed_residual_packages=("png",),
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
        forbidden_packages=("krilla", "krilla-svg"),
        forbidden_features=(
            PackageFeatureExclusion("merman-export", ("jpeg", "pdf")),
        ),
    ),
    ClosureClaim(
        claim_id="pdf-records-krilla-svg-residual",
        profile_id="rust-export-pdf",
        required_packages=("krilla", "merman-export", "merman-render"),
        forbidden_packages=(),
        forbidden_features=(
            PackageFeatureExclusion("merman-export", ("jpeg", "png")),
        ),
        observed_residual_packages=(
            "fontdb",
            "gif",
            "image-webp",
            "krilla-svg",
            "memmap2",
            "png",
            "resvg",
            "rustybuzz",
            "tiny-skia",
            "ttf-parser",
            "usvg",
            "zune-jpeg",
        ),
    ),
)


def load_verification_cases(
    *,
    descriptor_path: Path = DEFAULT_DESCRIPTOR,
    approvals: ApprovalCatalog = ARTIFACT_DEPENDENCY_APPROVALS,
    semantic_claims: Sequence[ClosureClaim] = SEMANTIC_CLAIMS,
) -> tuple[VerificationCase, ...]:
    """Join recipes, semantic checks, and approved runtime fingerprints."""
    try:
        profiles = load_artifact_profiles(descriptor_path)
    except ArtifactProfileError as error:
        raise ClosureVerificationError(str(error)) from error
    profile_ids = tuple(profile.profile_id for profile in profiles)
    missing = sorted(set(profile_ids) - set(approvals))
    unexpected = sorted(set(approvals) - set(profile_ids))
    if missing or unexpected:
        raise ClosureVerificationError(
            "artifact dependency approvals must match the profile directory "
            f"exactly: missing={missing!r} unexpected={unexpected!r}"
        )

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

        target_approvals = approvals[profile_id]
        approved_targets = tuple(target for target, _ in target_approvals)
        if len(approved_targets) != len(set(approved_targets)):
            raise ClosureVerificationError(
                f"profile {profile_id!r} has duplicate approval targets"
            )
        if approved_targets != expected_targets:
            raise ClosureVerificationError(
                f"profile {profile_id!r} approval targets must match descriptor "
                f"evidence targets exactly: expected={expected_targets!r} "
                f"observed={approved_targets!r}"
            )
        invalid_targets = [
            target
            for target, fingerprint in target_approvals
            if not FINGERPRINT_RE.fullmatch(fingerprint)
        ]
        if invalid_targets:
            raise ClosureVerificationError(
                f"profile {profile_id!r} has invalid runtime fingerprints for "
                f"targets {invalid_targets!r}"
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
            VerificationCase(recipe, claim, target, fingerprint)
            for target, fingerprint in target_approvals
        )
    return tuple(cases)


def cargo_tree_command(case: VerificationCase) -> list[str]:
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
        "cargo",
        "tree",
        "--color",
        "never",
        "--locked",
        "--package",
        recipe.package,
        "--manifest-path",
        str(REPO_ROOT / recipe.manifest),
        "--edges",
        "normal,no-proc-macro",
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


def parse_cargo_tree(output: str) -> DependencyClosure:
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
        match = PACKAGE_ID_RE.match(package_display)
        if match is None:
            malformed.append(f"line {line_number} has an invalid Cargo package display")
            continue
        package = match.group("name")
        version = match.group("version")
        try:
            source = _normalize_cargo_source(match.group("annotations"))
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


def _normalize_cargo_source(annotations: str) -> str:
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
            relative_path = resolved_path.relative_to(RESOLVED_REPO_ROOT)
        except ValueError:
            return f"path+file://{resolved_path.as_posix()}"
        return f"path+workspace://{relative_path.as_posix()}"
    if source.startswith(("http://", "https://", "ssh://", "git://", "file://")):
        return f"git+{source}"
    return source


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


def check_case(
    case: VerificationCase,
    closure: DependencyClosure,
    *,
    enforce_fingerprint: bool = True,
) -> tuple[list[str], ClosureObservation]:
    """Compare one observed runtime closure with its profile-owned contract."""
    claim = case.claim
    failures: list[str] = []
    required = set(claim.required_packages)
    forbidden = set(claim.forbidden_packages)
    residual = set(claim.observed_residual_packages)

    overlaps = sorted((required | residual) & forbidden)
    if overlaps:
        failures.append(
            "claim lists packages as both required/residual and forbidden: "
            + ", ".join(overlaps)
        )

    missing = sorted((required | residual) - closure.packages)
    if missing:
        failures.append("required packages missing: " + ", ".join(missing))

    present = sorted(forbidden & closure.packages)
    if present:
        failures.append("forbidden packages present: " + ", ".join(present))

    for exclusion in claim.forbidden_features:
        if exclusion.package not in required and exclusion.package not in residual:
            failures.append(
                f"feature exclusion owner {exclusion.package!r} is not a required "
                "or residual package"
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

    fingerprint = closure_fingerprint(closure)
    if enforce_fingerprint and fingerprint != case.approved_fingerprint:
        failures.append(
            "runtime dependency closure fingerprint drift: "
            f"approved={case.approved_fingerprint} observed={fingerprint}; "
            "review `cargo tree` output before updating the approval"
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
        observed_residual_packages=tuple(sorted(residual & closure.packages)),
        fingerprint=fingerprint,
        fingerprint_enforced=enforce_fingerprint,
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
    enforce_fingerprints: bool = True,
    running_host_target: str,
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
            command = cargo_tree_command(case)
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
            enforce_case_fingerprint = enforce_fingerprints and (
                case.recipe.build_target_kind != "host"
                or running_host_target == case.target
            )
            case_failures, observation = check_case(
                case,
                closure,
                enforce_fingerprint=enforce_case_fingerprint,
            )
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
            "profiles have no dependency-closure approval: " + ", ".join(unknown)
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
        "--print-fingerprints",
        action="store_true",
        help=(
            "Validate semantic closure claims and print observed fingerprints "
            "without accepting fingerprint drift."
        ),
    )
    args = parser.parse_args(argv)

    try:
        cases = _select_cases(
            load_verification_cases(descriptor_path=args.descriptor),
            args.profile,
        )
        try:
            running_host_target = rustc_host_target()
        except RuntimeError as error:
            raise ClosureVerificationError(str(error)) from error
        observations = verify_cases(
            cases,
            enforce_fingerprints=not args.print_fingerprints,
            running_host_target=running_host_target,
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

    for observation in observations:
        residual = ",".join(observation.observed_residual_packages) or "none"
        print(
            "artifact-closure OK "
            f"profile={observation.profile_id} "
            f"build-target-kind={observation.build_target_kind} "
            f"closure-scope={observation.closure_scope} "
            f"closure-target={observation.closure_target} "
            f"verifier-host={running_host_target} "
            "closure=runtime "
            f"packages={observation.package_count} "
            f"fingerprint={observation.fingerprint} "
            f"fingerprint-enforced={str(observation.fingerprint_enforced).lower()} "
            f"observed-residual={residual}"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
