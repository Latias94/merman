#!/usr/bin/env python3
"""Compile current and previous release lanes from fresh Cargo resolutions."""

from __future__ import annotations

import argparse
from dataclasses import dataclass
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile


ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from scripts.release_version import ReleaseVersion, parse_release_version
from tools.publish import cargo_metadata, publish_field_allows_crates_io


TOP_LEVEL_PACKAGE = "merman"
PACKAGE_EDITION = "2024"
FEATURES = ("ascii",)
PROJECT_PREFIX = "merman-prerelease-compatibility"


class PrereleaseCompatibilityError(RuntimeError):
    """A fresh prerelease Cargo lane did not compile safely."""


@dataclass(frozen=True)
class CandidatePackage:
    name: str
    version: str
    path: Path


def _independent_names(metadata: dict) -> frozenset[str]:
    raw_metadata = metadata.get("metadata", {})
    if raw_metadata is None:
        raw_metadata = {}
    if not isinstance(raw_metadata, dict):
        raise PrereleaseCompatibilityError("cargo metadata has invalid top-level metadata")
    release_metadata = raw_metadata.get("merman-release", {})
    if release_metadata is None:
        release_metadata = {}
    if not isinstance(release_metadata, dict):
        raise PrereleaseCompatibilityError(
            "cargo metadata has invalid merman-release metadata"
        )
    raw_names = release_metadata.get("independent-packages", [])
    if not isinstance(raw_names, list) or not all(
        isinstance(name, str) and name for name in raw_names
    ):
        raise PrereleaseCompatibilityError(
            "cargo metadata independent-packages must be a string array"
        )
    return frozenset(raw_names)


def candidate_packages(repo_root: Path, release: ReleaseVersion) -> tuple[CandidatePackage, ...]:
    """Read publishable coupled package paths from the repository metadata."""
    repo_root = repo_root.resolve()
    metadata = cargo_metadata(repo_root, quiet=True)
    workspace_ids = metadata.get("workspace_members")
    packages = metadata.get("packages")
    if not isinstance(workspace_ids, list) or not isinstance(packages, list):
        raise PrereleaseCompatibilityError(
            "cargo metadata must contain workspace_members and packages arrays"
        )
    member_ids = set(workspace_ids)
    independent = _independent_names(metadata)
    result: list[CandidatePackage] = []
    for package in packages:
        if not isinstance(package, dict) or package.get("id") not in member_ids:
            continue
        name = package.get("name")
        version = package.get("version")
        manifest = package.get("manifest_path")
        if not isinstance(name, str) or not isinstance(version, str) or not isinstance(
            manifest, str
        ):
            raise PrereleaseCompatibilityError(
                "cargo metadata package is missing name, version, or manifest_path"
            )
        if name in independent or not publish_field_allows_crates_io(package.get("publish")):
            continue
        if version != release.canonical:
            raise PrereleaseCompatibilityError(
                f"coupled package {name} has version {version}, expected {release.canonical}"
            )
        package_path = Path(manifest).resolve().parent
        try:
            package_path.relative_to(repo_root)
        except ValueError as error:
            raise PrereleaseCompatibilityError(
                f"candidate package {name} escapes the repository: {package_path}"
            ) from error
        result.append(CandidatePackage(name, version, package_path))

    result.sort(key=lambda package: package.name)
    names = {package.name for package in result}
    if TOP_LEVEL_PACKAGE not in names:
        raise PrereleaseCompatibilityError(
            f"publishable coupled package {TOP_LEVEL_PACKAGE} is missing from metadata"
        )
    return tuple(result)


def toml_string(value: str) -> str:
    """Return a TOML basic string suitable for a native path."""
    return '"' + value.replace("\\", "\\\\").replace('"', '\\"') + '"'


def render_manifest(
    dependency_version: str,
    patches: tuple[CandidatePackage, ...],
) -> str:
    """Render a deliberately lockfile-free consumer project."""
    lines = [
        "[package]",
        'name = "merman-prerelease-consumer"',
        'version = "0.0.0"',
        f'edition = "{PACKAGE_EDITION}"',
        "publish = false",
        "",
        "[dependencies]",
        (
            f'{TOP_LEVEL_PACKAGE} = {{ version = "={dependency_version}", '
            "default-features = false, "
            f'features = [{", ".join(toml_string(feature) for feature in FEATURES)}] }}'
        ),
    ]
    if patches:
        lines.extend(["", "[patch.crates-io]"])
        lines.extend(
            f'"{package.name}" = {{ path = {toml_string(str(package.path))} }}'
            for package in patches
        )
    return "\n".join(lines) + "\n"


def run_cargo_check(
    command: list[str],
    *,
    cwd: Path,
    env: dict[str, str],
) -> subprocess.CompletedProcess[str]:
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


