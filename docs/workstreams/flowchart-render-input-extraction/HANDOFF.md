# Flowchart Render Input Extraction - Handoff

Status: Complete
Last updated: 2026-05-28

## Current State

The workstream is complete. Flowchart render input preparation now lives in
`flowchart/render_input.rs`.

## Current Task

- Task ID: FRI-020/FRI-030
- Owner: codex
- Files:
  - `crates/merman-render/src/svg/parity/flowchart/render_input.rs`
  - `crates/merman-render/src/svg/parity/flowchart/svg_emit.rs`
  - `crates/merman-render/src/svg/parity/flowchart/mod.rs`
- Validation:
  - `cargo nextest run -p merman-render flowchart`
  - `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity --dom-decimals 3 --text-measurer vendored`
  - `cargo fmt -p merman-render -- --check`
  - `cargo nextest run -p merman-render`
  - `cargo clippy -p merman-render --all-targets -- -D warnings`
- Status: DONE

## Decisions

- Move cluster-edge DOM order partitioning with self-loop expansion because both shape the same
  `render_edges` vector.
- Defer viewBox bounds extraction to a later bounded lane.
