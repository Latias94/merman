# Flowchart Edge Geometry Compute Extraction - TODO

Status: Complete
Last updated: 2026-05-28

## M0 - Scope Freeze

- [x] FEGC-010 [owner=codex] [deps=none] [scope=docs/workstreams/flowchart-edge-geom-compute-extraction]
  Goal: Freeze the slice around moving edge path geometry computation out of `flowchart/mod.rs`.
  Validation: workstream docs exist.
  Evidence: `DESIGN.md`.
  Handoff: DONE.

## M1 - Extract Compute Module

- [x] FEGC-020 [owner=codex] [deps=FEGC-010] [scope=crates/merman-render/src/svg/parity/flowchart/mod.rs,crates/merman-render/src/svg/parity/flowchart/edge_geom.rs,crates/merman-render/src/svg/parity/flowchart/edge_geom/compute.rs]
  Goal: Move edge path geometry implementation into `flowchart/edge_geom/compute.rs`.
  Validation:
  - `cargo nextest run -p merman-render flowchart`
  Evidence: `EVIDENCE_AND_GATES.md`.
  Handoff: DONE.

## M2 - Verification And Closeout

- [x] FEGC-030 [owner=codex] [deps=FEGC-020] [scope=docs/workstreams/flowchart-edge-geom-compute-extraction,docs/rendering/REFACTOR_TODO.md]
  Goal: Verify package gates and close this extraction.
  Validation:
  - `cargo fmt -p merman-render -- --check`
  - `cargo nextest run -p merman-render`
  - `cargo clippy -p merman-render --all-targets -- -D warnings`
  Evidence: `EVIDENCE_AND_GATES.md`.
  Handoff: DONE.
