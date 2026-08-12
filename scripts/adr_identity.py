#!/usr/bin/env python3
"""Verify that ADR filenames and titles carry one matching unique identity."""

from __future__ import annotations

import re
import sys
from collections import defaultdict
from pathlib import Path


ADR_FILENAME = re.compile(r"^(?P<id>\d{4})-.+\.md$")
ADR_TITLE = re.compile(r"^#\s+(?:ADR[- ])?(?P<id>\d{4}):")
REPOSITORY_ROOT = Path(__file__).resolve().parents[1]


def identity_failures(adr_root: Path) -> list[str]:
    failures: list[str] = []
    paths_by_id: dict[str, list[Path]] = defaultdict(list)

    for path in sorted(adr_root.glob("*.md")):
        filename_match = ADR_FILENAME.fullmatch(path.name)
        if filename_match is None:
            failures.append(f"{path.name}: filename must start with a four-digit ADR id")
            continue

        filename_id = filename_match.group("id")
        paths_by_id[filename_id].append(path)

        try:
            with path.open(encoding="utf-8") as source:
                title = source.readline().rstrip("\r\n")
        except (OSError, UnicodeError) as error:
            failures.append(f"{path.name}: failed to read ADR title: {error}")
            continue

        title_match = ADR_TITLE.match(title)
        if title_match is None:
            failures.append(f"{path.name}: first line must contain a structured ADR id")
            continue
        title_id = title_match.group("id")
        if title_id != filename_id:
            failures.append(
                f"{path.name}: title ADR id {title_id} does not match filename id {filename_id}"
            )

    for adr_id, paths in sorted(paths_by_id.items()):
        if len(paths) > 1:
            names = ", ".join(path.name for path in paths)
            failures.append(f"ADR id {adr_id} is used by multiple files: {names}")

    return failures


def main() -> int:
    adr_root = REPOSITORY_ROOT / "docs" / "adr"
    failures = identity_failures(adr_root)
    if failures:
        print("ADR identity verification failed:", file=sys.stderr)
        for failure in failures:
            print(f"- {failure}", file=sys.stderr)
        return 1

    print("ADR identities verified")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
