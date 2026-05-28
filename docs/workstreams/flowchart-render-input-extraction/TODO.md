# Flowchart Render Input Extraction - TODO

Status: Complete
Last updated: 2026-05-28

## M0 - Scope Freeze

- [x] FRI-010 [owner=codex] [deps=none] [scope=docs/workstreams/flowchart-render-input-extraction]
  Goal: Freeze render input extraction around self-loop expansion and cluster edge ordering.
  Validation: workstream docs exist.
  Evidence: `DESIGN.md`.
  Handoff: DONE.

## M1 - Extract Render Input Module

- [x] FRI-020 [owner=codex] [deps=FRI-010] [scope=crates/merman-render/src/svg/parity/flowchart/svg_emit.rs,crates/merman-render/src/svg/parity/flowchart/render_input.rs,crates/merman-render/src/svg/parity/flowchart/mod.rs]
  Goal: Move render-time edge/helper-node preparation into `flowchart/render_input.rs`.
  Validation:
  - `cargo nextest run -p merman-render flowchart`
  - `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity --dom-decimals 3 --text-measurer vendored`
  Evidence: `EVIDENCE_AND_GATES.md`.
  Handoff: DONE.

## M2 - Verification And Closeout

- [x] FRI-030 [owner=codex] [deps=FRI-020] [scope=docs/workstreams/flowchart-render-input-extraction,docs/rendering/REFACTOR_TODO.md]
  Goal: Verify package gates and close this extraction.
  Validation:
  - `cargo fmt -p merman-render -- --check`
  - `cargo nextest run -p merman-render`
  - `cargo clippy -p merman-render --all-targets -- -D warnings`
  Evidence: `EVIDENCE_AND_GATES.md`.
  Handoff: DONE.
