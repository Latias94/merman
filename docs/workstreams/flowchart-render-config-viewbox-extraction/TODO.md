# Flowchart Render Config And ViewBox Extraction - TODO

Status: Complete
Last updated: 2026-05-28

## M0 - Scope Freeze

- [x] FRCV-010 [owner=codex] [deps=none] [scope=docs/workstreams/flowchart-render-config-viewbox-extraction]
  Goal: Freeze the dual extraction around render configuration and viewBox/content-bounds
  preparation.
  Validation: workstream docs exist.
  Evidence: `DESIGN.md`.
  Handoff: DONE.

## M1 - Extract Render Configuration

- [x] FRCV-020 [owner=codex] [deps=FRCV-010] [scope=crates/merman-render/src/svg/parity/flowchart/render_config.rs,crates/merman-render/src/svg/parity/flowchart/svg_emit.rs,crates/merman-render/src/svg/parity/flowchart/mod.rs]
  Goal: Move font, htmlLabels, wrap mode, theme color, spacing, and edge-default preparation into
  `flowchart/render_config.rs`.
  Validation:
  - `cargo nextest run -p merman-render flowchart`
  Evidence: `EVIDENCE_AND_GATES.md`.
  Handoff: DONE.

## M2 - Extract ViewBox And Content Bounds

- [x] FRCV-030 [owner=codex] [deps=FRCV-020] [scope=crates/merman-render/src/svg/parity/flowchart/viewbox.rs,crates/merman-render/src/svg/parity/flowchart/svg_emit.rs,crates/merman-render/src/svg/parity/flowchart/mod.rs]
  Goal: Move rendered content bounds, edge curve bbox union, recursive root expansion, and title bbox
  merging into `flowchart/viewbox.rs`.
  Validation:
  - `cargo nextest run -p merman-render flowchart`
  - `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity --dom-decimals 3 --text-measurer vendored`
  Evidence: `EVIDENCE_AND_GATES.md`.
  Handoff: DONE.

## M3 - Verification And Closeout

- [x] FRCV-040 [owner=codex] [deps=FRCV-030] [scope=docs/workstreams/flowchart-render-config-viewbox-extraction,docs/rendering/REFACTOR_TODO.md]
  Goal: Verify package gates and close this extraction.
  Validation:
  - `cargo fmt -p merman-render -- --check`
  - `cargo nextest run -p merman-render`
  - `cargo clippy -p merman-render --all-targets -- -D warnings`
  Evidence: `EVIDENCE_AND_GATES.md`.
  Handoff: DONE.
