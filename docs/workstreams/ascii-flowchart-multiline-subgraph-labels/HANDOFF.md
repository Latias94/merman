# ASCII Flowchart Multiline Subgraph Labels - Handoff

Status: Active
Last updated: 2026-05-30

## Current State

The lane is open. Existing code rejects direct-model subgraph titles with real newlines and renders
parser-preserved break syntax as a raw title string instead of multiple centered title rows.

## Active Task

- Task ID: AFMS-020
- Owner: unassigned
- Files:
  - `crates/merman-ascii/tests/flowchart_model.rs`
  - `crates/merman-ascii/src/lib.rs`
- Validation: targeted `cargo nextest run -p merman-ascii` filters for the new tests.
- Status: READY
- Review: Tests must use public rendering surfaces and avoid parser/core changes.
- Evidence: `EVIDENCE_AND_GATES.md`

## Decisions Since Opening

- Scope is limited to flowchart subgraph titles in `merman-ascii`.
- `GraphLabel` is the intended shared line-break model.
- Subgraph direction overrides and style/color roles remain separate follow-ons.

## Blockers

- None.

## Next Recommended Action

- Start AFMS-020 by adding parser-backed and direct-model tests that expose the current limitation.
