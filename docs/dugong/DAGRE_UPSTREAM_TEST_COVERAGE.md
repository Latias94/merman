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

Source: `repo-ref/dagre/test/rank/network-simplex-test.js`

- `can assign a rank to a single node` -> `crates/dugong/tests/network_simplex_test.rs::network_simplex_can_assign_a_rank_to_a_single_node`
- `can assign a rank to a 2-node connected graph` -> `crates/dugong/tests/network_simplex_test.rs::network_simplex_can_assign_a_rank_to_a_2_node_connected_graph`
- `can assign ranks for a diamond` -> `crates/dugong/tests/network_simplex_test.rs::network_simplex_can_assign_ranks_for_a_diamond`
- `uses the minlen attribute on the edge` -> `crates/dugong/tests/network_simplex_test.rs::network_simplex_uses_the_minlen_attribute_on_the_edge`
- `can rank the gansner graph` -> `crates/dugong/tests/network_simplex_test.rs::network_simplex_can_rank_the_gansner_graph`
- `can handle multi-edges` -> `crates/dugong/tests/network_simplex_test.rs::network_simplex_can_handle_multi_edges`
- `leaveEdge returns undefined if there is no edge with a negative cutvalue` -> `crates/dugong/tests/network_simplex_test.rs::leave_edge_returns_none_if_there_is_no_edge_with_a_negative_cutvalue`
- `leaveEdge returns an edge if one is found with a negative cutvalue` -> `crates/dugong/tests/network_simplex_test.rs::leave_edge_returns_an_edge_if_one_is_found_with_a_negative_cutvalue`
- `enterEdge finds an edge from the head to tail component` -> `crates/dugong/tests/network_simplex_test.rs::enter_edge_finds_an_edge_from_the_head_to_tail_component`
- `enterEdge works when the root of the tree is in the tail component` -> `crates/dugong/tests/network_simplex_test.rs::enter_edge_works_when_the_root_of_the_tree_is_in_the_tail_component`
- `enterEdge finds the edge with the least slack` -> `crates/dugong/tests/network_simplex_test.rs::enter_edge_finds_the_edge_with_the_least_slack`
- `enterEdge finds an appropriate edge for gansner graph #1` -> `crates/dugong/tests/network_simplex_test.rs::enter_edge_finds_an_appropriate_edge_for_gansner_graph_1`
- `enterEdge finds an appropriate edge for gansner graph #2` -> `crates/dugong/tests/network_simplex_test.rs::enter_edge_finds_an_appropriate_edge_for_gansner_graph_2`
- `enterEdge finds an appropriate edge for gansner graph #3` -> `crates/dugong/tests/network_simplex_test.rs::enter_edge_finds_an_appropriate_edge_for_gansner_graph_3`
- `enterEdge finds an appropriate edge for gansner graph #4` -> `crates/dugong/tests/network_simplex_test.rs::enter_edge_finds_an_appropriate_edge_for_gansner_graph_4`
- `initLowLimValues assigns low, lim, and parent for each node in a tree` -> `crates/dugong/tests/network_simplex_test.rs::init_low_lim_values_assigns_low_lim_and_parent_for_each_node_in_a_tree`
- `exchangeEdges exchanges edges and updates cut values and low/lim numbers` -> `crates/dugong/tests/network_simplex_test.rs::exchange_edges_exchanges_edges_and_updates_cut_values_and_low_lim_numbers`
- `exchangeEdges updates ranks` -> `crates/dugong/tests/network_simplex_test.rs::exchange_edges_updates_ranks`
- `calcCutValue works for a 2-node tree with c -> p` -> `crates/dugong/tests/network_simplex_test.rs::calc_cut_value_works_for_a_2_node_tree_with_c_to_p`
- `calcCutValue works for a 2-node tree with c <- p` -> `crates/dugong/tests/network_simplex_test.rs::calc_cut_value_works_for_a_2_node_tree_with_c_from_p`
- `calcCutValue works for 3-node tree with gc -> c -> p` -> `crates/dugong/tests/network_simplex_test.rs::calc_cut_value_works_for_3_node_tree_with_gc_to_c_to_p`
- `calcCutValue works for 3-node tree with gc -> c <- p` -> `crates/dugong/tests/network_simplex_test.rs::calc_cut_value_works_for_3_node_tree_with_gc_to_c_from_p`
- `calcCutValue works for 3-node tree with gc <- c -> p` -> `crates/dugong/tests/network_simplex_test.rs::calc_cut_value_works_for_3_node_tree_with_gc_from_c_to_p`
- `calcCutValue works for 3-node tree with gc <- c <- p` -> `crates/dugong/tests/network_simplex_test.rs::calc_cut_value_works_for_3_node_tree_with_gc_from_c_from_p`
- `calcCutValue works for 4-node tree with gc -> c -> p -> o, with o -> c` -> `crates/dugong/tests/network_simplex_test.rs::calc_cut_value_works_for_4_node_tree_gc_to_c_to_p_to_o_with_o_to_c`
- `calcCutValue works for 4-node tree with gc -> c -> p -> o, with o <- c` -> `crates/dugong/tests/network_simplex_test.rs::calc_cut_value_works_for_4_node_tree_gc_to_c_to_p_to_o_with_o_from_c`
- `calcCutValue works for 4-node tree with o -> gc -> c -> p, with o -> c` -> `crates/dugong/tests/network_simplex_test.rs::calc_cut_value_works_for_4_node_tree_o_to_gc_to_c_to_p_with_o_to_c`
- `calcCutValue works for 4-node tree with o -> gc -> c -> p, with o <- c` -> `crates/dugong/tests/network_simplex_test.rs::calc_cut_value_works_for_4_node_tree_o_to_gc_to_c_to_p_with_o_from_c`
- `calcCutValue works for 4-node tree with gc -> c <- p -> o, with o -> c` -> `crates/dugong/tests/network_simplex_test.rs::calc_cut_value_works_for_4_node_tree_gc_to_c_from_p_to_o_with_o_to_c`
- `calcCutValue works for 4-node tree with gc -> c <- p -> o, with o <- c` -> `crates/dugong/tests/network_simplex_test.rs::calc_cut_value_works_for_4_node_tree_gc_to_c_from_p_to_o_with_o_from_c`
- `calcCutValue works for 4-node tree with o -> gc -> c <- p, with o -> c` -> `crates/dugong/tests/network_simplex_test.rs::calc_cut_value_works_for_4_node_tree_o_to_gc_to_c_from_p_with_o_to_c`
- `calcCutValue works for 4-node tree with o -> gc -> c <- p, with o <- c` -> `crates/dugong/tests/network_simplex_test.rs::calc_cut_value_works_for_4_node_tree_o_to_gc_to_c_from_p_with_o_from_c`
- `initCutValues works for gansnerGraph` -> `crates/dugong/tests/network_simplex_test.rs::init_cut_values_works_for_gansner_graph`
- `initCutValues works for updated gansnerGraph` -> `crates/dugong/tests/network_simplex_test.rs::init_cut_values_works_for_updated_gansner_graph`

