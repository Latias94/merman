#!/usr/bin/env python3
"""Verify dependency-closure claims for exact Cargo artifact profiles."""

from __future__ import annotations

import argparse
from collections.abc import Callable, Iterable, Mapping, Sequence
from dataclasses import dataclass, replace
import hashlib
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
# Host artifacts are multi-platform, so their dependency closure is frozen against one explicit
# reference target instead of whichever machine happens to execute the release gate.
PORTABLE_HOST_REFERENCE_TARGET = "x86_64-unknown-linux-gnu"


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
    reference_target: str | None = None
    forbidden_features: tuple[PackageFeatureExclusion, ...] = ()
    observed_residual_packages: tuple[str, ...] = ()
    approved_fingerprint: str = ""
    approved_target_fingerprints: tuple[tuple[str, str], ...] = ()


@dataclass(frozen=True)
class DependencyClosure:
    packages: frozenset[str]
    features_by_package: Mapping[str, frozenset[str]]
    features_by_package_identity: Mapping[
        tuple[str, str, str], frozenset[str]
    ]


@dataclass(frozen=True)
class ClosureObservation:
    claim_id: str
    profile_id: str
    closure_target: str | None
    package_count: int
    packages: frozenset[str]
    package_versions: frozenset[tuple[str, str]]
    observed_residual_packages: tuple[str, ...]
    fingerprint: str


class ClosureVerificationError(RuntimeError):
    """One or more exact artifact closure claims failed."""


CommandRunner = Callable[[Sequence[str]], subprocess.CompletedProcess[str]]
RecipeLoader = Callable[[str, Path], CargoArtifactRecipe]


SEMANTIC_CLAIMS = (
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
        reference_target=PORTABLE_HOST_REFERENCE_TARGET,
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
        approved_fingerprint="sha256:629b6ceec6966c66c8e27238674dd51c633d94b0928ea258f29e5d9bccb5c86e",
    ),
    ClosureClaim(
        claim_id="svg-basic-excludes-optional-engines-and-products",
        profile_id="rust-svg-basic",
        target=None,
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
        reference_target=PORTABLE_HOST_REFERENCE_TARGET,
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
        approved_fingerprint="sha256:065328f5bde2cbf965f8576f5e4c0f038409d33098ecbd45a83a42c2ca0d39e1",
    ),
    ClosureClaim(
        claim_id="cli-analysis-is-render-and-tool-free",
        profile_id="cli-analysis",
        target=None,
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
        approved_target_fingerprints=(
            ("aarch64-apple-darwin", "sha256:a6a0e3ef0bda479f3b39bc9d9d905f113cf716b49d512b6b3d40451283257c16"),
            ("x86_64-apple-darwin", "sha256:a6a0e3ef0bda479f3b39bc9d9d905f113cf716b49d512b6b3d40451283257c16"),
            ("x86_64-pc-windows-msvc", "sha256:c391a77e1230b3f05344d95bb238189dc2f9f8bfa21c3ca24277ec698482aa70"),
            ("x86_64-unknown-linux-gnu", "sha256:a6a0e3ef0bda479f3b39bc9d9d905f113cf716b49d512b6b3d40451283257c16"),
        ),
    ),
    ClosureClaim(
        claim_id="jpeg-excludes-pdf-backend",
        profile_id="rust-export-jpeg",
        target=None,
        required_packages=(
            "image",
            "merman-export",
            "merman-render",
            "resvg",
            "tiny-skia",
            "usvg",
        ),
        forbidden_packages=("krilla", "krilla-svg"),
        reference_target=PORTABLE_HOST_REFERENCE_TARGET,
        forbidden_features=(
            PackageFeatureExclusion("merman-export", ("pdf", "png")),
        ),
        observed_residual_packages=("png",),
        approved_fingerprint="sha256:02e8b979732a50a7421bbbd365991f34834e8de02abfecbe81f5b8b3fcd795ec",
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
        reference_target=PORTABLE_HOST_REFERENCE_TARGET,
        forbidden_features=(
            PackageFeatureExclusion("merman-export", ("jpeg", "pdf")),
        ),
        approved_fingerprint="sha256:3843ecde063a79420298aaee380060cb2085b91045c11edd8bd4b650d304cd60",
    ),
    ClosureClaim(
        claim_id="pdf-records-krilla-svg-residual",
        profile_id="rust-export-pdf",
        target=None,
        required_packages=("krilla", "merman-export", "merman-render"),
        forbidden_packages=(),
        reference_target=PORTABLE_HOST_REFERENCE_TARGET,
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
        approved_fingerprint="sha256:c39a2fcaaf0e8794306c2cc2fe25ddf304360d83d379d1d25597beaa8d0ece1b",
    ),
)


