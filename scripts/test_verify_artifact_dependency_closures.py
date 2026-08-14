#!/usr/bin/env python3
"""Tests for exact artifact runtime dependency-closure verification."""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from typing import Sequence


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from artifact_profile_recipe import (  # noqa: E402
    CargoArtifactRecipe,
    load_artifact_profile,
    load_artifact_profiles,
)
from verify_artifact_dependency_closures import (  # noqa: E402
    HOST_CLOSURE_REFERENCE_TARGET,
    LINUX_REFERENCE_SCOPE,
    NATIVE_BINDING_FORBIDDEN_PACKAGES,
    NATIVE_BINDING_PROFILE_IDS,
    PROFILE_TARGET_SCOPE,
    TREE_SITTER_FORBIDDEN_PACKAGES,
    SEMANTIC_CLAIMS,
    ClosureClaim,
    ClosureVerificationError,
    PackageFeatureExclusion,
    VerificationCase,
    _lockfile_external_identities,
    _select_cases,
    cargo_metadata_command,
    check_case,
    load_verification_cases,
    parse_cargo_metadata,
    select_representative_cases,
    verify_cases,
    write_metadata_probe,
)


def recipe(
    profile_id: str,
    *,
    package: str = "fixture",
    features: tuple[str, ...] = (),
    default_features: bool = False,
    build_target_kind: str = "host",
    build_targets: tuple[str, ...] = (),
) -> CargoArtifactRecipe:
    return CargoArtifactRecipe(
        profile_id=profile_id,
        package=package,
        manifest=f"crates/{package}/Cargo.toml",
        cargo_profile="release",
        default_features=default_features,
        features=features,
        target_name=package.replace("-", "_"),
        target_kinds=("lib",),
        crate_types=("lib",),
        build_target_kind=build_target_kind,
        build_targets=build_targets,
    )


def claim(
    profile_id: str,
    *,
    required: tuple[str, ...] = ("fixture",),
    forbidden: tuple[str, ...] = (),
    forbidden_features: tuple[PackageFeatureExclusion, ...] = (),
) -> ClosureClaim:
    return ClosureClaim(
        claim_id=f"{profile_id}-claim",
        profile_id=profile_id,
        required_packages=required,
        forbidden_packages=forbidden,
        forbidden_features=forbidden_features,
    )


def case(
    profile_id: str = "fixture",
    *,
    loaded_recipe: CargoArtifactRecipe | None = None,
    loaded_claim: ClosureClaim | None = None,
    target: str = HOST_CLOSURE_REFERENCE_TARGET,
) -> VerificationCase:
    return VerificationCase(
        recipe=loaded_recipe or recipe(profile_id),
        claim=loaded_claim or claim(profile_id),
        target=target,
    )


def write_test_metadata_probe(
    current: VerificationCase,
    probe_dir: Path,
) -> Path:
    manifest = write_metadata_probe(
        current,
        probe_dir,
        package_metadata={
            "id": "fixture",
            "name": current.recipe.package,
            "version": "1.2.3",
            "edition": "2024",
            "features": {
                feature: [] for feature in current.recipe.features
            },
            "dependencies": [],
            "manifest_path": str(
                SCRIPT_DIR.parent / current.recipe.manifest
            ),
        },
    )
    (probe_dir / "Cargo.lock").write_text(
        'version = 4\n\n[[package]]\nname = "fixture"\nversion = "1.2.3"\n',
        encoding="utf-8",
    )
    return manifest


