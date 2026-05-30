# ASCII Class ER Spanning Level Routing - TODO

Status: Active
Last updated: 2026-05-30

## M0 - Lane Opening

- [x] ACESLR-010 [owner=codex] [deps=none] [scope=docs/workstreams/ascii-class-er-spanning-level-routing]
  Goal: Open a narrow lane for class/ER spanning-level relationship routing.
  Validation: `git diff --check -- docs/workstreams/ascii-class-er-spanning-level-routing`
  Review: Confirm this lane does not include cyclic, dense, or flowchart routing.
  Evidence: workstream docs.
  Handoff: DONE. ACESLR-020 starts with parser-backed spanning-level tests.

## M1 - Spanning Contract Tests

- [x] ACESLR-020 [owner=codex] [deps=ACESLR-010] [scope=crates/merman-ascii/tests]
  Goal: Add class and ER parser-backed tests for a relationship that skips over one intermediate
  level.
  Validation: `cargo nextest run -p merman-ascii class`; `cargo nextest run -p merman-ascii er`
  Review: Tests assert public output and fail red on the old spanning-level diagnostics.
  Evidence: class/ER spanning-level tests.
  Handoff: DONE. Parser-backed class and ER tests now assert spanning side-lane output.

## M2 - Side-Lane Spanning Routing

- [x] ACESLR-030 [owner=codex] [deps=ACESLR-020] [scope=crates/merman-ascii/src/relation_graph.rs,crates/merman-ascii/src/class,crates/merman-ascii/src/er]
  Goal: Route simple spanning-level relationships around intermediate boxes.
  Validation: `cargo nextest run -p merman-ascii class`; `cargo nextest run -p merman-ascii er`;
  `cargo clippy -p merman-ascii --all-targets -- -D warnings`
  Review: No relationship may be omitted or routed through an intermediate box.
  Evidence: Spanning tests pass and existing mixed-parallel/crossing behavior stays stable.
  Handoff: DONE. Planner reserves side-lane width and adapters route spanning edges around
  intermediate boxes.

## M3 - Docs And Closeout

- [ ] ACESLR-040 [owner=unassigned] [deps=ACESLR-030] [scope=crates/merman-ascii,docs]
  Goal: Update support docs, run final gates, and close or split remaining topology work.
  Validation: `cargo nextest run -p merman-ascii`; `cargo fmt --all --check`; `git diff --check`
  Review: Use `review-workstream` and `verify-rust-workstream` before closeout.
  Evidence: README/support docs and final handoff.
  Handoff: Lane closes or names narrower follow-ons.
