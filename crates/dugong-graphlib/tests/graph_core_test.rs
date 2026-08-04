use dugong_graphlib::{EdgeKey, Graph, GraphError, GraphOptions};

fn sorted(mut values: Vec<&str>) -> Vec<&str> {
    values.sort();
    values
}

fn sorted_owned(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values
}

fn sorted_edge_tuples(edges: Vec<EdgeKey>) -> Vec<(String, String, Option<String>)> {
    let mut out: Vec<(String, String, Option<String>)> =
        edges.into_iter().map(|e| (e.v, e.w, e.name)).collect();
    out.sort();
    out
}

#[test]
fn graph_initial_state_uses_default_directed_simple_options() {
    let g: Graph<(), (), Option<String>> = Graph::new(GraphOptions::default());

    assert_eq!(g.node_count(), 0);
    assert_eq!(g.edge_count(), 0);
    assert!(g.is_directed());
    assert!(!g.is_compound());
    assert!(!g.is_multigraph());
    assert_eq!(g.graph(), &None);
}

#[test]
fn graph_options_can_enable_undirected_compound_or_multigraph_modes() {
    let undirected: Graph<(), (), ()> = Graph::new(GraphOptions {
        directed: false,
        ..Default::default()
    });
    let compound: Graph<(), (), ()> = Graph::new(GraphOptions {
        compound: true,
        ..Default::default()
    });
    let multigraph: Graph<(), (), ()> = Graph::new(GraphOptions {
        multigraph: true,
        ..Default::default()
    });

    assert!(!undirected.is_directed());
    assert!(compound.is_compound());
    assert!(multigraph.is_multigraph());
}

#[test]
fn graph_label_can_be_set_and_read() {
    let mut g: Graph<(), (), Option<String>> = Graph::new(GraphOptions::default());

    g.set_graph(Some("graph label".to_string()));

    assert_eq!(g.graph().as_deref(), Some("graph label"));
}

#[test]
fn nodes_returns_inserted_node_ids() {
    let mut g: Graph<Option<i32>, (), ()> = Graph::new(GraphOptions::default());
    g.ensure_node("a");
    g.ensure_node("b");

    assert_eq!(sorted(g.nodes().collect()), vec!["a", "b"]);
}

#[test]
fn sources_returns_nodes_without_in_edges() {
    let mut g: Graph<(), (), ()> = Graph::new(GraphOptions::default());
    g.set_path(&["a", "b", "c"]);
    g.ensure_node("d");

    assert_eq!(sorted(g.sources()), vec!["a", "d"]);
}

#[test]
fn sinks_returns_nodes_without_out_edges() {
    let mut g: Graph<(), (), ()> = Graph::new(GraphOptions::default());
    g.set_path(&["a", "b", "c"]);
    g.ensure_node("d");

    assert_eq!(sorted(g.sinks()), vec!["c", "d"]);
}

#[test]
fn filter_nodes_copies_selected_graph_labels_edges_and_options() {
    let mut g: Graph<Option<i32>, Option<i32>, Option<String>> = Graph::new(GraphOptions {
        multigraph: true,
        compound: true,
        ..Default::default()
    });
    g.set_graph(Some("graph label".to_string()));
    g.set_node("a", Some(123));
    g.set_path(&["a", "b", "c"]);
    g.set_edge_named("a", "c", Some("named"), Some(Some(456)));

    let g2 = g.filter_nodes(|_| true);

    assert!(g2.is_directed());
    assert!(g2.is_multigraph());
    assert!(g2.is_compound());
    assert_eq!(g2.graph().as_deref(), Some("graph label"));
    assert_eq!(sorted(g2.nodes().collect()), vec!["a", "b", "c"]);
    assert_eq!(sorted(g2.successors("a")), vec!["b", "c"]);
    assert_eq!(sorted(g2.successors("b")), vec!["c"]);
    assert_eq!(g2.node("a"), Some(&Some(123)));
    assert_eq!(g2.edge("a", "c", Some("named")), Some(&Some(456)));

    let undirected: Graph<(), (), ()> = Graph::new(GraphOptions {
        directed: false,
        ..Default::default()
    });
    assert!(!undirected.filter_nodes(|_| true).is_directed());

    let simple: Graph<(), (), ()> = Graph::new(GraphOptions::default());
    assert!(!simple.filter_nodes(|_| true).is_multigraph());
    assert!(!simple.filter_nodes(|_| true).is_compound());
}

#[test]
fn filter_nodes_drops_rejected_nodes_and_incident_edges() {
    let mut g: Graph<(), (), ()> = Graph::new(GraphOptions::default());
    g.set_edge("a", "b");

    let g2 = g.filter_nodes(|v| v == "a");
    let empty = g.filter_nodes(|_| false);

    assert_eq!(g2.nodes().collect::<Vec<_>>(), vec!["a"]);
    assert!(g2.edge_keys().is_empty());
    assert!(empty.nodes().collect::<Vec<_>>().is_empty());
    assert!(empty.edge_keys().is_empty());
}

#[test]
fn filter_nodes_preserves_compound_subgraphs_and_promotes_missing_parent() {
    let mut g: Graph<(), (), ()> = Graph::new(GraphOptions {
        compound: true,
        ..Default::default()
    });
    g.set_parent("a", "parent");
    g.set_parent("parent", "root");

    let full = g.filter_nodes(|_| true);
    let promoted = g.filter_nodes(|v| v != "parent");

    assert_eq!(full.parent("a"), Some("parent"));
    assert_eq!(full.parent("parent"), Some("root"));
    assert_eq!(promoted.parent("a"), Some("root"));
}

#[test]
fn ensure_node_uses_default_label_for_new_nodes() {
    let mut g: Graph<Option<i32>, (), ()> = Graph::new(GraphOptions::default());
    g.set_default_node_label(|| Some(7));

    g.ensure_node("a");

    assert_eq!(g.node("a"), Some(&Some(7)));
}

#[test]
fn default_node_label_can_read_node_id() {
    let mut g: Graph<Option<String>, (), ()> = Graph::new(GraphOptions::default());
    g.set_default_node_label_with_id(|v| Some(format!("{v}-foo")));

    g.ensure_node("a");

    assert_eq!(g.node("a").and_then(|v| v.as_deref()), Some("a-foo"));
}

#[test]
fn default_node_label_is_not_used_if_explicit_label_is_set() {
    let mut g: Graph<Option<i32>, (), ()> = Graph::new(GraphOptions::default());
    g.set_default_node_label(|| Some(7));

    g.set_node("a", Some(3));

    assert_eq!(g.node("a"), Some(&Some(3)));
}

#[test]
fn ensure_node_does_not_change_existing_node_label() {
    let mut g: Graph<Option<i32>, (), ()> = Graph::new(GraphOptions::default());
    g.set_node("a", Some(3));
    g.set_default_node_label(|| Some(7));

    g.ensure_node("a");

    assert_eq!(g.node("a"), Some(&Some(3)));
}

#[test]
fn set_nodes_uses_default_labels_without_changing_existing_nodes() {
    let mut g: Graph<Option<String>, (), ()> = Graph::new(GraphOptions::default());
    g.set_default_node_label_with_id(|v| Some(format!("{v}-default")));
    g.set_node("a", Some("existing".to_string()));

    g.set_nodes(&["a", "b", "c"]);

    assert_eq!(g.node("a").and_then(|v| v.as_deref()), Some("existing"));
    assert_eq!(g.node("b").and_then(|v| v.as_deref()), Some("b-default"));
    assert_eq!(g.node("c").and_then(|v| v.as_deref()), Some("c-default"));
}

#[test]
fn set_nodes_with_label_sets_and_updates_all_node_labels() {
    let mut g: Graph<Option<String>, (), ()> = Graph::new(GraphOptions::default());

    g.set_nodes_with_label(&["a", "b", "c"], Some("foo".to_string()));

    assert_eq!(g.node("a").and_then(|v| v.as_deref()), Some("foo"));
    assert_eq!(g.node("b").and_then(|v| v.as_deref()), Some("foo"));
    assert_eq!(g.node("c").and_then(|v| v.as_deref()), Some("foo"));

    g.set_nodes_with_label(&["a", "b", "c"], Some("bar".to_string()));

    assert_eq!(g.node("a").and_then(|v| v.as_deref()), Some("bar"));
    assert_eq!(g.node("b").and_then(|v| v.as_deref()), Some("bar"));
    assert_eq!(g.node("c").and_then(|v| v.as_deref()), Some("bar"));
}

#[test]
fn set_node_is_idempotent_for_existing_node() {
    let mut g: Graph<Option<i32>, (), ()> = Graph::new(GraphOptions::default());
    g.set_node("a", Some(1));
    g.set_node("a", Some(1));

    assert_eq!(g.node("a"), Some(&Some(1)));
    assert_eq!(g.node_count(), 1);
}

#[test]
fn set_node_with_optional_label_can_clear_label_without_removing_node() {
    let mut g: Graph<Option<&str>, (), ()> = Graph::new(GraphOptions::default());

    assert_eq!(g.node("a"), None);

    g.set_node("a", Some("foo"));
    assert!(g.has_node("a"));
    assert_eq!(g.node("a"), Some(&Some("foo")));
    assert_eq!(g.node_count(), 1);

    g.set_node("a", None);
    assert!(g.has_node("a"));
    assert_eq!(g.node("a"), Some(&None));
    assert_eq!(g.node_count(), 1);
}

#[test]
fn remove_node_is_idempotent_and_removes_incident_edges() {
    let mut g: Graph<(), (), ()> = Graph::new(GraphOptions::default());
    g.set_edge("a", "b");
    g.set_edge("b", "c");

    assert!(g.remove_node("b"));
    assert!(!g.remove_node("b"));

    assert!(!g.has_node("b"));
    assert_eq!(g.edge_count(), 0);
}

