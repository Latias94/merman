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

AAD-010, AAD-020, AAD-030, and AAD-040 are complete. The next task is AAD-050.

## Active Task

- Task ID: AAD-050
- Owner: unassigned
- Files: `crates/merman-ascii/src/sequence`
- Validation: `cargo nextest run -p merman-ascii sequence`
- Status: NEEDS_CONTEXT
- Review: Pending implementation.
- Evidence: sequence tests for lifecycle/control/activation behavior

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
- AAD-040 moved `RelationGraphLine` onto `StyledLine`.
- AAD-040 moved class/ER box row construction, relation line merging, and centered relation text
  writing behind `relation_graph`.
- Class and ER still own family-specific relationship semantics, marker/cardinality selection, and
  charset mapping.

## Blockers

- None.

## Next Recommended Action

- Commit AAD-040, then start AAD-050 by extracting the smallest useful sequence event-plan seam
  from lifecycle, activation, visibility, and control-frame state.
