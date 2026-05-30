# ASCII Flowchart Subgraph Title Wrapping — Handoff

Status: Closed
Last updated: 2026-05-30

## Current State

The lane is closed. Long flowchart subgraph titles now wrap through the shared graph-label path,
and explicit multiline subgraph titles still render as hard-break centered rows.

## Active Task

- Task ID: none
- Owner: none
- Files: none
- Validation: complete; see `EVIDENCE_AND_GATES.md`
- Status: closed
- Review: no blocking findings from final self-review
- Evidence: `EVIDENCE_AND_GATES.md`

## Decisions Since Last Update

- Kept the feature inside the graph label/layout/draw path.
- Reused `wrap_display_lines` through `GraphLabel::wrapped`; no parser changes were needed.
- Preserved explicit multiline breaks as hard breaks before automatic wrapping.
- Documented wrapped subgraph titles as shipped behavior in crate and root support docs.

## Blockers

- None.

## Next Recommended Action

- Continue with a separate lane only if broader title wrapping, node wrapping, or subgraph direction
  overrides become priority.
