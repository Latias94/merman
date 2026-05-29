# ASCII Graph Junction Routing - Handoff

Status: Complete
Last updated: 2026-05-29

## Current State

The lane is opened after `ascii-graph-routing-parity` closed with 44 exact graph fixture matches.
AGJ-020 split `graph/mod.rs` into private charset, layout, draw, and routing modules without
changing fixture output. AGJ-030 added Go-style LR grid path routing and moved four fixtures into
the exact allowlist. AGJ-040 verified and closed the lane.

## Active Task

- Task ID: none
- Owner: none
- Files: none
- Validation: complete
- Status: DONE
- Review: complete
- Evidence: `EVIDENCE_AND_GATES.md`

## Decisions Since Last Update

- Treat label lanes and complex subgraphs as follow-ons unless they are required to land the
  selected junction fixtures.
- `adapter.rs` remains the only graph module that depends on `FlowchartV2Model`.
- Route-cell merging is deliberately separate from the base canvas so edges can cross subgraph
  borders without turning every border crossing into a junction.

## Blockers

- None.

## Next Recommended Action

- Open a follow-on for duplicate/bidirectional edge-label lanes, TD back-edge labels, padding
  fixture directives, or complex subgraph routing.
