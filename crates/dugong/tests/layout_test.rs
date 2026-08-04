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
        ..Default::default()
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
        ..Default::default()
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
        ..Default::default()
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
        ..Default::default()
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
fn layout_can_layout_a_long_edge_with_a_label() {
    let mut g: Graph<NodeLabel, EdgeLabel, GraphLabel> = Graph::new(GraphOptions {
        multigraph: true,
        compound: true,
        ..Default::default()
    });
    g.set_graph(GraphLabel {
        ranksep: 300.0,
        ..Default::default()
    });
    g.set_default_edge_label(EdgeLabel::default);

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
            minlen: 2,
            labelpos: LabelPos::C,
            ..Default::default()
        },
    );

    layout(&mut g);

    let edge = g.edge("a", "b", None).unwrap();
    assert_eq!(edge.x, Some(75.0 / 2.0));
    assert!(edge.y.unwrap() > g.node("a").unwrap().y.unwrap());
    assert!(edge.y.unwrap() < g.node("b").unwrap().y.unwrap());
}

#[test]
fn layout_second_run_matches_a_fresh_graph() {
    fn build_graph() -> Graph<NodeLabel, EdgeLabel, GraphLabel> {
        let mut graph = Graph::new(GraphOptions {
            multigraph: true,
            compound: true,
            ..Default::default()
        });
        graph.set_graph(GraphLabel {
            ranksep: 300.0,
            ..Default::default()
        });
        graph.set_default_edge_label(EdgeLabel::default);
        graph.set_node(
            "a",
            NodeLabel {
                width: 50.0,
                height: 100.0,
                ..Default::default()
            },
        );
        graph.set_node(
            "b",
            NodeLabel {
                width: 75.0,
                height: 200.0,
                ..Default::default()
            },
        );
        graph.set_edge_with_label(
            "a",
            "b",
            EdgeLabel {
                width: 60.0,
                height: 70.0,
                labelpos: LabelPos::R,
                labeloffset: 12.0,
                ..Default::default()
            },
        );
        graph
    }

    let mut graph = build_graph();
    let mut fresh = build_graph();

    layout(&mut graph);
    layout(&mut graph);
    layout(&mut fresh);

    assert_eq!(graph.graph().rankdir, fresh.graph().rankdir);
    assert_eq!(graph.graph().nodesep, fresh.graph().nodesep);
    assert_eq!(graph.graph().ranksep, fresh.graph().ranksep);
    assert_eq!(graph.graph().width, fresh.graph().width);
    assert_eq!(graph.graph().height, fresh.graph().height);
    assert_eq!(graph.node("a"), fresh.node("a"));
    assert_eq!(graph.node("b"), fresh.node("b"));
    assert_eq!(graph.edge("a", "b", None), fresh.edge("a", "b", None));
}

#[test]
fn layout_can_layout_a_short_cycle() {
    let mut g: Graph<NodeLabel, EdgeLabel, GraphLabel> = Graph::new(GraphOptions {
        multigraph: true,
        compound: true,
        ..Default::default()
    });
    g.set_graph(GraphLabel {
        ranksep: 200.0,
        ..Default::default()
    });
    g.set_default_edge_label(EdgeLabel::default);

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
            weight: 2.0,
            ..Default::default()
        },
    );
    g.set_edge("b", "a");

    layout(&mut g);

    assert_eq!(
        coords(&g),
        [
            ("a".to_string(), (100.0 / 2.0, 100.0 / 2.0)),
            ("b".to_string(), (100.0 / 2.0, 100.0 + 200.0 + 100.0 / 2.0)),
        ]
        .into()
    );

    let ab = g.edge("a", "b", None).unwrap();
    let ba = g.edge("b", "a", None).unwrap();
    assert!(ab.points[1].y > ab.points[0].y);
    assert!(ba.points[0].y > ba.points[1].y);
}

