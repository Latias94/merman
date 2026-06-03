# HPD-050 - Dagreish Bounding-Box Source Coverage

Date: 2026-06-04
Task: HPD-050 layout engine source-backed audit

## Context

The previous Dagreish slice implemented upstream `translateGraph(...)` graph-dimension writeback
and confirmed JS/Rust reference output dimensions match on State samples. The next adjacent
source-backed gap in `repo-ref/dagre/test/layout-test.js` was the bounding-box assertion group,
which verifies that final node and edge-label coordinates are translated into the graph bounds
across all rank directions.

This is a better HPD-050 target than JS-only Graphlib ergonomics because it exercises the same
consumer path that final layout/root-bounds adapters depend on: coordinate-system undo followed by
`translateGraph(...)`.

## Source Finding

Pinned Dagre checks two bounding-box surfaces for every `rankdir`:

- a single node should land at half its own width/height after layout;
- an edge label with `labelpos = l` and `labeloffset = 0` should be translated so its left/top
  coordinate is inside the graph bounding box.

For `TB` / `BT`, the edge-label assertion is on `x`; for `LR` / `RL`, the assertion is on `y`.

## Outcome

- Added `layout_dagreish_keeps_node_coordinates_in_graph_bounding_box_for_rankdirs`.
- Added `layout_dagreish_keeps_left_edge_label_coordinates_in_graph_bounding_box_for_rankdirs`.
- Updated `docs/dugong/DAGRE_UPSTREAM_TEST_COVERAGE.md` to map the two upstream bounding-box cases
  to the new Rust regressions.
- No production Dugong, Graphlib, renderer, SVG, or reference adapter behavior changed.

## Verification

- `cargo nextest run -p dugong --test layout_test` - passed, `19` tests run.
- `cargo nextest run -p dugong` - passed, `275` tests run.
- `cargo fmt --check -p dugong` - passed.
- `git diff --check` - passed with the existing `CONTEXT.jsonl` LF-to-CRLF warning only.
- JSON parse gates passed for `CONTEXT.jsonl` (`555` records) and `WORKSTREAM.json`.

## Residual Boundary

This slice proves the existing full Dagreish consumer path already satisfies these two upstream
bounding-box cases. It does not claim default minimal `dugong::layout(...)` equivalence, JS
object-key case-insensitivity, or Architecture FCoSE/Cytoscape root residual closure.
