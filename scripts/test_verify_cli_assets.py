#!/usr/bin/env python3
"""Tests for generated merman-cli support-asset validation."""

from __future__ import annotations

from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))

import verify_cli_assets as verifier


def write_asset_tree(root: Path) -> None:
    for relative in verifier.COMPLETION_PATHS.values():
        path = root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text("generated completion\n", encoding="utf-8")
    man = root / "man" / "merman-cli.1"
    man.parent.mkdir(parents=True, exist_ok=True)
    man.write_text(".TH MERMAN-CLI 1\n", encoding="utf-8")


class CliAssetValidationTests(unittest.TestCase):
    def test_required_check_parser_rejects_unknown_ids(self) -> None:
        with self.assertRaisesRegex(Exception, "unknown required check"):
            verifier.parse_required_checks("bash,unknown")

    def test_missing_required_checker_is_an_error(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            write_asset_tree(root)
            with self.assertRaisesRegex(
                verifier.AssetValidationError,
                "required bash parser",
            ):
                verifier.verify_assets(
                    root,
                    required={"bash"},
                    finder=lambda _name: None,
                )

    def test_unavailable_optional_checkers_are_reported_as_skipped(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            write_asset_tree(root)
            checked, skipped = verifier.verify_assets(
                root,
                finder=lambda _name: None,
            )
            self.assertEqual(checked, frozenset())
            self.assertEqual(skipped, frozenset(verifier.CHECK_IDS))

    def test_every_native_parser_and_bash_routing_are_invoked(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            write_asset_tree(root)
            commands: list[list[str]] = []

            def runner(
                command: list[str],
                **_kwargs: object,
            ) -> subprocess.CompletedProcess[str]:
                commands.append(command)
                if "-c" in command:
                    stdout = "--format\n__MMDC__\n-e\n"
                elif "utf8" in command:
                    stdout = "MERMAN-CLI(1)\n"
                else:
                    stdout = ""
                return subprocess.CompletedProcess(command, 0, stdout, "")

            checked, skipped = verifier.verify_assets(
                root,
                required=verifier.CHECK_IDS,
                finder=lambda name: f"/tools/{name}",
                runner=runner,
            )
            self.assertEqual(checked, frozenset(verifier.CHECK_IDS))
            self.assertEqual(skipped, frozenset())
            self.assertEqual(len(commands), 8)
            elvish = next(command for command in commands if "-compileonly" in command)
            self.assertEqual(elvish[1:3], ["-compileonly", "-c"])
            self.assertEqual(elvish[3], "use edit\ngenerated completion\n")
            self.assertTrue(any("--no-execute" in command for command in commands))
            self.assertTrue(any("-NoProfile" in command for command in commands))
            self.assertTrue(any("-T" in command and "lint" in command for command in commands))
            self.assertTrue(any("-T" in command and "utf8" in command for command in commands))

    def test_parser_failure_includes_native_diagnostics(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            write_asset_tree(root)

            def runner(
                command: list[str],
                **_kwargs: object,
            ) -> subprocess.CompletedProcess[str]:
                return subprocess.CompletedProcess(command, 2, "", "syntax error")

            with self.assertRaisesRegex(
                verifier.AssetValidationError,
                "syntax error",
            ):
                verifier.verify_assets(
                    root,
                    required={"zsh"},
                    finder=lambda name: "/tools/zsh" if name == "zsh" else None,
                    runner=runner,
                )


if __name__ == "__main__":
    unittest.main()
