#!/usr/bin/env python3
"""Read one exact Cargo artifact recipe from the capability descriptor."""

from __future__ import annotations

import argparse
from collections.abc import Mapping
from dataclasses import dataclass
import json
import os
from pathlib import Path
import subprocess
from typing import Any, Literal


REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_DESCRIPTOR = REPO_ROOT / "capabilities" / "artifact-profiles-v1.json"
CargoBuildTool = Literal["cargo", "cargo-zigbuild"]
NATIVE_SDK_CARGO_PROFILE = "native-sdk"


def _reject_duplicate_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate JSON key {key!r}")
        result[key] = value
    return result


@dataclass(frozen=True)
class CargoArtifactRecipe:
    profile_id: str
    package: str
    manifest: str
    cargo_profile: str
    default_features: bool
    features: tuple[str, ...]
    target_name: str
    target_kinds: tuple[str, ...]
    crate_types: tuple[str, ...]
    build_target_kind: str
    build_targets: tuple[str, ...]

    @property
    def feature_argument(self) -> str:
        return ",".join(self.features)

    @property
    def crate_type_argument(self) -> str:
        return ",".join(self.crate_types)

    @property
    def build_target_argument(self) -> str:
        return ",".join(self.build_targets)


def reject_cargo_profile_environment_overrides(
    recipe: CargoArtifactRecipe,
    environment: Mapping[str, str] | None = None,
) -> None:
    """Keep the repository-owned native SDK profile exact at execution time."""
    if recipe.cargo_profile != NATIVE_SDK_CARGO_PROFILE:
        return

    values = os.environ if environment is None else environment
    prefix = "CARGO_PROFILE_NATIVE_SDK_"
    overrides = sorted(key for key in values if key.startswith(prefix))
    if overrides:
        raise RuntimeError(
            f"artifact profile {recipe.profile_id!r} rejects Cargo profile "
            f"environment overrides: {', '.join(overrides)}"
        )


def cargo_build_args(
    recipe: CargoArtifactRecipe,
    *,
    locked: bool = False,
    target: str | None = None,
    build_tool: CargoBuildTool = "cargo",
) -> list[str]:
    """Project one validated descriptor recipe into a Cargo build command."""
    reject_cargo_profile_environment_overrides(recipe)
    if recipe.build_target_kind == "host":
        if target is not None:
            raise RuntimeError(
                f"artifact profile {recipe.profile_id!r} is host-only and rejects --target"
            )
    elif target is None:
        raise RuntimeError(
            f"artifact profile {recipe.profile_id!r} requires one descriptor-owned --target"
        )
    elif target not in recipe.build_targets:
        raise RuntimeError(
            f"artifact profile {recipe.profile_id!r} does not declare target {target!r}"
        )

    kinds = set(recipe.target_kinds)
    crate_types = set(recipe.crate_types)
    if kinds != crate_types:
        raise RuntimeError(
            f"artifact profile {recipe.profile_id!r} target kinds and crate types differ"
        )
    if kinds == {"bin"} and crate_types == {"bin"}:
        target_args = ["--bin", recipe.target_name]
    elif kinds & {"lib", "proc-macro", "cdylib", "rlib", "staticlib"} and crate_types & {
        "lib",
        "proc-macro",
        "cdylib",
        "rlib",
        "staticlib",
    }:
        target_args = ["--lib"]
    else:
        raise RuntimeError(
            f"artifact profile {recipe.profile_id!r} has unsupported Cargo target contract"
        )

    if build_tool == "cargo":
        command = ["cargo", "build"]
    elif build_tool == "cargo-zigbuild":
        command = ["cargo", "zigbuild"]
    else:
        raise RuntimeError(f"unsupported Cargo build tool {build_tool!r}")

    args = [
        *command,
        "--profile",
        recipe.cargo_profile,
        "--package",
        recipe.package,
        *target_args,
        "--manifest-path",
        str(REPO_ROOT / recipe.manifest),
    ]
    if locked:
        args.append("--locked")
    if not recipe.default_features:
        args.append("--no-default-features")
    if recipe.features:
        args.extend(["--features", recipe.feature_argument])
    if target is not None:
        args.extend(["--target", target])
    return args


