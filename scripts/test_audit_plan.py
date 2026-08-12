#!/usr/bin/env python3
"""Tests for tracked Cargo/npm audit matrix discovery."""

from __future__ import annotations

import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


sys.path.insert(0, str(Path(__file__).resolve().parent))

import audit_plan


class AuditPlanTests(unittest.TestCase):
    def test_nested_tracked_locks_are_discovered_without_registration(self) -> None:
        plan = audit_plan.build_audit_plan(
            {
                "Cargo.lock",
                "Cargo.toml",
                "crates/nested/Cargo.lock",
                "crates/nested/Cargo.toml",
                "platforms/node/package-lock.json",
                "platforms/node/package.json",
                "tools/new-owner/deep/package-lock.json",
                "tools/new-owner/deep/package.json",
            }
        )

        self.assertEqual(
            plan["cargo"],
            {
                "include": [
                    {"lockfile": "Cargo.lock"},
                    {"lockfile": "crates/nested/Cargo.lock"},
                ]
            },
        )
        self.assertEqual(
            plan["npm"],
            {
                "include": [
                    {
                        "directory": "platforms/node",
                        "lockfile": "platforms/node/package-lock.json",
                    },
                    {
                        "directory": "tools/new-owner/deep",
                        "lockfile": "tools/new-owner/deep/package-lock.json",
                    },
                ]
            },
        )

    def test_exact_fixture_exclusion_requires_a_documented_reason(self) -> None:
        tracked = {
            "Cargo.lock",
            "Cargo.toml",
            "fixtures/npm/package-lock.json",
        }

        plan = audit_plan.build_audit_plan(
            tracked,
            exclusions={
                "fixtures/npm/package-lock.json": "synthetic parser fixture without a package owner",
            },
        )

        self.assertEqual(plan["npm"], {"include": []})
        with self.assertRaisesRegex(audit_plan.AuditPlanError, "non-empty reason"):
            audit_plan.build_audit_plan(
                tracked,
                exclusions={"fixtures/npm/package-lock.json": ""},
            )

    def test_non_lock_fixtures_are_ignored_without_an_exclusion(self) -> None:
        plan = audit_plan.build_audit_plan(
            {
                "Cargo.lock",
                "Cargo.toml",
                "fixtures/package-lock.json.expected",
                "fixtures/Cargo.lock.invalid",
            }
        )

        self.assertEqual(plan["cargo"], {"include": [{"lockfile": "Cargo.lock"}]})
        self.assertEqual(plan["npm"], {"include": []})

    def test_included_locks_require_their_owner_manifest(self) -> None:
        with self.assertRaisesRegex(
            audit_plan.AuditPlanError,
            "nested/package-lock.json.*nested/package.json",
        ):
            audit_plan.build_audit_plan({"nested/package-lock.json"})

        with self.assertRaisesRegex(
            audit_plan.AuditPlanError,
            "nested/Cargo.lock.*nested/Cargo.toml",
        ):
            audit_plan.build_audit_plan({"nested/Cargo.lock"})

    def test_repository_discovery_uses_only_tracked_normalized_paths(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            repository = Path(temporary)
            subprocess.run(
                ["git", "init", "--quiet"],
                cwd=repository,
                check=True,
                capture_output=True,
                text=True,
            )
            self.write(repository / "Cargo.toml")
            self.write(repository / "Cargo.lock")
            self.write(repository / "nested" / "package.json", "{}\n")
            self.write(repository / "nested" / "package-lock.json", "{}\n")
            self.write(repository / "untracked" / "Cargo.toml")
            self.write(repository / "untracked" / "Cargo.lock")
            subprocess.run(
                ["git", "add", "Cargo.toml", "Cargo.lock", "nested"],
                cwd=repository,
                check=True,
                capture_output=True,
                text=True,
            )

            plan = audit_plan.discover_audit_plan(repository)

        self.assertEqual(
            plan,
            {
                "cargo": {"include": [{"lockfile": "Cargo.lock"}]},
                "npm": {
                    "include": [
                        {
                            "directory": "nested",
                            "lockfile": "nested/package-lock.json",
                        }
                    ]
                },
            },
        )

    def test_github_output_contains_the_exact_consumed_matrices(self) -> None:
        plan = {
            "cargo": {"include": [{"lockfile": "Cargo.lock"}]},
            "npm": {
                "include": [
                    {"directory": "platforms/node", "lockfile": "platforms/node/package-lock.json"}
                ]
            },
        }
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / "github-output"
            audit_plan.write_github_output(output, plan)
            lines = output.read_text(encoding="utf-8").splitlines()

        self.assertEqual(len(lines), 2)
        values = dict(line.split("=", 1) for line in lines)
        self.assertEqual(json.loads(values["cargo"]), plan["cargo"])
        self.assertEqual(json.loads(values["npm"]), plan["npm"])

    @staticmethod
    def write(path: Path, contents: str = "") -> None:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(contents, encoding="utf-8")


if __name__ == "__main__":
    unittest.main()
