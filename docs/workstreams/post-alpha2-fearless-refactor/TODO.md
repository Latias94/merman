# Post Alpha.2 Fearless Refactor — TODO

Status: Active
Last updated: 2026-06-08

## M0 — Plan Record

- [x] PA2R-010 [owner=codex] [deps=none] [scope=docs/workstreams/post-alpha2-fearless-refactor]
  Goal: Record the post-alpha.2 fearless refactor priorities and link the durable CE plan.
  Validation: `git diff --check -- docs/workstreams/post-alpha2-fearless-refactor`
  Evidence: `DESIGN.md`, `WORKSTREAM.json`, `docs/plans/2026-06-08-001-refactor-post-alpha2-fearless-refactor-plan.md`

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

- [x] PA2R-060 [owner=codex] [deps=PA2R-050] [scope=crates/xtask/src/cmd/admission.rs]
  Goal: Reassess fixture/family admission status after the alpha.2 release and move duplicated support, skip, defer, and root-coverage facts toward one inventory Module.
  Validation: `cargo nextest run -p xtask admission`; `cargo run -p xtask -- check-alignment`; `cargo check -p xtask`; `cargo fmt --all --check`
  Review: Admission status combinations now live behind `DiagramAdmissionRecord` and status helper methods instead of leaking enum combinations into projections and alignment checks.
  Evidence: `crates/xtask/src/cmd/admission.rs`; focused gates passed on 2026-06-08.

## M6 — Xtask Parity Harness Module

- [x] PA2R-070 [owner=codex] [deps=PA2R-060] [scope=crates/xtask/src/cmd/compare]
  Goal: Reduce compare/import/audit harness duplication and make DOM policy reporting more explicit without weakening comparator normalization.
  Validation: `cargo nextest run -p xtask compare_adapter_registry`; `cargo nextest run -p xtask diagram_filter_key`; `cargo run -p xtask -- compare-all-svgs --diagram info --filter upstream_info_spec --check-dom --dom-mode parity --dom-decimals 3`; `cargo check -p xtask`; `cargo fmt --all --check`
  Review: `compare-all-svgs` now delegates diagram dispatch to the per-diagram compare adapter registry instead of carrying a duplicate match over all primary SVG matrix diagrams.
  Evidence: `crates/xtask/src/cmd/compare/diagrams.rs`; focused gates passed on 2026-06-08.

## M7 — Compare Harness Common Arguments

- [x] PA2R-080 [owner=codex] [deps=PA2R-070] [scope=crates/xtask/src/cmd/compare]
  Goal: Continue deepening the compare harness by moving common compare argument construction, report path naming, and optional root-report argument policy behind a small internal Module.
  Validation: `cargo nextest run -p xtask compare_invocation`; `cargo nextest run -p xtask root_parity_policy`; `cargo run -p xtask -- compare-all-svgs --diagram info --filter upstream_info_spec --check-dom --dom-mode parity --dom-decimals 3`; `cargo check -p xtask`; `cargo fmt --all --check`
  Review: `compare-all-svgs` now builds per-diagram command arguments through `CompareAllInvocationOptions`, keeping DOM mode, mode-suffixed report paths, flowchart text measurement, and optional root-report flags out of the main execution loop.
  Evidence: `crates/xtask/src/cmd/compare/all.rs`; focused gates passed on 2026-06-08.

## M8 — Root Parity Residual Policy Module

- [x] PA2R-090 [owner=codex] [deps=PA2R-080] [scope=crates/xtask/src/cmd/compare]
  Goal: Reassess whether fixture-specific root parity residual acceptance should stay in `compare/all.rs` or move behind a dedicated policy Module with a narrower Interface.
  Validation: `cargo nextest run -p xtask root_parity_policy`; `cargo nextest run -p xtask compare_invocation`; `cargo run -p xtask -- compare-all-svgs --diagram info --filter upstream_info_spec --check-dom --dom-mode parity --dom-decimals 3`; `cargo check -p xtask`; `cargo fmt --all --check`
  Review: Fixture-specific root parity residual records and summary behavior now live behind `RootParityResidualPolicy`, so `compare-all-svgs` no longer owns acceptance fragments or mismatch summarization.
  Evidence: `crates/xtask/src/cmd/compare/root_parity.rs`; focused gates passed on 2026-06-08.

## M9 — Compare-All Options And Diagram Selection

- [x] PA2R-100 [owner=codex] [deps=PA2R-090] [scope=crates/xtask/src/cmd/compare]
  Goal: Continue shrinking `compare-all-svgs` by moving CLI option parsing, diagram allow/skip filtering, and root-parity global sweep filtering behind small internal types.
  Validation: `cargo nextest run -p xtask compare_all_options`; `cargo nextest run -p xtask compare_all_diagram_selection`; `cargo nextest run -p xtask compare_invocation`; `cargo nextest run -p xtask root_parity_policy`; `cargo run -p xtask -- compare-all-svgs --diagram info --filter upstream_info_spec --check-dom --dom-mode parity --dom-decimals 3`; `cargo check -p xtask`; `cargo fmt --all --check`
  Review: Option parsing, diagram allow/skip selection, `treeView` alias handling, unmatched-filter skip policy, and root-deferred global sweep filtering now live behind internal types while preserving legacy CLI behavior.
  Evidence: `CompareAllOptions`; `CompareAllDiagramSelection`; focused gates passed on 2026-06-08.

## M10 — Compare Harness Follow-Up Selection

- [ ] PA2R-110 [owner=codex] [deps=PA2R-100] [scope=crates/xtask/src/cmd/compare,docs/workstreams/post-alpha2-fearless-refactor]
  Goal: Reassess the remaining compare harness complexity after PA2R-100 and select the next deletion-test refactor only if it removes duplicated policy or narrows a release-facing interface.
  Validation: selected focused xtask tests; representative `compare-all-svgs` command if compare behavior changes; `cargo fmt --all --check`
  Review: Prefer failure aggregation/reporting cleanup or a higher-leverage non-xtask slice over style-only helper extraction.
