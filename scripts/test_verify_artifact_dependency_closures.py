#!/usr/bin/env python3
"""Tests for exact artifact runtime dependency-closure verification."""

from __future__ import annotations

import hashlib
import json
import os
import subprocess
import sys
import tempfile
import unittest
from unittest import mock
from pathlib import Path
from typing import Sequence


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from artifact_dependency_approvals import (  # noqa: E402
    ARTIFACT_DEPENDENCY_APPROVALS,
    HOST_CLOSURE_REFERENCE_TARGET,
)
from artifact_profile_recipe import (  # noqa: E402
    CargoArtifactRecipe,
    load_artifact_profile,
)
from github_workflow_contract import (  # noqa: E402
    load_workflow_contract,
    workflow_job,
    workflow_step,
)
from verify_artifact_dependency_closures import (  # noqa: E402
    AttributionClosure,
    AttributionPackage,
    FEATURE_MARKER,
    LINUX_REFERENCE_SCOPE,
    PACKAGE_MARKER,
    PROFILE_TARGET_SCOPE,
    SEMANTIC_CLAIMS,
    ClosureClaim,
    ClosureObservation,
    ClosureVerificationError,
    DependencyClosure,
    PackageFeatureExclusion,
    ProbeClosureObservation,
    VerificationCase,
    _closure_expansion_failures,
    _format_probe_matrix_failures,
    _select_cases,
    authoritative_rustsec_profile_ids,
    cargo_tree_command,
    check_case,
    closure_fingerprint,
    dependency_baseline_report,
    load_verification_cases,
    load_dependency_baseline,
    parse_attribution_cargo_tree,
    parse_cargo_tree,
    probe_attribution_command,
    probe_runtime_command,
    verify_cases,
    verify_dependency_baseline,
)
from ffi_contract_dependency_probes import (  # noqa: E402
    BASELINE_COMMIT,
    BASELINE_TREE,
    load_dependency_probes,
    probe_registry_sha256,
)
from ffi_contract_baseline_contract import (  # noqa: E402
    BASELINE_LOCK_SCHEMA_VERSION,
)
import verify_artifact_dependency_closures as dependency_verifier  # noqa: E402


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
    residual: tuple[str, ...] = (),
) -> ClosureClaim:
    return ClosureClaim(
        claim_id=f"{profile_id}-claim",
        profile_id=profile_id,
        required_packages=required,
        forbidden_packages=forbidden,
        forbidden_features=forbidden_features,
        observed_residual_packages=residual,
    )


