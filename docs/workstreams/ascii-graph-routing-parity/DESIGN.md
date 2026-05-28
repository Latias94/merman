# ASCII Graph Routing Parity - Design

Status: Complete
Last updated: 2026-05-29

## Problem

`merman-ascii` now renders useful flowchart subsets, but the graph renderer is still a linear
tracer-bullet layout. The reference `repo-ref/mermaid-ascii` graph algorithm contains grid
placement, path routing, self/back-edge handling, duplicate label separation, junction merging, and
subgraph bounding behavior that are not yet represented as durable Rust module boundaries.

The current `crates/merman-ascii/src/graph/mod.rs` file is large enough that adding the full routing
algorithm in-place would make the code harder to reason about and harder to verify.

## Target State

By the end of this lane, `merman-ascii` should have:

- A graph module boundary that can absorb the reference routing algorithm without a single
  ever-growing file.
- A fixture harness that can expand from selected graph fixtures toward the full copied
  `mermaid-ascii` graph corpus.
- Product-supported routing for the highest-value graph cases:
  - multi-root and branch layouts
  - fan-out / fan-in edges
  - non-adjacent edges
  - self references and back edges where feasible
  - simple nested/multiple subgraph routing where feasible
- Fresh validation evidence and autonomous commits after verified bounded tasks.

Exact byte-for-byte parity with every `mermaid-ascii` graph fixture is aspirational for follow-on
work. The product boundary remains readable deterministic terminal output driven by
`merman-core` typed models.

## Time Box

Execution target: 2026-05-29 09:00 +08:00.

The time box prioritizes durable structure and high-value routing behavior over attempting to force
all 75 graph reference fixtures green in one pass.

Closeout result: 44 copied graph fixtures are exact matches (26 ASCII, 18 Unicode). Remaining gaps
are explicit follow-ons around crossing junction merging, duplicate/bidirectional label placement,
padding directives, TD back-edge labels, and complex nested/external-edge subgraphs.

## Scope

Primary scope:

- `crates/merman-ascii/src/graph/**`
- `crates/merman-ascii/tests/flowchart_model.rs`
- `crates/merman-ascii/tests/**` fixture harnesses
- `crates/merman-ascii/FLOWCHART_SUPPORT.md`

Supporting scope:

- `crates/merman-ascii/README.md`
- `crates/merman-cli/tests/ascii_smoke.rs` only if CLI behavior changes
- `CHANGELOG.md`
- `docs/workstreams/ascii-graph-routing-parity/**`

Out of scope:

- Adding a second Mermaid parser.
- Porting the reference web UI, Go CLI, or parser modules.
- Using SVG geometry as the source of truth for ASCII layout.
- Broad sequence feature expansion.

## Boundary Plan

Refactor toward these modules:

- `adapter`: converts `FlowchartV2Model` into internal graph semantics.
- `model`: internal graph, node, edge, group, shape, and style enums.
- `layout`: node placement and graph-level layout policy.
- `routing`: edge path selection and routing metadata.
- `draw`: canvas drawing, charset, node/group/edge rendering.
- `fixtures` or integration tests: copied reference fixture allowlist and gap inventory.

`merman-core` types should stop at `adapter`; layout and drawing should work only against internal
ASCII graph types.

## Risk Plan

| Risk | Mitigation |
| --- | --- |
| Large routing changes regress current CLI output. | Keep existing behavior covered by current tests and run focused `merman-ascii` gates after each slice. |
| Fixture parity turns into a hard all-or-nothing target. | Add an allowlist harness first; expand it only as behavior ships. |
| The reference parser semantics leak into `merman-ascii`. | Keep parser concerns out; adapt from `FlowchartV2Model` only. |
| Subgraph routing consumes the whole time box. | Split nested/complex subgraphs as follow-ons if core branch/back-edge routing is not stable. |

## Exit Criteria

This lane can close when the graph module boundary is refactored, at least one routing-capability
slice beyond linear layout is implemented and verified, fixture evidence is recorded, and remaining
reference gaps are explicitly listed as follow-ons.
