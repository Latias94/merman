#!/usr/bin/env python3
"""Collect release-range facts without building or changing the source tree."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import re
import shutil
import subprocess
import sys
import tomllib
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


DEFAULT_MANIFESTS = [
    "merman",
    "merman-cli",
    "merman-wasm",
    "merman-core",
    "merman-render",
    "merman-bindings-core",
]


class FactError(RuntimeError):
    """Raised when a requested release fact cannot be collected."""


def command(
    args: list[str],
    cwd: Path,
    *,
    check: bool = True,
) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(
        args,
        cwd=cwd,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if check and result.returncode != 0:
        detail = result.stderr.strip() or result.stdout.strip()
        raise FactError(f"{' '.join(args)} failed ({result.returncode}): {detail}")
    return result


def git_text(repo: Path, args: list[str], *, check: bool = True) -> str:
    result = command(["git", *args], repo, check=check)
    return result.stdout


def resolve_commit(repo: Path, ref: str) -> str:
    return git_text(repo, ["rev-parse", "--verify", f"{ref}^{{commit}}"]).strip()


def is_ancestor(repo: Path, base: str, target: str) -> bool:
    return command(
        ["git", "merge-base", "--is-ancestor", base, target],
        repo,
        check=False,
    ).returncode == 0


def choose_base(repo: Path, target: str) -> str:
    result = command(
        [
            "git",
            "describe",
            "--tags",
            "--match",
            "v*",
            "--abbrev=0",
            "--first-parent",
            f"{target}^",
        ],
        repo,
        check=False,
    )
    tag = result.stdout.strip()
    if result.returncode == 0 and tag:
        return tag
    detail = result.stderr.strip() or "no earlier first-parent v* tag"
    raise FactError(
        "could not infer the previous release tag "
        f"for {target}: {detail}; pass --base explicitly"
    )


def show_file(repo: Path, ref: str, relative_path: str) -> str | None:
    result = command(
        ["git", "show", f"{ref}:{relative_path}"],
        repo,
        check=False,
    )
    if result.returncode != 0:
        return None
    return result.stdout


def manifest_facts(
    repo: Path,
    ref: str,
    package_name: str,
) -> dict[str, Any]:
    relative_path = f"crates/{package_name}/Cargo.toml"
    text = show_file(repo, ref, relative_path)
    if text is None:
        return {
            "package": package_name,
            "status": "unavailable",
            "path": relative_path,
        }
    try:
        document = tomllib.loads(text)
    except tomllib.TOMLDecodeError as error:
        return {
            "package": package_name,
            "status": "error",
            "path": relative_path,
            "error": f"invalid TOML: {error}",
        }
    package = document.get("package", {})
    features = document.get("features", {})
    dependencies = document.get("dependencies", {})
    optional_dependencies: list[str] = []
    direct_dependencies: list[str] = []
    for name, value in dependencies.items():
        direct_dependencies.append(name)
        if isinstance(value, dict) and value.get("optional") is True:
            optional_dependencies.append(name)
    return {
        "package": package_name,
        "status": "measured",
        "path": relative_path,
        "manifest_package_name": package.get("name"),
        "version": package.get("version"),
        "feature_count": len(features),
        "features": sorted(features),
        "direct_dependency_count": len(direct_dependencies),
        "direct_dependencies": sorted(direct_dependencies),
        "optional_dependency_count": len(optional_dependencies),
        "optional_dependencies": sorted(optional_dependencies),
    }


def support_facts(repo: Path, ref: str) -> dict[str, Any]:
    inventory_path = "crates/xtask/src/cmd/admission.rs"
    inventory = show_file(repo, ref, inventory_path)
    if inventory is not None:
        inventory_section = inventory.partition("const ADMISSION_INVENTORY")[2]
        families = re.findall(
            r'(?m)^\s*primary(?:_root_deferred)?!\(\s*\n\s*"([^"]+)"',
            inventory_section,
        )
        if families:
            return {
                "status": "measured",
                "path": inventory_path,
                "primary_svg_admission_record_count": len(families),
                "primary_svg_admission_ids": families,
                "matrix_format": "admission_inventory",
            }

    relative_path = "docs/alignment/STATUS.md"
    text = show_file(repo, ref, relative_path)
    if text is None:
        return {
            "status": "unavailable",
            "path": relative_path,
        }
    sections = (
        ("## Primary SVG Matrix", "## Non-Primary Families", "primary_svg"),
        ("## Diagram Coverage Matrix", "## Mermaid 11.15 Diagram Family Scope", "legacy"),
    )
    selected: tuple[str, str, str] | None = None
    for start_marker, end_marker, kind in sections:
        if start_marker in text:
            selected = (start_marker, end_marker, kind)
            break
    if selected is None:
        return {
            "status": "unavailable",
            "path": relative_path,
            "reason": "known coverage matrix heading is absent",
        }
    start_marker, end_marker, kind = selected
    section = text.split(start_marker, 1)[1]
    if end_marker in section:
        section = section.split(end_marker, 1)[0]
    families: list[str] = []
    for line in section.splitlines():
        if not line.startswith("|"):
            continue
        parts = line.split("|")
        if len(parts) < 3:
            continue
        name = parts[1].strip().strip("`")
        if not name or name in {"Family", "Diagram"} or set(name) == {"-"}:
            continue
        if kind == "primary_svg":
            families.append(name)
            continue
        if len(parts) >= 7 and parts[5].strip() == "yes" and parts[6].strip().startswith("yes"):
            families.append(name)
    return {
        "status": "measured",
        "path": relative_path,
        "primary_svg_admission_record_count": len(families),
        "primary_svg_admission_ids": families,
        "matrix_format": kind,
    }


def changed_paths(repo: Path, base: str, target: str) -> list[dict[str, str]]:
    output = git_text(repo, ["diff", "--name-status", base, target])
    changes: list[dict[str, str]] = []
    for line in output.splitlines():
        fields = line.split("\t")
        if len(fields) < 2:
            continue
        status = fields[0]
        if status.startswith(("R", "C")) and len(fields) >= 3:
            changes.append(
                {
                    "status": status,
                    "path": fields[-1],
                    "previous_path": fields[-2],
                }
            )
        else:
            changes.append({"status": status, "path": fields[-1]})
    return changes


def first_parent_commits(repo: Path, base: str, target: str) -> list[dict[str, str]]:
    output = git_text(
        repo,
        ["log", "--first-parent", "--format=%H%x09%h%x09%s", f"{base}..{target}"],
    )
    commits: list[dict[str, str]] = []
    for line in output.splitlines():
        fields = line.split("\t", 2)
        if len(fields) == 3:
            commits.append(
                {
                    "commit": fields[0],
                    "short": fields[1],
                    "subject": fields[2],
                }
            )
    return commits


def tool_version(executable: str, args: list[str] | None = None) -> str | None:
    if shutil.which(executable) is None:
        return None
    result = subprocess.run(
        [executable, *(args or ["--version"])],
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if result.returncode != 0:
        return None
    value = result.stdout.strip() or result.stderr.strip()
    return value or None


def digest_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return f"sha256:{digest.hexdigest()}"


def ref_file_digest(repo: Path, ref: str, relative_path: str) -> str | None:
    text = show_file(repo, ref, relative_path)
    if text is None:
        return None
    digest = hashlib.sha256(text.encode("utf-8")).hexdigest()
    return f"sha256:{digest}"


def compressed_size(path: Path, tool: str) -> int | None:
    executable = shutil.which(tool)
    if executable is None:
        return None
    if tool == "gzip":
        args = [executable, "-n", "-9", "-c", str(path)]
    else:
        args = [executable, "-q", "11", "-c", str(path)]
    result = subprocess.run(
        args,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if result.returncode != 0:
        return None
    return len(result.stdout)


def artifact_fact(repo: Path, specification: str) -> dict[str, Any]:
    if "=" not in specification:
        raise FactError(
            f"invalid --artifact {specification!r}; expected LABEL=PATH"
        )
    label, raw_path = specification.split("=", 1)
    if not label or not raw_path:
        raise FactError(
            f"invalid --artifact {specification!r}; expected LABEL=PATH"
        )
    path = Path(raw_path)
    if not path.is_absolute():
        path = repo / path
    path = path.resolve()
    if not path.is_file():
        return {
            "label": label,
            "path": str(path),
            "status": "unavailable",
        }
    return {
        "label": label,
        "path": str(path),
        "status": "measured",
        "raw_bytes": path.stat().st_size,
        "sha256": digest_file(path),
        "gzip_bytes": compressed_size(path, "gzip"),
        "brotli_bytes": compressed_size(path, "brotli"),
    }


def dirty_paths(repo: Path) -> list[str]:
    return [
        line[3:]
        for line in git_text(repo, ["status", "--porcelain"]).splitlines()
        if len(line) >= 4
    ]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Collect release comparison facts without building the repository."
    )
    parser.add_argument("--repo", type=Path, default=Path.cwd())
    parser.add_argument("--base")
    parser.add_argument("--target", default="HEAD")
    parser.add_argument(
        "--manifest",
        action="append",
        dest="manifests",
        help="Package name under crates/. Repeat to select packages.",
    )
    parser.add_argument(
        "--artifact",
        action="append",
        default=[],
        help="Artifact fact in LABEL=PATH form. Repeat for multiple artifacts.",
    )
    parser.add_argument("--output", type=Path)
    parser.add_argument("--pretty", action="store_true")
    return parser.parse_args()


def collect(args: argparse.Namespace) -> dict[str, Any]:
    repo = args.repo.resolve()
    if not (repo / ".git").exists() and not (repo / ".git").is_file():
        raise FactError(f"not a Git worktree: {repo}")
    target_commit = resolve_commit(repo, args.target)
    base_ref = args.base or choose_base(repo, args.target)
    base_commit = resolve_commit(repo, base_ref)
    if not is_ancestor(repo, base_commit, target_commit):
        raise FactError(
            f"base {base_ref} ({base_commit}) is not an ancestor of "
            f"target {args.target} ({target_commit})"
        )
    manifests = args.manifests or DEFAULT_MANIFESTS
    worktree_dirty_paths = dirty_paths(repo)
    comparison_changed_paths = changed_paths(repo, base_commit, target_commit)
    return {
        "schema_version": 1,
        "collected_at_utc": datetime.now(timezone.utc).isoformat(),
        "repository": str(repo),
        "worktree": {
            "dirty": bool(worktree_dirty_paths),
            "dirty_paths": worktree_dirty_paths,
        },
        "comparison": {
            "base_ref": base_ref,
            "base_commit": base_commit,
            "target_ref": args.target,
            "target_commit": target_commit,
            "ancestor_verified": True,
        },
        "source_digests": {
            "base": {
                "cargo_lock": ref_file_digest(repo, base_ref, "Cargo.lock"),
                "benchmark_corpus": ref_file_digest(
                    repo, base_ref, "tools/bench/corpus.json"
                ),
            },
            "target": {
                "cargo_lock": ref_file_digest(repo, args.target, "Cargo.lock"),
                "benchmark_corpus": ref_file_digest(
                    repo, args.target, "tools/bench/corpus.json"
                ),
            },
        },
        "diff": {
            "shortstat": git_text(
                repo, ["diff", "--shortstat", base_commit, target_commit]
            ).strip(),
            "changed_path_count": len(comparison_changed_paths),
            "changed_paths": comparison_changed_paths,
        },
        "first_parent_commits": first_parent_commits(
            repo, base_commit, target_commit
        ),
        "manifests": {
            "base": [
                manifest_facts(repo, base_ref, package) for package in manifests
            ],
            "target": [
                manifest_facts(repo, args.target, package) for package in manifests
            ],
        },
        "support": {
            "base": support_facts(repo, base_ref),
            "target": support_facts(repo, args.target),
        },
        "artifacts": [
            artifact_fact(repo, specification) for specification in args.artifact
        ],
        "tool_versions": {
            "git": tool_version("git"),
            "rustc": tool_version("rustc", ["--version", "--verbose"]),
            "cargo": tool_version("cargo"),
            "node": tool_version("node"),
            "npm": tool_version("npm"),
            "wasm_pack": tool_version("wasm-pack"),
            "wasm_tools": tool_version("wasm-tools"),
            "python": platform.python_version(),
        },
        "host": {
            "system": platform.system(),
            "release": platform.release(),
            "machine": platform.machine(),
            "processor": platform.processor(),
            "logical_cpu_count": os.cpu_count(),
        },
        "notes": [
            "Manifest facts describe source declarations; they are not a transitive dependency closure.",
            "Artifact facts describe only explicitly supplied files; labels and paths do not prove revision or capability provenance.",
            "Run the benchmark matrix separately and preserve raw samples and parity evidence.",
        ],
    }


def main() -> int:
    args = parse_args()
    try:
        facts = collect(args)
    except FactError as error:
        print(f"collect_release_facts: {error}", file=sys.stderr)
        return 2
    text = json.dumps(
        facts,
        indent=2 if args.pretty or args.output else None,
        sort_keys=False,
    )
    if args.output:
        output = args.output.resolve()
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(f"{text}\n", encoding="utf-8")
    else:
        print(text)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
