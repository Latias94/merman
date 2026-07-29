#!/usr/bin/env python3
"""Tests for the CLI binary-installation contract."""

from __future__ import annotations

from collections.abc import Iterator
from contextlib import contextmanager
from pathlib import Path
import re
import shutil
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))

import cli_installation_contract as contract


ROOT = Path(__file__).resolve().parents[1]


class CliInstallationContractTests(unittest.TestCase):
    def test_every_cli_release_target_resolves_to_the_cargo_dist_archive(self) -> None:
        artifacts = contract.validate_repository_contract(ROOT)
        workspace = contract._read_toml(ROOT, Path("Cargo.toml"))
        version = workspace["workspace"]["package"]["version"]

        self.assertEqual(
            {artifact.target for artifact in artifacts},
            {
                "aarch64-apple-darwin",
                "x86_64-apple-darwin",
                "x86_64-pc-windows-msvc",
                "x86_64-unknown-linux-gnu",
            },
        )
        for artifact in artifacts:
            with self.subTest(target=artifact.target):
                self.assertEqual(
                    artifact.url,
                    "https://github.com/Latias94/merman/releases/download/"
                    f"v{version}/{artifact.archive_name}",
                )

        windows = next(
            artifact
            for artifact in artifacts
            if artifact.target == "x86_64-pc-windows-msvc"
        )
        self.assertEqual(windows.package_format, "zip")
        self.assertEqual(windows.binary_path, "merman-cli.exe")
        for artifact in artifacts:
            if artifact is windows:
                continue
            with self.subTest(target=artifact.target):
                self.assertEqual(artifact.package_format, "txz")
                self.assertEqual(
                    artifact.binary_path,
                    f"merman-cli-{artifact.target}/merman-cli",
                )

    def test_source_fallback_is_preserved_but_quick_install_is_disabled(self) -> None:
        manifest = contract._read_toml(ROOT, contract.CLI_MANIFEST)
        disabled = manifest["package"]["metadata"]["binstall"][
            "disabled-strategies"
        ]

        self.assertEqual(disabled, ["quick-install"])
        self.assertNotIn("compile", disabled)

    def test_compile_strategy_cannot_be_disabled(self) -> None:
        with self.mutated_repository(
            "crates/merman-cli/Cargo.toml",
            '["quick-install"]',
            '["quick-install", "compile"]',
        ) as root:
            with self.assertRaisesRegex(
                contract.InstallationContractError,
                "third-party quick-install artifacts only",
            ):
                contract.validate_repository_contract(root)

    def test_archive_url_cannot_drift_from_cargo_dist(self) -> None:
        with self.mutated_repository(
            "crates/merman-cli/Cargo.toml",
            "{ name }-{ target }{ archive-suffix }",
            "{ name }-{ version }-{ target }{ archive-suffix }",
        ) as root:
            with self.assertRaisesRegex(
                contract.InstallationContractError,
                "cargo-binstall URL",
            ):
                contract.validate_repository_contract(root)

    def test_windows_binary_must_remain_flat(self) -> None:
        with self.mutated_repository(
            "crates/merman-cli/Cargo.toml",
            'bin-dir = "{ bin }{ binary-ext }"',
            'bin-dir = "{ name }-{ target }/{ bin }{ binary-ext }"',
        ) as root:
            with self.assertRaisesRegex(
                contract.InstallationContractError,
                "cargo-binstall binary for x86_64-pc-windows-msvc",
            ):
                contract.validate_repository_contract(root)

    def test_dist_and_profile_target_sets_cannot_diverge(self) -> None:
        with self.mutated_repository(
            "dist-workspace.toml",
            ', "x86_64-pc-windows-msvc"',
            "",
        ) as root:
            with self.assertRaisesRegex(
                contract.InstallationContractError,
                "same target set",
            ):
                contract.validate_repository_contract(root)

    def test_cargo_dist_archive_formats_are_explicit(self) -> None:
        for before, after, message in (
            ('unix-archive = ".tar.xz"', 'unix-archive = ".tar.gz"', "Unix archive"),
            (
                'windows-archive = ".zip"',
                'windows-archive = ".tar.xz"',
                "Windows archive",
            ),
        ):
            with self.subTest(before=before), self.mutated_repository(
                "dist-workspace.toml",
                before,
                after,
            ) as root, self.assertRaisesRegex(
                contract.InstallationContractError,
                message,
            ):
                contract.validate_repository_contract(root)

    def test_default_dist_and_release_features_cannot_diverge(self) -> None:
        with self.mutated_repository(
            "crates/merman-cli/Cargo.toml",
            'default = [\n    "analysis",',
            "default = [",
        ) as root:
            with self.assertRaisesRegex(
                contract.InstallationContractError,
                "same complete feature set",
            ):
                contract.validate_repository_contract(root)

    def test_release_recipe_must_use_the_cargo_dist_profile(self) -> None:
        with self.mutated_repository(
            "capabilities/artifact-profiles-v1.json",
            '"profile": "dist"',
            '"profile": "release"',
        ) as root:
            with self.assertRaisesRegex(
                contract.InstallationContractError,
                "must use Cargo's dist profile",
            ):
                contract.validate_repository_contract(root)

    def test_malformed_template_fails_closed(self) -> None:
        with self.mutated_repository(
            "crates/merman-cli/Cargo.toml",
            "{ name }-{ target }{ archive-suffix }",
            "{ name }-{ unsupported }{ archive-suffix }",
        ) as root:
            with self.assertRaisesRegex(
                contract.InstallationContractError,
                "unsupported template variable",
            ):
                contract.validate_repository_contract(root)

    def test_homebrew_guidance_uses_the_exact_release_features(self) -> None:
        profile = contract._cli_release_profile(ROOT)
        document = (ROOT / "docs/releasing/CLI.md").read_text(encoding="utf-8")
        match = re.search(r"  features = %w\[\n(?P<features>.*?)\n  \]", document, re.DOTALL)
        self.assertIsNotNone(match)
        assert match is not None
        self.assertEqual(
            match.group("features").split(),
            profile["cargo"]["features"],
        )
        for contract_text in (
            '"--no-default-features"',
            "bash_completion.install",
            "zsh_completion.install",
            "fish_completion.install",
            "pwsh_completion.install",
            "man1.install",
        ):
            with self.subTest(contract_text=contract_text):
                self.assertIn(contract_text, document)

    @contextmanager
    def mutated_repository(
        self,
        relative: str,
        before: str,
        after: str,
    ) -> Iterator[Path]:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for source_relative in (
                "Cargo.toml",
                "dist-workspace.toml",
                "crates/merman-cli/Cargo.toml",
                "capabilities/artifact-profiles-v1.json",
            ):
                destination = root / source_relative
                destination.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(ROOT / source_relative, destination)
            path = root / relative
            text = path.read_text(encoding="utf-8")
            self.assertIn(before, text)
            path.write_text(text.replace(before, after, 1), encoding="utf-8")
            yield root


if __name__ == "__main__":
    unittest.main()
