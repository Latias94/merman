# ASCII Class ER Topology Routing - Handoff

Status: Active
Last updated: 2026-05-30

## Current State

This lane follows the closed `docs/workstreams/ascii-class-er-layered-planner/` lane. Class and ER
share layered placement, but crossing adjacent-layer relationships still return explicit
unsupported diagnostics.

## Active Task

- Task ID: ACETR-040
- Owner: unassigned
- Files:
  - `crates/merman-ascii/README.md`
  - `docs/workstreams/ascii-class-er-topology-routing/*`
- Validation: `cargo nextest run -p merman-ascii`; `cargo fmt --all --check`; `git diff --check`
- Status: READY
- Review: Confirm public support docs mention crossing adjacent-layer support and remaining dense
  topology diagnostics.
- Evidence: `EVIDENCE_AND_GATES.md`

## Constraints

- Start with crossing adjacent-layer layouts only.
- Do not add dense, parallel, cyclic, spanning-level, or unrelated topology support in the first
  implementation slice.
- Keep class/ER relationship semantics out of `relation_graph`.
- Keep flowchart routing out of scope.
