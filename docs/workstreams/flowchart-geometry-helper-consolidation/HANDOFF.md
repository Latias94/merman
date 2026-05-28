# Flowchart Geometry Helper Consolidation - Handoff

Status: Complete
Last updated: 2026-05-28

## Current State

The workstream is complete. `svg_emit.rs` now reuses the shared node geometry helpers for viewBox
bounds estimation.

## Current Task

- Task ID: FGH-020/FGH-030
- Owner: codex
- Files:
  - `crates/merman-render/src/svg/parity/flowchart/svg_emit.rs`
  - `crates/merman-render/src/svg/parity/flowchart/render/node.rs`
  - `crates/merman-render/src/svg/parity/flowchart/render/node/geom.rs`
- Validation:
  - `cargo nextest run -p merman-render flowchart`
  - `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity --dom-decimals 3 --text-measurer vendored`
  - `cargo fmt -p merman-render -- --check`
  - `cargo nextest run -p merman-render`
  - `cargo clippy -p merman-render --all-targets -- -D warnings`
- Status: DONE

## Decisions

- Keep edge intersection local helpers out of scope.
- Defer the large viewBox bounds loop extraction to a later bounded lane.
