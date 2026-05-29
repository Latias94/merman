# ASCII Sequence Parity - TODO

Status: Closed
Last updated: 2026-05-29

## M0 - Scope And Gap Ledger

- [x] ASP-010 [owner=codex] [deps=none] [scope=docs/workstreams/ascii-sequence-parity,crates/merman-ascii/tests/testdata/mermaid-ascii]
  Goal: Open the sequence parity lane, record copied upstream fixture status, and name the first
  product gap slice.
  Validation:
  - Workstream docs exist and agree.
  - `SEQUENCE_FIXTURE_GAPS.md` records copied upstream fixture status.
  Evidence: `DESIGN.md`, `SEQUENCE_FIXTURE_GAPS.md`.
  Handoff: ASP-020 is next.

## M1 - Open Arrow Messages

- [x] ASP-020 [owner=codex] [deps=ASP-010] [scope=crates/merman-ascii/src/sequence.rs,crates/merman-ascii/tests/sequence_model.rs,crates/merman-ascii/SEQUENCE_SUPPORT.md]
  Goal: Render Mermaid sequence open-arrow message types `->` and `-->` from the typed sequence
  model instead of rejecting them as unsupported message types.
  Validation:
  - `cargo fmt --all --check`
  - `cargo nextest run -p merman-ascii sequence`
  - `cargo nextest run -p merman-ascii sequence_golden`
  Evidence: public behavior tests and support matrix updates.
  Handoff: ASP-030 or closeout depending on remaining goal budget.

## M2 - Rich Sequence Constructs Inventory

- [x] ASP-030 [owner=codex] [deps=ASP-020] [scope=crates/merman-ascii/SEQUENCE_SUPPORT.md,docs/workstreams/ascii-sequence-parity]
  Goal: Split the next rich sequence construct lane with notes, boxes, activations, and
  create/destroy ordered by parser/model readiness and rendering risk.
  Validation:
  - Support matrix names exact unsupported features.
  - Follow-on tasks are independently executable.
  Evidence: `SEQUENCE_SUPPORT.md`, `HANDOFF.md`.
  Handoff: ASP-050 is next.

## M3 - Notes Rendering

- [x] ASP-050 [owner=codex] [deps=ASP-030] [scope=crates/merman-ascii/src/sequence.rs,crates/merman-ascii/tests/sequence_model.rs,crates/merman-ascii/SEQUENCE_SUPPORT.md]
  Goal: Render single-line typed sequence notes for `Note right of`, `Note left of`, and `Note
  over` without changing copied upstream fixture output.
  Validation:
  - `cargo fmt --all --check`
  - `cargo nextest run -p merman-ascii sequence`
  - `cargo nextest run -p merman-ascii sequence_golden`
  Evidence: public behavior tests and support matrix updates.
  Handoff: ASP-060 or closeout depending on remaining goal budget.

## M4 - Remaining Rich Constructs

- [x] ASP-060 [owner=codex] [deps=ASP-050] [scope=crates/merman-ascii/src/sequence.rs,crates/merman-ascii/tests/sequence_model.rs,crates/merman-ascii/SEQUENCE_SUPPORT.md]
  Goal: Decide and implement the sequence boxes slice, or split it if group bounds require a larger
  layout refactor.
  Validation:
  - Focused sequence tests prove boxes or document why they remain unsupported.
  Evidence: Support matrix and tests.
  Handoff: ASP-070 is next.

- [x] ASP-070 [owner=codex] [deps=ASP-050] [scope=crates/merman-ascii/src/sequence.rs,crates/merman-ascii/tests/sequence_model.rs,crates/merman-ascii/SEQUENCE_SUPPORT.md]
  Goal: Render activation state and split create/destroy lifecycle behavior if it needs a deeper
  actor-lifetime model.
  Validation:
  - Focused sequence tests prove activation state.
  - Create/destroy follow-on names exact renderer state needed.
  Evidence: Support matrix and tests.
  Handoff: ASP-075 is next.

- [x] ASP-075 [owner=codex] [deps=ASP-070] [scope=crates/merman-ascii/src/sequence.rs,crates/merman-ascii/tests/sequence_model.rs,crates/merman-ascii/SEQUENCE_SUPPORT.md]
  Goal: Render actor create/destroy lifecycle from typed created/destroyed actor indices.
  Validation:
  - Created participants are hidden from the initial header, render at their creating message, and
    then keep a lifeline.
  - Destroyed participants render a termination marker on the destroying message and stop their
    lifeline afterward.
  - Hand-built lifecycle maps with invalid actors, indices, endpoint bindings, or visibility order
    return explicit unsupported-feature errors.
  Evidence: behavior tests and support matrix updates.
  Handoff: ASP-080 is next.

- [x] ASP-080 [owner=codex] [deps=ASP-050] [scope=crates/merman-ascii/src/sequence.rs,crates/merman-ascii/tests/sequence_model.rs,crates/merman-ascii/SEQUENCE_SUPPORT.md]
  Goal: Support wrapped messages and notes while preserving explicit unsupported boundaries for
  wrapped actor labels and wrapped boxes.
  Validation:
  - Wrapped message labels render as multiple display-width-bounded rows.
  - Wrapped notes render as taller note boxes with display-width-bounded text rows.
  - CJK text without spaces wraps by display width instead of staying on one long row.
  - Support matrix distinguishes supported message/note wrapping from unsupported actor/box
    wrapping.
  Evidence: behavior tests and `SEQUENCE_SUPPORT.md`.
  Handoff: Closeout or next implementation task.

