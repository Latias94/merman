#!/usr/bin/env python3
"""Validate the static ownership contract for the FFI/native release surfaces.

This file intentionally describes ownership and build inputs only. It is not a
live registry-status database; publication evidence still comes from the owning
registry, GitHub Release, or package index.
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass
import json
from pathlib import Path
import sys
import tomllib


ROOT = Path(__file__).resolve().parents[1]
WORKFLOW_ROOT = Path(".github/workflows")
PREFLIGHT_WORKFLOW = WORKFLOW_ROOT / "release-preflight.yml"
ARTIFACT_DESCRIPTOR = Path("capabilities/artifact-profiles-v1.json")
ANDROID_JNI_MANIFEST = Path("crates/merman-android-jni/Cargo.toml")

if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from scripts.release_version import parse_release_version
from tools.publish import cargo_metadata, crates_io_publish_order


class SurfaceContractError(RuntimeError):
    """The repository no longer matches its declared release surfaces."""


@dataclass(frozen=True)
class ReleaseSurface:
    surface_id: str
    workflow: Path
    delivery: str
    publication_mode: str
    preflight_job: str
    workflow_markers: tuple[str, ...]
    profile_packages: tuple[tuple[str, str], ...] = ()
    source_packages: tuple[str, ...] = ()


SURFACES = (
    ReleaseSurface(
        surface_id="rust-crates-and-ffi-source",
        workflow=WORKFLOW_ROOT / "release-crates.yml",
        delivery="crates.io source crates",
        publication_mode="tag-triggered",
        preflight_job="versions-and-packages",
        workflow_markers=(
            "      - 'v*'",
            "tools/publish.py",
            "trusted/scripts/crates_io_release.py publish-receipted",
        ),
        source_packages=(
            "merman-bindings-core",
            "merman-ffi",
            "merman-uniffi",
            "merman-wasm",
        ),
    ),
    ReleaseSurface(
        surface_id="android-aar",
        workflow=WORKFLOW_ROOT / "release-android.yml",
        delivery="GitHub Release AAR",
        publication_mode="manual-dispatch",
        preflight_job="android-aar",
        workflow_markers=(
            "workflow_dispatch:",
            "merman-android-${RELEASE_TAG}.aar",
            "Upload AAR to GitHub Release",
            "scripts/release-version.py check --version \"$VERSION\"",
        ),
        profile_packages=(("android-native", "merman-android-jni"),),
    ),
    ReleaseSurface(
        surface_id="apple-xcframework",
        workflow=WORKFLOW_ROOT / "release-apple.yml",
        delivery="GitHub Release XCFramework",
        publication_mode="manual-dispatch",
        preflight_job="apple-xcframework",
        workflow_markers=(
            "workflow_dispatch:",
            "Merman.xcframework-${RELEASE_TAG}.zip",
            "Upload XCFramework to GitHub Release",
            "scripts/release-version.py check --version \"$VERSION\"",
        ),
        profile_packages=(("apple-uniffi-native", "merman-uniffi"),),
    ),
    ReleaseSurface(
        surface_id="python-wheel",
        workflow=WORKFLOW_ROOT / "release-python.yml",
        delivery="GitHub Release wheels and PyPI",
        publication_mode="manual-dispatch",
        preflight_job="python-wheel",
        workflow_markers=(
            "workflow_dispatch:",
            "publish_to_pypi:",
            "environment: pypi",
            "scripts/release-version.py check --version \"$VERSION\"",
        ),
        profile_packages=(("python-uniffi-native", "merman-uniffi"),),
    ),
    ReleaseSurface(
        surface_id="flutter-pub",
        workflow=WORKFLOW_ROOT / "release-flutter.yml",
        delivery="pub.dev Native Assets",
        publication_mode="flutter-tag-or-manual-validation",
        preflight_job="flutter-dry-run",
        workflow_markers=(
            '      - "flutter-v*"',
            "workflow_dispatch:",
            "environment: pub.dev",
            "reconcile_pub_package",
            "scripts/release-version.py check --version \"$VERSION\"",
        ),
        profile_packages=(
            ("flutter-android-native", "merman-ffi"),
            ("flutter-ios-native", "merman-ffi"),
            ("flutter-desktop-native", "merman-ffi"),
        ),
    ),
)


def _read(root: Path, path: Path) -> str:
    target = root / path
    try:
        return target.read_text(encoding="utf-8")
    except OSError as error:
        raise SurfaceContractError(f"cannot read {path}: {error}") from error


def _load_json(root: Path, path: Path) -> dict:
    try:
        value = json.loads(_read(root, path))
    except json.JSONDecodeError as error:
        raise SurfaceContractError(f"invalid JSON in {path}: {error}") from error
    if not isinstance(value, dict):
        raise SurfaceContractError(f"{path} must contain a JSON object")
    return value


def _profile_map(root: Path) -> dict[str, dict]:
    descriptor = _load_json(root, ARTIFACT_DESCRIPTOR)
    raw_profiles = descriptor.get("profiles")
    if not isinstance(raw_profiles, list):
        raise SurfaceContractError(f"{ARTIFACT_DESCRIPTOR}.profiles must be an array")
    profiles: dict[str, dict] = {}
    for raw_profile in raw_profiles:
        if not isinstance(raw_profile, dict) or not isinstance(raw_profile.get("id"), str):
            raise SurfaceContractError(
                f"{ARTIFACT_DESCRIPTOR} contains an invalid profile declaration"
            )
        profile_id = raw_profile["id"]
        if profile_id in profiles:
            raise SurfaceContractError(f"duplicate artifact profile {profile_id}")
        profiles[profile_id] = raw_profile
    return profiles


def _assert_profile_packages(root: Path, surface: ReleaseSurface, errors: list[str]) -> None:
    profiles = _profile_map(root)
    for profile_id, expected_package in surface.profile_packages:
        profile = profiles.get(profile_id)
        if profile is None:
            errors.append(f"{surface.surface_id}: missing artifact profile {profile_id}")
            continue
        cargo = profile.get("cargo")
        actual_package = cargo.get("package") if isinstance(cargo, dict) else None
        if actual_package != expected_package:
            errors.append(
                f"{surface.surface_id}: profile {profile_id} targets "
                f"{actual_package!r}, expected {expected_package!r}"
            )


def _assert_android_jni_boundary(root: Path, publish_names: set[str], errors: list[str]) -> None:
    try:
        manifest = tomllib.loads(_read(root, ANDROID_JNI_MANIFEST))
    except tomllib.TOMLDecodeError as error:
        raise SurfaceContractError(f"invalid TOML in {ANDROID_JNI_MANIFEST}: {error}") from error
    package = manifest.get("package")
    publish = package.get("publish") if isinstance(package, dict) else None
    if publish is not False:
        errors.append(
            "android-native: merman-android-jni must remain publish = false; "
            "the AAR, not the JNI implementation crate, is the public delivery"
        )
    if "merman-android-jni" in publish_names:
        errors.append(
            "android-native: private merman-android-jni unexpectedly appears in the crates.io graph"
        )


def validate_repository(
    root: Path = ROOT,
    *,
    metadata: dict | None = None,
) -> tuple[ReleaseSurface, ...]:
    """Validate static surface declarations and return the declared surfaces."""
    root = root.resolve()
    if metadata is None:
        metadata = cargo_metadata(root, quiet=True)
    try:
        publish_names = set(crates_io_publish_order(metadata))
    except (RuntimeError, ValueError) as error:
        raise SurfaceContractError(f"cannot derive the crates.io publish graph: {error}") from error

    preflight = _read(root, PREFLIGHT_WORKFLOW)
    errors: list[str] = []
    for surface in SURFACES:
        try:
            workflow = _read(root, surface.workflow)
        except SurfaceContractError as error:
            errors.append(str(error))
            continue
        for marker in surface.workflow_markers:
            if marker not in workflow:
                errors.append(f"{surface.surface_id}: {surface.workflow} is missing {marker!r}")
        job_marker = f"  {surface.preflight_job}:\n"
        if job_marker not in preflight:
            errors.append(
                f"{surface.surface_id}: release preflight is missing job {surface.preflight_job!r}"
            )
        for package in surface.source_packages:
            if package not in publish_names:
                errors.append(
                    f"{surface.surface_id}: source package {package} is absent "
                    "from the crates.io graph"
                )
        _assert_profile_packages(root, surface, errors)

    _assert_android_jni_boundary(root, publish_names, errors)
    if errors:
        raise SurfaceContractError("\n".join(errors))
    return SURFACES


def render_report(surfaces: tuple[ReleaseSurface, ...], *, as_json: bool = False) -> str:
    """Render FFI/native declarations without implying current publication status."""
    rows = [
        {
            "id": surface.surface_id,
            "workflow": surface.workflow.as_posix(),
            "delivery": surface.delivery,
            "publication_mode": surface.publication_mode,
            "preflight_job": surface.preflight_job,
            "source_packages": list(surface.source_packages),
            "artifact_profiles": [profile for profile, _package in surface.profile_packages],
        }
        for surface in surfaces
    ]
    if as_json:
        return json.dumps(
            {"scope": "ffi-native", "surfaces": rows},
            ensure_ascii=False,
            indent=2,
        ) + "\n"
    lines = [
        f"FFI/native release surface contract: {len(rows)} static declarations",
        "id | workflow | delivery | publication mode | preflight job",
    ]
    lines.extend(
        " | ".join(
            (
                row["id"],
                row["workflow"],
                row["delivery"],
                row["publication_mode"],
                row["preflight_job"],
            )
        )
        for row in rows
    )
    return "\n".join(lines) + "\n"


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--version",
        help="optional release version to validate with the shared release parser",
    )
    parser.add_argument(
        "--json",
        action="store_true",
        help="render machine-readable static declarations",
    )
    parser.add_argument(
        "--repo-root",
        type=Path,
        default=ROOT,
        help=argparse.SUPPRESS,
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        if args.version is not None:
            parse_release_version(args.version, allow_v_prefix=False)
        surfaces = validate_repository(args.repo_root.resolve())
        print(render_report(surfaces, as_json=args.json), end="")
    except (OSError, SurfaceContractError, RuntimeError, ValueError) as error:
        print(f"release_surface_contract.py: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
