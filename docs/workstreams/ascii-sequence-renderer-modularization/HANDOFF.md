# ASCII Sequence Renderer Modularization - Handoff

Status: Active
Last updated: 2026-05-29

## Current State

ASRM-020 and ASRM-030 are implemented. The internal ASCII sequence render model, typed-model
adapter, autonumber handling, lifecycle model validation helpers, and unsupported-feature
validation now live in `sequence/model.rs` and `sequence/validate.rs`. Participant layout,
lifecycle visibility planning, lifecycle edge lookup, and participant-left geometry now live in
`sequence/layout.rs`. Existing sequence behavior and golden tests passed after both extractions.

## Active Task

- Task ID: ASRM-040
- Owner: unassigned
- Files:
  - `crates/merman-ascii/src/sequence.rs`
  - `crates/merman-ascii/src/sequence/render.rs`
  - `crates/merman-ascii/src/sequence/events.rs`
  - `crates/merman-ascii/src/sequence/notes.rs`
  - `crates/merman-ascii/src/sequence/boxes.rs`
  - `crates/merman-ascii/src/sequence/text.rs`
- Validation:
  - `cargo fmt --all --check`
  - `cargo nextest run -p merman-ascii sequence`
  - `cargo nextest run -p merman-ascii sequence_golden`
- Status: READY
- Review: ASRM-020 and ASRM-030 had no blocking findings; ASRM-040 review remains required before completion
- Evidence: update `EVIDENCE_AND_GATES.md` after fresh verification

## Decisions Since Open

- Keep this lane behavior-preserving.
- Keep sequence control blocks out of this lane.
- Start with model and validation extraction because it creates the safest seam before layout and
  rendering are split.
- ASRM-020 kept `sequence.rs` as the facade and re-exported `from_sequence_model` from the new
  model module.
- ASRM-030 kept row rendering in the facade while moving layout and lifecycle visibility helpers
  into `sequence/layout.rs`.

## Blockers

- None.

## Next Recommended Action

Run ASRM-040 as the next bounded no-behavior refactor task after ASRM-030 is reviewed and accepted.
