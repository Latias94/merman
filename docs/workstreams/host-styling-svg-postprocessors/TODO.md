# Host Styling SVG Postprocessors - TODO

Status: Complete
Last updated: 2026-05-28

## M0 - Scope Freeze

- [x] HSSP-010 [owner=codex] [deps=none] [scope=docs/adr/0064-host-styling-svg-postprocessors.md,docs/workstreams/host-styling-svg-postprocessors]
  Goal: Freeze the host styling lane and record the Zed PR 57644 signal as an extension-point
  requirement.
  Validation:
  - Workstream docs exist.
  - ADR records the preset/built-in/host split.
  Evidence: ADR-0064 and workstream docs.
  Handoff: DONE.

## M1 - Pipeline Module Boundary

- [x] HSSP-020 [owner=codex] [deps=HSSP-010] [scope=crates/merman-render/src/svg/pipeline]
  Goal: Replace the single `pipeline.rs` file with focused modules for public API, context, preset
  composition, and built-ins.
  Validation:
  - Existing pipeline tests still pass.
  - Public re-exports remain source-compatible.
  Evidence: `crates/merman-render/src/svg/pipeline/mod.rs`, `context.rs`, `preset.rs`, and
  `builtin/*`; `cargo nextest run -p merman-render svg::pipeline`.
  Handoff: DONE.

## M2 - Metadata-Aware Context

- [x] HSSP-030 [owner=codex] [deps=HSSP-020] [scope=crates/merman-render/src/svg/pipeline,crates/merman/src/lib.rs]
  Goal: Pass diagram type, diagram title, and root SVG id through `SvgPostprocessContext`.
  Validation:
  - Unit test proves custom passes can read metadata.
  - `render_svg_with_pipeline_sync` plumbs parsed metadata.
  Evidence: `SvgPostprocessMetadata`, `SvgPostprocessContext::{diagram_type,diagram_title,svg_id}`,
  and `render::svg_pipeline_tests::render_svg_with_pipeline_passes_parsed_metadata`.
  Handoff: DONE.

## M3 - Host Styling Built-ins

- [x] HSSP-040 [owner=codex] [deps=HSSP-030] [scope=crates/merman-render/src/svg/pipeline/builtin]
  Goal: Add product-neutral styling postprocessors for scoped CSS injection, opt-in CSS override
  policy, and fallback text style propagation.
  Validation:
  - Tests cover scoped root-id selectors.
  - Tests cover opt-in stripping of existing `!important`.
  - Tests cover generated fallback text receiving inherited style/class information.
  Evidence: scoped CSS tests, CSS override tests, and
  `svg::parity::fallback::tests::foreign_object_overlay_propagates_style_context`.
  Handoff: DONE.

## M4 - Docs, Examples, And Changelog

- [x] HSSP-050 [owner=codex] [deps=HSSP-040] [scope=README.md,CHANGELOG.md,docs/rendering,crates/merman/examples]
  Goal: Document the host styling extension pattern and provide runnable code examples.
  Validation:
  - Example compiles.
  - Docs show scoped CSS and host-only accent logic without adding product semantics to core.
  Evidence: `crates/merman/examples/svg_pipeline.rs`, `README.md`,
  `docs/rendering/SVG_OUTPUT_PIPELINE.md`, and `CHANGELOG.md`; example compile and smoke run.
  Handoff: DONE.

## M5 - Verification And Closeout

- [x] HSSP-060 [owner=codex] [deps=HSSP-050] [scope=docs/workstreams/host-styling-svg-postprocessors]
  Goal: Run focused package gates, record evidence, and close the lane or split follow-ups.
  Validation:
  - `cargo fmt -p merman-render -p merman -p merman-cli -- --check`
  - `cargo nextest run -p merman-render`
  - `cargo nextest run -p merman --features raster`
  - `cargo nextest run -p merman-cli`
  - `cargo clippy -p merman-render -p merman --features raster --all-targets -- -D warnings`
  - `cargo clippy -p merman-cli --all-targets -- -D warnings`
  - `git diff --check`
  Evidence: `EVIDENCE_AND_GATES.md`.
  Handoff: DONE.
