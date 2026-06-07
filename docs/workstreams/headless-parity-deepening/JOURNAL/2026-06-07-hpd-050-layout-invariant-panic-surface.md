# HPD-050 - Layout Invariant Panic Surface

Task: HPD-050 release-boundary panic-surface hardening

## Context

The filtered production panic scan still reported internal layout invariants after the Dugong
name-allocation cleanup:

- Architecture service bounds assumed a trimmed non-empty title must always produce Cytoscape child
  label bounds and used `unreachable!()` if that helper returned `None`.
- Dugong Brandes-Koepf positioning matched fixed `"u"/"d"` and `"l"/"r"` string tuples and kept an
  unreachable fallback arm.
- Dugong ordering exposed a default `OrderNodeRange::subgraph_layer_label(...)` implementation that
  panicked for generic node labels with `min_rank` / `max_rank` but no custom override.

Both invariants are true in normal control flow today, but neither should be exposed as a library
panic surface in release-boundary hardening.

## Changes

- Architecture service bounds now conditionally applies Cytoscape child-label contribution with
  `if let Some(...)`; if the helper boundary ever drifts, the function keeps the icon/root bounds
  instead of panicking.
- Dugong BK positioning now derives the `ul` / `ur` / `dl` / `dr` alignment key from the boolean
  traversal flags directly, eliminating the impossible string-match fallback.
- `OrderNodeRange::subgraph_layer_label(...)` now defaults to `Self::default()` for label types that
  do not override the subgraph-layer projection. Dagre's native `NodeLabel` and the JSON parity test
  label keep their explicit border-left/right projection behavior.
- Added a focused layer-graph regression for a generic label type that has subgraph rank metadata
  but intentionally does not override `subgraph_layer_label(...)`.

## Verification

- `cargo +1.95 fmt -p merman-render` - passed.
- `cargo +1.95 fmt -p dugong` - passed.
- `cargo +1.95 nextest run -p dugong --test order_build_layer_graph_test --test position_bk_test` -
  passed, `56` tests run.
- `cargo +1.95 nextest run -p merman-render architecture_cytoscape architecture_svg_group_bbox_padding architecture_text_constants` -
  passed, `5` tests run.
- Filtered production scan for `panic!` / `unreachable!` / `expect(...)` / `unwrap(...)` in
  `crates/dugong/src/order/types.rs`, `crates/dugong/src/position/bk/core.rs`, and
  `crates/merman-render/src/architecture_metrics.rs` reported `COUNT 0`.
- `git diff --check` - passed.

## Boundary

This is an internal layout panic-surface cleanup only. It does not change Architecture text
metrics, Cytoscape child-label formulas, root bounds, Dugong BK alignment order, horizontal
compaction, Dagre `NodeLabel` subgraph border projection, layout geometry, Graphlib APIs, SVG
baselines, root viewport formulas, or Mermaid parity residual classification.
