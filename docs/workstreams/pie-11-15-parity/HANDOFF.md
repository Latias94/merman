# Pie 11.15 Parity - Handoff

Status: Active
Last updated: 2026-05-31

## Current State

The lane is open. Mermaid 11.15 Pie behavior differs from the local renderer in two areas:

- Baseline behavior: upstream preserves input order via `d3pie().sort(null)`, while local layout
  still sorts visible slices by descending value.
- Configured behavior: upstream supports `textPosition`, `donutHole`, `legendPosition`, and
  `highlightSlice`; local layout/SVG currently ignore those keys, and generated defaults remove
  them.

## Active Task

- Task ID: PIE-020
- Owner: codex
- Files: `crates/merman-render/src/pie.rs`, `crates/merman-render/tests`
- Validation: `cargo nextest run -p merman-render pie`; `cargo fmt --check`; `git diff --check`
- Status: READY
- Review: Confirm input-order slices and hidden-slice color-domain behavior.
- Evidence: `docs/workstreams/pie-11-15-parity/EVIDENCE_AND_GATES.md`

## Decisions Since Last Update

- Treat Pie input-order rendering as the first implementation slice because it is a current-baseline
  behavior bug independent of new config keys.
- Restore generated Pie config defaults only after the lane exists, so default config does not claim
  unsupported renderer behavior.
- Implement configured behavior in separate slices: donut/text radius, legend placement, highlight
  classes/CSS.

## Concerns

- Existing upstream Pie SVG baselines may encode old order if they were not regenerated after the
  11.15 CLI toolchain update. Do not refresh broad fixtures until the order change is proven.
- Legend-position support may expose root viewBox measurement differences.

## Next Recommended Action

Start PIE-020 with a red renderer test that proves current local slice order differs from Mermaid
11.15 input order, then remove the descending sort while preserving hidden-slice color-domain
behavior.
