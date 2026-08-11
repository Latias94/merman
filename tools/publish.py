#!/usr/bin/env python3
"""
Publish merman workspace crates to crates.io in dependency order.

This is intentionally boring and explicit: a small helper around `cargo publish` that:
- optionally runs `cargo run -p xtask -- verify` once up-front (parity gate)
- optionally runs `cargo publish --dry-run` per crate before uploading
- derives dependency-safe publish batches from Cargo metadata
- waits between publishes for crates.io indexing

Usage:
  python tools/publish.py --dry-run
  python tools/publish.py
  python tools/publish.py --crates dugong-graphlib,dugong
  python tools/publish.py --start-from merman-core
  python tools/publish.py --tag v0.1.0
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
    quiet: bool = False,
) -> subprocess.CompletedProcess[str]:
    cmd_str = " ".join(str(c) for c in cmd)
    if not quiet:
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
    manifest_path: Path
    internal_deps: tuple[str, ...]


class PublishGraphError(RuntimeError):
    """Cargo workspace metadata cannot produce a safe crates.io publish graph."""


def cargo_metadata(repo_root: Path, *, quiet: bool = False) -> dict:
    cp = run_command(
        ["cargo", "metadata", "--locked", "--format-version", "1", "--no-deps"],
        cwd=repo_root,
        capture=True,
        quiet=quiet,
    )
    if cp.returncode != 0:
        raise RuntimeError("cargo metadata failed")
    return json.loads(cp.stdout)


def publish_field_allows_crates_io(publish_raw: object) -> bool:
    if publish_raw is None or publish_raw is True:
        return True
    if isinstance(publish_raw, list):
        return "crates-io" in publish_raw
    return False


def crates_io_publish_batches(metadata: dict) -> tuple[tuple[str, ...], ...]:
    """Return deterministic topological batches for publishable workspace packages."""
    workspace = _workspace_packages_by_name(metadata)
    publishable = {
        name: package
        for name, package in workspace.items()
        if publish_field_allows_crates_io(package.get("publish"))
    }
    if not publishable:
        raise PublishGraphError("Cargo workspace has no crates.io-publishable packages")

    manifest_owners = {
        Path(str(package["manifest_path"])).resolve().parent: name
        for name, package in workspace.items()
    }
    dependencies: dict[str, set[str]] = {name: set() for name in publishable}
    for name, package in publishable.items():
        raw_dependencies = package.get("dependencies", [])
        if not isinstance(raw_dependencies, list):
            raise PublishGraphError(f"workspace package {name} has invalid dependencies metadata")
        for dependency in raw_dependencies:
            if not isinstance(dependency, dict):
                raise PublishGraphError(
                    f"workspace package {name} has a non-object dependency entry"
                )
            if dependency.get("kind") == "dev":
                continue
            dependency_path = dependency.get("path")
            if not isinstance(dependency_path, str):
                continue
            dependency_name = manifest_owners.get(Path(dependency_path).resolve())
            if dependency_name is None or dependency_name == name:
                continue
            if dependency_name not in publishable:
                raise PublishGraphError(
                    f"publishable workspace package {name} depends on non-publishable "
                    f"workspace package {dependency_name}"
                )
            dependencies[name].add(dependency_name)

    batches: list[tuple[str, ...]] = []
    remaining = {name: set(deps) for name, deps in dependencies.items()}
    while remaining:
        ready = tuple(sorted(name for name, deps in remaining.items() if not deps))
        if not ready:
            cycle = ", ".join(sorted(remaining))
            raise PublishGraphError(
                f"crates.io workspace dependency cycle prevents publication: {cycle}"
            )
        batches.append(ready)
        ready_set = set(ready)
        remaining = {
            name: deps - ready_set
            for name, deps in remaining.items()
            if name not in ready_set
        }
    return tuple(batches)


def crates_io_publish_order(metadata: dict) -> tuple[str, ...]:
    return tuple(
        package
        for batch in crates_io_publish_batches(metadata)
        for package in batch
    )


def _workspace_packages_by_name(metadata: dict) -> dict[str, dict]:
    packages = metadata.get("packages")
    workspace_members = metadata.get("workspace_members")
    if not isinstance(packages, list) or not isinstance(workspace_members, list):
        raise PublishGraphError(
            "cargo metadata must contain packages and workspace_members arrays"
        )
    member_ids = set(workspace_members)
    workspace: dict[str, dict] = {}
    for package in packages:
        if not isinstance(package, dict) or package.get("id") not in member_ids:
            continue
        name = package.get("name")
        manifest_path = package.get("manifest_path")
        if not isinstance(name, str) or not isinstance(manifest_path, str):
            raise PublishGraphError("workspace package metadata is missing name/manifest_path")
        if name in workspace:
            raise PublishGraphError(f"cargo metadata repeats workspace package name {name}")
        workspace[name] = package
    missing_ids = sorted(member_ids - {package.get("id") for package in workspace.values()})
    if missing_ids:
        raise PublishGraphError(
            "cargo metadata workspace_members reference missing packages: "
            + ", ".join(str(package_id) for package_id in missing_ids)
        )
    return workspace


def workspace_package_infos(metadata: dict) -> dict[str, PackageInfo]:
    workspace = _workspace_packages_by_name(metadata)
    publish_order = crates_io_publish_order(metadata)
    publish_set = set(publish_order)
    manifest_owners = {
        Path(str(package["manifest_path"])).resolve().parent: name
        for name, package in workspace.items()
    }
    out: dict[str, PackageInfo] = {}
    for name in publish_order:
        package = workspace[name]
        internal_deps: set[str] = set()
        for dependency in package.get("dependencies", []) or []:
            if not isinstance(dependency, dict) or dependency.get("kind") == "dev":
                continue
            dependency_path = dependency.get("path")
            if not isinstance(dependency_path, str):
                continue
            dependency_name = manifest_owners.get(Path(dependency_path).resolve())
            if dependency_name in publish_set and dependency_name != name:
                internal_deps.add(dependency_name)
        out[name] = PackageInfo(
            name=name,
            version=str(package["version"]),
            manifest_path=Path(str(package["manifest_path"])),
            internal_deps=tuple(sorted(internal_deps)),
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

def git_tag_exists(repo_root: Path, tag: str) -> bool:
    cp = run_command(["git", "tag", "--list", tag], cwd=repo_root, capture=True)
    if cp.returncode != 0:
        raise RuntimeError("Failed to list git tags")
    return (cp.stdout or "").strip() == tag


def git_create_annotated_tag(repo_root: Path, tag: str, message: str, *, dry_run: bool) -> None:
    if git_tag_exists(repo_root, tag):
        raise RuntimeError(f"git tag already exists: {tag}")
    cp = run_command(["git", "tag", "-a", tag, "-m", message], cwd=repo_root, dry_run=dry_run)
    if cp.returncode != 0:
        raise RuntimeError(f"Failed to create git tag: {tag}")


def iter_publish_list(
    publish_order: Iterable[str],
    *,
    requested: Optional[set[str]],
    start_from: Optional[str],
) -> list[str]:
    crates = [crate for crate in publish_order if requested is None or crate in requested]
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
    listing = parser.add_mutually_exclusive_group()
    listing.add_argument(
        "--list-crates-io-packages",
        action="store_true",
        help="Print the dependency-safe crates.io publish order, one package per line",
    )
    listing.add_argument(
        "--list-crates-io-initial-batch",
        action="store_true",
        help="Print the dependency-free first crates.io publish batch",
    )
    parser.add_argument(
        "--yes",
        action="store_true",
        help="Assume 'yes' for confirmation prompts (required for non-interactive runs)",
    )
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
        "--preflight-publish-dry-run",
        action="store_true",
        help="Run `cargo publish --dry-run` per crate before uploading (slower, safer)",
    )
    parser.add_argument(
        "--preflight-only",
        action="store_true",
        help="Only run preflight checks (xtask verify + cargo publish --dry-run), do not upload",
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
    parser.add_argument(
        "--tag",
        help="Create an annotated git tag after publishing (e.g. v0.1.0). Does not push.",
    )

    args = parser.parse_args()

    repo_root = Path(__file__).resolve().parents[1]

    def confirm(prompt: str, *, default: bool) -> bool:
        if args.yes:
            print_info(f"--yes: auto-confirmed: {prompt}")
            return True
        if not sys.stdin.isatty():
            raise RuntimeError(f"Non-interactive session; rerun with --yes to confirm: {prompt}")
        suffix = " [Y/n]: " if default else " [y/N]: "
        resp = input(prompt + suffix).strip().lower()
        if not resp:
            return default
        return resp in ("y", "yes")

    try:
        require_tool("cargo")
        if not (args.list_crates_io_packages or args.list_crates_io_initial_batch):
            require_tool("git")
    except Exception as e:
        print_error(str(e))
        return 2

    try:
        metadata = cargo_metadata(
            repo_root,
            quiet=args.list_crates_io_packages or args.list_crates_io_initial_batch,
        )
        publish_batches = crates_io_publish_batches(metadata)
        publish_order = tuple(crate for batch in publish_batches for crate in batch)
        packages = workspace_package_infos(metadata)
    except Exception as e:
        print_error(str(e))
        return 2

    if args.list_crates_io_packages:
        for crate in publish_order:
            print(crate)
        return 0
    if args.list_crates_io_initial_batch:
        for crate in publish_batches[0]:
            print(crate)
        return 0

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
        unknown = requested - set(publish_order)
        if unknown:
            print_error(
                "Unknown or non-publishable crates: " + ", ".join(sorted(unknown))
            )
            print_info(f"Known crates: {', '.join(publish_order)}")
            return 2

    try:
        crates = iter_publish_list(
            publish_order,
            requested=requested,
            start_from=args.start_from,
        )
    except Exception as e:
        print_error(str(e))
        return 2

    missing = [c for c in crates if c not in packages]
    if missing:
        print_error(f"Crates not found in workspace: {', '.join(missing)}")
        return 2

    if args.preflight_only and not args.preflight_publish_dry_run:
        print_error("--preflight-only requires --preflight-publish-dry-run")
        return 2

    print_header("Publish Plan")
    print_info(f"Repo: {repo_root}")
    print_info(f"Dry run: {args.dry_run}")
    print_info(f"Wait time: {args.wait}s")
    print_info(f"Preflight xtask verify: {not args.skip_xtask_verify}")
    print_info(f"cargo publish --no-verify: {args.no_verify}")
    print_info(f"Preflight publish --dry-run: {args.preflight_publish_dry_run}")
    print_info(f"Tag after publish: {args.tag or '(none)'}")
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
        if not confirm(
            "Continue with publishing?"
            if not args.preflight_only
            else "Continue with preflight (no upload)?",
            default=False,
        ):
            print_info("Cancelled.")
            return 0

    failures: list[str] = []
    ok: list[str] = []
    skipped: list[str] = []
    for c in crates:
        p = packages[c]
        if args.preflight_only:
            print_header(f"Preflight {p.name} v{p.version}")
        else:
            print_header(f"Publishing {p.name} v{p.version}")

        if not args.no_check_published and not args.dry_run and not args.preflight_only:
            if check_crate_published(p.name, p.version):
                print_warning(f"{p.name} v{p.version} appears already published.")
                if confirm("Skip this crate?", default=True):
                    print_info(f"Skipping {p.name}")
                    skipped.append(p.name)
                    continue

        if args.preflight_publish_dry_run:
            if args.preflight_only and p.internal_deps:
                missing_internal: list[str] = []
                for dep in p.internal_deps:
                    dep_ver = packages[dep].version if dep in packages else "(unknown)"
                    if dep_ver == "(unknown)" or not check_crate_published(dep, dep_ver):
                        missing_internal.append(f"{dep} v{dep_ver}")
                if missing_internal:
                    print_warning(
                        "Skipping preflight: internal workspace dependencies are not published yet: "
                        + ", ".join(missing_internal)
                    )
                    skipped.append(p.name)
                    continue

            pre = ["cargo", "publish", "-p", p.name, "--dry-run"]
            cp = run_command(pre, cwd=repo_root, dry_run=args.dry_run)
            if cp.returncode != 0:
                print_error(f"Preflight failed for {p.name}")
                failures.append(p.name)
                break
            if args.preflight_only:
                print_success(f"Preflight ok for {p.name} v{p.version}")
                ok.append(p.name)
                continue

        cmd = ["cargo", "publish", "-p", p.name]
        if args.no_verify:
            cmd.append("--no-verify")
        cp = run_command(cmd, cwd=repo_root, dry_run=args.dry_run)
        if cp.returncode != 0:
            print_error(f"Failed to publish {p.name}")
            failures.append(p.name)
            if not args.dry_run:
                if not confirm("Continue with remaining crates?", default=False):
                    break
        else:
            print_success(f"Published {p.name} v{p.version}")
            ok.append(p.name)
            if not args.dry_run and args.wait > 0:
                print_info(f"Waiting {args.wait}s for crates.io indexing...")
                time.sleep(args.wait)

    print_header("Publish Result")
    if failures:
        print_error(f"Failed crates: {', '.join(failures)}")
        return 1

    if args.tag:
        tag = args.tag.strip()
        if tag:
            msg = f"Release {tag}"
            print_header(f"Tagging {tag}")
            try:
                git_create_annotated_tag(repo_root, tag, msg, dry_run=args.dry_run)
            except Exception as e:
                print_error(str(e))
                return 1
            print_success(f"Created git tag {tag}")
            print_info(f"Next: git push origin {tag}")

    if args.preflight_only:
        if skipped:
            print_warning(f"Skipped {len(skipped)} crate(s): {', '.join(skipped)}")
        print_success(f"Preflight ok for {len(ok)} crate(s).")
    else:
        if skipped:
            print_warning(f"Skipped {len(skipped)} crate(s): {', '.join(skipped)}")
        print_success(f"Published {len(ok)} crate(s).")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
