import json
from pathlib import Path
import subprocess
import sys
import unittest

if __package__:
    from .workload_svg import EXPECTED_OUTCOMES, validate_rejection, validate_svg
else:
    from workload_svg import EXPECTED_OUTCOMES, validate_rejection, validate_svg


class WorkloadSvgTests(unittest.TestCase):
    def test_viewport_applies_to_the_root_even_if_expected_text_survives(self):
        for attributes in (
            'width="0" height="10"', 'height=" -1px "', 'width="-0e2%"',
            'viewBox="0 0 0 0"', 'viewBox="-3,0,2,-4"', 'viewBox="0 0 nan 9"',
            'viewBox="0 0 1e999 9"', 'viewBox="0,,0,20,8"',
            'viewBox="0 0 30 10" style="width:0px !important;width:100%"',
            'width="100%" style="max-height:0px"',
        ):
            svg = f'<svg {attributes}><text>surviving label</text></svg>'
            for outcome in EXPECTED_OUTCOMES:
                with self.subTest(attributes=attributes, outcome=outcome):
                    self.assertEqual(validate_svg(svg, ["surviving label"], outcome)["status"], "invalid-svg")
        for attributes in ('', 'width="100%" style="max-width: 477.859px;"',
                           'viewBox="-20, -5 .1e3 4"', 'width="0" style="width:20px"'):
            svg = f'<svg {attributes}><g><rect width="0"/><text>label</text></g></svg>'
            with self.subTest(attributes=attributes):
                self.assertEqual(validate_svg(svg, ["label"]), {"status": "ok"})

    def test_allow_empty_needs_an_actual_sized_document_and_honors_label_facts(self):
        for svg in ('<svg viewBox="0 0 17 17"/>', '<svg width="17px" height="17"/>',
                    '<svg style="width:20px;height:2em"><defs><rect/></defs></svg>'):
            with self.subTest(svg=svg):
                self.assertEqual(validate_svg(svg)["status"], "empty-svg")
                self.assertEqual(validate_svg(svg, [], "allow-empty"), {"status": "ok", "output_class": "empty"})
                self.assertEqual(validate_svg(svg, ["missing"], "allow-empty")["status"], "missing-labels")
                self.assertEqual(validate_svg(svg, [], "reject")["status"], "unexpected-success")
                self.assertEqual(validate_svg(svg, [], "unreviewed"), {"status": "ok", "output_class": "empty"})
        for svg in (None, "", "  ", "<svg", "<html/>", "<svg/>", '<svg width="100%"/>'):
            for outcome in EXPECTED_OUTCOMES:
                with self.subTest(svg=svg, outcome=outcome):
                    self.assertIn(validate_svg(svg, [], outcome)["status"], ("empty-svg", "invalid-svg"))

    def test_error_diagram_does_not_establish_a_parse_rejection(self):
        for svg in ('<svg aria-roledescription="error"><rect/></svg>',
                    '<svg><text class="error-text">Syntax error in text</text></svg>'):
            self.assertEqual(validate_svg(svg, [], "reject")["status"], "error-svg")
        authored = '<svg><text>Syntax error in text</text></svg>'
        self.assertEqual(validate_svg(authored), {"status": "ok"})
        for outcome in ("allow-empty", "unreviewed"):
            self.assertEqual(validate_svg(authored, [], outcome), {"status": "ok", "output_class": "draw"})
        self.assertEqual(validate_svg(authored, [], "reject")["status"], "unexpected-success")

    def test_only_positive_product_parser_diagnostics_establish_rejection(self):
        merman = ("Diagram parse error (flowchart-v2): unexpected end of input; "
                  "expected node identifier, edge label")
        pele = "Parse error on line 9:\n  g-->\n------^\nExpecting 'NODE_STRING', got 'EOF'"
        mermaid = "Error at line 10, column 1: Expected a node id, got the end of the text."
        mmdc = ("Generating single mermaid chart\r\n\r\nError: Parse error on line 10:\n"
                "...->g g-->\n------^\nExpecting 'NODE_STRING', got 'EOF'")
        for code, message in (("MERMAN_PARSE_ERROR", merman), ("syntax", pele), ("SYNTAX", mermaid),
                              (1, merman), (1, mmdc), (None, merman), (None, pele), (None, mermaid)):
            with self.subTest(code=code, message=message):
                self.assertEqual(validate_rejection(code, message),
                                 {"status": "expected-rejection", "code": code, "message": message})
        max_edges = ("Diagram parse error (flowchart-v2): Flowchart edge limit exceeded: this edge group "
                     "would create 1 edges, but only 0 of the maxEdges budget remain")
        for code, message in (
            (None, max_edges), ("MERMAN_PARSE_ERROR", max_edges), (1, max_edges),
            ("MERMAN_PARSE_ERROR", "Diagram parse error (flowchart-v2): unexpected host error; expected a worker"),
            ("MERMAN_PARSE_ERROR", "parse failure"), (None, "Diagram parse error (flowchart-v2): unrelated failure"),
            ("MERMAN_RESOURCE_LIMIT_EXCEEDED", merman), ("limit", pele), ("LIMIT", mermaid),
            ("ENOENT", merman), ("INTERNAL", mermaid), ("semantic", pele), (-9, merman),
            (None, "Resource limit exceeded"), (1, "Parser.parseError (/path/chunk.js:1:20)"),
            (None, "Error: Failed to launch the browser\n" + pele), (None, "SyntaxError: JSON.parse failed"),
            (1, "Generating single mermaid chart\nError: Cannot find module 'parser'"), (1, ""),
        ):
            with self.subTest(code=code, message=message):
                self.assertEqual(validate_rejection(code, message)["status"], "render-error")

    def test_ordered_stdin_protocol_preserves_diagnostics_and_legacy_draw(self):
        script = Path(__file__).with_name("workload_svg.py")
        syntax = {"code": "SYNTAX", "message": "Error at line 10, column 1: Expected a node id, got EOF."}
        jobs = [
            {"error": syntax, "expected_outcome": "reject"},
            {"error": {**syntax, "code": "INTERNAL"}, "expected_outcome": "reject"},
            {"error": syntax, "expected_outcome": "unreviewed"}, {"error": syntax},
            {"svg": '<svg viewBox="0 0 2 2"/>'},
            {"svg": '<svg viewBox="0 0 2 2"/>', "expected_outcome": "allow-empty"},
            {"svg": "<svg><text>A</text></svg>", "expected_outcome": "reject"},
            {"expected_outcome": "reject"}, {"svg": "<svg/>", "expected_outcome": "typo"},
        ]
        reply = subprocess.run([sys.executable, "-B", str(script)], input=json.dumps(jobs),
                               capture_output=True, text=True, check=True, timeout=10)
        verdicts = json.loads(reply.stdout)
        self.assertEqual(
            [v["status"] for v in verdicts],
            ["expected-rejection", "render-error", "render-error", "render-error", "empty-svg", "ok",
             "unexpected-success", "invalid-svg", "invalid-request"],
        )
        self.assertEqual(verdicts[0], {"status": "expected-rejection", **syntax})


if __name__ == "__main__":
    unittest.main()
