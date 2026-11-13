"""Version-bound admission for the unpublished extension's audit exception."""

from __future__ import annotations

import copy
import unittest

from scripts.verify_vscode_audit_scope import verify_scope


class VscodeAuditScopeTests(unittest.TestCase):
    def setUp(self) -> None:
        self.lock = {
            "packages": {
                "node_modules/vscode-languageclient": {"version": "11.0.0"},
                "node_modules/minimatch": {"version": "10.2.6"},
            }
        }

    def test_current_versions_are_admitted(self) -> None:
        verify_scope(self.lock)

    def test_parent_or_matcher_upgrade_requires_exception_review(self) -> None:
        for package in self.lock["packages"]:
            with self.subTest(package=package):
                lock = copy.deepcopy(self.lock)
                lock["packages"][package]["version"] = "11.0.1"
                with self.assertRaisesRegex(ValueError, "review or remove"):
                    verify_scope(lock)

    def test_missing_or_nested_matcher_cannot_reuse_the_shared_version(self) -> None:
        for missing in [True, False]:
            with self.subTest(missing=missing):
                lock = copy.deepcopy(self.lock)
                if missing:
                    del lock["packages"]["node_modules/minimatch"]
                else:
                    lock["packages"]["node_modules/vscode-languageclient/node_modules/minimatch"] = {
                        "version": "10.2.5"
                    }
                with self.assertRaisesRegex(ValueError, "review or remove"):
                    verify_scope(lock)

    def test_unrelated_dependency_is_left_to_the_native_audit(self) -> None:
        self.lock["packages"]["node_modules/other"] = {"version": "1.0.0"}
        verify_scope(self.lock)


if __name__ == "__main__":
    unittest.main()
