#!/usr/bin/env python3
"""Tests for the Cargo package legal-material verifier."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from scripts import verify_crate_package_legal_materials as verify


class PackageLegalMaterialTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.manifest = self.root / "Cargo.toml"
        self.manifest.write_text("[package]\nname='demo'\nversion='0.1.0'\n", encoding="utf-8")

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def package(self, license_expression: str = "MIT OR Apache-2.0") -> dict[str, object]:
        return {
            "name": "demo",
            "manifest_path": str(self.manifest),
            "license": license_expression,
        }

    def test_dual_license_package_requires_both_texts(self) -> None:
        with self.assertRaisesRegex(verify.PackageLegalMaterialError, "LICENSE-APACHE"):
            verify.verify_package_listing(self.package(), {"LICENSE-MIT"})

    def test_third_party_bundle_is_required_when_present(self) -> None:
        (self.root / "THIRD_PARTY_NOTICES.md").write_text("notice\n", encoding="utf-8")
        licenses = self.root / "THIRD_PARTY_LICENSES/demo"
        licenses.mkdir(parents=True)
        (licenses / "LICENSE").write_text("license\n", encoding="utf-8")
        with self.assertRaisesRegex(verify.PackageLegalMaterialError, "THIRD_PARTY_LICENSES"):
            verify.verify_package_listing(
                self.package(),
                {"LICENSE-MIT", "LICENSE-APACHE", "THIRD_PARTY_NOTICES.md"},
            )

    def test_complete_listing_passes(self) -> None:
        verify.verify_package_listing(
            self.package(),
            {"LICENSE-MIT", "LICENSE-APACHE", "src/lib.rs"},
        )

    def test_package_listing_paths_are_normalized_for_windows(self) -> None:
        (self.root / "THIRD_PARTY_NOTICES.md").write_text("notice\n", encoding="utf-8")
        licenses = self.root / "THIRD_PARTY_LICENSES/demo"
        licenses.mkdir(parents=True)
        (licenses / "LICENSE").write_text("license\n", encoding="utf-8")

        package = self.package()
        package["metadata"] = {"merman-legal": {"third-party-bundle": True}}
        verify.verify_package_listing(
            package,
            {
                "LICENSE-MIT",
                "LICENSE-APACHE",
                "THIRD_PARTY_NOTICES.md",
                r"THIRD_PARTY_LICENSES\demo\LICENSE",
            },
        )

    def test_epl_package_requires_full_epl_text(self) -> None:
        with self.assertRaisesRegex(verify.PackageLegalMaterialError, "EPL-2.0"):
            verify.verify_package_listing(self.package("EPL-2.0"), {"src/lib.rs"})

    def test_dry_run_independent_package_is_legally_governed(self) -> None:
        private_package = self.package("MIT")
        private_package["publish"] = []
        metadata = {
            "metadata": {
                "merman-release": {"independent-packages": ["demo"]},
            },
            "packages": [private_package],
        }

        self.assertEqual(verify.governed_packages(metadata), [private_package])

    def test_declared_third_party_bundle_cannot_be_omitted(self) -> None:
        private_package = self.package("MIT")
        private_package["metadata"] = {
            "merman-legal": {"third-party-bundle": True}
        }

        with self.assertRaisesRegex(
            verify.PackageLegalMaterialError,
            "incomplete third-party legal bundle",
        ):
            verify.required_legal_paths(private_package)


if __name__ == "__main__":
    unittest.main()
