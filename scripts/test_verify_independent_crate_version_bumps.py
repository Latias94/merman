#!/usr/bin/env python3
"""Tests for independent crate version bump verification."""

from __future__ import annotations

import importlib.util
from pathlib import Path
import subprocess
import tempfile
import textwrap
import unittest


MODULE_PATH = Path(__file__).with_name("verify-independent-crate-version-bumps.py")
SPEC = importlib.util.spec_from_file_location("verify_independent_crate_version_bumps", MODULE_PATH)
assert SPEC is not None
verifier = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(verifier)


class IndependentCrateVersionBumpTests(unittest.TestCase):
    def test_unchanged_independent_crate_may_keep_version(self) -> None:
        with repository() as root:
            self.assertEqual(verifier.verify(root, "v0.1.0", "HEAD"), ())

    def test_changed_independent_crate_requires_version_bump(self) -> None:
        with repository() as root:
            (root / "crates/independent/src/lib.rs").write_text("pub fn value() -> u8 { 2 }\n")
            commit_all(root, "change source")

            failures = verifier.verify(root, "v0.1.0", "HEAD")

            self.assertEqual(len(failures), 1)
            self.assertIn("still uses 1.0.0", failures[0])
            self.assertIn("crates/independent/src/lib.rs", failures[0])

    def test_changed_independent_crate_accepts_new_version(self) -> None:
        with repository() as root:
            manifest = root / "crates/independent/Cargo.toml"
            manifest.write_text(manifest.read_text().replace('version = "1.0.0"', 'version = "1.0.1"'))
            (root / "crates/independent/src/lib.rs").write_text("pub fn value() -> u8 { 2 }\n")
            commit_all(root, "bump independent crate")

            self.assertEqual(verifier.verify(root, "v0.1.0", "HEAD"), ())


def repository() -> tempfile.TemporaryDirectory[Path]:
    return RepositoryFixture()


class RepositoryFixture(tempfile.TemporaryDirectory[Path]):
    def __enter__(self) -> Path:
        root = Path(super().__enter__())
        (root / "crates/independent/src").mkdir(parents=True)
        (root / "Cargo.toml").write_text(
            textwrap.dedent(
                """
                [workspace]
                members = ["crates/independent"]
                resolver = "2"

                [workspace.metadata.merman-release]
                independent-packages = ["independent"]
                """
            ).lstrip()
        )
        (root / "crates/independent/Cargo.toml").write_text(
            textwrap.dedent(
                """
                [package]
                name = "independent"
                version = "1.0.0"
                edition = "2021"
                """
            ).lstrip()
        )
        (root / "crates/independent/src/lib.rs").write_text("pub fn value() -> u8 { 1 }\n")
        run(root, "git", "init", "-q")
        run(root, "git", "config", "user.name", "Merman Test")
        run(root, "git", "config", "user.email", "test@example.invalid")
        commit_all(root, "initial")
        run(root, "git", "tag", "v0.1.0")
        return root


def commit_all(root: Path, message: str) -> None:
    run(root, "git", "add", ".")
    run(root, "git", "commit", "-qm", message)


def run(root: Path, *command: str) -> None:
    subprocess.run(command, cwd=root, check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)


if __name__ == "__main__":
    unittest.main()
