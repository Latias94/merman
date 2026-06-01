# ASCII Architecture Deepening

Status: Active
Last updated: 2026-06-02

## Why This Lane Exists

`merman-ascii` has moved past a prototype renderer. Flowchart, sequence, class, ER, and XYChart
all have shipped subsets, copied fixture gaps are largely closed, and color roles now exist across
the supported families. The next risk is not basic coverage; it is keeping the renderer deep enough
that future parity work does not spread the same text, color, routing, and event-state semantics
across many shallow modules.

This lane deepens the ASCII renderer around five durable concepts:

- styled terminal text/cells,
- graph route planning and painting,
- relation graph adapters for class and ER,
- sequence event planning,
- and an ASCII gap registry.

## Relevant Authority

- ADRs:
  - `docs/adr/0014-upstream-parity-policy.md`
  - `docs/adr/0065-ascii-output-boundary.md`
  - `docs/adr/0067-ascii-color-role-api.md`
- Existing docs:
  - `crates/merman-ascii/README.md`
  - `crates/merman-ascii/FLOWCHART_SUPPORT.md`
  - `crates/merman-ascii/SEQUENCE_SUPPORT.md`
  - `crates/merman-ascii/GRAPH_FIXTURE_GAPS.md`
  - `crates/merman-ascii/SEQUENCE_FIXTURE_GAPS.md`
- Related workstreams:
  - `docs/workstreams/ascii-color-role-api/`
  - `docs/workstreams/ascii-graph-final-parity/`
  - `docs/workstreams/ascii-reference-implementation-expansion/`
  - `docs/workstreams/ascii-sequence-control-blocks/`
  - `docs/workstreams/ascii-sequence-renderer-modularization/`

## Problem

The renderer now has several repeated internal implementations:

- sequence, relation graph, and XYChart all maintain their own role-aware line buffers;
- graph routing mixes route selection, label placement, canvas mutation, and edge style application;
- class and ER rendering share relation graph concepts but still duplicate placement and painting
  details;
- the sequence renderer is modularized, but the main render loop still owns event-state planning and
  row emission together;
- remaining parity gaps are scattered across support docs and closed workstream handoffs.

These are shallow module symptoms. The interfaces are nearly as large as the implementations, so
new behavior tends to require edits in several families instead of one local implementation.

## Target State

When this lane closes:

- shared styled text/cell behavior is represented by one internal module and reused by shipped
  families where it reduces duplication;
- graph route planning has a testable planning seam before canvas painting;
- class and ER relation rendering depend on a deeper relation graph adapter surface instead of
  duplicating relation layout and drawing details;
- sequence rendering has a separable event-plan seam for lifecycle, activation, visibility, and
  control-frame state before row painting;
- ASCII follow-ons are tracked in one gap registry with ownership, dependencies, and gates.

## In Scope

- Internal refactors inside `crates/merman-ascii`.
- Focused regression tests for each new seam.
- Documentation updates for support matrices and workstream evidence.
- Deleting redundant internal code after adoption.
- Maintaining plain-output snapshot stability unless a task explicitly records an intentional
  output correction.

## Out Of Scope

- Public API churn outside behavior already accepted by ADR 0067.
- FFI, bindings, or CLI packaging work.
- Full Mermaid SVG visual parity.
- Implementing every deferred Mermaid feature while doing this refactor.
- Background/fill rendering unless it naturally becomes a small proof for the styled cell module;
  otherwise it should remain a follow-on feature lane.

## Starting Assumptions

| Assumption | Confidence | Evidence | Consequence if wrong |
| --- | --- | --- | --- |
| The shipped ASCII family subset should remain model-driven and parser-free. | High | ADR 0065 and existing renderer entry points. | This lane would need an ADR before changing renderer ownership. |
| Color roles are the accepted extension point for styled terminal output. | High | ADR 0067 and `ascii-color-role-api` closeout. | Styled cell work would need to be narrowed to plain text only. |
| Flowchart route planning is the highest-risk graph refactor after color mapping. | Medium | `graph/routing.rs` combines routing, painting, labels, and style deltas. | Start with a smaller label/style seam before route planning. |
| Relation graph is worth deepening rather than deleting. | High | Class and ER both depend on relation layout concepts. | If callers diverge further, split adapters by family instead. |
| Sequence event planning can be extracted without changing public sequence output. | Medium | The renderer already has event/control/layout modules. | Keep the seam smaller and only isolate lifecycle/control state first. |

