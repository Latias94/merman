# ASCII Graph Final Parity - Design

Status: Complete
Last updated: 2026-05-30

## Intent

Finish the remaining ASCII graph parity work without letting `routing.rs` become the permanent
dumping ground for every lane, label, and subgraph behavior.

## Problem

After the parser/order/padding lane, graph parity is 60 exact matches and Unicode graph gaps are
clear. The remaining ASCII gaps are one multiline node-label fixture and a cluster of subgraph-heavy
fixtures. The next implementation work will touch route drawing, labels, node sizing, group bounds,
and subgraph routing; `routing.rs` is already about 39KB and needs deeper module boundaries first.

## Scope

- `crates/merman-ascii/src/graph/routing.rs`
- `crates/merman-ascii/src/graph/routing/`
- `crates/merman-ascii/src/graph/layout.rs`
- `crates/merman-ascii/src/graph/draw.rs`
- `crates/merman-ascii/src/graph/adapter.rs`
- `crates/merman-ascii/tests`
- `crates/merman-ascii/tests/testdata/mermaid-ascii/GRAPH_FIXTURE_GAPS.md`

## Target Fixtures

Multiline:

- `ascii/multiline_single_node.txt`

Subgraph-heavy:

- `ascii/subgraph_complex_mixed.txt`
- `ascii/subgraph_complex_nested.txt`
- `ascii/subgraph_explicit_title.txt`
- `ascii/subgraph_mixed_nodes_td.txt`
- `ascii/subgraph_mixed_nodes.txt`
- `ascii/subgraph_nested_with_external.txt`
- `ascii/subgraph_nested.txt`
- `ascii/subgraph_node_outside_lr.txt`
- `ascii/subgraph_standalone_labeled_node.txt`
- `ascii/subgraph_td_multiple_paddingy.txt`
- `ascii/subgraph_td_multiple.txt`
- `ascii/subgraph_three_levels_nested.txt`
- `ascii/subgraph_three_separate.txt`
- `ascii/subgraph_with_labels.txt`

## Refactor Plan

- Split `routing.rs` into route-cell merging, routed-label placement, and lane/back-edge drawing
  modules before adding new behavior.
- Keep `routing/path.rs` as the path planner; do not move layout concerns into it.
- Add multiline node-label support through model/layout/draw boundaries, not string post-processing.
- Treat subgraph-heavy parity as group layout/routing behavior. Avoid fixture-name conditionals.

## Testing Plan

Focused gates:

- `cargo fmt --all --check`
- `cargo nextest run -p merman-ascii graph_fixture`
- `cargo nextest run -p merman-ascii graph::`
- `cargo nextest run -p merman-ascii flowchart`

Broad gates:

- `cargo nextest run -p merman-ascii`
- `cargo nextest run -p merman --features ascii`
- `cargo nextest run -p merman-cli --features ascii`
- `cargo clippy -p merman-ascii -p merman --features ascii --all-targets -- -D warnings`
- `git diff --check`

## Risk Plan

- Subgraph routing can affect many already-exact fixtures. Move fixtures only when exact output is
  confirmed by the allowlist test.
- Multiline labels change node height and label positioning; this must preserve single-line node
  snapshots.
- Split large behavior-preserving refactors into their own commits when useful.
