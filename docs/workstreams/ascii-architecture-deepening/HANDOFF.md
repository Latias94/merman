# ASCII Architecture Deepening — Handoff

Status: Active
Last updated: 2026-06-26

## Current State

The lane was originally opened to execute five architecture deepening targets for `merman-ascii`:

- shared styled text/cell module,
- graph route planning and painting seam,
- relation graph adapter deepening,
- sequence event plan seam,
- ASCII gap registry.

AAD-010 through AAD-100 are complete. The lane remains active only as a registry-backed place for
future ASCII architecture slices.

## Active Task

- Task ID: none
- Owner: unassigned
- Files: choose from `crates/merman-ascii/ASCII_GAP_REGISTRY.md`
- Validation: choose the registry gate for the next slice; minimum package gate remains
  `cargo nextest run -p merman-ascii`,
  `cargo clippy -p merman-ascii --all-targets -- -D warnings`, and `git diff --check`.
- Status: NO ACTIVE TASK
- Review: Start the next slice from the registry, not from stale AAD-090 handoff text.
- Evidence: `EVIDENCE_AND_GATES.md`, `TODO.md`

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
- AAD-090 added explicit `DrawEdgeRequest` and `GridRouteOptions` request objects, preserving the
  routing seam while removing wide argument lists in graph edge drawing and grid route planning.
- TD same-rank merge edges now route through the existing LR direct route planner instead of being
  silently dropped.
- Unicode turn glyph selection now lives at the route-plan level and is shared by grid and TD bent
  routes, so a `Right -> Down` branch bend renders as `┐` instead of the disconnected `└`.
- Sequence bottom participant boxes are not part of the default Mermaid-compatible contract because
  Mermaid 11.15 defaults `sequence.mirrorActors` to `false`; they are now available through an
  explicit library option and CLI flag for terminal output.
- Namespace-qualified class relation endpoints now resolve to existing namespace member classes in
  core instead of synthesizing duplicate top-level classes.
- ER/class shared spanning relation lanes now choose a clear side around intermediate boxes instead
  of blindly offsetting to the right and potentially overwriting entity/class text.
- AAD-100 moved route canvas sizing into `RoutePlan::canvas_extent`, deleting the separate
  `graph/routing/plan/extent.rs` calculation path.
- Back-lane width contracts are now explicit minimum route-plan extents on the route families that
  need them, rather than global right padding.
- AAD-100 introduced `graph::topology::GraphGroupTopology`, shared by routing boundary context and
  graph group layout, so group membership/depth is no longer rebuilt separately in those modules.

## Blockers

- None for the shipped subset. Remaining parity gaps are tracked in
  `crates/merman-ascii/ASCII_GAP_REGISTRY.md`.
- `cargo nextest run -p merman-core` still has one unrelated snapshot failure in
  `fixtures/flowchart/stress_flowchart_edge_label_position_064.mmd` (`labelType` markdown vs
  checked-in text golden). The class namespace goldens touched by this slice were updated.

## Next Recommended Action

- Pick the next slice from `crates/merman-ascii/ASCII_GAP_REGISTRY.md`. Good candidates are broader
  flowchart route invariants for parsed edges, or a focused support-boundary slice for remaining
  uncommon graph shapes/labels. Treat complex fixtures as semantic evidence, not byte-perfect
  standards unless they come from the copied upstream `mermaid-ascii` corpus.
