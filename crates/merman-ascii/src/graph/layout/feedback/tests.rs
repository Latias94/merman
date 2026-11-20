use super::*;
use crate::AsciiRenderOptions;
use crate::graph::model::{GraphEdgeAttrs, GraphEdgeMarker, GraphGroupStyle};
use crate::graph::shape::{GraphNodeShapeSemantics, GraphNodeSide};
use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy};
use merman_model::{OperationControl, OperationPhase};

fn capacity_graph(shape: GraphNodeShape, direction: GraphDirection) -> AsciiGraph {
    let mut graph = AsciiGraph::new(direction);
    graph.add_node_with_shape_and_style("source", "", shape, Default::default());
    graph.add_node("target", "T");
    for _ in 0..97 {
        graph.add_edge_with_attrs(
            "source",
            "target",
            GraphEdgeAttrs {
                start_marker: GraphEdgeMarker::Circle,
                end_marker: GraphEdgeMarker::Open,
                ..Default::default()
            },
        );
    }
    graph
}

fn measured(
    graph: &AsciiGraph,
    feedback: &LayoutFeedback,
    resources: &mut ResourceContext,
) -> MeasuredLayout {
    let policy = AsciiRenderOptions::ascii()
        .flowchart_layout()
        .graph_policy();
    let limits = resources.policy();
    super::super::layout_graph_with_feedback(
        graph,
        &policy,
        feedback,
        resources,
        crate::operation::AsciiExecution::for_test(&limits),
    )
    .unwrap()
}

fn capacity_miss() -> AsciiError {
    AsciiError::PortCapacityExceeded {
        diagram_type: "flowchart",
        endpoint: "source".into(),
        required: 97,
        available: 96,
    }
}

fn component_rank_graph(direction: GraphDirection) -> AsciiGraph {
    let mut graph = AsciiGraph::new(direction);
    for prefix in ["a", "b"] {
        for index in 0..5 {
            let id = format!("{prefix}{index}");
            graph.add_node(&id, &id);
        }
        for index in 0..5 {
            graph.add_edge(
                format!("{prefix}{index}"),
                format!("{prefix}{}", (index + 1) % 5),
            );
        }
    }
    for index in 0..5 {
        graph.add_edge(format!("a{index}"), format!("b{index}"));
    }
    graph.add_node("tail", "Tail");
    graph.add_edge("b4", "tail");
    graph
}

#[test]
fn measured_bar_deficit_reserves_independent_contacts_at_both_ends() {
    let graph = capacity_graph(GraphNodeShape::ForkJoinHorizontal, GraphDirection::TopDown);
    let policy = AsciiRenderOptions::ascii()
        .flowchart_layout()
        .graph_policy();
    let mut resources = ResourceContext::new(AsciiResourcePolicy::unbounded());
    let mut feedback = LayoutFeedback::default();
    let initial = measured(&graph, &feedback, &mut resources);
    assert_eq!(initial.layout.nodes[0].width, 7);
    assert!(
        feedback
            .record_failure(&graph, &policy, &initial, &capacity_miss(), &resources)
            .unwrap()
    );
    let rebuilt = measured(&graph, &feedback, &mut resources);
    let source = &rebuilt.layout.nodes[0];
    let target = &rebuilt.layout.nodes[1];
    assert_eq!((source.width, source.height), (293, 3));
    assert!(target.width >= 293);
    assert_eq!(source.shape, GraphNodeShape::ForkJoinHorizontal);
    assert_eq!(source.grid, initial.layout.nodes[0].grid);
    assert_eq!(target.grid, initial.layout.nodes[1].grid);
    // Known constructive geometry: translating these two contours to (8,8) and (8,24)
    // admits 97 private vertical routes at x = 10 + 3*i, from y=11 through y=23.
    // The routing owner independently admits and writes those paths; no search assumption
    // is needed to establish the measured long-axis capacity required here.
    for index in 0..97 {
        let offset = 2 + 3 * index;
        assert!(offset < source.width - 1);
        assert!(offset < target.width - 1);
    }
    let mut source = source.clone();
    let mut target = target.clone();
    (source.x, source.y) = (8, 8);
    (target.x, target.y, target.height) = (8, 24, 3);
    for index in 0..97 {
        let x = 10 + 3 * index;
        assert_eq!(
            GraphNodeShapeSemantics::new(source.shape).route_contact(
                &source,
                GraphNodeSide::Bottom,
                x
            ),
            Some(super::super::CanvasCoord { x, y: 10 }),
        );
        assert_eq!(
            GraphNodeShapeSemantics::new(target.shape).route_contact(
                &target,
                GraphNodeSide::Top,
                x
            ),
            Some(super::super::CanvasCoord { x, y: 24 }),
        );
    }
    assert!(
        !feedback
            .record_failure(&graph, &policy, &rebuilt, &capacity_miss(), &resources)
            .unwrap()
    );
}

