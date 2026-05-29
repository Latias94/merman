# ASCII Graph Junction Routing

Status: Complete
Last updated: 2026-05-29

## Why This Lane Exists

`merman-ascii` now has enough `mermaid-ascii` graph parity to expose the next bottleneck: the graph
renderer still draws edges as direct canvas writes from a large `graph/mod.rs`. That makes shared
routes, line crossings, tee junctions, and later label lanes hard to evolve safely.

## Relevant Authority

- Existing docs:
  - `crates/merman-ascii/README.md`
  - `crates/merman-ascii/FLOWCHART_SUPPORT.md`
  - `crates/merman-ascii/tests/testdata/mermaid-ascii/GRAPH_FIXTURE_GAPS.md`
- Related workstreams:
  - `docs/workstreams/ascii-graph-routing-parity`
- Reference fixtures:
  - `repo-ref/mermaid-ascii`

## Problem

The current graph renderer mixes charset selection, node measurement, layout placement, group
layout, edge routing, and canvas drawing in one module. Edge drawing also overwrites existing
routes instead of merging compatible line segments into the correct ASCII or Unicode junction.

## Target State

- Graph rendering has explicit internal modules for charset, layout, drawing, and routing.
- Edge drawing has a small merge surface for shared horizontal and vertical route segments.
- At least the highest-value crossing fixtures move from named gaps to exact parity.
- Remaining gaps stay named, with label lanes and complex subgraphs deferred when they exceed this
  lane.

## In Scope

- Internal `crates/merman-ascii/src/graph` refactor.
- Junction-aware edge canvas writes for existing LR routing paths.
- Fixture allowlist updates for exact parity wins.
- Workstream evidence and handoff updates.

## Out Of Scope

- A second Mermaid parser.
- SVG layout as the source of truth for ASCII.
- Broad public API changes.
- Full complex subgraph parity.
- Full edge-label lane routing unless required by the selected junction fixtures.

## Starting Assumptions

| Assumption | Confidence | Evidence | Consequence if wrong |
| --- | --- | --- | --- |
| `ampersand_lhs_and_rhs` and `backlink_from_top` mostly need shared-route junction merging. | Medium | They are named as follow-ons after grid routing and self/back-edge work. | Scope may split after module refactor, leaving a clear handoff. |
| Splitting modules first lowers risk for routing changes. | High | `graph/mod.rs` currently owns all graph renderer responsibilities. | If the split churns too much, stop after behavior-preserving gates. |
| Existing fixture harness is the right parity gate. | High | Previous lane closed with explicit allowlist and gap inventory. | Add task-local tests only where fixtures are too coarse. |

## Architecture Direction

Keep `FlowchartV2Model` isolated in `adapter.rs` and keep graph-only types in `model.rs`. Split the
renderer into private modules:

- `charset.rs`: ASCII/Unicode glyph selection.
- `layout.rs`: node and group layout, measurement, extents.
- `draw.rs`: node/group drawing and final render orchestration.
- `routing.rs`: edge path selection and junction-aware line drawing.

The boundary should make layout a pure graph/options transformation and make routing depend on
layout output plus a small drawable edge surface.

## Closeout Condition

This lane can close when:

- the graph module split compiles and preserves current fixture parity,
- selected junction fixtures are either allowlisted or explicitly deferred with evidence,
- focused and broader Rust gates pass,
- docs reflect the shipped behavior,
- and changes are committed with only this lane's files staged.

Closeout result: complete on 2026-05-29 with 48 exact graph fixture matches.
