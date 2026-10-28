#!/usr/bin/env python3
"""Exercise diagram-size experiment staging without compiling Rust."""

from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest import mock

import measure_diagram_selection as measure


class DiagramSelectionTests(unittest.TestCase):
    def setUp(self) -> None:
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.parent = Path(temporary.name)
        self.root = self.parent / "repo"
        self.root.mkdir()
        self.real_run = subprocess.run
        self.git("init", "--quiet")
        self.git("config", "user.name", "Diagram selection test")
        self.git("config", "user.email", "test@example.invalid")
        self.git("config", "commit.gpgsign", "false")
        self.git("config", "core.autocrlf", "false")
        (self.root / "rust-toolchain.toml").write_text(
            '[toolchain]\nchannel = "1.95.0"\n', encoding="utf-8"
        )
        (self.root / "Cargo.lock").write_bytes(b"version = 4\n# historical lock\n")
        (self.root / "crates/merman").mkdir(parents=True)
        for path in measure.CORPUS:
            fixture = self.root / path
            fixture.parent.mkdir(parents=True, exist_ok=True)
            fixture.write_bytes(b"flowchart TD\nA --> B\n")
        self.git("add", ".")
        self.git("commit", "--quiet", "-m", "historical library without probe")
        self.baseline = self.git("rev-parse", "HEAD").decode().strip()
        probe = self.root / measure.FIXTURE_RELATIVE
        probe.parent.mkdir(parents=True)
        self.fixture_bytes = b"fn main() { /* committed candidate probe */ }\n"
        probe.write_bytes(self.fixture_bytes)
        self.git("add", ".")
        self.git("commit", "--quiet", "-m", "add shared probe")
        self.candidate = self.git("rev-parse", "HEAD").decode().strip()
        # Neither dirty probe bytes nor the candidate's modified lock may enter the baseline.
        probe.write_bytes(b"fn main() { panic!(\"dirty worktree\"); }\n")
        (self.root / "Cargo.lock").write_bytes(b"version = 4\n# unrelated worktree lock\n")
        self.calls: list[tuple[list[str], Path, dict[str, str]]] = []
        self.build_toolchain: str | None = None
        self.full = False
        self.target = "x86_64-test-host"
        self.suffix = ".exe" if os.name == "nt" else ""
        for name in ("platform", "processor"):
            patch = mock.patch.object(measure.platform, name, return_value="test host")
            patch.start()
            self.addCleanup(patch.stop)
        self.root_patch = mock.patch.object(measure, "ROOT", self.root)
        self.root_patch.start()
        self.addCleanup(self.root_patch.stop)
        self.run_patch = mock.patch.object(measure.subprocess, "run", side_effect=self.fake_run)
        self.run_patch.start()
        self.addCleanup(self.run_patch.stop)

    def git(self, *args: str) -> bytes:
        return self.real_run(
            ["git", *args], cwd=self.root, check=True, capture_output=True
        ).stdout

    def fake_run(self, command: list[str], **kwargs: object) -> subprocess.CompletedProcess:
        if command[0] == "git":
            return self.real_run(command, **kwargs)
        cwd = Path(kwargs["cwd"])
        env = dict(kwargs["env"])
        self.calls.append((list(command), cwd, env))
        if command[:2] == ["rustup", "run"]:
            toolchain, program, *args = command[2:]
            if program == "rustc":
                output = f"rustc {toolchain}\nhost: {self.target}\n".encode()
            elif args == ["-V"]:
                output = f"cargo {toolchain}\n".encode()
            elif args[0] == "metadata":
                output = b'{"packages": []}\n'
            elif args[0] == "build":
                self.build_toolchain = toolchain
                binary = Path(env["CARGO_TARGET_DIR"]) / self.target / "release" / ("diagram-selection-probe" + self.suffix)
                binary.parent.mkdir(parents=True)
                binary.write_bytes(b"stand-in for the measured executable")
                output = b""
            elif args[0] == "tree":
                output = b"merman v0.0.0|svg\n"
            else:
                raise AssertionError(command)
            return subprocess.CompletedProcess(command, 0, stdout=output, stderr=b"")
        if Path(command[0]).name == "diagram-selection-probe" + self.suffix:
            if command[1] == "--families":
                output = b"flowchart\ngantt\n" + (b"sequence\n" if self.full else b"")
            elif Path(command[1]).name == "sequence.mmd" and not self.full:
                return subprocess.CompletedProcess(
                    command, 2, stdout=b"", stderr=b"merman.parse.unsupported_diagram\n"
                )
            else:
                output = b"<svg>rendered fixture</svg>"
            return subprocess.CompletedProcess(command, 0, stdout=output, stderr=b"")
        raise AssertionError(f"unexpected or unpinned external command: {command}")

    def collect(self, output: Path, **overrides: object) -> dict[str, object]:
        arguments = {
            "revision": self.baseline,
            "fixture_revision": self.candidate,
            "features": ["svg"],
            "label": "pre-change-full",
            "output": output,
        }
        arguments.update(overrides)
        return measure.measure(**arguments)

    def test_historical_source_uses_committed_shared_probe_and_its_own_lock(self) -> None:
        self.full = True
        output = self.parent / "external-output"
        receipt = self.collect(output)
        self.assertFalse((output / "source" / measure.FIXTURE_RELATIVE).exists())
        self.assertEqual((output / "consumer/main.rs").read_bytes(), self.fixture_bytes)
        self.assertEqual(receipt["source_revision"], self.baseline)
        self.assertEqual(receipt["fixture_revision"], self.candidate)
        self.assertEqual(receipt["fixture_sha256"], hashlib.sha256(self.fixture_bytes).hexdigest())
        historical_lock = self.git("show", f"{self.baseline}:Cargo.lock")
        self.assertEqual((output / "consumer/Cargo.lock").read_bytes(), historical_lock)
        self.assertEqual(receipt["root_lock_sha256"], hashlib.sha256(historical_lock).hexdigest())
        self.assertEqual(receipt["sequence_exit"], 0)
        self.assertEqual(json.loads((output / "receipt.json").read_text()), receipt)

    def test_external_output_uses_one_toolchain_for_build_and_receipt(self) -> None:
        for explicit, expected in [(None, "1.95.0"), ("1.94.0", "1.94.0")]:
            with self.subTest(toolchain=explicit), mock.patch.dict(
                os.environ,
                {"RUSTUP_TOOLCHAIN": "nightly", "RUSTFLAGS": "-Copt-level=0", "CARGO_PROFILE_RELEASE_LTO": "off"},
            ):
                self.calls.clear()
                output = self.parent / f"external-{expected}"
                receipt = self.collect(output, toolchain=explicit)
                rust_calls = [call for call in self.calls if call[0][0] == "rustup"]
                self.assertEqual(len(rust_calls), 5)
                for command, cwd, env in rust_calls:
                    self.assertEqual(command[:3], ["rustup", "run", expected])
                    self.assertEqual(cwd, (output / "consumer").resolve())
                    self.assertEqual(env["RUSTUP_TOOLCHAIN"], expected)
                    self.assertNotIn("RUSTFLAGS", env)
                    self.assertNotIn("CARGO_PROFILE_RELEASE_LTO", env)
                self.assertEqual(receipt["toolchain"], self.build_toolchain)
                self.assertEqual(receipt["cargo"], f"cargo {self.build_toolchain}")
                self.assertIn(f"rustc {self.build_toolchain}\n", receipt["rustc"])
                self.assertEqual(receipt["sequence_exit"], 2)

    def test_missing_probe_revision_fails_before_creating_evidence(self) -> None:
        output = self.parent / "bad-fixture"
        with self.assertRaises(subprocess.CalledProcessError):
            self.collect(output, fixture_revision=self.baseline)
        self.assertFalse(output.exists())
        self.assertEqual(self.calls, [])

    def test_existing_evidence_is_never_overwritten(self) -> None:
        output = self.parent / "existing"
        output.mkdir()
        receipt = output / "receipt.json"
        receipt.write_bytes(b"original evidence")
        with self.assertRaises(FileExistsError):
            self.collect(output)
        self.assertEqual(receipt.read_bytes(), b"original evidence")
        self.assertEqual(self.calls, [])


if __name__ == "__main__":
    unittest.main()
