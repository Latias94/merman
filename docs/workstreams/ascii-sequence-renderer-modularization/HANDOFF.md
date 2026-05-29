# ASCII Sequence Renderer Modularization - Handoff

Status: Active
Last updated: 2026-05-29

## Current State

The workstream is opened. No renderer code has been changed in this lane yet. Existing uncommitted
`ascii-sequence-parity` closeout documentation changes are intentionally preserved and should not be
mixed into a refactor commit without explicit user confirmation.

## Active Task

- Task ID: ASRM-020
- Owner: unassigned
- Files:
  - `crates/merman-ascii/src/sequence.rs`
  - `crates/merman-ascii/src/sequence/model.rs`
  - `crates/merman-ascii/src/sequence/validate.rs`
- Validation:
  - `cargo fmt --all --check`
  - `cargo nextest run -p merman-ascii sequence`
  - `cargo nextest run -p merman-ascii sequence_golden`
- Status: READY
- Review: required before completion
- Evidence: update `EVIDENCE_AND_GATES.md` after fresh verification

## Decisions Since Open

- Keep this lane behavior-preserving.
- Keep sequence control blocks out of this lane.
- Start with model and validation extraction because it creates the safest seam before layout and
  rendering are split.

## Blockers

- None for planning.
- Implementation commits should wait until the existing closeout docs are either committed
  separately or intentionally included by explicit user confirmation.

## Next Recommended Action

Run ASRM-020 as a bounded no-behavior refactor task.
