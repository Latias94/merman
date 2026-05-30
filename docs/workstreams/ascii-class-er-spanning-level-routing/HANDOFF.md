# ASCII Class ER Spanning Level Routing - Handoff

Status: Active
Last updated: 2026-05-30

## Current State

This lane follows the closed mixed-parallel lane. Class and ER can render useful layered,
crossing-by-reorder, component, and parallel layouts, but a non-cyclic edge that skips an
intermediate level still returns a spanning-level diagnostic.

## Active Task

- Task ID: ACESLR-020
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

- Only non-cyclic spanning-level class/ER relationships are in scope.
- Keep dense, cyclic, and flowchart routing separate.
- Keep class/ER relationship semantics out of `relation_graph`.
- Do not route a spanning edge through an intermediate box.
