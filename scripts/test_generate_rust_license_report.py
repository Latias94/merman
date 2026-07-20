#!/usr/bin/env python3
"""Unit tests for the normalized cargo-about report."""

from __future__ import annotations

import importlib.util
import tempfile
import unittest
from pathlib import Path


MODULE_PATH = Path(__file__).with_name("generate-rust-license-report.py")
SPEC = importlib.util.spec_from_file_location("generate_rust_license_report", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
report = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(report)


class RustLicenseReportTests(unittest.TestCase):
    def test_normalization_removes_host_paths_and_private_workspace_crates(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "Cargo.lock").write_text("lock", encoding="utf-8")
            (root / "about.toml").write_text("accepted = []", encoding="utf-8")
            raw = {
                "licenses": [
                    {
                        "id": "MIT",
                        "name": "MIT License",
                        "source_path": "/Users/alice/.cargo/registry/example/LICENSE",
                        "text": "MIT terms",
                        "used_by": [
                            {
                                "crate": {
                                    "name": "external",
                                    "version": "1.2.3",
                                    "source": "registry+https://example.invalid/index",
                                    "license": "MIT",
                                    "authors": ["B", "A"],
                                    "repository": "https://example.invalid/external",
                                }
                            },
                            {
                                "crate": {
                                    "name": "workspace-member",
                                    "version": "0.1.0",
                                    "source": None,
                                }
                            },
                        ],
                    }
                ]
            }

            normalized = report.normalize_report(raw, root)

            self.assertNotIn("/Users/alice", str(normalized))
            self.assertEqual(
                normalized["licenses"][0]["packages"][0]["name"], "external"
            )
            self.assertEqual(
                normalized["licenses"][0]["packages"][0]["authors"], ["A", "B"]
            )

    def test_normalization_rejects_a_report_without_third_party_licenses(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "Cargo.lock").write_text("lock", encoding="utf-8")
            (root / "about.toml").write_text("accepted = []", encoding="utf-8")
            with self.assertRaisesRegex(
                report.RustLicenseReportError, "no third-party licenses"
            ):
                report.normalize_report({"licenses": []}, root)


if __name__ == "__main__":
    unittest.main()
