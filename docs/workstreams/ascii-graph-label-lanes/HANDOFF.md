# ASCII Graph Label Lanes - Handoff

Status: Complete
Last updated: 2026-05-29

## Current State

The lane is complete. Grid path planning lives in
`crates/merman-ascii/src/graph/routing/path.rs`, and routed labels are collected during edge
drawing then overlaid after all route cells. Current exact graph fixture count is 53: 32 ASCII and
21 Unicode.

## Active Task

- Task ID: AGL-040
- Owner: codex
- Files: `docs/workstreams/ascii-graph-label-lanes`, `crates/merman-ascii/tests/testdata/mermaid-ascii`, final verification
- Validation: `cargo nextest run -p merman-ascii`; `cargo nextest run -p merman --features ascii`; `cargo nextest run -p merman-cli --features ascii`; `cargo clippy -p merman-ascii -p merman --features ascii --all-targets -- -D warnings`; `git diff --check`
- Status: COMPLETE
- Review: Remaining graph gaps are comments, definition-order preservation, padding, multiline
  labels, and subgraph-heavy layouts.
- Evidence: Broad verification passed.

## Decisions Since Last Update

- Do routing module deepening before changing label behavior.
- Keep path planning private to `routing` until another renderer needs it.
- Draw edge labels after all routes, matching the Go renderer's label overlay phase.
- Normalize graph fixture expected outputs to include the renderer's trailing newline when copied
  upstream fixtures omit it.

## Blockers

- None.

## Next Recommended Action

- Start a follow-on lane for parser/comment/order gaps or subgraph-heavy graph parity.
