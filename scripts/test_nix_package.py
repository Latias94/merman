#!/usr/bin/env python3
"""Static contracts for the reusable Merman Nix source package."""

from __future__ import annotations

import json
import os
from pathlib import Path, PurePosixPath
import re
import tomllib
import unittest

from github_workflow_contract import load_workflow_contract, workflow_job, workflow_step


ROOT = Path(__file__).resolve().parents[1]
POLICY_PATH = ROOT / "nix/source-policy.json"
PACKAGE_PATH = ROOT / "nix/package.nix"
FLAKE_PATH = ROOT / "flake.nix"
LOCK_PATH = ROOT / "flake.lock"
SOURCE_SIZE_BUDGET = 128 * 1024 * 1024


def source_policy() -> dict:
    return json.loads(POLICY_PATH.read_text(encoding="utf-8"))


def workspace_members() -> list[str]:
    workspace = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
    return workspace["workspace"]["members"]


def included_by_policy(relative: str, *, is_directory: bool = False) -> bool:
    policy = source_policy()
    path = PurePosixPath(relative)
    parts = path.parts
    if not parts or relative.startswith("/") or ".." in parts:
        return False
    directory_parts = parts if is_directory else parts[:-1]
    if set(directory_parts) & set(policy["excluded_directory_names"]):
        return False
    if not is_directory and path.name in policy["excluded_file_names"]:
        return False
    if len(parts) == 1:
        allowed_trees = policy["root_directories"] + workspace_members()
        return (
            relative in policy["root_files"]
            or any(tree == relative or tree.startswith(f"{relative}/") for tree in allowed_trees)
            or relative == "scripts"
        )
    if parts[0] == "scripts":
        return relative in policy["script_files"]
    allowed_trees = policy["root_directories"] + workspace_members()
    return any(
        relative == tree
        or relative.startswith(f"{tree}/")
        or tree.startswith(f"{relative}/")
        for tree in allowed_trees
    )


def selected_source_files() -> list[Path]:
    policy = source_policy()
    selected = [ROOT / relative for relative in policy["root_files"]]
    selected.extend(ROOT / relative for relative in policy["script_files"])
    excluded = set(policy["excluded_directory_names"])
    excluded_files = set(policy["excluded_file_names"])
    for relative in policy["root_directories"] + workspace_members():
        source_root = ROOT / relative
        for directory, names, files in os.walk(source_root, followlinks=False):
            names[:] = [
                name
                for name in names
                if name not in excluded and not (Path(directory) / name).is_symlink()
            ]
            selected.extend(
                Path(directory) / name
                for name in files
                if name not in excluded_files and not (Path(directory) / name).is_symlink()
            )
    return selected


