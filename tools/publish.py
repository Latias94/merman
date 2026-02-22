#!/usr/bin/env python3
"""
Publish merman workspace crates to crates.io in dependency order.

This is intentionally boring and explicit: a small helper around `cargo publish` that:
- optionally runs `cargo run -p xtask -- verify` once up-front (parity gate)
- publishes crates in a fixed order
- waits between publishes for crates.io indexing

Usage:
  python tools/publish.py --dry-run
  python tools/publish.py
  python tools/publish.py --crates dugong-graphlib,dugong
  python tools/publish.py --start-from merman-core
"""

from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable, Optional


PUBLISH_ORDER = [
    # Forked dependency used for Mermaid roughjs parity.
    "roughr-merman",
    # Layout stack.
    "dugong-graphlib",
    "dugong",
    "manatee",
    # Mermaid pipeline.
    "merman-core",
    "merman-render",
    "merman",
    "merman-cli",
]


class Colors:
    HEADER = "\033[95m"
    OKBLUE = "\033[94m"
    OKGREEN = "\033[92m"
    WARNING = "\033[93m"
    FAIL = "\033[91m"
    ENDC = "\033[0m"
    BOLD = "\033[1m"


def print_header(msg: str) -> None:
    bar = "=" * 80
    print(f"\n{Colors.HEADER}{Colors.BOLD}{bar}{Colors.ENDC}")
    print(f"{Colors.HEADER}{Colors.BOLD}{msg}{Colors.ENDC}")
    print(f"{Colors.HEADER}{Colors.BOLD}{bar}{Colors.ENDC}\n")


def print_info(msg: str) -> None:
    print(f"{Colors.OKBLUE}INFO: {msg}{Colors.ENDC}")


def print_success(msg: str) -> None:
    print(f"{Colors.OKGREEN}OK: {msg}{Colors.ENDC}")


def print_warning(msg: str) -> None:
    print(f"{Colors.WARNING}WARN: {msg}{Colors.ENDC}")


def print_error(msg: str) -> None:
    print(f"{Colors.FAIL}ERR: {msg}{Colors.ENDC}", file=sys.stderr)


def run_command(
    cmd: list[str],
    *,
    cwd: Optional[Path] = None,
    dry_run: bool = False,
    capture: bool = False,
) -> subprocess.CompletedProcess[str]:
    cmd_str = " ".join(str(c) for c in cmd)
    print_info(f"Running: {cmd_str}")
    if dry_run:
        print_warning("DRY RUN: command not executed")
        return subprocess.CompletedProcess(args=cmd, returncode=0, stdout="", stderr="")

    if capture:
        return subprocess.run(
            cmd,
            cwd=str(cwd) if cwd else None,
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            check=False,
        )
    return subprocess.run(cmd, cwd=str(cwd) if cwd else None, check=False)


def require_tool(name: str) -> None:
    if shutil.which(name) is None:
        raise RuntimeError(f"Required tool not found in PATH: {name}")


def git_is_clean(repo_root: Path) -> bool:
    cp = run_command(["git", "status", "--porcelain"], cwd=repo_root, capture=True)
    if cp.returncode != 0:
        raise RuntimeError("Failed to run git status")
    return cp.stdout.strip() == ""


@dataclass(frozen=True)
class PackageInfo:
    name: str
    version: str
    publish: bool
    manifest_path: Path


def cargo_metadata(repo_root: Path) -> dict:
    cp = run_command(
        ["cargo", "metadata", "--format-version", "1", "--no-deps"],
        cwd=repo_root,
        capture=True,
    )
    if cp.returncode != 0:
        raise RuntimeError("cargo metadata failed")
    return json.loads(cp.stdout)


def get_workspace_packages(repo_root: Path) -> dict[str, PackageInfo]:
    md = cargo_metadata(repo_root)
    out: dict[str, PackageInfo] = {}
    for pkg in md.get("packages", []):
        name = pkg["name"]
        version = pkg["version"]
        publish_raw = pkg.get("publish", None)
        publish = publish_raw is None or publish_raw is True or publish_raw == []
        manifest_path = Path(pkg["manifest_path"])
        out[name] = PackageInfo(
            name=name,
            version=version,
            publish=publish,
            manifest_path=manifest_path,
        )
    return out


def check_crate_published(crate_name: str, version: str) -> bool:
    """
    Best-effort "already published?" check.

    We intentionally use `cargo search` to avoid hardcoding crates.io API calls.
    """
    cp = run_command(["cargo", "search", crate_name, "--limit", "1"], capture=True)
    if cp.returncode != 0:
        return False
    needle = f'{crate_name} = "{version}"'
    return needle in (cp.stdout or "")


