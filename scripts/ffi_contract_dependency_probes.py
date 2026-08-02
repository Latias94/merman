#!/usr/bin/env python3
"""Exact, verification-only dependency probes for the native SDK contract."""

from __future__ import annotations

from dataclasses import dataclass, replace
import json
from pathlib import Path
from typing import Any

from artifact_profile_recipe import (
    REPO_ROOT,
    CargoArtifactRecipe,
    load_artifact_profile,
)
from strict_json import StrictJsonContract, canonical_sha256


BASELINE_COMMIT = "5117c0ae12da2c0346b47061642286174cea3f5f"
BASELINE_TREE = "4ebfe46d8f48508ac6489d0bfea09ed469d97746"
LINUX_REFERENCE_TARGET = "x86_64-unknown-linux-gnu"
MACOS_ARM64_TARGET = "aarch64-apple-darwin"
ANDROID_ARM64_TARGET = "aarch64-linux-android"
WASM_TARGET = "wasm32-unknown-unknown"
NODE_DESCRIPTOR = Path("platforms/node/candidate-builds.json")

PUBLIC_NATIVE_DENYLIST = (
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

SEMANTIC_BACKEND_DENYLIST = (
    "image",
    "krilla",
    "krilla-svg",
    "manatee",
    "merman-export",
    "merman-layout-elk",
    "merman-render",
    "png",
    "ratex-layout",
    "ratex-parser",
    "ratex-svg",
    "ratex-types",
    "resvg",
    "tiny-skia",
    "usvg",
)


class DependencyProbeError(RuntimeError):
    """The checked-in probe registry or one of its authorities is invalid."""


STRICT_JSON = StrictJsonContract(
    error_factory=DependencyProbeError,
    read_error_prefix="cannot read dependency probe authority",
)


@dataclass(frozen=True)
class DependencyProbe:
    probe_id: str
    lane: str
    recipe: CargoArtifactRecipe
    target: str
    required_runtime_packages: tuple[str, ...]
    required_attribution_packages: tuple[str, ...]
    forbidden_packages: tuple[str, ...]
    synthetic: bool

    def projection(self) -> dict[str, Any]:
        return {
            "id": self.probe_id,
            "lane": self.lane,
            "synthetic": self.synthetic,
            "target": self.target,
            "cargo": {
                "package": self.recipe.package,
                "manifest": self.recipe.manifest,
                "profile": self.recipe.cargo_profile,
                "default_features": self.recipe.default_features,
                "features": list(self.recipe.features),
                "target_name": self.recipe.target_name,
                "crate_types": list(self.recipe.crate_types),
            },
            "required_runtime_packages": list(self.required_runtime_packages),
            "required_attribution_packages": list(
                self.required_attribution_packages
            ),
            "forbidden_packages": list(self.forbidden_packages),
        }


def load_dependency_probes(repo_root: Path = REPO_ROOT) -> tuple[DependencyProbe, ...]:
    """Load the fixed probe registry without turning probes into artifact SKUs."""
    descriptor = repo_root / "capabilities" / "artifact-profiles-v1.json"
    bindings_full = load_artifact_profile(
        "rust-bindings-core-native-sdk", descriptor
    )
    c_abi_full = load_artifact_profile("c-abi-native", descriptor)
    uniffi_full = load_artifact_profile("apple-uniffi-native", descriptor)
    android_full = load_artifact_profile("android-native", descriptor)
    node = _node_recipe(repo_root)

    semantic_forbidden = tuple(
        sorted(set((*PUBLIC_NATIVE_DENYLIST, *SEMANTIC_BACKEND_DENYLIST)))
    )
    public_forbidden = tuple(sorted(PUBLIC_NATIVE_DENYLIST))
    probes = (
        DependencyProbe(
            "bindings-core-semantic-linux",
            "public-native",
            replace(
                bindings_full,
                profile_id="bindings-core-semantic-linux",
                features=(),
            ),
            LINUX_REFERENCE_TARGET,
            ("merman-bindings-core",),
            ("merman-bindings-core",),
            semantic_forbidden,
            True,
        ),
        DependencyProbe(
            "ffi-semantic-linux",
            "public-native",
            replace(
                c_abi_full,
                profile_id="ffi-semantic-linux",
                features=(),
            ),
            LINUX_REFERENCE_TARGET,
            ("merman-ffi",),
            ("merman-ffi",),
            semantic_forbidden,
            True,
        ),
        DependencyProbe(
            "uniffi-semantic-linux",
            "public-native",
            replace(
                uniffi_full,
                profile_id="uniffi-semantic-linux",
                features=(),
            ),
            LINUX_REFERENCE_TARGET,
            ("merman-uniffi", "uniffi"),
            ("merman-uniffi", "uniffi"),
            semantic_forbidden,
            True,
        ),
        DependencyProbe(
            "android-semantic-arm64",
            "public-native",
            replace(
                android_full,
                profile_id="android-semantic-arm64",
                features=(),
            ),
            ANDROID_ARM64_TARGET,
            ("jni", "merman-android-jni"),
            ("jni", "merman-android-jni"),
            semantic_forbidden,
            True,
        ),
        DependencyProbe(
            "ffi-svg-linux",
            "public-native",
            replace(c_abi_full, profile_id="ffi-svg-linux", features=("svg",)),
            LINUX_REFERENCE_TARGET,
            ("merman-ffi", "merman-render"),
            ("merman-ffi", "merman-render"),
            public_forbidden,
            True,
        ),
        DependencyProbe(
            "ffi-full-linux",
            "public-native",
            replace(c_abi_full, profile_id="ffi-full-linux"),
            LINUX_REFERENCE_TARGET,
            ("merman-export", "merman-ffi", "merman-render"),
            ("merman-export", "merman-ffi", "merman-render"),
            public_forbidden,
            False,
        ),
        DependencyProbe(
            "uniffi-full-macos",
            "public-native",
            replace(uniffi_full, profile_id="uniffi-full-macos"),
            MACOS_ARM64_TARGET,
            ("merman-export", "merman-render", "merman-uniffi", "uniffi"),
            ("merman-export", "merman-render", "merman-uniffi", "uniffi"),
            public_forbidden,
            False,
        ),
        DependencyProbe(
            "android-full-arm64",
            "public-native",
            replace(android_full, profile_id="android-full-arm64"),
            ANDROID_ARM64_TARGET,
            ("jni", "merman-android-jni", "merman-render"),
            ("jni", "merman-android-jni", "merman-render"),
            public_forbidden,
            False,
        ),
        DependencyProbe(
            "node-napi-full-macos",
            "private-node",
            replace(
                node,
                profile_id="node-napi-full-macos",
                features=(*node.features, "transport-napi"),
            ),
            MACOS_ARM64_TARGET,
            ("merman-node-candidate", "napi"),
            ("merman-node-candidate", "napi", "napi-build", "napi-derive"),
            (),
            False,
        ),
        DependencyProbe(
            "node-wasm-full",
            "private-node",
            replace(
                node,
                profile_id="node-wasm-full",
                features=(*node.features, "transport-wasm"),
            ),
            WASM_TARGET,
            ("merman-node-candidate", "wasm-bindgen"),
            ("merman-node-candidate", "wasm-bindgen"),
            (),
            False,
        ),
    )
    sorted_probes = tuple(sorted(probes, key=lambda probe: probe.probe_id))
    _validate_probes(sorted_probes)
    return sorted_probes


def probe_registry_sha256(probes: tuple[DependencyProbe, ...]) -> str:
    return f"sha256:{canonical_sha256([probe.projection() for probe in probes])}"


def _node_recipe(repo_root: Path) -> CargoArtifactRecipe:
    document = STRICT_JSON.object(
        STRICT_JSON.load(repo_root / NODE_DESCRIPTOR),
        "Node candidate descriptor",
    )
    if document.get("schema_version") != 3:
        raise DependencyProbeError("Node candidate descriptor schema_version must be 3")
    capability_recipe = STRICT_JSON.object(
        document.get("capability_recipe"),
        "Node candidate capability recipe",
    )
    capabilities = capability_recipe.get("capabilities")
    if (
        not isinstance(capabilities, list)
        or not capabilities
        or not all(isinstance(value, str) and value for value in capabilities)
        or capabilities != sorted(set(capabilities))
    ):
        raise DependencyProbeError(
            "Node candidate capabilities must be sorted unique strings"
        )
    cargo = STRICT_JSON.object(document.get("cargo"), "Node candidate Cargo recipe")
    if cargo.get("manifest") != "crates/merman-node/Cargo.toml":
        raise DependencyProbeError("Node candidate manifest is not canonical")
    if cargo.get("default_features") is not False:
        raise DependencyProbeError("Node candidate must disable Cargo defaults")
    return CargoArtifactRecipe(
        profile_id="node-private-base",
        package="merman-node-candidate",
        manifest="crates/merman-node/Cargo.toml",
        cargo_profile="release",
        default_features=False,
        features=tuple(capabilities),
        target_name="merman_node",
        target_kinds=("cdylib", "rlib"),
        crate_types=("cdylib", "rlib"),
        build_target_kind="target-set",
        build_targets=(
            MACOS_ARM64_TARGET,
            "x86_64-apple-darwin",
            LINUX_REFERENCE_TARGET,
            "x86_64-unknown-linux-musl",
            "x86_64-pc-windows-msvc",
            WASM_TARGET,
        ),
    )


def _validate_probes(probes: tuple[DependencyProbe, ...]) -> None:
    ids = [probe.probe_id for probe in probes]
    if ids != sorted(set(ids)):
        raise DependencyProbeError("dependency probe ids must be sorted and unique")
    for probe in probes:
        if probe.recipe.default_features:
            raise DependencyProbeError(
                f"dependency probe {probe.probe_id!r} must disable default features"
            )
        if probe.recipe.features != tuple(sorted(set(probe.recipe.features))):
            raise DependencyProbeError(
                f"dependency probe {probe.probe_id!r} features must be sorted and unique"
            )
        if probe.required_runtime_packages != tuple(
            sorted(set(probe.required_runtime_packages))
        ):
            raise DependencyProbeError(
                f"dependency probe {probe.probe_id!r} runtime requirements must be sorted"
            )
        if probe.required_attribution_packages != tuple(
            sorted(set(probe.required_attribution_packages))
        ):
            raise DependencyProbeError(
                f"dependency probe {probe.probe_id!r} attribution requirements must be sorted"
            )
        if probe.forbidden_packages != tuple(sorted(set(probe.forbidden_packages))):
            raise DependencyProbeError(
                f"dependency probe {probe.probe_id!r} forbidden packages must be sorted"
            )
        overlap = (
            set(probe.required_runtime_packages)
            | set(probe.required_attribution_packages)
        ) & set(probe.forbidden_packages)
        if overlap:
            raise DependencyProbeError(
                f"dependency probe {probe.probe_id!r} requires and forbids "
                + ", ".join(sorted(overlap))
            )


def main() -> int:
    probes = load_dependency_probes()
    print(
        json.dumps(
            {
                "schema_version": 1,
                "registry_sha256": probe_registry_sha256(probes),
                "probes": [probe.projection() for probe in probes],
            },
            indent=2,
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
