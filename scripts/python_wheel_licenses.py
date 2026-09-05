#!/usr/bin/env python3
"""Install and verify target-exact Rust license reports in Python wheels."""

from __future__ import annotations

import argparse
import json
import re
import sys
import zipfile
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]
PYTHON_ARTIFACT_PROFILE_ID = "python-uniffi-native"
TARGET_REPORT_ROOT = Path("platforms/python/legal/rust-cargo-dependencies")
PACKAGE_REPORT = Path("THIRD_PARTY_LICENSES/rust-cargo-dependencies.json")
SHA256_RE = re.compile(r"^[0-9a-f]{64}$")
WHEEL_PLATFORM_BY_RUST_TARGET = {
    "aarch64-apple-darwin": "macosx-11.0-arm64",
    "x86_64-pc-windows-msvc": "win-amd64",
    "x86_64-unknown-linux-gnu": "linux-x86_64",
}


class PythonWheelLicenseError(RuntimeError):
    """A target report or packaged wheel legal projection is invalid."""


def _reject_duplicate_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate JSON key {key!r}")
        result[key] = value
    return result


def target_report_path(root: Path, target: str) -> Path:
    if not target or "/" in target or "\\" in target or target in {".", ".."}:
        raise PythonWheelLicenseError(f"invalid Rust target triple {target!r}")
    return root / TARGET_REPORT_ROOT / f"{target}.json"


def load_target_report(root: Path, target: str) -> tuple[dict[str, Any], bytes]:
    path = target_report_path(root, target)
    try:
        encoded = path.read_bytes()
        document = json.loads(encoded, object_pairs_hook=_reject_duplicate_keys)
    except (OSError, json.JSONDecodeError, UnicodeError, ValueError) as error:
        raise PythonWheelLicenseError(
            f"cannot read target-exact Python wheel license report {path}: {error}"
        ) from error
    if not isinstance(document, dict):
        raise PythonWheelLicenseError(f"Python wheel license report {path} must be an object")
    validate_target_report(document, target)
    return document, encoded


def validate_target_report(report: dict[str, Any], target: str) -> None:
    if report.get("schema_version") != 3:
        raise PythonWheelLicenseError("Python wheel license report schema_version must be 3")

    bundle = report.get("artifact_bundle")
    if not isinstance(bundle, dict) or bundle.get("id") != f"python-wheel-{target}":
        raise PythonWheelLicenseError(
            f"Python wheel license report must identify python-wheel-{target}"
        )
    profiles = bundle.get("artifact_profiles")
    if (
        not isinstance(profiles, list)
        or len(profiles) != 1
        or not isinstance(profiles[0], dict)
        or profiles[0].get("id") != PYTHON_ARTIFACT_PROFILE_ID
    ):
        raise PythonWheelLicenseError(
            f"Python wheel license report must bind {PYTHON_ARTIFACT_PROFILE_ID}"
        )
    expected_observation = {
        "artifact_profile_id": PYTHON_ARTIFACT_PROFILE_ID,
        "target": target,
    }
    if bundle.get("target_observations") != [expected_observation]:
        raise PythonWheelLicenseError(
            f"Python wheel license report must contain only target {target}"
        )

    generator = report.get("generator")
    if not isinstance(generator, dict) or generator.get("command_profile") != (
        "artifact-profile-target"
    ):
        raise PythonWheelLicenseError(
            "Python wheel license report must use the single-target command profile"
        )

    target_closures = report.get("target_dependency_closures")
    if not isinstance(target_closures, list) or len(target_closures) != 1:
        raise PythonWheelLicenseError(
            "Python wheel license report must contain one target dependency closure"
        )
    target_closure = target_closures[0]
    if not isinstance(target_closure, dict):
        raise PythonWheelLicenseError("Python wheel target dependency closure must be an object")
    if {
        "artifact_profile_id": target_closure.get("artifact_profile_id"),
        "target": target_closure.get("target"),
    } != expected_observation:
        raise PythonWheelLicenseError(
            "Python wheel target dependency closure does not match its artifact observation"
        )
    closure = report.get("dependency_closure")
    expected_closure = {
        "package_count": target_closure.get("package_count"),
        "packages_sha256": target_closure.get("packages_sha256"),
    }
    if closure != expected_closure:
        raise PythonWheelLicenseError(
            "Python wheel union closure must equal its single target closure"
        )
    package_count = expected_closure["package_count"]
    packages_sha256 = expected_closure["packages_sha256"]
    if not isinstance(package_count, int) or isinstance(package_count, bool) or package_count <= 0:
        raise PythonWheelLicenseError("Python wheel dependency package_count must be positive")
    if not isinstance(packages_sha256, str) or not SHA256_RE.fullmatch(packages_sha256):
        raise PythonWheelLicenseError("Python wheel dependency closure hash is invalid")
    licenses = report.get("licenses")
    if not isinstance(licenses, list) or not licenses:
        raise PythonWheelLicenseError("Python wheel license report contains no licenses")


def install_target_report(root: Path, package_dir: Path, target: str) -> None:
    _, encoded = load_target_report(root, target)
    destination = package_dir / PACKAGE_REPORT
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_bytes(encoded)


def wheel_platform_for_target(target: str) -> str:
    try:
        return WHEEL_PLATFORM_BY_RUST_TARGET[target]
    except KeyError as error:
        raise PythonWheelLicenseError(
            f"unsupported Rust target for a Python wheel: {target}"
        ) from error


def wheel_target(wheel: Path) -> str:
    name = wheel.name.lower()
    candidates: list[str] = []
    if "-macosx_" in name and name.endswith("_arm64.whl"):
        candidates.append("aarch64-apple-darwin")
    if "-win_amd64.whl" in name:
        candidates.append("x86_64-pc-windows-msvc")
    if (
        any(marker in name for marker in ("-linux_", "-manylinux", "-musllinux"))
        and name.endswith("_x86_64.whl")
    ):
        candidates.append("x86_64-unknown-linux-gnu")
    if len(candidates) != 1:
        raise PythonWheelLicenseError(
            f"cannot map Python wheel platform tag to one supported Rust target: {wheel.name}"
        )
    return candidates[0]


def verify_wheel_license_report(
    wheel: Path,
    *,
    root: Path = REPO_ROOT,
    expected_target: str | None = None,
) -> str:
    tagged_target = wheel_target(wheel)
    target = expected_target or tagged_target
    if expected_target is not None and tagged_target != expected_target:
        raise PythonWheelLicenseError(
            f"{wheel} platform tag identifies {tagged_target}, not {expected_target}"
        )
    _, expected = load_target_report(root, target)
    suffix = f".dist-info/licenses/{PACKAGE_REPORT.as_posix()}"
    try:
        with zipfile.ZipFile(wheel) as archive:
            members = [name for name in archive.namelist() if name.endswith(suffix)]
            if len(members) != 1:
                raise PythonWheelLicenseError(
                    f"{wheel} must contain exactly one target-exact Rust license report"
                )
            observed = archive.read(members[0])
    except (OSError, zipfile.BadZipFile, KeyError) as error:
        raise PythonWheelLicenseError(f"cannot inspect Python wheel {wheel}: {error}") from error
    if observed != expected:
        raise PythonWheelLicenseError(
            f"{wheel} does not embed the checked-in {target} Rust license report"
        )
    return target


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("wheels", type=Path, nargs="+")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv or sys.argv[1:])
    try:
        for wheel in args.wheels:
            target = verify_wheel_license_report(wheel)
            print(f"Python wheel Rust license report: ok ({wheel.name}, {target})")
    except PythonWheelLicenseError as error:
        print(f"Python wheel Rust license report failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
