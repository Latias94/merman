# FFI API — Handoff

Status: Draft
Last updated: 2026-05-30

## Current State

ADR 0066 records the binding strategy:

- stable C ABI is the canonical low-level boundary
- UniFFI is optional and should sit above the same safe facade
- existing safe crates keep unsafe code out
- SVG/JSON are the first deliverables; raster and RaTeX math stay feature-gated

The workstream is open as a draft because the exact C symbol names, result-code enum, and error
payload format still need a short review before implementation.

## Next Task

Start with `FFI-010`:

- review `DESIGN.md` and ADR 0066 for naming/protocol gaps
- decide whether non-zero errors return UTF-8 text or structured JSON
- decide final names for `MermanBuffer`, `MermanResult`, and first exported functions
- then mark `FFI-010` complete and begin `FFI-020`

## Guardrails

- Do not touch ASCII work.
- Do not move unsafe code into `merman-core`, `merman-render`, or `merman`.
- Do not make UniFFI the only supported ABI.
- Do not expose Rust structs directly across the ABI.
- Keep memory ownership rules documented before adding platform wrappers.

## Suggested First Implementation Slice

Implement only:

- `crates/merman-ffi`
- `merman_render_svg`
- `merman_buffer_free`
- panic containment
- invalid pointer/UTF-8 tests
- one successful flowchart SVG smoke test

Leave parse/layout JSON and UniFFI for later slices.
