#!/usr/bin/env python3
"""Unit tests for the static release-surface contract."""

from __future__ import annotations

import json
from pathlib import Path
import unittest
from unittest import mock

from scripts import release_surface_contract as contract


ROOT = Path(__file__).resolve().parents[1]


class ReleaseSurfaceContractTests(unittest.TestCase):
    def test_repository_contract_matches_current_workflows_and_profiles(self) -> None:
        surfaces = contract.validate_repository(ROOT)

        self.assertEqual(
            {surface.surface_id for surface in surfaces},
            {
                "rust-crates-and-ffi-source",
                "android-aar",
                "apple-xcframework",
                "python-wheel",
                "flutter-pub",
            },
        )
        rust_surface = next(
            surface for surface in surfaces if surface.surface_id == "rust-crates-and-ffi-source"
        )
        self.assertIn("merman-ffi", rust_surface.source_packages)
        self.assertNotIn("merman-android-jni", rust_surface.source_packages)

    def test_json_report_is_a_declaration_not_a_live_status_cache(self) -> None:
        report = json.loads(contract.render_report(contract.SURFACES, as_json=True))

        self.assertEqual(report["scope"], "ffi-native")
        self.assertEqual(len(report["surfaces"]), 5)
        self.assertNotIn("status", report["surfaces"][0])
        self.assertEqual(
            report["surfaces"][1]["artifact_profiles"],
            ["android-native"],
        )

    def test_missing_workflow_marker_fails_closed(self) -> None:
        original_read = contract._read

        def read_without_android_upload(root: Path, path: Path) -> str:
            text = original_read(root, path)
            if path == Path(".github/workflows/release-android.yml"):
                return text.replace("Upload AAR to GitHub Release", "Upload AAR")
            return text

        with mock.patch.object(contract, "_read", side_effect=read_without_android_upload):
            with self.assertRaisesRegex(contract.SurfaceContractError, "android-aar"):
                contract.validate_repository(ROOT)


if __name__ == "__main__":
    unittest.main()
