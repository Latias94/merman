#!/usr/bin/env python3
"""Install and verify target-exact Rust license reports in Python wheels."""

from __future__ import annotations

import argparse
from collections.abc import Iterable
from email.parser import Parser
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
    "aarch64-unknown-linux-gnu": "linux-aarch64",
    "x86_64-apple-darwin": "macosx-10.13-x86_64",
    "x86_64-pc-windows-msvc": "win-amd64",
    "x86_64-unknown-linux-gnu": "linux-x86_64",
}
MANYLINUX_POLICY = "manylinux_2_28"


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


def wheel_identity(wheel: Path) -> tuple[str, tuple[str, ...]]:
    parts = wheel.stem.rsplit("-", 3)
    if (
        wheel.suffix != ".whl"
        or len(parts) != 4
        or not parts[0].startswith("merman-")
        or parts[0].count("-") != 1
        or parts[1:3] != ["py3", "none"]
    ):
        raise PythonWheelLicenseError(f"expected a merman py3-none platform wheel: {wheel.name}")
    return parts[0].removeprefix("merman-"), tuple(parts[3].split("."))


def wheel_target(wheel: Path) -> str:
    _, platforms = wheel_identity(wheel)
    candidates: set[str] = set()
    for platform in platforms:
        if re.fullmatch(r"macosx_\d+_\d+_arm64", platform):
            candidates.add("aarch64-apple-darwin")
        elif re.fullmatch(r"macosx_\d+_\d+_x86_64", platform):
            candidates.add("x86_64-apple-darwin")
        elif platform == "win_amd64":
            candidates.add("x86_64-pc-windows-msvc")
        elif match := re.fullmatch(
            r"(?:linux|manylinux(?:1|2010|2014|_\d+_\d+))_(aarch64|x86_64)", platform
        ):
            candidates.add(f"{match[1]}-unknown-linux-gnu")
        else:
            raise PythonWheelLicenseError(
                f"cannot map Python wheel platform tag to one supported Rust target: {wheel.name}"
            )
    if len(candidates) != 1:
        raise PythonWheelLicenseError(
            f"cannot map Python wheel platform tag to one supported Rust target: {wheel.name}"
        )
    return candidates.pop()


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


def verify_wheel_set(
    wheels: Iterable[Path],
    *,
    expected_targets: Iterable[str],
    root: Path = REPO_ROOT,
) -> dict[str, Path]:
    """Check exact target coverage, the declared Linux floor, and package identity."""
    observed: dict[str, Path] = {}
    versions: set[str] = set()
    for wheel in wheels:
        target = verify_wheel_license_report(wheel, root=root)
        if target in observed:
            raise PythonWheelLicenseError(f"duplicate Python wheel target {target}: {wheel}")
        version, platforms = wheel_identity(wheel)
        if target.endswith("-unknown-linux-gnu"):
            expected_platform = f"{MANYLINUX_POLICY}_{target.split('-')[0]}"
        else:
            expected_platform = wheel_platform_for_target(target).replace("-", "_").replace(".", "_")
        if platforms != (expected_platform,):
            raise PythonWheelLicenseError(f"{wheel.name} must use the exact platform {expected_platform}")

        dist_info = f"merman-{version}.dist-info"
        try:
            with zipfile.ZipFile(wheel) as archive:
                wheel_metadata = Parser().parsestr(archive.read(f"{dist_info}/WHEEL").decode())
                package_metadata = Parser().parsestr(archive.read(f"{dist_info}/METADATA").decode())
        except (OSError, zipfile.BadZipFile, KeyError, UnicodeError) as error:
            raise PythonWheelLicenseError(f"cannot read wheel identity for {wheel}: {error}") from error
        if (
            wheel_metadata.get("Root-Is-Purelib") != "false"
            or wheel_metadata.get_all("Tag") != [f"py3-none-{expected_platform}"]
            or package_metadata.get("Name") != "merman"
            or package_metadata.get("Version") != version
            or package_metadata.get("Requires-Python") != ">=3.9"
        ):
            raise PythonWheelLicenseError(f"{wheel.name} embedded metadata does not match its identity")
        observed[target] = wheel
        versions.add(version)

    expected = set(expected_targets)
    if set(observed) != expected:
        raise PythonWheelLicenseError(
            f"Python wheel target set mismatch: missing {sorted(expected - observed.keys())}; "
            f"unexpected {sorted(observed.keys() - expected)}"
        )
    if len(versions) != 1:
        raise PythonWheelLicenseError("Python wheel release set must contain exactly one package version")
    return observed


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--require-release-set",
        action="store_true",
        help="Require exactly the descriptor's wheel targets and the executed manylinux_2_28 floor.",
    )
    parser.add_argument("wheels", type=Path, nargs="+")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(sys.argv[1:] if argv is None else argv)
    try:
        if args.require_release_set:
            from artifact_profile_recipe import load_artifact_profile

            recipe = load_artifact_profile(PYTHON_ARTIFACT_PROFILE_ID)
            verify_wheel_set(args.wheels, expected_targets=recipe.build_targets)
        for wheel in args.wheels:
            target = (
                wheel_target(wheel) if args.require_release_set
                else verify_wheel_license_report(wheel)
            )
            print(f"Python wheel Rust license report: ok ({wheel.name}, {target})")
    except PythonWheelLicenseError as error:
        print(f"Python wheel Rust license report failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
