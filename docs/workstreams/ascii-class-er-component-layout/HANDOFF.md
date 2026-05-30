# ASCII Class ER Component Layout - Handoff

Status: Active
Last updated: 2026-05-30

## Current State

This lane follows the closed `docs/workstreams/ascii-class-er-topology-routing/` lane. Class and ER
can render layered chains, stars, and adjacent-layer crossings, but unrelated classes/entities still
reject the whole diagram.

## Active Task

- Task ID: ACECL-020
- Owner: unassigned
- Files:
  - `crates/merman-ascii/tests/class_model.rs`
  - `crates/merman-ascii/tests/er_model.rs`
- Validation: `cargo nextest run -p merman-ascii class`; `cargo nextest run -p merman-ascii er`
- Status: READY
- Review: Tests must exercise public parser-backed `render_model` behavior and fail red before
  implementation.
- Evidence: `EVIDENCE_AND_GATES.md`

## Constraints

- Only disconnected component layout is in scope.
- Keep parallel, cyclic, spanning-level, and dense routing separate.
- Keep class/ER relationship semantics out of `relation_graph`.
