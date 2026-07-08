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
    metadata = cargo_metadata()
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

    publishable_packages = publishable_workspace_packages(metadata)
    publishable = list(publishable_packages)
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

    failed |= report_topology_violations(expected, publishable_packages)

    return 1 if failed else 0


def workflow_publish_order() -> list[str]:
    path = ROOT / ".github/workflows/release-crates.yml"
    text = path.read_text()
    orders = extract_workflow_publish_orders(text)
    if not orders:
        raise ValueError("could not find crates=(...) in .github/workflows/release-crates.yml")
    expected = orders[0]
    for index, order in enumerate(orders[1:], start=2):
        if order != expected:
            raise ValueError(
                "crates=(...) array "
                f"#{index} in .github/workflows/release-crates.yml does not match array #1"
            )
    return expected


def extract_workflow_publish_orders(text: str) -> list[list[str]]:
    matches = re.finditer(
        r"^\s+crates=\(\n(?P<body>.*?)^\s+\)",
        text,
        flags=re.MULTILINE | re.DOTALL,
    )
    return [shell_array_body(match.group("body")) for match in matches]


def shell_array_body(body: str) -> list[str]:
    return [
        line.strip()
        for line in body.splitlines()
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


def cargo_metadata() -> dict:
    result = subprocess.run(
        ["cargo", "metadata", "--locked", "--no-deps", "--format-version", "1"],
        cwd=ROOT,
        check=True,
        stdout=subprocess.PIPE,
        text=True,
    )
    return json.loads(result.stdout)


def publish_field_allows_crates_io(publish_raw: object) -> bool:
    if publish_raw is None or publish_raw is True:
        return True
    if isinstance(publish_raw, list):
        return "crates-io" in publish_raw
    return False


def publishable_workspace_packages(metadata: dict) -> dict[str, dict]:
    return {
        package["name"]: package
        for package in metadata["packages"]
        if publish_field_allows_crates_io(package.get("publish"))
    }


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


def report_topology_violations(order: list[str], packages: dict[str, dict]) -> bool:
    index = {crate: position for position, crate in enumerate(order)}
    violations = []
    edge_count = 0
    for crate, package in packages.items():
        if crate not in index:
            continue
        for dependency in package.get("dependencies", []):
            if dependency.get("kind") == "dev":
                continue
            dependency_name = dependency["name"]
            if dependency_name not in packages:
                continue
            if dependency_name not in index:
                continue
            edge_count += 1
            if index[dependency_name] > index[crate]:
                kind = dependency.get("kind") or "normal"
                violations.append((crate, dependency_name, kind))

    if not violations:
        print(f"cargo metadata topology: {edge_count} workspace dependency edges")
        return False

    for crate, dependency_name, kind in violations:
        error(
            "metadata",
            (
                f"release order publishes {crate} before its {kind} workspace dependency "
                f"{dependency_name}"
            ),
        )
    return True


def error(path: str, message: str) -> None:
    print(f"::error file={path}::{message}", file=sys.stderr)


if __name__ == "__main__":
    raise SystemExit(main())
