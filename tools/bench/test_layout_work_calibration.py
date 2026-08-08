from __future__ import annotations

import sys
import tempfile
import time
import unittest
from pathlib import Path

from tools.bench import run_layout_work_calibration as calibration


class LayoutWorkCalibrationRunnerTests(unittest.TestCase):
    def test_output_directory_must_be_empty(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            (output / "stale.json").write_text("{}", encoding="utf-8")

            with self.assertRaisesRegex(RuntimeError, "must be empty"):
                calibration.prepare_output_directory(output)

    @unittest.skipUnless(sys.platform.startswith(("darwin", "linux")), "POSIX only")
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

    @unittest.skipUnless(sys.platform.startswith(("darwin", "linux")), "POSIX only")
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

    @unittest.skipUnless(sys.platform.startswith(("darwin", "linux")), "POSIX only")
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


if __name__ == "__main__":
    unittest.main()
