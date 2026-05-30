# UniFFI Bindings — Handoff

Status: Active
Last updated: 2026-05-30

## Current State

`UBI-020` and `UBI-030` are complete. `crates/merman-bindings-core` is now the shared safe facade,
and `crates/merman-uniffi` exposes the minimal UniFFI object surface.

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

Completed UniFFI shape:

- `MermanEngine::new()`
- `MermanEngine::render_svg(source, options_json)`
- `MermanEngine::parse_json(source, options_json)`
- `MermanEngine::layout_json(source, options_json)`
- `MermanError::Binding { code, code_name, message }`

## Next Task

`UBI-040`: run a generated binding smoke into a temporary output directory. Do not commit generated
Swift/Kotlin/Python/Ruby artifacts unless a packaging lane explicitly decides to track them.

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
