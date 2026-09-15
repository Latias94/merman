"""Clean-source and record freshness contracts for scoped preset qualification."""

import copy
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

from scripts.qualify_theme_presets import BUILD, clean_revision, qualify_cli, verify_record


class PresetQualificationTests(unittest.TestCase):
    def test_build_recipe_includes_svg_and_png_layout_features(self):
        self.assertEqual(BUILD[BUILD.index("--features") + 1], "png,layout-cytoscape")

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


    def test_replay_preserves_json_types_and_ignores_object_key_order(self):
        record = {"schema_version": 1, "render_config": {"htmlLabels": False}, "scale": 1.0}
        verify_record({"scale": 1.0, "render_config": {"htmlLabels": False}, "schema_version": 1}, record)
        for changed in [
            {**record, "schema_version": True},
            {**record, "render_config": {"htmlLabels": 0}},
            {**record, "scale": 1},
        ]:
            with self.subTest(changed=changed), self.assertRaisesRegex(RuntimeError, "Stale"):
                verify_record(changed, record)


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
        self.catalog = {"schema_version": 1, "presets": [
            {"id": "brutalist", "display_name": "Brutalist", "appearance": "light", "maturity": "alpha", "available": True, "availability_reason_ids": [], "qualified_cells": [], "license_expression": "MIT OR Apache-2.0", "required_attribution": None, "export_kind": "complete_spec"},
            {"id": "editor-light", "display_name": "Editor Light", "appearance": "light", "maturity": "alpha", "available": True, "availability_reason_ids": [], "qualified_cells": [], "license_expression": "MIT OR Apache-2.0", "required_attribution": None, "export_kind": "complete_spec"},
        ]}
        self.qualification["catalog"] = copy.deepcopy(self.catalog)
        preset = self.qualification["presets"][0]
        preset["profile"] = "native-flowchart-state-sequence-system-fonts-v1"
        for cell in preset["cells"]:
            cell["family"] = "state"
            cell["admission"] = "host_dependent"
        preset["catalog_entry"] = {**self.catalog["presets"][0], "qualified_cells": [
            {"family_id": cell["family"], "output_id": cell["output"],
             "profile_id": preset["profile"], "admission_status": cell["admission"]}
            for cell in preset["cells"]
        ]}
        preset["catalog_entry"]["qualified_cells"].sort(
            key=lambda cell: (cell["family_id"], cell["output_id"], cell["profile_id"])
        )
        self.commands = []

    def run_cli(self, command, **options):
        self.commands.append(command)
        if command[1:] == ["capabilities", "--json"]:
            self.assertEqual(options["input"], b"")
            return subprocess.CompletedProcess(command, 0, stdout=json.dumps({
                "theme_presets": self.catalog,
            }).encode(), stderr=b"")
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

    def test_catalog_qualifies_only_receipt_scopes_after_matching_production_metadata(self):
        evidence = qualify_cli(self.binary, self.qualification, runner=self.run_cli)
        catalog = evidence["catalog"]
        self.assertEqual(catalog["presets"][0], self.qualification["presets"][0]["catalog_entry"])
        self.assertEqual([cell["output_id"] for cell in catalog["presets"][0]["qualified_cells"]], ["png", "svg"])
        self.assertEqual(catalog["presets"][1], self.catalog["presets"][1])
        self.assertEqual(self.catalog["presets"][0]["qualified_cells"], [])
        self.assertEqual(self.qualification["catalog"]["presets"][0]["qualified_cells"], [])
        self.assertIn([str(self.binary.resolve()), "capabilities", "--json"], self.commands)

    def test_catalog_rejects_drift_and_scope_forgery(self):
        original = copy.deepcopy(self.qualification)
        changes = []
        for field, value in [("family_id", "flowchart"), ("output_id", "pdf"),
                             ("admission_status", "portable"), ("profile_id", "different-profile")]:
            changed = copy.deepcopy(original)
            changed["presets"][0]["catalog_entry"]["qualified_cells"][0][field] = value
            changes.append(changed)
        changed = copy.deepcopy(original)
        changed["presets"][0]["catalog_entry"]["qualified_cells"].pop()
        changes.append(changed)
        changed = copy.deepcopy(original)
        changed["presets"][0]["cells"].append(copy.deepcopy(changed["presets"][0]["cells"][0]))
        changes.append(changed)
        changed = copy.deepcopy(original)
        changed["presets"].append(copy.deepcopy(changed["presets"][0]))
        changes.append(changed)
        changed = copy.deepcopy(original)
        changed["presets"][0]["catalog_entry"]["maturity"] = "stable"
        changes.append(changed)
        for field, value in [("profile", "future-profile"), ("family", "future-family"),
                             ("output", "pdf"), ("admission", "future-admission")]:
            changed = copy.deepcopy(original)
            if field == "profile":
                changed["presets"][0][field] = value
            else:
                changed["presets"][0]["cells"][0][field] = value
            with self.subTest(field=field), self.assertRaisesRegex(
                RuntimeError, "(qualification (profile|family|admission)|Undeclared CLI qualification target)"
            ):
                qualify_cli(self.binary, changed, runner=self.run_cli)
        for changed in changes:
            with self.subTest(changed=changed), self.assertRaisesRegex(RuntimeError, "catalog|scope"):
                qualify_cli(self.binary, changed, runner=self.run_cli)
        self.catalog["schema_version"] = True
        with self.assertRaisesRegex(RuntimeError, "catalog"):
            qualify_cli(self.binary, original, runner=self.run_cli)
        self.catalog["schema_version"] = 1
        self.catalog["presets"][0]["id"] = "another-preset"
        with self.assertRaisesRegex(RuntimeError, "catalog"):
            qualify_cli(self.binary, original, runner=self.run_cli)

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
