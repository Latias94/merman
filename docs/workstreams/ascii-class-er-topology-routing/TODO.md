# ASCII Class ER Topology Routing - TODO

Status: Active
Last updated: 2026-05-30

## M0 - Lane Opening

- [x] ACETR-010 [owner=codex] [deps=none] [scope=docs/workstreams/ascii-class-er-topology-routing]
  Goal: Open a narrow topology-routing lane for class/ER crossing support.
  Validation: `git diff --check -- docs/workstreams/ascii-class-er-topology-routing`
  Review: Confirm dense, parallel, cyclic, spanning-level, and unrelated topology support stay out
  of the first slice.
  Evidence: `DESIGN.md`, `TODO.md`, `MILESTONES.md`, `EVIDENCE_AND_GATES.md`, `WORKSTREAM.json`,
  `HANDOFF.md`
  Handoff: DONE. ACETR-020 starts with parser-backed crossing tests.

## M1 - Crossing Contract Tests

- [ ] ACETR-020 [owner=unassigned] [deps=ACETR-010] [scope=crates/merman-ascii/tests]
  Goal: Convert or add parser-backed class and ER crossing tests that describe readable rendered
  output instead of unsupported diagnostics.
  Validation: `cargo nextest run -p merman-ascii class`; `cargo nextest run -p merman-ascii er`
  Review: Tests must assert public output through `render_model`, not planner internals.
  Evidence: class and ER crossing tests fail red before implementation.
  Handoff: Final status must be DONE, DONE_WITH_CONCERNS, BLOCKED, or NEEDS_CONTEXT.

## M2 - Shared Crossing Planner

- [ ] ACETR-030 [owner=unassigned] [deps=ACETR-020] [scope=crates/merman-ascii/src/relation_graph.rs,crates/merman-ascii/src/class,crates/merman-ascii/src/er]
  Goal: Allow adjacent-layer crossing relationships when every edge remains drawable and keep other
  unsupported topology diagnostics unchanged.
  Validation: `cargo nextest run -p merman-ascii class`; `cargo nextest run -p merman-ascii er`;
  `cargo clippy -p merman-ascii --all-targets -- -D warnings`
  Review: Shared planner remains terminal-layout-only; class/ER semantics stay in adapters.
  Evidence: Crossing tests pass; unrelated/cyclic/parallel/spanning diagnostics remain explicit.
  Handoff: Final status must be DONE, DONE_WITH_CONCERNS, BLOCKED, or NEEDS_CONTEXT.

## M3 - Docs And Closeout

- [ ] ACETR-040 [owner=unassigned] [deps=ACETR-030] [scope=crates/merman-ascii,docs]
  Goal: Update support docs, run final gates, and close or split remaining dense topology work.
  Validation: `cargo nextest run -p merman-ascii`; `cargo fmt --all --check`; `git diff --check`
  Review: Use `review-workstream` and `verify-rust-workstream` before closeout.
  Evidence: `EVIDENCE_AND_GATES.md`, README/support docs, and final handoff.
  Handoff: Lane closes or names narrower follow-ons.
