#!/usr/bin/env python3
"""Security contract tests for manual release workflows."""

from __future__ import annotations

import json
import re
import shlex
import subprocess
import tempfile
import textwrap
import unittest
import uuid
from pathlib import Path

try:
    from scripts.github_workflow_contract import (
        WorkflowContractError,
        load_workflow_contract as parse_workflow_structure,
        workflow_job,
        workflow_step,
    )
except ModuleNotFoundError:
    from github_workflow_contract import (
        WorkflowContractError,
        load_workflow_contract as parse_workflow_structure,
        workflow_job,
        workflow_step,
    )


ROOT = Path(__file__).resolve().parents[1]
WORKFLOW_ROOT = ROOT / ".github" / "workflows"
WEB_PACKAGE_JSON = ROOT / "platforms" / "web" / "package.json"
NPM_CONFIG_PATHS = [
    ROOT / ".npmrc",
    ROOT / "platforms" / "web" / ".npmrc",
]
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


def validation_script(path: Path) -> str:
    text = read_workflow(path)
    for block in run_blocks(text):
        if "DISPATCH_SOURCE_REF" in block and "GITHUB_OUTPUT" in block:
            return textwrap.dedent(block)
    raise AssertionError(f"could not find validation script in {path.name}")


def run_workflow_validation(
    path: Path,
    *,
    release_tag: str = "v1.2.3",
    source_ref: str = "main",
    version: str | None = None,
) -> tuple[subprocess.CompletedProcess[str], dict[str, str]]:
    if version is None:
        version = release_tag.removeprefix("v")
    output_dir = ROOT / "target" / "release-workflow-tests"
    output_dir.mkdir(parents=True, exist_ok=True)
    run_id = uuid.uuid4().hex
    output_path = output_dir / f"github-output-{run_id}.txt"
    script_path = output_dir / f"{path.stem}-validation-{run_id}.sh"
    script = "\n".join(
        [
            "EVENT_NAME=workflow_dispatch",
            f"DISPATCH_RELEASE_TAG={shlex.quote(release_tag)}",
            f"DISPATCH_VERSION={shlex.quote(version)}",
            f"DISPATCH_SOURCE_REF={shlex.quote(source_ref)}",
            f"GIT_REF={shlex.quote(f'refs/tags/{release_tag}')}",
            f"GIT_REF_NAME={shlex.quote(release_tag)}",
            f"GIT_SHA={shlex.quote('0123456789abcdef0123456789abcdef01234567')}",
            f"GITHUB_OUTPUT={shlex.quote(output_path.relative_to(ROOT).as_posix())}",
            validation_script(path),
        ]
    )
    script_path.write_text(script, encoding="utf-8", newline="\n")
    try:
        result = subprocess.run(
            ["bash", script_path.relative_to(ROOT).as_posix()],
            cwd=ROOT,
            text=True,
            capture_output=True,
            timeout=10,
            check=False,
        )
        outputs = parse_github_output(output_path.read_text(encoding="utf-8")) if output_path.exists() else {}
        return result, outputs
    finally:
        script_path.unlink(missing_ok=True)
        output_path.unlink(missing_ok=True)


def parse_github_output(text: str) -> dict[str, str]:
    outputs: dict[str, str] = {}
    for line in text.splitlines():
        if not line or "=" not in line:
            continue
        name, value = line.split("=", 1)
        outputs[name] = value
    return outputs


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


def npm_publish_provenance_disabled_patterns() -> list[re.Pattern[str]]:
    return [
        re.compile(r"(?:^|\s)--(?:no-)?provenance\s*=\s*false(?:\s|$)", re.IGNORECASE),
        re.compile(r"(?:^|\s)--no-provenance(?:\s|$)", re.IGNORECASE),
        re.compile(r"(?:^|\s)provenance\s*=\s*false(?:\s|$)", re.IGNORECASE),
        re.compile(r"(?:^|\s)NPM_CONFIG_PROVENANCE\s*[:=]\s*[\"']?false[\"']?(?:\s|$)", re.IGNORECASE),
        re.compile(r'"provenance"\s*:\s*false', re.IGNORECASE),
    ]