def cargo_run_example_args(
    recipe: CargoArtifactRecipe,
    example: str,
    *,
    locked: bool = False,
    extra_features: tuple[str, ...] = (),
    example_args: tuple[str, ...] = (),
) -> list[str]:
    """Project descriptor-owned Cargo selectors into a maintenance example."""
    reject_cargo_profile_environment_overrides(recipe)
    if not example:
        raise RuntimeError("Cargo example name must not be empty")
    if any(not feature for feature in extra_features):
        raise RuntimeError("extra Cargo features must not be empty")

    features = tuple(sorted(set((*recipe.features, *extra_features))))
    args = [
        "cargo",
        "run",
        "--profile",
        recipe.cargo_profile,
        "--package",
        recipe.package,
        "--manifest-path",
        str(REPO_ROOT / recipe.manifest),
    ]
    if locked:
        args.append("--locked")
    if not recipe.default_features:
        args.append("--no-default-features")
    if features:
        args.extend(["--features", ",".join(features)])
    args.extend(["--example", example])
    if example_args:
        args.append("--")
        args.extend(example_args)
    return args


def load_artifact_profile(
    profile_id: str,
    descriptor_path: Path = DEFAULT_DESCRIPTOR,
) -> CargoArtifactRecipe:
    try:
        document = json.loads(
            descriptor_path.read_text(encoding="utf-8"),
            object_pairs_hook=_reject_duplicate_keys,
        )
    except (OSError, json.JSONDecodeError, ValueError) as error:
        raise RuntimeError(
            f"cannot read artifact profile descriptor {descriptor_path}: {error}"
        ) from error

    if not isinstance(document, dict):
        raise RuntimeError("artifact profile descriptor must be a JSON object")

    profiles = document.get("profiles")
    if not isinstance(profiles, list):
        raise RuntimeError("artifact profile descriptor must contain a profiles array")
    if not all(isinstance(profile, dict) for profile in profiles):
        raise RuntimeError("artifact profile descriptor profiles must be JSON objects")
    matches = [profile for profile in profiles if profile.get("id") == profile_id]
    if len(matches) != 1:
        raise RuntimeError(
            f"artifact profile {profile_id!r} must occur exactly once; found {len(matches)}"
        )

    cargo = matches[0].get("cargo")
    if not isinstance(cargo, dict):
        raise RuntimeError(f"artifact profile {profile_id!r} has no Cargo recipe")
    package = _required_string(cargo, "package", profile_id)
    manifest = _required_string(cargo, "manifest", profile_id)
    cargo_profile = _required_string(cargo, "profile", profile_id)
    default_features = cargo.get("default_features")
    if not isinstance(default_features, bool):
        raise RuntimeError(
            f"artifact profile {profile_id!r} default_features must be boolean"
        )
    features_value = cargo.get("features")
    if not isinstance(features_value, list) or not all(
        isinstance(feature, str) and feature for feature in features_value
    ):
        raise RuntimeError(
            f"artifact profile {profile_id!r} features must be non-empty strings"
        )
    features = tuple(features_value)
    if list(features) != sorted(set(features)):
        raise RuntimeError(
            f"artifact profile {profile_id!r} features must be sorted and unique"
        )
    target = cargo.get("target")
    if not isinstance(target, dict):
        raise RuntimeError(f"artifact profile {profile_id!r} has no Cargo target")
    target_name = _required_string(target, "name", profile_id)
    target_kinds = _required_string_list(target, "kinds", profile_id)
    crate_types = _required_string_list(target, "crate_types", profile_id)

    build_target = cargo.get("build_target")
    if not isinstance(build_target, dict):
        raise RuntimeError(f"artifact profile {profile_id!r} has no build_target")
    build_target_kind = _required_string(build_target, "kind", profile_id)
    if build_target_kind == "host":
        if "triples" in build_target:
            raise RuntimeError(
                f"artifact profile {profile_id!r} host build_target must not declare triples"
            )
        build_targets: tuple[str, ...] = ()
    elif build_target_kind == "target-set":
        build_targets = _required_string_list(build_target, "triples", profile_id)
    else:
        raise RuntimeError(
            f"artifact profile {profile_id!r} has unsupported build_target kind "
            f"{build_target_kind!r}"
        )

    return CargoArtifactRecipe(
        profile_id=profile_id,
        package=package,
        manifest=manifest,
        cargo_profile=cargo_profile,
        default_features=default_features,
        features=features,
        target_name=target_name,
        target_kinds=target_kinds,
        crate_types=crate_types,
        build_target_kind=build_target_kind,
        build_targets=build_targets,
    )


