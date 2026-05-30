# ASCII Class ER Component Layout - Evidence And Gates

Status: Closed
Last updated: 2026-05-30

## Smallest Current Repro

Former unrelated class/entity layouts were public unsupported diagnostics:

- `class_parser_relationship_layouts_with_unrelated_classes_are_explicitly_unsupported`
- `er_parser_relationship_layouts_with_unrelated_entities_are_explicitly_unsupported`

They are now parser-backed component rendering contracts:

- `class_parser_relationship_layouts_render_unrelated_classes_as_components`
- `er_parser_relationship_layouts_render_unrelated_entities_as_components`

The implementation renders the relationship-bearing set through the existing layered planner and
then renders isolated standalone nodes as separate components with a blank separator. This preserves
adjacent-layer crossing behavior for multiple disjoint relationships while removing the unrelated
node diagnostic.

## Gate Set

```bash
cargo nextest run -p merman-ascii class
cargo nextest run -p merman-ascii er
cargo nextest run -p merman-ascii
cargo clippy -p merman-ascii --all-targets -- -D warnings
cargo fmt --all --check
git diff --check
```

## Evidence Log

| Date | Task | Evidence | Result |
| --- | --- | --- | --- |
| 2026-05-30 | ACECL-010 | Opened follow-on lane from `ascii-class-er-topology-routing` closeout. | Scope is limited to disconnected component layout. |
| 2026-05-30 | ACECL-020 | Added parser-backed class and ER tests for one relationship plus one unrelated standalone node. | Red tests reproduced the old class/ER unsupported diagnostics before implementation. |
| 2026-05-30 | ACECL-030 | Added shared `relation_components` helper and class/ER component render adapters. | Relationship-bearing boxes reuse existing single-edge/layered paths; isolated boxes render independently. |
| 2026-05-30 | ACECL-040 | Updated README/support docs and closed the lane after fresh gates. | Full package tests, lint, fmt, and whitespace gates passed. |

## Verification Log

| Date | Task | Command | Scope | Result | Proves |
| --- | --- | --- | --- | --- | --- |
| 2026-05-30 | ACECL-010 | `git diff --check -- docs/workstreams/ascii-class-er-component-layout` | Workstream opening docs | PASS | Opening docs have no whitespace errors. |
| 2026-05-30 | ACECL-020 | `cargo nextest run -p merman-ascii class_parser_relationship_layouts_render_unrelated_classes_as_components` | Class parser-backed component test | RED | Old renderer rejected unrelated class layouts. |
| 2026-05-30 | ACECL-020 | `cargo nextest run -p merman-ascii er_parser_relationship_layouts_render_unrelated_entities_as_components` | ER parser-backed component test | RED | Old renderer rejected unrelated ER layouts. |
| 2026-05-30 | ACECL-030 | `cargo nextest run -p merman-ascii class_parser_relationship_layouts_render_unrelated_classes_as_components` | Class component implementation | PASS | Class diagrams render related and standalone components. |
| 2026-05-30 | ACECL-030 | `cargo nextest run -p merman-ascii er_parser_relationship_layouts_render_unrelated_entities_as_components` | ER component implementation | PASS | ER diagrams render related and standalone components. |
| 2026-05-30 | ACECL-030 | `cargo nextest run -p merman-ascii class` | Class focused suite | PASS | Existing class chain, star, crossing, marker, and component behavior remains stable. |
| 2026-05-30 | ACECL-030 | `cargo nextest run -p merman-ascii er` | ER focused suite | PASS | Existing ER chain, star, crossing, cardinality, and component behavior remains stable. |
| 2026-05-30 | ACECL-030 | `cargo clippy -p merman-ascii --all-targets -- -D warnings` | ASCII lint gate | PASS | Component refactor has no clippy warnings. |
| 2026-05-30 | ACECL-030 | `cargo fmt --all --check` | Workspace formatting | PASS | Formatting is stable. |
| 2026-05-30 | ACECL-030 | `git diff --check -- crates/merman-ascii/src/relation_graph.rs crates/merman-ascii/src/class/render.rs crates/merman-ascii/src/er/render.rs crates/merman-ascii/tests/class_model.rs crates/merman-ascii/tests/er_model.rs docs/workstreams/ascii-class-er-component-layout` | Lane diff | PASS | Implementation and docs have no whitespace errors. |
| 2026-05-30 | ACECL-040 | `cargo nextest run -p merman-ascii` | Full ASCII package gate | PASS | All 113 ASCII tests pass, including class/ER component behavior. |
| 2026-05-30 | ACECL-040 | `cargo clippy -p merman-ascii --all-targets -- -D warnings` | Full ASCII lint gate | PASS | Closeout code has no clippy warnings. |
| 2026-05-30 | ACECL-040 | `cargo fmt --all --check` | Workspace formatting | PASS | Formatting remains stable after support doc updates. |
| 2026-05-30 | ACECL-040 | `git diff --check` | Whole worktree diff | PASS | Closeout diff has no whitespace errors. |
