# Flowchart Edge Geometry Compute Extraction - Handoff

Status: Complete
Last updated: 2026-05-28

## Current State

The workstream is complete. Edge path geometry computation now lives in
`flowchart/edge_geom/compute.rs`. `flowchart/mod.rs` is back to flowchart façade duties.

## Current Task

- Task ID: FEGC-020/FEGC-030
- Owner: codex
- Files:
  - `crates/merman-render/src/svg/parity/flowchart/edge_geom/compute.rs`
  - `crates/merman-render/src/svg/parity/flowchart/edge_geom.rs`
  - `crates/merman-render/src/svg/parity/flowchart/mod.rs`
  - `docs/rendering/REFACTOR_TODO.md`
- Validation:
  - `cargo nextest run -p merman-render flowchart`
  - `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity --dom-decimals 3 --text-measurer vendored`
  - `cargo fmt -p merman-render -- --check`
  - `cargo nextest run -p merman-render`
  - `cargo clippy -p merman-render --all-targets -- -D warnings`
- Status: DONE

## Decisions

- Keep `FlowchartEdgePathGeomRequest` and `flowchart_compute_edge_path_geom` visible only inside
  the flowchart module tree.
- Keep `edge_geom.rs` as the small façade so callers do not need to know about `compute.rs`.
- Do not touch the user's concurrent SVG output pipeline work.
