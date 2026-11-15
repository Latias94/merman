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

fn channel_fixture(gap: usize) -> (AsciiGraph, crate::graph::layout::GraphLayout, GraphCharset) {
    let options = AsciiRenderOptions::ascii();
    let charset = GraphCharset::for_options(&options);
    let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
    graph.add_node("a", "A");
    graph.add_node("b", "B");
    graph.add_edge("a", "b");
    let mut layout = layout_graph(&graph, &options);
    for (node, x) in layout.nodes.iter_mut().zip([4, gap + 10]) {
        node.x = x;
        node.y = 8;
        node.width = 5;
        node.height = 5;
    }
    (graph, layout, charset)
}

fn channel_search<'a>(
    request: EdgeRouteRequest<'a>,
    obstacles: RouteObstacles<'a>,
    resources: &ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<(SearchGeometry<'a>, channels::ChannelAxes)> {
    let starts = terminal_ports(
        EndpointGeometry::for_route(request.from, request.to, request.graph_layout, resources)?,
        request,
        resources,
        execution,
    )?;
    let ends = terminal_ports(
        EndpointGeometry::for_route(request.to, request.from, request.graph_layout, resources)?,
        request,
        resources,
        execution,
    )?;
    let start = *starts
        .iter()
        .find(|port| {
            port.attachment.outward == StepDirection::Right
                && port.berth.y == request.from.center_y()
        })
        .unwrap();
    let end = *ends
        .iter()
        .find(|port| {
            port.attachment.outward == StepDirection::Left && port.berth.y == request.to.center_y()
        })
        .unwrap();
    let extent = obstacles.scene.occupied_extent();
    let bounds = CanvasCoord {
        x: resources.checked_grid_add(extent.x, ROUTING_GRID_MARGIN)?,
        y: resources.checked_grid_add(extent.y, ROUTING_GRID_MARGIN)?,
    };
    let axes = channels::ChannelAxes::new(obstacles, &starts, &ends, bounds, resources, execution)?;
    for port in starts.iter().chain(&ends) {
        for coord in [port.berth, port.attachment.contact] {
            assert!(axes.x.binary_search(&coord.x).is_ok());
            assert!(axes.y.binary_search(&coord.y).is_ok());
        }
    }
    assert_eq!(axes.x.first(), Some(&0));
    assert_eq!(axes.x.last(), Some(&bounds.x));
    assert_eq!(axes.y.first(), Some(&0));
    assert_eq!(axes.y.last(), Some(&bounds.y));
    Ok((
        SearchGeometry {
            start,
            end,
            bounds,
            obstacles,
            groups: CompoundRouteGeometry {
                layout: request.graph_layout,
                from: request.from,
                to: request.to,
                attachments: Some(NodeAttachments {
                    start: start.attachment,
                    end: end.attachment,
                }),
            },
        },
        axes,
    ))
}

fn channel_request<'a>(
    graph: &'a AsciiGraph,
    layout: &'a crate::graph::layout::GraphLayout,
    charset: &'a GraphCharset,
) -> EdgeRouteRequest<'a> {
    EdgeRouteRequest {
        graph,
        graph_layout: layout,
        edges: &graph.edges,
        from: &layout.nodes[0],
        to: &layout.nodes[1],
        edge_index: 0,
        edge: &graph.edges[0],
        charset,
    }
}

// A bounded test oracle exposes every integer coordinate to the same directional A*.
// Production axes contain scene events and ports exclusively.
fn unit_axes(bounds: CanvasCoord) -> channels::ChannelAxes {
    channels::ChannelAxes {
        x: (0..=bounds.x).collect(),
        y: (0..=bounds.y).collect(),
    }
}

