use super::*;
use crate::AsciiRenderOptions;
use crate::error::AsciiError;
use crate::graph::charset::GraphCharset;
use crate::graph::layout::layout_graph;
use crate::graph::model::{AsciiGraph, GraphDirection, GraphNodeShape};
use crate::graph::routing::occupancy::SceneOccupancy;
use crate::graph::routing::plan::{MarkerEndpoint, NodeAttachments};
use crate::graph::routing::{PreparedRoute, RouteOwner};
use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy};
use merman_model::{CancelReason, OperationControl, OperationPhase};

fn node_ports(
    width: usize,
    height: usize,
    shape: GraphNodeShape,
    resources: &ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<Vec<TerminalPort>> {
    let options = AsciiRenderOptions::ascii();
    let charset = GraphCharset::for_options(&options);
    let mut graph = AsciiGraph::new(GraphDirection::TopDown);
    graph.add_node("a", "A");
    graph.add_node("b", "B");
    graph.add_edge("a", "b");
    let mut layout = layout_graph(&graph, &options);
    layout.nodes[0].x = 8;
    layout.nodes[0].y = 8;
    layout.nodes[0].width = width;
    layout.nodes[0].height = height;
    layout.nodes[0].shape = shape;
    terminal_ports(
        EndpointGeometry {
            layout: &layout.nodes[0],
            interior: false,
            group: false,
        },
        EdgeRouteRequest {
            graph: &graph,
            graph_layout: &layout,
            edges: &graph.edges,
            from: &layout.nodes[0],
            to: &layout.nodes[1],
            edge_index: 0,
            edge: &graph.edges[0],
            charset: &charset,
        },
        resources,
        execution,
    )
}

#[test]
fn rectangular_ports_cover_each_noncorner_contact_in_center_out_order() {
    let policy = AsciiResourcePolicy::unbounded();
    let execution = AsciiExecution::for_test(&policy);
    for (width, height) in [(13, 5), (5, 13), (12, 6), (6, 12)] {
        let resources = ResourceContext::new(policy);
        let ports = node_ports(width, height, GraphNodeShape::Rect, &resources, execution).unwrap();
        assert_eq!(ports.len(), 2 * (width - 2 + height - 2));
        for outward in DIRECTIONS {
            let side: Vec<_> = ports
                .iter()
                .filter(|port| port.attachment.outward == outward)
                .collect();
            let (length, first) = match outward {
                StepDirection::Left | StepDirection::Right => (height, 8),
                StepDirection::Up | StepDirection::Down => (width, 8),
            };
            let center = first + length / 2;
            let mut expected: Vec<_> = (first + 1..first + length - 1).collect();
            expected.sort_unstable_by_key(|intercept| (intercept.abs_diff(center), *intercept));
            assert_eq!(
                side.iter()
                    .map(|port| port_intercept(**port))
                    .collect::<Vec<_>>(),
                expected
            );
            for pair in side.windows(2) {
                assert_ne!(pair[0].attachment, pair[1].attachment);
            }
        }
    }
}

#[test]
fn full_port_enumeration_keeps_diamond_contact_constraints() {
    let policy = AsciiResourcePolicy::unbounded();
    let execution = AsciiExecution::for_test(&policy);
    let resources = ResourceContext::new(policy);
    let ports = node_ports(13, 7, GraphNodeShape::Diamond, &resources, execution).unwrap();
    assert_eq!(ports.len(), 4);
    for outward in DIRECTIONS {
        assert_eq!(
            ports
                .iter()
                .filter(|port| port.attachment.outward == outward)
                .count(),
            1
        );
    }
}

#[test]
fn repair_uses_a_farther_contact_after_the_three_center_ports_are_occupied() {
    for transpose in [false, true] {
        let policy = AsciiResourcePolicy::unbounded();
        let execution = AsciiExecution::for_test(&policy);
        let mut resources = ResourceContext::new(policy);
        let options = AsciiRenderOptions::ascii();
        let charset = GraphCharset::for_options(&options);
        let mut graph = AsciiGraph::new(GraphDirection::TopDown);
        graph.add_node("a", "A");
        graph.add_node("b", "B");
        for _ in 0..4 {
            graph.add_edge("a", "b");
        }
        let coord = |x, y| {
            if transpose {
                CanvasCoord { x: y, y: x }
            } else {
                CanvasCoord { x, y }
            }
        };
        let mut layout = layout_graph(&graph, &options);
        for (node, y) in layout.nodes.iter_mut().zip([5, 20]) {
            let origin = coord(20, y);
            node.x = origin.x;
            node.y = origin.y;
            node.width = if transpose { 5 } else { 13 };
            node.height = if transpose { 13 } else { 5 };
        }
        let mut existing = Vec::new();
        for (index, x) in [26, 25, 27].into_iter().enumerate() {
            let path: Vec<_> = (10..20).map(|y| coord(x, y)).collect();
            let start = NodeAttachment::resolve(
                &layout.nodes[0],
                path[0],
                if transpose {
                    StepDirection::Left
                } else {
                    StepDirection::Up
                },
                &charset,
            )
            .unwrap();
            let end = NodeAttachment::resolve(
                &layout.nodes[1],
                *path.last().unwrap(),
                if transpose {
                    StepDirection::Right
                } else {
                    StepDirection::Down
                },
                &charset,
            )
            .unwrap();
            let mut plan = super::super::plan::orthogonal::plan_terminal_path(
                &path,
                NodeAttachments { start, end },
                &graph.edges[index],
                None,
                &charset,
                &mut resources,
            )
            .unwrap()
            .unwrap()
            .with_marker_requests(GraphEdgeMarker::Open, GraphEdgeMarker::Point, "flowchart")
            .unwrap();
            plan.attachments = Some(NodeAttachments { start, end });
            existing.push(PreparedRoute::for_test_with_endpoints(
                plan, index, "a", "b",
            ));
        }
        let occupancy =
            SceneOccupancy::try_new(&existing, &layout, &mut resources, "flowchart").unwrap();
        let owner = RouteOwner::for_test(3, "a", "b");
        let (_, plan) = plan_terminal_route(
            EdgeRouteRequest {
                graph: &graph,
                graph_layout: &layout,
                edges: &graph.edges,
                from: &layout.nodes[0],
                to: &layout.nodes[1],
                edge_index: 3,
                edge: &graph.edges[3],
                charset: &charset,
            },
            RouteObstacles {
                scene: &occupancy,
                routes: &existing,
                owner: &owner,
            },
            None,
            None,
            &mut resources,
            execution,
        )
        .unwrap()
        .expect("the fourth route must use the remaining noncentral contour");
        let end = plan.attachments.unwrap().end;
        assert_eq!(end.contact, coord(24, 20));
        assert_eq!(occupancy.target_port_usage(end), 0);
        assert_eq!(
            plan.terminal_candidate(MarkerEndpoint::End, "flowchart")
                .unwrap()
                .coord,
            coord(24, 19)
        );
    }
}

#[test]
fn lazy_port_pairs_match_exhaustive_usage_distance_and_ordinal_order() {
    let policy = AsciiResourcePolicy::unbounded();
    let execution = AsciiExecution::for_test(&policy);
    let resources = ResourceContext::new(policy);
    let mut starts = node_ports(13, 7, GraphNodeShape::Rect, &resources, execution).unwrap();
    let mut ends = node_ports(17, 9, GraphNodeShape::Rect, &resources, execution).unwrap();
    for port in &mut starts {
        port.usage = port.ordinal % 3;
    }
    for port in &mut ends {
        port.usage = port.ordinal % 4;
    }
    let mut expected = Vec::new();
    for start in &starts {
        for end in &ends {
            expected.push((
                end.usage,
                start.usage,
                distance(start.berth, end.berth, &resources).unwrap(),
                start.ordinal,
                end.ordinal,
            ));
        }
    }
    expected.sort_unstable();
    let mut pairs = PortPairs::new(starts, ends, &resources).unwrap();
    for expected_pair in expected {
        let (start, end) = pairs
            .next(&resources, execution)
            .unwrap()
            .expect("every legal port combination must be generated");
        assert_eq!(
            (
                end.usage,
                start.usage,
                distance(start.berth, end.berth, &resources).unwrap(),
                start.ordinal,
                end.ordinal,
            ),
            expected_pair
        );
    }
    assert!(pairs.next(&resources, execution).unwrap().is_none());
}

#[test]
fn wide_port_search_initializes_a_linear_frontier() {
    let policy = AsciiResourcePolicy::unbounded();
    let execution = AsciiExecution::for_test(&policy);
    let resources = ResourceContext::new(policy);
    let starts = node_ports(1_001, 3, GraphNodeShape::Rect, &resources, execution).unwrap();
    let ends = node_ports(2_001, 3, GraphNodeShape::Rect, &resources, execution).unwrap();
    let start_count = starts.len();
    let product = start_count * ends.len();
    let before = resources.layout_work_used();
    let mut pairs = PortPairs::new(starts, ends, &resources).unwrap();
    assert!(pairs.next(&resources, execution).unwrap().is_some());
    assert!(pairs.frontier.len() <= 8 * start_count);
    assert!(resources.layout_work_used() - before < product / 16);
}

#[test]
fn port_enumeration_and_frontier_initialization_observe_cancellation() {
    let policy = AsciiResourcePolicy::unbounded();
    let resources = ResourceContext::new(policy);
    let control = OperationControl::new();
    control.cancel_after_checkpoints(5);
    let result = node_ports(
        1_001,
        3,
        GraphNodeShape::Rect,
        &resources,
        AsciiExecution::new(&control, &policy),
    );
    assert!(matches!(result, Err(AsciiError::Cancelled(cancelled))
        if cancelled.phase == OperationPhase::Layout && cancelled.reason == CancelReason::Requested));
    let starts = node_ports(
        13,
        7,
        GraphNodeShape::Rect,
        &resources,
        AsciiExecution::for_test(&policy),
    )
    .unwrap();
    let ends = starts.clone();
    let mut pairs = PortPairs::new(starts, ends, &resources).unwrap();
    let control = OperationControl::new();
    control.cancel_after_checkpoints(5);
    let result = pairs.next(&resources, AsciiExecution::new(&control, &policy));
    assert!(matches!(result, Err(AsciiError::Cancelled(cancelled))
        if cancelled.phase == OperationPhase::Layout && cancelled.reason == CancelReason::Requested));
}

#[test]
fn port_enumeration_charges_before_growing_a_wide_frontier() {
    let policy = AsciiResourcePolicy::unbounded();
    let resources = ResourceContext::new(
        AsciiResourcePolicy::unbounded()
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 3)
            .unwrap(),
    );
    let result = node_ports(
        1_001,
        3,
        GraphNodeShape::Rect,
        &resources,
        AsciiExecution::for_test(&policy),
    );
    assert!(
        matches!(result, Err(AsciiError::ResourceLimitExceeded(details))
        if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits && details.actual == 4)
    );
}
