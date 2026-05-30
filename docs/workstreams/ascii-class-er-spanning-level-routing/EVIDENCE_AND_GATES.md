# ASCII Class ER Spanning Level Routing - Evidence And Gates

Status: Closed
Last updated: 2026-05-30

## Smallest Current Repro

Current spanning-level relationships reject with public diagnostics:

- `class relationships spanning multiple layout levels`
- `ER relationships spanning multiple layout levels`

The implementation slice should render a simple three-node transitive shape with a side lane for
the skipped relationship.

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
| 2026-05-30 | ACESLR-010 | Opened follow-on lane from `ascii-class-er-mixed-parallel-routing` closeout. | Scope is limited to non-cyclic spanning-level relationships. |
| 2026-05-30 | ACESLR-020 | Added parser-backed class and ER tests for three-node spanning-level relationship layouts. | Tests reproduced the old spanning-level diagnostics before implementation. |
| 2026-05-30 | ACESLR-030 | Added shared side-lane width reservation and class/ER adapter routing for edges that skip an intermediate box. | Spanning relationships render without crossing the intermediate box. |
| 2026-05-30 | ACESLR-040 | Updated public README support docs and reviewed the lane for closeout. | Lane can close; cyclic and dense topology work remain follow-ons. |

## Verification Log

| Date | Task | Command | Scope | Result | Proves |
| --- | --- | --- | --- | --- | --- |
| 2026-05-30 | ACESLR-010 | `git diff --check -- docs/workstreams/ascii-class-er-spanning-level-routing` | Workstream opening docs | PASS | Opening docs have no whitespace errors. |
| 2026-05-30 | ACESLR-020 | `cargo nextest run -p merman-ascii class_parser_spanning_level_relationship_layout_routes_around_intermediate_box` | Class parser-backed spanning-level test before implementation | RED | Old behavior rejected with `class relationships spanning multiple layout levels`. |
| 2026-05-30 | ACESLR-020 | `cargo nextest run -p merman-ascii er_parser_spanning_level_relationship_layout_routes_around_intermediate_entity` | ER parser-backed spanning-level test before implementation | RED | Old behavior rejected with `ER relationships spanning multiple layout levels`. |
| 2026-05-30 | ACESLR-030 | `cargo nextest run -p merman-ascii class_parser_spanning_level_relationship_layout_routes_around_intermediate_box` | Class parser-backed spanning-level test | PASS | Class spanning-level relationship uses the reserved side lane. |
| 2026-05-30 | ACESLR-030 | `cargo nextest run -p merman-ascii er_parser_spanning_level_relationship_layout_routes_around_intermediate_entity` | ER parser-backed spanning-level test | PASS | ER spanning-level relationship uses the reserved side lane. |
| 2026-05-30 | ACESLR-030 | `cargo nextest run -p merman-ascii class` | Class ASCII tests | PASS | Existing class relationship behavior stays stable with spanning-level support. |
| 2026-05-30 | ACESLR-030 | `cargo nextest run -p merman-ascii er` | ER ASCII tests | PASS | Existing ER relationship behavior stays stable with spanning-level support. |
| 2026-05-30 | ACESLR-030 | `cargo clippy -p merman-ascii --all-targets -- -D warnings` | `merman-ascii` lint gate | PASS | New shared routing and adapter code are warning-free. |
| 2026-05-30 | ACESLR-030 | `cargo fmt --all --check` | Workspace formatting | PASS | Rust formatting is stable. |
| 2026-05-30 | ACESLR-030 | `git diff --check -- crates/merman-ascii/src/relation_graph.rs crates/merman-ascii/src/class/render.rs crates/merman-ascii/src/er/render.rs crates/merman-ascii/tests/class_model.rs crates/merman-ascii/tests/er_model.rs docs/workstreams/ascii-class-er-spanning-level-routing` | Spanning-level implementation diff | PASS | Implementation and lane docs have no whitespace errors. |
| 2026-05-30 | ACESLR-040 | `review-workstream` | Workstream compliance and code quality | PASS | No blocking findings before closeout; shipped behavior matches lane scope. |
| 2026-05-30 | ACESLR-040 | `cargo nextest run -p merman-ascii` | Full `merman-ascii` package tests | PASS | All ASCII renderer tests pass with spanning-level support documented. |
| 2026-05-30 | ACESLR-040 | `cargo clippy -p merman-ascii --all-targets -- -D warnings` | `merman-ascii` lint gate | PASS | Closeout build remains warning-free. |
| 2026-05-30 | ACESLR-040 | `cargo fmt --all --check` | Workspace formatting | PASS | Rust formatting remains stable. |
| 2026-05-30 | ACESLR-040 | `git diff --check` | Workspace diff | PASS | Closeout docs and support README changes have no whitespace errors. |
