# ASCII Graph Routing Parity - Handoff

Status: Active
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
- The allowlist currently has 31 exact matches: 16 ASCII and 15 Unicode.
- Remaining high-value gaps are junction merging (`ampersand_lhs_and_rhs.txt`,
  `backlink_from_top.txt`), self/back references, duplicate/bidirectional labels, and subgraphs.

## Next Task

Run AGR-040:

- Add routing support for self references, back edges, and non-adjacent edges where feasible.
- Start by comparing `self_reference*.txt`, `backlink_from_top.txt`,
  `back_reference_from_child.txt`, `preserve_order_of_definition.txt`, and label fixtures.
- Move fixtures from `GRAPH_FIXTURE_GAPS` to `GRAPH_FIXTURE_ALLOWLIST` only after exact-output
  parity is verified.

## Constraints

- Do not add a second Mermaid parser.
- Do not use SVG layout as ASCII source of truth.
- Commit only verified bounded tasks.
- Stage only files touched for the current task.

## Target Window

Aim to stop, close, or hand off before 2026-05-29 09:00 +08:00.
