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
    def test_validate_web_descriptor_reads_closed_package_graph(self) -> None:
        descriptor = verify_release_surfaces.validate_web_surface_descriptor(
            minimal_web_descriptor()
        )

        self.assertEqual(descriptor["schema_version"], 1)
        self.assertEqual(descriptor["default_package"], "full")
        self.assertEqual(
            {package["name"] for package in descriptor["packages"]},
            {
                "@mermanjs/web",
                "@mermanjs/web-analysis",
                "@mermanjs/web-ascii",
                "@mermanjs/web-editor",
                "@mermanjs/web-render",
            },
        )
        self.assertEqual(
            {package["id"] for package in descriptor["packages"] if package["visibility"] == "public"},
            {"full", "analysis", "ascii", "editor", "render"},
        )

    def test_validate_web_descriptor_rejects_old_schema_and_duplicates(self) -> None:
        duplicate = minimal_web_descriptor()
        duplicate["packages"].append(dict(duplicate["packages"][0]))
        with self.assertRaisesRegex(
            verify_release_surfaces.CheckFailure,
            "(?i)duplicate package id",
        ):
            verify_release_surfaces.validate_web_surface_descriptor(duplicate)

        old = minimal_web_descriptor()
        old["presets"] = []
        with self.assertRaisesRegex(
            verify_release_surfaces.CheckFailure,
            "fields must be exact",
        ):
            verify_release_surfaces.validate_web_surface_descriptor(old)

    def test_validate_web_descriptor_rejects_candidate_default(self) -> None:
        descriptor = minimal_web_descriptor()
        full = next(package for package in descriptor["packages"] if package["id"] == "full")
        render = next(
            package for package in descriptor["packages"] if package["id"] == "render"
        )
        full["name"] = "@mermanjs/web-full"
        render["name"] = "@mermanjs/web"
        render["visibility"] = "candidate"
        descriptor["default_package"] = "render"

        with self.assertRaisesRegex(
            verify_release_surfaces.CheckFailure,
            "default package visibility must be 'public'",
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

    def test_package_manifest_name_reports_missing_fields_as_check_failures(self) -> None:
        cases = [
            ("npm", "package.json", "{}"),
            ("vscode", "vscode.json", '{"version": "0.1.0"}'),
            ("crate", "Cargo.toml", "[workspace]\n"),
            ("python", "pyproject.toml", "[project]\nversion = \"1.0.0\"\n"),
            ("typst", "typst.toml", "[package]\nversion = \"1.0.0\"\n"),
            ("flutter", "pubspec.yaml", "version: 1.0.0\n"),
            ("android", "build.gradle.kts", 'group = "io.merman"\n'),
            ("swiftpm", "Package.swift", "let unrelated = 1\n"),
        ]
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            for kind, manifest, contents in cases:
                with self.subTest(kind=kind):
                    write(root, manifest, contents)
                    with self.assertRaises(verify_release_surfaces.CheckFailure):
                        verify_release_surfaces.package_manifest_name(root, kind, manifest)

    def test_android_and_swiftpm_manifest_names_ignore_comments(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            write(
                root,
                "build.gradle.kts",
                textwrap.dedent('''
                // group = "comment.decoy"
                /* artifactId = "comment-decoy" */
                group = "io.merman"
                publishing {
                    artifactId = "merman-android"
                    description = "literal // and /* text are not comments"
                }
                '''),
            )
            write(
                root,
                "Package.swift",
                textwrap.dedent('''
                // let package = Package(name: "LineCommentDecoy")
                /* let package = Package(name: "BlockCommentDecoy") */
                let package = Package(
                    name: "Merman"
                )
                '''),
            )

            self.assertEqual(
                verify_release_surfaces.package_manifest_name(
                    root, "android", "build.gradle.kts"
                ),
                "io.merman:merman-android",
            )
            self.assertEqual(
                verify_release_surfaces.package_manifest_name(root, "swiftpm", "Package.swift"),
                "Merman",
            )

    def test_ci_wiring_requires_executable_verifier_commands(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            workflow = """
                name: ci
                jobs:
                  release-contract:
                    runs-on: ubuntu-latest
                    steps:
                      - run: |
                          python3 scripts/release-version.py
                          python3 scripts/verify-release-surfaces.py
                          python3 -m unittest \\
                            scripts/test_release_readme.py \\
                            scripts/test_release_status.py \\
                            scripts/test_verify_release_surfaces.py \\
                            scripts/test_web_package_group.py
            """
            write(root, ".github/workflows/ci.yml", textwrap.dedent(workflow))
            verify_release_surfaces.check_ci_wiring(root)

            commented_version = workflow.replace(
                "python3 scripts/release-version.py",
                "# python3 scripts/release-version.py",
            )
            write(root, ".github/workflows/ci.yml", textwrap.dedent(commented_version))
            with self.assertRaisesRegex(
                verify_release_surfaces.CheckFailure,
                "does not execute python3 scripts/release-version.py",
            ):
                verify_release_surfaces.check_ci_wiring(root)

            commented = workflow.replace(
                "python3 scripts/verify-release-surfaces.py",
                "# python3 scripts/verify-release-surfaces.py",
            )
            write(root, ".github/workflows/ci.yml", textwrap.dedent(commented))
            with self.assertRaisesRegex(
                verify_release_surfaces.CheckFailure,
                "does not execute python3 scripts/verify-release-surfaces.py",
            ):
                verify_release_surfaces.check_ci_wiring(root)


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

    def test_package_inventory_allows_private_node_candidate_manifests(self) -> None:
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
            for manifest in sorted(
                path
                for path in verify_release_surfaces.OPTIONAL_NON_SURFACE_PACKAGE_MANIFESTS
                if path.startswith("platforms/node/")
            ):
                write(root, manifest, json.dumps({"name": manifest, "private": True}))
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

    def test_package_inventory_rejects_public_node_candidate_manifest(self) -> None:
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
            for manifest in sorted(
                path
                for path in verify_release_surfaces.OPTIONAL_NON_SURFACE_PACKAGE_MANIFESTS
                if path.startswith("platforms/node/")
            ):
                private = manifest != "platforms/node/packages/node/package.json"
                write(root, manifest, json.dumps({"name": manifest, "private": private}))
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
                "non-surface package manifest must set private: true",
            ):
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
                "platforms/web/pkg/math/package.json",
                json.dumps({"name": "@mermanjs/web-math"}),
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

    def test_package_inventory_ignores_git_ignored_generated_manifest(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            subprocess.run(["git", "init", "-q", str(root)], check=True)
            write(root, ".gitignore", "ignored/\n")
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
            write(root, "ignored/package.json", json.dumps({"name": "generated-package"}))
            subprocess.run(
                ["git", "-C", str(root), "check-ignore", "-q", "ignored/package.json"],
                check=True,
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

    def test_package_inventory_rejects_nonignored_untracked_manifest(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            subprocess.run(["git", "init", "-q", str(root)], check=True)
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
            write(root, "untracked/package.json", json.dumps({"name": "undeclared-package"}))
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
                "untracked/package.json",
            ):
                verify_release_surfaces.check_package_inventory(root, contract)

    def test_package_inventory_requires_allowlisted_non_surface_package_jsons(self) -> None:
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

    def test_web_contract_rejects_candidate_package_in_release_group(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            write_minimal_web_surface(root)
            descriptor_path = root / verify_release_surfaces.WEB_SURFACE_DESCRIPTOR_PATH
            descriptor = json.loads(descriptor_path.read_text())
            render = next(
                package for package in descriptor["packages"] if package["id"] == "render"
            )
            render["visibility"] = "candidate"
            write(
                root,
                verify_release_surfaces.WEB_SURFACE_DESCRIPTOR_PATH,
                json.dumps(descriptor),
            )
            render_manifest_path = root / "platforms/web/packages/render/package.json"
            render_manifest = json.loads(render_manifest_path.read_text())
            render_manifest["private"] = True
            render_manifest.pop("publishConfig")
            write(
                root,
                "platforms/web/packages/render/package.json",
                json.dumps(render_manifest),
            )
            contract = web_contract()

            with self.assertRaisesRegex(
                verify_release_surfaces.CheckFailure,
                "must exactly match public descriptor packages",
            ):
                verify_release_surfaces.check_web_contract(root, contract)

    def test_web_contract_rejects_legacy_raw_wasm_export(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            write_minimal_web_surface(root)
            package_path = root / "platforms/web/packages/full/package.json"
            package = json.loads(package_path.read_text())
            package["exports"]["./pkg/merman_wasm_bg.wasm"] = "./pkg/merman_wasm_bg.wasm"
            write(root, "platforms/web/packages/full/package.json", json.dumps(package))

            with self.assertRaisesRegex(
                verify_release_surfaces.CheckFailure,
                r"export exactly '\.'",
            ):
                verify_release_surfaces.check_web_contract(root, web_contract())

    def test_web_contract_rejects_unknown_artifact_profile(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            write_minimal_web_surface(root)
            profiles_path = root / "capabilities/artifact-profiles-v1.json"
            profiles = json.loads(profiles_path.read_text())
            profiles["profiles"] = [
                profile
                for profile in profiles["profiles"]
                if profile["id"] != "web-full"
            ]
            write(root, "capabilities/artifact-profiles-v1.json", json.dumps(profiles))

            with self.assertRaisesRegex(
                verify_release_surfaces.CheckFailure,
                "references missing artifact profile",
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
                    environment: npm
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
            {"steps": [{"run": "exit 0\nnpm publish package.tgz"}]},
            {"steps": [{"run": "return 0\nnpm publish package.tgz"}]},
            {"steps": [{"run": "publish_package() {\n  npm publish package.tgz\n}\necho no-op"}]},
            {
                "steps": [
                    {
                        "run": "publish_package() {\n  return 0\n  npm publish package.tgz\n}\npublish_package"
                    }
                ]
            },
            {
                "steps": [
                    {
                        "run": "publish_package() {\n  {\n    :\n  }\n  npm publish package.tgz\n}\necho no-op"
                    }
                ]
            },
        ]
        for job in cases:
            with self.subTest(job=job):
                self.assertFalse(
                    verify_release_surfaces.workflow_job_performs_channel_operation(job, "npm")
                )

    def test_reachable_shell_function_can_prove_a_publish_operation(self) -> None:
        job = {
            "steps": [
                {
                    "run": "publish_package() {\n  npm publish package.tgz\n}\npublish_package"
                }
            ]
        }

        self.assertTrue(verify_release_surfaces.workflow_job_performs_channel_operation(job, "npm"))

    def test_conditional_exit_does_not_hide_a_reachable_publish_operation(self) -> None:
        job = {
            "steps": [
                {
                    "run": "if [[ -z \"$PACKAGE\" ]]; then\n  exit 1\nfi\nnpm publish package.tgz"
                }
            ]
        }

        self.assertTrue(verify_release_surfaces.workflow_job_performs_channel_operation(job, "npm"))

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
                    environment: npm
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
                    environment: npm
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
                        environment: github-release
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

    def test_publish_job_must_bind_the_contract_environment(self) -> None:
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
                    environment: staging
                    permissions:
                      id-token: write
                    steps:
                      - run: npm publish package.tgz
                """),
            )
            contract = operational_workflow_contract("npm", ".github/workflows/release-web.yml")

            with self.assertRaisesRegex(
                verify_release_surfaces.CheckFailure,
                "requires GitHub Environment 'npm'",
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
                          name: package-${{ steps.meta.outputs.extension_version }}-runtime-${{ steps.meta.outputs.runtime_version }}-${{ steps.meta.outputs.runtime_channel }}-${{ steps.meta.outputs.source_sha }}-${{ matrix.target }}
                          path: package.vsix
                """),
            )
            contract = operational_workflow_contract(
                "github-actions-artifact",
                ".github/workflows/vscode-extension.yml",
            )
            contract["surfaces"][0]["packages"] = [
                {
                    "kind": "vscode",
                    "name": "merman-vscode",
                    "manifest": "tools/vscode-extension/package.json",
                    "version_source": "manifest",
                }
            ]
            contract["surfaces"][0]["channels"][0]["artifact_patterns"] = [
                {
                    "glob": "package-{package_version}-runtime-{version}-{channel}-{source_sha}-linux-x64",
                    "min_matches": 1,
                    "max_matches": 1,
                },
                {
                    "glob": "package-{package_version}-runtime-{version}-{channel}-{source_sha}-darwin-arm64",
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

    def test_manifest_version_artifact_requires_package_version_output(self) -> None:
        job = {
            "matrix_include": [{"target": "linux-x64"}],
            "steps": [
                {
                    "uses": "actions/upload-artifact@v6",
                    "with": {
                        "name": "package-${{ steps.meta.outputs.release_version }}-${{ steps.meta.outputs.release_channel }}-${{ steps.meta.outputs.source_sha }}-${{ matrix.target }}"
                    },
                }
            ],
        }
        surface = {"packages": [{"version_source": "manifest"}]}
        channel = {
            "artifact_patterns": [
                {
                    "glob": "package-{version}-{channel}-{source_sha}-linux-x64",
                    "min_matches": 1,
                    "max_matches": 1,
                }
            ]
        }

        with self.assertRaisesRegex(
            verify_release_surfaces.CheckFailure,
            "must bind exactly one \\{package_version\\}",
        ):
            verify_release_surfaces.check_actions_artifact_contract(
                job,
                surface,
                channel,
                ".github/workflows/vscode-extension.yml",
                "vscode/github-actions-vsix",
            )

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
    return {
        "schema_version": 1,
        "default_package": "full",
        "packages": [
            {
                "id": "full",
                "name": "@mermanjs/web",
                "package_dir": "packages/full",
                "artifact_profile": "web-full",
                "runtime_profile": "full",
                "visibility": "public",
            },
            {
                "id": "analysis",
                "name": "@mermanjs/web-analysis",
                "package_dir": "packages/analysis",
                "artifact_profile": "web-analysis",
                "runtime_profile": "analysis",
                "visibility": "public",
            },
            {
                "id": "ascii",
                "name": "@mermanjs/web-ascii",
                "package_dir": "packages/ascii",
                "artifact_profile": "web-ascii",
                "runtime_profile": "ascii",
                "visibility": "public",
            },
            {
                "id": "editor",
                "name": "@mermanjs/web-editor",
                "package_dir": "packages/editor",
                "artifact_profile": "web-editor",
                "runtime_profile": "editor",
                "visibility": "public",
            },
            {
                "id": "render",
                "name": "@mermanjs/web-render",
                "package_dir": "packages/render",
                "artifact_profile": "web-render",
                "runtime_profile": "render",
                "visibility": "public",
            },
        ],
    }


def web_contract() -> dict:
    return {
        "feature_contract": {
            "web_descriptor": verify_release_surfaces.WEB_SURFACE_DESCRIPTOR_PATH,
            "web_package_group_surface": "web-package-group",
        },
        "surfaces": [
            {
                "id": "web-package-group",
                "packages": [
                    {
                        "kind": "npm",
                        "name": package["name"],
                        "manifest": f"platforms/web/{package['package_dir']}/package.json",
                    }
                    for package in minimal_web_descriptor()["packages"]
                    if package["visibility"] == "public"
                ],
            }
        ],
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
                        "environment": {
                            "crates.io": "crates.io",
                            "github-release-assets": "github-release",
                            "npm": "npm",
                            "pypi": "pypi",
                            "pub.dev": "pub.dev",
                        }.get(kind),
                    }
                ],
            }
        ]
    }


def write_minimal_web_surface(root: Path) -> None:
    descriptor = minimal_web_descriptor()
    write(
        root,
        "platforms/web/package.json",
        json.dumps({"name": "@mermanjs/web-workspace", "private": True}),
    )
    write(
        root,
        verify_release_surfaces.WEB_SURFACE_DESCRIPTOR_PATH,
        json.dumps(descriptor),
    )
    profiles = []
    for package in descriptor["packages"]:
        profiles.append({"id": package["artifact_profile"]})
        manifest = {
            "name": package["name"],
            "version": "0.8.0-alpha.4",
            "files": [
                "artifacts",
                "dist",
                "LICENSE",
                "README.md",
                "THIRD_PARTY_LICENSES",
                "THIRD_PARTY_NOTICES.md",
            ],
            "exports": {".": {"import": "./dist/index.js", "types": "./dist/index.d.ts"}},
        }
        if package["visibility"] == "candidate":
            manifest["private"] = True
        else:
            manifest["publishConfig"] = {"access": "public"}
        write(
            root,
            f"platforms/web/{package['package_dir']}/package.json",
            json.dumps(manifest),
        )
    write(root, "capabilities/artifact-profiles-v1.json", json.dumps({"profiles": profiles}))


def write(root: Path, rel_path: str, text: str) -> None:
    path = root / rel_path
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


if __name__ == "__main__":
    unittest.main()
