import importlib.util
import json
from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "flutter_packaged_app", ROOT / "platforms/flutter/tool/qualify_packaged_app.py",
)
assert SPEC is not None and SPEC.loader is not None
qualifier = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(qualifier)


class FlutterPackagedMathTests(unittest.TestCase):
    def output(self, formula: str) -> str:
        svg = ('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 50">'
               '<path d="M0 0 L10 10"/>' + formula + '</svg>')
        metadata = {
            "version": 1, "operation_id": "svg", "media_type": "image/svg+xml",
            "runtime_policy": "deterministic", "byte_length": len(svg.encode()),
            "svg_viewport": {"min_x": 0, "min_y": 0, "width": 100, "height": 50},
        }
        return qualifier.MATH_EVIDENCE_PREFIX + json.dumps({"svg": svg, "metadata": metadata})

    def test_formula_must_contain_glyph_paths_even_when_an_edge_is_present(self):
        for formula in (
            '<g class="merman-prepared-math-native"/>',
            '<g class="merman-prepared-math-native"><path d=" "/></g>',
        ):
            with self.subTest(formula=formula), self.assertRaisesRegex(ValueError, "glyph paths"):
                qualifier.validate_app_math(self.output(formula))

    def test_referenced_formula_glyphs_are_accepted(self):
        formula = ('<defs><g id="formula"><path d="M1 2 L3 4"/></g></defs>'
                   '<use class="merman-prepared-math-native" href="#formula"/>')
        self.assertGreater(qualifier.validate_app_math(self.output(formula)), 0)

    def test_missing_duplicate_and_unresolved_results_are_rejected(self):
        for output in ("merman Flutter smoke passed", self.output('') + '\n' + self.output('')):
            with self.subTest(output=output), self.assertRaisesRegex(RuntimeError, "exactly one"):
                qualifier.validate_app_math(output)
        with self.assertRaisesRegex(ValueError, "unresolved"):
            qualifier.validate_app_math(self.output(
                '<use class="merman-prepared-math-native" href="#missing"/>',
            ))


if __name__ == "__main__":
    unittest.main()
