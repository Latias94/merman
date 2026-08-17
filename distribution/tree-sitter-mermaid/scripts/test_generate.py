"""Focused tests for the cross-platform generation entry point."""

from __future__ import annotations

import os
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from scripts.generate import (
    AUTHORED_SOURCE_FILES,
    GENERATED_SOURCE_FILES,
    cli_command,
    compare_files,
    exact_source_set_failures,
    generate_sources,
    run,
    validate_cli_version,
)


def write_source_set(root: Path, marker: bytes = b"same") -> None:
    for relative in (*GENERATED_SOURCE_FILES, *AUTHORED_SOURCE_FILES):
        path = root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        contents = b"#define LANGUAGE_VERSION 15\n" if path.name == "parser.c" else marker
        path.write_bytes(contents)


class GenerationTests(unittest.TestCase):
    def test_cli_command_uses_package_local_wrapper(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            package = Path(directory)
            wrapper = package / "node_modules/tree-sitter-cli/cli.js"
            wrapper.parent.mkdir(parents=True)
            wrapper.write_text("wrapper", encoding="utf-8")
            with patch.dict(
                os.environ,
                {"TREE_SITTER_MERMAID_NODE": "C:/Program Files/nodejs/node.exe"},
            ):
                self.assertEqual(
                    cli_command(package),
                    ["C:/Program Files/nodejs/node.exe", str(wrapper)],
                )

    def test_cli_version_is_exact(self) -> None:
        with self.assertRaisesRegex(SystemExit, "expected tree-sitter 0.26.12"):
            validate_cli_version("tree-sitter 0.26.11")

    def test_generated_source_set_rejects_stale_files(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            write_source_set(root)
            self.assertEqual(exact_source_set_failures(root), [])
            (root / "src/obsolete.c").write_bytes(b"stale")
            self.assertIn("src artifact set mismatch", exact_source_set_failures(root)[0])

    def test_comparison_reports_stale_generated_file(self) -> None:
        with tempfile.TemporaryDirectory() as left_dir, tempfile.TemporaryDirectory() as right_dir:
            left = Path(left_dir)
            right = Path(right_dir)
            write_source_set(left)
            write_source_set(right)
            (right / "src/node-types.json").write_bytes(b"stale")
            self.assertEqual(
                compare_files(left, right, GENERATED_SOURCE_FILES),
                ["generated artifact is stale: src/node-types.json"],
            )

    def test_generation_explicitly_selects_abi_15(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            destination = Path(directory)
            with patch("scripts.generate.copy_generation_inputs") as copy_inputs, patch(
                "scripts.generate.run"
            ) as run_command:
                write_source_set(destination)
                generate_sources(Path("package"), ["node", "tree-sitter"], destination)

            copy_inputs.assert_called_once()
            command = run_command.call_args.args[0]
            self.assertIn("--abi", command)
            self.assertEqual(command[command.index("--abi") + 1], "15")

    def test_external_tool_timeout_has_a_bounded_failure(self) -> None:
        timeout = subprocess.TimeoutExpired(["tree-sitter", "build"], 900)
        with patch("scripts.generate.subprocess.run", side_effect=timeout):
            with self.assertRaisesRegex(SystemExit, "command timed out after 900 seconds"):
                run(["tree-sitter", "build"], cwd=Path("package"))


if __name__ == "__main__":
    unittest.main()
