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
    current: dict[str, str] | None = None

    for line in lines:
        stripped = line.strip()
        if stripped.startswith("- target: "):
            target = stripped.removeprefix("- target: ").strip('"')
            current = {"target": target}
            targets[target] = current
            continue

        if current is None:
            continue

        if line.startswith("    steps:"):
            break

        match = re.match(r"\s+(seed|dictionary|max_len):\s+(.+)$", line)
        if match:
            key, value = match.groups()
            current[key] = value.strip().strip('"')

    return targets


def workflow_named_step(name: str) -> str:
    lines = FUZZ_WORKFLOW.read_text(encoding="utf-8").splitlines()
    marker = f"      - name: {name}"
    try:
        start = lines.index(marker)
    except ValueError as exc:
        raise AssertionError(f"missing workflow step: {name}") from exc

    end = next(
        (
            index
            for index in range(start + 1, len(lines))
            if lines[index].startswith("      - ")
        ),
        len(lines),
    )
    return "\n".join(lines[start:end])


class FuzzConfigTests(unittest.TestCase):
    def test_workflow_installs_pinned_nightly_as_toolchain_input(self) -> None:
        install_step = workflow_named_step("Install Rust nightly")

        self.assertRegex(
            install_step,
            r"(?m)^        uses: dtolnay/rust-toolchain@(?:master|[0-9a-f]{40})$",
        )
        self.assertIn("toolchain: ${{ env.FUZZ_NIGHTLY }}", install_step)
        self.assertIn("components: rust-src", install_step)
        self.assertNotRegex(
            install_step,
            r"uses: dtolnay/rust-toolchain@nightly-\d{4}-\d{2}-\d{2}",
        )

    def test_workflow_matrix_matches_fuzz_bins(self) -> None:
        self.assertEqual(set(workflow_fuzz_targets()), set(fuzz_bins()))

    def test_framed_ffi_seed_combines_valid_options_and_source(self) -> None:
        data = FRAMED_FFI_OPTIONS_SEED.read_bytes()
        selector, options_len = data[:2]
        options_end = 2 + options_len
        options = data[2:options_end]
        source = data[options_end:]

        self.assertEqual(selector % 18, 4, "seed must select merman_render_svg")
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