Source: `repo-ref/dagre/test/rank/rank-test.js`

- `longest-path respects the minlen attribute` -> `crates/dugong/tests/rank_test.rs::rank_longest_path_respects_the_minlen_attribute`
- `tight-tree respects the minlen attribute` -> `crates/dugong/tests/rank_test.rs::rank_tight_tree_respects_the_minlen_attribute`
- `network-simplex respects the minlen attribute` -> `crates/dugong/tests/rank_test.rs::rank_network_simplex_respects_the_minlen_attribute`
- `unknown-should-still-work respects the minlen attribute` -> `crates/dugong/tests/rank_test.rs::rank_unknown_should_still_work_respects_the_minlen_attribute`
- `can rank a single node graph (all rankers)` -> `crates/dugong/tests/rank_test.rs::rank_can_rank_a_single_node_graph_for_each_ranker`

Source: `repo-ref/dagre/test/order/build-layer-graph-test.js`

- `places movable nodes with no parents under the root node` -> `crates/dugong/tests/order_build_layer_graph_test.rs::build_layer_graph_places_movable_nodes_with_no_parents_under_the_root_node`
- `copies flat nodes from the layer to the graph` -> `crates/dugong/tests/order_build_layer_graph_test.rs::build_layer_graph_copies_flat_nodes_from_the_layer_to_the_graph`
- `uses the original node label for copied nodes` -> `crates/dugong/tests/order_build_layer_graph_test.rs::build_layer_graph_uses_the_original_node_label_for_copied_nodes`
- `copies edges incident on rank nodes to the graph (inEdges)` -> `crates/dugong/tests/order_build_layer_graph_test.rs::build_layer_graph_copies_edges_incident_on_rank_nodes_to_the_graph_in_edges`
- `copies edges incident on rank nodes to the graph (outEdges)` -> `crates/dugong/tests/order_build_layer_graph_test.rs::build_layer_graph_copies_edges_incident_on_rank_nodes_to_the_graph_out_edges`
- `collapses multi-edges` -> `crates/dugong/tests/order_build_layer_graph_test.rs::build_layer_graph_collapses_multi_edges`
- `preserves hierarchy for the movable layer` -> `crates/dugong/tests/order_build_layer_graph_test.rs::build_layer_graph_preserves_hierarchy_for_the_movable_layer`