def iter_publish_list(
    *,
    requested: Optional[set[str]],
    start_from: Optional[str],
) -> list[str]:
    crates = [c for c in PUBLISH_ORDER if requested is None or c in requested]
    if start_from:
        if start_from not in crates:
            raise RuntimeError(f"--start-from crate not in publish list: {start_from}")
        idx = crates.index(start_from)
        crates = crates[idx:]
    return crates


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Publish merman crates in dependency order",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__,
    )
    parser.add_argument("--dry-run", action="store_true", help="Print actions without publishing")
    parser.add_argument(
        "--crates",
        help="Comma-separated subset of crates to publish (default: all in order)",
    )
    parser.add_argument("--start-from", help="Start publishing from this crate")
    parser.add_argument(
        "--wait",
        type=int,
        default=30,
        help="Seconds to wait between publishes for crates.io indexing (default: 30)",
    )
    parser.add_argument(
        "--no-verify",
        action="store_true",
        help="Pass --no-verify to cargo publish (not recommended)",
    )
    parser.add_argument(
        "--skip-xtask-verify",
        action="store_true",
        help="Skip `cargo run -p xtask -- verify` preflight (not recommended)",
    )
    parser.add_argument(
        "--allow-dirty",
        action="store_true",
        help="Allow publishing with a dirty git working tree (not recommended)",
    )
    parser.add_argument(
        "--no-check-published",
        action="store_true",
        help="Do not check crates.io for already-published versions",
    )

    args = parser.parse_args()

    repo_root = Path(__file__).resolve().parents[1]

    try:
        require_tool("cargo")
        require_tool("git")
    except Exception as e:
        print_error(str(e))
        return 2

    if not args.allow_dirty:
        try:
            if not git_is_clean(repo_root):
                print_error("Git working tree is not clean. Commit/stash changes or pass --allow-dirty.")
                return 2
        except Exception as e:
            print_error(str(e))
            return 2

    requested = None
    if args.crates:
        requested = {c.strip() for c in args.crates.split(",") if c.strip()}
        unknown = requested - set(PUBLISH_ORDER)
        if unknown:
            print_error(f"Unknown crates: {', '.join(sorted(unknown))}")
            print_info(f"Known crates: {', '.join(PUBLISH_ORDER)}")
            return 2

    try:
        crates = iter_publish_list(requested=requested, start_from=args.start_from)
    except Exception as e:
        print_error(str(e))
        return 2

    packages = get_workspace_packages(repo_root)
    missing = [c for c in crates if c not in packages]
    if missing:
        print_error(f"Crates not found in workspace: {', '.join(missing)}")
        return 2

    not_publishable = [c for c in crates if not packages[c].publish]
    if not_publishable:
        print_error(f"Crates are marked publish=false and cannot be published: {', '.join(not_publishable)}")
        return 2

    print_header("Publish Plan")
    print_info(f"Repo: {repo_root}")
    print_info(f"Dry run: {args.dry_run}")
    print_info(f"Wait time: {args.wait}s")
    print_info(f"Preflight xtask verify: {not args.skip_xtask_verify}")
    print_info(f"cargo publish --no-verify: {args.no_verify}")
    print()
    for i, c in enumerate(crates, 1):
        p = packages[c]
        print(f"  {i}. {p.name} v{p.version} ({p.manifest_path.parent.relative_to(repo_root)})")
    print()

    if not args.skip_xtask_verify:
        cp = run_command(["cargo", "run", "-p", "xtask", "--", "verify"], cwd=repo_root, dry_run=args.dry_run)
        if cp.returncode != 0:
            print_error("xtask verify failed; aborting publish.")
            return 1

    if not args.dry_run:
        resp = input("Continue with publishing? [y/N]: ").strip().lower()
        if resp not in ("y", "yes"):
            print_info("Cancelled.")
            return 0

    failures: list[str] = []
    for c in crates:
        p = packages[c]
        print_header(f"Publishing {p.name} v{p.version}")

        if not args.no_check_published and not args.dry_run:
            if check_crate_published(p.name, p.version):
                print_warning(f"{p.name} v{p.version} appears already published.")
                resp = input("Skip this crate? [Y/n]: ").strip().lower()
                if resp in ("", "y", "yes"):
                    print_info(f"Skipping {p.name}")
                    continue

        cmd = ["cargo", "publish", "-p", p.name]
        if args.no_verify:
            cmd.append("--no-verify")
        cp = run_command(cmd, cwd=repo_root, dry_run=args.dry_run)
        if cp.returncode != 0:
            print_error(f"Failed to publish {p.name}")
            failures.append(p.name)
            if not args.dry_run:
                resp = input("Continue with remaining crates? [y/N]: ").strip().lower()
                if resp not in ("y", "yes"):
                    break
        else:
            print_success(f"Published {p.name} v{p.version}")
            if not args.dry_run and args.wait > 0:
                print_info(f"Waiting {args.wait}s for crates.io indexing...")
                time.sleep(args.wait)

    print_header("Publish Result")
    if failures:
        print_error(f"Failed crates: {', '.join(failures)}")
        return 1
    print_success(f"Published {len(crates)} crate(s).")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

