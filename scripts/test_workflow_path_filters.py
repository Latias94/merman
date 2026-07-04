#!/usr/bin/env python3
"""Unit tests for workflow path filters that protect release gates."""

from __future__ import annotations

import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def workflow_event_paths(relative_path: str, event_name: str) -> list[str]:
    lines = (ROOT / relative_path).read_text().splitlines()
    event_line = f"  {event_name}:"
    try:
        start = lines.index(event_line) + 1
    except ValueError as exc:
        raise AssertionError(f"{relative_path} does not define on.{event_name}") from exc

    paths: list[str] = []
    in_paths = False
    for line in lines[start:]:
        stripped = line.strip()
        indent = len(line) - len(line.lstrip(" "))

        if indent <= 2 and stripped.endswith(":"):
            break

        if indent == 4 and stripped == "paths:":
            in_paths = True
            continue

        if not in_paths:
            continue

        if indent == 6 and stripped.startswith("- "):
            paths.append(stripped[2:].strip().strip("\"'"))
            continue

        if stripped == "":
            continue

        if indent <= 4:
            break

    return paths


class WorkflowPathFilterTests(unittest.TestCase):
    def test_pages_paths_cover_web_prepack_inputs(self) -> None:
        required_paths = {
            ".github/workflows/pages.yml",
            "Cargo.lock",
            "Cargo.toml",
            "crates/**",
            "docs/release/WASM_SIZE_BUDGETS.json",
            "platforms/web/**",
            "playground/**",
        }

        for event_name in ("push", "pull_request"):
            with self.subTest(event_name=event_name):
                self.assert_event_paths_include(
                    ".github/workflows/pages.yml",
                    event_name,
                    required_paths,
                )

    def test_vscode_extension_paths_cover_vsix_inputs(self) -> None:
        required_paths = {
            ".github/workflows/vscode-extension.yml",
            "Cargo.lock",
            "Cargo.toml",
            "crates/**",
            "tools/vscode-extension/**",
        }

        for event_name in ("push", "pull_request"):
            with self.subTest(event_name=event_name):
                self.assert_event_paths_include(
                    ".github/workflows/vscode-extension.yml",
                    event_name,
                    required_paths,
                )

    def assert_event_paths_include(
        self,
        workflow_path: str,
        event_name: str,
        required_paths: set[str],
    ) -> None:
        actual_paths = set(workflow_event_paths(workflow_path, event_name))
        missing = sorted(required_paths - actual_paths)
        self.assertEqual(missing, [], f"{workflow_path} on.{event_name} is missing paths")


if __name__ == "__main__":
    unittest.main()
