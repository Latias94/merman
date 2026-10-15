"""Check the bounded SVG oracle used by the installed Python authoring smoke."""

from __future__ import annotations

import importlib.util
from pathlib import Path
import types
import unittest
from unittest import mock


MODULE_PATH = (
    Path(__file__).resolve().parents[1]
    / "platforms/python/merman/examples/theme_authoring.py"
)
SPEC = importlib.util.spec_from_file_location("python_theme_authoring", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
AUTHORING = importlib.util.module_from_spec(SPEC)
# Only isolate the pure SVG oracle here. The wheel smoke loads the real UniFFI package.
with mock.patch.dict("sys.modules", {"merman": types.ModuleType("merman")}):
    SPEC.loader.exec_module(AUTHORING)


class StateFillOracleTests(unittest.TestCase):
    SVG = (
        '<svg xmlns="http://www.w3.org/2000/svg"><g class="statediagram-state">'
        '<rect class="label-container" width="40" height="20" fill="#123abc"/>'
        '<text>Active</text></g></svg>'
    )

    def test_positive_area_returns_the_terminal_fill(self) -> None:
        self.assertEqual(AUTHORING.state_fill(self.SVG), "#123abc")
        styled = self.SVG.replace('fill="#123abc"', 'style="fill:#456def!important"')
        self.assertEqual(AUTHORING.state_fill(styled), "#456def")

    def test_missing_nonfinite_and_nonpositive_dimensions_are_rejected(self) -> None:
        for name, original in (("width", "40"), ("height", "20")):
            for value in (None, "", "0", "-1", "NaN", "Infinity", "-Infinity", "invalid"):
                with self.subTest(dimension=name, value=value):
                    attribute = f'{name}="{original}"'
                    replacement = "" if value is None else f'{name}="{value}"'
                    with self.assertRaisesRegex(RuntimeError, "positive finite"):
                        AUTHORING.state_fill(self.SVG.replace(attribute, replacement))

    def test_non_svg_and_non_rect_lookalikes_are_rejected(self) -> None:
        for svg in (
            self.SVG.replace("http://www.w3.org/2000/svg", "urn:other"),
            self.SVG.replace("<rect ", "<g "),
        ):
            with self.subTest(svg=svg), self.assertRaises(RuntimeError):
                AUTHORING.state_fill(svg)


if __name__ == "__main__":
    unittest.main()
