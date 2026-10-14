#!/usr/bin/env python3
"""Check selected container archive bytes against the attested release receipt."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path


def verify_container_release(assets: Path, target: str, version: str, source_sha: str) -> None:
    manifest = json.loads((assets / "release-verification.json").read_text(encoding="utf-8"))
    if (manifest.get("schema_version"), manifest.get("version"), manifest.get("source_sha")) != (1, version, source_sha):
        raise ValueError("release receipt does not identify the selected version and source")
    name = f"merman-cli-{target}.tar.xz"
    entries = [entry for entry in manifest["assets"] if entry.get("name") == name]
    if len(entries) != 1:
        raise ValueError("release receipt must contain exactly one selected CLI archive")
    entry = entries[0]
    if (entry.get("kind"), entry.get("package"), entry.get("profile"), entry.get("target")) != ("archive", "merman-cli", "cli-release", target):
        raise ValueError("archive is not the selected complete CLI release profile")
    archive = assets / name
    with archive.open("rb") as handle:
        digest = hashlib.file_digest(handle, "sha256").hexdigest()
    if archive.stat().st_size != entry.get("size") or digest != entry.get("sha256"):
        raise ValueError("container archive bytes differ from the attested release receipt")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--assets", type=Path, required=True)
    parser.add_argument("--target", choices=("x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu"), required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--source-sha", required=True)
    args = parser.parse_args()
    verify_container_release(args.assets, args.target, args.version, args.source_sha)
    print(f"verified {args.target} container input for {args.version} at {args.source_sha}")


if __name__ == "__main__":
    main()
