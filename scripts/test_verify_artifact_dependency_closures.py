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
    FEATURE_MARKER,
    HOST_CLOSURE_REFERENCE_TARGET,
    LINUX_REFERENCE_SCOPE,
    PACKAGE_MARKER,
    PROFILE_TARGET_SCOPE,
    SEMANTIC_CLAIMS,
    ClosureClaim,
    ClosureObservation,
    ClosureVerificationError,
    PackageFeatureExclusion,
    VerificationCase,
    _select_cases,
    authoritative_rustsec_profile_ids,
    cargo_tree_command,
    check_case,
    load_verification_cases,
    parse_cargo_tree,
    verify_cases,
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


def tree_line(
    package: str,
    features: tuple[str, ...] = (),
    *,
    version: str = "1.2.3",
    source: str | None = None,
    proc_macro: bool = False,
) -> str:
    annotation = f" ({source})" if source is not None else ""
    if proc_macro:
        annotation += " (proc-macro)"
    return (
        f"__MERMAN_CLOSURE_PACKAGE__{package} v{version}{annotation}"
        f"\t__MERMAN_CLOSURE_FEATURES__{','.join(features)}"
    )


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
    def test_repository_cases_cover_every_profile_and_declared_target(self) -> None:
        profiles = load_artifact_profiles()
        cases = load_verification_cases()
        expected_targets = {
            profile.profile_id: (
                (HOST_CLOSURE_REFERENCE_TARGET,)
                if profile.cargo.build_target_kind == "host"
                else profile.cargo.build_targets
            )
            for profile in profiles
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
                semantic_claims=(),
            )

        self.assertEqual(
            tuple(current.target for current in cases),
            ("target-one", "target-two"),
        )

    def test_recipe_derives_scope_and_exact_root_claim(self) -> None:
        cases = load_verification_cases()
        semantic_profiles = {claim.profile_id for claim in SEMANTIC_CLAIMS}

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
                if current.recipe.profile_id not in semantic_profiles:
                    self.assertEqual(
                        current.claim.required_packages,
                        (current.recipe.package,),
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

class CargoTreeCommandTests(unittest.TestCase):
    def test_command_uses_exact_recipe_target_and_runtime_edges(self) -> None:
        loaded = recipe(
            "cli-analysis",
            package="merman-cli",
            features=("analysis",),
            build_target_kind="target-set",
            build_targets=("x86_64-unknown-linux-gnu",),
        )
        command = cargo_tree_command(
            case(
                "cli-analysis",
                loaded_recipe=loaded,
                target="x86_64-unknown-linux-gnu",
            )
        )

        self.assertEqual(command[:2], ["cargo", "tree"])
        self.assertEqual(command[command.index("--color") + 1], "never")
        self.assertIn("--locked", command)
        self.assertIn("--no-default-features", command)
        self.assertEqual(command[command.index("--package") + 1], "merman-cli")
        self.assertEqual(command[command.index("--features") + 1], "analysis")
        self.assertEqual(
            command[command.index("--target") + 1],
            "x86_64-unknown-linux-gnu",
        )
        self.assertEqual(
            command[command.index("--edges") + 1],
            "normal,no-proc-macro",
        )

    def test_host_requires_the_linux_reference_target(self) -> None:
        with self.assertRaisesRegex(
            ClosureVerificationError,
            "Linux reference target",
        ):
            cargo_tree_command(case(target="aarch64-apple-darwin"))

    def test_target_set_rejects_an_undeclared_target(self) -> None:
        loaded = recipe(
            "cross",
            build_target_kind="target-set",
            build_targets=("target-one",),
        )
        with self.assertRaisesRegex(ClosureVerificationError, "does not declare target"):
            cargo_tree_command(
                case(
                    "cross",
                    loaded_recipe=loaded,
                    target="target-two",
                )
            )

    def test_command_rejects_default_features(self) -> None:
        with self.assertRaisesRegex(ClosureVerificationError, "default_features=false"):
            cargo_tree_command(
                case(loaded_recipe=recipe("fixture", default_features=True))
            )


class CargoTreeParserTests(unittest.TestCase):
    def test_parser_unions_duplicate_package_features(self) -> None:
        closure = parse_cargo_tree(
            "\n".join(
                (
                    tree_line("root", ("one",)),
                    tree_line("root", ("two",)) + " (*)",
                )
            )
        )
        self.assertEqual(closure.packages, frozenset({"root"}))
        self.assertEqual(closure.features_by_package["root"], {"one", "two"})

    def test_parser_normalizes_workspace_registry_git_and_proc_macro_sources(self) -> None:
        workspace_path = SCRIPT_DIR.parent / "crates/merman"
        closure = parse_cargo_tree(
            "\n".join(
                (
                    tree_line("workspace", source=str(workspace_path)),
                    tree_line("registry"),
                    tree_line(
                        "git-package",
                        source="https://example.com/repo?rev=main#01234567",
                    ),
                    tree_line("macro", proc_macro=True),
                    tree_line(
                        "workspace-macro",
                        source=str(workspace_path),
                        proc_macro=True,
                    ),
                    (
                        f"{PACKAGE_MARKER}workspace-macro-first v1.2.3 "
                        f"(proc-macro) ({workspace_path})\t{FEATURE_MARKER}svg"
                    ),
                )
            )
        )
        identities = set(closure.features_by_package_identity)
        self.assertIn(
            ("workspace", "1.2.3", "path+workspace://crates/merman"),
            identities,
        )
        self.assertIn(
            (
                "registry",
                "1.2.3",
                "registry+https://github.com/rust-lang/crates.io-index",
            ),
            identities,
        )
        self.assertTrue(
            any(
                name == "git-package" and source.startswith("git+https://")
                for name, _version, source in identities
            )
        )
        self.assertIn(
            (
                "macro",
                "1.2.3",
                "registry+https://github.com/rust-lang/crates.io-index",
            ),
            identities,
        )
        for package in ("workspace-macro", "workspace-macro-first"):
            self.assertIn(
                (
                    package,
                    "1.2.3",
                    "path+workspace://crates/merman",
                ),
                identities,
            )

    def test_parser_rejects_unmarked_or_empty_output(self) -> None:
        for output in ("", "root v1.2.3"):
            with self.subTest(output=output), self.assertRaises(
                ClosureVerificationError
            ):
                parse_cargo_tree(output)

    def test_parser_rejects_ambiguous_proc_macro_annotations(self) -> None:
        output = (
            f"{PACKAGE_MARKER}root v1.2.3 (proc-macro) (proc-macro)"
            f"\t{FEATURE_MARKER}"
        )

        with self.assertRaisesRegex(
            ClosureVerificationError,
            "invalid Cargo proc-macro annotations",
        ):
            parse_cargo_tree(output)

class ClaimTests(unittest.TestCase):
    def test_semantic_failures_are_reported_together(self) -> None:
        loaded_claim = claim(
            "semantic",
            required=("root", "missing"),
            forbidden=("forbidden",),
            forbidden_features=(
                PackageFeatureExclusion("root", ("bad-feature",)),
            ),
        )
        closure = parse_cargo_tree(
            "\n".join(
                (
                    tree_line("root", ("bad-feature",)),
                    tree_line("forbidden"),
                )
            )
        )
        failures, _ = check_case(case(loaded_claim=loaded_claim), closure)
        self.assertTrue(any("required packages missing: missing" in x for x in failures))
        self.assertTrue(any("forbidden packages present" in x for x in failures))
        self.assertTrue(any("enables forbidden features" in x for x in failures))

    def test_observation_keeps_readable_package_evidence(self) -> None:
        closure = parse_cargo_tree(
            "\n".join(
                (
                    tree_line("fixture", version="1.0.0"),
                    tree_line("dependency", version="2.0.0"),
                )
            )
        )

        failures, observation = check_case(case(), closure)

        self.assertEqual(failures, [])
        self.assertEqual(observation.package_count, 2)
        self.assertEqual(
            observation.package_versions,
            frozenset({("fixture", "1.0.0"), ("dependency", "2.0.0")}),
        )

    def test_svg_basic_semantics_reject_optional_product_leaks(self) -> None:
        loaded_claim = next(
            current
            for current in SEMANTIC_CLAIMS
            if current.profile_id == "rust-svg-basic"
        )
        closure = parse_cargo_tree(
            "\n".join(
                (
                    tree_line("merman", ("layout-elk", "svg")),
                    tree_line("merman-core", ("system-timezone",)),
                    tree_line("merman-render", ("math",)),
                    tree_line("merman-analysis"),
                    tree_line("merman-layout-elk"),
                )
            )
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
    def test_rustsec_authority_uses_recipe_scope(self) -> None:
        observations = (
            ClosureObservation(
                profile_id="host",
                build_target_kind="host",
                closure_scope=LINUX_REFERENCE_SCOPE,
                closure_target=HOST_CLOSURE_REFERENCE_TARGET,
                package_count=1,
                package_versions=frozenset({("host", "1.0.0")}),
            ),
            ClosureObservation(
                profile_id="cross",
                build_target_kind="target-set",
                closure_scope=PROFILE_TARGET_SCOPE,
                closure_target="x86_64-unknown-linux-gnu",
                package_count=1,
                package_versions=frozenset({("cross", "1.0.0")}),
            ),
        )

        self.assertEqual(
            authoritative_rustsec_profile_ids(
                observations,
                running_host_target="aarch64-apple-darwin",
            ),
            frozenset({"cross"}),
        )
        self.assertEqual(
            authoritative_rustsec_profile_ids(
                observations,
                running_host_target=HOST_CLOSURE_REFERENCE_TARGET,
            ),
            frozenset({"cross", "host"}),
        )

    def test_every_target_runs_once_and_produces_runtime_evidence(self) -> None:
        targets = ("target-one", "target-two")
        loaded = recipe(
            "cross",
            package="root",
            build_target_kind="target-set",
            build_targets=targets,
        )
        output = tree_line("root")
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

        observations = verify_cases(cases, runner=runner)

        self.assertEqual(
            tuple(command[command.index("--target") + 1] for command in commands),
            targets,
        )
        self.assertEqual(
            tuple(observation.closure_target for observation in observations),
            targets,
        )
        self.assertEqual(
            {observation.closure_scope for observation in observations},
            {PROFILE_TARGET_SCOPE},
        )

    def test_identical_cargo_tree_commands_are_reused_across_claims(self) -> None:
        loaded = recipe("shared", package="root")
        output = tree_line("root")
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

        observations = verify_cases(cases, runner=runner)

        self.assertEqual(len(commands), 1)
        self.assertEqual(len(observations), 2)

    def test_semantic_claims_are_enforced_for_host_cases(self) -> None:
        output = tree_line("fixture")
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
                    stdout="\n".join((output, tree_line("forbidden"))),
                    stderr="",
                ),
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

        def runner(command: Sequence[str]) -> subprocess.CompletedProcess[str]:
            package = command[command.index("--package") + 1]
            return subprocess.CompletedProcess(
                command,
                0,
                stdout=tree_line(f"bad-{package}"),
                stderr="",
            )

        with self.assertRaises(ClosureVerificationError) as raised:
            verify_cases(cases, runner=runner)

        message = str(raised.exception)
        self.assertIn("one-claim (one", message)
        self.assertIn("two-claim (two", message)
        self.assertIn("required packages missing", message)
        self.assertIn("forbidden packages present", message)

    def test_runner_failure_is_fail_closed(self) -> None:
        def runner(command: Sequence[str]) -> subprocess.CompletedProcess[str]:
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
            verify_cases((case(),), runner=runner)

    def test_unknown_profile_selection_is_rejected(self) -> None:
        with self.assertRaisesRegex(
            ClosureVerificationError,
            "no dependency-closure recipe",
        ):
            _select_cases((case(),), ("not-a-profile",))


if __name__ == "__main__":
    unittest.main()
