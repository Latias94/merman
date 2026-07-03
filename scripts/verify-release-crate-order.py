#!/usr/bin/env python3
"""Verify crates.io publish order docs and automation stay in sync."""

from __future__ import annotations

import ast
import json
import re
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def main() -> int:
    expected = workflow_publish_order()
    sources = {
        ".github/workflows/release-crates.yml": expected,
        "tools/publish.py": tools_publish_order(),
        "docs/release/PUBLISH_ORDER.md": markdown_publish_order(
            "docs/release/PUBLISH_ORDER.md",
            start="## Publish Order",
            end="## Binding Release Chain",
        ),
        "docs/releasing/CRATES_IO.md": markdown_publish_order(
            "docs/releasing/CRATES_IO.md",
            start="## Recommended publish order",
            end="Example:",
        ),
        "docs/releasing/PUBLISHING.md": markdown_publish_order(
            "docs/releasing/PUBLISHING.md",
            start="Recommended order:",
            end="## Dry runs",
        ),
    }

    failed = False
    for path, order in sources.items():
        failed |= report_duplicates(path, order)
        if order == expected:
            print(f"{path}: {len(order)} crates")
            continue
        failed = True
        report_order_mismatch(path, expected, order)

    publishable = publishable_workspace_crates()
    expected_set = set(expected)
    publishable_set = set(publishable)
    if publishable_set != expected_set:
        failed = True
        missing = sorted(publishable_set - expected_set)
        extra = sorted(expected_set - publishable_set)
        if missing:
            error(
                "metadata",
                "publishable crates missing from release order: " + ", ".join(missing),
            )
        if extra:
            error(
                "metadata",
                "release order contains non-publishable crates: " + ", ".join(extra),
            )
    else:
        print(f"cargo metadata: {len(publishable)} publishable crates")

    return 1 if failed else 0


def workflow_publish_order() -> list[str]:
    path = ROOT / ".github/workflows/release-crates.yml"
    text = path.read_text()
    match = re.search(r"^\s+crates=\(\n(?P<body>.*?)^\s+\)", text, flags=re.MULTILINE | re.DOTALL)
    if not match:
        raise ValueError("could not find crates=(...) in .github/workflows/release-crates.yml")
    return [
        line.strip()
        for line in match.group("body").splitlines()
        if line.strip() and not line.strip().startswith("#")
    ]


def tools_publish_order() -> list[str]:
    path = ROOT / "tools/publish.py"
    module = ast.parse(path.read_text(), filename=str(path))
    for node in module.body:
        if isinstance(node, ast.Assign):
            for target in node.targets:
                if isinstance(target, ast.Name) and target.id == "PUBLISH_ORDER":
                    return literal_string_list(node.value, "tools/publish.py PUBLISH_ORDER")
    raise ValueError("could not find PUBLISH_ORDER in tools/publish.py")


def markdown_publish_order(relative_path: str, *, start: str, end: str) -> list[str]:
    path = ROOT / relative_path
    text = path.read_text()
    try:
        body = text.split(start, 1)[1].split(end, 1)[0]
    except IndexError as exc:
        raise ValueError(f"could not find publish order section in {relative_path}") from exc
    return re.findall(r"^\d+\.\s+`([^`]+)`", body, flags=re.MULTILINE)


def publishable_workspace_crates() -> list[str]:
    result = subprocess.run(
        ["cargo", "metadata", "--no-deps", "--format-version", "1"],
        cwd=ROOT,
        check=True,
        stdout=subprocess.PIPE,
        text=True,
    )
    metadata = json.loads(result.stdout)
    return [
        package["name"]
        for package in metadata["packages"]
        if package.get("publish") != []
    ]


def literal_string_list(node: ast.AST, label: str) -> list[str]:
    if not isinstance(node, ast.List):
        raise ValueError(f"{label} must be a list literal")
    values = []
    for item in node.elts:
        if not isinstance(item, ast.Constant) or not isinstance(item.value, str):
            raise ValueError(f"{label} must contain only string literals")
        values.append(item.value)
    return values


def report_duplicates(path: str, order: list[str]) -> bool:
    seen = set()
    duplicates = []
    for crate in order:
        if crate in seen:
            duplicates.append(crate)
        seen.add(crate)
    if not duplicates:
        return False
    error(path, "duplicate crates in publish order: " + ", ".join(sorted(set(duplicates))))
    return True


def report_order_mismatch(path: str, expected: list[str], actual: list[str]) -> None:
    error(path, "publish order does not match .github/workflows/release-crates.yml")
    expected_set = set(expected)
    actual_set = set(actual)
    missing = [crate for crate in expected if crate not in actual_set]
    extra = [crate for crate in actual if crate not in expected_set]
    if missing:
        error(path, "missing crates: " + ", ".join(missing))
    if extra:
        error(path, "extra crates: " + ", ".join(extra))
    for index, (left, right) in enumerate(zip(expected, actual), start=1):
        if left != right:
            error(path, f"first order mismatch at #{index}: expected {left}, found {right}")
            return
    if len(expected) != len(actual):
        error(path, f"length mismatch: expected {len(expected)}, found {len(actual)}")


def error(path: str, message: str) -> None:
    print(f"::error file={path}::{message}", file=sys.stderr)


if __name__ == "__main__":
    raise SystemExit(main())
