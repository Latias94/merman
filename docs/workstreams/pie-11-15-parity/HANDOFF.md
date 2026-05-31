# Pie 11.15 Parity - Handoff

Status: Active
Last updated: 2026-05-31

## Current State

PIE-020 is implemented and task-local gates are green. Mermaid 11.15 Pie behavior still differs
from the local renderer in configured Pie behavior:

- Configured behavior: upstream supports `textPosition`, `donutHole`, `legendPosition`, and
  `highlightSlice`; local layout/SVG currently ignore those keys, and generated defaults remove
  them.

## Active Task

- Task ID: PIE-030
- Owner: codex
- Files: `crates/xtask/default_config_overrides.json`,
  `crates/merman-core/src/generated/default_config.json`
- Validation: `cargo run -p xtask -- gen-default-config`; `cargo run -p xtask -- verify-default-config`; `cargo nextest run -p merman-core config`
- Status: READY
- Review: Confirm only Pie removals are changed in the override manifest and generated artifact.
- Evidence: `docs/workstreams/pie-11-15-parity/EVIDENCE_AND_GATES.md`

## Decisions Since Last Update

- Treat Pie input-order rendering as the first implementation slice because it is a current-baseline
  behavior bug independent of new config keys.
- PIE-020 removed local descending value sorting to match Mermaid 11.15 `d3pie().sort(null)`.
- Hidden slices still reserve color-domain slots by seeding the Pie color scale with all sections
  before visible-slice filtering.
- Restore generated Pie config defaults only after the lane exists, so default config does not claim
  unsupported renderer behavior.
- Implement configured behavior in separate slices: donut/text radius, legend placement, highlight
  classes/CSS.

## Concerns

- PIE-020 refreshed only the Pie layout goldens and the two upstream SVG baselines affected by
  input-order/color-domain behavior.
- Legend-position support may expose root viewBox measurement differences.

## Next Recommended Action

Run PIE-030 to restore the Pie 11.15 config defaults through the generated-default-config pipeline,
then continue with PIE-040/050/060 configured rendering slices.
