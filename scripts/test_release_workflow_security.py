#!/usr/bin/env python3
"""Focused repository-specific security boundaries for GitHub Actions workflows."""

from __future__ import annotations

import json
import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
WORKFLOW_ROOT = ROOT / ".github" / "workflows"
CI_WORKFLOW = WORKFLOW_ROOT / "ci.yml"
PR_REACHABLE_WORKFLOWS = [
    CI_WORKFLOW,
    WORKFLOW_ROOT / "fuzz.yml",
    WORKFLOW_ROOT / "npm-audit.yml",
    WORKFLOW_ROOT / "pages.yml",
    WORKFLOW_ROOT / "security-audit.yml",
    WORKFLOW_ROOT / "tree-sitter-mermaid.yml",
    WORKFLOW_ROOT / "vscode-extension.yml",
]
PUBLISH_WORKFLOWS = sorted(WORKFLOW_ROOT.glob("release-*.yml")) + [
    WORKFLOW_ROOT / "release.yml",
    WORKFLOW_ROOT / "pages-deploy.yml",
]
WRITE_CAPABILITIES = (
    "actions: write",
    "attestations: write",
    "contents: write",
    "deployments: write",
    "id-token: write",
    "packages: write",
    "pages: write",
    "pull-requests: write",
    "security-events: write",
)


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def workflow_job(text: str, name: str) -> str:
    jobs = text.split("\njobs:\n", 1)
    if len(jobs) != 2:
        raise AssertionError("workflow has no jobs mapping")
    marker = f"  {name}:\n"
    start = jobs[1].find(marker)
    if start < 0:
        raise AssertionError(f"workflow has no {name!r} job")
    body = jobs[1][start + len(marker) :]
    next_job = re.search(r"(?m)^  [A-Za-z0-9_-]+:\s*$", body)
    if next_job is not None:
        body = body[: next_job.start()]
    return marker + body


def assert_no_npm_provenance_disable(test_case: unittest.TestCase, text: str) -> None:
    patterns = (
        r"(?:^|\s)--no-provenance(?:\s|$)",
        r"(?:^|\s)(?:--)?provenance\s*=\s*false(?:\s|$)",
        r"(?:^|\s)NPM_CONFIG_PROVENANCE\s*[:=]\s*[\"']?false[\"']?(?:\s|$)",
        r'"provenance"\s*:\s*false',
    )
    for pattern in patterns:
        with test_case.subTest(pattern=pattern):
            test_case.assertIsNone(re.search(pattern, text, re.IGNORECASE))


