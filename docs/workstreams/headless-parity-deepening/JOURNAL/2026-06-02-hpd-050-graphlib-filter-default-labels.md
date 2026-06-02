# HPD-050 - Graphlib Filter Nodes And Default Label Callbacks

Date: 2026-06-02
Task: HPD-050 layout engine / Dugong-adjacent source audit

## Source Evidence

- Pinned Graphlib source: `repo-ref/graphlib` at
  `380d5efa1f4ab0904539f046bdba583d14ac2add`.
- Source tests:
  - `repo-ref/graphlib/test/graph-test.js` `filterNodes`
  - `setNodeDefaults` callback with node id
  - `setDefaultEdgeLabel` callback with edge endpoints and name
- Implementation source:
  - `repo-ref/graphlib/lib/graph.js` `filterNodes(...)`
  - `setDefaultNodeLabel(...)`
  - `setDefaultEdgeLabel(...)`

## Outcome

- Added `Graph::filter_nodes(...)` with method-level `N: Clone, E: Clone, G: Clone` bounds.
- The filtered graph preserves Graphlib options and graph label, copies selected node labels,
  copies only edges whose endpoints remain selected, and promotes compound children to the nearest
  selected ancestor when their direct parent is filtered out.
- Changed the stored default label closures to receive source-backed arguments:
  - node id for node defaults,
  - edge `v`, `w`, and optional name for edge defaults.
- Kept existing no-arg Rust setters by wrapping them in argument-ignoring closures.
- Added explicit argument-aware Rust setters:
  - `set_default_node_label_with_id(...)`
  - `set_default_edge_label_with_endpoints(...)`

## Boundary

This is public Graph API parity work, not a renderer tune. It does not implement unused Graphlib
shortest-path algorithms and does not force JS chainability or missing-node `undefined` semantics
into Rust collection APIs.

`filter_nodes(...)` is intentionally Clone-bound only at the method level so ordinary Dagre layout
graphs do not gain a broad type constraint.

## Verification

- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo test -p dugong-graphlib --test graph_core_test`
  passed with `52` tests.
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong-graphlib` passed with `78`
  tests.
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong` passed with `267` tests.
