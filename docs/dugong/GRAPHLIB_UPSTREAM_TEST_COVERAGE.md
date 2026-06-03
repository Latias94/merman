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
| `test/graph-test.js` | 129 | Partially ported in `crates/dugong-graphlib/tests/graph_core_test.rs` |
| `test/json-test.js` | 6 | Ported in `crates/dugong-graphlib/tests/json_test.rs` |
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

Source: `repo-ref/graphlib/test/graph-test.js`

- `initial state / has no nodes` -> `crates/dugong-graphlib/tests/graph_core_test.rs::graph_initial_state_uses_default_directed_simple_options`
- `initial state / has no edges` -> `crates/dugong-graphlib/tests/graph_core_test.rs::graph_initial_state_uses_default_directed_simple_options`
- `initial state / has no attributes` -> `crates/dugong-graphlib/tests/graph_core_test.rs::graph_initial_state_uses_default_directed_simple_options`
- `initial state / defaults to a simple directed graph` -> `crates/dugong-graphlib/tests/graph_core_test.rs::graph_initial_state_uses_default_directed_simple_options`
- `initial state / can be set to undirected` -> `crates/dugong-graphlib/tests/graph_core_test.rs::graph_options_can_enable_undirected_compound_or_multigraph_modes`
- `initial state / can be set to a compound graph` -> `crates/dugong-graphlib/tests/graph_core_test.rs::graph_options_can_enable_undirected_compound_or_multigraph_modes`
- `initial state / can be set to a mulitgraph` -> `crates/dugong-graphlib/tests/graph_core_test.rs::graph_options_can_enable_undirected_compound_or_multigraph_modes`
- `setGraph / can be used to get and set properties for the graph` -> `crates/dugong-graphlib/tests/graph_core_test.rs::graph_label_can_be_set_and_read`
- `nodes / is empty if there are no nodes in the graph` -> `crates/dugong-graphlib/tests/graph_core_test.rs::graph_initial_state_uses_default_directed_simple_options`
- `nodes / returns the ids of nodes in the graph` -> `crates/dugong-graphlib/tests/graph_core_test.rs::nodes_returns_inserted_node_ids`
- `sources / returns nodes in the graph that have no in-edges` -> `crates/dugong-graphlib/tests/graph_core_test.rs::sources_returns_nodes_without_in_edges`
- `sinks / returns nodes in the graph that have no out-edges` -> `crates/dugong-graphlib/tests/graph_core_test.rs::sinks_returns_nodes_without_out_edges`
- `filterNodes / returns an identical graph when the filter selects everything` -> `crates/dugong-graphlib/tests/graph_core_test.rs::filter_nodes_copies_selected_graph_labels_edges_and_options`
- `filterNodes / returns an empty graph when the filter selects nothing` -> `crates/dugong-graphlib/tests/graph_core_test.rs::filter_nodes_drops_rejected_nodes_and_incident_edges`
- `filterNodes / only includes nodes for which the filter returns true` -> `crates/dugong-graphlib/tests/graph_core_test.rs::filter_nodes_drops_rejected_nodes_and_incident_edges`
- `filterNodes / removes edges that are connected to removed nodes` -> `crates/dugong-graphlib/tests/graph_core_test.rs::filter_nodes_drops_rejected_nodes_and_incident_edges`
- `filterNodes / preserves the directed option` -> `crates/dugong-graphlib/tests/graph_core_test.rs::filter_nodes_copies_selected_graph_labels_edges_and_options`
- `filterNodes / preserves the multigraph option` -> `crates/dugong-graphlib/tests/graph_core_test.rs::filter_nodes_copies_selected_graph_labels_edges_and_options`
- `filterNodes / preserves the compound option` -> `crates/dugong-graphlib/tests/graph_core_test.rs::filter_nodes_copies_selected_graph_labels_edges_and_options`
- `filterNodes / includes subgraphs` -> `crates/dugong-graphlib/tests/graph_core_test.rs::filter_nodes_preserves_compound_subgraphs_and_promotes_missing_parent`
- `filterNodes / includes multi-level subgraphs` -> `crates/dugong-graphlib/tests/graph_core_test.rs::filter_nodes_preserves_compound_subgraphs_and_promotes_missing_parent`
- `filterNodes / promotes a node to a higher subgraph if its parent is not included` -> `crates/dugong-graphlib/tests/graph_core_test.rs::filter_nodes_preserves_compound_subgraphs_and_promotes_missing_parent`
- `setNode / creates the node if it isn't part of the graph` -> `crates/dugong-graphlib/tests/graph_core_test.rs::ensure_node_uses_default_label_for_new_nodes`
- `setNode / can set a value for the node` -> `crates/dugong-graphlib/tests/graph_core_test.rs::set_node_is_idempotent_for_existing_node`
- `setNode / can remove the node's value by passing undefined` -> `crates/dugong-graphlib/tests/graph_core_test.rs::set_node_with_optional_label_can_clear_label_without_removing_node`
- `setNode / is idempotent` -> `crates/dugong-graphlib/tests/graph_core_test.rs::set_node_is_idempotent_for_existing_node`
- `node / returns undefined if the node isn't part of the graph` -> `crates/dugong-graphlib/tests/graph_core_test.rs::set_node_with_optional_label_can_clear_label_without_removing_node`
- `node / returns the value of the node if it is part of the graph` -> `crates/dugong-graphlib/tests/graph_core_test.rs::set_node_with_optional_label_can_clear_label_without_removing_node`
- `setNodeDefaults / sets a default label for new nodes` -> `crates/dugong-graphlib/tests/graph_core_test.rs::ensure_node_uses_default_label_for_new_nodes`
- `setNodeDefaults / does not change existing nodes` -> `crates/dugong-graphlib/tests/graph_core_test.rs::ensure_node_does_not_change_existing_node_label`
- `setNodeDefaults / is not used if an explicit value is set` -> `crates/dugong-graphlib/tests/graph_core_test.rs::default_node_label_is_not_used_if_explicit_label_is_set`
- `setNodeDefaults / can take a function` -> `crates/dugong-graphlib/tests/graph_core_test.rs::ensure_node_uses_default_label_for_new_nodes`
- `setNodeDefaults / can take a function that takes the node's name` -> `crates/dugong-graphlib/tests/graph_core_test.rs::default_node_label_can_read_node_id`
- `removeNode / does nothing if the node is not in the graph` -> `crates/dugong-graphlib/tests/graph_core_test.rs::remove_node_is_idempotent_and_removes_incident_edges`
- `setNodes / creates multiple nodes` -> `crates/dugong-graphlib/tests/graph_core_test.rs::set_nodes_uses_default_labels_without_changing_existing_nodes`
- `setNodes / can set a value for all of the nodes` -> `crates/dugong-graphlib/tests/graph_core_test.rs::set_nodes_with_label_sets_and_updates_all_node_labels`
- `removeNode / removes the node if it is in the graph` -> `crates/dugong-graphlib/tests/graph_core_test.rs::remove_node_is_idempotent_and_removes_incident_edges`
- `removeNode / is idempotent` -> `crates/dugong-graphlib/tests/graph_core_test.rs::remove_node_is_idempotent_and_removes_incident_edges`
- `removeNode / removes edges incident on the node` -> `crates/dugong-graphlib/tests/graph_core_test.rs::remove_node_is_idempotent_and_removes_incident_edges`
- `removeNode / removes parent / child relationships for the node` -> `crates/dugong-graphlib/tests/graph_core_test.rs::remove_node_clears_parent_child_relationships`
- `setParent / creates the parent if it does not exist` -> `crates/dugong-graphlib/tests/graph_core_test.rs::set_parent_creates_parent_and_child_nodes`
- `setParent / creates the child if it does not exist` -> `crates/dugong-graphlib/tests/graph_core_test.rs::set_parent_creates_parent_and_child_nodes`
- `setParent / has the parent as undefined if it has never been invoked` -> `crates/dugong-graphlib/tests/graph_core_test.rs::parent_matches_graphlib_optional_query_shape`
- `setParent / moves the node from the previous parent` -> `crates/dugong-graphlib/tests/graph_core_test.rs::set_parent_moves_node_from_previous_parent`
- `setParent / removes the parent if the parent is undefined` -> `crates/dugong-graphlib/tests/graph_core_test.rs::clear_parent_returns_node_to_root_children`
- `setParent / removes the parent if no parent was specified` -> `crates/dugong-graphlib/tests/graph_core_test.rs::clear_parent_returns_node_to_root_children`
- `setParent / is idempotent to remove a parent` -> `crates/dugong-graphlib/tests/graph_core_test.rs::clear_parent_returns_node_to_root_children`
- `setParent / preserves the tree invariant` -> `crates/dugong-graphlib/tests/graph_core_test.rs::set_parent_preserves_tree_invariant`
- `parent / returns undefined if the graph is not compound` -> `crates/dugong-graphlib/tests/graph_core_test.rs::parent_matches_graphlib_optional_query_shape`
- `parent / returns undefined if the node is not in the graph` -> `crates/dugong-graphlib/tests/graph_core_test.rs::parent_matches_graphlib_optional_query_shape`
- `parent / defaults to undefined for new nodes` -> `crates/dugong-graphlib/tests/graph_core_test.rs::parent_matches_graphlib_optional_query_shape`
- `parent / returns the current parent assignment` -> `crates/dugong-graphlib/tests/graph_core_test.rs::parent_matches_graphlib_optional_query_shape`
- `children / returns undefined if the node is not in the graph` -> `crates/dugong-graphlib/tests/graph_core_test.rs::children_opt_distinguishes_missing_nodes_from_empty_children`
- `children / defaults to an empty list for new nodes` -> `crates/dugong-graphlib/tests/graph_core_test.rs::children_opt_distinguishes_missing_nodes_from_empty_children`
- `children / returns undefined for a non-compound graph without the node` -> `crates/dugong-graphlib/tests/graph_core_test.rs::children_opt_distinguishes_missing_nodes_from_empty_children`
- `children / returns an empty list for a non-compound graph with the node` -> `crates/dugong-graphlib/tests/graph_core_test.rs::children_opt_distinguishes_missing_nodes_from_empty_children`
- `children / returns all nodes for the root of a non-compound graph` -> `crates/dugong-graphlib/tests/graph_core_test.rs::children_root_matches_graphlib_no_arg_children_semantics`
- `children / returns children for the node` -> `crates/dugong-graphlib/tests/graph_core_test.rs::children_root_matches_graphlib_no_arg_children_semantics`
- `children / returns all nodes without a parent when the parent is not set` -> `crates/dugong-graphlib/tests/graph_core_test.rs::children_root_matches_graphlib_no_arg_children_semantics` and `crates/dugong-graphlib/tests/graph_core_test.rs::clear_parent_returns_node_to_root_children`
- `predecessors / returns the predecessors of a node` -> `crates/dugong-graphlib/tests/graph_core_test.rs::predecessors_returns_node_predecessors`
- `successors / returns the successors of a node` -> `crates/dugong-graphlib/tests/graph_core_test.rs::successors_returns_node_successors`
- `neighbors / returns the neighbors of a node` -> `crates/dugong-graphlib/tests/graph_core_test.rs::neighbors_returns_unique_in_and_out_neighbors`
- `isLeaf / returns false for connected node in undirected graph` -> `crates/dugong-graphlib/tests/graph_core_test.rs::is_leaf_follows_graphlib_directed_and_undirected_rules`
- `isLeaf / returns true for an unconnected node in undirected graph` -> `crates/dugong-graphlib/tests/graph_core_test.rs::is_leaf_follows_graphlib_directed_and_undirected_rules`
- `isLeaf / returns true for unconnected node in directed graph` -> `crates/dugong-graphlib/tests/graph_core_test.rs::is_leaf_follows_graphlib_directed_and_undirected_rules`
- `isLeaf / returns false for predecessor node in directed graph` -> `crates/dugong-graphlib/tests/graph_core_test.rs::is_leaf_follows_graphlib_directed_and_undirected_rules`
- `isLeaf / returns true for successor node in directed graph` -> `crates/dugong-graphlib/tests/graph_core_test.rs::is_leaf_follows_graphlib_directed_and_undirected_rules`
- `setPath / creates a path of mutiple edges` -> `crates/dugong-graphlib/tests/graph_core_test.rs::set_path_creates_path_edges`
- `setPath / can set a value for all of the edges` -> `crates/dugong-graphlib/tests/graph_core_test.rs::set_path_with_label_sets_and_updates_all_path_edge_labels`
- `edges / returns the keys for edges in the graph` -> `crates/dugong-graphlib/tests/graph_core_test.rs::edges_returns_inserted_edge_keys`
- `setEdge / creates the edge if it isn't part of the graph` -> `crates/dugong-graphlib/tests/graph_core_test.rs::set_edge_creates_endpoint_nodes_and_uses_default_edge_label`
- `setEdge / creates the nodes for the edge if they are not part of the graph` -> `crates/dugong-graphlib/tests/graph_core_test.rs::set_edge_creates_endpoint_nodes_and_uses_default_edge_label`
- `setEdge / changes the value for an edge if it is already in the graph` -> `crates/dugong-graphlib/tests/graph_core_test.rs::set_edge_with_label_updates_existing_edge_label`
- `setEdge / deletes the value for the edge if the value arg is undefined` -> `crates/dugong-graphlib/tests/graph_core_test.rs::set_edge_with_label_can_clear_optional_edge_label`
- `setEdge / creates a multi-edge if if it isn't part of the graph` -> `crates/dugong-graphlib/tests/graph_core_test.rs::multigraph_preserves_named_edges`
- `setEdge / throws if a multi-edge is used with a non-multigraph` -> `crates/dugong-graphlib/tests/graph_core_test.rs::set_edge_named_panics_on_named_edge_for_non_multigraph`
- `setEdge / changes the value for a multi-edge if it is already in the graph` -> `crates/dugong-graphlib/tests/graph_core_test.rs::set_edge_named_can_clear_optional_multiedge_label`
- `setEdge / can take an edge object as the first parameter` -> `crates/dugong-graphlib/tests/graph_core_test.rs::set_edge_key_sets_simple_and_named_edge_labels`
- `setEdge / can take an multi-edge object as the first parameter` -> `crates/dugong-graphlib/tests/graph_core_test.rs::set_edge_key_sets_simple_and_named_edge_labels`
- `setEdge / treats edges in opposite directions as distinct in a digraph` -> `crates/dugong-graphlib/tests/graph_core_test.rs::edge_lookup_respects_direction_for_directed_graphs`
- `setEdge / handles undirected graph edges` -> `crates/dugong-graphlib/tests/graph_core_test.rs::edge_lookup_accepts_either_direction_for_undirected_graphs`
- `setEdge / handles undirected edges where id has different order than Stringified id` ->
  `crates/dugong-graphlib/tests/graph_core_test.rs::undirected_edges_follow_graphlib_string_order_for_stringified_ids`
