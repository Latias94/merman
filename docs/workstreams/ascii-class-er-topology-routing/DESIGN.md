# ASCII Class ER Topology Routing

Status: Active
Last updated: 2026-05-30

## Why This Lane Exists

ClassDiagram and erDiagram ASCII output now render layered chains and stars through a shared
terminal-layout-only planner. The remaining graph-shape limitations are deliberate diagnostics:
crossing, dense, cyclic, parallel, spanning-level, and unrelated topologies.

This lane broadens that boundary one topology at a time. The first useful slice is crossing
relationships between adjacent layers, because the existing draw paths already have junction
characters and parser-backed diagnostics proving the current unsupported behavior.

## Relevant Authority

- ADRs:
  - `docs/adr/0014-upstream-parity-policy.md`
  - `docs/adr/0065-ascii-output-boundary.md`
- Parent lanes:
  - `docs/workstreams/ascii-class-er-graph-layout/`
  - `docs/workstreams/ascii-class-er-layered-planner/`
- Runtime/docs:
  - `crates/merman-ascii/src/relation_graph.rs`
  - `crates/merman-ascii/src/class/render.rs`
  - `crates/merman-ascii/src/er/render.rs`
  - `crates/merman-ascii/tests/class_model.rs`
  - `crates/merman-ascii/tests/er_model.rs`

## Problem

The current planner rejects crossing adjacent-layer relationships even when the terminal renderer can
show every edge with a visible junction. That keeps output honest, but it leaves common two-by-two
relationship shapes unsupported.

Dense topology support is riskier: labels can collide, markers can overwrite each other, and
parallel/cyclic/spanning-level layouts need separate route semantics. This lane should avoid broad
graph-engine scope and add only topologies with public tests and readable output.

## Target State

- Parser-backed class and ER crossing fixtures render every relationship without silent omission.
- The shared planner keeps structural validation but exposes crossing-adjacent-level relationships
  when the adapter can draw them honestly.
- Class and ER adapters still own relationship semantics, labels, markers, cardinalities, and
  diagnostic wording.
- Dense, parallel, cyclic, spanning-level, and unrelated graph shapes remain explicit diagnostics
  unless this lane splits a narrower follow-on.

## In Scope

- Public parser-backed tests for crossing class and ER relationship layouts.
- Shared planner changes needed to allow crossing adjacent-layer edges.
- Adapter draw safeguards for visible junctions and non-overwritten markers/cardinality labels.
- README/support-doc updates if the shipped subset changes.

## Out Of Scope

- Full graph-theory layout parity.
- Parallel relationship routing.
- Cyclic or spanning-level relationship routing.
- Unrelated component placement.
- Moving class/ER semantics into `relation_graph`.
- Flowchart routing changes.

## Starting Assumptions

| Assumption | Confidence | Evidence | Consequence if wrong |
| --- | --- | --- | --- |
| Crossing adjacent-layer edges can be rendered readably with current terminal junction characters. | Medium | Class/ER draw paths already merge relation-line crossings into junctions. | Keep crossing unsupported and split a route-lane design task. |
| Class and ER can share the structural planner change. | High | Both now consume `plan_layered_relation_boxes`. | Keep planner permissive and handle family-specific rendering limits in adapters. |
| Dense/parallel/cyclic routing should not ride along with crossing support. | High | Labels and markers need separate route semantics. | Split additional topologies into follow-ons. |

## Architecture Direction

Keep the route boundary layered:

```text
RelationGraphBox + LayeredRelationEdge
  -> shared planner validates supported structural categories
  -> class/ER adapters draw typed edges and labels
  -> public parser-backed snapshots prove every edge is visible
```

The planner should not know about class markers or ER cardinality. If crossing support needs
diagram-specific constraints, adapters should opt in with metadata rather than leaking Mermaid
concepts into `relation_graph`.

## Closeout Condition

This lane can close when:

- at least one class and one ER crossing fixture render through public parser-backed tests,
- full `merman-ascii` package and lint gates pass,
- docs describe the new crossing subset and remaining diagnostics,
- and any denser topology work is split or explicitly deferred.
