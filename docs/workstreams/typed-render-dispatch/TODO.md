# Typed Render Dispatch - TODO

Status: Complete
Last updated: 2026-05-28

## M0 - Scope And Evidence Freeze

- [x] TRD-010 [owner=planner] [deps=none] [scope=docs/workstreams/typed-render-dispatch]
  Goal: Freeze the dispatch duplication problem, target boundary, and validation gates.
  Validation: workstream docs exist and agree.
  Evidence: `docs/workstreams/typed-render-dispatch/DESIGN.md`
  Handoff: DONE.

## M1 - Model-Owned Metadata

- [x] TRD-020 [owner=codex] [deps=TRD-010] [scope=crates/merman-core/src/diagram/mod.rs,crates/merman-core/src/lib.rs]
  Goal: Move render model kind and alias compatibility onto `RenderSemanticModel`.
  Validation: `cargo nextest run -p merman-core render_semantic_model`
  Review: `review-workstream` before accepting completion.
  Evidence: `crates/merman-core/src/tests/misc.rs::render_semantic_model_*`
  Handoff: DONE.

## M2 - Variant-Only Layout Dispatch

- [x] TRD-030 [owner=codex] [deps=TRD-020] [scope=crates/merman-render/src/lib.rs]
  Goal: Remove duplicate alias patterns from typed layout dispatch while retaining fail-fast
  compatibility validation.
  Validation:
  - `cargo nextest run -p merman-render render_model`
  - `cargo nextest run -p merman-render`
  Review: `review-workstream` for compatibility and scope.
  Evidence: `EVIDENCE_AND_GATES.md`
  Handoff: DONE. Generated parse-render dispatch remains a possible follow-on, not required for this lane.

## M3 - Closeout Or Follow-On

- [x] TRD-040 [owner=planner] [deps=TRD-030] [scope=docs/workstreams/typed-render-dispatch]
  Goal: Close this lane or split a follow-on for macro/generated parse-render dispatch.
  Validation: `verify-rust-workstream` records fresh evidence.
  Review: no blocking findings.
  Evidence: `WORKSTREAM.json`, `HANDOFF.md`, `EVIDENCE_AND_GATES.md`
  Handoff: DONE. Do not expand this lane into text measurement caching.