fn path_cost(
    path: &[CanvasCoord],
    geometry: SearchGeometry<'_>,
    resources: &mut ResourceContext,
) -> Cost {
    assert_eq!(path.first(), Some(&geometry.start.berth));
    assert_eq!(path.last(), Some(&geometry.end.berth));
    let policy = resources.policy();
    let execution = AsciiExecution::for_test(&policy);
    let mut previous = geometry.start.attachment.outward;
    let mut cost = Cost {
        total: 0,
        shared: 0,
        length: 0,
        bends: 0,
    };
    for pair in path.windows(2) {
        assert_eq!(distance(pair[0], pair[1], resources).unwrap(), 1);
        let heading = if pair[1].x > pair[0].x {
            StepDirection::Right
        } else if pair[1].x < pair[0].x {
            StepDirection::Left
        } else if pair[1].y > pair[0].y {
            StepDirection::Down
        } else {
            StepDirection::Up
        };
        let segment = channels::segment_cost(
            geometry,
            Position {
                coord: pair[0],
                heading: previous,
            },
            pair[1],
            heading,
            resources,
            execution,
        )
        .unwrap()
        .expect("expanded path must retain every unit-cell and corner guard");
        cost.length += 1;
        cost.shared += segment.shared;
        cost.bends += usize::from(previous != heading);
        previous = heading;
    }
    assert_eq!(previous, geometry.end.attachment.outward.opposite());
    cost.total = cost.length + cost.shared;
    cost
}

#[test]
fn compressed_empty_gap_fits_the_repair_budget_without_unit_cell_states() {
    let policy = AsciiResourcePolicy::unbounded();
    let execution = AsciiExecution::for_test(&policy);
    let mut resources = ResourceContext::new(policy);
    let (graph, layout, charset) = channel_fixture(8_192);
    let scene = SceneOccupancy::try_new(&[], &layout, &mut resources, "flowchart").unwrap();
    let owner = RouteOwner::for_test(0, "a", "b");
    let (geometry, axes) = channel_search(
        channel_request(&graph, &layout, &charset),
        RouteObstacles {
            scene: &scene,
            routes: &[],
            owner: &owner,
        },
        &resources,
        execution,
    )
    .unwrap();
    assert!(axes.x.len() < 40 && axes.y.len() < 40);
    let mut remaining = MAX_REPAIR_EXPANSIONS;
    let path = find_path(geometry, &axes, &mut remaining, &mut resources, execution)
        .unwrap()
        .expect("empty distance must not consume one expansion per terminal cell");
    let expansions = MAX_REPAIR_EXPANSIONS - remaining;
    assert!(expansions < 32, "compressed expansions: {expansions}");
    assert_eq!(path.len(), 8_193);
    assert_eq!(path_cost(&path, geometry, &mut resources).length, 8_192);
    let mut exact = expansions;
    assert_eq!(
        find_path(geometry, &axes, &mut exact, &mut resources, execution).unwrap(),
        Some(path)
    );
    assert_eq!(exact, 0);
    let mut below = expansions - 1;
    assert!(
        find_path(geometry, &axes, &mut below, &mut resources, execution)
            .unwrap()
            .is_none()
    );
    let mut unit_remaining = MAX_REPAIR_EXPANSIONS;
    assert!(
        find_path(
            geometry,
            &unit_axes(geometry.bounds),
            &mut unit_remaining,
            &mut resources,
            execution
        )
        .unwrap()
        .is_none()
    );
    assert_eq!(unit_remaining, 0);
}

#[test]
fn critical_axes_do_not_scale_with_empty_scene_area() {
    let policy = AsciiResourcePolicy::unbounded();
    let execution = AsciiExecution::for_test(&policy);
    let mut resources = ResourceContext::new(policy);
    let (graph, mut layout, charset) = channel_fixture(1_000_000);
    layout.nodes[1].y = 2_000_000;
    let scene = SceneOccupancy::try_new(&[], &layout, &mut resources, "flowchart").unwrap();
    let owner = RouteOwner::for_test(0, "a", "b");
    let (_, axes) = channel_search(
        channel_request(&graph, &layout, &charset),
        RouteObstacles {
            scene: &scene,
            routes: &[],
            owner: &owner,
        },
        &resources,
        execution,
    )
    .unwrap();
    assert!(axes.x.len() < 40 && axes.y.len() < 40);
}

