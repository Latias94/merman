use super::*;
use crate::graphlib::GraphOptions;

#[derive(Default)]
struct RecordingWorkControl {
    charges: Vec<usize>,
    remaining: Option<usize>,
}

impl RecordingWorkControl {
    fn with_limit(limit: usize) -> Self {
        Self {
            remaining: Some(limit),
            ..Self::default()
        }
    }
}

impl WorkControl for RecordingWorkControl {
    fn charge(&mut self, units: usize) -> Result<(), WorkError> {
        self.charges.push(units);
        let Some(remaining) = self.remaining else {
            return Ok(());
        };
        let Some(next) = remaining.checked_sub(units) else {
            return Err(WorkError::Interrupted);
        };
        self.remaining = Some(next);
        Ok(())
    }
}

fn ranking_graph() -> Graph<NodeLabel, EdgeLabel, GraphLabel> {
    let mut graph = Graph::new(GraphOptions::default());
    graph.set_graph(GraphLabel::default());
    graph.set_default_node_label(NodeLabel::default);
    graph.set_default_edge_label(|| EdgeLabel {
        minlen: 1,
        weight: 1.0,
        ..EdgeLabel::default()
    });
    graph.set_path(&["a", "b", "c", "d"]);
    graph.set_edge("a", "c");
    graph.set_edge("b", "d");
    graph
}

fn tree(edges: &[(&str, &str)]) -> Graph<tree::TreeNodeLabel, tree::TreeEdgeLabel, ()> {
    let mut tree = Graph::new(GraphOptions {
        directed: false,
        ..GraphOptions::default()
    });
    tree.set_default_node_label(tree::TreeNodeLabel::default);
    tree.set_default_edge_label(tree::TreeEdgeLabel::default);
    for &(source, target) in edges {
        tree.set_edge(source, target);
    }
    tree
}

fn numeric_path_tree_fixture(
    width: usize,
    numeric_leaf: bool,
) -> (
    Graph<tree::TreeNodeLabel, tree::TreeEdgeLabel, ()>,
    Graph<NodeLabel, EdgeLabel, GraphLabel>,
) {
    let mut spanning_tree = Graph::new(GraphOptions {
        directed: false,
        ..GraphOptions::default()
    });
    spanning_tree.set_default_node_label(tree::TreeNodeLabel::default);
    spanning_tree.set_default_edge_label(tree::TreeEdgeLabel::default);

    let leaf = if numeric_leaf { "0" } else { "leaf" };
    spanning_tree.set_edge(leaf, "node-0");
    for index in 1..width {
        spanning_tree.set_edge(format!("node-{}", index - 1), format!("node-{index}"));
    }

    let mut graph = Graph::new(GraphOptions::default());
    graph.set_graph(GraphLabel::default());
    for edge in spanning_tree.edge_keys() {
        graph.set_edge_with_label(
            edge.v,
            edge.w,
            EdgeLabel {
                minlen: 1,
                weight: 1.0,
                ..EdgeLabel::default()
            },
        );
    }
    (spanning_tree, graph)
}

fn tree_node_id<'a>(
    state: &TreeState,
    graph: &'a Graph<NodeLabel, EdgeLabel, GraphLabel>,
    t_ix: usize,
) -> &'a str {
    state
        .g_ix_by_t_ix
        .get(t_ix)
        .copied()
        .flatten()
        .and_then(|g_ix| graph.node_id_by_ix(g_ix))
        .expect("tree nodes map to simplified graph nodes")
}

fn ordered_tree_edges(
    state: &TreeState,
    graph: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
) -> Vec<(String, String)> {
    state
        .tree_edges_in_order
        .iter()
        .map(|edge| {
            (
                tree_node_id(state, graph, edge.v_t_ix).to_string(),
                tree_node_id(state, graph, edge.w_t_ix).to_string(),
            )
        })
        .collect()
}

fn multi_pivot_graph() -> Graph<NodeLabel, EdgeLabel, GraphLabel> {
    let mut graph = Graph::new(GraphOptions::default());
    graph.set_graph(GraphLabel::default());
    graph.set_default_node_label(NodeLabel::default);
    for (v, w, minlen, weight) in [
        ("n0", "n1", 2, 1.0),
        ("n0", "n4", 3, 1.0),
        ("n1", "n3", 3, 6.0),
        ("n1", "n4", 1, 1.0),
        ("n1", "n5", 1, 7.0),
        ("n2", "n3", 3, 7.0),
        ("n2", "n4", 1, 4.0),
        ("n3", "n8", 2, 7.0),
        ("n4", "n5", 3, 3.0),
        ("n4", "n7", 2, 5.0),
        ("n4", "n8", 3, 4.0),
        ("n5", "n9", 3, 6.0),
        ("n7", "n9", 1, 2.0),
        ("n8", "n9", 2, 1.0),
    ] {
        graph.set_edge_with_label(
            v,
            w,
            EdgeLabel {
                minlen,
                weight,
                ..EdgeLabel::default()
            },
        );
    }
    graph
}

fn first_multi_pivot_state() -> (
    TreeState,
    Graph<NodeLabel, EdgeLabel, GraphLabel>,
    Vec<i128>,
    OrderedTreeEdge,
) {
    let source = multi_pivot_graph();
    let mut work_control = NoopWorkControl;
    let mut graph = build_simplified_graph(&source, &mut work_control)
        .expect("the fixture simplify step succeeds");
    util::longest_path_controlled(&mut graph, &mut work_control)
        .expect("the fixture ranks fit i32");
    let tree = feasible_tree::feasible_tree_controlled(&mut graph, &mut work_control)
        .expect("the fixture feasible tree succeeds");
    let mut state = TreeState::new(&tree, &graph).expect("the tree is a graph-edge subset");
    drop(tree);
    state
        .rebuild(&graph, None)
        .expect("the dense tree rebuild succeeds");

    let mut rank_by_ix = vec![0_i128; graph.node_slot_count()];
    graph.for_each_node_ix(|g_ix, _id, label| {
        rank_by_ix[g_ix] = i128::from(label.rank.unwrap_or(0));
    });
    let leaving = state
        .find_leave_edge_in_insertion_order()
        .expect("the pinned fixture begins with a negative tree edge");
    (state, graph, rank_by_ix, leaving)
}

fn rank_snapshot(graph: &Graph<NodeLabel, EdgeLabel, GraphLabel>) -> Vec<(String, Option<i32>)> {
    graph
        .nodes()
        .map(|id| (id.to_string(), graph.node(id).and_then(|node| node.rank)))
        .collect()
}

