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
BOUNDARY_FIXTURE = Path("tools/bench/fixtures/model-source-boundaries")
BOUNDARY_RECIPES = {
    "boundary-model": "model",
    "boundary-model-ascii": "model-ascii",
    "boundary-source-ascii": "source-ascii",
    "boundary-presentation-svg": "presentation-svg",
    "boundary-analysis-diagnostics": "analysis-diagnostics",
    "boundary-analysis-fixes": "analysis-fixes",
}
BOUNDARY_FILES = ("Cargo.toml", "main.rs", "alpha.json", "beta.json")


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(command: list[str], *, cwd: Path, env: dict[str, str]) -> bytes:
    return subprocess.run(command, cwd=cwd, env=env, check=True, capture_output=True).stdout


def measure(
    *, revision: str, fixture_revision: str, features: list[str] | None = None, label: str,
    output: Path, toolchain: str | None = None, recipe: str = "svg-selection",
    target_dir: Path | None = None, expect_receipt: Path | None = None,
) -> dict[str, object]:
    """Measure committed library sources with an independently pinned consumer."""
    boundary = recipe in BOUNDARY_RECIPES
    if not boundary and recipe not in {"svg-selection", "semantic-parser"}:
        raise ValueError(f"unknown recipe: {recipe}")
    if boundary:
        selected = BOUNDARY_RECIPES[recipe]
        if features and sorted(set(features)) != [selected]:
            raise ValueError(f"{recipe} has one fixed consumer feature: {selected}")
        features = [selected]
    elif features is None:
        raise ValueError("--features is required for the existing diagram-selection recipes")
    if expect_receipt is not None and not boundary:
        raise ValueError("--expect-receipt is supported only by the fixed boundary recipes")
    expected = json.loads(expect_receipt.read_text(encoding="utf-8")) if expect_receipt else None
    semantic = recipe == "semantic-parser"
    retain_raw = semantic or boundary
    fixture_relative = BOUNDARY_FIXTURE / "main.rs" if boundary else SEMANTIC_FIXTURE if semantic else FIXTURE_RELATIVE
    corpus_paths = tuple((BOUNDARY_FIXTURE / name).as_posix() for name in ("alpha.json", "beta.json")) if boundary else SEMANTIC_CORPUS if semantic else CORPUS
    dependency = "merman-core" if semantic else "merman"
    profile = {**PROFILE, "strip": "none"} if retain_raw else PROFILE
    output = output.resolve()
    env = dict(os.environ)
    # Freeze all Cargo release knobs instead of accepting ambient profile overrides.
    for key in list(env):
        if key.startswith("CARGO_PROFILE_RELEASE_") or key in {"RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS"}:
            del env[key]
    build_target = target_dir.resolve() if target_dir is not None else output / "cargo-target"
    env["CARGO_TARGET_DIR"] = str(build_target)
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
    boundary_files = {
        name: run(["git", "show", f"{fixture_revision}:{(BOUNDARY_FIXTURE / name).as_posix()}"], cwd=ROOT, env=env)
        for name in BOUNDARY_FILES if name != "main.rs"
    } if boundary else {}
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
    if boundary:
        # This checked-in manifest is the six fixed recipes, not a dependency resolver.
        manifest = boundary_files["Cargo.toml"].decode("utf-8")
        for package in ("merman-model", "merman-core", "merman-ascii", "merman-render", "merman-analysis"):
            manifest = manifest.replace(
                json.dumps(f"../../../../crates/{package}"),
                json.dumps((source / "crates" / package).as_posix()),
            )
        manifest += '\n[profile.release]\n' + ''.join(f'{key} = {json.dumps(value)}\n' for key, value in profile.items())
        for name in ("alpha.json", "beta.json"):
            (consumer / name).write_bytes(boundary_files[name])
    (consumer / "Cargo.toml").write_text(manifest, encoding="utf-8")
    shutil.copyfile(source / "Cargo.lock", consumer / "Cargo.lock")
    rustc = run([*rust, "rustc", "-vV"], cwd=consumer, env=env).decode()
    cargo = run([*rust, "cargo", "-V"], cwd=consumer, env=env).decode().strip()
    target = next(line.removeprefix("host: ") for line in rustc.splitlines() if line.startswith("host: "))
    # Cargo retains locked versions while adding only this independent consumer package.
    selection = ["--no-default-features", "--features", features[0]] if boundary else []
    metadata = run([*rust, "cargo", "metadata", "--offline", "--format-version", "1", *selection], cwd=consumer, env=env)
    (output / "metadata.json").write_bytes(metadata)
    command = [*rust, "cargo", "build", "--locked", "--offline", "--release", "--target", target, "-j", "1", *selection]
    with (output / "build.log").open("wb") as log:
        subprocess.run(command, cwd=consumer, env=env, check=True, stdout=log, stderr=subprocess.STDOUT)
    suffix = ".exe" if os.name == "nt" else ""
    executable = output / ("diagram-selection-probe" + suffix)
    shutil.copyfile(build_target / target / "release" / executable.name, executable)
    if os.name != "nt":
        executable.chmod(0o755)
    raw_artifact = None
    strip_command = None
    if retain_raw:
        raw = output / ("diagram-selection-probe.raw" + suffix)
        shutil.copy2(executable, raw)
        raw_artifact = {"bytes": raw.stat().st_size, "sha256": digest(raw)}
        sysroot = Path(run([*rust, "rustc", "--print", "sysroot"], cwd=consumer, env=env).decode().strip())
        objcopy = sysroot / "lib" / "rustlib" / target / "bin" / ("llvm-objcopy" + suffix)
        strip_command = [str(objcopy), "--strip-all", str(executable)]
        run(strip_command, cwd=consumer, env=env)
    families = run([str(executable), "--families"], cwd=source, env=env).decode().splitlines()
    tree = run([*rust, "cargo", "tree", "--locked", "--offline", "--edges", "normal,build" if boundary else "normal", "--prefix", "none", "--format", "{p}|{f}", *selection], cwd=consumer, env=env)
    (output / "cargo-tree.txt").write_bytes(tree)
    corpus = []
    for relative in corpus_paths:
        input_path = consumer / Path(relative).name if boundary else source / relative
        svg = run([str(executable), str(input_path)], cwd=source, env=env)
        if retain_raw and svg != run([str(raw), str(input_path)], cwd=source, env=env):
            raise RuntimeError(f"stripping changed semantic output for {relative}")
        if boundary:
            if not svg or svg != run([str(executable), str(input_path)], cwd=source, env=env):
                raise RuntimeError(f"empty or nondeterministic consumed output for {relative}")
            if recipe == "boundary-presentation-svg":
                if b"<svg" not in svg:
                    raise RuntimeError(f"missing presentation SVG for {relative}")
            else:
                observed = json.loads(svg)
                runtime = json.loads(input_path.read_bytes())
                if observed.get("identity") != runtime["identity"] or not observed.get("observations"):
                    raise RuntimeError(f"missing runtime consumed observations for {relative}")
        if not semantic and not boundary and b"<svg" not in svg:
            raise RuntimeError(f"missing SVG output for {relative}")
        if semantic and not svg.startswith(input_path.stem.split("_", 1)[0].encode() + b"\n"):
            raise RuntimeError(f"missing semantic output for {relative}")
        output_kind = "observations" if boundary and recipe != "boundary-presentation-svg" else "semantic" if semantic else "svg"
        extension = ".observations.json" if output_kind == "observations" else ".txt" if semantic else ".svg"
        (output / (input_path.stem + extension)).write_bytes(svg)
        corpus.append({"path": relative, "input_sha256": digest(input_path), f"{output_kind}_bytes": len(svg), f"{output_kind}_sha256": hashlib.sha256(svg).hexdigest()})
    if not boundary:
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
        "corpus": corpus,
    }
    if boundary:
        receipt.update({
            "recipe": recipe, "consumer_families": families,
            "fixture_files": {name: hashlib.sha256(data).hexdigest() for name, data in {"main.rs": fixture_bytes, **boundary_files}.items()},
            "manifest_sha256": digest(consumer / "Cargo.toml"),
            "dependency_edges": "normal,build", "measurement": "artifact-and-closure; no timing claim",
        })
    else:
        receipt.update({"diagram_families": families, "sequence_exit": rejected.returncode, "sequence_stderr": rejected.stderr.decode().strip()})
    if retain_raw:
        receipt.update({"recipe": recipe, "dependency": dependency, "raw_artifact": raw_artifact, "strip_command": strip_command, "strip_tool_sha256": digest(objcopy)})
        if boundary:
            del receipt["dependency"]  # The retained manifest describes the multi-owner consumer.
    if boundary or target_dir is not None:
        receipt.update({"target_dir": str(build_target), "target_dir_policy": "explicit-reuse" if target_dir is not None else "isolated-lane"})
    differences = []
    if expected is not None:
        differences = [key for key in ("recipe", "features", "profile", "target", "toolchain", "default_features", "consumer_families", "corpus") if receipt[key] != expected.get(key)]
        receipt["compared_receipt"] = {"path": str(expect_receipt.resolve()), "sha256": digest(expect_receipt), "matches": not differences, "differences": differences}
    (output / "receipt.json").write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
    if differences:
        raise RuntimeError(f"frozen base/head contract differs: {', '.join(differences)}")
    return receipt


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--revision", required=True, help="Committed library source to archive, never the moving worktree.")
    parser.add_argument("--fixture-revision", required=True, help="Committed probe source; use the same revision for every comparison lane.")
    parser.add_argument("--features", help="Exact existing recipe Cargo features; boundary recipes select their fixed feature automatically.")
    parser.add_argument("--recipe", choices=("svg-selection", "semantic-parser", *BOUNDARY_RECIPES), default="svg-selection")
    parser.add_argument("--toolchain", help="Rustup toolchain for every Rust command; defaults to this checkout's pinned toolchain.")
    parser.add_argument("--target-dir", type=Path, help="Explicit reusable Cargo target directory; defaults to a new isolated cache per lane. Never cleaned by this runner.")
    parser.add_argument("--expect-receipt", type=Path, help="Boundary recipes only: require identical base inputs and consumed outputs, profile, toolchain and target.")
    parser.add_argument("--label", required=True)
    parser.add_argument("--output", type=Path, required=True, help="New evidence directory; existing paths are rejected.")
    args = parser.parse_args()
    receipt = measure(
        revision=args.revision, fixture_revision=args.fixture_revision,
        features=[feature.strip() for feature in args.features.split(",") if feature.strip()] if args.features is not None else None,
        label=args.label, output=args.output, toolchain=args.toolchain, recipe=args.recipe,
        target_dir=args.target_dir, expect_receipt=args.expect_receipt,
    )
    print(json.dumps({"label": args.label, "bytes": receipt["executable_bytes"], "families": receipt.get("consumer_families", receipt.get("diagram_families"))}))


if __name__ == "__main__":
    main()