def exact_fingerprint_claim(
    profile_id: str,
    package: str,
    *,
    fingerprint: str = "",
    target_fingerprints: tuple[tuple[str, str], ...] = (),
) -> ClosureClaim:
    return ClosureClaim(
        claim_id=f"{profile_id}-exact-dependency-closure",
        profile_id=profile_id,
        target=None,
        required_packages=(package,),
        forbidden_packages=(),
        reference_target=(
            None if target_fingerprints else PORTABLE_HOST_REFERENCE_TARGET
        ),
        approved_fingerprint=fingerprint,
        approved_target_fingerprints=target_fingerprints,
    )


EXACT_FINGERPRINT_CLAIMS = (
    exact_fingerprint_claim(
        "android-native",
        "merman-android-jni",
        target_fingerprints=(
            ("aarch64-linux-android", "sha256:feb9947e98173e6d1b31a947a795bd0a2179899bb59a0bcd1f7e772eb6e2bd98"),
            ("x86_64-linux-android", "sha256:feb9947e98173e6d1b31a947a795bd0a2179899bb59a0bcd1f7e772eb6e2bd98"),
        ),
    ),
    exact_fingerprint_claim(
        "apple-uniffi-native",
        "merman-uniffi",
        target_fingerprints=(
            ("aarch64-apple-darwin", "sha256:f824dbe9e3d368e0ee79f5b0db7d7ff12236bf5c07127cc0c63df81851ed9d20"),
            ("aarch64-apple-ios", "sha256:159481336816bd132f931c97bfe30157b1727b62703b2f8cab3245a69692afce"),
            ("aarch64-apple-ios-sim", "sha256:159481336816bd132f931c97bfe30157b1727b62703b2f8cab3245a69692afce"),
            ("x86_64-apple-darwin", "sha256:f824dbe9e3d368e0ee79f5b0db7d7ff12236bf5c07127cc0c63df81851ed9d20"),
            ("x86_64-apple-ios", "sha256:159481336816bd132f931c97bfe30157b1727b62703b2f8cab3245a69692afce"),
        ),
    ),
    exact_fingerprint_claim(
        "c-abi-native",
        "merman-ffi",
        fingerprint="sha256:ad3a9998576ea8d4e3621ab398ff1dcbea65ba61f76b215cfff9398932a15b3f",
    ),
    exact_fingerprint_claim(
        "cli-release",
        "merman-cli",
        target_fingerprints=(
            ("aarch64-apple-darwin", "sha256:fd07f528b53ec0d621651bfd7492c661b304aa875c5268d02ac86e26c69a6795"),
            ("x86_64-apple-darwin", "sha256:fd07f528b53ec0d621651bfd7492c661b304aa875c5268d02ac86e26c69a6795"),
            ("x86_64-pc-windows-msvc", "sha256:45f2047712298cbf3ce34e0d223ea35c74a61e366d23f2384de1af5d0b4f8bc3"),
            ("x86_64-unknown-linux-gnu", "sha256:c2f2489bbcb278e22f2b009c32f37ece2e4cec64d4cc3c7d7c7119d9f03bca2b"),
        ),
    ),
    exact_fingerprint_claim(
        "flutter-android-native",
        "merman-ffi",
        target_fingerprints=(
            ("aarch64-linux-android", "sha256:8f25bf2797c967787bda38123ad8ed179bae8f7aea86a38fe15326e452a8caa1"),
            ("x86_64-linux-android", "sha256:8f25bf2797c967787bda38123ad8ed179bae8f7aea86a38fe15326e452a8caa1"),
        ),
    ),
    exact_fingerprint_claim(
        "flutter-desktop-native",
        "merman-ffi",
        target_fingerprints=(
            ("aarch64-apple-darwin", "sha256:8f25bf2797c967787bda38123ad8ed179bae8f7aea86a38fe15326e452a8caa1"),
            ("aarch64-unknown-linux-gnu", "sha256:ad3a9998576ea8d4e3621ab398ff1dcbea65ba61f76b215cfff9398932a15b3f"),
            ("x86_64-apple-darwin", "sha256:8f25bf2797c967787bda38123ad8ed179bae8f7aea86a38fe15326e452a8caa1"),
            ("x86_64-pc-windows-gnu", "sha256:b931b428b561ef91cc3a214535978d6bef7730bef28c7385555d049f2fcfed23"),
            ("x86_64-unknown-linux-gnu", "sha256:ad3a9998576ea8d4e3621ab398ff1dcbea65ba61f76b215cfff9398932a15b3f"),
        ),
    ),
    exact_fingerprint_claim(
        "flutter-ios-native",
        "merman-ffi",
        target_fingerprints=(
            ("aarch64-apple-ios", "sha256:ad3a9998576ea8d4e3621ab398ff1dcbea65ba61f76b215cfff9398932a15b3f"),
            ("aarch64-apple-ios-sim", "sha256:ad3a9998576ea8d4e3621ab398ff1dcbea65ba61f76b215cfff9398932a15b3f"),
            ("x86_64-apple-ios", "sha256:ad3a9998576ea8d4e3621ab398ff1dcbea65ba61f76b215cfff9398932a15b3f"),
        ),
    ),
    exact_fingerprint_claim(
        "lsp-library",
        "merman-lsp",
        fingerprint="sha256:aada1108a8466304389e91604b699cde27e3278fb9195e6fdc91ac5d67beaec3",
    ),
    exact_fingerprint_claim(
        "lsp-stdio-release",
        "merman-lsp",
        target_fingerprints=(
            ("aarch64-apple-darwin", "sha256:c4fe7a86d7281d37d436ddb2e1d916f97d6759ffe69b650b76addbb653953d12"),
            ("x86_64-apple-darwin", "sha256:c4fe7a86d7281d37d436ddb2e1d916f97d6759ffe69b650b76addbb653953d12"),
            ("x86_64-pc-windows-msvc", "sha256:3f86526b62233a6e757d476d8537818f8384033782b465538929fbfaad8f0821"),
            ("x86_64-unknown-linux-gnu", "sha256:c4fe7a86d7281d37d436ddb2e1d916f97d6759ffe69b650b76addbb653953d12"),
        ),
    ),
    exact_fingerprint_claim(
        "python-uniffi-native",
        "merman-uniffi",
        target_fingerprints=(
            ("aarch64-apple-darwin", "sha256:f824dbe9e3d368e0ee79f5b0db7d7ff12236bf5c07127cc0c63df81851ed9d20"),
            ("x86_64-pc-windows-msvc", "sha256:e8792b558016ce687837faaadbe4dabcf16eea270f32a573b30c289275487406"),
            ("x86_64-unknown-linux-gnu", "sha256:f40cbfbd11e5921d09762c477a7cd51fb9d3ffd4e270bd2e3b6a809973a65820"),
        ),
    ),
    exact_fingerprint_claim(
        "rust-all",
        "merman",
        fingerprint="sha256:eaeda8f20e9310dc2aa2474195f4e1f03cb24c781b7d1e0aa7b137aedae0bd09",
    ),
    exact_fingerprint_claim(
        "rust-analysis",
        "merman-analysis",
        fingerprint="sha256:081278a666026672a5271ae85d70fdce161ce54a8eff46c6e5c400b1cdb99a88",
    ),
    exact_fingerprint_claim(
        "rust-ascii",
        "merman-ascii",
        fingerprint="sha256:21469236dd6c26616c5cc08733ab3086d5b16cf9a6703bc414ab42ef089e18d7",
    ),
    exact_fingerprint_claim(
        "rust-bindings-core-native-sdk",
        "merman-bindings-core",
        fingerprint="sha256:316fe75029054c9ab90aba2ea5bc22be624328c1253f2d9e7b95d087e36f71ef",
    ),
    exact_fingerprint_claim(
        "rust-core",
        "merman-core",
        fingerprint="sha256:3fe62bf5edcca9836ffb9ea3cbf8625e5ed8bdc0cc438693bc03ba826e3bd547",
    ),
    exact_fingerprint_claim(
        "rust-editor-core",
        "merman-editor-core",
        fingerprint="sha256:1e80f9f44b3d68c98ead385eb569fc1a31e7dd40ef630d77aae4d7d6ff954811",
    ),
    exact_fingerprint_claim(
        "rust-editor-facade",
        "merman",
        fingerprint="sha256:e60a086188c889b9c0949711be367a9e8a2a86e6df423fe70daec9b6eaa30356",
    ),
    exact_fingerprint_claim(
        "rust-export-native-sdk",
        "merman-export",
        fingerprint="sha256:1c7b6b77bf8c53cc730eaba434a6779a878ca0b8ab0e34c26e9c4210dadca1b7",
    ),
    exact_fingerprint_claim(
        "rust-native-sdk",
        "merman",
        fingerprint="sha256:ad577ef9fcbc69b8d67fda46f65ea947c2dd7427c649ebdf2a532fb84010e8bd",
    ),
    exact_fingerprint_claim(
        "rust-native-svg",
        "merman",
        fingerprint="sha256:51cc796c15914d90b8a1e2b0228acc99c8514562e360f3ebc949acb83582b8f9",
    ),
    exact_fingerprint_claim(
        "rust-render-native-svg",
        "merman-render",
        fingerprint="sha256:b1cfd0f9d73a1ffe0766111b78b0831ad8d7cbf17b625336dbd371e2a5a4e379",
    ),
    exact_fingerprint_claim(
        "rustdoc-static-svg",
        "merman-rustdoc",
        fingerprint="sha256:414bb08c1f538a21a5e8852dd57d07622c0f9d2508bbc0b7967452efcf1c3e2b",
    ),
    exact_fingerprint_claim(
        "typst-wasm",
        "merman-typst-plugin",
        target_fingerprints=(("wasm32-unknown-unknown", "sha256:c2359a21afa7ed4ecdc1b0934c3ba56dca1c19bedcb3811ba1dc3abcfd952e54"),),
    ),
    exact_fingerprint_claim(
        "web-analysis",
        "merman-wasm",
        target_fingerprints=(("wasm32-unknown-unknown", "sha256:6634fe7ac7f8f9d3bf8afc1c022adcc6487dcceccb11771a7a607616e4ec8760"),),
    ),
    exact_fingerprint_claim(
        "web-ascii",
        "merman-wasm",
        target_fingerprints=(("wasm32-unknown-unknown", "sha256:1566d3cd4c6aa2a181e6fedec8f2b09b18a3c5ad0da515148d77c6cab281eeaa"),),
    ),
    exact_fingerprint_claim(
        "web-editor",
        "merman-wasm",
        target_fingerprints=(("wasm32-unknown-unknown", "sha256:80efaba294755dbed0d321e34d070506204838581a85ff832ed82eb45c519b4e"),),
    ),
    exact_fingerprint_claim(
        "web-full",
        "merman-wasm",
        target_fingerprints=(("wasm32-unknown-unknown", "sha256:9cec6344ed2c1da76709eb58ce06d80b793bf50ec1aafb84169ded2b6e898a92"),),
    ),
    exact_fingerprint_claim(
        "web-render",
        "merman-wasm",
        target_fingerprints=(("wasm32-unknown-unknown", "sha256:1ced47958c3301ba832baf01a56053228bb23891106d3e899b924e3a66f55558"),),
    ),
)