## M5 - Verification And Commit

- [x] ASP-040 [owner=codex] [deps=ASP-020] [scope=docs/workstreams/ascii-sequence-parity,CHANGELOG.md]
  Goal: Run focused gates, update evidence, and commit the initial sequence parity slice.
  Validation:
  - `cargo nextest run -p merman-ascii`
  - `cargo nextest run -p merman --features ascii`
  - `cargo nextest run -p merman-cli --features ascii`
  - `cargo clippy -p merman-ascii -p merman --features ascii --all-targets -- -D warnings`
  - `git diff --check`
  Evidence: `EVIDENCE_AND_GATES.md`.
  Handoff: Lane remains active for ASP-030 or closes if follow-on is split.

- [x] ASP-090 [owner=codex] [deps=ASP-050] [scope=docs/workstreams/ascii-sequence-parity,CHANGELOG.md]
  Goal: Run focused and broad gates, update evidence, and commit the notes rendering slice.
  Validation:
  - `cargo nextest run -p merman-ascii`
  - `cargo nextest run -p merman --features ascii`
  - `cargo nextest run -p merman-cli --features ascii`
  - `cargo clippy -p merman-ascii -p merman --features ascii --all-targets -- -D warnings`
  - `git diff --check`
  Evidence: `EVIDENCE_AND_GATES.md`.
  Handoff: ASP-060 remains next unless this lane closes.

- [x] ASP-100 [owner=codex] [deps=ASP-060] [scope=docs/workstreams/ascii-sequence-parity,CHANGELOG.md]
  Goal: Run focused and broad gates, update evidence, and commit the sequence boxes slice.
  Validation:
  - `cargo nextest run -p merman-ascii`
  - `cargo nextest run -p merman --features ascii`
  - `cargo nextest run -p merman-cli --features ascii`
  - `cargo clippy -p merman-ascii -p merman --features ascii --all-targets -- -D warnings`
  - `git diff --check`
  Evidence: `EVIDENCE_AND_GATES.md`.
  Handoff: ASP-070 remains next unless this lane closes.

- [x] ASP-110 [owner=codex] [deps=ASP-070] [scope=docs/workstreams/ascii-sequence-parity,CHANGELOG.md]
  Goal: Run focused and broad gates, update evidence, and commit the activation rendering slice.
  Validation:
  - `cargo nextest run -p merman-ascii`
  - `cargo nextest run -p merman --features ascii`
  - `cargo nextest run -p merman-cli --features ascii`
  - `cargo clippy -p merman-ascii -p merman --features ascii --all-targets -- -D warnings`
  - `git diff --check`
  Evidence: `EVIDENCE_AND_GATES.md`.
  Handoff: ASP-075 remains next unless this lane closes.

- [x] ASP-120 [owner=codex] [deps=ASP-075] [scope=docs/workstreams/ascii-sequence-parity,CHANGELOG.md]
  Goal: Run focused and broad gates, update evidence, and commit the actor lifecycle rendering
  slice.
  Validation:
  - `cargo nextest run -p merman-ascii`
  - `cargo nextest run -p merman --features ascii`
  - `cargo nextest run -p merman-cli --features ascii`
  - `cargo clippy -p merman-ascii -p merman --features ascii --all-targets -- -D warnings`
  - `git diff --check`
  Evidence: `EVIDENCE_AND_GATES.md`.
  Handoff: ASP-080 remains next unless this lane closes.

- [x] ASP-130 [owner=codex] [deps=ASP-080] [scope=docs/workstreams/ascii-sequence-parity,CHANGELOG.md]
  Goal: Run focused and broad gates, update evidence, and commit the wrapping rendering slice.
  Validation:
  - `cargo nextest run -p merman-ascii`
  - `cargo nextest run -p merman --features ascii`
  - `cargo nextest run -p merman-cli --features ascii`
  - `cargo clippy -p merman-ascii -p merman --features ascii --all-targets -- -D warnings`
  - `git diff --check`
  Evidence: `EVIDENCE_AND_GATES.md`.
  Handoff: Sequence parity lane is ready for closeout review.

## M6 - Closeout Review

- [x] ASP-140 [owner=codex] [deps=ASP-130] [scope=docs/workstreams/ascii-sequence-parity,crates/merman-ascii/SEQUENCE_SUPPORT.md]
  Goal: Close the sequence parity lane and split richer Mermaid sequence control blocks into a new
  workstream boundary.
  Validation:
  - Workstream docs mark the lane closed.
  - Support matrix names the remaining unsupported boundary.
  - Fresh closeout gates pass.
  Evidence: `DESIGN.md`, `MILESTONES.md`, `EVIDENCE_AND_GATES.md`, `WORKSTREAM.json`,
  `HANDOFF.md`, and `SEQUENCE_SUPPORT.md`.
  Handoff: Open a new lane for `loop`/`alt`/`opt`/`par`/`critical`/`break` rendering if that is the
  next product priority.
