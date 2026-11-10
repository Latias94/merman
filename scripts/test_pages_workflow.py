#!/usr/bin/env python3
"""Regression contracts for the Web artifact and Playground CI consumers."""

from __future__ import annotations

import json
from pathlib import Path
import unittest

try:
    from scripts.test_release_workflow_security import workflow_job
except ModuleNotFoundError:
    from test_release_workflow_security import workflow_job


ROOT = Path(__file__).resolve().parents[1]
PAGES = ROOT / ".github" / "workflows" / "pages.yml"


class PagesWorkflowTests(unittest.TestCase):
    def test_web_and_size_tool_builds_can_start_in_parallel(self) -> None:
        text = PAGES.read_text(encoding="utf-8")
        web = workflow_job(text, "web-package")
        tools = workflow_job(text, "size-tool")
        for job in (web, tools):
            self.assertNotIn("needs:", job)
        self.assertIn("shared-key: pages-web-wasm", web)
        self.assertIn("shared-key: pages-web-size-tool", tools)
        self.assertIn("CARGO_INCREMENTAL: '0'", tools)
        self.assertIn("CARGO_PROFILE_RELEASE_DEBUG: '0'", tools)
        self.assertIn("cargo build --locked --release -p xtask", tools)
        self.assertIn("path: target/release/xtask", tools)
        self.assertNotIn("path: target/\n", tools)

    def test_browser_and_size_report_consume_the_single_web_build(self) -> None:
        text = PAGES.read_text(encoding="utf-8")
        web = workflow_job(text, "web-package")
        browser = workflow_job(text, "build")
        report = workflow_job(text, "size-report")
        self.assertEqual(text.count("npm run build --prefix platforms/web"), 1)
        self.assertIn("npm run smoke --prefix platforms/web", web)
        self.assertLess(web.index("npm run smoke"), web.index("actions/upload-artifact@"))
        self.assertIn("platforms/web/packages", web)
        self.assertIn("platforms/web/pkg", web)
        self.assertIn("platforms/web/dist", web)
        self.assertIn("if-no-files-found: error", web)
        self.assertIn("needs: web-package", browser)
        self.assertNotIn("size-report", browser)
        self.assertIn("needs: [web-package, size-tool]", report)
        for job in (browser, report):
            self.assertIn("uses: actions/download-artifact@v8.0.2", job)
            self.assertIn("artifact-ids: ${{ needs.web-package.outputs.artifact_id }}", job)
            self.assertIn("path: platforms/web", job)
            self.assertNotIn("build:wasm", job)
        self.assertIn("chmod +x target/release/xtask", report)
        self.assertIn("target/release/xtask wasm-size-matrix", report)
        self.assertIn("--surface web", report)
        self.assertIn("--web-package-root platforms/web/packages", report)
        self.assertNotIn("--budget-file", report)
        self.assertIn("tool: wasm-tools@1.263.0", report)

    def test_web_ci_and_release_report_sizes_without_byte_ceilings(self) -> None:
        for workflow in ("pages.yml", "release-web.yml", "release-preflight.yml"):
            with self.subTest(workflow=workflow):
                text = (PAGES.parent / workflow).read_text(encoding="utf-8")
                report = text.split("      - name: Report final Web package WASM sizes\n", 1)[1]
                report = report.split("\n      - name:", 1)[0]
                self.assertIn("wasm-size-matrix", report)
                self.assertIn("--web-package-root platforms/web/packages", report)
                self.assertNotIn("--budget-file", report)
        budgets = json.loads((ROOT / "docs/release/WASM_SIZE_BUDGETS.json").read_text(encoding="utf-8"))
        self.assertEqual(set(budgets["artifact_profiles"]), {"typst-wasm"})

    def test_browser_consumer_installs_the_web_checker_dependencies(self) -> None:
        text = PAGES.read_text(encoding="utf-8")
        browser = workflow_job(text, "build")
        self.assertIn("platforms/web/package-lock.json", browser)
        self.assertIn("npm ci --prefix platforms/web", browser)
        self.assertLess(
            browser.index("npm ci --prefix platforms/web"),
            browser.index("npm run test:browser:chromium:desktop:built"),
        )

    def test_partial_reruns_download_the_successful_producers_artifact_ids(self) -> None:
        text = PAGES.read_text(encoding="utf-8")
        for producer in ("web-package", "size-tool"):
            job = workflow_job(text, producer)
            self.assertIn("artifact_id: ${{ steps.upload.outputs.artifact-id }}", job)
            self.assertIn("id: upload", job)
            self.assertIn("${{ github.run_attempt }}", job)
        for consumer in ("size-report", "build"):
            job = workflow_job(text, consumer)
            self.assertIn("artifact-ids: ${{ needs.web-package.outputs.artifact_id }}", job)
            self.assertNotIn("name: pages-web-package-${{ github.run_attempt }}", job)
        self.assertIn("artifact-ids: ${{ needs.size-tool.outputs.artifact_id }}", workflow_job(text, "size-report"))

    def test_browser_checks_and_deployment_wait_for_owner_success(self) -> None:
        text = PAGES.read_text(encoding="utf-8")
        browser = workflow_job(text, "build")
        for command in (
            "npm run prepare:browser-runtime --prefix playground",
            "npm run test:prepared --prefix playground",
            "npm run lint --prefix playground",
            "npm run build:prepared --prefix playground",
            "npm run test:browser:smoke:non-chromium:built --prefix playground",
            "npm run test:browser:chromium:desktop:built --prefix playground",
        ):
            self.assertIn(command, browser)
        self.assertIn("targets: wasm32-unknown-unknown", browser)
        self.assertIn("cargo install wasm-pack --version 0.15.1 --locked", browser)
        self.assertIn("if: failure()", browser)
        self.assertIn("playground/playwright-report", browser)
        self.assertIn("playground/test-results", browser)
        self.assertIn("uses: actions/upload-pages-artifact@v5.0.0", browser)
        self.assertNotIn("continue-on-error:", text)
        tools = workflow_job(text, "size-tool")
        for command in (
            "verify-mermaid-reference",
            "verify-capability-surface",
            "verify-editor-language-contract",
            "verify-text-measurement-protocol",
            "verify-web-diagram-catalog",
            "verify-resource-contract",
            "verify-playground-example-catalog",
        ):
            self.assertIn(f"target/release/xtask {command}", tools)
        self.assertIn('node-version: "26.13.0"', tools)
        self.assertIn("if: github.event_name != 'pull_request' && inputs.ci_only != true", tools)
        deployment = (PAGES.parent / "pages-deploy.yml").read_text(encoding="utf-8")
        self.assertIn("uses: ./.github/workflows/pages.yml", workflow_job(deployment, "build"))
        self.assertIn("needs: build", workflow_job(deployment, "deploy"))


if __name__ == "__main__":
    unittest.main()
