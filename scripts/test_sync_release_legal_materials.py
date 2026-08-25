#!/usr/bin/env python3
"""Unit tests for release legal material projections."""

from __future__ import annotations

import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path


MODULE_PATH = Path(__file__).with_name("sync-release-legal-materials.py")
SPEC = importlib.util.spec_from_file_location("sync_release_legal_materials", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
sync = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(sync)

GENERATOR_MODULE_PATH = Path(__file__).with_name("generate-rust-license-report.py")
GENERATOR_SPEC = importlib.util.spec_from_file_location(
    "generate_rust_license_report_contract",
    GENERATOR_MODULE_PATH,
)
assert GENERATOR_SPEC is not None and GENERATOR_SPEC.loader is not None
generator = importlib.util.module_from_spec(GENERATOR_SPEC)
sys.modules[GENERATOR_SPEC.name] = generator
GENERATOR_SPEC.loader.exec_module(generator)


class ReleaseLegalMaterialTests(unittest.TestCase):
    def test_native_report_sources_match_generator_outputs(self) -> None:
        generated_reports = {
            spec.output.parents[1].as_posix(): spec.output
            for spec in generator.NATIVE_REPORT_SPECS
        }

        self.assertEqual(sync.NATIVE_RUST_REPORTS, generated_reports)

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
            self.assertNotIn(
                root
                / "distribution/typst/merman/THIRD_PARTY_LICENSES/rust-cargo-dependencies.json",
                expected,
            )
            self.assertFalse(
                (
                    root
                    / "distribution/typst/merman/THIRD_PARTY_LICENSES/rust-cargo-dependencies.json"
                ).exists()
            )
            self.assertIn(
                root / "platforms/android/src/main/resources/META-INF/LICENSE",
                expected,
            )
            self.assertEqual(
                expected[
                    root
                    / "platforms/android/src/main/resources/META-INF/THIRD_PARTY_LICENSES/rust-cargo-dependencies.json"
                ],
                b"platforms/android exact report\n",
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
            external.write_bytes(b"generated npm report\n")
            expected = sync.expected_projections(root)

            sync.write_projections(root, expected)

            self.assertEqual(external.read_text(encoding="utf-8"), "generated npm report\n")
            self.assertEqual(sync.check_projections(root, expected), [])


def seed_root(root: Path) -> None:
    (root / "LICENSE-MIT").write_bytes(b"MIT\n")
    (root / "LICENSE-APACHE").write_bytes(b"Apache\n")
    (root / "THIRD_PARTY_NOTICES.md").write_bytes(b"Notices\n")
    licenses = root / "THIRD_PARTY_LICENSES"
    licenses.mkdir()
    (licenses / "rust-cargo-dependencies.json").write_bytes(b"workspace report\n")
    for bundle_root, report_path in sync.NATIVE_RUST_REPORTS.items():
        path = root / report_path
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(f"{bundle_root} exact report\n".encode())
    components = []
    component_ids = sorted(
        {
            component_id
            for values in sync.CRATE_COMPONENTS.values()
            for component_id in values
        }
        | {"eclipse-elk", "elkjs", "wasm-minimal-protocol", "ratex"}
    )
    for component_id in component_ids:
        license_file = licenses / component_id / "LICENSE"
        license_file.parent.mkdir()
        license_file.write_bytes(f"{component_id} terms\n".encode())
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
    typst_components = [
        component_id for component_id in component_ids if component_id != "ratex"
    ]
    contract = root / "docs/release/THIRD_PARTY_COMPONENTS.json"
    contract.parent.mkdir(parents=True)
    contract.write_bytes(
        json.dumps(
            {
                "artifact_scopes": [
                    {
                        "id": "typst-publish",
                        "description": "Typst publish fixture",
                        "extends": [],
                        "components": typst_components,
                    }
                ],
                "components": components,
            }
        ).encode()
    )


if __name__ == "__main__":
    unittest.main()
