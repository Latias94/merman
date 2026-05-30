# FFI API — Handoff

Status: Active
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

## Next Task

Start with `FFI-060` only if we want to validate UniFFI now:

- prototype `crates/merman-uniffi` over the same safe facade, or
- split UniFFI into a separate packaging workstream if generated Swift/Kotlin/Python artifacts become
  larger than a proof

## Guardrails

- Do not touch ASCII work.
- Do not move unsafe code into `merman-core`, `merman-render`, or `merman`.
- Do not make UniFFI the only supported ABI.
- Do not expose Rust structs directly across the ABI.
- Keep memory ownership rules documented before adding platform wrappers.

## Suggested Next Implementation Slice

For `FFI-060`, implement only:

- a minimal UniFFI facade exposing `render_svg`, `parse_json`, and `layout_json`, or a documented
  split decision explaining why UniFFI should be a follow-on package lane
- no iOS/Android/Flutter packaging yet

Leave UniFFI for `FFI-060` or a follow-on if packaging expands.
