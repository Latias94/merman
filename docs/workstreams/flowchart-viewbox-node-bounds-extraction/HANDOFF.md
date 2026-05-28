# Flowchart ViewBox Node Bounds Extraction - Handoff

Status: Complete
Last updated: 2026-05-28

## Current State

The workstream is complete. Node rendered-bounds preparation now lives in
`flowchart/viewbox_node_bounds.rs`, while `flowchart/viewbox.rs` retains cluster, edge, title, and
final viewBox orchestration.

## Current Task

- Task ID: FVNB-020/FVNB-030/FVNB-040
- Owner: codex
- Files:
  - `crates/merman-render/src/svg/parity/flowchart/viewbox_node_bounds.rs`
  - `crates/merman-render/src/svg/parity/flowchart/viewbox.rs`
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

- Keep node-bounds extraction as a sibling flowchart module instead of converting `viewbox.rs` into
  a directory module, minimizing churn around existing module paths.
- Preserve fallback behavior for partially known label metrics: some shapes use stored dimensions
  only when both width and height exist, while others intentionally use missing dimensions as zero.
