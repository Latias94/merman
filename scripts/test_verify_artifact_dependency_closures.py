#!/usr/bin/env python3
"""Tests for exact artifact dependency-closure verification."""

from __future__ import annotations

import subprocess
import sys
import unittest
from pathlib import Path


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from artifact_profile_recipe import (  # noqa: E402
    DEFAULT_DESCRIPTOR,
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
    ClosureClaim,
    ClosureVerificationError,
    PackageFeatureExclusion,
    _select_claims,
    cargo_tree_command,
    check_claim,
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
            "rust-export-pdf": ("merman-export", ("pdf",)),
            "rust-export-png": ("merman-export", ("png",)),
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

    def test_every_claim_has_a_unique_exact_profile(self) -> None:
        profile_ids = [claim.profile_id for claim in CLAIMS]
        self.assertEqual(len(profile_ids), len(set(profile_ids)))
        for profile_id in profile_ids:
            with self.subTest(profile_id=profile_id):
                self.assertFalse(
                    load_artifact_profile(profile_id, DEFAULT_DESCRIPTOR).default_features
                )


class CargoTreeCommandTests(unittest.TestCase):
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

        failures, observation = check_claim(claim, closure)

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
            claim, parse_cargo_tree(tree_line("merman-export", ("pdf",)))
        )

        self.assertEqual(failures, ["required packages missing: krilla-svg"])


class VerificationTests(unittest.TestCase):
    def test_fake_runner_proves_all_repository_claims_without_cargo(self) -> None:
        recipes = {
            "rust-static-svg": recipe(
                "rust-static-svg",
                package="merman",
                features=("layout-cytoscape", "layout-elk", "math", "svg"),
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

        observations = verify_claims(CLAIMS, runner=runner, recipe_loader=loader)

        self.assertEqual(len(commands), len(CLAIMS))
        self.assertEqual(len(observations), len(CLAIMS))
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

    def test_failures_are_aggregated_across_profiles(self) -> None:
        claims = (
            ClosureClaim("one", "one", None, ("root-one",), ("bad-one",)),
            ClosureClaim("two", "two", None, ("root-two",), ("bad-two",)),
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
            verify_claims(claims, runner=runner, recipe_loader=loader)

        message = str(raised.exception)
        self.assertIn("one (one)", message)
        self.assertIn("two (two)", message)
        self.assertIn("required packages missing", message)
        self.assertIn("forbidden packages present", message)

    def test_runner_failure_is_fail_closed(self) -> None:
        claim = ClosureClaim("failed", "failed", None, ("failed",), ())

        def loader(profile_id: str, _: Path) -> CargoArtifactRecipe:
            return recipe(profile_id, package=profile_id)

        def runner(command: Sequence[str]) -> subprocess.CompletedProcess[str]:
            return subprocess.CompletedProcess(
                command, 101, stdout="", stderr="dependency resolution failed"
            )

        with self.assertRaisesRegex(
            ClosureVerificationError, "dependency resolution failed"
        ):
            verify_claims((claim,), runner=runner, recipe_loader=loader)

    def test_unknown_profile_selection_is_rejected(self) -> None:
        with self.assertRaisesRegex(ClosureVerificationError, "no dependency-closure claim"):
            _select_claims(("not-a-profile",))


if __name__ == "__main__":
    unittest.main()
