# HPD-050 Graphlib Edge Query Coverage

Date: 2026-06-02

## What Changed

- Extended the direct `repo-ref/graphlib/test/graph-test.js` port in
  `crates/dugong-graphlib/tests/graph_core_test.rs` to cover public edge and adjacency queries:
  `sinks`, `predecessors`, `successors`, `neighbors`, `isLeaf`, `inEdges`, `outEdges`, and
  `nodeEdges`.
- Added `Graph::sinks(...)`, `Graph::is_leaf(...)`, and `Graph::node_edges_between(...)` to the
  Rust Graph API where upstream behavior had no current Rust entry point.
- Added regression coverage that edge removal updates predecessor/successor/neighbor queries and
  preserves the remaining named parallel edge in multigraphs.

## Findings

- This is a useful source-backed public API slice because Dagre and Graphlib algorithms consume
  these query methods directly; it is not a speculative implementation of unused algorithms.
- Upstream `nodeEdges(v, w)` is direction-insensitive because it combines `inEdges(v, w)` and
  `outEdges(v, w)`. The Rust `node_edges_between(...)` seam follows that endpoint-pair behavior.
- Missing-node query behavior remains a Rust/JS API-shape difference: upstream returns
  `undefined`, while Rust collection queries return empty vectors. That should stay explicit in
  the coverage ledger rather than being presented as identical parity.

## Verification

- `cargo test -p dugong-graphlib --tests`
- `cargo test -p dugong --tests`
