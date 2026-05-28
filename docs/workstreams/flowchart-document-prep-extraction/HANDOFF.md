# Flowchart Document Prep Extraction - Handoff

Status: Complete
Last updated: 2026-05-28

## Current State

The workstream is complete. Flowchart root document preparation now lives in
`flowchart/document.rs`; `svg_emit.rs` passes computed bounds into the document module and then
emits the prepared root shell and accessibility metadata.

## Current Task

- Task ID: FDP-020/FDP-030
- Owner: codex
- Files:
  - `crates/merman-render/src/svg/parity/flowchart/svg_emit.rs`
  - `crates/merman-render/src/svg/parity/flowchart/document.rs`
  - `crates/merman-render/src/svg/parity/flowchart/mod.rs`
- Validation:
  - `cargo nextest run -p merman-render flowchart`
  - `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity --dom-decimals 3 --text-measurer vendored`
  - `cargo fmt -p merman-render -- --check`
  - `cargo nextest run -p merman-render`
  - `cargo clippy -p merman-render --all-targets -- -D warnings`
- Status: DONE

## Decisions

- Keep edge curve bbox collection in `svg_emit.rs`.
- Move only final document prep after content bounds are known.
- Defer marker/defs extraction to a later bounded lane.
