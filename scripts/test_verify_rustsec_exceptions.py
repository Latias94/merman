#!/usr/bin/env python3
"""Tests for governed RustSec exceptions."""

from __future__ import annotations

from datetime import date
import json
import sys
import tempfile
import unittest
from pathlib import Path


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import verify_rustsec_exceptions as verify


class RustSecExceptionTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        seed_repository(self.root)

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def test_valid_exception_binds_deny_lock_paths_profiles_and_review_window(self) -> None:
        records = verify.load_exception_records(
            self.root,
            today=date(2026, 7, 26),
        )

        self.assertEqual(len(records), 1)
        self.assertEqual(records[0].package_name, "affected")
        verify.validate_profile_coverage(
            records,
            {
                "full": frozenset({("affected", "2.0.0"), ("root", "1.0.0")}),
                "slim": frozenset({("root", "1.0.0")}),
            },
        )

    def test_expired_review_is_rejected(self) -> None:
        with self.assertRaisesRegex(verify.RustSecExceptionError, "review expired"):
            verify.load_exception_records(
                self.root,
                today=date(2026, 10, 25),
            )

    def test_dependency_path_must_follow_cargo_lock_edges(self) -> None:
        ledger = read_ledger(self.root)
        ledger["exceptions"][0]["dependency_paths"][0][1] = "missing-edge@1.0.0"
        write_ledger(self.root, ledger)

        with self.assertRaisesRegex(verify.RustSecExceptionError, "missing Cargo.lock package"):
            verify.load_exception_records(self.root, today=date(2026, 7, 26))

    def test_deny_ignore_must_exactly_match_ledger(self) -> None:
        (self.root / verify.DENY_PATH).write_text(
            '[advisories]\nignore = [{ id = "RUSTSEC-2026-0001", reason = "different" }]\n',
            encoding="utf-8",
        )

        with self.assertRaisesRegex(verify.RustSecExceptionError, "must exactly match"):
            verify.load_exception_records(self.root, today=date(2026, 7, 26))

    def test_observed_profile_coverage_must_exactly_match_ledger(self) -> None:
        records = verify.load_exception_records(
            self.root,
            today=date(2026, 7, 26),
        )

        with self.assertRaisesRegex(verify.RustSecExceptionError, "coverage verification failed"):
            verify.validate_profile_coverage(
                records,
                {
                    "full": frozenset({("root", "1.0.0")}),
                    "slim": frozenset(
                        {("affected", "2.0.0"), ("root", "1.0.0")}
                    ),
                },
            )

    def test_observed_profile_coverage_is_version_exact(self) -> None:
        records = verify.load_exception_records(
            self.root,
            today=date(2026, 7, 26),
        )

        with self.assertRaisesRegex(
            verify.RustSecExceptionError,
            "coverage verification failed",
        ):
            verify.validate_profile_coverage(
                records,
                {
                    "full": frozenset({("affected", "3.0.0")}),
                    "slim": frozenset({("root", "1.0.0")}),
                },
            )


def seed_repository(root: Path) -> None:
    (root / verify.DENY_PATH).write_text(
        '[advisories]\nignore = [{ id = "RUSTSEC-2026-0001", reason = "No upgrade." }]\n',
        encoding="utf-8",
    )
    (root / verify.LOCK_PATH).write_text(
        """version = 4

[[package]]
name = "root"
version = "1.0.0"
dependencies = ["middle"]

[[package]]
name = "middle"
version = "1.0.0"
dependencies = ["affected 2.0.0"]

[[package]]
name = "affected"
version = "2.0.0"
""",
        encoding="utf-8",
    )
    descriptor = root / verify.ARTIFACT_PROFILES_PATH
    descriptor.parent.mkdir(parents=True)
    descriptor.write_text(
        json.dumps(
            {
                "schema_version": 1,
                "profiles": [{"id": "full"}, {"id": "slim"}],
            }
        ),
        encoding="utf-8",
    )
    write_ledger(
        root,
        {
            "schema_version": 1,
            "exceptions": [
                {
                    "id": "RUSTSEC-2026-0001",
                    "package": {"name": "affected", "version": "2.0.0"},
                    "reason": "No upgrade.",
                    "dependency_paths": [
                        ["root@1.0.0", "middle@1.0.0", "affected@2.0.0"]
                    ],
                    "affected_artifact_profiles": ["full"],
                    "upstream_issue": "https://github.com/example/project/issues/1",
                    "owner": "owner",
                    "reviewed_on": "2026-07-26",
                    "review_due": "2026-10-24",
                    "exit_condition": "Remove after a maintained release is available.",
                }
            ],
        },
    )


def read_ledger(root: Path) -> dict[str, object]:
    return json.loads((root / verify.LEDGER_PATH).read_text(encoding="utf-8"))


def write_ledger(root: Path, value: object) -> None:
    path = root / verify.LEDGER_PATH
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    unittest.main()
