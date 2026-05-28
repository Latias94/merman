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
- The allowlist currently has 13 exact matches: 7 ASCII and 6 Unicode.
- The gap inventory names every remaining copied graph fixture so future routing work can move
  fixtures from gap to allowlist intentionally.
- `tests/testdata/mermaid-ascii/GRAPH_FIXTURE_GAPS.md` mirrors the readable gap list.

## Next Task

Run AGR-030:

- Replace the linear-only LR layout assumption for branch/multi-root/fan-out/fan-in fixtures.
- Start by comparing `two_layer_single_graph*.txt`, `two_root_nodes*.txt`,
  `two_single_root_nodes.txt`, `ampersand_*.txt`, `comments.txt`, and
  `preserve_order_of_definition.txt`.
- Move fixtures from `GRAPH_FIXTURE_GAPS` to `GRAPH_FIXTURE_ALLOWLIST` only after exact-output
  parity is verified.

## Constraints

- Do not add a second Mermaid parser.
- Do not use SVG layout as ASCII source of truth.
- Commit only verified bounded tasks.
- Stage only files touched for the current task.

## Target Window

Aim to stop, close, or hand off before 2026-05-29 09:00 +08:00.