class WorkflowSecurityBoundaries(unittest.TestCase):
    def test_pull_request_orchestrator_and_reusable_owners_are_read_only(self) -> None:
        for path in [*PR_REACHABLE_WORKFLOWS, WORKFLOW_ROOT / "performance.yml"]:
            text = read(path)
            with self.subTest(workflow=path.name):
                for capability in WRITE_CAPABILITIES:
                    self.assertNotIn(capability, text)
                self.assertNotIn("secrets: inherit", text)
                self.assertNotRegex(text, r"(?m)^\s+environment:\s*(?:$|\S)")
                self.assertGreaterEqual(
                    text.count("persist-credentials: false"),
                    text.count("uses: actions/checkout@"),
                )

    def test_ci_is_the_only_pull_request_entrypoint_for_reusable_owners(self) -> None:
        ci = read(CI_WORKFLOW)
        self.assertIn("  pull_request:\n", ci)
        self.assertIn("  merge_group:\n", ci)
        self.assertNotIn("    paths:\n", ci.split("permissions:", 1)[0])
        for path in PR_REACHABLE_WORKFLOWS[1:]:
            with self.subTest(workflow=path.name):
                self.assertNotIn("  pull_request:", read(path))

    def test_main_security_calls_the_full_dependency_closure(self) -> None:
        ci = read(CI_WORKFLOW)
        self.assertIn(
            "representative_targets: ${{ github.event_name == 'pull_request' || github.event_name == 'merge_group' }}",
            ci,
        )
        self.assertNotIn("representative_targets: ${{ github.event_name != 'push' }}", ci)

    def test_pr_gate_is_same_run_and_fail_closed(self) -> None:
        ci = read(CI_WORKFLOW)
        self.assertIn("  pr-gate:\n", ci)
        self.assertIn(
            "name: ${{ (github.event_name == 'pull_request' || github.event_name == 'merge_group') && 'pr-gate' || format('{0}-gate', github.event_name) }}",
            ci,
        )
        self.assertNotIn("\n    name: pr-gate\n", ci)
        self.assertIn("if: ${{ always() }}", ci)
        self.assertIn("python3 scripts/ci_plan.py gate", ci)
        self.assertNotIn("workflow_run:", ci)
        self.assertNotIn("secrets: inherit", ci)

    def test_pages_credentials_exist_only_in_the_deployment_owner(self) -> None:
        reusable = read(WORKFLOW_ROOT / "pages.yml")
        deployment = read(WORKFLOW_ROOT / "pages-deploy.yml")
        self.assertNotIn("pages: write", reusable)
        self.assertNotIn("id-token: write", reusable)
        self.assertIn("pages: write", deployment)
        self.assertIn("id-token: write", deployment)
        self.assertIn("environment:\n      name: github-pages", deployment)
        self.assertIn("uses: ./.github/workflows/pages.yml", deployment)

    def test_publish_workflows_do_not_persist_checkout_tokens(self) -> None:
        for path in PUBLISH_WORKFLOWS:
            text = read(path)
            with self.subTest(workflow=path.name):
                self.assertGreaterEqual(
                    text.count("persist-credentials: false"),
                    text.count("uses: actions/checkout@"),
                )

    def test_publish_workflow_actions_are_immutable(self) -> None:
        for path in PUBLISH_WORKFLOWS:
            for line_number, line in enumerate(read(path).splitlines(), start=1):
                match = re.search(r"\buses:\s*([^\s#]+)", line)
                if match is None:
                    continue
                reference = match.group(1)
                if reference.startswith("./"):
                    continue
                self.assertIn(
                    "@",
                    reference,
                    f"{path.name}:{line_number} must identify an action ref",
                )
                ref = reference.rsplit("@", 1)[1]
                self.assertRegex(
                    ref,
                    r"^[0-9a-f]{40}$",
                    f"{path.name}:{line_number} uses a mutable or malformed action ref",
                )

    def test_tree_sitter_mermaid_release_is_protected_and_subdirectory_aware(self) -> None:
        independent = read(WORKFLOW_ROOT / "release-independent-crate.yml")
        workflow = read(WORKFLOW_ROOT / "release-tree-sitter-mermaid.yml")
        verify = workflow_job(workflow, "verify")
        prebuild = workflow_job(workflow, "prebuild")
        assemble = workflow_job(workflow, "assemble")
        attest = workflow_job(workflow, "attest")
        publish_crates = workflow_job(workflow, "publish-crates")
        publish_npm = workflow_job(workflow, "publish-npm")
        publish_github = workflow_job(workflow, "publish-github")

        self.assertNotIn("- tree-sitter-mermaid", independent)
        self.assertIn("tree-sitter-mermaid-v", workflow)
        self.assertIn("publish_github_release:", workflow)
        self.assertIn("default: false", workflow)
        self.assertIn("main is build-only", workflow)
        self.assertIn("publishing requires source_ref to be the matching immutable tag", workflow)
        self.assertIn(
            "a GitHub Release requires registry publication or reconciliation", workflow
        )
        self.assertIn("distribution/tree-sitter-mermaid", workflow)
        self.assertIn('npm_manifest["name"] != "@mermanjs/tree-sitter-mermaid"', verify)
        self.assertIn("npm run prebuild", prebuild)
        self.assertIn("PREBUILDS_ONLY=1 npm run test:node", prebuild)
        self.assertIn("TREE_SITTER_MERMAID_REQUIRE_PREBUILDS=1", assemble)
        self.assertIn('"metadata/provenance.json"', verify)
        self.assertIn('"CMakeLists.txt": cmake.group(1)', verify)
        self.assertIn('"Makefile": make.group(1)', verify)
        self.assertIn(
            "cargo package --locked --no-verify -p tree-sitter-mermaid",
            publish_crates,
        )
        self.assertIn(
            '"$release_dir/tree-sitter-mermaid-$VERSION.crate"', publish_crates
        )
        self.assertIn(
            '"target/package/tree-sitter-mermaid-$VERSION.crate"', publish_crates
        )
        self.assertIn(
            "cargo publish --locked --no-verify -p tree-sitter-mermaid",
            publish_crates,
        )
        self.assertIn(
            "https://crates.io/api/v1/crates/tree-sitter-mermaid/$VERSION/download",
            publish_crates,
        )
        self.assertIn('--user-agent "$registry_user_agent"', publish_crates)
        self.assertIn("npm publish", publish_npm)
        self.assertIn("--provenance", publish_npm)
        self.assertIn("--ignore-scripts", publish_npm)
        self.assertIn(
            'npm view "@mermanjs/tree-sitter-mermaid@$VERSION" dist.tarball',
            publish_npm,
        )
        self.assertIn(
            "uses: actions/attest@1e69f48acb82d1966a394da916b4c1698aa569d6 # v4.2.2",
            attest,
        )
        self.assertIn("attestations: write", attest)
        self.assertIn("gh release create", publish_github)
        self.assertIn("verify_existing_release", publish_github)
        self.assertIn(
            "if: ${{ inputs.publish && inputs.publish_github_release }}",
            publish_github,
        )
        self.assertIn("environment: crates.io", publish_crates)
        self.assertIn("environment: npm", publish_npm)
        self.assertIn("environment: github-release", publish_github)
        self.assertIn(
            "needs: [validate-inputs, assemble, attest, publish-crates, publish-npm]",
            publish_github,
        )
        self.assertIn("timeout-minutes: 60", verify)
        self.assertNotIn("recovery_run_id:", workflow)
        self.assertNotIn("Verify recovery run identity", workflow)
        self.assertIn("Download native prebuilds from this run", assemble)

    def test_workflow_job_does_not_accept_a_protection_from_another_job(self) -> None:
        workflow = """
jobs:
  wrong-owner:
    environment: npm
  publish-npm:
    runs-on: ubuntu-latest
"""
        self.assertNotIn("environment: npm", workflow_job(workflow, "publish-npm"))

    def test_tree_sitter_owner_runs_its_complete_package_gate(self) -> None:
        ci = read(CI_WORKFLOW)
        workflow = read(WORKFLOW_ROOT / "tree-sitter-mermaid.yml")

        self.assertIn("uses: ./.github/workflows/tree-sitter-mermaid.yml", ci)
        self.assertIn("cargo fmt --all --check", ci)
        self.assertNotIn("cargo fmt --all", workflow)
        self.assertIn("cargo clippy --locked -p tree-sitter-mermaid", workflow)
        self.assertIn("cargo nextest run --locked -p tree-sitter-mermaid", workflow)
        self.assertIn("npm run check:generated --prefix distribution/tree-sitter-mermaid", workflow)
        self.assertIn("npm run test:corpus --prefix distribution/tree-sitter-mermaid", workflow)
        self.assertIn("npm run test:wasm --prefix distribution/tree-sitter-mermaid", workflow)
        self.assertIn("npm run test:package-smoke --prefix distribution/tree-sitter-mermaid", workflow)

    def test_workspace_release_accepts_only_canonical_workspace_tags(self) -> None:
        text = read(WORKFLOW_ROOT / "release.yml")
        self.assertIn("      - 'v*'\n", text)
        self.assertNotIn("flutter-v", text.split("  workflow_dispatch:", 1)[0])
        self.assertNotIn("tree-sitter-mermaid-v", text.split("  workflow_dispatch:", 1)[0])

    def test_crates_publish_uses_trusted_receipt_operator_and_immutable_source(self) -> None:
        text = read(WORKFLOW_ROOT / "release-crates.yml")
        self.assertIn("ref: ${{ github.workflow_sha }}", text)
        self.assertIn("trusted/scripts/crates_io_release.py publish-receipted", text)
        self.assertIn("--source-tree \"$SOURCE_TREE\"", text)
        self.assertIn("Upload crates.io receipts", text)
        self.assertIn("recovery_run_id:", text)
        self.assertIn("Download prior crates.io receipts for recovery", text)
        self.assertIn("--recovery-receipts-dir recovery-receipts", text)
        self.assertNotIn('default: "main"', text)
        self.assertNotIn('SOURCE_REF" == "main"', text)
        self.assertNotIn("cargo yank", text)
        self.assertNotIn("--token \"$CARGO_REGISTRY_TOKEN\"", text)

    def test_roughr_semver_gate_is_pinned_before_publication(self) -> None:
        workflows = {
            WORKFLOW_ROOT / "release-preflight.yml": (
                "cargo semver-checks check-release -p roughr-merman --color always",
                "python3 scripts/crates_io_release.py preflight-initial",
            ),
            WORKFLOW_ROOT / "release-crates.yml": (
                "cargo semver-checks check-release -p roughr-merman --color always",
                "trusted/scripts/crates_io_release.py publish-receipted",
            ),
            WORKFLOW_ROOT / "release-independent-crate.yml": (
                'cargo semver-checks check-release -p "$PACKAGE" --color always',
                'cargo publish -p "$PACKAGE" --locked --no-verify --registry crates-io --token "$CARGO_REGISTRY_TOKEN"',
            ),
        }
        for path, (semver_command, publication_command) in workflows.items():
            text = read(path)
            with self.subTest(workflow=path.name):
                self.assertIn("tool: cargo-semver-checks@0.50.0", text)
                self.assertIn(semver_command, text)
                self.assertLess(
                    text.index("cargo semver-checks check-release"),
                    text.index(publication_command),
                )

    def test_release_preflight_requires_dated_changelog_projections(self) -> None:
        text = read(WORKFLOW_ROOT / "release-preflight.yml")
        self.assertIn(
            'scripts/verify_release_changelog.py --version "$VERSION" --require-date',
            text,
        )

    def test_npm_publish_provenance_cannot_be_disabled_by_repository_config(self) -> None:
        paths = [
            ROOT / ".npmrc",
            ROOT / "platforms" / "web" / ".npmrc",
            ROOT / "platforms" / "node" / ".npmrc",
            WORKFLOW_ROOT / "release-web.yml",
            WORKFLOW_ROOT / "release-node.yml",
            WORKFLOW_ROOT / "release-tree-sitter-mermaid.yml",
        ]
        for path in paths:
            if path.exists():
                with self.subTest(path=path.relative_to(ROOT).as_posix()):
                    assert_no_npm_provenance_disable(self, read(path))

        for package_json in [
            ROOT / "platforms" / "web" / "package.json",
            ROOT / "platforms" / "node" / "package.json",
        ]:
            manifest = json.loads(read(package_json))
            self.assertIsNot(manifest.get("publishConfig", {}).get("provenance"), False)

    def test_npm_manual_publish_requires_an_immutable_source(self) -> None:
        for workflow in ("release-web.yml", "release-node.yml"):
            text = read(WORKFLOW_ROOT / workflow)
            with self.subTest(workflow=workflow):
                self.assertIn("publish_to_npm:", text)
                self.assertIn("default: false", text)
                self.assertIn("DISPATCH_PUBLISH_TO_NPM", text)
                self.assertIn("main is allowed only for a non-publishing build", text)
                self.assertRegex(text, r"\[\[ \"\$SOURCE_REF\" =~ \^\[0-9a-f\]\{40\}\$ \]\]")

    def test_release_assets_fail_closed_without_clobber(self) -> None:
        for path in (
            WORKFLOW_ROOT / "release-android.yml",
            WORKFLOW_ROOT / "release-apple.yml",
            WORKFLOW_ROOT / "release-python.yml",
        ):
            text = read(path)
            with self.subTest(workflow=path.name):
                self.assertNotIn("--clobber", text)
                self.assertIn("gh release upload", text)
                self.assertNotIn("gh release download", text)
                self.assertNotIn("cmp ", text)

    def test_pypi_skip_existing_is_guarded_by_checksum_reconciliation(self) -> None:
        text = read(WORKFLOW_ROOT / "release-python.yml")
        publish = workflow_job(text, "publish")
        self.assertIn("Checkout trusted release verifier", publish)
        self.assertIn("path: trusted", publish)
        self.assertIn("scripts/reconcile_pypi_wheels.py", text)
        self.assertIn("skip-existing: true", text)

    def test_pubdev_skip_existing_is_guarded_by_archive_reconciliation(self) -> None:
        text = read(WORKFLOW_ROOT / "release-flutter.yml")
        self.assertIn("scripts/reconcile_pub_package.py", text)
        self.assertIn("exact)", text)
        self.assertIn("exists=true", text)

    def test_independent_crate_has_fresh_registry_dependent_compile_gate(self) -> None:
        independent = read(WORKFLOW_ROOT / "release-independent-crate.yml")
        for path in (
            WORKFLOW_ROOT / "release-preflight.yml",
            WORKFLOW_ROOT / "release-crates.yml",
        ):
            text = read(path)
            with self.subTest(workflow=path.name):
                self.assertIn("scripts/release_registry_dependents.py", text)
                self.assertIn("--candidate-path crates/roughr", text)
                self.assertIn("--dependent merman-render=0.7.0", text)
                self.assertIn("--dependent merman-render=0.8.0-alpha.5", text)
        self.assertIn("preflight-independent", independent)
        self.assertIn("Verify published dependent lanes", independent)

    def test_node_publish_supports_verified_cross_run_recovery(self) -> None:
        text = read(WORKFLOW_ROOT / "release-node.yml")
        publish = workflow_job(text, "publish")
        self.assertIn("recovery_run_id:", text)
        self.assertIn("DISPATCH_RECOVERY_RUN_ID", text)
        self.assertIn("recovery_run_id requires publish_to_npm=true", text)
        self.assertIn("actions: read", publish)
        self.assertIn("always()", publish)
        self.assertIn(
            "needs.package-group.result == 'success' || needs.validate-inputs.outputs.recovery_run_id != ''",
            publish,
        )
        self.assertIn("github-token: ${{ github.token }}", publish)
        self.assertIn(
            "run-id: ${{ needs.validate-inputs.outputs.recovery_run_id != '' && needs.validate-inputs.outputs.recovery_run_id || github.run_id }}",
            publish,
        )

    def test_performance_pull_requests_are_read_only_and_summary_only(self) -> None:
        text = read(WORKFLOW_ROOT / "performance.yml")
        self.assertNotIn("pull-requests: write", text)
        self.assertNotIn("issues: write", text)
        self.assertNotIn("github-script", text)
        self.assertIn("GITHUB_STEP_SUMMARY", text)
        self.assertIn("Upload measurement receipt", text)


if __name__ == "__main__":
    unittest.main()
