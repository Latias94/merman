#!/usr/bin/env python3
"""Unit tests for release surface verifier helpers."""

from __future__ import annotations

import importlib.util
import json
import subprocess
import tempfile
import textwrap
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
    def test_package_inventory_skips_downloaded_vscode_test_runtime(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            write(root, "source/package.json", json.dumps({"name": "source"}))
            write(
                root,
                "tools/vscode-extension/.vscode-test/vscode/package.json",
                json.dumps({"name": "downloaded-vscode"}),
            )

            manifests = {
                path.relative_to(root).as_posix()
                for path in verify_release_surfaces.iter_package_jsons(root)
            }
            self.assertEqual(manifests, {"source/package.json"})

    def test_package_inventory_rejects_unallowlisted_package_json(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            write(root, "package.json", json.dumps({"name": "internal-root"}))
            write(root, "playground/package.json", json.dumps({"name": "playground", "private": True}))
            write(
                root,
                "playground/tests/package.json",
                json.dumps({"name": "@merman/playground-browser-tests", "private": True}),
            )
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
                "playground/tests/package.json",
                json.dumps({"name": "@merman/playground-browser-tests", "private": True}),
            )
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
            subprocess.run(["git", "init", "-q", str(root)], check=True)
            write(root, ".gitignore", "/platforms/web/pkg/\n")
            write(root, "playground/package.json", json.dumps({"name": "playground", "private": True}))
            write(
                root,
                "playground/tests/package.json",
                json.dumps({"name": "@merman/playground-browser-tests", "private": True}),
            )
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

    def test_conditionally_not_applicable_channels_must_explain_why(self) -> None:
        contract = {
            "surfaces": [
                {
                    "id": "homebrew",
                    "channels": [
                        {
                            "id": "homebrew-core",
                            "declared_state": "published",
                            "release_kinds": ["stable"],
                        }
                    ],
                }
            ]
        }
        with self.assertRaisesRegex(
            verify_release_surfaces.CheckFailure,
            "conditionally not-applicable channels must explain why",
        ):
            verify_release_surfaces.check_blocked_channel_metadata(contract)

    def test_web_contract_rejects_analysis_subpath_export(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            write_minimal_web_surface(root, extra_exports={"./analysis": "./analysis.js"})

            with self.assertRaisesRegex(
                verify_release_surfaces.CheckFailure,
                "unexpected: ./analysis",
            ):
                verify_release_surfaces.check_web_contract(root, web_contract())

    def test_web_contract_rejects_wrong_export_target(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            write_minimal_web_surface(root)
            package = json.loads((root / "platforms/web/package.json").read_text())
            package["exports"]["./core"]["import"] = "./dist/surfaces/ascii.js"
            write(root, "platforms/web/package.json", json.dumps(package))

            with self.assertRaisesRegex(
                verify_release_surfaces.CheckFailure,
                "wrong targets: ./core",
            ):
                verify_release_surfaces.check_web_contract(root, web_contract())

    def test_web_contract_rejects_capabilities_that_disagree_with_features(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            write_minimal_web_surface(root)
            descriptor_path = root / verify_release_surfaces.WEB_SURFACE_DESCRIPTOR_PATH
            descriptor = json.loads(descriptor_path.read_text())
            core = next(preset for preset in descriptor["presets"] if preset["name"] == "browser-core")
            core["capabilities"]["render"] = True
            write(root, verify_release_surfaces.WEB_SURFACE_DESCRIPTOR_PATH, json.dumps(descriptor))

            with self.assertRaisesRegex(
                verify_release_surfaces.CheckFailure,
                "capabilities do not match its Cargo feature closure",
            ):
                verify_release_surfaces.check_web_contract(root, web_contract())


class WorkflowOperationContractTests(unittest.TestCase):
    def test_declared_job_must_exist_and_invoke_the_channel_operation(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            write(
                root,
                ".github/workflows/release-web.yml",
                textwrap.dedent("""
                name: release
                jobs:
                  publish:
                    runs-on: ubuntu-latest
                    permissions:
                      id-token: write
                    steps:
                      - name: Publish package
                        run: npm publish package.tgz --access public
                """),
            )
            contract = operational_workflow_contract("npm", ".github/workflows/release-web.yml")

            verify_release_surfaces.check_workflow_operations(root, contract)

            contract["surfaces"][0]["channels"][0]["workflow_job"] = "missing"
            with self.assertRaisesRegex(
                verify_release_surfaces.CheckFailure,
                "workflow job not found: missing",
            ):
                verify_release_surfaces.check_workflow_operations(root, contract)

    def test_dead_jobs_steps_and_heredocs_cannot_impersonate_operations(self) -> None:
        cases = [
            {"if": "false", "steps": [{"run": "npm publish package.tgz"}]},
            {"steps": [{"if": "${{ false }}", "run": "npm publish package.tgz"}]},
            {"steps": [{"run": "if false; then\n  npm publish package.tgz\nfi"}]},
            {"steps": [{"run": "false && npm publish package.tgz"}]},
            {"steps": [{"run": "cat <<'EOF'\nnpm publish package.tgz\nEOF"}]},
        ]
        for job in cases:
            with self.subTest(job=job):
                self.assertFalse(
                    verify_release_surfaces.workflow_job_performs_channel_operation(job, "npm")
                )

    def test_publish_operations_require_effective_permissions(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            write(
                root,
                ".github/workflows/release-web.yml",
                textwrap.dedent("""
                name: release
                permissions:
                  contents: read
                jobs:
                  publish:
                    runs-on: ubuntu-latest
                    steps:
                      - name: Publish package
                        run: npm publish package.tgz --access public
                """),
            )
            contract = operational_workflow_contract("npm", ".github/workflows/release-web.yml")
            with self.assertRaisesRegex(
                verify_release_surfaces.CheckFailure,
                "requires id-token: write",
            ):
                verify_release_surfaces.check_workflow_operations(root, contract)

    def test_explicit_empty_job_permissions_do_not_inherit_workflow_permissions(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            write(
                root,
                ".github/workflows/release-web.yml",
                textwrap.dedent("""
                name: release
                permissions:
                  id-token: write
                jobs:
                  publish:
                    runs-on: ubuntu-latest
                    permissions:
                    steps:
                      - run: npm publish package.tgz
                """),
            )
            contract = operational_workflow_contract("npm", ".github/workflows/release-web.yml")
            with self.assertRaisesRegex(
                verify_release_surfaces.CheckFailure,
                "requires id-token: write",
            ):
                verify_release_surfaces.check_workflow_operations(root, contract)

    def test_release_credentials_must_be_trusted_nonempty_expressions(self) -> None:
        for token in ["", "${{ inputs.token }}"]:
            with self.subTest(token=token), tempfile.TemporaryDirectory() as temp_dir:
                root = Path(temp_dir)
                write(
                    root,
                    ".github/workflows/release.yml",
                    textwrap.dedent(f"""
                    name: release
                    jobs:
                      publish:
                        runs-on: ubuntu-latest
                        permissions:
                          contents: write
                        steps:
                          - name: Upload release
                            env:
                              GH_REPO: ${{{{ github.repository }}}}
                              GH_TOKEN: "{token}"
                            run: gh release upload "$RELEASE_TAG" asset.zip
                    """),
                )
                contract = operational_workflow_contract(
                    "github-release-assets", ".github/workflows/release.yml"
                )
                with self.assertRaisesRegex(
                    verify_release_surfaces.CheckFailure,
                    "requires credential environment keys",
                ):
                    verify_release_surfaces.check_workflow_operations(root, contract)

    def test_comments_and_echoes_cannot_impersonate_a_publish_command(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            write(
                root,
                ".github/workflows/release-web.yml",
                textwrap.dedent("""
                name: release
                jobs:
                  publish:
                    runs-on: ubuntu-latest
                    steps:
                      - name: Pretend publication
                        run: |
                          # npm publish package.tgz
                          echo "npm publish package.tgz"
                """),
            )
            contract = operational_workflow_contract("npm", ".github/workflows/release-web.yml")

            with self.assertRaisesRegex(
                verify_release_surfaces.CheckFailure,
                "does not perform the declared npm operation",
            ):
                verify_release_surfaces.check_workflow_operations(root, contract)

    def test_action_channels_require_the_active_action_step(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            write(
                root,
                ".github/workflows/vscode-extension.yml",
                textwrap.dedent("""
                name: package
                jobs:
                  publish:
                    runs-on: ubuntu-latest
                    strategy:
                      matrix:
                        include:
                          - os: ubuntu-latest
                            target: linux-x64
                          - os: macos-latest
                            target: darwin-arm64
                    steps:
                      - name: Upload package
                        uses: actions/upload-artifact@v6
                        with:
                          name: package-${{ steps.meta.outputs.release_version }}-${{ steps.meta.outputs.release_channel }}-${{ steps.meta.outputs.source_sha }}-${{ matrix.target }}
                          path: package.vsix
                """),
            )
            contract = operational_workflow_contract(
                "github-actions-artifact",
                ".github/workflows/vscode-extension.yml",
            )
            contract["surfaces"][0]["channels"][0]["artifact_patterns"] = [
                {
                    "glob": "package-{version}-{channel}-{source_sha}-linux-x64",
                    "min_matches": 1,
                    "max_matches": 1,
                },
                {
                    "glob": "package-{version}-{channel}-{source_sha}-darwin-arm64",
                    "min_matches": 1,
                    "max_matches": 1,
                },
            ]

            verify_release_surfaces.check_workflow_operations(root, contract)

            contract["surfaces"][0]["channels"][0]["artifact_patterns"].pop()
            with self.assertRaisesRegex(
                verify_release_surfaces.CheckFailure,
                "artifact patterns and workflow matrix targets differ",
            ):
                verify_release_surfaces.check_workflow_operations(root, contract)

    def test_cargo_dist_assets_are_derived_from_packages_targets_and_installers(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            write(
                root,
                "dist-workspace.toml",
                textwrap.dedent("""
                [dist]
                packages = ["merman-cli"]
                targets = ["x86_64-unknown-linux-gnu", "x86_64-pc-windows-msvc"]
                installers = ["shell", "powershell"]
                """),
            )
            expected = [
                "merman-cli-x86_64-unknown-linux-gnu.tar.xz",
                "merman-cli-x86_64-unknown-linux-gnu.tar.xz.sha256",
                "merman-cli-x86_64-pc-windows-msvc.zip",
                "merman-cli-x86_64-pc-windows-msvc.zip.sha256",
                "merman-cli-installer.sh",
                "merman-cli-installer.ps1",
            ]
            contract = {
                "surfaces": [
                    {
                        "packages": [
                            {
                                "kind": "crate",
                                "name": "merman-cli",
                                "manifest": "crates/merman-cli/Cargo.toml",
                            }
                        ],
                        "channels": [
                            {
                                "kind": "github-release-assets",
                                "asset_patterns": [
                                    {"glob": glob, "min_matches": 1, "max_matches": 1}
                                    for glob in expected
                                ],
                            }
                        ],
                    }
                ]
            }

            verify_release_surfaces.check_cargo_dist_asset_contract(root, contract)

            contract["surfaces"][0]["channels"][0]["asset_patterns"].pop()
            with self.assertRaisesRegex(
                verify_release_surfaces.CheckFailure,
                "cargo-dist asset contract differs",
            ):
                verify_release_surfaces.check_cargo_dist_asset_contract(root, contract)


def minimal_web_descriptor() -> dict:
    preset_features = {
        "browser-bridge": [],
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
                "capabilities": preset_capabilities(
                    features,
                    default_features=name in {"browser-full", "browser-ratex-math"},
                ),
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
            "browser_presets": [
                "browser-bridge",
                "browser-core",
                "browser-render",
                "browser-render-only",
                "browser-ascii",
                "browser-editor",
                "browser-full",
                "browser-full-no-elk",
                "browser-ratex-math",
            ],
            "web_auxiliary_exports": {
                ".": {"import": "./dist/index.js", "types": "./dist/index.d.ts"},
                "./catalog": {
                    "import": "./dist/public-catalog.js",
                    "types": "./dist/public-catalog.d.ts",
                },
            },
        }
    }


def operational_workflow_contract(kind: str, workflow: str) -> dict:
    return {
        "surfaces": [
            {
                "id": "example",
                "channels": [
                    {
                        "id": "publish",
                        "kind": kind,
                        "declared_state": "published",
                        "workflow": workflow,
                        "workflow_job": "publish",
                    }
                ],
            }
        ]
    }


def write_minimal_web_surface(root: Path, *, extra_exports: dict[str, str] | None = None) -> None:
    contract = web_contract()
    descriptor = minimal_web_descriptor()
    exports = verify_release_surfaces.expected_web_package_exports(
        descriptor,
        contract["feature_contract"],
    )
    exports.update(extra_exports or {})
    write(
        root,
        "platforms/web/package.json",
        json.dumps({"name": "@mermanjs/web", "version": "0.8.0-alpha.3", "exports": exports}),
    )
    write(
        root,
        verify_release_surfaces.WEB_SURFACE_DESCRIPTOR_PATH,
        json.dumps(descriptor),
    )
    write(
        root,
        "crates/merman-wasm/Cargo.toml",
        """
        [package]
        name = "merman-wasm"

        [features]
        default = ["core-full", "core-host", "render", "analysis", "ascii", "elk-layout", "editor-language"]
        core-full = []
        core-host = []
        analysis = []
        ascii = []
        render = []
        cytoscape-layout = []
        elk-layout = []
        editor-language = ["analysis"]
        ratex-math = ["render"]
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


def preset_capabilities(features: list[str], *, default_features: bool) -> dict[str, bool]:
    enabled = set(features)
    if default_features:
        enabled.update(
            {
                "core-full",
                "core-host",
                "render",
                "analysis",
                "ascii",
                "elk-layout",
                "editor-language",
            }
        )
    if "editor-language" in enabled:
        enabled.add("analysis")
    if "ratex-math" in enabled:
        enabled.add("render")
    return {
        capability: feature in enabled
        for capability, feature in verify_release_surfaces.WEB_CAPABILITY_FEATURES.items()
    }


def write(root: Path, rel_path: str, text: str) -> None:
    path = root / rel_path
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


if __name__ == "__main__":
    unittest.main()
