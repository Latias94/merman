# ASCII Class ER Spanning Level Routing

Status: Closed
Last updated: 2026-05-30

## Why This Lane Exists

ClassDiagram and erDiagram ASCII output can render adjacent layered relationships, crossings that
are resolved by reordering, standalone components, and parallel lanes. The next narrow topology gap
is a relationship that skips over an intermediate level, such as `A -> B`, `B -> C`, and `A -> C`.

## Relevant Authority

- ADRs:
  - `docs/adr/0014-upstream-parity-policy.md`
  - `docs/adr/0065-ascii-output-boundary.md`
- Parent lanes:
  - `docs/workstreams/ascii-class-er-mixed-parallel-routing/`
  - `docs/workstreams/ascii-class-er-topology-routing/`
- Runtime/docs:
  - `crates/merman-ascii/src/relation_graph.rs`
  - `crates/merman-ascii/src/class/render.rs`
  - `crates/merman-ascii/src/er/render.rs`
  - `crates/merman-ascii/tests/class_model.rs`
  - `crates/merman-ascii/tests/er_model.rs`

## Problem

The shared layered planner currently rejects edges whose bottom level is more than one level below
the top level. For simple transitive shapes, that is too strict: the skipped edge can be routed
around the intermediate box in a side lane without silently dropping the relationship.

## Target State

- Parser-backed class and ER tests cover a simple three-node spanning-level relationship.
- The planner accepts non-cyclic spanning-level edges.
- Class and ER drawing route spanning-level edges around intermediate boxes through a side lane.
- Cyclic layouts and dense collision routing remain explicit follow-ons.

## In Scope

- Class/ER parser-backed spanning-level tests.
- Shared planner width allowance for spanning side lanes.
- Class/ER adapter drawing changes for simple spanning-level relationships.
- Support doc and workstream evidence updates.

## Out Of Scope

- Cyclic relationship routing.
- Dense label/marker collision routing.
- Flowchart routing changes.
- Public API changes.

## Closeout Condition

This lane can close when:

- class and ER spanning-level relationships render through public parser-backed tests,
- focused and full `merman-ascii` gates pass,
- support docs describe the shipped spanning-level subset,
- and remaining dense/cyclic topology work remains split or deferred.

Closeout result:

- Class and ER parser-backed tests now cover a simple three-node spanning-level relationship.
- The shared planner accepts non-cyclic spanning-level edges and reserves side-lane width.
- Class and ER drawing route simple spanning-level relationships around intermediate boxes.
- Cyclic layouts and dense label/marker collision routing remain outside this lane.
