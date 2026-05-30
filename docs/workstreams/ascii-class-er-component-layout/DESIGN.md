# ASCII Class ER Component Layout

Status: Active
Last updated: 2026-05-30

## Why This Lane Exists

ClassDiagram and erDiagram ASCII output now share layered relationship planning and can render
adjacent-layer crossings by reordering layers. The next low-risk graph-shape limit is disconnected
components: a diagram with one relationship plus an unrelated class/entity still fails even though
each connected component can be rendered honestly on its own.

## Relevant Authority

- ADRs:
  - `docs/adr/0014-upstream-parity-policy.md`
  - `docs/adr/0065-ascii-output-boundary.md`
- Parent lanes:
  - `docs/workstreams/ascii-class-er-layered-planner/`
  - `docs/workstreams/ascii-class-er-topology-routing/`
- Runtime/docs:
  - `crates/merman-ascii/src/relation_graph.rs`
  - `crates/merman-ascii/src/class/render.rs`
  - `crates/merman-ascii/src/er/render.rs`
  - `crates/merman-ascii/tests/class_model.rs`
  - `crates/merman-ascii/tests/er_model.rs`

## Problem

The shared planner currently treats unrelated boxes as an error for relationship diagrams. That is
correct inside a single component, but too strict for a whole diagram: disconnected components can be
rendered independently and stacked with a blank separator without implying missing edges.

## Target State

- Class and ER parser-backed fixtures with a related pair plus an unrelated standalone node render
  every component.
- Shared relation-graph helpers can partition boxes and edges into deterministic connected
  components without learning class or ER semantics.
- Component renderers reuse existing single-edge, layered, and no-edge rendering paths.
- Parallel, cyclic, spanning-level, and dense routing remain explicit diagnostics inside each
  component.

## In Scope

- Public parser-backed disconnected component tests for class and ER.
- A shared component partition helper in `relation_graph`.
- Class/ER adapter refactors to render components independently and join them with a blank line.
- Support doc and workstream evidence updates.

## Out Of Scope

- Parallel relationship routing.
- Cyclic or spanning-level routing.
- Dense label/marker collision routing.
- Flowchart routing changes.
- Public API changes.

## Closeout Condition

This lane can close when:

- class and ER disconnected components render through public parser-backed tests,
- full `merman-ascii` package and lint gates pass,
- support docs describe disconnected component support,
- and remaining topology work is split or deferred.