def case(
    profile_id: str = "fixture",
    *,
    loaded_recipe: CargoArtifactRecipe | None = None,
    loaded_claim: ClosureClaim | None = None,
    target: str = HOST_CLOSURE_REFERENCE_TARGET,
    fingerprint: str = "sha256:" + "0" * 64,
) -> VerificationCase:
    return VerificationCase(
        recipe=loaded_recipe or recipe(profile_id),
        claim=loaded_claim or claim(profile_id),
        target=target,
        approved_fingerprint=fingerprint,
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


class ApprovalCatalogTests(unittest.TestCase):
    def test_repository_catalog_covers_every_profile_and_declared_target(self) -> None:
        cases = load_verification_cases()
        self.assertEqual(
            {case.recipe.profile_id for case in cases},
            set(ARTIFACT_DEPENDENCY_APPROVALS),
        )
        self.assertGreater(len(cases), len(ARTIFACT_DEPENDENCY_APPROVALS))

    def test_catalog_must_match_profile_directory_exactly(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            descriptor = write_descriptor(
                Path(temporary_directory),
                "known",
                "missing",
            )
            approvals = {
                "known": (
                    (HOST_CLOSURE_REFERENCE_TARGET, "sha256:" + "0" * 64),
                ),
                "unexpected": (
                    (HOST_CLOSURE_REFERENCE_TARGET, "sha256:" + "0" * 64),
                ),
            }
            with self.assertRaisesRegex(
                ClosureVerificationError,
                r"missing=\['missing'\] unexpected=\['unexpected'\]",
            ):
                load_verification_cases(
                    descriptor_path=descriptor,
                    approvals=approvals,
                    semantic_claims=(),
                )

    def test_targets_are_ordered_exact_and_fingerprints_are_valid(self) -> None:
        fingerprint = "sha256:" + "0" * 64
        invalid_catalogs = (
            (
                {"cross": (("target-one", fingerprint),)},
                "must match descriptor evidence targets exactly",
            ),
            (
                {
                    "cross": (
                        ("target-one", fingerprint),
                        ("target-one", fingerprint),
                    )
                },
                "duplicate approval targets",
            ),
            (
                {
                    "cross": (
                        ("target-one", fingerprint),
                        ("target-two", "not-a-fingerprint"),
                    )
                },
                "invalid runtime fingerprints",
            ),
        )
        with tempfile.TemporaryDirectory() as temporary_directory:
            descriptor = write_descriptor(
                Path(temporary_directory),
                "cross",
                build_target={
                    "kind": "target-set",
                    "triples": ["target-one", "target-two"],
                },
            )
            for approvals, message in invalid_catalogs:
                with self.subTest(message=message), self.assertRaisesRegex(
                    ClosureVerificationError,
                    message,
                ):
                    load_verification_cases(
                        descriptor_path=descriptor,
                        approvals=approvals,
                        semantic_claims=(),
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

    def test_ci_and_release_preflights_execute_the_closure_gate(self) -> None:
        workflows = (
            (".github/workflows/ci.yml", "build-test", "Verify generated architecture contracts"),
            (
                ".github/workflows/release-crates.yml",
                "preflight",
                "Verify target-scoped artifact dependency closures",
            ),
            (
                ".github/workflows/release-preflight.yml",
                "versions-and-packages",
                "Verify target-scoped artifact dependency closures",
            ),
        )
        for path, job, step_name in workflows:
            with self.subTest(workflow=path):
                workflow = load_workflow_contract(SCRIPT_DIR.parent / path)
                step = workflow_step(workflow_job(workflow, job), name=step_name)
                self.assertIn(
                    "python3 scripts/verify_artifact_dependency_closures.py",
                    step["run"],
                )


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
        self.assertIn('build.rustc="rustc"', command)
        self.assertIn("build.incremental=false", command)

    def test_command_accepts_resolved_toolchain_paths(self) -> None:
        command = cargo_tree_command(
            case(),
            cargo_path="/toolchain/bin/cargo",
            rustc_path="/toolchain/bin/rustc",
        )

        self.assertEqual(command[:2], ["/toolchain/bin/cargo", "tree"])
        self.assertIn('build.rustc="/toolchain/bin/rustc"', command)

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

    def test_fingerprint_includes_version_features_and_source(self) -> None:
        closures = (
            parse_cargo_tree(tree_line("root", ("svg",), version="1.0.0")),
            parse_cargo_tree(tree_line("root", ("math",), version="1.0.0")),
            parse_cargo_tree(tree_line("root", ("svg",), version="2.0.0")),
            parse_cargo_tree(
                tree_line(
                    "root",
                    ("svg",),
                    version="1.0.0",
                    source="https://example.com/root#01234567",
                )
            ),
        )
        self.assertEqual(len({closure_fingerprint(item) for item in closures}), 4)


class FixedFfiBaselineTests(unittest.TestCase):
    def test_fixed_baseline_rejects_local_build_environment_overrides(self) -> None:
        with (
            mock.patch.dict(os.environ, {"RUSTC": "/tmp/rustc"}, clear=True),
            self.assertRaisesRegex(
                ClosureVerificationError,
                "environment overrides: RUSTC",
            ),
        ):
            verify_dependency_baseline(
                Path("baseline-is-not-read.json"),
                repo_root=SCRIPT_DIR.parent,
                runner=lambda command: subprocess.CompletedProcess(command, 1, "", ""),
            )

    def test_readiness_reports_public_and_private_lanes_independently(self) -> None:
        probes = load_dependency_probes()
        report = _format_probe_matrix_failures(
            "fixture failure",
            probes,
            (("private-node", "node-wasm-full", "fixture"),),
        )
        self.assertIn(
            "ffi-contract-readiness lane=public-native status=ok",
            report,
        )
        self.assertIn(
            "ffi-contract-readiness lane=private-node status=failed",
            report,
        )

    def test_fixed_baseline_allows_dependency_removal_and_role_feature_narrowing(
        self,
    ) -> None:
        baseline = self._comparison_probe(
            runtime=(
                self._runtime_package("root", ("a", "b")),
                self._runtime_package("removed", ()),
            ),
            attribution=(
                self._attribution_package(
                    "root",
                    ("a", "b"),
                    ("build", "normal"),
                    {"build": ("b",), "normal": ("a",)},
                ),
                self._attribution_package(
                    "removed",
                    (),
                    ("normal",),
                    {"normal": ()},
                ),
            ),
        )
        current = self._comparison_probe(
            runtime=(self._runtime_package("root", ("a",)),),
            attribution=(
                self._attribution_package(
                    "root",
                    ("a",),
                    ("normal",),
                    {"normal": ("a",)},
                ),
            ),
        )

        self.assertEqual(
            _closure_expansion_failures(baseline, current, "fixture"),
            [],
        )

    def test_fixed_baseline_rejects_package_feature_and_role_expansion(self) -> None:
        baseline = self._comparison_probe(
            runtime=(self._runtime_package("root", ("a",)),),
            attribution=(
                self._attribution_package(
                    "root",
                    ("a",),
                    ("normal",),
                    {"normal": ("a",)},
                ),
            ),
        )
        current = self._comparison_probe(
            runtime=(
                self._runtime_package("root", ("a", "new-feature")),
                self._runtime_package("added", ()),
            ),
            attribution=(
                self._attribution_package(
                    "root",
                    ("a", "build-feature"),
                    ("build", "normal"),
                    {"build": ("build-feature",), "normal": ("a",)},
                ),
            ),
        )

        failures = _closure_expansion_failures(baseline, current, "fixture")
        self.assertTrue(any("gained package added" in failure for failure in failures))
        self.assertTrue(any("gained features: new-feature" in failure for failure in failures))
        self.assertTrue(any("gained roles: build" in failure for failure in failures))
        self.assertTrue(
            any("gained build features: build-feature" in failure for failure in failures)
        )

        replaced_version = self._comparison_probe(
            runtime=(
                {
                    **self._runtime_package("root", ("a",)),
                    "version": "2.0.0",
                },
            ),
            attribution=tuple(baseline["attribution"]["packages"]),
        )
        self.assertTrue(
            any(
                "gained package root v2.0.0" in failure
                for failure in _closure_expansion_failures(
                    baseline,
                    replaced_version,
                    "fixture",
                )
            )
        )

    def test_probe_commands_are_package_scoped_and_split_runtime_from_attribution(
        self,
    ) -> None:
        probe = next(
            item for item in load_dependency_probes() if item.probe_id == "ffi-svg-linux"
        )
        runtime = probe_runtime_command(probe)
        attribution = probe_attribution_command(probe)

        for command in (runtime, attribution):
            self.assertEqual(command[command.index("--package") + 1], "merman-ffi")
            self.assertTrue(
                command[command.index("--manifest-path") + 1].endswith(
                    "crates/merman-ffi/Cargo.toml"
                )
            )
            self.assertEqual(command[command.index("--features") + 1], "svg")
            self.assertEqual(
                command[command.index("--target") + 1],
                "x86_64-unknown-linux-gnu",
            )
        self.assertEqual(
            runtime[runtime.index("--edges") + 1],
            "normal,no-proc-macro",
        )
        self.assertIn("--prefix", runtime)
        self.assertEqual(
            attribution[attribution.index("--edges") + 1],
            "normal,build",
        )
        self.assertIn("--no-dedupe", attribution)
        self.assertNotIn("--prefix", attribution)

    def test_attribution_parser_propagates_normal_build_and_proc_macro_roles(
        self,
    ) -> None:
        output = "\n".join(
            (
                tree_line("root", ("root-feature",)),
                "|-- " + tree_line("shared", ("normal-feature",)),
                "|-- " + tree_line("macro", ("derive",), proc_macro=True),
                "|   `-- " + tree_line("macro-support", ("parse",)),
                "|   [build-dependencies]",
                "|   `-- " + tree_line("macro-build", ("cc",)),
                "[build-dependencies]",
                "`-- " + tree_line("build-root", ("build",)),
                "    `-- " + tree_line("shared", ("build-feature",)),
            )
        )

        closure = parse_attribution_cargo_tree(output)
        packages = {package.package: package for package in closure.packages}
        self.assertEqual(packages["root"].roles, ("normal",))
        self.assertEqual(packages["macro"].roles, ("proc-macro",))
        self.assertEqual(packages["macro-support"].roles, ("proc-macro",))
        self.assertEqual(packages["macro-build"].roles, ("build",))
        self.assertEqual(packages["build-root"].roles, ("build",))
        self.assertEqual(packages["shared"].roles, ("build", "normal"))
        self.assertEqual(
            packages["shared"].role_features,
            {
                "build": ("build-feature",),
                "normal": ("normal-feature",),
            },
        )

    def test_attribution_parser_merges_one_package_across_all_compiler_roles(
        self,
    ) -> None:
        output = "\n".join(
            (
                tree_line("root"),
                "|-- " + tree_line("shared", ("normal",)),
                "|-- " + tree_line("macro", proc_macro=True),
                "|   `-- " + tree_line("shared", ("proc",)),
                "[build-dependencies]",
                "|-- " + tree_line("build-macro", ("derive",), proc_macro=True),
                "|   `-- " + tree_line("shared", ("build",)),
                "`-- " + tree_line("shared", ("build",)),
            )
        )

        closure = parse_attribution_cargo_tree(output)
        packages = {package.package: package for package in closure.packages}
        self.assertEqual(
            packages["build-macro"].roles,
            ("build", "proc-macro"),
        )
        self.assertEqual(
            packages["shared"].roles,
            ("build", "normal", "proc-macro"),
        )
        self.assertEqual(
            packages["shared"].role_features,
            {
                "build": ("build",),
                "normal": ("normal",),
                "proc-macro": ("proc",),
            },
        )

    def test_fixed_report_is_self_digested_and_whole_file_locked(self) -> None:
        probes = load_dependency_probes()
        observations = tuple(self._observation(probe) for probe in probes)
        report = dependency_baseline_report(
            observations,
            repo_root=SCRIPT_DIR.parent,
            toolchain=self._toolchain(),
            source_snapshot_sha256=self._snapshot_sha256(),
        )
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            baseline = root / "dependency-closures.json"
            baseline.write_text(
                json.dumps(report, indent=2, sort_keys=True) + "\n",
                encoding="utf-8",
            )
            digest = "sha256:" + hashlib.sha256(baseline.read_bytes()).hexdigest()
            lock = root / "lock.json"
            lock.write_text(
                json.dumps(
                    {
                        "schema_version": BASELINE_LOCK_SCHEMA_VERSION,
                        "baseline_commit": BASELINE_COMMIT,
                        "baseline_input_sha256": {
                            record["path"]: record["sha256"]
                            for record in report["inputs"]
                        },
                        "source_snapshot_sha256": self._snapshot_sha256(),
                        "baseline_tree": BASELINE_TREE,
                        "dependency_report_schema_version": 3,
                        "dependency_report_file_sha256": digest,
                        "native_artifact_report_schema_version": 3,
                        "native_artifact_report_file_sha256": "sha256:" + "4" * 64,
                        "probe_registry_sha256": probe_registry_sha256(probes),
                    }
                ),
                encoding="utf-8",
            )

            loaded = load_dependency_baseline(
                baseline,
                lock_path=lock,
                repo_root=SCRIPT_DIR.parent,
            )
            self.assertEqual(loaded["baseline_commit"], BASELINE_COMMIT)

            report["probes"][0]["runtime_legal"]["package_count"] += 1
            baseline.write_text(json.dumps(report), encoding="utf-8")
            with self.assertRaisesRegex(ClosureVerificationError, "embedded digest"):
                load_dependency_baseline(
                    baseline,
                    lock_path=lock,
                    repo_root=SCRIPT_DIR.parent,
                )

            tampered = json.loads(json.dumps(report))
            tampered["probes"][0]["runtime_legal"]["package_count"] -= 1
            tampered["toolchain"]["cargo_version"] = "cargo changed"
            tampered["report_sha256"] = dependency_verifier._embedded_report_sha256(
                tampered
            )
            baseline.write_text(json.dumps(tampered), encoding="utf-8")
            with self.assertRaisesRegex(ClosureVerificationError, "whole-file digest"):
                load_dependency_baseline(
                    baseline,
                    lock_path=lock,
                    repo_root=SCRIPT_DIR.parent,
                )

    def test_pending_lock_digest_is_rejected(self) -> None:
        probes = load_dependency_probes()
        observations = tuple(self._observation(probe) for probe in probes)
        report = dependency_baseline_report(
            observations,
            repo_root=SCRIPT_DIR.parent,
            toolchain=self._toolchain(),
            source_snapshot_sha256=self._snapshot_sha256(),
        )
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            baseline = root / "dependency-closures.json"
            baseline.write_text(json.dumps(report), encoding="utf-8")
            lock = root / "lock.json"
            lock.write_text(
                json.dumps(
                    {
                        "schema_version": BASELINE_LOCK_SCHEMA_VERSION,
                        "baseline_commit": BASELINE_COMMIT,
                        "baseline_input_sha256": {
                            record["path"]: record["sha256"]
                            for record in report["inputs"]
                        },
                        "source_snapshot_sha256": self._snapshot_sha256(),
                        "baseline_tree": BASELINE_TREE,
                        "dependency_report_schema_version": 3,
                        "dependency_report_file_sha256": "pending",
                        "native_artifact_report_schema_version": 3,
                        "native_artifact_report_file_sha256": "pending",
                        "probe_registry_sha256": probe_registry_sha256(probes),
                    }
                ),
                encoding="utf-8",
            )
            with self.assertRaisesRegex(ClosureVerificationError, "not finalized"):
                load_dependency_baseline(
                    baseline,
                    lock_path=lock,
                    repo_root=SCRIPT_DIR.parent,
                )

    def test_other_report_zero_digest_is_not_an_atomic_lock(self) -> None:
        probes = load_dependency_probes()
        report = dependency_baseline_report(
            tuple(self._observation(probe) for probe in probes),
            repo_root=SCRIPT_DIR.parent,
            toolchain=self._toolchain(),
            source_snapshot_sha256=self._snapshot_sha256(),
        )
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            baseline = root / "dependency-closures.json"
            baseline.write_text(json.dumps(report), encoding="utf-8")
            lock = root / "lock.json"
            lock.write_text(
                json.dumps(
                    {
                        "schema_version": BASELINE_LOCK_SCHEMA_VERSION,
                        "baseline_commit": BASELINE_COMMIT,
                        "baseline_input_sha256": {
                            record["path"]: record["sha256"]
                            for record in report["inputs"]
                        },
                        "source_snapshot_sha256": self._snapshot_sha256(),
                        "baseline_tree": BASELINE_TREE,
                        "dependency_report_schema_version": 3,
                        "dependency_report_file_sha256": (
                            "sha256:" + hashlib.sha256(baseline.read_bytes()).hexdigest()
                        ),
                        "native_artifact_report_schema_version": 3,
                        "native_artifact_report_file_sha256": "sha256:" + "0" * 64,
                        "probe_registry_sha256": probe_registry_sha256(probes),
                    }
                ),
                encoding="utf-8",
            )
            with self.assertRaisesRegex(ClosureVerificationError, "not finalized"):
                load_dependency_baseline(
                    baseline,
                    lock_path=lock,
                    repo_root=SCRIPT_DIR.parent,
                )

    def test_known_workspace_feature_unified_report_is_rejected_before_decode(
        self,
    ) -> None:
        rejected = next(iter(dependency_verifier.REJECTED_BASELINE_FILE_SHA256))
        with (
            mock.patch.object(dependency_verifier, "bytes_sha256", return_value=rejected),
            mock.patch.object(
                type(dependency_verifier.STRICT_BASELINE_JSON),
                "load_bytes",
                return_value=(b"rejected", None),
            ),
            self.assertRaisesRegex(ClosureVerificationError, "feature-unified"),
        ):
            load_dependency_baseline(Path("does-not-need-to-exist.json"))

    @staticmethod
    def _observation(probe: object) -> ProbeClosureObservation:
        runtime_identities = {
            (package, "1.0.0", "registry+https://github.com/rust-lang/crates.io-index"):
                frozenset()
            for package in probe.required_runtime_packages
        }
        attribution_identities = {
            (package, "1.0.0", "registry+https://github.com/rust-lang/crates.io-index")
            for package in probe.required_attribution_packages
        }
        runtime = DependencyClosure(runtime_identities)
        attribution = AttributionClosure(
            tuple(
                AttributionPackage(
                    package,
                    version,
                    source,
                    (),
                    ("normal",),
                    {"normal": ()},
                )
                for package, version, source in sorted(attribution_identities)
            )
        )
        return ProbeClosureObservation(probe, runtime, attribution)

    @staticmethod
    def _toolchain() -> dict[str, object]:
        return {
            "cargo": {
                "path": "/toolchain/bin/cargo",
                "sha256": "sha256:" + "1" * 64,
            },
            "rustc": {
                "path": "/toolchain/bin/rustc",
                "sha256": "sha256:" + "2" * 64,
            },
            "cargo_version": "cargo 1.95.0",
            "rustc_verbose": "rustc 1.95.0\nhost: aarch64-apple-darwin",
            "host_target": "aarch64-apple-darwin",
        }

    @staticmethod
    def _snapshot_sha256() -> str:
        return "sha256:" + "3" * 64

    @staticmethod
    def _comparison_probe(
        *,
        runtime: tuple[dict[str, object], ...],
        attribution: tuple[dict[str, object], ...],
    ) -> dict[str, object]:
        return {
            "runtime_legal": {"packages": list(runtime)},
            "attribution": {"packages": list(attribution)},
        }

    @staticmethod
    def _runtime_package(
        package: str,
        features: tuple[str, ...],
    ) -> dict[str, object]:
        return {
            "package": package,
            "version": "1.0.0",
            "source": "registry+https://github.com/rust-lang/crates.io-index",
            "features": list(features),
        }

    @classmethod
    def _attribution_package(
        cls,
        package: str,
        features: tuple[str, ...],
        roles: tuple[str, ...],
        role_features: dict[str, tuple[str, ...]],
    ) -> dict[str, object]:
        row = cls._runtime_package(package, features)
        row["roles"] = list(roles)
        row["role_features"] = {
            role: list(values) for role, values in role_features.items()
        }
        return row


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
        failures, _ = check_case(
            case(loaded_claim=loaded_claim),
            closure,
            enforce_fingerprint=False,
        )
        self.assertTrue(any("required packages missing: missing" in x for x in failures))
        self.assertTrue(any("forbidden packages present" in x for x in failures))
        self.assertTrue(any("enables forbidden features" in x for x in failures))

    def test_residual_packages_must_remain_observed(self) -> None:
        loaded_claim = claim(
            "residual",
            required=("root",),
            residual=("upstream-residual",),
        )
        failures, _ = check_case(
            case("residual", loaded_claim=loaded_claim),
            parse_cargo_tree(tree_line("root")),
            enforce_fingerprint=False,
        )
        self.assertIn("required packages missing: upstream-residual", failures)

    def test_fingerprint_drift_is_fail_closed_but_print_mode_can_observe(self) -> None:
        closure = parse_cargo_tree(tree_line("fixture"))
        current = case(fingerprint="sha256:" + "0" * 64)

        failures, observation = check_case(current, closure)
        print_failures, _ = check_case(
            current,
            closure,
            enforce_fingerprint=False,
        )

        self.assertTrue(any("fingerprint drift" in failure for failure in failures))
        self.assertEqual(print_failures, [])
        self.assertEqual(observation.fingerprint, closure_fingerprint(closure))

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
            enforce_fingerprint=False,
        )
        self.assertTrue(any("forbidden packages present" in x for x in failures))
        self.assertTrue(any("layout-elk" in x for x in failures))
        self.assertTrue(any("system-timezone" in x for x in failures))
        self.assertTrue(any("math" in x for x in failures))


class VerificationTests(unittest.TestCase):
    def test_rustsec_authority_uses_recipe_scope_not_fingerprint_mode(self) -> None:
        observations = (
            ClosureObservation(
                profile_id="host",
                build_target_kind="host",
                closure_scope=LINUX_REFERENCE_SCOPE,
                closure_target=HOST_CLOSURE_REFERENCE_TARGET,
                package_count=1,
                package_versions=frozenset({("host", "1.0.0")}),
                observed_residual_packages=(),
                fingerprint="sha256:" + "0" * 64,
                fingerprint_enforced=False,
            ),
            ClosureObservation(
                profile_id="cross",
                build_target_kind="target-set",
                closure_scope=PROFILE_TARGET_SCOPE,
                closure_target="x86_64-unknown-linux-gnu",
                package_count=1,
                package_versions=frozenset({("cross", "1.0.0")}),
                observed_residual_packages=(),
                fingerprint="sha256:" + "0" * 64,
                fingerprint_enforced=False,
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
        fingerprint = closure_fingerprint(parse_cargo_tree(output))
        cases = tuple(
            case(
                "cross",
                loaded_recipe=loaded,
                loaded_claim=claim("cross", required=("root",)),
                target=target,
                fingerprint=fingerprint,
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
            running_host_target="aarch64-apple-darwin",
        )

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
        fingerprint = closure_fingerprint(parse_cargo_tree(output))
        cases = tuple(
            case(
                "shared",
                loaded_recipe=loaded,
                loaded_claim=claim("shared", required=("root",)),
                fingerprint=fingerprint,
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
            running_host_target=HOST_CLOSURE_REFERENCE_TARGET,
        )

        self.assertEqual(len(commands), 1)
        self.assertEqual(len(observations), 2)

    def test_host_fingerprint_is_only_enforced_on_the_reference_host(self) -> None:
        output = tree_line("fixture")

        def runner(command: Sequence[str]) -> subprocess.CompletedProcess[str]:
            return subprocess.CompletedProcess(
                command,
                0,
                stdout=output,
                stderr="",
            )

        with self.assertRaisesRegex(ClosureVerificationError, "fingerprint drift"):
            verify_cases(
                (case(fingerprint="sha256:" + "0" * 64),),
                runner=runner,
                running_host_target=HOST_CLOSURE_REFERENCE_TARGET,
            )

        observations = verify_cases(
            (case(fingerprint="sha256:" + "0" * 64),),
            runner=runner,
            running_host_target="aarch64-apple-darwin",
        )

        self.assertFalse(observations[0].fingerprint_enforced)
        self.assertEqual(
            observations[0].fingerprint,
            closure_fingerprint(parse_cargo_tree(output)),
        )

        semantic_case = case(
            loaded_claim=claim("fixture", forbidden=("forbidden",)),
            fingerprint="sha256:" + "0" * 64,
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
                running_host_target="aarch64-apple-darwin",
            )

    def test_target_set_fingerprint_remains_enforced_off_reference_host(self) -> None:
        loaded = recipe(
            "cross",
            build_target_kind="target-set",
            build_targets=("x86_64-unknown-linux-gnu",),
        )
        with self.assertRaisesRegex(ClosureVerificationError, "fingerprint drift"):
            verify_cases(
                (
                    case(
                        "cross",
                        loaded_recipe=loaded,
                        target="x86_64-unknown-linux-gnu",
                    ),
                ),
                runner=lambda command: subprocess.CompletedProcess(
                    command,
                    0,
                    stdout=tree_line("fixture"),
                    stderr="",
                ),
                running_host_target="aarch64-apple-darwin",
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
            verify_cases(
                cases,
                runner=runner,
                enforce_fingerprints=False,
                running_host_target=HOST_CLOSURE_REFERENCE_TARGET,
            )

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
            verify_cases(
                (case(),),
                runner=runner,
                enforce_fingerprints=False,
                running_host_target=HOST_CLOSURE_REFERENCE_TARGET,
            )

    def test_unknown_profile_selection_is_rejected(self) -> None:
        with self.assertRaisesRegex(
            ClosureVerificationError,
            "no dependency-closure approval",
        ):
            _select_cases((case(),), ("not-a-profile",))


if __name__ == "__main__":
    unittest.main()
