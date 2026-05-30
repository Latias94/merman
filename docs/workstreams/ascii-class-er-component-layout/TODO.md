# ASCII Class ER Component Layout - TODO

Status: Closed
Last updated: 2026-05-30

## M0 - Lane Opening

- [x] ACECL-010 [owner=codex] [deps=none] [scope=docs/workstreams/ascii-class-er-component-layout]
  Goal: Open a narrow lane for disconnected component layout.
  Validation: `git diff --check -- docs/workstreams/ascii-class-er-component-layout`
  Review: Confirm this lane does not add parallel, cyclic, spanning-level, or dense routing.
  Evidence: workstream docs.
  Handoff: DONE. ACECL-020 starts with parser-backed component tests.

## M1 - Component Contract Tests

- [x] ACECL-020 [owner=codex] [deps=ACECL-010] [scope=crates/merman-ascii/tests]
  Goal: Add class and ER parser-backed tests for rendering a related component plus an unrelated
  standalone node.
  Validation: `cargo nextest run -p merman-ascii class`; `cargo nextest run -p merman-ascii er`
  Review: Tests assert public output and fail red before implementation.
  Evidence: class/ER unrelated component tests.
  Handoff: DONE. Parser-backed class and ER tests now assert unrelated standalone components.

## M2 - Shared Component Partition

- [x] ACECL-030 [owner=codex] [deps=ACECL-020] [scope=crates/merman-ascii/src/relation_graph.rs,crates/merman-ascii/src/class,crates/merman-ascii/src/er]
  Goal: Partition relationship-bearing boxes from isolated standalone boxes and render each output
  component through existing class/ER paths.
  Validation: `cargo nextest run -p merman-ascii class`; `cargo nextest run -p merman-ascii er`;
  `cargo clippy -p merman-ascii --all-targets -- -D warnings`
  Review: Component partitioning stays terminal-layout-only; adapters keep semantics.
  Evidence: Component tests pass and existing crossing/chain/star diagnostics remain stable.
  Handoff: DONE. Relation-bearing boxes stay in one layered planner domain; isolated boxes render
  as standalone components.

## M3 - Docs And Closeout

- [x] ACECL-040 [owner=codex] [deps=ACECL-030] [scope=crates/merman-ascii,docs]
  Goal: Update support docs, run final gates, and close or split remaining topology work.
  Validation: `cargo nextest run -p merman-ascii`; `cargo fmt --all --check`; `git diff --check`
  Review: Use `review-workstream` and `verify-rust-workstream` before closeout.
  Evidence: README/support docs and final handoff.
  Handoff: DONE. Support docs now describe unrelated standalone components, full package gates
  pass, and broader topology work remains deferred outside this lane.
