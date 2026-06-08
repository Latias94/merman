# Post Alpha.2 Fearless Refactor — TODO

Status: Active
Last updated: 2026-06-08

## M0 — Plan Record

- [x] PA2R-010 [owner=codex] [deps=none] [scope=docs/workstreams/post-alpha2-fearless-refactor]
  Goal: Record the post-alpha.2 fearless refactor priorities and the local fallback because `ce-plan` is unavailable.
  Validation: `git diff --check -- docs/workstreams/post-alpha2-fearless-refactor`
  Evidence: `DESIGN.md`, `WORKSTREAM.json`

## M1 — Binding Render Request Module

- [x] PA2R-020 [owner=codex] [deps=PA2R-010] [scope=crates/merman-bindings-core/src/render.rs,crates/merman-bindings-core/src/render]
  Goal: Move binding render options, renderer construction, SVG pipeline construction, request execution, and render error classification behind a deeper request Module used by one-shot functions and cached engines.
  Validation: `cargo nextest run -p merman-bindings-core`; `cargo nextest run -p merman-ffi render_svg`; `cargo fmt --all --check`
  Review: Keep FFI/platform ABI stable; the refactor should reduce caller knowledge rather than only move code.
  Evidence: `crates/merman-bindings-core/src/render/request.rs`; focused gates passed on 2026-06-08.

## M2 — Next Architecture Slice

- [x] PA2R-030 [owner=codex] [deps=PA2R-020] [scope=crates/merman-core/src/family.rs,crates/merman-core/src/tests/registry.rs]
  Goal: Reassess Diagram Family Facts after bindings cleanup and deepen the next highest-leverage projection without changing public JSON output.
  Validation: `cargo nextest run -p merman-core registry`; `cargo nextest run -p merman-core detect`; `cargo run -p xtask -- check-alignment`
  Review: Supported diagram metadata should be projected from render parser facts instead of duplicating render parser id lists.
  Evidence: `RenderParserFact.metadata_id`; focused gates passed on 2026-06-08.

## M3 — Presentation Theme View

- [x] PA2R-040 [owner=codex] [deps=PA2R-030] [scope=crates/merman-render/src/svg/parity/theme.rs,crates/merman-render/src/svg/parity]
  Goal: Continue ADR-0068 by migrating one high-duplication raw `themeVariables` reader into `PresentationTheme` roles without moving host styling policy into the parity renderer.
  Validation: `cargo nextest run -p merman-render presentation_theme`; targeted renderer test for the migrated family; `cargo fmt --all --check`
  Review: Timeline renderer should consume prepared presentation roles for colors, section palette, disabled colors, root colors, and redux flags instead of walking raw `themeVariables` paths in `timeline.rs`.
  Evidence: `PresentationTheme::timeline`; focused gates passed on 2026-06-08.

## M4 — Public Headless Operation Interface Cleanup

- [x] PA2R-050 [owner=codex] [deps=PA2R-040] [scope=crates/merman]
  Goal: Reassess public headless operation entry points after bindings/theme cleanup, delete pass-through or duplicate operation surfaces that are not earning their interface, and keep parse/layout/render expert paths clearly separate from canonical headless render paths.
  Validation: `cargo check -p merman --features render`; `cargo nextest run -p merman --features render svg_pipeline_tests`; `cargo nextest run -p merman --features render render_svg`; `cargo fmt --all --check`
  Review: `HeadlessOperation` now owns the semantic-layout path and typed-render SVG path while public free functions and `HeadlessRenderer` remain adapters.
  Evidence: `crates/merman/src/render/operation.rs`; focused gates passed on 2026-06-08.

## M5 — Admission Inventory Module

- [ ] PA2R-060 [owner=codex] [deps=PA2R-050] [scope=xtask,docs/alignment,docs/workstreams]
  Goal: Reassess fixture/family admission status after the alpha.2 release and move duplicated support, skip, defer, and root-coverage facts toward one inventory Module.
  Validation: `cargo run -p xtask -- check-alignment`; focused xtask tests for touched inventory/report code; `cargo fmt --all --check`
  Review: Admission facts should be projectable into docs and gates without hand-maintained family lists drifting across reports.

## M6 — Xtask Parity Harness Module

- [ ] PA2R-070 [owner=codex] [deps=PA2R-060] [scope=xtask]
  Goal: Reduce compare/import/audit harness duplication and make DOM policy reporting more explicit without weakening comparator normalization.
  Validation: focused xtask tests for touched harness code; representative parity compare command; `cargo fmt --all --check`
  Review: Keep comparator normalization narrow and source-backed.
