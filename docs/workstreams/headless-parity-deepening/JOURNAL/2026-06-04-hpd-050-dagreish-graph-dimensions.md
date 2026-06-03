# HPD-050 - Dagreish Graph Dimension Output Seam

Date: 2026-06-04
Task: HPD-050 layout engine source-backed audit

## Context

The previous Dagreish layout coverage slice mapped four upstream `layout-test.js` cases to direct
Rust coverage on the full `layout_dagreish(...)` consumer path. The next higher-value layout-output
gap was upstream Dagre's graph-dimension writeback: `layout(...)` mutates `graph().width` and
`graph().height` during `translateGraph(...)`, but local `GraphLabel` did not expose those output
fields.

This is closer to HPD root-bounds work than ordinary Graphlib API coverage because graph
dimensions are final layout output and can be used by debug/reference adapters to detect root-size
drift.

## Source Finding

Pinned Dagre `repo-ref/dagre/lib/layout.js::translateGraph(...)` computes graph dimensions from:

- positioned node boxes,
- edge-label boxes only when the edge has explicit `x/y`,
- `marginx` / `marginy`,
- not intermediate edge route points.

The source formula writes:

- `graphLabel.width = maxX - minX + marginX`
- `graphLabel.height = maxY - minY + marginY`

after subtracting margins from the minimum x/y translation origin. Therefore a single
`100x50` node with `marginx=8` and `marginy=10` should report `116x70`.

## Outcome

- Added `width` and `height` output fields to `dugong::GraphLabel`.
- Updated `layout_dagreish(...)` to compute and write graph dimensions in the same phase as
  upstream `translateGraph(...)`.
- Added direct coverage for upstream `repo-ref/dagre/test/layout-test.js` case
  `adds dimensions to the graph`.
- Added a focused margin regression for the source formula.
- Updated the Dagre reference adapter so output snapshots include graph dimensions while input
  snapshots continue omitting them.
- No renderer or SVG output behavior changed in this slice.

## Verification

- `cargo nextest run -p dugong --test layout_test` - passed, `17` tests run.
- `cargo nextest run -p dugong` - passed, `273` tests run.
- `cargo nextest run -p dugong-graphlib` - passed, `96` tests run.
- `cargo nextest run -p xtask dagre_reference` - passed, `5` tests run.
- `cargo run -p xtask -- compare-dagre-layout --diagram state --fixture basic --out-dir target\compare\dagre-layout-hpd050-graph-dimensions` -
  passed with max node/edge delta `0.000000` and zero node/edge identity drift. The generated
  input artifact omitted graph dimensions; JS and Rust output artifacts both reported graph
  dimensions `100.109375 x 298`.
- `cargo run -p xtask -- compare-dagre-layout --diagram state --fixture stress_state_composite_with_external_edges_028 --out-dir target\compare\dagre-layout-hpd050-graph-dimensions-composite` -
  passed with max node/edge delta `0.000000` and zero node/edge identity drift.
- `cargo run -p xtask -- compare-dagre-layout --diagram state --fixture stress_state_composite_with_external_edges_028 --cluster state-Big-7 --out-dir target\compare\dagre-layout-hpd050-graph-dimensions-cluster` -
  passed with max node/edge delta `0.000000` and zero node/edge identity drift.
- `cargo fmt --check -p dugong -p dugong-graphlib -p xtask` - passed.
- `git diff --check` - passed with the existing `CONTEXT.jsonl` LF-to-CRLF warning only.
- JSON parse gates passed for `CONTEXT.jsonl` (`555` records), `WORKSTREAM.json`, `TASKS.jsonl`
  (`8` records), and `CAMPAIGNS.jsonl` (`4` records).

## Residual Boundary

This slice closes the full Dagreish consumer-path graph-dimension output seam. It does not claim
default minimal `dugong::layout(...)` graph-dimension parity, nor does it close Architecture
FCoSE/Cytoscape root residuals. The new fields should make future Dagre reference artifacts more
diagnostic when layout graph dimensions drift.