#[test]
fn remove_node_removes_self_loop_once_with_cached_adjacency() {
    let mut g: Graph<(), (), ()> = Graph::new(GraphOptions {
        multigraph: true,
        ..Default::default()
    });
    g.set_edge("a", "b");
    g.set_edge_named("b", "b", Some("self"), Some(()));
    g.set_edge("b", "c");

    assert_eq!(g.successors("b"), vec!["b", "c"]);
    assert_eq!(g.predecessors("b"), vec!["a", "b"]);

    assert!(g.remove_node("b"));

    assert_eq!(g.edge_count(), 0);
    assert!(g.successors("a").is_empty());
    assert!(g.predecessors("c").is_empty());
}

#[test]
fn remove_nodes_batches_incident_edges_and_preserves_live_order() {
    let mut g: Graph<(), i32, ()> = Graph::new(GraphOptions {
        multigraph: true,
        compound: true,
        ..Default::default()
    });
    g.set_parent("a", "parent");
    g.set_parent("b", "parent");
    g.set_parent("child", "b");
    g.set_edge_named("a", "b", Some("ab"), Some(1));
    g.set_edge_named("c", "d", Some("cd"), Some(2));
    g.set_edge_named("b", "b", Some("self"), Some(3));
    g.set_edge_named("d", "a", Some("da"), Some(4));
    g.set_edge_named("b", "c", Some("bc"), Some(5));

    // Materialize the cache first: a batch must remain correct after cached queries and only
    // invalidate once when the mutation begins.
    assert_eq!(g.successors("b"), vec!["b", "c"]);
    assert_eq!(g.remove_nodes(["missing", "b", "b"].into_iter()), 1);

    assert!(!g.has_node("b"));
    assert_eq!(g.parent("child"), None);
    assert_eq!(g.children("parent"), vec!["a"]);
    assert_eq!(g.edge_count(), 2);
    assert_eq!(
        g.edge_keys()
            .into_iter()
            .map(|edge| (edge.v, edge.w, edge.name))
            .collect::<Vec<_>>(),
        vec![
            ("c".to_string(), "d".to_string(), Some("cd".to_string())),
            ("d".to_string(), "a".to_string(), Some("da".to_string())),
        ]
    );
    assert_eq!(g.edge("c", "d", Some("cd")), Some(&2));
    assert_eq!(g.edge("d", "a", Some("da")), Some(&4));
}

#[test]
fn remove_nodes_clears_removed_parents_and_children_together() {
    let mut g: Graph<(), (), ()> = Graph::new(GraphOptions {
        compound: true,
        ..Default::default()
    });
    g.set_parent("child", "middle");
    g.set_parent("middle", "root");
    g.set_parent("sibling", "root");

    assert_eq!(g.remove_nodes(["root", "middle"].into_iter()), 2);

    assert_eq!(g.parent("child"), None);
    assert_eq!(g.parent("sibling"), None);
    // Graphlib removes `root` first, promoting `middle` and `sibling`, then removes `middle` and
    // recreates `child` after the surviving `sibling` root property.
    assert_eq!(g.children_root(), vec!["sibling", "child"]);
    assert_eq!(g.remove_nodes(std::iter::empty::<&str>()), 0);
}

#[test]
fn remove_nodes_replays_requested_order_for_root_property_recreation() {
    let build = || {
        let mut g: Graph<(), (), ()> = Graph::new(GraphOptions {
            compound: true,
            ..Default::default()
        });
        for id in ["root", "middle", "child", "sibling"] {
            g.ensure_node(id);
        }
        g.set_parent("middle", "root");
        g.set_parent("child", "middle");
        g.set_parent("sibling", "root");
        g
    };

    let mut root_then_middle = build();
    let mut sequential = build();
    assert_eq!(
        root_then_middle.remove_nodes(["root", "middle"].into_iter()),
        2
    );
    assert!(sequential.remove_node("root"));
    assert!(sequential.remove_node("middle"));
    assert_eq!(root_then_middle.children_root(), sequential.children_root());
    assert_eq!(root_then_middle.children_root(), vec!["sibling", "child"]);

    let mut middle_then_root = build();
    let mut sequential = build();
    assert_eq!(
        middle_then_root.remove_nodes(["middle", "root"].into_iter()),
        2
    );
    assert!(sequential.remove_node("middle"));
    assert!(sequential.remove_node("root"));
    assert_eq!(middle_then_root.children_root(), sequential.children_root());
    assert_eq!(middle_then_root.children_root(), vec!["child", "sibling"]);
}

#[test]
fn remove_nodes_does_not_mutate_if_the_target_iterator_panics() {
    let mut g: Graph<(), (), ()> = Graph::new(GraphOptions::default());
    g.set_edge("a", "b");

    let mut first = true;
    let ids = std::iter::from_fn(move || {
        if first {
            first = false;
            Some("a")
        } else {
            panic!("target iterator failed")
        }
    });
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| g.remove_nodes(ids)));

    assert!(result.is_err());
    assert!(g.has_node("a"));
    assert!(g.has_node("b"));
    assert!(g.has_edge("a", "b", None));
    assert_eq!(g.node_count(), 2);
    assert_eq!(g.edge_count(), 1);
}

#[test]
fn set_parent_ix_rejects_removed_node_slots() {
    let mut g: Graph<(), (), ()> = Graph::new(GraphOptions {
        compound: true,
        ..Default::default()
    });
    for id in ["child", "sibling", "removed_parent", "tail"] {
        g.set_node(id, ());
    }

    assert!(g.remove_node("removed_parent"));
    g.set_parent_ix(0, 2);
    assert_eq!(g.parent("child"), None);

    // Removing the tail trims both trailing slots. No stale parent index may survive and make a
    // later batch index outside its live slot vector.
    assert!(g.remove_node("tail"));
    assert_eq!(g.remove_nodes(["sibling"].into_iter()), 1);
    assert_eq!(g.children_root(), vec!["child"]);
}

#[test]
fn slot_counts_report_the_actual_graph_wide_scan_span() {
    let mut g: Graph<(), (), ()> = Graph::new(GraphOptions::default());
    for id in ["a", "b", "c", "d"] {
        g.ensure_node(id);
    }
    g.set_edge("a", "b");
    g.set_edge("b", "c");
    g.set_edge("c", "d");

    assert_eq!(g.node_count(), 4);
    assert_eq!(g.node_slot_count(), 4);
    assert_eq!(g.edge_count(), 3);
    assert_eq!(g.edge_slot_count(), 3);

    assert!(g.remove_edge("b", "c", None));
    assert!(g.remove_node("b"));
    assert_eq!(g.node_count(), 3);
    assert_eq!(g.node_slot_count(), 4);
    assert_eq!(g.edge_count(), 1);
    assert_eq!(g.edge_slot_count(), 3);

    assert!(g.remove_edge("c", "d", None));
    assert!(g.remove_node("d"));
    assert_eq!(g.node_count(), 2);
    assert_eq!(g.node_slot_count(), 3);
    assert_eq!(g.edge_count(), 0);
    assert_eq!(g.edge_slot_count(), 0);
}

#[test]
fn set_parent_repeating_the_same_relation_moves_the_child_to_the_end() {
    let mut g: Graph<(), (), ()> = Graph::new(GraphOptions {
        compound: true,
        ..Default::default()
    });
    for id in ["parent", "a", "b"] {
        g.ensure_node(id);
    }
    g.set_parent("a", "parent");
    g.set_parent("b", "parent");

    g.set_parent("a", "parent");

    assert_eq!(g.children("parent"), vec!["b", "a"]);
}

#[test]
fn set_parent_repeating_array_index_keys_preserves_javascript_enumeration_order() {
    let mut g: Graph<(), (), ()> = Graph::new(GraphOptions {
        compound: true,
        ..Default::default()
    });
    for id in ["parent", "0", "4294967294", "00", "01", "-1", "4294967295"] {
        g.ensure_node(id);
    }
    for child in ["0", "4294967294", "00", "01", "-1", "4294967295"] {
        g.set_parent(child, "parent");
    }

    g.set_parent("0", "parent");
    g.set_parent("4294967294", "parent");
    assert_eq!(
        g.children("parent"),
        vec!["0", "4294967294", "00", "01", "-1", "4294967295"]
    );

    // Leading-zero, negative, and `2^32 - 1` keys are ordinary strings, not JavaScript array
    // indexes. Recreating any of those properties moves it to the end of the ordinary segment.
    g.set_parent("00", "parent");
    g.set_parent("-1", "parent");
    assert_eq!(
        g.children("parent"),
        vec!["0", "4294967294", "01", "4294967295", "00", "-1"]
    );
}

