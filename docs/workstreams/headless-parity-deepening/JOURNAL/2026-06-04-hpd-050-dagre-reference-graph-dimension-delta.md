# HPD-050 - Dagre Reference Graph-Dimension Delta

Date: 2026-06-04
Task: HPD-050 layout engine source-backed audit

## Context

The previous Dagreish graph-dimension output seam made `GraphLabel.width` and
`GraphLabel.height` available in Rust output snapshots, while the JS harness already writes the
same fields through Graphlib JSON. The remaining truth-surface gap was comparison visibility:
`compare-dagre-layout` still reported node/edge geometry and identity drift, but graph-level root
dimensions could only be inspected manually in artifacts.

## Outcome

- Added `graph_width_delta` and `graph_height_delta` to `DagreReferenceComparison`.
- Read JS reference dimensions from top-level Graphlib JSON `value.width` and `value.height`.
- Treated missing JS graph dimensions as infinite diagnostic deltas, matching the existing
  missing-coordinate/point philosophy.
- Printed graph dimension deltas from `compare-dagre-layout` before node/edge deltas.
- Added focused tests for matching graph dimensions, explicit width/height drift, and missing
  graph dimensions.
- No layout, renderer, Graphlib, SVG, fixture, or baseline behavior changed.

## Verification

- `cargo nextest run -p xtask dagre_reference` - passed, `6` tests run.
- `cargo fmt --check -p xtask` - passed.
- `cargo run -p xtask -- compare-dagre-layout --diagram state --fixture basic --out-dir target\compare\dagre-layout-hpd050-graph-dimension-delta` -
  passed with graph dimension delta `width=0.000000 height=0.000000`, max node delta
  `0.000000`, max edge delta `0.000000`, and zero node/edge identity drift.
- `git diff --check` - passed with the existing `CONTEXT.jsonl` LF-to-CRLF warning only.
- JSON parse gates passed for `CONTEXT.jsonl` (`559` records) and `WORKSTREAM.json`.

## Residual Boundary

This slice only strengthens the Dagre reference comparison surface. It does not claim a new layout
fix, default minimal `dugong::layout(...)` equivalence, or Architecture FCoSE root residual
closure. Future Dagre-backed residual audits should use the printed graph dimension deltas before
manually opening JS/Rust artifacts.
