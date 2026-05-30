# ASCII Class ER Mixed Parallel Routing - Evidence And Gates

Status: Active
Last updated: 2026-05-30

## Smallest Current Repro

Current mixed-parallel components still reject with public parallel diagnostics:

- `parallel class relationship layouts`
- `parallel ER relationship layouts`

The implementation slice should render a simple star-like component where one endpoint pair has two
parallel relationships and another endpoint pair has one ordinary relationship.

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
| 2026-05-30 | ACEMPR-010 | Opened follow-on lane from `ascii-class-er-parallel-routing` closeout. | Scope is limited to mixed-parallel relationship components. |
| 2026-05-30 | ACEMPR-020 | Added parser-backed class and ER tests for a parallel endpoint pair plus another ordinary edge. | Red tests reproduced the old class/ER parallel diagnostics before implementation. |
| 2026-05-30 | ACEMPR-030 | Removed duplicate endpoint-pair rejection from layered planning and added lane-offset drawing order. | Mixed-parallel class/ER components render all relationships without omitting duplicate lanes. |

## Verification Log

| Date | Task | Command | Scope | Result | Proves |
| --- | --- | --- | --- | --- | --- |
| 2026-05-30 | ACEMPR-010 | `git diff --check -- docs/workstreams/ascii-class-er-mixed-parallel-routing` | Workstream opening docs | PASS | Opening docs have no whitespace errors. |
| 2026-05-30 | ACEMPR-020 | `cargo nextest run -p merman-ascii class_parser_mixed_parallel_relationship_layout_renders_each_lane` | Class parser-backed mixed-parallel test | RED | Old renderer rejected mixed class parallel layouts. |
| 2026-05-30 | ACEMPR-020 | `cargo nextest run -p merman-ascii er_parser_mixed_parallel_relationship_layout_renders_each_lane` | ER parser-backed mixed-parallel test | RED | Old renderer rejected mixed ER parallel layouts. |
| 2026-05-30 | ACEMPR-030 | `cargo nextest run -p merman-ascii class_parser_mixed_parallel_relationship_layout_renders_each_lane` | Class mixed-parallel implementation | PASS | Class diagrams render a duplicate endpoint pair plus another ordinary edge. |
| 2026-05-30 | ACEMPR-030 | `cargo nextest run -p merman-ascii er_parser_mixed_parallel_relationship_layout_renders_each_lane` | ER mixed-parallel implementation | PASS | ER diagrams render a duplicate endpoint pair plus another ordinary edge. |
| 2026-05-30 | ACEMPR-030 | `cargo nextest run -p merman-ascii class` | Class focused suite | PASS | Existing class component, same-endpoint parallel, crossing, star, and chain behavior remains stable. |
| 2026-05-30 | ACEMPR-030 | `cargo nextest run -p merman-ascii er` | ER focused suite | PASS | Existing ER component, same-endpoint parallel, crossing, star, chain, and cardinality behavior remains stable. |
| 2026-05-30 | ACEMPR-030 | `cargo clippy -p merman-ascii --all-targets -- -D warnings` | ASCII lint gate | PASS | Mixed-parallel refactor has no clippy warnings. |
| 2026-05-30 | ACEMPR-030 | `cargo fmt --all --check` | Workspace formatting | PASS | Formatting is stable. |
| 2026-05-30 | ACEMPR-030 | `git diff --check -- crates/merman-ascii/src/relation_graph.rs crates/merman-ascii/src/class/render.rs crates/merman-ascii/src/er/render.rs crates/merman-ascii/tests/class_model.rs crates/merman-ascii/tests/er_model.rs docs/workstreams/ascii-class-er-mixed-parallel-routing` | Lane diff | PASS | Implementation and docs have no whitespace errors. |
