# State Node Renderer Extraction - TODO

Status: Complete
Last updated: 2026-05-28

## M0 - Scope Freeze

- [x] SNR-010 [owner=codex] [deps=none] [scope=docs/workstreams/state-node-renderer-extraction]
  Goal: Freeze the next state renderer split around leaf-node emission.
  Validation: workstream docs exist.
  Evidence: `DESIGN.md`.
  Handoff: DONE.

## M1 - Extract Node Module

- [x] SNR-020 [owner=codex] [deps=SNR-010] [scope=crates/merman-render/src/svg/parity/state/render.rs,crates/merman-render/src/svg/parity/state/node.rs,crates/merman-render/src/svg/parity/state/mod.rs]
  Goal: Move state leaf-node rendering to `state/node.rs` without changing emitted SVG.
  Validation:
  - `cargo nextest run -p merman-render state`
  - `cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity --dom-decimals 3`
  Review: node shape markup, label HTML, link wrapping, Rough.js output, and timing counters stay
  unchanged.
  Evidence: `EVIDENCE_AND_GATES.md`.
  Handoff: DONE.

## M2 - Verification And Closeout

- [x] SNR-030 [owner=codex] [deps=SNR-020] [scope=docs/workstreams/state-node-renderer-extraction,docs/rendering/REFACTOR_TODO.md]
  Goal: Verify package gates and close this bounded extraction.
  Validation:
  - `cargo fmt -p merman-render -- --check`
  - `cargo nextest run -p merman-render`
  - `cargo clippy -p merman-render --all-targets -- -D warnings`
  Evidence: `EVIDENCE_AND_GATES.md`.
  Handoff: DONE.
