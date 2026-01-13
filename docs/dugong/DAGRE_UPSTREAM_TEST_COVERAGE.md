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
- `minimizes the height of subgraphs` → `crates/dugong/tests/layout_test.rs::layout_minimizes_the_height_of_subgraphs`

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

Source: `repo-ref/dagre/test/coordinate-system-test.js`

- `does nothing to node dimensions with rankdir = TB` -> `crates/dugong/tests/coordinate_system_test.rs::coordinate_system_adjust_does_nothing_to_node_dimensions_with_rankdir_tb`
- `does nothing to node dimensions with rankdir = BT` -> `crates/dugong/tests/coordinate_system_test.rs::coordinate_system_adjust_does_nothing_to_node_dimensions_with_rankdir_bt`
- `swaps width and height for nodes with rankdir = LR` -> `crates/dugong/tests/coordinate_system_test.rs::coordinate_system_adjust_swaps_width_and_height_for_nodes_with_rankdir_lr`
- `swaps width and height for nodes with rankdir = RL` -> `crates/dugong/tests/coordinate_system_test.rs::coordinate_system_adjust_swaps_width_and_height_for_nodes_with_rankdir_rl`
- `does nothing to points with rankdir = TB` -> `crates/dugong/tests/coordinate_system_test.rs::coordinate_system_undo_does_nothing_to_points_with_rankdir_tb`
- `flips the y coordinate for points with rankdir = BT` -> `crates/dugong/tests/coordinate_system_test.rs::coordinate_system_undo_flips_the_y_coordinate_for_points_with_rankdir_bt`
- `swaps dimensions and coordinates for points with rankdir = LR` -> `crates/dugong/tests/coordinate_system_test.rs::coordinate_system_undo_swaps_dimensions_and_coordinates_for_points_with_rankdir_lr`
- `swaps dims and coords and flips x for points with rankdir = RL` -> `crates/dugong/tests/coordinate_system_test.rs::coordinate_system_undo_swaps_dims_and_coords_and_flips_x_for_points_with_rankdir_rl`
- `does nothing to node dimensions with rankdir = TB` → `crates/dugong/tests/coordinate_system_test.rs::coordinate_system_adjust_does_nothing_to_node_dimensions_with_rankdir_tb`
- `does nothing to node dimensions with rankdir = BT` → `crates/dugong/tests/coordinate_system_test.rs::coordinate_system_adjust_does_nothing_to_node_dimensions_with_rankdir_bt`
- `swaps width and height for nodes with rankdir = LR` → `crates/dugong/tests/coordinate_system_test.rs::coordinate_system_adjust_swaps_width_and_height_for_nodes_with_rankdir_lr`
- `swaps width and height for nodes with rankdir = RL` → `crates/dugong/tests/coordinate_system_test.rs::coordinate_system_adjust_swaps_width_and_height_for_nodes_with_rankdir_rl`
- `does nothing to points with rankdir = TB` → `crates/dugong/tests/coordinate_system_test.rs::coordinate_system_undo_does_nothing_to_points_with_rankdir_tb`
- `flips the y coordinate for points with rankdir = BT` → `crates/dugong/tests/coordinate_system_test.rs::coordinate_system_undo_flips_the_y_coordinate_for_points_with_rankdir_bt`
- `swaps dimensions and coordinates for points with rankdir = LR` → `crates/dugong/tests/coordinate_system_test.rs::coordinate_system_undo_swaps_dimensions_and_coordinates_for_points_with_rankdir_lr`
- `swaps dims and coords and flips x for points with rankdir = RL` → `crates/dugong/tests/coordinate_system_test.rs::coordinate_system_undo_swaps_dims_and_coords_and_flips_x_for_points_with_rankdir_rl`

Source: `repo-ref/dagre/test/acyclic-test.js`

- `does not change an already acyclic graph` → `crates/dugong/tests/acyclic_test.rs::acyclic_run_does_not_change_an_already_acyclic_graph`
- `breaks cycles in the input graph` → `crates/dugong/tests/acyclic_test.rs::acyclic_run_breaks_cycles_in_the_input_graph`
- `creates a multi-edge where necessary` → `crates/dugong/tests/acyclic_test.rs::acyclic_run_creates_a_multi_edge_where_necessary`
- `does not change edges where the original graph was acyclic` → `crates/dugong/tests/acyclic_test.rs::acyclic_undo_does_not_change_edges_where_the_original_graph_was_acyclic`
- `can restore previosuly reversed edges` → `crates/dugong/tests/acyclic_test.rs::acyclic_undo_can_restore_previously_reversed_edges`
- `prefers to break cycles at low-weight edges` → `crates/dugong/tests/acyclic_test.rs::acyclic_greedy_prefers_to_break_cycles_at_low_weight_edges`

Source: `repo-ref/dagre/test/normalize-test.js`

- `does not change a short edge` -> `crates/dugong/tests/normalize_test.rs::normalize_run_does_not_change_a_short_edge`
- `splits a two layer edge into two segments` -> `crates/dugong/tests/normalize_test.rs::normalize_run_splits_a_two_layer_edge_into_two_segments`
- `assigns width = 0, height = 0 to dummy nodes by default` -> `crates/dugong/tests/normalize_test.rs::normalize_run_assigns_width_and_height_0_to_dummy_nodes_by_default`
- `assigns width and height from the edge for the node on labelRank` -> `crates/dugong/tests/normalize_test.rs::normalize_run_assigns_width_and_height_from_the_edge_for_the_node_on_label_rank`
- `preserves the weight for the edge` -> `crates/dugong/tests/normalize_test.rs::normalize_run_preserves_the_weight_for_the_edge`
- `reverses the run operation` -> `crates/dugong/tests/normalize_test.rs::normalize_undo_reverses_the_run_operation`
- `restores previous edge labels` -> `crates/dugong/tests/normalize_test.rs::normalize_undo_restores_previous_edge_labels`
- `collects assigned coordinates into the 'points' attribute` -> `crates/dugong/tests/normalize_test.rs::normalize_undo_collects_assigned_coordinates_into_points`
- `merges assigned coordinates into the 'points' attribute` -> `crates/dugong/tests/normalize_test.rs::normalize_undo_merges_assigned_coordinates_into_points`
- `sets coords and dims for the label, if the edge has one` -> `crates/dugong/tests/normalize_test.rs::normalize_undo_sets_coords_and_dims_for_the_label_if_the_edge_has_one`
- `sets coords and dims for the label, if the long edge has one` -> `crates/dugong/tests/normalize_test.rs::normalize_undo_sets_coords_and_dims_for_the_label_if_the_long_edge_has_one`
- `restores multi-edges` -> `crates/dugong/tests/normalize_test.rs::normalize_undo_restores_multi_edges`
