# Pie 11.15 Parity - Handoff

Status: Active
Last updated: 2026-05-31

## Current State

PIE-020, PIE-030, and PIE-040 are implemented and task-local gates are green. Mermaid 11.15 Pie
behavior still differs from the local renderer in the remaining configured Pie behavior:

- Remaining configured behavior: upstream supports `legendPosition` and `highlightSlice`;
  generated defaults expose those keys, but local layout/SVG still need to consume them.

## Active Task

- Task ID: PIE-050
- Owner: codex
- Files: `crates/merman-render/src/pie.rs`, `crates/merman-render/src/svg/parity/pie.rs`,
  `crates/merman-render/tests`
- Validation: `cargo nextest run -p merman-render pie`; selected `compare-pie-svgs` parity checks
- Status: READY
- Review: Confirm viewBox dimensions and pie/legend transforms match upstream layout rules for
  `top`, `bottom`, `left`, `right`, and `center`.
- Evidence: `docs/workstreams/pie-11-15-parity/EVIDENCE_AND_GATES.md`

## Decisions Since Last Update

- Treat Pie input-order rendering as the first implementation slice because it is a current-baseline
  behavior bug independent of new config keys.
- PIE-020 removed local descending value sorting to match Mermaid 11.15 `d3pie().sort(null)`.
- Hidden slices still reserve color-domain slots by seeding the Pie color scale with all sections
  before visible-slice filtering.
- PIE-030 restored `pie.donutHole`, `pie.highlightSlice`, and `pie.legendPosition` in generated
  defaults and added a core config exposure regression test.
- `gen-default-config` now recursively sorts JSON keys and writes a trailing newline, preventing
  noisy generated diffs from `serde_json` insertion-order output.
- PIE-040 applies `pie.textPosition` to slice label radius and renders valid `pie.donutHole` values
  as annular paths; invalid donut values fall back to solid slices like upstream.
- Implement configured behavior in separate slices: donut/text radius, legend placement, highlight
  classes/CSS.

## Concerns

- PIE-020 refreshed only the Pie layout goldens and the two upstream SVG baselines affected by
  input-order/color-domain behavior.
- `default_config.json` text now uses plain `forceLegacyMathML` / `legacyMathML` keys instead of
  equivalent Unicode escapes because the stabilized generator emits normal ASCII JSON keys.
- PIE-040 refreshed only the three Pie layout goldens that already configured `textPosition`.
- Legend-position support may expose root viewBox measurement differences.

## Next Recommended Action

Run PIE-050 to implement `pie.legendPosition` layout/SVG behavior, then continue with highlight
classes.
