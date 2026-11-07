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


class PairedThemeConsumerTests(unittest.TestCase):
    def test_existing_json_operations_carry_the_complete_pair_and_concrete_selection(self) -> None:
        recipe = {
            "schema_version": 1, "kind": "design",
            "design": {"appearances": {"light": {}, "dark": {}}, "default_appearance": "light"},
        }
        requests, renders = [], []

        def execute(request):
            requests.append(request)
            if request.operation_id == "export-theme-preset-json":
                self.assertEqual(request.source, "editor")
                result = recipe
            else:
                payload = AUTHORING.json.loads(request.source)
                self.assertEqual(payload["schema_version"], 1)
                self.assertEqual(payload["recipe"], recipe)
                if payload["action"] == "inspect":
                    result = {"supported_appearances": ["light", "dark"], "default_appearance": "light"}
                else:
                    result = {
                        "effective_appearance": payload["appearance"],
                        "spec": {"canvas": {"base": payload["appearance"]}},
                    }
            return types.SimpleNamespace(
                operation_id=request.operation_id, media_type="application/json",
                data=AUTHORING.json.dumps(result).encode("utf-8"),
            )

        def render(source, options_json):
            options = AUTHORING.json.loads(options_json)
            self.assertEqual(options["theme"]["recipe"], recipe)
            self.assertEqual(source, AUTHORING.SOURCES["flowchart"])
            mode = options["theme"]["appearance"]
            renders.append(mode)
            return f'<svg xmlns="http://www.w3.org/2000/svg"><text>{mode}</text></svg>'

        client = types.SimpleNamespace(execute=execute, render_svg=render)
        with mock.patch.object(
            AUTHORING.merman, "MermanOperationRequestV4",
            side_effect=lambda **kwargs: types.SimpleNamespace(**kwargs), create=True,
        ):
            AUTHORING.verify_paired_theme(client, recipe)
        self.assertEqual(renders, ["light", "dark", "light"])
        self.assertEqual([request.operation_id for request in requests], [
            "materialize-theme-json", "materialize-theme-json", "materialize-theme-json",
            "materialize-theme-json", "export-theme-preset-json",
        ])


if __name__ == "__main__":
    unittest.main()
