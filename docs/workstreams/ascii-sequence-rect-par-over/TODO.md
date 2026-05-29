# ASCII Sequence Rect And ParOver Blocks - TODO

Status: Active
Last updated: 2026-05-29

## M0 - Scope And Evidence Freeze

- [x] ASRP-010 [owner=codex] [deps=none] [scope=docs/workstreams/ascii-sequence-rect-par-over]
  Goal: Open the `rect` / `par_over` follow-on lane with source-of-truth line types, product
  boundary, non-goals, and validation gates.
  Validation:
  - Workstream docs exist and agree.
  - First executable task is bounded.
  Evidence: `DESIGN.md`, `TODO.md`, `WORKSTREAM.json`.
  Handoff: ASRP-020 should freeze current parser/model behavior before rendering changes.

## M1 - Executable Boundary Tests

- [ ] ASRP-020 [owner=codex] [deps=ASRP-010] [scope=crates/merman-ascii/tests/sequence_model.rs,crates/merman-ascii/SEQUENCE_SUPPORT.md]
  Goal: Add tests proving `rect` and `par_over` core control-signal line types, labels, and current
  ASCII unsupported diagnostics.
  Validation:
  - `cargo fmt --all --check`
  - `cargo nextest run -p merman-ascii sequence_rect_par_over`
  - `git diff --check`
  Review: Confirm the tests freeze behavior through public parser/render APIs and do not assume
  implementation internals.
  Evidence: Pending.
  Handoff: ASRP-030 should implement `rect` frame rendering.

## M2 - Rect Region Frames

- [ ] ASRP-030 [owner=codex] [deps=ASRP-020] [scope=crates/merman-ascii/src/sequence,crates/merman-ascii/tests/sequence_model.rs,crates/merman-ascii/SEQUENCE_SUPPORT.md]
  Goal: Render `rect <style>` as a single-section region frame that preserves contained rows and
  the source style expression as text.
  Validation:
  - `cargo fmt --all --check`
  - `cargo nextest run -p merman-ascii sequence_rect`
  - `cargo nextest run -p merman-ascii sequence_golden`
  - `git diff --check`
  Review: Check that terminal output preserves region semantics without introducing ANSI styling.
  Evidence: Pending.
  Handoff: ASRP-040 should add `par_over` without regressing `par`.

## M3 - ParOver Frames

- [ ] ASRP-040 [owner=codex] [deps=ASRP-030] [scope=crates/merman-ascii/src/sequence,crates/merman-ascii/tests/sequence_model.rs,crates/merman-ascii/SEQUENCE_SUPPORT.md]
  Goal: Render `par_over <label>` as a single-section frame while preserving existing `par` and
  `par`/`and` section behavior.
  Validation:
  - `cargo fmt --all --check`
  - `cargo nextest run -p merman-ascii sequence_par_over`
  - `cargo nextest run -p merman-ascii sequence`
  - `git diff --check`
  Review: Confirm asymmetric `par_over` start plus `par` end is handled intentionally.
  Evidence: Pending.
  Handoff: ASRP-050 should cover combinations and remaining diagnostics.

## M4 - Combinations And Edge Policy

- [ ] ASRP-050 [owner=codex] [deps=ASRP-040] [scope=crates/merman-ascii/src/sequence,crates/merman-ascii/tests/sequence_model.rs,crates/merman-ascii/SEQUENCE_SUPPORT.md]
  Goal: Cover notes, activations, create/destroy lifecycle rows, participant boxes, empty sections,
  nested blocks, and malformed hand-built ordering for `rect` and `par_over`.
  Validation:
  - `cargo fmt --all --check`
  - `cargo nextest run -p merman-ascii sequence_rect_par_over sequence_control_blocks`
  - `cargo nextest run -p merman-ascii`
  - `git diff --check`
  Review: Supported combinations must be rendered; unsupported edge cases must stay explicit.
  Evidence: Pending.
  Handoff: ASRP-060 should package examples and close or split the lane.

## M5 - Examples, Verification, And Closeout

- [ ] ASRP-060 [owner=codex] [deps=ASRP-050] [scope=docs/workstreams/ascii-sequence-rect-par-over,README.md,crates/merman-ascii/SEQUENCE_SUPPORT.md]
  Goal: Generate manual example outputs, run final gates, update docs, and close or split remaining
  parity work.
  Validation:
  - `cargo fmt --all --check`
  - `cargo nextest run -p merman-ascii`
  - `cargo nextest run -p merman --features ascii`
  - `cargo nextest run -p merman-cli --features ascii`
  - `cargo clippy -p merman-ascii -p merman --features ascii --all-targets -- -D warnings`
  - `git diff --check`
  Review: Run `review-workstream` and `verify-rust-workstream` before closeout.
  Evidence: Pending.
  Handoff: Lane closes if `rect` and `par_over` are shipped; otherwise split the unfinished form.
