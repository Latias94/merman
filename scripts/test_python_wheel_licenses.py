#!/usr/bin/env python3

from __future__ import annotations

import json
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path


sys.path.insert(0, str(Path(__file__).resolve().parent))

import python_wheel_licenses as licenses


class PythonWheelLicenseTests(unittest.TestCase):
    def test_wheel_target_recognizes_every_published_platform_tag(self) -> None:
        cases = {
            "merman-0.8.0a4-py3-none-macosx_11_0_arm64.whl": "aarch64-apple-darwin",
            "merman-0.9.3-py3-none-macosx_10_13_x86_64.whl": "x86_64-apple-darwin",
            "merman-0.8.0a4-py3-none-win_amd64.whl": "x86_64-pc-windows-msvc",
            "merman-0.8.0a4-py3-none-manylinux_2_17_x86_64.whl": (
                "x86_64-unknown-linux-gnu"
            ),
            "merman-0.9.3-py3-none-manylinux_2_28_aarch64.whl": (
                "aarch64-unknown-linux-gnu"
            ),
            "merman-0.9.3-py3-none-linux_aarch64.whl": "aarch64-unknown-linux-gnu",
            "merman-0.9.2-py3-none-manylinux_2_17_x86_64.manylinux2014_x86_64.whl": (
                "x86_64-unknown-linux-gnu"
            ),
        }
        for wheel, target in cases.items():
            with self.subTest(wheel=wheel):
                self.assertEqual(licenses.wheel_target(Path(wheel)), target)

    def test_wheel_platform_for_target_matches_single_target_wheel_tags(self) -> None:
        cases = {
            "aarch64-apple-darwin": "macosx-11.0-arm64",
            "x86_64-apple-darwin": "macosx-10.13-x86_64",
            "aarch64-unknown-linux-gnu": "linux-aarch64",
            "x86_64-pc-windows-msvc": "win-amd64",
            "x86_64-unknown-linux-gnu": "linux-x86_64",
        }
        for target, platform in cases.items():
            with self.subTest(target=target):
                self.assertEqual(licenses.wheel_platform_for_target(target), platform)

    def test_wheel_target_rejects_incompatible_or_ambiguous_tags(self) -> None:
        for tag in (
            "py3-none-musllinux_1_2_x86_64",
            "py3-none-musllinux_1_2_aarch64",
            "py3-none-manylinux_2_28_x86_64.linux_aarch64",
            "cp314-cp314-manylinux_2_28_x86_64",
            "py3-none-win_arm64",
        ):
            with self.subTest(tag=tag), self.assertRaises(licenses.PythonWheelLicenseError):
                licenses.wheel_target(Path(f"merman-0.9.3-{tag}.whl"))

    def test_release_set_verifies_every_target_identity_and_embedded_metadata(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            wheels = release_wheels(root)
            self.assertEqual(
                set(licenses.verify_wheel_set(wheels, root=root, expected_targets=RELEASE_TAGS)),
                set(RELEASE_TAGS),
            )

    def test_release_set_rejects_same_count_with_a_duplicate_and_missing_target(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            wheels = release_wheels(root)
            wheels[-1] = write_wheel(root, next(iter(RELEASE_TAGS)), version="0.9.4")
            self.assertEqual(len({wheel.name for wheel in wheels}), 5)
            with self.assertRaisesRegex(licenses.PythonWheelLicenseError, "duplicate.*target"):
                licenses.verify_wheel_set(wheels, root=root, expected_targets=RELEASE_TAGS)

    def test_release_set_rejects_missing_target(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            wheels = release_wheels(root)[:-1]
            with self.assertRaisesRegex(licenses.PythonWheelLicenseError, "missing.*x86_64"):
                licenses.verify_wheel_set(wheels, root=root, expected_targets=RELEASE_TAGS)

    def test_release_set_rejects_a_linux_tag_outside_the_declared_floor(self) -> None:
        for platform in ("linux_aarch64", "manylinux_2_17_aarch64", "manylinux_2_34_aarch64"):
            with self.subTest(platform=platform), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                wheels = release_wheels(root, {"aarch64-unknown-linux-gnu": platform})
                with self.assertRaisesRegex(licenses.PythonWheelLicenseError, "manylinux_2_28"):
                    licenses.verify_wheel_set(wheels, root=root, expected_targets=RELEASE_TAGS)

    def test_release_set_rejects_mismatched_wheel_tags_or_package_versions(self) -> None:
        for overrides in (
            {"wheel_tag": "py3-none-win_amd64"},
            {"metadata_version": "0.9.2"},
            {"version": "0.9.2"},
            {"metadata_name": "another-package"},
            {"requires_python": ">=3.10"},
            {"root_is_purelib": "true"},
        ):
            with self.subTest(overrides=overrides), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                wheels = release_wheels(root)
                wheels[0] = write_wheel(root, next(iter(RELEASE_TAGS)), **overrides)
                with self.assertRaises(licenses.PythonWheelLicenseError):
                    licenses.verify_wheel_set(wheels, root=root, expected_targets=RELEASE_TAGS)

    def test_wheel_target_rejects_a_universal2_tag_for_a_single_target_build(self) -> None:
        wheel = Path("merman-0.8.0a6-py3-none-macosx_11_0_universal2.whl")
        with self.assertRaisesRegex(
            licenses.PythonWheelLicenseError,
            "cannot map Python wheel platform tag",
        ):
            licenses.wheel_target(wheel)

    def test_install_and_wheel_verification_use_the_same_target_report_bytes(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            target = "x86_64-unknown-linux-gnu"
            report = target_report(target)
            encoded = (json.dumps(report, indent=2) + "\n").encode()
            report_path = licenses.target_report_path(root, target)
            report_path.parent.mkdir(parents=True)
            report_path.write_bytes(encoded)
            package = root / "package"

            licenses.install_target_report(root, package, target)

            self.assertEqual((package / licenses.PACKAGE_REPORT).read_bytes(), encoded)
            wheel = root / "merman-0.8.0a4-py3-none-linux_x86_64.whl"
            with zipfile.ZipFile(wheel, "w") as archive:
                archive.writestr(
                    "merman-0.8.0a4.dist-info/licenses/"
                    + licenses.PACKAGE_REPORT.as_posix(),
                    encoded,
                )
            self.assertEqual(
                licenses.verify_wheel_license_report(wheel, root=root),
                target,
            )

    def test_validation_rejects_a_union_or_mismatched_target(self) -> None:
        target = "aarch64-apple-darwin"
        report = target_report(target)
        report["artifact_bundle"]["target_observations"].append(
            {
                "artifact_profile_id": licenses.PYTHON_ARTIFACT_PROFILE_ID,
                "target": "x86_64-apple-darwin",
            }
        )
        with self.assertRaisesRegex(
            licenses.PythonWheelLicenseError,
            "only target",
        ):
            licenses.validate_target_report(report, target)

    def test_wheel_verification_rejects_noncanonical_report_bytes(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            target = "x86_64-pc-windows-msvc"
            report = target_report(target)
            canonical = (json.dumps(report, indent=2) + "\n").encode()
            report_path = licenses.target_report_path(root, target)
            report_path.parent.mkdir(parents=True)
            report_path.write_bytes(canonical)
            wheel = root / "merman-0.8.0a4-py3-none-win_amd64.whl"
            with zipfile.ZipFile(wheel, "w") as archive:
                archive.writestr(
                    "merman-0.8.0a4.dist-info/licenses/"
                    + licenses.PACKAGE_REPORT.as_posix(),
                    json.dumps(report, separators=(",", ":")).encode(),
                )

            with self.assertRaisesRegex(
                licenses.PythonWheelLicenseError,
                "does not embed the checked-in",
            ):
                licenses.verify_wheel_license_report(wheel, root=root)


def target_report(target: str) -> dict:
    closure = {
        "package_count": 1,
        "packages_sha256": "0" * 64,
    }
    return {
        "schema_version": 3,
        "artifact_bundle": {
            "id": f"python-wheel-{target}",
            "artifact_profiles": [
                {"id": licenses.PYTHON_ARTIFACT_PROFILE_ID}
            ],
            "target_observations": [
                {
                    "artifact_profile_id": licenses.PYTHON_ARTIFACT_PROFILE_ID,
                    "target": target,
                }
            ],
        },
        "generator": {"command_profile": "artifact-profile-target"},
        "target_dependency_closures": [
            {
                "artifact_profile_id": licenses.PYTHON_ARTIFACT_PROFILE_ID,
                "target": target,
                **closure,
            }
        ],
        "dependency_closure": closure,
        "licenses": [{"id": "MIT"}],
    }


RELEASE_TAGS = {
    "aarch64-apple-darwin": "macosx_11_0_arm64",
    "aarch64-unknown-linux-gnu": "manylinux_2_28_aarch64",
    "x86_64-apple-darwin": "macosx_10_13_x86_64",
    "x86_64-pc-windows-msvc": "win_amd64",
    "x86_64-unknown-linux-gnu": "manylinux_2_28_x86_64",
}


def write_wheel(
    root: Path,
    target: str,
    *,
    platform: str | None = None,
    version: str = "0.9.3",
    metadata_version: str | None = None,
    wheel_tag: str | None = None,
    metadata_name: str = "merman",
    requires_python: str = ">=3.9",
    root_is_purelib: str = "false",
) -> Path:
    encoded = (json.dumps(target_report(target), indent=2) + "\n").encode()
    report_path = licenses.target_report_path(root, target)
    report_path.parent.mkdir(parents=True, exist_ok=True)
    report_path.write_bytes(encoded)
    platform = platform or RELEASE_TAGS[target]
    wheel = root / f"merman-{version}-py3-none-{platform}.whl"
    dist_info = f"merman-{version}.dist-info"
    with zipfile.ZipFile(wheel, "w") as archive:
        archive.writestr(f"{dist_info}/licenses/{licenses.PACKAGE_REPORT.as_posix()}", encoded)
        archive.writestr(
            f"{dist_info}/WHEEL",
            f"Wheel-Version: 1.0\nRoot-Is-Purelib: {root_is_purelib}\n"
            f"Tag: {wheel_tag or f'py3-none-{platform}'}\n",
        )
        archive.writestr(
            f"{dist_info}/METADATA",
            f"Metadata-Version: 2.4\nName: {metadata_name}\nVersion: {metadata_version or version}\n"
            f"Requires-Python: {requires_python}\n",
        )
    return wheel


def release_wheels(root: Path, platforms: dict[str, str] | None = None) -> list[Path]:
    return [
        write_wheel(root, target, platform=(platforms or {}).get(target))
        for target in RELEASE_TAGS
    ]


if __name__ == "__main__":
    unittest.main()
