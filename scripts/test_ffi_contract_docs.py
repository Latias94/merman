#!/usr/bin/env python3
"""Tests for the current-facing FFI documentation contract."""

from __future__ import annotations

from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))

import ffi_contract_docs


class FfiContractDocsTests(unittest.TestCase):
    def test_repository_documents_match_the_current_object_model(self) -> None:
        self.assertEqual(ffi_contract_docs.verify_repository(), ())

    def test_stale_api_is_rejected_outside_migration_sections(self) -> None:
        path = Path("docs/bindings/UNIFFI.md")
        failures = ffi_contract_docs.document_failures(
            path,
            "# Binding\n\nUse `MermanReusableEngine` for current calls.\n",
        )
        self.assertTrue(any("MermanReusableEngine" in item for item in failures))

    def test_stale_api_is_allowed_as_migration_evidence(self) -> None:
        text = (
            "# Binding\n\n"
            "Use `Merman` and `MermanEngine`.\n\n"
            "## Migrating From The Previous Prerelease API\n\n"
            "Delete `MermanReusableEngine` and `reusable_engine(...)`.\n"
        )
        current = ffi_contract_docs.current_contract_text(text)
        self.assertNotIn("MermanReusableEngine", current)
        self.assertNotIn("reusable_engine", current)

    def test_flutter_stateless_facade_rejects_close_examples(self) -> None:
        failures = ffi_contract_docs.document_failures(
            Path("docs/bindings/FLUTTER_DART_FFI.md"),
            "# Flutter\n\nfinal merman = Merman.open();\nmerman.close();\n",
        )
        self.assertTrue(any("stateless Merman" in item for item in failures))

    def test_android_docs_reject_native_registry_lifecycle_claims(self) -> None:
        failures = ffi_contract_docs.document_failures(
            Path("platforms/android/README.md"),
            "# Android\n\nregistry.close()\n",
        )
        self.assertTrue(any("stale Android icon-registry" in item for item in failures))

    def test_upgrade_guide_rejects_android_as_c_abi_transport(self) -> None:
        failures = ffi_contract_docs.document_failures(
            Path("docs/release/ALPHA3_TO_ALPHA4_UPGRADE_GUIDE.md"),
            "# Upgrade\n\nC, Flutter, and Android hosts use ABI 3.\n",
        )
        self.assertTrue(any("Android must not be described" in item for item in failures))

    def test_python_docs_reject_old_document_helper_order(self) -> None:
        failures = ffi_contract_docs.document_failures(
            Path("docs/bindings/PYTHON_UNIFFI.md"),
            """# Python

api.analyze_document_json(
    source,
    None,
    "file:///tmp/example.md",
)
""",
        )
        self.assertTrue(any("source, uri, options_json" in item for item in failures))

    def test_features_rejects_stale_flutter_install_version(self) -> None:
        failures = ffi_contract_docs.document_failures(
            Path("docs/FEATURES.md"),
            "# Features\n\nflutter pub add 'merman:^0.8.0-alpha.3'\n",
        )
        self.assertTrue(any("still pins alpha.3" in item for item in failures))


if __name__ == "__main__":
    unittest.main()
