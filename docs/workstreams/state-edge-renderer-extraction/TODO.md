# State Edge Renderer Extraction - TODO

Status: Complete
Last updated: 2026-05-28

## M0 - Scope Freeze

- [x] SER-010 [owner=codex] [deps=none] [scope=docs/workstreams/state-edge-renderer-extraction]
  Goal: Freeze the first state renderer split around edge rendering.
  Validation: workstream docs exist.
  Evidence: `DESIGN.md`.
  Handoff: DONE.

## M1 - Extract Edge Module

- [x] SER-020 [owner=codex] [deps=SER-010] [scope=crates/merman-render/src/svg/parity/state/render.rs,crates/merman-render/src/svg/parity/state/edge.rs,crates/merman-render/src/svg/parity/state/mod.rs]
  Goal: Move state edge path, label, cluster-boundary clipping, and self-loop special-edge logic to
  `state/edge.rs` without changing emitted SVG.
  Validation:
  - `cargo nextest run -p merman-render state`
  - `cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity --dom-decimals 3`
  Review: output strings, label positioning, and `data-points` behavior remain unchanged.
  Evidence: `EVIDENCE_AND_GATES.md`.
  Handoff: DONE.

## M2 - Verification And Closeout

- [x] SER-030 [owner=codex] [deps=SER-020] [scope=docs/workstreams/state-edge-renderer-extraction,docs/rendering/REFACTOR_TODO.md]
  Goal: Verify package gates and close this bounded extraction.
  Validation:
  - `cargo fmt -p merman-render -- --check`
  - `cargo nextest run -p merman-render`
  - `cargo clippy -p merman-render --all-targets -- -D warnings`
  Evidence: `EVIDENCE_AND_GATES.md`.
  Handoff: DONE.
