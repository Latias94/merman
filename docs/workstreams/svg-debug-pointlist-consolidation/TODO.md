# SVG Debug Point-List Consolidation - TODO

Status: Complete
Last updated: 2026-05-28

## M0 - Scope Freeze

- [x] SDP-010 [owner=codex] [deps=none] [scope=docs/workstreams/svg-debug-pointlist-consolidation]
  Goal: Freeze the debug point-list consolidation slice.
  Validation: workstream docs exist.
  Evidence: `DESIGN.md`.
  Handoff: DONE.

## M1 - Debug Polyline Adopters

- [x] SDP-020 [owner=codex] [deps=SDP-010] [scope=crates/merman-render/src/svg/parity.rs,crates/merman-render/src/svg/parity/er.rs,crates/merman-render/src/svg/parity/flowchart/debug_svg.rs,crates/merman-render/src/svg/parity/class/debug_svg.rs,crates/merman-render/src/svg/parity/state/debug_svg.rs,crates/merman-render/src/svg/parity/sequence/debug.rs]
  Goal: Use the shared point-list helper in low-risk debug polyline emitters.
  Validation:
  - `cargo nextest run -p merman-render fmt_points`
  - `cargo nextest run -p merman-render debug_svg`
  Review: emitted point separators and number formatting stay unchanged.
  Evidence: `EVIDENCE_AND_GATES.md`.
  Handoff: DONE.

## M2 - Verification And Closeout

- [x] SDP-030 [owner=codex] [deps=SDP-020] [scope=docs/workstreams/svg-debug-pointlist-consolidation,docs/rendering/REFACTOR_TODO.md]
  Goal: Verify package gates and close this bounded follow-on.
  Validation:
  - `cargo fmt -p merman-render -- --check`
  - `cargo nextest run -p merman-render`
  - `cargo clippy -p merman-render --all-targets -- -D warnings`
  Evidence: `EVIDENCE_AND_GATES.md`.
  Handoff: DONE.
