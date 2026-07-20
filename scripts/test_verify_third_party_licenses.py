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
            "schema_version": 1,
            "generated_notice": "THIRD_PARTY_NOTICES.md",
            "license_root": "THIRD_PARTY_LICENSES",
            "repository_lock": {
                "path": "tools/upstreams/REPOS.lock.json",
                "schema_version": 1,
            },
            "externally_managed_files": [],
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

    def test_duplicate_json_key_is_rejected(self) -> None:
        self.fixture.contract_path.write_text(
            '{"schema_version":1,"schema_version":1}\n', encoding="utf-8"
        )
        with self.assertRaisesRegex(verify.ContractError, "duplicate JSON key"):
            verify.verify_repository(self.root)


class RepositoryContractTests(unittest.TestCase):
    def test_repository_contract_and_notice_are_current(self) -> None:
        verify.verify_repository(REPOSITORY_ROOT)

    def test_web_full_scope_tracks_structured_wasm_capabilities(self) -> None:
        contract = json.loads(
            (REPOSITORY_ROOT / "docs/release/THIRD_PARTY_COMPONENTS.json").read_text(
                encoding="utf-8"
            )
        )
        descriptor = json.loads(
            (REPOSITORY_ROOT / "platforms/web/web-surface-descriptor.json").read_text(
                encoding="utf-8"
            )
        )
        scopes = {scope["id"]: scope for scope in contract["artifact_scopes"]}
        presets = {preset["name"]: preset for preset in descriptor["presets"]}

        def inherits(scope_id: str, expected_parent: str) -> bool:
            return expected_parent in scopes[scope_id]["extends"] or any(
                inherits(parent, expected_parent) for parent in scopes[scope_id]["extends"]
            )

        capabilities = presets["browser-full"]["capabilities"]
        self.assertEqual(
            inherits("web-full", "elk-render"), capabilities["elk_layout"]
        )
        self.assertEqual(
            inherits("web-full", "ratex-render"), capabilities["ratex_math"]
        )


if __name__ == "__main__":
    unittest.main()
