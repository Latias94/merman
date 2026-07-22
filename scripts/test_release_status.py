#!/usr/bin/env python3
"""Unit tests for release surface status reporting."""

from __future__ import annotations

import contextlib
import importlib.util
import io
import json
import subprocess
import sys
import tempfile
import unittest
import urllib.error
from pathlib import Path
from unittest import mock

from scripts import release_projection


MODULE_PATH = Path(__file__).with_name("release-status.py")
SPEC = importlib.util.spec_from_file_location("release_status", MODULE_PATH)
assert SPEC is not None
release_status = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(release_status)

VERSION_SCRIPT_PATH = Path(__file__).with_name("release-version.py")
VERSION_SCRIPT_SPEC = importlib.util.spec_from_file_location("release_version_script", VERSION_SCRIPT_PATH)
assert VERSION_SCRIPT_SPEC is not None
release_version_script = importlib.util.module_from_spec(VERSION_SCRIPT_SPEC)
assert VERSION_SCRIPT_SPEC.loader is not None
VERSION_SCRIPT_SPEC.loader.exec_module(release_version_script)

SOURCE_SHA = "a" * 40


def replace_once(text: str, old: str, new: str) -> str:
    return replace_nth(text, old, new, 0)


def replace_nth(text: str, old: str, new: str, occurrence: int) -> str:
    parts = text.split(old)
    if len(parts) <= occurrence + 1:
        raise AssertionError(f"test fixture does not contain occurrence {occurrence} of {old!r}")
    return old.join(parts[: occurrence + 1]) + new + old.join(parts[occurrence + 1 :])


class ReleaseStatusVersionTests(unittest.TestCase):
    def test_release_kind_detects_stable_and_prerelease_versions(self) -> None:
        self.assertEqual(release_status.release_kind("0.8.0"), "stable")
        self.assertEqual(release_status.release_kind("v0.8.0"), "stable")
        self.assertEqual(release_status.release_kind("0.8.0-alpha.3"), "prerelease")
        self.assertEqual(release_status.release_kind("0.8.0-beta.2+sha.1"), "prerelease")
        self.assertEqual(release_status.release_kind("0.8.0+sha.1"), "stable")
        self.assertIsNone(release_status.release_kind(None))

    def test_registry_versions_share_the_canonical_parser(self) -> None:
        self.assertEqual(release_status.python_version("v0.8.0-alpha.3"), "0.8.0a3")
        self.assertEqual(release_status.python_version("0.8.0-beta.2+SHA-1"), "0.8.0b2+sha.1")
        version = release_status.parse_release_version("v0.8.0-rc.4+build.7")
        self.assertEqual(version.canonical, "0.8.0-rc.4+build.7")
        self.assertEqual(version.tag, "v0.8.0-rc.4+build.7")
        self.assertEqual(version.channel, "rc")
        self.assertEqual(version.to_npm_dist_tag(), "rc")
        self.assertEqual(
            release_status.parse_release_version("0.8.0").to_npm_dist_tag(),
            "latest",
        )

    def test_release_parser_rejects_non_contract_versions(self) -> None:
        for version in ["0.8", "01.2.3", "vv1.2.3", "0.8.0a3", "0.8.0-dev.1", "0.8.0-"]:
            with self.subTest(version=version), self.assertRaises(ValueError):
                release_status.parse_release_version(version)

    def test_manifest_check_normalizes_a_tag_prefix(self) -> None:
        version = release_version_script.cargo_workspace_version()

        stdout = io.StringIO()
        with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(io.StringIO()):
            exit_code = release_version_script.check_versions(f"v{version}")

        self.assertEqual(exit_code, 0)
        self.assertNotIn("VS Code extension", stdout.getvalue())


