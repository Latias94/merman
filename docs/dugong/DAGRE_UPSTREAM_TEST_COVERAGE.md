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

Source: `repo-ref/dagre/test/parent-dummy-chains-test.js`

- `does not set a parent if both the tail and head have no parent` -> `crates/dugong/tests/parent_dummy_chains_test.rs::parent_dummy_chains_does_not_set_a_parent_if_both_tail_and_head_have_no_parent`
- `uses the tail's parent for the first node if it is not the root` -> `crates/dugong/tests/parent_dummy_chains_test.rs::parent_dummy_chains_uses_the_tails_parent_for_the_first_node_if_it_is_not_the_root`
- `uses the heads's parent for the first node if tail's is root` -> `crates/dugong/tests/parent_dummy_chains_test.rs::parent_dummy_chains_uses_the_heads_parent_for_the_first_node_if_tails_is_root`
- `handles a long chain starting in a subgraph` -> `crates/dugong/tests/parent_dummy_chains_test.rs::parent_dummy_chains_handles_a_long_chain_starting_in_a_subgraph`
- `handles a long chain ending in a subgraph` -> `crates/dugong/tests/parent_dummy_chains_test.rs::parent_dummy_chains_handles_a_long_chain_ending_in_a_subgraph`
- `handles nested subgraphs` -> `crates/dugong/tests/parent_dummy_chains_test.rs::parent_dummy_chains_handles_nested_subgraphs`
- `handles overlapping rank ranges` -> `crates/dugong/tests/parent_dummy_chains_test.rs::parent_dummy_chains_handles_overlapping_rank_ranges`
- `handles an LCA that is not the root of the graph #1` -> `crates/dugong/tests/parent_dummy_chains_test.rs::parent_dummy_chains_handles_an_lca_that_is_not_the_root_1`
- `handles an LCA that is not the root of the graph #2` -> `crates/dugong/tests/parent_dummy_chains_test.rs::parent_dummy_chains_handles_an_lca_that_is_not_the_root_2`

Source: `repo-ref/dagre/test/add-border-segments-test.js`

- `does not add border nodes for a non-compound graph` -> `crates/dugong/tests/add_border_segments_test.rs::add_border_segments_does_not_add_border_nodes_for_a_non_compound_graph`
- `does not add border nodes for a graph with no clusters` -> `crates/dugong/tests/add_border_segments_test.rs::add_border_segments_does_not_add_border_nodes_for_a_graph_with_no_clusters`
- `adds a border for a single-rank subgraph` -> `crates/dugong/tests/add_border_segments_test.rs::add_border_segments_adds_a_border_for_a_single_rank_subgraph`
- `adds a border for a multi-rank subgraph` -> `crates/dugong/tests/add_border_segments_test.rs::add_border_segments_adds_a_border_for_a_multi_rank_subgraph`
- `adds borders for nested subgraphs` -> `crates/dugong/tests/add_border_segments_test.rs::add_border_segments_adds_borders_for_nested_subgraphs`

Source: `repo-ref/dagre/test/util-test.js`

- `copies without change a graph with no multi-edges` -> `crates/dugong/tests/util_test.rs::util_simplify_copies_without_change_a_graph_with_no_multi_edges`
- `collapses multi-edges` -> `crates/dugong/tests/util_test.rs::util_simplify_collapses_multi_edges`
- `copies the graph object` (simplify) -> `crates/dugong/tests/util_test.rs::util_simplify_copies_the_graph_object`
- `copies all nodes` -> `crates/dugong/tests/util_test.rs::util_as_non_compound_graph_copies_all_nodes`
- `copies all edges` -> `crates/dugong/tests/util_test.rs::util_as_non_compound_graph_copies_all_edges`
- `does not copy compound nodes` -> `crates/dugong/tests/util_test.rs::util_as_non_compound_graph_does_not_copy_compound_nodes`
- `copies the graph object` (asNonCompoundGraph) -> `crates/dugong/tests/util_test.rs::util_as_non_compound_graph_copies_the_graph_object`
- `maps a node to its successors with associated weights` -> `crates/dugong/tests/util_test.rs::util_successor_weights_maps_a_node_to_its_successors_with_associated_weights`
- `maps a node to its predecessors with associated weights` -> `crates/dugong/tests/util_test.rs::util_predecessor_weights_maps_a_node_to_its_predecessors_with_associated_weights`
- `creates a slope that will intersect the rectangle's center` -> `crates/dugong/tests/util_test.rs::util_intersect_rect_creates_a_slope_that_will_intersect_the_rectangles_center`
- `touches the border of the rectangle` -> `crates/dugong/tests/util_test.rs::util_intersect_rect_touches_the_border_of_the_rectangle`
- `throws an error if the point is at the center of the rectangle` -> `crates/dugong/tests/util_test.rs::util_intersect_rect_throws_if_the_point_is_at_the_center_of_the_rectangle`
- `creates a matrix based on rank and order of nodes in the graph` -> `crates/dugong/tests/util_test.rs::util_build_layer_matrix_creates_a_matrix_based_on_rank_and_order_of_nodes_in_the_graph`
- `logs timing information` -> `crates/dugong/tests/util_test.rs::util_time_logs_timing_information`
- `returns the value from the evaluated function` -> `crates/dugong/tests/util_test.rs::util_time_returns_the_value_from_the_evaluated_function`
- `adjust ranks such that all are >= 0, and at least one is 0` -> `crates/dugong/tests/util_test.rs::util_normalize_ranks_adjusts_ranks_such_that_all_are_gte_0_and_at_least_one_is_0`
- `works for negative ranks` -> `crates/dugong/tests/util_test.rs::util_normalize_ranks_works_for_negative_ranks`
- `does not assign a rank to subgraphs` -> `crates/dugong/tests/util_test.rs::util_normalize_ranks_does_not_assign_a_rank_to_subgraphs`
- `Removes border ranks without any nodes` -> `crates/dugong/tests/util_test.rs::util_remove_empty_ranks_removes_border_ranks_without_any_nodes`
- `Does not remove non-border ranks` -> `crates/dugong/tests/util_test.rs::util_remove_empty_ranks_does_not_remove_non_border_ranks`
- `Handles parents with undefined ranks` -> `crates/dugong/tests/util_test.rs::util_remove_empty_ranks_handles_parents_with_undefined_ranks`
- `Builds an array to the limit` -> `crates/dugong/tests/util_test.rs::util_range_builds_an_array_to_the_limit`
- `Builds an array with a start` -> `crates/dugong/tests/util_test.rs::util_range_builds_an_array_with_a_start`
- `Builds an array with a negative step` -> `crates/dugong/tests/util_test.rs::util_range_builds_an_array_with_a_negative_step`
- `Creates an object with the same keys` -> `crates/dugong/tests/util_test.rs::util_map_values_creates_an_object_with_the_same_keys`
- `Can take a property name` -> `crates/dugong/tests/util_test.rs::util_map_values_can_take_a_property_name`

