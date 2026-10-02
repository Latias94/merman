from __future__ import annotations

import copy
import sys
import tempfile
import time
import unittest
from pathlib import Path
from unittest import mock

from tools.bench import run_layout_work_calibration as calibration


class LayoutWorkCalibrationRunnerTests(unittest.TestCase):
    def test_output_directory_must_be_empty(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            (output / "stale.json").write_text("{}", encoding="utf-8")

            with self.assertRaisesRegex(RuntimeError, "must be empty"):
                calibration.prepare_output_directory(output)

    @unittest.skipUnless(sys.platform.startswith(("darwin", "linux", "win32")), "supported hosts only")
    def test_timeout_terminates_the_managed_process_group(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            started = root / "grandchild-started"
            orphan = root / "grandchild-survived"
            grandchild = (
                "import signal, time; from pathlib import Path; "
                "signal.signal(signal.SIGTERM, signal.SIG_IGN); "
                f"Path({str(started)!r}).write_text('started'); "
                "time.sleep(1.5); "
                f"Path({str(orphan)!r}).write_text('orphan'); "
                "time.sleep(60)"
            )
            parent = (
                "import subprocess, sys, time; "
                f"subprocess.Popen([sys.executable, '-c', {grandchild!r}]); "
                "time.sleep(60)"
            )

            result = calibration.run_managed_process(
                [sys.executable, "-c", parent],
                cwd=calibration.ROOT,
                timeout_seconds=1.0,
                termination_grace_seconds=0.1,
            )

            self.assertTrue(result["timed_out"])
            self.assertTrue(started.is_file(), "grandchild never started")
            time.sleep(1.7)
            self.assertFalse(orphan.exists(), "grandchild survived the timeout")

    @unittest.skipUnless(sys.platform.startswith(("darwin", "linux", "win32")), "supported hosts only")
    def test_timeout_terminates_descendants_after_group_leader_exits(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            started = root / "grandchild-started"
            orphan = root / "grandchild-survived"
            grandchild = (
                "import signal, time; from pathlib import Path; "
                "signal.signal(signal.SIGTERM, signal.SIG_IGN); "
                f"Path({str(started)!r}).write_text('started'); "
                "time.sleep(1.5); "
                f"Path({str(orphan)!r}).write_text('orphan'); "
                "time.sleep(60)"
            )
            parent = (
                "import subprocess, sys; "
                f"subprocess.Popen([sys.executable, '-c', {grandchild!r}])"
            )

            result = calibration.run_managed_process(
                [sys.executable, "-c", parent],
                cwd=calibration.ROOT,
                timeout_seconds=1.0,
                termination_grace_seconds=0.1,
            )

            self.assertTrue(result["timed_out"])
            self.assertEqual(result["returncode"], 0, "group leader did not exit first")
            self.assertTrue(started.is_file(), "grandchild never started")
            time.sleep(1.7)
            self.assertFalse(orphan.exists(), "grandchild survived the timeout")

    @unittest.skipUnless(sys.platform.startswith(("darwin", "linux", "win32")), "supported hosts only")
    def test_timeout_terminates_descendants_that_close_output_pipes(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            started = root / "grandchild-started"
            orphan = root / "grandchild-survived"
            grandchild = (
                "import os, signal, time; from pathlib import Path; "
                "sink = os.open(os.devnull, os.O_WRONLY); "
                "os.dup2(sink, 1); os.dup2(sink, 2); os.close(sink); "
                "signal.signal(signal.SIGTERM, signal.SIG_IGN); "
                f"Path({str(started)!r}).write_text('started'); "
                "time.sleep(1.5); "
                f"Path({str(orphan)!r}).write_text('orphan'); "
                "time.sleep(60)"
            )
            parent = (
                "import subprocess, sys, time; "
                f"subprocess.Popen([sys.executable, '-c', {grandchild!r}]); "
                "time.sleep(60)"
            )

            result = calibration.run_managed_process(
                [sys.executable, "-c", parent],
                cwd=calibration.ROOT,
                timeout_seconds=1.0,
                termination_grace_seconds=0.1,
            )

            self.assertTrue(result["timed_out"])
            self.assertTrue(started.is_file(), "grandchild never started")
            time.sleep(1.7)
            self.assertFalse(orphan.exists(), "grandchild survived the timeout")


    @unittest.skipUnless(sys.platform == "win32", "Windows only")
    def test_windows_reads_peak_working_set_after_target_releases_memory(self) -> None:
        result = calibration.run_managed_process(
            [sys.executable, "-c", "memory = bytearray(64 * 1024 * 1024); del memory; print('done')"],
            cwd=calibration.ROOT,
            timeout_seconds=10,
        )
        self.assertEqual(result["returncode"], 0)
        self.assertFalse(result["timed_out"])
        self.assertEqual(result["stdout"].strip(), "done")
        self.assertGreaterEqual(result["maximum_resident_set_size_bytes"], 64 * 1024 * 1024)

    @unittest.skipUnless(sys.platform == "win32", "Windows only")
    def test_windows_preserves_target_exit_code_and_stderr(self) -> None:
        result = calibration.run_managed_process(
            [sys.executable, "-c", "import sys; print('failed', file=sys.stderr); sys.exit(7)"],
            cwd=calibration.ROOT,
            timeout_seconds=10,
        )
        self.assertEqual(result["returncode"], 7)
        self.assertFalse(result["timed_out"])
        self.assertEqual(result["stderr"].strip(), "failed")

    @unittest.skipUnless(sys.platform == "win32", "Windows only")
    def test_windows_launch_gate_prevents_execution_when_job_assignment_fails(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            marker = Path(directory) / "must-not-run"
            command = [
                sys.executable, "-c",
                f"from pathlib import Path; Path({str(marker)!r}).write_text('ran')",
            ]
            with mock.patch.object(calibration.WindowsJob, "assign", side_effect=OSError("assignment failed")):
                with self.assertRaisesRegex(OSError, "assignment failed"):
                    calibration.run_managed_process(
                        command, cwd=calibration.ROOT, timeout_seconds=10,
                    )
            self.assertFalse(marker.exists())

    @unittest.skipUnless(sys.platform == "win32", "Windows only")
    def test_successful_windows_leader_cannot_leave_background_descendants(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            started = root / "grandchild-started"
            orphan = root / "grandchild-survived"
            grandchild = (
                "import time; from pathlib import Path; "
                f"Path({str(started)!r}).write_text('started'); "
                "time.sleep(1.0); "
                f"Path({str(orphan)!r}).write_text('orphan'); "
                "time.sleep(60)"
            )
            parent = (
                "import subprocess, sys, time; from pathlib import Path; "
                f"subprocess.Popen([sys.executable, '-c', {grandchild!r}], "
                "stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL); "
                f"marker = Path({str(started)!r}); "
                "\nwhile not marker.exists(): time.sleep(0.01)"
            )
            result = calibration.run_managed_process(
                [sys.executable, "-c", parent], cwd=calibration.ROOT,
                timeout_seconds=10,
            )
            self.assertEqual(result["returncode"], 0)
            self.assertFalse(result["timed_out"])
            self.assertTrue(started.is_file(), "grandchild never started")
            time.sleep(1.2)
            self.assertFalse(orphan.exists(), "grandchild survived successful completion")

    def test_windows_host_metadata_does_not_require_uname_command(self) -> None:
        with mock.patch.object(calibration.platform, "system", return_value="Windows"):
            with mock.patch.object(calibration, "command_output", side_effect=AssertionError("unexpected command")):
                self.assertEqual(calibration.host_report()["system"], "Windows")

    def test_windows_timing_uses_explicit_working_set_metric(self) -> None:
        with mock.patch.object(calibration.platform, "system", return_value="Windows"):
            command, metric = calibration.timing_command(Path("calibration.exe"), ["--theme", "dark"])
        self.assertEqual(command, ["calibration.exe", "--theme", "dark"])
        self.assertEqual(metric, "windows-peak-working-set")

    def test_arguments_select_profile_theme_and_candidate_limit(self) -> None:
        with mock.patch.object(sys, "argv", [
            "runner", "--authoritative-date", "2026-10-01",
            "--resource-profile", "trusted-native", "--theme", "dark",
            "--layout-work-limit", "15000000",
        ]):
            args = calibration.parse_arguments()
        self.assertEqual(args.resource_profile, "trusted-native")
        self.assertEqual(args.theme, "dark")
        self.assertEqual(args.layout_work_limit, 15_000_000)
        self.assertEqual(args.full_repeats, 5)

    def test_fewer_than_five_fresh_full_reports_are_rejected(self) -> None:
        with mock.patch.object(sys, "argv", [
            "runner", "--authoritative-date", "2026-10-01", "--full-repeats", "4",
        ]):
            with self.assertRaisesRegex(RuntimeError, "at least five"):
                calibration.main()

    def test_full_report_checks_selected_policy_theme_and_fixture_acceptance(self) -> None:
        full = {
            "schema_version": 2, "theme": "dark",
            "policy": {"profile": "trusted-native", "max_layout_work_units": 15_000_000},
            "fixture_corpus": {"fixtures": [{"name": "nested_class", "profile_accepted": True}]},
            "cardinality_boundary": {
                "scan_start_nodes": 1, "scanned_through_nodes": 3,
                "first_rejected_nodes": 3, "accepted_prefix_count": 2,
                "accepted": {"nodes": 2}, "rejected": {"nodes": 3},
                "accepted_prefix_digest_encoding": "repeated u64-le(nodes) || u64-le(layout_work_units)",
                "accepted_prefix_observations_sha256": "a" * 64,
            },
        }
        arguments = {
            "resource_profile": "trusted-native", "theme": "dark", "layout_work_limit": 15_000_000,
        }
        calibration.validate_full_report_contract(full, **arguments)
        for field, value in (("theme", "default"), ("resource_profile", "interactive"), ("layout_work_limit", 1)):
            with self.subTest(field=field):
                with self.assertRaises(RuntimeError):
                    calibration.validate_full_report_contract(full, **{**arguments, field: value})
        full["fixture_corpus"]["fixtures"][0]["profile_accepted"] = False
        with self.assertRaisesRegex(RuntimeError, "profile_accepted"):
            calibration.validate_full_report_contract(full, **arguments)

    def test_semantic_probe_uses_the_maximum_fixtures_actual_diagram_type(self) -> None:
        policy = {
            "profile": "trusted-native", "max_layout_work_units": 15_000_000,
            "explicit_overrides": [{"id": "max_layout_work_units", "value": 15_000_000}],
        }
        fixture = {
            "name": "nested_class", "source_path": "class.mmd",
            "source_sha256": "a" * 64, "source_bytes": 100,
        }
        full = {
            "schema_version": 2, "theme": "dark", "policy": policy,
            "fixture_corpus": {
                "fixtures": [fixture], "maximum_layout_work_fixture": "nested_class",
                "maximum_semantic_kind": "class", "maximum_diagram_type": "classDiagram",
                "exact_limit_check": {},
            },
            "cardinality_boundary": {},
        }
        probe = {
            "schema_version": 2, "theme": "dark", "report_kind": "single_probe",
            "policy": policy, "input": calibration.fixture_probe_input(fixture),
            "stage": "semantic",
            "outcome": {
                "status": "accepted_semantic", "semantic_kind": "class",
                "diagram_type": "classDiagram", "elapsed_ns": 1,
            },
        }
        calibration.validate_single_probe_contract("max-semantic", probe, full)
        invalid = copy.deepcopy(probe)
        invalid["outcome"]["diagram_type"] = "flowchart-v2"
        with self.assertRaisesRegex(RuntimeError, "diagram_type"):
            calibration.validate_single_probe_contract("max-semantic", invalid, full)
        invalid = copy.deepcopy(probe)
        invalid["theme"] = "default"
        with self.assertRaisesRegex(RuntimeError, "theme"):
            calibration.validate_single_probe_contract("max-semantic", invalid, full)
        invalid = copy.deepcopy(probe)
        invalid["policy"]["profile"] = "interactive"
        with self.assertRaisesRegex(RuntimeError, "profile"):
            calibration.validate_single_probe_contract("max-semantic", invalid, full)


if __name__ == "__main__":
    unittest.main()
