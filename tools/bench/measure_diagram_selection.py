#!/usr/bin/env python3
"""Measure one frozen native diagram-selection recipe; retain its executable and evidence."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import tarfile

ROOT = Path(__file__).resolve().parents[2]
FIXTURE_RELATIVE = Path("tools/bench/fixtures/diagram-selection/main.rs")
CORPUS = (
    "crates/merman/benches/fixtures/flowchart_small.mmd",
    "crates/merman/benches/fixtures/flowchart_medium.mmd",
    "crates/merman/benches/fixtures/gantt_medium.mmd",
)
PROFILE = {
    "opt-level": "s", "lto": "thin", "codegen-units": 1,
    "panic": "abort", "strip": "symbols", "debug": False, "incremental": False,
}


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(command: list[str], *, cwd: Path, env: dict[str, str]) -> bytes:
    return subprocess.run(command, cwd=cwd, env=env, check=True, capture_output=True).stdout


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--revision", required=True, help="Committed source to archive, never the moving worktree.")
    parser.add_argument("--features", required=True, help="Exact facade Cargo features, with defaults disabled.")
    parser.add_argument("--label", required=True)
    parser.add_argument("--output", type=Path, required=True, help="New evidence directory; existing paths are rejected.")
    args = parser.parse_args()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    env = dict(os.environ)
    # Freeze all Cargo release knobs instead of accepting ambient profile overrides.
    for key in list(env):
        if key.startswith("CARGO_PROFILE_RELEASE_") or key in {"RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS"}:
            del env[key]
    env["CARGO_TARGET_DIR"] = str(output / "cargo-target")
    env["CARGO_INCREMENTAL"] = "0"
    rustc = run(["rustc", "-vV"], cwd=ROOT, env=env).decode()
    target = next(line.removeprefix("host: ") for line in rustc.splitlines() if line.startswith("host: "))
    revision = run(["git", "rev-parse", "--verify", args.revision + "^{commit}"], cwd=ROOT, env=env).decode().strip()
    archive = output / "source.tar"
    with archive.open("wb") as handle:
        subprocess.run(["git", "archive", revision], cwd=ROOT, env=env, check=True, stdout=handle)
    source = output / "source"
    source.mkdir()
    with tarfile.open(archive) as handle:
        handle.extractall(source, filter="data")
    consumer = output / "consumer"
    consumer.mkdir()
    fixture = source / FIXTURE_RELATIVE
    shutil.copyfile(fixture, consumer / "main.rs")
    features = sorted(set(args.features.split(",")))
    manifest = (
        '[package]\nname = "diagram-selection-probe"\nversion = "0.0.0"\nedition = "2024"\n'
        '[workspace]\n[[bin]]\nname = "diagram-selection-probe"\npath = "main.rs"\n'
        '[dependencies]\nmerman = { path = ' + json.dumps((source / "crates/merman").as_posix())
        + ', default-features = false, features = ' + json.dumps(features) + ' }\n'
        '[profile.release]\n' + ''.join(f'{key} = {json.dumps(value)}\n' for key, value in PROFILE.items())
    )
    (consumer / "Cargo.toml").write_text(manifest, encoding="utf-8")
    shutil.copyfile(source / "Cargo.lock", consumer / "Cargo.lock")
    # Cargo retains locked versions while adding only this independent consumer package.
    metadata = run(["cargo", "metadata", "--offline", "--format-version", "1"], cwd=consumer, env=env)
    (output / "metadata.json").write_bytes(metadata)
    command = ["cargo", "build", "--locked", "--offline", "--release", "--target", target, "-j", "2"]
    with (output / "build.log").open("wb") as log:
        subprocess.run(command, cwd=consumer, env=env, check=True, stdout=log, stderr=subprocess.STDOUT)
    suffix = ".exe" if os.name == "nt" else ""
    executable = output / ("diagram-selection-probe" + suffix)
    shutil.copyfile(output / "cargo-target" / target / "release" / executable.name, executable)
    if os.name != "nt":
        executable.chmod(0o755)
    families = run([str(executable), "--families"], cwd=source, env=env).decode().splitlines()
    tree = run(["cargo", "tree", "--locked", "--offline", "--edges", "normal", "--prefix", "none", "--format", "{p}|{f}"], cwd=consumer, env=env)
    (output / "cargo-tree.txt").write_bytes(tree)
    corpus = []
    for relative in CORPUS:
        input_path = source / relative
        svg = run([str(executable), str(input_path)], cwd=source, env=env)
        if b"<svg" not in svg:
            raise RuntimeError(f"missing SVG output for {relative}")
        (output / (input_path.stem + ".svg")).write_bytes(svg)
        corpus.append({"path": relative, "input_sha256": digest(input_path), "svg_bytes": len(svg), "svg_sha256": hashlib.sha256(svg).hexdigest()})
    sequence = output / "sequence.mmd"
    sequence.write_text("sequenceDiagram\nAlice->>Bob: Hello\n", encoding="utf-8")
    rejected = subprocess.run([str(executable), str(sequence)], cwd=source, env=env, capture_output=True)
    if "sequence" in families:
        if rejected.returncode != 0 or b"<svg" not in rejected.stdout:
            raise RuntimeError("full family build failed Sequence control")
    elif rejected.returncode != 2 or rejected.stderr.strip() != b"merman.parse.unsupported_diagram":
        raise RuntimeError("subset did not return the specified unsupported-diagram error")
    receipt = {
        "label": args.label, "source_revision": revision, "source_archive_sha256": digest(archive),
        "fixture_sha256": digest(fixture), "root_lock_sha256": digest(source / "Cargo.lock"),
        "consumer_lock_sha256": digest(consumer / "Cargo.lock"), "features": features,
        "default_features": False, "profile": PROFILE, "command": command,
        "host": platform.platform(), "cpu": platform.processor(),
        "rustc": rustc, "target": target,
        "cargo": run(["cargo", "-V"], cwd=ROOT, env=env).decode().strip(),
        "executable_bytes": executable.stat().st_size, "executable_sha256": digest(executable),
        "diagram_families": families, "corpus": corpus,
        "sequence_exit": rejected.returncode, "sequence_stderr": rejected.stderr.decode().strip(),
    }
    (output / "receipt.json").write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"label": args.label, "bytes": receipt["executable_bytes"], "families": families}))


if __name__ == "__main__":
    main()
