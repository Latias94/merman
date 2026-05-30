# ASCII Architecture Deepening — Handoff

Status: Active
Last updated: 2026-05-30

## Current State

The lane has been opened to execute five architecture deepening targets for `merman-ascii`:

- shared styled text/cell module,
- graph route planning and painting seam,
- relation graph adapter deepening,
- sequence event plan seam,
- ASCII gap registry.

AAD-010 and AAD-020 are complete. The next task is AAD-030.

## Active Task

- Task ID: AAD-030
- Owner: unassigned
- Files: `crates/merman-ascii/src/graph/routing.rs`, `crates/merman-ascii/src/graph`
- Validation: `cargo nextest run -p merman-ascii flowchart`
- Status: NEEDS_CONTEXT
- Review: Pending implementation.
- Evidence: graph routing tests and unchanged or intentionally updated flowchart snapshots

## Decisions Since Last Update

- Use one durable workstream instead of reopening closed ASCII workstreams.
- Start with the styled text/cell module because it has the broadest leverage across families.
- Keep fill/background rendering out of this lane unless it becomes a small proof of the styled cell
  substrate.
- AAD-010 passed `git diff --check -- docs/workstreams/ascii-architecture-deepening`.
- AAD-020 introduced `StyledCell` and `StyledLine` in `crates/merman-ascii/src/text.rs`.
- AAD-020 migrated `SequenceLine` and XYChart `ChartLine`/`ChartCell` to the shared styled
  substrate without changing plain output.
- Relation graph line migration is intentionally left for AAD-040, where relation graph adapters
  will be deepened.

## Blockers

- None.

## Next Recommended Action

- Commit AAD-020, then start AAD-030 by finding the smallest graph route family that can be planned
  before painting.
