"""Real-runtime tests; no module means explicit skips, never simulated evidence."""

import math
import os
from pathlib import Path
import struct
import unittest
from unittest import mock
import xml.etree.ElementTree as ET
import zlib

MODULE = os.environ.get("MERMAN_WASI_MODULE")
FONT = os.environ.get("MERMAN_WASI_FONT")
if MODULE:
    from client import Client, GuestError

SOURCE = (b"---\nconfig:\n  fontFamily: Cousine\n  flowchart:\n"
          b"    htmlLabels: false\n---\nflowchart TD\nA[Memory fonts] --> B[Retained bytes]\n")
OPTIONS = {"svg": {"pipeline": "resvg-safe", "diagram_id": "python-example",
                    "root_background_color": "transparent"}}


def png_info(data):
    """Read dimensions and the first RGBA8 pixel using only the standard library."""
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise AssertionError("not a PNG")
    width, height, depth, color = struct.unpack(">IIBB", data[16:26])
    if (depth, color) != (8, 6):
        raise AssertionError("expected the exporter's RGBA8 PNG")
    offset, compressed = 8, bytearray()
    while offset < len(data):
        length = struct.unpack(">I", data[offset:offset + 4])[0]
        if data[offset + 4:offset + 8] == b"IDAT":
            compressed.extend(data[offset + 8:offset + 8 + length])
        offset += length + 12
    row = zlib.decompress(compressed)
    if row[0] not in range(5):
        raise AssertionError("invalid PNG filter")
    # All PNG predictors are zero at the first pixel of the first row.
    return width, height, bytes(row[1:5])


@unittest.skipUnless(MODULE, "set MERMAN_WASI_MODULE to a real ABI 1 reactor")
class ClientTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.module = Path(MODULE).read_bytes()
        cls.client = Client(cls.module, options=OPTIONS)
        cls.addClassCleanup(cls.client.close)

    def test_catalog(self):
        catalog = self.client.catalog()
        self.assertEqual(catalog["schema_version"], 1)
        self.assertEqual(catalog["transport_api_version"], 1)
        operations = catalog["capabilities"]["operation_ids"]
        self.assertIn("svg", operations)
        self.assertIn("png", operations)
        self.assertIn("native-font-context", catalog["constructor_service_ids"])

    def test_svg_and_png_scale_matte_dimensions(self):
        svg, metadata = self.client.execute("svg", SOURCE)
        root = ET.fromstring(svg)
        self.assertEqual(root.tag, "{http://www.w3.org/2000/svg}svg")
        self.assertIn(b"Memory fonts", svg)
        self.assertEqual(metadata["media_type"], "image/svg+xml")
        self.assertEqual(metadata["byte_length"], len(svg))
        viewbox = [float(n) for n in root.attrib["viewBox"].replace(",", " ").split()]
        viewport = metadata["svg_viewport"]
        for key, value in zip(("min_x", "min_y", "width", "height"), viewbox):
            self.assertAlmostEqual(viewport[key], value, delta=0.001)
        outputs = []
        for scale in (1, 2):
            png, meta = self.client.execute("png", SOURCE, {
                "svg": OPTIONS["svg"], "raster": {"scale": scale, "matte": "#f1e2d3"}})
            width, height, pixel = png_info(png)
            self.assertEqual(pixel, bytes((241, 226, 211, 255)))
            self.assertEqual(meta["media_type"], "image/png")
            self.assertEqual(meta["byte_length"], len(png))
            plan = meta["output_plan"]
            self.assertEqual(plan["kind"], "raster")
            self.assertFalse(plan["limited"])
            self.assertEqual((plan["width_px"], plan["height_px"]), (width, height))
            self.assertEqual(plan["requested_scale"], scale)
            self.assertEqual(plan["effective_scale"], scale)
            self.assertEqual((width, height), (math.ceil(viewport["width"]) * scale,
                                              math.ceil(viewport["height"]) * scale))
            outputs.append((width, height))
        self.assertEqual(outputs[1], tuple(n * 2 for n in outputs[0]))

    def test_parse_error_then_reuse(self):
        with self.assertRaises(GuestError) as caught:
            self.client.execute("svg", b"flowchart ???")
        payload = caught.exception.payload
        self.assertEqual(payload["version"], 1)
        self.assertIs(payload["ok"], False)
        self.assertEqual(payload["code_name"], "MERMAN_PARSE_ERROR")
        self.assertIsInstance(payload["code"], int)
        self.assertTrue(payload["kind"])
        self.assertTrue(payload["message"])
        self.assertIn("capability_id", payload)
        data, metadata = self.client.execute("svg", SOURCE)
        self.assertIn(b"Memory fonts", data)
        self.assertEqual(metadata["operation_id"], "svg")

    @unittest.skipUnless(FONT, "set MERMAN_WASI_FONT to an actual supported font file")
    def test_extra_font_bytes(self):
        with Client(self.module, options=OPTIONS, fonts=[Path(FONT).read_bytes()]) as client:
            svg, metadata = client.execute("svg", SOURCE)
            self.assertIn(b"Memory fonts", svg)
            self.assertEqual(metadata["media_type"], "image/svg+xml")
        client.close()
        with self.assertRaises(RuntimeError):
            client.catalog()

    def test_trap_requires_replacement(self):
        import wasmtime

        with Client(self.module) as client:
            with mock.patch.object(client, "_fuel", 0), self.assertRaises(wasmtime.Trap):
                client.catalog()
            with self.assertRaisesRegex(RuntimeError, "unusable"):
                client.execute("svg", SOURCE)
        with Client(self.module) as replacement:
            self.assertEqual(replacement.catalog()["transport_api_version"], 1)


if __name__ == "__main__":
    unittest.main()
