# ASCII Flowchart Direction Transform - Handoff

Status: Active
Last updated: 2026-05-30

## Current State

The flowchart ASCII support matrix still rejects BT and RL root directions. Existing LR and TD
support is stable.

## Active Task

- Task ID: AFDT-020
- Owner: unassigned
- Files:
  - `crates/merman-ascii/tests/flowchart_model.rs`
- Validation: `cargo nextest run -p merman-ascii flowchart`
- Status: READY
- Review: Parser-backed tests must exercise public `render_model` output and fail red on the
  current unsupported-direction diagnostic.
- Evidence: `EVIDENCE_AND_GATES.md`

## Decisions Since Last Update

- BT and RL are the only root directions in scope.
- Root-direction transforms belong in the ASCII flowchart rendering path, not in `merman-core` or
  the parser.
- Subgraph direction overrides, color/style roles, and state diagrams remain out of scope.

## Blockers

- None.

## Next Recommended Action

- Start AFDT-020 with parser-backed BT and RL tests.
