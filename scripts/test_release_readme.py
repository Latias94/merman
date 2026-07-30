#!/usr/bin/env python3
"""Tests for README installation-command projections."""

from __future__ import annotations

import re
import tomllib
import unittest
from pathlib import Path

from scripts import release_readme
from scripts.release_version import parse_release_version


ROOT = Path(__file__).resolve().parents[1]
REPOSITORY_URL = "https://github.com/Latias94/merman"
TOML_FENCE = re.compile(r"```toml\n(?P<body>.*?)\n```", flags=re.DOTALL)


def render_document(path: str, mode: str) -> str:
    text = (ROOT / path).read_text(encoding="utf-8")
    release = parse_release_version("0.8.0-alpha.4")
    if path == release_readme.ROOT_README:
        return release_readme.render_readme(
            text,
            release,
            mode,
            REPOSITORY_URL,
        )
    return release_readme.render_projected_readme(
        path,
        text,
        release,
        mode,
        REPOSITORY_URL,
    )


def verify_document(path: str, text: str, mode: str) -> None:
    release = parse_release_version("0.8.0-alpha.4")
    if path == release_readme.ROOT_README:
        release_readme.verify_readme(
            text,
            release,
            mode=mode,
            repository_url=REPOSITORY_URL,
        )
    else:
        release_readme.verify_projected_readme(
            path,
            text,
            release,
            mode=mode,
            repository_url=REPOSITORY_URL,
        )


def generated_blocks(path: str, text: str, mode: str) -> tuple[str, ...]:
    release = parse_release_version("0.8.0-alpha.4")
    block_ids = release_readme._expected_blocks(
        path,
        release,
        mode,
        REPOSITORY_URL,
    )
    return tuple(
        release_readme._read_block(text, block_id)
        for block_id in block_ids
    )


