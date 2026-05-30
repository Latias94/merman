# Platform FFI Packaging - Handoff

Status: Closed
Last updated: 2026-05-30

## Current State

The lane is closed. It hardened platform FFI package verification by adding cross-platform Python
entry points and proving the Apple path locally on macOS.

Completed:

- `PFP-010`: scope and evidence docs created.
- `PFP-020`: Python equivalents added for platform verification, Android native slices, Flutter
  Android smoke, and Python UniFFI wheel staging.
- `PFP-030`: Apple iOS/macOS XCFramework generated and SwiftPM verified on macOS.
- `PFP-040`: focused `merman-ffi` and UniFFI bindgen smoke gates passed.
- `PFP-050`: lane closed with publication and CI matrix work split out.

## Verification Summary

- `python3 -m py_compile ...` passed for all new Python scripts.
- `bash scripts/build-apple-xcframework.sh` passed.
- `swift package describe` passed.
- `swift build` passed.
- `cargo nextest run -p merman-ffi` passed (`15` tests).
- `cargo nextest run -p merman-uniffi --features bindgen-smoke --test bindgen_smoke` passed (`2`
  tests).
- `git diff --check -- .gitignore scripts platforms bindings docs README.md Package.swift` passed.

## Guardrails

- Do not change the canonical C ABI protocol.
- Do not delete existing PS1 scripts in this lane.
- Do not commit generated XCFramework, Android `jniLibs`, Python generated bindings, wheels, or
  test app output.
- Keep platform wrappers thin over `merman-ffi` or `merman-uniffi`.

## Expected Follow-Ons

- CI matrix for Android/Flutter/Apple packaging.
- Published binary SwiftPM target with checksum.
- AAR, PyPI, and pub.dev packaging once release repositories are chosen.
- Android/Flutter full packaging smoke on a host with Android SDK/NDK, Gradle, Kotlin, and Flutter
  installed. At closeout, this macOS host had Flutter and `ANDROID_HOME`, but `kotlinc` and `gradle`
  were not on `PATH`.
