# ASCII Graph Label Lanes - Handoff

Status: Active
Last updated: 2026-05-29

## Current State

The lane is opened and AGL-020 is complete. Grid path planning now lives in
`crates/merman-ascii/src/graph/routing/path.rs`, while `routing.rs` keeps edge drawing behavior.
Current exact graph fixture count remains 48.

## Active Task

- Task ID: AGL-030
- Owner: codex
- Files: `crates/merman-ascii/src/graph`, `crates/merman-ascii/tests`
- Validation: `cargo fmt --all --check`; `cargo nextest run -p merman-ascii graph_fixture`; `cargo nextest run -p merman-ascii flowchart`
- Status: NEEDS_CONTEXT
- Review: Move duplicate/bidirectional label fixtures through real routing metadata, not fixture-name
  special cases.
- Evidence: Pending for label lane behavior.

## Decisions Since Last Update

- Do routing module deepening before changing label behavior.
- Keep path planning private to `routing` until another renderer needs it.

## Blockers

- None.

## Next Recommended Action

- Execute AGL-030 by inspecting current duplicate/bidirectional fixture output, then add label lane
  metadata to routed edges.