def metadata_document(
    root_package: str,
    package_features: dict[str, tuple[str, ...]],
    *,
    edges: dict[str, tuple[str, ...]] | None = None,
    proc_macros: tuple[str, ...] = (),
) -> dict[str, object]:
    package_ids = {
        package: f"registry+https://github.com/rust-lang/crates.io-index#{package}@1.2.3"
        for package in package_features
    }
    graph = edges or {
        root_package: tuple(
            package for package in package_features if package != root_package
        )
    }
    packages: list[dict[str, object]] = []
    nodes: list[dict[str, object]] = []
    for package, features in package_features.items():
        package_id = package_ids[package]
        dependencies = graph.get(package, ())
        packages.append(
            {
                "id": package_id,
                "name": package,
                "version": "1.2.3",
                "source": "registry+https://github.com/rust-lang/crates.io-index",
                "manifest_path": f"/registry/{package}/Cargo.toml",
                "targets": [
                    {"kind": ["proc-macro" if package in proc_macros else "lib"]}
                ],
            }
        )
        nodes.append(
            {
                "id": package_id,
                "dependencies": [package_ids[dependency] for dependency in dependencies],
                "deps": [
                    {
                        "name": dependency.replace("-", "_"),
                        "pkg": package_ids[dependency],
                        "dep_kinds": [{"kind": None, "target": None}],
                    }
                    for dependency in dependencies
                ],
                "features": list(features),
            }
        )
    return {
        "packages": packages,
        "resolve": {"root": package_ids[root_package], "nodes": nodes},
    }


def write_descriptor(
    directory: Path,
    *profile_ids: str,
    build_target: dict[str, object] | None = None,
) -> Path:
    path = directory / "profiles.json"
    path.write_text(
        json.dumps(
            {
                "schema_version": 1,
                "profiles": [
                    {
                        "id": profile_id,
                        "semantic_target": "native",
                        "cargo": {
                            "package": "fixture",
                            "manifest": "Cargo.toml",
                            "profile": "release",
                            "default_features": False,
                            "features": ["fixture"],
                            "target": {
                                "name": "fixture",
                                "kinds": ["bin"],
                                "crate_types": ["bin"],
                                "required_features": [],
                            },
                            "build_target": build_target or {"kind": "host"},
                        },
                    }
                    for profile_id in profile_ids
                ],
            }
        ),
        encoding="utf-8",
    )
    return path


