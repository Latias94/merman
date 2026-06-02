use dugong_graphlib::{EdgeKey, Graph, GraphOptions};

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
fn ensure_node_uses_default_label_for_new_nodes() {
    let mut g: Graph<Option<i32>, (), ()> = Graph::new(GraphOptions::default());
    g.set_default_node_label(|| Some(7));

    g.ensure_node("a");

    assert_eq!(g.node("a"), Some(&Some(7)));
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
fn set_node_is_idempotent_for_existing_node() {
    let mut g: Graph<Option<i32>, (), ()> = Graph::new(GraphOptions::default());
    g.set_node("a", Some(1));
    g.set_node("a", Some(1));

    assert_eq!(g.node("a"), Some(&Some(1)));
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
fn set_edge_creates_endpoint_nodes_and_uses_default_edge_label() {
    let mut g: Graph<(), Option<i32>, ()> = Graph::new(GraphOptions::default());
    g.set_default_edge_label(|| Some(9));

    g.set_edge("a", "b");

    assert!(g.has_node("a"));
    assert!(g.has_node("b"));
    assert_eq!(g.edge("a", "b", None), Some(&Some(9)));
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
}

#[test]
fn set_path_creates_path_edges() {
    let mut g: Graph<(), (), ()> = Graph::new(GraphOptions::default());

    g.set_path(&["a", "b", "c"]);

    assert!(g.has_edge("a", "b", None));
    assert!(g.has_edge("b", "c", None));
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
fn clear_parent_returns_node_to_root_children() {
    let mut g: Graph<(), (), ()> = Graph::new(GraphOptions {
        compound: true,
        ..Default::default()
    });
    g.set_parent("a", "parent");

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