- `edge / returns undefined if the edge isn't part of the graph` -> `crates/dugong-graphlib/tests/graph_core_test.rs::edge_lookup_returns_none_for_missing_edges`
- `edge / returns the value of the edge if it is part of the graph` -> `crates/dugong-graphlib/tests/graph_core_test.rs::edge_lookup_respects_direction_for_directed_graphs`
- `edge / returns the value of a multi-edge if it is part of the graph` -> `crates/dugong-graphlib/tests/graph_core_test.rs::multigraph_preserves_named_edges`
- `edge / returns an edge in either direction in an undirected graph` -> `crates/dugong-graphlib/tests/graph_core_test.rs::edge_lookup_accepts_either_direction_for_undirected_graphs`
- `removeEdge / has no effect if the edge is not in the graph` -> `crates/dugong-graphlib/tests/graph_core_test.rs::remove_edge_missing_edge_is_noop`
- `removeEdge / can remove an edge by edgeObj` -> `crates/dugong-graphlib/tests/graph_core_test.rs::remove_edge_key_removes_named_multigraph_edge`
- `removeEdge / can remove an edge by separate ids` -> `crates/dugong-graphlib/tests/graph_core_test.rs::remove_edge_with_named_ids_removes_named_multigraph_edge`
- `removeEdge / correctly removes neighbors` -> `crates/dugong-graphlib/tests/graph_core_test.rs::remove_edge_updates_neighbor_queries`
- `removeEdge / correctly decrements neighbor counts` -> `crates/dugong-graphlib/tests/graph_core_test.rs::remove_edge_keeps_named_parallel_edges`
- `removeEdge / works with undirected graphs` -> `crates/dugong-graphlib/tests/graph_core_test.rs::remove_edge_accepts_reversed_endpoints_for_undirected_graphs`
- `inEdges / returns the edges that point at the specified node` -> `crates/dugong-graphlib/tests/graph_core_test.rs::in_edges_returns_edges_pointing_at_node`
- `inEdges / works for multigraphs` -> `crates/dugong-graphlib/tests/graph_core_test.rs::edge_queries_work_for_multigraphs_and_endpoint_filters`
- `inEdges / can return only edges from a specified node` -> `crates/dugong-graphlib/tests/graph_core_test.rs::edge_queries_work_for_multigraphs_and_endpoint_filters`
- `outEdges / returns all edges that this node points at` -> `crates/dugong-graphlib/tests/graph_core_test.rs::out_edges_returns_edges_pointing_from_node`
- `outEdges / works for multigraphs` -> `crates/dugong-graphlib/tests/graph_core_test.rs::edge_queries_work_for_multigraphs_and_endpoint_filters`
- `outEdges / can return only edges to a specified node` -> `crates/dugong-graphlib/tests/graph_core_test.rs::edge_queries_work_for_multigraphs_and_endpoint_filters`
- `nodeEdges / returns all edges that this node points at` -> `crates/dugong-graphlib/tests/graph_core_test.rs::node_edges_returns_all_incident_edges`
- `nodeEdges / works for multigraphs` -> `crates/dugong-graphlib/tests/graph_core_test.rs::node_edges_returns_parallel_multigraph_edges`
- `nodeEdges / can return only edges between specific nodes` -> `crates/dugong-graphlib/tests/graph_core_test.rs::node_edges_between_returns_edges_between_specific_nodes`
- `setDefaultEdgeLabel / sets a default label for new edges` -> `crates/dugong-graphlib/tests/graph_core_test.rs::set_edge_creates_endpoint_nodes_and_uses_default_edge_label`
- `setDefaultEdgeLabel / does not change existing edges` -> `crates/dugong-graphlib/tests/graph_core_test.rs::default_edge_label_does_not_change_existing_edge`
- `setDefaultEdgeLabel / is not used if an explicit value is set` -> `crates/dugong-graphlib/tests/graph_core_test.rs::default_edge_label_is_not_used_if_explicit_label_is_set`
- `setDefaultEdgeLabel / can take a function` -> `crates/dugong-graphlib/tests/graph_core_test.rs::set_edge_creates_endpoint_nodes_and_uses_default_edge_label`
- `setDefaultEdgeLabel / can take a function that takes the edge's endpoints and name` -> `crates/dugong-graphlib/tests/graph_core_test.rs::default_edge_label_can_read_endpoints_and_name`
- `setDefaultEdgeLabel / does not set a default value for a multi-edge that already exists` -> `crates/dugong-graphlib/tests/graph_core_test.rs::default_edge_label_does_not_replace_existing_named_edge`

