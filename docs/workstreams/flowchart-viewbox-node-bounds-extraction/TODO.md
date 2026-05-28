# Flowchart ViewBox Node Bounds Extraction - TODO

Status: Complete
Last updated: 2026-05-28

## M0 - Scope Freeze

- [x] FVNB-010 [owner=codex] [deps=none] [scope=docs/workstreams/flowchart-viewbox-node-bounds-extraction]
  Goal: Freeze the slice around node/shape bounds extraction from flowchart viewBox preparation.
  Validation: workstream docs exist.
  Evidence: `DESIGN.md`.
  Handoff: DONE.

## M1 - Extract Node Bounds Module

- [x] FVNB-020 [owner=codex] [deps=FVNB-010] [scope=crates/merman-render/src/svg/parity/flowchart/viewbox.rs,crates/merman-render/src/svg/parity/flowchart/viewbox_node_bounds.rs,crates/merman-render/src/svg/parity/flowchart/mod.rs]
  Goal: Move node rendered-bounds logic and RoughJS shape bbox helpers into
  `flowchart/viewbox_node_bounds.rs`.
  Validation:
  - `cargo nextest run -p merman-render flowchart`
  Evidence: `EVIDENCE_AND_GATES.md`.
  Handoff: DONE.

## M2 - Consolidate Label Metrics

- [x] FVNB-030 [owner=codex] [deps=FVNB-020] [scope=crates/merman-render/src/svg/parity/flowchart/viewbox_node_bounds.rs]
  Goal: Replace repeated layout-node label measurement blocks with shared helper functions while
  preserving existing fallback behavior.
  Validation:
  - `cargo nextest run -p merman-render flowchart`
  Evidence: `EVIDENCE_AND_GATES.md`.
  Handoff: DONE.

## M3 - Verification And Closeout

- [x] FVNB-040 [owner=codex] [deps=FVNB-030] [scope=docs/workstreams/flowchart-viewbox-node-bounds-extraction,docs/rendering/REFACTOR_TODO.md]
  Goal: Verify package gates and close this extraction.
  Validation:
  - `cargo fmt -p merman-render -- --check`
  - `cargo nextest run -p merman-render`
  - `cargo clippy -p merman-render --all-targets -- -D warnings`
  Evidence: `EVIDENCE_AND_GATES.md`.
  Handoff: DONE.
