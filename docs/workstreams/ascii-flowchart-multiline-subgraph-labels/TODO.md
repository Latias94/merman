# ASCII Flowchart Multiline Subgraph Labels - TODO

Status: Active
Last updated: 2026-05-30

## M0 - Scope And Evidence Freeze

- [x] AFMS-010 [owner=codex] [deps=none] [scope=docs/workstreams/ascii-flowchart-multiline-subgraph-labels]
  Goal: Open a focused workstream for multiline flowchart subgraph titles.
  Validation: `git diff --check -- docs/workstreams/ascii-flowchart-multiline-subgraph-labels`
  Review: Confirm scope stays inside `merman-ascii` flowchart subgraph title layout/drawing.
  Evidence: `DESIGN.md`
  Handoff: AFMS-020 adds failing public tests for parser-backed and direct-model multiline titles.

## M1 - Contract Tests

- [x] AFMS-020 [owner=codex] [deps=AFMS-010] [scope=crates/merman-ascii/tests,crates/merman-ascii/src/lib.rs]
  Goal: Capture multiline subgraph title behavior with public render surfaces.
  Validation: targeted `cargo nextest run -p merman-ascii` filters for the new tests.
  Review: Tests should fail on the current raw single-line title rendering or unsupported diagnostic.
  Evidence: Parser-backed `<br>` title test and direct-model newline title test.
  Handoff: DONE. Parser-backed `<br>` currently renders as raw title text; direct-model newline
  titles currently hit `multiline subgraph labels`.

## M2 - Layout And Drawing

- [ ] AFMS-030 [owner=unassigned] [deps=AFMS-020] [scope=crates/merman-ascii/src/graph,crates/merman-ascii/src/lib.rs,crates/merman-ascii/tests]
  Goal: Render multiline subgraph titles as centered title rows inside the group box.
  Validation: `cargo nextest run -p merman-ascii flowchart`; `cargo nextest run -p merman-ascii graph_fixture`
  Review: Existing single-line subgraph fixtures must stay stable.
  Evidence: Multiline title snapshots and subgraph fixture gate.
  Handoff: AFMS-040 updates support docs and closes or splits remaining title work.

## M3 - Docs And Closeout

- [ ] AFMS-040 [owner=planner] [deps=AFMS-030] [scope=docs/workstreams/ascii-flowchart-multiline-subgraph-labels,crates/merman-ascii/FLOWCHART_SUPPORT.md,crates/merman-ascii/README.md]
  Goal: Update support docs, run final gates, and close or split follow-ons.
  Validation: `cargo nextest run -p merman-ascii`; `cargo fmt --all --check`; `git diff --check`
  Review: Use `review-workstream` and `verify-rust-workstream` before closeout.
  Evidence: Support docs and final handoff.
  Handoff: Lane closes or names narrower follow-ons.
