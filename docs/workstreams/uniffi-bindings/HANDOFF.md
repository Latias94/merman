# UniFFI Bindings — Handoff

Status: Active
Last updated: 2026-05-30

## Current State

`UBI-020` is complete. `crates/merman-bindings-core` is now the shared safe facade used by external
binding crates.

Confirmed dependency context:

- `cargo search uniffi --limit 3` reports `uniffi = "0.31.1"`.
- `cargo info uniffi@0.31.1` reports features including `bindgen`, `build`, `cli`, and
  `scaffolding-ffi-buffer-fns`.

Completed facade shape:

- safe facade accepts `&[u8]` source and `&[u8]` options JSON
- returns `Vec<u8>` on success
- exposes `BindingStatus`, `BindingError`, and JSON error payload serialization
- owns renderer setup, options parsing, pipeline selection, and feature-gated RaTeX selection
- keeps unsafe pointer and buffer ownership inside `merman-ffi`

## Next Task

`UBI-030`: add `crates/merman-uniffi` exposing `render_svg`, `parse_json`, and `layout_json` over
`merman-bindings-core`.

Recommended shape:

- use UniFFI 0.31.1
- expose a small `MermanEngine` object or equivalent free functions
- map `BindingError` into UniFFI-compatible structured errors
- keep generated platform packages out of this task

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
