#!/usr/bin/env python3
"""Check GNU CLI/LSP version needs against the Ubuntu 22.04 glibc baseline."""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import re
import subprocess
import sys


GLIBC_CEILING = (2, 35)


def required_glibc_version(version_info: str) -> tuple[int, ...]:
    versions: list[tuple[int, ...]] = []
    in_needs = False
    for line in version_info.splitlines():
        if line.startswith("Version "):
            in_needs = line.startswith("Version needs section ")
        if in_needs:
            for match in re.finditer(r"\bName:\s+GLIBC_(\d+(?:\.\d+)+)\b", line):
                versions.append(tuple(int(part) for part in match[1].split(".")))
    if not versions:
        raise RuntimeError("readelf reported no numeric GLIBC version needs")
    return max(versions)


def verify_binary(binary: Path) -> tuple[int, ...]:
    try:
        result = subprocess.run(
            ["readelf", "--version-info", "--wide", str(binary)],
            check=True,
            capture_output=True,
            text=True,
            env={**os.environ, "LC_ALL": "C"},
        )
    except (OSError, subprocess.CalledProcessError) as error:
        raise RuntimeError(f"cannot inspect {binary} with readelf: {error}") from error
    required = required_glibc_version(result.stdout)
    if required > GLIBC_CEILING:
        version = ".".join(map(str, required))
        raise RuntimeError(f"{binary} requires GLIBC_{version}, above GLIBC_2.35")
    return required


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("cli_binary", type=Path)
    parser.add_argument("lsp_binary", type=Path)
    args = parser.parse_args(argv)
    for binary in (args.cli_binary, args.lsp_binary):
        required = verify_binary(binary)
        print(f"{binary}: GLIBC_{'.'.join(map(str, required))} <= GLIBC_2.35")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except RuntimeError as error:
        print(f"linux_binary_compatibility.py: {error}", file=sys.stderr)
        sys.exit(1)