#[test]
fn empty_feedback_does_not_expand_readable_fans() {
    let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
    graph.add_node("source", "S");
    for index in 0..12 {
        let id = format!("n{index}");
        graph.add_node(&id, &id);
        graph.add_edge("source", &id);
    }
    let mut resources = ResourceContext::new(AsciiResourcePolicy::unbounded());
    let initial = measured(&graph, &LayoutFeedback::default(), &mut resources).layout;
    assert_eq!(initial.nodes[0].height, 3);
    assert!(
        initial
            .nodes
            .iter()
            .map(|node| node.bottom() + 1)
            .max()
            .unwrap()
            <= 48
    );
}

#[test]
fn fixed_terminals_keep_their_shape_label_and_dimensions() {
    let policy = AsciiRenderOptions::ascii()
        .flowchart_layout()
        .graph_policy();
    for shape in [
        GraphNodeShape::Text,
        GraphNodeShape::StateStart,
        GraphNodeShape::StateEnd,
        GraphNodeShape::Choice,
    ] {
        let graph = capacity_graph(shape, GraphDirection::TopDown);
        let mut resources = ResourceContext::new(AsciiResourcePolicy::unbounded());
        let mut feedback = LayoutFeedback::default();
        let initial = measured(&graph, &feedback, &mut resources);
        feedback
            .record_failure(&graph, &policy, &initial, &capacity_miss(), &resources)
            .unwrap();
        let rebuilt = measured(&graph, &feedback, &mut resources);
        let before = &initial.layout.nodes[0];
        let after = &rebuilt.layout.nodes[0];
        assert_eq!(
            (after.shape, &after.label, after.width, after.height),
            (before.shape, &before.label, before.width, before.height)
        );
    }
}

#[test]
fn feedback_growth_retains_local_direction_and_nested_containment() {
    let mut graph = capacity_graph(
        GraphNodeShape::ForkJoinHorizontal,
        GraphDirection::LeftRight,
    );
    graph.add_group_with_style(
        "inner",
        "Inner",
        Some(GraphDirection::TopDown),
        vec!["source".into(), "target".into()],
        GraphGroupStyle::default(),
    );
    graph.add_group_with_style(
        "outer",
        "Outer",
        None,
        vec!["inner".into()],
        GraphGroupStyle::default(),
    );
    let policy = AsciiRenderOptions::ascii()
        .flowchart_layout()
        .graph_policy();
    let mut resources = ResourceContext::new(AsciiResourcePolicy::unbounded());
    let mut feedback = LayoutFeedback::default();
    let initial = measured(&graph, &feedback, &mut resources);
    feedback
        .record_failure(&graph, &policy, &initial, &capacity_miss(), &resources)
        .unwrap();
    let rebuilt = measured(&graph, &feedback, &mut resources);
    for (before, after) in initial.layout.nodes.iter().zip(&rebuilt.layout.nodes) {
        assert_eq!(before.grid, after.grid);
        let inner = &rebuilt.layout.groups[0];
        assert!(inner.x < after.x && after.right() < inner.right());
        assert!(inner.y < after.y && after.bottom() < inner.bottom());
    }
    let inner = &rebuilt.layout.groups[0];
    let outer = &rebuilt.layout.groups[1];
    assert!(outer.x < inner.x && inner.right() < outer.right());
    assert!(outer.y < inner.y && inner.bottom() < outer.bottom());
    assert!(rebuilt.layout.nodes[0].y < rebuilt.layout.nodes[1].y);
}

