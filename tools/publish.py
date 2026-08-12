#!/usr/bin/env python3
"""Derive the crates.io publish graph from Cargo metadata."""

from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path


def print_error(msg: str) -> None:
    print(f"error: {msg}", file=sys.stderr)


def run_command(
    cmd: list[str],
    *,
    cwd: Path | None = None,
    capture: bool = False,
    quiet: bool = False,
    env: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[str]:
    if not quiet:
        print("Running:", " ".join(str(component) for component in cmd))

    if capture:
        return subprocess.run(
            cmd,
            cwd=str(cwd) if cwd else None,
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            check=False,
            env=env,
        )
    return subprocess.run(
        cmd,
        cwd=str(cwd) if cwd else None,
        check=False,
        env=env,
    )


def require_tool(name: str) -> None:
    if shutil.which(name) is None:
        raise RuntimeError(f"Required tool not found in PATH: {name}")


@dataclass(frozen=True)
class PackageInfo:
    name: str
    version: str
    manifest_path: Path
    internal_deps: tuple[str, ...]


@dataclass(frozen=True)
class PublishPlan:
    batches: tuple[tuple[str, ...], ...]
    order: tuple[str, ...]
    packages: dict[str, PackageInfo]


@dataclass(frozen=True)
class _PublishGraph:
    workspace: dict[str, dict]
    dependencies: dict[str, frozenset[str]]
    batches: tuple[tuple[str, ...], ...]
    order: tuple[str, ...]


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


def _crates_io_publish_graph(metadata: dict) -> _PublishGraph:
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
    publish_batches = tuple(batches)
    return _PublishGraph(
        workspace=workspace,
        dependencies={
            name: frozenset(package_dependencies)
            for name, package_dependencies in dependencies.items()
        },
        batches=publish_batches,
        order=tuple(package for batch in publish_batches for package in batch),
    )


def crates_io_publish_batches(metadata: dict) -> tuple[tuple[str, ...], ...]:
    """Return deterministic topological batches for publishable workspace packages."""
    return _crates_io_publish_graph(metadata).batches


def crates_io_publish_order(metadata: dict) -> tuple[str, ...]:
    return _crates_io_publish_graph(metadata).order


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


def crates_io_publish_plan(metadata: dict) -> PublishPlan:
    graph = _crates_io_publish_graph(metadata)
    return PublishPlan(
        batches=graph.batches,
        order=graph.order,
        packages={
            name: PackageInfo(
                name=name,
                version=str(graph.workspace[name]["version"]),
                manifest_path=Path(str(graph.workspace[name]["manifest_path"])),
                internal_deps=tuple(sorted(graph.dependencies[name])),
            )
            for name in graph.order
        },
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    listing = parser.add_mutually_exclusive_group(required=True)
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
        "--repo-root",
        type=Path,
        default=Path(__file__).resolve().parents[1],
    )
    args = parser.parse_args()
    try:
        require_tool("cargo")
        metadata = cargo_metadata(args.repo_root.resolve(), quiet=True)
        graph = _crates_io_publish_graph(metadata)
    except (OSError, RuntimeError, ValueError) as error:
        print_error(str(error))
        return 2
    selected = graph.order if args.list_crates_io_packages else graph.batches[0]
    for crate in selected:
        print(crate)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
