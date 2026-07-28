#!/usr/bin/env python3
"""Read one exact Cargo artifact recipe from the capability descriptor."""

from __future__ import annotations

import argparse
from collections.abc import Mapping
from dataclasses import dataclass
import json
import os
from pathlib import Path, PurePosixPath, PureWindowsPath
import re
import subprocess
import tomllib
from typing import Any, Literal


REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_DESCRIPTOR = REPO_ROOT / "capabilities" / "artifact-profiles-v1.json"
CargoBuildTool = Literal["cargo", "cargo-zigbuild"]
NATIVE_SDK_CARGO_PROFILE = "native-sdk"
PROFILE_ID = re.compile(r"[a-z0-9]+(?:-[a-z0-9]+)*\Z")


class ArtifactProfileError(RuntimeError):
    """A fail-closed artifact profile descriptor violation."""


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
    required_features: tuple[str, ...] = ()

    @property
    def feature_argument(self) -> str:
        return ",".join(self.features)

    @property
    def crate_type_argument(self) -> str:
        return ",".join(self.crate_types)

    @property
    def build_target_argument(self) -> str:
        return ",".join(self.build_targets)


@dataclass(frozen=True)
class ArtifactProfile:
    profile_id: str
    semantic_target: str
    cargo: CargoArtifactRecipe

    def report_projection(self) -> dict[str, Any]:
        build_target: dict[str, Any] = {"kind": self.cargo.build_target_kind}
        if self.cargo.build_target_kind == "target-set":
            build_target["triples"] = list(self.cargo.build_targets)
        return {
            "id": self.profile_id,
            "semantic_target": self.semantic_target,
            "cargo": {
                "package": self.cargo.package,
                "manifest": self.cargo.manifest,
                "profile": self.cargo.cargo_profile,
                "default_features": self.cargo.default_features,
                "features": list(self.cargo.features),
                "target": {
                    "name": self.cargo.target_name,
                    "kinds": list(self.cargo.target_kinds),
                    "crate_types": list(self.cargo.crate_types),
                    "required_features": list(self.cargo.required_features),
                },
                "build_target": build_target,
            },
        }


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
    matches = [
        profile
        for profile in load_artifact_profiles(descriptor_path)
        if profile.profile_id == profile_id
    ]
    if len(matches) != 1:
        raise ArtifactProfileError(
            f"artifact profile {profile_id!r} must occur exactly once; found {len(matches)}"
        )
    return matches[0].cargo


def load_artifact_profiles(
    descriptor_path: Path = DEFAULT_DESCRIPTOR,
) -> tuple[ArtifactProfile, ...]:
    try:
        document = json.loads(
            descriptor_path.read_text(encoding="utf-8"),
            object_pairs_hook=_reject_duplicate_keys,
        )
    except (OSError, json.JSONDecodeError, ValueError) as error:
        raise ArtifactProfileError(
            f"cannot read artifact profile descriptor {descriptor_path}: {error}"
        ) from error

    if not isinstance(document, dict):
        raise ArtifactProfileError("artifact profile descriptor must be a JSON object")
    if document.get("schema_version") != 1:
        raise ArtifactProfileError("artifact profile descriptor schema_version must be 1")

    profiles = document.get("profiles")
    if not isinstance(profiles, list):
        raise ArtifactProfileError(
            "artifact profile descriptor must contain a profiles array"
        )
    if not all(isinstance(profile, dict) for profile in profiles):
        raise ArtifactProfileError(
            "artifact profile descriptor profiles must be JSON objects"
        )

    parsed: list[ArtifactProfile] = []
    seen: set[str] = set()
    for index, profile in enumerate(profiles):
        profile_id = _required_string(profile, "id", f"profile[{index}]")
        if not PROFILE_ID.fullmatch(profile_id):
            raise ArtifactProfileError(
                f"artifact profile id must be a lowercase slug: {profile_id!r}"
            )
        if profile_id in seen:
            raise ArtifactProfileError(
                f"artifact profile {profile_id!r} must occur exactly once; found more than one"
            )
        seen.add(profile_id)
        semantic_target = _required_string(
            profile, "semantic_target", profile_id
        )
        parsed.append(
            ArtifactProfile(
                profile_id=profile_id,
                semantic_target=semantic_target,
                cargo=_parse_cargo_recipe(profile, profile_id),
            )
        )
    return tuple(parsed)


def validate_artifact_profile_manifest(
    root: Path,
    profile: ArtifactProfile,
) -> None:
    recipe = profile.cargo
    manifest_path = root / recipe.manifest
    try:
        if manifest_path.is_symlink() or not manifest_path.is_file():
            raise ArtifactProfileError(
                f"artifact profile {profile.profile_id} manifest must be a regular, "
                f"non-symlink file: {recipe.manifest}"
            )
        document = tomllib.loads(manifest_path.read_text(encoding="utf-8"))
    except ArtifactProfileError:
        raise
    except (OSError, UnicodeError, tomllib.TOMLDecodeError) as error:
        raise ArtifactProfileError(
            f"cannot read artifact profile {profile.profile_id} manifest "
            f"{recipe.manifest}: {error}"
        ) from error
    package = document.get("package")
    if not isinstance(package, dict) or package.get("name") != recipe.package:
        raise ArtifactProfileError(
            f"artifact profile {profile.profile_id} package does not match "
            f"{recipe.manifest}"
        )
    manifest_features = document.get("features")
    if not isinstance(manifest_features, dict):
        raise ArtifactProfileError(
            f"artifact profile {profile.profile_id} manifest has no features table"
        )
    referenced_features = set((*recipe.features, *recipe.required_features))
    missing = sorted(referenced_features - manifest_features.keys())
    if missing:
        raise ArtifactProfileError(
            f"artifact profile {profile.profile_id} references missing Cargo features: "
            + ", ".join(missing)
        )


