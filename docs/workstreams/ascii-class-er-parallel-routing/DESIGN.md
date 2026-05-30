# ASCII Class ER Parallel Routing

Status: Active
Last updated: 2026-05-30

## Why This Lane Exists

ClassDiagram and erDiagram ASCII output can now render layered chains, stars, adjacent-layer
crossings resolved by reordering, and isolated standalone components. The next smallest topology
gap is parallel relationships: multiple relationships between the same two endpoints still fail
even though a terminal renderer can show them as separate lanes.

## Relevant Authority

- ADRs:
  - `docs/adr/0014-upstream-parity-policy.md`
  - `docs/adr/0065-ascii-output-boundary.md`
- Parent lanes:
  - `docs/workstreams/ascii-class-er-layered-planner/`
  - `docs/workstreams/ascii-class-er-topology-routing/`
  - `docs/workstreams/ascii-class-er-component-layout/`
- Runtime/docs:
  - `crates/merman-ascii/src/relation_graph.rs`
  - `crates/merman-ascii/src/class/render.rs`
  - `crates/merman-ascii/src/er/render.rs`
  - `crates/merman-ascii/tests/class_model.rs`
  - `crates/merman-ascii/tests/er_model.rs`

## Problem

The shared layered planner rejects duplicate top/bottom edge pairs as `ParallelEdges`. That is still
the right default for unsupported dense routing, but it is too strict for the simple vertical case
where every parallel relationship shares the same two endpoints and can be rendered in adjacent
terminal lanes without dropping markers, labels, cardinality, or line style.

## Target State

- Class parser-backed tests cover multiple relationships between the same two classes.
- ER parser-backed tests cover multiple relationships between the same two entities.
- The simple same-endpoint parallel case renders every relationship with distinct terminal lanes.
- Class and ER semantics stay in their adapters; shared helpers remain terminal-layout-only.
- Mixed dense cases, cyclic layouts, spanning-level edges, and general graph bundling remain
  explicit diagnostics.

## In Scope

- Public parser-backed class and ER tests for same-endpoint parallel relationships.
- Shared terminal helper(s) for widening a vertical relation stack into multiple lanes when useful.
- Class/ER adapter changes for rendering simple same-endpoint parallel components.
- Support doc and workstream evidence updates.

## Out Of Scope

- Parallel routing between more than one endpoint pair in the same component.
- Cyclic relationship routing.
- Spanning-level routing.
- Dense label/marker collision routing.
- Flowchart routing changes.
- Public API changes.

## Closeout Condition

This lane can close when:

- class and ER same-endpoint parallel relationships render through public parser-backed tests,
- focused and full `merman-ascii` gates pass,
- support docs describe the shipped parallel subset,
- and broader dense topology work remains split or deferred.
