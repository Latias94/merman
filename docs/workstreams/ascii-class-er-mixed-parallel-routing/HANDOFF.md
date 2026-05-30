# ASCII Class ER Mixed Parallel Routing - Handoff

Status: Active
Last updated: 2026-05-30

## Current State

This lane follows the closed same-endpoint parallel lane. Class and ER can render a component where
all relationships share one endpoint pair, but a parallel pair mixed with another ordinary edge still
falls back to the layered planner's parallel diagnostic.

## Active Task

- Task ID: ACEMPR-020
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

- Only mixed-parallel class/ER relationship components are in scope.
- Keep cyclic, spanning-level, dense, and flowchart routing separate.
- Keep class/ER relationship semantics out of `relation_graph`.
- Do not silently merge, overwrite, or omit parallel relationships.