class VerificationCaseTests(unittest.TestCase):
    def test_tree_sitter_packages_are_forbidden_from_every_production_profile(self) -> None:
        loaded_case = case(
            loaded_claim=claim(
                "fixture",
                required=("fixture",),
            )
        )
        closure = parse_cargo_metadata(
            metadata_document(
                "fixture",
                {
                    "fixture": (),
                    "tree-sitter": (),
                },
            ),
            root_package="fixture",
        )

        failures, _observation = check_case(loaded_case, closure)

        self.assertEqual(
            failures,
            ["forbidden packages present: tree-sitter"],
        )
        self.assertIn("tree-sitter-mermaid", TREE_SITTER_FORBIDDEN_PACKAGES)

    def test_repository_cases_cover_governed_profiles_and_declared_targets(self) -> None:
        profiles = load_artifact_profiles()
        cases = load_verification_cases()
        governed_profiles = {
            claim.profile_id for claim in SEMANTIC_CLAIMS
        } | NATIVE_BINDING_PROFILE_IDS
        expected_targets = {
            profile.profile_id: (
                (HOST_CLOSURE_REFERENCE_TARGET,)
                if profile.cargo.build_target_kind == "host"
                else profile.cargo.build_targets
            )
            for profile in profiles
            if profile.profile_id in governed_profiles
        }
        actual_targets: dict[str, list[str]] = {}
        for current in cases:
            actual_targets.setdefault(current.recipe.profile_id, []).append(current.target)

        self.assertEqual(
            {
                profile_id: tuple(targets)
                for profile_id, targets in actual_targets.items()
            },
            expected_targets,
        )

    def test_semantic_claims_must_reference_known_profiles(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            descriptor = write_descriptor(Path(temporary_directory), "known")
            with self.assertRaisesRegex(
                ClosureVerificationError,
                r"unknown profiles: \['unexpected'\]",
            ):
                load_verification_cases(
                    descriptor_path=descriptor,
                    semantic_claims=(claim("unexpected"),),
                )

    def test_cases_follow_descriptor_target_order(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            descriptor = write_descriptor(
                Path(temporary_directory),
                "cross",
                build_target={
                    "kind": "target-set",
                    "triples": ["target-one", "target-two"],
                },
            )
            cases = load_verification_cases(
                descriptor_path=descriptor,
                semantic_claims=(claim("cross"),),
            )

        self.assertEqual(
            tuple(current.target for current in cases),
            ("target-one", "target-two"),
        )

    def test_representative_selection_keeps_first_target_per_profile(self) -> None:
        loaded = recipe(
            "cross",
            package="root",
            build_target_kind="target-set",
            build_targets=("target-one", "target-two"),
        )
        cases = (
            case("cross", loaded_recipe=loaded, target="target-one"),
            case("cross", loaded_recipe=loaded, target="target-two"),
            case("host", target=HOST_CLOSURE_REFERENCE_TARGET),
        )

        selected = select_representative_cases(cases)

        self.assertEqual(
            tuple((current.recipe.profile_id, current.target) for current in selected),
            (
                ("cross", "target-one"),
                ("host", HOST_CLOSURE_REFERENCE_TARGET),
            ),
        )

    def test_only_semantic_and_native_binding_profiles_have_cases(self) -> None:
        cases = load_verification_cases()
        semantic_profiles = {claim.profile_id for claim in SEMANTIC_CLAIMS}
        governed_profiles = semantic_profiles | NATIVE_BINDING_PROFILE_IDS

        self.assertEqual(
            {current.recipe.profile_id for current in cases},
            governed_profiles,
        )

        for current in cases:
            with self.subTest(
                profile=current.recipe.profile_id,
                target=current.target,
            ):
                expected_scope = (
                    LINUX_REFERENCE_SCOPE
                    if current.recipe.build_target_kind == "host"
                    else PROFILE_TARGET_SCOPE
                )
                self.assertEqual(current.closure_scope, expected_scope)
                if current.recipe.profile_id in NATIVE_BINDING_PROFILE_IDS:
                    self.assertEqual(
                        current.claim.required_packages,
                        (current.recipe.package,),
                    )

    def test_native_binding_profiles_share_one_dependency_denylist(self) -> None:
        claims = {
            current.recipe.profile_id: current.claim
            for current in load_verification_cases()
        }

        for profile_id in NATIVE_BINDING_PROFILE_IDS:
            with self.subTest(profile=profile_id):
                self.assertEqual(
                    claims[profile_id].forbidden_packages,
                    NATIVE_BINDING_FORBIDDEN_PACKAGES,
                )


class DescriptorTests(unittest.TestCase):
    def test_maintenance_profiles_are_default_empty_exact_recipes(self) -> None:
        expected = {
            "cli-analysis": ("merman-cli", ("analysis",)),
            "rust-export-jpeg": ("merman-export", ("jpeg",)),
            "rust-export-pdf": ("merman-export", ("pdf",)),
            "rust-export-png": ("merman-export", ("png",)),
            "rust-svg-basic": ("merman", ("svg",)),
        }
        for profile_id, (package, features) in expected.items():
            with self.subTest(profile_id=profile_id):
                loaded = load_artifact_profile(profile_id)
                self.assertFalse(loaded.default_features)
                self.assertEqual((loaded.package, loaded.features), (package, features))

class CargoMetadataCommandTests(unittest.TestCase):
    def test_command_uses_exact_recipe_target_and_feature_recipe(self) -> None:
        loaded = recipe(
            "cli-analysis",
            package="merman-cli",
            features=("analysis",),
            build_target_kind="target-set",
            build_targets=("x86_64-unknown-linux-gnu",),
        )
        probe_manifest = Path("/tmp/merman-closure-probe/Cargo.toml")
        command = cargo_metadata_command(
            case(
                "cli-analysis",
                loaded_recipe=loaded,
                target="x86_64-unknown-linux-gnu",
            ),
            probe_manifest=probe_manifest,
        )

        self.assertEqual(command[:2], ["cargo", "metadata"])
        self.assertIn("--format-version", command)
        self.assertIn("1", command)
        self.assertIn("--frozen", command)
        self.assertIn("--no-default-features", command)
        self.assertNotIn("--package", command)
        self.assertEqual(command[command.index("--features") + 1], "analysis")
        self.assertEqual(
            command[command.index("--filter-platform") + 1],
            "x86_64-unknown-linux-gnu",
        )
        self.assertEqual(
            command[command.index("--manifest-path") + 1],
            str(probe_manifest),
        )

    def test_probe_projects_features_and_normal_dependency_semantics(self) -> None:
        loaded = recipe(
            "cli-analysis",
            package="merman-cli",
            features=("analysis",),
        )
        package_metadata = {
            "id": "merman-cli-id",
            "name": "merman-cli",
            "version": "1.2.3",
            "edition": "2024",
            "manifest_path": str(SCRIPT_DIR.parent / loaded.manifest),
            "features": {"analysis": ["dep:renamed-core"]},
            "dependencies": [
                {
                    "name": "merman-core",
                    "source": None,
                    "req": "^1.2.3",
                    "kind": None,
                    "rename": "renamed-core",
                    "optional": True,
                    "uses_default_features": False,
                    "features": ["svg"],
                    "target": "cfg(unix)",
                    "registry": None,
                    "path": str(SCRIPT_DIR.parent / "crates/merman-core"),
                },
                {
                    "name": "ignored-build-tool",
                    "source": "registry+https://github.com/rust-lang/crates.io-index",
                    "req": "^1",
                    "kind": "build",
                    "rename": None,
                    "optional": False,
                    "uses_default_features": True,
                    "features": [],
                    "target": None,
                    "registry": None,
                },
            ],
        }

        with tempfile.TemporaryDirectory() as temporary_directory:
            manifest = write_metadata_probe(
                case("cli-analysis", loaded_recipe=loaded),
                Path(temporary_directory),
                package_metadata=package_metadata,
            )
            text = manifest.read_text(encoding="utf-8")

        self.assertIn('resolver = "2"', text)
        self.assertIn('"analysis" = ["dep:renamed-core"]', text)
        self.assertIn('[target."cfg(unix)".dependencies]', text)
        self.assertIn('"renamed-core" = {', text)
        self.assertIn('package = "merman-core"', text)
        self.assertIn('optional = true', text)
        self.assertIn('default-features = false', text)
        self.assertIn('features = ["svg"]', text)
        self.assertNotIn("ignored-build-tool", text)

    def test_probe_rejects_an_empty_explicit_package_projection(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            with self.assertRaisesRegex(
                ClosureVerificationError,
                "does not match recipe root",
            ):
                write_metadata_probe(
                    case(),
                    Path(temporary_directory),
                    package_metadata={},
                )

    def test_lockfile_external_identities_ignore_workspace_packages(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            lockfile = Path(temporary_directory) / "Cargo.lock"
            lockfile.write_text(
                """\
version = 4

[[package]]
name = "workspace-root"
version = "1.0.0"

[[package]]
name = "registry-dependency"
version = "2.0.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
""",
                encoding="utf-8",
            )

            identities = _lockfile_external_identities(lockfile)

        self.assertEqual(
            identities,
            frozenset(
                {
                    (
                        "registry-dependency",
                        "2.0.0",
                        "registry+https://github.com/rust-lang/crates.io-index",
                    )
                }
            ),
        )

    def test_host_requires_the_linux_reference_target(self) -> None:
        with self.assertRaisesRegex(
            ClosureVerificationError,
            "Linux reference target",
        ):
            cargo_metadata_command(
                case(target="aarch64-apple-darwin"),
                probe_manifest=Path("/tmp/probe/Cargo.toml"),
            )

    def test_target_set_rejects_an_undeclared_target(self) -> None:
        loaded = recipe(
            "cross",
            build_target_kind="target-set",
            build_targets=("target-one",),
        )
        with self.assertRaisesRegex(ClosureVerificationError, "does not declare target"):
            cargo_metadata_command(
                case(
                    "cross",
                    loaded_recipe=loaded,
                    target="target-two",
                ),
                probe_manifest=Path("/tmp/probe/Cargo.toml"),
            )

    def test_command_rejects_default_features(self) -> None:
        with self.assertRaisesRegex(ClosureVerificationError, "default_features=false"):
            cargo_metadata_command(
                case(loaded_recipe=recipe("fixture", default_features=True)),
                probe_manifest=Path("/tmp/probe/Cargo.toml"),
            )


class CargoMetadataParserTests(unittest.TestCase):
    def test_parser_traverses_normal_dependencies_and_unions_features(self) -> None:
        closure = parse_cargo_metadata(
            metadata_document(
                "app",
                {"app": ("one", "two"), "dep": ("dep-feature",), "proc": ()},
                edges={"app": ("dep", "proc")},
                proc_macros=("proc",),
            ),
            root_package="app",
        )
        self.assertEqual(closure.packages, frozenset({"app", "dep"}))
        self.assertEqual(closure.features_by_package["app"], {"one", "two"})

class ClaimTests(unittest.TestCase):
    def test_native_binding_claim_rejects_tooling_and_application_dependencies(
        self,
    ) -> None:
        loaded_claim = next(
            current.claim
            for current in load_verification_cases()
            if current.recipe.profile_id == "c-abi-native"
        )
        closure = parse_cargo_metadata(
            metadata_document(
                "merman-ffi",
                {
                    "merman-ffi": (),
                    "merman-cli": (),
                    "reqwest": (),
                    "tokio": (),
                    "uniffi_bindgen": (),
                },
            ),
            root_package="merman-ffi",
        )

        failures, _ = check_case(
            case("c-abi-native", loaded_claim=loaded_claim),
            closure,
        )

        self.assertEqual(len(failures), 1)
        for package in ("merman-cli", "reqwest", "tokio", "uniffi_bindgen"):
            self.assertIn(package, failures[0])

    def test_semantic_failures_are_reported_together(self) -> None:
        loaded_claim = claim(
            "semantic",
            required=("root", "missing"),
            forbidden=("forbidden",),
            forbidden_features=(
                PackageFeatureExclusion("root", ("bad-feature",)),
            ),
        )
        closure = parse_cargo_metadata(
            metadata_document(
                "root",
                {"root": ("bad-feature",), "forbidden": ()},
            ),
            root_package="root",
        )
        failures, _ = check_case(case(loaded_claim=loaded_claim), closure)
        self.assertTrue(any("required packages missing: missing" in x for x in failures))
        self.assertTrue(any("forbidden packages present" in x for x in failures))
        self.assertTrue(any("enables forbidden features" in x for x in failures))

    def test_observation_keeps_readable_package_count(self) -> None:
        closure = parse_cargo_metadata(
            metadata_document("fixture", {"fixture": (), "dependency": ()}),
            root_package="fixture",
        )

        failures, observation = check_case(case(), closure)

        self.assertEqual(failures, [])
        self.assertEqual(observation.package_count, 2)

    def test_svg_basic_semantics_reject_optional_product_leaks(self) -> None:
        loaded_claim = next(
            current
            for current in SEMANTIC_CLAIMS
            if current.profile_id == "rust-svg-basic"
        )
        closure = parse_cargo_metadata(
            metadata_document(
                "merman",
                {
                    "merman": ("layout-elk", "svg"),
                    "merman-core": ("system-timezone",),
                    "merman-render": ("math",),
                    "merman-analysis": (),
                    "merman-layout-elk": (),
                },
            ),
            root_package="merman",
        )
        failures, _ = check_case(
            case("rust-svg-basic", loaded_claim=loaded_claim),
            closure,
        )
        self.assertTrue(any("forbidden packages present" in x for x in failures))
        self.assertTrue(any("layout-elk" in x for x in failures))
        self.assertTrue(any("system-timezone" in x for x in failures))
        self.assertTrue(any("math" in x for x in failures))


class VerificationTests(unittest.TestCase):
    def test_every_target_runs_once_and_produces_runtime_evidence(self) -> None:
        targets = ("target-one", "target-two")
        loaded = recipe(
            "cross",
            package="root",
            build_target_kind="target-set",
            build_targets=targets,
        )
        output = json.dumps(metadata_document("root", {"root": ()}))
        cases = tuple(
            case(
                "cross",
                loaded_recipe=loaded,
                loaded_claim=claim("cross", required=("root",)),
                target=target,
            )
            for target in targets
        )
        commands: list[Sequence[str]] = []

        def runner(command: Sequence[str]) -> subprocess.CompletedProcess[str]:
            commands.append(command)
            return subprocess.CompletedProcess(command, 0, stdout=output, stderr="")

        observations = verify_cases(
            cases,
            runner=runner,
            probe_preparer=write_test_metadata_probe,
        )

        self.assertEqual(
            tuple(
                command[command.index("--filter-platform") + 1]
                for command in commands
            ),
            targets,
        )
        self.assertTrue(all("--offline" in command for command in commands))
        self.assertTrue(all("--frozen" not in command for command in commands))
        self.assertEqual(
            tuple(observation.closure_target for observation in observations),
            targets,
        )
        self.assertEqual(
            {observation.closure_scope for observation in observations},
            {PROFILE_TARGET_SCOPE},
        )

    def test_identical_cargo_metadata_commands_are_reused_across_claims(self) -> None:
        loaded = recipe("shared", package="root")
        output = json.dumps(metadata_document("root", {"root": ()}))
        cases = tuple(
            case(
                "shared",
                loaded_recipe=loaded,
                loaded_claim=claim("shared", required=("root",)),
            )
            for _ in range(2)
        )
        commands: list[Sequence[str]] = []

        def runner(command: Sequence[str]) -> subprocess.CompletedProcess[str]:
            commands.append(command)
            return subprocess.CompletedProcess(command, 0, stdout=output, stderr="")

        observations = verify_cases(
            cases,
            runner=runner,
            probe_preparer=write_test_metadata_probe,
        )

        self.assertEqual(len(commands), 1)
        self.assertEqual(len(observations), 2)

    def test_semantic_claims_are_enforced_for_host_cases(self) -> None:
        semantic_case = case(
            loaded_claim=claim("fixture", forbidden=("forbidden",)),
        )
        with self.assertRaisesRegex(
            ClosureVerificationError,
            "forbidden packages present",
        ):
            verify_cases(
                (semantic_case,),
                runner=lambda command: subprocess.CompletedProcess(
                    command,
                    0,
                    stdout=json.dumps(
                        metadata_document(
                            "fixture",
                            {"fixture": (), "forbidden": ()},
                        )
                    ),
                    stderr="",
                ),
                probe_preparer=write_test_metadata_probe,
            )

    def test_failures_are_aggregated_across_profiles(self) -> None:
        cases = tuple(
            case(
                profile_id,
                loaded_recipe=recipe(profile_id, package=profile_id),
                loaded_claim=claim(
                    profile_id,
                    required=(f"root-{profile_id}",),
                    forbidden=(f"bad-{profile_id}",),
                ),
            )
            for profile_id in ("one", "two")
        )

        outputs = iter(
            json.dumps(
                metadata_document(
                    profile_id,
                    {profile_id: (), f"bad-{profile_id}": ()},
                )
            )
            for profile_id in ("one", "two")
        )

        def runner(command: Sequence[str]) -> subprocess.CompletedProcess[str]:
            return subprocess.CompletedProcess(
                command,
                0,
                stdout=next(outputs),
                stderr="",
            )

        with self.assertRaises(ClosureVerificationError) as raised:
            verify_cases(
                cases,
                runner=runner,
                probe_preparer=write_test_metadata_probe,
            )

        message = str(raised.exception)
        self.assertIn("one-claim (one", message)
        self.assertIn("two-claim (two", message)
        self.assertIn("required packages missing", message)
        self.assertIn("forbidden packages present", message)

    def test_runner_failure_is_fail_closed(self) -> None:
        commands: list[Sequence[str]] = []

        def runner(command: Sequence[str]) -> subprocess.CompletedProcess[str]:
            commands.append(command)
            return subprocess.CompletedProcess(
                command,
                101,
                stdout="",
                stderr="dependency resolution failed",
            )

        with self.assertRaisesRegex(
            ClosureVerificationError,
            "dependency resolution failed",
        ):
            verify_cases(
                (case(), case()),
                runner=runner,
                probe_preparer=write_test_metadata_probe,
            )

        self.assertEqual(len(commands), 1)

    def test_unknown_profile_selection_is_rejected(self) -> None:
        with self.assertRaisesRegex(
            ClosureVerificationError,
            "no dependency-closure recipe",
        ):
            _select_cases((case(),), ("not-a-profile",))


if __name__ == "__main__":
    unittest.main()