def assert_no_npm_provenance_disable(test_case: unittest.TestCase, text: str) -> None:
    for pattern in npm_publish_provenance_disabled_patterns():
        with test_case.subTest(pattern=pattern.pattern):
            test_case.assertIsNone(pattern.search(text))


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
            pinned_ref_count = text.count("ref: ${{ needs.preflight.outputs.source_sha }}")
            with self.subTest(workflow=path.name):
                self.assertEqual(validated_ref_count + pinned_ref_count, checkout_count)
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

    def test_release_web_validation_computes_npm_dist_tags(self) -> None:
        workflow = WORKFLOW_ROOT / "release-web.yml"
        cases = [
            ("v1.2.3", "latest"),
            ("v1.2.3-alpha.1", "alpha"),
            ("v1.2.3-beta.1", "beta"),
            ("v1.2.3-rc.1", "rc"),
        ]

        for release_tag, expected_dist_tag in cases:
            with self.subTest(release_tag=release_tag):
                result, outputs = run_workflow_validation(
                    workflow,
                    release_tag=release_tag,
                    source_ref=release_tag,
                )

                self.assertEqual(
                    result.returncode,
                    0,
                    msg=f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}",
                )
                self.assertEqual(outputs["release_tag"], release_tag)
                self.assertEqual(outputs["source_ref"], f"refs/tags/{release_tag}")
                self.assertEqual(outputs["version"], release_tag.removeprefix("v"))
                self.assertEqual(outputs["npm_dist_tag"], expected_dist_tag)

    def test_release_web_validation_rejects_unsupported_prerelease_shapes(self) -> None:
        workflow = WORKFLOW_ROOT / "release-web.yml"
        cases = [
            "v1.2.3-",
            "v1.2.3-alpha",
            "v1.2.3-alpha.1.2",
            "v1.2.3-dev.1",
        ]

        for release_tag in cases:
            with self.subTest(release_tag=release_tag):
                result, outputs = run_workflow_validation(
                    workflow,
                    release_tag=release_tag,
                    source_ref=release_tag,
                )

                self.assertNotEqual(
                    result.returncode,
                    0,
                    msg=f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}",
                )
                self.assertNotIn("npm_dist_tag", outputs)

    def test_validation_scripts_accept_protected_branch_source_ref(self) -> None:
        for path in SOURCE_REF_WORKFLOWS:
            with self.subTest(workflow=path.name):
                result, outputs = run_workflow_validation(path, source_ref="main")

                self.assertEqual(
                    result.returncode,
                    0,
                    msg=f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}",
                )
                self.assertEqual(outputs["source_ref"], "main")

    def test_validation_scripts_canonicalize_tag_source_refs(self) -> None:
        cases = ["v1.2.3", "refs/tags/v1.2.3"]
        for path in SOURCE_REF_WORKFLOWS:
            for source_ref in cases:
                with self.subTest(workflow=path.name, source_ref=source_ref):
                    result, outputs = run_workflow_validation(
                        path,
                        release_tag="v1.2.3",
                        version="1.2.3",
                        source_ref=source_ref,
                    )

                    self.assertEqual(
                        result.returncode,
                        0,
                        msg=f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}",
                    )
                    self.assertEqual(outputs["source_ref"], "refs/tags/v1.2.3")

    def test_validation_scripts_accept_semver_build_metadata(self) -> None:
        release_tag = "v1.2.3-rc.4+build.7"
        for path in SOURCE_REF_WORKFLOWS:
            with self.subTest(workflow=path.name):
                result, outputs = run_workflow_validation(
                    path,
                    release_tag=release_tag,
                    version=release_tag.removeprefix("v"),
                    source_ref=release_tag,
                )
                self.assertEqual(
                    result.returncode,
                    0,
                    msg=f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}",
                )
                self.assertEqual(outputs["source_ref"], f"refs/tags/{release_tag}")

    def test_validation_scripts_reject_multiline_source_ref_values(self) -> None:
        for path in SOURCE_REF_WORKFLOWS:
            with self.subTest(workflow=path.name):
                result, outputs = run_workflow_validation(path, source_ref="main\nrefs/heads/main")

                self.assertNotEqual(
                    result.returncode,
                    0,
                    msg=f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}",
                )
                self.assertNotIn("source_ref", outputs)

    def test_validation_scripts_reject_full_sha_source_ref_values(self) -> None:
        full_sha = "0123456789abcdef0123456789abcdef01234567"
        for path in SOURCE_REF_WORKFLOWS:
            with self.subTest(workflow=path.name):
                result, outputs = run_workflow_validation(path, source_ref=full_sha)

                self.assertNotEqual(
                    result.returncode,
                    0,
                    msg=f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}",
                )
                self.assertNotIn("source_ref", outputs)

    def test_release_validation_scripts_reject_mismatched_source_tags(self) -> None:
        for path in RELEASE_WORKFLOWS:
            text = read_workflow(path)
            if "source_ref:" not in text:
                continue

            with self.subTest(workflow=path.name):
                result, outputs = run_workflow_validation(
                    path,
                    release_tag="v1.2.3",
                    version="1.2.3",
                    source_ref="v9.9.9",
                )

                self.assertNotEqual(
                    result.returncode,
                    0,
                    msg=f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}",
                )
                self.assertNotIn("source_ref", outputs)

    def test_validation_jobs_reject_untrusted_source_ref_shapes(self) -> None:
        for path in SOURCE_REF_WORKFLOWS:
            text = read_workflow(path)

            validate_job = indented_block(text, "validate-inputs:")
            with self.subTest(workflow=path.name):
                self.assertIn('[[ "$SOURCE_REF" != *$\'\\n\'*', validate_job)
                self.assertIn("source_ref must be", validate_job)
                self.assertNotIn("sha_re=", validate_job)
                self.assertNotIn("is_sha_ref", validate_job)
                self.assertNotIn("40-character SHA", text)

    def test_release_validation_jobs_reject_untrusted_ref_and_version_shapes(self) -> None:
        for path in RELEASE_WORKFLOWS:
            text = read_workflow(path)
            if "source_ref:" not in text:
                continue

            validate_job = indented_block(text, "validate-inputs:")
            with self.subTest(workflow=path.name):
                self.assertTrue(
                    ("semver_re=" in validate_job and "0|[1-9]" in validate_job)
                    or ("is_uint()" in validate_job and "is_release_version()" in validate_job)
                )
                self.assertIn("source_ref tag must match", validate_job)
                self.assertIn("refs/tags/<release-tag>", validate_job)

    def test_crates_publish_step_defines_its_version_regex_locally(self) -> None:
        workflow = parse_workflow_structure(WORKFLOW_ROOT / "release-crates.yml")
        publish = workflow_job(workflow, "publish")
        upload = workflow_step(publish, name="Upload crates to crates.io")
        run = upload["run"]
        self.assertIn("semver_re=", run)
        self.assertLess(run.index("semver_re="), run.index('[[ ! "$version" =~ $semver_re ]]'))

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
                self.assertIn("::error::GitHub Release", upload_job)
                self.assertIn("exit 1", upload_job)
                self.assertNotIn("::warning::GitHub Release", upload_job)

    def test_crates_token_upload_step_is_isolated_from_preflight(self) -> None:
        text = read_workflow(WORKFLOW_ROOT / "release-crates.yml")
        preflight_job = indented_block(text, "preflight:")
        publish_job = indented_block(text, "publish:")
        preflight_step = indented_block(text, "- name: Preflight crates in dependency order")
        upload_step = indented_block(text, "- name: Upload crates to crates.io")
        upload_run = upload_step.split("run: |", 1)[1]

        self.assertNotIn("--dry-run", preflight_step)
        self.assertNotIn("CARGO_REGISTRY_TOKEN", preflight_job)
        self.assertNotIn("secrets.", preflight_job)
        self.assertNotIn("environment: crates.io", preflight_job)
        self.assertIn("source_sha: ${{ steps.source.outputs.source_sha }}", preflight_job)
        self.assertIn('source_sha="$(git rev-parse HEAD)"', preflight_job)
        self.assertIn("needs: [validate-inputs, preflight]", publish_job)
        self.assertIn("ref: ${{ needs.preflight.outputs.source_sha }}", publish_job)
        self.assertNotIn("ref: ${{ needs.validate-inputs.outputs.source_ref }}", publish_job)
        self.assertIn("Verify pinned source commit", publish_job)
        self.assertIn(
            "CARGO_REGISTRY_TOKEN: ${{ secrets.CARGO_REGISTRY_TOKEN }}",
            upload_step,
        )
        self.assertIn(
            'env -u CARGO_REGISTRY_TOKEN cargo publish -p "$crate" --locked --dry-run --registry crates-io',
            upload_run,
        )
        self.assertIn('--token "$CARGO_REGISTRY_TOKEN"', upload_run)
        self.assertNotIn("secrets.CARGO_REGISTRY_TOKEN", upload_run)
        self.assertNotIn("${{ secrets.", upload_run)
        self.assertIn('verify_workspace_crate_version "$crate" "$crate_version"', upload_run)
        self.assertIn('actual_version="$(workspace_crate_version "$crate")"', upload_run)
        self.assertLess(
            upload_run.index(
                'env -u CARGO_REGISTRY_TOKEN cargo publish -p "$crate" --locked --dry-run --registry crates-io'
            ),
            upload_run.index(
                'cargo publish -p "$crate" --locked --no-verify --registry crates-io --token "$CARGO_REGISTRY_TOKEN"'
            ),
        )
        self.assertGreaterEqual(
            upload_run.count('wait_for_crate_version "$crate" "$crate_version"'),
            2,
        )

    def test_trusted_pypi_publish_job_only_downloads_artifact_and_publishes(self) -> None:
        text = read_workflow(WORKFLOW_ROOT / "release-python.yml")
        verify_job = indented_block(text, "verify-wheel-metadata:")
        github_release_job = indented_block(text, "github-release:")
        publish_job = indented_block(text, "publish:")

        self.assertIn("contents: read", verify_job)
        self.assertNotIn("contents: write", verify_job)
        self.assertNotIn("id-token: write", verify_job)
        self.assertIn("python -m pip install --upgrade twine", verify_job)
        self.assertIn("python -m twine check wheels/merman-*.whl", verify_job)

        self.assertIn("contents: write", github_release_job)
        self.assertIn("environment: github-release", github_release_job)
        self.assertNotIn("environment: pypi", github_release_job)
        self.assertNotIn("id-token: write", github_release_job)
        self.assertIn("actions/download-artifact", github_release_job)
        self.assertIn("gh release upload", github_release_job)
        self.assertIn("::error::GitHub Release", github_release_job)
        self.assertIn("exit 1", github_release_job)

        self.assertIn("if: ${{ inputs.publish_to_pypi }}", publish_job)
        self.assertIn("- github-release", publish_job)
        self.assertIn("environment: pypi", publish_job)
        self.assertIn("contents: read", publish_job)
        self.assertNotIn("contents: write", publish_job)
        self.assertIn("id-token: write", publish_job)
        self.assertIn("actions/download-artifact", publish_job)
        self.assertIn("pypa/gh-action-pypi-publish", publish_job)
        for forbidden in [
            "actions/checkout",
            "python -m pip install",
            "twine check",
            "npm ",
            "cargo ",
            "gh release",
        ]:
            with self.subTest(forbidden=forbidden):
                self.assertNotIn(forbidden, publish_job)

    def test_trusted_npm_publish_job_only_downloads_artifact_and_publishes(self) -> None:
        text = read_workflow(WORKFLOW_ROOT / "release-web.yml")
        publish_job = indented_block(text, "publish:")

        self.assertIn("runs-on: ubuntu-24.04", publish_job)
        self.assertIn("environment: npm", publish_job)
        self.assertIn("contents: read", publish_job)
        self.assertIn("id-token: write", publish_job)
        self.assertIn("actions/setup-node@", publish_job)
        self.assertIn('node-version: "24"', publish_job)
        self.assertIn('registry-url: "https://registry.npmjs.org"', publish_job)
        self.assertIn("package-manager-cache: false", publish_job)
        self.assertIn("actions/download-artifact", publish_job)
        self.assertIn('npm publish "$package_file"', publish_job)
        self.assertIn("NPM_DIST_TAG: ${{ needs.validate-inputs.outputs.npm_dist_tag }}", publish_job)
        self.assertIn('--tag "$NPM_DIST_TAG"', publish_job)
        for forbidden in [
            "actions/checkout",
            "NPM_TOKEN",
            "NODE_AUTH_TOKEN",
            "platforms/web/scripts",
            "npm ci",
            "npm run",
            "npm test",
            "cargo install",
            "dtolnay/rust-toolchain",
            "wasm-pack",
        ]:
            with self.subTest(forbidden=forbidden):
                    self.assertNotIn(forbidden, publish_job)

    def test_release_web_pack_output_is_the_exact_uploaded_and_published_artifact(self) -> None:
        workflow = parse_workflow_structure(WORKFLOW_ROOT / "release-web.yml")
        build = workflow_job(workflow, "build")
        publish = workflow_job(workflow, "publish")
        pack = workflow_step(build, name="Pack web package")
        upload = workflow_step(build, name="Upload web package artifact")
        download = workflow_step(publish, name="Download web package artifact")
        publish_step = workflow_step(publish, name="Publish to npm")

        self.assertEqual(pack["id"], "pack")
        self.assertIn("npm pack --ignore-scripts --json", pack["run"])
        self.assertIn("JSON.parse", pack["run"])
        self.assertIn("Array.isArray(pack) || pack.length !== 1", pack["run"])
        self.assertIn("printf 'package_file=%s\\n'", pack["run"])
        self.assertEqual(upload["uses"], "actions/upload-artifact@v6")
        self.assertEqual(upload["with"]["name"], "merman-web-npm-package")
        self.assertEqual(upload["with"]["path"], "${{ steps.pack.outputs.package_file }}")
        self.assertEqual(download["uses"], "actions/download-artifact@v7")
        self.assertEqual(download["with"]["name"], "merman-web-npm-package")
        self.assertIn("find target/npm-package -maxdepth 1 -type f -name '*.tgz'", publish_step["run"])
        self.assertIn('npm publish "$package_file" --ignore-scripts', publish_step["run"])

    def test_trusted_npm_publish_job_does_not_disable_provenance(self) -> None:
        text = read_workflow(WORKFLOW_ROOT / "release-web.yml")
        publish_job = indented_block(text, "publish:")

        self.assertNotIn("--provenance", publish_job)
        assert_no_npm_provenance_disable(self, publish_job)

    def test_release_web_workflow_does_not_disable_provenance(self) -> None:
        text = read_workflow(WORKFLOW_ROOT / "release-web.yml")

        assert_no_npm_provenance_disable(self, text)

    def test_release_web_does_not_expose_npm_publish_tokens(self) -> None:
        text = read_workflow(WORKFLOW_ROOT / "release-web.yml")

        for forbidden in [
            "NPM_TOKEN",
            "NODE_AUTH_TOKEN",
            "secrets.NPM",
            "secrets.NODE_AUTH_TOKEN",
        ]:
            with self.subTest(forbidden=forbidden):
                self.assertNotIn(forbidden, text)

    def test_web_package_metadata_supports_trusted_npm_provenance(self) -> None:
        package = json.loads(WEB_PACKAGE_JSON.read_text(encoding="utf-8"))

        self.assertEqual(package["name"], "@mermanjs/web")
        self.assertEqual(package["repository"]["type"], "git")
        self.assertEqual(
            package["repository"]["url"],
            "git+https://github.com/Latias94/merman.git",
        )
        self.assertEqual(package["publishConfig"]["access"], "public")
        self.assertIsNot(package["publishConfig"].get("provenance"), False)
        assert_no_npm_provenance_disable(self, json.dumps(package, sort_keys=True))

    def test_npmrc_files_do_not_disable_provenance(self) -> None:
        for path in NPM_CONFIG_PATHS:
            with self.subTest(path=path.relative_to(ROOT).as_posix()):
                if not path.exists():
                    continue

                text = path.read_text(encoding="utf-8")
                assert_no_npm_provenance_disable(self, text)

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

    def test_cargo_dist_release_workflow_is_tag_only_and_isolates_publish_authority(self) -> None:
        workflow = parse_workflow_structure(WORKFLOW_ROOT / "release.yml")
        text = read_workflow(WORKFLOW_ROOT / "release.yml")
        dist_config = read_workflow(ROOT / "dist-workspace.toml")
        header = text.split("\njobs:", 1)[0]
        plan = workflow_job(workflow, "plan")
        local_build = workflow_job(workflow, "build-local-artifacts")
        global_build = workflow_job(workflow, "build-global-artifacts")
        host = workflow_job(workflow, "host")

        self.assertEqual(workflow["permissions"], {"contents": "read"})
        self.assertNotIn("pull_request:", header)
        self.assertIn("push:", header)
        self.assertIn("tags:", header)
        self.assertIn("'**[0-9]+.[0-9]+.[0-9]+*'", header)
        self.assertIn('pr-run-mode = "skip"', dist_config)
        self.assertNotIn('pr-run-mode = "plan"', dist_config)
        self.assertIn('cargo-dist-version = "0.32.0"', dist_config)
        self.assertIn('allow-dirty = ["ci"]', dist_config)
        self.assertIn('packages = ["merman-cli", "merman-lsp"]', dist_config)
        self.assertIn("Merman maintains the permission split", header)

        for job_name, job in workflow["jobs"].items():
            assert isinstance(job, dict)
            permissions = job["permissions"]
            assert isinstance(permissions, dict)
            env = job["env"]
            assert isinstance(env, dict)
            steps = job["steps"]
            assert isinstance(steps, list)
            for step in steps:
                assert isinstance(step, dict)
                run = step.get("run", "")
                self.assertNotRegex(run, re.compile(r"\$\{\{[^}]*github\.ref(?:_name)?"))
                self.assertNotRegex(run, re.compile(r"\$\{\{[^}]*outputs\.tag"))
            if job_name == "host":
                continue
            with self.subTest(job=job_name):
                self.assertNotEqual(permissions.get("contents"), "write")
                self.assertNotIn("GH_TOKEN", env)
                for step in steps:
                    self.assertNotIn("GH_TOKEN", step["env"])

        plan_step = workflow_step(plan, step_id="plan")
        self.assertEqual(plan_step["env"]["RELEASE_TAG"], "${{ github.ref_name }}")
        self.assertIn('dist host --steps=create "--tag=$RELEASE_TAG"', plan_step["run"])
        self.assertNotIn("github.event.pull_request", text)
        self.assertNotIn("tag-flag", text)

        for build_job in [local_build, global_build]:
            self.assertEqual(build_job["env"]["RELEASE_TAG"], "${{ needs.plan.outputs.tag }}")
            build_step = next(
                step
                for step in build_job["steps"]
                if isinstance(step, dict) and "dist build" in step.get("run", "")
            )
            self.assertIn('dist build "--tag=$RELEASE_TAG"', build_step["run"])
            self.assertEqual(build_step["shell"], "bash")

        self.assertEqual(host["environment"], "github-release")
        self.assertEqual(host["permissions"], {"contents": "write"})
        self.assertNotIn("GH_TOKEN", host["env"])
        self.assertFalse(
            any(
                isinstance(step, dict) and step.get("uses", "").startswith("actions/checkout")
                for step in host["steps"]
            )
        )
        create_release = workflow_step(host, name="Create GitHub Release")
        self.assertEqual(create_release["env"]["GH_TOKEN"], "${{ secrets.GITHUB_TOKEN }}")
        self.assertEqual(create_release["env"]["GH_REPO"], "${{ github.repository }}")
        self.assertEqual(create_release["env"]["RELEASE_TAG"], "${{ needs.plan.outputs.tag }}")
        self.assertNotIn("dist host", create_release["run"])
        self.assertIn('release_args+=(-- "$RELEASE_TAG" "${assets[@]}")', create_release["run"])
        self.assertIn('gh release create "${release_args[@]}"', create_release["run"])

    def test_cargo_dist_tag_is_passed_as_one_literal_argument(self) -> None:
        workflow = parse_workflow_structure(WORKFLOW_ROOT / "release.yml")
        plan = workflow_job(workflow, "plan")
        plan_step = workflow_step(plan, step_id="plan")

        cases = [
            "v1.2.3$(printf injected > exploit-marker)",
            "v1.2.3;printf injected > exploit-marker;#",
            "--help1.2.3",
        ]
        for release_tag in cases:
            with self.subTest(release_tag=release_tag), tempfile.TemporaryDirectory() as temp_dir:
                temp = Path(temp_dir)
                captured_args = temp / "captured-args.txt"
                github_output = temp / "github-output.txt"
                script = "\n".join(
                    [
                        'dist() { printf \'%s\\n\' "$@" > "$CAPTURED_ARGS"; printf \'{}\\n\'; }',
                        'jq() { printf \'{}\'; }',
                        str(plan_step["run"]),
                    ]
                )
                env = {
                    "PATH": "/usr/bin:/bin",
                    "RELEASE_TAG": release_tag,
                    "CAPTURED_ARGS": str(captured_args),
                    "GITHUB_OUTPUT": str(github_output),
                }

                result = subprocess.run(
                    ["bash", "-c", script],
                    cwd=temp,
                    env=env,
                    text=True,
                    capture_output=True,
                    timeout=10,
                    check=False,
                )

                self.assertEqual(result.returncode, 0, msg=result.stderr)
                self.assertFalse((temp / "exploit-marker").exists())
                self.assertIn(f"--tag={release_tag}", captured_args.read_text(encoding="utf-8").splitlines())

    def test_workflow_contract_parser_rejects_ambiguous_security_shapes(self) -> None:
        cases = {
            "duplicate-permissions": """
                permissions:
                  contents: read
                permissions:
                  contents: write
                jobs:
                  build:
                    steps:
                      - run: true
            """,
            "duplicate-job": """
                jobs:
                  build:
                    steps:
                      - run: true
                  build:
                    steps:
                      - run: false
            """,
            "duplicate-env": """
                jobs:
                  build:
                    env:
                      TOKEN: first
                      TOKEN: second
                    steps:
                      - run: true
            """,
            "uses-and-run": """
                jobs:
                  build:
                    steps:
                      - uses: actions/checkout@v6
                        run: echo ambiguous
            """,
            "modified-block-scalar": """
                jobs:
                  build:
                    steps:
                      - run: >-
                          echo hidden
            """,
            "permission-block-scalar": """
                permissions:
                  contents: >
                    write
                jobs:
                  build:
                    permissions:
                      contents: read
                    steps:
                      - run: echo safe
            """,
            "yaml-alias": """
                permissions:
                  contents: read
                jobs:
                  build:
                    env:
                      GH_TOKEN: *secret
                    steps:
                      - run: echo ambiguous
            """,
        }
        with tempfile.TemporaryDirectory() as temp_dir:
            temp = Path(temp_dir)
            for name, source in cases.items():
                with self.subTest(case=name):
                    workflow = temp / f"{name}.yml"
                    workflow.write_text(textwrap.dedent(source).lstrip(), encoding="utf-8")
                    with self.assertRaises(WorkflowContractError):
                        parse_workflow_structure(workflow)

    def test_github_release_upload_jobs_pin_repository_context(self) -> None:
        cases = [
            ("release-android.yml", "upload-release", "Upload AAR to GitHub Release"),
            ("release-apple.yml", "upload-release", "Upload XCFramework to GitHub Release"),
            ("release-python.yml", "github-release", "Create GitHub Release"),
        ]
        for workflow_name, job_id, step_name in cases:
            with self.subTest(workflow=workflow_name):
                workflow = parse_workflow_structure(WORKFLOW_ROOT / workflow_name)
                job = workflow_job(workflow, job_id)
                step = workflow_step(job, name=step_name)
                self.assertEqual(step["env"]["GH_REPO"], "${{ github.repository }}")
                self.assertEqual(job["environment"], "github-release")
                self.assertEqual(job["permissions"]["contents"], "write")


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

    def test_ci_pins_cypress_corpus_source_alignment(self) -> None:
        text = read_workflow(WORKFLOW_ROOT / "ci.yml")

        self.assertIn("repository: mermaid-js/mermaid", text)
        self.assertIn(
            "ref: 7c0cafcf42e76bfaf79d0cbbd12edb986612f014",
            text,
        )
        for source in (
            "cypress/integration/rendering/treeView/treeView.spec.ts",
            "cypress/integration/rendering/cynefin/cynefin.spec.js",
            "cypress/integration/rendering/railroad/railroad.spec.ts",
        ):
            self.assertIn(source, text)
        self.assertIn(
            "import-upstream-cypress --check-11-16-corpus-manifest-source",
            text,
        )


