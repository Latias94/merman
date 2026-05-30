# ASCII Architecture Deepening — Handoff

Status: Active
Last updated: 2026-05-30

## Current State

The lane has been opened to execute five architecture deepening targets for `merman-ascii`:

- shared styled text/cell module,
- graph route planning and painting seam,
- relation graph adapter deepening,
- sequence event plan seam,
- ASCII gap registry.

Implementation has not started yet. AAD-010 is complete and the next task is AAD-020.

## Active Task

- Task ID: AAD-020
- Owner: unassigned
- Files: `crates/merman-ascii/src/canvas.rs`, `crates/merman-ascii/src/text` or equivalent internal module
- Validation: `cargo nextest run -p merman-ascii canvas color`
- Status: NEEDS_CONTEXT
- Review: Pending implementation.
- Evidence: focused tests for trim, padding, role preservation, and ANSI/HTML finalization

## Decisions Since Last Update

- Use one durable workstream instead of reopening closed ASCII workstreams.
- Start with the styled text/cell module because it has the broadest leverage across families.
- Keep fill/background rendering out of this lane unless it becomes a small proof of the styled cell
  substrate.
- AAD-010 passed `git diff --check -- docs/workstreams/ascii-architecture-deepening`.

## Blockers

- None.

## Next Recommended Action

- Commit the workstream docs, then start AAD-020 with a focused migration of existing role-aware
  line buffers.
