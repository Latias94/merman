#!/usr/bin/env python3
"""Run Dart's publish dry-run and enforce the Flutter package size budget."""

from __future__ import annotations

import argparse
import math
from pathlib import Path
import re
import subprocess
import sys


REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_PACKAGE_ROOT = REPO_ROOT / "platforms" / "flutter"
DEFAULT_MAX_COMPRESSED_MB = 99.0
ANSI_ESCAPE = re.compile(r"\x1b\[[0-?]*[ -/]*[@-~]")
TOTAL_SIZE = re.compile(
    r"Total compressed archive size:\s*([0-9]+(?:\.[0-9]+)?)\s*([KMGT]?B)",
    re.IGNORECASE,
)
PRECISE_HINT = re.compile(
    r"Your package is\s*([0-9]+(?:\.[0-9]+)?)\s*MB",
    re.IGNORECASE,
)
UNIT_TO_MB = {
    "B": 0.000001,
    "KB": 0.001,
    "MB": 1.0,
    "GB": 1000.0,
    "TB": 1000000.0,
}


class FlutterPackageSizeError(RuntimeError):
    """The Dart dry-run output did not satisfy the package-size contract."""


def compressed_archive_megabytes(output: str) -> float:
    """Read the most precise compressed archive size reported by Dart pub."""
    plain = ANSI_ESCAPE.sub("", output)
    sizes = [
        float(value) * UNIT_TO_MB[unit.upper()]
        for value, unit in TOTAL_SIZE.findall(plain)
    ]
    sizes.extend(float(value) for value in PRECISE_HINT.findall(plain))
    if not sizes:
        raise FlutterPackageSizeError(
            "dart pub publish --dry-run did not report a compressed archive size"
        )
    return max(sizes)


def positive_float(value: str) -> float:
    parsed = float(value)
    if not math.isfinite(parsed) or parsed <= 0:
        raise argparse.ArgumentTypeError("value must be positive")
    return parsed


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--package-root",
        type=Path,
        default=DEFAULT_PACKAGE_ROOT,
        help="Flutter package directory passed to Dart pub.",
    )
    parser.add_argument(
        "--max-compressed-mb",
        type=positive_float,
        default=DEFAULT_MAX_COMPRESSED_MB,
        help="Maximum decimal megabytes accepted from Dart's compressed archive report.",
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    package_root = args.package_root.resolve()
    if not (package_root / "pubspec.yaml").is_file():
        print(f"Flutter package root has no pubspec.yaml: {package_root}", file=sys.stderr)
        return 2

    completed = subprocess.run(
        ["dart", "pub", "publish", "--dry-run"],
        cwd=package_root,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
    )
    if completed.stdout:
        print(completed.stdout, end="" if completed.stdout.endswith("\n") else "\n")
    if completed.returncode != 0:
        return completed.returncode

    try:
        size_mb = compressed_archive_megabytes(completed.stdout)
    except FlutterPackageSizeError as error:
        print(f"Flutter package size verification failed: {error}", file=sys.stderr)
        return 1

    if size_mb > args.max_compressed_mb:
        print(
            "Flutter package size verification failed: "
            f"compressed archive is {size_mb:.1f} MB; "
            f"maximum is {args.max_compressed_mb:.1f} MB",
            file=sys.stderr,
        )
        return 1

    print(
        f"Flutter package compressed archive verified: {size_mb:.1f} MB "
        f"<= {args.max_compressed_mb:.1f} MB"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
