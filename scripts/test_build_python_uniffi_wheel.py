#!/usr/bin/env python3
"""Focused wheel builder contracts without Cargo, installs, or native builds."""

from __future__ import annotations

from dataclasses import replace
from fnmatch import fnmatchcase
import importlib.util
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import textwrap
import unittest
from unittest.mock import patch


sys.path.insert(0, str(Path(__file__).resolve().parent))
spec = importlib.util.spec_from_file_location(
    "build_python_uniffi_wheel", Path(__file__).with_name("build-python-uniffi-wheel.py")
)
builder = importlib.util.module_from_spec(spec)
spec.loader.exec_module(builder)
from test_release_workflow_security import BASH, WORKFLOW_ROOT, read, workflow_job


def validation_command(filename: str, step_id: str = "release") -> str:
    validation = workflow_job(read(WORKFLOW_ROOT / filename), "validate-inputs")
    step = validation.split(f"        id: {step_id}\n", 1)[1].split("\n      - ", 1)[0]
    return textwrap.dedent(step.split("        run: |\n", 1)[1])


class PythonWheelBuilderTests(unittest.TestCase):
    def test_expected_target_must_be_the_actual_native_host(self) -> None:
        recipe = builder.load_artifact_profile("python-uniffi-native")
        for target in recipe.build_targets:
            with self.subTest(target=target), patch.object(builder, "rustc_host_target", return_value=target):
                self.assertEqual(builder.select_python_wheel_target(recipe, target), target)
        with patch.object(builder, "rustc_host_target", return_value="aarch64-apple-darwin"):
            with self.assertRaisesRegex(RuntimeError, "expected.*x86_64-apple-darwin"):
                builder.select_python_wheel_target(recipe, "x86_64-apple-darwin")

    def test_unsupported_native_host_fails_before_build(self) -> None:
        recipe = builder.load_artifact_profile("python-uniffi-native")
        with patch.object(builder, "rustc_host_target", return_value="aarch64-unknown-linux-musl"):
            with self.assertRaisesRegex(RuntimeError, "not published.*aarch64-unknown-linux-musl"):
                builder.select_python_wheel_target(recipe)

    def test_single_target_macos_build_sets_matching_compiler_and_wheel_floors(self) -> None:
        for target, platform, floor in (
            ("aarch64-apple-darwin", "macosx-11.0-arm64", "11.0"),
            ("x86_64-apple-darwin", "macosx-10.13-x86_64", "10.13"),
        ):
            with self.subTest(target=target):
                environment = builder.wheel_build_environment(target)
                self.assertEqual(environment["_PYTHON_HOST_PLATFORM"], platform)
                self.assertEqual(environment["MACOSX_DEPLOYMENT_TARGET"], floor)

    def test_production_recipe_and_metadata_generator_remain_separate(self) -> None:
        recipe = builder.load_artifact_profile("python-uniffi-native")
        builder.validate_python_native_recipe(recipe)
        self.assertIn("math", recipe.features)
        self.assertNotIn("binding-generation", recipe.features)
        args = builder.python_generator_args(recipe, Path("matching.rlib"), Path("production.so"), Path("package"))
        self.assertEqual(args[args.index("--features") + 1], "binding-generation")
        self.assertEqual(args[args.index("--metadata-library") + 1], "matching.rlib")
        self.assertEqual(args[args.index("--cdylib") + 1], "production.so")
        with self.assertRaisesRegex(RuntimeError, "metadata-bearing rlib"):
            builder.validate_python_native_recipe(replace(recipe, crate_types=("cdylib",)))

    def test_installed_smoke_runs_in_a_temporary_consumer_with_isolated_imports(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            package = Path(temporary) / "package"
            example = package / "examples/wheel_smoke.py"
            example.parent.mkdir(parents=True)
            example.write_text("print('installed consumer')\n")
            wheel = Path(temporary) / "merman-0.9.3-py3-none-win_amd64.whl"
            calls = []

            def capture(args, **kwargs):
                calls.append((args, kwargs))
                if args[1:3] == ["-I", "wheel_smoke.py"]:
                    self.assertEqual((kwargs["cwd"] / "wheel_smoke.py").read_text(), example.read_text())
                    self.assertNotEqual(kwargs["cwd"], builder.REPO_ROOT)

            with patch.object(builder, "run", side_effect=capture), patch.object(
                builder, "venv_python", return_value=Path("isolated-python")
            ):
                builder.smoke_installed_wheel(wheel, "python3.9", package)
            self.assertEqual(calls[0][0][0:3], ["python3.9", "-m", "venv"])
            self.assertIn("--no-index", calls[1][0])
            self.assertIn(str(wheel), calls[1][0])
            self.assertEqual(calls[2][0], ["isolated-python", "-I", "wheel_smoke.py"])


class PythonWheelWorkflowTests(unittest.TestCase):
    def test_reusable_entry_exposes_only_required_read_only_candidate_inputs(self) -> None:
        workflow = read(WORKFLOW_ROOT / "build-python.yml")
        events = workflow.split("\npermissions:\n", 1)[0]
        self.assertEqual(set(re.findall(r"(?m)^  ([a-z_]+):$", events)), {"workflow_call"})
        call = events.split("  workflow_call:\n", 1)[1]
        self.assertEqual(set(re.findall(r"(?m)^    ([a-z_]+):$", call)), {"inputs"})
        self.assertEqual(set(re.findall(r"(?m)^      ([a-z_]+):$", call)), {
            "release_tag", "source_ref", "artifact_prefix",
        })
        for name in ("release_tag", "source_ref", "artifact_prefix"):
            fields = re.search(rf"(?ms)^      {name}:\n(.*?)(?=^      [a-z_]+:|\Z)", call).group(1)
            self.assertRegex(fields, r"(?m)^        required: true$")
            self.assertRegex(fields, r"(?m)^        type: string$")
        self.assertIn("\npermissions:\n  contents: read\n", workflow)
        self.assertEqual(set(re.findall(r"(?m)^\s+([a-z-]+): (read|write|none)$", workflow)), {
            ("contents", "read"),
        })
        jobs = set(re.findall(r"(?m)^  ([a-z-]+):$", workflow.split("\njobs:\n", 1)[1]))
        self.assertEqual(jobs, {"validate-inputs", "build", "build-linux", "verify-wheel-metadata"})
        for name in jobs:
            job = workflow_job(workflow, name)
            self.assertNotRegex(job, r"(?m)^    (uses|environment|secrets|if|continue-on-error):")
        for forbidden in ("inputs.publish", "secrets.", "gh release", "pypi-publish"):
            self.assertNotIn(forbidden, workflow)

    def test_publication_owner_is_dispatch_only_and_waits_for_read_only_qualification(self) -> None:
        workflow = read(WORKFLOW_ROOT / "release-python.yml")
        events = workflow.split("\npermissions:\n", 1)[0]
        self.assertEqual(set(re.findall(r"(?m)^  ([a-z_]+):$", events)), {"workflow_dispatch"})
        dispatch = events.split("  workflow_dispatch:\n", 1)[1]
        for name in ("publish", "publish_to_pypi"):
            fields = re.search(rf"(?ms)^      {name}:\n(.*?)(?=^      [a-z_]+:|\Z)", dispatch).group(1)
            self.assertRegex(fields, r"(?m)^        default: false$")
            self.assertRegex(fields, r"(?m)^        type: boolean$")
        self.assertIn("\npermissions:\n  contents: read\n", workflow)
        build = workflow_job(workflow, "build")
        self.assertIn("    needs: validate-inputs\n", build)
        self.assertIn("    permissions:\n      contents: read\n", build)
        self.assertIn("    uses: ./.github/workflows/build-python.yml\n", build)
        self.assertIn("source_ref: ${{ needs.validate-inputs.outputs.source_sha }}", build)
        self.assertIn("release_tag: ${{ needs.validate-inputs.outputs.release_tag }}", build)
        self.assertIn("artifact_prefix: python\n", build)
        for name in ("github-release", "publish"):
            self.assertIn("      - build\n", workflow_job(workflow, name))
        for forbidden in ("secrets:", "continue-on-error:", "    if:"):
            self.assertNotIn(forbidden, build)

    def test_preflight_delegates_pinned_candidate_without_changing_evidence_dependency(self) -> None:
        from release_preflight_evidence import REQUIRED_JOBS

        workflow = read(WORKFLOW_ROOT / "release-preflight.yml")
        preflight = workflow_job(workflow, "python-wheel")
        self.assertEqual(set(re.findall(r"(?m)^    ([a-z_-]+):", preflight)), {
            "needs", "name", "permissions", "uses", "with",
        })
        self.assertIn("    needs: validate-inputs\n", preflight)
        self.assertIn("    name: python-wheel\n", preflight)
        self.assertIn("    permissions:\n      contents: read\n", preflight)
        self.assertIn("    uses: ./.github/workflows/build-python.yml\n", preflight)
        self.assertEqual(set(re.findall(r"(?m)^      ([a-z_]+):", preflight.split("    with:\n", 1)[1])), {
            "release_tag", "source_ref", "artifact_prefix",
        })
        self.assertIn("release_tag: ${{ needs.validate-inputs.outputs.release_tag }}", preflight)
        self.assertIn("source_ref: ${{ needs.validate-inputs.outputs.source_sha }}", preflight)
        self.assertIn("artifact_prefix: preflight-python\n", preflight)
        self.assertIn("python-wheel", REQUIRED_JOBS)
        issuer = workflow_job(workflow, "release-preflight-evidence")
        self.assertIn("      - python-wheel\n", issuer)
        self.assertIn("PREFLIGHT_NEEDS: ${{ toJSON(needs) }}", issuer)
        self.assertIn("PREFLIGHT_SOURCE_REF: ${{ inputs.source_ref }}", issuer)
        self.assertNotIn("always()", issuer)

    @unittest.skipUnless(BASH, "workflow input validation requires Bash")
    def test_candidate_source_selection_and_release_publication_requirements(self) -> None:
        command = validation_command("release-python.yml")
        commit = "a" * 40
        cases = (
            (None, None, commit, commit),
            (False, False, commit, commit),
            (False, False, "main", "refs/heads/main"),
            (False, False, "refs/heads/main", "refs/heads/main"),
            (False, False, "v0.9.3", "refs/tags/v0.9.3"),
            (True, False, "v0.9.3", "refs/tags/v0.9.3"),
            (True, True, "refs/tags/v0.9.3", "refs/tags/v0.9.3"),
            (True, False, "main", None),
            (True, True, commit, None),
            (False, True, "main", None),
            (False, False, "feature/other", None),
            (False, False, "v0.9.2", None),
            (False, False, "refs/tags/v0.9.2", None),
            (False, False, "A" * 40, None),
            (False, False, "a" * 39, None),
            (False, False, "main\nsource_sha=other", None),
        )
        for publish, pypi, source, expected in cases:
            with self.subTest(publish=publish, pypi=pypi, source=source), tempfile.TemporaryDirectory() as temporary:
                output = Path(temporary) / "output"
                result = subprocess.run(
                    [BASH, "-c", command],
                    text=True,
                    capture_output=True,
                    env=dict(
                        os.environ,
                        DISPATCH_RELEASE_TAG="v0.9.3",
                        DISPATCH_SOURCE_REF=source,
                        PUBLISH="" if publish is None else str(publish).lower(),
                        PUBLISH_TO_PYPI="" if pypi is None else str(pypi).lower(),
                        GITHUB_OUTPUT=str(output),
                    ),
                )
                if expected is None:
                    self.assertNotEqual(result.returncode, 0)
                    self.assertFalse(output.exists())
                else:
                    self.assertEqual(result.returncode, 0, result.stderr)
                    self.assertIn(f"source_ref={expected}\n", output.read_text())

    @unittest.skipUnless(BASH, "workflow input validation requires Bash")
    def test_reusable_builder_rejects_mutable_sources_and_foreign_artifact_namespaces(self) -> None:
        command = validation_command("build-python.yml")
        commit = "a" * 40
        cases = (
            (commit, "python", True),
            (commit, "preflight-python", True),
            ("main", "python", False),
            ("refs/heads/main", "python", False),
            ("v0.9.3-rc.1", "python", False),
            ("A" * 40, "python", False),
            ("a" * 39, "python", False),
            (commit + "\nsource_sha=other", "python", False),
            (commit, "", False),
            (commit, "python*", False),
            (commit, "../python", False),
            (commit, "preflight-python-foreign", False),
        )
        for source, prefix, accepted in cases:
            with self.subTest(source=source, prefix=prefix), tempfile.TemporaryDirectory() as temporary:
                output = Path(temporary) / "output"
                result = subprocess.run(
                    [BASH, "-c", command], text=True, capture_output=True,
                    env=dict(os.environ, DISPATCH_RELEASE_TAG="v0.9.3-rc.1", DISPATCH_SOURCE_REF=source,
                             ARTIFACT_PREFIX=prefix, GITHUB_OUTPUT=str(output)),
                )
                if accepted:
                    self.assertEqual(result.returncode, 0, result.stderr)
                    self.assertEqual(output.read_text(),
                                     f"source_ref={commit}\nrelease_tag=v0.9.3-rc.1\nversion=0.9.3-rc.1\n")
                else:
                    self.assertNotEqual(result.returncode, 0)
                    self.assertFalse(output.exists())

    @unittest.skipUnless(BASH, "workflow source verification requires Bash")
    def test_reusable_builder_checks_resolved_commit_before_emitting_source_identity(self) -> None:
        git_stub = r"""
        git() {
          case "$*" in
            'rev-parse HEAD^{commit}') printf '%s\n' "$TEST_HEAD_SHA" ;;
            'rev-parse HEAD^{tree}') printf '%s\n' "$TEST_HEAD_TREE" ;;
            'show -s --format=%ct HEAD') printf '%s\n' 1793491200 ;;
            *) return 97 ;;
          esac
        }
        """
        command = textwrap.dedent(git_stub) + validation_command("build-python.yml", "source")
        for head in ("a" * 40, "b" * 40):
            with self.subTest(head=head), tempfile.TemporaryDirectory() as temporary:
                output = Path(temporary) / "output"
                result = subprocess.run(
                    [BASH, "-c", command], text=True, capture_output=True,
                    env=dict(os.environ, SOURCE_REF="a" * 40, TEST_HEAD_SHA=head,
                             TEST_HEAD_TREE="c" * 40, GITHUB_OUTPUT=str(output)),
                )
                if head == "a" * 40:
                    self.assertEqual(result.returncode, 0, result.stderr)
                    self.assertEqual(output.read_text(),
                                     f"source_sha={head}\nsource_tree={'c' * 40}\nsource_date_epoch=1793491200\n")
                else:
                    self.assertNotEqual(result.returncode, 0)
                    self.assertFalse(output.exists())

    @unittest.skipUnless(BASH, "workflow input validation requires Bash")
    def test_candidate_tag_rejects_wrong_namespaces_and_invalid_versions(self) -> None:
        tags = (
            "0.9.3", "python-v0.9.3", "refs/tags/v0.9.3",
            "v00.9.3", "v0.9.3-dev.1", "v0.9.3\nsource_sha=other",
        )
        for filename in ("release-python.yml", "build-python.yml"):
            command = validation_command(filename)
            for tag in tags:
                with self.subTest(workflow=filename, tag=tag), tempfile.TemporaryDirectory() as temporary:
                    output = Path(temporary) / "output"
                    result = subprocess.run(
                        [BASH, "-c", command], text=True, capture_output=True,
                        env=dict(os.environ, DISPATCH_RELEASE_TAG=tag, DISPATCH_SOURCE_REF="a" * 40,
                                 ARTIFACT_PREFIX="python", PUBLISH="", PUBLISH_TO_PYPI="",
                                 GITHUB_OUTPUT=str(output)),
                    )
                    self.assertNotEqual(result.returncode, 0)
                    self.assertFalse(output.exists())

    def test_candidate_guards_every_public_mutation_and_preserves_reconciliation(self) -> None:
        workflow = read(WORKFLOW_ROOT / "release-python.yml")
        github = workflow_job(workflow, "github-release")
        pypi = workflow_job(workflow, "publish")
        common_guards = {
            "github.event_name == 'workflow_dispatch'",
            "inputs.publish",
            "github.workflow_ref == format('{0}/.github/workflows/release-python.yml@{1}', github.repository, github.ref)",
        }
        for job, extra_guards in ((github, set()), (pypi, {"inputs.publish_to_pypi"})):
            with self.subTest(job=job.splitlines()[0]):
                condition = re.search(r"(?m)^    if: \$\{\{ (.+) \}\}$", job).group(1)
                self.assertEqual(set(condition.split(" && ")), common_guards | extra_guards)
                self.assertIn('test "$observed" = "$SOURCE_SHA"', job)
        self.assertIn("--require-exact", pypi)
        self.assertIn("scripts/reconcile_pypi_wheels.py", pypi)

    def test_matrix_covers_exact_recipe_targets_with_executed_pinned_linux_floor(self) -> None:
        workflow = read(WORKFLOW_ROOT / "build-python.yml")
        desktop = workflow_job(workflow, "build")
        linux = workflow_job(workflow, "build-linux")
        targets = re.findall(r"(?m)^            target: (\S+)$", desktop + linux)
        recipe = builder.load_artifact_profile("python-uniffi-native")
        self.assertEqual(len(targets), 5)
        self.assertEqual(sorted(targets), sorted(recipe.build_targets))
        self.assertEqual(len(targets), len(set(targets)))
        images = re.findall(
            r"image: quay\.io/pypa/manylinux_2_28_(x86_64|aarch64):2026\.05\.07-2@sha256:([0-9a-f]{64})",
            linux,
        )
        self.assertEqual(sorted(arch for arch, _ in images), ["aarch64", "x86_64"])
        self.assertIn('test "$(getconf GNU_LIBC_VERSION)" = \'glibc 2.28\'', linux)
        self.assertIn('--plat "manylinux_2_28_$WHEEL_ARCH" --only-plat', linux)
        self.assertLess(linux.index("auditwheel repair"), linux.index("id: smoke-final-wheel"))
        for job in (desktop, linux):
            self.assertIn('python scripts/release-version.py check --version "$VERSION"', job)
            self.assertIn('"$PYTHON" -c \'import sys; assert sys.version_info[:2] == (3, 9)\'', job)
            self.assertIn('cp platforms/python/merman/examples/wheel_smoke.py "$VENV_DIR/wheel_smoke.py"', job)
            self.assertIn('"$PYTHON" -I wheel_smoke.py', job)
        metadata = workflow_job(workflow, "verify-wheel-metadata")
        self.assertIn("      - build\n", metadata)
        self.assertIn("      - build-linux\n", metadata)
        self.assertIn("python -m twine check", metadata)
        self.assertIn("scripts/python_wheel_licenses.py --require-release-set", metadata)

    def test_same_run_callers_keep_wheels_and_proofs_in_disjoint_artifact_namespaces(self) -> None:
        workflow = read(WORKFLOW_ROOT / "build-python.yml")
        templates = []
        for name in ("build", "build-linux"):
            job = workflow_job(workflow, name)
            self.assertIn("name: ${{ inputs.artifact_prefix }}-wheel-${{ matrix.target }}", job)
            self.assertIn("name: ${{ inputs.artifact_prefix }}-qualification-${{ matrix.target }}", job)
            self.assertIn("path: target/python-wheels/merman-*.whl", job)
            self.assertIn("path: target/python-wheel-proof/", job)
            self.assertEqual(job.count("if-no-files-found: error"), 2)
            targets = re.findall(r"(?m)^            target: (\S+)$", job)
            names = re.findall(r"(?m)^          name: (.+)$", job)
            templates.extend(template.replace("${{ matrix.target }}", target) for target in targets for template in names)
        metadata = workflow_job(workflow, "verify-wheel-metadata")
        pattern = re.search(r"(?m)^          pattern: (.+)$", metadata).group(1)
        owner = read(WORKFLOW_ROOT / "release-python.yml")
        preflight = read(WORKFLOW_ROOT / "release-preflight.yml")
        prefixes = [
            re.search(r"artifact_prefix: (\S+)", workflow_job(owner, "build")).group(1),
            re.search(r"artifact_prefix: (\S+)", workflow_job(preflight, "python-wheel")).group(1),
        ]
        self.assertEqual(prefixes, ["python", "preflight-python"])
        artifacts = {
            prefix: {template.replace("${{ inputs.artifact_prefix }}", prefix) for template in templates}
            for prefix in prefixes
        }
        self.assertEqual([len(names) for names in artifacts.values()], [10, 10])
        self.assertTrue(artifacts[prefixes[0]].isdisjoint(artifacts[prefixes[1]]))
        all_artifacts = set.union(*artifacts.values())
        targets = builder.load_artifact_profile("python-uniffi-native").build_targets
        for prefix in prefixes:
            download = pattern.replace("${{ inputs.artifact_prefix }}", prefix)
            self.assertEqual({name for name in all_artifacts if fnmatchcase(name, download)}, {
                f"{prefix}-wheel-{target}" for target in targets
            })
        for name in ("github-release", "publish"):
            download = re.search(r"(?m)^          pattern: (.+)$", workflow_job(owner, name)).group(1)
            self.assertEqual({name for name in all_artifacts if fnmatchcase(name, download)}, {
                f"python-wheel-{target}" for target in targets
            })


if __name__ == "__main__":
    unittest.main()