CLAIMS = SEMANTIC_CLAIMS + EXACT_FINGERPRINT_CLAIMS


def cargo_tree_command(
    recipe: CargoArtifactRecipe,
    target: str | None,
    *,
    reference_target: str | None = None,
) -> list[str]:
    """Project one exact recipe into a target-stable normal-dependency Cargo tree."""
    if recipe.default_features:
        raise ClosureVerificationError(
            f"profile {recipe.profile_id!r} must set default_features=false"
        )
    if recipe.build_target_kind == "host":
        if target is not None:
            raise ClosureVerificationError(
                f"profile {recipe.profile_id!r} is host-only and rejects target {target!r}"
            )
        if reference_target != PORTABLE_HOST_REFERENCE_TARGET:
            raise ClosureVerificationError(
                f"host profile {recipe.profile_id!r} must use portable reference target "
                f"{PORTABLE_HOST_REFERENCE_TARGET!r}"
            )
        closure_target = reference_target
    elif recipe.build_target_kind == "target-set":
        if reference_target is not None:
            raise ClosureVerificationError(
                f"target-set profile {recipe.profile_id!r} rejects a reference target"
            )
        if target is None:
            raise ClosureVerificationError(
                f"profile {recipe.profile_id!r} requires a descriptor-owned target"
            )
        if target not in recipe.build_targets:
            raise ClosureVerificationError(
                f"profile {recipe.profile_id!r} does not declare target {target!r}"
            )
        closure_target = target
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
    command.extend(("--target", closure_target))
    return command


