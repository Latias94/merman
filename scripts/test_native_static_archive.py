#!/usr/bin/env python3
"""Build-free tests for native static distribution copies."""

from __future__ import annotations

import contextlib
import io
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

from scripts import native_static_archive


class NativeStaticArchiveTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.source = self.root / "built" / "library.a"
        self.source.parent.mkdir()
        self.original = b"native objects with embedded LLVM bitcode\n"
        self.source.write_bytes(self.original)
        self.destination = self.root / "staged" / "library.a"
        self.destination.parent.mkdir()
        self.sysroot = self.root / "pinned toolchain"
        self.host = "x86_64-unknown-linux-gnu"
        self.objcopy = self.sysroot / "lib" / "rustlib" / self.host / "bin" / "llvm-objcopy"
        self.objcopy.parent.mkdir(parents=True)
        self.objcopy.touch()
        self.cleaned = b"native objects without embedded LLVM bitcode\n"
        self.failure = False
        self.process = self.enterContext(mock.patch.object(
            native_static_archive.subprocess, "run", side_effect=self.run_process,
        ))

    def run_process(self, command, **options):
        command = [str(argument) for argument in command]
        if command == ["rustc", "--print", "sysroot"]:
            return subprocess.CompletedProcess(command, 0, str(self.sysroot) + "\n", "")
        if command == ["rustc", "-vV"]:
            return subprocess.CompletedProcess(command, 0, f"host: {self.host}\n", "")
        self.assertEqual(command[0], str(self.objcopy))
        self.assertEqual(Path(command[-2]), self.source)
        self.assertTrue(options["check"])
        Path(command[-1]).write_bytes(self.cleaned)
        if self.failure:
            raise subprocess.CalledProcessError(1, command, stderr="fixture-objcopy-error")
        return subprocess.CompletedProcess(command, 0, "", "")

    def test_stages_only_the_requested_section_removals_and_preserves_source(self):
        targets = {
            "aarch64-apple-ios": ("__LLVM,__bitcode", "__LLVM,__cmdline"),
            "aarch64-apple-ios-sim": ("__LLVM,__bitcode", "__LLVM,__cmdline"),
            "x86_64-apple-ios": ("__LLVM,__bitcode", "__LLVM,__cmdline"),
            "aarch64-apple-darwin": ("__LLVM,__bitcode", "__LLVM,__cmdline"),
            "x86_64-apple-darwin": ("__LLVM,__bitcode", "__LLVM,__cmdline"),
            "x86_64-unknown-linux-gnu": (".llvmbc", ".llvmcmd"),
            "aarch64-unknown-linux-gnu": (".llvmbc", ".llvmcmd"),
        }
        for target, sections in targets.items():
            with self.subTest(target=target):
                self.process.reset_mock()
                self.destination.write_bytes(b"previous accepted library")
                native_static_archive.stage_static_library(self.source, self.destination, target)
                self.assertEqual(self.source.read_bytes(), self.original)
                self.assertEqual(self.destination.read_bytes(), self.cleaned)
                commands = [[str(argument) for argument in call.args[0]]
                            for call in self.process.call_args_list]
                self.assertEqual(len(commands), 3)
                self.assertIn(["rustc", "--print", "sysroot"], commands)
                self.assertIn(["rustc", "-vV"], commands)
                objcopy = next(command for command in commands if command[0] == str(self.objcopy))
                self.assertEqual(objcopy[1:-2], [f"--remove-section={section}" for section in sections])
                self.assertNotEqual(Path(objcopy[-1]), self.destination)
                self.assertEqual(set(self.destination.parent.iterdir()), {self.destination})

    def test_windows_copies_short_import_members_without_any_tool_lookup(self):
        self.source.write_bytes(b"!<arch>\nCOFF short import members\x00\xff")
        self.objcopy.unlink()
        native_static_archive.stage_static_library(
            self.source, self.destination, "x86_64-pc-windows-msvc",
        )
        self.assertEqual(self.destination.read_bytes(), self.source.read_bytes())
        self.process.assert_not_called()

    def test_failed_transform_does_not_publish_partial_output_or_replace_accepted_output(self):
        self.failure = True
        for accepted in (None, b"previous accepted library"):
            with self.subTest(accepted=accepted):
                if accepted is not None:
                    self.destination.write_bytes(accepted)
                with self.assertRaises(RuntimeError) as caught:
                    native_static_archive.stage_static_library(
                        self.source, self.destination, "aarch64-apple-darwin",
                    )
                self.assertIn("fixture-objcopy-error", str(caught.exception))
                self.assertEqual(self.source.read_bytes(), self.original)
                if accepted is None:
                    self.assertFalse(self.destination.exists())
                    self.assertEqual(list(self.destination.parent.iterdir()), [])
                else:
                    self.assertEqual(self.destination.read_bytes(), accepted)
                    self.assertEqual(set(self.destination.parent.iterdir()), {self.destination})

    def test_missing_component_identifies_tool_and_install_command(self):
        self.objcopy.unlink()
        with self.assertRaises(RuntimeError) as caught:
            native_static_archive.stage_static_library(
                self.source, self.destination, "aarch64-apple-darwin",
            )
        self.assertIn(str(self.objcopy), str(caught.exception))
        self.assertIn("rustup component add llvm-tools-preview", str(caught.exception))
        self.assertFalse(self.destination.exists())
        self.assertEqual(self.source.read_bytes(), self.original)

    def test_rejects_output_that_would_overwrite_the_original(self):
        with self.assertRaises(ValueError):
            native_static_archive.stage_static_library(
                self.source, self.source, "aarch64-apple-darwin",
            )
        self.assertEqual(self.source.read_bytes(), self.original)
        self.process.assert_not_called()

    def test_cli_stages_a_library_and_reports_tool_diagnostics_on_failure(self):
        arguments = ["native_static_archive.py", "--source", str(self.source),
                     "--output", str(self.destination), "--target", "aarch64-apple-darwin"]
        with mock.patch.object(sys, "argv", arguments):
            self.assertEqual(native_static_archive.main(), 0)
        self.assertEqual(self.destination.read_bytes(), self.cleaned)
        self.failure = True
        diagnostics = io.StringIO()
        with mock.patch.object(sys, "argv", arguments), contextlib.redirect_stderr(diagnostics):
            self.assertEqual(native_static_archive.main(), 1)
        self.assertIn("fixture-objcopy-error", diagnostics.getvalue())
        self.assertEqual(self.destination.read_bytes(), self.cleaned)
        self.assertEqual(self.source.read_bytes(), self.original)


if __name__ == "__main__":
    unittest.main()
