# ASCII Sequence Renderer Modularization - Handoff

Status: Active
Last updated: 2026-05-29

## Current State

ASRM-020 is implemented. The internal ASCII sequence render model, typed-model adapter,
autonumber handling, lifecycle model validation helpers, and unsupported-feature validation now
live in `sequence/model.rs` and `sequence/validate.rs`. Existing sequence behavior and golden
tests passed after the extraction.

## Active Task

- Task ID: ASRM-030
- Owner: unassigned
- Files:
  - `crates/merman-ascii/src/sequence.rs`
  - `crates/merman-ascii/src/sequence/layout.rs`
- Validation:
  - `cargo fmt --all --check`
  - `cargo nextest run -p merman-ascii sequence`
  - `cargo nextest run -p merman-ascii sequence_golden`
- Status: READY
- Review: ASRM-020 had no blocking findings; ASRM-030 review remains required before completion
- Evidence: update `EVIDENCE_AND_GATES.md` after fresh verification

## Decisions Since Open

- Keep this lane behavior-preserving.
- Keep sequence control blocks out of this lane.
- Start with model and validation extraction because it creates the safest seam before layout and
  rendering are split.
- ASRM-020 kept `sequence.rs` as the facade and re-exported `from_sequence_model` from the new
  model module.

## Blockers

- None.

## Next Recommended Action

Run ASRM-030 as the next bounded no-behavior refactor task after ASRM-020 is reviewed and accepted.
