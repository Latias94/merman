#!/usr/bin/env python3

from __future__ import annotations

import hashlib
import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path


REPOSITORY_ROOT = Path(__file__).resolve().parent.parent
SCRIPT_PATH = REPOSITORY_ROOT / "scripts/verify-third-party-licenses.py"
SPEC = importlib.util.spec_from_file_location("verify_third_party_licenses", SCRIPT_PATH)
assert SPEC is not None and SPEC.loader is not None
verify = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = verify
SPEC.loader.exec_module(verify)


class ContractFixture:
    def __init__(self, root: Path) -> None:
        self.root = root
        self.contract_path = root / "docs/release/THIRD_PARTY_COMPONENTS.json"
        self.license_path = root / "THIRD_PARTY_LICENSES/demo/LICENSE"
        self.notice_path = root / "THIRD_PARTY_NOTICES.md"
        self.source_path = root / "source.txt"
        self.repo_lock_path = root / "tools/upstreams/REPOS.lock.json"
        self.contract_path.parent.mkdir(parents=True)
        self.license_path.parent.mkdir(parents=True)
        self.repo_lock_path.parent.mkdir(parents=True)
        self.license_path.write_text("Demo license\n", encoding="utf-8")
        self.source_path.write_text("source\n", encoding="utf-8")
        self.repo_lock_path.write_text(
            json.dumps({"schemaVersion": 1, "repos": {}}, indent=2) + "\n", encoding="utf-8"
        )
        self.contract = {
            "schema_version": 3,
            "generated_notice": "THIRD_PARTY_NOTICES.md",
            "license_root": "THIRD_PARTY_LICENSES",
            "repository_lock": {
                "path": "tools/upstreams/REPOS.lock.json",
                "schema_version": 1,
            },
            "externally_managed_files": [],
            "scoped_external_materials": [],
            "artifact_scopes": [
                {
                    "id": "demo-artifact",
                    "description": "Demo artifact.",
                    "extends": [],
                    "components": ["demo"],
                }
            ],
            "components": [
                {
                    "id": "demo",
                    "name": "Demo",
                    "version": "1.0.0",
                    "source": {
                        "repository": "https://example.com/demo.git",
                        "ref": "v1.0.0",
                        "commit": "a" * 40,
                        "path": ".",
                    },
                    "relationships": ["translated"],
                    "local_paths": ["source.txt"],
                    "license_expression": "MIT",
                    "license_files": [
                        {
                            "path": "THIRD_PARTY_LICENSES/demo/LICENSE",
                            "sha256": hashlib.sha256(self.license_path.read_bytes()).hexdigest(),
                            "source_url": "https://example.com/demo/LICENSE",
                            "source_path": "LICENSE",
                            "role": "license",
                        }
                    ],
                    "locks": [{"type": "pinned-source", "evidence": "test fixture"}],
                    "notice": "Demo translated source.",
                }
            ],
        }
        self.write_contract()
        verify.verify_repository(self.root, write=True)

    def write_contract(self) -> None:
        self.contract_path.write_text(
            json.dumps(self.contract, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )


class ThirdPartyContractTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.fixture = ContractFixture(self.root)

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def test_minimal_contract_is_deterministic(self) -> None:
        first = self.fixture.notice_path.read_bytes()
        verify.verify_repository(self.root, write=True)
        second = self.fixture.notice_path.read_bytes()
        self.assertEqual(first, second)
        verify.verify_repository(self.root)

    def test_license_hash_drift_fails_closed(self) -> None:
        self.fixture.license_path.write_text("Tampered\n", encoding="utf-8")
        with self.assertRaisesRegex(verify.ContractError, "license hash mismatch"):
            verify.verify_repository(self.root)

    def test_notice_drift_fails_closed(self) -> None:
        self.fixture.notice_path.write_text("stale\n", encoding="utf-8")
        with self.assertRaisesRegex(verify.ContractError, "is stale"):
            verify.verify_repository(self.root)

    def test_unknown_scope_component_fails_closed(self) -> None:
        self.fixture.contract["artifact_scopes"][0]["components"].append("missing")
        self.fixture.write_contract()
        with self.assertRaisesRegex(verify.ContractError, "unknown components: missing"):
            verify.verify_repository(self.root)

    def test_repository_lock_drift_fails_closed(self) -> None:
        component = self.fixture.contract["components"][0]
        component["locks"] = [{"type": "repository", "repository_id": "demo"}]
        self.fixture.repo_lock_path.write_text(
            json.dumps(
                {
                    "schemaVersion": 1,
                    "repos": {
                        "demo": {
                            "path": "repo-ref/demo",
                            "url": "https://example.com/demo.git",
                            "ref": "v1.0.0",
                            "commit": "b" * 40,
                        }
                    },
                },
                indent=2,
            )
            + "\n",
            encoding="utf-8",
        )
        self.fixture.write_contract()
        with self.assertRaisesRegex(verify.ContractError, "does not match repository lock"):
            verify.verify_repository(self.root)

    def test_unregistered_license_file_fails_closed(self) -> None:
        extra = self.root / "THIRD_PARTY_LICENSES/extra.txt"
        extra.write_text("extra\n", encoding="utf-8")
        with self.assertRaisesRegex(verify.ContractError, "unregistered files"):
            verify.verify_repository(self.root)

    def test_declared_external_json_is_allowed(self) -> None:
        external = self.root / "THIRD_PARTY_LICENSES/generated.json"
        external.write_text("{}\n", encoding="utf-8")
        self.fixture.contract["externally_managed_files"] = [
            {
                "path": "THIRD_PARTY_LICENSES/generated.json",
                "owner": "test generator",
                "required": True,
                "format": "json",
            }
        ]
        self.fixture.write_contract()
        verify.verify_repository(self.root, write=True)
        verify.verify_repository(self.root)

    def test_missing_required_external_json_fails_closed(self) -> None:
        self.fixture.contract["externally_managed_files"] = [
            {
                "path": "THIRD_PARTY_LICENSES/generated.json",
                "owner": "test generator",
                "required": True,
                "format": "json",
            }
        ]
        self.fixture.write_contract()
        with self.assertRaisesRegex(verify.ContractError, "required external material is missing"):
            verify.verify_repository(self.root)

    def test_scoped_external_material_must_reference_a_known_scope(self) -> None:
        self.fixture.contract["scoped_external_materials"] = [
            {
                "artifact_scope": "missing-artifact",
                "path": "platforms/web/legal/rust-cargo-dependencies/missing.json",
                "projection_path": "THIRD_PARTY_LICENSES/rust-cargo-dependencies.json",
                "owner": "test generator",
                "required": True,
                "format": "json",
            }
        ]
        self.fixture.write_contract()
        with self.assertRaisesRegex(verify.ContractError, "unknown artifact scope"):
            verify.verify_repository(self.root)

    def test_duplicate_json_key_is_rejected(self) -> None:
        self.fixture.contract_path.write_text(
            '{"schema_version":1,"schema_version":1}\n', encoding="utf-8"
        )
        with self.assertRaisesRegex(verify.ContractError, "duplicate JSON key"):
            verify.verify_repository(self.root)

    def test_unknown_contract_field_is_rejected(self) -> None:
        self.fixture.contract["unexpected"] = True
        self.fixture.write_contract()

        with self.assertRaisesRegex(verify.ContractError, "unknown fields: unexpected"):
            verify.verify_repository(self.root)

    def test_non_normalized_repository_path_is_rejected(self) -> None:
        self.fixture.contract["generated_notice"] = "./THIRD_PARTY_NOTICES.md"
        self.fixture.write_contract()
        with self.assertRaisesRegex(
            verify.ContractError, "normalized repository-relative path"
        ):
            verify.verify_repository(self.root)


class WebRustReportValidationTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        (self.root / "Cargo.lock").write_text("lock", encoding="utf-8")
        (self.root / "about.toml").write_text("accepted = []", encoding="utf-8")
        profile_path = self.root / verify.ARTIFACT_PROFILES_PATH
        profile_path.parent.mkdir(parents=True)
        profile_path.write_text("{}\n", encoding="utf-8")
        self.profile = {
            "id": "web-analysis",
            "semantic_target": "web",
            "cargo": {
                "package": "merman-wasm",
                "manifest": "crates/merman-wasm/Cargo.toml",
                "profile": "wasm-size",
                "default_features": False,
                "features": ["analysis"],
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
        }
        self.report = web_rust_report_fixture(self.root, self.profile)

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def test_valid_report_is_accepted(self) -> None:
        verify.validate_web_rust_report(
            self.root, self.report, self.profile, "web-analysis"
        )

    def test_profile_id_features_and_target_drift_fail_closed(self) -> None:
        for mutate in (
            lambda profile: profile.update({"id": "web-editor"}),
            lambda profile: profile["cargo"].update({"features": ["editor"]}),
            lambda profile: profile["cargo"]["build_target"].update(
                {"triples": ["x86_64-unknown-linux-gnu"]}
            ),
        ):
            candidate = json.loads(json.dumps(self.report))
            mutate(candidate["artifact_profile"])
            with self.assertRaisesRegex(verify.ContractError, "artifact profile recipe"):
                verify.validate_web_rust_report(
                    self.root, candidate, self.profile, "web-analysis"
                )

    def test_missing_dependency_or_generator_input_drift_fails_closed(self) -> None:
        incomplete = json.loads(json.dumps(self.report))
        incomplete["licenses"][0]["packages"].pop()
        with self.assertRaisesRegex(verify.ContractError, "closure is incomplete or stale"):
            verify.validate_web_rust_report(
                self.root, incomplete, self.profile, "web-analysis"
            )

        stale = json.loads(json.dumps(self.report))
        stale["generator"]["cargo_lock_sha256"] = "0" * 64
        with self.assertRaisesRegex(verify.ContractError, "generator inputs have drifted"):
            verify.validate_web_rust_report(
                self.root, stale, self.profile, "web-analysis"
            )


def web_rust_report_fixture(root: Path, profile: dict[str, object]) -> dict[str, object]:
    packages = [
        {
            "name": name,
            "version": "1.0.0",
            "source": "registry+https://example.invalid/index",
            "license_expression": "MIT",
            "authors": ["Example"],
            "repository": f"https://example.invalid/{name}",
        }
        for name in ("dependency-a", "dependency-b")
    ]
    encoded = json.dumps(
        packages,
        ensure_ascii=True,
        separators=(",", ":"),
        sort_keys=True,
    ).encode()
    text = "MIT terms\n"
    return {
        "schema_version": 2,
        "artifact_profile": profile,
        "generator": {
            "name": "cargo-about",
            "version": "0.9.1",
            "command_profile": "artifact-profile-runtime",
            "offline": True,
            "cargo_lock_sha256": verify.sha256(root / "Cargo.lock"),
            "configuration_sha256": verify.sha256(root / "about.toml"),
            "artifact_profile_sha256": verify.sha256_json(profile),
        },
        "dependency_closure": {
            "package_count": len(packages),
            "packages_sha256": hashlib.sha256(encoded).hexdigest(),
        },
        "licenses": [
            {
                "id": "MIT",
                "name": "MIT License",
                "text_sha256": hashlib.sha256(text.encode()).hexdigest(),
                "text": text,
                "packages": packages,
            }
        ],
    }


class RepositoryContractTests(unittest.TestCase):
    def test_repository_contract_and_notice_are_current(self) -> None:
        verify.verify_repository(REPOSITORY_ROOT)

    def test_web_full_scope_tracks_structured_wasm_capabilities(self) -> None:
        contract = json.loads(
            (REPOSITORY_ROOT / "docs/release/THIRD_PARTY_COMPONENTS.json").read_text(
                encoding="utf-8"
            )
        )
        artifact_profiles = json.loads(
            (REPOSITORY_ROOT / "capabilities/artifact-profiles-v1.json").read_text(
                encoding="utf-8"
            )
        )
        scopes = {scope["id"]: scope for scope in contract["artifact_scopes"]}
        profiles = {
            profile["id"]: profile for profile in artifact_profiles["profiles"]
        }

        def inherits(scope_id: str, expected_parent: str) -> bool:
            return expected_parent in scopes[scope_id]["extends"] or any(
                inherits(parent, expected_parent) for parent in scopes[scope_id]["extends"]
            )

        capabilities = set(profiles["web-full"]["expected"]["capabilities"])
        self.assertTrue(inherits("web-full", "elk-render"))
        self.assertIn("layout-elk", capabilities)
        self.assertIn("math", capabilities)
        self.assertTrue(inherits("web-full", "ratex-render"))
        self.assertIn("ascii", capabilities)
        self.assertTrue(inherits("web-full", "ascii-render"))

    def test_every_web_scope_owns_one_exact_rust_dependency_report(self) -> None:
        contract = json.loads(
            (REPOSITORY_ROOT / "docs/release/THIRD_PARTY_COMPONENTS.json").read_text(
                encoding="utf-8"
            )
        )
        scoped = {
            material["artifact_scope"]: material
            for material in contract["scoped_external_materials"]
        }
        expected = {
            "web-analysis",
            "web-ascii",
            "web-editor",
            "web-full",
            "web-render",
        }
        self.assertEqual(set(scoped), expected)
        self.assertEqual(len({item["path"] for item in scoped.values()}), len(expected))
        for profile_id, material in scoped.items():
            self.assertEqual(
                material["projection_path"],
                "THIRD_PARTY_LICENSES/rust-cargo-dependencies.json",
            )
            report_path = REPOSITORY_ROOT / material["path"]
            generated = json.loads(report_path.read_text(encoding="utf-8"))
            self.assertEqual(generated["artifact_profile"]["id"], profile_id)


if __name__ == "__main__":
    unittest.main()
