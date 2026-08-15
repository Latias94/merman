#!/usr/bin/env python3
"""Tests for the exact merman-cli process-level feature matrix."""

from __future__ import annotations

import io
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import tomllib
import unittest
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))

import verify_cli_process_matrix as matrix


EXPECTED_SELECTIONS = (
    ("base", (), "exact"),
    ("analysis", ("analysis",), "exact"),
    ("svg", ("svg",), "exact"),
    ("ascii", ("ascii",), "exact"),
    ("local-icons", ("icons",), "exact"),
    ("markdown", ("markdown",), "exact"),
    ("parallel-markdown", ("parallel-markdown",), "exact"),
    ("network-icons", ("network-icons",), "exact"),
    ("png", ("png",), "exact"),
    ("jpeg", ("jpeg",), "exact"),
    ("pdf", ("pdf",), "exact"),
    ("parallel-pdf", ("parallel-markdown", "pdf"), "exact"),
    ("cytoscape-layout", ("layout-cytoscape",), "exact"),
    ("elk-layout", ("layout-elk",), "exact"),
    ("math", ("math",), "exact"),
    ("rustdoc", ("rustdoc",), "exact"),
    ("completions", ("shell-completions",), "exact"),
    ("svg-completions", ("shell-completions", "svg"), "exact"),
    ("system-clock", ("system-clock",), "exact"),
    ("system-timezone", ("system-timezone",), "exact"),
    ("system-random", ("system-random",), "exact"),
    ("system-timing", ("system-timing",), "exact"),
    ("default", (), "default"),
    ("release", (), "all"),
)


def selection_projection(profile: matrix.ProfileCase) -> tuple[object, ...]:
    return (
        profile.case_id,
        profile.features,
        (
            "all"
            if profile.use_all_features
            else "default"
            if profile.use_default_features
            else "exact"
        ),
    )


def expected_command(selection: tuple[object, ...]) -> list[str]:
    case_id, raw_features, mode = selection
    del case_id
    features = tuple(raw_features)
    command = ["cargo", "nextest", "run", "-p", "merman-cli"]
    if mode == "all":
        command.append("--all-features")
    elif mode == "exact":
        command.append("--no-default-features")
        if features:
            command.extend(["--features", ",".join(features)])
    command.extend(["--test", "profile_contract"])
    return command