class PagesWorkflowSecurityTests(unittest.TestCase):
    def test_pages_workflow_header_is_read_only(self) -> None:
        text = read_workflow(WORKFLOW_ROOT / "pages.yml")
        header = text.split("\njobs:", 1)[0]

        self.assertIn("permissions:\n  contents: read", header)
        self.assertNotIn("pages: write", header)
        self.assertNotIn("id-token: write", header)

    def test_pages_build_job_does_not_hold_deploy_permissions(self) -> None:
        text = read_workflow(WORKFLOW_ROOT / "pages.yml")
        build_job = indented_block(text, "build:")
        blocks = checkout_blocks(build_job)

        self.assertIn("permissions:\n      contents: read", build_job)
        self.assertNotIn("pages: write", build_job)
        self.assertNotIn("id-token: write", build_job)
        self.assertGreater(len(blocks), 0)
        for index, block in enumerate(blocks):
            with self.subTest(checkout=index):
                self.assertIn("persist-credentials: false", block)

    def test_pages_deploy_job_owns_pages_write_permissions(self) -> None:
        text = read_workflow(WORKFLOW_ROOT / "pages.yml")
        deploy_job = indented_block(text, "deploy:")

        self.assertIn("pages: write", deploy_job)
        self.assertIn("id-token: write", deploy_job)
        self.assertIn("uses: actions/deploy-pages", deploy_job)


