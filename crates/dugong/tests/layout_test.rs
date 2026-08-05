use dugong::graphlib::{Graph, GraphOptions};
use dugong::{
    EdgeLabel, GraphLabel, LabelPos, LayoutError, NodeLabel, Point, RankDir, WorkControl,
    WorkError, layout, layout_controlled,
};

#[derive(Default)]
struct RecordingWorkControl {
    charges: Vec<usize>,
    remaining: Option<usize>,
    reject_at_call: Option<usize>,
}

impl RecordingWorkControl {
    fn with_limit(limit: usize) -> Self {
        Self {
            remaining: Some(limit),
            ..Self::default()
        }
    }

    fn reject_at(call: usize) -> Self {
        Self {
            reject_at_call: Some(call),
            ..Self::default()
        }
    }

    fn total(&self) -> usize {
        self.charges.iter().copied().sum()
    }
}

impl WorkControl for RecordingWorkControl {
    fn charge(&mut self, units: usize) -> Result<(), WorkError> {
        self.charges.push(units);
        if self.reject_at_call == Some(self.charges.len()) {
            return Err(WorkError::Interrupted);
        }
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

fn controlled_budget_graph() -> Graph<NodeLabel, EdgeLabel, GraphLabel> {
    let mut graph = Graph::new(GraphOptions {
        multigraph: true,
        compound: true,
        ..Default::default()
    });
    graph.set_graph(GraphLabel::default());
    graph.set_default_edge_label(EdgeLabel::default);
    for id in ["a", "b", "c"] {
        graph.set_node(
            id,
            NodeLabel {
                width: 40.0,
                height: 20.0,
                x: Some(-10.0),
                y: Some(-20.0),
                ..Default::default()
            },
        );
    }
    graph.set_edge_with_label(
        "a",
        "b",
        EdgeLabel {
            width: 12.0,
            height: 8.0,
            points: vec![Point { x: -1.0, y: -2.0 }],
            ..Default::default()
        },
    );
    graph.set_edge("b", "c");
    graph.graph_mut().width = -30.0;
    graph.graph_mut().height = -40.0;
    graph
}

fn controlled_self_loop_graph(rankdir: RankDir) -> Graph<NodeLabel, EdgeLabel, GraphLabel> {
    let mut graph = Graph::new(GraphOptions {
        multigraph: true,
        compound: true,
        ..Default::default()
    });
    graph.set_graph(GraphLabel {
        rankdir,
        nodesep: 50.0,
        ranksep: 50.0,
        edgesep: 75.0,
        ..Default::default()
    });
    graph.set_default_edge_label(EdgeLabel::default);
    graph.set_node(
        "a",
        NodeLabel {
            width: 100.0,
            height: 80.0,
            ..Default::default()
        },
    );
    graph.set_edge_with_label(
        "a",
        "a",
        EdgeLabel {
            width: 50.0,
            height: 30.0,
            ..Default::default()
        },
    );
    graph
}

fn assert_budget_graph_layout_eq(
    left: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    right: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
) {
    let left_options = left.options();
    let right_options = right.options();
    assert_eq!(left_options.multigraph, right_options.multigraph);
    assert_eq!(left_options.compound, right_options.compound);
    assert_eq!(left_options.directed, right_options.directed);

    let left_graph = left.graph();
    let right_graph = right.graph();
    assert_eq!(left_graph.rankdir, right_graph.rankdir);
    assert_eq!(left_graph.nodesep, right_graph.nodesep);
    assert_eq!(left_graph.ranksep, right_graph.ranksep);
    assert_eq!(left_graph.edgesep, right_graph.edgesep);
    assert_eq!(left_graph.marginx, right_graph.marginx);
    assert_eq!(left_graph.marginy, right_graph.marginy);
    assert_eq!(left_graph.width, right_graph.width);
    assert_eq!(left_graph.height, right_graph.height);
    assert_eq!(left_graph.align, right_graph.align);
    assert_eq!(left_graph.ranker, right_graph.ranker);
    assert_eq!(left_graph.acyclicer, right_graph.acyclicer);
    assert_eq!(left_graph.dummy_chains, right_graph.dummy_chains);
    assert_eq!(left_graph.nesting_root, right_graph.nesting_root);
    assert_eq!(left_graph.node_rank_factor, right_graph.node_rank_factor);

    assert_eq!(left.node_ids(), right.node_ids());
    for id in left.node_ids() {
        assert_eq!(left.node(&id), right.node(&id));
        assert_eq!(left.parent(&id), right.parent(&id));
        assert_eq!(left.children(&id), right.children(&id));
    }
    assert_eq!(left.edge_keys(), right.edge_keys());
    for key in left.edge_keys() {
        assert_eq!(left.edge_by_key(&key), right.edge_by_key(&key));
    }
}

#[test]
fn controlled_layout_records_work_and_matches_compatibility_output() {
    let mut compatibility = controlled_budget_graph();
    layout(&mut compatibility).unwrap();

    let mut controlled = controlled_budget_graph();
    let mut recorder = RecordingWorkControl::default();
    layout_controlled(&mut controlled, &mut recorder).unwrap();

    assert!(!recorder.charges.is_empty());
    assert!(recorder.total() > 0);
    assert_budget_graph_layout_eq(&compatibility, &controlled);
}

#[test]
fn controlled_layout_honors_below_equal_and_above_work_boundaries() {
    let mut measured = controlled_budget_graph();
    let mut recorder = RecordingWorkControl::default();
    layout_controlled(&mut measured, &mut recorder).unwrap();
    let exact = recorder.total();
    assert!(exact > 0);

    let mut below = controlled_budget_graph();
    let mut below_control = RecordingWorkControl::with_limit(exact - 1);
    assert_eq!(
        layout_controlled(&mut below, &mut below_control),
        Err(LayoutError::Work(WorkError::Interrupted))
    );
    let initial = controlled_budget_graph();
    assert_budget_graph_layout_eq(&below, &initial);

    for limit in [exact, exact + 1] {
        let mut graph = controlled_budget_graph();
        let mut control = RecordingWorkControl::with_limit(limit);
        layout_controlled(&mut graph, &mut control).unwrap();
        assert_budget_graph_layout_eq(&measured, &graph);
    }
}

#[test]
fn controlled_self_loop_layout_is_transactional_at_the_exact_boundary() {
    for rankdir in [RankDir::TB, RankDir::BT, RankDir::LR, RankDir::RL] {
        let mut measured = controlled_self_loop_graph(rankdir);
        let mut recorder = RecordingWorkControl::default();
        layout_controlled(&mut measured, &mut recorder).unwrap();
        let exact = recorder.total();
        assert!(exact > 0);

        let initial = controlled_self_loop_graph(rankdir);
        let mut rejected = controlled_self_loop_graph(rankdir);
        let mut below = RecordingWorkControl::with_limit(exact - 1);
        assert_eq!(
            layout_controlled(&mut rejected, &mut below),
            Err(LayoutError::Work(WorkError::Interrupted))
        );
        assert_budget_graph_layout_eq(&rejected, &initial);

        let mut admitted = controlled_self_loop_graph(rankdir);
        let mut exact_control = RecordingWorkControl::with_limit(exact);
        layout_controlled(&mut admitted, &mut exact_control).unwrap();
        assert_eq!(exact_control.remaining, Some(0));
        assert_budget_graph_layout_eq(&admitted, &measured);
    }
}

#[test]
fn controlled_layout_stops_after_the_first_rejected_tranche() {
    let mut graph = controlled_budget_graph();
    let mut control = RecordingWorkControl::reject_at(3);
    assert_eq!(
        layout_controlled(&mut graph, &mut control),
        Err(LayoutError::Work(WorkError::Interrupted))
    );
    assert_eq!(control.charges.len(), 3);
}

#[test]
fn controlled_layout_accepts_a_normal_250_node_sparse_graph() {
    fn sparse_graph() -> Graph<NodeLabel, EdgeLabel, GraphLabel> {
        let mut graph = Graph::new(GraphOptions {
            multigraph: true,
            compound: true,
            ..Default::default()
        });
        graph.set_graph(GraphLabel::default());
        graph.set_default_edge_label(EdgeLabel::default);
        for index in 0..250 {
            graph.set_node(
                format!("n{index}"),
                NodeLabel {
                    width: 40.0,
                    height: 20.0,
                    ..Default::default()
                },
            );
            if index > 0 {
                graph.set_edge(format!("n{}", index - 1), format!("n{index}"));
            }
        }
        graph
    }

    const LIMIT: usize = 125_000;

    let mut graph = sparse_graph();
    let mut control = RecordingWorkControl::default();
    layout_controlled(&mut graph, &mut control)
        .expect("a normal sparse graph fits the constrained profile budget");

    assert!(
        control.total() <= LIMIT,
        "sparse graph work was {}",
        control.total()
    );
    assert!(graph.nodes().all(|id| {
        graph
            .node(id)
            .is_some_and(|node| node.x.is_some() && node.y.is_some())
    }));

    let mut limited_graph = sparse_graph();
    let mut limited = RecordingWorkControl::with_limit(LIMIT);
    layout_controlled(&mut limited_graph, &mut limited)
        .expect("the constrained profile must admit the measured sparse graph");
    assert_budget_graph_layout_eq(&graph, &limited_graph);
}

#[test]
fn controlled_layout_accepts_a_normal_250_node_compound_graph() {
    fn compound_graph() -> Graph<NodeLabel, EdgeLabel, GraphLabel> {
        let mut graph = Graph::new(GraphOptions {
            multigraph: true,
            compound: true,
            ..Default::default()
        });
        graph.set_graph(GraphLabel::default());
        graph.set_default_edge_label(EdgeLabel::default);
        for cluster in 0..10 {
            graph.set_node(format!("cluster{cluster}"), NodeLabel::default());
        }
        for index in 0..240 {
            let id = format!("n{index}");
            graph.set_node(
                id.clone(),
                NodeLabel {
                    width: 40.0,
                    height: 20.0,
                    ..Default::default()
                },
            );
            graph.set_parent(id.clone(), format!("cluster{}", index / 24));
            if index > 0 {
                graph.set_edge("n0", id);
            }
        }
        graph
    }

    const LIMIT: usize = 300_000;

    let mut graph = compound_graph();
    let mut control = RecordingWorkControl::default();
    layout_controlled(&mut graph, &mut control)
        .expect("a normal compound graph fits the default profile budget");

    assert!(
        control.total() <= LIMIT,
        "compound graph work was {}",
        control.total()
    );
    assert!(graph.nodes().all(|id| {
        graph
            .node(id)
            .is_some_and(|node| node.x.is_some() && node.y.is_some())
    }));

    let mut limited_graph = compound_graph();
    let mut limited = RecordingWorkControl::with_limit(LIMIT);
    layout_controlled(&mut limited_graph, &mut limited)
        .expect("the default profile must admit the measured compound graph");
    assert_budget_graph_layout_eq(&graph, &limited_graph);
}

#[test]
fn controlled_layout_arithmetic_failure_preserves_the_entire_caller_graph() {
    fn overflow_graph() -> Graph<NodeLabel, EdgeLabel, GraphLabel> {
        let mut graph = Graph::new(GraphOptions {
            multigraph: true,
            compound: true,
            ..Default::default()
        });
        graph.set_graph(GraphLabel::default());
        graph.set_default_edge_label(EdgeLabel::default);
        graph.set_node("cluster", NodeLabel::default());
        graph.set_node("a", NodeLabel::default());
        graph.set_node("b", NodeLabel::default());
        graph.set_parent("a", "cluster");
        graph.set_edge_with_label(
            "a",
            "b",
            EdgeLabel {
                minlen: usize::MAX,
                ..Default::default()
            },
        );
        graph
    }

    let mut graph = overflow_graph();
    let initial = overflow_graph();
    let mut control = RecordingWorkControl::default();

    assert_eq!(
        layout_controlled(&mut graph, &mut control),
        Err(LayoutError::Work(WorkError::ArithmeticOverflow))
    );
    assert_budget_graph_layout_eq(&graph, &initial);

    let mut compatibility = overflow_graph();
    assert_eq!(
        layout(&mut compatibility),
        Err(LayoutError::Work(WorkError::ArithmeticOverflow))
    );
    assert_budget_graph_layout_eq(&compatibility, &initial);
}

#[test]
fn controlled_layout_rejects_cumulative_rank_span_overflow_transactionally() {
    fn overflow_graph() -> Graph<NodeLabel, EdgeLabel, GraphLabel> {
        let mut graph = Graph::new(GraphOptions {
            multigraph: true,
            compound: true,
            ..Default::default()
        });
        graph.set_graph(GraphLabel::default());
        graph.set_default_edge_label(EdgeLabel::default);
        for id in ["a", "b", "c"] {
            graph.set_node(id, NodeLabel::default());
        }
        for (from, to) in [("a", "b"), ("b", "c")] {
            graph.set_edge_with_label(
                from,
                to,
                EdgeLabel {
                    // The label-spacing pass doubles each edge to 2_147_483_646. Either edge
                    // fits i32 independently, but the complete rank path does not.
                    minlen: 1_073_741_823,
                    ..Default::default()
                },
            );
        }
        graph
    }

    let mut graph = overflow_graph();
    let initial = overflow_graph();
    let mut control = RecordingWorkControl::default();

    assert_eq!(
        layout_controlled(&mut graph, &mut control),
        Err(LayoutError::Work(WorkError::ArithmeticOverflow))
    );
    assert_budget_graph_layout_eq(&graph, &initial);
}

fn zero_minlen_graph(ranker: Option<&str>) -> Graph<NodeLabel, EdgeLabel, GraphLabel> {
    let mut graph = Graph::new(GraphOptions {
        multigraph: true,
        compound: true,
        ..Default::default()
    });
    graph.set_graph(GraphLabel {
        ranker: ranker.map(str::to_string),
        ..Default::default()
    });
    for id in ["a", "b"] {
        graph.set_node(
            id,
            NodeLabel {
                width: 10.0,
                height: 10.0,
                ..Default::default()
            },
        );
    }
    graph.set_edge_named(
        "a",
        "b",
        Some("zero"),
        Some(EdgeLabel {
            minlen: 0,
            weight: 1.0,
            labelpos: LabelPos::R,
            labeloffset: 10.0,
            ..Default::default()
        }),
    );
    graph
}

#[test]
fn full_layout_matches_mermaid_dagre_zero_minlen_network_simplex_geometry() {
    for ranker in [None, Some("network-simplex"), Some("unknown")] {
        let mut graph = zero_minlen_graph(ranker);

        layout(&mut graph).unwrap();

        assert_eq!(graph.graph().width, 10.0, "ranker={ranker:?}");
        assert_eq!(graph.graph().height, 45.0, "ranker={ranker:?}");
        assert_eq!(graph.node("a").and_then(|node| node.x), Some(5.0));
        assert_eq!(graph.node("a").and_then(|node| node.y), Some(5.0));
        assert_eq!(graph.node("b").and_then(|node| node.x), Some(5.0));
        assert_eq!(graph.node("b").and_then(|node| node.y), Some(40.0));
        let edge = graph.edge("a", "b", Some("zero")).unwrap();
        assert_eq!(edge.minlen, 0);
        assert_eq!(
            edge.points,
            vec![Point { x: 5.0, y: 10.0 }, Point { x: 5.0, y: 35.0 }],
            "ranker={ranker:?}"
        );
    }
}

#[test]
fn full_layout_returns_typed_degenerate_geometry_errors_transactionally() {
    for ranker in ["longest-path", "tight-tree"] {
        let initial = zero_minlen_graph(Some(ranker));
        let expected_edge = initial.edge_keys()[0].clone();
        let mut graph = zero_minlen_graph(Some(ranker));
        let mut recorder = RecordingWorkControl::default();

        let error = layout_controlled(&mut graph, &mut recorder).unwrap_err();
        assert!(
            matches!(
                error,
                LayoutError::DegenerateEdgeGeometry { ref edge, .. } if edge == &expected_edge
            ),
            "ranker={ranker}: {error}"
        );
        assert_budget_graph_layout_eq(&graph, &initial);

        let mut compatibility = zero_minlen_graph(Some(ranker));
        assert!(matches!(
            layout(&mut compatibility),
            Err(LayoutError::DegenerateEdgeGeometry { ref edge, .. }) if edge == &expected_edge
        ));
        assert_budget_graph_layout_eq(&compatibility, &initial);
    }
}

#[test]
fn controlled_zero_minlen_layout_still_honors_work_rejection_transactionally() {
    fn graph() -> Graph<NodeLabel, EdgeLabel, GraphLabel> {
        let mut graph = Graph::new(GraphOptions {
            multigraph: true,
            compound: true,
            ..Default::default()
        });
        graph.set_graph(GraphLabel::default());
        graph.set_node("a", NodeLabel::default());
        graph.set_node("b", NodeLabel::default());
        graph.set_edge_named(
            "a",
            "b",
            Some("zero"),
            Some(EdgeLabel {
                minlen: 0,
                ..Default::default()
            }),
        );
        graph
    }

    let initial = graph();
    let mut rejected = graph();
    let mut limit = RecordingWorkControl::with_limit(0);
    assert_eq!(
        layout_controlled(&mut rejected, &mut limit),
        Err(LayoutError::Work(WorkError::Interrupted))
    );
    assert_budget_graph_layout_eq(&rejected, &initial);
}

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
fn layout_can_layout_an_empty_graph() {
    let mut graph: Graph<NodeLabel, EdgeLabel, GraphLabel> = Graph::new(GraphOptions {
        multigraph: true,
        compound: true,
        ..Default::default()
    });
    graph.set_graph(GraphLabel::default());
    graph.set_default_edge_label(EdgeLabel::default);

    layout(&mut graph).unwrap();

    assert!(graph.node_ids().is_empty());
    assert!(graph.edge_keys().is_empty());
    assert_eq!(graph.graph().width, 0.0);
    assert_eq!(graph.graph().height, 0.0);
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

    layout(&mut g).unwrap();
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

    layout(&mut g).unwrap();
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

    layout(&mut g).unwrap();
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

    layout(&mut g).unwrap();
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

    layout(&mut g).unwrap();

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

    layout(&mut graph).unwrap();
    layout(&mut graph).unwrap();
    layout(&mut fresh).unwrap();

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

    layout(&mut g).unwrap();

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

    layout(&mut g).unwrap();
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

    layout(&mut g).unwrap();
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

        layout(&mut g).unwrap();

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

        layout(&mut g).unwrap();

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

        layout(&mut g).unwrap();
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
fn layout_non_square_self_loop_matches_pinned_dagre_phase_order() {
    // Generated from repo-ref/dagre-d3-es-7.0.14 at
    // 4b95ad0a0bca6ce4cfa4cb83dfa9be19ca740e8f. The non-square label makes an
    // insertSelfEdges/coordinateSystem phase inversion observable for LR/RL.
    let cases = [
        (
            RankDir::TB,
            (212.5, 80.0),
            (187.5, 40.0),
            [
                (100.0, 21.538_461_538_461_54),
                (158.333_333_333_333_34, 0.0),
                (172.916_666_666_666_69, 0.0),
                (187.5, 40.0),
                (172.916_666_666_666_69, 80.0),
                (158.333_333_333_333_34, 80.0),
                (100.0, 58.461_538_461_538_46),
            ],
        ),
        (
            RankDir::BT,
            (212.5, 80.0),
            (187.5, 40.0),
            [
                (100.0, 58.461_538_461_538_46),
                (158.333_333_333_333_34, 80.0),
                (172.916_666_666_666_69, 80.0),
                (187.5, 40.0),
                (172.916_666_666_666_69, 0.0),
                (158.333_333_333_333_34, 0.0),
                (100.0, 21.538_461_538_461_54),
            ],
        ),
        (
            RankDir::LR,
            (100.0, 182.5),
            (50.0, 157.5),
            [
                (28.181_818_181_818_18, 80.0),
                (0.0, 131.666_666_666_666_66),
                (0.0, 144.583_333_333_333_31),
                (50.0, 157.5),
                (100.0, 144.583_333_333_333_31),
                (100.0, 131.666_666_666_666_66),
                (71.818_181_818_181_81, 80.0),
            ],
        ),
        (
            RankDir::RL,
            (100.0, 182.5),
            (50.0, 157.5),
            [
                (71.818_181_818_181_81, 80.0),
                (100.0, 131.666_666_666_666_66),
                (100.0, 144.583_333_333_333_31),
                (50.0, 157.5),
                (0.0, 144.583_333_333_333_31),
                (0.0, 131.666_666_666_666_66),
                (28.181_818_181_818_18, 80.0),
            ],
        ),
    ];

    for (rankdir, graph_size, edge_center, expected_points) in cases {
        let mut graph = controlled_self_loop_graph(rankdir);
        layout(&mut graph).unwrap();

        assert_eq!((graph.graph().width, graph.graph().height), graph_size);
        let node = graph.node("a").unwrap();
        assert_eq!((node.x, node.y), (Some(50.0), Some(40.0)));
        assert_eq!((node.width, node.height), (100.0, 80.0));
        let edge = graph.edge("a", "a", None).unwrap();
        assert_eq!((edge.x, edge.y), (Some(edge_center.0), Some(edge_center.1)));
        assert_eq!((edge.width, edge.height), (50.0, 30.0));
        assert_eq!(edge.points.len(), expected_points.len());
        for (actual, expected) in edge.points.iter().zip(expected_points) {
            assert!(
                (actual.x - expected.0).abs() <= f64::EPSILON
                    && (actual.y - expected.1).abs() <= f64::EPSILON,
                "{rankdir:?}: expected {expected:?}, got ({}, {})",
                actual.x,
                actual.y
            );
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
    layout(&mut g).unwrap();

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

    layout(&mut g).unwrap();

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

    layout(&mut g).unwrap();
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

    layout(&mut g).unwrap();

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

        layout(&mut g).unwrap();

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

    layout(&mut g).unwrap();

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

    layout(&mut g).unwrap();

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

        layout(&mut g).unwrap();

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

        layout(&mut g).unwrap();

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