Source: `repo-ref/dagre/test/order/barycenter-test.js`

- `assigns an undefined barycenter for a node with no predecessors` -> `crates/dugong/tests/order_barycenter_test.rs::barycenter_assigns_an_undefined_barycenter_for_a_node_with_no_predecessors`
- `assigns the position of the sole predecessors` -> `crates/dugong/tests/order_barycenter_test.rs::barycenter_assigns_the_position_of_the_sole_predecessors`
- `assigns the average of multiple predecessors` -> `crates/dugong/tests/order_barycenter_test.rs::barycenter_assigns_the_average_of_multiple_predecessors`
- `takes into account the weight of edges` -> `crates/dugong/tests/order_barycenter_test.rs::barycenter_takes_into_account_the_weight_of_edges`
- `calculates barycenters for all nodes in the movable layer` -> `crates/dugong/tests/order_barycenter_test.rs::barycenter_calculates_barycenters_for_all_nodes_in_the_movable_layer`

Source: `repo-ref/dagre/test/order/resolve-conflicts-test.js`

- `returns back nodes unchanged when no constraints exist` -> `crates/dugong/tests/order_resolve_conflicts_test.rs::resolve_conflicts_returns_back_nodes_unchanged_when_no_constraints_exist`
- `returns back nodes unchanged when no conflicts exist` -> `crates/dugong/tests/order_resolve_conflicts_test.rs::resolve_conflicts_returns_back_nodes_unchanged_when_no_conflicts_exist`
- `coalesces nodes when there is a conflict` -> `crates/dugong/tests/order_resolve_conflicts_test.rs::resolve_conflicts_coalesces_nodes_when_there_is_a_conflict`
- `coalesces nodes when there is a conflict #2` -> `crates/dugong/tests/order_resolve_conflicts_test.rs::resolve_conflicts_coalesces_nodes_when_there_is_a_conflict_2`
- `works with multiple constraints for the same target #1` -> `crates/dugong/tests/order_resolve_conflicts_test.rs::resolve_conflicts_works_with_multiple_constraints_for_the_same_target_1`
- `works with multiple constraints for the same target #2` -> `crates/dugong/tests/order_resolve_conflicts_test.rs::resolve_conflicts_works_with_multiple_constraints_for_the_same_target_2`
- `does nothing to a node lacking both a barycenter and a constraint` -> `crates/dugong/tests/order_resolve_conflicts_test.rs::resolve_conflicts_does_nothing_to_a_node_lacking_both_a_barycenter_and_a_constraint`
- `treats a node w/o a barycenter as always violating constraints #1` -> `crates/dugong/tests/order_resolve_conflicts_test.rs::resolve_conflicts_treats_a_node_without_a_barycenter_as_always_violating_constraints_1`
- `treats a node w/o a barycenter as always violating constraints #2` -> `crates/dugong/tests/order_resolve_conflicts_test.rs::resolve_conflicts_treats_a_node_without_a_barycenter_as_always_violating_constraints_2`
- `ignores edges not related to entries` -> `crates/dugong/tests/order_resolve_conflicts_test.rs::resolve_conflicts_ignores_edges_not_related_to_entries`

