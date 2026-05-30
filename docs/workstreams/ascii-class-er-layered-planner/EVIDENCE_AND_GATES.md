# ASCII Class ER Layered Planner - Evidence And Gates

Status: Active
Last updated: 2026-05-30

## Smallest Current Repro

Class and ER have shipped layered chain/star rendering, but each adapter owns a duplicated planner.
The behavior to preserve is public parser-backed rendering through `merman-ascii`:

- `class_parser_extension_star_renders_all_children`
- `class_parser_extension_chain_renders_each_relationship`
- `class_parser_crossing_relationship_layouts_are_explicitly_unsupported`
- `er_parser_relationship_chain_renders_each_cardinality_and_label`
- `er_parser_relationship_star_renders_each_label_and_leaf_cardinality`
- `er_parser_crossing_relationship_layouts_are_explicitly_unsupported`

## Gate Set

### Focused Iteration

```bash
cargo nextest run -p merman-ascii class
cargo nextest run -p merman-ascii er
```

### Package And Hygiene

```bash
cargo nextest run -p merman-ascii
cargo clippy -p merman-ascii --all-targets -- -D warnings
cargo fmt --all --check
git diff --check
```

## Evidence Anchors

- `docs/workstreams/ascii-class-er-layered-planner/DESIGN.md`
- `docs/workstreams/ascii-class-er-layered-planner/TODO.md`
- `docs/workstreams/ascii-class-er-layered-planner/MILESTONES.md`
- `docs/workstreams/ascii-class-er-graph-layout/`
- `crates/merman-ascii/src/relation_graph.rs`
- `crates/merman-ascii/src/class/render.rs`
- `crates/merman-ascii/src/er/render.rs`
- `crates/merman-ascii/tests/class_model.rs`
- `crates/merman-ascii/tests/er_model.rs`

## Evidence Log

| Date | Task | Evidence | Result |
| --- | --- | --- | --- |
| 2026-05-30 | ACELP-010 | Opened follow-on lane from `ascii-class-er-graph-layout` closeout. | Scope is limited to behavior-preserving shared layered planner extraction. |
| 2026-05-30 | ACELP-020 | Added `relation_graph::plan_layered_relation_boxes` and routed classDiagram layered layout through it. | Class semantics and diagnostics remain in `class/render.rs`; class chain/star/crossing public tests stayed green. |
| 2026-05-30 | ACELP-030 | Routed erDiagram layered layout through `relation_graph::plan_layered_relation_boxes`. | ER cardinality, label, line-style, and diagnostics remain in `er/render.rs`; ER and class public tests stayed green. |

## Verification Log

| Date | Task | Command | Scope | Result | Proves |
| --- | --- | --- | --- | --- | --- |
| 2026-05-30 | ACELP-010 | `git diff --check -- docs/workstreams/ascii-class-er-layered-planner` | Workstream opening docs | PASS | Opening docs have no whitespace errors. |
| 2026-05-30 | ACELP-020 | `cargo nextest run -p merman-ascii class` | Focused class ASCII tests | PASS, 14 tests | Class layered rendering consumes the shared planner without public output drift. |
| 2026-05-30 | ACELP-020 | `cargo clippy -p merman-ascii --all-targets -- -D warnings` | merman-ascii lint gate | PASS | Shared planner and class adapter extraction are warning-free. |
| 2026-05-30 | ACELP-020 | `cargo fmt --all --check` | Workspace formatting check | PASS | Refactor formatting is stable. |
| 2026-05-30 | ACELP-020 | `git diff --check -- crates/merman-ascii/src/class/render.rs crates/merman-ascii/src/relation_graph.rs docs/workstreams/ascii-class-er-layered-planner` | Scoped whitespace hygiene | PASS | ACELP-020 files have no whitespace errors. |
| 2026-05-30 | ACELP-030 | `cargo nextest run -p merman-ascii er` | Focused ER/filter gate | PASS, 81 tests | ER layered rendering consumes the shared planner without public output drift. |
| 2026-05-30 | ACELP-030 | `cargo nextest run -p merman-ascii class` | Focused class regression gate | PASS, 14 tests | Class behavior remains stable after ER joins the shared planner. |
| 2026-05-30 | ACELP-030 | `cargo clippy -p merman-ascii --all-targets -- -D warnings` | merman-ascii lint gate | PASS | ER adapter extraction is warning-free. |
| 2026-05-30 | ACELP-030 | `cargo fmt --all --check` | Workspace formatting check | PASS | Refactor formatting is stable. |
| 2026-05-30 | ACELP-030 | `git diff --check -- crates/merman-ascii/src/er/render.rs crates/merman-ascii/src/relation_graph.rs docs/workstreams/ascii-class-er-layered-planner` | Scoped whitespace hygiene | PASS | ACELP-030 files have no whitespace errors. |
