# ASCII Class ER Graph Layout - Handoff

Status: Active
Last updated: 2026-05-30

## Current State

This lane is a follow-on from the closed
`docs/workstreams/ascii-reference-implementation-expansion/` lane. It exists because class and ER
ASCII rendering are now useful but still intentionally reject multi-relationship layouts.

Current class support renders boxes, members, methods, labels, and single-relationship layouts for
extension, dependency, aggregation, and composition. Current ER support renders entity boxes,
attributes, labels, identifying/non-identifying relationships, and common cardinality markers.

## Active Task

- Task ID: ACEG-010
- Owner: codex
- Files:
  - `docs/workstreams/ascii-class-er-graph-layout/*`
- Validation: `git diff --check`
- Status: DONE once the opening docs are committed.
- Review: Check that this lane does not absorb color/style, state, flowchart direction, or XYChart
  layout work.
- Evidence: `EVIDENCE_AND_GATES.md`

## Next Recommended Action

Run ACEG-020:

- Add parser-backed tests that capture currently unsupported class and ER multi-relationship cases.
- Keep production code unchanged until the test contract is explicit.
- Do not add a shared layout abstraction before both class and ER tests justify it.

## Constraints

- Do not port a Mermaid parser from any reference implementation.
- Do not reuse SVG layout or browser measurement as the ASCII source of truth.
- Do not silently omit relationships.
- Keep relationship semantics in class/ER adapters; keep any shared module terminal-layout-only.
- Stage only files for the active task.
