use dugong::graphlib::{EdgeKey, Graph, GraphOptions};
use dugong::parent_dummy_chains;
use dugong::{DummyEdge, EdgeLabel, GraphLabel, NodeLabel, OriginalEdge};

fn graph() -> Graph<NodeLabel, EdgeLabel, GraphLabel> {
    let mut g: Graph<NodeLabel, EdgeLabel, GraphLabel> = Graph::new(GraphOptions {
        multigraph: false,
        compound: true,
        ..Default::default()
    });
    g.set_graph(GraphLabel::default());
    g
}

fn chain_head_edge() -> DummyEdge {
    DummyEdge::Original(Box::new(OriginalEdge {
        key: EdgeKey::new("a", "b", None::<String>),
        label: EdgeLabel::default(),
    }))
}

fn compound_label(min_rank: i32, max_rank: i32) -> NodeLabel {
    NodeLabel {
        compound: Some(Box::new(dugong::CompoundNodeLabel {
            min_rank: Some(min_rank),
            max_rank: Some(max_rank),
            ..Default::default()
        })),
        ..Default::default()
    }
}

#[test]
fn parent_dummy_chains_does_not_set_a_parent_if_both_tail_and_head_have_no_parent() {
    let mut g = graph();
    g.set_node("a", NodeLabel::default());
    g.set_node("b", NodeLabel::default());
    g.set_node(
        "d1",
        NodeLabel {
            dummy_edge: chain_head_edge(),
            ..Default::default()
        },
    );
    g.graph_mut().dummy_chains = vec!["d1".to_string()];
    g.set_path(&["a", "d1", "b"]);

    parent_dummy_chains::parent_dummy_chains(&mut g);
    assert_eq!(g.parent("d1"), None);
}

#[test]
fn parent_dummy_chains_uses_the_tails_parent_for_the_first_node_if_it_is_not_the_root() {
    let mut g = graph();
    g.set_parent("a", "sg1");
    g.set_node("sg1", compound_label(0, 2));
    g.set_node(
        "d1",
        NodeLabel {
            dummy_edge: chain_head_edge(),
            rank: Some(2),
            ..Default::default()
        },
    );
    g.graph_mut().dummy_chains = vec!["d1".to_string()];
    g.set_path(&["a", "d1", "b"]);

    parent_dummy_chains::parent_dummy_chains(&mut g);
    assert_eq!(g.parent("d1"), Some("sg1"));
}

#[test]
fn parent_dummy_chains_uses_the_heads_parent_for_the_first_node_if_tails_is_root() {
    let mut g = graph();
    g.set_parent("b", "sg1");
    g.set_node("sg1", compound_label(1, 3));
    g.set_node(
        "d1",
        NodeLabel {
            dummy_edge: chain_head_edge(),
            rank: Some(1),
            ..Default::default()
        },
    );
    g.graph_mut().dummy_chains = vec!["d1".to_string()];
    g.set_path(&["a", "d1", "b"]);

    parent_dummy_chains::parent_dummy_chains(&mut g);
    assert_eq!(g.parent("d1"), Some("sg1"));
}

#[test]
fn parent_dummy_chains_handles_a_long_chain_starting_in_a_subgraph() {
    let mut g = graph();
    g.set_parent("a", "sg1");
    g.set_node("sg1", compound_label(0, 2));
    g.set_node(
        "d1",
        NodeLabel {
            dummy_edge: chain_head_edge(),
            rank: Some(2),
            ..Default::default()
        },
    );
    g.set_node(
        "d2",
        NodeLabel {
            rank: Some(3),
            ..Default::default()
        },
    );
    g.set_node(
        "d3",
        NodeLabel {
            rank: Some(4),
            ..Default::default()
        },
    );
    g.graph_mut().dummy_chains = vec!["d1".to_string()];
    g.set_path(&["a", "d1", "d2", "d3", "b"]);

    parent_dummy_chains::parent_dummy_chains(&mut g);
    assert_eq!(g.parent("d1"), Some("sg1"));
    assert_eq!(g.parent("d2"), None);
    assert_eq!(g.parent("d3"), None);
}

#[test]
fn parent_dummy_chains_handles_a_long_chain_ending_in_a_subgraph() {
    let mut g = graph();
    g.set_parent("b", "sg1");
    g.set_node("sg1", compound_label(3, 5));
    g.set_node(
        "d1",
        NodeLabel {
            dummy_edge: chain_head_edge(),
            rank: Some(1),
            ..Default::default()
        },
    );
    g.set_node(
        "d2",
        NodeLabel {
            rank: Some(2),
            ..Default::default()
        },
    );
    g.set_node(
        "d3",
        NodeLabel {
            rank: Some(3),
            ..Default::default()
        },
    );
    g.graph_mut().dummy_chains = vec!["d1".to_string()];
    g.set_path(&["a", "d1", "d2", "d3", "b"]);

    parent_dummy_chains::parent_dummy_chains(&mut g);
    assert_eq!(g.parent("d1"), None);
    assert_eq!(g.parent("d2"), None);
    assert_eq!(g.parent("d3"), Some("sg1"));
}

