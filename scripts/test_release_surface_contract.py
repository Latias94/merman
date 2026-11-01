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
    def setUp(self) -> None:
        packages = [
            {
                "id": name,
                "name": name,
                "manifest_path": str(ROOT / "crates" / name / "Cargo.toml"),
                "dependencies": [],
            }
            for name in (
                "merman-bindings-core", "merman-ffi", "merman-uniffi",
                "merman-wasm", "merman-wasi",
            )
        ]
        self.metadata = {
            "packages": packages,
            "workspace_members": [package["id"] for package in packages],
        }
        cargo = mock.patch.object(
            contract, "cargo_metadata", side_effect=AssertionError("unit tests must not run Cargo"),
        )
        cargo.start()
        self.addCleanup(cargo.stop)

    def test_repository_contract_matches_current_workflows_and_profiles(self) -> None:
        surfaces = contract.validate_repository(ROOT, metadata=self.metadata)

        self.assertEqual(
            {surface.surface_id for surface in surfaces},
            {
                "rust-crates-and-ffi-source",
                "android-aar",
                "apple-xcframework",
                "python-wheel",
                "flutter-pub",
                "headless-sdk-archives",
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
        self.assertEqual(len(report["surfaces"]), 6)
        self.assertNotIn("status", report["surfaces"][0])
        self.assertEqual(
            report["surfaces"][1]["artifact_profiles"],
            ["android-native"],
        )

    def test_android_handoff_requires_packaging_qualification_and_signing_owner(self) -> None:
        original_read = contract._read
        for operation in ("package", "qualify", "sign"):
            def missing_operation(root: Path, path: Path) -> str:
                text = original_read(root, path)
                if path.name == "release-android.yml":
                    return text.replace(f"scripts/android_package.py {operation}", "missing-owner")
                return text

            with self.subTest(operation=operation), mock.patch.object(
                contract, "_read", side_effect=missing_operation,
            ), self.assertRaisesRegex(contract.SurfaceContractError, "android-aar"):
                contract.validate_repository(ROOT, metadata=self.metadata)

    def test_missing_workflow_marker_fails_closed(self) -> None:
        original_read = contract._read

        def read_without_android_upload(root: Path, path: Path) -> str:
            text = original_read(root, path)
            if path == Path(".github/workflows/release-android.yml"):
                return text.replace("Upload AAR to GitHub Release", "Upload AAR")
            return text

        with mock.patch.object(contract, "_read", side_effect=read_without_android_upload):
            with self.assertRaisesRegex(contract.SurfaceContractError, "android-aar"):
                contract.validate_repository(ROOT, metadata=self.metadata)

    def test_python_owner_requires_shared_build_and_qualification_before_publication(self) -> None:
        python = next(surface for surface in contract.SURFACES if surface.surface_id == "python-wheel")
        self.assertEqual(python.workflow, Path(".github/workflows/release-python.yml"))
        self.assertEqual(python.preflight_job, "python-wheel")
        self.assertEqual(python.publication_mode, "manual-dispatch")
        marker = "uses: ./.github/workflows/build-python.yml"
        self.assertIn(marker, python.workflow_markers)
        original_read = contract._read

        def missing_python_builder(root: Path, path: Path) -> str:
            text = original_read(root, path)
            return text.replace(marker, "missing-python-builder") if path == python.workflow else text

        with mock.patch.object(contract, "_read", side_effect=missing_python_builder):
            with self.assertRaisesRegex(contract.SurfaceContractError, "python-wheel"):
                contract.validate_repository(ROOT, metadata=self.metadata)

    def test_sdk_owner_governs_all_six_exact_profile_packages(self) -> None:
        sdk = next(surface for surface in contract.SURFACES
                   if surface.surface_id == "headless-sdk-archives")
        self.assertEqual(sdk.workflow, Path(".github/workflows/release-sdk.yml"))
        self.assertEqual(sdk.preflight_job, "sdk-archives")
        self.assertEqual(sdk.publication_mode, "manual-dispatch-via-release-sdk-publish")
        self.assertEqual(dict(sdk.profile_packages), {
            f"sdk-{kind}-{variant}": package
            for kind, package in (("native", "merman-ffi"), ("wasi", "merman-wasi"))
            for variant in ("standard", "math", "full")
        })

    def test_missing_sdk_recipe_or_wrong_package_fails_closed(self) -> None:
        original_read = contract._read
        for mutation in ("missing", "wrong-package"):
            def changed_descriptor(root: Path, path: Path) -> str:
                text = original_read(root, path)
                if path == contract.ARTIFACT_DESCRIPTOR:
                    descriptor = json.loads(text)
                    if mutation == "missing":
                        descriptor["profiles"] = [p for p in descriptor["profiles"]
                                                  if p["id"] != "sdk-wasi-standard"]
                    else:
                        for profile in descriptor["profiles"]:
                            if profile["id"] == "sdk-wasi-standard":
                                profile["cargo"]["package"] = "merman-ffi"
                    return json.dumps(descriptor)
                return text
            with self.subTest(mutation=mutation), mock.patch.object(
                contract, "_read", side_effect=changed_descriptor,
            ), self.assertRaisesRegex(contract.SurfaceContractError, "sdk-wasi-standard"):
                contract.validate_repository(ROOT, metadata=self.metadata)

    def test_missing_sdk_build_or_qualification_owner_fails_closed(self) -> None:
        original_read = contract._read
        for marker in ("scripts/sdk_qualify.py", '--profile "$SDK_PROFILE"',
                       '--target "$SDK_TARGET"', "--work-dir", "--output-dir",
                       '--source-commit "$SOURCE_SHA"'):
            def missing_owner(root: Path, path: Path) -> str:
                text = original_read(root, path)
                return text.replace(marker, "missing-owner") if path.name == "release-sdk.yml" else text
            with self.subTest(marker=marker), mock.patch.object(
                contract, "_read", side_effect=missing_owner,
            ), self.assertRaisesRegex(contract.SurfaceContractError, "headless-sdk-archives"):
                contract.validate_repository(ROOT, metadata=self.metadata)


if __name__ == "__main__":
    unittest.main()
