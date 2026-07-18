#!/usr/bin/env python3
"""Unit tests for release surface verifier helpers."""

from __future__ import annotations

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


MODULE_PATH = Path(__file__).with_name("verify-release-surfaces.py")
SPEC = importlib.util.spec_from_file_location("verify_release_surfaces", MODULE_PATH)
assert SPEC is not None
verify_release_surfaces = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(verify_release_surfaces)


class ReleaseSurfaceParsingTests(unittest.TestCase):
    def test_validate_web_descriptor_reads_closed_preset_and_surface_graph(self) -> None:
        descriptor = verify_release_surfaces.validate_web_surface_descriptor(
            minimal_web_descriptor()
        )

        self.assertEqual(descriptor["schema_version"], 1)
        self.assertEqual(descriptor["default_preset"], "browser-full")
        self.assertEqual(
            {preset["name"] for preset in descriptor["presets"]},
            set(web_contract()["feature_contract"]["browser_presets"]),
        )
        self.assertEqual(
            {surface["entry"] for surface in descriptor["public_surfaces"]},
            {"core", "render", "render-only", "ascii", "editor", "full"},
        )

    def test_validate_web_descriptor_rejects_duplicates_and_dangling_references(self) -> None:
        duplicate = minimal_web_descriptor()
        duplicate["presets"].append(dict(duplicate["presets"][0]))
        with self.assertRaisesRegex(
            verify_release_surfaces.CheckFailure,
            "duplicate Web preset name",
        ):
            verify_release_surfaces.validate_web_surface_descriptor(duplicate)

        dangling = minimal_web_descriptor()
        dangling["public_surfaces"][0]["preset"] = "browser-missing"
        with self.assertRaisesRegex(
            verify_release_surfaces.CheckFailure,
            "references unknown preset browser-missing",
        ):
            verify_release_surfaces.validate_web_surface_descriptor(dangling)

    def test_validate_web_descriptor_rejects_incomplete_capabilities(self) -> None:
        descriptor = minimal_web_descriptor()
        del descriptor["presets"][0]["capabilities"]["editor_language"]

        with self.assertRaisesRegex(
            verify_release_surfaces.CheckFailure,
            "capabilities keys must be exactly",
        ):
            verify_release_surfaces.validate_web_surface_descriptor(descriptor)

    def test_package_manifest_name_reads_multiple_manifest_formats(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            write(root, "package.json", json.dumps({"name": "@scope/pkg"}))
            write(root, "Cargo.toml", "[package]\nname = \"merman-core\"\n")
            write(root, "pyproject.toml", "[project]\nname = \"merman\"\n")
            write(root, "pubspec.yaml", "name: merman\n")

            self.assertEqual(
                verify_release_surfaces.package_manifest_name(root, "npm", "package.json"),
                "@scope/pkg",
            )
            self.assertEqual(
                verify_release_surfaces.package_manifest_name(root, "crate", "Cargo.toml"),
                "merman-core",
            )
            self.assertEqual(
                verify_release_surfaces.package_manifest_name(root, "python", "pyproject.toml"),
                "merman",
            )
            self.assertEqual(
                verify_release_surfaces.package_manifest_name(root, "flutter", "pubspec.yaml"),
                "merman",
            )


class ReleaseSurfaceInventoryTests(unittest.TestCase):
    def test_package_inventory_rejects_unallowlisted_package_json(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            write(root, "package.json", json.dumps({"name": "internal-root"}))
            write(root, "playground/package.json", json.dumps({"name": "playground", "private": True}))
            write(
                root,
                "tools/mermaid-cli/package.json",
                json.dumps({"name": "mermaid-cli", "private": True}),
            )
            write(root, "platforms/web/package.json", json.dumps({"name": "@mermanjs/web"}))
            write(root, "unknown/package.json", json.dumps({"name": "unknown"}))
            contract = {
                "surfaces": [
                    {
                        "packages": [
                            {
                                "kind": "npm",
                                "name": "@mermanjs/web",
                                "manifest": "platforms/web/package.json",
                            }
                        ]
                    }
                ]
            }

            with self.assertRaisesRegex(
                verify_release_surfaces.CheckFailure,
                "unknown/package.json",
            ):
                verify_release_surfaces.check_package_inventory(root, contract)

    def test_package_inventory_allows_missing_optional_root_package_json(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            write(root, "playground/package.json", json.dumps({"name": "playground", "private": True}))
            write(
                root,
                "tools/mermaid-cli/package.json",
                json.dumps({"name": "mermaid-cli", "private": True}),
            )
            write(root, "platforms/web/package.json", json.dumps({"name": "@mermanjs/web"}))
            contract = {
                "surfaces": [
                    {
                        "packages": [
                            {
                                "kind": "npm",
                                "name": "@mermanjs/web",
                                "manifest": "platforms/web/package.json",
                            }
                        ]
                    }
                ]
            }

            verify_release_surfaces.check_package_inventory(root, contract)

    def test_package_inventory_allows_internal_generated_web_presets(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            write(root, "playground/package.json", json.dumps({"name": "playground", "private": True}))
            write(
                root,
                "tools/mermaid-cli/package.json",
                json.dumps({"name": "mermaid-cli", "private": True}),
            )
            write(root, "platforms/web/package.json", json.dumps({"name": "@mermanjs/web"}))
            write(
                root,
                "platforms/web/pkg/full-no-elk/package.json",
                json.dumps({"name": "@mermanjs/web-full-no-elk"}),
            )
            write(
                root,
                "platforms/web/pkg/ratex-math/package.json",
                json.dumps({"name": "@mermanjs/web-ratex-math"}),
            )
            contract = {
                "surfaces": [
                    {
                        "packages": [
                            {
                                "kind": "npm",
                                "name": "@mermanjs/web",
                                "manifest": "platforms/web/package.json",
                            }
                        ]
                    }
                ]
            }

            verify_release_surfaces.check_package_inventory(root, contract)

    def test_package_inventory_requires_tracked_non_surface_package_jsons(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            write(root, "platforms/web/package.json", json.dumps({"name": "@mermanjs/web"}))
            contract = {
                "surfaces": [
                    {
                        "packages": [
                            {
                                "kind": "npm",
                                "name": "@mermanjs/web",
                                "manifest": "platforms/web/package.json",
                            }
                        ]
                    }
                ]
            }

            with self.assertRaisesRegex(
                verify_release_surfaces.CheckFailure,
                "allowlisted non-surface package manifest is missing",
            ):
                verify_release_surfaces.check_package_inventory(root, contract)

    def test_blocked_channels_must_explain_blocker(self) -> None:
        contract = {
            "surfaces": [
                {
                    "id": "vscode",
                    "channels": [
                        {
                            "id": "vs-marketplace",
                            "declared_state": "credential-blocked",
                            "release_kinds": ["stable", "prerelease"],
                        }
                    ],
                }
            ]
        }

        with self.assertRaisesRegex(
            verify_release_surfaces.CheckFailure,
            "credential-blocked channels must name the missing credential",
        ):
            verify_release_surfaces.check_blocked_channel_metadata(contract)

    def test_web_contract_rejects_analysis_subpath_export(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            write_minimal_web_surface(root, extra_exports={"./analysis": "./analysis.js"})

            with self.assertRaisesRegex(
                verify_release_surfaces.CheckFailure,
                "analysis is not a supported export",
            ):
                verify_release_surfaces.check_web_contract(root, web_contract())


def minimal_web_descriptor() -> dict:
    capabilities = {
        name: False for name in verify_release_surfaces.WEB_CAPABILITY_NAMES
    }
    preset_features = {
        "browser-core": ["analysis"],
        "browser-render": ["render", "analysis"],
        "browser-render-only": ["render"],
        "browser-ascii": ["ascii"],
        "browser-editor": ["core-full", "editor-language"],
        "browser-full": [],
        "browser-full-no-elk": [
            "core-full",
            "core-host",
            "render",
            "analysis",
            "ascii",
            "editor-language",
        ],
        "browser-ratex-math": ["ratex-math"],
    }
    public = [
        ("core", "browser-core"),
        ("render", "browser-render"),
        ("render-only", "browser-render-only"),
        ("ascii", "browser-ascii"),
        ("editor", "browser-editor"),
        ("full", "browser-full"),
    ]
    return {
        "schema_version": 1,
        "default_preset": "browser-full",
        "presets": [
            {
                "name": name,
                "surface": "browser",
                "default_features": name in {"browser-full", "browser-ratex-math"},
                "features": features,
                "capabilities": dict(capabilities),
            }
            for name, features in preset_features.items()
        ],
        "public_surfaces": [
            {
                "entry": entry,
                "preset": preset,
                "pkg_dir_rel": f"pkg/{entry}",
                "runtime_profile": entry,
            }
            for entry, preset in public
        ],
    }


def web_contract() -> dict:
    return {
        "feature_contract": {
            "web_descriptor": verify_release_surfaces.WEB_SURFACE_DESCRIPTOR_PATH,
            "web_default_preset": "browser-full",
            "web_subpaths": [".", "./core", "./render", "./render-only", "./ascii", "./editor", "./full"],
            "browser_presets": [
                "browser-core",
                "browser-render",
                "browser-render-only",
                "browser-ascii",
                "browser-editor",
                "browser-full",
                "browser-full-no-elk",
                "browser-ratex-math",
            ],
        }
    }


def write_minimal_web_surface(root: Path, *, extra_exports: dict[str, str] | None = None) -> None:
    exports = {
        ".": "./index.js",
        "./core": "./core.js",
        "./render": "./render.js",
        "./render-only": "./render-only.js",
        "./ascii": "./ascii.js",
        "./editor": "./editor.js",
        "./full": "./full.js",
    }
    exports.update(extra_exports or {})
    write(
        root,
        "platforms/web/package.json",
        json.dumps({"name": "@mermanjs/web", "version": "0.8.0-alpha.3", "exports": exports}),
    )
    write(
        root,
        verify_release_surfaces.WEB_SURFACE_DESCRIPTOR_PATH,
        json.dumps(minimal_web_descriptor()),
    )
    write(
        root,
        "crates/merman-wasm/Cargo.toml",
        """
        [package]
        name = "merman-wasm"

        [features]
        core-full = []
        core-host = []
        analysis = []
        ascii = []
        render = []
        cytoscape-layout = []
        elk-layout = []
        editor-language = []
        ratex-math = []
        """,
    )
    for subdir in ["core", "render", "render-only", "ascii", "editor", "full"]:
        write(root, f"platforms/web/pkg/{subdir}/README.md", "# package\n")
    docs = "\n".join(
        f"@mermanjs/web/{surface['entry']}"
        for surface in minimal_web_descriptor()["public_surfaces"]
    )
    write(root, "README.md", docs)
    write(root, "platforms/web/README.md", docs)
    write(root, "docs/release/PACKAGE_SURFACES.md", docs)


def write(root: Path, rel_path: str, text: str) -> None:
    path = root / rel_path
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


if __name__ == "__main__":
    unittest.main()
