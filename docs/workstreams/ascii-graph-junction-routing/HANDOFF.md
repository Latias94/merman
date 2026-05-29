# ASCII Graph Junction Routing - Handoff

Status: Active
Last updated: 2026-05-29

## Current State

The lane is opened after `ascii-graph-routing-parity` closed with 44 exact graph fixture matches.
AGJ-020 split `graph/mod.rs` into private charset, layout, draw, and routing modules without
changing fixture output.

## Active Task

- Task ID: AGJ-030
- Owner: codex
- Files: `crates/merman-ascii/src/graph`, `crates/merman-ascii/tests`
- Validation: `cargo fmt --all --check`; `cargo nextest run -p merman-ascii graph_fixture`; `cargo nextest run -p merman-ascii flowchart`
- Status: NEEDS_CONTEXT
- Review: Route merging must be glyph/segment based, not fixture-name based.
- Evidence: Pending.

## Decisions Since Last Update

- Treat label lanes and complex subgraphs as follow-ons unless they are required to land the
  selected junction fixtures.
- `adapter.rs` remains the only graph module that depends on `FlowchartV2Model`.

## Blockers

- None.

## Next Recommended Action

- Execute AGJ-030 by adding junction-aware LR edge writes in `routing.rs`, then move exact fixtures
  from the gap list to the allowlist.
