# ASCII Class ER Parallel Routing - Handoff

Status: Active
Last updated: 2026-05-30

## Current State

This lane follows the closed class/ER layered planner, topology routing, and component layout lanes.
Class and ER can render chains, stars, adjacent-layer crossings, and isolated standalone components,
but same-endpoint parallel relationships still reject the whole relationship layout.

## Active Task

- Task ID: ACEPR-020
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

- Only same-endpoint parallel class/ER relationships are in scope.
- Keep cyclic, spanning-level, dense, and flowchart routing separate.
- Keep class/ER relationship semantics out of `relation_graph`.
- Do not silently merge, overwrite, or omit parallel relationships.