#[test]
fn feedback_uses_the_existing_grid_limit() {
    let graph = capacity_graph(GraphNodeShape::ForkJoinHorizontal, GraphDirection::TopDown);
    let policy = AsciiRenderOptions::ascii()
        .flowchart_layout()
        .graph_policy();
    let mut resources = ResourceContext::new(AsciiResourcePolicy::unbounded());
    let initial = measured(&graph, &LayoutFeedback::default(), &mut resources);
    let mut feedback = LayoutFeedback::default();
    feedback
        .record_failure(&graph, &policy, &initial, &capacity_miss(), &resources)
        .unwrap();
    let limits = AsciiResourcePolicy::unbounded()
        .with_limit(AsciiResourceLimitId::MaxGridCells, 292 * 3)
        .unwrap();
    let mut limited = ResourceContext::new(limits);
    assert!(matches!(
        super::super::layout_graph_with_feedback(
            &graph,
            &policy,
            &feedback,
            &mut limited,
            crate::operation::AsciiExecution::for_test(&limits)
        ),
        Err(AsciiError::ResourceLimitExceeded(_))
    ));
}

#[test]
fn scoped_overlapping_returns_reserve_independent_lanes_in_every_direction() {
    let policy = AsciiRenderOptions::ascii()
        .flowchart_layout()
        .graph_policy();
    let directions = [
        GraphDirection::LeftRight,
        GraphDirection::RightLeft,
        GraphDirection::TopDown,
        GraphDirection::BottomTop,
    ];
    for root_direction in directions {
        for direction in directions {
            let horizontal = direction.canonical() == GraphDirection::LeftRight;
            let mut graph = AsciiGraph::new(root_direction);
            for id in ["a", "b", "c", "outside"] {
                graph.add_node(id, id);
            }
            for (index, (from, to)) in [("a", "b"), ("b", "c"), ("c", "a"), ("c", "a")]
                .into_iter()
                .enumerate()
            {
                let label = if index < 2 {
                    "s"
                } else if horizontal {
                    "r\nx\ny\nz"
                } else {
                    "longreturn"
                };
                graph.add_edge_with_attrs(
                    from,
                    to,
                    GraphEdgeAttrs {
                        label: Some(label.into()),
                        ..Default::default()
                    },
                );
            }
            graph.add_edge("outside", "outside");
            graph.add_group_with_style(
                "scope",
                "Scope",
                Some(direction),
                vec!["a".into(), "b".into(), "c".into()],
                GraphGroupStyle::default(),
            );
            let mut resources = ResourceContext::new(AsciiResourcePolicy::unbounded());
            let mut initial = measured(&graph, &LayoutFeedback::default(), &mut resources);
            // Independently prescribed intervals: either two c -> a returns cover [10,50],
            // or the opposite direction makes a -> b and b -> c overlap at their turn rank.
            // The unrelated root loop belongs to a different scope and cannot add a lane here.
            for (index, node) in initial.layout.nodes[..3].iter_mut().enumerate() {
                node.x = if horizontal { 10 + index * 20 } else { 10 };
                node.y = if horizontal { 10 } else { 10 + index * 20 };
            }
            let request = scoped_return_demand(&graph, &policy, &initial, 2, &resources)
                .unwrap()
                .unwrap();
            assert_eq!(request.scope, Some(0));
            assert_eq!(request.cross_x, !horizontal);
            // The root mirror reverses only its own axis. Distinct forward/return label
            // pitches make an incorrect sign choose a different measured requirement.
            let reversed = match direction {
                GraphDirection::LeftRight => root_direction == GraphDirection::RightLeft,
                GraphDirection::RightLeft => root_direction != GraphDirection::RightLeft,
                GraphDirection::TopDown => root_direction == GraphDirection::BottomTop,
                GraphDirection::BottomTop => root_direction != GraphDirection::BottomTop,
            };
            let expected = if reversed {
                8
            } else if horizontal {
                14
            } else {
                26
            };
            assert_eq!(request.cells, expected, "{root_direction:?}/{direction:?}");
            let input = LayoutInput::prepare(&graph, &policy, &mut resources.clone()).unwrap();
            let mut initial_feedback = LayoutFeedback::default();
            assert!(
                initial_feedback
                    .record_initial_returns_with_input(&input, &initial, &resources)
                    .unwrap()
            );
            let initial_request = initial_feedback
                .returns
                .iter()
                .find(|request| request.scope == Some(0))
                .unwrap();
            assert_eq!(initial_request.cross_x, request.cross_x);
            assert_eq!(initial_request.cells, request.cells);
            let mut feedback = LayoutFeedback::default();
            let miss = AsciiError::RoutingSearchExhausted {
                diagram_type: "flowchart",
                edge_index: 2,
            };
            assert!(
                feedback
                    .record_return_failure(&graph, &policy, &initial, &miss, &resources)
                    .unwrap()
            );
            let rebuilt = measured(&graph, &feedback, &mut resources);
            let scope = &rebuilt.layout.groups[0];
            for node in &rebuilt.layout.nodes[..3] {
                assert!(scope.x < node.x && node.right() < scope.right());
                assert!(scope.y < node.y && node.bottom() < scope.bottom());
                let margin = if horizontal {
                    (node.y - scope.y - 1).min(scope.bottom() - node.bottom() - 1)
                } else {
                    (node.x - scope.x - 1).min(scope.right() - node.right() - 1)
                };
                assert!(margin >= expected);
            }
        }
    }
}

