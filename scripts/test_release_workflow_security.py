#!/usr/bin/env python3
"""Security contract tests for manual release workflows."""

from __future__ import annotations

import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
WORKFLOW_ROOT = ROOT / ".github" / "workflows"
RELEASE_WORKFLOWS = sorted(WORKFLOW_ROOT.glob("release-*.yml"))
SOURCE_REF_WORKFLOWS = sorted(
    path
    for path in WORKFLOW_ROOT.glob("*.yml")
    if "source_ref:" in path.read_text(encoding="utf-8")
)


def read_workflow(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def indented_block(text: str, marker: str) -> str:
    lines = text.splitlines()
    for index, line in enumerate(lines):
        if line.strip() != marker:
            continue
        marker_indent = len(line) - len(line.lstrip(" "))
        block: list[str] = []
        for child in lines[index + 1 :]:
            if child.strip() == "":
                block.append(child)
                continue
            child_indent = len(child) - len(child.lstrip(" "))
            if child_indent <= marker_indent:
                break
            block.append(child)
        return "\n".join(block)
    raise AssertionError(f"could not find {marker!r}")


def run_blocks(text: str) -> list[str]:
    lines = text.splitlines()
    blocks: list[str] = []
    for index, line in enumerate(lines):
        stripped = line.strip()
        if not stripped.startswith("run:"):
            continue

        indent = len(line) - len(line.lstrip(" "))
        inline = stripped.removeprefix("run:").strip()
        if inline not in {"|", ">"}:
            blocks.append(inline)
            continue

        block: list[str] = []
        for child in lines[index + 1 :]:
            if child.strip() == "":
                block.append(child)
                continue
            child_indent = len(child) - len(child.lstrip(" "))
            if child_indent <= indent:
                break
            block.append(child)
        blocks.append("\n".join(block))
    return blocks


def checkout_blocks(text: str) -> list[str]:
    lines = text.splitlines()
    blocks: list[str] = []
    for index, line in enumerate(lines):
        if "uses: actions/checkout" not in line.strip():
            continue

        indent = len(line) - len(line.lstrip(" "))
        block = [line]
        for child in lines[index + 1 :]:
            if child.strip() == "":
                block.append(child)
                continue
            child_indent = len(child) - len(child.lstrip(" "))
            if child_indent <= indent:
                break
            block.append(child)
        blocks.append("\n".join(block))
    return blocks


class ReleaseWorkflowSecurityTests(unittest.TestCase):
    def test_release_run_blocks_do_not_interpolate_dispatch_inputs(self) -> None:
        for path in RELEASE_WORKFLOWS:
            text = read_workflow(path)
            for index, block in enumerate(run_blocks(text)):
                with self.subTest(workflow=path.name, run_block=index):
                    self.assertNotIn("${{ inputs.", block)

    def test_source_ref_checkouts_use_validated_output(self) -> None:
        for path in SOURCE_REF_WORKFLOWS:
            text = read_workflow(path)

            checkout_count = text.count("uses: actions/checkout")
            validated_ref_count = text.count("ref: ${{ needs.validate-inputs.outputs.source_ref }}")
            with self.subTest(workflow=path.name):
                self.assertEqual(validated_ref_count, checkout_count)
                self.assertNotIn("ref: ${{ inputs.source_ref }}", text)
                self.assertNotIn("inputs.source_ref ||", text)

    def test_source_ref_checkouts_do_not_persist_credentials(self) -> None:
        for path in SOURCE_REF_WORKFLOWS:
            text = read_workflow(path)

            blocks = checkout_blocks(text)
            with self.subTest(workflow=path.name, checkout_count=len(blocks)):
                self.assertGreater(len(blocks), 0)

            for index, block in enumerate(blocks):
                with self.subTest(workflow=path.name, checkout=index):
                    self.assertIn("persist-credentials: false", block)

    def test_validation_jobs_precede_release_checkouts(self) -> None:
        for path in SOURCE_REF_WORKFLOWS:
            text = read_workflow(path)

            with self.subTest(workflow=path.name):
                self.assertIn("validate-inputs:", text)
                self.assertLess(text.index("validate-inputs:"), text.index("uses: actions/checkout"))

    def test_validation_jobs_expose_safe_source_ref_output(self) -> None:
        for path in SOURCE_REF_WORKFLOWS:
            text = read_workflow(path)

            validate_job = indented_block(text, "validate-inputs:")
            with self.subTest(workflow=path.name):
                self.assertIn("GITHUB_OUTPUT", validate_job)
                self.assertRegex(validate_job, re.compile(r"""(printf 'source_ref=%s\\n'|echo "source_ref=)"""))

    def test_release_validation_jobs_expose_safe_release_output_names(self) -> None:
        for path in RELEASE_WORKFLOWS:
            text = read_workflow(path)
            if "source_ref:" not in text:
                continue

            validate_job = indented_block(text, "validate-inputs:")
            with self.subTest(workflow=path.name):
                self.assertRegex(validate_job, re.compile(r"""(printf 'version=%s\\n'|echo "version=)"""))
                if "release_tag:" in text:
                    self.assertRegex(validate_job, re.compile(r"""(printf 'release_tag=%s\\n'|echo "release_tag=)"""))
                if path.name == "release-web.yml":
                    self.assertRegex(validate_job, re.compile(r"""(printf 'npm_dist_tag=%s\\n'|echo "npm_dist_tag=)"""))

    def test_validation_jobs_reject_untrusted_source_ref_shapes(self) -> None:
        for path in SOURCE_REF_WORKFLOWS:
            text = read_workflow(path)

            validate_job = indented_block(text, "validate-inputs:")
            with self.subTest(workflow=path.name):
                self.assertIn("sha_re='^[0-9a-fA-F]{40}$'", validate_job)
                self.assertIn('[[ "$SOURCE_REF" != *$\'\\n\'*', validate_job)
                self.assertIn("source_ref must be", validate_job)

    def test_release_validation_jobs_reject_untrusted_ref_and_version_shapes(self) -> None:
        for path in RELEASE_WORKFLOWS:
            text = read_workflow(path)
            if "source_ref:" not in text:
                continue

            validate_job = indented_block(text, "validate-inputs:")
            with self.subTest(workflow=path.name):
                self.assertIn("semver_re='^[0-9]+\\.[0-9]+\\.[0-9]+", validate_job)
                self.assertIn("source_ref tag must match", validate_job)
                self.assertIn("refs/tags/<release-tag>", validate_job)

    def test_validation_jobs_do_not_hold_publish_permissions(self) -> None:
        for path in SOURCE_REF_WORKFLOWS:
            text = read_workflow(path)

            validate_job = indented_block(text, "validate-inputs:")
            with self.subTest(workflow=path.name):
                self.assertNotIn("contents: write", validate_job)
                self.assertNotIn("id-token: write", validate_job)

    def test_platform_release_build_jobs_do_not_hold_release_write_permission(self) -> None:
        for path in [
            WORKFLOW_ROOT / "release-android.yml",
            WORKFLOW_ROOT / "release-apple.yml",
        ]:
            text = read_workflow(path)
            build_job = indented_block(text, "build:")
            upload_job = indented_block(text, "upload-release:")

            with self.subTest(workflow=path.name):
                self.assertIn("contents: read", build_job)
                self.assertNotIn("contents: write", build_job)
                self.assertNotIn("environment: github-release", build_job)
                self.assertIn("environment: github-release", upload_job)
                self.assertIn("contents: write", upload_job)
                self.assertIn("gh release upload", upload_job)

    def test_crates_token_is_only_used_for_no_verify_upload(self) -> None:
        text = read_workflow(WORKFLOW_ROOT / "release-crates.yml")
        publish_step = indented_block(text, "- name: Publish crates in dependency order")

        self.assertNotIn("- name: Preflight crates in dependency order", text)
        self.assertIn("--dry-run", publish_step)
        self.assertIn("--no-verify", publish_step)
        self.assertIn('--token "${{ secrets.CARGO_REGISTRY_TOKEN }}"', publish_step)
        self.assertNotIn("CARGO_REGISTRY_TOKEN:", publish_step)
        self.assertLess(publish_step.index("--dry-run"), publish_step.index("--no-verify"))

    def test_trusted_npm_publish_job_only_downloads_artifact_and_publishes(self) -> None:
        text = read_workflow(WORKFLOW_ROOT / "release-web.yml")
        publish_job = indented_block(text, "publish:")

        self.assertIn("id-token: write", publish_job)
        self.assertIn("actions/download-artifact", publish_job)
        self.assertIn('npm publish "$package_file"', publish_job)
        self.assertIn("NPM_DIST_TAG: ${{ needs.validate-inputs.outputs.npm_dist_tag }}", publish_job)
        self.assertIn('--tag "$NPM_DIST_TAG"', publish_job)
        for forbidden in [
            "actions/checkout",
            "platforms/web/scripts",
            "npm ci",
            "npm run",
            "cargo install",
            "wasm-pack",
        ]:
            with self.subTest(forbidden=forbidden):
                self.assertNotIn(forbidden, publish_job)

    def test_trusted_pubdev_publish_job_only_downloads_artifact_and_publishes(self) -> None:
        text = read_workflow(WORKFLOW_ROOT / "release-flutter.yml")
        publish_job = indented_block(text, "publish:")

        self.assertIn("id-token: write", publish_job)
        self.assertIn("actions/download-artifact", publish_job)
        self.assertIn("dart pub publish --force --skip-validation", publish_job)
        for forbidden in [
            "actions/checkout",
            "flutter pub get",
            "flutter analyze",
            "dart format",
            "dart pub publish --dry-run",
            "cargo install",
            "build-android.py",
            "build-ios.sh",
            "build-desktop.sh",
            "subosito/flutter-action",
        ]:
            with self.subTest(forbidden=forbidden):
                self.assertNotIn(forbidden, publish_job)

    def test_release_preflight_uses_crates_io_publish_helper(self) -> None:
        text = read_workflow(WORKFLOW_ROOT / "release-preflight.yml")

        self.assertIn("tools/publish.py --list-crates-io-packages", text)
        self.assertNotIn('package.get("publish") != []', text)


class CiWorkflowSecurityTests(unittest.TestCase):
    def test_ci_workflow_declares_read_only_contents_permission(self) -> None:
        text = read_workflow(WORKFLOW_ROOT / "ci.yml")
        header = text.split("\njobs:", 1)[0]

        self.assertIn("permissions:\n  contents: read", header)

    def test_ci_checkouts_do_not_persist_credentials(self) -> None:
        text = read_workflow(WORKFLOW_ROOT / "ci.yml")
        blocks = checkout_blocks(text)

        self.assertGreater(len(blocks), 0)
        for index, block in enumerate(blocks):
            with self.subTest(checkout=index):
                self.assertIn("persist-credentials: false", block)


class PerformanceWorkflowSecurityTests(unittest.TestCase):
    def test_comment_jobs_do_not_request_pull_request_write_permission(self) -> None:
        text = read_workflow(WORKFLOW_ROOT / "performance.yml")
        for job_name in ["regression:", "frontmatter:"]:
            job = indented_block(text, job_name)
            with self.subTest(job=job_name.removesuffix(":")):
                self.assertIn("issues: write", job)
                self.assertNotIn("pull-requests: write", job)

    def test_performance_checkouts_do_not_persist_credentials(self) -> None:
        text = read_workflow(WORKFLOW_ROOT / "performance.yml")
        checkout_count = text.count("uses: actions/checkout")
        persisted_false_count = text.count("persist-credentials: false")

        self.assertEqual(persisted_false_count, checkout_count)


if __name__ == "__main__":
    unittest.main()
