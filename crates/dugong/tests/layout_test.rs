use dugong::graphlib::{Graph, GraphOptions};
use dugong::{EdgeLabel, GraphLabel, LabelPos, NodeLabel, Point, RankDir, layout};

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

#[test]
fn layout_can_layout_an_edge_with_a_label() {
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
    g.set_edge_with_label(
        "a",
        "b",
        EdgeLabel {
            width: 60.0,
            height: 70.0,
            labelpos: LabelPos::C,
            ..Default::default()
        },
    );

    layout(&mut g);
    assert_eq!(
        coords(&g),
        [
            ("a".to_string(), (75.0 / 2.0, 100.0 / 2.0)),
            (
                "b".to_string(),
                (75.0 / 2.0, 100.0 + 150.0 + 70.0 + 150.0 + 200.0 / 2.0),
            ),
        ]
        .into()
    );

    let e = g.edge("a", "b", None).unwrap();
    assert_eq!(e.x, Some(75.0 / 2.0));
    assert_eq!(e.y, Some(100.0 + 150.0 + 70.0 / 2.0));
}

#[test]
fn layout_adds_rectangle_intersects_for_edges() {
    let mut g: Graph<NodeLabel, EdgeLabel, GraphLabel> = Graph::new(GraphOptions {
        multigraph: true,
        compound: true,
    });
    g.set_graph(GraphLabel::default());
    g.set_default_edge_label(EdgeLabel::default);

    g.graph_mut().ranksep = 200.0;
    g.set_node(
        "a",
        NodeLabel {
            width: 100.0,
            height: 100.0,
            ..Default::default()
        },
    );
    g.set_node(
        "b",
        NodeLabel {
            width: 100.0,
            height: 100.0,
            ..Default::default()
        },
    );
    g.set_edge("a", "b");

    layout(&mut g);
    let points = &g.edge("a", "b", None).unwrap().points;
    assert_eq!(
        points.as_slice(),
        [
            Point { x: 50.0, y: 100.0 },
            Point {
                x: 50.0,
                y: 100.0 + 200.0 / 2.0,
            },
            Point {
                x: 50.0,
                y: 100.0 + 200.0,
            },
        ]
    );
}

#[test]
fn layout_adds_rectangle_intersects_for_edges_spanning_multiple_ranks() {
    let mut g: Graph<NodeLabel, EdgeLabel, GraphLabel> = Graph::new(GraphOptions {
        multigraph: true,
        compound: true,
    });
    g.set_graph(GraphLabel::default());
    g.set_default_edge_label(EdgeLabel::default);

    g.graph_mut().ranksep = 200.0;
    g.set_node(
        "a",
        NodeLabel {
            width: 100.0,
            height: 100.0,
            ..Default::default()
        },
    );
    g.set_node(
        "b",
        NodeLabel {
            width: 100.0,
            height: 100.0,
            ..Default::default()
        },
    );
    g.set_edge_with_label(
        "a",
        "b",
        EdgeLabel {
            minlen: 2,
            ..Default::default()
        },
    );

    layout(&mut g);
    let points = &g.edge("a", "b", None).unwrap().points;
    assert_eq!(
        points.as_slice(),
        [
            Point { x: 50.0, y: 100.0 },
            Point {
                x: 50.0,
                y: 100.0 + 200.0 / 2.0,
            },
            Point {
                x: 50.0,
                y: 100.0 + 400.0 / 2.0,
            },
            Point {
                x: 50.0,
                y: 100.0 + 600.0 / 2.0,
            },
            Point {
                x: 50.0,
                y: 100.0 + 800.0 / 2.0,
            },
        ]
    );
}

#[test]
fn layout_can_apply_an_offset() {
    for rankdir in [RankDir::TB, RankDir::BT, RankDir::LR, RankDir::RL] {
        let mut g: Graph<NodeLabel, EdgeLabel, GraphLabel> = Graph::new(GraphOptions {
            multigraph: true,
            compound: true,
        });
        g.set_graph(GraphLabel {
            rankdir,
            nodesep: 10.0,
            ranksep: 10.0,
            edgesep: 10.0,
        });
        g.set_default_edge_label(EdgeLabel::default);

        for id in ["a", "b", "c", "d"] {
            g.set_node(
                id,
                NodeLabel {
                    width: 10.0,
                    height: 10.0,
                    ..Default::default()
                },
            );
        }
        g.set_edge_with_label(
            "a",
            "b",
            EdgeLabel {
                width: 10.0,
                height: 10.0,
                labelpos: LabelPos::L,
                labeloffset: 1000.0,
                ..Default::default()
            },
        );
        g.set_edge_with_label(
            "c",
            "d",
            EdgeLabel {
                width: 10.0,
                height: 10.0,
                labelpos: LabelPos::R,
                labeloffset: 1000.0,
                ..Default::default()
            },
        );

        layout(&mut g);

        let e1 = g.edge("a", "b", None).unwrap();
        let e2 = g.edge("c", "d", None).unwrap();
        if rankdir == RankDir::TB || rankdir == RankDir::BT {
            assert_eq!(e1.x.unwrap() - e1.points[0].x, -1000.0 - 10.0 / 2.0);
            assert_eq!(e2.x.unwrap() - e2.points[0].x, 1000.0 + 10.0 / 2.0);
        } else {
            assert_eq!(e1.y.unwrap() - e1.points[0].y, -1000.0 - 10.0 / 2.0);
            assert_eq!(e2.y.unwrap() - e2.points[0].y, 1000.0 + 10.0 / 2.0);
        }
    }
}
