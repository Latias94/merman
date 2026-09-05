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
            "merman-0.8.0a4-py3-none-win_amd64.whl": "x86_64-pc-windows-msvc",
            "merman-0.8.0a4-py3-none-manylinux_2_17_x86_64.whl": (
                "x86_64-unknown-linux-gnu"
            ),
        }
        for wheel, target in cases.items():
            with self.subTest(wheel=wheel):
                self.assertEqual(licenses.wheel_target(Path(wheel)), target)

    def test_wheel_platform_for_target_matches_single_target_wheel_tags(self) -> None:
        cases = {
            "aarch64-apple-darwin": "macosx-11.0-arm64",
            "x86_64-pc-windows-msvc": "win-amd64",
            "x86_64-unknown-linux-gnu": "linux-x86_64",
        }
        for target, platform in cases.items():
            with self.subTest(target=target):
                self.assertEqual(licenses.wheel_platform_for_target(target), platform)

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


if __name__ == "__main__":
    unittest.main()
