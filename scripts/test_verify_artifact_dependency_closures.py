#!/usr/bin/env python3
"""Tests for exact artifact dependency-closure verification."""

from __future__ import annotations

import json
import subprocess
import sys
import unittest
from dataclasses import replace
from pathlib import Path


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from artifact_profile_recipe import (  # noqa: E402
    DEFAULT_DESCRIPTOR,
    REPO_ROOT,
    CargoArtifactRecipe,
    load_artifact_profile,
)
from github_workflow_contract import (  # noqa: E402
    load_workflow_contract,
    workflow_job,
    workflow_step,
)
from verify_artifact_dependency_closures import (  # noqa: E402
    CLAIMS,
    EXACT_FINGERPRINT_CLAIMS,
    PORTABLE_HOST_REFERENCE_TARGET,
    SEMANTIC_CLAIMS,
    ClosureClaim,
    ClosureVerificationError,
    PackageFeatureExclusion,
    _select_claims,
    cargo_tree_command,
    check_claim,
    closure_fingerprint,
    parse_cargo_tree,
    verify_claims,
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


def tree_line(package: str, features: tuple[str, ...] = ()) -> str:
    return (
        f"__MERMAN_CLOSURE_PACKAGE__{package} v1.2.3 (/workspace/{package})"
        f"\t__MERMAN_CLOSURE_FEATURES__{','.join(features)}"
    )


class DescriptorTests(unittest.TestCase):
    def test_maintenance_profiles_are_exact_default_empty_recipes(self) -> None:
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
                self.assertEqual(loaded.package, package)
                self.assertEqual(loaded.features, features)

    def test_ci_and_release_preflights_execute_the_closure_gate(self) -> None:
        repository_root = SCRIPT_DIR.parent
        workflows = (
            (".github/workflows/ci.yml", "build-test", "Verify generated architecture contracts"),
            (
                ".github/workflows/release-crates.yml",
                "preflight",
                "Verify exact artifact dependency closures",
            ),
            (
                ".github/workflows/release-preflight.yml",
                "versions-and-packages",
                "Verify exact artifact dependency closures",
            ),
        )

        for path, job, step_name in workflows:
            with self.subTest(workflow=path):
                workflow = load_workflow_contract(repository_root / path)
                step = workflow_step(workflow_job(workflow, job), name=step_name)
                self.assertIn(
                    "python3 scripts/verify_artifact_dependency_closures.py",
                    step["run"],
                )

    def test_ci_runs_owner_local_tests_from_exact_artifact_recipes(self) -> None:
        repository_root = SCRIPT_DIR.parent
        workflow = load_workflow_contract(repository_root / ".github/workflows/ci.yml")
        step = workflow_step(
            workflow_job(workflow, "build-test"),
            name="Test exact artifact owner APIs",
        )
        run = step["run"]

        self.assertIn(
            'artifact_profile_recipe.py "$profile" --field package',
            run,
        )
        self.assertIn(
            'artifact_profile_recipe.py "$profile"',
            run,
        )
        for profile_id in (
            "cli-analysis",
            "rust-analysis",
            "rust-ascii",
            "rust-bindings-core-native-sdk",
            "rust-editor-core",
            "rust-editor-facade",
            "rust-export-jpeg",
            "rust-export-pdf",
            "rust-export-png",
            "rust-svg-basic",
        ):
            with self.subTest(profile_id=profile_id):
                self.assertIn(f"run_owner_test {profile_id} ", run)

    def test_every_claim_has_a_unique_exact_profile(self) -> None:
        profile_ids = [claim.profile_id for claim in CLAIMS]
        self.assertEqual(len(profile_ids), len(set(profile_ids)))
        for profile_id in profile_ids:
            with self.subTest(profile_id=profile_id):
                self.assertFalse(
                    load_artifact_profile(profile_id, DEFAULT_DESCRIPTOR).default_features
                )

    def test_exact_claims_match_descriptor_root_and_target_contract(self) -> None:
        for claim in EXACT_FINGERPRINT_CLAIMS:
            with self.subTest(profile_id=claim.profile_id):
                loaded = load_artifact_profile(claim.profile_id, DEFAULT_DESCRIPTOR)
                self.assertEqual(claim.required_packages, (loaded.package,))
                if loaded.build_target_kind == "host":
                    self.assertIsNone(claim.target)
                    self.assertEqual(
                        claim.reference_target,
                        PORTABLE_HOST_REFERENCE_TARGET,
                    )
                    command = cargo_tree_command(
                        loaded,
                        claim.target,
                        reference_target=claim.reference_target,
                    )
                    self.assertEqual(
                        command[command.index("--target") + 1],
                        PORTABLE_HOST_REFERENCE_TARGET,
                    )
                else:
                    self.assertIsNone(claim.target)
                    self.assertIsNone(claim.reference_target)
                    for target, _ in claim.approved_target_fingerprints:
                        command = cargo_tree_command(loaded, target)
                        self.assertEqual(
                            command[command.index("--target") + 1],
                            target,
                        )
                        self.assertEqual(
                            command[command.index("--package") + 1],
                            loaded.package,
                        )
                        self.assertNotIn("all", command)

    def test_repository_exact_claims_have_approved_fingerprints(self) -> None:
        for claim in EXACT_FINGERPRINT_CLAIMS:
            with self.subTest(profile_id=claim.profile_id):
                loaded = load_artifact_profile(claim.profile_id, DEFAULT_DESCRIPTOR)
                if loaded.build_target_kind == "host":
                    self.assertRegex(
                        claim.approved_fingerprint,
                        r"^sha256:[0-9a-f]{64}$",
                    )
                    self.assertEqual(claim.approved_target_fingerprints, ())
                else:
                    self.assertEqual(claim.approved_fingerprint, "")
                    for _, fingerprint in claim.approved_target_fingerprints:
                        self.assertRegex(fingerprint, r"^sha256:[0-9a-f]{64}$")

    def test_repository_claims_cover_every_declared_target(self) -> None:
        for claim in CLAIMS:
            with self.subTest(profile_id=claim.profile_id):
                loaded = load_artifact_profile(claim.profile_id, DEFAULT_DESCRIPTOR)
                if loaded.build_target_kind == "host":
                    self.assertIsNone(claim.target)
                    self.assertEqual(
                        claim.reference_target,
                        PORTABLE_HOST_REFERENCE_TARGET,
                    )
                    self.assertEqual(claim.approved_target_fingerprints, ())
                    continue

                self.assertIsNone(claim.target)
                self.assertIsNone(claim.reference_target)
                approved_targets = [
                    target for target, _ in claim.approved_target_fingerprints
                ]
                self.assertEqual(tuple(approved_targets), loaded.build_targets)
                for _, fingerprint in claim.approved_target_fingerprints:
                    self.assertRegex(fingerprint, r"^sha256:[0-9a-f]{64}$")


class CargoTreeCommandTests(unittest.TestCase):
    def test_host_command_uses_an_explicit_portable_linux_reference_target(self) -> None:
        loaded = recipe("host", package="host-package")

        command = cargo_tree_command(
            loaded,
            None,
            reference_target=PORTABLE_HOST_REFERENCE_TARGET,
        )

        self.assertEqual(
            command[command.index("--target") + 1],
            "x86_64-unknown-linux-gnu",
        )

    def test_host_command_rejects_an_implicit_or_unapproved_reference_target(self) -> None:
        loaded = recipe("host")

        for reference_target in (None, "aarch64-apple-darwin", "all"):
            with self.subTest(reference_target=reference_target), self.assertRaisesRegex(
                ClosureVerificationError,
                "portable reference target",
            ):
                cargo_tree_command(
                    loaded,
                    None,
                    reference_target=reference_target,
                )

    def test_target_set_command_rejects_a_reference_target(self) -> None:
        loaded = recipe(
            "cross",
            build_target_kind="target-set",
            build_targets=("aarch64-linux-android",),
        )

        with self.assertRaisesRegex(ClosureVerificationError, "reference target"):
            cargo_tree_command(
                loaded,
                "aarch64-linux-android",
                reference_target=PORTABLE_HOST_REFERENCE_TARGET,
            )

    def test_command_uses_the_exact_recipe_and_descriptor_target(self) -> None:
        loaded = recipe(
            "cli-analysis",
            package="merman-cli",
            features=("analysis",),
            build_target_kind="target-set",
            build_targets=("x86_64-unknown-linux-gnu",),
        )

        command = cargo_tree_command(loaded, "x86_64-unknown-linux-gnu")

        self.assertEqual(command[0:2], ["cargo", "tree"])
        self.assertIn("--locked", command)
        self.assertIn("--no-default-features", command)
        self.assertEqual(command[command.index("--package") + 1], "merman-cli")
        self.assertEqual(command[command.index("--features") + 1], "analysis")
        self.assertEqual(
            command[command.index("--target") + 1],
            "x86_64-unknown-linux-gnu",
        )
        self.assertEqual(command[command.index("--edges") + 1], "normal")

    def test_command_rejects_default_features(self) -> None:
        loaded = recipe("bad", default_features=True)

        with self.assertRaisesRegex(
            ClosureVerificationError, "default_features=false"
        ):
            cargo_tree_command(loaded, None)

    def test_command_rejects_target_outside_the_exact_recipe(self) -> None:
        loaded = recipe(
            "cross",
            build_target_kind="target-set",
            build_targets=("aarch64-apple-darwin",),
        )

        with self.assertRaisesRegex(
            ClosureVerificationError, "does not declare target"
        ):
            cargo_tree_command(loaded, "x86_64-unknown-linux-gnu")


class CargoTreeParserTests(unittest.TestCase):
    def test_parser_unions_features_for_duplicate_package_versions(self) -> None:
        output = "\n".join(
            (
                tree_line("root", ("svg",)),
                tree_line("shared", ("std",)),
                "__MERMAN_CLOSURE_PACKAGE__shared v2.0.0 "
                "(/workspace/shared-v2)\t__MERMAN_CLOSURE_FEATURES__serde",
            )
        )

        closure = parse_cargo_tree(output)

        self.assertEqual(closure.packages, frozenset(("root", "shared")))
        self.assertEqual(
            closure.features_by_package["shared"], frozenset(("serde", "std"))
        )
        self.assertEqual(
            {
                (package, version)
                for package, version, _ in closure.features_by_package_identity
            },
            {("root", "1.2.3"), ("shared", "1.2.3"), ("shared", "2.0.0")},
        )

    def test_parser_normalizes_workspace_package_paths(self) -> None:
        package_path = REPO_ROOT / "crates" / "merman-core"
        closure = parse_cargo_tree(
            "__MERMAN_CLOSURE_PACKAGE__merman-core v1.2.3 "
            f"({package_path})\t__MERMAN_CLOSURE_FEATURES__"
        )

        self.assertEqual(
            set(closure.features_by_package_identity),
            {("merman-core", "1.2.3", "path+workspace://crates/merman-core")},
        )

    def test_parser_discards_cargo_deduplication_annotations(self) -> None:
        closure = parse_cargo_tree(
            "__MERMAN_CLOSURE_PACKAGE__root v1.2.3"
            "\t__MERMAN_CLOSURE_FEATURES__std (*)"
        )

        self.assertEqual(closure.features_by_package["root"], frozenset(("std",)))

    def test_parser_rejects_unmarked_or_empty_output(self) -> None:
        with self.assertRaisesRegex(ClosureVerificationError, "package marker"):
            parse_cargo_tree("fixture v1.0.0")
        with self.assertRaisesRegex(ClosureVerificationError, "no dependency packages"):
            parse_cargo_tree("")


class ClaimTests(unittest.TestCase):
    def test_claim_reports_missing_forbidden_and_feature_drift_together(self) -> None:
        claim = ClosureClaim(
            claim_id="fixture",
            profile_id="fixture",
            target=None,
            required_packages=("root", "required"),
            forbidden_packages=("forbidden",),
            forbidden_features=(
                PackageFeatureExclusion("root", ("system-clock",)),
            ),
            observed_residual_packages=("residual",),
        )
        closure = parse_cargo_tree(
            "\n".join(
                (
                    tree_line("root", ("system-clock",)),
                    tree_line("forbidden"),
                )
            )
        )

        failures, observation = check_claim(
            claim,
            closure,
            enforce_fingerprint=False,
        )

        self.assertTrue(any("required, residual" in failure for failure in failures))
        self.assertTrue(any("forbidden packages present" in failure for failure in failures))
        self.assertTrue(any("system-clock" in failure for failure in failures))
        self.assertEqual(observation.observed_residual_packages, ())

    def test_observed_residual_must_remain_explicitly_present(self) -> None:
        claim = ClosureClaim(
            claim_id="pdf",
            profile_id="rust-export-pdf",
            target=None,
            required_packages=("merman-export",),
            forbidden_packages=(),
            observed_residual_packages=("krilla-svg",),
        )

        failures, _ = check_claim(
            claim,
            parse_cargo_tree(tree_line("merman-export", ("pdf",))),
            enforce_fingerprint=False,
        )

        self.assertEqual(failures, ["required packages missing: krilla-svg"])

    def test_exact_fingerprint_rejects_an_unknown_dependency(self) -> None:
        approved = parse_cargo_tree(tree_line("root", ("svg",)))
        claim = ClosureClaim(
            claim_id="exact",
            profile_id="exact",
            target=None,
            required_packages=("root",),
            forbidden_packages=(),
            approved_fingerprint=closure_fingerprint(approved),
        )
        observed = parse_cargo_tree(
            "\n".join(
                (
                    tree_line("root", ("svg",)),
                    tree_line("new-heavy-backend"),
                )
            )
        )

        failures, observation = check_claim(claim, observed)

        self.assertEqual(observation.package_count, 2)
        self.assertTrue(
            any("dependency closure fingerprint drift" in failure for failure in failures)
        )

    def test_exact_fingerprint_rejects_a_missing_approval(self) -> None:
        claim = ClosureClaim(
            claim_id="exact",
            profile_id="exact",
            target=None,
            required_packages=("root",),
            forbidden_packages=(),
        )

        failures, _ = check_claim(claim, parse_cargo_tree(tree_line("root")))

        self.assertIn(
            "claim has no valid approved dependency closure fingerprint",
            failures,
        )

    def test_exact_fingerprint_includes_versions_and_features(self) -> None:
        baseline = parse_cargo_tree(tree_line("root", ("svg",)))
        baseline_fingerprint = closure_fingerprint(baseline)

        version_changed = parse_cargo_tree(
            "__MERMAN_CLOSURE_PACKAGE__root v1.2.4 "
            "(/workspace/root)\t__MERMAN_CLOSURE_FEATURES__svg"
        )
        feature_changed = parse_cargo_tree(tree_line("root", ("math", "svg")))

        self.assertNotEqual(
            baseline_fingerprint,
            closure_fingerprint(version_changed),
        )
        self.assertNotEqual(
            baseline_fingerprint,
            closure_fingerprint(feature_changed),
        )

    def test_exact_fingerprint_includes_cargo_source_identity(self) -> None:
        registry = parse_cargo_tree(
            "__MERMAN_CLOSURE_PACKAGE__root v1.2.3"
            "\t__MERMAN_CLOSURE_FEATURES__svg"
        )
        git = parse_cargo_tree(
            "__MERMAN_CLOSURE_PACKAGE__root v1.2.3 "
            "(https://github.com/example/root?rev=main#01234567)"
            "\t__MERMAN_CLOSURE_FEATURES__svg"
        )
        workspace_path = parse_cargo_tree(tree_line("root", ("svg",)))

        fingerprints = {
            closure_fingerprint(registry),
            closure_fingerprint(git),
            closure_fingerprint(workspace_path),
        }

        self.assertEqual(len(fingerprints), 3)

    def test_svg_basic_claim_rejects_optional_engine_and_product_leaks(self) -> None:
        claim = next(
            claim
            for claim in CLAIMS
            if claim.profile_id == "rust-svg-basic"
        )
        closure = parse_cargo_tree(
            "\n".join(
                (
                    tree_line("merman", ("layout-elk", "svg")),
                    tree_line("merman-core", ("system-timezone",)),
                    tree_line("merman-render", ("math",)),
                    tree_line("getrandom"),
                    tree_line("image"),
                    tree_line("jiff"),
                    tree_line("krilla"),
                    tree_line("manatee"),
                    tree_line("merman-analysis"),
                    tree_line("merman-ascii"),
                    tree_line("merman-editor-core"),
                    tree_line("merman-layout-elk"),
                    tree_line("ratex-svg"),
                    tree_line("resvg"),
                    tree_line("web-time"),
                )
            )
        )

        failures, _ = check_claim(
            claim,
            closure,
            enforce_fingerprint=False,
        )

        self.assertTrue(
            any("forbidden packages present" in failure for failure in failures)
        )
        self.assertTrue(
            any("layout-elk" in failure for failure in failures)
        )
        self.assertTrue(
            any("system-timezone" in failure for failure in failures)
        )
        self.assertTrue(any("math" in failure for failure in failures))


class VerificationTests(unittest.TestCase):
    def test_every_artifact_profile_has_one_exact_closure_claim(self) -> None:
        descriptor = json.loads(DEFAULT_DESCRIPTOR.read_text(encoding="utf-8"))
        profile_ids = {profile["id"] for profile in descriptor["profiles"]}
        claimed_ids = [claim.profile_id for claim in CLAIMS]

        self.assertEqual(set(claimed_ids), profile_ids)
        self.assertEqual(len(claimed_ids), len(set(claimed_ids)))

    def test_fake_runner_proves_semantic_repository_claims_without_cargo(self) -> None:
        recipes = {
            "rust-static-svg": recipe(
                "rust-static-svg",
                package="merman",
                features=("layout-cytoscape", "layout-elk", "math", "svg"),
            ),
            "rust-svg-basic": recipe(
                "rust-svg-basic",
                package="merman",
                features=("svg",),
            ),
            "cli-analysis": recipe(
                "cli-analysis",
                package="merman-cli",
                features=("analysis",),
                build_target_kind="target-set",
                build_targets=("x86_64-unknown-linux-gnu",),
            ),
            "rust-export-png": recipe(
                "rust-export-png",
                package="merman-export",
                features=("png",),
            ),
            "rust-export-jpeg": recipe(
                "rust-export-jpeg",
                package="merman-export",
                features=("jpeg",),
            ),
            "rust-export-pdf": recipe(
                "rust-export-pdf",
                package="merman-export",
                features=("pdf",),
            ),
        }
        outputs = {
            ("merman", "layout-cytoscape,layout-elk,math,svg"): "\n".join(
                (
                    tree_line(
                        "merman", ("layout-cytoscape", "layout-elk", "math", "svg")
                    ),
                    tree_line("merman-core"),
                    tree_line(
                        "merman-render", ("layout-cytoscape", "layout-elk", "math")
                    ),
                )
            ),
            ("merman", "svg"): "\n".join(
                (
                    tree_line("merman", ("svg",)),
                    tree_line("merman-core"),
                    tree_line("merman-render"),
                )
            ),
            ("merman-cli", "analysis"): "\n".join(
                (
                    tree_line("merman-cli", ("analysis",)),
                    tree_line("merman-analysis"),
                    tree_line("merman-core"),
                )
            ),
            ("merman-export", "png"): "\n".join(
                (
                    tree_line("merman-export", ("png",)),
                    tree_line("merman-render"),
                    tree_line("resvg"),
                    tree_line("tiny-skia"),
                    tree_line("usvg"),
                )
            ),
            ("merman-export", "jpeg"): "\n".join(
                (
                    tree_line("merman-export", ("jpeg",)),
                    tree_line("image"),
                    tree_line("merman-render"),
                    tree_line("png"),
                    tree_line("resvg"),
                    tree_line("tiny-skia"),
                    tree_line("usvg"),
                )
            ),
            ("merman-export", "pdf"): "\n".join(
                (
                    tree_line("merman-export", ("pdf",)),
                    tree_line("merman-render"),
                    tree_line("krilla"),
                    tree_line("fontdb"),
                    tree_line("gif"),
                    tree_line("image-webp"),
                    tree_line("krilla-svg"),
                    tree_line("memmap2"),
                    tree_line("png"),
                    tree_line("resvg"),
                    tree_line("rustybuzz"),
                    tree_line("tiny-skia"),
                    tree_line("ttf-parser"),
                    tree_line("usvg"),
                    tree_line("zune-jpeg"),
                )
            ),
        }
        commands: list[Sequence[str]] = []

        def loader(profile_id: str, _: Path) -> CargoArtifactRecipe:
            return recipes[profile_id]

        def runner(command: Sequence[str]) -> subprocess.CompletedProcess[str]:
            commands.append(command)
            package = command[command.index("--package") + 1]
            features = command[command.index("--features") + 1]
            return subprocess.CompletedProcess(
                command,
                0,
                stdout=outputs[(package, features)],
                stderr="",
            )

        fixture_claims = []
        for claim in SEMANTIC_CLAIMS:
            loaded = recipes[claim.profile_id]
            fingerprint = closure_fingerprint(
                parse_cargo_tree(
                    outputs[(loaded.package, loaded.feature_argument)]
                )
            )
            if loaded.build_target_kind == "target-set":
                fixture_claims.append(
                    replace(
                        claim,
                        approved_fingerprint="",
                        approved_target_fingerprints=(
                            (loaded.build_targets[0], fingerprint),
                        ),
                    )
                )
            else:
                fixture_claims.append(
                    replace(claim, approved_fingerprint=fingerprint)
                )
        observations = verify_claims(
            fixture_claims,
            runner=runner,
            recipe_loader=loader,
        )

        self.assertEqual(len(commands), len(SEMANTIC_CLAIMS))
        self.assertEqual(len(observations), len(SEMANTIC_CLAIMS))
        for command in commands:
            package = command[command.index("--package") + 1]
            expected_target = (
                "x86_64-unknown-linux-gnu"
                if package == "merman-cli"
                else PORTABLE_HOST_REFERENCE_TARGET
            )
            self.assertEqual(
                command[command.index("--target") + 1],
                expected_target,
            )
        pdf = next(
            observation
            for observation in observations
            if observation.profile_id == "rust-export-pdf"
        )
        self.assertEqual(
            pdf.observed_residual_packages,
            (
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
        )

    def test_target_set_runs_every_declared_target(self) -> None:
        targets = ("target-one", "target-two")
        loaded = recipe(
            "multi-target",
            package="root",
            build_target_kind="target-set",
            build_targets=targets,
        )
        output = tree_line("root")
        fingerprint = closure_fingerprint(parse_cargo_tree(output))
        claim = ClosureClaim(
            claim_id="multi-target",
            profile_id="multi-target",
            target=None,
            required_packages=("root",),
            forbidden_packages=(),
            approved_target_fingerprints=tuple(
                (target, fingerprint) for target in targets
            ),
        )
        commands: list[Sequence[str]] = []

        def loader(_: str, __: Path) -> CargoArtifactRecipe:
            return loaded

        def runner(command: Sequence[str]) -> subprocess.CompletedProcess[str]:
            commands.append(command)
            return subprocess.CompletedProcess(command, 0, stdout=output, stderr="")

        observations = verify_claims(
            (claim,),
            runner=runner,
            recipe_loader=loader,
        )

        self.assertEqual(
            tuple(command[command.index("--target") + 1] for command in commands),
            targets,
        )
        self.assertEqual(
            tuple(observation.closure_target for observation in observations),
            targets,
        )

    def test_target_set_rejects_partial_or_duplicate_approvals(self) -> None:
        loaded = recipe(
            "multi-target",
            package="root",
            build_target_kind="target-set",
            build_targets=("target-one", "target-two"),
        )

        def loader(_: str, __: Path) -> CargoArtifactRecipe:
            return loaded

        cases = (
            (
                (("target-one", "sha256:" + "0" * 64),),
                "must match descriptor targets exactly",
            ),
            (
                (
                    ("target-one", "sha256:" + "0" * 64),
                    ("target-one", "sha256:" + "0" * 64),
                ),
                "duplicate fingerprint targets",
            ),
        )
        for target_fingerprints, message in cases:
            with self.subTest(target_fingerprints=target_fingerprints):
                claim = ClosureClaim(
                    claim_id="multi-target",
                    profile_id="multi-target",
                    target=None,
                    required_packages=("root",),
                    forbidden_packages=(),
                    approved_target_fingerprints=target_fingerprints,
                )
                with self.assertRaisesRegex(ClosureVerificationError, message):
                    verify_claims(
                        (claim,),
                        runner=lambda _: self.fail("runner must not execute"),
                        recipe_loader=loader,
                        enforce_fingerprints=False,
                    )

    def test_failures_are_aggregated_across_profiles(self) -> None:
        claims = (
            ClosureClaim(
                "one",
                "one",
                None,
                ("root-one",),
                ("bad-one",),
                reference_target=PORTABLE_HOST_REFERENCE_TARGET,
            ),
            ClosureClaim(
                "two",
                "two",
                None,
                ("root-two",),
                ("bad-two",),
                reference_target=PORTABLE_HOST_REFERENCE_TARGET,
            ),
        )

        def loader(profile_id: str, _: Path) -> CargoArtifactRecipe:
            return recipe(profile_id, package=profile_id)

        def runner(command: Sequence[str]) -> subprocess.CompletedProcess[str]:
            package = command[command.index("--package") + 1]
            return subprocess.CompletedProcess(
                command,
                0,
                stdout=tree_line(f"bad-{package}"),
                stderr="",
            )

        with self.assertRaises(ClosureVerificationError) as raised:
            verify_claims(
                claims,
                runner=runner,
                recipe_loader=loader,
                enforce_fingerprints=False,
            )

        message = str(raised.exception)
        self.assertIn("one (one, target=x86_64-unknown-linux-gnu)", message)
        self.assertIn("two (two, target=x86_64-unknown-linux-gnu)", message)
        self.assertIn("required packages missing", message)
        self.assertIn("forbidden packages present", message)

    def test_runner_failure_is_fail_closed(self) -> None:
        claim = ClosureClaim(
            "failed",
            "failed",
            None,
            ("failed",),
            (),
            reference_target=PORTABLE_HOST_REFERENCE_TARGET,
        )

        def loader(profile_id: str, _: Path) -> CargoArtifactRecipe:
            return recipe(profile_id, package=profile_id)

        def runner(command: Sequence[str]) -> subprocess.CompletedProcess[str]:
            return subprocess.CompletedProcess(
                command, 101, stdout="", stderr="dependency resolution failed"
            )

        with self.assertRaisesRegex(
            ClosureVerificationError, "dependency resolution failed"
        ):
            verify_claims(
                (claim,),
                runner=runner,
                recipe_loader=loader,
                enforce_fingerprints=False,
            )

    def test_unknown_profile_selection_is_rejected(self) -> None:
        with self.assertRaisesRegex(ClosureVerificationError, "no dependency-closure claim"):
            _select_claims(("not-a-profile",))


if __name__ == "__main__":
    unittest.main()
