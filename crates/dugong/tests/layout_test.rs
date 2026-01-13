use dugong::graphlib::{Graph, GraphOptions};
use dugong::{EdgeLabel, GraphLabel, NodeLabel, layout};

fn coords(
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
) -> std::collections::BTreeMap<String, (f64, f64)> {
    let mut out = std::collections::BTreeMap::new();
    for id in g.nodes() {
        let n = g.node(id).unwrap();
        out.insert(id.to_string(), (n.x.unwrap(), n.y.unwrap()));
    }
    out
}

#[test]
fn layout_can_layout_a_single_node() {
    let mut g: Graph<NodeLabel, EdgeLabel, GraphLabel> = Graph::new(GraphOptions {
        multigraph: true,
        compound: true,
    });
    g.set_graph(GraphLabel::default());
    g.set_default_edge_label(EdgeLabel::default);

    g.set_node(
        "a",
        NodeLabel {
            width: 50.0,
            height: 100.0,
            ..Default::default()
        },
    );

    layout(&mut g);
    assert_eq!(coords(&g), [("a".to_string(), (25.0, 50.0))].into());
    assert_eq!(g.node("a").unwrap().x, Some(25.0));
    assert_eq!(g.node("a").unwrap().y, Some(50.0));
}

#[test]
fn layout_can_layout_two_nodes_on_the_same_rank() {
    let mut g: Graph<NodeLabel, EdgeLabel, GraphLabel> = Graph::new(GraphOptions {
        multigraph: true,
        compound: true,
    });
    g.set_graph(GraphLabel::default());
    g.set_default_edge_label(EdgeLabel::default);

    g.graph_mut().nodesep = 200.0;
    g.set_node(
        "a",
        NodeLabel {
            width: 50.0,
            height: 100.0,
            ..Default::default()
        },
    );
    g.set_node(
        "b",
        NodeLabel {
            width: 75.0,
            height: 200.0,
            ..Default::default()
        },
    );

    layout(&mut g);
    assert_eq!(
        coords(&g),
        [
            ("a".to_string(), (25.0, 100.0)),
            ("b".to_string(), (50.0 + 200.0 + 75.0 / 2.0, 100.0)),
        ]
        .into()
    );
}

#[test]
fn layout_can_layout_two_nodes_connected_by_an_edge() {
    let mut g: Graph<NodeLabel, EdgeLabel, GraphLabel> = Graph::new(GraphOptions {
        multigraph: true,
        compound: true,
    });
    g.set_graph(GraphLabel::default());
    g.set_default_edge_label(EdgeLabel::default);

    g.graph_mut().ranksep = 300.0;
    g.set_node(
        "a",
        NodeLabel {
            width: 50.0,
            height: 100.0,
            ..Default::default()
        },
    );
    g.set_node(
        "b",
        NodeLabel {
            width: 75.0,
            height: 200.0,
            ..Default::default()
        },
    );
    g.set_edge("a", "b");

    layout(&mut g);
    assert_eq!(
        coords(&g),
        [
            ("a".to_string(), (75.0 / 2.0, 100.0 / 2.0)),
            ("b".to_string(), (75.0 / 2.0, 100.0 + 300.0 + 200.0 / 2.0)),
        ]
        .into()
    );
}
