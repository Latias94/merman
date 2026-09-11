#!/usr/bin/env python3
"""Record or recheck scoped preset qualification from a clean candidate build.

The Rust runner owns qualification. This script binds its output to the source commit and actual
executable, and reexecutes it for freshness checks. Reports never populate public qualified cells.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import sys

try:
    from .run_theme_acceptance import acceptance_environment
except ImportError:
    from run_theme_acceptance import acceptance_environment

ROOT = Path(__file__).resolve().parents[1]
BUILD = [
    "cargo", "build", "--release", "--locked", "-p", "merman-theme-acceptance",
    "--no-default-features", "--features", "png", "--example", "preset_qualification",
    "--jobs", "2", "--message-format=json-render-diagnostics",
]


def clean_revision(root: Path) -> str:
    status = subprocess.check_output(
        ["git", "status", "--porcelain", "--untracked-files=all"], cwd=root, text=True,
    )
    if status.strip():
        raise RuntimeError("Qualification requires a clean checkout, including untracked files")
    return subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip()


def sha256_file(path: Path) -> str:
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def collect(root: Path) -> dict:
    revision = clean_revision(root)
    lock_digest = sha256_file(root / "Cargo.lock")
    env = acceptance_environment(dict(os.environ))
    build = subprocess.run(BUILD, cwd=root, env=env, check=True, capture_output=True, text=True)
    # Cargo sends diagnostics to stderr and artifact identities as JSON lines to stdout.
    sys.stderr.write(build.stderr)
    artifacts = [json.loads(line) for line in build.stdout.splitlines() if line.strip()]
    executables = {
        item["executable"] for item in artifacts
        if item.get("reason") == "compiler-artifact"
        and item.get("target", {}).get("name") == "preset_qualification"
        and "example" in item.get("target", {}).get("kind", [])
        and item.get("executable")
    }
    if len(executables) != 1:
        raise RuntimeError("Cargo did not identify exactly one preset qualification executable")
    executable = Path(executables.pop())
    executable_digest = sha256_file(executable)
    execution = subprocess.run(
        [str(executable)], cwd=root, env=env, check=True, capture_output=True, text=True,
    )
    result = json.loads(execution.stdout)
    if (clean_revision(root) != revision or sha256_file(root / "Cargo.lock") != lock_digest
            or sha256_file(executable) != executable_digest):
        raise RuntimeError("Candidate source, lockfile, or executable changed during qualification")
    return {
        "schema_version": 1,
        "source_commit": revision,
        "lockfile_sha256": lock_digest,
        "executable_sha256": executable_digest,
        "build_command": BUILD,
        "rustc": subprocess.check_output(["rustc", "-Vv"], cwd=root, text=True).strip(),
        "host": {"system": platform.system(), "release": platform.release(), "machine": platform.machine()},
        "qualification": result,
    }


def verify_record(expected: dict, actual: dict) -> None:
    if expected != actual:
        raise RuntimeError("Stale qualification: candidate build, host profile, or execution evidence differs")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--output", type=Path, help="write a new execution record, normally under target/")
    mode.add_argument("--check", type=Path, help="rebuild and reexecute; reject any record mismatch")
    args = parser.parse_args()
    try:
        expected = json.loads(args.check.read_text(encoding="utf-8")) if args.check else None
        record = collect(ROOT)
        if args.check:
            verify_record(expected, record)
            print("Scoped preset qualification matches the clean candidate build and execution")
        else:
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(json.dumps(record, indent=2, sort_keys=True) + "\n", encoding="utf-8")
            print(f"Recorded scoped preset qualification: {args.output}")
        return 0
    except (OSError, RuntimeError, ValueError, subprocess.CalledProcessError) as error:
        print(f"Preset qualification failed: {error}", file=sys.stderr)
        if isinstance(error, subprocess.CalledProcessError) and error.stderr:
            print(error.stderr, file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
