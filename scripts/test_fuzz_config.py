#!/usr/bin/env python3
"""Contract tests for the cargo-fuzz workspace and CI matrix."""

from __future__ import annotations

import json
import os
import re
import shutil
import subprocess
import tempfile
import textwrap
import tomllib
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
FUZZ_CARGO = ROOT / "fuzz" / "Cargo.toml"
FUZZ_WORKFLOW = ROOT / ".github" / "workflows" / "fuzz.yml"
FRAMED_FFI_OPTIONS_SEED = ROOT / "fuzz" / "seeds" / "ffi" / "04_framed_render_options.txt"
TREE_SITTER_FAMILY_SEEDS = (
    ROOT
    / "distribution"
    / "tree-sitter-mermaid"
    / "fuzz"
    / "corpus"
    / "all-families"
)
def fuzz_bins() -> dict[str, str]:
    with FUZZ_CARGO.open("rb") as manifest:
        return {entry["name"]: entry["path"] for entry in tomllib.load(manifest)["bin"]}


def workflow_fuzz_targets() -> dict[str, dict[str, str]]:
    lines = FUZZ_WORKFLOW.read_text(encoding="utf-8").splitlines()
    targets: dict[str, dict[str, str]] = {}

    for line in lines:
        match = re.match(r"\s+entry='(\{.+\})'$", line)
        if not match:
            continue
        entry = json.loads(match.group(1))
        targets[entry["target"]] = entry

    return targets


def workflow_named_step(name: str) -> str:
    lines = FUZZ_WORKFLOW.read_text(encoding="utf-8").splitlines()
    starts = [index for index, line in enumerate(lines) if line.startswith("      - ")]
    for position, start in enumerate(starts):
        end = starts[position + 1] if position + 1 < len(starts) else len(lines)
        step = "\n".join(lines[start:end])
        if f"name: {name}" in step:
            return step
    raise AssertionError(f"missing workflow step: {name}")


def workflow_dispatch_choice_options(input_name: str) -> list[str]:
    lines = FUZZ_WORKFLOW.read_text(encoding="utf-8").splitlines()
    marker = f"      {input_name}:"
    try:
        start = lines.index(marker) + 1
    except ValueError as exc:
        raise AssertionError(f"fuzz workflow does not define workflow_dispatch.{input_name}") from exc

    values: list[str] = []
    in_options = False
    for line in lines[start:]:
        stripped = line.strip()
        indent = len(line) - len(line.lstrip(" "))
        if indent <= 6 and stripped.endswith(":"):
            break
        if indent == 8 and stripped == "options:":
            in_options = True
            continue
        if not in_options:
            continue
        if indent == 10 and stripped.startswith("- "):
            values.append(stripped[2:].strip().strip("\"'"))
            continue
        if stripped == "":
            continue
        if indent <= 8:
            break
    return values


