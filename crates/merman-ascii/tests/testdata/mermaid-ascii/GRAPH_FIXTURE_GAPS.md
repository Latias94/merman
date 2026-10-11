# Graph Fixture Parity Gaps

This file records copied `mermaid-ascii` graph fixtures that remain useful semantic evidence but no
longer match byte-for-byte after Merman adopted Mermaid-backed Dagre ranking, compound ownership,
and occupancy-aware routing. The copied fixture bytes remain immutable. The executable corpus and
gap inventory live in `tests/graph_fixture.rs`.

## Current Status

- Copied corpus: 79 fixtures from the pinned `mermaid-ascii` source boundary.
- Exact-output subset: 33 fixtures, derived as the corpus minus named gaps and corrections.
- Strict corrected-output subset: 7 fixtures, listed in `GRAPH_FIXTURE_CORRECTIONS`.
- Named intentional layout differences: 39 fixtures.
- Every exact fixture must still match byte-for-byte.
- Every named gap must still render successfully and retain every non-marker visible text token
  from the copied output; parser-backed semantic tests own topology, endpoint, direction, and
  compound-boundary behavior.

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
- Mermaid 11.16.1 lays out isolated subgraphs without an explicit `direction` on the axis
  perpendicular to their effective parent direction. The copied renderer inherits the root axis;
  Merman follows the pinned Mermaid source instead.
- Labels and junctions are placed through planner-owned occupancy, so spacing can differ when the
  copied output relied on post-render overlays.

## Named Corrected Oracles

Seven copied ASCII outputs erase a compound frame at a legal edge crossing: horizontal routes
replace a vertical frame segment with `-`, or vertical routes replace a horizontal segment with
`|`. A crossing must preserve both directions as `+`. Merman's independent corrected snapshots
live in `tests/graph_fixture/corrected-fixtures/ascii/`; the copied files and their provenance
remain unchanged. These cases are strict output oracles, not tolerated layout gaps:

- `subgraph_mixed_nodes.txt`
- `subgraph_nested_with_external.txt`
- `subgraph_standalone_labeled_node.txt`
- `subgraph_td_multiple_paddingy.txt`
- `subgraph_td_multiple.txt`
- `subgraph_three_separate.txt`
- `subgraph_two_separate.txt`

The corrected subset uses the original fixture options, including `(3, 3)` graph padding for
`subgraph_td_multiple_paddingy.txt`. Its test compares bytes exactly, preserves visible text
counts, and checks closed group frames, all four group and node corners, unique authored labels,
first-parent node ownership, full node containment, full nested-group containment, and a distinct
arrowhead entering the declared target for every edge. It also
requires an actual route/frame crossing so a named disposition cannot silently become obsolete.

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
- `graph_tb_direction.txt`
- `preserve_order_of_definition.txt`
- `subgraph_complex_nested.txt`
- `subgraph_complex_mixed.txt`
- `subgraph_empty.txt`
- `subgraph_explicit_title.txt`
- `subgraph_mixed_nodes_td.txt`
- `subgraph_multiple_edges.txt`
- `subgraph_multiple_nodes.txt`
- `subgraph_node_outside_lr.txt`
- `subgraph_td_direction.txt`
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
- `graph_fixture_named_corrections_preserve_closed_compound_frames` protects strict corrected
  snapshots and frame/node geometry independently of copied output.
- `graph_fixture_named_gaps_preserve_visible_text_and_render` prevents named differences from
  hiding render failures or authored visible-text loss.
- `flowchart_local_semantic_fixture_covers_ampersand_fanin_and_fanout` covers ampersand topology.
- `flowchart_parser_cross_subgraph_routes_follow_compound_parent_topology` covers legal group-border
  crossing and repeated-node first-parent ownership.
- `flowchart_parser_isolated_implicit_subgraph_uses_mermaid_perpendicular_default` protects the
  pinned Mermaid default axis across TD, LR, BT, and RL roots.
- `flowchart_parser_explicit_subgraph_direction_survives_cross_boundary_edges` and
  `flowchart_parser_sibling_groups_keep_explicit_directions_across_external_edges` protect authored
  local directions independently of external connectivity.
- The focused Flowchart suite covers back edges, parallel labels, directions, subgraphs, endpoint
  markers, route reachability, terminal safety, and resource limits independently of copied spacing.
