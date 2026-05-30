# UniFFI Bindings — Handoff

Status: Active
Last updated: 2026-05-30

## Current State

This lane is open and ready for implementation. The first task is not to add UniFFI macros; it is to
extract a safe shared binding facade so C ABI and UniFFI call the same behavior.

Confirmed dependency context:

- `cargo search uniffi --limit 3` reports `uniffi = "0.31.1"`.
- `cargo info uniffi@0.31.1` reports features including `bindgen`, `build`, `cli`, and
  `scaffolding-ffi-buffer-fns`.

## Next Task

`UBI-020`: add `crates/merman-bindings-core` or an equivalent safe facade and refactor
`merman-ffi` to delegate to it without changing public C symbols.

Recommended shape:

- safe facade accepts `&[u8]` source and `&[u8]` options JSON
- returns `Vec<u8>` on success
- exposes structured binding errors with the existing result codes/code names
- owns renderer setup, options parsing, and feature-gated RaTeX selection

## Guardrails

- Do not touch ASCII workstreams or renderer code.
- Do not move unsafe code into `merman-core`, `merman-render`, `merman`, or the shared facade.
- Do not make UniFFI the only supported ABI.
- Do not expose renderer internals or Rust layout structs directly as UniFFI API.
- Keep platform packaging out of this lane.

## Expected Follow-Ons

- `ios-xcframework`
- `android-kotlin-jni`
- `python-uniffi-package`
- `flutter-dart-ffi`
- `ffi-raster-output`
- workspace release-versioning/publish-order lane before a real crate release
