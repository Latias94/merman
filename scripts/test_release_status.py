#!/usr/bin/env python3
"""Unit tests for release surface status reporting."""

from __future__ import annotations

import contextlib
import importlib.util
import io
import json
import tempfile
import unittest
from pathlib import Path


MODULE_PATH = Path(__file__).with_name("release-status.py")
SPEC = importlib.util.spec_from_file_location("release_status", MODULE_PATH)
assert SPEC is not None
release_status = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(release_status)


class ReleaseStatusVersionTests(unittest.TestCase):
    def test_release_kind_detects_stable_and_prerelease_versions(self) -> None:
        self.assertEqual(release_status.release_kind("0.8.0"), "stable")
        self.assertEqual(release_status.release_kind("v0.8.0"), "stable")
        self.assertEqual(release_status.release_kind("0.8.0-alpha.3"), "prerelease")
        self.assertEqual(release_status.release_kind("0.8.0a3"), "prerelease")
        self.assertIsNone(release_status.release_kind(None))

    def test_python_version_normalizes_alpha_suffix_for_pypi(self) -> None:
        self.assertEqual(release_status.python_version("v0.8.0-alpha.3"), "0.8.0a3")


class ReleaseStatusProbeTests(unittest.TestCase):
    def test_npm_probe_uses_registry_http_for_scoped_packages(self) -> None:
        original_urlopen = release_status.urllib.request.urlopen
        captured: dict[str, str] = {}
        try:
            def urlopen(request, timeout: int = 0):  # noqa: ANN001
                captured["url"] = request.full_url
                captured["timeout"] = str(timeout)
                return JsonResponse({"version": "0.8.0-alpha.3"})

            release_status.urllib.request.urlopen = urlopen
            result = release_status.probe_npm("@mermanjs/web", "0.8.0-alpha.3")
        finally:
            release_status.urllib.request.urlopen = original_urlopen

        self.assertEqual(result["state"], "found")
        self.assertEqual(
            captured["url"],
            "https://registry.npmjs.org/%40mermanjs%2Fweb/0.8.0-alpha.3",
        )
        self.assertEqual(captured["timeout"], "10")

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


class ReleaseStatusContractTests(unittest.TestCase):
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

        self.assertIn("Surface | Entry point | Availability | Weight | Key capabilities", output)
        self.assertIn("Example surface | `example-entry` | published | low | parse, render", output)

    def test_crates_probe_checks_every_crate_in_a_surface(self) -> None:
        original_probe_crates_io = release_status.probe_crates_io
        checked: list[str] = []
        try:
            def probe_crates_io(package: str, _version: str) -> dict[str, str]:
                checked.append(package)
                state = "missing" if package == "beta" else "found"
                return {"state": state, "reason": f"{package} {state}"}

            release_status.probe_crates_io = probe_crates_io
            row = release_status.channel_probe(
                {"kind": "crates.io"},
                {"packages": [{"kind": "crate", "name": "alpha"}, {"kind": "crate", "name": "beta"}]},
                "1.0.0",
            )
        finally:
            release_status.probe_crates_io = original_probe_crates_io

        self.assertEqual(checked, ["alpha", "beta"])
        self.assertEqual(row["state"], "missing")
        self.assertIn("beta missing", row["reason"])

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


def contract(*, surfaces: list[dict] | None = None) -> dict:
    return {
        "schema_version": 1,
        "states": [
            "published",
            "artifact-only",
            "manual-registry",
            "credential-blocked",
            "registry-blocked",
            "not-built",
            "not-applicable",
        ],
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
) -> dict:
    return {
        "id": surface_id,
        "name": "Example surface",
        "entry_point": entry_point,
        "support_level": "published",
        "dependency_weight": "low",
        "capabilities": capabilities or ["parse"],
        "docs": ["README.md"],
        "channels": channels or [channel()],
    }


def channel(
    *,
    channel_id: str = "example-channel",
    kind: str = "example",
    declared_state: str = "published",
    release_kinds: list[str] | None = None,
) -> dict:
    return {
        "id": channel_id,
        "kind": kind,
        "declared_state": declared_state,
        "release_kinds": release_kinds or ["stable", "prerelease"],
    }


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
