"""Negative tests for deterministic generation boundaries."""

from __future__ import annotations

import os
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from scripts.generate import (
    AUTHORED_SOURCE_FILES,
    C_BINDING_FILES,
    C_HEADER,
    GENERATED_ARTIFACTS,
    assert_exact_generated_set,
    cli_command,
    compare_sets,
    install_artifacts_transactionally,
    package_artifact_set_failures,
    receipt_input_drift,
    snapshot_receipt_inputs,
    validate_cli_version,
)


def write_artifacts(root: Path, marker: bytes = b"same") -> None:
    for relative in (
        *GENERATED_ARTIFACTS,
        *AUTHORED_SOURCE_FILES,
        *C_BINDING_FILES,
    ):
        path = root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(marker)


class GenerationBoundaryTests(unittest.TestCase):
    def test_cli_command_uses_the_package_wrapper_through_node(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            package = Path(directory)
            wrapper = package / "node_modules/tree-sitter-cli/cli.js"
            wrapper.parent.mkdir(parents=True)
            wrapper.write_text("wrapper")
            with patch.dict(
                os.environ,
                {"TREE_SITTER_MERMAID_NODE": "C:/Program Files/nodejs/node.exe"},
            ):
                self.assertEqual(
                    cli_command(package),
                    ["C:/Program Files/nodejs/node.exe", str(wrapper)],
                )

    def test_mixed_generation_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as left_dir, tempfile.TemporaryDirectory() as right_dir:
            left = Path(left_dir)
            right = Path(right_dir)
            write_artifacts(left)
            write_artifacts(right)
            (right / "src/parser.c").write_bytes(b"another generation")
            self.assertEqual(
                compare_sets(left, right, "mixed"),
                ["mixed: differs src/parser.c"],
            )

    def test_extra_generated_paths_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            package = Path(directory)
            write_artifacts(package)
            (package / "src/obsolete.c").write_bytes(b"stale")
            failures = assert_exact_generated_set(package)
            self.assertEqual(len(failures), 1)
            self.assertIn("src artifact set mismatch", failures[0])

    def test_extra_c_binding_paths_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            package = Path(directory)
            write_artifacts(package)
            (package / "bindings/c/tree_sitter/obsolete.h").write_bytes(b"stale")
            failures = package_artifact_set_failures(
                package, require_complete=True
            )
            self.assertEqual(
                failures,
                [
                    "unexpected package artifacts: "
                    "['bindings/c/tree_sitter/obsolete.h']"
                ],
            )

    def test_stale_file_blocks_write_before_any_replacement(self) -> None:
        with tempfile.TemporaryDirectory() as package_dir, tempfile.TemporaryDirectory() as generated_dir:
            package = Path(package_dir)
            generated = Path(generated_dir)
            write_artifacts(package, b"committed")
            write_artifacts(generated, b"generated")
            stale = package / "src/obsolete.c"
            stale.write_bytes(b"stale")
            before = {
                relative: (package / relative).read_bytes()
                for relative in GENERATED_ARTIFACTS
            }

            with self.assertRaisesRegex(
                RuntimeError, "unexpected package artifacts.*src/obsolete.c"
            ):
                install_artifacts_transactionally(
                    generated,
                    package,
                    b"generated receipt",
                    {C_HEADER: b"generated header"},
                )

            after = {
                relative: (package / relative).read_bytes()
                for relative in GENERATED_ARTIFACTS
            }
            self.assertEqual(after, before)
            self.assertEqual(stale.read_bytes(), b"stale")

    def test_cli_version_must_match_exactly(self) -> None:
        with self.assertRaisesRegex(SystemExit, "expected tree-sitter 0.26.12"):
            validate_cli_version("tree-sitter 0.26.11")

    def test_receipt_input_drift_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            package = Path(directory)
            grammar = package / "grammar.js"
            grammar.write_bytes(b"before")
            with patch(
                "scripts.generate.receipt_inputs",
                return_value=[grammar],
            ):
                expected = snapshot_receipt_inputs(package)
                grammar.write_bytes(b"after")
                self.assertEqual(
                    receipt_input_drift(package, expected),
                    ["generation inputs changed while artifacts were being generated"],
                )

    def test_transactional_install_rolls_back_every_replaced_file(self) -> None:
        with tempfile.TemporaryDirectory() as package_dir, tempfile.TemporaryDirectory() as generated_dir:
            package = Path(package_dir)
            generated = Path(generated_dir)
            write_artifacts(package, b"committed")
            write_artifacts(generated, b"generated")
            receipt = package / "metadata/artifact-receipt.json"
            receipt.parent.mkdir(parents=True)
            receipt.write_bytes(b"committed receipt")
            header = package / C_HEADER
            header.parent.mkdir(parents=True, exist_ok=True)
            header.write_bytes(b"committed header")
            before = {
                relative: (package / relative).read_bytes()
                for relative in (
                    *GENERATED_ARTIFACTS,
                    Path("metadata/artifact-receipt.json"),
                    C_HEADER,
                )
            }

            real_replace = os.replace
            calls = 0

            def fail_once(source: Path, destination: Path) -> None:
                nonlocal calls
                calls += 1
                if calls == 4:
                    raise OSError("injected replacement failure")
                real_replace(source, destination)

            with patch("scripts.generate.os.replace", side_effect=fail_once):
                with self.assertRaisesRegex(OSError, "injected replacement failure"):
                    install_artifacts_transactionally(
                        generated,
                        package,
                        b"generated receipt",
                        {C_HEADER: b"generated header"},
                    )

            after = {
                relative: (package / relative).read_bytes()
                for relative in (
                    *GENERATED_ARTIFACTS,
                    Path("metadata/artifact-receipt.json"),
                    C_HEADER,
                )
            }
            self.assertEqual(after, before)

    def test_post_install_validation_failure_rolls_back_every_file(self) -> None:
        with tempfile.TemporaryDirectory() as package_dir, tempfile.TemporaryDirectory() as generated_dir:
            package = Path(package_dir)
            generated = Path(generated_dir)
            write_artifacts(package, b"committed")
            write_artifacts(generated, b"generated")
            receipt = package / "metadata/artifact-receipt.json"
            receipt.parent.mkdir(parents=True)
            receipt.write_bytes(b"committed receipt")
            before = {
                relative: (package / relative).read_bytes()
                for relative in (
                    *GENERATED_ARTIFACTS,
                    Path("metadata/artifact-receipt.json"),
                    C_HEADER,
                )
            }

            with self.assertRaisesRegex(RuntimeError, "injected validation failure"):
                install_artifacts_transactionally(
                    generated,
                    package,
                    b"generated receipt",
                    {C_HEADER: b"generated header"},
                    lambda _: ["injected validation failure"],
                )

            after = {
                relative: (package / relative).read_bytes()
                for relative in (
                    *GENERATED_ARTIFACTS,
                    Path("metadata/artifact-receipt.json"),
                    C_HEADER,
                )
            }
            self.assertEqual(after, before)


if __name__ == "__main__":
    unittest.main()
