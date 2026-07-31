#!/usr/bin/env python3
"""Security contract tests for manual release workflows."""

from __future__ import annotations

import json
import re
import shlex
import subprocess
import sys
import tempfile
import textwrap
import tomllib
import unittest
import uuid
from pathlib import Path

try:
    from scripts.artifact_profile_recipe import (
        load_artifact_profile,
        load_artifact_profiles,
    )
    from scripts.github_workflow_contract import (
        WorkflowContractError,
        load_workflow_contract as parse_workflow_structure,
        workflow_job,
        workflow_step,
    )
except ModuleNotFoundError:
    from artifact_profile_recipe import (
        load_artifact_profile,
        load_artifact_profiles,
    )
    from github_workflow_contract import (
        WorkflowContractError,
        load_workflow_contract as parse_workflow_structure,
        workflow_job,
        workflow_step,
    )


ROOT = Path(__file__).resolve().parents[1]
WORKFLOW_ROOT = ROOT / ".github" / "workflows"
WEB_WORKSPACE_PACKAGE_JSON = ROOT / "platforms" / "web" / "package.json"
WEB_DESCRIPTOR_JSON = ROOT / "platforms" / "web" / "web-surface-descriptor.json"
NPM_CONFIG_PATHS = [
    ROOT / ".npmrc",
    ROOT / "platforms" / "web" / ".npmrc",
]
RELEASE_WORKFLOWS = sorted(WORKFLOW_ROOT.glob("release-*.yml"))
TAG_BOUND_SOURCE_WORKFLOWS = [
    WORKFLOW_ROOT / "release-python.yml",
    WORKFLOW_ROOT / "release-apple.yml",
    WORKFLOW_ROOT / "release-android.yml",
    WORKFLOW_ROOT / "release-flutter.yml",
]
PUBLISH_WORKFLOWS = [
    WORKFLOW_ROOT / "release.yml",
    WORKFLOW_ROOT / "release-crates.yml",
    WORKFLOW_ROOT / "release-web.yml",
    *TAG_BOUND_SOURCE_WORKFLOWS,
]
SOURCE_REF_WORKFLOWS = sorted(
    path
    for path in WORKFLOW_ROOT.glob("*.yml")
    if path not in TAG_BOUND_SOURCE_WORKFLOWS
    and "source_ref:" in path.read_text(encoding="utf-8")
)
SOURCE_REF_RELEASE_WORKFLOWS = [
    path for path in RELEASE_WORKFLOWS if path in SOURCE_REF_WORKFLOWS
]