#[test]
fn node_enumeration_follows_javascript_object_keys_across_all_entry_points() {
    let mut g: Graph<usize, (), ()> = Graph::new(GraphOptions::default());
    for (index, id) in [
        "z",
        "4294967295",
        "00",
        "1",
        "-1",
        "0",
        "01",
        "4294967294",
        "1.0",
        "9007199254740991",
        "2",
        "a",
    ]
    .into_iter()
    .enumerate()
    {
        g.set_node(id, index);
    }
    let expected = vec![
        "0",
        "1",
        "2",
        "4294967294",
        "z",
        "4294967295",
        "00",
        "-1",
        "01",
        "1.0",
        "9007199254740991",
        "a",
    ];

    assert_eq!(g.nodes().collect::<Vec<_>>(), expected);
    assert_eq!(g.node_ids(), expected);
    let mut immutable_order = Vec::new();
    g.for_each_node(|id, _| immutable_order.push(id.to_string()));
    assert_eq!(immutable_order, expected);
    let mut indexed_order = Vec::new();
    g.for_each_node_ix(|_, id, _| indexed_order.push(id.to_string()));
    assert_eq!(indexed_order, expected);
    let mut mutable_order = Vec::new();
    g.for_each_node_mut(|id, label| {
        mutable_order.push(id.to_string());
        *label += 1;
    });
    assert_eq!(mutable_order, expected);
    assert_eq!(g.array_index_node_count(), 4);
    assert_eq!(g.node_order_slot_count(), expected.len());

    assert!(g.remove_node("z"));
    assert!(g.remove_node("1"));
    assert!(g.node_order_slot_count() >= g.node_count());
    g.set_node("z", 100);
    g.set_node("1", 101);
    assert_eq!(
        g.nodes().collect::<Vec<_>>(),
        vec![
            "0",
            "1",
            "2",
            "4294967294",
            "4294967295",
            "00",
            "-1",
            "01",
            "1.0",
            "9007199254740991",
            "a",
            "z",
        ]
    );
}

#[test]
fn descending_array_index_construction_curve_keeps_linear_order_storage() {
    for width in (0..=12).map(|shift| 1usize << shift) {
        let mut g: Graph<(), (), ()> = Graph::with_capacity(
            GraphOptions {
                compound: true,
                ..Default::default()
            },
            width,
            0,
        );
        for index in (0..width).rev() {
            g.ensure_node(index.to_string());
        }

        let expected = (0..width)
            .map(|index| index.to_string())
            .collect::<Vec<_>>();
        assert_eq!(g.node_ids(), expected);
        assert_eq!(g.array_index_node_count(), width);
        assert_eq!(g.node_order_slot_count(), width);
        assert_eq!(g.root_array_index_child_count(), width);
        assert_eq!(g.root_child_order_slot_count(), width);
    }
}

#[test]
fn node_creation_entry_points_share_javascript_object_keys_order() {
    let inserted = ["4294967295", "2", "01", "1", "-1", "0"];
    let expected = vec!["0", "1", "2", "4294967295", "01", "-1"];

    let mut set_nodes: Graph<(), (), ()> = Graph::new(GraphOptions::default());
    set_nodes.set_nodes(&inserted);
    assert_eq!(set_nodes.nodes().collect::<Vec<_>>(), expected);

    let mut labeled: Graph<usize, (), ()> = Graph::new(GraphOptions::default());
    labeled.set_nodes_with_label(&inserted, 7);
    assert_eq!(labeled.nodes().collect::<Vec<_>>(), expected);

    let mut implicit: Graph<(), (), ()> = Graph::new(GraphOptions::default());
    implicit.set_edge("4294967295", "2");
    implicit.set_edge("01", "1");
    implicit.set_edge("-1", "0");
    assert_eq!(implicit.nodes().collect::<Vec<_>>(), expected);
}

#[test]
fn compound_children_follow_javascript_object_keys_boundary_order() {
    let mut g: Graph<(), (), ()> = Graph::new(GraphOptions {
        compound: true,
        ..Default::default()
    });
    for id in [
        "parent",
        "00",
        "2",
        "01",
        "-1",
        "1.0",
        "4294967295",
        "9007199254740991",
        "4294967294",
        "0",
        "1",
    ] {
        g.ensure_node(id);
    }
    assert_eq!(
        g.children_root(),
        vec![
            "0",
            "1",
            "2",
            "4294967294",
            "parent",
            "00",
            "01",
            "-1",
            "1.0",
            "4294967295",
            "9007199254740991",
        ]
    );
    for child in [
        "00",
        "2",
        "01",
        "-1",
        "1.0",
        "4294967295",
        "9007199254740991",
        "4294967294",
        "0",
        "1",
    ] {
        g.set_parent(child, "parent");
    }

    assert_eq!(
        g.children("parent"),
        vec![
            "0",
            "1",
            "2",
            "4294967294",
            "00",
            "01",
            "-1",
            "1.0",
            "4294967295",
            "9007199254740991",
        ]
    );
}

#[test]
fn root_children_follow_object_keys_across_parent_and_clear_lifecycle() {
    let mut g: Graph<(), (), ()> = Graph::new(GraphOptions {
        compound: true,
        ..Default::default()
    });
    for id in ["first", "2", "1", "tail", "parent"] {
        g.ensure_node(id);
    }
    assert_eq!(g.children_root(), vec!["1", "2", "first", "tail", "parent"]);

    g.set_parent("first", "parent");
    g.set_parent("2", "parent");
    assert_eq!(g.children_root(), vec!["1", "tail", "parent"]);

    g.clear_parent("first");
    g.clear_parent("2");
    assert_eq!(g.children_root(), vec!["1", "2", "tail", "parent", "first"]);
}

#[test]
fn clear_parent_recreates_an_existing_root_ordinary_property() {
    let mut g: Graph<(), (), ()> = Graph::new(GraphOptions {
        compound: true,
        ..Default::default()
    });
    for id in ["a", "b", "2", "1"] {
        g.ensure_node(id);
    }

    g.clear_parent("a");
    assert_eq!(g.children_root(), vec!["1", "2", "b", "a"]);

    g.clear_parent("1");
    assert_eq!(g.children_root(), vec!["1", "2", "b", "a"]);
}

#[test]
fn cross_parent_moves_use_target_object_keys_order() {
    let mut g: Graph<(), (), ()> = Graph::new(GraphOptions {
        compound: true,
        ..Default::default()
    });
    for id in ["left", "right", "a", "2", "right-tail", "3"] {
        g.ensure_node(id);
    }
    g.set_parent("a", "left");
    g.set_parent("2", "left");
    g.set_parent("right-tail", "right");
    g.set_parent("3", "right");

    g.set_parent("a", "right");
    g.set_parent("2", "right");

    assert!(g.children("left").is_empty());
    assert_eq!(g.children("right"), vec!["2", "3", "right-tail", "a"]);
}

#[test]
fn removing_a_parent_promotes_children_in_object_keys_order() {
    let mut g: Graph<(), (), ()> = Graph::new(GraphOptions {
        compound: true,
        ..Default::default()
    });
    for id in [
        "root-anchor",
        "parent",
        "ordinary-a",
        "2",
        "1",
        "ordinary-b",
    ] {
        g.ensure_node(id);
    }
    for child in ["ordinary-a", "2", "ordinary-b", "1"] {
        g.set_parent(child, "parent");
    }

    assert!(g.remove_node("parent"));

    assert_eq!(
        g.children_root(),
        vec!["1", "2", "root-anchor", "ordinary-a", "ordinary-b"]
    );
    for child in ["ordinary-a", "2", "1", "ordinary-b"] {
        assert_eq!(g.parent(child), None);
    }
}

#[test]
fn batch_parent_removal_matches_pinned_object_keys_child_promotion() {
    let build = || {
        let mut g: Graph<(), (), ()> = Graph::new(GraphOptions {
            compound: true,
            ..Default::default()
        });
        for id in [
            "anchor",
            "parent",
            "4294967295",
            "2",
            "01",
            "1",
            "-1",
            "0",
            "tail",
        ] {
            g.ensure_node(id);
        }
        for child in ["4294967295", "2", "01", "1", "-1", "0"] {
            g.set_parent(child, "parent");
        }
        g
    };

    let mut batch = build();
    let mut sequential = build();
    assert_eq!(batch.remove_nodes(["parent"].into_iter()), 1);
    assert!(sequential.remove_node("parent"));

    assert_eq!(batch.node_ids(), sequential.node_ids());
    assert_eq!(batch.children_root(), sequential.children_root());
    assert_eq!(
        batch.children_root(),
        vec!["0", "1", "2", "anchor", "tail", "4294967295", "01", "-1"]
    );
}

#[test]
fn deleting_and_recreating_nodes_recreates_ordinary_root_properties() {
    let mut g: Graph<(), (), ()> = Graph::new(GraphOptions {
        compound: true,
        ..Default::default()
    });
    for id in ["anchor", "victim", "2", "1", "tail"] {
        g.ensure_node(id);
    }

    assert!(g.remove_node("victim"));
    assert!(g.remove_node("1"));
    g.ensure_node("victim");
    g.ensure_node("1");

    assert_eq!(
        g.children_root(),
        vec!["1", "2", "anchor", "tail", "victim"]
    );
}

#[test]
fn compaction_preserves_compound_object_keys_order() {
    let mut g: Graph<(), (), ()> = Graph::new(GraphOptions {
        compound: true,
        ..Default::default()
    });
    for id in ["root-a", "parent", "b", "2", "1", "a", "removed", "root-b"] {
        g.ensure_node(id);
    }
    for child in ["b", "2", "1", "a"] {
        g.set_parent(child, "parent");
    }
    g.set_parent("b", "parent");
    assert!(g.remove_node("removed"));
    assert!(g.remove_node("root-a"));
    g.ensure_node("root-a");
    let nodes_before = g.node_ids();
    let root_before = g
        .children_root()
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let children_before = g
        .children("parent")
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();

    assert!(g.compact_if_sparse(1.01));

    assert_eq!(
        g.children_root(),
        root_before.iter().map(String::as_str).collect::<Vec<_>>()
    );
    assert_eq!(
        g.children("parent"),
        children_before
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
    );
    assert_eq!(g.children("parent"), vec!["1", "2", "a", "b"]);
    assert_eq!(g.nodes().collect::<Vec<_>>(), nodes_before);
    assert_eq!(g.node_ids(), nodes_before);
    let mut immutable = Vec::new();
    g.for_each_node(|id, _| immutable.push(id.to_string()));
    assert_eq!(immutable, nodes_before);
    let mut indexed = Vec::new();
    g.for_each_node_ix(|_, id, _| indexed.push(id.to_string()));
    assert_eq!(indexed, nodes_before);
    let mut mutable = Vec::new();
    g.for_each_node_mut(|id, _| mutable.push(id.to_string()));
    assert_eq!(mutable, nodes_before);
}