fn mixed_simplify_graph(numeric: bool) -> Graph<NodeLabel, EdgeLabel, GraphLabel> {
    let mut graph = Graph::new(GraphOptions {
        multigraph: true,
        ..GraphOptions::default()
    });
    graph.set_graph(GraphLabel::default());
    let ids = if numeric {
        ["0", "node-a", "1", "node-b"]
    } else {
        ["node-0", "node-a", "node-1", "node-b"]
    };
    for id in ids {
        graph.set_node(id, NodeLabel::default());
    }
    for (v, w, name, weight) in [
        (ids[0], ids[1], "parallel-0", 1.0),
        (ids[0], ids[1], "parallel-1", 2.0),
        (ids[1], ids[2], "mixed", 3.0),
        (ids[0], ids[2], "numeric", 4.0),
        (ids[2], ids[2], "self", 5.0),
        (ids[1], ids[3], "ordinary", 6.0),
    ] {
        graph.set_edge_named(
            v,
            w,
            Some(name),
            Some(EdgeLabel {
                weight,
                minlen: 1,
                ..EdgeLabel::default()
            }),
        );
    }
    graph
}

#[test]
fn simplify_work_curve_includes_numeric_object_key_rebuilds() {
    for width in (0..=10).map(|shift| 1usize << shift) {
        let mut numeric = Graph::new(GraphOptions::default());
        let mut ordinary = Graph::new(GraphOptions::default());
        for index in (0..width).rev() {
            numeric.set_node(index.to_string(), NodeLabel::default());
            ordinary.set_node(format!("node-{index}"), NodeLabel::default());
        }
        for index in 1..width {
            numeric.set_edge((index - 1).to_string(), index.to_string());
            ordinary.set_edge(format!("node-{}", index - 1), format!("node-{index}"));
        }

        let numeric_work = simplify_work_units(&numeric).unwrap();
        let ordinary_work = simplify_work_units(&ordinary).unwrap();
        let numeric_updates = width + (width.saturating_sub(1) * 2);
        let ordered_work = checked_ordered_key_updates(width, numeric_updates).unwrap();
        assert_eq!(numeric_work, ordinary_work + ordered_work);
    }
}

