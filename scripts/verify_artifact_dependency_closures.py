#!/usr/bin/env python3
"""Verify dependency-closure claims for exact Cargo artifact profiles."""

from __future__ import annotations

import argparse
from collections.abc import Callable, Iterable, Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
import re
import subprocess
import sys

from artifact_profile_recipe import (
    DEFAULT_DESCRIPTOR,
    REPO_ROOT,
    CargoArtifactRecipe,
    load_artifact_profile,
)


PACKAGE_MARKER = "__MERMAN_CLOSURE_PACKAGE__"
FEATURE_MARKER = "__MERMAN_CLOSURE_FEATURES__"
PACKAGE_NAME_RE = re.compile(r"^(?P<name>[A-Za-z0-9_-]+)(?:\s+v[^\s]+)?(?:\s|$)")


@dataclass(frozen=True)
class PackageFeatureExclusion:
    package: str
    features: tuple[str, ...]


@dataclass(frozen=True)
class ClosureClaim:
    claim_id: str
    profile_id: str
    target: str | None
    required_packages: tuple[str, ...]
    forbidden_packages: tuple[str, ...]
    forbidden_features: tuple[PackageFeatureExclusion, ...] = ()
    observed_residual_packages: tuple[str, ...] = ()


@dataclass(frozen=True)
class DependencyClosure:
    packages: frozenset[str]
    features_by_package: Mapping[str, frozenset[str]]


@dataclass(frozen=True)
class ClosureObservation:
    claim_id: str
    profile_id: str
    package_count: int
    observed_residual_packages: tuple[str, ...]


class ClosureVerificationError(RuntimeError):
    """One or more exact artifact closure claims failed."""


CommandRunner = Callable[[Sequence[str]], subprocess.CompletedProcess[str]]
RecipeLoader = Callable[[str, Path], CargoArtifactRecipe]


CLAIMS = (
    ClosureClaim(
        claim_id="static-svg-is-environment-and-export-free",
        profile_id="rust-static-svg",
        target=None,
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
        claim_id="cli-analysis-is-render-and-tool-free",
        profile_id="cli-analysis",
        target="x86_64-unknown-linux-gnu",
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
                    "jpeg",
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
        claim_id="png-excludes-pdf-backend",
        profile_id="rust-export-png",
        target=None,
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
        target=None,
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


def cargo_tree_command(recipe: CargoArtifactRecipe, target: str | None) -> list[str]:
    """Project one exact recipe into a normal-dependency Cargo tree command."""
    if recipe.default_features:
        raise ClosureVerificationError(
            f"profile {recipe.profile_id!r} must set default_features=false"
        )
    if recipe.build_target_kind == "host":
        if target is not None:
            raise ClosureVerificationError(
                f"profile {recipe.profile_id!r} is host-only and rejects target {target!r}"
            )
    elif recipe.build_target_kind == "target-set":
        if target is None:
            raise ClosureVerificationError(
                f"profile {recipe.profile_id!r} requires a descriptor-owned target"
            )
        if target not in recipe.build_targets:
            raise ClosureVerificationError(
                f"profile {recipe.profile_id!r} does not declare target {target!r}"
            )
    else:
        raise ClosureVerificationError(
            f"profile {recipe.profile_id!r} has unsupported build target "
            f"{recipe.build_target_kind!r}"
        )

    command = [
        "cargo",
        "tree",
        "--locked",
        "--package",
        recipe.package,
        "--manifest-path",
        str(REPO_ROOT / recipe.manifest),
        "--edges",
        "normal",
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
    if target is not None:
        command.extend(("--target", target))
    return command


def parse_cargo_tree(output: str) -> DependencyClosure:
    """Parse the marker-delimited Cargo tree format emitted by this verifier."""
    packages: set[str] = set()
    features: dict[str, set[str]] = {}
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
        match = PACKAGE_NAME_RE.match(package_display)
        if match is None:
            malformed.append(f"line {line_number} has an invalid Cargo package display")
            continue
        package = match.group("name")
        packages.add(package)
        package_features = features.setdefault(package, set())
        package_features.update(
            feature.strip()
            for feature in raw_features.split(",")
            if feature.strip()
        )

    if malformed:
        raise ClosureVerificationError("; ".join(malformed))
    if not packages:
        raise ClosureVerificationError("cargo tree produced no dependency packages")

    return DependencyClosure(
        packages=frozenset(packages),
        features_by_package={
            package: frozenset(package_features)
            for package, package_features in sorted(features.items())
        },
    )


def check_claim(
    claim: ClosureClaim,
    closure: DependencyClosure,
) -> tuple[list[str], ClosureObservation]:
    """Compare one observed dependency closure with its surface-owned claim."""
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
        claim_id=claim.claim_id,
        profile_id=claim.profile_id,
        package_count=len(closure.packages),
        observed_residual_packages=tuple(
            sorted(residual & closure.packages)
        ),
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


def verify_claims(
    claims: Iterable[ClosureClaim],
    *,
    descriptor_path: Path = DEFAULT_DESCRIPTOR,
    runner: CommandRunner = _default_runner,
    recipe_loader: RecipeLoader = load_artifact_profile,
) -> tuple[ClosureObservation, ...]:
    """Run and aggregate every selected exact-profile closure check."""
    failures: list[str] = []
    observations: list[ClosureObservation] = []

    for claim in claims:
        try:
            recipe = recipe_loader(claim.profile_id, descriptor_path)
            command = cargo_tree_command(recipe, claim.target)
            completed = runner(command)
            if completed.returncode != 0:
                stderr = (completed.stderr or "").strip() or "<empty stderr>"
                raise ClosureVerificationError(
                    f"cargo tree exited with {completed.returncode}: {stderr}"
                )
            if not isinstance(completed.stdout, str):
                raise ClosureVerificationError("cargo tree stdout was not text")
            closure = parse_cargo_tree(completed.stdout)
            claim_failures, observation = check_claim(claim, closure)
            observations.append(observation)
            failures.extend(
                f"{claim.claim_id} ({claim.profile_id}): {failure}"
                for failure in claim_failures
            )
        except (ClosureVerificationError, RuntimeError) as error:
            failures.append(f"{claim.claim_id} ({claim.profile_id}): {error}")

    if failures:
        raise ClosureVerificationError(
            "artifact dependency closure verification failed:\n- "
            + "\n- ".join(failures)
        )
    return tuple(observations)


def _select_claims(profile_ids: Sequence[str]) -> tuple[ClosureClaim, ...]:
    if not profile_ids:
        return CLAIMS
    requested = set(profile_ids)
    known = {claim.profile_id for claim in CLAIMS}
    unknown = sorted(requested - known)
    if unknown:
        raise ClosureVerificationError(
            "profiles have no dependency-closure claim: " + ", ".join(unknown)
        )
    return tuple(claim for claim in CLAIMS if claim.profile_id in requested)


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
    args = parser.parse_args(argv)

    try:
        claims = _select_claims(args.profile)
        observations = verify_claims(claims, descriptor_path=args.descriptor)
    except ClosureVerificationError as error:
        print(error, file=sys.stderr)
        return 1

    for observation in observations:
        residual = ",".join(observation.observed_residual_packages) or "none"
        print(
            "artifact-closure OK "
            f"profile={observation.profile_id} "
            f"packages={observation.package_count} "
            f"observed-residual={residual}"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
