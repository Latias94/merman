# Flowchart Defs Extraction - Handoff

Status: Complete
Last updated: 2026-05-28

## Current State

The workstream is complete. Flowchart marker/defs preparation and emission now lives in
`flowchart/defs.rs`; `css.rs` owns CSS generation and edge class attributes only.

## Current Task

- Task ID: FDE-020/FDE-030
- Owner: codex
- Files:
  - `crates/merman-render/src/svg/parity/flowchart/css.rs`
  - `crates/merman-render/src/svg/parity/flowchart/defs.rs`
  - `crates/merman-render/src/svg/parity/flowchart/svg_emit.rs`
  - `crates/merman-render/src/svg/parity/flowchart/mod.rs`
  - `crates/merman-render/src/svg/parity/flowchart/render/edge_path.rs`
- Validation:
  - `cargo nextest run -p merman-render flowchart`
  - `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity --dom-decimals 3 --text-measurer vendored`
  - `cargo fmt -p merman-render -- --check`
  - `cargo nextest run -p merman-render`
  - `cargo clippy -p merman-render --all-targets -- -D warnings`
- Status: DONE

## Decisions

- Keep base markers and extra colored marker emission as separate methods to preserve DOM order.
- Keep edge class attribute emission out of scope.
- Defer broader flowchart node/edge renderer splits to later bounded lanes.