#[test]
fn compressed_obstacle_routes_match_bounded_unit_oracle_costs_and_repeat_deterministically() {
    let policy = AsciiResourcePolicy::unbounded();
    let execution = AsciiExecution::for_test(&policy);
    for x in [12, 16, 20] {
        for y in [5, 8, 11] {
            for (width, height) in [(1, 1), (3, 4)] {
                for offset in [0, 3] {
                    let mut resources = ResourceContext::new(policy);
                    let (graph, mut layout, charset) = channel_fixture(18);
                    layout.nodes[1].y += offset;
                    let mut obstacle = layout.nodes[0].clone();
                    obstacle.id = "obstacle".to_owned();
                    obstacle.x = x;
                    obstacle.y = y;
                    obstacle.width = width;
                    obstacle.height = height;
                    layout.nodes.push(obstacle);
                    let scene =
                        SceneOccupancy::try_new(&[], &layout, &mut resources, "flowchart").unwrap();
                    let owner = RouteOwner::for_test(0, "a", "b");
                    let (geometry, axes) = channel_search(
                        channel_request(&graph, &layout, &charset),
                        RouteObstacles {
                            scene: &scene,
                            routes: &[],
                            owner: &owner,
                        },
                        &resources,
                        execution,
                    )
                    .unwrap();
                    let mut compressed_budget = usize::MAX;
                    let compressed = find_path(
                        geometry,
                        &axes,
                        &mut compressed_budget,
                        &mut resources,
                        execution,
                    )
                    .unwrap();
                    let mut unit_budget = usize::MAX;
                    let unit = find_path(
                        geometry,
                        &unit_axes(geometry.bounds),
                        &mut unit_budget,
                        &mut resources,
                        execution,
                    )
                    .unwrap();
                    assert_eq!(
                        compressed.is_some(),
                        unit.is_some(),
                        "{x}, {y}, {width}, {height}, {offset}"
                    );
                    if let (Some(path), Some(unit)) = (compressed, unit) {
                        assert_eq!(
                            path_cost(&path, geometry, &mut resources),
                            path_cost(&unit, geometry, &mut resources),
                            "{x}, {y}, {width}, {height}, {offset}"
                        );
                        let mut repeat_budget = usize::MAX;
                        assert_eq!(
                            find_path(
                                geometry,
                                &axes,
                                &mut repeat_budget,
                                &mut resources,
                                execution
                            )
                            .unwrap(),
                            Some(path)
                        );
                    }
                }
            }
        }
    }
}

fn straight_occupant(start: CanvasCoord, end: CanvasCoord) -> PreparedRoute {
    use crate::color::AsciiColorRole;
    use crate::graph::model::GraphEdgeStroke;
    use crate::graph::routing::plan::{
        MarkerAnchor, MarkerAnchors, PlannedCellId, PlannedRouteCell, PlannedRouteCellKind,
        PlannedRoutePaint, PlannedRouteSegment,
    };
    let horizontal = start.y == end.y;
    let length = if horizontal {
        end.x - start.x
    } else {
        end.y - start.y
    };
    let cells = (0..=length)
        .map(|offset| PlannedRouteCell {
            coord: CanvasCoord {
                x: start.x + if horizontal { offset } else { 0 },
                y: start.y + if horizontal { 0 } else { offset },
            },
            ch: if horizontal { '-' } else { '|' },
            kind: PlannedRouteCellKind::EdgeLine,
            segment: PlannedRouteSegment::Direct,
            stroke: GraphEdgeStroke::Normal,
            directions: if horizontal {
                DIR_LEFT | DIR_RIGHT
            } else {
                DIR_UP | DIR_DOWN
            },
            unicode: false,
            paint: PlannedRoutePaint::role(AsciiColorRole::EdgeLine),
        })
        .collect();
    PreparedRoute::for_test_with_endpoints(
        RoutePlan::new(
            cells,
            Vec::new(),
            MarkerAnchors::new(
                MarkerAnchor::new(
                    PlannedCellId::new(0),
                    if horizontal {
                        StepDirection::Left
                    } else {
                        StepDirection::Up
                    },
                ),
                MarkerAnchor::new(
                    PlannedCellId::new(length),
                    if horizontal {
                        StepDirection::Right
                    } else {
                        StepDirection::Down
                    },
                ),
            ),
        ),
        1,
        "other-source",
        "other-target",
    )
}

