# Resvg-Safe SVG Output Pipeline - TODO

Status: Active
Last updated: 2026-05-28

## M0 - Scope Freeze

- [x] RSO-010 [owner=codex] [deps=none] [scope=docs/adr/0063-extensible-svg-output-pipeline.md,docs/workstreams/resvg-safe-svg-output]
  Goal: Freeze the output-pipeline boundary and record the Zed PR 57644 signal as requirements
  evidence.
  Validation:
  - Workstream docs exist.
  - ADR records the parity-vs-consumer-output split.
  Evidence: `DESIGN.md`, `WORKSTREAM.json`, ADR-0063.
  Handoff: DONE.

## M1 - Readable Fallback Correctness

- [x] RSO-020 [owner=codex] [deps=RSO-010] [scope=crates/merman-render/src/svg/parity/fallback.rs]
  Goal: Fix readable fallback text extraction for literal `\n` inside `<foreignObject>` labels.
  Validation:
  - `cargo nextest run -p merman-render foreign_object_overlay_splits_literal_backslash_n`
  - `cargo nextest run -p merman-render svg::parity::fallback::tests::foreign_object_overlay`
  Evidence: `EVIDENCE_AND_GATES.md`.
  Handoff: DONE.

## M2 - Pipeline Skeleton

- [ ] RSO-030 [owner=codex] [deps=RSO-020] [scope=crates/merman-render/src/svg,crates/merman/src/lib.rs]
  Goal: Introduce `SvgPipeline` and built-in `Parity`, `Readable`, and `ResvgSafe` presets without
  changing default `render_svg_sync` behavior.
  Validation:
  - `cargo nextest run -p merman-render fallback`
  - `cargo nextest run -p merman`
  - `cargo fmt -p merman-render -p merman -- --check`
  Evidence: pending.
  Handoff: TODO.

- [ ] RSO-040 [owner=codex] [deps=RSO-030] [scope=crates/merman/src/lib.rs,crates/merman/src/render/raster.rs]
  Goal: Route `render_svg_readable_sync`, PNG, JPEG, and PDF export through the shared pipeline.
  Validation:
  - `cargo nextest run -p merman`
  - `cargo nextest run -p merman-cli png_smoke jpeg_smoke pdf_smoke`
  Evidence: pending.
  Handoff: TODO.

## M3 - Resvg-Safe Built-ins

- [ ] RSO-050 [owner=codex] [deps=RSO-040] [scope=crates/merman-render/src/svg]
  Goal: Add built-in cleanup passes for unsupported CSS, empty/invalid visual attributes, and
  malformed dimensions that are unsafe for `usvg` / `resvg`.
  Validation:
  - New regression tests for `@keyframes`, `:root`, `deg`, empty fill/stroke/width/height, and
    `NaN`.
  - `cargo nextest run -p merman-render svg`
  Evidence: pending.
  Handoff: TODO.

## M4 - Host Extension API

- [ ] RSO-060 [owner=codex] [deps=RSO-050] [scope=crates/merman-render/src/svg,crates/merman/src/lib.rs]
  Goal: Expose a public string/Cow `SvgPostprocessor` trait and builder API for host-provided SVG
  passes.
  Validation:
  - Test that a custom pass runs after built-in presets in deterministic order.
  - Test that custom pass errors surface as render errors without panics.
  Evidence: pending.
  Handoff: TODO.

- [ ] RSO-070 [owner=codex] [deps=RSO-060] [scope=README.md,CHANGELOG.md,docs/rendering]
  Goal: Document the pipeline, presets, and extension boundary for UI/raster consumers.
  Validation:
  - README examples compile or are syntax-checked where practical.
  - Changelog records public API and raster/readable behavior changes.
  Evidence: pending.
  Handoff: TODO.

## M5 - Verification And Closeout

- [ ] RSO-080 [owner=codex] [deps=RSO-070] [scope=docs/workstreams/resvg-safe-svg-output]
  Goal: Run package gates, record evidence, and close or split follow-up work.
  Validation:
  - `cargo fmt -p merman-render -p merman -- --check`
  - `cargo nextest run -p merman-render`
  - `cargo nextest run -p merman`
  - `cargo clippy -p merman-render -p merman --all-targets -- -D warnings`
  - `git diff --check`
  Evidence: pending.
  Handoff: TODO.
