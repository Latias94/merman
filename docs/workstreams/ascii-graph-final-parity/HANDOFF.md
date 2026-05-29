# ASCII Graph Final Parity - Handoff

Status: Active
Last updated: 2026-05-29

## Current State

AGF-020 is complete. Current exact graph fixture count is 60: 37 ASCII and 23 Unicode. Unicode graph
gaps are clear. Remaining work is ASCII-only multiline labels and subgraph-heavy layouts.

## Active Task

- Task ID: AGF-030
- Owner: codex
- Files: `crates/merman-ascii/src/graph`, `crates/merman-ascii/tests`
- Validation: `cargo fmt --all --check`; `cargo nextest run -p merman-ascii graph_fixture`;
  `cargo nextest run -p merman-ascii flowchart`
- Status: NEEDS_CONTEXT
- Review: Add multiline node label behavior without fixture-name special cases.
- Evidence: Pending.

## Decisions Since Last Update

- Do routing module deepening before multiline or subgraph behavior.
- Split route-cell merging into `graph/routing/cell.rs` and routed-label placement into
  `graph/routing/label.rs`; keep `routing.rs` as route orchestration.
- Keep remaining subgraph parity in this workstream but split blockers if exact parity becomes
  larger than one lane.

## Blockers

- None.

## Next Recommended Action

- Execute AGF-030 by making node label measurement and drawing line-aware, then remove
  `multiline_single_node.txt` from the ASCII graph gap allowlist if exact output matches.
