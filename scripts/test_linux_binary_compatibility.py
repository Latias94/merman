#!/usr/bin/env python3
"""Tests for the GNU release binaries' glibc version-needs check."""

from __future__ import annotations

import contextlib
import io
from pathlib import Path
import subprocess
import unittest
from unittest import mock

from scripts import linux_binary_compatibility as compatibility


def version_needs(*versions: str) -> str:
    return (
        "Version needs section '.gnu.version_r' contains 1 entry:\n"
        "  000000: Version: 1  File: libc.so.6  Cnt: 2\n"
        + "".join(
            f"  0x0010: Name: GLIBC_{version}  Flags: none  Version: 2\n"
            for version in versions
        )
    )


class LinuxBinaryCompatibilityTests(unittest.TestCase):
    def test_version_ordering_is_numeric(self) -> None:
        self.assertEqual(
            compatibility.required_glibc_version(version_needs("2.9", "2.35", "2.2.5")),
            (2, 35),
        )

    def test_only_version_needs_are_parsed(self) -> None:
        output = (
            "Version definition section '.gnu.version_d' contains 1 entry:\n"
            "  0x0010: Name: GLIBC_2.99\n"
            + version_needs("2.34")
            + "Version symbols section '.gnu.version' contains 1 entry:\n"
            "  0x0010: Name: GLIBC_2.98\n"
        )
        self.assertEqual(compatibility.required_glibc_version(output), (2, 34))

    def test_missing_glibc_needs_are_an_error(self) -> None:
        for output in ("", "No version information found in this file.", version_needs("PRIVATE")):
            with self.subTest(output=output), self.assertRaisesRegex(RuntimeError, "no numeric GLIBC"):
                compatibility.required_glibc_version(output)

    def test_ceiling_accepts_both_binaries_and_uses_readelf(self) -> None:
        output = io.StringIO()
        with (
            mock.patch.object(
                compatibility.subprocess, "run",
                side_effect=[
                    subprocess.CompletedProcess([], 0, stdout=version_needs("2.35")),
                    subprocess.CompletedProcess([], 0, stdout=version_needs("2.34")),
                ],
            ) as run,
            contextlib.redirect_stdout(output),
        ):
            self.assertEqual(compatibility.main(["merman-cli", "merman-lsp"]), 0)
        self.assertEqual(len(run.call_args_list), 2)
        for call, name in zip(run.call_args_list, ("merman-cli", "merman-lsp")):
            self.assertEqual(call.args[0], ["readelf", "--version-info", "--wide", name])
            self.assertEqual(call.kwargs["env"]["LC_ALL"], "C")
            self.assertTrue(call.kwargs["check"])
        self.assertIn("merman-cli: GLIBC_2.35 <= GLIBC_2.35", output.getvalue())
        self.assertIn("merman-lsp: GLIBC_2.34 <= GLIBC_2.35", output.getvalue())

    def test_higher_required_version_is_rejected(self) -> None:
        with (
            mock.patch.object(
                compatibility.subprocess, "run",
                return_value=subprocess.CompletedProcess([], 0, stdout=version_needs("2.35", "2.38")),
            ),
            self.assertRaisesRegex(RuntimeError, "requires GLIBC_2.38, above GLIBC_2.35"),
        ):
            compatibility.verify_binary(Path("merman-cli"))

    def test_readelf_failures_are_errors(self) -> None:
        for error in (FileNotFoundError("readelf"), subprocess.CalledProcessError(1, ["readelf"])):
            with (
                self.subTest(error=error),
                mock.patch.object(compatibility.subprocess, "run", side_effect=error),
                self.assertRaisesRegex(RuntimeError, "cannot inspect"),
            ):
                compatibility.verify_binary(Path("merman-cli"))


if __name__ == "__main__":
    unittest.main()
