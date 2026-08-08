use dugong::graphlib::{Graph, GraphOptions};
use dugong::rank;
use dugong::{EdgeLabel, GraphLabel, NodeLabel};

fn gansner_graph() -> Graph<NodeLabel, EdgeLabel, GraphLabel> {
    let mut g: Graph<NodeLabel, EdgeLabel, GraphLabel> = Graph::new(GraphOptions::default());
    g.set_graph(GraphLabel::default());
    g.set_default_node_label(NodeLabel::default);
    g.set_default_edge_label(|| EdgeLabel {
        minlen: 1,
        weight: 1.0,
        ..Default::default()
    });

    g.set_path(&["a", "b", "c", "d", "h"]);
    g.set_path(&["a", "e", "g", "h"]);
    g.set_path(&["a", "f", "g"]);
    g
}

fn assert_respects_minlen(g: &Graph<NodeLabel, EdgeLabel, GraphLabel>) {
    for e in g.edges() {
        let v_rank = g.node(&e.v).unwrap().rank.unwrap();
        let w_rank = g.node(&e.w).unwrap().rank.unwrap();
        let minlen = g.edge_by_key(e).unwrap().minlen as i32;
        assert!(
            w_rank - v_rank >= minlen,
            "edge {} -> {} violates minlen {}: {} - {}",
            e.v,
            e.w,
            minlen,
            w_rank,
            v_rank
        );
    }
}

#[test]
fn rank_longest_path_respects_the_minlen_attribute() {
    let mut g = gansner_graph();
    g.graph_mut().ranker = Some("longest-path".to_string());
    rank::rank(&mut g).expect("longest-path ranking succeeds");
    assert_respects_minlen(&g);
}

#[test]
fn rank_tight_tree_respects_the_minlen_attribute() {
    let mut g = gansner_graph();
    g.graph_mut().ranker = Some("tight-tree".to_string());
    rank::rank(&mut g).expect("tight-tree ranking succeeds");
    assert_respects_minlen(&g);
}

#[test]
fn rank_network_simplex_respects_the_minlen_attribute() {
    let mut g = gansner_graph();
    g.graph_mut().ranker = Some("network-simplex".to_string());
    rank::rank(&mut g).expect("network-simplex ranking succeeds");
    assert_respects_minlen(&g);
}

#[test]
fn rank_unknown_should_still_work_respects_the_minlen_attribute() {
    for ranker in ["unknown-should-still-work", "none"] {
        let mut g = gansner_graph();
        g.graph_mut().ranker = Some(ranker.to_string());
        rank::rank(&mut g).expect("unknown rankers fall back to network simplex");
        assert_respects_minlen(&g);
    }
}

#[test]
fn rankers_match_dagres_zero_minlen_boundary() {
    fn zero_minlen_graph(ranker: &str) -> Graph<NodeLabel, EdgeLabel, GraphLabel> {
        let mut graph = Graph::new(GraphOptions::default());
        graph.set_graph(GraphLabel {
            ranker: Some(ranker.to_string()),
            ..Default::default()
        });
        graph.set_node("a", NodeLabel::default());
        graph.set_node("b", NodeLabel::default());
        graph.set_edge_with_label(
            "a",
            "b",
            EdgeLabel {
                minlen: 0,
                weight: 1.0,
                ..Default::default()
            },
        );
        graph
    }

    for ranker in ["longest-path", "tight-tree"] {
        let mut graph = zero_minlen_graph(ranker);
        rank::rank(&mut graph).expect("zero-minlen ranking succeeds");
        assert_eq!(graph.node("a").unwrap().rank, graph.node("b").unwrap().rank);
    }

    let mut network_simplex = zero_minlen_graph("network-simplex");
    rank::rank(&mut network_simplex).expect("zero-minlen network simplex succeeds");
    assert_eq!(
        network_simplex.node("b").unwrap().rank.unwrap()
            - network_simplex.node("a").unwrap().rank.unwrap(),
        1
    );
}

#[test]
fn rank_can_rank_a_single_node_graph_for_each_ranker() {
    for ranker in [
        "longest-path",
        "tight-tree",
        "network-simplex",
        "unknown-should-still-work",
    ] {
        let mut g: Graph<NodeLabel, EdgeLabel, GraphLabel> = Graph::new(GraphOptions::default());
        g.set_graph(GraphLabel {
            ranker: Some(ranker.to_string()),
            ..Default::default()
        });
        g.set_node("a", NodeLabel::default());
        rank::rank(&mut g).expect("single-node ranking succeeds");
        assert_eq!(g.node("a").unwrap().rank, Some(0));
    }
}