class ReleaseProjectionTests(unittest.TestCase):
    ROOT = Path(__file__).resolve().parents[1]

    def test_no_argument_verifier_covers_the_complete_current_projection(self) -> None:
        result = release_projection.verify_repository(self.ROOT)

        self.assertTrue(result.ok)
        labels = {observation.label for observation in result.observations}
        self.assertIn("Cargo workspace dependency merman-core", labels)
        self.assertIn("Cargo.lock package merman-lsp", labels)
        self.assertIn("fuzz/Cargo.lock package merman-ffi", labels)
        self.assertIn("Web lock workspace package", labels)
        self.assertIn("Playground local Web lock", labels)
        self.assertIn("Playground license lock digest", labels)
        self.assertIn("Python package", labels)
        self.assertIn("Flutter Android package", labels)
        self.assertIn("Flutter iOS Podspec", labels)
        self.assertIn("Flutter macOS Podspec", labels)
        self.assertIn("Flutter iOS framework bundle version", labels)

    def test_cli_without_arguments_runs_the_authority_verifier(self) -> None:
        authority = release_projection.verify_repository(self.ROOT).authority.canonical
        stdout = io.StringIO()
        with mock.patch.object(sys, "argv", ["release-version.py"]), contextlib.redirect_stdout(
            stdout
        ):
            exit_code = release_version_script.main()

        self.assertEqual(exit_code, 0)
        self.assertIn(f"Cargo workspace authority: {authority}", stdout.getvalue())

    def test_every_release_projection_category_fails_closed_on_drift(self) -> None:
        version = release_projection.verify_repository(self.ROOT).authority
        canonical = version.canonical
        mutations = [
            (
                Path("Cargo.toml"),
                lambda text: replace_once(
                    text,
                    f'version = "{canonical}"',
                    'version = "9.9.9"',
                ),
            ),
            (
                Path("Cargo.toml"),
                lambda text: replace_once(
                    text,
                    f'merman-core = {{ path = "crates/merman-core", version = "{canonical}"',
                    'merman-core = { path = "crates/merman-core", version = "9.9.9"',
                ),
            ),
            (
                Path("crates/merman-bindings-core/Cargo.toml"),
                lambda text: replace_once(
                    text,
                    "version.workspace = true",
                    'version = "9.9.9"',
                ),
            ),
            (
                Path("crates/merman-bindings-core/Cargo.toml"),
                lambda text: replace_once(
                    text,
                    "merman.workspace = true",
                    'merman = { path = "../merman", version = "9.9.9", '
                    "default-features = false }",
                ),
            ),
            (
                Path("Cargo.lock"),
                lambda text: replace_once(
                    text,
                    f'name = "merman"\nversion = "{canonical}"',
                    'name = "merman"\nversion = "9.9.9"',
                ),
            ),
            (
                release_projection.FUZZ_LOCK,
                lambda text: replace_once(
                    text,
                    f'name = "merman"\nversion = "{canonical}"',
                    'name = "merman"\nversion = "9.9.9"',
                ),
            ),
            (
                release_projection.WEB_PACKAGE,
                lambda text: replace_once(
                    text,
                    f'"version": "{canonical}"',
                    '"version": "9.9.9"',
                ),
            ),
            (
                release_projection.WEB_LOCK,
                lambda text: replace_nth(
                    text,
                    f'"version": "{canonical}"',
                    '"version": "9.9.9"',
                    0,
                ),
            ),
            (
                release_projection.WEB_LOCK,
                lambda text: replace_nth(
                    text,
                    f'"version": "{canonical}"',
                    '"version": "9.9.9"',
                    1,
                ),
            ),
            (
                release_projection.PLAYGROUND_LOCK,
                lambda text: replace_once(
                    text,
                    (
                        '"../platforms/web": {\n'
                        '      "name": "@mermanjs/web",\n'
                        f'      "version": "{canonical}"'
                    ),
                    (
                        '"../platforms/web": {\n'
                        '      "name": "@mermanjs/web",\n'
                        '      "version": "9.9.9"'
                    ),
                ),
            ),
            (
                release_projection.PLAYGROUND_LICENSE_REPORT,
                lambda text: replace_once(
                    text,
                    f" - @mermanjs/web@{canonical}",
                    " - @mermanjs/web@9.9.9",
                ),
            ),
            (
                release_projection.PYTHON_MANIFEST,
                lambda text: replace_once(
                    text,
                    f'version = "{version.to_pep440()}"',
                    'version = "9.9.9"',
                ),
            ),
            (
                release_projection.ANDROID_MANIFEST,
                lambda text: replace_once(
                    text,
                    f'version = "{canonical}"',
                    'version = "9.9.9"',
                ),
            ),
            (
                release_projection.FLUTTER_MANIFEST,
                lambda text: replace_once(
                    text,
                    f"version: {canonical}",
                    "version: 9.9.9",
                ),
            ),
            (
                release_projection.FLUTTER_ANDROID_MANIFEST,
                lambda text: replace_once(
                    text,
                    f"version = '{canonical}'",
                    "version = '9.9.9'",
                ),
            ),
            *[
                (
                    podspec,
                    lambda text, expected=canonical: replace_once(
                        text,
                        f"s.version          = '{expected}'",
                        "s.version          = '9.9.9'",
                    ),
                )
                for podspec in (
                    release_projection.FLUTTER_IOS_PODSPEC,
                    release_projection.FLUTTER_MACOS_PODSPEC,
                )
            ],
            (
                release_projection.FLUTTER_IOS_BUILD,
                lambda text: replace_once(
                    text,
                    (
                        "<key>CFBundleShortVersionString</key>\n"
                        f"  <string>{version.base}</string>"
                    ),
                    (
                        "<key>CFBundleShortVersionString</key>\n"
                        "  <string>9.9.9</string>"
                    ),
                ),
            ),
            (
                release_projection.FLUTTER_IOS_BUILD,
                lambda text: replace_once(
                    text,
                    f"<key>CFBundleVersion</key>\n  <string>{version.base}</string>",
                    "<key>CFBundleVersion</key>\n  <string>9.9.9</string>",
                ),
            ),
        ]

        for path, mutate in mutations:
            with self.subTest(path=path):
                original = (self.ROOT / path).read_text(encoding="utf-8")
                drifted = mutate(original)
                try:
                    result = release_projection.verify_repository(
                        self.ROOT,
                        overrides={path: drifted},
                    )
                except release_projection.ReleaseProjectionError:
                    continue
                self.assertFalse(result.ok)

    def test_transaction_plan_projects_one_authority_without_touching_independent_axes(self) -> None:
        current = release_projection.verify_repository(self.ROOT).authority
        next_version = f"{current.major}.{current.minor + 1}.0-alpha.1"

        updates = release_projection.plan_version_update(self.ROOT, next_version)
        result = release_projection.verify_repository(
            self.ROOT,
            expected_version=next_version,
            overrides=updates,
        )

        self.assertTrue(result.ok)
        self.assertIn(Path("Cargo.toml"), updates)
        self.assertIn(release_projection.FUZZ_LOCK, updates)
        self.assertIn(release_projection.PLAYGROUND_LICENSE_REPORT, updates)
        self.assertIn(release_projection.FLUTTER_IOS_BUILD, updates)
        self.assertNotIn(Path("tools/vscode-extension/package.json"), updates)
        self.assertNotIn(Path("packages/typst/merman/typst.toml"), updates)

    def test_multi_file_update_rolls_back_when_a_replace_fails(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            first = root / "first.txt"
            second = root / "second.txt"
            first.write_text("first-old", encoding="utf-8")
            second.write_text("second-old", encoding="utf-8")
            real_replace = release_projection.os.replace
            replace_count = 0

            def replace_with_second_call_failure(source, destination):  # noqa: ANN001
                nonlocal replace_count
                replace_count += 1
                if replace_count == 2:
                    raise OSError("injected replace failure")
                return real_replace(source, destination)

            with mock.patch.object(
                release_projection.os,
                "replace",
                side_effect=replace_with_second_call_failure,
            ), self.assertRaisesRegex(OSError, "injected replace failure"):
                release_projection._atomic_replace(
                    root,
                    {
                        Path("first.txt"): "first-new",
                        Path("second.txt"): "second-new",
                    },
                    expected={
                        Path("first.txt"): "first-old",
                        Path("second.txt"): "second-old",
                    },
                )

            self.assertEqual(first.read_text(encoding="utf-8"), "first-old")
            self.assertEqual(second.read_text(encoding="utf-8"), "second-old")
            self.assertEqual(list(root.glob(".*.release-version-*")), [])


class ReleaseStatusProbeTests(unittest.TestCase):
    def test_npm_probe_requires_version_and_matching_channel_dist_tag(self) -> None:
        original_urlopen = release_status.urllib.request.urlopen
        captured: dict[str, str] = {}
        try:
            def urlopen(request, timeout: int = 0):  # noqa: ANN001
                captured["url"] = request.full_url
                captured["timeout"] = str(timeout)
                return JsonResponse(
                    {
                        "versions": {"0.8.0-alpha.3": {"version": "0.8.0-alpha.3"}},
                        "dist-tags": {"alpha": "0.8.0-alpha.3", "latest": "0.7.0"},
                    }
                )

            release_status.urllib.request.urlopen = urlopen
            result = release_status.probe_npm(
                "@mermanjs/web",
                "0.8.0-alpha.3",
                dist_tags(),
            )
        finally:
            release_status.urllib.request.urlopen = original_urlopen

        self.assertEqual(result["state"], "found")
        self.assertEqual(
            captured["url"],
            "https://registry.npmjs.org/%40mermanjs%2Fweb",
        )
        self.assertEqual(captured["timeout"], "10")

    def test_npm_probe_rejects_a_version_on_the_wrong_dist_tag(self) -> None:
        with mock.patch.object(
            release_status.urllib.request,
            "urlopen",
            return_value=JsonResponse(
                {
                    "versions": {"0.8.0-alpha.3": {}},
                    "dist-tags": {"alpha": "0.8.0-alpha.2", "latest": "0.8.0-alpha.3"},
                }
            ),
        ):
            result = release_status.probe_npm(
                "@mermanjs/web",
                "0.8.0-alpha.3",
                dist_tags(),
            )

        self.assertEqual(result["state"], "missing")
        self.assertIn("alpha", result["reason"])
        self.assertIn("0.8.0-alpha.2", result["reason"])

    def test_npm_probe_marks_malformed_dist_tag_value_unknown(self) -> None:
        with mock.patch.object(
            release_status.urllib.request,
            "urlopen",
            return_value=JsonResponse(
                {
                    "versions": {"0.8.0-alpha.3": {}},
                    "dist-tags": {"alpha": ["0.8.0-alpha.3"]},
                }
            ),
        ):
            result = release_status.probe_npm(
                "@mermanjs/web",
                "0.8.0-alpha.3",
                dist_tags(),
            )

        self.assertEqual(result["state"], "unknown")
        self.assertIn("invalid", result["reason"])

    def test_npm_operational_failure_is_unknown(self) -> None:
        failure = urllib.error.HTTPError("url", 429, "rate limited", {}, None)
        with mock.patch.object(
            release_status.urllib.request,
            "urlopen",
            side_effect=failure,
        ):
            result = release_status.probe_npm("@mermanjs/web", "0.8.0", dist_tags())
        failure.close()

        self.assertEqual(result["state"], "unknown")
        self.assertIn("429", result["reason"])

    def test_npm_malformed_versions_payload_is_unknown(self) -> None:
        with mock.patch.object(
            release_status.urllib.request,
            "urlopen",
            return_value=JsonResponse(
                {"versions": ["0.8.0-alpha.3"], "dist-tags": {"alpha": "0.8.0-alpha.3"}}
            ),
        ):
            result = release_status.probe_npm("@mermanjs/web", "0.8.0-alpha.3", dist_tags())

        self.assertEqual(result["state"], "unknown")
        self.assertIn("versions object", result["reason"])

    def test_pub_dev_probe_finds_prerelease_versions(self) -> None:
        original_urlopen = release_status.urllib.request.urlopen
        captured: dict[str, str] = {}
        try:
            def urlopen(request, timeout: int = 0):  # noqa: ANN001
                captured["url"] = request.full_url
                captured["timeout"] = str(timeout)
                return JsonResponse(
                    {
                        "versions": [
                            {"version": "0.7.0"},
                            {"version": "0.8.0-alpha.3"},
                        ]
                    }
                )

            release_status.urllib.request.urlopen = urlopen
            result = release_status.probe_pub_dev("merman", "0.8.0-alpha.3")
        finally:
            release_status.urllib.request.urlopen = original_urlopen

        self.assertEqual(result["state"], "found")
        self.assertEqual(captured["url"], "https://pub.dev/api/packages/merman")
        self.assertEqual(captured["timeout"], "10")

    def test_pub_dev_malformed_versions_payload_is_unknown(self) -> None:
        with mock.patch.object(
            release_status.urllib.request,
            "urlopen",
            return_value=JsonResponse({"versions": {"version": "0.8.0-alpha.3"}}),
        ):
            result = release_status.probe_pub_dev("merman", "0.8.0-alpha.3")

        self.assertEqual(result["state"], "unknown")
        self.assertIn("valid versions list", result["reason"])

    def test_github_release_probe_requires_markers_and_exact_assets(self) -> None:
        payload = {
            "tagName": "v0.8.0-alpha.3",
            "isDraft": False,
            "isPrerelease": True,
            "assets": [
                {
                    "name": "merman-cli-v0.8.0-alpha.3-x86_64.zip",
                    "state": "uploaded",
                    "size": 1,
                },
                {"name": "merman-cli-installer.sh", "state": "uploaded", "size": 1},
            ],
        }
        channel = {
            "asset_patterns": [
                {"glob": "merman-cli-*-x86_64.zip", "min_matches": 1, "max_matches": 1},
                {"glob": "merman-cli-installer.sh", "min_matches": 1, "max_matches": 1},
            ]
        }
        completed = subprocess.CompletedProcess(
            args=["gh"],
            returncode=0,
            stdout=json.dumps(payload),
            stderr="",
        )
        with mock.patch.object(release_status.shutil, "which", return_value="/usr/bin/gh"), mock.patch.object(
            release_status.subprocess, "run", return_value=completed
        ):
            result = release_status.probe_github_release(channel, "0.8.0-alpha.3")

        self.assertEqual(result["state"], "found")

    def test_github_release_draft_or_wrong_marker_is_missing(self) -> None:
        channel = {
            "asset_patterns": [
                {"glob": "asset.zip", "min_matches": 1, "max_matches": 1},
            ]
        }
        for field, value in [("isDraft", True), ("isPrerelease", False)]:
            payload = {
                "tagName": "v0.8.0-alpha.3",
                "isDraft": False,
                "isPrerelease": True,
                "assets": [{"name": "asset.zip", "state": "uploaded", "size": 1}],
            }
            payload[field] = value
            completed = subprocess.CompletedProcess(
                args=["gh"], returncode=0, stdout=json.dumps(payload), stderr=""
            )
            with self.subTest(field=field), mock.patch.object(
                release_status.shutil, "which", return_value="/usr/bin/gh"
            ), mock.patch.object(release_status.subprocess, "run", return_value=completed):
                result = release_status.probe_github_release(channel, "0.8.0-alpha.3")
            self.assertEqual(result["state"], "missing")

    def test_github_release_operational_failures_are_unknown_without_traceback(self) -> None:
        channel = {
            "asset_patterns": [{"glob": "asset.zip", "min_matches": 1, "max_matches": 1}]
        }
        cases = [
            subprocess.CompletedProcess(args=["gh"], returncode=1, stdout="", stderr="HTTP 401"),
            subprocess.CompletedProcess(args=["gh"], returncode=1, stdout="   ", stderr="   "),
            subprocess.TimeoutExpired(cmd=["gh"], timeout=20),
            OSError("cannot execute"),
        ]
        for outcome in cases:
            with self.subTest(outcome=type(outcome).__name__), mock.patch.object(
                release_status.shutil, "which", return_value="/usr/bin/gh"
            ), mock.patch.object(release_status.subprocess, "run", side_effect=outcome if isinstance(outcome, BaseException) else None, return_value=None if isinstance(outcome, BaseException) else outcome):
                result = release_status.probe_github_release(channel, "0.8.0")
            self.assertEqual(result["state"], "unknown")

    def test_github_release_explicit_not_found_is_missing(self) -> None:
        completed = subprocess.CompletedProcess(
            args=["gh"], returncode=1, stdout="", stderr="release not found"
        )
        with mock.patch.object(release_status.shutil, "which", return_value="/usr/bin/gh"), mock.patch.object(
            release_status.subprocess, "run", return_value=completed
        ):
            result = release_status.probe_github_release(
                {"asset_patterns": []},
                "0.8.0",
            )
        self.assertEqual(result["state"], "missing")

    def test_github_release_probe_rejects_wrong_version_and_incomplete_assets(self) -> None:
        channel = {
            "asset_patterns": [
                {"glob": "merman-android-{tag}.aar", "min_matches": 1, "max_matches": 1},
            ]
        }
        cases = [
            ([{"name": "merman-android-v0.7.0.aar", "state": "uploaded", "size": 1}], "matched 0"),
            ([{"name": "merman-android-v0.8.0-alpha.3.aar", "state": "starter", "size": 1}], "not uploaded"),
            ([{"name": "merman-android-v0.8.0-alpha.3.aar", "state": "uploaded", "size": 0}], "empty"),
        ]
        for assets, reason in cases:
            payload = {
                "tagName": "v0.8.0-alpha.3",
                "isDraft": False,
                "isPrerelease": True,
                "assets": assets,
            }
            completed = subprocess.CompletedProcess(
                args=["gh"], returncode=0, stdout=json.dumps(payload), stderr=""
            )
            with self.subTest(reason=reason), mock.patch.object(
                release_status.shutil, "which", return_value="/usr/bin/gh"
            ), mock.patch.object(release_status.subprocess, "run", return_value=completed):
                result = release_status.probe_github_release(channel, "0.8.0-alpha.3")
            self.assertEqual(result["state"], "missing")
            self.assertIn(reason, result["reason"])

    def test_github_release_probe_requires_asset_metadata(self) -> None:
        payload = {
            "tagName": "v0.8.0-alpha.3",
            "isDraft": False,
            "isPrerelease": True,
            "assets": [{"name": "asset.zip"}],
        }
        completed = subprocess.CompletedProcess(
            args=["gh"], returncode=0, stdout=json.dumps(payload), stderr=""
        )
        with mock.patch.object(
            release_status.shutil, "which", return_value="/usr/bin/gh"
        ), mock.patch.object(release_status.subprocess, "run", return_value=completed):
            result = release_status.probe_github_release(
                {"asset_patterns": [{"glob": "asset.zip", "min_matches": 1, "max_matches": 1}]},
                "0.8.0-alpha.3",
            )
        self.assertEqual(result["state"], "unknown")
        self.assertIn("state and size", result["reason"])

    def test_actions_artifact_probe_refuses_unversioned_contract(self) -> None:
        result = release_status.probe_github_actions_artifacts(
            {
                "workflow": ".github/workflows/vscode-extension.yml",
                "artifact_patterns": [
                    {"glob": "merman-vscode-linux-x64", "min_matches": 1, "max_matches": 1}
                ],
            },
            "0.8.0-alpha.3",
        )

        self.assertEqual(result["state"], "unknown")
        self.assertIn("target release version", result["reason"])

    def test_actions_artifact_probe_matches_successful_workflow_run(self) -> None:
        responses = [
            subprocess.CompletedProcess(
                args=["gh"],
                returncode=0,
                stdout=f"{SOURCE_SHA}\n",
                stderr="",
            ),
            subprocess.CompletedProcess(
                args=["gh"],
                returncode=0,
                stdout=json.dumps(
                    [
                        {
                            "workflow_runs": [
                                {
                                    "id": 42,
                                    "status": "completed",
                                    "conclusion": "success",
                                    "event": "workflow_dispatch",
                                    "head_sha": SOURCE_SHA,
                                }
                            ]
                        }
                    ]
                ),
                stderr="",
            ),
            subprocess.CompletedProcess(
                args=["gh"],
                returncode=0,
                stdout=json.dumps(
                    [
                        {
                            "artifacts": [
                                {
                                    "name": f"merman-vscode-0.8.0-alpha.3-alpha-{SOURCE_SHA}-linux-x64",
                                    "expired": False,
                                    "size_in_bytes": 1,
                                    "workflow_run": {"id": 42},
                                }
                            ]
                        }
                    ]
                ),
                stderr="",
            ),
        ]
        channel = {
            "workflow": ".github/workflows/vscode-extension.yml",
            "artifact_patterns": [
                {
                    "glob": "merman-vscode-{version}-{channel}-{source_sha}-linux-x64",
                    "min_matches": 1,
                    "max_matches": 1,
                }
            ],
        }
        with mock.patch.object(release_status.shutil, "which", return_value="/usr/bin/gh"), mock.patch.object(
            release_status, "github_repository", return_value="Latias94/merman"
        ), mock.patch.object(release_status.subprocess, "run", side_effect=responses):
            result = release_status.probe_github_actions_artifacts(channel, "0.8.0-alpha.3")

        self.assertEqual(result["state"], "found")

    def test_actions_artifact_probe_expands_independent_package_version(self) -> None:
        responses = actions_artifact_responses(
            event="workflow_dispatch",
            package_version="0.1.0",
        )
        channel = {
            "workflow": ".github/workflows/vscode-extension.yml",
            "artifact_patterns": [
                {
                    "glob": (
                        "merman-vscode-{package_version}-runtime-{version}-"
                        "{channel}-{source_sha}-linux-x64"
                    ),
                    "min_matches": 1,
                    "max_matches": 1,
                }
            ],
        }
        with mock.patch.object(
            release_status.shutil, "which", return_value="/usr/bin/gh"
        ), mock.patch.object(
            release_status, "github_repository", return_value="Latias94/merman"
        ), mock.patch.object(release_status.subprocess, "run", side_effect=responses):
            result = release_status.probe_github_actions_artifacts(
                channel,
                "0.8.0-alpha.3",
                package_version="0.1.0",
            )

        self.assertEqual(result["state"], "found")

    def test_actions_artifact_probe_marks_legacy_artifacts_unknown(self) -> None:
        responses = actions_artifact_responses(event="workflow_dispatch", source_sha=None)
        with mock.patch.object(
            release_status.shutil, "which", return_value="/usr/bin/gh"
        ), mock.patch.object(
            release_status, "github_repository", return_value="Latias94/merman"
        ), mock.patch.object(release_status.subprocess, "run", side_effect=responses):
            result = release_status.probe_github_actions_artifacts(
                versioned_actions_channel(), "0.8.0-alpha.3"
            )
        self.assertEqual(result["state"], "unknown")
        self.assertIn("source provenance", result["reason"])

    def test_actions_artifact_probe_rejects_pull_request_runs(self) -> None:
        responses = actions_artifact_responses(event="pull_request")

        with mock.patch.object(
            release_status.shutil, "which", return_value="/usr/bin/gh"
        ), mock.patch.object(
            release_status, "github_repository", return_value="Latias94/merman"
        ), mock.patch.object(release_status.subprocess, "run", side_effect=responses):
            result = release_status.probe_github_actions_artifacts(
                versioned_actions_channel(),
                "0.8.0-alpha.3",
            )

        self.assertEqual(result["state"], "missing")
        self.assertIn("no successful runs", result["reason"])

    def test_actions_artifact_probe_treats_missing_run_provenance_as_unknown(self) -> None:
        responses = actions_artifact_responses(event=None)

        with mock.patch.object(
            release_status.shutil, "which", return_value="/usr/bin/gh"
        ), mock.patch.object(
            release_status, "github_repository", return_value="Latias94/merman"
        ), mock.patch.object(release_status.subprocess, "run", side_effect=responses):
            result = release_status.probe_github_actions_artifacts(
                versioned_actions_channel(),
                "0.8.0-alpha.3",
            )

        self.assertEqual(result["state"], "unknown")
        self.assertIn("metadata omitted event or head SHA", result["reason"])

    def test_actions_artifact_probe_treats_malformed_artifact_response_as_unknown(self) -> None:
        responses = actions_artifact_responses(event="workflow_dispatch")
        responses[2] = subprocess.CompletedProcess(
            args=["gh"],
            returncode=0,
            stdout=json.dumps([{"artifacts": [{"expired": False, "workflow_run": {"id": 42}}]}]),
            stderr="",
        )

        with mock.patch.object(
            release_status.shutil, "which", return_value="/usr/bin/gh"
        ), mock.patch.object(
            release_status, "github_repository", return_value="Latias94/merman"
        ), mock.patch.object(release_status.subprocess, "run", side_effect=responses):
            result = release_status.probe_github_actions_artifacts(
                versioned_actions_channel(), "0.8.0-alpha.3"
            )

        self.assertEqual(result["state"], "unknown")
        self.assertIn("malformed metadata", result["reason"])


class ReleaseStatusContractTests(unittest.TestCase):
    def test_contract_loader_rejects_duplicate_json_keys(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            path = Path(temp_dir) / "SURFACES.json"
            path.write_text(
                '{"schema_version": 1, "schema_version": 1}',
                encoding="utf-8",
            )

            with self.assertRaisesRegex(release_status.SurfaceError, "duplicate JSON object key"):
                release_status.load_contract(path)

    def test_prerelease_marks_stable_only_channels_not_applicable(self) -> None:
        data = contract(
            surfaces=[
                surface(
                    surface_id="homebrew",
                    channels=[
                        channel(
                            channel_id="homebrew-core",
                            kind="homebrew",
                            declared_state="published",
                            release_kinds=["stable"],
                        )
                    ],
                )
            ]
        )

        rows = release_status.build_rows(data, version="0.8.0-alpha.3", probe=False)

        self.assertEqual(rows[0]["declared_state"], "not-applicable")
        self.assertEqual(rows[0]["channels"][0]["declared_state"], "not-applicable")

    def test_public_availability_uses_declared_primary_channel(self) -> None:
        data = contract(
            surfaces=[
                surface(
                    surface_id="typst",
                    channels=[
                        channel(
                            channel_id="typst-registry",
                            kind="typst-registry",
                            declared_state="manual-registry",
                        ),
                        channel(
                            channel_id="crates.io",
                            kind="crates.io",
                            declared_state="published",
                        ),
                    ],
                    public_channel="typst-registry",
                )
            ]
        )
        rows = release_status.build_rows(data, version="0.8.0", probe=False)
        self.assertEqual(rows[0]["declared_state"], "manual-registry")
        self.assertEqual(rows[0]["availability_channel"], "typst-registry")

    def test_json_output_separates_declared_and_observed_status(self) -> None:
        data = contract(
            surfaces=[
                surface(
                    surface_id="web-wasm",
                    entry_point="@mermanjs/web",
                    channels=[
                        channel(
                            channel_id="npm",
                            kind="npm",
                            declared_state="published",
                            release_kinds=["stable", "prerelease"],
                        )
                    ],
                )
            ]
        )

        with tempfile.TemporaryDirectory() as temp_dir:
            path = Path(temp_dir) / "SURFACES.json"
            path.write_text(json.dumps(data), encoding="utf-8")
            stdout = io.StringIO()
            with contextlib.redirect_stdout(stdout):
                exit_code = release_status.main(
                    [
                        "--contract",
                        str(path),
                        "--version",
                        "0.8.0-alpha.3",
                        "--format",
                        "json",
                    ]
                )

        payload = json.loads(stdout.getvalue())
        channel_row = payload["surfaces"][0]["channels"][0]
        self.assertEqual(exit_code, 0)
        self.assertEqual(payload["release_kind"], "prerelease")
        self.assertEqual(channel_row["declared_state"], "published")
        self.assertNotIn("observed_status", channel_row)

    def test_public_table_uses_user_facing_package_choice_columns(self) -> None:
        rows = release_status.build_rows(contract(), version=None, probe=False)

        output = release_status.render_public(rows)

        self.assertIn("Surface | Entry point | Install | Support | Availability", output)
        self.assertIn(
            "Example surface | `example-entry` | install example | published | published | low | parse, render",
            output,
        )

    def test_public_json_projection_does_not_expose_maintainer_channels(self) -> None:
        rows = release_status.build_rows(contract(), version="0.8.0", probe=False)

        projected = release_status.public_projection(rows)

        self.assertEqual(len(projected), 1)
        self.assertNotIn("channels", projected[0])
        self.assertNotIn("docs", projected[0])

    def test_maintainer_view_includes_protected_environment(self) -> None:
        rows = release_status.build_rows(contract(), version="0.8.0", probe=False)

        self.assertEqual(rows[0]["channels"][0]["environment"], "crates.io")
        self.assertIn("Environment", release_status.render_maintainer(rows))

    def test_crates_probe_checks_every_crate_in_a_surface(self) -> None:
        original_probe_crates_io = release_status.probe_crates_io
        checked: list[str] = []
        try:
            def probe_crates_io(package: str, version: str) -> dict[str, str]:
                checked.append(f"{package}@{version}")
                state = "missing" if package == "beta" else "found"
                return {"state": state, "reason": f"{package} {state}"}

            release_status.probe_crates_io = probe_crates_io
            row = release_status.channel_probe(
                {"kind": "crates.io"},
                {
                    "packages": [
                        {"kind": "crate", "name": "alpha"},
                        {"kind": "crate", "name": "beta"},
                    ]
                },
                "1.0.0",
            )
        finally:
            release_status.probe_crates_io = original_probe_crates_io

        self.assertEqual(checked, ["alpha@1.0.0", "beta@1.0.0"])
        self.assertEqual(row["state"], "missing")
        self.assertIn("beta missing", row["reason"])

    def test_crates_probe_uses_manifest_version_when_declared(self) -> None:
        package = {
            "kind": "crate",
            "name": "roughr-merman",
            "manifest": "crates/roughr/Cargo.toml",
            "version_source": "manifest",
        }

        version = release_status.package_registry_version(
            package,
            release_status.parse_release_version("0.8.0-alpha.3"),
        )

        self.assertEqual(version, "0.12.1")

    def test_vscode_artifact_probe_uses_its_independent_manifest_version(self) -> None:
        observed: list[str] = []

        def probe(
            _channel: dict,
            _target: release_status.ReleaseVersion,
            *,
            package_version: str | None = None,
        ) -> dict[str, str]:
            observed.append(package_version or "")
            return {"state": "found", "reason": "ok"}

        with mock.patch.object(release_status, "probe_github_actions_artifacts", side_effect=probe):
            result = release_status.channel_probe(
                {"kind": "github-actions-artifact"},
                {
                    "id": "vscode",
                    "packages": [
                        {
                            "kind": "vscode",
                            "name": "merman-vscode",
                            "manifest": "tools/vscode-extension/package.json",
                            "version_source": "manifest",
                        }
                    ],
                },
                "0.8.0-alpha.3",
            )

        self.assertEqual(result["state"], "found")
        self.assertEqual(observed, ["0.1.0"])

    def test_pub_dev_probe_checks_flutter_package(self) -> None:
        original_probe_pub_dev = release_status.probe_pub_dev
        checked: list[str] = []
        try:
            def probe_pub_dev(package: str, version: str) -> dict[str, str]:
                checked.append(f"{package}@{version}")
                return {"state": "found", "reason": "ok"}

            release_status.probe_pub_dev = probe_pub_dev
            row = release_status.channel_probe(
                {"kind": "pub.dev"},
                {"packages": [{"kind": "flutter", "name": "merman"}]},
                "0.8.0-alpha.3",
            )
        finally:
            release_status.probe_pub_dev = original_probe_pub_dev

        self.assertEqual(checked, ["merman@0.8.0-alpha.3"])
        self.assertEqual(row["state"], "found")

    def test_probe_requires_target_version(self) -> None:
        stderr = io.StringIO()
        with contextlib.redirect_stderr(stderr):
            exit_code = release_status.main(["--probe"])

        self.assertEqual(exit_code, 2)
        self.assertIn("--probe requires --version", stderr.getvalue())

    def test_contract_rejects_unknown_declared_state(self) -> None:
        data = contract(
            surfaces=[
                surface(
                    channels=[
                        channel(
                            declared_state="almost-published",
                            release_kinds=["stable", "prerelease"],
                        )
                    ],
                )
            ]
        )

        with self.assertRaisesRegex(release_status.SurfaceError, "unknown declared_state"):
            release_status.validate_contract(data)

    def test_contract_rejects_unknown_fields_and_incomplete_channels(self) -> None:
        cases = []
        extra_top = contract()
        extra_top["unexpected"] = True
        cases.append((extra_top, "unknown fields"))

        missing_kind = contract()
        del missing_kind["surfaces"][0]["channels"][0]["kind"]
        cases.append((missing_kind, "missing string field kind"))

        unknown_channel_kind = contract()
        unknown_channel_kind["surfaces"][0]["channels"][0]["kind"] = "future-registry"
        cases.append((unknown_channel_kind, "unsupported channel kind"))

        unknown_package_kind = contract()
        unknown_package_kind["surfaces"][0]["packages"][0]["kind"] = "future-package"
        cases.append((unknown_package_kind, "unsupported package kind"))

        extra_channel = contract()
        extra_channel["surfaces"][0]["channels"][0]["surprise"] = True
        cases.append((extra_channel, "unknown fields"))

        missing_job = contract()
        del missing_job["surfaces"][0]["channels"][0]["workflow_job"]
        cases.append((missing_job, "missing string field workflow_job"))

        missing_release_assets = contract()
        missing_release_assets["surfaces"][0]["channels"][0]["kind"] = (
            "github-release-assets"
        )
        cases.append((missing_release_assets, "requires asset_patterns"))

        missing_actions_artifacts = contract()
        missing_actions_artifacts["surfaces"][0]["channels"][0]["kind"] = (
            "github-actions-artifact"
        )
        cases.append((missing_actions_artifacts, "requires artifact_patterns"))

        invalid_release_kind = contract()
        invalid_release_kind["surfaces"][0]["channels"][0]["release_kinds"] = ["nightly"]
        cases.append((invalid_release_kind, "unsupported values"))

        invalid_npm_dist_tags = contract(
            surfaces=[surface(channels=[channel(kind="npm")])]
        )
        invalid_npm_dist_tags["surfaces"][0]["channels"][0]["dist_tags"]["alpha"] = "latest"
        cases.append((invalid_npm_dist_tags, "canonical stable/alpha/beta/rc mapping"))

        empty_release_asset_group = contract(
            surfaces=[surface(channels=[channel(kind="github-release-assets")])]
        )
        empty_release_asset_group["surfaces"][0]["channels"][0]["asset_patterns"] = [
            {"glob": "asset.zip", "min_matches": 0, "max_matches": 1}
        ]
        cases.append((empty_release_asset_group, "at least one asset"))

        for data, message in cases:
            with self.subTest(message=message), self.assertRaisesRegex(
                release_status.SurfaceError, message
            ):
                release_status.validate_contract(data)

    def test_contract_requires_reason_for_conditional_channels(self) -> None:
        data = contract(
            surfaces=[
                surface(
                    public_channel="homebrew-core",
                    channels=[
                        channel(
                            channel_id="homebrew-core",
                            kind="homebrew",
                            release_kinds=["stable"],
                        )
                    ],
                )
            ]
        )
        with self.assertRaisesRegex(release_status.SurfaceError, "not_applicable_reason"):
            release_status.validate_contract(data)

    def test_contract_requires_literal_environment_for_protected_publication(self) -> None:
        missing_environment = contract()
        del missing_environment["surfaces"][0]["channels"][0]["environment"]
        dynamic_environment = contract()
        dynamic_environment["surfaces"][0]["channels"][0]["environment"] = (
            "${{ inputs.environment }}"
        )

        for data, message in [
            (missing_environment, "missing string field environment"),
            (dynamic_environment, "environment must be a literal identifier"),
        ]:
            with self.subTest(message=message), self.assertRaisesRegex(
                release_status.SurfaceError, message
            ):
                release_status.validate_contract(data)

    def test_invalid_cli_version_is_reported_without_traceback(self) -> None:
        stderr = io.StringIO()
        with contextlib.redirect_stderr(stderr):
            exit_code = release_status.main(["--version", "v1.2.3-dev.1"])

        self.assertEqual(exit_code, 1)
        self.assertIn("unsupported release version", stderr.getvalue())
        self.assertNotIn("Traceback", stderr.getvalue())


def contract(*, surfaces: list[dict] | None = None) -> dict:
    return {
        "schema_version": 2,
        "states": [
            "published",
            "artifact-only",
            "manual-registry",
            "credential-blocked",
            "registry-blocked",
            "not-built",
            "not-applicable",
        ],
        "release_kinds": ["stable", "prerelease"],
        "feature_contract": {
            "docs": ["README.md"],
            "browser_presets": ["browser-full"],
            "web_descriptor": "platforms/web/web-surface-descriptor.json",
            "web_default_preset": "browser-full",
            "web_auxiliary_exports": {
                ".": {"import": "./dist/index.js", "types": "./dist/index.d.ts"}
            },
        },
        "surfaces": surfaces
        or [
            surface(
                capabilities=["parse", "render"],
                channels=[
                    channel(
                        channel_id="crates.io",
                        kind="crates.io",
                        declared_state="published",
                        release_kinds=["stable", "prerelease"],
                    )
                ],
            )
        ],
    }


def surface(
    *,
    surface_id: str = "example",
    entry_point: str = "example-entry",
    capabilities: list[str] | None = None,
    channels: list[dict] | None = None,
    public_channel: str | None = None,
) -> dict:
    result = {
        "id": surface_id,
        "name": "Example surface",
        "audience": "Example users",
        "public": True,
        "entry_point": entry_point,
        "install": "install example",
        "support_level": "published",
        "dependency_weight": "low",
        "capabilities": capabilities or ["parse"],
        "docs": ["README.md"],
        "packages": [
            {"kind": "crate", "name": "example", "manifest": "Cargo.toml"}
        ],
        "channels": channels or [channel()],
        "gates": ["verify example"],
    }
    result["public_channel"] = public_channel or result["channels"][0]["id"]
    return result


def channel(
    *,
    channel_id: str = "example-channel",
    kind: str = "homebrew",
    declared_state: str = "published",
    release_kinds: list[str] | None = None,
) -> dict:
    result = {
        "id": channel_id,
        "kind": kind,
        "declared_state": declared_state,
        "release_kinds": release_kinds or ["stable", "prerelease"],
        "workflow": ".github/workflows/release.yml",
        "workflow_job": "publish",
        "credential": None,
    }
    environment = {
        "crates.io": "crates.io",
        "github-release-assets": "github-release",
        "npm": "npm",
        "pypi": "pypi",
        "pub.dev": "pub.dev",
    }.get(kind)
    if environment is not None:
        result["environment"] = environment
    if kind == "npm":
        result["dist_tags"] = dist_tags()
    return result


def dist_tags() -> dict[str, str]:
    return {
        "stable": "latest",
        "alpha": "alpha",
        "beta": "beta",
        "rc": "rc",
    }


def versioned_actions_channel() -> dict:
    return {
        "workflow": ".github/workflows/vscode-extension.yml",
        "artifact_patterns": [
            {
                "glob": "merman-vscode-{version}-{channel}-{source_sha}-linux-x64",
                "min_matches": 1,
                "max_matches": 1,
            }
        ],
    }


def actions_artifact_responses(
    *,
    event: str | None,
    source_sha: str | None = SOURCE_SHA,
    package_version: str | None = None,
) -> list[subprocess.CompletedProcess[str]]:
    run = {
        "id": 42,
        "status": "completed",
        "conclusion": "success",
        "head_sha": SOURCE_SHA,
    }
    if event is not None:
        run["event"] = event
    artifact_prefix = (
        f"merman-vscode-{package_version}-runtime-0.8.0-alpha.3-alpha"
        if package_version is not None
        else "merman-vscode-0.8.0-alpha.3-alpha"
    )
    artifact_name = (
        f"{artifact_prefix}-{SOURCE_SHA}-linux-x64"
        if source_sha is not None
        else f"{artifact_prefix}-linux-x64"
    )
    responses = [
        subprocess.CompletedProcess(
            args=["gh"], returncode=0, stdout=f"{SOURCE_SHA}\n", stderr=""
        ),
        subprocess.CompletedProcess(
            args=["gh"],
            returncode=0,
            stdout=json.dumps([{"workflow_runs": [run]}]),
            stderr="",
        ),
        subprocess.CompletedProcess(
            args=["gh"],
            returncode=0,
            stdout=json.dumps(
                [
                    {
                        "artifacts": [
                            {
                                "name": artifact_name,
                                "expired": False,
                                "size_in_bytes": 1,
                                "workflow_run": {"id": 42},
                            }
                        ]
                    }
                ]
            ),
            stderr="",
        ),
    ]
    return responses


class JsonResponse:
    status = 200

    def __init__(self, data: dict) -> None:
        self.data = json.dumps(data).encode("utf-8")

    def read(self) -> bytes:
        return self.data

    def __enter__(self):
        return self

    def __exit__(self, _exc_type, _exc, _traceback) -> None:  # noqa: ANN001
        return None


if __name__ == "__main__":
    unittest.main()