Source: `repo-ref/dagre/test/order/sort-test.js`

- `sorts nodes by barycenter` -> `crates/dugong/tests/order_sort_test.rs::sort_sorts_nodes_by_barycenter`
- `can sort super-nodes` -> `crates/dugong/tests/order_sort_test.rs::sort_can_sort_super_nodes`
- `biases to the left by default` -> `crates/dugong/tests/order_sort_test.rs::sort_biases_to_the_left_by_default`
- `biases to the right if biasRight = true` -> `crates/dugong/tests/order_sort_test.rs::sort_biases_to_the_right_if_bias_right_is_true`
- `can sort nodes without a barycenter` -> `crates/dugong/tests/order_sort_test.rs::sort_can_sort_nodes_without_a_barycenter`
- `can handle no barycenters for any nodes` -> `crates/dugong/tests/order_sort_test.rs::sort_can_handle_no_barycenters_for_any_nodes`
- `can handle a barycenter of 0` -> `crates/dugong/tests/order_sort_test.rs::sort_can_handle_a_barycenter_of_0`

Source: `repo-ref/dagre/test/order/sort-subgraph-test.js`

- `sorts a flat subgraph based on barycenter` -> `crates/dugong/tests/order_sort_subgraph_test.rs::sort_subgraph_sorts_a_flat_subgraph_based_on_barycenter`
- `preserves the pos of a node (y) w/o neighbors in a flat subgraph` -> `crates/dugong/tests/order_sort_subgraph_test.rs::sort_subgraph_preserves_the_pos_of_a_node_without_neighbors_in_a_flat_subgraph`
- `biases to the left without reverse bias` -> `crates/dugong/tests/order_sort_subgraph_test.rs::sort_subgraph_biases_to_the_left_without_reverse_bias`
- `biases to the right with reverse bias` -> `crates/dugong/tests/order_sort_subgraph_test.rs::sort_subgraph_biases_to_the_right_with_reverse_bias`
- `aggregates stats about the subgraph` -> `crates/dugong/tests/order_sort_subgraph_test.rs::sort_subgraph_aggregates_stats_about_the_subgraph`
- `can sort a nested subgraph with no barycenter` -> `crates/dugong/tests/order_sort_subgraph_test.rs::sort_subgraph_can_sort_a_nested_subgraph_with_no_barycenter`
- `can sort a nested subgraph with a barycenter` -> `crates/dugong/tests/order_sort_subgraph_test.rs::sort_subgraph_can_sort_a_nested_subgraph_with_a_barycenter`
- `can sort a nested subgraph with no in-edges` -> `crates/dugong/tests/order_sort_subgraph_test.rs::sort_subgraph_can_sort_a_nested_subgraph_with_no_in_edges`
- `sorts border nodes to the extremes of the subgraph` -> `crates/dugong/tests/order_sort_subgraph_test.rs::sort_subgraph_sorts_border_nodes_to_the_extremes_of_the_subgraph`
- `assigns a barycenter to a subgraph based on previous border nodes` -> `crates/dugong/tests/order_sort_subgraph_test.rs::sort_subgraph_assigns_a_barycenter_to_a_subgraph_based_on_previous_border_nodes`

Source: `repo-ref/dagre/test/order/add-subgraph-constraints-test.js`

- `does not change CG for a flat set of nodes` -> `crates/dugong/tests/order_add_subgraph_constraints_test.rs::add_subgraph_constraints_does_not_change_cg_for_a_flat_set_of_nodes`
- `doesn't create a constraint for contiguous subgraph nodes` -> `crates/dugong/tests/order_add_subgraph_constraints_test.rs::add_subgraph_constraints_does_not_create_a_constraint_for_contiguous_subgraph_nodes`
- `adds a constraint when the parents for adjacent nodes are different` -> `crates/dugong/tests/order_add_subgraph_constraints_test.rs::add_subgraph_constraints_adds_a_constraint_when_parents_for_adjacent_nodes_are_different`
- `works for multiple levels` -> `crates/dugong/tests/order_add_subgraph_constraints_test.rs::add_subgraph_constraints_works_for_multiple_levels`

