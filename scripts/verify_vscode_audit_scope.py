#!/usr/bin/env python3
"""Require review when the unpublished VS Code audit exception's versions change."""

from __future__ import annotations

import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def verify_scope(lock: dict) -> None:
    packages = lock.get("packages", {})
    for path, version in {
        "node_modules/vscode-languageclient": "11.0.0",
        "node_modules/minimatch": "10.2.6",
    }.items():
        if packages.get(path, {}).get("version") != version:
            raise ValueError(f"review or remove the VS Code audit exception: expected {path}@{version}")
    if "node_modules/vscode-languageclient/node_modules/minimatch" in packages:
        raise ValueError("review or remove the VS Code audit exception: minimatch is no longer shared")


if __name__ == "__main__":
    lockfile = ROOT / "tools/vscode-extension/package-lock.json"
    verify_scope(json.loads(lockfile.read_text(encoding="utf-8")))
    print("VS Code audit exception dependency versions: ok")
