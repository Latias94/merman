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
import tomllib

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
SEMANTIC_FIXTURE = Path("tools/bench/fixtures/semantic-parser/main.rs")
SEMANTIC_CORPUS = tuple(
    f"crates/merman/benches/fixtures/{family}_medium.mmd"
    for family in ("flowchart", "sequence", "state")
)


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(command: list[str], *, cwd: Path, env: dict[str, str]) -> bytes:
    return subprocess.run(command, cwd=cwd, env=env, check=True, capture_output=True).stdout


def measure(
    *, revision: str, fixture_revision: str, features: list[str], label: str,
    output: Path, toolchain: str | None = None, recipe: str = "svg-selection",
) -> dict[str, object]:
    """Measure committed library sources with an independently pinned consumer."""
    if recipe not in {"svg-selection", "semantic-parser"}:
        raise ValueError(f"unknown recipe: {recipe}")
    semantic = recipe == "semantic-parser"
    fixture_relative = SEMANTIC_FIXTURE if semantic else FIXTURE_RELATIVE
    corpus_paths = SEMANTIC_CORPUS if semantic else CORPUS
    dependency = "merman-core" if semantic else "merman"
    profile = {**PROFILE, "strip": "none"} if semantic else PROFILE
    output = output.resolve()
    env = dict(os.environ)
    # Freeze all Cargo release knobs instead of accepting ambient profile overrides.
    for key in list(env):
        if key.startswith("CARGO_PROFILE_RELEASE_") or key in {"RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS"}:
            del env[key]
    env["CARGO_TARGET_DIR"] = str(output / "cargo-target")
    env["CARGO_INCREMENTAL"] = "0"
    env["CARGO_BUILD_JOBS"] = "1"
    # Use one explicit Rustup toolchain even outside this checkout's directory tree.
    if toolchain is None:
        toolchain = tomllib.loads((ROOT / "rust-toolchain.toml").read_text(encoding="utf-8"))["toolchain"]["channel"]
    env["RUSTUP_TOOLCHAIN"] = toolchain
    rust = ["rustup", "run", toolchain]
    revision = run(["git", "rev-parse", "--verify", revision + "^{commit}"], cwd=ROOT, env=env).decode().strip()
    fixture_revision = run(["git", "rev-parse", "--verify", fixture_revision + "^{commit}"], cwd=ROOT, env=env).decode().strip()
    fixture_bytes = run(["git", "show", f"{fixture_revision}:{fixture_relative.as_posix()}"], cwd=ROOT, env=env)
    output.mkdir(parents=True, exist_ok=False)
    archive = output / "source.tar"
    with archive.open("wb") as handle:
        subprocess.run(["git", "archive", revision], cwd=ROOT, env=env, check=True, stdout=handle)
    source = output / "source"
    source.mkdir()
    with tarfile.open(archive) as handle:
        handle.extractall(source, filter="data")
    consumer = output / "consumer"
    consumer.mkdir()
    fixture = consumer / "main.rs"
    fixture.write_bytes(fixture_bytes)
    features = sorted(set(features))
    if semantic and features != ["diagram-flowchart", "diagram-sequence", "diagram-state"]:
        raise ValueError("semantic-parser requires exactly Flowchart, Sequence and State")
    manifest = (
        '[package]\nname = "diagram-selection-probe"\nversion = "0.0.0"\nedition = "2024"\n'
        '[workspace]\n[[bin]]\nname = "diagram-selection-probe"\npath = "main.rs"\n'
        '[dependencies]\n' + dependency + ' = { path = ' + json.dumps((source / "crates" / dependency).as_posix())
        + ', default-features = false, features = ' + json.dumps(features) + ' }\n'
        '[profile.release]\n' + ''.join(f'{key} = {json.dumps(value)}\n' for key, value in profile.items())
    )
    (consumer / "Cargo.toml").write_text(manifest, encoding="utf-8")
    shutil.copyfile(source / "Cargo.lock", consumer / "Cargo.lock")
    rustc = run([*rust, "rustc", "-vV"], cwd=consumer, env=env).decode()
    cargo = run([*rust, "cargo", "-V"], cwd=consumer, env=env).decode().strip()
    target = next(line.removeprefix("host: ") for line in rustc.splitlines() if line.startswith("host: "))
    # Cargo retains locked versions while adding only this independent consumer package.
    metadata = run([*rust, "cargo", "metadata", "--offline", "--format-version", "1"], cwd=consumer, env=env)
    (output / "metadata.json").write_bytes(metadata)
    command = [*rust, "cargo", "build", "--locked", "--offline", "--release", "--target", target, "-j", "1"]
    with (output / "build.log").open("wb") as log:
        subprocess.run(command, cwd=consumer, env=env, check=True, stdout=log, stderr=subprocess.STDOUT)
    suffix = ".exe" if os.name == "nt" else ""
    executable = output / ("diagram-selection-probe" + suffix)
    shutil.copyfile(output / "cargo-target" / target / "release" / executable.name, executable)
    if os.name != "nt":
        executable.chmod(0o755)
    raw_artifact = None
    strip_command = None
    if semantic:
        raw = output / ("diagram-selection-probe.raw" + suffix)
        shutil.copy2(executable, raw)
        raw_artifact = {"bytes": raw.stat().st_size, "sha256": digest(raw)}
        sysroot = Path(run([*rust, "rustc", "--print", "sysroot"], cwd=consumer, env=env).decode().strip())
        objcopy = sysroot / "lib" / "rustlib" / target / "bin" / ("llvm-objcopy" + suffix)
        strip_command = [str(objcopy), "--strip-all", str(executable)]
        run(strip_command, cwd=consumer, env=env)
    families = run([str(executable), "--families"], cwd=source, env=env).decode().splitlines()
    tree = run([*rust, "cargo", "tree", "--locked", "--offline", "--edges", "normal", "--prefix", "none", "--format", "{p}|{f}"], cwd=consumer, env=env)
    (output / "cargo-tree.txt").write_bytes(tree)
    corpus = []
    for relative in corpus_paths:
        input_path = source / relative
        svg = run([str(executable), str(input_path)], cwd=source, env=env)
        if semantic and svg != run([str(raw), str(input_path)], cwd=source, env=env):
            raise RuntimeError(f"stripping changed semantic output for {relative}")
        if not semantic and b"<svg" not in svg:
            raise RuntimeError(f"missing SVG output for {relative}")
        if semantic and not svg.startswith(input_path.stem.split("_", 1)[0].encode() + b"\n"):
            raise RuntimeError(f"missing semantic output for {relative}")
        output_kind = "semantic" if semantic else "svg"
        (output / (input_path.stem + (".txt" if semantic else ".svg"))).write_bytes(svg)
        corpus.append({"path": relative, "input_sha256": digest(input_path), f"{output_kind}_bytes": len(svg), f"{output_kind}_sha256": hashlib.sha256(svg).hexdigest()})
    sequence = output / "sequence.mmd"
    sequence.write_text("sequenceDiagram\nAlice->>Bob: Hello\n", encoding="utf-8")
    rejected = subprocess.run([str(executable), str(sequence)], cwd=source, env=env, capture_output=True)
    if "sequence" in families:
        valid_output = rejected.stdout.startswith(b"sequence\n") if semantic else b"<svg" in rejected.stdout
        if rejected.returncode != 0 or not valid_output:
            raise RuntimeError("full family build failed Sequence control")
    elif rejected.returncode != 2 or rejected.stderr.strip() != b"merman.parse.unsupported_diagram":
        raise RuntimeError("subset did not return the specified unsupported-diagram error")
    receipt = {
        "label": label, "source_revision": revision, "source_archive_sha256": digest(archive),
        "fixture_revision": fixture_revision, "fixture_sha256": digest(fixture), "root_lock_sha256": digest(source / "Cargo.lock"),
        "consumer_lock_sha256": digest(consumer / "Cargo.lock"), "features": features,
        "default_features": False, "profile": profile, "command": command,
        "host": platform.platform(), "cpu": platform.processor(),
        "rustc": rustc, "target": target,
        "cargo": cargo, "toolchain": toolchain,
        "executable_bytes": executable.stat().st_size, "executable_sha256": digest(executable),
        "diagram_families": families, "corpus": corpus,
        "sequence_exit": rejected.returncode, "sequence_stderr": rejected.stderr.decode().strip(),
    }
    if semantic:
        receipt.update({"recipe": recipe, "dependency": dependency, "raw_artifact": raw_artifact, "strip_command": strip_command, "strip_tool_sha256": digest(objcopy)})
    (output / "receipt.json").write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
    return receipt


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--revision", required=True, help="Committed library source to archive, never the moving worktree.")
    parser.add_argument("--fixture-revision", required=True, help="Committed probe source; use the same revision for every comparison lane.")
    parser.add_argument("--features", required=True, help="Exact facade Cargo features, with defaults disabled.")
    parser.add_argument("--recipe", choices=("svg-selection", "semantic-parser"), default="svg-selection")
    parser.add_argument("--toolchain", help="Rustup toolchain for every Rust command; defaults to this checkout's pinned toolchain.")
    parser.add_argument("--label", required=True)
    parser.add_argument("--output", type=Path, required=True, help="New evidence directory; existing paths are rejected.")
    args = parser.parse_args()
    receipt = measure(
        revision=args.revision, fixture_revision=args.fixture_revision,
        features=[feature.strip() for feature in args.features.split(",") if feature.strip()],
        label=args.label, output=args.output, toolchain=args.toolchain, recipe=args.recipe,
    )
    print(json.dumps({"label": args.label, "bytes": receipt["executable_bytes"], "families": receipt["diagram_families"]}))


if __name__ == "__main__":
    main()
