#!/usr/bin/env python3
"""Verify current-facing FFI documentation against the native SDK object model."""

from __future__ import annotations

from collections.abc import Iterable
from pathlib import Path
import re
import sys


REPO_ROOT = Path(__file__).resolve().parents[1]

CURRENT_DOCS = (
    Path("docs/FEATURES.md"),
    Path("docs/release/ALPHA3_TO_ALPHA4_UPGRADE_GUIDE.md"),
    Path("docs/release/FFI_CONTRACT_READINESS.md"),
    Path("docs/bindings/ABI3_MIGRATION.md"),
    Path("docs/bindings/ANDROID_JNI.md"),
    Path("docs/bindings/APPLE_SWIFT.md"),
    Path("docs/bindings/FFI_PROTOCOL.md"),
    Path("docs/bindings/FLUTTER_DART_FFI.md"),
    Path("docs/bindings/HOST_TEXT_MEASUREMENT.md"),
    Path("docs/bindings/ICON_REGISTRIES.md"),
    Path("docs/bindings/OPTIONS_JSON.md"),
    Path("docs/bindings/PYTHON_UNIFFI.md"),
    Path("docs/bindings/UNIFFI.md"),
    Path("crates/merman-ffi/README.md"),
    Path("crates/merman-uniffi/README.md"),
    Path("platforms/android/README.md"),
    Path("platforms/apple/README.md"),
    Path("platforms/flutter/README.md"),
    Path("platforms/node/README.md"),
    Path("platforms/node/packages/node/README.md"),
    Path("platforms/python/merman/README.md"),
)

FORBIDDEN_CURRENT_API = (
    re.compile(r"\bMermanReusableEngine\b"),
    re.compile(r"\bMermanNodeEngine\b"),
    re.compile(r"\breusable_engine(?:_with_text_measurer)?\s*\("),
    re.compile(r"\.reusableEngine(?:WithTextMeasurer)?\s*\("),
    re.compile(r"\bMermanEngine\.runtimeCatalogJson\s*\("),
    re.compile(r"\bMermanEngine\(\)\.runtimeCatalogJson\s*\("),
    re.compile(r"\bMermanEngine\.runtime_catalog_json\s*\("),
    re.compile(r"\bmerman\.MermanEngine\(\)"),
)

REQUIRED_TEXT = {
    Path("docs/FEATURES.md"): (
        "merman.MermanEngine(None, None)",
        "merman:^0.8.0-alpha.4",
        "direct JNI\ntransport API 1 rather than C ABI 3",
    ),
    Path("docs/release/ALPHA3_TO_ALPHA4_UPGRADE_GUIDE.md"): (
        "Android uses direct JNI transport API 1",
        "### Language SDK object-model migration",
        "analyze_document_json(source, uri,",
    ),
    Path("docs/release/FFI_CONTRACT_READINESS.md"): (
        "public-native",
        "private-node",
        "5117c0ae12da2c0346b47061642286174cea3f5f",
        "clean Cargo build-and-link wall time",
    ),
    Path("docs/bindings/ABI3_MIGRATION.md"): (
        "published six-slot prefix",
        "engine_new_with_services",
        "non-empty icon-pack services",
    ),
    Path("docs/bindings/ANDROID_JNI.md"): (
        "MermanEngine(optionsJson, services)",
        "MermanIconRegistry.fromPacks",
        "SafeInlineSvg",
    ),
    Path("docs/bindings/APPLE_SWIFT.md"): (
        "MermanEngine(optionsJson:services:)",
        "MermanEngineServices",
        "try? engine.close()",
    ),
    Path("docs/bindings/FFI_PROTOCOL.md"): (
        "published six-slot prefix",
        "engine_new_with_services` at code `6`",
        "only until return and may be released immediately after success",
    ),
    Path("docs/bindings/FLUTTER_DART_FFI.md"): (
        "stateless discovery and one-shot facade",
        "MermanEngineServices",
        "There is no `dispose()` compatibility alias",
    ),
    Path("docs/bindings/HOST_TEXT_MEASUREMENT.md"): (
        "successful close detaches them under synchronization",
        "MermanEngineServices",
    ),
    Path("docs/bindings/ICON_REGISTRIES.md"): (
        "does not turn parity/readable SVG into a browser-DOM-safe type",
        "Merman performs no icon acquisition",
    ),
    Path("docs/bindings/OPTIONS_JSON.md"): (
        "Merman.runtimeCatalogJson()",
        "Merman().runtimeCatalogJson()",
        "Merman.runtime_catalog_json()",
    ),
    Path("docs/bindings/PYTHON_UNIFFI.md"): (
        "MermanEngine(options_json, services)",
        "engine.close()",
    ),
    Path("docs/bindings/UNIFFI.md"): (
        "`Merman` for discovery, metadata, and one-shot operations",
        "`MermanEngine::close()` is explicit and idempotent",
    ),
    Path("crates/merman-ffi/README.md"): (
        "api.engine_new_with_services",
        "only until construction returns",
        "Android JNI transport code lives in the internal `merman-android-jni` crate",
    ),
    Path("crates/merman-uniffi/README.md"): (
        "`Merman` for discovery and one-shot calls",
        "`MermanEngine` for reusable calls",
        "Call `close()` deterministically",
    ),
    Path("platforms/android/README.md"): (
        "direct JNI transport",
        "MermanIconRegistry.fromPacks",
        "no native registry handle is shared",
    ),
    Path("platforms/apple/README.md"): (
        "MermanEngine(optionsJson:services:)",
        "MermanEngineServices(iconRegistry:textMeasurer:)",
        "Call `close()` deterministically",
    ),
    Path("platforms/flutter/README.md"): (
        "non-empty icon-pack services fail explicitly",
        "MermanEngineServices",
        "no separate native registry handle",
    ),
    Path("platforms/node/README.md"): (
        "Node `>=22.0.0`",
        "they are not origin authentication",
        "exchange JSON strings only",
    ),
    Path("platforms/node/packages/node/README.md"): (
        "Node `>=22.0.0`",
        "deterministic-only and text-only",
        "checks establish compatibility",
    ),
    Path("platforms/python/merman/README.md"): (
        "MermanEngine(",
        "analyze_document_facts_json(",
        "MermanEngine(options_json, services)",
    ),
}


