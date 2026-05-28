# Flowchart Geometry Helper Consolidation - TODO

Status: Complete
Last updated: 2026-05-28

## M0 - Scope Freeze

- [x] FGH-010 [owner=codex] [deps=none] [scope=docs/workstreams/flowchart-geometry-helper-consolidation]
  Goal: Freeze the cleanup around duplicate node geometry helpers.
  Validation: workstream docs exist.
  Evidence: `DESIGN.md`.
  Handoff: DONE.

## M1 - Remove Duplicate Helpers

- [x] FGH-020 [owner=codex] [deps=FGH-010] [scope=crates/merman-render/src/svg/parity/flowchart/svg_emit.rs,crates/merman-render/src/svg/parity/flowchart/render/node.rs,crates/merman-render/src/svg/parity/flowchart/render/node/geom.rs]
  Goal: Reuse node geometry helpers from the viewBox bounds code and delete local duplicates.
  Validation:
  - `cargo nextest run -p merman-render flowchart`
  - `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity --dom-decimals 3 --text-measurer vendored`
  Evidence: `EVIDENCE_AND_GATES.md`.
  Handoff: DONE.

## M2 - Verification And Closeout

- [x] FGH-030 [owner=codex] [deps=FGH-020] [scope=docs/workstreams/flowchart-geometry-helper-consolidation,docs/rendering/REFACTOR_TODO.md]
  Goal: Verify package gates and close this cleanup.
  Validation:
  - `cargo fmt -p merman-render -- --check`
  - `cargo nextest run -p merman-render`
  - `cargo clippy -p merman-render --all-targets -- -D warnings`
  Evidence: `EVIDENCE_AND_GATES.md`.
  Handoff: DONE.
