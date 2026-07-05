#!/usr/bin/env python3
"""Security contract tests for manual release workflows."""

from __future__ import annotations

import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
WORKFLOW_ROOT = ROOT / ".github" / "workflows"
RELEASE_WORKFLOWS = sorted(WORKFLOW_ROOT.glob("release-*.yml"))


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


class ReleaseWorkflowSecurityTests(unittest.TestCase):
    def test_release_run_blocks_do_not_interpolate_dispatch_inputs(self) -> None:
        for path in RELEASE_WORKFLOWS:
            text = read_workflow(path)
            for index, block in enumerate(run_blocks(text)):
                with self.subTest(workflow=path.name, run_block=index):
                    self.assertNotIn("${{ inputs.", block)

    def test_source_ref_checkouts_use_validated_output(self) -> None:
        for path in RELEASE_WORKFLOWS:
            text = read_workflow(path)
            if "source_ref:" not in text:
                continue

            checkout_count = text.count("uses: actions/checkout")
            validated_ref_count = text.count("ref: ${{ needs.validate-inputs.outputs.source_ref }}")
            with self.subTest(workflow=path.name):
                self.assertEqual(validated_ref_count, checkout_count)
                self.assertNotIn("ref: ${{ inputs.source_ref }}", text)
                self.assertNotIn("inputs.source_ref ||", text)

    def test_validation_jobs_precede_release_checkouts(self) -> None:
        for path in RELEASE_WORKFLOWS:
            text = read_workflow(path)
            if "source_ref:" not in text:
                continue

            with self.subTest(workflow=path.name):
                self.assertIn("validate-inputs:", text)
                self.assertLess(text.index("validate-inputs:"), text.index("uses: actions/checkout"))

    def test_validation_jobs_expose_safe_output_names(self) -> None:
        for path in RELEASE_WORKFLOWS:
            text = read_workflow(path)
            if "source_ref:" not in text:
                continue

            validate_job = indented_block(text, "validate-inputs:")
            with self.subTest(workflow=path.name):
                self.assertIn("GITHUB_OUTPUT", validate_job)
                self.assertRegex(validate_job, re.compile(r"""(printf 'source_ref=%s\\n'|echo "source_ref=)"""))
                self.assertRegex(validate_job, re.compile(r"""(printf 'version=%s\\n'|echo "version=)"""))
                if "release_tag:" in text:
                    self.assertRegex(validate_job, re.compile(r"""(printf 'release_tag=%s\\n'|echo "release_tag=)"""))

    def test_validation_jobs_reject_untrusted_ref_and_version_shapes(self) -> None:
        for path in RELEASE_WORKFLOWS:
            text = read_workflow(path)
            if "source_ref:" not in text:
                continue

            validate_job = indented_block(text, "validate-inputs:")
            with self.subTest(workflow=path.name):
                self.assertIn("semver_re='^[0-9]+\\.[0-9]+\\.[0-9]+", validate_job)
                self.assertIn("sha_re='^[0-9a-fA-F]{40}$'", validate_job)
                self.assertIn('[[ "$SOURCE_REF" != *$\'\\n\'*', validate_job)
                self.assertIn("source_ref tag must match", validate_job)
                self.assertIn("refs/tags/<release-tag>", validate_job)

    def test_validation_jobs_do_not_hold_publish_permissions(self) -> None:
        for path in RELEASE_WORKFLOWS:
            text = read_workflow(path)
            if "source_ref:" not in text:
                continue

            validate_job = indented_block(text, "validate-inputs:")
            with self.subTest(workflow=path.name):
                self.assertNotIn("contents: write", validate_job)
                self.assertNotIn("id-token: write", validate_job)


if __name__ == "__main__":
    unittest.main()
