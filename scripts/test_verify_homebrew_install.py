#!/usr/bin/env python3
"""Tests for version-gated Homebrew installation verification."""

from __future__ import annotations

import gzip
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))

import verify_homebrew_install as verifier
import verify_cli_installation as installation_verifier
from ascii_capability_contract import canonical_ascii_capabilities


ROOT = Path(__file__).resolve().parents[1]
SUPPORT_ASSETS_SINCE = "0.8.0"


def ascii_capabilities_contract() -> dict[str, object]:
    return canonical_ascii_capabilities()


class HomebrewInstallVerifierTests(unittest.TestCase):
    def test_contract_five_retains_the_rustdoc_command_and_manpages(self) -> None:
        self.assertEqual(verifier.CLI_CONTRACT_VERSION, 5)
        self.assertIn("rustdoc", verifier.COMMANDS)
        self.assertEqual(len(verifier.MANPAGE_NAMES), 15)
        self.assertTrue(
            {
                "merman-cli-rustdoc.1",
                "merman-cli-rustdoc-build.1",
                "merman-cli-rustdoc-check.1",
            }.issubset(verifier.MANPAGE_NAMES)
        )

    def test_versions_before_threshold_keep_the_binary_only_contract(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.assertFalse(
                verifier.verify_homebrew_install(
                    formula_version="0.7.99",
                    support_assets_since=SUPPORT_ASSETS_SINCE,
                    prefix=root / "missing",
                    binary=root / "missing/merman-cli",
                    contract_root=root / "missing-release-source",
                )
            )

    def test_threshold_and_future_versions_require_support_assets(self) -> None:
        for version in ("0.8.0", "0.10.0", "1.0.0"):
            with self.subTest(version=version):
                self.assertTrue(
                    verifier.requires_support_assets(version, SUPPORT_ASSETS_SINCE)
                )

    def test_invalid_or_prerelease_formula_versions_fail_closed(self) -> None:
        for version in ("0.8", "v0.8.0", "0.8.0-alpha.1", "0.8.0+local"):
            with self.subTest(version=version), self.assertRaises(
                verifier.HomebrewVerificationError
            ):
                verifier.requires_support_assets(version, SUPPORT_ASSETS_SINCE)

    def test_version_owned_verifier_is_required_at_the_asset_threshold(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            fallback = root / "current/verify_homebrew_install.py"
            with self.assertRaisesRegex(
                verifier.HomebrewVerificationError,
                "does not contain its Homebrew verifier",
            ):
                verifier.select_version_verifier(
                    formula_version="0.8.0",
                    support_assets_since=SUPPORT_ASSETS_SINCE,
                    contract_root=root / "release",
                    fallback=fallback,
                )

    def test_legacy_version_uses_the_current_compatibility_verifier(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            fallback = root / "current/verify_homebrew_install.py"
            self.assertEqual(
                verifier.select_version_verifier(
                    formula_version="0.7.0",
                    support_assets_since=SUPPORT_ASSETS_SINCE,
                    contract_root=root / "release",
                    fallback=fallback,
                ),
                fallback,
            )

    def test_release_uses_its_version_owned_verifier(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            candidate = root / "release/scripts/verify_homebrew_install.py"
            candidate.parent.mkdir(parents=True)
            candidate.write_text("# version-owned verifier\n", encoding="utf-8")
            self.assertEqual(
                verifier.select_version_verifier(
                    formula_version="0.8.0",
                    support_assets_since=SUPPORT_ASSETS_SINCE,
                    contract_root=root / "release",
                    fallback=root / "current/verify_homebrew_install.py",
                ),
                candidate,
            )

    def test_complete_formula_matches_runtime_and_profile_contract(self) -> None:
        with self.installation_fixture() as fixture:
            self.assertTrue(
                verifier.verify_homebrew_install(
                    formula_version="0.8.0",
                    support_assets_since=SUPPORT_ASSETS_SINCE,
                    prefix=fixture.prefix,
                    binary=fixture.binary,
                    contract_root=fixture.contract_root,
                    runner=fixture.run,
                )
            )

    def test_generic_installation_contract_checks_all_nix_completion_paths(self) -> None:
        with self.installation_fixture() as fixture:
            for shell, relative in installation_verifier.NIX_COMPLETION_PATHS.items():
                path = fixture.prefix / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(fixture.completion(shell))
            fixture.gzip_manpages()

            installation_verifier.verify_cli_installation(
                package_version="0.8.0",
                prefix=fixture.prefix,
                binary=fixture.binary,
                contract_root=fixture.contract_root,
                completion_layout="nix",
                runner=fixture.run,
            )

    def test_generic_installation_contract_rejects_duplicate_manpage_encodings(self) -> None:
        with self.installation_fixture() as fixture:
            name = installation_verifier.MANPAGE_NAMES[0]
            source = fixture.prefix / "share/man/man1" / name
            source.with_name(f"{name}.gz").write_bytes(gzip.compress(source.read_bytes()))

            with self.assertRaisesRegex(
                installation_verifier.CliInstallationError,
                "compressed and uncompressed copies",
            ):
                installation_verifier.verify_cli_installation(
                    package_version="0.8.0",
                    prefix=fixture.prefix,
                    binary=fixture.binary,
                    contract_root=fixture.contract_root,
                    completion_layout="homebrew",
                    runner=fixture.run,
                )

    def test_generic_installation_contract_requires_nix_elvish_completion(self) -> None:
        with self.installation_fixture() as fixture:
            for shell, relative in installation_verifier.NIX_COMPLETION_PATHS.items():
                if shell != "elvish":
                    path = fixture.prefix / relative
                    path.parent.mkdir(parents=True, exist_ok=True)
                    path.write_bytes(fixture.completion(shell))

            with self.assertRaisesRegex(
                installation_verifier.CliInstallationError,
                "missing installed elvish completion",
            ):
                installation_verifier.verify_cli_installation(
                    package_version="0.8.0",
                    prefix=fixture.prefix,
                    binary=fixture.binary,
                    contract_root=fixture.contract_root,
                    completion_layout="nix",
                    runner=fixture.run,
                )

    def test_generic_installation_contract_rejects_unknown_layouts(self) -> None:
        with self.installation_fixture() as fixture, self.assertRaisesRegex(
            installation_verifier.CliInstallationError,
            "unsupported completion layout",
        ):
            installation_verifier.verify_cli_installation(
                package_version="0.8.0",
                prefix=fixture.prefix,
                binary=fixture.binary,
                contract_root=fixture.contract_root,
                completion_layout="custom",  # type: ignore[arg-type]
                runner=fixture.run,
            )

    def test_homebrew_opt_prefix_symlink_is_supported(self) -> None:
        with self.installation_fixture() as fixture:
            opt_prefix = Path(fixture.temporary.name) / "opt/merman-cli"
            opt_prefix.parent.mkdir()
            opt_prefix.symlink_to(fixture.prefix, target_is_directory=True)
            self.assertTrue(
                verifier.verify_homebrew_install(
                    formula_version="0.8.0",
                    support_assets_since=SUPPORT_ASSETS_SINCE,
                    prefix=opt_prefix,
                    binary=opt_prefix / "bin/merman-cli",
                    contract_root=fixture.contract_root,
                    runner=fixture.run,
                )
            )

    def test_missing_support_asset_is_rejected_at_threshold(self) -> None:
        with self.installation_fixture() as fixture:
            (fixture.prefix / verifier.COMPLETION_PATHS["fish"]).unlink()
            with self.assertRaisesRegex(
                verifier.HomebrewVerificationError,
                "missing installed fish completion",
            ):
                verifier.verify_homebrew_install(
                    formula_version="0.8.0",
                    support_assets_since=SUPPORT_ASSETS_SINCE,
                    prefix=fixture.prefix,
                    binary=fixture.binary,
                    contract_root=fixture.contract_root,
                    runner=fixture.run,
                )

    def test_completion_must_match_the_installed_binary(self) -> None:
        with self.installation_fixture() as fixture:
            (fixture.prefix / verifier.COMPLETION_PATHS["zsh"]).write_bytes(b"stale\n")
            with self.assertRaisesRegex(
                verifier.HomebrewVerificationError,
                "zsh completion differs",
            ):
                verifier.verify_homebrew_install(
                    formula_version="0.8.0",
                    support_assets_since=SUPPORT_ASSETS_SINCE,
                    prefix=fixture.prefix,
                    binary=fixture.binary,
                    contract_root=fixture.contract_root,
                    runner=fixture.run,
                )

    def test_all_fifteen_man_pages_are_required(self) -> None:
        with self.installation_fixture() as fixture:
            (fixture.prefix / "share/man/man1/merman-cli-rustdoc-check.1").unlink()
            with self.assertRaisesRegex(
                verifier.HomebrewVerificationError,
                "man page set differs",
            ):
                verifier.verify_homebrew_install(
                    formula_version="0.8.0",
                    support_assets_since=SUPPORT_ASSETS_SINCE,
                    prefix=fixture.prefix,
                    binary=fixture.binary,
                    contract_root=fixture.contract_root,
                    runner=fixture.run,
                )

    def test_capability_sets_are_exact(self) -> None:
        with self.installation_fixture() as fixture:
            fixture.capabilities["commands"].append("future-command")
            with self.assertRaisesRegex(
                verifier.HomebrewVerificationError,
                "command set differs",
            ):
                verifier.verify_homebrew_install(
                    formula_version="0.8.0",
                    support_assets_since=SUPPORT_ASSETS_SINCE,
                    prefix=fixture.prefix,
                    binary=fixture.binary,
                    contract_root=fixture.contract_root,
                    runner=fixture.run,
                )

    def test_ascii_capability_subcontract_is_exact(self) -> None:
        for mutation in ("missing_encodings", "missing_swimlane", "extra_field"):
            with self.subTest(mutation=mutation):
                with self.installation_fixture() as fixture:
                    ascii_contract = fixture.capabilities["ascii"]
                    if mutation == "missing_encodings":
                        flowchart = next(
                            family
                            for family in ascii_contract["families"]
                            if family["family"] == "flowchart"
                        )
                        flowchart.pop("encodings")
                    elif mutation == "missing_swimlane":
                        ascii_contract["detected_type_mappings"] = [
                            mapping
                            for mapping in ascii_contract["detected_type_mappings"]
                            if mapping["detected_type"] != "swimlane"
                        ]
                    else:
                        ascii_contract["families"][0]["unknown"] = None

                    with self.assertRaisesRegex(
                        verifier.HomebrewVerificationError,
                        "ASCII",
                    ):
                        verifier.verify_homebrew_install(
                            formula_version="0.8.0",
                            support_assets_since=SUPPORT_ASSETS_SINCE,
                            prefix=fixture.prefix,
                            binary=fixture.binary,
                            contract_root=fixture.contract_root,
                            runner=fixture.run,
                        )

    def test_capability_and_output_sets_reject_drift_and_duplicates(self) -> None:
        cases = (
            ("capabilities", "extra", "capability set differs"),
            ("capabilities", "missing", "capability set differs"),
            ("capabilities", "duplicate", "must not contain duplicates"),
            ("outputs", "extra", "output set differs"),
            ("outputs", "missing", "output set differs"),
            ("outputs", "duplicate", "must not contain duplicates"),
        )
        for field, mutation, error in cases:
            with self.subTest(field=field, mutation=mutation):
                with self.installation_fixture() as fixture:
                    entries = fixture.capabilities[field]
                    if mutation == "extra":
                        entries.append({"id": "future-surface"})
                    elif mutation == "missing":
                        entries.pop()
                    else:
                        entries.append(dict(entries[0]))
                    with self.assertRaisesRegex(
                        verifier.HomebrewVerificationError,
                        error,
                    ):
                        verifier.verify_homebrew_install(
                            formula_version="0.8.0",
                            support_assets_since=SUPPORT_ASSETS_SINCE,
                            prefix=fixture.prefix,
                            binary=fixture.binary,
                            contract_root=fixture.contract_root,
                            runner=fixture.run,
                        )

    def test_capabilities_package_version_must_match_formula(self) -> None:
        with self.installation_fixture() as fixture:
            fixture.capabilities["package"]["version"] = "0.8.1"
            with self.assertRaisesRegex(
                verifier.HomebrewVerificationError,
                "package identity does not match",
            ):
                verifier.verify_homebrew_install(
                    formula_version="0.8.0",
                    support_assets_since=SUPPORT_ASSETS_SINCE,
                    prefix=fixture.prefix,
                    binary=fixture.binary,
                    contract_root=fixture.contract_root,
                    runner=fixture.run,
                )

    def test_capabilities_descriptor_must_match_release_authority(self) -> None:
        cases = (
            ({"digest": "sha256:" + "f" * 64}, "descriptor does not match"),
            ({"schema_version": 99}, "descriptor does not match"),
        )
        for mutation, error in cases:
            with self.subTest(mutation=mutation):
                with self.installation_fixture() as fixture:
                    fixture.capabilities["descriptor"].update(mutation)
                    with self.assertRaisesRegex(
                        verifier.HomebrewVerificationError,
                        error,
                    ):
                        verifier.verify_homebrew_install(
                            formula_version="0.8.0",
                            support_assets_since=SUPPORT_ASSETS_SINCE,
                            prefix=fixture.prefix,
                            binary=fixture.binary,
                            contract_root=fixture.contract_root,
                            runner=fixture.run,
                        )

    def test_manpage_header_matches_each_asset_and_formula_version(self) -> None:
        cases = (
            ({"title": "MERMAN-CLI-WRONG"}, "title or section"),
            ({"version": "0.8.1"}, "version or manual name"),
            ({"published": "2026-02-30"}, "date is invalid"),
        )
        for overrides, error in cases:
            with self.subTest(overrides=overrides):
                with self.installation_fixture() as fixture:
                    fixture.write_manpage("merman-cli-render.1", **overrides)
                    with self.assertRaisesRegex(
                        verifier.HomebrewVerificationError,
                        error,
                    ):
                        verifier.verify_homebrew_install(
                            formula_version="0.8.0",
                            support_assets_since=SUPPORT_ASSETS_SINCE,
                            prefix=fixture.prefix,
                            binary=fixture.binary,
                            contract_root=fixture.contract_root,
                            runner=fixture.run,
                        )

    def test_installed_manpage_body_must_match_the_release_source_asset(self) -> None:
        with self.installation_fixture() as fixture:
            path = fixture.prefix / "share/man/man1/merman-cli-rustdoc-build.1"
            path.write_bytes(path.read_bytes() + b".SH STALE\nstale body\n")

            with self.assertRaisesRegex(
                verifier.HomebrewVerificationError,
                "differs from release source asset",
            ):
                verifier.verify_homebrew_install(
                    formula_version="0.8.0",
                    support_assets_since=SUPPORT_ASSETS_SINCE,
                    prefix=fixture.prefix,
                    binary=fixture.binary,
                    contract_root=fixture.contract_root,
                    runner=fixture.run,
                )

    def test_checked_manpage_inventory_matches_the_source_assets(self) -> None:
        observed = {
            path.name for path in (ROOT / "crates/merman-cli/assets/man").glob("*.1")
        }
        self.assertEqual(observed, set(verifier.MANPAGE_NAMES))

    def installation_fixture(self):
        return InstallationFixture()


class InstallationFixture:
    def __init__(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.contract_root = Path(self.temporary.name) / "release-source"
        descriptor = self.contract_root / installation_verifier.PROFILE_DESCRIPTOR
        descriptor.parent.mkdir(parents=True)
        descriptor.write_bytes(
            (ROOT / installation_verifier.PROFILE_DESCRIPTOR).read_bytes()
        )
        self.prefix = Path(self.temporary.name) / "Cellar/merman-cli/0.8.0"
        self.binary = self.prefix / "bin/merman-cli"
        self.binary.parent.mkdir(parents=True)
        self.binary.write_bytes(b"synthetic executable")
        for shell, relative in verifier.COMPLETION_PATHS.items():
            path = self.prefix / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(self.completion(shell))
        man_root = self.prefix / "share/man/man1"
        man_root.mkdir(parents=True)
        source_man_root = self.contract_root / "crates/merman-cli/assets/man"
        source_man_root.mkdir(parents=True)
        for name in verifier.MANPAGE_NAMES:
            contents = self.manpage_contents(name)
            (man_root / name).write_text(contents, encoding="utf-8")
            (source_man_root / name).write_text(contents, encoding="utf-8")

        profile, authority = verifier._read_release_contract(self.contract_root)
        self.capabilities = {
            "schema_version": verifier.CAPABILITIES_SCHEMA_VERSION,
            "cli_contract_version": verifier.CLI_CONTRACT_VERSION,
            "package": {"name": "merman-cli", "version": "0.8.0"},
            "compatibility": {"mermaid": "11.16.0", "mmdc": "11.16.0"},
            "descriptor": {
                "schema_version": authority["schema_version"],
                "digest": authority["digest"],
            },
            "commands": list(verifier.COMMANDS),
            "capabilities": [
                {"id": identifier}
                for identifier in profile["expected"]["capabilities"]
            ],
            "outputs": [
                {"id": identifier} for identifier in profile["expected"]["outputs"]
            ],
            "ascii": ascii_capabilities_contract(),
        }

    def __enter__(self):
        return self

    def __exit__(self, *_args):
        self.temporary.cleanup()

    @staticmethod
    def completion(shell: str) -> bytes:
        return f"completion:{shell}\n".encode()

    def write_manpage(
        self,
        name: str,
        *,
        title: str | None = None,
        version: str = "0.8.0",
        published: str = "2026-07-29",
    ) -> None:
        contents = self.manpage_contents(
            name,
            title=title,
            version=version,
            published=published,
        )
        (self.prefix / "share/man/man1" / name).write_text(contents, encoding="utf-8")

    @staticmethod
    def manpage_contents(
        name: str,
        *,
        title: str | None = None,
        version: str = "0.8.0",
        published: str = "2026-07-29",
    ) -> str:
        title = title or name.removesuffix(".1").upper()
        return (
            ".ie \\n(.g .ds Aq \\(aq\n"
            ".el .ds Aq '\n"
            f'.TH {title} 1 {published} "Merman {version}" "Merman CLI Manual"\n'
            ".SH NAME\n"
            f"{name.removesuffix('.1')} - test\n"
        )

    def gzip_manpages(self) -> None:
        for name in verifier.MANPAGE_NAMES:
            path = self.prefix / "share/man/man1" / name
            path.with_name(f"{name}.gz").write_bytes(gzip.compress(path.read_bytes()))
            path.unlink()

    def run(self, command: list[str], **_kwargs) -> subprocess.CompletedProcess[bytes]:
        if command[1:] == ["capabilities", "--json"]:
            stdout = json.dumps(self.capabilities).encode()
        elif len(command) == 3 and command[1] == "completion":
            stdout = self.completion(command[2])
        else:
            raise AssertionError(f"unexpected command: {command}")
        return subprocess.CompletedProcess(command, 0, stdout=stdout, stderr=b"")


if __name__ == "__main__":
    unittest.main()
