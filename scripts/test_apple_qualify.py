"""Behavior checks for installed Apple consumer evidence."""

import json
from pathlib import Path
import tempfile
import unittest

from scripts import apple_qualify


class AppleQualificationTests(unittest.TestCase):
    def test_matched_compression_requires_at_least_half_the_original_payload(self):
        self.assertEqual(apple_qualify.compare_sizes(1000, 499)["reduction_fraction"], 0.501)
        with self.assertRaisesRegex(ValueError, "50%"):
            apple_qualify.compare_sizes(1000, 501)

    def test_installed_consumer_requires_formula_paths_and_exact_catalog(self):
        svg = ('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 20">'
               '<g class="merman-prepared-math-native"><path d="M0 0L1 1"/></g></svg>')
        metadata = {"version": 1, "operation_id": "svg", "media_type": "image/svg+xml",
                    "byte_length": len(svg.encode()), "runtime_policy": "deterministic",
                    "svg_viewport": {"min_x": 0, "min_y": 0, "width": 10, "height": 20}}
        catalog = {"capabilities": {"capability_ids": ["layout-elk", "math", "svg"],
                                    "output_ids": ["ascii", "svg"]},
                   "package_version": "1.2.3"}
        payload = {"catalog": catalog, "basic": {"svg": svg, "metadata": metadata},
                   "math": {"svg": svg, "metadata": metadata},
                   "rejected_capability_ids": ["jpeg", "math", "pdf", "png"]}
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "consumer.json"
            path.write_text(json.dumps(payload))
            report = apple_qualify.validate_consumer(
                path, capabilities=("layout-elk", "math", "svg"),
                outputs=("ascii", "svg"), version="1.2.3",
            )
            self.assertEqual(report["catalog"], catalog)
            payload["math"]["svg"] = svg.replace('d="M0 0L1 1"', 'd=""')
            payload["math"]["metadata"] = dict(metadata, byte_length=len(payload["math"]["svg"].encode()))
            path.write_text(json.dumps(payload))
            with self.assertRaisesRegex(ValueError, "glyph paths"):
                apple_qualify.validate_consumer(path, capabilities=("layout-elk", "math", "svg"),
                                                outputs=("ascii", "svg"), version="1.2.3")


if __name__ == "__main__":
    unittest.main()
