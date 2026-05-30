# ASCII Class ER Graph Layout - TODO

Status: Active
Last updated: 2026-05-30

## M0 - Lane Opening

- [x] ACEG-010 [owner=codex] [deps=none] [scope=docs/workstreams/ascii-class-er-graph-layout]
  Goal: Open the follow-on lane for class/ER multi-relationship ASCII graph layout.
  Validation: `git diff --check`
  Review: Confirm this lane is narrower than the closed reference-expansion lane and does not
  overlap active flowchart/text-style workstreams.
  Evidence: `DESIGN.md`, `TODO.md`, `MILESTONES.md`, `EVIDENCE_AND_GATES.md`, `WORKSTREAM.json`,
  `HANDOFF.md`
  Handoff: DONE. ACEG-020 is the first implementation task.

## M1 - Contract Tracer Tests

- [x] ACEG-020 [owner=codex] [deps=ACEG-010] [scope=crates/merman-ascii/tests]
  Goal: Add parser-backed tracer tests for class and ER multi-relationship diagrams that currently
  return explicit unsupported diagnostics.
  Validation: `cargo nextest run -p merman-ascii class`; `cargo nextest run -p merman-ascii er`
  Review: Tests prove public behavior through parser-backed `render_model` helpers and do not add
  layout internals.
  Evidence: `class_parser_multiple_relationships_are_explicitly_unsupported` and
  `er_parser_relationship_layouts_with_unrelated_entities_are_explicitly_unsupported`; gates
  recorded in `EVIDENCE_AND_GATES.md`.
  Handoff: DONE. ACEG-030 implements the shared placement seam.

## M2 - Shared Relationship Layout Boundary

- [x] ACEG-030 [owner=codex] [deps=ACEG-020] [scope=crates/merman-ascii/src]
  Goal: Introduce a small terminal relationship-graph placement boundary and route existing
  single-relationship class/ER outputs through it without broad behavior drift.
  Validation: `cargo nextest run -p merman-ascii class`; `cargo nextest run -p merman-ascii er`;
  `cargo fmt --all --check`
  Review: Shared code must be terminal-layout-only; class/ER semantics stay in their adapters.
  Evidence: `crates/merman-ascii/src/relation_graph.rs` now owns shared terminal box stacking,
  center calculation, and vertical relation-line assembly; existing class/ER snapshots stayed
  stable under focused nextest gates.
  Handoff: DONE. ACEG-040 can add class multi-relationship rendering.

## M3 - Class Multi-Relationship Rendering

- [x] ACEG-040 [owner=codex] [deps=ACEG-030] [scope=crates/merman-ascii/src/class,crates/merman-ascii/tests/class_model.rs]
  Goal: Render useful classDiagram multi-relationship topologies such as chains and stars while
  preserving markers, labels, and explicit diagnostics for unsupported dense graphs.
  Validation: `cargo nextest run -p merman-ascii class`; `cargo clippy -p merman-ascii --all-targets -- -D warnings`
  Review: No silent omission of relations; snapshots must show every supported relation.
  Evidence: `class_parser_extension_star_renders_all_children`,
  `class_parser_extension_chain_renders_each_relationship`, and
  `class_parser_crossing_relationship_layouts_are_explicitly_unsupported`; support docs updated in
  `crates/merman-ascii/README.md`.
  Handoff: DONE. ACEG-050 reuses the boundary for ER.

## M4 - ER Multi-Relationship Rendering

- [ ] ACEG-050 [owner=unassigned] [deps=ACEG-030] [scope=crates/merman-ascii/src/er,crates/merman-ascii/tests/er_model.rs]
  Goal: Render useful erDiagram multi-relationship topologies with cardinality markers,
  identifying/non-identifying line style, and relationship labels.
  Validation: `cargo nextest run -p merman-ascii er`; `cargo clippy -p merman-ascii --all-targets -- -D warnings`
  Review: Cardinality direction must remain typed-model-driven and visually honest.
  Evidence: ER snapshots and support-doc updates.
  Handoff: ACEG-060 runs broad public gates and closes or splits remaining topology gaps.

## M5 - Public Gates And Closeout

- [ ] ACEG-060 [owner=unassigned] [deps=ACEG-040,ACEG-050] [scope=crates/merman-ascii,crates/merman,crates/merman-cli,docs]
  Goal: Verify public APIs and CLI still expose the shipped class/ER graph layouts, update docs,
  and close or split remaining dense-topology work.
  Validation: `cargo nextest run -p merman-ascii`; `cargo nextest run -p merman --features ascii`;
  `cargo nextest run -p merman-cli --features ascii`; `cargo fmt --all --check`; relevant clippy gates.
  Review: Use `review-workstream` and `verify-rust-workstream` before closeout.
  Evidence: `EVIDENCE_AND_GATES.md`, README/support docs, and final handoff.
  Handoff: Lane closes or names narrower follow-ons.
