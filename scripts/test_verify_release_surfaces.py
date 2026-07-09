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
    def test_extract_browser_presets_reads_build_wasm_keys(self) -> None:
        presets = verify_release_surfaces.extract_browser_presets(
            """
            const presets = {
              "browser-core": { features: ["analysis"] },
              "browser-render-only": { features: ["render"] },
            };
            """
        )

        self.assertEqual(presets, {"browser-core", "browser-render-only"})

    def test_extract_wrapper_surfaces_reads_entry_to_preset_pairs(self) -> None:
        wrappers = verify_release_surfaces.extract_wrapper_surfaces(
            """
            export const surfaces = [
              { entry: "core", preset: "browser-core" },
              { entry: "render-only", preset: "browser-render-only" },
            ];
            """
        )

        self.assertEqual(
            wrappers,
            {("core", "browser-core"), ("render-only", "browser-render-only")},
        )

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


def web_contract() -> dict:
    return {
        "feature_contract": {
            "web_subpaths": [".", "./core", "./render", "./render-only", "./ascii", "./full"],
            "browser_presets": [
                "browser-core",
                "browser-render",
                "browser-render-only",
                "browser-ascii",
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
        "platforms/web/scripts/build-wasm.mjs",
        """
        const presets = {
          "browser-core": {},
          "browser-render": {},
          "browser-render-only": {},
          "browser-ascii": {},
          "browser-full": {},
          "browser-full-no-elk": {},
          "browser-ratex-math": {},
        };
        """,
    )
    write(
        root,
        "platforms/web/scripts/surface-manifest.mjs",
        """
        export const surfaces = [
          { entry: "core", preset: "browser-core" },
          { entry: "render", preset: "browser-render" },
          { entry: "render-only", preset: "browser-render-only" },
          { entry: "ascii", preset: "browser-ascii" },
          { entry: "full", preset: "browser-full" },
        ];
        """,
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
    for subdir in ["core", "render", "render-only", "ascii", "full"]:
        write(root, f"platforms/web/pkg/{subdir}/README.md", "# package\n")
    docs = "\n".join(verify_release_surfaces.REQUIRED_WEB_DOC_SUBPATHS)
    write(root, "README.md", docs)
    write(root, "platforms/web/README.md", docs)
    write(root, "docs/release/PACKAGE_SURFACES.md", docs)


def write(root: Path, rel_path: str, text: str) -> None:
    path = root / rel_path
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


if __name__ == "__main__":
    unittest.main()