#[test]
fn initial_return_analysis_merges_components_without_mixing_scopes() {
    let policy = AsciiRenderOptions::ascii()
        .flowchart_layout()
        .graph_policy();
    let mut graph = AsciiGraph::new(GraphDirection::TopDown);
    for id in ["a", "b", "c", "d", "e", "f"] {
        graph.add_node(id, id);
    }
    for (from, to, label) in [
        ("a", "b", "f"),
        ("b", "a", "bbbb"),
        ("c", "d", "f"),
        ("d", "c", "dddd"),
        ("d", "c", "dddd"),
        ("e", "f", "f"),
        ("f", "e", "back\nmore\nlast"),
    ] {
        graph.add_edge_with_attrs(
            from,
            to,
            GraphEdgeAttrs {
                label: Some(label.into()),
                ..Default::default()
            },
        );
    }
    graph.add_group_with_style(
        "outer",
        "Outer",
        Some(GraphDirection::TopDown),
        vec![
            "a".into(),
            "b".into(),
            "c".into(),
            "d".into(),
            "inner".into(),
        ],
        Default::default(),
    );
    graph.add_group_with_style(
        "inner",
        "Inner",
        Some(GraphDirection::LeftRight),
        vec!["e".into(), "f".into()],
        Default::default(),
    );
    let mut resources = ResourceContext::new(AsciiResourcePolicy::unbounded());
    let mut initial = measured(&graph, &LayoutFeedback::default(), &mut resources);
    for (node, (x, y)) in initial.layout.nodes.iter_mut().zip([
        (2, 10),
        (2, 30),
        (20, 10),
        (20, 30),
        (52, 12),
        (62, 12),
    ]) {
        node.x = x;
        node.y = y;
    }
    for (group, (x, y, width, height)) in initial
        .layout
        .groups
        .iter_mut()
        .zip([(0, 0, 100, 80), (50, 10, 35, 40)])
    {
        group.x = x;
        group.y = y;
        group.width = width;
        group.height = height;
    }
    let input = LayoutInput::prepare(&graph, &policy, &mut resources).unwrap();
    let mut feedback = LayoutFeedback::default();
    assert!(
        feedback
            .record_initial_returns_with_input(&input, &initial, &resources)
            .unwrap()
    );
    assert_eq!(feedback.returns.len(), 2);
    for request in &feedback.returns {
        match request.scope {
            // Outer SCCs independently need 1*(4+2)+2=8 and 2*(4+2)+2=14.
            Some(0) => {
                assert!(request.cross_x);
                assert_eq!(request.cells, 14);
            }
            // The inner LR return has three rows: 1*(3+2)+2=7.
            Some(1) => {
                assert!(!request.cross_x);
                assert_eq!(request.cells, 7);
            }
            scope => panic!("unexpected request scope {scope:?}"),
        }
    }
}