Source: `repo-ref/graphlib/test/json-test.js`

- `preserves the graph options` -> `crates/dugong-graphlib/tests/json_test.rs::json_preserves_graph_options`
- `preserves the graph value, if any` -> `crates/dugong-graphlib/tests/json_test.rs::json_preserves_graph_value_if_any`
- `preserves nodes` -> `crates/dugong-graphlib/tests/json_test.rs::json_preserves_nodes`
- `preserves simple edges` -> `crates/dugong-graphlib/tests/json_test.rs::json_preserves_simple_edges`
- `preserves multi-edges` -> `crates/dugong-graphlib/tests/json_test.rs::json_preserves_multi_edges`
- `preserves parent / child relationships` -> `crates/dugong-graphlib/tests/json_test.rs::json_preserves_parent_child_relationships`

Additional Rust regression:

- `crates/dugong-graphlib/tests/json_test.rs::json_distinguishes_undefined_from_explicit_null_for_option_labels`
  protects the primary `Option<T>` seam: omitted `value` fields map to `None`, while explicit JSON
  `null` remains a present label value.
- `crates/dugong-graphlib/tests/json_test.rs::json_with_defaults_can_collapse_missing_values_to_rust_defaults`
  protects the explicit default-collapsing fallback helpers without weakening the primary seam.

