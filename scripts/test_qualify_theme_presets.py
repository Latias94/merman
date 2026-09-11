"""Clean-source and record freshness contracts for scoped preset qualification."""

import copy
from pathlib import Path
import subprocess
import tempfile
import unittest

from scripts.qualify_theme_presets import clean_revision, verify_record


class PresetQualificationTests(unittest.TestCase):
    def test_clean_revision_rejects_untracked_unstaged_and_staged_work(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)

            def git(*args):
                return subprocess.check_output(["git", *args], cwd=root, text=True).strip()

            git("init", "--quiet")
            git("config", "user.name", "Qualification test")
            git("config", "user.email", "qualification@example.invalid")
            source = root / "source.rs"
            source.write_text("initial", encoding="utf-8")
            git("add", "source.rs")
            git("-c", "commit.gpgsign=false", "commit", "--quiet", "-m", "fixture")
            self.assertEqual(clean_revision(root), git("rev-parse", "HEAD"))
            extra = root / "extra.rs"
            extra.write_text("new input", encoding="utf-8")
            with self.assertRaisesRegex(RuntimeError, "clean checkout"):
                clean_revision(root)
            git("add", "extra.rs")
            with self.assertRaisesRegex(RuntimeError, "clean checkout"):
                clean_revision(root)
            git("-c", "commit.gpgsign=false", "commit", "--quiet", "-m", "extra input")
            source.write_text("changed input", encoding="utf-8")
            with self.assertRaisesRegex(RuntimeError, "clean checkout"):
                clean_revision(root)

    def test_replay_rejects_build_host_and_nested_artifact_drift(self):
        record = {
            "source_commit": "source-a", "executable_sha256": "binary-a",
            "host": {"system": "host-a"},
            "qualification": {"cells": [{"artifact_digest": "artifact-a"}]},
        }
        verify_record(copy.deepcopy(record), record)
        changes = [
            {**record, "source_commit": "source-b"},
            {**record, "executable_sha256": "binary-b"},
            {**record, "host": {"system": "host-b"}},
            {**record, "qualification": {"cells": [{"artifact_digest": "artifact-b"}]}},
        ]
        for changed in changes:
            with self.subTest(changed=changed), self.assertRaisesRegex(RuntimeError, "Stale"):
                verify_record(record, changed)


if __name__ == "__main__":
    unittest.main()
