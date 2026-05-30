# ASCII Flowchart Direction Transform - Handoff

Status: Active
Last updated: 2026-05-30

## Current State

The flowchart ASCII support matrix still rejects BT and RL root directions. Parser-backed BT/RL
contract tests now describe the intended output and fail red on the current unsupported-direction
diagnostic. Existing LR and TD support is stable.

## Active Task

- Task ID: AFDT-030
- Owner: unassigned
- Files:
  - `crates/merman-ascii/src/graph`
  - `crates/merman-ascii/src/lib.rs`
  - `crates/merman-ascii/tests/flowchart_model.rs`
- Validation: `cargo nextest run -p merman-ascii flowchart`; `cargo clippy -p merman-ascii --all-targets -- -D warnings`
- Status: READY
- Review: BT and RL must render honestly without changing LR/TD output or moving direction logic
  into `merman-core`.
- Evidence: `EVIDENCE_AND_GATES.md`

## Decisions Since Last Update

- BT and RL are the only root directions in scope.
- Root-direction transforms belong in the ASCII flowchart rendering path, not in `merman-core` or
  the parser.
- Subgraph direction overrides, color/style roles, and state diagrams remain out of scope.
- AFDT-020 added expected ASCII output for `flowchart BT\nA --> B` and `flowchart RL\nA --> B`.
- `flowchart_parser_unsupported_direction_is_explicit` still uses BT and must be adjusted once
  BT/RL become supported.

## Blockers

- None.

## Next Recommended Action

- Start AFDT-030 by making the BT/RL tests green and updating the old unsupported-direction test.
