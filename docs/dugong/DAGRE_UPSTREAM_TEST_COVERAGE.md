# Dugong: Dagre Upstream Test Coverage

Scope: `@dagrejs/dagre@2.0.2` (see `repo-ref/REPOS.lock.json`).

Source: `repo-ref/dagre/test/layout-test.js`

- `can layout a single node` → `crates/dugong/tests/layout_test.rs::layout_can_layout_a_single_node`
- `can layout two nodes on the same rank` → `crates/dugong/tests/layout_test.rs::layout_can_layout_two_nodes_on_the_same_rank`
- `can layout two nodes connected by an edge` → `crates/dugong/tests/layout_test.rs::layout_can_layout_two_nodes_connected_by_an_edge`
- `can layout an edge with a label` → `crates/dugong/tests/layout_test.rs::layout_can_layout_an_edge_with_a_label`
- `adds rectangle intersects for edges` → `crates/dugong/tests/layout_test.rs::layout_adds_rectangle_intersects_for_edges`
- `adds rectangle intersects for edges spanning multiple ranks` → `crates/dugong/tests/layout_test.rs::layout_adds_rectangle_intersects_for_edges_spanning_multiple_ranks`
- `can apply an offset, with rankdir = ...` → `crates/dugong/tests/layout_test.rs::layout_can_apply_an_offset`
- `can layout an edge with a long label, with rankdir = ...` → `crates/dugong/tests/layout_test.rs::layout_can_layout_an_edge_with_a_long_label`
- `can layout a self loop` → `crates/dugong/tests/layout_test.rs::layout_can_layout_a_self_loop`
- `can layout a graph with subgraphs` → `crates/dugong/tests/layout_test.rs::layout_can_layout_a_graph_with_subgraphs`

Source: `repo-ref/dagre/test/nesting-graph-test.js`

- `connects a disconnected graph` → `crates/dugong/tests/nesting_graph_test.rs::nesting_graph_connects_a_disconnected_graph`
- `adds border nodes to the top and bottom of a subgraph` → `crates/dugong/tests/nesting_graph_test.rs::nesting_graph_adds_border_nodes_to_top_and_bottom_of_a_subgraph`
- `adds edges between borders of nested subgraphs` → `crates/dugong/tests/nesting_graph_test.rs::nesting_graph_adds_edges_between_borders_of_nested_subgraphs`
- `adds sufficient weight to border to node edges` → `crates/dugong/tests/nesting_graph_test.rs::nesting_graph_adds_sufficient_weight_to_border_to_node_edges`
- `adds an edge from the root to the tops of top-level subgraphs` → `crates/dugong/tests/nesting_graph_test.rs::nesting_graph_adds_edge_from_root_to_tops_of_top_level_subgraphs`
- `adds an edge from root to each node with the correct minlen #1` → `crates/dugong/tests/nesting_graph_test.rs::nesting_graph_adds_edge_from_root_to_each_node_minlen_1`
- `adds an edge from root to each node with the correct minlen #2` → `crates/dugong/tests/nesting_graph_test.rs::nesting_graph_adds_edge_from_root_to_each_node_minlen_2`
- `adds an edge from root to each node with the correct minlen #3` → `crates/dugong/tests/nesting_graph_test.rs::nesting_graph_adds_edge_from_root_to_each_node_minlen_3`
- `does not add an edge from the root to itself` → `crates/dugong/tests/nesting_graph_test.rs::nesting_graph_does_not_add_an_edge_from_root_to_itself`
- `expands inter-node edges to separate SG border and nodes #1` → `crates/dugong/tests/nesting_graph_test.rs::nesting_graph_expands_inter_node_edges_minlen_1`
- `expands inter-node edges to separate SG border and nodes #2` → `crates/dugong/tests/nesting_graph_test.rs::nesting_graph_expands_inter_node_edges_minlen_2`
- `expands inter-node edges to separate SG border and nodes #3` → `crates/dugong/tests/nesting_graph_test.rs::nesting_graph_expands_inter_node_edges_minlen_3`
- `sets minlen correctly for nested SG boder to children` → `crates/dugong/tests/nesting_graph_test.rs::nesting_graph_sets_minlen_correctly_for_nested_border_to_children`
- `removes nesting graph edges` → `crates/dugong/tests/nesting_graph_test.rs::nesting_graph_cleanup_removes_nesting_graph_edges`
- `removes the root node` → `crates/dugong/tests/nesting_graph_test.rs::nesting_graph_cleanup_removes_the_root_node`

Source: `repo-ref/dagre/test/position-test.js`

- `respects ranksep` → `crates/dugong/tests/position_test.rs::position_respects_ranksep`
- `use the largest height in each rank with ranksep` → `crates/dugong/tests/position_test.rs::position_uses_largest_height_in_each_rank_with_ranksep`
- `respects nodesep` → `crates/dugong/tests/position_test.rs::position_respects_nodesep`
- `should not try to position the subgraph node itself` → `crates/dugong/tests/position_test.rs::position_does_not_try_to_position_the_subgraph_node_itself`