class PerformanceWorkflowSecurityTests(unittest.TestCase):
    def test_performance_head_jobs_do_not_hold_comment_tokens(self) -> None:
        text = read_workflow(WORKFLOW_ROOT / "performance.yml")
        for job_name in ["regression:", "frontmatter:"]:
            job = indented_block(text, job_name)
            with self.subTest(job=job_name.removesuffix(":")):
                self.assertNotIn("issues: write", job)
                self.assertNotIn("pull-requests: write", job)
                self.assertNotIn("GH_TOKEN:", job)
                self.assertNotIn("gh api", job)

    def test_performance_comment_jobs_are_isolated_from_pr_checkout(self) -> None:
        text = read_workflow(WORKFLOW_ROOT / "performance.yml")
        for job_name, artifact in [
            ("regression-comment:", "perf-regression"),
            ("frontmatter-comment:", "perf-frontmatter"),
        ]:
            job = indented_block(text, job_name)
            with self.subTest(job=job_name.removesuffix(":")):
                self.assertIn("issues: write", job)
                self.assertIn("actions/download-artifact", job)
                self.assertIn(f"name: {artifact}", job)
                self.assertIn("GH_TOKEN: ${{ github.token }}", job)
                self.assertIn("gh api", job)
                self.assertNotIn("actions/checkout", job)
                self.assertNotIn("working-directory: head", job)
                self.assertNotIn("tools/bench/", job)

    def test_performance_paths_cover_render_dependencies(self) -> None:
        text = read_workflow(WORKFLOW_ROOT / "performance.yml")
        paths = indented_block(text, "paths:")

        self.assertIn('"Cargo.toml"', paths)
        self.assertIn('"Cargo.lock"', paths)
        self.assertIn('"crates/merman-render/**"', paths)
        self.assertIn('"crates/roughr/**"', paths)

    def test_performance_comment_bodies_are_rendered_before_artifact_upload(self) -> None:
        text = read_workflow(WORKFLOW_ROOT / "performance.yml")
        cases = [
            (
                "regression:",
                "- name: Render regression PR comment",
                "- name: Upload regression artifacts",
                "head/target/performance/pr_comment.md",
            ),
            (
                "frontmatter:",
                "- name: Render frontmatter PR comment",
                "- name: Upload frontmatter artifacts",
                "head/target/performance/frontmatter_pr_comment.md",
            ),
        ]

        for job_name, render_step, upload_step, comment_path in cases:
            job = indented_block(text, job_name)
            upload_block = indented_block(job, upload_step)
            with self.subTest(job=job_name.removesuffix(":")):
                self.assertLess(job.index(render_step), job.index(upload_step))
                self.assertIn(comment_path, upload_block)

    def test_performance_checkouts_do_not_persist_credentials(self) -> None:
        text = read_workflow(WORKFLOW_ROOT / "performance.yml")
        checkout_count = text.count("uses: actions/checkout")
        persisted_false_count = text.count("persist-credentials: false")

        self.assertEqual(persisted_false_count, checkout_count)

    def test_performance_run_blocks_do_not_interpolate_dispatch_inputs(self) -> None:
        text = read_workflow(WORKFLOW_ROOT / "performance.yml")

        for index, block in enumerate(run_blocks(text)):
            with self.subTest(run_block=index):
                self.assertNotIn("inputs.", block)
                self.assertNotIn("${{ inputs.", block)

    def test_performance_reference_toolchain_input_is_validated_before_shell_use(self) -> None:
        text = read_workflow(WORKFLOW_ROOT / "performance.yml")
        install_step = indented_block(text, "- name: Install mermaid-rs-renderer toolchain")
        comparison_step = indented_block(text, "- name: Run cross-repo comparison")

        self.assertIn(
            "MMDR_TOOLCHAIN: ${{ github.event_name == 'workflow_dispatch' && inputs.mmdr_toolchain || '1.92.0' }}",
            install_step,
        )
        self.assertIn('[[ ! "$MMDR_TOOLCHAIN" =~ ^([0-9]+(\\.[0-9]+){0,2}|stable|beta|nightly)', install_step)
        self.assertIn('rustup toolchain install "$MMDR_TOOLCHAIN" --profile minimal', install_step)
        self.assertIn("--mmdr-toolchain \"$MMDR_TOOLCHAIN\"", comparison_step)
        self.assertIn("case \"$PRESET\" in", comparison_step)
        self.assertIn("case \"$SUITE\" in", comparison_step)
        self.assertIn('[[ ! "$MMDR_TOOLCHAIN" =~ ^([0-9]+(\\.[0-9]+){0,2}|stable|beta|nightly)', comparison_step)


if __name__ == "__main__":
    unittest.main()