Source: `repo-ref/dagre/test/unique-id-test.js`

- `uniqueId(name) generates a valid identifier` -> `crates/dugong/tests/unique_id_test.rs::unique_id_name_generates_a_valid_identifier`
- `Calling uniqueId(name) multiple times generate distinct values` -> `crates/dugong/tests/unique_id_test.rs::unique_id_multiple_calls_generate_distinct_values`
- `Calling uniqueId(number) with a number creates a valid identifier string` -> `crates/dugong/tests/unique_id_test.rs::unique_id_number_prefix_creates_a_valid_identifier_string`

Source: `repo-ref/dagre/test/greedy-fas-test.js`

- `returns the empty set for empty graphs` -> `crates/dugong/tests/greedy_fas_test.rs::greedy_fas_returns_the_empty_set_for_empty_graphs`
- `returns the empty set for single-node graphs` -> `crates/dugong/tests/greedy_fas_test.rs::greedy_fas_returns_the_empty_set_for_single_node_graphs`
- `returns an empty set if the input graph is acyclic` -> `crates/dugong/tests/greedy_fas_test.rs::greedy_fas_returns_an_empty_set_if_the_input_graph_is_acyclic`
- `returns a single edge with a simple cycle` -> `crates/dugong/tests/greedy_fas_test.rs::greedy_fas_returns_a_single_edge_with_a_simple_cycle`
- `returns a single edge in a 4-node cycle` -> `crates/dugong/tests/greedy_fas_test.rs::greedy_fas_returns_a_single_edge_in_a_4_node_cycle`
- `returns two edges for two 4-node cycles` -> `crates/dugong/tests/greedy_fas_test.rs::greedy_fas_returns_two_edges_for_two_4_node_cycles`
- `works with arbitrarily weighted edges` -> `crates/dugong/tests/greedy_fas_test.rs::greedy_fas_works_with_arbitrarily_weighted_edges`
- `works for multigraphs` -> `crates/dugong/tests/greedy_fas_test.rs::greedy_fas_works_for_multigraphs`

Source: `repo-ref/dagre/test/rank/util-test.js`

- `can assign a rank to a single node graph` -> `crates/dugong/tests/rank_util_test.rs::longest_path_can_assign_a_rank_to_a_single_node_graph`
- `can assign ranks to unconnected nodes` -> `crates/dugong/tests/rank_util_test.rs::longest_path_can_assign_ranks_to_unconnected_nodes`
- `can assign ranks to connected nodes` -> `crates/dugong/tests/rank_util_test.rs::longest_path_can_assign_ranks_to_connected_nodes`
- `can assign ranks for a diamond` -> `crates/dugong/tests/rank_util_test.rs::longest_path_can_assign_ranks_for_a_diamond`
- `uses the minlen attribute on the edge` -> `crates/dugong/tests/rank_util_test.rs::longest_path_uses_the_minlen_attribute_on_the_edge`

Source: `repo-ref/dagre/test/rank/feasible-tree-test.js`

- `creates a tree for a trivial input graph` -> `crates/dugong/tests/feasible_tree_test.rs::feasible_tree_creates_a_tree_for_a_trivial_input_graph`
- `correctly shortens slack by pulling a node up` -> `crates/dugong/tests/feasible_tree_test.rs::feasible_tree_correctly_shortens_slack_by_pulling_a_node_up`
- `correctly shortens slack by pulling a node down` -> `crates/dugong/tests/feasible_tree_test.rs::feasible_tree_correctly_shortens_slack_by_pulling_a_node_down`