#[test]
fn filter_nodes_replays_parent_links_in_pinned_object_keys_node_order() {
    let mut g: Graph<(), (), ()> = Graph::new(GraphOptions {
        compound: true,
        ..Default::default()
    });
    for id in ["parent", "b", "2", "1", "a", "root"] {
        g.ensure_node(id);
    }
    for child in ["b", "2", "1", "a"] {
        g.set_parent(child, "parent");
    }
    g.set_parent("b", "parent");
    assert_eq!(g.children("parent"), vec!["1", "2", "a", "b"]);

    let copy = g.filter_nodes(|_| true);

    // Pinned graphlib's filterNodes iterates copy.nodes(), so ordinary child creation follows the
    // node object's Object.keys order rather than the source parent's last reassignment order.
    assert_eq!(copy.children("parent"), vec!["1", "2", "b", "a"]);
}

#[test]
fn filter_nodes_promotes_boundary_keys_in_pinned_object_keys_order() {
    let mut g: Graph<(), (), ()> = Graph::new(GraphOptions {
        compound: true,
        ..Default::default()
    });
    for id in [
        "anchor",
        "parent",
        "4294967295",
        "2",
        "01",
        "1",
        "-1",
        "0",
        "tail",
    ] {
        g.ensure_node(id);
    }
    for child in ["4294967295", "2", "01", "1", "-1", "0"] {
        g.set_parent(child, "parent");
    }

    let copy = g.filter_nodes(|id| id != "parent");

    assert_eq!(
        copy.node_ids(),
        vec!["0", "1", "2", "anchor", "4294967295", "01", "-1", "tail"]
    );
    assert_eq!(
        copy.children_root(),
        vec!["0", "1", "2", "anchor", "4294967295", "01", "-1", "tail"]
    );
}

#[test]
fn unparented_parent_batch_merges_numeric_and_ordinary_children_linearly() {
    let mut g: Graph<(), (), ()> = Graph::new(GraphOptions {
        compound: true,
        ..Default::default()
    });
    for id in ["parent", "existing", "2", "ordinary-b", "1", "ordinary-a"] {
        g.ensure_node(id);
    }
    g.set_parent("existing", "parent");
    let parent_ix = g.node_ix("parent").unwrap();
    let assignments =
        ["2", "ordinary-b", "1", "ordinary-a"].map(|child| (g.node_ix(child).unwrap(), parent_ix));

    g.try_set_unparented_parents_ix(&assignments)
        .expect("unique first assignments satisfy the batch contract");

    assert_eq!(
        g.children("parent"),
        vec!["1", "2", "existing", "ordinary-b", "ordinary-a"]
    );
    assert_eq!(g.children_root(), vec!["parent"]);
}

#[test]
fn unparented_parent_batch_matches_sequential_first_assignments_and_child_order() {
    let build = || {
        let mut graph: Graph<(), (), ()> = Graph::new(GraphOptions {
            compound: true,
            ..Default::default()
        });
        for id in ["left", "right", "existing", "a", "b", "c", "d"] {
            graph.ensure_node(id);
        }
        graph.set_parent("existing", "right");
        graph
    };
    let mut batch = build();
    let mut sequential = build();
    let ids = [("b", "right"), ("a", "right"), ("c", "left"), ("d", "left")];
    let assignments = ids
        .iter()
        .map(|&(child, parent)| {
            (
                batch.node_ix(child).unwrap(),
                batch.node_ix(parent).unwrap(),
            )
        })
        .collect::<Vec<_>>();

    batch
        .try_set_unparented_parents_ix(&assignments)
        .expect("first parent assignments satisfy the batch contract");
    for (child, parent) in ids {
        let child_ix = sequential.node_ix(child).unwrap();
        let parent_ix = sequential.node_ix(parent).unwrap();
        sequential.set_parent_ix(child_ix, parent_ix);
    }

    for id in ["left", "right", "existing", "a", "b", "c", "d"] {
        assert_eq!(batch.parent(id), sequential.parent(id));
        assert_eq!(batch.children(id), sequential.children(id));
    }
    assert_eq!(batch.children("left"), vec!["c", "d"]);
    assert_eq!(batch.children("right"), vec!["existing", "b", "a"]);
}

#[test]
fn unparented_parent_batch_rejects_a_cycle_without_partial_mutation() {
    let mut g: Graph<(), (), ()> = Graph::new(GraphOptions {
        compound: true,
        ..Default::default()
    });
    for id in ["root", "a", "b", "c", "d"] {
        g.ensure_node(id);
    }
    g.set_parent("c", "b");
    g.set_parent("b", "a");
    g.set_parent("d", "root");

    let a_ix = g.node_ix("a").unwrap();
    let b_ix = g.node_ix("b").unwrap();
    let c_ix = g.node_ix("c").unwrap();
    let d_ix = g.node_ix("d").unwrap();
    let root_before = g
        .children_root()
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let before = ["root", "a", "b", "c", "d"].map(|id| {
        (
            id,
            g.parent(id).map(str::to_string),
            g.children(id)
                .into_iter()
                .map(str::to_string)
                .collect::<Vec<_>>(),
        )
    });

    let err = match g.try_set_unparented_parents_ix(&[(a_ix, c_ix), (d_ix, b_ix)]) {
        Ok(_) => panic!("cycle batch unexpectedly succeeded"),
        Err(err) => err,
    };

    assert_eq!(
        err,
        GraphError::ParentCycle {
            child_ix: a_ix,
            parent_ix: c_ix,
        }
    );
    let after = ["root", "a", "b", "c", "d"].map(|id| {
        (
            id,
            g.parent(id).map(str::to_string),
            g.children(id)
                .into_iter()
                .map(str::to_string)
                .collect::<Vec<_>>(),
        )
    });
    assert_eq!(after, before);
    assert_eq!(
        g.children_root(),
        root_before.iter().map(String::as_str).collect::<Vec<_>>()
    );
}

#[test]
fn unparented_parent_batch_reports_the_first_transient_cycle_like_graphlib() {
    let mut g: Graph<(), (), ()> = Graph::new(GraphOptions {
        compound: true,
        ..Default::default()
    });
    for id in ["a", "b", "root"] {
        g.ensure_node(id);
    }
    g.set_parent("b", "a");
    let a_ix = g.node_ix("a").unwrap();
    let b_ix = g.node_ix("b").unwrap();
    let root_ix = g.node_ix("root").unwrap();

    let error = match g.try_set_unparented_parents_ix(&[(a_ix, b_ix), (b_ix, root_ix)]) {
        Ok(_) => panic!("the first sequential assignment unexpectedly accepted a cycle"),
        Err(error) => error,
    };

    assert_eq!(
        error,
        GraphError::ParentCycle {
            child_ix: a_ix,
            parent_ix: b_ix,
        }
    );
    assert_eq!(g.parent("a"), None);
    assert_eq!(g.parent("b"), Some("a"));
    assert_eq!(g.children("a"), vec!["b"]);
    assert!(g.children("root").is_empty());
}

#[test]
fn unparented_parent_batch_ignores_removed_and_out_of_range_slots() {
    let mut g: Graph<(), (), ()> = Graph::new(GraphOptions {
        compound: true,
        ..Default::default()
    });
    for id in ["root", "kept", "removed", "tail"] {
        g.ensure_node(id);
    }
    let root_ix = g.node_ix("root").unwrap();
    let kept_ix = g.node_ix("kept").unwrap();
    let removed_ix = g.node_ix("removed").unwrap();
    assert!(g.remove_node("removed"));

    g.try_set_unparented_parents_ix(&[
        (removed_ix, root_ix),
        (kept_ix, usize::MAX),
        (usize::MAX, root_ix),
        (kept_ix, root_ix),
    ])
    .expect("invalid slots match set_parent_ix no-op semantics");

    assert_eq!(g.parent("kept"), Some("root"));
    assert_eq!(g.children("root"), vec!["kept"]);
}

#[test]
fn unparented_parent_batch_rejects_reparenting_and_duplicate_children_atomically() {
    let mut g: Graph<(), (), ()> = Graph::new(GraphOptions {
        compound: true,
        ..Default::default()
    });
    for id in ["left", "right", "a", "b"] {
        g.ensure_node(id);
    }
    g.set_parent("a", "left");
    let left_ix = g.node_ix("left").unwrap();
    let right_ix = g.node_ix("right").unwrap();
    let a_ix = g.node_ix("a").unwrap();
    let b_ix = g.node_ix("b").unwrap();

    let reparent_error = match g.try_set_unparented_parents_ix(&[(b_ix, left_ix), (a_ix, right_ix)])
    {
        Ok(_) => panic!("a reparenting batch unexpectedly succeeded"),
        Err(error) => error,
    };
    assert_eq!(
        reparent_error,
        GraphError::ParentBatchRequiresUnparentedChild { child_ix: a_ix }
    );
    assert_eq!(g.parent("b"), None);

    let duplicate_error =
        match g.try_set_unparented_parents_ix(&[(b_ix, left_ix), (b_ix, right_ix)]) {
            Ok(_) => panic!("a duplicate-child batch unexpectedly succeeded"),
            Err(error) => error,
        };
    assert_eq!(
        duplicate_error,
        GraphError::ParentBatchRequiresUnparentedChild { child_ix: b_ix }
    );
    assert_eq!(g.parent("b"), None);
}

