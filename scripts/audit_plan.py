#!/usr/bin/env python3
"""Discover tracked Cargo and npm lockfiles for dependency audit jobs."""

from __future__ import annotations

import argparse
from collections.abc import Iterable, Mapping, Sequence
import json
from pathlib import Path, PurePosixPath
import subprocess
import sys
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]

# Keep exclusions exact, rare, and documented. An empty set means every tracked lock is audited.
LOCKFILE_EXCLUSIONS: dict[str, str] = {}


class AuditPlanError(RuntimeError):
    """The tracked lockfile set cannot produce a complete audit plan."""


def build_audit_plan(
    tracked_paths: Iterable[str],
    *,
    exclusions: Mapping[str, str] = LOCKFILE_EXCLUSIONS,
) -> dict[str, dict[str, list[dict[str, str]]]]:
    """Build the exact GitHub Actions matrices from tracked repository paths."""
    tracked = {_normalize_repo_path(path) for path in tracked_paths}
    normalized_exclusions = {
        _normalize_repo_path(path): reason.strip()
        for path, reason in exclusions.items()
    }
    for path, reason in normalized_exclusions.items():
        if not reason:
            raise AuditPlanError(f"lockfile exclusion {path!r} requires a non-empty reason")
        if PurePosixPath(path).name not in {"Cargo.lock", "package-lock.json"}:
            raise AuditPlanError(f"lockfile exclusion {path!r} is not an audit lockfile")
        if path not in tracked:
            raise AuditPlanError(f"lockfile exclusion {path!r} is stale or untracked")

    cargo_rows: list[dict[str, str]] = []
    npm_rows: list[dict[str, str]] = []
    for lockfile in sorted(tracked):
        name = PurePosixPath(lockfile).name
        if name not in {"Cargo.lock", "package-lock.json"}:
            continue
        if lockfile in normalized_exclusions:
            continue

        owner = PurePosixPath(lockfile).parent
        if name == "Cargo.lock":
            manifest = _owner_path(owner, "Cargo.toml")
            if manifest not in tracked:
                raise AuditPlanError(
                    f"tracked Cargo lock {lockfile!r} has no tracked owner manifest {manifest!r}"
                )
            cargo_rows.append({"lockfile": lockfile})
            continue

        manifest = _owner_path(owner, "package.json")
        if manifest not in tracked:
            raise AuditPlanError(
                f"tracked npm lock {lockfile!r} has no tracked owner manifest {manifest!r}"
            )
        npm_rows.append(
            {
                "directory": "." if owner == PurePosixPath(".") else owner.as_posix(),
                "lockfile": lockfile,
            }
        )

    return {
        "cargo": {"include": cargo_rows},
        "npm": {"include": npm_rows},
    }


def discover_audit_plan(repo_root: Path = REPO_ROOT) -> dict[str, dict[str, list[dict[str, str]]]]:
    """Read the version-controlled file set from Git and build both matrices."""
    root = repo_root.resolve()
    try:
        completed = subprocess.run(
            ["git", "-C", str(root), "ls-files", "-z"],
            check=False,
            capture_output=True,
        )
    except OSError as error:
        raise AuditPlanError(f"could not run git ls-files: {error}") from error
    if completed.returncode != 0:
        detail = completed.stderr.decode("utf-8", errors="replace").strip()
        suffix = f": {detail}" if detail else ""
        raise AuditPlanError(f"git ls-files failed with status {completed.returncode}{suffix}")
    paths = [
        raw.decode("utf-8", errors="surrogateescape")
        for raw in completed.stdout.split(b"\0")
        if raw
    ]
    return build_audit_plan(paths)


def write_github_output(
    output_path: Path,
    plan: Mapping[str, Any],
) -> None:
    """Append compact matrices to a GitHub Actions step output file."""
    with output_path.open("a", encoding="utf-8", newline="\n") as output:
        for key in ("cargo", "npm"):
            output.write(
                f"{key}={json.dumps(plan[key], sort_keys=True, separators=(',', ':'))}\n"
            )


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--repo-root",
        type=Path,
        default=REPO_ROOT,
        help="repository root used for tracked-file discovery",
    )
    parser.add_argument(
        "--github-output",
        type=Path,
        help="append cargo and npm matrices to this GitHub Actions output file",
    )
    args = parser.parse_args(argv)
    try:
        plan = discover_audit_plan(args.repo_root)
        if args.github_output is not None:
            write_github_output(args.github_output, plan)
        else:
            print(json.dumps(plan, indent=2, sort_keys=True))
    except (AuditPlanError, OSError, KeyError) as error:
        print(f"audit plan failed: {error}", file=sys.stderr)
        return 1
    return 0


def _normalize_repo_path(raw_path: str) -> str:
    if not isinstance(raw_path, str) or not raw_path:
        raise AuditPlanError("tracked repository paths must be non-empty strings")
    normalized = raw_path.replace("\\", "/")
    path = PurePosixPath(normalized)
    if path.is_absolute() or ".." in path.parts or normalized.startswith("./"):
        raise AuditPlanError(f"tracked repository path must be normalized and relative: {raw_path!r}")
    return path.as_posix()


def _owner_path(owner: PurePosixPath, manifest: str) -> str:
    return manifest if owner == PurePosixPath(".") else (owner / manifest).as_posix()


if __name__ == "__main__":
    raise SystemExit(main())