def read_workflow(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def web_package_entries() -> list[dict]:
    descriptor = json.loads(WEB_DESCRIPTOR_JSON.read_text(encoding="utf-8"))
    packages = descriptor.get("packages")
    if not isinstance(packages, list):
        raise AssertionError("Web package descriptor must contain packages")
    return packages


def web_package_manifest(entry: dict) -> Path:
    package_dir = entry.get("package_dir")
    if not isinstance(package_dir, str):
        raise AssertionError("Web package descriptor entry has no package_dir")
    return ROOT / "platforms" / "web" / package_dir / "package.json"


def exact_binary_build_command(profile_id: str) -> str:
    recipe = load_artifact_profile(profile_id)
    if recipe.cargo_profile != "dist":
        raise AssertionError(f"{profile_id} must use the cargo-dist Cargo profile")
    if recipe.default_features is not False:
        raise AssertionError(f"{profile_id} must disable Cargo default features")
    if recipe.target_kinds != ("bin",) or recipe.crate_types != ("bin",):
        raise AssertionError(f"{profile_id} must select exactly one binary target")
    return (
        f"python3 scripts/artifact_profile_recipe.py {profile_id} "
        "--build-host --locked"
    )


def exact_dependency_gate_command(profile_id: str) -> str:
    matches = [
        profile
        for profile in load_artifact_profiles()
        if profile.profile_id == profile_id
    ]
    if len(matches) != 1:
        raise AssertionError(f"expected one artifact profile {profile_id!r}")
    profile = matches[0]
    recipe = profile.cargo
    if profile.semantic_target != "typst":
        raise AssertionError(f"{profile_id} must be a Typst artifact profile")
    if recipe.default_features is not False:
        raise AssertionError(f"{profile_id} must disable Cargo default features")
    if recipe.build_target_kind != "target-set" or recipe.build_targets != (
        "wasm32-unknown-unknown",
    ):
        raise AssertionError(f"{profile_id} must select the canonical WASM target")
    return (
        "cargo run --locked -p xtask -- profile-budget check-deps "
        f"--profile typst-wasm --artifact-profile {profile_id}"
    )


def workflow_document(path: Path) -> dict:
    return parse_workflow_structure(path)


def workflow_run_blocks(path: Path) -> list[str]:
    document = workflow_document(path)
    return [
        step["run"]
        for job in document["jobs"].values()
        for step in job["steps"]
        if isinstance(step.get("run"), str)
    ]


def contract_text(value: object) -> str:
    """Render parsed contract values for concise substring assertions."""
    lines: list[str] = []

    def append(item: object) -> None:
        if isinstance(item, dict):
            for key, child in item.items():
                if isinstance(child, (dict, list)):
                    lines.append(f"{key}:")
                    append(child)
                else:
                    lines.append(f"{key}: {child}")
        elif isinstance(item, list):
            for child in item:
                append(child)
        else:
            lines.append(str(item))

    append(value)
    return "\n".join(lines)


def job_contract(path: Path, job_id: str) -> dict:
    return workflow_job(workflow_document(path), job_id)


def job_contract_text(path: Path, job_id: str) -> str:
    return contract_text(job_contract(path, job_id))


def step_contract_text(path: Path, job_id: str, step_name: str) -> str:
    return contract_text(workflow_step(job_contract(path, job_id), name=step_name))


def checkout_steps(contract: dict) -> list[dict]:
    if "jobs" in contract:
        jobs = contract["jobs"].values()
    else:
        jobs = (contract,)
    return [
        step
        for job in jobs
        for step in job.get("steps", [])
        if isinstance(step.get("uses"), str)
        and step["uses"].startswith("actions/checkout@")
    ]


def validation_script(path: Path) -> str:
    for block in workflow_run_blocks(path):
        if (
            "DISPATCH_RELEASE_TAG" in block or "DISPATCH_SOURCE_REF" in block
        ) and "GITHUB_OUTPUT" in block:
            return textwrap.dedent(block)
    raise AssertionError(f"could not find validation script in {path.name}")


def run_workflow_validation(
    path: Path,
    *,
    release_tag: str = "v1.2.3",
    source_ref: str = "main",
    version: str | None = None,
    event_name: str = "workflow_dispatch",
    git_ref: str | None = None,
) -> tuple[subprocess.CompletedProcess[str], dict[str, str]]:
    if version is None:
        version = release_tag.removeprefix("v")
    if git_ref is None:
        git_ref = f"refs/tags/{release_tag}"
    output_dir = ROOT / "target" / "release-workflow-tests"
    output_dir.mkdir(parents=True, exist_ok=True)
    run_id = uuid.uuid4().hex
    output_path = output_dir / f"github-output-{run_id}.txt"
    script_path = output_dir / f"{path.stem}-validation-{run_id}.sh"
    script = "\n".join(
        [
            f"EVENT_NAME={shlex.quote(event_name)}",
            f"DISPATCH_RELEASE_TAG={shlex.quote(release_tag)}",
            f"DISPATCH_VERSION={shlex.quote(version)}",
            f"DISPATCH_SOURCE_REF={shlex.quote(source_ref)}",
            f"GIT_REF={shlex.quote(git_ref)}",
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
            for index, block in enumerate(workflow_run_blocks(path)):
                with self.subTest(workflow=path.name, run_block=index):
                    self.assertNotIn("${{ inputs.", block)

    def test_source_ref_checkouts_use_validated_output(self) -> None:
        for path in SOURCE_REF_WORKFLOWS:
            document = workflow_document(path)
            text = read_workflow(path)
            checkouts = checkout_steps(document)
            refs = [step["with"].get("ref") for step in checkouts]
            allowed_refs = {
                "${{ steps.release.outputs.source_ref }}",
                "${{ needs.validate-inputs.outputs.source_ref }}",
                "${{ needs.validate-inputs.outputs.source_sha }}",
                "${{ needs.preflight.outputs.source_sha }}",
                "${{ github.workflow_sha }}",
            }
            with self.subTest(workflow=path.name):
                self.assertTrue(checkouts)
                self.assertTrue(all(ref in allowed_refs for ref in refs))
                validate_checkouts = checkout_steps(
                    workflow_job(document, "validate-inputs")
                )
                self.assertGreaterEqual(
                    refs.count("${{ github.workflow_sha }}"),
                    sum(
                        step["with"].get("ref") == "${{ github.workflow_sha }}"
                        for step in validate_checkouts
                    ),
                )
                self.assertNotIn("ref: ${{ inputs.source_ref }}", text)
                self.assertNotIn("inputs.source_ref ||", text)

    def test_release_web_validation_uses_the_trusted_canonical_version_parser(self) -> None:
        validate_job = job_contract_text(
            WORKFLOW_ROOT / "release-web.yml",
            "validate-inputs",
        )

        self.assertIn("uses: actions/checkout@v6", validate_job)
        self.assertIn("ref: ${{ github.workflow_sha }}", validate_job)
        self.assertIn("persist-credentials: false", validate_job)
        self.assertIn("scripts/release-version.py canonical", validate_job)
        self.assertIn("scripts/release-version.py npm-dist-tag", validate_job)
        self.assertNotIn("is_release_version()", validate_job)

    def test_native_release_workflows_pin_tag_commit_and_tree_before_building(
        self,
    ) -> None:
        for path in TAG_BOUND_SOURCE_WORKFLOWS:
            document = workflow_document(path)
            validate_job = contract_text(workflow_job(document, "validate-inputs"))
            build_job = contract_text(workflow_job(document, "build"))

            with self.subTest(workflow=path.name):
                self.assertNotIn("source_ref:", document["header"])
                self.assertIn(
                    "source_sha: ${{ steps.source.outputs.source_sha }}",
                    validate_job,
                )
                self.assertIn(
                    "source_tree: ${{ steps.source.outputs.source_tree }}",
                    validate_job,
                )
                self.assertIn(
                    "ref: ${{ steps.release.outputs.source_ref }}",
                    validate_job,
                )
                self.assertIn(
                    'git rev-parse "refs/tags/${RELEASE_TAG}^{commit}"',
                    validate_job,
                )
                self.assertIn(
                    'git rev-parse "refs/tags/${RELEASE_TAG}^{tree}"',
                    validate_job,
                )
                self.assertIn(
                    "ref: ${{ needs.validate-inputs.outputs.source_sha }}",
                    build_job,
                )
                self.assertIn("Verify pinned release source", build_job)
                self.assertIn("EXPECTED_SOURCE_SHA", build_job)
                self.assertIn("EXPECTED_SOURCE_TREE", build_job)
                self.assertNotIn(
                    "ref: ${{ needs.validate-inputs.outputs.source_ref }}",
                    build_job,
                )
                for step in checkout_steps(document):
                    self.assertEqual(step["with"].get("persist-credentials"), "false")

    def test_native_release_validation_derives_source_ref_from_release_tag(
        self,
    ) -> None:
        for path in TAG_BOUND_SOURCE_WORKFLOWS:
            with self.subTest(workflow=path.name):
                result, outputs = run_workflow_validation(
                    path,
                    release_tag="v1.2.3",
                    source_ref="main",
                )

                self.assertEqual(
                    result.returncode,
                    0,
                    msg=f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}",
                )
                self.assertEqual(outputs["source_ref"], "refs/tags/v1.2.3")
                self.assertEqual(outputs["release_tag"], "v1.2.3")
                self.assertEqual(outputs["version"], "1.2.3")

    def test_native_release_validation_rejects_invalid_release_tags(self) -> None:
        for path in TAG_BOUND_SOURCE_WORKFLOWS:
            with self.subTest(workflow=path.name):
                result, outputs = run_workflow_validation(
                    path,
                    release_tag="v1.2.3;echo-unsafe",
                )

                self.assertNotEqual(
                    result.returncode,
                    0,
                    msg=f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}",
                )
                self.assertNotIn("source_ref", outputs)
                self.assertNotIn("release_tag", outputs)

    def test_flutter_tag_push_must_match_the_release_tag(self) -> None:
        workflow = WORKFLOW_ROOT / "release-flutter.yml"
        result, outputs = run_workflow_validation(
            workflow,
            release_tag="v1.2.3",
            event_name="push",
        )
        self.assertEqual(
            result.returncode,
            0,
            msg=f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}",
        )
        self.assertEqual(outputs["source_ref"], "refs/tags/v1.2.3")

        result, outputs = run_workflow_validation(
            workflow,
            release_tag="v1.2.3",
            event_name="push",
            git_ref="refs/heads/main",
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn("source_ref", outputs)

    def test_publish_workflows_verify_all_version_and_readme_projections(self) -> None:
        for path in PUBLISH_WORKFLOWS:
            with self.subTest(workflow=path.name):
                self.assertIn(
                    "scripts/release-version.py check --version",
                    read_workflow(path),
                )

    def test_native_release_workflows_fail_closed_on_generated_projection_drift(
        self,
    ) -> None:
        apple = job_contract_text(
            WORKFLOW_ROOT / "release-apple.yml",
            "build",
        )
        flutter = job_contract_text(
            WORKFLOW_ROOT / "release-flutter.yml",
            "build",
        )
        python = job_contract_text(
            WORKFLOW_ROOT / "release-python.yml",
            "build",
        )

        self.assertIn(
            "git ls-files --error-unmatch -- "
            "platforms/apple/Sources/Merman/Generated/Merman.swift",
            apple,
        )
        self.assertIn(
            "git diff --exit-code -- platforms/apple/Sources/Merman/Generated",
            apple,
        )
        self.assertIn(
            "git ls-files --error-unmatch -- lib/src/generated/native_abi.dart",
            flutter,
        )
        self.assertIn(
            "git diff --exit-code -- lib/src/generated/native_abi.dart",
            flutter,
        )
        self.assertIn(
            "python scripts/build-python-uniffi-wheel.py --run-smoke",
            python,
        )

    def test_flutter_release_smokes_the_packaged_macos_library(self) -> None:
        build_job = job_contract_text(
            WORKFLOW_ROOT / "release-flutter.yml",
            "build",
        )

        self.assertIn(
            'dart run example/smoke.dart "macos/Libraries/libmerman_ffi.dylib"',
            build_job,
        )

    def test_source_ref_checkouts_do_not_persist_credentials(self) -> None:
        for path in SOURCE_REF_WORKFLOWS:
            steps = checkout_steps(workflow_document(path))
            with self.subTest(workflow=path.name, checkout_count=len(steps)):
                self.assertTrue(steps)

            for index, step in enumerate(steps):
                with self.subTest(workflow=path.name, checkout=index):
                    self.assertEqual(step["with"].get("persist-credentials"), "false")

    def test_validation_jobs_precede_release_checkouts(self) -> None:
        for path in SOURCE_REF_WORKFLOWS:
            text = read_workflow(path)

            with self.subTest(workflow=path.name):
                self.assertIn("validate-inputs:", text)
                self.assertLess(text.index("validate-inputs:"), text.index("uses: actions/checkout"))

    def test_validation_jobs_expose_safe_source_ref_output(self) -> None:
        for path in SOURCE_REF_WORKFLOWS:
            validate_job = job_contract_text(path, "validate-inputs")
            with self.subTest(workflow=path.name):
                self.assertIn("GITHUB_OUTPUT", validate_job)
                self.assertRegex(validate_job, re.compile(r"""(printf 'source_ref=%s\\n'|echo "source_ref=)"""))

    def test_release_validation_jobs_expose_safe_release_output_names(self) -> None:
        for path in RELEASE_WORKFLOWS:
            text = read_workflow(path)
            if "source_ref:" not in text:
                continue

            validate_job = job_contract_text(path, "validate-inputs")
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
            "v1.2.3.",
            "v1.2.3-alpha",
            "v1.2.3-alpha.1.2",
            "v1.2.3-alpha.1.",
            "v1.2.3+build.",
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

    def test_only_release_preflight_accepts_full_sha_source_ref_values(self) -> None:
        full_sha = "0123456789abcdef0123456789abcdef01234567"
        for path in SOURCE_REF_WORKFLOWS:
            with self.subTest(workflow=path.name):
                result, outputs = run_workflow_validation(path, source_ref=full_sha)

                if path.name == "release-preflight.yml":
                    self.assertEqual(
                        result.returncode,
                        0,
                        msg=f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}",
                    )
                    self.assertEqual(outputs["source_ref"], full_sha)
                else:
                    self.assertNotEqual(
                        result.returncode,
                        0,
                        msg=f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}",
                    )
                    self.assertNotIn("source_ref", outputs)

    def test_release_validation_scripts_reject_mismatched_source_tags(self) -> None:
        for path in SOURCE_REF_RELEASE_WORKFLOWS:
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
            validate_job = job_contract_text(path, "validate-inputs")
            with self.subTest(workflow=path.name):
                self.assertIn('[[ "$SOURCE_REF" != *$\'\\n\'*', validate_job)
                self.assertIn("source_ref must be", validate_job)
                self.assertNotIn("sha_re=", validate_job)
                self.assertNotIn("is_sha_ref", validate_job)
                self.assertNotIn("40-character SHA", text)

    def test_release_validation_jobs_reject_untrusted_ref_and_version_shapes(self) -> None:
        for path in SOURCE_REF_RELEASE_WORKFLOWS:
            validate_job = job_contract_text(path, "validate-inputs")
            with self.subTest(workflow=path.name):
                self.assertTrue(
                    ("semver_re=" in validate_job and "0|[1-9]" in validate_job)
                    or ("is_uint()" in validate_job and "is_release_version()" in validate_job)
                    or "scripts/release-version.py canonical" in validate_job
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
            validate_job = job_contract_text(path, "validate-inputs")
            with self.subTest(workflow=path.name):
                self.assertNotIn("contents: write", validate_job)
                self.assertNotIn("id-token: write", validate_job)

    def test_platform_release_build_jobs_do_not_hold_release_write_permission(self) -> None:
        for path in [
            WORKFLOW_ROOT / "release-android.yml",
            WORKFLOW_ROOT / "release-apple.yml",
        ]:
            build_job = job_contract_text(path, "build")
            upload_job = job_contract_text(path, "upload-release")

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
        path = WORKFLOW_ROOT / "release-crates.yml"
        text = read_workflow(path)
        document = workflow_document(path)
        preflight = workflow_job(document, "preflight")
        publish = workflow_job(document, "publish")
        preflight_job = contract_text(preflight)
        web_owner_job = contract_text(workflow_job(document, "web-owner-preflight"))
        typst_owner_job = contract_text(
            workflow_job(document, "typst-owner-preflight")
        )
        publish_job = contract_text(publish)
        preflight_step = contract_text(
            workflow_step(preflight, name="Preflight crates in dependency order")
        )
        upload = workflow_step(publish, name="Upload crates to crates.io")
        upload_step = contract_text(upload)
        upload_run = upload["run"]

        self.assertNotIn("--dry-run", preflight_step)
        for job in [preflight_job, web_owner_job, typst_owner_job]:
            self.assertNotIn("CARGO_REGISTRY_TOKEN", job)
            self.assertNotIn("secrets.", job)
            self.assertNotIn("environment: crates.io", job)
            self.assertNotIn("contents: write", job)
        self.assertIn("source_sha: ${{ steps.source.outputs.source_sha }}", preflight_job)
        self.assertIn('source_sha="$(git rev-parse HEAD)"', preflight_job)
        self.assertIn(
            "needs: [validate-inputs, preflight, web-owner-preflight, typst-owner-preflight]",
            publish_job,
        )
        self.assertIn("needs: preflight", web_owner_job)
        self.assertIn("needs: preflight", typst_owner_job)
        self.assertIn("ref: ${{ needs.preflight.outputs.source_sha }}", web_owner_job)
        self.assertIn("ref: ${{ needs.preflight.outputs.source_sha }}", typst_owner_job)
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
        self.assertEqual(text.count("secrets.CARGO_REGISTRY_TOKEN"), 1)
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
        path = WORKFLOW_ROOT / "release-python.yml"
        publish = job_contract(path, "publish")
        verify_job = job_contract_text(path, "verify-wheel-metadata")
        github_release_job = job_contract_text(path, "github-release")
        publish_job = contract_text(publish)

        self.assertIn("contents: read", verify_job)
        self.assertNotIn("contents: write", verify_job)
        self.assertNotIn("id-token: write", verify_job)
        self.assertIn("python -m pip install --upgrade twine", verify_job)
        self.assertIn("python -m twine check wheels/merman-*.whl", verify_job)
        self.assertIn('test "${#wheels[@]}" -eq 3', verify_job)
        self.assertIn(
            'python scripts/python_wheel_licenses.py "${wheels[@]}"',
            verify_job,
        )

        self.assertIn("contents: write", github_release_job)
        self.assertIn("environment: github-release", github_release_job)
        self.assertNotIn("environment: pypi", github_release_job)
        self.assertNotIn("id-token: write", github_release_job)
        self.assertIn("actions/download-artifact", github_release_job)
        self.assertIn("gh release upload", github_release_job)
        self.assertIn("::error::GitHub Release", github_release_job)
        self.assertIn("exit 1", github_release_job)

        self.assertIn("if: ${{ inputs.publish_to_pypi }}", publish_job)
        self.assertIn("github-release", publish["needs"])
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
        publish_job = job_contract_text(
            WORKFLOW_ROOT / "release-web.yml",
            "publish",
        )

        self.assertIn("runs-on: ubuntu-24.04", publish_job)
        self.assertIn("environment: npm", publish_job)
        self.assertIn("contents: read", publish_job)
        self.assertIn("id-token: write", publish_job)
        self.assertIn("actions/setup-node@", publish_job)
        self.assertIn("node-version: 24", publish_job)
        self.assertIn("registry-url: https://registry.npmjs.org", publish_job)
        self.assertIn("package-manager-cache: false", publish_job)
        self.assertIn("Checkout trusted release verifier", publish_job)
        self.assertIn("ref: ${{ github.workflow_sha }}", publish_job)
        self.assertIn("persist-credentials: false", publish_job)
        self.assertIn("actions/download-artifact", publish_job)
        self.assertIn("python3 scripts/web_package_group.py verify-artifact", publish_job)
        self.assertIn("python3 scripts/web_package_group.py reconcile", publish_job)
        self.assertIn("--source-sha \"$SOURCE_SHA\"", publish_job)
        self.assertIn("--target-dist-tag \"$NPM_DIST_TAG\"", publish_job)
        self.assertIn("--descriptor platforms/web/web-surface-descriptor.json", publish_job)
        self.assertIn("--report target/npm-package-group/reconciliation-report.json", publish_job)
        self.assertNotIn("npm publish", publish_job)
        self.assertNotIn("target/npm-package-group/web_package_group.py", publish_job)
        for forbidden in [
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
        verify_packages = workflow_step(build, name="Verify Web package group")
        pack = workflow_step(build, name="Pack verified Web package group")
        upload = workflow_step(build, name="Upload verified Web package group")
        download = workflow_step(publish, name="Download verified Web package group")
        verify = workflow_step(publish, name="Verify downloaded Web package group")
        reconcile = workflow_step(publish, name="Reconcile npm package group")
        report = workflow_step(publish, name="Upload npm reconciliation report")

        self.assertEqual(verify_packages["run"], "npm run verify:packages --prefix platforms/web")
        self.assertEqual(pack["id"], "pack")
        self.assertIn("python3 scripts/web_package_group.py pack", pack["run"])
        self.assertIn("python3 scripts/web_package_group.py verify-artifact", pack["run"])
        self.assertIn("--descriptor platforms/web/web-surface-descriptor.json", pack["run"])
        self.assertNotIn("cp scripts/web_package_group.py", pack["run"])
        self.assertEqual(upload["uses"], "actions/upload-artifact@v6")
        self.assertEqual(upload["with"]["name"], "merman-web-npm-package-group")
        self.assertEqual(upload["with"]["path"], "${{ steps.pack.outputs.artifact_dir }}")
        self.assertEqual(download["uses"], "actions/download-artifact@v7")
        self.assertEqual(download["with"]["name"], "merman-web-npm-package-group")
        self.assertEqual(verify["shell"], "bash")
        self.assertIn("python3 scripts/web_package_group.py verify-artifact", verify["run"])
        self.assertIn("--source-sha \"$SOURCE_SHA\"", verify["run"])
        self.assertIn("--target-dist-tag \"$NPM_DIST_TAG\"", verify["run"])
        self.assertEqual(reconcile["shell"], "bash")
        self.assertIn("python3 scripts/web_package_group.py reconcile", reconcile["run"])
        self.assertIn("--report target/npm-package-group/reconciliation-report.json", reconcile["run"])
        self.assertEqual(report["uses"], "actions/upload-artifact@v6")
        self.assertEqual(report["if"], "${{ always() }}")
        self.assertEqual(report["with"]["name"], "merman-web-npm-reconciliation-report")

    def test_release_preflight_packs_and_verifies_the_web_package_group(self) -> None:
        workflow = parse_workflow_structure(WORKFLOW_ROOT / "release-preflight.yml")
        preflight = workflow_job(workflow, "web-npm-dry-run")
        verify_packages = workflow_step(preflight, name="Verify web package")
        pack = workflow_step(preflight, name="Pack and verify Web package group dry-run")

        self.assertEqual(verify_packages["run"], "npm run verify:packages --prefix platforms/web")
        self.assertIn("python3 scripts/web_package_group.py pack", pack["run"])
        self.assertIn("python3 scripts/web_package_group.py verify-artifact", pack["run"])
        self.assertIn("--source-sha \"$source_sha\"", pack["run"])
        self.assertIn("--target-dist-tag staging", pack["run"])
        self.assertNotIn("npm pack --dry-run", pack["run"])

    def test_release_preflight_can_pin_an_immutable_dispatch_sha(self) -> None:
        workflow = parse_workflow_structure(WORKFLOW_ROOT / "release-preflight.yml")
        validate = workflow_job(workflow, "validate-inputs")
        input_step = workflow_step(validate, name="Validate release inputs")
        pin_step = workflow_step(validate, name="Pin resolved source commit")

        self.assertIn(
            '[[ "$SOURCE_REF" =~ ^[0-9a-f]{40}$ ]]',
            input_step["run"],
        )
        self.assertEqual(
            pin_step["env"]["EXPECTED_SOURCE_REF"],
            "${{ steps.release.outputs.source_ref }}",
        )
        self.assertIn(
            'test "$source_sha" = "$EXPECTED_SOURCE_REF"',
            pin_step["run"],
        )
        self.assertIn(
            'printf \'source_sha=%s\\n\' "$source_sha"',
            pin_step["run"],
        )

    def test_release_preflight_recomputes_exact_rust_license_reports(self) -> None:
        workflow = parse_workflow_structure(WORKFLOW_ROOT / "release-preflight.yml")
        preflight = workflow_job(workflow, "versions-and-packages")
        install = workflow_step(preflight, name="Install cargo-about")
        verify = workflow_step(
            preflight,
            name="Verify exact Rust dependency license reports",
        )

        self.assertEqual(install["uses"], "taiki-e/install-action@v2")
        self.assertEqual(install["with"]["tool"], "cargo-about@0.9.1")
        self.assertEqual(
            verify["run"],
            "python3 scripts/generate-rust-license-report.py --check",
        )

    def test_release_preflight_validates_generated_cli_assets_natively(self) -> None:
        workflow = parse_workflow_structure(WORKFLOW_ROOT / "release-preflight.yml")
        preflight = workflow_job(workflow, "versions-and-packages")
        validation = workflow_step(
            preflight,
            name="Validate CLI distribution assets",
        )
        self.assertEqual(
            validation["run"],
            "python3 scripts/verify_cli_assets.py "
            "--require bash,zsh,fish,elvish,mandoc",
        )

    def test_trusted_npm_publish_job_does_not_disable_provenance(self) -> None:
        publish_job = job_contract_text(
            WORKFLOW_ROOT / "release-web.yml",
            "publish",
        )

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
        workspace = json.loads(WEB_WORKSPACE_PACKAGE_JSON.read_text(encoding="utf-8"))
        self.assertIs(workspace.get("private"), True)

        public_names: set[str] = set()
        for entry in web_package_entries():
            with self.subTest(package=entry["id"]):
                package = json.loads(web_package_manifest(entry).read_text(encoding="utf-8"))
                self.assertEqual(package["name"], entry["name"])
                self.assertEqual(package["merman"]["artifact_profile"], entry["artifact_profile"])
                self.assertEqual(set(package["exports"]), {"."})
                self.assertEqual(package["repository"]["type"], "git")
                self.assertEqual(
                    package["repository"]["url"],
                    "git+https://github.com/Latias94/merman.git",
                )
                if entry["visibility"] == "candidate":
                    self.assertIs(package.get("private"), True)
                    self.assertNotIn("publishConfig", package)
                else:
                    public_names.add(package["name"])
                    self.assertIsNot(package.get("private"), True)
                    self.assertEqual(package["publishConfig"]["access"], "public")
                    self.assertIsNot(package["publishConfig"].get("provenance"), False)
                assert_no_npm_provenance_disable(self, json.dumps(package, sort_keys=True))

        self.assertEqual(
            public_names,
            {
                "@mermanjs/web",
                "@mermanjs/web-analysis",
                "@mermanjs/web-ascii",
                "@mermanjs/web-editor",
                "@mermanjs/web-render",
            },
        )

    def test_npmrc_files_do_not_disable_provenance(self) -> None:
        for path in NPM_CONFIG_PATHS:
            with self.subTest(path=path.relative_to(ROOT).as_posix()):
                if not path.exists():
                    continue

                text = path.read_text(encoding="utf-8")
                assert_no_npm_provenance_disable(self, text)

    def test_trusted_pubdev_publish_job_only_downloads_artifact_and_publishes(self) -> None:
        publish_job = job_contract_text(
            WORKFLOW_ROOT / "release-flutter.yml",
            "publish",
        )

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

    def test_vscode_runtime_builds_use_exact_artifact_profile_recipes(self) -> None:
        expected_commands = [
            exact_binary_build_command("lsp-stdio-release"),
            exact_binary_build_command("cli-release"),
        ]
        cases = [
            ("release-preflight.yml", "vscode-extension-dry-run"),
            ("vscode-extension.yml", "package"),
        ]
        for workflow_name, job_id in cases:
            with self.subTest(workflow=workflow_name):
                workflow = parse_workflow_structure(WORKFLOW_ROOT / workflow_name)
                job = workflow_job(workflow, job_id)
                step = workflow_step(job, name="Build release runtime binaries")
                self.assertEqual(step["run"].splitlines(), expected_commands)

    def test_release_docs_use_separate_exact_lsp_and_cli_recipes(self) -> None:
        lsp_command = exact_binary_build_command("lsp-stdio-release")
        cli_command = exact_binary_build_command("cli-release")
        releasing = (ROOT / "docs/release/RELEASING.md").read_text(encoding="utf-8")
        self.assertIn(lsp_command, releasing)
        self.assertIn(cli_command, releasing)
        self.assertNotIn("-p merman-lsp -p merman-cli", releasing)

        surfaces_doc = (ROOT / "docs/release/PACKAGE_SURFACES.md").read_text(
            encoding="utf-8"
        )
        self.assertNotIn(lsp_command, surfaces_doc)
        self.assertNotIn(cli_command, surfaces_doc)


    def test_typst_dependency_gate_uses_the_exact_artifact_profile(self) -> None:
        command = exact_dependency_gate_command("typst-wasm")
        ci = read_workflow(WORKFLOW_ROOT / "ci.yml")
        self.assertEqual(ci.count(command), 2)

        for relative_path in [
            "crates/merman-typst-plugin/README.md",
            "docs/release/RELEASING.md",
        ]:
            with self.subTest(path=relative_path):
                text = (ROOT / relative_path).read_text(encoding="utf-8")
                self.assertIn(command, text)

        surfaces_doc = (ROOT / "docs/release/PACKAGE_SURFACES.md").read_text(
            encoding="utf-8"
        )
        self.assertNotIn(command, surfaces_doc)

    def test_cargo_dist_metadata_matches_exact_release_profiles(self) -> None:
        dist_config = tomllib.loads((ROOT / "dist-workspace.toml").read_text(encoding="utf-8"))
        expected_targets = set(dist_config["dist"]["targets"])
        expected_packages = set(dist_config["dist"]["packages"])
        actual_packages: set[str] = set()

        for profile_id in ["cli-release", "lsp-stdio-release"]:
            with self.subTest(profile=profile_id):
                recipe = load_artifact_profile(profile_id)
                actual_packages.add(recipe.package)
                manifest = tomllib.loads(
                    (ROOT / recipe.manifest).read_text(encoding="utf-8")
                )
                dist = manifest["package"]["metadata"]["dist"]
                self.assertIs(dist["default-features"], recipe.default_features)
                self.assertEqual(dist["features"], list(recipe.features))
                self.assertEqual(set(recipe.build_targets), expected_targets)

        self.assertEqual(actual_packages, expected_packages)

    def test_cargo_dist_release_workflow_is_tag_only_and_isolates_publish_authority(self) -> None:
        workflow = parse_workflow_structure(WORKFLOW_ROOT / "release.yml")
        text = read_workflow(WORKFLOW_ROOT / "release.yml")
        dist_config = read_workflow(ROOT / "dist-workspace.toml")
        header = text.split("\njobs:", 1)[0]
        plan = workflow_job(workflow, "plan")
        local_build = workflow_job(workflow, "build-local-artifacts")
        central_verification = workflow_job(workflow, "verify-release-archives")
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
        self.assertNotIn("build-global-artifacts", workflow["jobs"])

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
                    if (
                        job_name == "attest-release-assets"
                        and step.get("name")
                        == "Verify release tag still resolves to the source commit"
                    ):
                        self.assertEqual(
                            step["env"].get("GH_TOKEN"),
                            "${{ secrets.GITHUB_TOKEN }}",
                        )
                    else:
                        self.assertNotIn("GH_TOKEN", step["env"])

        plan_step = workflow_step(plan, step_id="plan")
        validate_tag = workflow_step(plan, name="Validate release tag")
        install_dist = workflow_step(plan, name="Install dist")
        self.assertEqual(validate_tag["env"]["RELEASE_TAG"], "${{ github.ref_name }}")
        self.assertIn(
            'scripts/release-version.py canonical --version "$RELEASE_TAG"',
            validate_tag["run"],
        )
        self.assertIn(
            'scripts/release-version.py check --version "$RELEASE_TAG"',
            validate_tag["run"],
        )
        self.assertIn("Install dist", install_dist["name"])
        self.assertLess(plan["steps"].index(validate_tag), plan["steps"].index(install_dist))
        self.assertEqual(plan_step["env"]["RELEASE_TAG"], "${{ steps.release.outputs.tag }}")
        self.assertIn('dist host --steps=create "--tag=$RELEASE_TAG"', plan_step["run"])
        self.assertNotIn("github.event.pull_request", text)
        self.assertNotIn("tag-flag", text)

        self.assertEqual(local_build["env"]["RELEASE_TAG"], "${{ needs.plan.outputs.tag }}")
        local_build_step = next(
            step
            for step in local_build["steps"]
            if isinstance(step, dict) and "dist build" in step.get("run", "")
        )
        self.assertIn('dist build "--tag=$RELEASE_TAG"', local_build_step["run"])
        self.assertEqual(local_build_step["shell"], "bash")

        self.assertEqual(
            central_verification["env"]["RELEASE_TAG"],
            "${{ needs.plan.outputs.tag }}",
        )
        verify_step = workflow_step(
            central_verification,
            name="Verify CLI and LSP archive structure",
        )
        global_build_step = workflow_step(
            central_verification,
            name="Generate final installers and checksum index",
        )
        self.assertLess(
            central_verification["steps"].index(verify_step),
            central_verification["steps"].index(global_build_step),
        )
        self.assertIn('"--tag=$RELEASE_TAG"', global_build_step["run"])
        self.assertIn("--artifacts=global", global_build_step["run"])
        self.assertEqual(global_build_step["shell"], "bash")

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

    def test_cargo_dist_rejects_noncanonical_tags_before_dist(self) -> None:
        workflow = parse_workflow_structure(WORKFLOW_ROOT / "release.yml")
        plan = workflow_job(workflow, "plan")
        validate_tag = workflow_step(plan, name="Validate release tag")
        release_version = ROOT / "scripts" / "release-version.py"
        validation_script = str(validate_tag["run"]).replace(
            "python3 scripts/release-version.py",
            f"{shlex.quote(sys.executable)} {shlex.quote(str(release_version))}",
        )

        cases = [
            "v1.2.3$(printf injected > exploit-marker)",
            "v1.2.3;printf injected > exploit-marker;#",
            "--help1.2.3",
        ]
        for release_tag in cases:
            with self.subTest(release_tag=release_tag), tempfile.TemporaryDirectory() as temp_dir:
                temp = Path(temp_dir)
                github_output = temp / "github-output.txt"
                env = {
                    "PATH": "/usr/bin:/bin",
                    "RELEASE_TAG": release_tag,
                    "GITHUB_OUTPUT": str(github_output),
                }

                result = subprocess.run(
                    ["bash", "-c", validation_script],
                    cwd=temp,
                    env=env,
                    text=True,
                    capture_output=True,
                    timeout=10,
                    check=False,
                )

                self.assertNotEqual(result.returncode, 0, msg=result.stderr)
                self.assertFalse((temp / "exploit-marker").exists())
                self.assertFalse(github_output.exists())

    def test_cargo_dist_passes_valid_tag_as_one_literal_argument(self) -> None:
        workflow = parse_workflow_structure(WORKFLOW_ROOT / "release.yml")
        plan = workflow_job(workflow, "plan")
        plan_step = workflow_step(plan, step_id="plan")

        with tempfile.TemporaryDirectory() as temp_dir:
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
                "RELEASE_TAG": "v1.2.3",
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
            self.assertEqual(
                captured_args.read_text(encoding="utf-8").splitlines()[0:2],
                ["host", "--steps=create"],
            )
            self.assertIn(
                "--tag=v1.2.3",
                captured_args.read_text(encoding="utf-8").splitlines(),
            )

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
            "folded-run-scalar": """
                jobs:
                  build:
                    steps:
                      - run: >
                          echo unsupported
            """,
            "chomped-run-scalar": """
                jobs:
                  build:
                    steps:
                      - run: |-
                          echo unsupported
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

    def test_workflow_contract_supports_literal_run_block_scalars(self) -> None:
        source = """
            jobs:
              build:
                steps:
                  - name: Literal command
                    run: |
                      echo third
                      echo fourth
        """
        with tempfile.TemporaryDirectory() as temp_dir:
            workflow = Path(temp_dir) / "workflow.yml"
            workflow.write_text(textwrap.dedent(source).lstrip(), encoding="utf-8")
            document = parse_workflow_structure(workflow)

        job = workflow_job(document, "build")
        self.assertEqual(
            workflow_step(job, name="Literal command")["run"],
            "echo third\necho fourth",
        )

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
        workflow = workflow_document(WORKFLOW_ROOT / "ci.yml")
        self.assertEqual(workflow["permissions"], {"contents": "read"})

    def test_ci_checkouts_do_not_persist_credentials(self) -> None:
        steps = checkout_steps(workflow_document(WORKFLOW_ROOT / "ci.yml"))

        self.assertTrue(steps)
        for index, step in enumerate(steps):
            with self.subTest(checkout=index):
                self.assertEqual(step["with"].get("persist-credentials"), "false")

    def test_ci_runs_node_candidate_contracts_against_the_nested_lockfile(self) -> None:
        workflow = parse_workflow_structure(WORKFLOW_ROOT / "ci.yml")
        job = workflow_job(workflow, "build-test")
        setup = workflow_step(job, name="Setup Node for generated Mermaid artifacts")
        install = workflow_step(
            job, name="Install Node candidate development dependencies"
        )
        verify = workflow_step(
            job, name="Verify Node candidate contracts and nested lock freshness"
        )

        self.assertEqual(
            setup["with"]["cache-dependency-path"],
            "platforms/node/package-lock.json\ntools/mermaid-cli/package-lock.json\n",
        )
        self.assertEqual(install["run"], "npm ci --prefix platforms/node")
        self.assertEqual(verify["run"], "npm test --prefix platforms/node")
        self.assertLess(job["steps"].index(install), job["steps"].index(verify))

    def test_ci_proves_binding_catalog_resists_external_timing_unification(self) -> None:
        workflow = parse_workflow_structure(WORKFLOW_ROOT / "ci.yml")
        job = workflow_job(workflow, "build-test")
        binding_step = workflow_step(
            job, name="Test external Cargo feature unification contract"
        )
        ffi_step = workflow_step(job, name="Test C FFI artifact-profile consumer smoke")

        for contract in (
            "cargo nextest run --locked -p merman-bindings-core --no-default-features",
            "--features merman-bindings-core/svg,merman/math,merman/system-timing",
            "artifact_profile_recipe.py apple-uniffi-native",
            'qualified_features="merman-uniffi/${features//,/,merman-uniffi/}"',
            "cargo nextest run --locked -p merman-uniffi -p merman --no-default-features",
            '--features "$qualified_features,merman/system-timing"',
            "test(engine_exposes_metadata)",
        ):
            self.assertIn(contract, binding_step["run"])
        self.assertIn(
            'qualified_features="merman-ffi/${features//,/,merman-ffi/}"',
            ffi_step["run"],
        )
        self.assertIn(
            "cargo nextest run --locked -p merman-ffi -p merman "
            "--no-default-features",
            ffi_step["run"],
        )
        self.assertIn(
            '--features "$qualified_features,merman/system-timing"',
            ffi_step["run"],
        )

    def test_ci_compiles_the_apple_package_with_swift_5_9(self) -> None:
        workflow = parse_workflow_structure(WORKFLOW_ROOT / "ci.yml")
        job = workflow_job(workflow, "apple-swift-5-9-smoke")

        self.assertEqual(job["runs-on"], "macos-14")

        select_step = workflow_step(
            job, name="Select Xcode 15.2 and verify Swift 5.9"
        )
        self.assertIn(
            "sudo xcode-select --switch "
            "/Applications/Xcode_15.2.app/Contents/Developer",
            select_step["run"],
        )
        self.assertIn(
            "grep -Eq 'Apple Swift version 5\\.9",
            select_step["run"],
        )

        toolchain_step = workflow_step(job, name="Install Rust toolchain")
        self.assertEqual(toolchain_step["uses"], "dtolnay/rust-toolchain@1.95.0")
        self.assertEqual(
            toolchain_step["with"]["targets"],
            "aarch64-apple-ios,aarch64-apple-ios-sim,x86_64-apple-ios,"
            "aarch64-apple-darwin,x86_64-apple-darwin",
        )

        build_step = workflow_step(
            job, name="Build Apple XCFramework with Swift 5.9"
        )
        self.assertEqual(
            build_step["env"]["MERMAN_AUTO_INSTALL_RUST_TARGETS"],
            "false",
        )
        self.assertEqual(
            build_step["run"],
            "bash scripts/build-apple-xcframework.sh",
        )

        validation_step = workflow_step(job, name="Validate Swift 5.9 package")
        for contract in (
            "swift package describe",
            "swift build",
            "swiftc -typecheck -module-name Merman",
            "-target arm64-apple-ios14.0",
            "-target arm64-apple-ios14.0-simulator",
            "xcrun --sdk iphoneos --show-sdk-path",
            "xcrun --sdk iphonesimulator --show-sdk-path",
            "-I platforms/apple/Merman.xcframework/ios-arm64/Headers",
            "-I platforms/apple/Merman.xcframework/ios-arm64_x86_64-simulator/Headers",
            "platforms/apple/Sources/Merman/Generated/Merman.swift",
            "swift run --package-path platforms/apple/examples/smoke MermanAppleSmoke",
            "git diff --exit-code -- platforms/apple/Sources/Merman/Generated",
        ):
            self.assertIn(contract, validation_step["run"])
        self.assertNotIn("swift build --triple", validation_step["run"])

        modern_job = workflow_job(workflow, "apple-uniffi-smoke")
        modern_validation_step = workflow_step(
            modern_job, name="Validate Swift package"
        )
        for contract in (
            "swift build --triple arm64-apple-ios14.0",
            "swift build --triple arm64-apple-ios14.0-simulator",
            "xcrun --sdk iphoneos --show-sdk-path",
            "xcrun --sdk iphonesimulator --show-sdk-path",
        ):
            self.assertIn(contract, modern_validation_step["run"])

    def test_ci_pins_cypress_corpus_source_alignment(self) -> None:
        text = read_workflow(WORKFLOW_ROOT / "ci.yml")

        self.assertIn("repository: mermaid-js/mermaid", text)
        self.assertIn("tools/upstreams/MERMAID_REFERENCE_BUNDLE.json", text)
        self.assertIn("ref: ${{ steps.mermaid-source.outputs.commit }}", text)
        self.assertIn("MERMAID_SOURCE_COMMIT: ${{ steps.mermaid-source.outputs.commit }}", text)
        self.assertNotRegex(text, r"ref: [0-9a-f]{40}")
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

    def test_ci_prepares_pinned_generated_artifact_inputs_before_verification(self) -> None:
        workflow_path = WORKFLOW_ROOT / "ci.yml"
        text = read_workflow(workflow_path)
        workflow = parse_workflow_structure(workflow_path)
        job = workflow_job(workflow, "build-test")
        step_names = [
            step["name"]
            for step in job["steps"]
            if isinstance(step, dict) and isinstance(step.get("name"), str)
        ]

        verification_index = step_names.index("Verify generated architecture contracts")
        reference_index = step_names.index(
            "Verify pinned Mermaid reference before runtime install"
        )
        runtime_install_index = step_names.index("Install pinned Mermaid runtime")
        self.assertLess(reference_index, runtime_install_index)
        self.assertLess(
            step_names.index("Setup Node for generated Mermaid artifacts"),
            verification_index,
        )
        self.assertLess(
            runtime_install_index,
            verification_index,
        )
        self.assertLess(
            step_names.index("Checkout pinned DOMPurify source"),
            verification_index,
        )
        self.assertLess(
            step_names.index("Verify pinned DOMPurify source checkout"),
            verification_index,
        )

        self.assertIn('print(bundle["sanitizer"]["source"]["commit"])', text)
        self.assertIn("repository: cure53/DOMPurify", text)
        self.assertIn("ref: ${{ steps.dompurify-source.outputs.commit }}", text)
        self.assertIn("path: repo-ref/dompurify", text)
        self.assertIn("dist/purify.cjs.js", text)
        self.assertIn(
            "DOMPURIFY_SOURCE_COMMIT: ${{ steps.dompurify-source.outputs.commit }}",
            text,
        )


class PagesWorkflowSecurityTests(unittest.TestCase):
    def test_pages_workflow_header_is_read_only(self) -> None:
        workflow = workflow_document(WORKFLOW_ROOT / "pages.yml")
        self.assertEqual(workflow["permissions"], {"contents": "read"})

    def test_pages_build_job_does_not_hold_deploy_permissions(self) -> None:
        build = job_contract(WORKFLOW_ROOT / "pages.yml", "build")
        steps = checkout_steps(build)

        self.assertEqual(build["permissions"], {"contents": "read"})
        self.assertTrue(steps)
        for index, step in enumerate(steps):
            with self.subTest(checkout=index):
                self.assertEqual(step["with"].get("persist-credentials"), "false")

    def test_pages_deploy_job_owns_pages_write_permissions(self) -> None:
        deploy = job_contract(WORKFLOW_ROOT / "pages.yml", "deploy")
        self.assertEqual(
            deploy["permissions"],
            {"contents": "read", "id-token": "write", "pages": "write"},
        )
        self.assertTrue(
            any(
                step.get("uses", "").startswith("actions/deploy-pages@")
                for step in deploy["steps"]
            )
        )


class PerformanceWorkflowSecurityTests(unittest.TestCase):
    def test_performance_head_jobs_do_not_hold_comment_tokens(self) -> None:
        path = WORKFLOW_ROOT / "performance.yml"
        for job_name in ["regression", "frontmatter"]:
            job = job_contract_text(path, job_name)
            with self.subTest(job=job_name):
                self.assertNotIn("issues: write", job)
                self.assertNotIn("pull-requests: write", job)
                self.assertNotIn("GH_TOKEN:", job)
                self.assertNotIn("gh api", job)

    def test_performance_comment_jobs_are_isolated_from_pr_checkout(self) -> None:
        path = WORKFLOW_ROOT / "performance.yml"
        for job_name, artifact in [
            ("regression-comment", "perf-regression"),
            ("frontmatter-comment", "perf-frontmatter"),
        ]:
            job = job_contract_text(path, job_name)
            with self.subTest(job=job_name):
                self.assertIn("issues: write", job)
                self.assertIn("actions/download-artifact", job)
                self.assertIn(f"name: {artifact}", job)
                self.assertIn("GH_TOKEN: ${{ github.token }}", job)
                self.assertIn("gh api", job)
                self.assertNotIn("actions/checkout", job)
                self.assertNotIn("working-directory: head", job)
                self.assertNotIn("tools/bench/", job)

    def test_performance_paths_cover_render_dependencies(self) -> None:
        paths = workflow_document(WORKFLOW_ROOT / "performance.yml")["header"]

        self.assertIn('"Cargo.toml"', paths)
        self.assertIn('"Cargo.lock"', paths)
        self.assertIn('"crates/merman-render/**"', paths)
        self.assertIn('"crates/roughr/**"', paths)

    def test_performance_comment_bodies_are_rendered_before_artifact_upload(self) -> None:
        path = WORKFLOW_ROOT / "performance.yml"
        cases = [
            (
                "regression",
                "Render regression PR comment",
                "Upload regression artifacts",
                "head/target/performance/pr_comment.md",
            ),
            (
                "frontmatter",
                "Render frontmatter PR comment",
                "Upload frontmatter artifacts",
                "head/target/performance/frontmatter_pr_comment.md",
            ),
        ]

        for job_name, render_step, upload_step, comment_path in cases:
            job = job_contract(path, job_name)
            step_names = [step.get("name") for step in job["steps"]]
            upload = workflow_step(job, name=upload_step)
            with self.subTest(job=job_name):
                self.assertLess(
                    step_names.index(render_step),
                    step_names.index(upload_step),
                )
                self.assertIn(comment_path, contract_text(upload))

    def test_performance_checkouts_do_not_persist_credentials(self) -> None:
        steps = checkout_steps(workflow_document(WORKFLOW_ROOT / "performance.yml"))
        self.assertTrue(steps)
        self.assertTrue(
            all(step["with"].get("persist-credentials") == "false" for step in steps)
        )

    def test_performance_run_blocks_do_not_interpolate_dispatch_inputs(self) -> None:
        for index, block in enumerate(
            workflow_run_blocks(WORKFLOW_ROOT / "performance.yml")
        ):
            with self.subTest(run_block=index):
                self.assertNotIn("inputs.", block)
                self.assertNotIn("${{ inputs.", block)

    def test_performance_reference_toolchain_input_is_validated_before_shell_use(self) -> None:
        reference = job_contract(WORKFLOW_ROOT / "performance.yml", "reference")
        install_step = contract_text(
            workflow_step(reference, name="Install mermaid-rs-renderer toolchain")
        )
        comparison_step = contract_text(
            workflow_step(reference, name="Run cross-repo comparison")
        )

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
