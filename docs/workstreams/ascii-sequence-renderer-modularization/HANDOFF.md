# ASCII Sequence Renderer Modularization - Handoff

Status: Active
Last updated: 2026-05-29

## Current State

ASRM-020, ASRM-030, and ASRM-040 are implemented. The internal ASCII sequence render model,
typed-model adapter, autonumber handling, lifecycle model validation helpers, and
unsupported-feature validation now live in `sequence/model.rs` and `sequence/validate.rs`.
Participant layout, lifecycle visibility planning, lifecycle edge lookup, and participant-left
geometry now live in `sequence/layout.rs`. Render orchestration, event rows, notes, group-box
overlays, and sequence-local text helpers now live in owner modules. Existing sequence behavior,
golden tests, and the package gate passed after the extractions.

## Active Task

- Task ID: ASRM-050
- Owner: unassigned
- Files:
  - `docs/workstreams/ascii-sequence-renderer-modularization`
  - `crates/merman-ascii/src/sequence.rs`
  - `crates/merman-ascii/src/sequence`
- Validation:
  - `cargo nextest run -p merman-ascii`
  - `git diff --check`
- Status: READY
- Review: ASRM-020, ASRM-030, and ASRM-040 had no blocking findings; ASRM-050 review remains required before completion
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
- ASRM-040 turned `sequence.rs` into a facade and split rendering responsibilities into owner
  modules without adding control-block behavior.

## Blockers

- None.

## Next Recommended Action

Run ASRM-050 to document the final module boundary and confirm that sequence control blocks remain
a separate follow-on lane.
