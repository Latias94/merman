#!/usr/bin/env python3
"""Unit tests for the normalized cargo-about report."""

from __future__ import annotations

import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock


MODULE_PATH = Path(__file__).with_name("generate-rust-license-report.py")
SPEC = importlib.util.spec_from_file_location("generate_rust_license_report", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
report = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = report
SPEC.loader.exec_module(report)

WEB_PROFILE_FEATURES = {
    "web-analysis": ["analysis"],
    "web-ascii": ["ascii"],
    "web-editor": ["editor"],
    "web-full": [
        "analysis",
        "ascii",
        "editor",
        "layout-cytoscape",
        "layout-elk",
        "math",
        "svg",
    ],
    "web-render": ["layout-cytoscape", "layout-elk", "math", "svg"],
}


class RustLicenseReportTests(unittest.TestCase):
    def test_strict_json_rejects_duplicate_keys_with_report_error(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "duplicate.json"
            path.write_text('{"schema_version": 1, "schema_version": 1}\n', encoding="utf-8")

            with self.assertRaisesRegex(
                report.RustLicenseReportError,
                "duplicate JSON key",
            ):
                report.load_json_strict(path)

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

    def test_web_recipes_are_loaded_from_the_artifact_profile_authority(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            write_web_profile_fixture(root)

            recipes = report.load_web_profile_recipes(root)

            self.assertEqual(set(recipes), set(WEB_PROFILE_FEATURES))
            self.assertEqual(recipes["web-analysis"]["cargo"]["features"], ["analysis"])
            command = report.cargo_about_command(
                root / "report.json",
                recipes["web-analysis"],
            )
            self.assertIn("--no-default-features", command)
            self.assertNotIn("--workspace", command)
            self.assertNotIn("--all-features", command)
            self.assertEqual(command[command.index("--target") + 1], "wasm32-unknown-unknown")
            self.assertEqual(command[command.index("--features") + 1], "analysis")
            self.assertEqual(
                command[command.index("--manifest-path") + 1],
                "crates/merman-wasm/Cargo.toml",
            )

    def test_web_recipe_rejects_default_features_or_non_wasm_targets(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            profiles = write_web_profile_fixture(root)
            profiles["profiles"][0]["cargo"]["default_features"] = True
            write_json(root / report.ARTIFACT_PROFILES_PATH, profiles)
            with self.assertRaisesRegex(report.RustLicenseReportError, "default_features=false"):
                report.load_web_profile_recipes(root)

            profiles["profiles"][0]["cargo"]["default_features"] = False
            profiles["profiles"][0]["cargo"]["build_target"]["triples"] = [
                "x86_64-unknown-linux-gnu"
            ]
            write_json(root / report.ARTIFACT_PROFILES_PATH, profiles)
            with self.assertRaisesRegex(report.RustLicenseReportError, "wasm32-unknown-unknown"):
                report.load_web_profile_recipes(root)

    def test_web_recipe_rejects_unknown_cargo_fields(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            profiles = write_web_profile_fixture(root)
            profiles["profiles"][0]["cargo"]["unexpected"] = True
            write_json(root / report.ARTIFACT_PROFILES_PATH, profiles)

            with self.assertRaisesRegex(
                report.RustLicenseReportError,
                "unknown fields: unexpected",
            ):
                report.load_web_profile_recipes(root)

    def test_web_recipe_rejects_non_normalized_manifest_paths(self) -> None:
        for manifest in (
            "crates//merman-wasm/Cargo.toml",
            "C:/crates/merman-wasm/Cargo.toml",
        ):
            with self.subTest(manifest=manifest), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                profiles = write_web_profile_fixture(root)
                profiles["profiles"][0]["cargo"]["manifest"] = manifest
                write_json(root / report.ARTIFACT_PROFILES_PATH, profiles)

                with self.assertRaisesRegex(
                    report.RustLicenseReportError,
                    "normalized repository-relative path",
                ):
                    report.load_web_profile_recipes(root)

    def test_web_normalization_binds_recipe_and_dependency_closure(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            write_web_profile_fixture(root)
            recipe = report.load_web_profile_recipes(root)["web-analysis"]
            raw = cargo_about_fixture()

            normalized = report.normalize_report(raw, root, artifact_profile=recipe)

            self.assertEqual(normalized["schema_version"], 2)
            self.assertEqual(normalized["artifact_profile"], recipe)
            self.assertEqual(normalized["dependency_closure"]["package_count"], 1)
            self.assertRegex(
                normalized["dependency_closure"]["packages_sha256"], r"^[0-9a-f]{64}$"
            )
            self.assertEqual(
                normalized["generator"]["artifact_profile_sha256"],
                report.sha256_json(recipe),
            )

    def test_multi_target_profile_requires_an_explicit_report_target(self) -> None:
        recipe = artifact_profile_fixture(
            "native",
            build_target={
                "kind": "target-set",
                "triples": ["target-a", "target-b"],
            },
        )

        with self.assertRaisesRegex(report.RustLicenseReportError, "requires an explicit"):
            report.cargo_about_command(Path("report.json"), recipe)

        command = report.cargo_about_command(
            Path("report.json"),
            recipe,
            target="target-b",
        )
        self.assertEqual(command[command.index("--target") + 1], "target-b")

    def test_native_target_observations_cover_descriptor_targets_and_explicit_subsets(self) -> None:
        target_recipe = artifact_profile_fixture(
            "target-profile",
            build_target={
                "kind": "target-set",
                "triples": ["target-a", "target-b"],
            },
        )
        spec = report.NativeReportSpec(
            bundle_id="native-sdk",
            output=Path("platform/report.json"),
            profile_ids=("target-profile",),
            target_selections=(("target-profile", ("target-b",)),),
        )

        observations = report.native_target_observations(
            spec,
            {"target-profile": target_recipe},
        )

        self.assertEqual(
            observations,
            ({"artifact_profile_id": "target-profile", "target": "target-b"},),
        )

    def test_native_target_observations_reject_host_or_out_of_profile_targets(self) -> None:
        host_recipe = artifact_profile_fixture(
            "host-profile",
            build_target={"kind": "host"},
        )
        host_spec = report.NativeReportSpec(
            bundle_id="native-sdk",
            output=Path("platform/report.json"),
            profile_ids=("host-profile",),
        )
        with self.assertRaisesRegex(
            report.RustLicenseReportError,
            "requires descriptor-owned target-set",
        ):
            report.native_target_observations(
                host_spec,
                {"host-profile": host_recipe},
            )

        target_recipe = artifact_profile_fixture(
            "target-profile",
            build_target={"kind": "target-set", "triples": ["target-a"]},
        )
        invalid_spec = report.NativeReportSpec(
            bundle_id="native-sdk",
            output=Path("platform/report.json"),
            profile_ids=("target-profile",),
            target_selections=(("target-profile", ("target-b",)),),
        )
        with self.assertRaisesRegex(
            report.RustLicenseReportError,
            "non-empty subset",
        ):
            report.native_target_observations(
                invalid_spec,
                {"target-profile": target_recipe},
            )

    def test_python_target_reports_are_derived_from_the_profile_target_set(self) -> None:
        profile = artifact_profile_fixture(
            report.PYTHON_ARTIFACT_PROFILE_ID,
            build_target={
                "kind": "target-set",
                "triples": ["target-a", "target-b"],
            },
        )

        specs = report.python_target_report_specs(
            {report.PYTHON_ARTIFACT_PROFILE_ID: profile}
        )

        self.assertEqual(
            [spec.output for spec in specs],
            [
                report.PYTHON_TARGET_REPORT_ROOT / "target-a.json",
                report.PYTHON_TARGET_REPORT_ROOT / "target-b.json",
            ],
        )
        self.assertEqual(
            [spec.target_selections for spec in specs],
            [
                ((report.PYTHON_ARTIFACT_PROFILE_ID, ("target-a",)),),
                ((report.PYTHON_ARTIFACT_PROFILE_ID, ("target-b",)),),
            ],
        )

    def test_native_outputs_generate_each_shared_observation_once(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "Cargo.lock").write_text("lock", encoding="utf-8")
            (root / "about.toml").write_text("accepted = []", encoding="utf-8")
            profile = artifact_profile_fixture(
                report.PYTHON_ARTIFACT_PROFILE_ID,
                build_target={
                    "kind": "target-set",
                    "triples": ["target-a", "target-b"],
                },
            )
            recipes = {report.PYTHON_ARTIFACT_PROFILE_ID: profile}
            union = report.NativeReportSpec(
                bundle_id="python-native-sdk",
                output=Path("platforms/python/union.json"),
                profile_ids=(report.PYTHON_ARTIFACT_PROFILE_ID,),
            )
            specs = (union, *report.python_target_report_specs(recipes))

            with mock.patch.object(
                report,
                "generate_normalized_report_for_profile",
                return_value={"licenses": [normalized_license("shared")]},
            ) as generate:
                outputs = report.generate_native_outputs(root, specs, recipes)

            self.assertEqual(len(outputs), 3)
            self.assertEqual(generate.call_count, 2)
            self.assertEqual(
                [call.kwargs["target"] for call in generate.call_args_list],
                ["target-a", "target-b"],
            )

    def test_native_report_merges_target_licenses_and_keeps_target_closures(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "Cargo.lock").write_text("lock", encoding="utf-8")
            (root / "about.toml").write_text("accepted = []", encoding="utf-8")
            recipe = artifact_profile_fixture(
                "native",
                build_target={
                    "kind": "target-set",
                    "triples": ["target-a", "target-b"],
                },
            )
            spec = report.NativeReportSpec(
                bundle_id="native-sdk",
                output=Path("platform/report.json"),
                profile_ids=("native",),
            )
            shared = normalized_license("shared")
            target_only = normalized_license("target-only")

            native_report = report.build_native_report(
                root,
                spec,
                {"native": recipe},
                {
                    ("native", "target-a"): [shared],
                    ("native", "target-b"): [shared, target_only],
                },
            )

            self.assertEqual(native_report["schema_version"], 3)
            self.assertEqual(
                native_report["generator"]["command_profile"],
                "artifact-profile-target-union",
            )
            self.assertEqual(
                [entry["package_count"] for entry in native_report["target_dependency_closures"]],
                [1, 2],
            )
            self.assertEqual(native_report["dependency_closure"]["package_count"], 2)
            self.assertEqual(
                native_report["generator"]["artifact_bundle_sha256"],
                report.sha256_json(native_report["artifact_bundle"]),
            )


def cargo_about_fixture() -> dict[str, object]:
    return {
        "licenses": [
            {
                "id": "MIT",
                "name": "MIT License",
                "text": "MIT terms",
                "used_by": [
                    {
                        "crate": {
                            "name": "external",
                            "version": "1.2.3",
                            "source": "registry+https://example.invalid/index",
                            "license": "MIT",
                            "authors": ["Example"],
                            "repository": "https://example.invalid/external",
                        }
                    }
                ],
            }
        ]
    }


def artifact_profile_fixture(
    profile_id: str,
    *,
    build_target: dict[str, object],
) -> dict[str, object]:
    return {
        "id": profile_id,
        "semantic_target": "native",
        "cargo": {
            "package": "example",
            "manifest": "crates/example/Cargo.toml",
            "profile": "release",
            "default_features": False,
            "features": ["feature"],
            "target": {
                "name": "example",
                "kinds": ["lib"],
                "crate_types": ["lib"],
                "required_features": [],
            },
            "build_target": build_target,
        },
    }


def normalized_license(package_name: str) -> dict[str, object]:
    text = "MIT terms"
    return {
        "id": "MIT",
        "name": "MIT License",
        "text_sha256": report.hashlib.sha256(text.encode()).hexdigest(),
        "text": text,
        "packages": [
            {
                "name": package_name,
                "version": "1.0.0",
                "source": "registry+https://example.invalid/index",
                "license_expression": "MIT",
                "authors": ["Example"],
                "repository": f"https://example.invalid/{package_name}",
            }
        ],
    }


def write_web_profile_fixture(root: Path) -> dict[str, object]:
    (root / "Cargo.lock").write_text("lock", encoding="utf-8")
    (root / "about.toml").write_text("accepted = []", encoding="utf-8")
    manifest = root / "crates/merman-wasm/Cargo.toml"
    manifest.parent.mkdir(parents=True)
    manifest.write_text(
        '[package]\nname = "merman-wasm"\nversion = "0.1.0"\n'
        "[features]\nanalysis = []\nascii = []\neditor = []\n"
        'layout-cytoscape = []\nlayout-elk = []\nmath = []\nsvg = []\n',
        encoding="utf-8",
    )
    profiles: dict[str, object] = {"schema_version": 1, "profiles": []}
    entries = profiles["profiles"]
    assert isinstance(entries, list)
    for profile_id, features in WEB_PROFILE_FEATURES.items():
        entries.append(
            {
                "id": profile_id,
                "semantic_target": "web",
                "cargo": {
                    "package": "merman-wasm",
                    "manifest": "crates/merman-wasm/Cargo.toml",
                    "profile": "wasm-size",
                    "default_features": False,
                    "features": features,
                    "target": {
                        "name": "merman_wasm",
                        "kinds": ["cdylib", "rlib"],
                        "crate_types": ["cdylib", "rlib"],
                        "required_features": [],
                    },
                    "build_target": {
                        "kind": "target-set",
                        "triples": ["wasm32-unknown-unknown"],
                    },
                },
                "expected": {"capabilities": features, "runtime_ids": features, "outputs": []},
            }
        )
    write_json(root / report.ARTIFACT_PROFILES_PATH, profiles)
    return profiles


def write_json(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    unittest.main()