class ReleaseReadmeTests(unittest.TestCase):
    release = parse_release_version("0.8.0-alpha.4")

    def test_every_readme_verifies_and_round_trips_between_modes(self) -> None:
        for path in (
            release_readme.ROOT_README,
            *release_readme.projected_readme_paths(),
        ):
            with self.subTest(path=path):
                source = render_document(path, release_readme.SOURCE_MODE)
                verify_document(path, source, release_readme.SOURCE_MODE)
                registry = (
                    release_readme.render_readme(
                        source,
                        self.release,
                        release_readme.REGISTRY_MODE,
                        REPOSITORY_URL,
                    )
                    if path == release_readme.ROOT_README
                    else release_readme.render_projected_readme(
                        path,
                        source,
                        self.release,
                        release_readme.REGISTRY_MODE,
                        REPOSITORY_URL,
                    )
                )
                verify_document(path, registry, release_readme.REGISTRY_MODE)
                source_again = (
                    release_readme.render_readme(
                        registry,
                        self.release,
                        release_readme.SOURCE_MODE,
                        REPOSITORY_URL,
                    )
                    if path == release_readme.ROOT_README
                    else release_readme.render_projected_readme(
                        path,
                        registry,
                        self.release,
                        release_readme.SOURCE_MODE,
                        REPOSITORY_URL,
                    )
                )
                self.assertEqual(source_again, source)

    def test_registry_commands_pin_exact_versions(self) -> None:
        cargo_adds = 0
        cargo_installs = 0
        cargo_dependencies = 0
        npm_installs = 0

        for path in (
            release_readme.ROOT_README,
            *release_readme.projected_readme_paths(),
        ):
            registry = render_document(path, release_readme.REGISTRY_MODE)
            for body in generated_blocks(
                path,
                registry,
                release_readme.REGISTRY_MODE,
            ):
                for line in body.splitlines():
                    if line.startswith("cargo add "):
                        cargo_adds += 1
                        package = line.split()[2]
                        self.assertTrue(
                            package.endswith(f"@={self.release.canonical}"),
                            line,
                        )
                        self.assertNotIn("--git", line)
                    elif line.startswith("cargo install "):
                        cargo_installs += 1
                        self.assertIn(
                            f"--version {self.release.canonical}",
                            line,
                        )
                        self.assertIn("--locked", line)
                        self.assertNotIn("--git", line)
                    elif line.startswith("npm install "):
                        npm_installs += 1
                        self.assertRegex(
                            line,
                            rf"^npm install @mermanjs/[A-Za-z-]+@"
                            rf"{re.escape(self.release.canonical)}$",
                        )

                for match in TOML_FENCE.finditer(body):
                    document = tomllib.loads(match.group("body"))
                    for package, dependency in document.get(
                        "dependencies", {}
                    ).items():
                        cargo_dependencies += 1
                        self.assertIsInstance(dependency, dict, package)
                        self.assertEqual(
                            dependency.get("version"),
                            f"={self.release.canonical}",
                            package,
                        )
                        self.assertNotIn("git", dependency, package)

        self.assertGreater(cargo_adds, 0)
        self.assertGreater(cargo_installs, 0)
        self.assertGreater(cargo_dependencies, 0)
        self.assertGreater(npm_installs, 0)

    def test_cli_install_heading_is_valid_in_source_and_registry_modes(self) -> None:
        path = "crates/merman-cli/README.md"
        for mode in (release_readme.SOURCE_MODE, release_readme.REGISTRY_MODE):
            with self.subTest(mode=mode):
                rendered = render_document(path, mode)
                self.assertIn("Install the complete CLI from source:", rendered)
                self.assertNotIn("current repository revision", rendered)

    def test_source_commands_use_the_repository_and_keep_version_guards(self) -> None:
        cargo_adds = 0
        cargo_installs = 0
        cargo_dependencies = 0
        npm_installs = 0

        for path in (
            release_readme.ROOT_README,
            *release_readme.projected_readme_paths(),
        ):
            source = render_document(path, release_readme.SOURCE_MODE)
            for body in generated_blocks(
                path,
                source,
                release_readme.SOURCE_MODE,
            ):
                self.assertTrue(body.startswith("```"), body)
                for line in body.splitlines():
                    if line.startswith("cargo add "):
                        cargo_adds += 1
                        self.assertIn(f"--git {REPOSITORY_URL}", line)
                    elif line.startswith("cargo install "):
                        cargo_installs += 1
                        self.assertIn(f"--git {REPOSITORY_URL}", line)
                        self.assertIn("--locked", line)
                    elif line.startswith("npm install "):
                        npm_installs += 1
                        self.assertTrue(
                            line.startswith(
                                "npm install /path/to/merman/platforms/web/packages/"
                            ),
                            line,
                        )

                for match in TOML_FENCE.finditer(body):
                    document = tomllib.loads(match.group("body"))
                    for package, dependency in document.get(
                        "dependencies", {}
                    ).items():
                        cargo_dependencies += 1
                        self.assertIsInstance(dependency, dict, package)
                        self.assertEqual(
                            dependency.get("version"),
                            f"={self.release.canonical}",
                            package,
                        )
                        self.assertEqual(
                            dependency.get("git"),
                            REPOSITORY_URL,
                            package,
                        )

        self.assertGreater(cargo_adds, 0)
        self.assertGreater(cargo_installs, 0)
        self.assertGreater(cargo_dependencies, 0)
        self.assertGreater(npm_installs, 0)

    def test_expected_commands_cover_cli_lsp_and_web_render(self) -> None:
        registry_root = render_document(
            release_readme.ROOT_README,
            release_readme.REGISTRY_MODE,
        )
        registry_lsp = render_document(
            "crates/merman-lsp/README.md",
            release_readme.REGISTRY_MODE,
        )
        registry_render = render_document(
            "platforms/web/packages/render/README.md",
            release_readme.REGISTRY_MODE,
        )
        registry_flutter = render_document(
            "platforms/flutter/README.md",
            release_readme.REGISTRY_MODE,
        )
        source_flutter = render_document(
            "platforms/flutter/README.md",
            release_readme.SOURCE_MODE,
        )
        registry_raster = render_document(
            "docs/rendering/RASTER_OUTPUT.md",
            release_readme.REGISTRY_MODE,
        )

        self.assertIn(
            "cargo add merman@=0.8.0-alpha.4",
            registry_root,
        )
        self.assertIn(
            "cargo install merman-cli --version 0.8.0-alpha.4 --locked",
            registry_root,
        )
        self.assertIn(
            "merman-cli render - --output diagram.svg",
            registry_root,
        )
        self.assertNotIn(
            "merman-cli -i - -o diagram.svg",
            registry_root,
        )
        self.assertIn(
            "--no-default-features --features stdio",
            registry_lsp,
        )
        self.assertIn(
            "npm install @mermanjs/web-render@0.8.0-alpha.4",
            registry_render,
        )
        self.assertIn(
            "dependencies:\n  merman: 0.8.0-alpha.4",
            registry_flutter,
        )
        self.assertIn(
            "dependencies:\n"
            "  merman:\n"
            "    git:\n"
            f"      url: {REPOSITORY_URL}\n"
            "      path: platforms/flutter",
            source_flutter,
        )
        self.assertEqual(registry_raster.count('version = "=0.8.0-alpha.4"'), 2)
        self.assertNotIn("git =", registry_raster)

    def test_verifier_rejects_stale_missing_duplicate_and_reordered_blocks(
        self,
    ) -> None:
        source = render_document(
            release_readme.ROOT_README,
            release_readme.SOURCE_MODE,
        )
        cli_begin, cli_end = release_readme._markers("CLI")
        rust_begin, rust_end = release_readme._markers("RUST")
        cli_segment = source[
            source.index(cli_begin) : source.index(cli_end) + len(cli_end)
        ]
        rust_segment = source[
            source.index(rust_begin) : source.index(rust_end) + len(rust_end)
        ]
        cases = {
            "stale": source.replace(
                "cargo add merman --git",
                "cargo add stale --git",
                1,
            ),
            "missing": source.replace(cli_begin, "", 1),
            "duplicate": source.replace(cli_begin, f"{cli_begin}\n{cli_begin}", 1),
            "reordered": source.replace(cli_segment, "__CLI__", 1)
            .replace(rust_segment, cli_segment, 1)
            .replace("__CLI__", rust_segment, 1),
        }

        for label, text in cases.items():
            with self.subTest(case=label), self.assertRaises(
                release_readme.ReleaseReadmeError
            ):
                verify_document(
                    release_readme.ROOT_README,
                    text,
                    release_readme.SOURCE_MODE,
                )

    def test_requires_one_mode_marker_and_a_command_safe_repository_url(self) -> None:
        source = render_document(
            release_readme.ROOT_README,
            release_readme.SOURCE_MODE,
        )
        marker = "<!-- merman-release-install-mode: source -->"
        for text in (
            source.replace(marker, "", 1),
            source.replace(marker, f"{marker}\n{marker}", 1),
        ):
            with self.assertRaises(release_readme.ReleaseReadmeError):
                verify_document(
                    release_readme.ROOT_README,
                    text,
                    release_readme.SOURCE_MODE,
                )

        for repository_url in (
            "../merman",
            "https://example.invalid/repo$(touch-pwned)",
            "https://example.invalid/repo;echo-pwned",
            'https://example.invalid/"repo"',
            "https://example.invalid/repo?ref=main",
        ):
            with self.subTest(repository_url=repository_url), self.assertRaisesRegex(
                release_readme.ReleaseReadmeError,
                "absolute HTTP",
            ):
                release_readme.render_readme(
                    source,
                    self.release,
                    release_readme.SOURCE_MODE,
                    repository_url,
                )


if __name__ == "__main__":
    unittest.main()
