# Pie 11.15 Parity - Handoff

Status: Active
Last updated: 2026-05-31

## Current State

PIE-020 and PIE-030 are implemented and task-local gates are green. Mermaid 11.15 Pie behavior
still differs from the local renderer in configured Pie behavior:

- Configured behavior: upstream supports `textPosition`, `donutHole`, `legendPosition`, and
  `highlightSlice`; generated defaults now expose those keys, but local layout/SVG still need to
  consume them.

## Active Task

- Task ID: PIE-040
- Owner: codex
- Files: `crates/merman-render/src/pie.rs`, `crates/merman-render/src/svg/parity/pie.rs`,
  `crates/merman-render/tests`
- Validation: `cargo nextest run -p merman-render pie`; targeted SVG/path assertions
- Status: READY
- Review: Confirm invalid donut values fall back to `0` like upstream and label radius uses
  configured `textPosition`.
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
- Implement configured behavior in separate slices: donut/text radius, legend placement, highlight
  classes/CSS.

## Concerns

- PIE-020 refreshed only the Pie layout goldens and the two upstream SVG baselines affected by
  input-order/color-domain behavior.
- `default_config.json` text now uses plain `forceLegacyMathML` / `legacyMathML` keys instead of
  equivalent Unicode escapes because the stabilized generator emits normal ASCII JSON keys.
- Legend-position support may expose root viewBox measurement differences.

## Next Recommended Action

Run PIE-040 to implement `pie.textPosition` and valid `pie.donutHole` geometry, then continue with
legend placement and highlight classes.
