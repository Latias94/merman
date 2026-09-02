#!/usr/bin/env python3
"""Compile published dependents from fresh Cargo projects.

The check intentionally lives outside the workspace.  A workspace build can
silently resolve a path dependency, while a downstream consumer resolves the
independent crate through crates.io.  Each lane therefore gets a new manifest
and a new lockfile; the candidate lane adds only an explicit ``[patch]`` for
the package under release.
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import tomllib


ROOT = Path(__file__).resolve().parents[1]
PACKAGE_NAME = "merman-registry-dependent-smoke"
PACKAGE_VERSION = "0.0.0"
PACKAGE_EDITION = "2024"
PACKAGE_NAME_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9_-]*$")
VERSION_RE = re.compile(
    r"^(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)"
    r"(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?"
    r"(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$"
)


class RegistryDependentError(RuntimeError):
    """A fresh registry-dependent project could not be compiled safely."""


@dataclass(frozen=True)
class PackageSpec:
    name: str
    version: str


def parse_spec(raw: str, *, option: str) -> PackageSpec:
    if raw.count("=") != 1:
        raise RegistryDependentError(
            f"{option} must use PACKAGE=VERSION, received {raw!r}"
        )
    name, version = raw.split("=", 1)
    if PACKAGE_NAME_RE.fullmatch(name) is None:
        raise RegistryDependentError(f"{option} has an invalid package name: {name!r}")
    if VERSION_RE.fullmatch(version) is None:
        raise RegistryDependentError(f"{option} has an invalid package version: {version!r}")
    return PackageSpec(name, version)


def validate_candidate(candidate_path: Path, dependency: PackageSpec) -> Path:
    candidate_path = candidate_path.resolve()
    manifest_path = candidate_path / "Cargo.toml"
    if not manifest_path.is_file():
        raise RegistryDependentError(f"candidate path has no Cargo.toml: {candidate_path}")
    try:
        manifest = tomllib.loads(manifest_path.read_text(encoding="utf-8"))
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise RegistryDependentError(f"cannot read candidate manifest: {error}") from error
    package = manifest.get("package")
    if not isinstance(package, dict):
        raise RegistryDependentError("candidate manifest has no [package] table")
    actual_name = package.get("name")
    actual_version = package.get("version")
    if actual_name != dependency.name or actual_version != dependency.version:
        raise RegistryDependentError(
            "candidate manifest does not match the requested dependency: "
            f"expected {dependency.name} {dependency.version}, "
            f"found {actual_name} {actual_version}"
        )
    return candidate_path


def render_manifest(
    dependency: PackageSpec,
    dependent: PackageSpec,
    *,
    candidate_path: Path | None,
) -> str:
    lines = [
        "[package]",
        f'name = "{PACKAGE_NAME}"',
        f'version = "{PACKAGE_VERSION}"',
        f'edition = "{PACKAGE_EDITION}"',
        "publish = false",
        "",
        "[dependencies]",
        f'"{dependent.name}" = {{ version = "={dependent.version}" }}',
    ]
    if candidate_path is not None:
        lines.extend(
            [
                "",
                "[patch.crates-io]",
                f'"{dependency.name}" = {{ path = {toml_string(str(candidate_path))} }}',
            ]
        )
    return "\n".join(lines) + "\n"


def toml_string(value: str) -> str:
    """Return a basic TOML string with the characters Cargo paths need escaped."""
    return '"' + value.replace("\\", "\\\\").replace('"', '\\"') + '"'


def run_cargo_check(command: list[str], *, cwd: Path, env: dict[str, str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        command,
        cwd=cwd,
        env=env,
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        check=False,
    )


def verify(
    dependency: PackageSpec,
    dependents: tuple[PackageSpec, ...],
    *,
    candidate_path: Path | None = None,
    target_directory: Path | None = None,
    run_check=run_cargo_check,
) -> None:
    if not dependents:
        raise RegistryDependentError("at least one --dependent is required")
    if candidate_path is not None:
        candidate_path = validate_candidate(candidate_path, dependency)

    lanes: tuple[tuple[str, Path | None], ...]
    if candidate_path is None:
        lanes = (("registry", None),)
    else:
        lanes = (("candidate", candidate_path), ("registry", None))

    with tempfile.TemporaryDirectory(prefix="merman-registry-dependents-") as temp_dir:
        root = Path(temp_dir)
        shared_target = (target_directory or root / "target").resolve()
        for lane, patch_path in lanes:
            for index, dependent in enumerate(dependents):
                project = root / f"{lane}-{index}-{dependent.name}"
                project.mkdir()
                manifest = project / "Cargo.toml"
                manifest.write_text(
                    render_manifest(
                        dependency,
                        dependent,
                        candidate_path=patch_path,
                    ),
                    encoding="utf-8",
                )
                src = project / "src"
                src.mkdir()
                (src / "main.rs").write_text(
                    "fn main() {}\n",
                    encoding="utf-8",
                )
                if (project / "Cargo.lock").exists():
                    raise RegistryDependentError(
                        f"fresh project unexpectedly contains a lockfile: {project}"
                    )
                command = [
                    "cargo",
                    "check",
                    "--manifest-path",
                    str(manifest),
                    "--quiet",
                ]
                env = dict(os.environ)
                env["CARGO_TERM_COLOR"] = "never"
                env["CARGO_TARGET_DIR"] = str(shared_target)
                completed = run_check(command, cwd=project, env=env)
                if completed.returncode != 0:
                    detail = (completed.stderr or completed.stdout or "").strip()
                    raise RegistryDependentError(
                        f"{lane} registry-dependent check failed for "
                        f"{dependent.name} {dependent.version}: "
                        f"{detail or f'exit status {completed.returncode}'}"
                    )
                print(f"{lane}: {dependent.name} {dependent.version} compiled")


def require_cargo() -> None:
    if shutil.which("cargo") is None:
        raise RegistryDependentError("required tool not found in PATH: cargo")


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dependency", required=True, help="PACKAGE=VERSION under release")
    parser.add_argument(
        "--dependent",
        action="append",
        required=True,
        help="published dependent lane as PACKAGE=VERSION; repeat for each lane",
    )
    parser.add_argument(
        "--candidate-path",
        type=Path,
        help="local candidate package path; when present, run candidate and registry lanes",
    )
    parser.add_argument("--repo-root", type=Path, default=ROOT, help=argparse.SUPPRESS)
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        require_cargo()
        dependency = parse_spec(args.dependency, option="--dependency")
        dependents = tuple(
            parse_spec(raw, option="--dependent") for raw in args.dependent
        )
        verify(
            dependency,
            dependents,
            candidate_path=args.candidate_path,
            target_directory=(
                args.repo_root.resolve() / "target" / "registry-dependent-smoke"
            ),
        )
    except (OSError, RegistryDependentError) as error:
        print(f"release_registry_dependents.py: {error}", file=os.sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
