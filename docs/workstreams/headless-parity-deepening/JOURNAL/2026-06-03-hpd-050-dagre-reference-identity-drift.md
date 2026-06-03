# HPD-050 - Dagre Reference Identity Drift Detection

Date: 2026-06-03
Task: HPD-050 Architecture-first layout engine audit

## Context

The Graphlib JSON seam is now connected to `compare-dagre-layout`, but the JS/Rust reference
comparison still only compared coordinate and point data for graph entries present on both sides.
That meant a malformed JS output, a Rust graph mutation, or a future producer mismatch could report
`0.000000` max delta while silently dropping nodes or edges from the comparison.

## Outcome

- Added Rust-only and JS-only node/edge identity lists to `DagreReferenceComparison`.
- Split JS id collection from JS coordinate/point extraction so missing coordinate payloads are
  visible as infinite diagnostic deltas instead of skipped entries.
- Updated `compare-dagre-layout` output to print identity drift counts and concrete ids when drift
  exists.
- Kept the adapter State-only and did not change Dagre, Graphlib, renderer, or layout behavior.

## Verification

- `cargo fmt --check -p xtask` - passed.
- `cargo nextest run -p xtask dagre_reference` - passed, `5` tests run.
- `cargo run -p xtask -- compare-dagre-layout --diagram state --fixture basic --out-dir target\compare\dagre-layout-hpd050-reference-identity` -
  passed with max node delta `0.000000`, max edge delta `0.000000`, node identity drift
  `rust-only=0 js-only=0`, and edge identity drift `rust-only=0 js-only=0`.

## Residual Boundary

This is a reference-harness truth seam. It does not claim Architecture root-bounds closure, but it
does make future Dagre/Graphlib source audits less likely to accept a false zero-delta result.