#[test]
fn initial_return_analysis_has_exact_admission_and_cancellation() {
    let policy = AsciiRenderOptions::ascii()
        .flowchart_layout()
        .graph_policy();
    let mut graph = AsciiGraph::new(GraphDirection::TopDown);
    graph.add_node("a", "A");
    graph.add_node("b", "B");
    graph.add_edge("a", "b");
    graph.add_edge_with_attrs(
        "b",
        "a",
        GraphEdgeAttrs {
            label: Some("a complete return condition".into()),
            ..Default::default()
        },
    );
    graph.add_group_with_style(
        "scope",
        "Scope",
        Some(GraphDirection::TopDown),
        vec!["a".into(), "b".into()],
        Default::default(),
    );
    let mut preparation = ResourceContext::new(AsciiResourcePolicy::unbounded());
    let initial = measured(&graph, &LayoutFeedback::default(), &mut preparation);
    let input = LayoutInput::prepare(&graph, &policy, &mut preparation).unwrap();
    let resources = ResourceContext::new(AsciiResourcePolicy::unbounded());
    assert!(
        LayoutFeedback::default()
            .record_initial_returns_with_input(&input, &initial, &resources)
            .unwrap()
    );
    let work = resources.layout_work_used();
    for (limit, admitted) in [(work, true), (work - 1, false)] {
        let resources = ResourceContext::new(
            AsciiResourcePolicy::unbounded()
                .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, limit)
                .unwrap(),
        );
        let mut feedback = LayoutFeedback::default();
        let result = feedback.record_initial_returns_with_input(&input, &initial, &resources);
        assert_eq!(result.is_ok(), admitted);
        if !admitted {
            assert!(matches!(result, Err(AsciiError::ResourceLimitExceeded(_))));
            assert!(
                feedback.returns.is_empty(),
                "incomplete analysis must not commit requests"
            );
        }
    }
    for during in [false, true] {
        let control = OperationControl::new();
        if during {
            control.cancel_after_checkpoints(10);
        } else {
            control.cancel();
        }
        let resources = ResourceContext::new(AsciiResourcePolicy::unbounded())
            .controlled(control, OperationPhase::Layout);
        let mut feedback = LayoutFeedback::default();
        assert!(matches!(
            feedback.record_initial_returns_with_input(&input, &initial, &resources),
            Err(AsciiError::Cancelled(_))
        ));
        assert!(feedback.returns.is_empty());
        assert_eq!(resources.layout_work_used() > 0, during);
    }
}

#[test]
fn measured_corridor_gap_survives_flat_compaction_and_keeps_minlen_ranks() {
    let policy = AsciiRenderOptions::ascii()
        .flowchart_layout()
        .graph_policy();
    for direction in [GraphDirection::LeftRight, GraphDirection::TopDown] {
        let mut graph = AsciiGraph::new(direction);
        graph.add_node("source", "S");
        graph.add_node("target", "T");
        for _ in 0..6 {
            graph.add_edge("source", "target");
        }
        for edge in &mut graph.edges {
            edge.length = 3;
        }
        let mut resources = ResourceContext::new(AsciiResourcePolicy::unbounded());
        let initial = measured(&graph, &LayoutFeedback::default(), &mut resources);
        let mut feedback = LayoutFeedback::default();
        let miss = AsciiError::RoutingSearchExhausted {
            diagram_type: "flowchart",
            edge_index: 0,
        };
        assert!(
            feedback
                .record_failure(&graph, &policy, &initial, &miss, &resources)
                .unwrap()
        );
        let rebuilt = measured(&graph, &feedback, &mut resources);
        let source = &rebuilt.layout.nodes[0];
        let target = &rebuilt.layout.nodes[1];
        assert_eq!(source.grid, initial.layout.nodes[0].grid);
        assert_eq!(target.grid, initial.layout.nodes[1].grid);
        let gap = if direction == GraphDirection::LeftRight {
            target.x - source.right() - 1
        } else {
            target.y - source.bottom() - 1
        };
        assert!(gap >= 12);
        assert!(
            !feedback
                .record_failure(&graph, &policy, &rebuilt, &miss, &resources)
                .unwrap()
        );
    }
}

#[test]
fn component_rank_candidate_keeps_every_direction_and_source_fact() {
    let policy = AsciiRenderOptions::ascii()
        .flowchart_layout()
        .graph_policy();
    for direction in [
        GraphDirection::LeftRight,
        GraphDirection::RightLeft,
        GraphDirection::TopDown,
        GraphDirection::BottomTop,
    ] {
        let mut graph = component_rank_graph(direction);
        graph.add_edge("a0", "a0");
        graph.add_edge("tail", "tail");
        graph.edges[10].id = Some("crossing".into());
        graph.edges[10].is_user_defined_id = true;
        graph.edges[10].label = Some("crossing condition".into());
        graph.edges[10].start_marker = GraphEdgeMarker::Circle;
        graph.edges[10].end_marker = GraphEdgeMarker::Cross;
        let original = graph.clone();
        let mut resources = ResourceContext::new(AsciiResourcePolicy::unbounded());
        let mut feedback = LayoutFeedback::default();
        let mut initial = measured(&graph, &feedback, &mut resources);
        let input = LayoutInput::prepare(&graph, &policy, &mut resources).unwrap();
        assert!(feedback.component_ranks().is_none());
        assert!(
            !feedback
                .record_initial_returns_with_input(&input, &initial, &resources)
                .unwrap()
        );
        assert!(feedback.component_ranks().is_none());
        feedback
            .record_failure_with_input(&input, &initial, &capacity_miss(), &resources)
            .unwrap();
        assert!(feedback.component_ranks().is_none());
        assert!(
            feedback
                .record_component_ranks_with_input(&input, &initial, &resources)
                .unwrap()
        );
        let expected = [0, 0, 0, 0, 0, 4, 4, 4, 4, 4, 8];
        assert_eq!(feedback.component_ranks(), Some(expected.as_slice()));
        assert!(feedback.corridors.is_empty());
        assert_eq!(graph, original);

        // A new analysis must not request another generation when only the transverse
        // placement differs; the candidate compares measured main-grid coordinates.
        for (node, &rank) in initial.layout.nodes.iter_mut().zip(&expected) {
            if direction.canonical() == GraphDirection::LeftRight {
                node.grid.x = rank;
                node.grid.y += 1;
            } else {
                node.grid.y = rank;
                node.grid.x += 1;
            }
        }
        let mut unchanged = LayoutFeedback::default();
        assert!(
            !unchanged
                .record_component_ranks_with_input(&input, &initial, &resources)
                .unwrap()
        );
        assert!(unchanged.component_ranks().is_none());
    }
}

