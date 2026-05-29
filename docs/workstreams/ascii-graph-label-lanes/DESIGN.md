# ASCII Graph Label Lanes

Status: Active
Last updated: 2026-05-29

## Why This Lane Exists

The graph renderer now has Go-style LR grid path routing, but `routing.rs` has grown into a large
module that mixes path planning, line drawing, junction merging, and label placement. The remaining
high-value graph fixture gaps are mostly label-lane cases where multiple or reverse edges need
separate label corridors instead of overwriting the same straight edge.

## Relevant Authority

- `repo-ref/mermaid-ascii/cmd/mapping_edge.go`
- `repo-ref/mermaid-ascii/cmd/arrow.go`
- `repo-ref/mermaid-ascii/cmd/draw.go`
- `crates/merman-ascii/tests/testdata/mermaid-ascii/GRAPH_FIXTURE_GAPS.md`
- `docs/workstreams/ascii-graph-junction-routing`

## Problem

Duplicate and bidirectional labeled edges are still named gaps. The current renderer writes labels
near the direct from/to span, so labels collide when edges share endpoints or route in opposite
directions.

## Target State

- Routing internals are split so path planning, route drawing, and label placement can evolve
  independently.
- Duplicate LR edge labels and LR bidirectional labels use separate lanes.
- TD bidirectional/back-edge labels are either landed or explicitly split with evidence.
- Exact fixture parity increases without fixture-name special casing.

## In Scope

- `crates/merman-ascii/src/graph` routing refactor.
- Label-line selection inspired by `mermaid-ascii`'s `determineLabelLine`.
- Fixture allowlist/gap updates for exact wins.
- Workstream evidence, support docs, and changelog updates.

## Out Of Scope

- Full complex subgraph routing.
- Mermaid parser changes.
- Fixture-level `paddingX` / `paddingY` directive parsing unless required for a label fixture.
- Multiline labels.

## Starting Assumptions

| Assumption | Confidence | Evidence | Consequence if wrong |
| --- | --- | --- | --- |
| LR duplicate/bidirectional labels can be handled with existing grid paths plus parallel ports. | Medium | Go reference uses duplicate edge counts and label-line selection. | Split TD labels or broader lane routing into follow-on. |
| Splitting `routing.rs` first reduces risk. | High | `routing.rs` is over 1,100 lines after the path-routing port. | If behavior drifts, stop after the refactor and keep label work pending. |
| The fixture harness is the correct gate. | High | Previous lanes used allowlist/gap inventory as executable source of truth. | Add focused tests only if fixture output is too coarse. |

## Architecture Direction

Keep `routing.rs` as orchestration and split implementation behind private submodules:

- `routing/path.rs`: ports, relative directions, A* path planning, path compaction.
- `routing/draw.rs`: path line/corner/arrow/connector drawing and route-cell merge rules.
- `routing/labels.rs`: label line selection and label drawing.

## Closeout Condition

This lane can close when the module split is verified, at least one label-lane fixture moves from
gap to allowlist or is split with evidence, broad gates pass, and docs name the remaining work.
