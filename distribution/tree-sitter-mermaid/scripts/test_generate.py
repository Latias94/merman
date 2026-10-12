"""Focused tests for the cross-platform generation entry point."""

from __future__ import annotations

import json
import os
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from scripts.generate import (
    AUTHORED_SOURCE_FILES,
    GENERATED_SOURCE_FILES,
    PACKAGE_ROOT,
    cli_command,
    compare_files,
    exact_source_set_failures,
    generate_sources,
    run,
    resolve_wasi_clang,
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
        validate_cli_version("tree-sitter 0.27.1")
        with self.assertRaisesRegex(SystemExit, "expected tree-sitter 0.27.1"):
            validate_cli_version("tree-sitter 0.26.12")

    def test_checked_in_parser_records_grammar_version(self) -> None:
        version = json.loads((PACKAGE_ROOT / "tree-sitter.json").read_text(encoding="utf-8"))[
            "metadata"
        ]["version"]
        parser = (PACKAGE_ROOT / "src/parser.c").read_text(encoding="utf-8")
        metadata = {line for line in parser.splitlines() if "_version = " in line}
        for field, value in zip(("major", "minor", "patch"), version.split("."), strict=True):
            self.assertIn(f"      .{field}_version = {value},", metadata)

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

    def test_pinned_wasi_sdk_accepts_platform_clang_banners(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            sdk = Path(directory)
            clang = sdk / "bin" / "clang.exe"
            (sdk / "VERSION").write_text("34.0\n", encoding="utf-8")
            for host, banner in [
                ("win32", "23.1.0-rc3"),
                ("linux", "23.1.0-wasi-sdk"),
                ("darwin", "23.1.0-wasi-sdk"),
            ]:
                with self.subTest(host=host), patch(
                    "scripts.generate.sys.platform", host
                ), patch("scripts.generate.find_wasi_clang", return_value=clang), patch(
                    "scripts.generate.subprocess.run",
                    return_value=subprocess.CompletedProcess(
                        [], 0, f"clang version {banner} (https://github.com/llvm/llvm-project "
                        "895aa2c896ada719451be2e3673c83da8ddf1141)\n"
                    ),
                ):
                    self.assertEqual(resolve_wasi_clang([]), clang)
            (sdk / "VERSION").write_text("28.0\n", encoding="utf-8")
            with patch("scripts.generate.find_wasi_clang", return_value=clang), patch(
                "scripts.generate.subprocess.run",
                return_value=subprocess.CompletedProcess([], 0, "clang version 23.1.0-rc3"),
            ):
                with self.assertRaisesRegex(SystemExit, "expected wasi-sdk 34.0"):
                    resolve_wasi_clang([])

    def test_pinned_wasi_sdk_rejects_unmatched_compiler_identity(self) -> None:
        commit = "895aa2c896ada719451be2e3673c83da8ddf1141"
        with tempfile.TemporaryDirectory() as directory:
            sdk = Path(directory)
            clang = sdk / "bin" / "clang"
            (sdk / "VERSION").write_text("34.0\n", encoding="utf-8")
            for banner in [
                f"clang version 23.1.0-rc3 (https://github.com/llvm/llvm-project {commit})",
                "clang version 23.1.0-wasi-sdk (https://github.com/llvm/llvm-project wrong)",
                f"clang version 23.1.0 (https://github.com/llvm/llvm-project {commit})",
                f"clang version 24.0.0-wasi-sdk (https://github.com/llvm/llvm-project {commit})",
                "clang version 23.1.0-wasi-sdk",
            ]:
                with self.subTest(banner=banner), patch(
                    "scripts.generate.sys.platform", "linux"
                ), patch("scripts.generate.find_wasi_clang", return_value=clang), patch(
                    "scripts.generate.subprocess.run",
                    return_value=subprocess.CompletedProcess([], 0, banner),
                ):
                    with self.assertRaisesRegex(SystemExit, "expected clang version"):
                        resolve_wasi_clang([])

    def test_external_tool_timeout_has_a_bounded_failure(self) -> None:
        timeout = subprocess.TimeoutExpired(["tree-sitter", "build"], 900)
        with patch("scripts.generate.subprocess.run", side_effect=timeout):
            with self.assertRaisesRegex(SystemExit, "command timed out after 900 seconds"):
                run(["tree-sitter", "build"], cwd=Path("package"))


if __name__ == "__main__":
    unittest.main()
