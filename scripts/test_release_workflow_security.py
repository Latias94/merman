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

    def test_tree_sitter_mermaid_release_is_dry_run_only(self) -> None:
        workflow = read(WORKFLOW_ROOT / "release-independent-crate.yml")

        self.assertIn("- tree-sitter-mermaid", workflow)
        self.assertIn("publish_admitted=false", workflow)
        self.assertIn(
            "if: ${{ needs.validate-inputs.outputs.publish_admitted == 'true' }}",
            workflow,
        )
        self.assertIn(
            "npm pack ./distribution/tree-sitter-mermaid --dry-run --json",
            workflow,
        )
        self.assertIn('record.get("name") != "tree-sitter-mermaid"', workflow)
        self.assertIn('record.get("version") != expected_version', workflow)
        self.assertIn('"THIRD_PARTY_NOTICES.md"', workflow)
        self.assertIn("npm package omits legal files", workflow)

    def test_tree_sitter_owner_runs_its_complete_package_gate(self) -> None:
        ci = read(CI_WORKFLOW)
        workflow = read(WORKFLOW_ROOT / "tree-sitter-mermaid.yml")

        self.assertIn("uses: ./.github/workflows/tree-sitter-mermaid.yml", ci)
        self.assertIn("cargo fmt --all -- --check", workflow)
        self.assertIn("cargo clippy --locked -p tree-sitter-mermaid -p xtask", workflow)
        self.assertIn("cargo nextest run --locked -p tree-sitter-mermaid", workflow)
        self.assertIn(
            "cargo nextest run --locked -p xtask tree_sitter_mermaid",
            workflow,
        )
        self.assertIn("npm test --prefix distribution/tree-sitter-mermaid", workflow)
        self.assertIn("cargo package --locked -p tree-sitter-mermaid", workflow)
        self.assertIn(
            "npm pack ./distribution/tree-sitter-mermaid --dry-run --json",
            workflow,
        )

    def test_workspace_release_ignores_flutter_package_tags(self) -> None:
        text = read(WORKFLOW_ROOT / "release.yml")
        self.assertIn("      - '!flutter-v*'\n", text)

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

    def test_npm_publish_provenance_cannot_be_disabled_by_repository_config(self) -> None:
        paths = [
            ROOT / ".npmrc",
            ROOT / "platforms" / "web" / ".npmrc",
            ROOT / "platforms" / "node" / ".npmrc",
            WORKFLOW_ROOT / "release-web.yml",
            WORKFLOW_ROOT / "release-node.yml",
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

    def test_performance_pull_requests_are_read_only_and_summary_only(self) -> None:
        text = read(WORKFLOW_ROOT / "performance.yml")
        self.assertNotIn("pull-requests: write", text)
        self.assertNotIn("issues: write", text)
        self.assertNotIn("github-script", text)
        self.assertIn("GITHUB_STEP_SUMMARY", text)
        self.assertIn("Upload measurement receipt", text)


if __name__ == "__main__":
    unittest.main()
