# Graph Fixture Parity Gaps

This file records copied `mermaid-ascii` graph fixtures that remain useful semantic evidence but no
longer match byte-for-byte after Merman adopted Mermaid-backed Dagre ranking, compound ownership,
and occupancy-aware routing. The copied fixture bytes remain immutable. The executable corpus and
gap inventory live in `tests/graph_fixture.rs`.

## Current Status

- Copied corpus: 79 fixtures from the pinned `mermaid-ascii` source boundary.
- Exact-output subset: 45 fixtures, derived as `GRAPH_FIXTURE_CORPUS - GRAPH_FIXTURE_GAPS`.
- Named intentional differences: 34 fixtures.
- Every exact fixture must still match byte-for-byte.
- Every named gap must still render successfully; parser-backed semantic tests own topology,
  endpoint, label, direction, and compound-boundary behavior.

The gap classification does not authorize missing nodes, edges, labels, or groups. It records only
deterministic layout and route-shape differences from the narrow copied renderer. Resource errors or
silent semantic loss are regressions, not acceptable gaps.

## Why These Differ

- Dagre-compatible rank assignment can choose different rows and declaration-order dispositions
  than the copied renderer's local placement rules.
- The bounded A* router may choose a different equal-cost orthogonal path while retaining the same
  endpoints and reachability.
- Compound routes use explicit first-parent ownership and protected group borders instead of the
  copied renderer's looser subgraph geometry.
- Labels and junctions are placed through planner-owned occupancy, so spacing can differ when the
  copied output relied on post-render overlays.

## Named Gaps

ASCII:

- `ampersand_lhs.txt`
- `ampersand_lhs_and_rhs.txt`
- `ampersand_rhs.txt`
- `back_reference_from_child.txt`
- `backlink_from_bottom.txt`
- `backlink_from_top.txt`
- `backlink_with_short_y_padding.txt`
- `back_edges_two_labels_td.txt`
- `bidirectional_edge_labels_lr.txt`
- `bidirectional_edge_labels_td.txt`
- `comments.txt`
- `duplicate_edge_labels.txt`
- `preserve_order_of_definition.txt`
- `subgraph_complex_mixed.txt`
- `subgraph_empty.txt`
- `subgraph_mixed_nodes_td.txt`
- `subgraph_multiple_edges.txt`
- `subgraph_node_outside_lr.txt`
- `subgraph_with_labels.txt`
- `tight_arrow_mixed.txt`
- `two_layer_single_graph.txt`
- `two_layer_single_graph_longer_names.txt`

Unicode:

- `ampersand_lhs.txt`
- `ampersand_lhs_and_rhs.txt`
- `ampersand_rhs.txt`
- `back_reference_from_child.txt`
- `backlink_from_bottom.txt`
- `backlink_from_top.txt`
- `back_edges_two_labels_td.txt`
- `comments.txt`
- `preserve_order_of_definition.txt`
- `tight_arrow_mixed.txt`
- `two_layer_single_graph_longer_names.txt`
- `two_layer_single_graph.txt`

## Executable Evidence

- `graph_fixture_exact_subset_matches_upstream` protects the remaining byte oracle.
- `graph_fixture_named_gaps_remain_renderable` prevents named differences from hiding render
  failures.
- `flowchart_local_semantic_fixture_covers_ampersand_fanin_and_fanout` covers ampersand topology.
- `flowchart_parser_cross_subgraph_routes_follow_compound_parent_topology` covers legal group-border
  crossing and repeated-node first-parent ownership.
- The focused Flowchart suite covers back edges, parallel labels, directions, subgraphs, endpoint
  markers, route reachability, terminal safety, and resource limits independently of copied spacing.
