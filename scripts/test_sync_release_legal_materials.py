#!/usr/bin/env python3
"""Unit tests for release legal material projections."""

from __future__ import annotations

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


MODULE_PATH = Path(__file__).with_name("sync-release-legal-materials.py")
SPEC = importlib.util.spec_from_file_location("sync_release_legal_materials", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
sync = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(sync)


class ReleaseLegalMaterialTests(unittest.TestCase):
    def test_write_and_check_cover_every_release_surface(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            seed_root(root)
            expected = sync.expected_projections(root)

            sync.write_projections(root, expected)

            self.assertEqual(sync.check_projections(root, expected), [])
            self.assertIn(
                root / "platforms/web/THIRD_PARTY_LICENSES/mermaid/LICENSE", expected
            )
            self.assertIn(
                root / "platforms/android/src/main/resources/META-INF/LICENSE",
                expected,
            )

    def test_check_rejects_stale_and_unexpected_files(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            seed_root(root)
            expected = sync.expected_projections(root)
            sync.write_projections(root, expected)
            (root / "platforms/web/LICENSE").write_text("stale", encoding="utf-8")
            extra = root / "platforms/web/THIRD_PARTY_LICENSES/extra.txt"
            extra.write_text("extra", encoding="utf-8")

            failures = sync.check_projections(root, expected)

            self.assertIn("stale projection: platforms/web/LICENSE", failures)
            self.assertIn(
                "unexpected projection file: platforms/web/THIRD_PARTY_LICENSES/extra.txt",
                failures,
            )

    def test_write_preserves_owned_external_npm_reports(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            seed_root(root)
            external = (
                root
                / "playground/public/THIRD_PARTY_LICENSES/npm-production-dependencies.txt"
            )
            external.parent.mkdir(parents=True)
            external.write_text("generated npm report\n", encoding="utf-8")
            expected = sync.expected_projections(root)

            sync.write_projections(root, expected)

            self.assertEqual(external.read_text(encoding="utf-8"), "generated npm report\n")
            self.assertEqual(sync.check_projections(root, expected), [])


def seed_root(root: Path) -> None:
    (root / "LICENSE-MIT").write_text("MIT\n", encoding="utf-8")
    (root / "LICENSE-APACHE").write_text("Apache\n", encoding="utf-8")
    (root / "THIRD_PARTY_NOTICES.md").write_text("Notices\n", encoding="utf-8")
    licenses = root / "THIRD_PARTY_LICENSES"
    licenses.mkdir()
    components = []
    component_ids = sorted(
        {
            component_id
            for values in sync.CRATE_COMPONENTS.values()
            for component_id in values
        }
    )
    for component_id in component_ids:
        license_file = licenses / component_id / "LICENSE"
        license_file.parent.mkdir()
        license_file.write_text(f"{component_id} terms\n", encoding="utf-8")
        components.append(
            {
                "id": component_id,
                "name": component_id,
                "version": "1.0.0",
                "source": {
                    "repository": f"https://example.invalid/{component_id}",
                    "ref": "v1.0.0",
                    "commit": "0" * 40,
                    "path": ".",
                },
                "relationships": ["translated"],
                "license_expression": "MIT",
                "license_files": [
                    {"path": f"THIRD_PARTY_LICENSES/{component_id}/LICENSE"}
                ],
                "notice": f"{component_id} notice.",
            }
        )
    contract = root / "docs/release/THIRD_PARTY_COMPONENTS.json"
    contract.parent.mkdir(parents=True)
    contract.write_text(json.dumps({"components": components}), encoding="utf-8")


if __name__ == "__main__":
    unittest.main()
