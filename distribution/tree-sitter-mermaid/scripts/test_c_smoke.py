"""Tests for portable C smoke compiler invocation."""

from __future__ import annotations

import unittest
from pathlib import Path

from scripts.c_smoke import compiler_command


class CompilerCommandTests(unittest.TestCase):
    def test_gnu_style_compiler_uses_c11_and_dash_includes(self) -> None:
        command = compiler_command(["cc"], Path("runtime"), Path("smoke"))
        self.assertIn("-std=c11", command)
        self.assertIn("-Iruntime/include", command)
        self.assertEqual(command[-2:], ["-o", "smoke"])

    def test_msvc_style_compiler_uses_slash_options(self) -> None:
        command = compiler_command(["cl.exe"], Path("runtime"), Path("smoke.exe"))
        self.assertIn("/std:c11", command)
        self.assertIn("/Iruntime/include", command)
        self.assertEqual(command[-1], "/Fe:smoke.exe")


if __name__ == "__main__":
    unittest.main()
