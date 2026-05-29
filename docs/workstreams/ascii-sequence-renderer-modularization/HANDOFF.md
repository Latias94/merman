# ASCII Sequence Renderer Modularization - Handoff

Status: Closed
Last updated: 2026-05-29

## Current State

ASRM-020, ASRM-030, ASRM-040, ASRM-050, and ASRM-060 are complete. The internal ASCII sequence
render model, typed-model adapter, autonumber handling, lifecycle model validation helpers, and
unsupported-feature validation now live in `sequence/model.rs` and `sequence/validate.rs`.
Participant layout, lifecycle visibility planning, lifecycle edge lookup, and participant-left
geometry now live in `sequence/layout.rs`. Render orchestration, event rows, notes, group-box
overlays, and sequence-local text helpers now live in owner modules. Existing sequence behavior,
golden tests, and the package gate passed after the extractions.

The final module boundary is documented in `DESIGN.md`. Sequence control blocks remain a separate
follow-on lane.

## Active Task

- Task ID: none
- Status: CLOSED

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
- ASRM-050 documented the final boundary and kept sequence control blocks as follow-on scope.

## Blockers

- None.

## Next Recommended Action

Open `ascii-sequence-control-blocks` if Mermaid sequence control-block rendering is the next
priority.
