"""Bind container input bytes to the attested CLI release manifest."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import tempfile
import unittest

from scripts.cli_container_release import verify_container_release


class ContainerReleaseTests(unittest.TestCase):
    def setUp(self) -> None:
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.assets = Path(self.directory.name)
        self.target = "x86_64-unknown-linux-gnu"
        self.name = f"merman-cli-{self.target}.tar.xz"
        self.source = "a" * 40
        self.payload = b"published archive bytes"
        (self.assets / self.name).write_bytes(self.payload)
        self.manifest = {
            "schema_version": 1,
            "version": "0.9.0",
            "source_sha": self.source,
            "assets": [{
                "kind": "archive", "package": "merman-cli", "profile": "cli-release",
                "name": self.name, "target": self.target, "size": len(self.payload),
                "sha256": hashlib.sha256(self.payload).hexdigest(),
            }],
        }

    def verify(self) -> None:
        (self.assets / "release-verification.json").write_text(json.dumps(self.manifest))
        verify_container_release(self.assets, self.target, "0.9.0", self.source)

    def test_accepts_matching_published_bytes(self) -> None:
        self.verify()

    def test_rejects_changed_archive_bytes(self) -> None:
        (self.assets / self.name).write_bytes(b"replacement archive bytes")
        with self.assertRaises(ValueError):
            self.verify()

    def test_rejects_another_source_or_profile(self) -> None:
        self.manifest["source_sha"] = "b" * 40
        with self.assertRaises(ValueError):
            self.verify()
        self.manifest["source_sha"] = self.source
        self.manifest["assets"][0]["profile"] = "cli-analysis"
        with self.assertRaises(ValueError):
            self.verify()

    def test_rejects_duplicate_archive_records(self) -> None:
        self.manifest["assets"].append(dict(self.manifest["assets"][0]))
        with self.assertRaises(ValueError):
            self.verify()


if __name__ == "__main__":
    unittest.main()
