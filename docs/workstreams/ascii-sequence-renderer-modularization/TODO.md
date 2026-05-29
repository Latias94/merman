# ASCII Sequence Renderer Modularization - TODO

Status: Active
Last updated: 2026-05-29

## M0 - Scope And Evidence Freeze

- [x] ASRM-010 [owner=codex] [deps=none] [scope=docs/workstreams/ascii-sequence-renderer-modularization]
  Goal: Open the modularization lane, name the target module boundary, and select the first
  no-behavior extraction task.
  Validation:
  - Workstream docs exist and agree.
  - First task has bounded file scope and validation gates.
  Evidence: `DESIGN.md`, `TODO.md`, `WORKSTREAM.json`.
  Handoff: ASRM-020 is the first executable implementation task.

## M1 - Model And Validation Boundary

- [x] ASRM-020 [owner=codex] [deps=ASRM-010] [scope=crates/merman-ascii/src/sequence.rs,crates/merman-ascii/src/sequence/model.rs,crates/merman-ascii/src/sequence/validate.rs]
  Goal: Extract the internal sequence render model, typed-model adapter, and unsupported-feature
  validation from `sequence.rs` with no behavior or public API change.
  Validation:
  - `cargo fmt --all --check`
  - `cargo nextest run -p merman-ascii sequence`
  - `cargo nextest run -p merman-ascii sequence_golden`
  Review: `review-workstream` before accepting completion.
  Evidence: `sequence/model.rs`, `sequence/validate.rs`, and focused sequence gates.
  Handoff: ASRM-030 is next; output and unsupported-feature diagnostics remained stable under the
  focused gates.

## M2 - Layout And Rendering Boundaries

- [ ] ASRM-030 [owner=unassigned] [deps=ASRM-020] [scope=crates/merman-ascii/src/sequence.rs,crates/merman-ascii/src/sequence/layout.rs]
  Goal: Extract participant layout, lifecycle visibility, and lifeline geometry into a layout
  module with no behavior change.
  Validation:
  - `cargo fmt --all --check`
  - `cargo nextest run -p merman-ascii sequence`
  - `cargo nextest run -p merman-ascii sequence_golden`
  Review: `review-workstream` before accepting completion.
  Evidence: module diff and sequence golden gates.
  Handoff: ASRM-040 is next.

- [ ] ASRM-040 [owner=unassigned] [deps=ASRM-030] [scope=crates/merman-ascii/src/sequence.rs,crates/merman-ascii/src/sequence/render.rs,crates/merman-ascii/src/sequence/events.rs,crates/merman-ascii/src/sequence/notes.rs,crates/merman-ascii/src/sequence/boxes.rs,crates/merman-ascii/src/sequence/text.rs]
  Goal: Extract row rendering, note rendering, group-box overlays, and sequence-local text helpers
  into owner modules with no intended output change.
  Validation:
  - `cargo fmt --all --check`
  - `cargo nextest run -p merman-ascii sequence`
  - `cargo nextest run -p merman-ascii sequence_golden`
  - `cargo nextest run -p merman-ascii`
  Review: `review-workstream` before accepting completion.
  Evidence: module diff and package gate.
  Handoff: ASRM-050 is next.

## M3 - Control-Block Readiness

- [ ] ASRM-050 [owner=unassigned] [deps=ASRM-040] [scope=docs/workstreams/ascii-sequence-renderer-modularization,crates/merman-ascii/src/sequence.rs,crates/merman-ascii/src/sequence]
  Goal: Document the final module boundary and confirm that sequence control blocks remain a
  separate follow-on lane.
  Validation:
  - `cargo nextest run -p merman-ascii`
  - `git diff --check`
  Evidence: `EVIDENCE_AND_GATES.md`, `HANDOFF.md`.
  Handoff: Open `ascii-sequence-control-blocks` after this lane closes if control-block rendering
  is the next priority.

## M4 - Closeout

- [ ] ASRM-060 [owner=planner] [deps=ASRM-050] [scope=docs/workstreams/ascii-sequence-renderer-modularization]
  Goal: Close the modularization lane or split any remaining extraction debt into a smaller
  follow-on.
  Validation:
  - `verify-rust-workstream` records fresh final gate evidence.
  - `review-workstream` has no blocking findings.
  Evidence: `EVIDENCE_AND_GATES.md`, `WORKSTREAM.json`, `HANDOFF.md`.
  Handoff: Next lane should be `ascii-sequence-control-blocks`, not more opportunistic edits inside
  this workstream.
