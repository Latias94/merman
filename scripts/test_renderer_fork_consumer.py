"""Contracts for the archive-only renderer fork compatibility check."""

from __future__ import annotations

import io
import json
from pathlib import Path
import subprocess
import tarfile
import tempfile
import tomllib
import unittest

from scripts import renderer_fork_consumer as consumer


VERSIONS = dict(zip(consumer.FORKS, ("0.8.2", "0.8.1", "0.50.0", "0.50.1"), strict=True))


def resolution(candidate: str, path: Path) -> dict:
    packages = [
        {"id": name, "name": name, "version": version,
         "source": None if name == candidate else consumer.CRATES_IO_SOURCE,
         "manifest_path": str(path / "Cargo.toml") if name == candidate else "unused"}
        for name, version in VERSIONS.items()
    ]
    return {"packages": packages, "resolve": {"nodes": [{"id": package["id"]} for package in packages]}}


class RendererForkConsumerTests(unittest.TestCase):
    def test_each_candidate_archive_is_the_only_patch_in_a_fresh_consumer(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "rust-toolchain.toml").write_text('[toolchain]\nchannel = "stable"\n', encoding="utf-8")
            for candidate, version in VERSIONS.items():
                with self.subTest(candidate=candidate):
                    archive = root / f"{candidate}-{version}.crate"
                    cargo_toml = f'[package]\nname = "{candidate}"\nversion = "{version}"\n'.encode()
                    with tarfile.open(archive, "w:gz") as package:
                        member = tarfile.TarInfo(f"{archive.stem}/Cargo.toml")
                        member.size = len(cargo_toml)
                        package.addfile(member, io.BytesIO(cargo_toml))
                    calls = []

                    def run(command, *, cwd, env):
                        calls.append(command)
                        self.assertEqual(Path(env["CARGO_TARGET_DIR"]), (root / "target").resolve())
                        self.assertEqual(env["RUSTUP_TOOLCHAIN"], "stable")
                        self.assertEqual((cwd / "src/main.rs").read_text(encoding="utf-8"), consumer.CONSUMER)
                        manifest = tomllib.loads((cwd / "Cargo.toml").read_text(encoding="utf-8"))
                        patches = manifest["patch"]["crates-io"]
                        self.assertEqual(set(patches), {candidate})
                        self.assertEqual(
                            {dep["package"]: dep["version"] for dep in manifest["dependencies"].values()},
                            {name: f"={version}" for name, version in VERSIONS.items()},
                        )
                        unpacked = Path(patches[candidate]["path"])
                        self.assertEqual((unpacked / "Cargo.toml").read_bytes(), cargo_toml)
                        lock = cwd / "Cargo.lock"
                        if command[1] == "metadata":
                            self.assertFalse(lock.exists())
                            lock.write_text("fresh lock", encoding="utf-8")
                        else:
                            self.assertEqual(lock.read_text(encoding="utf-8"), "fresh lock")
                        return subprocess.CompletedProcess(
                            command, 0, json.dumps(resolution(candidate, unpacked)), ""
                        )

                    consumer.verify(
                        archive, candidate=candidate, versions=VERSIONS, repo_root=root,
                        target_directory=root / "target", run_check=run,
                    )
                    self.assertEqual(calls, [
                        ["cargo", "metadata", "--format-version", "1"], ["cargo", "check", "--locked"]
                    ])

    def test_resolution_rejects_duplicate_forks_and_wrong_sources(self) -> None:
        candidate, sibling = "merman-usvg", "merman-resvg"
        path = Path(__file__).resolve().parent
        for name, change in (
            (candidate, {"manifest_path": str(path / "elsewhere/Cargo.toml")}),
            (candidate, {"source": consumer.CRATES_IO_SOURCE}),
            (sibling, {"source": None}),
            (sibling, {"version": "0.50.3"}),
        ):
            with self.subTest(name=name, change=change):
                metadata = resolution(candidate, path)
                next(package for package in metadata["packages"] if package["name"] == name).update(change)
                with self.assertRaisesRegex(consumer.RegistryDependentError, f"expected exactly one {name}"):
                    consumer._verify_resolution(metadata, VERSIONS, candidate, path)
        metadata = resolution(candidate, path)
        metadata["packages"].append({
            "id": "second", "name": candidate, "version": "0.50.0", "source": consumer.CRATES_IO_SOURCE
        })
        metadata["resolve"]["nodes"].append({"id": "second"})
        with self.assertRaisesRegex(consumer.RegistryDependentError, f"expected exactly one {candidate}"):
            consumer._verify_resolution(metadata, VERSIONS, candidate, path)
