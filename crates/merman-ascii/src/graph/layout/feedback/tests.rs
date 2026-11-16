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
    for direction in [
        GraphDirection::LeftRight,
        GraphDirection::RightLeft,
        GraphDirection::TopDown,
        GraphDirection::BottomTop,
    ] {
        let mut graph = AsciiGraph::new(direction);
        for id in ["a", "b", "c", "outside"] {
            graph.add_node(id, id);
        }
        for (from, to) in [("a", "b"), ("b", "c"), ("c", "a"), ("c", "a")] {
            graph.add_edge(from, to);
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
        let horizontal = direction.canonical() == GraphDirection::LeftRight;
        // Independently prescribed intervals: both c -> a returns cover [10,50].
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
        assert_eq!(request.cells, 8);
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
            assert!(margin >= 8);
        }
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
