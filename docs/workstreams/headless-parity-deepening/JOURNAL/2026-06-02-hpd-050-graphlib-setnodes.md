# HPD-050 - Graphlib setNodes API

Date: 2026-06-02
Task: HPD-050 layout engine / Dugong-adjacent source audit

## Source Evidence

- Pinned Graphlib source: `repo-ref/graphlib` at
  `380d5efa1f4ab0904539f046bdba583d14ac2add`.
- Source tests:
  - `repo-ref/graphlib/test/graph-test.js` `setNodes / creates multiple nodes`
  - `repo-ref/graphlib/test/graph-test.js` `setNodes / can set a value for all of the nodes`
- Implementation source:
  - `repo-ref/graphlib/lib/graph.js` `setNodes(vs, value)`
- Dagre source usage:
  - `repo-ref/dagre/test/order/sort-subgraph-test.js`
  - `repo-ref/dagre/test/rank/network-simplex-test.js`

## Outcome

- Added `Graph::set_nodes(nodes)` as the Rust mapping for Graphlib's `setNodes(nodes)` no-label
  behavior.
- Added `Graph::set_nodes_with_label(nodes, label)` as the Rust mapping for Graphlib's
  `setNodes(nodes, value)` behavior.
- `set_nodes(...)` reuses default node label callbacks and preserves existing node labels, matching
  Graphlib's `setNode(v)` no-value behavior.
- `set_nodes_with_label(...)` creates or updates every listed node with the same label. Its
  `N: Clone` bound is scoped to this batch-label API only.
- Updated `docs/dugong/GRAPHLIB_UPSTREAM_TEST_COVERAGE.md` so both source cases map to Rust
  regressions.

## Boundary

This is a small graph-construction API seam. It does not add JS argument overloading, chainability,
ID stringification, or a broad Graphlib port.

## Verification

- Red:
  `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong-graphlib set_nodes`
  failed because `Graph::set_nodes(...)` and `Graph::set_nodes_with_label(...)` did not exist.
- Green:
  `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong-graphlib set_nodes`
  passed with `2` tests.
- Format:
  `cargo fmt --check -p dugong-graphlib`
  passed.
- Package guard:
  `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong-graphlib`
  passed with `83` tests.
- Downstream guard:
  `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong`
  passed with `267` tests.