class CliProcessMatrixTests(unittest.TestCase):
    def test_matrix_matches_the_complete_selection_table(self) -> None:
        self.assertTupleEqual(
            tuple(selection_projection(profile) for profile in matrix.PROFILE_CASES),
            EXPECTED_SELECTIONS,
        )
        self.assertEqual(
            len({profile.case_id for profile in matrix.PROFILE_CASES}),
            len(matrix.PROFILE_CASES),
        )
        for profile in matrix.PROFILE_CASES:
            with self.subTest(profile=profile.case_id):
                self.assertTrue(profile.name)
                self.assertTrue(profile.workflow)

    def test_cli_defaults_are_all_public_features(self) -> None:
        cargo_toml = tomllib.loads(
            (matrix.REPO_ROOT / "crates/merman-cli/Cargo.toml").read_text(
                encoding="utf-8"
            )
        )
        features = cargo_toml["features"]
        self.assertSetEqual(
            set(features["default"]),
            set(features) - {"default"},
            "workspace default tests only replace a separate all-features run "
            "while every public CLI feature is enabled by default",
        )

    def test_unlocked_commands_project_every_selection_exactly(self) -> None:
        self.assertListEqual(
            [
                matrix.cargo_nextest_args(profile)
                for profile in matrix.PROFILE_CASES
            ],
            [expected_command(selection) for selection in EXPECTED_SELECTIONS],
        )

    def test_locked_is_forwarded_once_to_every_cargo_invocation(self) -> None:
        for profile in matrix.PROFILE_CASES:
            with self.subTest(profile=profile.case_id):
                command = matrix.cargo_nextest_args(profile, locked=True)
                self.assertEqual(command.count("--locked"), 1)
                self.assertEqual(command[:4], ["cargo", "nextest", "run", "--locked"])

    def test_runner_receives_repository_cwd_and_an_isolated_case_environment(
        self,
    ) -> None:
        calls: list[tuple[list[str], dict[str, object]]] = []
        source_environment = {"PATH": "synthetic", "CALLER_VALUE": "preserved"}

        def runner(command: list[str], **kwargs: object) -> subprocess.CompletedProcess:
            calls.append((command, kwargs))
            return subprocess.CompletedProcess(command, 0)

        with tempfile.TemporaryDirectory() as temp_dir:
            repo_root = Path(temp_dir)
            matrix.run_process_matrix(
                locked=True,
                repo_root=repo_root,
                environment=source_environment,
                runner=runner,
            )

        self.assertEqual(len(calls), len(matrix.PROFILE_CASES))
        self.assertDictEqual(
            source_environment,
            {"PATH": "synthetic", "CALLER_VALUE": "preserved"},
        )
        for (command, kwargs), profile in zip(
            calls, matrix.PROFILE_CASES, strict=True
        ):
            with self.subTest(profile=profile.case_id):
                self.assertEqual(
                    command,
                    matrix.cargo_nextest_args(profile, locked=True),
                )
                self.assertEqual(kwargs["cwd"], repo_root)
                self.assertEqual(kwargs["timeout"], matrix.CASE_TIMEOUT_SECONDS)
                self.assertDictEqual(
                    kwargs["env"],
                    {
                        "PATH": "synthetic",
                        "CALLER_VALUE": "preserved",
                        matrix.PROFILE_CASE_ENV: profile.case_id,
                    },
                )
                self.assertIsNot(kwargs["env"], source_environment)

    def test_first_nonzero_case_stops_the_matrix_and_reports_its_identity(self) -> None:
        calls: list[str] = []

        def runner(command: list[str], **_kwargs: object) -> subprocess.CompletedProcess:
            case_id = matrix.PROFILE_CASES[len(calls)].case_id
            calls.append(case_id)
            return subprocess.CompletedProcess(
                command,
                73 if case_id == "svg" else 0,
            )

        with self.assertRaises(matrix.ProcessMatrixError) as raised:
            matrix.run_process_matrix(
                environment={},
                runner=runner,
            )

        self.assertEqual(calls, ["base", "analysis", "svg"])
        self.assertEqual(raised.exception.profile.case_id, "svg")
        self.assertEqual(raised.exception.returncode, 73)
        self.assertIn("'SVG'", str(raised.exception))
        self.assertIn("status 73", str(raised.exception))

    def test_process_start_failure_is_attributed_to_the_current_case(self) -> None:
        profile = matrix.PROFILE_CASES[0]

        def runner(
            _command: list[str], **_kwargs: object
        ) -> subprocess.CompletedProcess:
            raise FileNotFoundError("cargo")

        with self.assertRaises(matrix.ProcessMatrixError) as raised:
            matrix.run_process_matrix(
                environment={},
                runner=runner,
                profiles=(profile,),
            )

        self.assertIs(raised.exception.profile, profile)
        self.assertIsNone(raised.exception.returncode)
        self.assertIn("could not start Cargo", str(raised.exception))

    def test_timeout_is_attributed_to_the_current_case_and_stops_the_matrix(self) -> None:
        profile = matrix.PROFILE_CASES[1]

        def runner(
            command: list[str], **kwargs: object
        ) -> subprocess.CompletedProcess:
            raise subprocess.TimeoutExpired(command, kwargs["timeout"])

        with self.assertRaises(matrix.ProcessMatrixError) as raised:
            matrix.run_process_matrix(
                environment={},
                runner=runner,
                profiles=(profile,),
            )

        self.assertIs(raised.exception.profile, profile)
        self.assertIsNone(raised.exception.returncode)
        self.assertIn("timed out", str(raised.exception))
        self.assertIn(str(matrix.CASE_TIMEOUT_SECONDS), str(raised.exception))

    @unittest.skipUnless(os.name == "posix", "process-group assertion is POSIX-specific")
    def test_timeout_runner_creates_and_terminates_an_isolated_process_group(self) -> None:
        command = [
            sys.executable,
            "-c",
            "import subprocess, sys, time; subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(60)']); time.sleep(60)",
        ]

        with self.assertRaises(subprocess.TimeoutExpired):
            matrix.run_case_subprocess(
                command,
                cwd=Path.cwd(),
                env=os.environ,
                timeout=0.05,
            )

    @unittest.skipUnless(os.name == "posix", "process-group assertion is POSIX-specific")
    def test_parent_exit_does_not_skip_forced_process_group_cleanup(self) -> None:
        process = mock.Mock(pid=73)
        process.poll.return_value = 0
        with (
            mock.patch.object(matrix, "TERMINATION_GRACE_SECONDS", 0.0),
            mock.patch.object(matrix.os, "killpg") as kill_group,
        ):
            matrix._terminate_process_tree(process)

        self.assertEqual(
            kill_group.call_args_list,
            [
                mock.call(73, signal.SIGTERM),
                mock.call(73, signal.SIGKILL),
            ],
        )
        process.wait.assert_called_once_with(timeout=0.0)

    def test_windows_cleanup_targets_the_complete_process_tree(self) -> None:
        process = mock.Mock(pid=91)
        process.poll.return_value = 0
        with mock.patch.object(matrix.subprocess, "run") as run:
            matrix._terminate_windows_process_tree(process)

        run.assert_called_once_with(
            ["taskkill", "/PID", "91", "/T", "/F"],
            check=False,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            timeout=matrix.TERMINATION_GRACE_SECONDS,
        )

    def test_main_preserves_a_valid_failing_exit_code_and_prints_the_case(
        self,
    ) -> None:
        failure = matrix.ProcessMatrixError(
            matrix.PROFILE_CASES[1],
            "nextest exited with status 17",
            returncode=17,
        )
        stderr = io.StringIO()
        with (
            mock.patch.object(matrix, "run_process_matrix", side_effect=failure) as run,
            mock.patch.object(sys, "stderr", stderr),
        ):
            self.assertEqual(matrix.main(["--locked"]), 17)

        run.assert_called_once_with(locked=True)
        self.assertIn("Analysis", stderr.getvalue())
        self.assertIn("status 17", stderr.getvalue())

    def test_main_maps_start_failures_and_invalid_exit_codes_to_one(self) -> None:
        failures = (
            matrix.ProcessMatrixError(matrix.PROFILE_CASES[0], "start failed"),
            matrix.ProcessMatrixError(
                matrix.PROFILE_CASES[0],
                "invalid status",
                returncode=999,
            ),
        )
        for failure in failures:
            with (
                self.subTest(failure=failure),
                mock.patch.object(
                    matrix,
                    "run_process_matrix",
                    side_effect=failure,
                ),
                mock.patch.object(sys, "stderr", io.StringIO()),
            ):
                self.assertEqual(matrix.main([]), 1)


if __name__ == "__main__":
    unittest.main()