#[test]
fn unparented_parent_batch_reports_the_first_cycle_closing_assignment() {
    let mut g: Graph<(), (), ()> = Graph::new(GraphOptions {
        compound: true,
        ..Default::default()
    });
    for id in ["a", "b", "c", "d"] {
        g.ensure_node(id);
    }
    let a_ix = g.node_ix("a").unwrap();
    let b_ix = g.node_ix("b").unwrap();
    let c_ix = g.node_ix("c").unwrap();
    let d_ix = g.node_ix("d").unwrap();

    let error = match g.try_set_unparented_parents_ix(&[
        (c_ix, d_ix),
        (a_ix, b_ix),
        (b_ix, a_ix),
        (d_ix, c_ix),
    ]) {
        Ok(_) => panic!("the first cycle-closing assignment unexpectedly succeeded"),
        Err(error) => error,
    };

    assert_eq!(
        error,
        GraphError::ParentCycle {
            child_ix: b_ix,
            parent_ix: a_ix,
        }
    );
    assert_eq!(g.parent("a"), None);
    assert_eq!(g.parent("b"), None);
    assert_eq!(g.parent("c"), None);
    assert_eq!(g.parent("d"), None);
}

#[test]
fn unparented_parent_batch_width_curve_preserves_first_insertion_order() {
    for width in (0..=12).map(|shift| 1usize << shift) {
        let mut g: Graph<(), (), ()> = Graph::with_capacity(
            GraphOptions {
                compound: true,
                ..Default::default()
            },
            width + 1,
            0,
        );
        g.ensure_node("root");
        for index in 0..width {
            g.ensure_node(format!("child-{index}"));
        }
        let root_ix = g.node_ix("root").unwrap();
        let assignments = (0..width)
            .map(|index| (g.node_ix(&format!("child-{index}")).unwrap(), root_ix))
            .collect::<Vec<_>>();

        g.try_set_unparented_parents_ix(&assignments)
            .expect("wide first assignments satisfy the batch contract");

        let children = g.children("root");
        assert_eq!(children.len(), width);
        assert_eq!(children.first().copied(), Some("child-0"));
        let last_child = format!("child-{}", width - 1);
        assert_eq!(children.last().copied(), Some(last_child.as_str()));
    }
}

#[test]
fn unparented_parent_batch_deep_chain_curve_is_iterative_and_order_stable() {
    for depth in (0..=12).map(|shift| 1usize << shift) {
        let mut g: Graph<(), (), ()> = Graph::with_capacity(
            GraphOptions {
                compound: true,
                ..Default::default()
            },
            depth + 1,
            0,
        );
        for index in 0..=depth {
            g.ensure_node(format!("node-{index}"));
        }
        let assignments = (1..=depth)
            .rev()
            .map(|index| {
                (
                    g.node_ix(&format!("node-{index}")).unwrap(),
                    g.node_ix(&format!("node-{}", index - 1)).unwrap(),
                )
            })
            .collect::<Vec<_>>();

        g.try_set_unparented_parents_ix(&assignments)
            .expect("deep first assignments satisfy the batch contract");

        assert_eq!(g.parent("node-0"), None);
        let deepest = format!("node-{depth}");
        let expected_parent = format!("node-{}", depth - 1);
        assert_eq!(g.parent(&deepest), Some(expected_parent.as_str()));
        assert_eq!(g.children("node-0"), vec!["node-1"]);
    }
}

fn batch_reference_graph(directed: bool) -> Graph<(), i32, ()> {
    let mut g = Graph::new(GraphOptions {
        directed,
        multigraph: true,
        compound: true,
    });
    for id in [
        "root", "middle", "old", "keep_a", "drop_a", "keep_b", "drop_b", "tail",
    ] {
        g.set_node(id, ());
    }
    assert!(g.remove_node("old"));
    g.set_parent("middle", "root");
    g.set_parent("keep_a", "middle");
    g.set_parent("drop_a", "middle");
    g.set_parent("keep_b", "root");
    g.set_parent("drop_b", "root");
    g.set_edge_named("keep_a", "drop_a", Some("parallel_1"), Some(1));
    g.set_edge_named("drop_a", "keep_a", Some("parallel_2"), Some(2));
    g.set_edge_named("keep_a", "keep_b", Some("survivor"), Some(3));
    g.set_edge_named("drop_b", "drop_b", Some("self"), Some(4));
    g.set_edge_named("tail", "keep_b", Some("tail"), Some(5));
    g
}

#[test]
fn remove_nodes_matches_sequential_removal_across_graph_modes() {
    let targets = ["drop_b", "missing", "middle", "drop_a", "drop_b", "tail"];

    for directed in [true, false] {
        let mut batch = batch_reference_graph(directed);
        let mut sequential = batch_reference_graph(directed);

        assert_eq!(batch.remove_nodes(targets), 4);
        let sequential_count = targets
            .into_iter()
            .filter(|id| sequential.remove_node(id))
            .count();
        assert_eq!(sequential_count, 4);

        let batch_nodes: Vec<_> = batch.nodes().collect();
        assert_eq!(batch_nodes, sequential.nodes().collect::<Vec<_>>());
        assert_eq!(batch.edge_keys(), sequential.edge_keys());
        assert_eq!(batch.children_root(), sequential.children_root());
        for id in batch_nodes {
            assert_eq!(batch.parent(id), sequential.parent(id));
            assert_eq!(batch.children(id), sequential.children(id));
        }
    }
}

#[test]
fn set_edge_creates_endpoint_nodes_and_uses_default_edge_label() {
    let mut g: Graph<(), Option<i32>, ()> = Graph::new(GraphOptions::default());
    g.set_default_edge_label(|| Some(9));

    g.set_edge("a", "b");

    assert!(g.has_node("a"));
    assert!(g.has_node("b"));
    assert_eq!(g.edge("a", "b", None), Some(&Some(9)));
    assert_eq!(g.edge_count(), 1);
}

#[test]
fn default_edge_label_can_read_endpoints_and_name() {
    let mut g: Graph<(), Option<String>, ()> = Graph::new(GraphOptions {
        multigraph: true,
        ..Default::default()
    });
    g.set_default_edge_label_with_endpoints(|v, w, name| {
        Some(format!("{v}-{w}-{}-foo", name.unwrap_or("none")))
    });

    g.set_edge_named("a", "b", Some("name"), None);

    assert_eq!(
        g.edge("a", "b", Some("name")).and_then(|v| v.as_deref()),
        Some("a-b-name-foo")
    );
}

#[test]
fn panicking_default_edge_label_does_not_commit_adjacency() {
    let mut g: Graph<(), (), ()> = Graph::new(GraphOptions::default());
    g.set_default_edge_label(|| panic!("default edge label failed"));

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        g.set_edge("a", "b");
    }));

    assert!(result.is_err());
    assert!(g.has_node("a"));
    assert!(g.has_node("b"));
    assert_eq!(g.edge_count(), 0);
    assert!(g.edge_keys().is_empty());
    assert!(g.successors("a").is_empty());
    assert!(g.predecessors("b").is_empty());
}

#[test]
fn default_edge_label_does_not_change_existing_edge() {
    let mut g: Graph<(), Option<i32>, ()> = Graph::new(GraphOptions::default());
    g.set_edge("a", "b");
    g.set_default_edge_label(|| Some(9));

    assert_eq!(g.edge("a", "b", None), Some(&None));
}

#[test]
fn default_edge_label_is_not_used_if_explicit_label_is_set() {
    let mut g: Graph<(), Option<i32>, ()> = Graph::new(GraphOptions::default());
    g.set_default_edge_label(|| Some(9));

    g.set_edge_with_label("a", "b", Some(3));

    assert_eq!(g.edge("a", "b", None), Some(&Some(3)));
}

#[test]
fn default_edge_label_does_not_replace_existing_named_edge() {
    let mut g: Graph<(), Option<String>, ()> = Graph::new(GraphOptions {
        multigraph: true,
        ..Default::default()
    });
    g.set_edge_named("a", "b", Some("name"), Some(Some("old".to_string())));
    g.set_default_edge_label_with_endpoints(|_, _, _| Some("should not set this".to_string()));

    g.set_edge_named("a", "b", Some("name"), None);

    assert_eq!(
        g.edge("a", "b", Some("name")).and_then(|v| v.as_deref()),
        Some("old")
    );
}

#[test]
fn set_edge_with_label_updates_existing_edge_label() {
    let mut g: Graph<(), Option<i32>, ()> = Graph::new(GraphOptions::default());
    g.set_edge_with_label("a", "b", Some(1));
    g.set_edge_with_label("a", "b", Some(2));

    assert_eq!(g.edge("a", "b", None), Some(&Some(2)));
    assert_eq!(g.edge_count(), 1);
}

#[test]
fn set_edge_with_label_can_clear_optional_edge_label() {
    let mut g: Graph<(), Option<&str>, ()> = Graph::new(GraphOptions::default());
    g.set_edge_with_label("a", "b", Some("foo"));
    g.set_edge_with_label("a", "b", None);

    assert!(g.has_edge("a", "b", None));
    assert_eq!(g.edge("a", "b", None), Some(&None));
}

#[test]
fn set_edge_named_can_clear_optional_multiedge_label() {
    let mut g: Graph<(), Option<&str>, ()> = Graph::new(GraphOptions {
        multigraph: true,
        ..Default::default()
    });
    g.set_edge_named("a", "b", Some("name"), Some(Some("foo")));
    g.set_edge_named("a", "b", Some("name"), Some(None));

    assert!(g.has_edge("a", "b", Some("name")));
    assert_eq!(g.edge("a", "b", Some("name")), Some(&None));
}

