# Architecture Indexed FCoSE - TODO

Status: Complete
Last updated: 2026-05-28

## M0 - Scope And Evidence Freeze

- [x] AIF-010 [owner=planner] [deps=none] [scope=docs/workstreams/architecture-indexed-fcose]
  Goal: Freeze the problem, target boundary, non-goals, and validation gates for the indexed FCoSE
  lane.
  Validation: `DESIGN.md`, `MILESTONES.md`, `EVIDENCE_AND_GATES.md`, `WORKSTREAM.json`, and
  `HANDOFF.md` exist and agree.
  Evidence: `docs/workstreams/architecture-indexed-fcose/DESIGN.md`
  Handoff: Planner owns this before implementation starts.

## M1 - Indexed FCoSE API

- [x] AIF-020 [owner=codex] [deps=AIF-010] [scope=crates/manatee/src/algo/fcose,crates/manatee/src/graph]
  Goal: Add an indexed FCoSE input/output API and make the existing string-keyed API delegate
  through it without changing public compatibility behavior.
  Validation: `cargo nextest run -p manatee`
  Review: Use `review-workstream` before accepting completion.
  Evidence: `algo::fcose::tests::indexed_layout_matches_string_graph_layout_for_compound_constraints`
  Handoff: DONE. Existing Graph/FcoseOptions path delegates through indexed input and returns the
  same compatibility `LayoutResult`.

## M2 - Architecture Direct Indexed Layout

- [x] AIF-030 [owner=codex] [deps=AIF-020] [scope=crates/merman-render/src/architecture.rs,crates/manatee/src/algo/fcose]
  Goal: Build indexed FCoSE input directly from the Architecture typed model and delete the
  transient string-keyed graph construction from the Architecture layout path.
  Validation:
  - `cargo nextest run -p merman-render architecture`
  - `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3`
  Review: Use `review-workstream` for boundary and parity risk before marking complete.
  Evidence: `docs/workstreams/architecture-indexed-fcose/EVIDENCE_AND_GATES.md`
  Handoff: DONE. Architecture now builds `manatee::algo::fcose::IndexedGraph` directly and updates
  layout nodes by index.

## M3 - Performance Evidence

- [x] AIF-040 [owner=codex] [deps=AIF-030] [scope=crates/merman/benches,docs/performance,docs/workstreams/architecture-indexed-fcose]
  Goal: Record before/after evidence for Architecture layout and end-to-end performance.
  Validation:
  - `cargo bench -p merman --features render --bench architecture_layout_stress`
  - `cargo bench -p merman --features render --bench pipeline -- architecture_medium`
  Review: Verify that any performance claim has fresh command evidence.
  Evidence: `EVIDENCE_AND_GATES.md`
  Handoff: DONE. Indexed boundary reduced Architecture layout stress and pipeline timings; see
  evidence notes.

## M4 - Closeout Or Split Follow-ons

- [x] AIF-050 [owner=planner] [deps=AIF-040] [scope=docs/workstreams/architecture-indexed-fcose]
  Goal: Close this lane or split follow-on workstreams for typed dispatch and text measurement.
  Validation: `verify-rust-workstream` records fresh final gate evidence.
  Review: `review-workstream` has no blocking findings.
  Evidence: `EVIDENCE_AND_GATES.md`, `WORKSTREAM.json`, `HANDOFF.md`
  Handoff: DONE. This lane is complete; typed render dispatch and text measurement cache/context
  remain separate follow-on candidates.
