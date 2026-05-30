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

- [x] ACETR-020 [owner=codex] [deps=ACETR-010] [scope=crates/merman-ascii/tests]
  Goal: Convert or add parser-backed class and ER crossing tests that describe readable rendered
  output instead of unsupported diagnostics.
  Validation: `cargo nextest run -p merman-ascii class`; `cargo nextest run -p merman-ascii er`
  Review: Tests must assert public output through `render_model`, not planner internals.
  Evidence: `class_parser_crossing_relationship_layout_reorders_layer_to_render_each_edge` and
  `er_parser_crossing_relationship_layout_reorders_layer_to_render_each_edge` failed red against the
  previous crossing unsupported diagnostics.
  Handoff: DONE. ACETR-030 implements the planner change.

## M2 - Shared Crossing Planner

- [x] ACETR-030 [owner=codex] [deps=ACETR-020] [scope=crates/merman-ascii/src/relation_graph.rs,crates/merman-ascii/src/class,crates/merman-ascii/src/er]
  Goal: Allow adjacent-layer crossing relationships when every edge remains drawable and keep other
  unsupported topology diagnostics unchanged.
  Validation: `cargo nextest run -p merman-ascii class`; `cargo nextest run -p merman-ascii er`;
  `cargo clippy -p merman-ascii --all-targets -- -D warnings`
  Review: Shared planner remains terminal-layout-only; class/ER semantics stay in adapters.
  Evidence: Shared planner now reorders each layer by previous-layer parent order before crossing
  validation. Crossing class and ER tests pass while unrelated diagnostics remain covered by focused
  gates.
  Handoff: DONE. ACETR-040 updates support docs and closes or splits remaining dense topology work.

## M3 - Docs And Closeout

- [ ] ACETR-040 [owner=unassigned] [deps=ACETR-030] [scope=crates/merman-ascii,docs]
  Goal: Update support docs, run final gates, and close or split remaining dense topology work.
  Validation: `cargo nextest run -p merman-ascii`; `cargo fmt --all --check`; `git diff --check`
  Review: Use `review-workstream` and `verify-rust-workstream` before closeout.
  Evidence: `EVIDENCE_AND_GATES.md`, README/support docs, and final handoff.
  Handoff: Lane closes or names narrower follow-ons.