#[test]
fn multigraph_preserves_named_edges() {
    let mut g: Graph<(), Option<i32>, ()> = Graph::new(GraphOptions {
        multigraph: true,
        ..Default::default()
    });

    g.set_edge_named("a", "b", Some("first"), Some(Some(1)));
    g.set_edge_named("a", "b", Some("second"), Some(Some(2)));

    assert_eq!(g.edge("a", "b", Some("first")), Some(&Some(1)));
    assert_eq!(g.edge("a", "b", Some("second")), Some(&Some(2)));
    assert_eq!(g.edge_count(), 2);
    assert!(!g.has_edge("a", "b", None));
}

#[test]
fn set_edge_named_is_noop_for_named_edge_on_non_multigraph() {
    let mut g: Graph<(), (), ()> = Graph::new(GraphOptions::default());

    g.set_edge_named("a", "b", Some("name"), None);

    assert_eq!(g.node_count(), 0);
    assert_eq!(g.edge_count(), 0);
    assert!(!g.has_edge("a", "b", Some("name")));
}

#[test]
fn try_set_edge_named_reports_named_edge_on_non_multigraph() {
    let mut g: Graph<(), (), ()> = Graph::new(GraphOptions::default());

    let err = match g.try_set_edge_named("a", "b", Some("name"), None) {
        Ok(_) => panic!("named edge on simple graph should report an error"),
        Err(err) => err,
    };

    assert_eq!(err, GraphError::NamedEdgeInNonMultigraph);
    assert_eq!(g.node_count(), 0);
    assert_eq!(g.edge_count(), 0);
}

#[test]
fn named_edge_queries_do_not_match_unnamed_edges_in_simple_graph() {
    let mut g: Graph<(), i32, ()> = Graph::new(GraphOptions::default());
    g.set_edge_with_label("a", "b", 5);

    assert!(g.has_edge("a", "b", None));
    assert!(!g.has_edge("a", "b", Some("name")));
    assert_eq!(g.edge("a", "b", Some("name")), None);
    assert!(!g.remove_edge("a", "b", Some("name")));
    assert!(g.has_edge("a", "b", None));
}

#[test]
fn edges_returns_inserted_edge_keys() {
    let mut g: Graph<(), (), ()> = Graph::new(GraphOptions::default());
    g.set_edge("a", "b");
    g.set_edge("b", "c");

    assert_eq!(
        sorted_edge_tuples(g.edge_keys()),
        vec![
            ("a".to_string(), "b".to_string(), None),
            ("b".to_string(), "c".to_string(), None)
        ]
    );
}

#[test]
fn set_path_creates_path_edges() {
    let mut g: Graph<(), (), ()> = Graph::new(GraphOptions::default());

    g.set_path(&["a", "b", "c"]);

    assert!(g.has_edge("a", "b", None));
    assert!(g.has_edge("b", "c", None));
}

#[test]
fn set_path_with_label_sets_and_updates_all_path_edge_labels() {
    let mut g: Graph<(), String, ()> = Graph::new(GraphOptions::default());

    g.set_path_with_label(&["a", "b", "c"], "foo".to_string());

    assert_eq!(g.edge("a", "b", None).map(String::as_str), Some("foo"));
    assert_eq!(g.edge("b", "c", None).map(String::as_str), Some("foo"));

    g.set_path_with_label(&["a", "b", "c"], "bar".to_string());

    assert_eq!(g.edge("a", "b", None).map(String::as_str), Some("bar"));
    assert_eq!(g.edge("b", "c", None).map(String::as_str), Some("bar"));
}

#[test]
fn set_edge_key_sets_simple_and_named_edge_labels() {
    let mut simple: Graph<(), String, ()> = Graph::new(GraphOptions::default());
    simple.set_edge_key(EdgeKey::new("a", "b", None::<String>), "value".to_string());

    assert_eq!(
        simple.edge("a", "b", None).map(String::as_str),
        Some("value")
    );

    let mut multi: Graph<(), String, ()> = Graph::new(GraphOptions {
        multigraph: true,
        ..Default::default()
    });
    multi.set_edge_key(EdgeKey::new("a", "b", Some("name")), "named".to_string());

    assert_eq!(
        multi.edge("a", "b", Some("name")).map(String::as_str),
        Some("named")
    );
}

#[test]
fn try_set_edge_key_reports_named_edge_on_non_multigraph() {
    let mut g: Graph<(), String, ()> = Graph::new(GraphOptions::default());

    let err = match g.try_set_edge_key(EdgeKey::new("a", "b", Some("name")), "value".to_string()) {
        Ok(_) => panic!("named edge key on simple graph should report an error"),
        Err(err) => err,
    };

    assert_eq!(err, GraphError::NamedEdgeInNonMultigraph);
    assert_eq!(g.node_count(), 0);
    assert_eq!(g.edge_count(), 0);
}

#[test]
fn edge_lookup_respects_direction_for_directed_graphs() {
    let mut g: Graph<(), i32, ()> = Graph::new(GraphOptions::default());
    g.set_edge_with_label("a", "b", 7);

    assert_eq!(g.edge("a", "b", None), Some(&7));
    assert_eq!(g.edge("b", "a", None), None);
    assert!(g.has_edge("a", "b", None));
    assert!(!g.has_edge("b", "a", None));
}

#[test]
fn edge_lookup_returns_none_for_missing_edges() {
    let g: Graph<(), i32, ()> = Graph::new(GraphOptions::default());

    assert_eq!(g.edge("a", "b", None), None);
    assert_eq!(g.edge("a", "b", Some("foo")), None);
    assert!(!g.has_edge("a", "b", None));
    assert!(!g.has_edge("a", "b", Some("foo")));
}

#[test]
fn edge_lookup_accepts_either_direction_for_undirected_graphs() {
    let mut g: Graph<(), i32, ()> = Graph::new(GraphOptions {
        directed: false,
        ..Default::default()
    });
    g.set_edge_with_label("a", "b", 7);

    assert_eq!(g.edge("a", "b", None), Some(&7));
    assert_eq!(g.edge("b", "a", None), Some(&7));
    assert!(g.has_edge("a", "b", None));
    assert!(g.has_edge("b", "a", None));
}

#[test]
fn undirected_edges_follow_graphlib_string_order_for_stringified_ids() {
    let mut g: Graph<(), String, ()> = Graph::new(GraphOptions {
        directed: false,
        ..Default::default()
    });
    g.set_edge_with_label("9", "10", "foo".to_string());

    assert_eq!(g.edge("9", "10", None).map(String::as_str), Some("foo"));
    assert_eq!(g.edge("10", "9", None).map(String::as_str), Some("foo"));
    assert!(g.has_edge("9", "10", None));
    assert!(g.has_edge("10", "9", None));

    let keys = g.edge_keys();
    assert_eq!(keys.len(), 1);
    assert_eq!(keys[0].v, "10");
    assert_eq!(keys[0].w, "9");
}

#[test]
fn predecessors_returns_node_predecessors() {
    let mut g: Graph<(), (), ()> = Graph::new(GraphOptions::default());
    g.set_edge("a", "b");
    g.set_edge("b", "c");
    g.set_edge("a", "a");

    assert_eq!(sorted(g.predecessors("a")), vec!["a"]);
    assert_eq!(sorted(g.predecessors("b")), vec!["a"]);
    assert_eq!(sorted(g.predecessors("c")), vec!["b"]);
}

#[test]
fn successors_returns_node_successors() {
    let mut g: Graph<(), (), ()> = Graph::new(GraphOptions::default());
    g.set_edge("a", "b");
    g.set_edge("b", "c");
    g.set_edge("a", "a");

    assert_eq!(sorted(g.successors("a")), vec!["a", "b"]);
    assert_eq!(sorted(g.successors("b")), vec!["c"]);
    assert!(g.successors("c").is_empty());
}

#[test]
fn directed_node_adjacency_deduplicates_parallel_edges_but_edge_queries_do_not() {
    let mut g: Graph<(), (), ()> = Graph::new(GraphOptions {
        multigraph: true,
        ..Default::default()
    });
    g.set_edge_named("a", "b", Some("first"), None);
    g.set_edge_named("a", "b", Some("second"), None);
    g.set_edge_named("a", "c", Some("third"), None);

    assert_eq!(g.successors("a"), vec!["b", "c"]);
    assert_eq!(g.predecessors("b"), vec!["a"]);
    assert_eq!(g.first_successor("a"), Some("b"));
    assert_eq!(g.first_predecessor("b"), Some("a"));

    let mut successors = Vec::new();
    g.extend_successors("a", &mut successors);
    assert_eq!(successors, vec!["b", "c"]);
    let mut predecessors = Vec::new();
    g.extend_predecessors("b", &mut predecessors);
    assert_eq!(predecessors, vec!["a"]);

    let mut visited_successors = Vec::new();
    g.for_each_successor("a", |node| visited_successors.push(node));
    assert_eq!(visited_successors, vec!["b", "c"]);
    let mut visited_predecessors = Vec::new();
    g.for_each_predecessor("b", |node| visited_predecessors.push(node));
    assert_eq!(visited_predecessors, vec!["a"]);

    assert_eq!(g.out_edges("a", Some("b")).len(), 2);
    assert_eq!(g.in_edges("b", Some("a")).len(), 2);
}

