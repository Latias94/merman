# Dugong Graphlib: Upstream Test Coverage

Scope: `@dagrejs/graphlib@2.2.4` (see `tools/upstreams/REPOS.lock.json`).

Source checkout:

- `repo-ref/graphlib`
- pinned commit: `380d5efa1f4ab0904539f046bdba583d14ac2add`

This ledger tracks direct Graphlib source-test ports. It is not a completion percentage: many
Graphlib behaviors are also indirectly exercised by the Dagre tests in `crates/dugong/tests`.

## Current Source Inventory

As of the pinned checkout, `repo-ref/graphlib/test` contains 212 `it(...)` cases across these files:

| Source file | Cases | Direct Rust coverage status |
| --- | ---: | --- |
| `test/alg/components-test.js` | 4 | Ported in `crates/dugong-graphlib/tests/alg_test.rs` |
| `test/alg/find-cycles-test.js` | 6 | Ported in `crates/dugong-graphlib/tests/alg_test.rs` |
| `test/alg/postorder-test.js` | 6 | Ported in `crates/dugong-graphlib/tests/alg_test.rs` |
| `test/alg/preorder-test.js` | 5 | Ported in `crates/dugong-graphlib/tests/alg_test.rs` |
| `test/graph-test.js` | 129 | Not yet ledger-ported independently |
| `test/json-test.js` | 6 | Not yet implemented as a Graphlib JSON seam |
| `test/bundle-test.js` | 3 | Not applicable as a JS bundle test; Rust crate smoke tests may replace it |
| `test/version-test.js` | 1 | Not yet ledger-ported independently |
| `test/data/priority-queue-test.js` | 18 | Not implemented as a public Graphlib data structure |
| `test/alg/all-shortest-paths-test.js` | 5 | Not implemented |
| `test/alg/dijkstra-all-test.js` | 1 | Not implemented |
| `test/alg/dijkstra-test.js` | 7 | Not implemented |
| `test/alg/floyd-warshall-test.js` | 2 | Not implemented |
| `test/alg/is-acyclic-test.js` | 4 | Not implemented as a separate public API |
| `test/alg/prim-test.js` | 4 | Not implemented |
| `test/alg/tarjan-test.js` | 5 | Internal behavior is covered through `find_cycles`; no public `tarjan` API |
| `test/alg/topsort-test.js` | 6 | Not implemented |

## Ported Cases

Source: `repo-ref/graphlib/test/alg/components-test.js`

- `returns an empty list for an empty graph` -> `crates/dugong-graphlib/tests/alg_test.rs::components_returns_empty_for_empty_graph`
- `returns singleton lists for unconnected nodes` -> `crates/dugong-graphlib/tests/alg_test.rs::components_returns_singletons_for_unconnected_nodes`
- `returns a list of nodes in a component` -> `crates/dugong-graphlib/tests/alg_test.rs::components_returns_undirected_component_nodes`
- `returns nodes connected by a neighbor relationship in a digraph` -> `crates/dugong-graphlib/tests/alg_test.rs::components_uses_neighbor_relationships_in_directed_graphs`

Source: `repo-ref/graphlib/test/alg/find-cycles-test.js`

- `returns an empty array for an empty graph` -> `crates/dugong-graphlib/tests/alg_test.rs::find_cycles_returns_empty_for_empty_graph`
- `returns an empty array if the graph has no cycles` -> `crates/dugong-graphlib/tests/alg_test.rs::find_cycles_returns_empty_for_acyclic_graph`
- `returns a single entry for a cycle of 1 node` -> `crates/dugong-graphlib/tests/alg_test.rs::find_cycles_returns_single_node_cycle`
- `returns a single entry for a cycle of 2 nodes` -> `crates/dugong-graphlib/tests/alg_test.rs::find_cycles_returns_two_node_cycle`
- `returns a single entry for a triangle` -> `crates/dugong-graphlib/tests/alg_test.rs::find_cycles_returns_triangle_cycle`
- `returns multiple entries for multiple cycles` -> `crates/dugong-graphlib/tests/alg_test.rs::find_cycles_returns_multiple_cycles`

Source: `repo-ref/graphlib/test/alg/preorder-test.js`

- `returns the root for a singleton graph` -> `crates/dugong-graphlib/tests/alg_test.rs::preorder_returns_singleton_root`
- `visits each node in the graph once` -> `crates/dugong-graphlib/tests/alg_test.rs::preorder_visits_each_node_once`
- `works for a tree` -> `crates/dugong-graphlib/tests/alg_test.rs::preorder_preserves_parent_before_child_order`
- `works for an array of roots` -> `crates/dugong-graphlib/tests/alg_test.rs::preorder_accepts_multiple_roots`
- `fails if root is not in the graph` -> `crates/dugong-graphlib/tests/alg_test.rs::preorder_panics_if_root_is_not_in_the_graph`

Source: `repo-ref/graphlib/test/alg/postorder-test.js`

- `returns the root for a singleton graph` -> `crates/dugong-graphlib/tests/alg_test.rs::postorder_returns_singleton_root`
- `visits each node in the graph once` -> `crates/dugong-graphlib/tests/alg_test.rs::postorder_visits_each_node_once`
- `works for a tree` -> `crates/dugong-graphlib/tests/alg_test.rs::postorder_preserves_child_before_parent_order`
- `works for an array of roots` -> `crates/dugong-graphlib/tests/alg_test.rs::postorder_accepts_multiple_roots`
- `works for multiple connected roots` -> `crates/dugong-graphlib/tests/alg_test.rs::postorder_handles_multiple_connected_roots`
- `fails if root is not in the graph` -> `crates/dugong-graphlib/tests/alg_test.rs::postorder_panics_if_root_is_not_in_the_graph`

## Next Priority

1. Port a focused slice of `test/graph-test.js` for the public APIs that `dugong` and Mermaid-facing
   renderers rely on: options, node creation/defaults, edge creation/defaults, compound
   parent/children semantics, multigraph edge keys, and remove-node/remove-edge cleanup.
2. Decide whether Graphlib JSON should exist as a Rust seam. If yes, port `test/json-test.js`
   before adding ad hoc snapshot serializers elsewhere.
3. Keep non-used algorithms such as shortest paths, Prim, and Floyd-Warshall out of scope unless a
   Mermaid/Dagre path starts consuming them.
