# ASCII Class ER Topology Routing - Handoff

Status: Active
Last updated: 2026-05-30

## Current State

This lane follows the closed `docs/workstreams/ascii-class-er-layered-planner/` lane. Class and ER
share layered placement, but crossing adjacent-layer relationships still return explicit
unsupported diagnostics.

## Active Task

- Task ID: ACETR-020
- Owner: unassigned
- Files:
  - `crates/merman-ascii/tests/class_model.rs`
  - `crates/merman-ascii/tests/er_model.rs`
- Validation: `cargo nextest run -p merman-ascii class`; `cargo nextest run -p merman-ascii er`
- Status: READY
- Review: Tests must exercise public parser-backed `render_model` behavior and should fail red
  before implementation.
- Evidence: `EVIDENCE_AND_GATES.md`

## Constraints

- Start with crossing adjacent-layer layouts only.
- Do not add dense, parallel, cyclic, spanning-level, or unrelated topology support in the first
  implementation slice.
- Keep class/ER relationship semantics out of `relation_graph`.
- Keep flowchart routing out of scope.