#[test]
fn layout_adds_rectangle_intersects_for_edges() {
    let mut g: Graph<NodeLabel, EdgeLabel, GraphLabel> = Graph::new(GraphOptions {
        multigraph: true,
        compound: true,
        ..Default::default()
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
        ..Default::default()
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
            ..Default::default()
        });
        g.set_graph(GraphLabel {
            rankdir,
            nodesep: 10.0,
            ranksep: 10.0,
            edgesep: 10.0,
            ..Default::default()
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

#[test]
fn layout_can_layout_an_edge_with_a_long_label() {
    for rankdir in [RankDir::TB, RankDir::BT, RankDir::LR, RankDir::RL] {
        let mut g: Graph<NodeLabel, EdgeLabel, GraphLabel> = Graph::new(GraphOptions {
            multigraph: true,
            compound: true,
            ..Default::default()
        });
        g.set_graph(GraphLabel {
            rankdir,
            nodesep: 10.0,
            ranksep: 50.0,
            edgesep: 10.0,
            ..Default::default()
        });
        g.set_default_edge_label(EdgeLabel::default);

        for v in ["a", "b", "c", "d"] {
            g.set_node(
                v,
                NodeLabel {
                    width: 10.0,
                    height: 10.0,
                    ..Default::default()
                },
            );
        }
        g.set_edge_with_label(
            "a",
            "c",
            EdgeLabel {
                width: 2000.0,
                height: 10.0,
                ..Default::default()
            },
        );
        g.set_edge_with_label(
            "b",
            "d",
            EdgeLabel {
                width: 1.0,
                height: 1.0,
                ..Default::default()
            },
        );

        layout(&mut g);

        if rankdir == RankDir::TB || rankdir == RankDir::BT {
            let p1 = g.edge("a", "c", None).unwrap();
            let p2 = g.edge("b", "d", None).unwrap();
            assert!((p1.x.unwrap() - p2.x.unwrap()).abs() > 1000.0);
        } else {
            let p1 = g.node("a").unwrap();
            let p2 = g.node("c").unwrap();
            assert!((p1.x.unwrap() - p2.x.unwrap()).abs() > 1000.0);
        }
    }
}

#[test]
fn layout_can_layout_a_self_loop() {
    for rankdir in [RankDir::TB, RankDir::BT, RankDir::LR, RankDir::RL] {
        let mut g: Graph<NodeLabel, EdgeLabel, GraphLabel> = Graph::new(GraphOptions {
            multigraph: true,
            compound: true,
            ..Default::default()
        });
        g.set_graph(GraphLabel {
            rankdir,
            nodesep: 50.0,
            ranksep: 50.0,
            edgesep: 75.0,
            ..Default::default()
        });
        g.set_default_edge_label(EdgeLabel::default);

        g.set_node(
            "a",
            NodeLabel {
                width: 100.0,
                height: 100.0,
                ..Default::default()
            },
        );
        g.set_edge_with_label(
            "a",
            "a",
            EdgeLabel {
                width: 50.0,
                height: 50.0,
                ..Default::default()
            },
        );

        layout(&mut g);
        let node_a = g.node("a").unwrap();
        let points = &g.edge("a", "a", None).unwrap().points;
        assert_eq!(points.len(), 7);
        for p in points {
            if rankdir != RankDir::LR && rankdir != RankDir::RL {
                assert!(p.x > node_a.x.unwrap());
                assert!((p.y - node_a.y.unwrap()).abs() <= node_a.height / 2.0);
            } else {
                assert!(p.y > node_a.y.unwrap());
                assert!((p.x - node_a.x.unwrap()).abs() <= node_a.width / 2.0);
            }
        }
    }
}

#[test]
fn layout_can_layout_a_graph_with_subgraphs() {
    let mut g: Graph<NodeLabel, EdgeLabel, GraphLabel> = Graph::new(GraphOptions {
        multigraph: true,
        compound: true,
        ..Default::default()
    });
    g.set_graph(GraphLabel::default());
    g.set_default_edge_label(EdgeLabel::default);

    g.set_node(
        "a",
        NodeLabel {
            width: 50.0,
            height: 50.0,
            ..Default::default()
        },
    );
    g.set_parent("a", "sg1");
    layout(&mut g);

    // The canonical Dagre pipeline derives and publishes compound-node geometry from border nodes.
    assert!(g.has_node("sg1"));
    let sg = g.node("sg1").unwrap();
    assert!(sg.x.is_some_and(f64::is_finite));
    assert!(sg.y.is_some_and(f64::is_finite));
    assert!(sg.width > 0.0);
    assert!(sg.height > 0.0);
}

#[test]
fn layout_matches_mermaid_for_state_recursive_regions_with_implicit_parent_order() {
    const ROOT_START: &str = "state-root_start-0";
    const RUNNING: &str = "state-Running-5";
    const DIVIDER_1: &str = "state-divider-id-1-1";
    const DIVIDER_1_START: &str = "state-divider-id-1_start-1";
    const R1: &str = "state-r1-5";
    const DIVIDER_2: &str = "state-divider-id-2-2";
    const DIVIDER_2_START: &str = "state-divider-id-2_start-2";
    const R2: &str = "state-r2-5";
    const DIVIDER_3: &str = "state-divider-id-3-3";
    const DIVIDER_3_START: &str = "state-divider-id-3_start-3";
    const R3: &str = "state-r3-5";
    const REGION: &str = "state-id-38dvt516gkfs-1-4";
    const REGION_START: &str = "state-id-38dvt516gkfs-1_start-4";
    const R4: &str = "state-r4-5";
    const ROOT_END: &str = "state-root_end-5";

    let mut g: Graph<NodeLabel, EdgeLabel, GraphLabel> = Graph::new(GraphOptions {
        multigraph: true,
        compound: true,
        directed: true,
    });
    g.set_graph(GraphLabel {
        marginx: 8.0,
        marginy: 8.0,
        ..Default::default()
    });

    // The source declares REGION after r1-r3. Interleaved setParent calls insert the unseen parent
    // immediately after r1, matching Mermaid's State graph construction.
    for (id, width, height, parent) in [
        (ROOT_START, 14.0, 14.0, None),
        (RUNNING, 1.0, 1.0, None),
        (DIVIDER_1, 1.0, 1.0, Some(RUNNING)),
        (DIVIDER_1_START, 14.0, 14.0, Some(DIVIDER_1)),
        (R1, 216.0, 64.0, Some(REGION)),
        (DIVIDER_2, 1.0, 1.0, Some(RUNNING)),
        (DIVIDER_2_START, 14.0, 14.0, Some(DIVIDER_2)),
        (R2, 216.0, 64.0, Some(REGION)),
        (DIVIDER_3, 1.0, 1.0, Some(RUNNING)),
        (DIVIDER_3_START, 14.0, 14.0, Some(DIVIDER_3)),
        (R3, 145.875, 40.0, Some(REGION)),
        (REGION, 1.0, 1.0, Some(RUNNING)),
        (REGION_START, 14.0, 14.0, Some(REGION)),
        (R4, 125.453125, 40.0, Some(REGION)),
        (ROOT_END, 14.0, 14.0, None),
    ] {
        g.set_node(
            id,
            NodeLabel {
                width,
                height,
                ..Default::default()
            },
        );
        if let Some(parent) = parent {
            g.set_parent(id, parent);
        }
    }

    let edge = EdgeLabel {
        labelpos: LabelPos::C,
        labeloffset: 10.0,
        weight: 1.0,
        ..Default::default()
    };
    for (name, from, to) in [
        ("edge1", DIVIDER_1_START, R1),
        ("edge2", DIVIDER_2_START, R2),
        ("edge3", DIVIDER_3_START, R3),
        ("edge4", REGION_START, R4),
        ("edge0", ROOT_START, DIVIDER_1_START),
        ("edge5", DIVIDER_1_START, ROOT_END),
    ] {
        g.set_edge_named(from, to, Some(name), Some(edge.clone()));
    }

    assert_eq!(
        g.node_ids(),
        [
            ROOT_START,
            RUNNING,
            DIVIDER_1,
            DIVIDER_1_START,
            R1,
            REGION,
            DIVIDER_2,
            DIVIDER_2_START,
            R2,
            DIVIDER_3,
            DIVIDER_3_START,
            R3,
            REGION_START,
            R4,
            ROOT_END,
        ]
    );

    layout(&mut g);

    assert_eq!((g.graph().width, g.graph().height), (1349.328125, 333.0));
    assert_eq!(g.node(R1).and_then(|node| node.x), Some(333.0));
    assert_eq!(g.node(R2).and_then(|node| node.x), Some(774.453125));
    assert_eq!(g.node(R3).and_then(|node| node.x), Some(1005.390625));
    assert_eq!(g.node(R4).and_then(|node| node.x), Some(553.7265625));
}

#[test]
fn layout_minimizes_the_height_of_subgraphs() {
    let mut g: Graph<NodeLabel, EdgeLabel, GraphLabel> = Graph::new(GraphOptions {
        multigraph: true,
        compound: true,
        ..Default::default()
    });
    g.set_graph(GraphLabel::default());
    g.set_default_edge_label(EdgeLabel::default);

    for v in ["a", "b", "c", "d", "x", "y"] {
        g.set_node(
            v,
            NodeLabel {
                width: 50.0,
                height: 50.0,
                ..Default::default()
            },
        );
    }

    // setPath(["a", "b", "c", "d"])
    g.set_edge("a", "b");
    g.set_edge("b", "c");
    g.set_edge("c", "d");

    g.set_edge_with_label(
        "a",
        "x",
        EdgeLabel {
            weight: 100.0,
            ..Default::default()
        },
    );
    g.set_edge_with_label(
        "y",
        "d",
        EdgeLabel {
            weight: 100.0,
            ..Default::default()
        },
    );
    g.set_parent("x", "sg");
    g.set_parent("y", "sg");

    layout(&mut g);
    assert_eq!(g.node("x").unwrap().y, g.node("y").unwrap().y);
}

#[test]
fn layout_minimizes_separation_between_nodes_not_adjacent_to_subgraphs() {
    let mut g: Graph<NodeLabel, EdgeLabel, GraphLabel> = Graph::new(GraphOptions {
        multigraph: true,
        compound: true,
        ..Default::default()
    });
    g.set_graph(GraphLabel::default());
    g.set_default_edge_label(EdgeLabel::default);

    for v in ["a", "b", "c"] {
        g.set_node(
            v,
            NodeLabel {
                width: 50.0,
                height: 50.0,
                ..Default::default()
            },
        );
    }
    g.set_edge("a", "b");
    g.set_edge("b", "c");
    g.ensure_node("sg");
    g.set_parent("c", "sg");

    layout(&mut g);

    assert_eq!(
        g.node("b").unwrap().y.unwrap() - g.node("a").unwrap().y.unwrap(),
        100.0
    );
}

#[test]
fn layout_can_layout_subgraphs_with_different_rankdirs() {
    for rankdir in [RankDir::TB, RankDir::BT, RankDir::LR, RankDir::RL] {
        let mut g: Graph<NodeLabel, EdgeLabel, GraphLabel> = Graph::new(GraphOptions {
            multigraph: true,
            compound: true,
            ..Default::default()
        });
        g.set_graph(GraphLabel {
            rankdir,
            ..Default::default()
        });
        g.set_default_edge_label(EdgeLabel::default);

        g.set_node(
            "a",
            NodeLabel {
                width: 50.0,
                height: 50.0,
                ..Default::default()
            },
        );
        g.ensure_node("sg");
        g.set_parent("a", "sg");

        layout(&mut g);

        let sg = g.node("sg").unwrap();
        assert!(sg.width > 50.0);
        assert!(sg.height > 50.0);
        assert!(sg.x.unwrap() > 50.0 / 2.0);
        assert!(sg.y.unwrap() > 50.0 / 2.0);
    }
}

#[test]
fn layout_adds_dimensions_to_graph() {
    let mut g: Graph<NodeLabel, EdgeLabel, GraphLabel> = Graph::new(GraphOptions {
        multigraph: true,
        compound: true,
        ..Default::default()
    });
    g.set_graph(GraphLabel::default());
    g.set_default_edge_label(EdgeLabel::default);

    g.set_node(
        "a",
        NodeLabel {
            width: 100.0,
            height: 50.0,
            ..Default::default()
        },
    );

    layout(&mut g);

    assert_eq!(g.graph().width, 100.0);
    assert_eq!(g.graph().height, 50.0);
}

#[test]
fn layout_graph_dimensions_include_margins() {
    let mut g: Graph<NodeLabel, EdgeLabel, GraphLabel> = Graph::new(GraphOptions {
        multigraph: true,
        compound: true,
        ..Default::default()
    });
    g.set_graph(GraphLabel {
        marginx: 8.0,
        marginy: 10.0,
        ..Default::default()
    });
    g.set_default_edge_label(EdgeLabel::default);

    g.set_node(
        "a",
        NodeLabel {
            width: 100.0,
            height: 50.0,
            ..Default::default()
        },
    );

    layout(&mut g);

    let a = g.node("a").unwrap();
    assert_eq!(a.x, Some(50.0 + 8.0));
    assert_eq!(a.y, Some(25.0 + 10.0));
    assert_eq!(g.graph().width, 100.0 + 8.0 * 2.0);
    assert_eq!(g.graph().height, 50.0 + 10.0 * 2.0);
}

#[test]
fn layout_keeps_node_coordinates_in_graph_bounding_box_for_rankdirs() {
    for rankdir in [RankDir::TB, RankDir::BT, RankDir::LR, RankDir::RL] {
        let mut g: Graph<NodeLabel, EdgeLabel, GraphLabel> = Graph::new(GraphOptions {
            multigraph: true,
            compound: true,
            ..Default::default()
        });
        g.set_graph(GraphLabel {
            rankdir,
            ..Default::default()
        });
        g.set_default_edge_label(EdgeLabel::default);

        g.set_node(
            "a",
            NodeLabel {
                width: 100.0,
                height: 200.0,
                ..Default::default()
            },
        );

        layout(&mut g);

        let a = g.node("a").unwrap();
        assert_eq!(a.x, Some(100.0 / 2.0));
        assert_eq!(a.y, Some(200.0 / 2.0));
        assert_eq!(g.graph().width, 100.0);
        assert_eq!(g.graph().height, 200.0);
        assert!(a.x.unwrap() - a.width / 2.0 >= 0.0);
        assert!(a.x.unwrap() + a.width / 2.0 <= g.graph().width);
        assert!(a.y.unwrap() - a.height / 2.0 >= 0.0);
        assert!(a.y.unwrap() + a.height / 2.0 <= g.graph().height);
    }
}

#[test]
fn layout_keeps_left_edge_label_coordinates_in_graph_bounding_box_for_rankdirs() {
    for rankdir in [RankDir::TB, RankDir::BT, RankDir::LR, RankDir::RL] {
        let mut g: Graph<NodeLabel, EdgeLabel, GraphLabel> = Graph::new(GraphOptions {
            multigraph: true,
            compound: true,
            ..Default::default()
        });
        g.set_graph(GraphLabel {
            rankdir,
            ..Default::default()
        });
        g.set_default_edge_label(EdgeLabel::default);

        for v in ["a", "b"] {
            g.set_node(
                v,
                NodeLabel {
                    width: 100.0,
                    height: 100.0,
                    ..Default::default()
                },
            );
        }
        g.set_edge_with_label(
            "a",
            "b",
            EdgeLabel {
                width: 1000.0,
                height: 2000.0,
                labelpos: LabelPos::L,
                labeloffset: 0.0,
                ..Default::default()
            },
        );

        layout(&mut g);

        let edge = g.edge("a", "b", None).unwrap();
        if matches!(rankdir, RankDir::TB | RankDir::BT) {
            assert_eq!(edge.x, Some(1000.0 / 2.0));
        } else {
            assert_eq!(edge.y, Some(2000.0 / 2.0));
        }
        assert!(edge.x.unwrap() - edge.width / 2.0 >= 0.0);
        assert!(edge.x.unwrap() + edge.width / 2.0 <= g.graph().width);
        assert!(edge.y.unwrap() - edge.height / 2.0 >= 0.0);
        assert!(edge.y.unwrap() + edge.height / 2.0 <= g.graph().height);
    }
}