def parse_cargo_tree(output: str) -> DependencyClosure:
    """Parse the marker-delimited Cargo tree format emitted by this verifier."""
    packages: set[str] = set()
    features: dict[str, set[str]] = {}
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
        packages.add(package)
        raw_features = raw_features.removesuffix(CARGO_DEDUPLICATION_SUFFIX)
        parsed_features = {
            feature.strip()
            for feature in raw_features.split(",")
            if feature.strip()
        }
        package_features = features.setdefault(package, set())
        package_features.update(parsed_features)
        identity_features.setdefault((package, version, source), set()).update(
            parsed_features
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
        features_by_package_identity={
            package_id: frozenset(package_features)
            for package_id, package_features in sorted(identity_features.items())
        },
    )


def _normalize_cargo_source(annotations: str) -> str:
    """Return a stable source identity from Cargo's package display annotations."""
    source_annotation = annotations
    if source_annotation.endswith(CARGO_PROC_MACRO_ANNOTATION):
        source_annotation = source_annotation.removesuffix(
            CARGO_PROC_MACRO_ANNOTATION
        )
    source_annotation = source_annotation.strip()
    if not source_annotation:
        return DEFAULT_REGISTRY_SOURCE
    if not (source_annotation.startswith("(") and source_annotation.endswith(")")):
        raise ValueError("has invalid Cargo source annotations")

    source = source_annotation[1:-1]
    path = Path(source)
    if path.is_absolute():
        resolved_path = path.resolve()
        try:
            relative_path = resolved_path.relative_to(REPO_ROOT.resolve())
        except ValueError:
            return f"path+file://{resolved_path.as_posix()}"
        return f"path+workspace://{relative_path.as_posix()}"
    if source.startswith(("http://", "https://", "ssh://", "git://", "file://")):
        return f"git+{source}"
    return source