#[test]
fn parent_dummy_chains_handles_nested_subgraphs() {
    let mut g = graph();
    g.set_parent("a", "sg2");
    g.set_parent("sg2", "sg1");
    g.set_node("sg1", compound_label(0, 4));
    g.set_node("sg2", compound_label(1, 3));
    g.set_parent("b", "sg4");
    g.set_parent("sg4", "sg3");
    g.set_node("sg3", compound_label(6, 10));
    g.set_node("sg4", compound_label(7, 9));
    for i in 0..5 {
        g.set_node(
            format!("d{}", i + 1),
            NodeLabel {
                rank: Some(i + 3),
                ..Default::default()
            },
        );
    }
    if let Some(n) = g.node_mut("d1") {
        n.dummy_edge = chain_head_edge();
    }
    g.graph_mut().dummy_chains = vec!["d1".to_string()];
    g.set_path(&["a", "d1", "d2", "d3", "d4", "d5", "b"]);

    parent_dummy_chains::parent_dummy_chains(&mut g);
    assert_eq!(g.parent("d1"), Some("sg2"));
    assert_eq!(g.parent("d2"), Some("sg1"));
    assert_eq!(g.parent("d3"), None);
    assert_eq!(g.parent("d4"), Some("sg3"));
    assert_eq!(g.parent("d5"), Some("sg4"));
}

#[test]
fn parent_dummy_chains_handles_overlapping_rank_ranges() {
    let mut g = graph();
    g.set_parent("a", "sg1");
    g.set_node("sg1", compound_label(0, 3));
    g.set_parent("b", "sg2");
    g.set_node("sg2", compound_label(2, 6));
    g.set_node(
        "d1",
        NodeLabel {
            dummy_edge: chain_head_edge(),
            rank: Some(2),
            ..Default::default()
        },
    );
    g.set_node(
        "d2",
        NodeLabel {
            rank: Some(3),
            ..Default::default()
        },
    );
    g.set_node(
        "d3",
        NodeLabel {
            rank: Some(4),
            ..Default::default()
        },
    );
    g.graph_mut().dummy_chains = vec!["d1".to_string()];
    g.set_path(&["a", "d1", "d2", "d3", "b"]);

    parent_dummy_chains::parent_dummy_chains(&mut g);
    assert_eq!(g.parent("d1"), Some("sg1"));
    assert_eq!(g.parent("d2"), Some("sg1"));
    assert_eq!(g.parent("d3"), Some("sg2"));
}

#[test]
fn parent_dummy_chains_handles_an_lca_that_is_not_the_root_1() {
    let mut g = graph();
    g.set_parent("a", "sg1");
    g.set_parent("sg2", "sg1");
    g.set_node("sg1", compound_label(0, 6));
    g.set_parent("b", "sg2");
    g.set_node("sg2", compound_label(3, 5));
    g.set_node(
        "d1",
        NodeLabel {
            dummy_edge: chain_head_edge(),
            rank: Some(2),
            ..Default::default()
        },
    );
    g.set_node(
        "d2",
        NodeLabel {
            rank: Some(3),
            ..Default::default()
        },
    );
    g.graph_mut().dummy_chains = vec!["d1".to_string()];
    g.set_path(&["a", "d1", "d2", "b"]);

    parent_dummy_chains::parent_dummy_chains(&mut g);
    assert_eq!(g.parent("d1"), Some("sg1"));
    assert_eq!(g.parent("d2"), Some("sg2"));
}

#[test]
fn parent_dummy_chains_handles_an_lca_that_is_not_the_root_2() {
    let mut g = graph();
    g.set_parent("a", "sg2");
    g.set_parent("sg2", "sg1");
    g.set_node("sg1", compound_label(0, 6));
    g.set_parent("b", "sg1");
    g.set_node("sg2", compound_label(1, 3));
    g.set_node(
        "d1",
        NodeLabel {
            dummy_edge: chain_head_edge(),
            rank: Some(3),
            ..Default::default()
        },
    );
    g.set_node(
        "d2",
        NodeLabel {
            rank: Some(4),
            ..Default::default()
        },
    );
    g.graph_mut().dummy_chains = vec!["d1".to_string()];
    g.set_path(&["a", "d1", "d2", "b"]);

    parent_dummy_chains::parent_dummy_chains(&mut g);
    assert_eq!(g.parent("d1"), Some("sg2"));
    assert_eq!(g.parent("d2"), Some("sg1"));
}
