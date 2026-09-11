"""Clean-source and record freshness contracts for scoped preset qualification."""

import copy
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

from scripts.qualify_theme_presets import clean_revision, qualify_cli, verify_record


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


class CliQualificationTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.binary = Path(self.temporary.name) / "merman-cli"
        self.binary.write_bytes(b"production binary")
        self.payloads = {"svg": b"qualified SVG bytes", "png": b"qualified PNG bytes"}
        source = "stateDiagram-v2\n[*] --> Active\n"
        self.qualification = {"presets": [{"preset": "brutalist", "cells": [
            {"source_id": "state-v1", "source": source,
             "source_digest": hashlib.sha256(source.encode()).hexdigest(),
             "output": output, "png_scale": 4.0,
             "artifact_digest": hashlib.sha256(payload).hexdigest()}
            for output, payload in self.payloads.items()
        ]}]}
        self.commands = []

    def run_cli(self, command, **options):
        self.commands.append(command)
        self.assertEqual(options["input"], self.qualification["presets"][0]["cells"][0]["source"].encode())
        config_path = Path(command[command.index("--config-file") + 1])
        self.assertEqual(json.loads(config_path.read_text()), {"htmlLabels": False})
        output = command[command.index("--format") + 1]
        return subprocess.CompletedProcess(command, 0, stdout=self.payloads[output], stderr=b"")

    def test_cli_matches_every_qualified_target_and_preserves_scale(self):
        evidence = qualify_cli(self.binary, self.qualification, runner=self.run_cli)
        self.assertEqual(evidence["matched_outputs"], 2)
        self.assertEqual(evidence["executable_sha256"], hashlib.sha256(self.binary.read_bytes()).hexdigest())
        self.assertEqual(self.commands[0][-3:], ["--svg-pipeline", "resvg-safe", "-"])
        self.assertEqual(self.commands[1][-3:], ["--scale", "4.0", "-"])

    def test_cli_rejects_changed_output_and_source(self):
        self.payloads["png"] = b"different PNG"
        with self.assertRaisesRegex(RuntimeError, "CLI output differs"):
            qualify_cli(self.binary, self.qualification, runner=self.run_cli)
        self.qualification["presets"][0]["cells"][0]["source"] = "changed source"
        with self.assertRaisesRegex(RuntimeError, "source digest differs"):
            qualify_cli(self.binary, self.qualification, runner=self.run_cli)

    def test_cli_rejects_missing_observations_or_replaced_executable(self):
        with self.assertRaisesRegex(RuntimeError, "actual target observations"):
            qualify_cli(self.binary, {"presets": []}, runner=self.run_cli)

        def replacing_cli(command, **options):
            self.binary.write_bytes(b"replaced executable")
            return self.run_cli(command, **options)

        with self.assertRaisesRegex(RuntimeError, "executable changed"):
            qualify_cli(self.binary, self.qualification, runner=replacing_cli)

    def test_replay_rejects_different_cli_identity(self):
        record = {"cli": {"executable_sha256": "cli-a", "matched_outputs": 2}}
        changed = {"cli": {"executable_sha256": "cli-b", "matched_outputs": 2}}
        with self.assertRaisesRegex(RuntimeError, "Stale"):
            verify_record(record, changed)


if __name__ == "__main__":
    unittest.main()
