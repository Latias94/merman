# SVG Parity Helper Consolidation - TODO

Status: Complete
Last updated: 2026-05-28

## M0 - Scope Freeze

- [x] SPH-010 [owner=planner] [deps=none] [scope=docs/workstreams/svg-parity-helper-consolidation]
  Goal: Freeze first bounded helper consolidation task.
  Validation: workstream docs exist.
  Evidence: `DESIGN.md`
  Handoff: DONE.

## M1 - Point List Helper

- [x] SPH-020 [owner=codex] [deps=SPH-010] [scope=crates/merman-render/src/svg/parity/util.rs,crates/merman-render/src/svg/parity/radar.rs]
  Goal: Add shared point-list formatting helper and adopt it in radar polygon emission.
  Validation:
  - `cargo nextest run -p merman-render fmt_points`
  - `cargo nextest run -p merman-render radar`
  Review: preserve point separator and `fmt_display` semantics.
  Evidence: focused util/render tests and radar SVG DOM parity gate.
  Handoff: DONE.

## M2 - Verification And Closeout

- [x] SPH-030 [owner=codex] [deps=SPH-020] [scope=docs/workstreams/svg-parity-helper-consolidation]
  Goal: Verify package gates and close or split next adopter.
  Validation:
  - `cargo fmt -p merman-render -- --check`
  - `cargo nextest run -p merman-render`
  - `cargo clippy -p merman-render --all-targets -- -D warnings`
  Evidence: `EVIDENCE_AND_GATES.md`
  Handoff: DONE. Next adopter should be a separate bounded task.