def current_contract_text(text: str) -> str:
    """Remove migration-only sections before checking current API examples."""

    retained: list[str] = []
    skip_level: int | None = None
    for line in text.splitlines(keepends=True):
        heading = re.match(r"^(#{1,6})\s+(.+?)\s*$", line)
        if heading is not None:
            level = len(heading.group(1))
            title = heading.group(2).lower()
            if skip_level is not None and level <= skip_level:
                skip_level = None
            if "migrat" in title:
                skip_level = level
                continue
        if skip_level is None:
            retained.append(line)
    return "".join(retained)


def document_failures(path: Path, text: str) -> list[str]:
    failures: list[str] = []
    current = current_contract_text(text)
    for pattern in FORBIDDEN_CURRENT_API:
        match = pattern.search(current)
        if match is not None:
            failures.append(
                f"{path}: stale current-facing API {match.group(0)!r}"
            )
    for required in REQUIRED_TEXT.get(path, ()):
        if required not in text:
            failures.append(f"{path}: missing required contract text {required!r}")

    if path == Path("docs/bindings/FLUTTER_DART_FFI.md"):
        for stale in ("merman.close()", "merman.dispose()"):
            if stale in current:
                failures.append(f"{path}: stateless Merman must not call {stale}")

    if path in {
        Path("docs/bindings/ANDROID_JNI.md"),
        Path("platforms/android/README.md"),
    }:
        for stale in (
            "registry.close()",
            "without reparsing",
            "sealed native registry",
        ):
            if stale in current:
                failures.append(f"{path}: stale Android icon-registry claim {stale!r}")

    if path == Path("docs/release/ALPHA3_TO_ALPHA4_UPGRADE_GUIDE.md"):
        stale_android_abi = re.search(
            r"C, Flutter, (?:and|or) Android[^\n]*ABI 3",
            current,
        )
        if stale_android_abi is not None:
            failures.append(
                f"{path}: Android must not be described as a C ABI 3 transport"
            )

    if path == Path("docs/FEATURES.md") and "merman:^0.8.0-alpha.3" in current:
        failures.append(f"{path}: Flutter install example still pins alpha.3")

    if path in {
        Path("docs/bindings/PYTHON_UNIFFI.md"),
        Path("platforms/python/merman/README.md"),
    }:
        stale_document_order = re.search(
            r"analyze_document_(?:json|facts_json)\([^)]*?\n\s*None,\s*\n\s*[\"'](?:file|https?)://",
            current,
            re.DOTALL,
        )
        if stale_document_order is not None:
            failures.append(
                f"{path}: document helpers must order arguments as source, uri, options_json"
            )
    return failures


def verify_repository(root: Path = REPO_ROOT) -> tuple[str, ...]:
    failures: list[str] = []
    for relative in CURRENT_DOCS:
        path = root / relative
        try:
            text = path.read_text(encoding="utf-8")
        except OSError as error:
            failures.append(f"{relative}: cannot read document: {error}")
            continue
        failures.extend(document_failures(relative, text))
    return tuple(failures)


def format_failures(failures: Iterable[str]) -> str:
    return "FFI documentation contract failed:\n- " + "\n- ".join(failures)


def main() -> int:
    failures = verify_repository()
    if failures:
        print(format_failures(failures), file=sys.stderr)
        return 1
    print(f"ffi-contract-docs status=ok files={len(CURRENT_DOCS)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
