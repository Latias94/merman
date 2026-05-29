# ASCII Sequence Parity - TODO

Status: Active
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

- [ ] ASP-030 [owner=codex] [deps=ASP-020] [scope=crates/merman-ascii/SEQUENCE_SUPPORT.md,docs/workstreams/ascii-sequence-parity]
  Goal: Split the next rich sequence construct lane with notes, boxes, activations, and
  create/destroy ordered by parser/model readiness and rendering risk.
  Validation:
  - Support matrix names exact unsupported features.
  - Follow-on tasks are independently executable.
  Evidence: `SEQUENCE_SUPPORT.md`, `HANDOFF.md`.
  Handoff: Follow-on workstream or task.

## M3 - Verification And Commit

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
