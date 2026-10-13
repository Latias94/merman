#!/usr/bin/env python3
"""Keep indexed repository paths within the Windows checkout budget."""

from __future__ import annotations

import subprocess
from collections.abc import Iterable
from pathlib import Path


MAX_RELATIVE_PATH_UNITS = 160


def oversized_paths(paths: Iterable[str]) -> list[tuple[str, int]]:
    return [
        (path, units)
        for path in paths
        if (units := len(path.encode("utf-16-le")) // 2) > MAX_RELATIVE_PATH_UNITS
    ]


def main() -> int:
    root = Path(__file__).resolve().parents[1]
    result = subprocess.run(
        ["git", "ls-files", "-z"], cwd=root, capture_output=True, check=True
    )
    paths = result.stdout.decode("utf-8").split("\0")
    failures = oversized_paths(path for path in paths if path)
    for path, units in failures:
        print(f"{units} UTF-16 units exceeds {MAX_RELATIVE_PATH_UNITS}: {path}")
    if failures:
        return 1
    print(f"Indexed repository paths fit the {MAX_RELATIVE_PATH_UNITS}-unit budget.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
