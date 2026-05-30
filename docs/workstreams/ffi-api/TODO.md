# FFI API — TODO

Status: Complete
Last updated: 2026-05-30

## M0 — Scope And Protocol Freeze

- [x] FFI-010 [owner=planner] [deps=none] [scope=docs/adr,docs/workstreams/ffi-api]
  Goal: Freeze the FFI strategy, initial protocol, and non-goals.
  Validation: ADR 0066, DESIGN.md, MILESTONES.md, EVIDENCE_AND_GATES.md, and WORKSTREAM.json agree.
  Evidence: docs/adr/0066-ffi-binding-strategy.md
  Handoff: DONE. `DESIGN.md` freezes names, result codes, error JSON, buffer ownership, input rules,
  options JSON, and threading for the first ABI slice.

## M1 — C ABI SVG Proof

- [x] FFI-020 [owner=codex] [deps=FFI-010] [scope=crates/merman-ffi]
  Goal: Add `crates/merman-ffi` with `render_svg` C ABI proof, owned buffers, and explicit free.
  Validation: cargo nextest run -p merman-ffi
  Review: review-workstream before accepting completion.
  Evidence: crates/merman-ffi tests proving success, invalid UTF-8, null pointer, and free behavior.
  Handoff: DONE. Added `merman_render_svg`, `merman_buffer_free`, owned buffer transfer, structured
  error JSON, options JSON parsing, panic containment, and focused ABI tests.

- [x] FFI-030 [owner=codex] [deps=FFI-020] [scope=crates/merman-ffi,docs/bindings]
  Goal: Add a public C header and protocol doc for result codes, buffers, options, and errors.
  Validation: C header compile/link smoke test.
  Review: Check ABI naming and memory ownership clarity.
  Evidence: docs/bindings/FFI_PROTOCOL.md and a C smoke test.
  Handoff: DONE. Added `include/merman.h`, `docs/bindings/FFI_PROTOCOL.md`, and
  `header_smoke` C compile coverage.

## M2 — JSON Surfaces

- [x] FFI-040 [owner=codex] [deps=FFI-030] [scope=crates/merman-ffi]
  Goal: Expose `parse_json` and `layout_json` through the same ABI pattern.
  Validation: cargo nextest run -p merman-ffi parse_json layout_json
  Review: Ensure output schema is documented as versioned/tolerant rather than byte-stable.
  Evidence: parse/layout C ABI tests.
  Handoff: DONE. Added `merman_parse_json`, `merman_layout_json`, header/protocol docs, and ABI
  tests for semantic JSON, layout JSON, and shared error payloads.

## M3 — Optional Feature Surfaces

- [x] FFI-050 [owner=codex] [deps=FFI-040] [scope=crates/merman-ffi]
  Goal: Add feature-gated RaTeX math and raster output options if required by downstream hosts.
  Validation: cargo nextest run -p merman-ffi --features raster,ratex-math
  Review: Confirm default library size and feature matrix stay intentional.
  Evidence: feature-gated tests and size/build notes.
  Handoff: DONE_WITH_CONCERNS. RaTeX math is already exposed as a feature-gated SVG option.
  `raster,ratex-math` feature gates pass, but PNG/JPEG/PDF functions are intentionally split out
  until a real downstream host asks for first-release raster ABI.

- [x] FFI-060 [owner=codex] [deps=FFI-050] [scope=crates/merman-uniffi,docs/bindings]
  Goal: Prototype a UniFFI facade over the safe binding API.
  Validation: cargo check -p merman-uniffi and generated binding smoke tests where available.
  Review: Keep UniFFI optional; do not let generated APIs redefine the canonical C ABI.
  Evidence: UniFFI smoke docs and generated binding notes.
  Handoff: DONE_WITH_CONCERNS. Split UniFFI into a follow-on package lane. UniFFI 0.31.1 is
  available and production-used, but it is pre-1.0 and would require a shared safe bindings facade
  or duplicated options/error plumbing before it is worth adding.

## M4 — Closeout

- [x] FFI-070 [owner=planner] [deps=FFI-050,FFI-060] [scope=docs/workstreams/ffi-api]
  Goal: Close or split the lane after the first ABI release candidate is verified.
  Validation: verify-rust-workstream records fresh gate evidence.
  Review: review-workstream has no blocking findings.
  Evidence: EVIDENCE_AND_GATES.md, WORKSTREAM.json, HANDOFF.md.
  Handoff: DONE_WITH_CONCERNS. First C ABI release candidate is in place for SVG, parse JSON, and
  layout JSON. Raster and UniFFI are explicit follow-ons.