#[test]
fn compressed_segments_inspect_interior_blockage_and_accumulate_crossing_cost() {
    let policy = AsciiResourcePolicy::unbounded();
    let execution = AsciiExecution::for_test(&policy);
    for blocked in [false, true] {
        let mut resources = ResourceContext::new(policy);
        let (graph, mut layout, charset) = channel_fixture(64);
        if blocked {
            let mut obstacle = layout.nodes[0].clone();
            obstacle.id = "block".to_owned();
            obstacle.x = 35;
            layout.nodes.push(obstacle);
        }
        let routes = [straight_occupant(
            CanvasCoord { x: 25, y: 3 },
            CanvasCoord { x: 25, y: 17 },
        )];
        let scene = SceneOccupancy::try_new(&routes, &layout, &mut resources, "flowchart").unwrap();
        let owner = RouteOwner::for_test(0, "a", "b");
        let (geometry, axes) = channel_search(
            channel_request(&graph, &layout, &charset),
            RouteObstacles {
                scene: &scene,
                routes: &routes,
                owner: &owner,
            },
            &resources,
            execution,
        )
        .unwrap();
        let segment = channels::segment_cost(
            geometry,
            Position {
                coord: geometry.start.berth,
                heading: StepDirection::Right,
            },
            geometry.end.berth,
            StepDirection::Right,
            &mut resources,
            execution,
        )
        .unwrap();
        if blocked {
            assert!(
                segment.is_none(),
                "clear segment endpoints cannot hide a blocked interior"
            );
        } else {
            let segment = segment.unwrap();
            assert_eq!(segment.length, 64);
            assert_eq!(segment.shared, 1);
            assert!(
                channels::segment_cost(
                    geometry,
                    Position {
                        coord: CanvasCoord { x: 25, y: 10 },
                        heading: StepDirection::Right
                    },
                    CanvasCoord { x: 25, y: 11 },
                    StepDirection::Down,
                    &mut resources,
                    execution
                )
                .unwrap()
                .is_none(),
                "a transverse crossing cannot become a shared corner"
            );
        }
        let mut compressed_budget = usize::MAX;
        let compressed = find_path(
            geometry,
            &axes,
            &mut compressed_budget,
            &mut resources,
            execution,
        )
        .unwrap()
        .unwrap();
        let mut oracle_budget = usize::MAX;
        let oracle = find_path(
            geometry,
            &unit_axes(geometry.bounds),
            &mut oracle_budget,
            &mut resources,
            execution,
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            path_cost(&compressed, geometry, &mut resources),
            path_cost(&oracle, geometry, &mut resources)
        );
    }
}

#[test]
fn compressed_search_preserves_materialized_markers_and_label_host_gaps() {
    use crate::graph::routing::label::{RoutedLabelPlacement, RoutedLabelText};
    use crate::graph::routing::plan::{LabelAnchor, PlannedRouteLabel};
    let policy = AsciiResourcePolicy::unbounded();
    let execution = AsciiExecution::for_test(&policy);
    for marker in [
        None,
        Some(GraphEdgeMarker::Point),
        Some(GraphEdgeMarker::Circle),
        Some(GraphEdgeMarker::Cross),
    ] {
        let mut resources = ResourceContext::new(policy);
        let (graph, layout, charset) = channel_fixture(40);
        let route = if let Some(marker) = marker {
            let mut route =
                straight_occupant(CanvasCoord { x: 25, y: 10 }, CanvasCoord { x: 25, y: 18 });
            route.plan = route
                .plan
                .with_markers(marker, GraphEdgeMarker::Open, &charset, "flowchart")
                .unwrap();
            route
        } else {
            let mut route =
                straight_occupant(CanvasCoord { x: 14, y: 13 }, CanvasCoord { x: 36, y: 13 });
            let mut label = PlannedRouteLabel::new(
                RoutedLabelText::new("label").unwrap(),
                RoutedLabelPlacement::new(23, 8, 5),
            );
            label.anchor = LabelAnchor::Segment {
                start: CanvasCoord { x: 14, y: 13 },
                end: CanvasCoord { x: 36, y: 13 },
                route_segment: None,
            };
            route.plan.labels.push(label);
            route
        };
        let routes = [route];
        let scene = SceneOccupancy::try_new(&routes, &layout, &mut resources, "flowchart").unwrap();
        // Replay installs finalized marker glyphs and the complete text-to-host reservation.
        let scene = scene
            .rebuild_for_routes(&routes, &[], &mut resources, "flowchart")
            .unwrap();
        let owner = RouteOwner::for_test(0, "a", "b");
        let (geometry, axes) = channel_search(
            channel_request(&graph, &layout, &charset),
            RouteObstacles {
                scene: &scene,
                routes: &routes,
                owner: &owner,
            },
            &resources,
            execution,
        )
        .unwrap();
        assert!(
            channels::segment_cost(
                geometry,
                Position {
                    coord: geometry.start.berth,
                    heading: StepDirection::Right
                },
                geometry.end.berth,
                StepDirection::Right,
                &mut resources,
                execution
            )
            .unwrap()
            .is_none()
        );
        let mut search_budget = usize::MAX;
        let path = find_path(
            geometry,
            &axes,
            &mut search_budget,
            &mut resources,
            execution,
        )
        .unwrap()
        .unwrap();
        assert!(!path.contains(&CanvasCoord { x: 25, y: 10 }));
        let mut oracle_budget = usize::MAX;
        let oracle = find_path(
            geometry,
            &unit_axes(geometry.bounds),
            &mut oracle_budget,
            &mut resources,
            execution,
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            path_cost(&path, geometry, &mut resources),
            path_cost(&oracle, geometry, &mut resources)
        );
    }
}