class NixPackageContractTests(unittest.TestCase):
    def test_source_policy_is_small_explicit_and_safe(self) -> None:
        policy = source_policy()
        self.assertEqual(policy["schema_version"], 1)
        for key in (
            "root_files",
            "root_directories",
            "script_files",
            "excluded_directory_names",
            "excluded_file_names",
        ):
            values = policy[key]
            self.assertIsInstance(values, list)
            self.assertEqual(len(values), len(set(values)), key)
        self.assertEqual(
            policy["root_directories"],
            ["THIRD_PARTY_LICENSES", "capabilities"],
        )
        self.assertEqual(
            policy["script_files"],
            ["scripts/verify_cli_installation.py"],
        )
        self.assertIn("target", policy["excluded_directory_names"])
        self.assertIn("node_modules", policy["excluded_directory_names"])

    def test_source_policy_includes_build_inputs_and_excludes_repository_state(self) -> None:
        required = {
            "Cargo.toml",
            "Cargo.lock",
            "THIRD_PARTY_LICENSES/rust-cargo-dependencies.json",
            "capabilities/artifact-profiles-v1.json",
            "crates/merman-bindings-core/src/generated/capability_surface.rs",
            "crates/merman-cli/Cargo.toml",
            "crates/merman-cli/assets/completions/merman-cli.bash",
            "crates/merman-cli/assets/man/merman-cli.1",
            "scripts/verify_cli_installation.py",
        }
        required.update(f"{member}/Cargo.toml" for member in workspace_members())
        for relative in sorted(required):
            with self.subTest(relative=relative):
                self.assertTrue((ROOT / relative).is_file())
                self.assertTrue(included_by_policy(relative))

        excluded = (
            ".git/config",
            "CHANGELOG.md",
            "docs/releasing/CLI.md",
            "fixtures/example.mmd",
            "platforms/web/package.json",
            "repo-ref/mermaid/package.json",
            "scripts/test_nix_package.py",
            "target/release/merman-cli",
            "tools/upstreams/REPOS.lock.json",
            "crates/merman-node/target/release/merman-cli",
            "crates/merman-node/src/lib.rs",
            "crates/merman-wasm/pkg/merman.js",
        )
        for relative in excluded:
            with self.subTest(relative=relative):
                self.assertFalse(included_by_policy(relative))

    def test_filtered_source_inventory_stays_bounded(self) -> None:
        files = selected_source_files()
        self.assertGreater(len(files), 100)
        self.assertTrue(all(path.is_file() and not path.is_symlink() for path in files))
        size = sum(path.stat().st_size for path in files)
        self.assertLess(size, SOURCE_SIZE_BUDGET)

    def test_package_derives_the_exact_profile_instead_of_copying_features(self) -> None:
        text = PACKAGE_PATH.read_text(encoding="utf-8")
        self.assertIn('candidate.id == "cli-release"', text)
        self.assertIn("buildType = profile.cargo.profile;", text)
        self.assertIn("buildNoDefaultFeatures = !profile.cargo.default_features;", text)
        self.assertIn("buildFeatures = profile.cargo.features;", text)
        self.assertIn("source-policy.json", text)
        self.assertIn("verify_cli_installation.py", text)
        self.assertIn('top = if segments == [ ] then "" else builtins.head segments;', text)
        self.assertNotRegex(text, r"buildFeatures\s*=\s*\[")

    def test_flake_is_thin_and_lock_is_immutable(self) -> None:
        flake = FLAKE_PATH.read_text(encoding="utf-8")
        self.assertNotIn("./nix/package.nix", flake)
        self.assertEqual(flake.count("import ./default.nix"), 1)
        self.assertIn("packages = forAllSystems", flake)
        self.assertIn("apps = forAllSystems", flake)
        self.assertIn("checks = forAllSystems", flake)
        self.assertIn("source-contract", flake)

        lock = json.loads(LOCK_PATH.read_text(encoding="utf-8"))
        self.assertEqual(lock["version"], 7)
        nixpkgs = lock["nodes"]["nixpkgs"]
        self.assertEqual(nixpkgs["original"]["ref"], "nixos-26.05")
        self.assertRegex(nixpkgs["locked"]["rev"], r"^[0-9a-f]{40}$")
        self.assertRegex(
            nixpkgs["locked"]["narHash"],
            r"^sha256-[A-Za-z0-9+/]{43}=$",
        )

    def test_ci_builds_reusable_and_flake_interfaces_before_running_the_app(self) -> None:
        workflow = load_workflow_contract(ROOT / ".github/workflows/ci.yml")
        job = workflow_job(workflow, "nix-cli-package")
        self.assertEqual(job["runs-on"], "ubuntu-24.04")
        install = workflow_step(job, name="Install Nix")
        self.assertEqual(
            install["uses"],
            "DeterminateSystems/determinate-nix-action@v3.21.8",
        )
        reusable = workflow_step(job, name="Build reusable Nix derivation")
        self.assertIn("nix-build", reusable["run"])
        self.assertIn("builtins.getFlake", reusable["run"])
        check = workflow_step(job, name="Check Nix flake")
        self.assertIn("nix flake check", check["run"])
        self.assertIn("--all-systems --no-build", check["run"])
        self.assertIn("--no-write-lock-file", check["run"])
        run = workflow_step(job, name="Run Nix flake app")
        self.assertIn("nix run", run["run"])
        immutable = workflow_step(job, name="Check immutable Nix lock")
        self.assertEqual(immutable["run"], "git diff --exit-code -- flake.lock")


if __name__ == "__main__":
    unittest.main()
