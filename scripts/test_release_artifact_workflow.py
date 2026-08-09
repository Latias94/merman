#!/usr/bin/env python3
"""Topology and failure-propagation tests for final release artifacts."""

from __future__ import annotations

import os
from pathlib import Path
import re
import subprocess
import tomllib
import unittest

try:
    from scripts.artifact_profile_recipe import load_artifact_profile
    from scripts.github_workflow_contract import (
        load_workflow_contract,
        workflow_job,
        workflow_step,
    )
except ModuleNotFoundError:
    from artifact_profile_recipe import load_artifact_profile
    from github_workflow_contract import (
        load_workflow_contract,
        workflow_job,
        workflow_step,
    )


ROOT = Path(__file__).resolve().parents[1]
WORKFLOW_PATH = ROOT / ".github" / "workflows" / "release.yml"
FULL_SHA_ACTION = re.compile(r"[^@]+@[0-9a-f]{40}\Z")


class ReleaseArtifactWorkflowTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.workflow = load_workflow_contract(WORKFLOW_PATH)

    def job(self, job_id: str) -> dict:
        return workflow_job(self.workflow, job_id)

    def assert_step_compares_observed_sha_to(
        self,
        job: dict,
        expected_sha: str,
    ) -> dict:
        matches = [
            (step, name)
            for step in job["steps"]
            for name, value in step.get("env", {}).items()
            if value == expected_sha
        ]
        self.assertEqual(len(matches), 1)
        step, variable = matches[0]
        self.assertIn(f'"$observed" != "${variable}"', step["run"])
        return step

    def test_every_action_is_pinned_to_one_commit(self) -> None:
        for job_id, job in self.workflow["jobs"].items():
            for step in job["steps"]:
                action = step.get("uses")
                if action is not None:
                    with self.subTest(job=job_id, action=action):
                        self.assertRegex(action, FULL_SHA_ACTION)

    def test_plan_binds_the_canonical_tag_to_one_commit(self) -> None:
        job = self.job("plan")
        validate = workflow_step(job, step_id="source")["run"]
        install = workflow_step(job, name="Install dist")

        self.assertEqual(job["outputs"]["source_sha"], "${{ steps.source.outputs.source_sha }}")
        self.assertEqual(job["outputs"]["tag_sha"], "${{ steps.source.outputs.tag_sha }}")
        self.assertIn('refs/tags/$RELEASE_TAG^{commit}', validate)
        self.assertIn("git rev-parse HEAD", validate)
        self.assertIn('"$source_sha" != "$tag_sha"', validate)
        self.assertIn("source_sha=", validate)
        self.assertIn("tag_sha=", validate)
        self.assertIn("sha256sum --check", install["run"])
        self.assertEqual(
            install["env"]["DIST_INSTALLER_SHA256"],
            "b657cf8c04a8b7bc28f39d220f7e6dd11bbd2bdb072c552262bd9ccf597261b5",
        )

    def test_local_builders_use_pinned_dist_and_only_the_plan_artifact(self) -> None:
        job = self.job("build-local-artifacts")
        unix = workflow_step(job, name="Install pinned dist on Unix")
        windows = workflow_step(job, name="Install pinned dist on Windows")
        plan = workflow_step(job, name="Fetch the cargo-dist plan")
        upload = workflow_step(job, name="Upload artifacts")

        self.assertIn("shasum -a 256", unix["run"])
        self.assertIn("Get-FileHash -Algorithm SHA256", windows["run"])
        self.assertEqual(plan["with"]["name"], "artifacts-plan-dist-manifest")
        self.assertNotIn("pattern", plan["with"])
        self.assertNotIn("merge-multiple", plan["with"])
        self.assertEqual(
            job["env"]["BUILD_MANIFEST_NAME"],
            "target/distrib/${{ join(matrix.targets, '-') }}-dist-manifest.json",
        )
        self.assertIn("${{ steps.cargo-dist.outputs.paths }}", upload["with"]["path"])
        self.assertIn("${{ env.BUILD_MANIFEST_NAME }}", upload["with"]["path"])

    def test_central_job_verifies_archives_before_generating_global_assets(self) -> None:
        self.assertNotIn("build-global-artifacts", self.workflow["jobs"])
        job = self.job("verify-release-archives")
        verify_step = workflow_step(job, name="Verify CLI and LSP archive structure")
        prepare_step = workflow_step(job, name="Prepare verified global-generation snapshots")
        generate_step = workflow_step(job, name="Generate final installers and checksum index")
        assemble_step = workflow_step(job, name="Assemble the verified bundle")
        verify = verify_step["run"]
        prepare = prepare_step["run"]
        generate = generate_step["run"]
        assemble = assemble_step["run"]
        local_download = workflow_step(job, name="Download isolated local cargo-dist artifacts")
        plan_download = workflow_step(job, name="Download the trusted cargo-dist plan")

        self.assertEqual(job["needs"], ["plan", "build-local-artifacts"])
        self.assertEqual(local_download["with"]["pattern"], "artifacts-build-local-*")
        self.assertNotIn("merge-multiple", local_download["with"])
        self.assertEqual(plan_download["with"]["name"], "artifacts-plan-dist-manifest")
        for package, profile, verifier in (
            ("merman-cli", "cli-release", "scripts/verify_cli_release_archive.py"),
            ("merman-lsp", "lsp-stdio-release", "scripts/verify_lsp_release_archive.py"),
        ):
            with self.subTest(package=package):
                self.assertIn(f"{package}:{profile}:{verifier}", verify)
        self.assertNotIn("--execute", verify)
        self.assertIn('--verified-output "$VERIFIED_ARCHIVE_DIR/$archive_name"', verify)
        self.assertIn("prepare-global", prepare)
        self.assertIn('"$LOCAL_ARTIFACT_DIR"', prepare)
        self.assertLess(
            prepare.index('"$LOCAL_ARTIFACT_DIR"'),
            prepare.index('"$VERIFIED_ARCHIVE_DIR"'),
        )
        self.assertIn("dist build", generate)
        self.assertIn("--artifacts=global", generate)
        self.assertIn("> generated-global-dist-manifest.json", generate)
        self.assertIn(
            'mv generated-global-dist-manifest.json "$GENERATED_ASSET_DIR/dist-manifest.json"',
            generate,
        )
        self.assertIn("release_artifact_bundle.py harden-installers", generate)
        self.assertIn("sh -n", generate)
        self.assertIn("release_artifact_bundle.py assemble", assemble)
        self.assertNotIn("/.release-verification", job["env"]["RELEASE_ASSET_DIR"])
        self.assertLess(job["steps"].index(verify_step), job["steps"].index(prepare_step))
        self.assertLess(job["steps"].index(prepare_step), job["steps"].index(generate_step))
        self.assertLess(job["steps"].index(generate_step), job["steps"].index(assemble_step))

    def test_central_assembly_has_no_raw_archive_fallback(self) -> None:
        job = self.job("verify-release-archives")
        assemble = workflow_step(job, name="Assemble the verified bundle")["run"]
        self.assertIn('"$GENERATED_ASSET_DIR"', assemble)
        self.assertIn('"$VERIFIED_ARCHIVE_DIR"', assemble)
        self.assertNotIn("RAW_ARTIFACT_DIR", job["env"])

    def test_native_matrix_matches_both_release_profiles_and_dist_runners(self) -> None:
        job = self.job("verify-release-archives-native")
        observed = {
            (row["package"], row["target"]): (row["verifier"], row["runner"])
            for row in job["matrix_include"]
        }
        cli_targets = set(load_artifact_profile("cli-release").build_targets)
        lsp_targets = set(load_artifact_profile("lsp-stdio-release").build_targets)
        expected_runners = {
            "aarch64-apple-darwin": "macos-15",
            "x86_64-apple-darwin": "macos-15-intel",
            "x86_64-unknown-linux-gnu": "ubuntu-24.04",
            "x86_64-pc-windows-msvc": "windows-2025",
        }
        self.assertEqual(cli_targets, lsp_targets)
        expected = {
            (package, target): (verifier, expected_runners[target])
            for package, verifier in (
                ("merman-cli", "scripts/verify_cli_release_archive.py"),
                ("merman-lsp", "scripts/verify_lsp_release_archive.py"),
            )
            for target in cli_targets
        }
        self.assertEqual(observed, expected)

    def test_each_native_cell_executes_only_its_product_from_the_pinned_bundle(self) -> None:
        job = self.job("verify-release-archives-native")
        checkout = job["steps"][0]
        download = workflow_step(job, name="Download verified release bundle")
        parse_installers = workflow_step(job, name="Parse generated PowerShell installers")
        execute_step = workflow_step(job, name="Execute final product archive")
        execute = execute_step["run"]

        self.assertEqual(checkout["with"]["ref"], "${{ needs.plan.outputs.source_sha }}")
        self.assertEqual(download["with"]["name"], "verified-release-assets")
        self.assertNotIn("pattern", download["with"])
        self.assertEqual(parse_installers["if"], "runner.os == 'Windows'")
        self.assertEqual(parse_installers["shell"], "pwsh")
        self.assertIn("[scriptblock]::Create", parse_installers["run"])
        self.assertIn("$env:PACKAGE-installer.ps1", parse_installers["run"])
        self.assertNotIn("SOURCE_SHA", job["env"])
        self.assertEqual(job["env"]["PACKAGE"], "${{ matrix.package }}")
        self.assertEqual(job["env"]["VERIFIER"], "${{ matrix.verifier }}")
        self.assertNotIn("for specification", execute)
        self.assertIn('archive="$PACKAGE-$TARGET.$extension"', execute)
        self.assertIn('python3 "$VERIFIER"', execute)
        self.assertIn("--execute", execute)
        self.assertNotIn("if", execute_step)
        self.assertNotIn("--verified-output", execute)
        self.assertIn('"verified-release-assets/$archive.sha256"', execute)
        self.assertFalse(any("receipt" in step.get("name", "").lower() for step in job["steps"]))

    def test_aggregate_gate_fails_closed_for_every_upstream_failure(self) -> None:
        gate = self.job("release-verification-gate")
        checkout = workflow_step(gate, name="Checkout verified release source")
        step = workflow_step(gate, name="Require central and native verification success")
        reverify = workflow_step(gate, name="Reverify the immutable release bundle")
        self.assertEqual(checkout["with"]["ref"], "${{ needs.plan.outputs.source_sha }}")
        self.assertLess(gate["steps"].index(step), gate["steps"].index(checkout))
        self.assertLess(gate["steps"].index(checkout), gate["steps"].index(reverify))
        self.assertIn("verify-bundle", reverify["run"])
        self.assertIn('--version "$RELEASE_VERSION"', reverify["run"])
        self.assertIn('--source-sha "$SOURCE_SHA"', reverify["run"])
        cases = {
            ("success", "success"): True,
            ("failure", "success"): False,
            ("success", "failure"): False,
            ("cancelled", "success"): False,
            ("success", "skipped"): False,
        }
        for (central, native), succeeds in cases.items():
            with self.subTest(central=central, native=native):
                result = subprocess.run(
                    ["bash", "-c", step["run"]],
                    cwd=ROOT,
                    env={
                        "PATH": os.environ.get("PATH", ""),
                        "CENTRAL_RESULT": central,
                        "NATIVE_RESULT": native,
                    },
                    text=True,
                    capture_output=True,
                    timeout=10,
                    check=False,
                )
                self.assertEqual(result.returncode == 0, succeeds, msg=result.stderr)

    def test_downstream_jobs_have_no_path_around_the_aggregate_gate(self) -> None:
        for job_id in ("generate-cli-registry-candidates", "attest-release-assets", "host"):
            with self.subTest(job=job_id):
                job = self.job(job_id)
                self.assertIn("release-verification-gate", job["needs"])
                self.assertIn("needs.release-verification-gate.result == 'success'", job["if"])
        for job_id in ("generate-cli-registry-candidates", "attest-release-assets"):
            with self.subTest(job=job_id):
                self.assertNotIn("verify-release-archives", self.job(job_id)["needs"])

    def test_stable_candidates_are_native_and_prereleases_skip_explicitly(self) -> None:
        job = self.job("generate-cli-registry-candidates")
        checkout = next(step for step in job["steps"] if step.get("uses", "").startswith("actions/checkout@"))
        download = workflow_step(job, name="Download verified release bundle")
        generate = workflow_step(job, name="Generate and validate stable registry candidates")
        prerelease = workflow_step(job, name="Record prerelease candidate policy")
        upload = workflow_step(job, name="Upload stable registry candidates")

        self.assertEqual(job["runs-on"], "windows-2025")
        self.assertIn("needs.plan.outputs.prerelease == 'true'", prerelease["if"])
        stable_condition = "${{ needs.plan.outputs.prerelease != 'true' }}"
        self.assertEqual(
            {checkout["if"], download["if"], generate["if"], upload["if"]},
            {stable_condition},
        )
        self.assertEqual(download["with"]["name"], "verified-release-assets")
        self.assertNotIn("pattern", download["with"])
        self.assertIn("verified-release-assets", generate["run"])
        self.assertIn("scripts/generate_cli_registry_candidates.py", generate["run"])
        self.assertIn("Add-AppxPackage", generate["run"])
        self.assertIn("Microsoft.DesktopAppInstaller_8wekyb3d8bbwe", generate["run"])
        self.assertIn("winget", generate["run"])
        self.assertIn("--disable-interactivity", generate["run"])

    def test_attestation_is_pinned_least_privilege_and_environment_protected(self) -> None:
        job = self.job("attest-release-assets")
        download = workflow_step(job, name="Download verified release bundle")
        tag = self.assert_step_compares_observed_sha_to(
            job,
            "${{ needs.plan.outputs.tag_sha }}",
        )
        attest = workflow_step(job, name="Attest verified release assets")

        self.assertEqual(job["environment"], "github-release")
        self.assertEqual(
            job["permissions"],
            {
                "contents": "read",
                "id-token": "write",
                "attestations": "write",
                "artifact-metadata": "write",
            },
        )
        self.assertRegex(download["uses"], FULL_SHA_ACTION)
        self.assertRegex(attest["uses"], FULL_SHA_ACTION)
        self.assertIn("commits/$RELEASE_TAG", tag["run"])
        self.assertEqual(attest["with"]["subject-path"], "verified-release-assets/*")

    def test_host_uploads_the_same_pinned_bundle_without_repacking(self) -> None:
        job = self.job("host")
        download = workflow_step(job, name="Download GitHub Artifacts")
        tag = self.assert_step_compares_observed_sha_to(
            job,
            "${{ needs.plan.outputs.tag_sha }}",
        )
        create = workflow_step(job, name="Create GitHub Release")["run"]

        self.assertEqual(job["permissions"], {"contents": "write"})
        self.assertEqual(job["environment"], "github-release")
        self.assertEqual(
            job["needs"],
            [
                "plan",
                "release-verification-gate",
                "generate-cli-registry-candidates",
                "attest-release-assets",
            ],
        )
        for required in (
            "needs.release-verification-gate.result == 'success'",
            "needs.generate-cli-registry-candidates.result == 'success'",
            "needs.attest-release-assets.result == 'success'",
        ):
            self.assertIn(required, job["if"])
        self.assertRegex(download["uses"], FULL_SHA_ACTION)
        self.assertEqual(download["with"]["name"], "verified-release-assets")
        self.assertNotIn("pattern", download["with"])
        self.assertIn("commits/$RELEASE_TAG", tag["run"])
        for forbidden in ("dist build", "tar ", "zip ", "--target", "verify_cli_release_archive.py"):
            self.assertNotIn(forbidden, create)

    def test_cargo_dist_source_tarball_is_intentionally_disabled(self) -> None:
        config = (ROOT / "dist-workspace.toml").read_text(encoding="utf-8")
        self.assertIn("source-tarball = false", config)

    def test_archive_includes_are_explicit_for_both_products(self) -> None:
        with (ROOT / "dist-workspace.toml").open("rb") as source:
            workspace_dist = tomllib.load(source)["dist"]
        self.assertFalse(workspace_dist["auto-includes"])
        self.assertEqual(
            workspace_dist["include"],
            [
                "CHANGELOG.md",
                "LICENSE-APACHE",
                "LICENSE-MIT",
                "THIRD_PARTY_NOTICES.md",
                "THIRD_PARTY_LICENSES/",
            ],
        )
        package_common = [
            "../../CHANGELOG.md",
            "../../LICENSE-APACHE",
            "../../LICENSE-MIT",
            "../../THIRD_PARTY_NOTICES.md",
            "../../THIRD_PARTY_LICENSES/",
        ]
        package_specific = {
            "merman-cli": ["README.md", "assets/completions/", "assets/man/"],
            "merman-lsp": ["README.md"],
        }
        for package, include in package_specific.items():
            with (ROOT / f"crates/{package}/Cargo.toml").open("rb") as source:
                package_dist = tomllib.load(source)["package"]["metadata"]["dist"]
            with self.subTest(package=package):
                self.assertFalse(package_dist["auto-includes"])
                # cargo-dist package layers replace the workspace include list, so every
                # package-local layer must repeat the common legal payload explicitly.
                self.assertEqual(package_dist["include"], [*package_common, *include])

    def test_legal_payloads_are_checkout_stable_on_windows(self) -> None:
        paths = (
            "THIRD_PARTY_LICENSES/katex-fonts/FONT_NOTICE.txt",
            "THIRD_PARTY_LICENSES/katex-fonts/OFL.txt",
            "THIRD_PARTY_LICENSES/ratex/THIRD_PARTY_NOTICES.txt",
            "crates/merman-cli/THIRD_PARTY_LICENSES/future-license.txt",
        )
        result = subprocess.run(
            ["git", "check-attr", "eol", "--", *paths],
            cwd=ROOT,
            text=True,
            capture_output=True,
            timeout=10,
            check=True,
        )
        for path in paths:
            with self.subTest(path=path):
                self.assertIn(f"{path}: eol: lf", result.stdout)


if __name__ == "__main__":
    unittest.main()
