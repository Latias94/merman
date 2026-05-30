# ASCII Class ER Mixed Parallel Routing

Status: Active
Last updated: 2026-05-30

## Why This Lane Exists

ClassDiagram and erDiagram ASCII output can render simple same-endpoint parallel relationships when
the whole component is that endpoint pair. The next smallest parallel gap is a mixed component: one
endpoint pair has parallel relationships, and the same relationship component also contains another
ordinary edge.

## Relevant Authority

- ADRs:
  - `docs/adr/0014-upstream-parity-policy.md`
  - `docs/adr/0065-ascii-output-boundary.md`
- Parent lanes:
  - `docs/workstreams/ascii-class-er-parallel-routing/`
  - `docs/workstreams/ascii-class-er-component-layout/`
- Runtime/docs:
  - `crates/merman-ascii/src/relation_graph.rs`
  - `crates/merman-ascii/src/class/render.rs`
  - `crates/merman-ascii/src/er/render.rs`
  - `crates/merman-ascii/tests/class_model.rs`
  - `crates/merman-ascii/tests/er_model.rs`

## Problem

The layered planner still rejects duplicate top/bottom edge pairs before it can render otherwise
simple stars or chains. Once duplicate endpoint pairs appear alongside another edge, the
same-endpoint vertical stack shortcut no longer applies.

## Target State

- Parser-backed class and ER tests cover a simple mixed-parallel component.
- The planner accepts duplicate edge pairs for level assignment.
- Layered drawing offsets duplicate endpoint-pair lanes so every relationship remains visible.
- Dense label collisions, cyclic layouts, spanning-level edges, and general graph bundling remain
  explicit follow-ons.

## In Scope

- Mixed-parallel parser-backed tests for class and ER.
- Terminal lane offsets for duplicate edge pairs in layered relationship drawing.
- Support doc and workstream evidence updates.

## Out Of Scope

- Cyclic relationship routing.
- Spanning-level routing.
- Dense label/marker collision routing.
- Flowchart routing changes.
- Public API changes.

## Closeout Condition

This lane can close when:

- class and ER mixed-parallel components render through public parser-backed tests,
- focused and full `merman-ascii` gates pass,
- support docs describe the shipped mixed-parallel subset,
- and broader dense topology work remains split or deferred.
