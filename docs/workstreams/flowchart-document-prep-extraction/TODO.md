# Flowchart Document Prep Extraction - TODO

Status: Complete
Last updated: 2026-05-28

## M0 - Scope Freeze

- [x] FDP-010 [owner=codex] [deps=none] [scope=docs/workstreams/flowchart-document-prep-extraction]
  Goal: Freeze the first flowchart `svg_emit.rs` split around root document preparation.
  Validation: workstream docs exist.
  Evidence: `DESIGN.md`.
  Handoff: DONE.

## M1 - Extract Document Prep

- [x] FDP-020 [owner=codex] [deps=FDP-010] [scope=crates/merman-render/src/svg/parity/flowchart/svg_emit.rs,crates/merman-render/src/svg/parity/flowchart/document.rs,crates/merman-render/src/svg/parity/flowchart/mod.rs]
  Goal: Move flowchart root viewport formatting, root overrides, root attrs, and accessibility
  title/desc handling to `flowchart/document.rs`.
  Validation:
  - `cargo nextest run -p merman-render flowchart`
  - `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity --dom-decimals 3 --text-measurer vendored`
  Review: emitted root attrs, title/desc nodes, and viewBox/max-width behavior stay unchanged.
  Evidence: `EVIDENCE_AND_GATES.md`.
  Handoff: DONE.

## M2 - Verification And Closeout

- [x] FDP-030 [owner=codex] [deps=FDP-020] [scope=docs/workstreams/flowchart-document-prep-extraction,docs/rendering/REFACTOR_TODO.md]
  Goal: Verify package gates and close this bounded extraction.
  Validation:
  - `cargo fmt -p merman-render -- --check`
  - `cargo nextest run -p merman-render`
  - `cargo clippy -p merman-render --all-targets -- -D warnings`
  Evidence: `EVIDENCE_AND_GATES.md`.
  Handoff: DONE.
