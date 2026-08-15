#!/usr/bin/env python3
"""Tests for the checked merman-cli support-asset generator."""

from __future__ import annotations

from pathlib import Path
import subprocess
import sys
import unittest
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))

import generate_cli_assets as generator


class CliAssetGenerationTests(unittest.TestCase):
    def test_cargo_test_rejects_an_undiscovered_filtered_test(self) -> None:
        completed = subprocess.CompletedProcess([], 0, "unrelated::test: test\n", "")
        with (
            mock.patch.object(generator, "cargo_feature_args", return_value=[]),
            mock.patch.object(generator.subprocess, "run", return_value=completed) as run,
            self.assertRaisesRegex(RuntimeError, "did not discover required CLI asset test"),
        ):
            generator.cargo_test(
                generator.CHECK_TEST,
                ignored=False,
                environment={},
            )

        run.assert_called_once()
        self.assertIn("--list", run.call_args.args[0])

    def test_cargo_test_executes_the_discovered_test(self) -> None:
        discovered = subprocess.CompletedProcess(
            [],
            0,
            f"{generator.CHECK_TEST}: test\n",
            "",
        )
        executed = subprocess.CompletedProcess([], 0, "", "")
        with (
            mock.patch.object(generator, "cargo_feature_args", return_value=[]),
            mock.patch.object(
                generator.subprocess,
                "run",
                side_effect=[discovered, executed],
            ) as run,
        ):
            generator.cargo_test(
                generator.CHECK_TEST,
                ignored=False,
                environment={},
            )

        self.assertEqual(run.call_count, 2)
        self.assertIn("--list", run.call_args_list[0].args[0])
        self.assertIn("--exact", run.call_args_list[1].args[0])

    def test_ignored_writer_is_filtered_during_discovery_and_execution(self) -> None:
        discovered = subprocess.CompletedProcess(
            [],
            0,
            f"{generator.WRITE_TEST}: test\n",
            "",
        )
        executed = subprocess.CompletedProcess([], 0, "", "")
        with (
            mock.patch.object(generator, "cargo_feature_args", return_value=[]),
            mock.patch.object(
                generator.subprocess,
                "run",
                side_effect=[discovered, executed],
            ) as run,
        ):
            generator.cargo_test(
                generator.WRITE_TEST,
                ignored=True,
                environment={},
            )

        self.assertIn("--ignored", run.call_args_list[0].args[0])
        self.assertIn("--ignored", run.call_args_list[1].args[0])


if __name__ == "__main__":
    unittest.main()
