# ASCII Class ER Parallel Routing - Evidence And Gates

Status: Active
Last updated: 2026-05-30

## Smallest Current Repro

Current same-endpoint parallel layouts are public unsupported diagnostics:

- `parallel class relationship layouts`
- `parallel ER relationship layouts`

The first implementation slice should render multiple relationships between the same two endpoints
as adjacent terminal lanes while preserving each relationship's text semantics.

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
| 2026-05-30 | ACEPR-010 | Opened follow-on lane from class/ER topology and component closeouts. | Scope is limited to same-endpoint parallel relationship routing. |
| 2026-05-30 | ACEPR-020 | Added parser-backed class and ER tests for two relationships between the same endpoints. | Red tests reproduced the old class/ER parallel diagnostics before implementation. |
| 2026-05-30 | ACEPR-030 | Added shared parallel vertical stack formatting and class/ER adapter routing for same-endpoint parallel relationships. | Each class/ER parallel relationship renders in a distinct lane with markers, labels, cardinality, and line style preserved. |

## Verification Log

| Date | Task | Command | Scope | Result | Proves |
| --- | --- | --- | --- | --- | --- |
| 2026-05-30 | ACEPR-010 | `git diff --check -- docs/workstreams/ascii-class-er-parallel-routing` | Workstream opening docs | PASS | Opening docs have no whitespace errors. |
| 2026-05-30 | ACEPR-020 | `cargo nextest run -p merman-ascii class_parser_parallel_relationship_layout_renders_each_lane` | Class parser-backed parallel test | RED | Old renderer rejected same-endpoint class parallel layouts. |
| 2026-05-30 | ACEPR-020 | `cargo nextest run -p merman-ascii er_parser_parallel_relationship_layout_renders_each_lane` | ER parser-backed parallel test | RED | Old renderer rejected same-endpoint ER parallel layouts. |
| 2026-05-30 | ACEPR-030 | `cargo nextest run -p merman-ascii class_parser_parallel_relationship_layout_renders_each_lane` | Class parallel implementation | PASS | Class diagrams render each same-endpoint parallel relationship lane. |
| 2026-05-30 | ACEPR-030 | `cargo nextest run -p merman-ascii er_parser_parallel_relationship_layout_renders_each_lane` | ER parallel implementation | PASS | ER diagrams render each same-endpoint parallel relationship lane. |
| 2026-05-30 | ACEPR-030 | `cargo nextest run -p merman-ascii class` | Class focused suite | PASS | Existing class chain, star, crossing, component, and marker behavior remains stable. |
| 2026-05-30 | ACEPR-030 | `cargo nextest run -p merman-ascii er` | ER focused suite | PASS | Existing ER chain, star, crossing, component, cardinality, and line-style behavior remains stable. |
| 2026-05-30 | ACEPR-030 | `cargo clippy -p merman-ascii --all-targets -- -D warnings` | ASCII lint gate | PASS | Parallel refactor has no clippy warnings. |
| 2026-05-30 | ACEPR-030 | `cargo fmt --all --check` | Workspace formatting | PASS | Formatting is stable. |
| 2026-05-30 | ACEPR-030 | `git diff --check -- crates/merman-ascii/src/relation_graph.rs crates/merman-ascii/src/class/render.rs crates/merman-ascii/src/er/render.rs crates/merman-ascii/tests/class_model.rs crates/merman-ascii/tests/er_model.rs docs/workstreams/ascii-class-er-parallel-routing` | Lane diff | PASS | Implementation and docs have no whitespace errors. |
