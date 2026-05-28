# Flowchart Render Config And ViewBox Extraction - Handoff

Status: Complete
Last updated: 2026-05-28

## Current State

The workstream is complete. Flowchart render configuration preparation now lives in
`flowchart/render_config.rs`, and rendered content/viewBox bounds preparation now lives in
`flowchart/viewbox.rs`.

## Current Task

- Task ID: FRCV-020/FRCV-030/FRCV-040
- Owner: codex
- Files:
  - `crates/merman-render/src/svg/parity/flowchart/render_config.rs`
  - `crates/merman-render/src/svg/parity/flowchart/viewbox.rs`
  - `crates/merman-render/src/svg/parity/flowchart/svg_emit.rs`
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

- Keep the extraction behavior-preserving: all config defaults, bbox heuristics, and DOM output
  rules moved as-is.
- Keep edge path geometry in `edge_geom.rs`; `viewbox.rs` only asks it for geometry while computing
  the final bounds and warming the render cache.
- Store the trimmed diagram title as an owned `String` in the viewBox result to avoid tying short
  mutable borrows for details/cache to later SVG emission.
