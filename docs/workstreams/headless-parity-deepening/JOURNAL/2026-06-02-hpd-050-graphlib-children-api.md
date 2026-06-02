# HPD-050 - Graphlib Children API Shape

Date: 2026-06-02
Task: HPD-050 layout engine / Dugong-adjacent source audit

## Source Evidence

- Pinned Graphlib source: `repo-ref/graphlib` at
  `380d5efa1f4ab0904539f046bdba583d14ac2add`.
- Source tests:
  - `repo-ref/graphlib/test/graph-test.js` `children`
  - `repo-ref/graphlib/test/graph-test.js` `removeNode / removes parent / child relationships`
- Implementation source:
  - `repo-ref/graphlib/lib/graph.js` `children(v = GRAPH_NODE)`
  - `repo-ref/graphlib/lib/graph.js` `parent(...)`
  - `repo-ref/graphlib/lib/graph.js` `setParent(...)`

## Outcome

- Added `Graph::children_opt(parent)` as a narrow optional-return seam for Graphlib's
  `children(v)` query shape:
  - missing queried node -> `None`,
  - existing non-compound node -> `Some([])`,
  - existing compound node with no children -> `Some([])`,
  - existing compound node with children -> `Some(children)`.
- Kept `Graph::children(parent) -> Vec<&str>` unchanged so existing Rust callers keep the
  ergonomic empty-vector behavior.
- Reused `Graph::children_root()` as the Rust mapping for Graphlib's no-argument `children()`
  root query:
  - non-compound root -> all nodes,
  - compound root -> nodes without a parent.
- Tightened `remove_node_clears_parent_child_relationships` to assert that removing a parent makes
  `children_opt(removed)` return `None`, while the old `children(removed)` remains empty.
- Updated `docs/dugong/GRAPHLIB_UPSTREAM_TEST_COVERAGE.md` so the pinned `children` source cases
  map to concrete Rust tests.

## Boundary

This slice adds an explicit Rust seam where Graphlib's optional result shape is useful. It does not
change existing collection-returning query APIs, force JS overloads into Rust, add ID
stringification, add JS chainability, or change the explicit non-compound `setParent(...)` API
shape difference.

## Verification

- Red:
  `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong-graphlib children_opt`
  failed because `Graph::children_opt(...)` did not exist.
- Green:
  `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong-graphlib children_opt`
  passed with `1` test.
- Follow-up:
  `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong-graphlib children`
  passed with `3` tests.
- Full package:
  `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong-graphlib`
  passed with `80` tests.
- Downstream layout guard:
  `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong`
  passed with `267` tests.
