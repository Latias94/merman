# ASCII Class ER Topology Routing - Handoff

Status: Closed
Last updated: 2026-05-30

## Current State

This lane followed the closed `docs/workstreams/ascii-class-er-layered-planner/` lane. Class and ER
share layered placement, and adjacent-layer crossing relationships now render when they can be
resolved by layer reordering.

## Final Task

- Task ID: ACETR-040
- Owner: codex
- Files:
  - `README.md`
  - `crates/merman-cli/README.md`
  - `crates/merman-ascii/README.md`
  - `docs/workstreams/ascii-class-er-topology-routing/*`
- Validation: `cargo nextest run -p merman-ascii`; `cargo fmt --all --check`; `git diff --check`
- Status: DONE
- Review: Confirm public support docs mention crossing adjacent-layer support and remaining dense
  topology diagnostics.
- Evidence: `EVIDENCE_AND_GATES.md`

## Follow-Ons

- Dense class/ER topology routing with label and marker collision tests.
- Parallel relationship routing, if terminal output can distinguish each edge.
- Cyclic, spanning-level, and unrelated-component layout support as separate slices.

## Constraints

- Keep remaining dense, parallel, cyclic, spanning-level, and unrelated topology support in separate
  lanes.
- Keep class/ER relationship semantics out of `relation_graph`.
- Keep flowchart routing out of scope.