#[test]
fn component_ranks_keep_parallel_minlen_and_longest_paths_with_singletons() {
    let mut graph = component_rank_graph(GraphDirection::TopDown);
    for (edge, length) in graph.edges[10..15].iter_mut().zip([0, 1, 3, 2, 5]) {
        edge.length = length;
    }
    graph.edges[15].length = 0;
    graph.add_edge_with_attrs(
        "a0",
        "tail",
        GraphEdgeAttrs {
            length: 5,
            ..Default::default()
        },
    );
    graph.add_node("isolated", "Isolated");
    let policy = AsciiRenderOptions::ascii()
        .flowchart_layout()
        .graph_policy();
    let mut resources = ResourceContext::new(AsciiResourcePolicy::unbounded());
    let initial = measured(&graph, &LayoutFeedback::default(), &mut resources);
    let input = LayoutInput::prepare(&graph, &policy, &mut resources).unwrap();
    let mut feedback = LayoutFeedback::default();
    assert!(
        feedback
            .record_component_ranks_with_input(&input, &initial, &resources)
            .unwrap()
    );
    // The five parallel condensation edges require five ranks. The zero-length tail
    // still needs one more, so this path wins over the direct five-rank source edge.
    assert_eq!(
        feedback.component_ranks(),
        Some([0, 0, 0, 0, 0, 20, 20, 20, 20, 20, 24, 0].as_slice())
    );
}

#[test]
fn component_rank_feedback_rejects_unsupported_scenes() {
    use crate::graph::model::{
        GraphEdgeStroke, GraphNodeSide as AuthoredNodeSide, GraphNodeSideConstraint,
    };

    let graph = component_rank_graph(GraphDirection::TopDown);
    let mut other_family = graph.clone();
    other_family.diagram_type = "stateDiagram";
    let mut grouped = graph.clone();
    grouped.add_group_with_style(
        "scope",
        "Scope",
        None,
        vec![
            "a0".into(),
            "a1".into(),
            "a2".into(),
            "a3".into(),
            "a4".into(),
        ],
        Default::default(),
    );
    let mut constrained = graph.clone();
    constrained.nodes[1].semantics.side_constraint =
        Some(GraphNodeSideConstraint::new("a0", AuthoredNodeSide::Left));
    let mut invisible = graph.clone();
    invisible.edges[10].stroke = GraphEdgeStroke::Invisible;
    let mut acyclic = graph.clone();
    acyclic.edges.retain(|edge| {
        !(edge.from == "a4" && edge.to == "a0" || edge.from == "b4" && edge.to == "b0")
    });
    let mut self_loop = graph.clone();
    self_loop.edges.clear();
    self_loop.add_edge("tail", "tail");
    let policy = AsciiRenderOptions::ascii()
        .flowchart_layout()
        .graph_policy();
    let mut resources = ResourceContext::new(AsciiResourcePolicy::unbounded());
    let initial = measured(&graph, &LayoutFeedback::default(), &mut resources);
    for (name, graph) in [
        ("diagram family", other_family),
        ("compound groups", grouped),
        ("authored side constraint", constrained),
        ("invisible rank constraint", invisible),
        ("acyclic graph", acyclic),
        ("self loop alone", self_loop),
    ] {
        let input = LayoutInput::prepare(&graph, &policy, &mut resources).unwrap();
        let mut feedback = LayoutFeedback::default();
        assert!(
            !feedback
                .record_component_ranks_with_input(&input, &initial, &resources)
                .unwrap(),
            "{name}"
        );
        assert!(feedback.component_ranks().is_none(), "{name}");
    }
}