## Architecture Direction

The lane follows the existing ASCII output boundary: typed models enter family renderers, renderers
produce deterministic terminal text, and unsupported features remain explicit diagnostics.

The first deepening target is shared styled text/cell behavior because it has the broadest leverage:
it is already duplicated across sequence, relation graph, XYChart, and graph labels, and it is also
the natural locality for future CJK/emoji placement and background/fill semantics.

Graph, relation graph, and sequence work should proceed as vertical slices. Each slice must introduce
one seam, migrate one meaningful caller, and prove that the interface is smaller than the
implementation it hides. Do not add pass-through modules solely for testability.

## Closeout Condition

This lane can close when:

- all five target concepts have landed or a load-bearing deferral is documented;
- each landed seam has focused tests and package-level `merman-ascii` verification;
- support docs and the ASCII gap registry reflect the shipped state;
- redundant implementations are removed where the new module supersedes them;
- final gates are recorded in `EVIDENCE_AND_GATES.md`.

## Reopened Follow-On

This lane was closed on 2026-05-30 after the original five deepening targets landed, then resumed
for follow-on ASCII graph architecture work that still fits this lane's purpose: making flowchart
direction behavior deeper, more local, and less dependent on global-layout accidents.

The first reopened slice, AAD-080, shipped a bounded `FlowSubgraph.dir` subset:

- canonical `LR` subgraphs inside canonical `TD` roots;
- local direction applies only to edges whose endpoints both stay inside the same subgraph;
- support docs and the ASCII gap registry now document this subset honestly.

The next follow-on is not another local heuristic in `layout.rs`. It is a routing-seam expansion:
cross-boundary mixed-direction flowchart routing for edges that enter or leave a direction-bearing
subgraph.

## Cross-Boundary Direction Design

### Problem

The current graph stack has enough information to place nodes with a local subgraph direction and to
route purely internal edges with that local direction. It does not yet have a first-class model for
an edge whose path crosses the boundary between:

- a root graph using one canonical direction, and
- a subgraph using another canonical direction.

Without that model, one of two bad outcomes happens:

- the renderer falls back to global root behavior and loses the local direction semantics; or
- layout-layer heuristics try to simulate routing intent by moving unrelated nodes.

Both outcomes are architecture debt. Mixed-direction boundary routing belongs in the route-planning
seam, not as accidental placement side effects.

### Desired Properties

The follow-on implementation should satisfy these constraints:

- local subgraph direction remains authoritative for the internal segment of a cross-boundary edge;
- the root direction remains authoritative for the external segment outside the subgraph;
- the boundary transition is explicit, testable, and deterministic;
- label placement stays attached to a specific planned segment instead of depending on whatever
  segment happens to be longest after painting;
- route planning remains separable from canvas mutation.

### Proposed Seam

Extend graph routing around three explicit concepts:

1. `EdgeBoundaryContext`
   Describes whether an edge is:
   - fully internal to a direction-bearing subgraph,
   - fully external to such a subgraph,
   - entering a direction-bearing subgraph,
   - leaving a direction-bearing subgraph,
   - or crossing between two different direction-bearing subgraphs.

2. `BoundaryAnchor`
   Represents the planned entry/exit side of a subgraph box for a mixed-direction edge. This is a
   routing concern, not a node-placement concern.

3. `SegmentedRoutePlan`
   Allows a route to be composed from internal, boundary, and external segments before painting.
   The existing `RoutePlan` can remain the painted-cell output, but planning should be able to
   assemble it from named segments first.

### Implementation Order

The next implementation slice should stay narrow:

1. classify boundary context in `graph::routing::plan::select`;
2. introduce one planned boundary-anchor strategy for the shipped subset:
   root `TD` + local `LR` subgraph;
3. support entering and leaving one direction-bearing subgraph;
4. keep subgraph-to-subgraph mixed-direction routing as a documented follow-on unless it naturally
   falls out of the same seam.

### Why This Beats More Layout Heuristics

`beautiful-mermaid` solves this class of problem by explicitly separating internal edges,
cross-hierarchy edges, and boundary ports. `merman-ascii` does not need to copy ELK hierarchy
handling, but it does need the same architectural idea: cross-boundary direction is an edge-routing
problem. Treating it as node-placement special casing makes behavior fragile and harder to extend to
nested subgraphs or label lanes later.
