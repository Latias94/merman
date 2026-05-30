# Platform FFI Packaging

Status: Closed
Last updated: 2026-05-30

## Why This Lane Exists

The core native binding layers are already in place:

- `merman-ffi` exposes the canonical C ABI for SVG, semantic JSON, and layout JSON.
- `merman-bindings-core` centralizes safe options, rendering, and error mapping.
- `merman-uniffi` exposes the same safe facade to generated bindings.
- Experimental Apple, Android, Flutter, and Python package scaffolds exist.

This lane turns those pieces into repeatable platform verification flows. The immediate trigger is
the RaTeX reference pattern: keep one small Rust FFI core, wrap it per platform, and make local
platform smoke checks easy to run from a macOS development machine.

## Relevant Authority

- `docs/adr/0066-ffi-binding-strategy.md`
- `docs/bindings/FFI_PROTOCOL.md`
- `docs/workstreams/ffi-api/HANDOFF.md`
- `docs/workstreams/ffi-release-hardening/HANDOFF.md`
- `docs/workstreams/uniffi-bindings/HANDOFF.md`
- `docs/workstreams/python-uniffi-package/HANDOFF.md`
- Local reference: `repo-ref/RaTeX/docs/binding-architecture.md`
- Local reference: `repo-ref/RaTeX/scripts/build-apple-xcframework.sh`
- Local reference: `repo-ref/RaTeX/Package.swift`

## Problem

Platform wrappers are present, but some verification entry points are PowerShell-only and the Apple
path needs fresh local evidence on macOS. That makes it harder to use the same scripts from macOS,
Linux, Windows, and CI, and harder to prove that iOS/macOS slices and SwiftPM packaging still match
the C ABI.

## Target State

- Cross-platform Python entry points exist for platform verification, Android native-slice builds,
  Flutter Android smoke setup, and Python UniFFI wheel staging.
- Existing PowerShell scripts remain as compatibility wrappers or legacy alternatives.
- Apple XCFramework generation can be run on the current macOS host and records iOS/macOS evidence.
- Docs prefer Python commands for cross-platform flows while preserving platform-specific notes.
- Generated native artifacts remain ignored by git.

## In Scope

- `scripts/verify-platform-bindings.py`
- `scripts/build-python-uniffi-wheel.py`
- `platforms/android/build-android.py`
- `platforms/flutter/tool/android-smoke.py`
- Apple build script and SwiftPM verification docs if needed.
- Binding docs and README command examples.
- Workstream evidence.

## Out Of Scope

- Changing the C ABI protocol.
- Replacing the canonical C ABI with UniFFI.
- Publishing PyPI, AAR, SwiftPM binary, CocoaPods, or pub.dev packages.
- Adding raster FFI functions.
- Reworking ASCII, parser, or renderer parity behavior.

## Starting Assumptions

| Assumption | Confidence | Evidence | Consequence if wrong |
| --- | --- | --- | --- |
| Keeping PS1 scripts is safer than deleting them. | High | Existing docs and scripts reference them. | Delete only in a later cleanup once Python entry points are adopted. |
| Python is the right cross-platform orchestration layer. | High | User requested Python and scripts mostly shell out to Rust/platform tools. | Use Rust xtask only if logic needs workspace-native integration. |
| Apple build can be verified locally on the current macOS arm64 host. | High | `xcodebuild`, `lipo`, and `swift` are present. | Record missing target/tool blockers as evidence rather than weakening scripts. |
| Android and Flutter full packaging remain optional local gates. | Medium | They require Android SDK/NDK, Gradle, Flutter, and Kotlin. | Keep their Python scripts robust, but record skipped gates when tools are unavailable. |

## Architecture Direction

Mirror the RaTeX wrapper architecture:

```text
host wrapper (Swift/Kotlin/Dart/Python)
        |
        v
canonical merman-ffi C ABI or merman-uniffi generated ABI
        |
        v
merman-bindings-core safe facade
        |
        v
merman render/parser crates
```

Scripts should only orchestrate builds and smoke checks. They must not encode rendering semantics or
copy business logic out of Rust. Python entry points should use `pathlib`, `subprocess.run(...,
check=True)`, platform-aware library names, and explicit failures for missing external toolchains.

## Closeout Condition

Closed on 2026-05-30 after Python platform entry points were added, docs were updated to prefer
them, Apple iOS/macOS XCFramework generation was verified on macOS, SwiftPM could describe and build
the local package, and focused Rust/script gates were recorded in `EVIDENCE_AND_GATES.md`.