#[test]
fn directed_node_adjacency_preserves_first_edge_order_across_interleaved_sources() {
    let mut g: Graph<(), (), ()> = Graph::new(GraphOptions {
        multigraph: true,
        ..Default::default()
    });
    g.set_edge_named("a", "c", Some("a-c-first"), None);
    g.set_edge_named("x", "c", Some("x-c"), None);
    g.set_edge_named("a", "b", Some("a-b"), None);
    g.set_edge_named("a", "c", Some("a-c-parallel"), None);
    g.set_edge_named("y", "c", Some("y-c"), None);
    g.set_edge_named("a", "d", Some("a-d"), None);

    assert_eq!(g.successors("a"), vec!["c", "b", "d"]);
    assert_eq!(g.predecessors("c"), vec!["a", "x", "y"]);
    assert_eq!(g.out_edges("a", Some("c")).len(), 2);
    assert_eq!(g.in_edges("c", Some("a")).len(), 2);
}

#[test]
fn directed_node_adjacency_preserves_counted_key_lifecycle_through_removal_and_compaction() {
    let mut g: Graph<(), (), ()> = Graph::new(GraphOptions {
        multigraph: true,
        ..Default::default()
    });
    g.set_node("removed", ());
    g.set_edge_named("source", "b", Some("b-first"), None);
    g.set_edge_named("source", "c", Some("c"), None);
    g.set_edge_named("source", "b", Some("b-second"), None);
    g.set_edge_named("a", "target", Some("a-first"), None);
    g.set_edge_named("x", "target", Some("x"), None);
    g.set_edge_named("a", "target", Some("a-second"), None);

    assert!(g.remove_edge("source", "b", Some("b-first")));
    assert!(g.remove_edge("a", "target", Some("a-first")));
    assert_eq!(g.successors("source"), vec!["b", "c"]);
    assert_eq!(g.predecessors("target"), vec!["a", "x"]);

    assert!(g.remove_node("removed"));
    assert!(g.compact_if_sparse(1.01));
    assert_eq!(g.successors("source"), vec!["b", "c"]);
    assert_eq!(g.predecessors("target"), vec!["a", "x"]);

    assert!(g.remove_edge("source", "b", Some("b-second")));
    assert_eq!(g.successors("source"), vec!["c"]);
    g.set_edge_named("source", "b", Some("b-readded"), None);
    assert_eq!(g.successors("source"), vec!["c", "b"]);
}

#[test]
fn directed_edge_callbacks_can_query_node_adjacency_without_prewarming() {
    let mut g: Graph<(), (), ()> = Graph::new(GraphOptions {
        multigraph: true,
        ..Default::default()
    });
    g.set_edge_named("a", "b", Some("first"), None);
    g.set_edge_named("a", "b", Some("second"), None);
    g.set_edge_named("a", "c", Some("third"), None);

    let mut observations = Vec::new();
    g.for_each_out_edge("a", None, |_, _| {
        observations.push(g.successors("a"));
    });

    assert_eq!(observations, vec![vec!["b", "c"]; 3]);
}

#[test]
fn directed_node_adjacency_reuses_high_fanout_removals_without_historical_order() {
    let mut g: Graph<(), (), ()> = Graph::new(GraphOptions::default());
    for index in 0..256 {
        g.set_edge("source", format!("old-{index:03}"));
    }

    for index in 0..255 {
        assert!(g.remove_edge("source", &format!("old-{index:03}"), None));
    }
    assert_eq!(g.successors("source"), vec!["old-255"]);
    assert_eq!(g.first_successor("source"), Some("old-255"));

    for index in 0..255 {
        g.set_edge("source", format!("new-{index:03}"));
    }
    let successors = g.successors("source");
    assert_eq!(successors.len(), 256);
    assert_eq!(successors.first(), Some(&"old-255"));
    assert_eq!(successors.get(1), Some(&"new-000"));
    assert_eq!(successors.last(), Some(&"new-254"));
    assert_eq!(g.predecessors("new-000"), vec!["source"]);
    assert_eq!(g.predecessors("new-254"), vec!["source"]);
}

#[test]
fn neighbors_returns_unique_in_and_out_neighbors() {
    let mut g: Graph<(), (), ()> = Graph::new(GraphOptions::default());
    g.set_edge("a", "b");
    g.set_edge("b", "c");
    g.set_edge("a", "a");

    assert_eq!(sorted(g.neighbors("a")), vec!["a", "b"]);
    assert_eq!(sorted(g.neighbors("b")), vec!["a", "c"]);
    assert_eq!(sorted(g.neighbors("c")), vec!["b"]);
}

#[test]
fn is_leaf_follows_graphlib_directed_and_undirected_rules() {
    let mut directed: Graph<(), (), ()> = Graph::new(GraphOptions::default());
    directed.ensure_node("isolated");
    directed.set_edge("a", "b");

    assert!(directed.is_leaf("isolated"));
    assert!(!directed.is_leaf("a"));
    assert!(directed.is_leaf("b"));

    let mut undirected: Graph<(), (), ()> = Graph::new(GraphOptions {
        directed: false,
        ..Default::default()
    });
    undirected.ensure_node("isolated");
    undirected.set_edge("a", "b");

    assert!(undirected.is_leaf("isolated"));
    assert!(!undirected.is_leaf("b"));
}

#[test]
fn in_edges_returns_edges_pointing_at_node() {
    let mut g: Graph<(), (), ()> = Graph::new(GraphOptions::default());
    g.set_edge("a", "b");
    g.set_edge("b", "c");

    assert!(g.in_edges("a", None).is_empty());
    assert_eq!(
        sorted_edge_tuples(g.in_edges("b", None)),
        vec![("a".to_string(), "b".to_string(), None)]
    );
    assert_eq!(
        sorted_edge_tuples(g.in_edges("c", None)),
        vec![("b".to_string(), "c".to_string(), None)]
    );
}

#[test]
fn out_edges_returns_edges_pointing_from_node() {
    let mut g: Graph<(), (), ()> = Graph::new(GraphOptions::default());
    g.set_edge("a", "b");
    g.set_edge("b", "c");

    assert_eq!(
        sorted_edge_tuples(g.out_edges("a", None)),
        vec![("a".to_string(), "b".to_string(), None)]
    );
    assert_eq!(
        sorted_edge_tuples(g.out_edges("b", None)),
        vec![("b".to_string(), "c".to_string(), None)]
    );
    assert!(g.out_edges("c", None).is_empty());
}

#[test]
fn edge_queries_work_for_multigraphs_and_endpoint_filters() {
    let mut g: Graph<(), (), ()> = Graph::new(GraphOptions {
        multigraph: true,
        ..Default::default()
    });
    g.set_edge("a", "b");
    g.set_edge_named("a", "b", Some("bar"), None);
    g.set_edge_named("a", "b", Some("foo"), None);
    g.set_edge("a", "c");
    g.set_edge("b", "c");
    g.set_edge("z", "a");
    g.set_edge("z", "b");

    let ab = vec![
        ("a".to_string(), "b".to_string(), None),
        ("a".to_string(), "b".to_string(), Some("bar".to_string())),
        ("a".to_string(), "b".to_string(), Some("foo".to_string())),
    ];
    assert_eq!(sorted_edge_tuples(g.out_edges("a", Some("b"))), ab);
    assert!(g.out_edges("b", Some("a")).is_empty());

    let ab = vec![
        ("a".to_string(), "b".to_string(), None),
        ("a".to_string(), "b".to_string(), Some("bar".to_string())),
        ("a".to_string(), "b".to_string(), Some("foo".to_string())),
    ];
    assert_eq!(sorted_edge_tuples(g.in_edges("b", Some("a"))), ab);
    assert!(g.in_edges("a", Some("b")).is_empty());
}

#[test]
fn node_edges_returns_all_incident_edges() {
    let mut g: Graph<(), (), ()> = Graph::new(GraphOptions::default());
    g.set_edge("a", "b");
    g.set_edge("b", "c");

    assert_eq!(
        sorted_edge_tuples(g.node_edges("a")),
        vec![("a".to_string(), "b".to_string(), None)]
    );
    assert_eq!(
        sorted_edge_tuples(g.node_edges("b")),
        vec![
            ("a".to_string(), "b".to_string(), None),
            ("b".to_string(), "c".to_string(), None)
        ]
    );
    assert_eq!(
        sorted_edge_tuples(g.node_edges("c")),
        vec![("b".to_string(), "c".to_string(), None)]
    );
}

#[test]
fn node_edges_returns_parallel_multigraph_edges() {
    let mut g: Graph<(), (), ()> = Graph::new(GraphOptions {
        multigraph: true,
        ..Default::default()
    });
    g.set_edge("a", "b");
    g.set_edge_named("a", "b", Some("bar"), None);
    g.set_edge_named("a", "b", Some("foo"), None);

    let ab = vec![
        ("a".to_string(), "b".to_string(), None),
        ("a".to_string(), "b".to_string(), Some("bar".to_string())),
        ("a".to_string(), "b".to_string(), Some("foo".to_string())),
    ];
    assert_eq!(sorted_edge_tuples(g.node_edges("a")), ab);

    let ab = vec![
        ("a".to_string(), "b".to_string(), None),
        ("a".to_string(), "b".to_string(), Some("bar".to_string())),
        ("a".to_string(), "b".to_string(), Some("foo".to_string())),
    ];
    assert_eq!(sorted_edge_tuples(g.node_edges("b")), ab);
}

