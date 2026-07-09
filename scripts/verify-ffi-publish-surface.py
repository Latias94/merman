#!/usr/bin/env python3
"""Verify FFI ABI contracts and package-page metadata."""

from __future__ import annotations

import re
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


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


def check_equal_versions(group_name: str, entries: list[tuple[str, str, str]]) -> int:
    values: list[tuple[str, str]] = []
    for label, rel_path, pattern in entries:
        values.append((label, require_match(rel_path, pattern, label)))

    unique_values = {value for _, value in values}
    if len(unique_values) != 1:
        details = ", ".join(f"{label}={value}" for label, value in values)
        raise CheckFailure(f"{group_name} mismatch: {details}")

    value = values[0][1]
    print(f"{group_name}: {value}")
    return int(value)


def check_c_abi() -> int:
    return check_equal_versions(
        "C ABI version",
        [
            (
                "Rust constant",
                "crates/merman-ffi/src/lib.rs",
                r"pub const MERMAN_ABI_VERSION: u32 = (\d+);",
            ),
            (
                "C header",
                "crates/merman-ffi/include/merman.h",
                r"#define\s+MERMAN_ABI_VERSION\s+(\d+)",
            ),
            (
                "protocol docs",
                "docs/bindings/FFI_PROTOCOL.md",
                r"#define\s+MERMAN_ABI_VERSION\s+(\d+)",
            ),
            (
                "Android wrapper",
                "platforms/android/src/main/kotlin/io/merman/MermanEngine.kt",
                r"const val ABI_VERSION: Int = (\d+)",
            ),
            (
                "Apple wrapper",
                "platforms/apple/Sources/Merman/MermanEngine.swift",
                r"public static let abiVersion: UInt32 = (\d+)",
            ),
            (
                "Flutter wrapper",
                "platforms/flutter/lib/src/merman_ffi.dart",
                r"const int mermanAbiVersion = (\d+);",
            ),
        ],
    )


def check_uniffi_abi() -> int:
    return check_equal_versions(
        "Python UniFFI ABI version",
        [
            (
                "Rust constant",
                "crates/merman-uniffi/src/lib.rs",
                r"pub const MERMAN_UNIFFI_ABI_VERSION: u32 = (\d+);",
            ),
            (
                "wheel smoke",
                "scripts/build-python-uniffi-wheel.py",
                r"abi_version\(\)\s*(?:==|!=)\s*(\d+)",
            ),
            (
                "release smoke",
                ".github/workflows/release-python.yml",
                r"abi_version\(\)\s*(?:==|!=)\s*(\d+)",
            ),
            (
                "binding docs",
                "docs/bindings/PYTHON_UNIFFI.md",
                r"abi_version\(\)\s*(?:==|!=)\s*(\d+)",
            ),
            (
                "package README",
                "platforms/python/merman/README.md",
                r"abi_version\(\)\s*(?:==|!=)\s*(\d+)",
            ),
            (
                "package example",
                "platforms/python/merman/examples/smoke.py",
                r"abi_version\(\)\s*(?:==|!=)\s*(\d+)",
            ),
        ],
    )


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
        check_c_abi()
        check_uniffi_abi()
        check_python_package_metadata()
        check_flutter_package_metadata()
    except CheckFailure as exc:
        print(f"::error::{exc}", file=sys.stderr)
        return 1

    print("FFI publish surface verification completed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