#[test]
fn component_ranks_wait_until_existing_capacity_feedback_stops_growing() {
    let graph = component_rank_graph(GraphDirection::TopDown);
    let policy = AsciiRenderOptions::ascii()
        .flowchart_layout()
        .graph_policy();
    let mut resources = ResourceContext::new(AsciiResourcePolicy::unbounded());
    let mut initial = measured(&graph, &LayoutFeedback::default(), &mut resources);
    let input = LayoutInput::prepare(&graph, &policy, &mut resources).unwrap();
    initial.layout.nodes[5].x = initial.layout.nodes[0].x;
    initial.layout.nodes[5].y = initial.layout.nodes[0].bottom() + 1;
    let miss = AsciiError::RoutingSearchExhausted {
        diagram_type: "flowchart",
        edge_index: 10,
    };
    let mut feedback = LayoutFeedback::default();
    assert!(
        feedback
            .record_failure_with_input(&input, &initial, &miss, &resources)
            .unwrap()
    );
    assert_eq!(feedback.corridors.len(), 1);
    assert!(feedback.component_ranks().is_none());
    // Even the final recovery phase must honor an available capacity request first.
    let mut return_feedback = LayoutFeedback::default();
    assert!(
        return_feedback
            .record_return_failure_with_input(&input, &initial, &miss, &resources)
            .unwrap()
    );
    assert_eq!(return_feedback.corridors.len(), 1);
    assert!(return_feedback.component_ranks().is_none());
    assert!(
        feedback
            .record_return_failure_with_input(&input, &initial, &miss, &resources)
            .unwrap()
    );
    assert_eq!(
        feedback.component_ranks(),
        Some([0, 0, 0, 0, 0, 4, 4, 4, 4, 4, 8].as_slice())
    );
    assert_eq!(feedback.corridors.len(), 1);
    assert!(
        !feedback
            .record_return_failure_with_input(&input, &initial, &miss, &resources)
            .unwrap()
    );
}

#[test]
fn scoped_return_growth_precedes_component_rank_recovery_for_flat_cycles() {
    let graph = component_rank_graph(GraphDirection::TopDown);
    let policy = AsciiRenderOptions::ascii()
        .flowchart_layout()
        .graph_policy();
    let mut resources = ResourceContext::new(AsciiResourcePolicy::unbounded());
    let initial = measured(&graph, &LayoutFeedback::default(), &mut resources);
    let input = LayoutInput::prepare(&graph, &policy, &mut resources).unwrap();
    let miss = AsciiError::RoutingSearchExhausted {
        diagram_type: "flowchart",
        edge_index: 4,
    };
    let mut feedback = LayoutFeedback::default();
    feedback
        .record_failure_with_input(&input, &initial, &miss, &resources)
        .unwrap();
    assert!(
        feedback
            .record_return_failure_with_input(&input, &initial, &miss, &resources)
            .unwrap()
    );
    assert!(!feedback.returns.is_empty());
    assert!(feedback.component_ranks().is_none());
}