#[test]
fn simplify_work_uses_one_ordered_merge_and_charges_tombstone_slots() {
    let mut graph = mixed_simplify_graph(false);
    let expected_dense = checked_add(
        checked_mul(graph.node_count(), 2).unwrap(),
        checked_add(
            checked_add(
                checked_mul(graph.edge_slot_count(), 2).unwrap(),
                checked_mul(graph.edge_count(), 3).unwrap(),
            )
            .unwrap(),
            checked_n_log_n(graph.edge_count()).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(simplify_work_units(&graph), Ok(expected_dense));

    assert!(graph.remove_edge("node-0", "node-a", Some("parallel-0")));
    assert_eq!(graph.edge_slot_count(), 6);
    assert_eq!(graph.edge_count(), 5);
    let expected_sparse = checked_add(
        checked_mul(graph.node_count(), 2).unwrap(),
        checked_add(
            checked_add(
                checked_mul(graph.edge_slot_count(), 2).unwrap(),
                checked_mul(graph.edge_count(), 3).unwrap(),
            )
            .unwrap(),
            checked_n_log_n(graph.edge_count()).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(simplify_work_units(&graph), Ok(expected_sparse));
}

#[test]
fn simplify_precharges_mixed_numeric_directed_adjacency() {
    let numeric = mixed_simplify_graph(true);
    let ordinary = mixed_simplify_graph(false);
    let adjacency_entries = numeric.directed_array_index_adjacency_entry_count();
    assert_eq!(adjacency_entries, 6);
    assert_eq!(ordinary.directed_array_index_adjacency_entry_count(), 0);

    let node_order_work =
        checked_ordered_key_updates(numeric.node_count(), numeric.array_index_node_count())
            .unwrap();
    let adjacency_work =
        checked_ordered_key_updates(numeric.node_count(), adjacency_entries).unwrap();
    let numeric_exact = simplify_work_units(&numeric).unwrap();
    let ordinary_exact = simplify_work_units(&ordinary).unwrap();
    assert_eq!(
        numeric_exact,
        checked_add(
            ordinary_exact,
            checked_add(node_order_work, adjacency_work).unwrap(),
        )
        .unwrap()
    );

    let mut measured = RecordingWorkControl::default();
    let simplified = build_simplified_graph(&numeric, &mut measured)
        .expect("the unbounded control admits the numeric simplify copy");
    assert_eq!(measured.charges, [numeric_exact]);
    assert_eq!(simplified.edge_count(), 5);
    assert_eq!(
        simplified.directed_array_index_adjacency_entry_count(),
        adjacency_entries
    );

    let source_nodes = numeric.node_ids();
    let source_edges = numeric.edge_keys();
    let mut below = RecordingWorkControl::with_limit(numeric_exact - 1);
    assert!(matches!(
        build_simplified_graph(&numeric, &mut below),
        Err(WorkError::Interrupted)
    ));
    assert_eq!(below.charges, [numeric_exact]);
    assert_eq!(below.remaining, Some(numeric_exact - 1));
    assert_eq!(numeric.node_ids(), source_nodes);
    assert_eq!(numeric.edge_keys(), source_edges);

    for limit in [numeric_exact, numeric_exact + 1] {
        let mut admitted = RecordingWorkControl::with_limit(limit);
        let simplified = build_simplified_graph(&numeric, &mut admitted)
            .expect("equal and above numeric simplify-copy budgets succeed");
        assert_eq!(simplified.edge_count(), 5);
    }
}

#[test]
fn dense_tree_exchange_preserves_stable_delete_then_append_order() {
    let graph = ranking_graph();
    let first_tree = tree(&[("a", "b"), ("b", "c"), ("c", "d")]);
    let mut state = TreeState::new(&first_tree, &graph).expect("the tree is a graph-edge subset");
    let edge_capacity = state.tree_edges_in_order.capacity();

    state
        .exchange_edge(
            OrderedTreeEdge {
                position: 1,
                v_ix: first_tree.node_ix("b").expect("b is present"),
                w_ix: first_tree.node_ix("c").expect("c is present"),
            },
            &EdgeKey {
                v: "a".to_string(),
                w: "c".to_string(),
                name: None,
            },
            &graph,
        )
        .expect("the tree exchange is valid");

    assert_eq!(
        ordered_tree_edges(&state, &graph),
        [("a", "b"), ("c", "d"), ("a", "c")].map(|(v, w)| (v.to_string(), w.to_string()))
    );
    assert_eq!(state.edge_count(), 3);
    assert_eq!(state.tree_edges_in_order.capacity(), edge_capacity);

    state
        .rebuild(&graph, Some("a"))
        .expect("the exchanged tree rebuild succeeds");
    let second_tree = tree(&[("a", "b"), ("c", "d"), ("a", "c")]);
    let mut fresh = TreeState::new(&second_tree, &graph).expect("the tree is a graph-edge subset");
    fresh
        .rebuild(&graph, Some("a"))
        .expect("the fresh tree rebuild succeeds");
    assert_eq!(state, fresh);
}

#[test]
fn generic_graph_tree_slots_grow_across_the_pinned_pivot_trace() {
    let mut generic_tree = tree(&[
        ("n0", "n1"),
        ("n1", "n3"),
        ("n2", "n3"),
        ("n2", "n4"),
        ("n4", "n5"),
        ("n5", "n9"),
        ("n7", "n9"),
        ("n8", "n9"),
    ]);
    let mut slot_curve = vec![generic_tree.edge_slot_count()];
    for ((leave_v, leave_w), (enter_v, enter_w)) in [
        (("n2", "n3"), ("n0", "n4")),
        (("n0", "n1"), ("n1", "n4")),
        (("n7", "n9"), ("n4", "n7")),
        (("n8", "n9"), ("n3", "n8")),
        (("n1", "n4"), ("n0", "n1")),
        (("n0", "n4"), ("n2", "n3")),
    ] {
        assert!(generic_tree.remove_edge(leave_v, leave_w, None));
        generic_tree.set_edge(enter_v, enter_w);
        assert_eq!(generic_tree.edge_count(), 8);
        slot_curve.push(generic_tree.edge_slot_count());
    }
    assert_eq!(slot_curve, [8, 9, 10, 11, 12, 13, 14]);
}

#[test]
fn dense_tree_storage_stays_fixed_across_a_long_exchange_history() {
    const PIVOTS: usize = 256;
    const TREE_EDGES: usize = 4;
    const NODE_IDS: [&str; 5] = ["a", "b", "c", "d", "e"];

    let mut graph = Graph::new(GraphOptions::default());
    graph.set_graph(GraphLabel::default());
    graph.set_default_node_label(NodeLabel::default);
    let mut complete_edges = Vec::new();
    for (position, &v) in NODE_IDS.iter().enumerate() {
        for &w in &NODE_IDS[position + 1..] {
            complete_edges.push((v, w));
            graph.set_edge_with_label(
                v,
                w,
                EdgeLabel {
                    minlen: 1,
                    weight: 1.0,
                    ..EdgeLabel::default()
                },
            );
        }
    }

    let initial_edges = [("a", "b"), ("b", "c"), ("c", "d"), ("d", "e")];
    let source_tree = tree(&initial_edges);
    let mut generic_tree = tree(&initial_edges);
    let mut state = TreeState::new(&source_tree, &graph).expect("the tree is a graph-edge subset");
    let dense_capacity = state.tree_edges_in_order.capacity();
    let mut dense_scan_entries = 0usize;
    let mut generic_slot_scan_entries = 0usize;

    for _ in 0..PIVOTS {
        let leaving_edge = state.tree_edges_in_order[0];
        let leaving_ends = (leaving_edge.v_t_ix, leaving_edge.w_t_ix);
        let leave_v = tree_node_id(&state, &graph, leaving_ends.0).to_string();
        let leave_w = tree_node_id(&state, &graph, leaving_ends.1).to_string();

        let mut reachable = vec![false; state.g_ix_by_t_ix.len()];
        let mut stack = vec![leaving_ends.0];
        reachable[leaving_ends.0] = true;
        while let Some(node_ix) = stack.pop() {
            for edge in &state.tree_edges_in_order[1..] {
                let (v_ix, w_ix) = (edge.v_t_ix, edge.w_t_ix);
                let adjacent = if v_ix == node_ix {
                    Some(w_ix)
                } else if w_ix == node_ix {
                    Some(v_ix)
                } else {
                    None
                };
                let Some(adjacent) = adjacent else {
                    continue;
                };
                if !reachable[adjacent] {
                    reachable[adjacent] = true;
                    stack.push(adjacent);
                }
            }
        }

        let &(enter_v, enter_w) = complete_edges
            .iter()
            .find(|&&(v, w)| {
                let v_ix = source_tree.node_ix(v).unwrap();
                let w_ix = source_tree.node_ix(w).unwrap();
                let candidate = (v_ix, w_ix);
                let same_edge =
                    |ends: (usize, usize)| ends == candidate || ends == (candidate.1, candidate.0);
                !same_edge(leaving_ends)
                    && !state
                        .tree_edges_in_order
                        .iter()
                        .map(|edge| (edge.v_t_ix, edge.w_t_ix))
                        .any(same_edge)
                    && reachable[v_ix] != reachable[w_ix]
            })
            .expect("the complete graph supplies another cross-component edge");
        state
            .exchange_edge(
                OrderedTreeEdge {
                    position: 0,
                    v_ix: leaving_ends.0,
                    w_ix: leaving_ends.1,
                },
                &EdgeKey {
                    v: enter_v.to_string(),
                    w: enter_w.to_string(),
                    name: None,
                },
                &graph,
            )
            .expect("the selected cross-component exchange preserves a connected tree");

        assert!(generic_tree.remove_edge(&leave_v, &leave_w, None));
        generic_tree.set_edge(enter_v, enter_w);

        let generic_order = generic_tree
            .edge_keys()
            .into_iter()
            .map(|edge| (edge.v, edge.w))
            .collect::<Vec<_>>();
        assert_eq!(ordered_tree_edges(&state, &graph), generic_order);
        assert_eq!(state.edge_count(), TREE_EDGES);
        assert_eq!(state.tree_edges_in_order.capacity(), dense_capacity);

        dense_scan_entries += state.edge_count();
        generic_slot_scan_entries += generic_tree.edge_slot_count();
    }

    assert_eq!(generic_tree.edge_slot_count(), TREE_EDGES + PIVOTS);
    assert_eq!(dense_scan_entries, TREE_EDGES * PIVOTS);
    assert_eq!(
        generic_slot_scan_entries,
        TREE_EDGES * PIVOTS + PIVOTS * (PIVOTS + 1) / 2
    );
}

#[test]
fn dense_pivot_precharges_post_exchange_numeric_adjacency_upper_bound() {
    let mut graph = Graph::new(GraphOptions::default());
    graph.set_graph(GraphLabel::default());
    graph.set_default_node_label(NodeLabel::default);
    for (v, w) in [("2", "a"), ("a", "b"), ("b", "10"), ("2", "10")] {
        graph.set_edge_with_label(
            v,
            w,
            EdgeLabel {
                minlen: 1,
                weight: 1.0,
                ..EdgeLabel::default()
            },
        );
    }
    let spanning_tree = tree(&[("2", "a"), ("a", "b"), ("b", "10")]);
    let mut state =
        TreeState::new(&spanning_tree, &graph).expect("the tree is a graph-edge subset");
    assert!(state.has_array_index_nodes);
    assert_eq!(state.array_index_adjacency_entry_count, 2);

    let entering_and_rank_work = checked_add(
        checked_mul(graph.node_count(), 2).unwrap(),
        graph.edge_count(),
    )
    .unwrap();
    let stable_exchange_work = checked_mul(state.edge_count(), 2).unwrap();
    let common_work = checked_add(entering_and_rank_work, stable_exchange_work).unwrap();
    let current_rebuild_work = dense_tree_state_rebuild_work_units(
        &graph,
        state.node_slot_count(),
        state.edge_count(),
        state.array_index_adjacency_entry_count,
    )
    .unwrap();
    let post_exchange_upper_bound = std::cmp::min(
        checked_mul(state.edge_count(), 2).unwrap(),
        checked_add(state.array_index_adjacency_entry_count, 2).unwrap(),
    );
    assert_eq!(post_exchange_upper_bound, 4);
    let charged_rebuild_work = dense_tree_state_rebuild_work_units(
        &graph,
        state.node_slot_count(),
        state.edge_count(),
        post_exchange_upper_bound,
    )
    .unwrap();
    let exact = simplex_iteration_work_units(&graph, &state).unwrap();
    assert_eq!(
        exact,
        checked_add(common_work, charged_rebuild_work).unwrap()
    );
    assert!(exact > checked_add(common_work, current_rebuild_work).unwrap());

    let leaving = OrderedTreeEdge {
        position: 1,
        v_ix: spanning_tree.node_ix("a").expect("a is present"),
        w_ix: spanning_tree.node_ix("b").expect("b is present"),
    };
    state
        .exchange_edge(
            leaving,
            &EdgeKey {
                v: "2".to_string(),
                w: "10".to_string(),
                name: None,
            },
            &graph,
        )
        .expect("the numeric entering edge reconnects the two tree components");
    assert_eq!(state.array_index_adjacency_entry_count, 4);
    assert!(state.array_index_adjacency_entry_count <= post_exchange_upper_bound);
    state
        .rebuild(&graph, None)
        .expect("the post-exchange tree rebuild succeeds");
}

#[test]
fn tree_state_uses_pinned_graphlib_neighbor_object_key_order() {
    let mut spanning_tree = tree(&[
        ("m", "z"),
        ("a", "m"),
        ("m", "y"),
        ("b", "m"),
        ("10", "m"),
        ("2", "m"),
    ]);
    let mut graph = Graph::new(GraphOptions::default());
    graph.set_graph(GraphLabel::default());
    for id in spanning_tree.node_ids() {
        graph.set_node(id, NodeLabel::default());
    }
    for edge in spanning_tree.edge_keys() {
        graph.set_edge_with_label(
            edge.v,
            edge.w,
            EdgeLabel {
                minlen: 1,
                weight: 1.0,
                ..EdgeLabel::default()
            },
        );
    }

    let mut state =
        TreeState::new(&spanning_tree, &graph).expect("the tree is a graph-edge subset");
    state
        .rebuild(&graph, Some("m"))
        .expect("the mixed-key tree rebuild succeeds");
    let m_tix = spanning_tree.node_ix("m").expect("m is present");
    let neighbors = state
        .neighbors
        .slice(m_tix)
        .iter()
        .map(|&t_ix| tree_node_id(&state, &graph, t_ix))
        .collect::<Vec<_>>();

    // dagre-d3-es 7.0.14 Graph.neighbors is union(predecessors, successors). Each bucket uses
    // Object.keys order, so numeric IDs precede ordinary IDs and are sorted numerically.
    assert_eq!(neighbors, ["2", "10", "a", "b", "z", "y"]);
    for (id, expected_lim) in [("2", 1), ("10", 2), ("a", 3), ("b", 4), ("z", 5), ("y", 6)] {
        let g_ix = graph.node_ix(id).expect("fixture node is present");
        assert_eq!(
            state.node_low_lim_by_gix(g_ix),
            Some((expected_lim, expected_lim))
        );
        let t_ix = spanning_tree
            .node_ix(id)
            .expect("fixture tree node is present");
        assert_eq!(state.parent_t_ix[t_ix], Some(m_tix));
    }
    let m_gix = graph.node_ix("m").expect("m is present");
    assert_eq!(state.node_low_lim_by_gix(m_gix), Some((1, 7)));
    assert_eq!(state.parent_t_ix[m_tix], None);

    init_low_lim_values(&mut spanning_tree, Some("m"));
    for (id, expected_lim) in [("2", 1), ("10", 2), ("a", 3), ("b", 4), ("z", 5), ("y", 6)] {
        let label = spanning_tree
            .node(id)
            .expect("fixture tree label is present");
        assert_eq!((label.low, label.lim), (expected_lim, expected_lim));
        assert_eq!(label.parent.as_deref(), Some("m"));
    }
    let root = spanning_tree.node("m").expect("root tree label is present");
    assert_eq!((root.low, root.lim), (1, 7));
    assert_eq!(root.parent, None);
}

#[test]
fn numeric_neighbor_storage_scales_with_numeric_entries_not_node_slots() {
    for width in [64, 256, 1024] {
        let (numeric_tree, numeric_graph) = numeric_path_tree_fixture(width, true);
        let mut numeric_state = TreeState::new(&numeric_tree, &numeric_graph)
            .expect("the numeric tree is a graph-edge subset");
        numeric_state
            .rebuild(&numeric_graph, None)
            .expect("the numeric star rebuild succeeds");
        assert_eq!(numeric_state.array_index_adjacency_entry_count, 1);
        assert_eq!(numeric_state.neighbors.offsets.len(), width + 2);
        assert_eq!(numeric_state.neighbors.build_state.len(), width + 1);
        assert_eq!(numeric_state.neighbors.entries.len(), width * 2);
        assert_eq!(numeric_state.neighbors.numeric_entries.len(), 1);

        let (ordinary_tree, ordinary_graph) = numeric_path_tree_fixture(width, false);
        let mut ordinary_state = TreeState::new(&ordinary_tree, &ordinary_graph)
            .expect("the ordinary tree is a graph-edge subset");
        ordinary_state
            .rebuild(&ordinary_graph, None)
            .expect("the ordinary star rebuild succeeds");
        assert_eq!(ordinary_state.array_index_adjacency_entry_count, 0);
        assert_eq!(ordinary_state.neighbors.offsets.len(), width + 2);
        assert_eq!(ordinary_state.neighbors.build_state.len(), width + 1);
        assert_eq!(ordinary_state.neighbors.entries.len(), width * 2);
        assert!(ordinary_state.neighbors.numeric_entries.is_empty());

        let numeric_work = dense_tree_state_rebuild_work_units(
            &numeric_graph,
            numeric_state.node_slot_count(),
            numeric_state.edge_count(),
            numeric_state.array_index_adjacency_entry_count,
        )
        .unwrap();
        let ordinary_work = dense_tree_state_rebuild_work_units(
            &ordinary_graph,
            ordinary_state.node_slot_count(),
            ordinary_state.edge_count(),
            ordinary_state.array_index_adjacency_entry_count,
        )
        .unwrap();
        assert_eq!(
            numeric_work,
            checked_add(
                ordinary_work,
                checked_ordered_key_updates(width + 1, 1).unwrap(),
            )
            .unwrap()
        );
    }
}

#[test]
fn initial_tree_state_work_scales_with_live_numeric_adjacency_entries() {
    for width in [64, 256, 1024] {
        let (numeric_tree, numeric_graph) = numeric_path_tree_fixture(width, true);
        let numeric_state = TreeState::new(&numeric_tree, &numeric_graph)
            .expect("the numeric tree is a graph-edge subset");
        let (ordinary_tree, ordinary_graph) = numeric_path_tree_fixture(width, false);
        let ordinary_state = TreeState::new(&ordinary_tree, &ordinary_graph)
            .expect("the ordinary tree is a graph-edge subset");
        let numeric_work = dense_tree_state_rebuild_work_units(
            &numeric_graph,
            numeric_state.node_slot_count(),
            numeric_state.edge_count(),
            numeric_state.array_index_adjacency_entry_count,
        )
        .unwrap();
        let ordinary_work = dense_tree_state_rebuild_work_units(
            &ordinary_graph,
            ordinary_state.node_slot_count(),
            ordinary_state.edge_count(),
            ordinary_state.array_index_adjacency_entry_count,
        )
        .unwrap();

        assert_eq!(
            numeric_work,
            checked_add(
                ordinary_work,
                checked_ordered_key_updates(width + 1, 1).unwrap(),
            )
            .unwrap()
        );
    }
}

#[test]
fn initial_tree_state_budget_uses_the_exact_numeric_adjacency_count() {
    let mut spanning_tree = Graph::new(GraphOptions {
        directed: false,
        ..GraphOptions::default()
    });
    spanning_tree.set_default_node_label(tree::TreeNodeLabel::default);
    spanning_tree.set_default_edge_label(tree::TreeEdgeLabel::default);
    spanning_tree.set_edge("0", "node-0");
    for index in 1..128 {
        spanning_tree.set_edge(format!("node-{}", index - 1), format!("node-{index}"));
    }

    let mut graph = Graph::new(GraphOptions::default());
    graph.set_graph(GraphLabel::default());
    for edge in spanning_tree.edge_keys() {
        graph.set_edge_with_label(
            edge.v,
            edge.w,
            EdgeLabel {
                minlen: 1,
                weight: 1.0,
                ..EdgeLabel::default()
            },
        );
    }

    assert_eq!(spanning_tree.pinned_array_index_adjacency_entry_count(), 1);
    let expected_state =
        TreeState::new(&spanning_tree, &graph).expect("the tree is a graph-edge subset");
    let construction = tree_state_new_work_units(&graph, &spanning_tree).unwrap();
    let rebuild = dense_tree_state_rebuild_work_units(
        &graph,
        expected_state.node_slot_count(),
        expected_state.edge_count(),
        expected_state.array_index_adjacency_entry_count,
    )
    .unwrap();
    let exact = checked_add(construction, rebuild).unwrap();

    let mut below_construction = RecordingWorkControl::with_limit(construction - 1);
    assert!(matches!(
        build_tree_state_controlled(&spanning_tree, &graph, None, &mut below_construction),
        Err(RankError::Work(WorkError::Interrupted))
    ));
    assert_eq!(below_construction.charges, [construction]);
    assert_eq!(below_construction.remaining, Some(construction - 1));

    let mut below_rebuild = RecordingWorkControl::with_limit(exact - 1);
    assert!(matches!(
        build_tree_state_controlled(&spanning_tree, &graph, None, &mut below_rebuild),
        Err(RankError::Work(WorkError::Interrupted))
    ));
    assert_eq!(below_rebuild.charges, [construction, rebuild]);
    assert_eq!(below_rebuild.remaining, Some(rebuild - 1));

    for limit in [exact, exact + 1] {
        let mut admitted = RecordingWorkControl::with_limit(limit);
        let state = build_tree_state_controlled(&spanning_tree, &graph, None, &mut admitted)
            .expect("equal and above tree-state budgets succeed");
        assert_eq!(state.array_index_adjacency_entry_count, 1);
        assert_eq!(state.tree_edges_in_order.len(), spanning_tree.edge_count());
        assert_eq!(admitted.charges, [construction, rebuild]);
        assert_eq!(admitted.remaining, Some(limit - exact));
    }
}

#[test]
fn initial_tree_state_precharges_sparse_slots_and_a_cold_graph_cache() {
    let mut spanning_tree = tree(&[("temporary", "removed"), ("a", "b"), ("b", "c")]);
    assert!(spanning_tree.remove_edge("temporary", "removed", None));
    assert!(spanning_tree.remove_node("temporary"));
    assert!(spanning_tree.remove_node("removed"));

    let mut graph = Graph::new(GraphOptions::default());
    graph.set_graph(GraphLabel::default());
    for (v, w) in [("temporary", "removed"), ("a", "b"), ("b", "c")] {
        graph.set_edge_with_label(
            v,
            w,
            EdgeLabel {
                minlen: 1,
                weight: 1.0,
                ..EdgeLabel::default()
            },
        );
    }
    assert!(graph.remove_edge("temporary", "removed", None));
    assert!(graph.remove_node("temporary"));
    assert!(graph.remove_node("removed"));
    assert!(!graph.is_adjacency_cache_current());
    assert!(spanning_tree.node_slot_count() > spanning_tree.node_count());
    assert!(spanning_tree.edge_slot_count() > spanning_tree.edge_count());
    assert!(graph.node_slot_count() > graph.node_count());
    assert!(graph.edge_slot_count() > graph.edge_count());

    let expected_state =
        TreeState::new(&spanning_tree, &graph).expect("the live tree remains valid");
    let construction = tree_state_new_work_units(&graph, &spanning_tree).unwrap();
    let rebuild = dense_tree_state_rebuild_work_units(
        &graph,
        expected_state.node_slot_count(),
        expected_state.edge_count(),
        expected_state.array_index_adjacency_entry_count,
    )
    .unwrap();
    let exact = checked_add(construction, rebuild).unwrap();
    let mut work_control = RecordingWorkControl::with_limit(exact);
    let state = build_tree_state_controlled(&spanning_tree, &graph, None, &mut work_control)
        .expect("slot-aware exact budget admits the sparse fixture");

    assert_eq!(work_control.charges, [construction, rebuild]);
    assert_eq!(work_control.remaining, Some(0));
    assert_eq!(state.node_count(), 3);
    assert!(graph.is_adjacency_cache_current());
}

#[test]
fn dense_tree_edges_remove_high_fanout_endpoint_rescans() {
    const LEAVES: usize = 512;

    let mut spanning_tree = Graph::new(GraphOptions {
        directed: false,
        ..GraphOptions::default()
    });
    spanning_tree.set_default_node_label(tree::TreeNodeLabel::default);
    spanning_tree.set_default_edge_label(tree::TreeEdgeLabel::default);

    let mut graph = Graph::new(GraphOptions::default());
    graph.set_graph(GraphLabel::default());
    graph.set_node("root", NodeLabel::default());
    for index in 0..LEAVES {
        let leaf = format!("leaf-{index}");
        let minlen = index + 1;
        let weight = index as f64 + 0.5;
        spanning_tree.set_edge("root", leaf.clone());
        graph.set_edge_with_label(
            "root",
            leaf,
            EdgeLabel {
                minlen,
                weight,
                ..EdgeLabel::default()
            },
        );
    }

    let mut state =
        TreeState::new(&spanning_tree, &graph).expect("the tree is a graph-edge subset");
    assert_eq!(state.tree_edges_in_order.len(), LEAVES);

    // Rebuild and rank propagation own immutable snapshots. Mutating the source labels after
    // construction proves these O(V) passes no longer query the root's growing out-edge list.
    for index in 0..LEAVES {
        let leaf = format!("leaf-{index}");
        let edge = graph
            .edge_mut("root", &leaf, None)
            .expect("the star edge exists");
        edge.minlen = usize::MAX;
        edge.weight = -1.0;
    }

    state
        .rebuild(&graph, Some("root"))
        .expect("the wide star rebuild succeeds");
    let root_tix = spanning_tree.node_ix("root").expect("root is present");
    for index in 0..LEAVES {
        let leaf = format!("leaf-{index}");
        let leaf_tix = spanning_tree.node_ix(&leaf).expect("leaf is present");
        assert_eq!(state.parent_t_ix[leaf_tix], Some(root_tix));
        let parent_edge = state.parent_edge_position_by_t_ix[leaf_tix]
            .and_then(|position| state.tree_edges_in_order.get(position))
            .expect("the leaf retains one parent edge");
        assert_eq!(parent_edge.minlen, index + 1);
        assert_eq!(parent_edge.weight, index as f64 + 0.5);
    }

    let mut rank_by_ix = vec![0_i128; graph.node_slot_count()];
    update_ranks_fast(&mut state, &mut graph, &mut rank_by_ix)
        .expect("snapshot minlen values fit the rank domain");
    for index in 0..LEAVES {
        let leaf = format!("leaf-{index}");
        assert_eq!(
            graph.node(&leaf).and_then(|node| node.rank),
            Some((index + 1) as i32)
        );
    }
}

#[test]
fn cut_value_accumulates_incoming_edges_before_outgoing_edges() {
    const TWO_TO_53: f64 = 9_007_199_254_740_992.0;

    let mut graph = Graph::new(GraphOptions::default());
    graph.set_graph(GraphLabel::default());
    for (v, w, weight) in [
        ("c", "p", 1.0),
        ("x", "p", 0.0),
        ("y", "p", 0.0),
        ("z", "p", 0.0),
        ("x", "c", TWO_TO_53),
        ("y", "c", 1.0),
        ("c", "z", TWO_TO_53),
    ] {
        graph.set_edge_with_label(
            v,
            w,
            EdgeLabel {
                minlen: 1,
                weight,
                ..EdgeLabel::default()
            },
        );
    }

    let mut spanning_tree = tree(&[("c", "p"), ("p", "x"), ("p", "y"), ("p", "z")]);
    init_low_lim_values(&mut spanning_tree, Some("p"));
    assert_eq!(calc_cut_value(&spanning_tree, &graph, "c"), 0.0);

    let mut state =
        TreeState::new(&spanning_tree, &graph).expect("the tree is a graph-edge subset");
    state
        .rebuild(&graph, Some("p"))
        .expect("the parallel-edge tree rebuild succeeds");
    let c_tix = spanning_tree.node_ix("c").expect("c is present");
    assert_eq!(state.cut_to_parent[c_tix], 0.0);

    let mut undirected = Graph::new(GraphOptions {
        directed: false,
        ..GraphOptions::default()
    });
    undirected.set_graph(GraphLabel::default());
    for (v, w, weight) in [
        ("m", "z", 1.0),
        ("a", "m", TWO_TO_53),
        ("b", "m", 1.0),
        ("m", "x", TWO_TO_53),
    ] {
        undirected.set_edge_with_label(
            v,
            w,
            EdgeLabel {
                minlen: 1,
                weight,
                ..EdgeLabel::default()
            },
        );
    }
    let mut undirected_tree = tree(&[("m", "z"), ("z", "a"), ("z", "b"), ("z", "x")]);
    init_low_lim_values(&mut undirected_tree, Some("z"));
    assert_eq!(calc_cut_value(&undirected_tree, &undirected, "m"), 0.0);
}

#[test]
fn tree_state_counts_numeric_endpoints_and_rejects_self_loops() {
    let numeric_tree = tree(&[("0", "1")]);
    let mut graph = Graph::new(GraphOptions::default());
    graph.set_graph(GraphLabel::default());
    graph.set_edge_with_label(
        "0",
        "1",
        EdgeLabel {
            minlen: 1,
            weight: 1.0,
            ..EdgeLabel::default()
        },
    );
    let state = TreeState::new(&numeric_tree, &graph).expect("a simple numeric tree is valid");
    assert_eq!(state.array_index_adjacency_entry_count, 2);

    let self_loop_tree = tree(&[("0", "0")]);
    let mut self_loop_graph = Graph::new(GraphOptions::default());
    self_loop_graph.set_graph(GraphLabel::default());
    self_loop_graph.set_edge_with_label(
        "0",
        "0",
        EdgeLabel {
            minlen: 1,
            weight: 1.0,
            ..EdgeLabel::default()
        },
    );
    assert_eq!(
        TreeState::new(&self_loop_tree, &self_loop_graph),
        Err(RankError::InvalidNetworkSimplexTree)
    );
}

#[test]
fn network_simplex_rejects_a_negative_tree_without_an_entering_edge() {
    let mut graph = Graph::new(GraphOptions::default());
    graph.set_graph(GraphLabel::default());
    graph.set_edge_with_label(
        "a",
        "b",
        EdgeLabel {
            minlen: 1,
            weight: -1.0,
            ..EdgeLabel::default()
        },
    );
    let source_nodes = graph.node_ids();
    let source_edges = graph.edge_keys();

    let mut work_control = NoopWorkControl;
    assert_eq!(
        network_simplex_controlled(&mut graph, &mut work_control),
        Err(RankError::InvalidNetworkSimplexTree)
    );
    assert_eq!(
        network_simplex(&mut graph),
        Err(crate::LayoutError::InvalidNetworkSimplexTree)
    );
    assert_eq!(
        crate::rank::rank(&mut graph),
        Err(crate::LayoutError::InvalidNetworkSimplexTree)
    );
    assert_eq!(graph.node_ids(), source_nodes);
    assert_eq!(graph.edge_keys(), source_edges);
    assert!(
        graph
            .nodes()
            .all(|id| graph.node(id).is_some_and(|node| node.rank.is_none()))
    );
}

#[test]
fn network_simplex_initial_longest_path_owns_cold_csr_once() {
    let source = ranking_graph();
    let simplify_units = simplify_work_units(&source).unwrap();
    let mut preparation_control = NoopWorkControl;
    let mut simplified = build_simplified_graph(&source, &mut preparation_control)
        .expect("the fixture simplify step succeeds");
    assert!(!simplified.is_adjacency_cache_current());
    let mut longest_control = RecordingWorkControl::default();
    util::longest_path_controlled(&mut simplified, &mut longest_control)
        .expect("the fixture longest-path step succeeds");
    assert_eq!(longest_control.charges.len(), 1);
    let longest_units = longest_control.charges[0];
    assert!(simplified.is_adjacency_cache_current());

    let exact_prefix = checked_add(simplify_units, longest_units).unwrap();
    let mut rejected_graph = ranking_graph();
    let source_nodes = rejected_graph.node_ids();
    let source_edges = rejected_graph.edge_keys();
    let mut rejected = RecordingWorkControl::with_limit(exact_prefix - 1);
    assert_eq!(
        network_simplex_controlled(&mut rejected_graph, &mut rejected),
        Err(RankError::Work(WorkError::Interrupted))
    );
    assert_eq!(rejected.charges, [simplify_units, longest_units]);
    assert_eq!(rejected.remaining, Some(longest_units - 1));
    assert_eq!(rejected_graph.node_ids(), source_nodes);
    assert_eq!(rejected_graph.edge_keys(), source_edges);
    assert!(rejected_graph.nodes().all(|id| {
        rejected_graph
            .node(id)
            .is_some_and(|node| node.rank.is_none())
    }));

    let mut admitted_graph = ranking_graph();
    let mut admitted = RecordingWorkControl::default();
    network_simplex_controlled(&mut admitted_graph, &mut admitted)
        .expect("the unbounded control admits network simplex");
    assert_eq!(admitted.charges[0], simplify_units);
    assert_eq!(admitted.charges[1], longest_units);
}

#[test]
fn compact_feasible_tree_matches_graph_with_late_numeric_nodes_and_a_second_root() {
    let make_graph = || {
        let mut graph = Graph::new(GraphOptions::default());
        for id in ["10", "other", "2", "1", "last"] {
            graph.set_node(id, NodeLabel::default());
        }
        for (v, w) in [("1", "10"), ("10", "2"), ("other", "last")] {
            graph.set_edge(v, w);
        }
        util::longest_path(&mut graph);
        graph
    };
    let mut reference = make_graph();
    let mut reference_work = RecordingWorkControl::default();
    let forest = feasible_tree::feasible_tree_controlled(&mut reference, &mut reference_work)
        .expect("the public Graph path selects a feasible forest");
    assert_eq!(forest.node_ids(), ["1", "2", "10", "other", "last"]);
    assert_eq!(forest.node_ix("10"), Some(1));
    assert_eq!(forest.node_ix("2"), Some(2));
    let expected = build_tree_state_controlled(&forest, &reference, None, &mut reference_work)
        .expect("the reference state accepts the forest");
    assert_eq!(expected.roots.len(), 2);

    let mut compact = make_graph();
    let mut compact_work = RecordingWorkControl::default();
    let actual = build_initial_tree_state_controlled(&mut compact, &mut compact_work)
        .expect("the compact seed produces a feasible forest");
    assert_eq!(*actual, expected);
    assert_eq!(rank_snapshot(&compact), rank_snapshot(&reference));
    assert_eq!(compact_work.charges, reference_work.charges);
}

#[test]
fn compact_feasible_tree_uses_the_canonical_direction_for_a_bidirectional_edge() {
    let make_graph = || {
        let mut graph = Graph::new(GraphOptions::default());
        for (id, rank) in [("2", 0), ("10", 1)] {
            graph.set_node(
                id,
                NodeLabel {
                    rank: Some(rank),
                    ..NodeLabel::default()
                },
            );
        }
        graph.set_edge_with_label("2", "10", EdgeLabel::default());
        graph.set_edge_with_label(
            "10",
            "2",
            EdgeLabel {
                minlen: 2,
                weight: 7.0,
                ..EdgeLabel::default()
            },
        );
        graph
    };

    let mut reference = make_graph();
    let forest = feasible_tree::feasible_tree_controlled(&mut reference, &mut NoopWorkControl)
        .expect("the tight edge spans both nodes");
    let expected = TreeState::new(&forest, &reference).expect("the forest uses graph edges");
    let mut graph = make_graph();
    let compact = build_initial_tree_state_controlled(&mut graph, &mut NoopWorkControl)
        .expect("the compact forest uses graph edges");
    assert_eq!(compact.tree_edges_in_order, expected.tree_edges_in_order);

    let [edge] = compact.tree_edges_in_order.as_slice() else {
        panic!("a two-node tree must have one edge");
    };
    assert_eq!(tree_node_id(&compact, &graph, edge.tail_t_ix), "10");
    assert_eq!(tree_node_id(&compact, &graph, edge.head_t_ix), "2");
    assert_eq!((edge.minlen, edge.weight), (2, 7.0));
}

#[test]
fn dense_tree_matches_the_pinned_six_pivot_trace_without_slot_growth() {
    let source = multi_pivot_graph();
    let mut work_control = NoopWorkControl;
    let mut graph = build_simplified_graph(&source, &mut work_control)
        .expect("the fixture simplify step succeeds");
    util::longest_path_controlled(&mut graph, &mut work_control)
        .expect("the fixture ranks fit i32");
    let mut state = build_initial_tree_state_controlled(&mut graph, &mut work_control)
        .expect("the compact forest produces the initial pinned tree state");
    let edge_capacity = state.tree_edges_in_order.capacity();
    assert_eq!(state.edge_count(), 8);
    assert_eq!(
        ordered_tree_edges(&state, &graph),
        [
            ("n0", "n1"),
            ("n1", "n3"),
            ("n2", "n3"),
            ("n2", "n4"),
            ("n4", "n5"),
            ("n5", "n9"),
            ("n7", "n9"),
            ("n8", "n9"),
        ]
        .map(|(v, w)| (v.to_string(), w.to_string()))
    );
    let mut rank_by_ix = vec![0_i128; graph.node_slot_count()];
    graph.for_each_node_ix(|g_ix, _id, label| {
        rank_by_ix[g_ix] = i128::from(label.rank.unwrap_or(0));
    });

    let mut pivots = Vec::new();
    while let Some(leaving) = state.find_leave_edge_in_insertion_order() {
        assert!(pivots.len() < 16, "the pinned fixture must converge");
        let entering = enter_edge_fast(&mut state, &graph, &rank_by_ix, leaving)
            .expect("the pinned pivot has an entering edge");
        pivots.push((
            (
                tree_node_id(&state, &graph, leaving.v_ix).to_string(),
                tree_node_id(&state, &graph, leaving.w_ix).to_string(),
            ),
            (entering.v.clone(), entering.w.clone()),
        ));
        state
            .exchange_edge(leaving, &entering, &graph)
            .expect("the pinned pivot exchanges one live tree edge");
        assert_eq!(state.edge_count(), 8);
        assert_eq!(state.tree_edges_in_order.capacity(), edge_capacity);
        state
            .rebuild(&graph, None)
            .expect("the pivoted pinned tree rebuild succeeds");
        update_ranks_fast(&mut state, &mut graph, &mut rank_by_ix)
            .expect("the pinned fixture ranks fit i32");
    }

    assert_eq!(
        pivots,
        [
            (("n2", "n3"), ("n0", "n4")),
            (("n0", "n1"), ("n1", "n4")),
            (("n7", "n9"), ("n4", "n7")),
            (("n8", "n9"), ("n3", "n8")),
            (("n1", "n4"), ("n0", "n1")),
            (("n0", "n4"), ("n2", "n3")),
        ]
        .map(|((leave_v, leave_w), (enter_v, enter_w))| {
            (
                (leave_v.to_string(), leave_w.to_string()),
                (enter_v.to_string(), enter_w.to_string()),
            )
        })
    );
    assert_eq!(
        ordered_tree_edges(&state, &graph),
        [
            ("n1", "n3"),
            ("n2", "n4"),
            ("n4", "n5"),
            ("n5", "n9"),
            ("n4", "n7"),
            ("n3", "n8"),
            ("n0", "n1"),
            ("n2", "n3"),
        ]
        .map(|(v, w)| (v.to_string(), w.to_string()))
    );
    for (id, expected_rank) in [
        ("n0", -9),
        ("n1", -7),
        ("n2", -7),
        ("n3", -4),
        ("n4", -6),
        ("n5", -3),
        ("n7", -4),
        ("n8", -2),
        ("n9", 0),
    ] {
        assert_eq!(
            graph.node(id).and_then(|node| node.rank),
            Some(expected_rank)
        );
    }
}

#[test]
fn dense_pivot_precharges_exact_work_before_mutation() {
    let (mut expected_state, mut expected_graph, mut expected_ranks, expected_leaving) =
        first_multi_pivot_state();
    let exact = simplex_iteration_work_units(&expected_graph, &expected_state)
        .expect("the fixture work bound fits usize");
    assert_eq!(exact, 237);

    let mut measured = RecordingWorkControl::default();
    pivot_controlled(
        &mut expected_state,
        &mut expected_graph,
        &mut expected_ranks,
        expected_leaving,
        &mut measured,
    )
    .expect("the unbounded control admits the first pivot");
    assert_eq!(measured.charges, [exact]);
    let expected_rank_snapshot = rank_snapshot(&expected_graph);

    let (mut rejected_state, mut rejected_graph, mut rejected_ranks, rejected_leaving) =
        first_multi_pivot_state();
    let source_state = rejected_state.clone();
    let source_nodes = rejected_graph.node_ids();
    let source_edges = rejected_graph.edge_keys();
    let source_rank_snapshot = rank_snapshot(&rejected_graph);
    let source_ranks = rejected_ranks.clone();
    let mut rejected = RecordingWorkControl::with_limit(exact - 1);
    assert_eq!(
        pivot_controlled(
            &mut rejected_state,
            &mut rejected_graph,
            &mut rejected_ranks,
            rejected_leaving,
            &mut rejected,
        ),
        Err(RankError::Work(WorkError::Interrupted))
    );
    assert_eq!(rejected.charges, [exact]);
    assert_eq!(rejected.remaining, Some(exact - 1));
    assert_eq!(rejected_state, source_state);
    assert_eq!(rejected_graph.node_ids(), source_nodes);
    assert_eq!(rejected_graph.edge_keys(), source_edges);
    assert_eq!(rank_snapshot(&rejected_graph), source_rank_snapshot);
    assert_eq!(rejected_ranks, source_ranks);

    for limit in [exact, exact + 1] {
        let (mut state, mut graph, mut ranks, leaving) = first_multi_pivot_state();
        let mut work_control = RecordingWorkControl::with_limit(limit);
        pivot_controlled(
            &mut state,
            &mut graph,
            &mut ranks,
            leaving,
            &mut work_control,
        )
        .expect("equal and above dense-pivot budgets succeed");
        assert_eq!(work_control.charges, [exact]);
        assert_eq!(work_control.remaining, Some(limit - exact));
        assert_eq!(state, expected_state);
        assert_eq!(rank_snapshot(&graph), expected_rank_snapshot);
        assert_eq!(ranks, expected_ranks);
    }
}

#[test]
fn enter_edge_fast_breaks_equal_slack_ties_by_global_edge_order() {
    let mut graph = Graph::new(GraphOptions::default());
    graph.set_graph(GraphLabel::default());
    graph.set_default_node_label(NodeLabel::default);
    graph.set_default_edge_label(|| EdgeLabel {
        minlen: 1,
        weight: 1.0,
        ..EdgeLabel::default()
    });
    for node in ["r", "a", "b", "c", "x", "y"] {
        graph.set_node(node, NodeLabel::default());
    }
    for (tail, head) in [
        ("a", "r"),
        ("a", "b"),
        ("a", "c"),
        ("r", "x"),
        ("r", "y"),
        ("y", "c"),
        ("x", "b"),
    ] {
        graph.set_edge(tail, head);
    }

    let mut candidate_edge_ixs = Vec::new();
    graph.for_each_edge_entry_ix(|edge_ix, _tail_ix, _head_ix, key, _label| {
        if (key.v == "y" && key.w == "c") || (key.v == "x" && key.w == "b") {
            candidate_edge_ixs.push((key.clone(), edge_ix));
        }
    });
    assert_eq!(
        candidate_edge_ixs,
        [
            (
                EdgeKey {
                    v: "y".to_string(),
                    w: "c".to_string(),
                    name: None,
                },
                5,
            ),
            (
                EdgeKey {
                    v: "x".to_string(),
                    w: "b".to_string(),
                    name: None,
                },
                6,
            ),
        ]
    );

    let mut spanning_tree = tree(&[("r", "a"), ("a", "b"), ("a", "c"), ("r", "x"), ("r", "y")]);
    init_low_lim_values(&mut spanning_tree, Some("r"));

    let mut rank_by_ix = vec![0_i128; graph.node_slot_count()];
    rank_by_ix[graph.node_ix("b").expect("b is present")] = 1;
    rank_by_ix[graph.node_ix("c").expect("c is present")] = 1;
    let rank_by_ix_i32 = rank_by_ix
        .iter()
        .map(|rank| i32::try_from(*rank).expect("test ranks fit i32"))
        .collect::<Vec<_>>();

    let leaving = EdgeKey {
        v: "a".to_string(),
        w: "r".to_string(),
        name: None,
    };
    let expected = EdgeKey {
        v: "y".to_string(),
        w: "c".to_string(),
        name: None,
    };
    let slow = enter_edge(&spanning_tree, &graph, &rank_by_ix_i32, &leaving);
    assert_eq!(slow, expected);

    let mut tree_state =
        TreeState::new(&spanning_tree, &graph).expect("the tree is a graph-edge subset");
    tree_state
        .rebuild(&graph, Some("r"))
        .expect("the equal-slack tree rebuild succeeds");
    let a_ix = spanning_tree.node_ix("a").expect("a is present");
    let r_ix = spanning_tree.node_ix("r").expect("r is present");
    let leaving = OrderedTreeEdge {
        position: tree_state
            .tree_edges_in_order
            .iter()
            .position(|edge| {
                (edge.v_t_ix, edge.w_t_ix) == (a_ix, r_ix)
                    || (edge.v_t_ix, edge.w_t_ix) == (r_ix, a_ix)
            })
            .expect("the leaving edge is present"),
        v_ix: a_ix,
        w_ix: r_ix,
    };
    let fast = enter_edge_fast(&mut tree_state, &graph, &rank_by_ix, leaving)
        .expect("the fixture has an entering edge");

    assert_eq!(fast, slow);
}
