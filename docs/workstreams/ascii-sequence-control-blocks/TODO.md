# ASCII Sequence Control Blocks - TODO

Status: Active
Last updated: 2026-05-29

## M0 - Scope And Semantic Inventory

- [x] ASCB-010 [owner=codex] [deps=none] [scope=docs/workstreams/ascii-sequence-control-blocks]
  Goal: Open the control-block lane, record parser/SVG reference points, and define a bounded
  implementation path for the primary Mermaid sequence control-block subset.
  Validation:
  - Workstream docs exist and agree.
  - First executable task has bounded code scope and validation gates.
  Evidence: `DESIGN.md`, `TODO.md`, `WORKSTREAM.json`.
  Handoff: ASCB-020 is the first executable implementation task.

## M1 - Executable Boundary Tests

- [ ] ASCB-020 [owner=codex] [deps=ASCB-010] [scope=crates/merman-ascii/tests/sequence_model.rs,crates/merman-ascii/SEQUENCE_SUPPORT.md,crates/merman-ascii/src/sequence/model.rs]
  Goal: Add typed inventory tests for `loop`, `opt`, `break`, `alt`, `par`, and `critical` inputs
  that prove how `merman-core` represents control signals and how ASCII currently rejects them.
  Validation:
  - `cargo fmt --all --check`
  - `cargo nextest run -p merman-ascii sequence`
  - `git diff --check`
  Review: Confirm the tests freeze the current unsupported boundary without silently accepting
  control blocks.
  Evidence: Focused tests documenting control line types, labels, endpoints, and current diagnostic.
  Handoff: ASCB-030 should add the first real render-plan slice.

## M2 - Single-Section Blocks

- [ ] ASCB-030 [owner=codex] [deps=ASCB-020] [scope=crates/merman-ascii/src/sequence,crates/merman-ascii/tests/sequence_model.rs,crates/merman-ascii/SEQUENCE_SUPPORT.md]
  Goal: Implement control-block collection and text rendering for single-section `loop`, `opt`, and
  `break` frames around contained message/note rows.
  Validation:
  - `cargo fmt --all --check`
  - `cargo nextest run -p merman-ascii sequence`
  - `cargo nextest run -p merman-ascii sequence_golden`
  - `cargo nextest run -p merman-ascii`
  Review: Verify non-control sequence snapshots are unchanged except intentional new assertions.
  Evidence: New passing render tests for the three single-section block forms.
  Handoff: ASCB-040 should extend the same plan to sectioned blocks.

## M3 - Sectioned Blocks

- [ ] ASCB-040 [owner=codex] [deps=ASCB-030] [scope=crates/merman-ascii/src/sequence,crates/merman-ascii/tests/sequence_model.rs,crates/merman-ascii/SEQUENCE_SUPPORT.md]
  Goal: Implement `alt`/`else`, `par`/`and`, and `critical`/`option` sectioned frames with labels
  and deterministic separators.
  Validation:
  - `cargo fmt --all --check`
  - `cargo nextest run -p merman-ascii sequence`
  - `cargo nextest run -p merman-ascii sequence_golden`
  - `cargo nextest run -p merman-ascii`
  Review: Check empty sections, multi-section labels, and notes/messages inside sections.
  Evidence: Passing render tests for all sectioned block forms.
  Handoff: ASCB-050 should settle nesting and edge-case policy.

## M4 - Nesting And Edge Cases

- [ ] ASCB-050 [owner=codex] [deps=ASCB-040] [scope=crates/merman-ascii/src/sequence,crates/merman-ascii/tests/sequence_model.rs,crates/merman-ascii/SEQUENCE_SUPPORT.md]
  Goal: Decide and implement the supported policy for nested blocks, empty blocks, lifecycle events,
  created/destroyed actors, and participant boxes inside control-block spans.
  Validation:
  - `cargo fmt --all --check`
  - `cargo nextest run -p merman-ascii sequence`
  - `cargo nextest run -p merman-ascii sequence_golden`
  - `cargo nextest run -p merman-ascii`
  Review: Unsupported edge cases must be explicit diagnostics; supported edge cases must have
  regression tests.
  Evidence: Tests and docs for final supported/deferred edge-case policy.
  Handoff: ASCB-060 should package examples and close the lane.

## M5 - Examples, Verification, And Closeout

- [ ] ASCB-060 [owner=codex] [deps=ASCB-050] [scope=docs/workstreams/ascii-sequence-control-blocks,crates/merman-ascii,README.md]
  Goal: Generate manual example outputs, run final gates, update docs, and close or split remaining
  control-block parity work.
  Validation:
  - `cargo fmt --all --check`
  - `cargo nextest run -p merman-ascii`
  - `cargo nextest run -p merman --features ascii`
  - `cargo nextest run -p merman-cli --features ascii`
  - `cargo clippy -p merman-ascii -p merman --features ascii --all-targets -- -D warnings`
  - `git diff --check`
  Review: Run `review-workstream` and `verify-rust-workstream` before closeout.
  Evidence: Final gate log, example `.txt` outputs for manual inspection, and updated support docs.
  Handoff: Lane closes if the primary subset is complete; otherwise split the remaining scope.