## Open API Shape Differences

- Missing-node query methods: upstream JS returns `undefined` for several collection queries.
  `children(...)` now has an explicit optional Rust seam, `children_opt(...)`, while the existing
  ergonomic `children(...)` still returns an empty vector for missing nodes. `predecessors`,
  `successors`, `neighbors`, `inEdges`, `outEdges`, and `nodeEdges` still use empty vectors for
  missing nodes; that remains a deliberate shape difference until consumers justify additional
  fallible/optional seams.
- Chainable mutators: upstream `removeEdge(...)` returns the graph object. Rust mutators currently
  return booleans or `&mut Self` depending on the method; coverage focuses on state changes rather
  than JS chaining.
- Non-compound `setParent(...)`: upstream throws; current Rust parent methods no-op on
  non-compound graphs. This remains an explicit API-shape decision.
- ID stringification: upstream JS coerces node ids, edge endpoints, and edge names through string
  conversion. Rust accepts typed string inputs, so numeric/object coercion is not a parity target
  unless a public FFI seam needs it. The consumer-relevant post-coercion rule for undirected edge
  endpoint ordering is covered by
  `undirected_edges_follow_graphlib_string_order_for_stringified_ids`.
- Graphlib JSON omitted-value semantics: `dugong_graphlib::json::{write, read}` maps upstream
  `undefined` to Rust `Option<T>` labels and preserves explicit JSON `null` as `Some(null)`.
  `write_with_defaults` / `read_with_defaults` are a separate fallback seam for Rust callers that
  intentionally want missing labels collapsed onto `Default`.

## Next Priority

1. Continue `test/graph-test.js` only where it maps to current Rust API shape and real consumers.
   Compound child/root API-shape coverage, `filterNodes`, and endpoint-aware default label
   callbacks now have direct Rust coverage.
2. Reuse `dugong_graphlib::json` before introducing another ad hoc Graphlib-shaped serializer
   elsewhere. Prefer the primary `Option<T>` seam when upstream `undefined` versus `null`
   semantics matter; use the default-collapsing helpers only as an explicit Rust bridge. Existing
   Rust-specific debug snapshots such as `xtask`'s Dagre reference input format remain separate
   because they carry Dagre label payloads, not plain Graphlib graph labels.
3. Keep non-used algorithms such as shortest paths, Prim, and Floyd-Warshall out of scope unless a
   Mermaid/Dagre path starts consuming them.
