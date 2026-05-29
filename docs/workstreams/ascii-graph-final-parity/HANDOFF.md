# ASCII Graph Final Parity - Handoff

Status: Active
Last updated: 2026-05-29

## Current State

AGF-030 is complete. Current exact graph fixture count is 61: 38 ASCII and 23 Unicode. Unicode graph
gaps are clear. Remaining work is ASCII-only subgraph-heavy layouts.

## Active Task

- Task ID: AGF-040
- Owner: codex
- Files: `crates/merman-ascii/src/graph`, `crates/merman-ascii/tests`
- Validation: `cargo fmt --all --check`; `cargo nextest run -p merman-ascii graph_fixture`;
  `cargo nextest run -p merman-ascii flowchart`
- Status: NEEDS_CONTEXT
- Review: Improve subgraph-heavy fixture parity without fixture-name special cases.
- Evidence: Pending.

## Decisions Since Last Update

- Do routing module deepening before multiline or subgraph behavior.
- Split route-cell merging into `graph/routing/cell.rs` and routed-label placement into
  `graph/routing/label.rs`; keep `routing.rs` as route orchestration.
- Added `GraphLabel` for graph node labels so layout and drawing share line-aware width/height
  semantics.
- Keep remaining subgraph parity in this workstream but split blockers if exact parity becomes
  larger than one lane.

## Blockers

- None.

## Next Recommended Action

- Execute AGF-040 by comparing the remaining subgraph-heavy fixtures against `repo-ref/mermaid-ascii`
  layout behavior, then move only exact matches into the allowlist.
