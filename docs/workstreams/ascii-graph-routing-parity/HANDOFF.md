# ASCII Graph Routing Parity - Handoff

Status: Complete
Last updated: 2026-05-29

## Current State

The previous ASCII compatibility expansion is committed as
`da528d3d feat: expand ascii flowchart compatibility`.

AGR-010 is implemented and verified:

- `graph/model.rs` owns graph-only data types and test builders.
- `graph/adapter.rs` owns `FlowchartV2Model` conversion and validation.
- `graph/mod.rs` keeps rendering/layout behavior unchanged.

AGR-020 is implemented and verified:

- `tests/graph_fixture.rs` checks the copied upstream graph fixture allowlist.
- The gap inventory names every remaining copied graph fixture so future routing work can move
  fixtures from gap to allowlist intentionally.
- `tests/testdata/mermaid-ascii/GRAPH_FIXTURE_GAPS.md` mirrors the readable gap list.

AGR-030 is implemented and verified:

- LR graph layout now uses a reference-style 3x3 grid for root and child placement.
- Basic multi-root, fan-out, fan-in, same-column downward edges, and right-then-up edges are
  rendered through grid-derived node positions.
- Remaining high-value gaps after AGR-030 were junction merging (`ampersand_lhs_and_rhs.txt`,
  `backlink_from_top.txt`), self/back references, duplicate/bidirectional labels, and subgraphs.

AGR-040 is implemented and verified:

- LR self-loops now draw their loop body and expand the canvas.
- Self-loops are drawn after same-row edges so shared junctions remain visible.
- Same-row right-to-left back edges now route below the row.
- The allowlist currently has 37 exact matches: 19 ASCII and 18 Unicode.
- Remaining high-value gaps are crossing junction merging, TD back-edge labels, duplicate and
  bidirectional label separation, padding directives, and subgraph exact layout parity.

AGR-050 is implemented and verified:

- Simple subgraphs use upstream-style title rows inside the group box.
- Empty subgraphs no longer offset otherwise ordinary graph layouts.
- Seven more ASCII subgraph fixtures moved into the exact allowlist.
- The allowlist currently has 44 exact matches: 26 ASCII and 18 Unicode.
- Remaining subgraph gaps are complex nested/external-edge layouts and multi-subgraph spacing.

## Follow-Ons

This lane is closed. Split remaining work into follow-on tasks or a new workstream:

- Crossing junction merging: `ampersand_lhs_and_rhs.txt`, `backlink_from_top.txt`.
- Label separation: duplicate labels and bidirectional LR/TD labels.
- Parser/options parity for copied fixtures with `paddingX` / `paddingY` directives.
- TD back-edge labels: `back_edges_two_labels_td.txt`.
- Complex subgraphs: nested membership, external edges through group boxes, multi-subgraph spacing.

## Constraints

- Do not add a second Mermaid parser.
- Do not use SVG layout as ASCII source of truth.
- Commit only verified bounded tasks.
- Stage only files touched for the current task.

## Target Window

Aim to stop, close, or hand off before 2026-05-29 09:00 +08:00.

Closed before the target window with 44 exact graph fixture matches.
