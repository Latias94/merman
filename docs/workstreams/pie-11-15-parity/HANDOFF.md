# Pie 11.15 Parity - Handoff

Status: Active
Last updated: 2026-05-31

## Current State

PIE-020, PIE-030, PIE-040, and PIE-050 are implemented and task-local gates are green. Mermaid
11.15 Pie behavior still differs from the local renderer in highlight classes:

- Remaining configured behavior: upstream supports `highlightSlice`; generated defaults expose it,
  but local layout/SVG still need to consume it.

## Active Task

- Task ID: PIE-060
- Owner: codex
- Files: `crates/merman-render/src/pie.rs`, `crates/merman-render/src/svg/parity/pie.rs`,
  `crates/merman-render/tests`
- Validation: `cargo nextest run -p merman-render pie`; selected `compare-pie-svgs` parity checks
- Status: READY
- Review: Confirm default output remains unchanged when `highlightSlice` is empty and matching
  slice classes are emitted for hover and direct highlight cases.
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
- PIE-050 matches upstream legend placement for `top`, `bottom`, `left`, `right`, and `center`
  without changing the default-right SVG shape.
- Implement configured behavior in separate slices: donut/text radius, legend placement, highlight
  classes/CSS.

## Concerns

- PIE-020 refreshed only the Pie layout goldens and the two upstream SVG baselines affected by
  input-order/color-domain behavior.
- `default_config.json` text now uses plain `forceLegacyMathML` / `legacyMathML` keys instead of
  equivalent Unicode escapes because the stabilized generator emits normal ASCII JSON keys.
- PIE-040 refreshed only the three Pie layout goldens that already configured `textPosition`.
- PIE-050 did not require new Pie layout goldens because the tested legend-position cases were
  covered through direct layout/SVG assertions.
- Legend-position support may expose root viewBox measurement differences.

## Next Recommended Action

Run PIE-060 to implement `pie.highlightSlice` classes and Pie highlight CSS.
