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

## Next Task

Start with `FFI-030`:

- add a public C header for `MermanBuffer`, `MermanResult`, result codes, `merman_render_svg`, and
  `merman_buffer_free`
- add `docs/bindings/FFI_PROTOCOL.md`
- add a C header compile/link smoke test
- keep parse/layout JSON for `FFI-040`

## Guardrails

- Do not touch ASCII work.
- Do not move unsafe code into `merman-core`, `merman-render`, or `merman`.
- Do not make UniFFI the only supported ABI.
- Do not expose Rust structs directly across the ABI.
- Keep memory ownership rules documented before adding platform wrappers.

## Suggested Next Implementation Slice

Implement only:

- `crates/merman-ffi/include/merman.h`
- `docs/bindings/FFI_PROTOCOL.md`
- a test that compiles/links a tiny C consumer against the header

Leave parse/layout JSON and UniFFI for later slices.
