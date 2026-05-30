# ASCII Class ER Parallel Routing - TODO

Status: Active
Last updated: 2026-05-30

## M0 - Lane Opening

- [x] ACEPR-010 [owner=codex] [deps=none] [scope=docs/workstreams/ascii-class-er-parallel-routing]
  Goal: Open a narrow lane for same-endpoint class/ER parallel relationship routing.
  Validation: `git diff --check -- docs/workstreams/ascii-class-er-parallel-routing`
  Review: Confirm this lane does not include cyclic, spanning-level, dense, or flowchart routing.
  Evidence: workstream docs.
  Handoff: DONE. ACEPR-020 starts with parser-backed parallel relationship tests.

## M1 - Parallel Contract Tests

- [x] ACEPR-020 [owner=codex] [deps=ACEPR-010] [scope=crates/merman-ascii/tests]
  Goal: Add class and ER parser-backed tests for multiple relationships between the same endpoints.
  Validation: `cargo nextest run -p merman-ascii class`; `cargo nextest run -p merman-ascii er`
  Review: Tests assert public output and fail red on the old parallel diagnostics.
  Evidence: class/ER parallel relationship tests.
  Handoff: DONE. Parser-backed class and ER tests now assert same-endpoint parallel lanes.

## M2 - Shared Parallel Lane Helper

- [x] ACEPR-030 [owner=codex] [deps=ACEPR-020] [scope=crates/merman-ascii/src/relation_graph.rs,crates/merman-ascii/src/class,crates/merman-ascii/src/er]
  Goal: Render simple same-endpoint parallel relationships with distinct terminal lanes.
  Validation: `cargo nextest run -p merman-ascii class`; `cargo nextest run -p merman-ascii er`;
  `cargo clippy -p merman-ascii --all-targets -- -D warnings`
  Review: Shared code stays terminal-layout-only; class/ER adapters keep marker, label, line style,
  and cardinality ownership.
  Evidence: Parallel tests pass and existing chain, star, crossing, and component behavior remains
  stable.
  Handoff: DONE. Simple same-endpoint parallel layouts render through shared vertical lane
  formatting while adapters retain semantics.

## M3 - Docs And Closeout

- [ ] ACEPR-040 [owner=unassigned] [deps=ACEPR-030] [scope=crates/merman-ascii,docs]
  Goal: Update support docs, run final gates, and close or split remaining topology work.
  Validation: `cargo nextest run -p merman-ascii`; `cargo fmt --all --check`; `git diff --check`
  Review: Use `review-workstream` and `verify-rust-workstream` before closeout.
  Evidence: README/support docs and final handoff.
  Handoff: Lane closes or names narrower follow-ons.
