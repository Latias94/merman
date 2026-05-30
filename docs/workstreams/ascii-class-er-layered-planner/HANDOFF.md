# ASCII Class ER Layered Planner - Handoff

Status: Closed
Last updated: 2026-05-30

## Current State

This lane followed the closed `docs/workstreams/ascii-class-er-graph-layout/` lane. Class and ER
ASCII output already render layered chain/star relationships; this lane extracts their duplicated
layered planning into `relation_graph` without changing public behavior.

The lane is closed. Both class and ER layered renderers now consume
`relation_graph::plan_layered_relation_boxes`; adapter-owned semantics and diagnostics remain in
their diagram modules.

## Final Task

- Task ID: ACELP-040
- Owner: codex
- Files:
  - `crates/merman-ascii/src/relation_graph.rs`
  - `docs/workstreams/ascii-class-er-layered-planner/*`
- Validation: `cargo nextest run -p merman-ascii`; `cargo fmt --all --check`; `git diff --check`
- Status: DONE
- Review: Confirm both class and ER consume the shared planner and no dense/crossing topology scope
  leaked into this lane.
- Evidence: `EVIDENCE_AND_GATES.md`

## Follow-Ons

- Dense/crossing class/ER topology routing remains separate and should start with public
  parser-backed tests.
- No additional layered planner extraction follow-on is needed unless new topology work reveals a
  smaller routing primitive.

## Constraints

- Do not add new graph topology support in this lane.
- Do not move class or ER relationship semantics into `relation_graph`.
- Do not change public APIs or CLI behavior.
- Leave unrelated `crates/merman-render/src/math.rs` test assertion edits untouched.