Source: `repo-ref/dagre/test/order/init-order-test.js`

- `assigns non-overlapping orders for each rank in a tree` -> `crates/dugong/tests/order_init_order_test.rs::init_order_assigns_non_overlapping_orders_for_each_rank_in_a_tree`
- `assigns non-overlapping orders for each rank in a DAG` -> `crates/dugong/tests/order_init_order_test.rs::init_order_assigns_non_overlapping_orders_for_each_rank_in_a_dag`
- `does not assign an order to subgraph nodes` -> `crates/dugong/tests/order_init_order_test.rs::init_order_does_not_assign_an_order_to_subgraph_nodes`

Source: `repo-ref/dagre/test/order/cross-count-test.js`

- `returns 0 for an empty layering` -> `crates/dugong/tests/order_cross_count_test.rs::cross_count_returns_0_for_an_empty_layering`
- `returns 0 for a layering with no crossings` -> `crates/dugong/tests/order_cross_count_test.rs::cross_count_returns_0_for_a_layering_with_no_crossings`
- `returns 1 for a layering with 1 crossing` -> `crates/dugong/tests/order_cross_count_test.rs::cross_count_returns_1_for_a_layering_with_1_crossing`
- `returns a weighted crossing count for a layering with 1 crossing` -> `crates/dugong/tests/order_cross_count_test.rs::cross_count_returns_a_weighted_crossing_count_for_a_layering_with_1_crossing`
- `calculates crossings across layers` -> `crates/dugong/tests/order_cross_count_test.rs::cross_count_calculates_crossings_across_layers`
- `works for graph #1` -> `crates/dugong/tests/order_cross_count_test.rs::cross_count_works_for_graph_1`

Source: `repo-ref/dagre/test/order/order-test.js`

- `does not add crossings to a tree structure` -> `crates/dugong/tests/order_order_test.rs::order_does_not_add_crossings_to_a_tree_structure`
- `can solve a simple graph` -> `crates/dugong/tests/order_order_test.rs::order_can_solve_a_simple_graph`
- `can minimize crossings` -> `crates/dugong/tests/order_order_test.rs::order_can_minimize_crossings`
- `can skip the optimal ordering` -> `crates/dugong/tests/order_order_test.rs::order_can_skip_the_optimal_ordering`

Source: `repo-ref/dagre/test/data/list-test.js`

- `returns undefined with an empty list` -> `crates/dugong/tests/data_list_test.rs::list_dequeue_returns_none_with_an_empty_list`
- `unlinks and returns the first entry` -> `crates/dugong/tests/data_list_test.rs::list_dequeue_unlinks_and_returns_the_first_entry`
- `unlinks and returns multiple entries in FIFO order` -> `crates/dugong/tests/data_list_test.rs::list_dequeue_unlinks_and_returns_multiple_entries_in_fifo_order`
- `unlinks and relinks an entry if it is re-enqueued` -> `crates/dugong/tests/data_list_test.rs::list_dequeue_unlinks_and_relinks_an_entry_if_it_is_re_enqueued`
- `unlinks and relinks an entry if it is enqueued on another list` -> `crates/dugong/tests/data_list_test.rs::list_dequeue_unlinks_and_relinks_an_entry_if_it_is_enqueued_on_another_list`
- `can return a string representation` -> `crates/dugong/tests/data_list_test.rs::list_can_return_a_string_representation`

Source: `repo-ref/dagre/test/version-test.js`

- `should match the version from package.json` -> `crates/dugong/tests/version_test.rs::version_matches_cargo_pkg_version`

Source: `repo-ref/dagre/test/bundle-test.js`

- `exports dagre` -> `crates/dugong/tests/bundle_test.rs::bundle_exports_expected_symbols`
- `can do trivial layout` -> `crates/dugong/tests/bundle_test.rs::bundle_can_do_trivial_layout`

Source: `repo-ref/dagre/test/position/bk-test.js`

