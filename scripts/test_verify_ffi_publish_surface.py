#!/usr/bin/env python3
"""Unit tests for the FFI publish-surface verifier."""

from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path


MODULE_PATH = Path(__file__).with_name("verify-ffi-publish-surface.py")
SPEC = importlib.util.spec_from_file_location("verify_ffi_publish_surface", MODULE_PATH)
assert SPEC is not None
verify_ffi = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(verify_ffi)


class PythonUniffiExampleTests(unittest.TestCase):
    def valid_contract(self) -> set[tuple[str, str, int]]:
        source = """
import merman

class Measurer(merman.MermanTextMeasurer):
    def measure(self, request):
        return None

engine = merman.MermanEngine()
svg = engine.render_svg("flowchart TD\\nA --> B", None)
reusable = engine.reusable_engine_with_text_measurer(None, Measurer())
svg = reusable.render_svg("flowchart TD\\nA --> B", None)
plain = engine.reusable_engine(None)
plain.render_svg("flowchart TD\\nA --> B", None)
"""

        return verify_ffi.validate_python_uniffi_usage(
            source, "example.py", require_text_measurer=True
        )

    def test_accepts_generated_engine_and_reusable_call_shapes(self) -> None:
        self.valid_contract()

    def test_rejects_nonexistent_measure_text_callback(self) -> None:
        source = """
import merman

class Measurer(merman.MermanTextMeasurer):
    def measure_text(self, request):
        return None
"""

        with self.assertRaisesRegex(verify_ffi.CheckFailure, r"measure\(\), not measure_text"):
            verify_ffi.validate_python_uniffi_usage(source, "example.py")

    def test_rejects_missing_one_shot_options_json(self) -> None:
        source = """
import merman
engine = merman.MermanEngine()
engine.render_svg("flowchart TD\\nA --> B")
"""

        allowed_calls = self.valid_contract()
        with self.assertRaisesRegex(
            verify_ffi.CheckFailure, r"MermanEngine\.render_svg expects 2 argument\(s\)"
        ):
            verify_ffi.validate_python_uniffi_usage(
                source, "example.py", allowed_calls=allowed_calls
            )

    def test_rejects_missing_reusable_request_options_json(self) -> None:
        source = """
import merman
engine = merman.MermanEngine()
reusable = engine.reusable_engine(None)
reusable.render_svg("flowchart TD\\nA --> B")
"""

        allowed_calls = self.valid_contract()
        with self.assertRaisesRegex(
            verify_ffi.CheckFailure,
            r"MermanReusableEngine\.render_svg expects 2 argument\(s\)",
        ):
            verify_ffi.validate_python_uniffi_usage(
                source, "example.py", allowed_calls=allowed_calls
            )

    def test_rejects_engine_operation_called_as_module_function(self) -> None:
        source = """
import merman
merman.render_svg("flowchart TD\\nA --> B", None)
"""

        allowed_calls = self.valid_contract()
        with self.assertRaisesRegex(verify_ffi.CheckFailure, "not a merman module function"):
            verify_ffi.validate_python_uniffi_usage(
                source, "example.py", allowed_calls=allowed_calls
            )

    def test_repository_examples_match_the_structured_contract(self) -> None:
        verify_ffi.check_python_examples()
        verify_ffi.check_python_package_exports()


if __name__ == "__main__":
    unittest.main()
