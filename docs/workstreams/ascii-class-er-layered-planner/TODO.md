# ASCII Class ER Layered Planner - TODO

Status: Active
Last updated: 2026-05-30

## M0 - Lane Opening

- [x] ACELP-010 [owner=codex] [deps=none] [scope=docs/workstreams/ascii-class-er-layered-planner]
  Goal: Open a narrow follow-on lane for extracting the class/ER layered planner.
  Validation: `git diff --check -- docs/workstreams/ascii-class-er-layered-planner`
  Review: Confirm this lane does not expand into dense/crossing topology support.
  Evidence: `DESIGN.md`, `TODO.md`, `MILESTONES.md`, `EVIDENCE_AND_GATES.md`,
  `WORKSTREAM.json`, `HANDOFF.md`
  Handoff: DONE. ACELP-020 can route class layered layout through the shared planner.

## M1 - Class Adapter Extraction

- [x] ACELP-020 [owner=codex] [deps=ACELP-010] [scope=crates/merman-ascii/src/relation_graph.rs,crates/merman-ascii/src/class/render.rs]
  Goal: Add the shared layered planner and route classDiagram layered relationship placement through
  it without changing public output.
  Validation: `cargo nextest run -p merman-ascii class`; `cargo clippy -p merman-ascii --all-targets -- -D warnings`
  Review: Shared planner must stay terminal-layout-only; class semantics and diagnostics stay in
  `class/render.rs`.
  Evidence: `relation_graph::plan_layered_relation_boxes` now owns generic level assignment,
  crossing rejection, relation gaps, and box placement; class adapter maps generic structural errors
  back to existing class diagnostics. Class parser-backed tests stayed green.
  Handoff: DONE. ACELP-030 can route ER through the same planner.

## M2 - ER Adapter Extraction

- [x] ACELP-030 [owner=codex] [deps=ACELP-020] [scope=crates/merman-ascii/src/relation_graph.rs,crates/merman-ascii/src/er/render.rs]
  Goal: Route erDiagram layered relationship placement through the same planner without changing
  cardinality, label, or line-style behavior.
  Validation: `cargo nextest run -p merman-ascii er`; `cargo nextest run -p merman-ascii class`;
  `cargo clippy -p merman-ascii --all-targets -- -D warnings`
  Review: ER cardinality direction and identifying/non-identifying line style remain typed-model
  owned.
  Evidence: ER layered rendering now consumes `relation_graph::plan_layered_relation_boxes` while
  preserving ER-owned cardinality, label, line-style, and diagnostic semantics. ER and class
  parser-backed tests stayed green.
  Handoff: DONE. ACELP-040 can run final package gates and close the lane.

## M3 - Closeout

- [ ] ACELP-040 [owner=unassigned] [deps=ACELP-030] [scope=crates/merman-ascii,docs]
  Goal: Run final ASCII package gates, update evidence, and close or split any residual planner
  work.
  Validation: `cargo nextest run -p merman-ascii`; `cargo fmt --all --check`; `git diff --check`
  Review: Use `review-workstream` and `verify-rust-workstream` before closeout.
  Evidence: `EVIDENCE_AND_GATES.md`, `HANDOFF.md`, and journal notes.
  Handoff: Lane closes or names a narrower follow-on.
