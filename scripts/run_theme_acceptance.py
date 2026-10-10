#!/usr/bin/env python3
"""Run Cargo with the workspace-only theme acceptance configuration."""

from __future__ import annotations

import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
ACCEPTANCE_CFG = "merman_internal_theme_acceptance"


def acceptance_environment(environ: dict[str, str]) -> dict[str, str]:
    env = environ.copy()
    # Match Cargo's precedence and whitespace splitting for ordinary RUSTFLAGS.
    if "CARGO_ENCODED_RUSTFLAGS" in env:
        flags = env["CARGO_ENCODED_RUSTFLAGS"].split("\x1f")
    else:
        flags = env.get("RUSTFLAGS", "").split()
    flags = [flag for flag in flags if flag]
    flags.extend(["--cfg", ACCEPTANCE_CFG])
    env["CARGO_ENCODED_RUSTFLAGS"] = "\x1f".join(flags)
    env["RUSTDOCFLAGS"] = f"{env.get('RUSTDOCFLAGS', '')} --cfg {ACCEPTANCE_CFG}".strip()
    return env


def main() -> int:
    args = sys.argv[1:]
    if not args or args[0] in {"-h", "--help"}:
        print("Usage: python3 scripts/run_theme_acceptance.py <cargo arguments>")
        return 0 if args else 2
    return subprocess.run(
        ["cargo", *args], cwd=ROOT, env=acceptance_environment(dict(os.environ)), check=False
    ).returncode


if __name__ == "__main__":
    raise SystemExit(main())
