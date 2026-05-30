# ASCII Class ER Mixed Parallel Routing - TODO

Status: Closed
Last updated: 2026-05-30

## M0 - Lane Opening

- [x] ACEMPR-010 [owner=codex] [deps=none] [scope=docs/workstreams/ascii-class-er-mixed-parallel-routing]
  Goal: Open a narrow lane for mixed class/ER parallel relationship routing.
  Validation: `git diff --check -- docs/workstreams/ascii-class-er-mixed-parallel-routing`
  Review: Confirm this lane does not include cyclic, spanning-level, dense, or flowchart routing.
  Evidence: workstream docs.
  Handoff: DONE. ACEMPR-020 starts with parser-backed mixed-parallel tests.

## M1 - Mixed Parallel Contract Tests

- [x] ACEMPR-020 [owner=codex] [deps=ACEMPR-010] [scope=crates/merman-ascii/tests]
  Goal: Add class and ER parser-backed tests for a parallel endpoint pair plus another edge in the
  same relationship component.
  Validation: `cargo nextest run -p merman-ascii class`; `cargo nextest run -p merman-ascii er`
  Review: Tests assert public output and fail red on the old parallel diagnostics.
  Evidence: class/ER mixed-parallel tests.
  Handoff: DONE. Parser-backed class and ER tests now assert mixed-parallel output.

## M2 - Layered Parallel Lane Offsets

- [x] ACEMPR-030 [owner=codex] [deps=ACEMPR-020] [scope=crates/merman-ascii/src/relation_graph.rs,crates/merman-ascii/src/class,crates/merman-ascii/src/er]
  Goal: Allow duplicate edge pairs in layered planning and offset their rendered lanes.
  Validation: `cargo nextest run -p merman-ascii class`; `cargo nextest run -p merman-ascii er`;
  `cargo clippy -p merman-ascii --all-targets -- -D warnings`
  Review: No relationship may be omitted or overwritten; dense collisions remain follow-ons.
  Evidence: Mixed-parallel tests pass and existing parallel/component/crossing behavior stays
  stable.
  Handoff: DONE. Layered planning accepts duplicate endpoint pairs and drawing offsets duplicate
  lanes.

## M3 - Docs And Closeout

- [x] ACEMPR-040 [owner=codex] [deps=ACEMPR-030] [scope=crates/merman-ascii,docs]
  Goal: Update support docs, run final gates, and close or split remaining topology work.
  Validation: `cargo nextest run -p merman-ascii`; `cargo fmt --all --check`; `git diff --check`
  Review: Use `review-workstream` and `verify-rust-workstream` before closeout.
  Evidence: README/support docs and final handoff.
  Handoff: DONE. Support docs now describe simple mixed-parallel relationship lanes, full package
  gates pass, and broader topology work remains deferred outside this lane.
