# Render Parser Registry - TODO

Status: Complete
Last updated: 2026-05-28

## M0 - Scope Freeze

- [x] RPR-010 [owner=planner] [deps=none] [scope=docs/workstreams/render-parser-registry]
  Goal: Freeze registry extraction scope and non-goals.
  Validation: workstream docs exist.
  Evidence: `DESIGN.md`
  Handoff: DONE.

## M1 - Registry Extraction

- [x] RPR-020 [owner=codex] [deps=RPR-010] [scope=crates/merman-core/src/diagram/mod.rs,crates/merman-core/src/lib.rs]
  Goal: Replace the core typed render parser match with `RenderDiagramRegistry`.
  Validation: `cargo nextest run -p merman-core render_parser_registry`
  Review: no duplicated behavior drift.
  Evidence: `crates/merman-core/src/tests/misc.rs::render_parser_registry_*`
  Handoff: DONE.

## M2 - Verification And Closeout

- [x] RPR-030 [owner=codex] [deps=RPR-020] [scope=docs/workstreams/render-parser-registry]
  Goal: Verify package gates and close the lane.
  Validation:
  - `cargo fmt -p merman-core -p merman-render -- --check`
  - `cargo nextest run -p merman-core`
  - `cargo nextest run -p merman-render`
  - `cargo clippy -p merman-core --all-targets -- -D warnings`
  - `cargo clippy -p merman-render --all-targets -- -D warnings`
  Evidence: `EVIDENCE_AND_GATES.md`
  Handoff: DONE. Macro/table generation remains a follow-on only if registry boilerplate keeps
  growing.