- `does not mark edges that have no conflict` -> `crates/dugong/tests/position_bk_test.rs::find_type1_conflicts_does_not_mark_edges_that_have_no_conflict`
- `does not mark type-0 conflicts (no dummies)` -> `crates/dugong/tests/position_bk_test.rs::find_type1_conflicts_does_not_mark_type_0_conflicts_no_dummies`
- `does not mark type-0 conflicts (a is dummy)` -> `crates/dugong/tests/position_bk_test.rs::find_type1_conflicts_does_not_mark_type_0_conflicts_a_is_dummy`
- `does not mark type-0 conflicts (b is dummy)` -> `crates/dugong/tests/position_bk_test.rs::find_type1_conflicts_does_not_mark_type_0_conflicts_b_is_dummy`
- `does not mark type-0 conflicts (c is dummy)` -> `crates/dugong/tests/position_bk_test.rs::find_type1_conflicts_does_not_mark_type_0_conflicts_c_is_dummy`
- `does not mark type-0 conflicts (d is dummy)` -> `crates/dugong/tests/position_bk_test.rs::find_type1_conflicts_does_not_mark_type_0_conflicts_d_is_dummy`
- `does mark type-1 conflicts (a is non-dummy)` -> `crates/dugong/tests/position_bk_test.rs::find_type1_conflicts_does_mark_type_1_conflicts_a_is_non_dummy`
- `does mark type-1 conflicts (b is non-dummy)` -> `crates/dugong/tests/position_bk_test.rs::find_type1_conflicts_does_mark_type_1_conflicts_b_is_non_dummy`
- `does mark type-1 conflicts (c is non-dummy)` -> `crates/dugong/tests/position_bk_test.rs::find_type1_conflicts_does_mark_type_1_conflicts_c_is_non_dummy`
- `does mark type-1 conflicts (d is non-dummy)` -> `crates/dugong/tests/position_bk_test.rs::find_type1_conflicts_does_mark_type_1_conflicts_d_is_non_dummy`
- `does not mark type-2 conflicts (all dummies)` -> `crates/dugong/tests/position_bk_test.rs::find_type1_conflicts_does_not_mark_type_2_conflicts_all_dummies`
- `marks type-2 conflicts favoring border segments #1` -> `crates/dugong/tests/position_bk_test.rs::find_type2_conflicts_marks_type_2_conflicts_favoring_border_segments_1`
- `marks type-2 conflicts favoring border segments #2` -> `crates/dugong/tests/position_bk_test.rs::find_type2_conflicts_marks_type_2_conflicts_favoring_border_segments_2`
- `can test for a type-1 conflict regardless of edge orientation` -> `crates/dugong/tests/position_bk_test.rs::has_conflict_can_test_for_a_type_1_conflict_regardless_of_edge_orientation`
- `works for multiple conflicts with the same node` -> `crates/dugong/tests/position_bk_test.rs::has_conflict_works_for_multiple_conflicts_with_the_same_node`
- `Aligns with itself if the node has no adjacencies` -> `crates/dugong/tests/position_bk_test.rs::vertical_alignment_aligns_with_itself_if_the_node_has_no_adjacencies`
- `Aligns with its sole adjacency` -> `crates/dugong/tests/position_bk_test.rs::vertical_alignment_aligns_with_its_sole_adjacency`
- `aligns with its left median when possible` -> `crates/dugong/tests/position_bk_test.rs::vertical_alignment_aligns_with_its_left_median_when_possible`
- `aligns correctly even regardless of node name / insertion order` -> `crates/dugong/tests/position_bk_test.rs::vertical_alignment_aligns_correctly_regardless_of_node_name_or_insertion_order`
- `aligns with its right median when left is unavailable` -> `crates/dugong/tests/position_bk_test.rs::vertical_alignment_aligns_with_its_right_median_when_left_is_unavailable`
- `aligns with neither median if both are unavailable` -> `crates/dugong/tests/position_bk_test.rs::vertical_alignment_aligns_with_neither_median_if_both_are_unavailable`
- `aligns with the single median for an odd number of adjacencies` -> `crates/dugong/tests/position_bk_test.rs::vertical_alignment_aligns_with_the_single_median_for_an_odd_number_of_adjacencies`
- `aligns blocks across multiple layers` -> `crates/dugong/tests/position_bk_test.rs::vertical_alignment_aligns_blocks_across_multiple_layers`
- `places the center of a single node graph at origin (0,0)` -> `crates/dugong/tests/position_bk_test.rs::horizontal_compaction_places_the_center_of_a_single_node_graph_at_origin`
- `separates adjacent nodes by specified node separation` -> `crates/dugong/tests/position_bk_test.rs::horizontal_compaction_separates_adjacent_nodes_by_specified_node_separation`
- `separates adjacent edges by specified node separation` -> `crates/dugong/tests/position_bk_test.rs::horizontal_compaction_separates_adjacent_edges_by_specified_edge_separation`
- `aligns the centers of nodes in the same block` -> `crates/dugong/tests/position_bk_test.rs::horizontal_compaction_aligns_the_centers_of_nodes_in_the_same_block`
- `separates blocks with the appropriate separation` -> `crates/dugong/tests/position_bk_test.rs::horizontal_compaction_separates_blocks_with_the_appropriate_separation`
- `separates classes with the appropriate separation` -> `crates/dugong/tests/position_bk_test.rs::horizontal_compaction_separates_classes_with_the_appropriate_separation`
- `shifts classes by max sep from the adjacent block #1` -> `crates/dugong/tests/position_bk_test.rs::horizontal_compaction_shifts_classes_by_max_sep_from_the_adjacent_block_1`
- `shifts classes by max sep from the adjacent block #2` -> `crates/dugong/tests/position_bk_test.rs::horizontal_compaction_shifts_classes_by_max_sep_from_the_adjacent_block_2`
- `cascades class shift` -> `crates/dugong/tests/position_bk_test.rs::horizontal_compaction_cascades_class_shift`
- `handles labelpos = l` -> `crates/dugong/tests/position_bk_test.rs::horizontal_compaction_handles_labelpos_l`
- `handles labelpos = c` -> `crates/dugong/tests/position_bk_test.rs::horizontal_compaction_handles_labelpos_c`
- `handles labelpos = r` -> `crates/dugong/tests/position_bk_test.rs::horizontal_compaction_handles_labelpos_r`
- `aligns a single node` -> `crates/dugong/tests/position_bk_test.rs::align_coordinates_aligns_a_single_node`
- `aligns multiple nodes` -> `crates/dugong/tests/position_bk_test.rs::align_coordinates_aligns_multiple_nodes`
- `finds the alignment with the smallest width` -> `crates/dugong/tests/position_bk_test.rs::find_smallest_width_alignment_finds_the_alignment_with_the_smallest_width`
- `takes node width into account` -> `crates/dugong/tests/position_bk_test.rs::find_smallest_width_alignment_takes_node_width_into_account`
- `aligns a single node to the shared median value` -> `crates/dugong/tests/position_bk_test.rs::balance_aligns_a_single_node_to_the_shared_median_value`
- `aligns a single node to the average of different median values` -> `crates/dugong/tests/position_bk_test.rs::balance_aligns_a_single_node_to_the_average_of_different_median_values`
- `balances multiple nodes` -> `crates/dugong/tests/position_bk_test.rs::balance_balances_multiple_nodes`
- `positions a single node at origin` -> `crates/dugong/tests/position_bk_test.rs::position_x_positions_a_single_node_at_origin`
- `positions a single node block at origin` -> `crates/dugong/tests/position_bk_test.rs::position_x_positions_a_single_node_block_at_origin`
- `positions a single node block at origin even when their sizes differ` -> `crates/dugong/tests/position_bk_test.rs::position_x_positions_a_single_node_block_at_origin_even_when_their_sizes_differ`
- `centers a node if it is a predecessor of two same sized nodes` -> `crates/dugong/tests/position_bk_test.rs::position_x_centers_a_node_if_it_is_a_predecessor_of_two_same_sized_nodes`
- `shifts blocks on both sides of aligned block` -> `crates/dugong/tests/position_bk_test.rs::position_x_shifts_blocks_on_both_sides_of_aligned_block`
- `aligns inner segments` -> `crates/dugong/tests/position_bk_test.rs::position_x_aligns_inner_segments`
