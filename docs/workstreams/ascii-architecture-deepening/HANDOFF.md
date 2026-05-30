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

AAD-010 through AAD-060 are complete. The next task is AAD-070.

## Active Task

- Task ID: AAD-070
- Owner: planner
- Files: `docs/workstreams/ascii-architecture-deepening`
- Validation: `cargo nextest run -p merman-ascii`; `cargo fmt --all --check`;
  `cargo clippy -p merman-ascii --all-targets -- -D warnings`; `git diff --check`
- Status: NEEDS_CONTEXT
- Review: Pending final review.
- Evidence: `EVIDENCE_AND_GATES.md`, `WORKSTREAM.json`, closeout journal note

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
- AAD-050 added `SequenceEventPlan` in `crates/merman-ascii/src/sequence/plan.rs`.
- The sequence render loop now delegates activation counts, actor visibility, lifecycle transitions,
  and control-frame ordering state to the event plan before row painting.
- AAD-060 added `crates/merman-ascii/ASCII_GAP_REGISTRY.md`.
- The ASCII README links to the registry from the current status section.

## Blockers

- None.

## Next Recommended Action

- Commit AAD-060, then run final AAD-070 gates, update closeout docs, and close this lane if gates
  remain green.
