# ASCII Architecture Deepening — Handoff

Status: Active
Last updated: 2026-06-01

## Current State

The lane was originally opened to execute five architecture deepening targets for `merman-ascii`:

- shared styled text/cell module,
- graph route planning and painting seam,
- relation graph adapter deepening,
- sequence event plan seam,
- ASCII gap registry.

AAD-010 through AAD-070 remain complete.
The lane has been resumed for AAD-080, a bounded `A-GRAPH-010` follow-on.

## Active Task

- Task ID: AAD-080
- Owner: codex
- Files: `crates/merman-ascii/src/graph`, `crates/merman-ascii/tests/flowchart_model.rs`,
  `crates/merman-ascii/FLOWCHART_SUPPORT.md`, `crates/merman-ascii/ASCII_GAP_REGISTRY.md`,
  `docs/workstreams/ascii-architecture-deepening`
- Validation: `cargo nextest run -p merman-ascii flowchart subgraph`; `cargo nextest run -p merman-ascii graph_fixture`
- Status: IN PROGRESS
- Review: Verify local-direction routing is limited to fully internal subgraph edges and that
  cross-boundary edges keep the global fallback.
- Evidence: `JOURNAL/2026-06-01-aad-080.md`

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
- AAD-070 final gates passed and the lane was closed.
- AAD-080 resumes the lane to ship a narrow local-direction subset for `FlowSubgraph.dir`.
- `AsciiGraphGroup` now stores optional local direction metadata.
- Graph placement can re-place eligible subgraph members in local canonical direction when the
  subgraph direction differs from the root direction and there are no cross-boundary edges.
- Edge route selection and extent calculation now derive direction from local group membership for
  edges whose endpoints both live inside the same direction-bearing subgraph.
- Cross-boundary mixed-direction subgraph cases still fall back to the global root layout and are
  documented as remaining work rather than hidden partial parity.

## Blockers

- None for the shipped subset. Remaining mixed-direction parity gaps are tracked in
  `crates/merman-ascii/ASCII_GAP_REGISTRY.md`.

## Next Recommended Action

- Finish AAD-080 closeout by updating `WORKSTREAM.json` and evidence once this slice is committed.
- After that, choose between extending `A-GRAPH-010` into cross-boundary mixed-direction routing or
  moving to another high-yield gap from `crates/merman-ascii/ASCII_GAP_REGISTRY.md`.
