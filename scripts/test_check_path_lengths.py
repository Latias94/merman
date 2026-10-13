"""Boundary cases for Windows checkout path accounting."""

from __future__ import annotations

import unittest

from scripts.check_path_lengths import oversized_paths


class PathLengthTests(unittest.TestCase):
    def test_exact_boundary_is_allowed(self) -> None:
        self.assertEqual(oversized_paths(["a" * 160]), [])

    def test_excessive_nested_path_is_reported(self) -> None:
        path = "docs/" + "a" * 153 + ".md"
        self.assertEqual(oversized_paths([path]), [(path, 161)])

    def test_supplementary_unicode_uses_two_windows_units(self) -> None:
        path = "a" * 159 + "\U0001f600"
        self.assertEqual(len(path), 160)
        self.assertEqual(oversized_paths([path]), [(path, 161)])


if __name__ == "__main__":
    unittest.main()
