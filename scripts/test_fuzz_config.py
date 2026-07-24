#!/usr/bin/env python3
"""Contract tests for the cargo-fuzz workspace and CI matrix."""

from __future__ import annotations

import json
import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
FUZZ_CARGO = ROOT / "fuzz" / "Cargo.toml"
FUZZ_WORKFLOW = ROOT / ".github" / "workflows" / "fuzz.yml"
FUZZING_DOC = ROOT / "docs" / "security" / "FUZZING.md"
FRAMED_FFI_OPTIONS_SEED = ROOT / "fuzz" / "seeds" / "ffi" / "04_framed_render_options.txt"

EXPECTED_ACTION_PINS = {
    "actions/checkout": "3d3c42e5aac5ba805825da76410c181273ba90b1",
    "dtolnay/rust-toolchain": "2c7215f132e9ebf062739d9130488b56d53c060c",
    "Swatinem/rust-cache": "c19371144df3bb44fab255c43d04cbc2ab54d1c4",
    "actions/upload-artifact": "043fb46d1a93c77aae656e7c1c64a875d1fc6a0a",
}


def fuzz_bins() -> dict[str, str]:
    text = FUZZ_CARGO.read_text(encoding="utf-8")
    bins: dict[str, str] = {}
    for block in re.split(r"(?m)^\[\[bin\]\]\s*$", text)[1:]:
        name_match = re.search(r'(?m)^name = "([^"]+)"$', block)
        path_match = re.search(r'(?m)^path = "([^"]+)"$', block)
        if name_match and path_match:
            bins[name_match.group(1)] = path_match.group(1)
    return bins


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


def workflow_event_list(event_name: str, key: str) -> list[str]:
    lines = FUZZ_WORKFLOW.read_text(encoding="utf-8").splitlines()
    event_line = f"  {event_name}:"
    try:
        start = lines.index(event_line) + 1
    except ValueError as exc:
        raise AssertionError(f"fuzz workflow does not define on.{event_name}") from exc

    values: list[str] = []
    in_key = False
    for line in lines[start:]:
        stripped = line.strip()
        indent = len(line) - len(line.lstrip(" "))
        if indent <= 2 and stripped.endswith(":"):
            break
        if indent == 4 and stripped == f"{key}:":
            in_key = True
            continue
        if not in_key:
            continue
        if indent == 6 and stripped.startswith("- "):
            values.append(stripped[2:].strip().strip("\"'"))
            continue
        if stripped == "":
            continue
        if indent <= 4:
            break
    return values


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
    def test_workflow_installs_pinned_nightly_as_toolchain_input(self) -> None:
        install_step = workflow_named_step("Install Rust nightly")

        self.assertIn(
            f"uses: dtolnay/rust-toolchain@{EXPECTED_ACTION_PINS['dtolnay/rust-toolchain']}",
            install_step,
        )
        self.assertIn("toolchain: ${{ env.FUZZ_NIGHTLY }}", install_step)
        self.assertIn("components: rust-src", install_step)
        self.assertNotIn("@master", install_step)
        self.assertNotRegex(install_step, r"uses: dtolnay/rust-toolchain@nightly-\d{4}-\d{2}-\d{2}")

    def test_workflow_uses_reviewed_immutable_action_pins(self) -> None:
        text = FUZZ_WORKFLOW.read_text(encoding="utf-8")
        uses = dict(re.findall(r"(?m)^\s*(?:-\s+)?uses:\s*([^@\s]+)@([^\s#]+)", text))

        self.assertEqual(uses, EXPECTED_ACTION_PINS)
        for action, revision in uses.items():
            with self.subTest(action=action):
                self.assertRegex(revision, r"^[0-9a-f]{40}$")

    def test_push_and_pull_request_remain_smoke_triggers(self) -> None:
        self.assertEqual(workflow_event_list("push", "branches"), ["main"])
        self.assertNotEqual(workflow_event_list("pull_request", "paths"), [])

        plan_step = workflow_named_step("Select bounded target and budget")
        self.assertIn("pull_request|push)", plan_step)
        self.assertIn("profile=smoke", plan_step)
        self.assertIn("schedule)", plan_step)
        self.assertIn("profile=scheduled", plan_step)

    def test_concurrency_keeps_discovery_runs_outside_push_cancellation(self) -> None:
        text = FUZZ_WORKFLOW.read_text(encoding="utf-8")
        self.assertIn("group: fuzz-${{ github.workflow }}-${{ github.event_name }}-", text)
        self.assertIn("github.event.pull_request.number", text)
        self.assertIn("github.ref", text)
        self.assertIn("github.run_id", text)
        self.assertIn(
            "cancel-in-progress: ${{ github.event_name == 'pull_request' || github.event_name == 'push' }}",
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
        self.assertIn("all|parse_mermaid|render_mermaid|svg_pipeline|ffi_api)", run)
        self.assertIn('case "$DISPATCH_PRESET" in', run)
        self.assertIn("smoke|extended|long)", run)
        self.assertIn('case "$selected_targets" in', run)

    def test_budget_and_result_classification_keep_harness_panics_distinct(self) -> None:
        text = FUZZ_WORKFLOW.read_text(encoding="utf-8")
        run_step = workflow_named_step("Run libFuzzer with AddressSanitizer enabled")
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

    def test_framed_ffi_seed_combines_valid_options_and_source(self) -> None:
        data = FRAMED_FFI_OPTIONS_SEED.read_bytes()
        selector, options_len = data[:2]
        options_end = 2 + options_len
        options = data[2:options_end]
        source = data[options_end:]

        self.assertEqual(selector % 18, 4, "seed must select the ABI 3 SVG operation")
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

    def test_fuzzing_doc_lists_all_targets_and_smoke_commands(self) -> None:
        text = FUZZING_DOC.read_text(encoding="utf-8")
        for target in fuzz_bins():
            with self.subTest(target=target):
                self.assertIn(f"| `{target}` |", text)
                self.assertIn(f"fuzz run --fuzz-dir fuzz --sanitizer address {target}", text)
                self.assertIn(f"fuzz/corpus/{target}", text)


if __name__ == "__main__":
    unittest.main()
