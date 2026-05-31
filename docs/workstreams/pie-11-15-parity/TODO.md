# Pie 11.15 Parity - TODO

Status: Active
Last updated: 2026-05-31

## M0 - Scope And Evidence Freeze

- [x] PIE-010 [owner=planner] [deps=none] [scope=docs/workstreams/pie-11-15-parity]
  Goal: Open the lane, identify upstream authority, and split Pie 11.15 behavior into provable
  vertical slices.
  Validation: Workstream docs exist and agree.
  Review: Confirm this is not a generated-config-only problem.
  Evidence: `docs/workstreams/pie-11-15-parity/DESIGN.md`
  Context: `docs/workstreams/pie-11-15-parity/CONTEXT.jsonl`
  Handoff: DONE. Upstream 11.15 Pie behavior includes input-order slices plus `textPosition`,
  `donutHole`, `legendPosition`, and `highlightSlice`.

## M1 - 11.15 Baseline Behavior

- [x] PIE-020 [owner=codex] [deps=PIE-010] [scope=crates/merman-render/src/pie.rs,crates/merman-render/tests]
  Goal: Render visible Pie slices in input order while preserving Mermaid's hidden-slice color-domain
  behavior.
  Validation: `cargo nextest run -p merman-render pie`; `cargo run -p xtask -- compare-pie-svgs --check-dom --dom-mode parity --dom-decimals 3` when fixture baselines are updated.
  Review: Confirm the change follows Mermaid 11.15 `d3pie().sort(null)` and does not regress color
  assignment for hidden slices.
  Evidence: `docs/workstreams/pie-11-15-parity/EVIDENCE_AND_GATES.md`
  Context: upstream `pieRenderer.ts`.
  Handoff: DONE. Added renderer tests for input-order slices and hidden-slice color-domain
  reservation, removed the descending value sort, and refreshed affected Pie layout/SVG baselines.

- [x] PIE-030 [owner=codex] [deps=PIE-010] [scope=crates/xtask/default_config_overrides.json,crates/xtask/src/cmd/generate.rs,crates/merman-core/src/generated/default_config.json,crates/merman-core/src/tests/pie.rs]
  Goal: Restore supported Pie 11.15 config keys in generated defaults.
  Validation: `cargo run -p xtask -- gen-default-config`; `cargo run -p xtask -- verify-default-config`; `cargo nextest run -p merman-core config`.
  Review: Confirm only Pie removals are changed in the override manifest and generated artifact.
  Evidence: `docs/workstreams/pie-11-15-parity/EVIDENCE_AND_GATES.md`
  Context: ADR-0019 and generated-default-config closeout.
  Handoff: DONE. Removed the Pie key removals from the override manifest, regenerated defaults,
  added a Pie config default/override regression test, and made default-config generation
  recursively key-sorted with a trailing newline to avoid noisy generated diffs.

## M2 - Configured Rendering

- [x] PIE-040 [owner=codex] [deps=PIE-020,PIE-030] [scope=crates/merman-render/src/pie.rs,crates/merman-render/src/svg/parity/pie.rs,crates/merman-render/tests]
  Goal: Implement `pie.textPosition` and valid `pie.donutHole` geometry.
  Validation: `cargo nextest run -p merman-render pie`; targeted SVG/path assertions.
  Review: Confirm invalid donut values fall back to `0` like upstream and label radius uses
  configured text position.
  Evidence: `docs/workstreams/pie-11-15-parity/EVIDENCE_AND_GATES.md`
  Context: upstream `pieRenderer.ts`.
  Handoff: DONE. Layout now reads `pie.textPosition`; SVG path generation renders valid donut
  holes as annular arcs and falls back to solid slices for invalid values.

- [ ] PIE-050 [owner=codex] [deps=PIE-030] [scope=crates/merman-render/src/pie.rs,crates/merman-render/src/svg/parity/pie.rs,crates/merman-render/tests]
  Goal: Implement `pie.legendPosition` for `top`, `bottom`, `left`, `right`, and `center`.
  Validation: `cargo nextest run -p merman-render pie`; selected `compare-pie-svgs` parity checks.
  Review: Confirm viewBox dimensions and pie/legend transforms match upstream layout rules.
  Evidence: `docs/workstreams/pie-11-15-parity/EVIDENCE_AND_GATES.md`
  Context: upstream `pieRenderer.ts`.
  Handoff: Not started.

- [ ] PIE-060 [owner=codex] [deps=PIE-030] [scope=crates/merman-render/src/svg/parity/pie.rs,crates/merman-render/src/svg/parity/css.rs,crates/merman-render/tests]
  Goal: Implement `pie.highlightSlice` classes and Pie highlight CSS.
  Validation: `cargo nextest run -p merman-render pie`; SVG assertions for matching labels and
  `hover`.
  Review: Confirm default output remains unchanged when `highlightSlice` is empty.
  Evidence: `docs/workstreams/pie-11-15-parity/EVIDENCE_AND_GATES.md`
  Context: upstream `pieStyles.ts`.
  Handoff: Not started.

## M3 - Closeout

- [ ] PIE-070 [owner=planner] [deps=PIE-020,PIE-030,PIE-040,PIE-050,PIE-060] [scope=docs/workstreams/pie-11-15-parity,docs/alignment]
  Goal: Close the lane or split residual Pie parity debt.
  Validation: Fresh closeout gates recorded in `EVIDENCE_AND_GATES.md`.
  Review: Run workstream review and fresh verification before marking complete.
  Evidence: `docs/workstreams/pie-11-15-parity/EVIDENCE_AND_GATES.md`
  Context: this workstream.
  Handoff: Not started.