#[test]
fn compressed_segments_cross_owned_frame_sides_without_corners_or_turns_on_the_frame() {
    use crate::graph::label::GraphLabel;
    use crate::graph::layout::GroupLayout;
    use crate::graph::model::GraphGroupStyle;
    let policy = AsciiResourcePolicy::unbounded();
    let execution = AsciiExecution::for_test(&policy);
    let mut resources = ResourceContext::new(policy);
    let (graph, mut layout, charset) = channel_fixture(40);
    layout.groups.push(GroupLayout {
        id: "frame".to_owned(),
        kind: GraphGroupKind::Container,
        title: GraphLabel::new(""),
        style: GraphGroupStyle::default(),
        divider_span: None,
        x: 2,
        y: 4,
        width: 20,
        height: 16,
    });
    let scene = SceneOccupancy::try_new(&[], &layout, &mut resources, "flowchart").unwrap();
    let mut owner = RouteOwner::for_test(0, "a", "b");
    owner.boundary_group_indices.push(0);
    let (geometry, axes) = channel_search(
        channel_request(&graph, &layout, &charset),
        RouteObstacles {
            scene: &scene,
            routes: &[],
            owner: &owner,
        },
        &resources,
        execution,
    )
    .unwrap();
    assert!(
        channels::segment_cost(
            geometry,
            Position {
                coord: geometry.start.berth,
                heading: StepDirection::Right
            },
            geometry.end.berth,
            StepDirection::Right,
            &mut resources,
            execution
        )
        .unwrap()
        .is_some()
    );
    assert!(
        channels::segment_cost(
            geometry,
            Position {
                coord: CanvasCoord { x: 21, y: 10 },
                heading: StepDirection::Right
            },
            CanvasCoord { x: 21, y: 11 },
            StepDirection::Down,
            &mut resources,
            execution
        )
        .unwrap()
        .is_none()
    );
    assert!(
        channels::segment_cost(
            geometry,
            Position {
                coord: CanvasCoord { x: 20, y: 4 },
                heading: StepDirection::Right
            },
            CanvasCoord { x: 22, y: 4 },
            StepDirection::Right,
            &mut resources,
            execution
        )
        .unwrap()
        .is_none()
    );
    let mut search_budget = usize::MAX;
    let path = find_path(
        geometry,
        &axes,
        &mut search_budget,
        &mut resources,
        execution,
    )
    .unwrap()
    .unwrap();
    assert!(path.contains(&CanvasCoord { x: 21, y: 10 }));
    assert_eq!(path_cost(&path, geometry, &mut resources).bends, 0);
    let unowned = RouteOwner::for_test(0, "a", "b");
    let geometry = SearchGeometry {
        obstacles: RouteObstacles {
            owner: &unowned,
            ..geometry.obstacles
        },
        ..geometry
    };
    assert!(
        channels::segment_cost(
            geometry,
            Position {
                coord: geometry.start.berth,
                heading: StepDirection::Right
            },
            geometry.end.berth,
            StepDirection::Right,
            &mut resources,
            execution
        )
        .unwrap()
        .is_none()
    );
}