class FuzzConfigTests(unittest.TestCase):
    def test_ci_caller_selects_regressions_without_inherited_dispatch_inputs(self) -> None:
        from scripts.test_release_workflow_security import workflow_job

        workflow = FUZZ_WORKFLOW.read_text(encoding="utf-8")
        call = workflow.split("  workflow_call:\n", 1)[1].split("\npermissions:", 1)[0]
        self.assertIn("ci_only:", call)
        self.assertIn("type: boolean", call)
        self.assertIn("default: false", call)
        ci = (ROOT / ".github/workflows/ci.yml").read_text(encoding="utf-8")
        self.assertIn("ci_only: true", workflow_job(ci, "fuzz-owner"))
        step = workflow_named_step("Select bounded target and budget")
        self.assertIn("FUZZ_EVENT_NAME: ${{ inputs.ci_only && 'workflow_call' || github.event_name }}", step)

    @unittest.skipUnless(os.name == "posix" and shutil.which("bash"), "requires a native POSIX shell")
    def test_plan_shell_preserves_ci_schedule_and_manual_budgets(self) -> None:
        from scripts.test_release_workflow_security import workflow_job

        job = workflow_job(FUZZ_WORKFLOW.read_text(encoding="utf-8"), "resolve-fuzz-plan")
        script = textwrap.dedent(job.split("        run: |\n", 1)[1])
        cases = (
            ("workflow_call", "", "", "regression", set(fuzz_bins())),
            ("schedule", "", "", "scheduled", set(fuzz_bins())),
            ("workflow_dispatch", "parse_mermaid", "smoke", "smoke", {"parse_mermaid"}),
        )
        for event, target, preset, profile, targets in cases:
            with self.subTest(event=event), tempfile.TemporaryDirectory() as temporary:
                output = Path(temporary) / "outputs"
                env = {**os.environ, "FUZZ_EVENT_NAME": event, "DISPATCH_TARGET": target,
                       "DISPATCH_PRESET": preset, "GITHUB_OUTPUT": str(output)}
                result = subprocess.run(["bash", "-euo", "pipefail", "-c", script], env=env,
                                        capture_output=True, text=True, timeout=10)
                self.assertEqual(result.returncode, 0, result.stderr)
                values = dict(line.split("=", 1) for line in output.read_text().splitlines())
                self.assertEqual(values["profile"], profile)
                self.assertEqual({row["target"] for row in json.loads(values["matrix"])["include"]}, targets)

    def test_workflow_installs_declared_nightly_with_rustup(self) -> None:
        install_step = workflow_named_step("Install Rust nightly")

        self.assertIn(
            'rustup toolchain install "$FUZZ_NIGHTLY" --profile minimal --component rust-src',
            install_step,
        )
        self.assertNotIn("uses:", install_step)
        self.assertNotIn("rustup default", install_step)

    def test_pr_calls_run_fixed_regressions_and_schedules_run_discovery(self) -> None:
        text = FUZZ_WORKFLOW.read_text(encoding="utf-8")
        self.assertNotIn("  pull_request:", text)
        self.assertNotIn("  push:", text)
        plan_step = workflow_named_step("Select bounded target and budget")
        self.assertIn("pull_request|push|merge_group|workflow_call)", plan_step)
        self.assertIn("profile=regression", plan_step)
        self.assertIn("schedule)", plan_step)
        self.assertIn("profile=scheduled", plan_step)

        build_step = workflow_named_step("Build libFuzzer harness")
        regression_step = workflow_named_step("Run committed regression inputs")
        randomized_step = workflow_named_step("Run bounded randomized libFuzzer discovery")
        self.assertIn("fuzz build", build_step)
        self.assertIn("fuzz/corpus/$FUZZ_TARGET", regression_step)
        self.assertIn("${inputs[@]}", regression_step)
        self.assertIn("profile != 'regression'", randomized_step)

    def test_concurrency_keeps_discovery_runs_outside_push_cancellation(self) -> None:
        text = FUZZ_WORKFLOW.read_text(encoding="utf-8")
        self.assertIn(
            "group: fuzz-${{ github.workflow }}-${{ github.event_name }}-",
            text,
        )
        self.assertIn("github.event.pull_request.number", text)
        self.assertIn("github.run_id", text)
        self.assertIn(
            "cancel-in-progress: ${{ github.event_name == 'pull_request' || github.event_name == 'merge_group' }}",
            text,
        )

    def test_manual_inputs_are_enumerated_and_never_interpolated_into_shell(self) -> None:
        targets = list(fuzz_bins())
        self.assertEqual(workflow_dispatch_choice_options("target"), ["all", *targets])
        self.assertEqual(workflow_dispatch_choice_options("preset"), ["smoke", "extended", "long"])

        plan_step = workflow_named_step("Select bounded target and budget")
        self.assertIn("DISPATCH_TARGET: ${{ inputs.target }}", plan_step)
        self.assertIn("DISPATCH_PRESET: ${{ inputs.preset }}", plan_step)
        run = plan_step.split("        run: |\n", maxsplit=1)[1]
        self.assertNotIn("${{ inputs.", run)
        self.assertNotIn("github.event.inputs", run)
        self.assertIn('case "$DISPATCH_TARGET" in', run)
        self.assertIn(
            "all|parse_mermaid|render_mermaid|svg_pipeline|ffi_api|"
            "tree_sitter_mermaid_parse|tree_sitter_mermaid_edits|"
            "tree_sitter_mermaid_scanner)",
            run,
        )
        self.assertIn('case "$DISPATCH_PRESET" in', run)
        self.assertIn("smoke|extended|long)", run)
        self.assertIn('case "$selected_targets" in', run)

    def test_budget_and_result_classification_keep_harness_panics_distinct(self) -> None:
        text = FUZZ_WORKFLOW.read_text(encoding="utf-8")
        run_step = workflow_named_step("Run bounded randomized libFuzzer discovery")
        classify_step = workflow_named_step("Classify fuzz result")

        self.assertIn("-runs=64", run_step)
        self.assertIn("extended)", run_step)
        self.assertIn("-max_total_time=300", run_step)
        self.assertIn("scheduled|long)", run_step)
        self.assertIn("-max_total_time=900", run_step)
        self.assertIn("AddressSanitizer|UndefinedBehaviorSanitizer", classify_step)
        self.assertIn("non-sanitizer Rust or harness panic", classify_step)
        self.assertIn("fuzz/logs/**", text)
        self.assertNotIn("name: libFuzzer ASan", text)

    def test_workflow_matrix_matches_fuzz_bins(self) -> None:
        self.assertEqual(set(workflow_fuzz_targets()), set(fuzz_bins()))

    def test_tree_sitter_parse_lane_uses_all_family_seeds(self) -> None:
        targets = workflow_fuzz_targets()
        parse_entry = targets["tree_sitter_mermaid_parse"]

        self.assertEqual(
            parse_entry["seed"],
            "distribution/tree-sitter-mermaid/fuzz/corpus/all-families",
        )

    def test_tree_sitter_family_fuzz_corpus_covers_every_public_family(self) -> None:
        seeds = list(TREE_SITTER_FAMILY_SEEDS.glob("*.mmd"))
        self.assertEqual(len(seeds), 35)
        self.assertTrue(all(seed.stat().st_size > 0 for seed in seeds))

    def test_framed_ffi_seed_combines_valid_options_and_source(self) -> None:
        data = FRAMED_FFI_OPTIONS_SEED.read_bytes()
        selector, options_len = data[:2]
        options_end = 2 + options_len
        options = data[2:options_end]
        source = data[options_end:]

        self.assertEqual(selector % 19, 4, "seed must select the ABI 3 SVG operation")
        self.assertIsInstance(json.loads(options), dict)
        self.assertTrue(source.startswith(b"flowchart TD\n"))

    def test_fuzz_bin_paths_exist(self) -> None:
        for target, relative_path in fuzz_bins().items():
            with self.subTest(target=target):
                self.assertTrue((ROOT / "fuzz" / relative_path).is_file())

    def test_workflow_seed_and_dictionary_paths_exist(self) -> None:
        for target, entry in workflow_fuzz_targets().items():
            with self.subTest(target=target):
                seed = ROOT / entry["seed"]
                dictionary = ROOT / entry["dictionary"]

                self.assertTrue(seed.is_dir(), f"missing seed directory: {seed}")
                self.assertNotEqual(list(seed.iterdir()), [], f"empty seed directory: {seed}")
                self.assertTrue(dictionary.is_file(), f"missing dictionary: {dictionary}")
                self.assertIn("max_len", entry)


if __name__ == "__main__":
    unittest.main()
