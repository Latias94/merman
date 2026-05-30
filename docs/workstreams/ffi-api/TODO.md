# FFI API — TODO

Status: Draft
Last updated: 2026-05-30

## M0 — Scope And Protocol Freeze

- [ ] FFI-010 [owner=planner] [deps=none] [scope=docs/adr,docs/workstreams/ffi-api]
  Goal: Freeze the FFI strategy, initial protocol, and non-goals.
  Validation: ADR 0066, DESIGN.md, MILESTONES.md, EVIDENCE_AND_GATES.md, and WORKSTREAM.json agree.
  Evidence: docs/adr/0066-ffi-binding-strategy.md
  Handoff: Keep this lane draft until the protocol names and result-code policy are reviewed.

## M1 — C ABI SVG Proof

- [ ] FFI-020 [owner=unassigned] [deps=FFI-010] [scope=crates/merman-ffi]
  Goal: Add `crates/merman-ffi` with `render_svg` C ABI proof, owned buffers, and explicit free.
  Validation: cargo nextest run -p merman-ffi
  Review: review-workstream before accepting completion.
  Evidence: crates/merman-ffi tests proving success, invalid UTF-8, null pointer, and free behavior.
  Handoff: The slice is not done until panic-safe ABI behavior is covered.

- [ ] FFI-030 [owner=unassigned] [deps=FFI-020] [scope=crates/merman-ffi,docs/bindings]
  Goal: Add a public C header and protocol doc for result codes, buffers, options, and errors.
  Validation: C header compile/link smoke test.
  Review: Check ABI naming and memory ownership clarity.
  Evidence: docs/bindings/FFI_PROTOCOL.md and a C smoke test.
  Handoff: Header docs must be enough for a non-Rust caller to free all outputs correctly.

## M2 — JSON Surfaces

- [ ] FFI-040 [owner=unassigned] [deps=FFI-030] [scope=crates/merman-ffi]
  Goal: Expose `parse_json` and `layout_json` through the same ABI pattern.
  Validation: cargo nextest run -p merman-ffi parse_json layout_json
  Review: Ensure output schema is documented as versioned/tolerant rather than byte-stable.
  Evidence: parse/layout C ABI tests.
  Handoff: Do not add raster until SVG/JSON behavior is stable.

## M3 — Optional Feature Surfaces

- [ ] FFI-050 [owner=unassigned] [deps=FFI-040] [scope=crates/merman-ffi]
  Goal: Add feature-gated RaTeX math and raster output options if required by downstream hosts.
  Validation: cargo nextest run -p merman-ffi --features raster,ratex-math
  Review: Confirm default library size and feature matrix stay intentional.
  Evidence: feature-gated tests and size/build notes.
  Handoff: Split raster into a follow-on if the first ABI release should remain SVG-only.

- [ ] FFI-060 [owner=unassigned] [deps=FFI-040] [scope=crates/merman-uniffi,docs/bindings]
  Goal: Prototype a UniFFI facade over the safe binding API.
  Validation: cargo check -p merman-uniffi and generated binding smoke tests where available.
  Review: Keep UniFFI optional; do not let generated APIs redefine the canonical C ABI.
  Evidence: UniFFI smoke docs and generated binding notes.
  Handoff: This task may split into a separate workstream if packaging becomes platform-heavy.

## M4 — Closeout

- [ ] FFI-070 [owner=planner] [deps=FFI-050,FFI-060] [scope=docs/workstreams/ffi-api]
  Goal: Close or split the lane after the first ABI release candidate is verified.
  Validation: verify-rust-workstream records fresh gate evidence.
  Review: review-workstream has no blocking findings.
  Evidence: EVIDENCE_AND_GATES.md, WORKSTREAM.json, HANDOFF.md.
  Handoff: Record remaining platform wrappers as follow-on workstreams.
