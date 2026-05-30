# Platform FFI Packaging - TODO

Status: Closed
Last updated: 2026-05-30

## M0 - Scope And Evidence Freeze

- [x] PFP-010 [owner=planner] [deps=none] [scope=docs/workstreams/platform-ffi-packaging]
  Goal: Freeze the platform packaging lane around Python orchestration and Apple local
  verification.
  Validation: DESIGN.md, MILESTONES.md, EVIDENCE_AND_GATES.md, WORKSTREAM.json, and HANDOFF.md
  exist and agree.
  Evidence: docs/workstreams/platform-ffi-packaging/DESIGN.md
  Handoff: DONE. This lane reuses completed C ABI, UniFFI, and Python package workstreams.

## M1 - Cross-Platform Script Entrypoints

- [x] PFP-020 [owner=codex] [deps=PFP-010] [scope=scripts,platforms/android,platforms/flutter,bindings/python]
  Goal: Add Python equivalents for the platform verification, Android slice build, Flutter Android
  smoke, and Python UniFFI wheel scripts.
  Validation: python3 -m py_compile scripts/verify-platform-bindings.py scripts/build-python-uniffi-wheel.py platforms/android/build-android.py platforms/flutter/tool/android-smoke.py
  Review: Python scripts must preserve PS1 behavior where practical, fix platform-specific path and
  dynamic-library naming assumptions, and keep generated outputs under ignored target/platform
  directories.
  Evidence: py_compile output and updated platform docs.
  Handoff: DONE. Added Python entry points for platform verification, Android native-slice builds,
  Flutter Android smoke setup, and Python UniFFI wheel staging. Updated platform docs to prefer
  Python commands while keeping PS1 compatibility notes.

## M2 - Apple Local Verification

- [x] PFP-030 [owner=codex] [deps=PFP-020] [scope=scripts,platforms/apple,Package.swift]
  Goal: Verify the Apple C ABI wrapper path on the current macOS host, including iOS and macOS
  XCFramework slices where the toolchain permits.
  Validation: bash scripts/build-apple-xcframework.sh; swift package describe
  Review: Confirm generated `platforms/apple/Merman.xcframework` has module maps and expected
  platform slices.
  Evidence: command output plus `xcodebuild -showdestinations` or `find`/`lipo` inspection as
  appropriate.
  Handoff: DONE. `bash scripts/build-apple-xcframework.sh` produced iOS device, iOS simulator, and
  macOS slices on the current macOS arm64 host. `swift package describe` and `swift build` passed.

## M3 - Focused Binding Gates

- [x] PFP-040 [owner=codex] [deps=PFP-020] [scope=crates/merman-ffi,crates/merman-uniffi,docs/workstreams/platform-ffi-packaging]
  Goal: Run targeted Rust binding gates after script/docs changes and record the result.
  Validation: cargo nextest run -p merman-ffi && cargo nextest run -p merman-uniffi --features bindgen-smoke --test bindgen_smoke
  Review: Failures should be fixed if caused by this lane; unrelated toolchain blockers should be
  recorded with exact commands.
  Evidence: EVIDENCE_AND_GATES.md.
  Handoff: DONE. `merman-ffi` and UniFFI bindgen smoke gates passed after platform script/doc
  changes.

## M4 - Closeout

- [x] PFP-050 [owner=planner] [deps=PFP-030,PFP-040] [scope=docs/workstreams/platform-ffi-packaging]
  Goal: Close or split remaining platform packaging follow-ons.
  Validation: git diff --check -- scripts platforms bindings docs README.md Package.swift
  Review: Confirm no generated native artifacts are staged.
  Evidence: EVIDENCE_AND_GATES.md and HANDOFF.md.
  Handoff: DONE. Lane closed. Remaining package publication and CI matrix work is split into
  follow-ons.
