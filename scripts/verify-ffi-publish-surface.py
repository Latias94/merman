#!/usr/bin/env python3
"""Verify FFI ABI contracts and package-page metadata."""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
EXPECTED_ALPHA_ABI_VERSION = 2


class CheckFailure(Exception):
    pass


def read_text(rel_path: str) -> str:
    return (ROOT / rel_path).read_text(encoding="utf-8")


def require_match(rel_path: str, pattern: str, label: str) -> str:
    text = read_text(rel_path)
    match = re.search(pattern, text, flags=re.MULTILINE)
    if not match:
        raise CheckFailure(f"{rel_path}: missing {label}")
    return match.group(1)


def require_contains(rel_path: str, needle: str, label: str) -> None:
    if needle not in read_text(rel_path):
        raise CheckFailure(f"{rel_path}: missing {label}")


def require_alpha_abi(group_name: str, actual: int) -> None:
    if actual != EXPECTED_ALPHA_ABI_VERSION:
        raise CheckFailure(
            f"{group_name} must remain {EXPECTED_ALPHA_ABI_VERSION} during alpha; got {actual}"
        )


def text_measurement_abi_version() -> int:
    descriptor_path = ROOT / "abi" / "merman-v2.json"
    descriptor = json.loads(descriptor_path.read_text(encoding="utf-8"))
    version = descriptor.get("abi_version")
    if not isinstance(version, int):
        raise CheckFailure(f"{descriptor_path}: abi_version must be an integer")
    return version


def check_c_abi() -> int:
    version = text_measurement_abi_version()
    print(f"C ABI descriptor version: {version}")
    return version


def check_uniffi_abi() -> int:
    version = text_measurement_abi_version()
    print(f"Python UniFFI ABI descriptor version: {version}")
    return version


def check_wasm_abi() -> int:
    version = text_measurement_abi_version()
    print(f"WASM/Web ABI descriptor version: {version}")
    return version


def check_python_package_metadata() -> None:
    rel_path = "platforms/python/merman/pyproject.toml"
    require_contains(rel_path, 'readme = "README.md"', "PyPI README metadata")
    for label in ["Homepage", "Repository", "Documentation", "Issues", "Changelog"]:
        require_match(rel_path, rf"^{label}\s*=\s*\"([^\"]+)\"", f"project.urls {label}")

    require_contains(
        "platforms/python/merman/README.md",
        "CHANGELOG.md",
        "package changelog link",
    )
    require_contains(
        "platforms/python/merman/README.md",
        "UniFFI ABI",
        "UniFFI ABI compatibility note",
    )
    for rel_path in [
        "README.md",
        "docs/bindings/HOST_TEXT_MEASUREMENT.md",
        "platforms/python/merman/README.md",
        "platforms/python/merman/examples/smoke.py",
        "docs/bindings/PYTHON_UNIFFI.md",
    ]:
        require_contains(
            rel_path,
            "MermanTextMeasurer",
            "Python UniFFI text measurer surface",
        )
        require_contains(
            rel_path,
            "reusable_engine_with_text_measurer",
            "Python UniFFI reusable text measurer entry point",
        )
        require_contains(
            rel_path,
            "set_text_measurer",
            "Python UniFFI reusable text measurer setter",
        )
        require_contains(
            rel_path,
            "clear_text_measurer",
            "Python UniFFI reusable text measurer reset",
        )
        require_contains(
            rel_path,
            "diagram_family_capabilities",
            "Python UniFFI family capabilities entry point",
        )
    if "does not expose host text-measurement callbacks yet" in read_text("README.md"):
        raise CheckFailure("README.md: stale Python host text-measurement limitation")
    require_contains(
        "platforms/python/merman/src/merman/__init__.py",
        "MermanTextMeasurer",
        "Python UniFFI text measurer export",
    )
    require_contains(
        "platforms/python/merman/src/merman/__init__.py",
        "MermanReusableEngine",
        "Python UniFFI reusable engine export",
    )
    print("Python package page metadata: README, urls, changelog, and ABI note present")


def check_flutter_package_metadata() -> None:
    rel_path = "platforms/flutter/pubspec.yaml"
    text = read_text(rel_path)
    for field in ["homepage", "repository", "issue_tracker", "documentation"]:
        if not re.search(rf"^{field}:\s+\S+", text, flags=re.MULTILINE):
            raise CheckFailure(f"{rel_path}: missing {field}")

    topics_match = re.search(r"^topics:\s*\n((?:\s+-\s+\S+\s*\n)+)", text, flags=re.MULTILINE)
    if not topics_match:
        raise CheckFailure(f"{rel_path}: missing topics list")
    topics = {
        line.split("-", 1)[1].strip()
        for line in topics_match.group(1).splitlines()
        if "-" in line
    }
    required_topics = {"mermaid", "ffi", "flutter", "svg", "diagrams"}
    missing_topics = sorted(required_topics - topics)
    if missing_topics:
        raise CheckFailure(f"{rel_path}: missing topics {', '.join(missing_topics)}")

    require_contains(
        "platforms/flutter/README.md",
        "CHANGELOG.md",
        "package changelog link",
    )
    require_contains(
        "platforms/flutter/README.md",
        "C ABI version",
        "C ABI compatibility note",
    )
    print("Flutter package page metadata: docs links, topics, changelog, and ABI note present")


def main() -> int:
    try:
        require_alpha_abi("C ABI version", check_c_abi())
        require_alpha_abi("Python UniFFI ABI version", check_uniffi_abi())
        require_alpha_abi("WASM/Web ABI version", check_wasm_abi())
        check_python_package_metadata()
        check_flutter_package_metadata()
    except CheckFailure as exc:
        print(f"::error::{exc}", file=sys.stderr)
        return 1

    print("FFI publish surface verification completed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
