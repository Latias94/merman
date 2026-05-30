# FFI API — Handoff

Status: Complete
Last updated: 2026-05-30

## Current State

ADR 0066 records the binding strategy:

- stable C ABI is the canonical low-level boundary
- UniFFI is optional and should sit above the same safe facade
- existing safe crates keep unsafe code out
- SVG/JSON are the first deliverables; raster and RaTeX math stay feature-gated

`FFI-010` is complete. `DESIGN.md` now freezes the first-slice protocol:

- types: `MermanBuffer`, `MermanResult`
- first symbols: `merman_render_svg`, `merman_buffer_free`
- planned symbols: `merman_parse_json`, `merman_layout_json`
- result codes: `MERMAN_OK` through `MERMAN_INTERNAL_ERROR`
- error payload: UTF-8 JSON in `MermanResult.data`
- success payload: raw output bytes
- options: versioned tolerant JSON
- first ABI slice: stateless and thread-safe

`FFI-020` is complete. `crates/merman-ffi` now exports:

- `merman_render_svg`
- `merman_buffer_free`

The first implementation covers SVG success, readable pipeline options, invalid pointer/length
pairs, invalid UTF-8, empty/no-diagram input, invalid options JSON, feature-gated RaTeX handling,
null buffer free, and panic containment.

`FFI-030` is complete:

- `crates/merman-ffi/include/merman.h` documents the C ABI.
- `docs/bindings/FFI_PROTOCOL.md` documents result codes, memory ownership, inputs, errors, and
  options.
- `crates/merman-ffi/tests/header_smoke.rs` compiles a small C consumer against the header.

`FFI-040` is complete:

- `merman_parse_json` returns semantic model JSON.
- `merman_layout_json` returns `LayoutedDiagram` JSON.
- Both functions use the same buffer ownership and structured error JSON policy as
  `merman_render_svg`.
- The public header and protocol document include the new functions.

`FFI-050` is complete with concerns:

- RaTeX math is already available through `layout.math_renderer = "ratex"` when the `ratex-math`
  feature is enabled.
- `raster,ratex-math` feature gates pass.
- PNG/JPEG/PDF FFI functions are intentionally deferred. They should be split into a narrower
  follow-on when a downstream host actually needs raster bytes from the native library.

`FFI-060` is complete with concerns:

- Current `uniffi` is `0.31.1`.
- UniFFI is suitable for generated Swift/Kotlin/Python/Ruby bindings, but it should not redefine the
  canonical C ABI.
- A proper UniFFI lane should first extract or formalize a shared safe bindings facade so options
  parsing, renderer setup, and error mapping are not duplicated.
- No UniFFI crate was added in this lane.

`FFI-070` is complete. The first FFI release candidate scope is:

- `merman_render_svg`
- `merman_parse_json`
- `merman_layout_json`
- `merman_buffer_free`
- public C header
- protocol doc
- C header smoke test

## Follow-ons

Open separate workstreams when needed:

- `ffi-raster-output`: add feature-gated PNG/JPEG/PDF functions and raster byte protocol.
- `uniffi-bindings`: extract/formalize a shared safe bindings facade, then add a minimal UniFFI
  crate and generated binding smoke tests.
- platform package lanes: iOS XCFramework, Android/JNI or Kotlin, Flutter/Dart FFI, Node.

## Guardrails

- Do not touch ASCII work.
- Do not move unsafe code into `merman-core`, `merman-render`, or `merman`.
- Do not make UniFFI the only supported ABI.
- Do not expose Rust structs directly across the ABI.
- Keep memory ownership rules documented before adding platform wrappers.

## Suggested Next Implementation Slice

For `uniffi-bindings`, implement only:

- a minimal UniFFI facade exposing `render_svg`, `parse_json`, and `layout_json`, or a documented
  split decision explaining why UniFFI should be a follow-on package lane
- no iOS/Android/Flutter packaging yet

For `ffi-raster-output`, implement only one raster format first, preferably PNG.