def _required_string(cargo: dict[str, Any], field: str, profile_id: str) -> str:
    value = cargo.get(field)
    if not isinstance(value, str) or not value:
        raise RuntimeError(
            f"artifact profile {profile_id!r} Cargo field {field!r} must be a string"
        )
    return value


def _required_string_list(
    cargo: dict[str, Any], field: str, profile_id: str
) -> tuple[str, ...]:
    value = cargo.get(field)
    if not isinstance(value, list) or not value or not all(
        isinstance(item, str) and item for item in value
    ):
        raise RuntimeError(
            f"artifact profile {profile_id!r} Cargo field {field!r} must be "
            "a non-empty list of strings"
        )
    values = tuple(value)
    if list(values) != sorted(set(values)):
        raise RuntimeError(
            f"artifact profile {profile_id!r} Cargo field {field!r} must be sorted and unique"
        )
    return values


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("profile_id")
    parser.add_argument(
        "--field",
        choices=(
            "features",
            "package",
            "manifest",
            "profile",
            "default-features",
            "target",
            "target-kinds",
            "crate-types",
            "build-target",
            "triples",
        ),
        default="features",
    )
    parser.add_argument("--descriptor", type=Path, default=DEFAULT_DESCRIPTOR)
    parser.add_argument(
        "--build",
        action="store_true",
        help="Execute the descriptor-owned Cargo build recipe.",
    )
    parser.add_argument(
        "--build-tool",
        choices=("cargo", "cargo-zigbuild"),
        default="cargo",
        help="Cargo build frontend used with --build.",
    )
    parser.add_argument(
        "--run-example",
        metavar="NAME",
        help="Run one maintenance example with descriptor-owned Cargo selectors.",
    )
    parser.add_argument(
        "--extra-feature",
        action="append",
        default=[],
        help="Add one maintenance-only feature to --run-example.",
    )
    parser.add_argument(
        "--example-argument",
        action="append",
        default=[],
        help="Append one argument to --run-example; use = for values beginning with --.",
    )
    parser.add_argument("--locked", action="store_true", help="Pass --locked to Cargo.")
    parser.add_argument(
        "--target-triple",
        help="Build one triple from a target-set recipe; rejected for host recipes.",
    )
    args = parser.parse_args()

    recipe = load_artifact_profile(args.profile_id, args.descriptor)
    if args.build and args.run_example is not None:
        parser.error("--build and --run-example are mutually exclusive")
    if args.build:
        if args.extra_feature or args.example_argument:
            parser.error("--extra-feature and --example-argument require --run-example")
        subprocess.run(
            cargo_build_args(
                recipe,
                locked=args.locked,
                target=args.target_triple,
                build_tool=args.build_tool,
            ),
            cwd=REPO_ROOT,
            check=True,
        )
        return 0
    if args.run_example is not None:
        if args.target_triple is not None:
            parser.error("--target-triple requires --build")
        if args.build_tool != "cargo":
            parser.error("--build-tool requires --build")
        subprocess.run(
            cargo_run_example_args(
                recipe,
                args.run_example,
                locked=args.locked,
                extra_features=tuple(args.extra_feature),
                example_args=tuple(args.example_argument),
            ),
            cwd=REPO_ROOT,
            check=True,
        )
        return 0
    if (
        args.locked
        or args.target_triple is not None
        or args.build_tool != "cargo"
        or args.extra_feature
        or args.example_argument
    ):
        parser.error(
            "--locked, --target-triple, --build-tool, --extra-feature, and "
            "--example-argument require an execution mode"
        )
    values = {
        "features": recipe.feature_argument,
        "package": recipe.package,
        "manifest": recipe.manifest,
        "profile": recipe.cargo_profile,
        "default-features": "true" if recipe.default_features else "false",
        "target": recipe.target_name,
        "target-kinds": ",".join(recipe.target_kinds),
        "crate-types": recipe.crate_type_argument,
        "build-target": recipe.build_target_kind,
        "triples": recipe.build_target_argument,
    }
    print(values[args.field])
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