def _parse_cargo_recipe(
    profile: dict[str, Any],
    profile_id: str,
) -> CargoArtifactRecipe:
    cargo = profile.get("cargo")
    if not isinstance(cargo, dict):
        raise ArtifactProfileError(
            f"artifact profile {profile_id!r} has no Cargo recipe"
        )
    _require_exact_fields(
        cargo,
        {
            "package",
            "manifest",
            "profile",
            "default_features",
            "features",
            "target",
            "build_target",
        },
        f"artifact profile {profile_id!r} Cargo recipe",
    )
    package = _required_string(cargo, "package", profile_id)
    manifest = _required_repo_path(cargo, "manifest", profile_id)
    cargo_profile = _required_string(cargo, "profile", profile_id)
    default_features = cargo.get("default_features")
    if not isinstance(default_features, bool):
        raise ArtifactProfileError(
            f"artifact profile {profile_id!r} default_features must be boolean"
        )
    features = _required_string_list(
        cargo,
        "features",
        profile_id,
        allow_empty=True,
    )
    target = cargo.get("target")
    if not isinstance(target, dict):
        raise ArtifactProfileError(
            f"artifact profile {profile_id!r} has no Cargo target"
        )
    _require_exact_fields(
        target,
        {"name", "kinds", "crate_types", "required_features"},
        f"artifact profile {profile_id!r} Cargo target",
    )
    target_name = _required_string(target, "name", profile_id)
    target_kinds = _required_string_list(target, "kinds", profile_id)
    crate_types = _required_string_list(target, "crate_types", profile_id)
    required_features = _required_string_list(
        target,
        "required_features",
        profile_id,
        allow_empty=True,
    )

    build_target = cargo.get("build_target")
    if not isinstance(build_target, dict):
        raise ArtifactProfileError(
            f"artifact profile {profile_id!r} has no build_target"
        )
    build_target_kind = _required_string(build_target, "kind", profile_id)
    if build_target_kind == "host":
        _require_exact_fields(
            build_target,
            {"kind"},
            f"artifact profile {profile_id!r} build_target",
        )
        build_targets: tuple[str, ...] = ()
    elif build_target_kind == "target-set":
        _require_exact_fields(
            build_target,
            {"kind", "triples"},
            f"artifact profile {profile_id!r} build_target",
        )
        build_targets = _required_string_list(build_target, "triples", profile_id)
    else:
        raise ArtifactProfileError(
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
        required_features=required_features,
    )


def _require_exact_fields(
    value: dict[str, Any],
    expected: set[str],
    context: str,
) -> None:
    missing = sorted(expected - value.keys())
    unknown = sorted(value.keys() - expected)
    if missing:
        raise ArtifactProfileError(
            f"{context} is missing fields: {', '.join(missing)}"
        )
    if unknown:
        raise ArtifactProfileError(
            f"{context} has unknown fields: {', '.join(unknown)}"
        )


def _required_string(value: dict[str, Any], field: str, profile_id: str) -> str:
    raw = value.get(field)
    if not isinstance(raw, str) or not raw:
        raise ArtifactProfileError(
            f"artifact profile {profile_id!r} Cargo field {field!r} must be a string"
        )
    return raw


def _required_repo_path(
    value: dict[str, Any],
    field: str,
    profile_id: str,
) -> str:
    raw = _required_string(value, field, profile_id)
    path = PurePosixPath(raw)
    if (
        path.is_absolute()
        or bool(PureWindowsPath(raw).drive)
        or "\\" in raw
        or path.as_posix() != raw
        or not path.parts
        or any(part in {"", ".", ".."} for part in path.parts)
    ):
        raise ArtifactProfileError(
            f"artifact profile {profile_id!r} Cargo field {field!r} must be a "
            "normalized repository-relative path"
        )
    return raw


def _required_string_list(
    cargo: dict[str, Any],
    field: str,
    profile_id: str,
    *,
    allow_empty: bool = False,
) -> tuple[str, ...]:
    value = cargo.get(field)
    if (
        not isinstance(value, list)
        or (not allow_empty and not value)
        or not all(isinstance(item, str) and item for item in value)
    ):
        qualifier = "" if allow_empty else "non-empty "
        raise ArtifactProfileError(
            f"artifact profile {profile_id!r} Cargo field {field!r} must be "
            f"a {qualifier}list of strings"
        )
    values = tuple(value)
    if list(values) != sorted(set(values)):
        raise ArtifactProfileError(
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