#[test]
fn compressed_search_and_materialization_share_exact_layout_work_limits() {
    let policy = AsciiResourcePolicy::unbounded();
    let execution = AsciiExecution::for_test(&policy);
    let mut setup = ResourceContext::new(policy);
    let (graph, layout, charset) = channel_fixture(256);
    let scene = SceneOccupancy::try_new(&[], &layout, &mut setup, "flowchart").unwrap();
    let owner = RouteOwner::for_test(0, "a", "b");
    let run = |resources: &mut ResourceContext| {
        let (geometry, axes) = channel_search(
            channel_request(&graph, &layout, &charset),
            RouteObstacles {
                scene: &scene,
                routes: &[],
                owner: &owner,
            },
            resources,
            execution,
        )?;
        let mut search_budget = usize::MAX;
        find_path(geometry, &axes, &mut search_budget, resources, execution)
    };
    let mut measured = ResourceContext::new(policy);
    let expected = run(&mut measured).unwrap().unwrap();
    let exact = measured.layout_work_used();
    let mut at_limit = ResourceContext::new(
        policy
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact)
            .unwrap(),
    );
    assert_eq!(run(&mut at_limit).unwrap(), Some(expected));
    assert_eq!(at_limit.layout_work_used(), exact);
    let mut below = ResourceContext::new(
        policy
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact - 1)
            .unwrap(),
    );
    assert!(
        matches!(run(&mut below), Err(AsciiError::ResourceLimitExceeded(details)) if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits && details.actual == exact)
    );
}

#[test]
fn compressed_axes_segment_scans_and_winner_expansion_observe_cancellation() {
    let policy = AsciiResourcePolicy::unbounded();
    let mut resources = ResourceContext::new(policy);
    let (graph, layout, charset) = channel_fixture(4_096);
    let scene = SceneOccupancy::try_new(&[], &layout, &mut resources, "flowchart").unwrap();
    let owner = RouteOwner::for_test(0, "a", "b");
    let obstacles = RouteObstacles {
        scene: &scene,
        routes: &[],
        owner: &owner,
    };
    let (geometry, _) = channel_search(
        channel_request(&graph, &layout, &charset),
        obstacles,
        &resources,
        AsciiExecution::for_test(&policy),
    )
    .unwrap();
    let assert_cancelled = |error: AsciiError| {
        assert!(
            matches!(error, AsciiError::Cancelled(cancelled) if cancelled.phase == OperationPhase::Layout && cancelled.reason == CancelReason::Requested)
        )
    };
    let control = OperationControl::new();
    control.cancel_after_checkpoints(5);
    let controlled = resources.controlled(control.clone(), OperationPhase::Layout);
    let result = channels::ChannelAxes::new(
        obstacles,
        &[geometry.start],
        &[geometry.end],
        geometry.bounds,
        &controlled,
        AsciiExecution::new(&control, &policy),
    );
    assert_cancelled(
        result
            .err()
            .expect("axis collection must stop on cancellation"),
    );
    let control = OperationControl::new();
    control.cancel_after_checkpoints(40);
    let result = channels::segment_cost(
        geometry,
        Position {
            coord: geometry.start.berth,
            heading: StepDirection::Right,
        },
        geometry.end.berth,
        StepDirection::Right,
        &mut resources,
        AsciiExecution::new(&control, &policy),
    );
    assert_cancelled(
        result
            .err()
            .expect("a long channel must poll between inspected cells"),
    );
    let initial = Position {
        coord: geometry.start.berth,
        heading: StepDirection::Right,
    };
    let goal = Position {
        coord: geometry.end.berth,
        heading: StepDirection::Right,
    };
    let parents = HashMap::from_iter([(goal, initial)]);
    let cost = Cost {
        total: 4_096,
        shared: 0,
        length: 4_096,
        bends: 0,
    };
    let control = OperationControl::new();
    control.cancel_after_checkpoints(40);
    assert_cancelled(
        reconstruct_path(
            OpenEntry {
                position: goal,
                cost,
                priority: cost,
                sequence: 0,
            },
            &parents,
            &resources,
            AsciiExecution::new(&control, &policy),
        )
        .unwrap_err(),
    );
}
