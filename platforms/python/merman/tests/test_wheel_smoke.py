"""Exercise the installed-wheel geometry assertion without loading a native library."""

import importlib.util
from pathlib import Path
import sys
import types
import unittest
from unittest.mock import patch


spec = importlib.util.spec_from_file_location(
    "wheel_smoke", Path(__file__).resolve().parents[1] / "examples/wheel_smoke.py"
)
smoke = importlib.util.module_from_spec(spec)
with patch.dict(sys.modules, {"merman": types.ModuleType("merman")}):
    spec.loader.exec_module(smoke)


class WheelSmokeGeometryTests(unittest.TestCase):
    def test_package_version_matches_pep440_release_identity(self):
        for native, installed in (
            ("0.9.3", "0.9.3"),
            ("0.9.3-alpha.5", "0.9.3a5"),
            ("0.9.3-beta.2", "0.9.3b2"),
            ("0.9.3-rc.1+Build-ABC", "0.9.3rc1+build.abc"),
        ):
            with self.subTest(native=native):
                self.assertEqual(smoke.python_package_version(native), installed)

    def test_formula_requires_real_geometry(self):
        svg = (
            '<svg xmlns="http://www.w3.org/2000/svg"><foreignObject><div>'
            '<svg viewBox="0 0 12 8"><path d="M0 0L1 1"/></svg>'
            '</div></foreignObject></svg>'
        )
        self.assertEqual(smoke.assert_math_svg(svg), 1)
        for invalid in (
            svg.replace('viewBox="0 0 12 8"', 'viewBox="0 0 0 8"'),
            svg.replace('viewBox="0 0 12 8"', 'viewBox="0 0 nan 8"'),
            svg.replace('<path d="M0 0L1 1"/>', '<text>x^2</text>'),
            svg.replace('</foreignObject>', '<span>$$x^2$$</span></foreignObject>'),
            '<svg xmlns="http://www.w3.org/2000/svg"><path d="M0 0L1 1"/></svg>',
        ):
            with self.subTest(invalid=invalid), self.assertRaises(RuntimeError):
                smoke.assert_math_svg(invalid)


if __name__ == "__main__":
    unittest.main()
