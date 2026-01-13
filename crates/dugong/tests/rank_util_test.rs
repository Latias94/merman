use dugong::graphlib::{Graph, GraphOptions};
use dugong::{EdgeLabel, GraphLabel, NodeLabel, rank, util};

fn new_graph() -> Graph<NodeLabel, EdgeLabel, GraphLabel> {
    let mut g: Graph<NodeLabel, EdgeLabel, GraphLabel> = Graph::new(GraphOptions::default());
    g.set_graph(GraphLabel::default());
    g.set_default_node_label(NodeLabel::default);
    g.set_default_edge_label(|| EdgeLabel {
        minlen: 1,
        ..Default::default()
    });
    g
}

#[test]
fn longest_path_can_assign_a_rank_to_a_single_node_graph() {
    let mut g = new_graph();
    g.set_node("a", NodeLabel::default());

    rank::util::longest_path(&mut g);
    util::normalize_ranks(&mut g);

    assert_eq!(g.node("a").unwrap().rank, Some(0));
}

#[test]
fn longest_path_can_assign_ranks_to_unconnected_nodes() {
    let mut g = new_graph();
    g.set_node("a", NodeLabel::default());
    g.set_node("b", NodeLabel::default());

    rank::util::longest_path(&mut g);
    util::normalize_ranks(&mut g);

    assert_eq!(g.node("a").unwrap().rank, Some(0));
    assert_eq!(g.node("b").unwrap().rank, Some(0));
}

#[test]
fn longest_path_can_assign_ranks_to_connected_nodes() {
    let mut g = new_graph();
    g.set_edge("a", "b");

    rank::util::longest_path(&mut g);
    util::normalize_ranks(&mut g);

    assert_eq!(g.node("a").unwrap().rank, Some(0));
    assert_eq!(g.node("b").unwrap().rank, Some(1));
}

#[test]
fn longest_path_can_assign_ranks_for_a_diamond() {
    let mut g = new_graph();
    g.set_path(&["a", "b", "d"]);
    g.set_path(&["a", "c", "d"]);

    rank::util::longest_path(&mut g);
    util::normalize_ranks(&mut g);

    assert_eq!(g.node("a").unwrap().rank, Some(0));
    assert_eq!(g.node("b").unwrap().rank, Some(1));
    assert_eq!(g.node("c").unwrap().rank, Some(1));
    assert_eq!(g.node("d").unwrap().rank, Some(2));
}

#[test]
fn longest_path_uses_the_minlen_attribute_on_the_edge() {
    let mut g = new_graph();
    g.set_path(&["a", "b", "d"]);
    g.set_edge("a", "c");
    g.set_edge_with_label(
        "c",
        "d",
        EdgeLabel {
            minlen: 2,
            ..Default::default()
        },
    );

    rank::util::longest_path(&mut g);
    util::normalize_ranks(&mut g);

    assert_eq!(g.node("a").unwrap().rank, Some(0));
    assert_eq!(g.node("b").unwrap().rank, Some(2));
    assert_eq!(g.node("c").unwrap().rank, Some(1));
    assert_eq!(g.node("d").unwrap().rank, Some(3));
}
