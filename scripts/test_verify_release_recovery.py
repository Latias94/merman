#!/usr/bin/env python3
"""Focused policy tests for release recovery path admission."""

from __future__ import annotations

import importlib.util
from pathlib import Path
import unittest


MODULE_PATH = Path(__file__).with_name("verify-release-recovery.py")
SPEC = importlib.util.spec_from_file_location("verify_release_recovery", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
verifier = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(verifier)


class RecoveryPathPolicyTests(unittest.TestCase):
    def test_ascii_capability_helper_is_trusted_but_not_recovery_mutable(self) -> None:
        helper = "scripts/ascii_capability_contract.py"
        self.assertIn(helper, verifier.TRUSTED_FILES)
        self.assertNotIn(helper, verifier.RECOVERY_PATHS)

    def test_accepts_only_the_needed_subset_of_admitted_paths(self) -> None:
        verifier.verify_recovery_paths(
            (
                "scripts/test_verify_lsp_release_archive.py",
                "scripts/verify_lsp_release_archive.py",
            )
        )

    def test_rejects_a_noop_recovery(self) -> None:
        with self.assertRaisesRegex(
            verifier.RecoveryVerificationError,
            "does not change any admitted path",
        ):
            verifier.verify_recovery_paths(())

    def test_rejects_every_unexpected_path(self) -> None:
        with self.assertRaisesRegex(
            verifier.RecoveryVerificationError,
            r"unexpected paths: Cargo\.toml, README\.md",
        ):
            verifier.verify_recovery_paths(
                (
                    "scripts/verify_lsp_release_archive.py",
                    "README.md",
                    "Cargo.toml",
                )
            )


if __name__ == "__main__":
    unittest.main()
