#!/usr/bin/env python3
"""Tests for the Cargo package legal-material verifier."""

from __future__ import annotations

import hashlib
import io
import json
import tarfile
import tempfile
import unittest
from pathlib import Path

from scripts import verify_crate_package_legal_materials as verify


class PackageLegalMaterialTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.manifest = self.root / "Cargo.toml"
        self.manifest.write_text("[package]\nname='demo'\nversion='0.1.0'\n", encoding="utf-8")

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def package(self, license_expression: str = "MIT OR Apache-2.0") -> dict[str, object]:
        return {
            "name": "demo",
            "version": "0.1.0",
            "manifest_path": str(self.manifest),
            "license": license_expression,
        }

    def archive(self, package: dict[str, object], files: dict[str, bytes]) -> Path:
        archive = self.root / f"{package['name']}.crate"
        prefix = f"{package['name']}-{package['version']}/"
        with tarfile.open(archive, "w:gz") as crate:
            for name, data in files.items():
                member = tarfile.TarInfo(prefix + name)
                member.size = len(data)
                crate.addfile(member, io.BytesIO(data))
        return archive

    def test_dual_license_package_requires_both_texts(self) -> None:
        with self.assertRaisesRegex(verify.PackageLegalMaterialError, "LICENSE-APACHE"):
            verify.verify_package_listing(self.package(), {"LICENSE-MIT"})

    def test_dual_license_package_with_bsd_code_requires_all_three_texts(self) -> None:
        package = self.package("(MIT OR Apache-2.0) AND BSD-2-Clause")
        dual_license = {"LICENSE-MIT", "LICENSE-APACHE"}
        with self.assertRaisesRegex(verify.PackageLegalMaterialError, "LICENSE-BSD-2-Clause"):
            verify.verify_package_listing(package, dual_license)
        verify.verify_package_listing(package, dual_license | {"LICENSE-BSD-2-Clause"})

    def test_apache_only_package_requires_its_upstream_license_text(self) -> None:
        package = self.package("Apache-2.0")
        self.assertEqual(verify.required_legal_paths(package), {"LICENSE"})
        verify.verify_package_listing(package, {"LICENSE", "src/lib.rs"})

    def test_krilla_requires_the_embedded_icc_profile_license(self) -> None:
        package = self.package() | {"name": "merman-krilla"}
        files = {"LICENSE-MIT", "LICENSE-APACHE", "icc/sRGB-v4.icc"}
        with self.assertRaisesRegex(verify.PackageLegalMaterialError, "icc/LICENSE"):
            verify.verify_package_listing(package, files)
        verify.verify_package_listing(package, files | {"icc/LICENSE"})

    def test_pinned_archive_licenses_require_upstream_bytes(self) -> None:
        for crate_name, expression, pinned in (
            ("resvg", "(MIT OR Apache-2.0) AND BSD-2-Clause", "LICENSE-BSD-2-Clause"),
            ("krilla", "MIT OR Apache-2.0", "icc/LICENSE"),
        ):
            with self.subTest(crate=crate_name):
                package = self.package(expression) | {"name": f"merman-{crate_name}"}
                vendor = verify.ROOT / "vendor" / crate_name
                original = {
                    path: (vendor / path).read_bytes()
                    for path in ("LICENSE-MIT", "LICENSE-APACHE", pinned)
                }
                truncated = original | {pinned: b"x"}
                for path, data in truncated.items():
                    source = self.root / path
                    source.parent.mkdir(parents=True, exist_ok=True)
                    source.write_bytes(data)
                with self.assertRaisesRegex(
                    verify.PackageLegalMaterialError, f"pinned upstream SHA-256: {pinned}$"
                ):
                    verify.verify_package_archive(package, self.archive(package, truncated), repo_root=verify.ROOT)
                verify.verify_package_archive(package, self.archive(package, original), repo_root=verify.ROOT)

    def test_archive_uses_the_candidate_checkout_license_pin(self) -> None:
        package = self.package() | {"name": "merman-krilla"}
        files = {"LICENSE-MIT": b"MIT\n", "LICENSE-APACHE": b"Apache\n", "icc/LICENSE": b"Candidate ICC terms\n"}
        for name in ("LICENSE-MIT", "LICENSE-APACHE"):
            (self.root / name).write_bytes(files[name])
        contract = self.root / "docs/release/THIRD_PARTY_COMPONENTS.json"
        contract.parent.mkdir(parents=True)
        contract.write_text(json.dumps({"components": [{"license_files": [{
            "path": "THIRD_PARTY_LICENSES/krilla/LICENSE-ICC",
            "sha256": hashlib.sha256(files["icc/LICENSE"]).hexdigest(),
        }]}]}), encoding="utf-8")
        archive = self.archive(package, files)
        verify.verify_package_archive(package, archive, repo_root=self.root)
        with self.assertRaisesRegex(verify.PackageLegalMaterialError, "pinned upstream SHA-256"):
            verify.verify_package_archive(package, archive, repo_root=verify.ROOT)

    def test_unpinned_archive_license_must_match_checkout(self) -> None:
        package = self.package("MIT")
        (self.root / "LICENSE").write_bytes(b"MIT\n")
        verify.verify_package_archive(package, self.archive(package, {"LICENSE": b"MIT\n"}), repo_root=self.root)
        with self.assertRaisesRegex(verify.PackageLegalMaterialError, "differs from source: LICENSE$"):
            verify.verify_package_archive(package, self.archive(package, {"LICENSE": b"MIT!"}), repo_root=self.root)

    def test_apache_only_package_cannot_omit_its_license_text(self) -> None:
        with self.assertRaisesRegex(
            verify.PackageLegalMaterialError, "package omits legal files: LICENSE$"
        ):
            verify.verify_package_listing(self.package("Apache-2.0"), {"src/lib.rs"})

    def test_unknown_or_missing_license_expressions_are_rejected(self) -> None:
        for expression in (None, "", "BSD-3-Clause", "Apache-2.0 OR MIT"):
            with self.subTest(expression=expression):
                package = self.package()
                package["license"] = expression
                with self.assertRaisesRegex(
                    verify.PackageLegalMaterialError,
                    "unsupported or missing license expression",
                ):
                    verify.required_legal_paths(package)

    def test_third_party_bundle_is_required_when_present(self) -> None:
        (self.root / "THIRD_PARTY_NOTICES.md").write_text("notice\n", encoding="utf-8")
        licenses = self.root / "THIRD_PARTY_LICENSES/demo"
        licenses.mkdir(parents=True)
        (licenses / "LICENSE").write_text("license\n", encoding="utf-8")
        with self.assertRaisesRegex(verify.PackageLegalMaterialError, "THIRD_PARTY_LICENSES"):
            verify.verify_package_listing(
                self.package(),
                {"LICENSE-MIT", "LICENSE-APACHE", "THIRD_PARTY_NOTICES.md"},
            )

    def test_complete_listing_passes(self) -> None:
        verify.verify_package_listing(
            self.package(),
            {"LICENSE-MIT", "LICENSE-APACHE", "src/lib.rs"},
        )

    def test_package_listing_paths_are_normalized_for_windows(self) -> None:
        (self.root / "THIRD_PARTY_NOTICES.md").write_text("notice\n", encoding="utf-8")
        licenses = self.root / "THIRD_PARTY_LICENSES/demo"
        licenses.mkdir(parents=True)
        (licenses / "LICENSE").write_text("license\n", encoding="utf-8")

        package = self.package()
        package["metadata"] = {"merman-legal": {"third-party-bundle": True}}
        verify.verify_package_listing(
            package,
            {
                "LICENSE-MIT",
                "LICENSE-APACHE",
                "THIRD_PARTY_NOTICES.md",
                r"THIRD_PARTY_LICENSES\demo\LICENSE",
            },
        )

    def test_epl_package_requires_full_epl_text(self) -> None:
        with self.assertRaisesRegex(verify.PackageLegalMaterialError, "EPL-2.0"):
            verify.verify_package_listing(self.package("EPL-2.0"), {"src/lib.rs"})

    def test_dry_run_independent_package_is_legally_governed(self) -> None:
        private_package = self.package("MIT")
        private_package["publish"] = []
        metadata = {
            "metadata": {
                "merman-release": {"independent-packages": ["demo"]},
            },
            "packages": [private_package],
        }

        self.assertEqual(verify.governed_packages(metadata), [private_package])

    def test_declared_third_party_bundle_cannot_be_omitted(self) -> None:
        private_package = self.package("MIT")
        private_package["metadata"] = {
            "merman-legal": {"third-party-bundle": True}
        }

        with self.assertRaisesRegex(
            verify.PackageLegalMaterialError,
            "incomplete third-party legal bundle",
        ):
            verify.required_legal_paths(private_package)


if __name__ == "__main__":
    unittest.main()