#[test]
fn node_edges_between_returns_edges_between_specific_nodes() {
    let mut g: Graph<(), (), ()> = Graph::new(GraphOptions {
        multigraph: true,
        ..Default::default()
    });
    g.set_edge("a", "b");
    g.set_edge_named("a", "b", Some("bar"), None);
    g.set_edge_named("a", "b", Some("foo"), None);
    g.set_edge("a", "c");
    g.set_edge("b", "c");
    g.set_edge("z", "a");
    g.set_edge("z", "b");

    let ab = vec![
        ("a".to_string(), "b".to_string(), None),
        ("a".to_string(), "b".to_string(), Some("bar".to_string())),
        ("a".to_string(), "b".to_string(), Some("foo".to_string())),
    ];
    assert_eq!(sorted_edge_tuples(g.node_edges_between("a", "b")), ab);

    let ab = vec![
        ("a".to_string(), "b".to_string(), None),
        ("a".to_string(), "b".to_string(), Some("bar".to_string())),
        ("a".to_string(), "b".to_string(), Some("foo".to_string())),
    ];
    assert_eq!(sorted_edge_tuples(g.node_edges_between("b", "a")), ab);
}

#[test]
fn remove_edge_missing_edge_is_noop() {
    let mut g: Graph<(), (), ()> = Graph::new(GraphOptions::default());

    assert!(!g.remove_edge("a", "b", None));

    assert!(!g.has_edge("a", "b", None));
    assert_eq!(g.edge_count(), 0);
}

#[test]
fn remove_edge_key_removes_named_multigraph_edge() {
    let mut g: Graph<(), (), ()> = Graph::new(GraphOptions {
        multigraph: true,
        ..Default::default()
    });
    let key = EdgeKey::new("a", "b", Some("foo"));
    g.set_edge_key(key.clone(), ());

    assert!(g.remove_edge_key(&key));

    assert!(!g.has_edge("a", "b", Some("foo")));
    assert_eq!(g.edge_count(), 0);
}

#[test]
fn remove_edge_with_named_ids_removes_named_multigraph_edge() {
    let mut g: Graph<(), (), ()> = Graph::new(GraphOptions {
        multigraph: true,
        ..Default::default()
    });
    g.set_edge_named("a", "b", Some("foo"), None);

    assert!(g.remove_edge("a", "b", Some("foo")));

    assert!(!g.has_edge("a", "b", Some("foo")));
    assert_eq!(g.edge_count(), 0);
}

#[test]
fn remove_edge_accepts_reversed_endpoints_for_undirected_graphs() {
    let mut g: Graph<(), (), ()> = Graph::new(GraphOptions {
        directed: false,
        ..Default::default()
    });
    g.set_edge("h", "g");

    assert!(g.remove_edge("g", "h", None));

    assert!(g.neighbors("g").is_empty());
    assert!(g.neighbors("h").is_empty());
}

#[test]
fn remove_edge_updates_neighbor_queries() {
    let mut g: Graph<(), (), ()> = Graph::new(GraphOptions::default());
    g.set_edge("a", "b");

    assert!(g.remove_edge("a", "b", None));

    assert!(g.successors("a").is_empty());
    assert!(g.neighbors("a").is_empty());
    assert!(g.predecessors("b").is_empty());
    assert!(g.neighbors("b").is_empty());
}

#[test]
fn remove_edge_keeps_named_parallel_edges() {
    let mut g: Graph<(), (), ()> = Graph::new(GraphOptions {
        multigraph: true,
        ..Default::default()
    });
    g.set_edge("a", "b");
    g.set_edge_named("a", "b", Some("foo"), None);

    assert!(g.remove_edge("a", "b", None));

    assert!(g.has_edge("a", "b", Some("foo")));
    assert_eq!(g.successors("a"), vec!["b"]);
    assert_eq!(g.neighbors("a"), vec!["b"]);
    assert_eq!(g.predecessors("b"), vec!["a"]);
    assert_eq!(g.neighbors("b"), vec!["a"]);
}

#[test]
fn set_parent_creates_parent_and_child_nodes() {
    let mut g: Graph<(), (), ()> = Graph::new(GraphOptions {
        compound: true,
        ..Default::default()
    });

    g.set_parent("a", "parent");

    assert!(g.has_node("a"));
    assert!(g.has_node("parent"));
    assert_eq!(g.parent("a"), Some("parent"));
    assert_eq!(g.children("parent"), vec!["a"]);
}

#[test]
fn children_opt_distinguishes_missing_nodes_from_empty_children() {
    let mut compound: Graph<(), (), ()> = Graph::new(GraphOptions {
        compound: true,
        ..Default::default()
    });

    assert_eq!(compound.children_opt("missing"), None);

    compound.ensure_node("a");
    assert_eq!(compound.children_opt("a"), Some(Vec::<&str>::new()));

    let mut simple: Graph<(), (), ()> = Graph::new(GraphOptions::default());

    assert_eq!(simple.children_opt("missing"), None);

    simple.ensure_node("a");
    assert_eq!(simple.children_opt("a"), Some(Vec::<&str>::new()));
}

#[test]
fn child_count_matches_direct_children_without_allocating_a_snapshot() {
    let mut compound: Graph<(), (), ()> = Graph::new(GraphOptions {
        compound: true,
        ..Default::default()
    });
    compound.set_parent("a", "parent");
    compound.set_parent("b", "parent");
    compound.set_parent("nested", "a");

    assert_eq!(compound.child_count("parent"), 2);
    assert_eq!(compound.child_count("a"), 1);
    assert_eq!(compound.child_count("b"), 0);
    assert_eq!(compound.child_count("missing"), 0);
    assert_eq!(
        compound.child_count("parent"),
        compound.children("parent").len()
    );

    let mut simple: Graph<(), (), ()> = Graph::new(GraphOptions::default());
    simple.ensure_node("parent");
    assert_eq!(simple.child_count("parent"), 0);
    assert_eq!(simple.child_count("missing"), 0);
}

#[test]
fn children_root_matches_graphlib_no_arg_children_semantics() {
    let mut simple: Graph<(), (), ()> = Graph::new(GraphOptions::default());
    simple.ensure_node("a");
    simple.ensure_node("b");

    assert_eq!(sorted(simple.children_root()), vec!["a", "b"]);

    let mut compound: Graph<(), (), ()> = Graph::new(GraphOptions {
        compound: true,
        ..Default::default()
    });
    compound.ensure_node("b");
    compound.ensure_node("c");
    compound.set_parent("a", "parent");

    assert_eq!(sorted(compound.children_opt("parent").unwrap()), vec!["a"]);
    assert_eq!(sorted(compound.children_root()), vec!["b", "c", "parent"]);
}

#[test]
fn set_parent_moves_node_from_previous_parent() {
    let mut g: Graph<(), (), ()> = Graph::new(GraphOptions {
        compound: true,
        ..Default::default()
    });

    g.set_parent("a", "parent");
    g.set_parent("a", "parent2");

    assert_eq!(g.parent("a"), Some("parent2"));
    assert!(g.children("parent").is_empty());
    assert_eq!(g.children("parent2"), vec!["a"]);
}

#[test]
fn parent_matches_graphlib_optional_query_shape() {
    let mut simple: Graph<(), (), ()> = Graph::new(GraphOptions::default());
    simple.ensure_node("a");
    assert_eq!(simple.parent("a"), None);
    assert_eq!(simple.parent("missing"), None);

    let mut compound: Graph<(), (), ()> = Graph::new(GraphOptions {
        compound: true,
        ..Default::default()
    });
    assert_eq!(compound.parent("missing"), None);

    compound.ensure_node("a");
    assert_eq!(compound.parent("a"), None);

    compound.set_parent("a", "parent");
    assert_eq!(compound.parent("a"), Some("parent"));
}

#[test]
fn clear_parent_returns_node_to_root_children() {
    let mut g: Graph<(), (), ()> = Graph::new(GraphOptions {
        compound: true,
        ..Default::default()
    });
    g.set_parent("a", "parent");

    g.clear_parent("a");
    g.clear_parent("a");

    assert_eq!(g.parent("a"), None);
    assert_eq!(sorted(g.children_root()), vec!["a", "parent"]);
}

#[test]
#[should_panic(expected = "set_parent would create a cycle")]
fn set_parent_preserves_tree_invariant() {
    let mut g: Graph<(), (), ()> = Graph::new(GraphOptions {
        compound: true,
        ..Default::default()
    });
    g.set_parent("c", "b");
    g.set_parent("b", "a");

    g.set_parent("a", "c");
}

#[test]
fn remove_node_clears_parent_child_relationships() {
    let mut g: Graph<(), (), ()> = Graph::new(GraphOptions {
        compound: true,
        ..Default::default()
    });
    g.set_parent("c", "b");
    g.set_parent("b", "a");

    assert!(g.remove_node("b"));

    assert_eq!(g.parent("b"), None);
    assert_eq!(g.children_opt("b"), None);
    assert!(g.children("b").is_empty());
    assert!(!g.children("a").contains(&"b"));
    assert_eq!(g.parent("c"), None);
}

#[test]
fn edge_key_lookup_uses_named_edges() {
    let mut g: Graph<(), i32, ()> = Graph::new(GraphOptions {
        multigraph: true,
        ..Default::default()
    });
    g.set_edge_named("a", "b", Some("name"), Some(5));
    let key = EdgeKey::new("a", "b", Some("name"));

    assert_eq!(g.edge_by_key(&key), Some(&5));
    assert!(g.remove_edge_key(&key));
    assert_eq!(g.edge_count(), 0);
}

#[test]
fn node_ids_and_edge_keys_keep_insertion_order_after_removal() {
    let mut g: Graph<(), (), ()> = Graph::new(GraphOptions::default());
    g.set_edge("a", "b");
    g.set_edge("b", "c");
    g.remove_node("b");
    g.ensure_node("d");

    assert_eq!(sorted_owned(g.node_ids()), vec!["a", "c", "d"]);
    assert!(g.edge_keys().is_empty());
}
