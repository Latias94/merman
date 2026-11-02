#!/usr/bin/env python3
"""Exercise the manual bootstrap owner's credential and immutable recovery boundaries."""

from __future__ import annotations

from contextlib import ExitStack
import hashlib
import json
import os
from pathlib import Path
import re
from subprocess import CompletedProcess
import sys
import tempfile
import textwrap
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
from scripts import crates_io_release as release
from scripts import release_preflight_evidence as preflight
from tools import publish

WORKFLOW = (ROOT / ".github/workflows/release-crates-bootstrap.yml").read_text(encoding="utf-8")
NAMES = ("merman-model", "merman-theme-contract", "merman-markup")
VERSION = "0.10.0-alpha.1"
VALIDATE = "Validate immutable dispatch identity"
PREFLIGHT = "Require complete immutable release preflight"
UPLOAD = "Prepare and bootstrap only the three new crates"


def step(name: str) -> str:
    for block in re.split(r"(?m)^      - ", WORKFLOW)[1:]:
        if block.startswith(f"name: {name}\n"):
            return block
    raise AssertionError(f"missing workflow step: {name}")


def python_source(name: str) -> str:
    return textwrap.dedent(step(name).split("        run: |\n", 1)[1])


class BootstrapWorkflowTests(unittest.TestCase):
    def setUp(self) -> None:
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve() / "release-source"
        self.root.mkdir()
        (self.root / "Cargo.toml").write_text(f'[workspace.package]\nversion = "{VERSION}"\n')
        self.receipts = self.root / "target/bootstrap-receipts"
        self.receipts.mkdir(parents=True)
        self.env = {
            "SOURCE_ROOT": str(self.root), "SOURCE_SHA": "a" * 40, "SOURCE_TREE": "b" * 40,
            "WORKFLOW_SHA": "c" * 40, "GITHUB_SHA": "c" * 40, "VERSION": VERSION,
            "GITHUB_REPOSITORY": "Latias94/merman", "GITHUB_REPOSITORY_ID": "42",
            "GITHUB_RUN_ID": "100", "GITHUB_RUN_ATTEMPT": "1", "PREFLIGHT_RUN_ID": "200",
            "GITHUB_OUTPUT": str(self.root / "output"),
        }
        packages = []
        for name in reversed(NAMES):
            directory = self.root / "crates" / name
            directory.mkdir(parents=True)
            manifest = directory / "Cargo.toml"
            manifest.write_text(f'[package]\nname = "{name}"\nversion = "{VERSION}"\n')
            packages.append({
                "id": name, "name": name, "version": VERSION, "manifest_path": str(manifest),
                "dependencies": [{"path": str(self.root / "crates/merman-model")}]
                if name == "merman-markup" else [],
            })
        self.metadata = {
            "packages": packages, "workspace_members": list(NAMES),
            "target_directory": str(self.root / "target"),
        }
        self.registry: dict[str, str] = {}
        self.events: list[tuple[str, str]] = []
        self.barrier_state = "complete"
        self.publish_returncode = 0
        self.dry_run_returncode = 0
        self.corrupt_publish = False

    def execute(self, name: str) -> None:
        exec(compile(python_source(name), f"{name}.py", "exec"), {})

    def assert_credential_free(self) -> None:
        self.assertNotIn("CARGO_REGISTRY_TOKEN", os.environ)
        self.assertNotIn("CARGO_REGISTRIES_CRATES_IO_TOKEN", os.environ)

    def prepare(self, root, target, package):
        self.assert_credential_free()
        self.assertEqual(root, self.root)
        self.events.append(("package", package.name))
        artifact = target / "package" / f"{package.name}-{package.version}.crate"
        artifact.parent.mkdir(parents=True, exist_ok=True)
        artifact.write_bytes(f"original package: {package.name}".encode())
        return release.PreparedCrate(package, artifact, release.sha256_file(artifact),
                                     artifact.stat().st_size, release.sha256_file(package.manifest_path))

    def command(self, args, *, cwd, env=None):
        self.assert_credential_free()
        self.assertEqual(cwd, self.root)
        name = args[3]
        if "--dry-run" in args:
            self.assertNotIn("CARGO_REGISTRY_TOKEN", env or {})
            self.assertIn("--locked", args)
            self.events.append(("dry-run", name))
            return CompletedProcess(args, self.dry_run_returncode)
        self.assertIn("--no-verify", args)
        self.assertIn("--locked", args)
        self.assertEqual(env["CARGO_REGISTRY_TOKEN"], "synthetic-upload-token")
        record = json.loads((self.receipts / f"{name}.attempt-1.json").read_text())
        self.assertEqual(record["states"][-1], "upload-intent")
        self.assertEqual(record["source_sha"], self.env["SOURCE_SHA"])
        self.assertEqual(record["source_tree"], self.env["SOURCE_TREE"])
        artifact = self.root / "target/package" / f"{name}-{VERSION}.crate"
        self.assertEqual(record["artifact_sha256"], release.sha256_file(artifact))
        self.events.append(("publish", name))
        self.registry[name] = record["artifact_sha256"]
        if self.corrupt_publish:
            artifact.write_bytes(b"unexpectedly repackaged bytes")
        return CompletedProcess(args, self.publish_returncode)

    def barrier(self, prepared, **kwargs):
        self.assert_credential_free()
        self.assertEqual(kwargs["registry_api"], release.CRATES_IO_API)
        self.assertEqual(kwargs["registry_index"], release.CRATES_IO_INDEX)
        self.assertGreater(kwargs["attempts"], 0)
        name = prepared[0].package.name
        self.events.append(("barrier", name))
        checksum = self.registry.get(name)
        return release.RegistryBarrier(self.barrier_state, {name: checksum}, {}, {}, {name: checksum})

    def upload(self, *, attempt=1):
        environment = dict(self.env, GITHUB_RUN_ATTEMPT=str(attempt),
                           CARGO_REGISTRY_TOKEN="synthetic-upload-token",
                           CARGO_REGISTRIES_CRATES_IO_TOKEN="synthetic-ambient-token")
        with ExitStack() as stack:
            stack.enter_context(patch.dict(os.environ, environment, clear=True))
            stack.enter_context(patch.object(publish, "cargo_metadata", return_value=self.metadata))
            stack.enter_context(patch.object(publish, "run_command", side_effect=self.command))
            stack.enter_context(patch.object(release, "_prepare_crate", side_effect=self.prepare))
            source_check = stack.enter_context(patch.object(release, "assert_release_source"))
            source_check.side_effect = lambda root, source, tree: (
                self.assert_credential_free(), self.assertEqual(root, self.root),
                self.assertEqual((source, tree), (self.env["SOURCE_SHA"], self.env["SOURCE_TREE"])))
            stack.enter_context(patch.object(release, "fetch_crates_io_checksum",
                                           side_effect=lambda name, version: self.registry.get(name)))
            stack.enter_context(patch.object(release, "reconcile_registry_barrier", side_effect=self.barrier))
            self.execute(UPLOAD)

    def test_owner_is_manual_least_privilege_and_retains_each_attempt(self):
        triggers = WORKFLOW.split("\non:\n", 1)[1].split("\npermissions:", 1)[0]
        self.assertEqual(re.findall(r"(?m)^  ([a-z_]+):", triggers), ["workflow_dispatch"])
        self.assertEqual(re.findall(r"(?m)^      ([a-z_]+):", triggers),
                         ["source_sha", "version", "preflight_run_id"])
        self.assertIn("permissions:\n  contents: read\n  actions: read\n", WORKFLOW)
        self.assertNotRegex(WORKFLOW, r"(?m)^\s+[\w-]+: write$")
        self.assertIn("environment: crates.io", WORKFLOW)
        self.assertIn('CARGO_BUILD_JOBS: "2"', WORKFLOW)
        self.assertIn("dtolnay/rust-toolchain@1.95.0", WORKFLOW)
        self.assertIn("SOURCE_ROOT: release-source", WORKFLOW)
        self.assertIn("ref: ${{ github.workflow_sha }}", step("Check out reviewed bootstrap tooling"))
        self.assertIn("path: release-source", step("Check out immutable product source"))
        self.assertEqual(WORKFLOW.count("persist-credentials: false"), 2)
        recovery = step("Restore original bytes on reruns")
        self.assertIn("if: ${{ github.run_attempt > 1 }}", recovery)
        self.assertIn("name: crates-bootstrap-${{ github.run_id }}-attempt-1", recovery)
        self.assertIn("path: release-source/target", recovery)
        retained = step("Retain original crates and receipts for this attempt")
        self.assertIn("if: always()", retained)
        self.assertIn("attempt-${{ github.run_attempt }}", retained)
        self.assertIn("if-no-files-found: warn", retained)
        self.assertIn("release-source/target/bootstrap-receipts/", retained)
        for name in NAMES:
            self.assertIn(f"release-source/target/bootstrap-crates/{name}-{VERSION}.crate", retained)

    def test_secret_is_confined_to_upload_after_required_preflight(self):
        self.assertEqual(WORKFLOW.count("${{ secrets."), 1)
        self.assertIn("CARGO_REGISTRY_TOKEN: ${{ secrets.CARGO_REGISTRY_TOKEN }}", step(UPLOAD))
        self.assertLess(WORKFLOW.index(f"name: {PREFLIGHT}"), WORKFLOW.index(f"name: {UPLOAD}"))
        self.assertTrue(python_source(UPLOAD).startswith(
            'import os\ntoken = os.environ.pop("CARGO_REGISTRY_TOKEN", "")\n'))
        for name in (VALIDATE, PREFLIGHT, UPLOAD):
            compile(python_source(name), f"{name}.py", "exec")

    def test_dispatch_validation_allows_distinct_reviewed_tooling_and_rejects_bad_inputs(self):
        with patch.dict(os.environ, self.env, clear=True):
            self.execute(VALIDATE)
        for key, value in (("SOURCE_SHA", "main"), ("VERSION", "0.10.0-alpha.2"),
                           ("PREFLIGHT_RUN_ID", "0"), ("PREFLIGHT_RUN_ID", "2\n3")):
            with self.subTest(key=key, value=value), patch.dict(os.environ, dict(self.env, **{key: value}), clear=True):
                with self.assertRaises(SystemExit):
                    self.execute(VALIDATE)

    def test_full_preflight_binds_product_tree_and_exact_run_not_tooling_head(self):
        evidence = {"run_id": 200, "run_attempt": 1, "artifact_id": 300, "artifact_digest": "sha256:test"}
        with ExitStack() as stack:
            stack.enter_context(patch.dict(os.environ, dict(self.env, GH_TOKEN="synthetic-read-token"), clear=True))
            git = stack.enter_context(patch("subprocess.run"))
            stack.enter_context(patch("subprocess.check_output", return_value=self.env["SOURCE_TREE"] + "\n"))
            source_check = stack.enter_context(patch.object(release, "assert_release_source"))
            stack.enter_context(patch.object(preflight, "GitHubApi"))
            finder = stack.enter_context(patch.object(preflight, "find_evidence", return_value=evidence))
            self.execute(PREFLIGHT)
            identity = finder.call_args.args[0]
            self.assertEqual(identity.workflow_sha, self.env["SOURCE_SHA"])
            self.assertEqual(identity.source_sha, self.env["SOURCE_SHA"])
            self.assertEqual(identity.source_tree, self.env["SOURCE_TREE"])
            source_check.assert_called_once_with(self.root, identity.source_sha, identity.source_tree)
            self.assertEqual(git.call_args_list[0].args[0],
                             ["git", "fetch", "--no-tags", "origin", "+refs/heads/main:refs/remotes/origin/main"])
            self.assertIn("--is-ancestor", git.call_args_list[1].args[0])
            self.assertTrue(all(call.kwargs["cwd"] == self.root and call.kwargs["check"] for call in git.call_args_list))
            for result in (None, dict(evidence, run_id=201)):
                finder.return_value = result
                with self.assertRaises(SystemExit):
                    self.execute(PREFLIGHT)
        receipt = json.loads((self.receipts / "preflight.json").read_text())
        self.assertEqual(receipt["tooling_sha"], self.env["WORKFLOW_SHA"])
        self.assertEqual(receipt["workflow_sha"], receipt["source_sha"])

    def test_each_missing_crate_is_dry_run_uploaded_once_and_barriered_in_graph_order(self):
        self.upload()
        self.assertEqual(self.events, [(action, name) for name in NAMES
                                       for action in ("package", "dry-run", "publish", "barrier")])
        for name in NAMES:
            record = json.loads((self.receipts / f"{name}.attempt-1.json").read_text())
            self.assertEqual(record["states"], ["packaged", "prepared", "upload-intent", "upload-returned", "complete"])

    def test_existing_matching_crates_skip_upload_and_conflicts_stop(self):
        self.registry.update({name: hashlib.sha256(f"original package: {name}".encode()).hexdigest() for name in NAMES})
        self.upload()
        self.assertEqual(self.events, [(action, name) for name in NAMES for action in ("package", "barrier")])
        self.events.clear()
        self.registry[NAMES[0]] = "d" * 64
        with self.assertRaisesRegex(RuntimeError, "checksum conflicts"):
            self.upload()
        self.assertEqual(self.events, [("package", NAMES[0])])

    def test_dependency_closure_and_version_are_checked_before_packaging(self):
        self.metadata["packages"][0]["version"] = "0.10.0-alpha.2"
        with self.assertRaisesRegex(RuntimeError, "dependency closure"):
            self.upload()
        self.metadata["packages"][0]["version"] = VERSION
        other = dict(self.metadata["packages"][1], id="outside", name="outside",
                     manifest_path=str(self.root / "outside/Cargo.toml"), dependencies=[])
        self.metadata["packages"].append(other)
        self.metadata["workspace_members"].append("outside")
        self.metadata["packages"][0]["dependencies"].append({"path": str(self.root / "outside")})
        with self.assertRaisesRegex(RuntimeError, "dependency closure"):
            self.upload()
        self.assertEqual(self.events, [])

    def test_dry_run_failure_prevents_any_upload(self):
        self.dry_run_returncode = 1
        with self.assertRaisesRegex(RuntimeError, "dry-run failed"):
            self.upload()
        self.assertEqual(self.events, [("package", NAMES[0]), ("dry-run", NAMES[0])])

    def test_response_loss_only_observes_and_pending_recovery_retains_intent(self):
        self.publish_returncode = 17
        self.barrier_state = "pending_recovery"
        with self.assertRaisesRegex(RuntimeError, "never resend"):
            self.upload()
        record = json.loads((self.receipts / f"{NAMES[0]}.attempt-1.json").read_text())
        self.assertIn("upload-intent", record["states"])
        self.assertIn("pending_recovery", record["states"])
        self.assertEqual(record["returncode"], 17)
        self.events.clear()
        with self.assertRaisesRegex(RuntimeError, "never resend"):
            self.upload(attempt=2)
        self.assertEqual(self.events, [("barrier", NAMES[0])])

    def test_lost_response_with_matching_api_and_index_does_not_repeat_upload(self):
        self.publish_returncode = 17
        self.upload()
        self.assertEqual([event for event in self.events if event[0] == "publish"],
                         [("publish", name) for name in NAMES])
        for name in NAMES:
            record = json.loads((self.receipts / f"{name}.attempt-1.json").read_text())
            self.assertEqual(record["returncode"], 17)
            self.assertEqual(record["states"][-1], "complete")

    def test_rerun_only_observes_original_bytes_and_never_prepares_unreached_markup(self):
        self.upload()
        self.events.clear()
        self.registry.clear()
        self.barrier_state = "pending_recovery"
        with self.assertRaisesRegex(RuntimeError, "never resend"):
            self.upload(attempt=2)
        self.assertEqual(self.events, [("barrier", NAMES[0])])
        self.events.clear()
        self.barrier_state = "complete"
        (self.receipts / "merman-markup.prepared.json").unlink()
        with self.assertRaisesRegex(RuntimeError, "no original prepared bytes; manual review required"):
            self.upload(attempt=3)
        self.assertEqual(self.events, [("barrier", name) for name in NAMES[:2]])

    def test_changed_original_or_repackaged_bytes_fail_before_further_publication(self):
        self.corrupt_publish = True
        with self.assertRaisesRegex(release.CratesIoPublishError, "artifact changed"):
            self.upload()
        self.assertEqual([event for event in self.events if event[0] == "publish"], [("publish", NAMES[0])])
        original = self.root / "target/bootstrap-crates" / f"{NAMES[0]}-{VERSION}.crate"
        self.assertEqual(original.read_bytes(), f"original package: {NAMES[0]}".encode())
        original.write_bytes(b"tampered retained original")
        self.events.clear()
        with self.assertRaisesRegex(release.CratesIoPublishError, "artifact changed"):
            self.upload(attempt=2)
        self.assertEqual(self.events, [])


if __name__ == "__main__":
    unittest.main()
