# ASCII Class ER Layered Planner

Status: Active
Last updated: 2026-05-30

## Why This Lane Exists

The closed `ascii-class-er-graph-layout` lane shipped useful layered chain and star relationship
layouts for classDiagram and erDiagram. It intentionally left the layered planning logic duplicated
inside the class and ER adapters because each adapter still owned marker, cardinality, label, and
diagnostic semantics.

That duplication is now the next architecture seam to tighten. The target is a shared
terminal-layout-only planner that computes levels, adjacency legality, crossing rejection, grouping,
and box placement without learning Mermaid syntax or typed relationship semantics.

## Relevant Authority

- ADRs:
  - `docs/adr/0014-upstream-parity-policy.md`
  - `docs/adr/0065-ascii-output-boundary.md`
- Parent lane:
  - `docs/workstreams/ascii-class-er-graph-layout/`
- Runtime/docs:
  - `crates/merman-ascii/src/relation_graph.rs`
  - `crates/merman-ascii/src/class/render.rs`
  - `crates/merman-ascii/src/er/render.rs`
  - `crates/merman-ascii/tests/class_model.rs`
  - `crates/merman-ascii/tests/er_model.rs`

## Problem

Class and ER now implement the same terminal graph-planning algorithm twice:

- source/sink level assignment,
- cycle and disconnected endpoint diagnostics,
- adjacent-level validation,
- crossing detection by stable box order,
- row grouping, centering, and vertical relation gaps.

The duplicated code is still small, but the next topology slice would either copy more planner logic
or force a riskier extraction later. The right boundary is a planner over terminal boxes and generic
directed edges, while diagram-specific adapters keep drawing and semantic error wording.

## Target State

- `relation_graph` exposes an internal layered planner for `RelationGraphBox` IDs and generic
  directed edges.
- Class and ER adapters consume the same planner for level assignment, crossing validation, and box
  placement.
- Class and ER continue to own relationship semantics, marker/cardinality rendering, labels, and
  user-facing unsupported-feature text.
- Existing public ASCII/Unicode outputs remain stable.

## In Scope

- Shared internal planner types/functions in `crates/merman-ascii/src/relation_graph.rs`.
- Refactoring class and ER layered renderers to consume the shared planner.
- Focused class/ER parser-backed regression gates and package lint checks.
- Workstream evidence and handoff updates.

## Out Of Scope

- Supporting new dense, crossing, cyclic, parallel, spanning-level, or unrelated graph shapes.
- Changing public APIs or CLI behavior.
- Moving class/ER marker, cardinality, label, or diagnostic semantics into `relation_graph`.
- Flowchart layout changes.
- SVG layout reuse or pixel-to-character quantization.

## Starting Assumptions

| Assumption | Confidence | Evidence | Consequence if wrong |
| --- | --- | --- | --- |
| Class and ER layered planning are isomorphic after adapting relationships into top/bottom IDs. | High | Both adapters already have near-identical level, crossing, and placement code. | Keep only placement shared and leave level planning duplicated. |
| Existing parser-backed class/ER tests are enough to protect behavior during extraction. | High | The shipped chain/star/crossing tests exercise public `render_model` paths. | Add one focused public regression before refactoring further. |
| The shared planner should not own Mermaid terminology. | High | ADR 0065 keeps ASCII output model-driven and terminal-native. | Stop extraction if the shared type starts importing class/ER domain types. |

## Architecture Direction

Add a narrow planner to `relation_graph`:

```text
class/ER typed relationship adapter
  -> generic LayeredRelationEdge { top_id, bottom_id, kind }
  -> relation_graph layered planner over RelationGraphBox IDs
  -> class/ER relation drawing with typed markers, labels, and diagnostics
```

The planner may report structural categories such as disconnected boxes, non-adjacent levels,
cycles, and crossings. The adapters translate those categories to their existing
`AsciiError::UnsupportedFeature` text so public diagnostics remain stable.

## Closeout Condition

This lane can close when:

- both class and ER layered renderers use the shared planner,
- focused class and ER gates pass without snapshot drift,
- `cargo clippy -p merman-ascii --all-targets -- -D warnings` passes,
- docs record any residual duplication,
- and dense/crossing topology support remains split into its own lane.
