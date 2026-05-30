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

AAD-010, AAD-020, and AAD-030 are complete. The next task is AAD-040.

## Active Task

- Task ID: AAD-040
- Owner: unassigned
- Files: `crates/merman-ascii/src/relation_graph.rs`, `crates/merman-ascii/src/class`, `crates/merman-ascii/src/er`
- Validation: `cargo nextest run -p merman-ascii class er`
- Status: NEEDS_CONTEXT
- Review: Pending implementation.
- Evidence: class and ER regression tests covering relation routing and color roles

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
- AAD-030 added `crates/merman-ascii/src/graph/routing/plan.rs`.
- AAD-030 moved top-down direct route connector, route cells, arrow endpoint, and label anchors into
  a route plan before painting.
- Other graph route families intentionally remain on the old drawing path until a future graph
  routing expansion.

## Blockers

- None.

## Next Recommended Action

- Commit AAD-030, then start AAD-040 by moving relation graph line behavior onto the shared styled
  substrate and reducing class/ER relation-rendering duplication where the seam has enough depth.
