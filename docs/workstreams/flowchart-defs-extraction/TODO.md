# Flowchart Defs Extraction - TODO

Status: Complete
Last updated: 2026-05-28

## M0 - Scope Freeze

- [x] FDE-010 [owner=codex] [deps=none] [scope=docs/workstreams/flowchart-defs-extraction]
  Goal: Freeze the flowchart defs extraction around markers and marker color preparation.
  Validation: workstream docs exist.
  Evidence: `DESIGN.md`.
  Handoff: DONE.

## M1 - Extract Defs Module

- [x] FDE-020 [owner=codex] [deps=FDE-010] [scope=crates/merman-render/src/svg/parity/flowchart/css.rs,crates/merman-render/src/svg/parity/flowchart/defs.rs,crates/merman-render/src/svg/parity/flowchart/svg_emit.rs,crates/merman-render/src/svg/parity/flowchart/mod.rs,crates/merman-render/src/svg/parity/flowchart/render/edge_path.rs]
  Goal: Move flowchart marker/defs preparation and emission into `flowchart/defs.rs`.
  Validation:
  - `cargo nextest run -p merman-render flowchart`
  - `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity --dom-decimals 3 --text-measurer vendored`
  Review: marker ids, colored marker suffixes, and DOM order stay unchanged.
  Evidence: `EVIDENCE_AND_GATES.md`.
  Handoff: DONE.

## M2 - Verification And Closeout

- [x] FDE-030 [owner=codex] [deps=FDE-020] [scope=docs/workstreams/flowchart-defs-extraction,docs/rendering/REFACTOR_TODO.md]
  Goal: Verify package gates and close this bounded extraction.
  Validation:
  - `cargo fmt -p merman-render -- --check`
  - `cargo nextest run -p merman-render`
  - `cargo clippy -p merman-render --all-targets -- -D warnings`
  Evidence: `EVIDENCE_AND_GATES.md`.
  Handoff: DONE.