def closure_fingerprint(closure: DependencyClosure) -> str:
    """Hash the exact source-aware normal-dependency and Cargo-feature closure."""
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


def check_claim(
    claim: ClosureClaim,
    closure: DependencyClosure,
    *,
    enforce_fingerprint: bool = True,
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

    fingerprint = closure_fingerprint(closure)
    if enforce_fingerprint:
        if not FINGERPRINT_RE.fullmatch(claim.approved_fingerprint):
            failures.append(
                "claim has no valid approved dependency closure fingerprint"
            )
        elif fingerprint != claim.approved_fingerprint:
            failures.append(
                "dependency closure fingerprint drift: "
                f"approved={claim.approved_fingerprint} observed={fingerprint}; "
                "review `cargo tree` output before updating the claim"
            )

    observation = ClosureObservation(
        claim_id=claim.claim_id,
        profile_id=claim.profile_id,
        closure_target=claim.target or claim.reference_target,
        package_count=len(closure.features_by_package_identity),
        packages=closure.packages,
        package_versions=frozenset(
            (name, version)
            for name, version, _source in closure.features_by_package_identity
        ),
        observed_residual_packages=tuple(
            sorted(residual & closure.packages)
        ),
        fingerprint=fingerprint,
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


def _claim_variants(
    claim: ClosureClaim,
    recipe: CargoArtifactRecipe,
) -> tuple[ClosureClaim, ...]:
    """Expand one profile-owned claim into every descriptor-owned target check."""
    if recipe.build_target_kind == "host":
        if claim.approved_target_fingerprints:
            raise ClosureVerificationError(
                f"host profile {recipe.profile_id!r} rejects target-specific fingerprints"
            )
        return (claim,)

    if recipe.build_target_kind != "target-set":
        raise ClosureVerificationError(
            f"profile {recipe.profile_id!r} has unsupported build target kind "
            f"{recipe.build_target_kind!r}"
        )
    if claim.target is not None or claim.reference_target is not None:
        raise ClosureVerificationError(
            f"target-set profile {recipe.profile_id!r} must not select one representative target"
        )
    if claim.approved_fingerprint:
        raise ClosureVerificationError(
            f"target-set profile {recipe.profile_id!r} rejects a profile-wide fingerprint"
        )

    target_fingerprints = claim.approved_target_fingerprints
    approved_targets = tuple(target for target, _ in target_fingerprints)
    if len(approved_targets) != len(set(approved_targets)):
        raise ClosureVerificationError(
            f"target-set profile {recipe.profile_id!r} has duplicate fingerprint targets"
        )
    if approved_targets != recipe.build_targets:
        raise ClosureVerificationError(
            f"target-set profile {recipe.profile_id!r} fingerprints must match descriptor "
            f"targets exactly: expected={recipe.build_targets!r} observed={approved_targets!r}"
        )

    return tuple(
        replace(
            claim,
            target=target,
            approved_fingerprint=fingerprint,
            approved_target_fingerprints=(),
        )
        for target, fingerprint in target_fingerprints
    )


def verify_claims(
    claims: Iterable[ClosureClaim],
    *,
    descriptor_path: Path = DEFAULT_DESCRIPTOR,
    runner: CommandRunner = _default_runner,
    recipe_loader: RecipeLoader = load_artifact_profile,
    enforce_fingerprints: bool = True,
) -> tuple[ClosureObservation, ...]:
    """Run and aggregate every selected exact-profile closure check."""
    failures: list[str] = []
    observations: list[ClosureObservation] = []

    for claim in claims:
        try:
            recipe = recipe_loader(claim.profile_id, descriptor_path)
            variants = _claim_variants(claim, recipe)
        except (ClosureVerificationError, RuntimeError) as error:
            failures.append(f"{claim.claim_id} ({claim.profile_id}): {error}")
            continue

        for variant in variants:
            closure_target = variant.target or variant.reference_target
            context = (
                f"{claim.claim_id} ({claim.profile_id}, target={closure_target})"
            )
            try:
                command = cargo_tree_command(
                    recipe,
                    variant.target,
                    reference_target=variant.reference_target,
                )
                completed = runner(command)
                if completed.returncode != 0:
                    stderr = (completed.stderr or "").strip() or "<empty stderr>"
                    raise ClosureVerificationError(
                        f"cargo tree exited with {completed.returncode}: {stderr}"
                    )
                if not isinstance(completed.stdout, str):
                    raise ClosureVerificationError("cargo tree stdout was not text")
                closure = parse_cargo_tree(completed.stdout)
                claim_failures, observation = check_claim(
                    variant,
                    closure,
                    enforce_fingerprint=enforce_fingerprints,
                )
                observations.append(observation)
                failures.extend(
                    f"{context}: {failure}" for failure in claim_failures
                )
            except (ClosureVerificationError, RuntimeError) as error:
                failures.append(f"{context}: {error}")

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
        claims = _select_claims(args.profile)
        observations = verify_claims(
            claims,
            descriptor_path=args.descriptor,
            enforce_fingerprints=not args.print_fingerprints,
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
            )
    except (ClosureVerificationError, RustSecExceptionError) as error:
        print(error, file=sys.stderr)
        return 1

    for observation in observations:
        residual = ",".join(observation.observed_residual_packages) or "none"
        print(
            "artifact-closure OK "
            f"profile={observation.profile_id} "
            f"target={observation.closure_target or 'unspecified'} "
            f"packages={observation.package_count} "
            f"fingerprint={observation.fingerprint} "
            f"observed-residual={residual}"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
