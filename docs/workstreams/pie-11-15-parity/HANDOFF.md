# Pie 11.15 Parity - Handoff

Status: Active
Last updated: 2026-05-31

## Current State

PIE-020 through PIE-060 are implemented and task-local gates are green. The lane is ready for
closeout/review.

- Remaining behavior: no known Pie 11.15 configured-rendering task remains in this lane.

## Active Task

- Task ID: PIE-070
- Owner: codex
- Files: `docs/workstreams/pie-11-15-parity`, `docs/alignment`
- Validation: Fresh closeout gates recorded in `EVIDENCE_AND_GATES.md`
- Status: READY
- Review: Run workstream review and fresh verification before marking complete.
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
- PIE-060 emits upstream Pie highlight CSS and `highlighted`/`highlightedOnHover` path classes.
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
- Closeout should decide whether `docs/alignment/PIE_UPSTREAM_TEST_COVERAGE.md` needs a short
  update for the newly covered 11.15 config keys.

## Next Recommended Action

Run PIE-070 closeout: review the lane, record fresh final gates, update alignment notes if useful,
and close or split any residual Pie parity debt.