#[test]
fn component_rank_feedback_has_exact_work_admission_and_atomic_cancellation() {
    let graph = component_rank_graph(GraphDirection::TopDown);
    let policy = AsciiRenderOptions::ascii()
        .flowchart_layout()
        .graph_policy();
    let mut preparation = ResourceContext::new(AsciiResourcePolicy::unbounded());
    let initial = measured(&graph, &LayoutFeedback::default(), &mut preparation);
    let input = LayoutInput::prepare(&graph, &policy, &mut preparation).unwrap();
    let resources = ResourceContext::new(AsciiResourcePolicy::unbounded());
    assert!(
        LayoutFeedback::default()
            .record_component_ranks_with_input(&input, &initial, &resources)
            .unwrap()
    );
    let work = resources.layout_work_used();
    for (limit, admitted) in [(work, true), (work - 1, false)] {
        for private_allowance in [false, true] {
            let resources = if private_allowance {
                ResourceContext::new(AsciiResourcePolicy::unbounded())
                    .with_planning_work_allowance(limit)
            } else {
                ResourceContext::new(
                    AsciiResourcePolicy::unbounded()
                        .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, limit)
                        .unwrap(),
                )
            };
            let mut feedback = LayoutFeedback::default();
            let result = feedback.record_component_ranks_with_input(&input, &initial, &resources);
            assert_eq!(result.is_ok(), admitted);
            if admitted {
                assert!(result.unwrap());
                assert!(feedback.component_ranks().is_some());
                assert_eq!(resources.layout_work_used(), work);
            } else {
                if private_allowance {
                    assert!(matches!(
                        result,
                        Err(AsciiError::PlanningWorkAllowanceExhausted)
                    ));
                } else {
                    assert!(matches!(result, Err(AsciiError::ResourceLimitExceeded(_))));
                }
                assert!(feedback.component_ranks().is_none());
            }
            assert!(feedback.corridors.is_empty());
        }
    }
    for checkpoints in [0, 30, 300] {
        let control = OperationControl::new();
        if checkpoints > 0 {
            control.cancel_after_checkpoints(checkpoints);
        } else {
            control.cancel();
        }
        let resources = ResourceContext::new(AsciiResourcePolicy::unbounded())
            .controlled(control, OperationPhase::Layout);
        let mut feedback = LayoutFeedback::default();
        assert!(matches!(
            feedback.record_component_ranks_with_input(&input, &initial, &resources),
            Err(AsciiError::Cancelled(_))
        ));
        assert!(feedback.component_ranks().is_none());
        assert!(feedback.corridors.is_empty());
        assert_eq!(resources.layout_work_used() > 0, checkpoints > 0);
    }
}

#[test]
fn feedback_work_is_cumulative_and_cancellable() {
    let graph = capacity_graph(GraphNodeShape::ForkJoinHorizontal, GraphDirection::TopDown);
    let policy = AsciiRenderOptions::ascii()
        .flowchart_layout()
        .graph_policy();
    let mut resources = ResourceContext::new(AsciiResourcePolicy::unbounded());
    let initial = measured(&graph, &LayoutFeedback::default(), &mut resources);
    let before = resources.layout_work_used();
    let mut feedback = LayoutFeedback::default();
    feedback
        .record_failure(&graph, &policy, &initial, &capacity_miss(), &resources)
        .unwrap();
    assert!(resources.layout_work_used() > before);
    measured(&graph, &feedback, &mut resources);
    assert!(resources.layout_work_used() > before);
    for during in [false, true] {
        let control = OperationControl::new();
        if during {
            control.cancel_after_checkpoints(30);
        } else {
            control.cancel();
        }
        let controlled = ResourceContext::new(AsciiResourcePolicy::unbounded())
            .controlled(control, OperationPhase::Layout);
        assert!(matches!(
            LayoutFeedback::default().record_failure(
                &graph,
                &policy,
                &initial,
                &capacity_miss(),
                &controlled
            ),
            Err(AsciiError::Cancelled(_))
        ));
        assert_eq!(controlled.layout_work_used() > 0, during);
    }
}

#[test]
fn scope_analysis_accepts_exact_work_and_rejects_one_unit_less() {
    let mut graph = AsciiGraph::new(GraphDirection::TopDown);
    for id in ["a", "b", "c"] {
        graph.add_node(id, id);
    }
    for (from, to) in [("a", "b"), ("b", "c"), ("c", "a")] {
        graph.add_edge(from, to);
    }
    let resources = ResourceContext::new(AsciiResourcePolicy::unbounded());
    let scoped = ScopeGraph::try_new(&graph, &resources).unwrap();
    let (components, sizes) = scoped.components(&resources).unwrap();
    assert_eq!(sizes, [3]);
    assert!(components.windows(2).all(|pair| pair[0] == pair[1]));
    let work = resources.layout_work_used();
    for (limit, admitted) in [(work, true), (work - 1, false)] {
        let resources = ResourceContext::new(
            AsciiResourcePolicy::unbounded()
                .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, limit)
                .unwrap(),
        );
        let result = ScopeGraph::try_new(&graph, &resources)
            .and_then(|scoped| scoped.components(&resources));
        assert_eq!(result.is_ok(), admitted);
    }
    let control = OperationControl::new();
    control.cancel_after_checkpoints(8);
    let resources = ResourceContext::new(AsciiResourcePolicy::unbounded())
        .controlled(control, OperationPhase::Layout);
    assert!(matches!(
        ScopeGraph::try_new(&graph, &resources).and_then(|scoped| scoped.components(&resources)),
        Err(AsciiError::Cancelled(_))
    ));
}