def _validate_version(value: str, *, option: str) -> ReleaseVersion:
    try:
        return parse_release_version(value, allow_v_prefix=False)
    except ValueError as error:
        raise PrereleaseCompatibilityError(f"{option} is invalid: {error}") from error


def same_compatibility_line(first: ReleaseVersion, second: ReleaseVersion) -> bool:
    """Return whether Cargo's ordinary caret range can admit both releases."""
    if first.major == 0 or second.major == 0:
        return first.major == second.major and first.minor == second.minor
    return first.major == second.major


def _run_lane(
    lane: str,
    version: str,
    patches: tuple[CandidatePackage, ...],
    *,
    root: Path,
    target_directory: Path,
    run_check=run_cargo_check,
) -> None:
    project = root / lane
    project.mkdir()
    manifest = project / "Cargo.toml"
    manifest.write_text(render_manifest(version, patches), encoding="utf-8")
    source = project / "src"
    source.mkdir()
    (source / "main.rs").write_text("fn main() {}\n", encoding="utf-8")
    if (project / "Cargo.lock").exists():
        raise PrereleaseCompatibilityError(
            f"fresh {lane} consumer unexpectedly contains Cargo.lock"
        )

    environment = dict(os.environ)
    environment["CARGO_TERM_COLOR"] = "never"
    environment["CARGO_TARGET_DIR"] = str(target_directory)
    command = [
        "cargo",
        "check",
        "--manifest-path",
        str(manifest),
        "--quiet",
    ]
    completed = run_check(command, cwd=project, env=environment)
    if completed.returncode != 0:
        detail = (completed.stderr or completed.stdout or "").strip()
        raise PrereleaseCompatibilityError(
            f"{lane} lane failed for merman {version}: "
            f"{detail or f'exit status {completed.returncode}'}"
        )
    print(f"{lane}: merman {version} compiled from a fresh resolution")


def verify(
    repo_root: Path,
    version: str,
    previous_version: str | None = None,
    *,
    allow_missing_previous: bool = False,
    target_directory: Path | None = None,
    run_check=run_cargo_check,
) -> None:
    """Verify the candidate lane and, when available, the previous lane."""
    candidate = _validate_version(version, option="--version")
    if candidate.kind != "prerelease":
        print(f"stable release {candidate.canonical}: prerelease compatibility check skipped")
        return
    if previous_version is None:
        if allow_missing_previous:
            previous = None
        else:
            raise PrereleaseCompatibilityError(
                "a prerelease requires --previous-version; use --allow-missing-previous "
                "only for the first prerelease in a repository"
            )
    else:
        previous = _validate_version(previous_version, option="--previous-version")
        if previous.canonical == candidate.canonical:
            raise PrereleaseCompatibilityError(
                "--previous-version must differ from --version"
            )

    packages = candidate_packages(repo_root, candidate)
    all_patches = packages
    previous_patches = tuple(
        package for package in packages if package.name != TOP_LEVEL_PACKAGE
    )

    with tempfile.TemporaryDirectory(prefix=f"{PROJECT_PREFIX}-") as temp_dir:
        temp_root = Path(temp_dir)
        shared_target = (target_directory or temp_root / "target").resolve()
        _run_lane(
            "candidate",
            candidate.canonical,
            all_patches,
            root=temp_root,
            target_directory=shared_target,
            run_check=run_check,
        )
        same_line_previous = (
            previous is not None
            and previous.kind == "prerelease"
            and same_compatibility_line(candidate, previous)
        )
        if same_line_previous:
            _run_lane(
                "previous-with-candidate-siblings",
                previous.canonical,
                previous_patches,
                root=temp_root,
                target_directory=shared_target,
                run_check=run_check,
            )
        elif previous is not None:
            print(
                "previous lane: skipped because the previous version is outside the "
                "candidate prerelease compatibility line"
            )
        else:
            print("previous lane: skipped because this is the first prerelease")


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", required=True, help="candidate release version")
    parser.add_argument(
        "--previous-version",
        help="latest published workspace version used by the previous consumer lane",
    )
    parser.add_argument(
        "--allow-missing-previous",
        action="store_true",
        help="allow the first prerelease to run only the candidate lane",
    )
    parser.add_argument(
        "--repo-root",
        type=Path,
        default=ROOT,
        help=argparse.SUPPRESS,
    )
    return parser.parse_args(argv)


def require_cargo() -> None:
    if shutil.which("cargo") is None:
        raise PrereleaseCompatibilityError("required tool not found in PATH: cargo")


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        require_cargo()
        verify(
            args.repo_root.resolve(),
            args.version,
            args.previous_version,
            allow_missing_previous=args.allow_missing_previous,
            target_directory=args.repo_root.resolve() / "target" / "prerelease-compatibility",
        )
    except (OSError, PrereleaseCompatibilityError, RuntimeError, ValueError) as error:
        print(f"verify_prerelease_compatibility.py: {error}", file=os.sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
