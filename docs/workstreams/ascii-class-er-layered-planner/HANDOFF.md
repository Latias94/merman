# ASCII Class ER Layered Planner - Handoff

Status: Active
Last updated: 2026-05-30

## Current State

This lane follows the closed `docs/workstreams/ascii-class-er-graph-layout/` lane. Class and ER
ASCII output already render layered chain/star relationships; this lane extracts their duplicated
layered planning into `relation_graph` without changing public behavior.

## Active Task

- Task ID: ACELP-020
- Owner: unassigned
- Files:
  - `crates/merman-ascii/src/relation_graph.rs`
  - `crates/merman-ascii/src/class/render.rs`
- Validation: `cargo nextest run -p merman-ascii class`;
  `cargo clippy -p merman-ascii --all-targets -- -D warnings`
- Status: READY
- Review: Shared planner stays terminal-layout-only; class semantics and diagnostics stay in the
  class adapter.
- Evidence: `EVIDENCE_AND_GATES.md`

## Constraints

- Do not add new graph topology support in this lane.
- Do not move class or ER relationship semantics into `relation_graph`.
- Do not change public APIs or CLI behavior.
- Leave unrelated `crates/merman-render/src/math.rs` test assertion edits untouched.
