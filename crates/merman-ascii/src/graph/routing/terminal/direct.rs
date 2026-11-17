use super::{DIRECTIONS, TerminalPort, terminal_ports, unit_neighbor};
use crate::error::Result;
use crate::graph::layout::CanvasCoord;
use crate::graph::model::{GraphEdgeMarker, GraphGroupKind};
use crate::graph::routing::geometry::{CompoundRouteGeometry, FrameSegment};
use crate::graph::routing::label::RoutedLabelDescriptor;
use crate::graph::routing::occupancy::RouteCandidateScore;
use crate::graph::routing::path::{RouteObstacles, StepDirection};
use crate::graph::routing::plan::orthogonal::{plan_terminal_path, terminal_label_hosts};
use crate::graph::routing::plan::{
    EdgeRouteRequest, EndpointGeometry, MarkerEndpoint, NodeAttachments, RoutePlan,
};
use crate::graph::routing::{
    admit_route_candidate, forward_route_direction, layout_allocation_failed,
    route_candidate_is_better,
};
use crate::operation::AsciiExecution;
use crate::resource::ResourceContext;
use merman_model::OperationPhase;

/// Compare a bounded set of direct and exterior channels at actual node contacts.
pub(in crate::graph::routing) fn plan_direct_route(
    request: EdgeRouteRequest<'_>,
    obstacles: RouteObstacles<'_>,
    label: Option<RoutedLabelDescriptor>,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<Option<(RouteCandidateScore, RoutePlan)>> {
    execution.checkpoint(OperationPhase::Layout)?;
    for group in &request.graph_layout.groups {
        execution.checkpoint(OperationPhase::Layout)?;
        resources.charge_layout_work(1)?;
        if (group.id == request.from.id || group.id == request.to.id)
            && group.kind != GraphGroupKind::Container
        {
            return Ok(None);
        }
    }
    let from = EndpointGeometry::for_request(request, MarkerEndpoint::Start, resources)?;
    let to = EndpointGeometry::for_request(request, MarkerEndpoint::End, resources)?;
    let starts = central_ports(from, request, resources, execution)?;
    let ends = central_ports(to, request, resources, execution)?;
    let mut extent = obstacles.scene.occupied_extent();
    let mut first = CanvasCoord {
        x: request.from.x.min(request.to.x),
        y: request.from.y.min(request.to.y),
    };
    resources.charge_layout_work(request.graph_layout.nodes.len())?;
    for node in &request.graph_layout.nodes {
        resources.checkpoint()?;
        first.x = first.x.min(node.x);
        first.y = first.y.min(node.y);
        extent.x = extent.x.max(node.right());
        extent.y = extent.y.max(node.bottom());
    }
    resources.charge_layout_work(request.graph_layout.groups.len())?;
    for group in &request.graph_layout.groups {
        resources.checkpoint()?;
        first.x = first.x.min(group.x);
        first.y = first.y.min(group.y);
        extent.x = extent.x.max(group.right());
        extent.y = extent.y.max(group.bottom());
    }
    let right = resources.checked_grid_add(extent.x, 2)?;
    let bottom = resources.checked_grid_add(extent.y, 2)?;
    let forward = forward_route_direction(request, resources)?;
    let preferred_starts = preferred_ports(
        from, request, obstacles, forward, true, resources, execution,
    )?;
    let preferred_ends =
        preferred_ports(to, request, obstacles, forward, false, resources, execution)?;
    let mut best: Option<(RouteCandidateScore, RoutePlan)> = None;
    for (preferred, starts, ends) in [
        (true, preferred_starts.as_slice(), preferred_ends.as_slice()),
        (false, starts.as_slice(), ends.as_slice()),
    ] {
        for start in starts.iter().copied().flatten() {
            for end in ends.iter().copied().flatten() {
                execution.checkpoint(OperationPhase::Layout)?;
                resources.charge_layout_work(1)?;
                if request.from.id == request.to.id
                    && start.attachment.contact == end.attachment.contact
                {
                    continue;
                }
                let attachments = NodeAttachments {
                    start: start.attachment,
                    end: end.attachment,
                };
                let groups = CompoundRouteGeometry {
                    layout: request.graph_layout,
                    from: request.from,
                    to: request.to,
                    attachments: Some(attachments),
                };
                let (s, e) = (start.berth, end.berth);
                let middle_x = resources.checked_grid_add(s.x.min(e.x), s.x.abs_diff(e.x) / 2)?;
                let middle_y = resources.checked_grid_add(s.y.min(e.y), s.y.abs_diff(e.y) / 2)?;
                let bounds = CanvasCoord {
                    x: right,
                    y: bottom,
                };
                let local_channels = if preferred {
                    // Try turns just beyond the endpoint reading margins as well as the
                    // midpoint. A preceding branch can occupy that midpoint channel.
                    [
                        unit_neighbor(s, start.attachment.outward, bounds),
                        unit_neighbor(e, end.attachment.outward, bounds),
                    ]
                    .map(|turn| {
                        turn.map(|turn| match start.attachment.outward {
                            StepDirection::Left | StepDirection::Right => [
                                s,
                                CanvasCoord { x: turn.x, y: s.y },
                                CanvasCoord { x: turn.x, y: e.y },
                                e,
                            ],
                            StepDirection::Up | StepDirection::Down => [
                                s,
                                CanvasCoord { x: s.x, y: turn.y },
                                CanvasCoord { x: e.x, y: turn.y },
                                e,
                            ],
                        })
                    })
                } else {
                    [None; 2]
                };
                for points in [
                    [s, CanvasCoord { x: e.x, y: s.y }, e, e],
                    [s, CanvasCoord { x: s.x, y: e.y }, e, e],
                    [
                        s,
                        CanvasCoord {
                            x: middle_x,
                            y: s.y,
                        },
                        CanvasCoord {
                            x: middle_x,
                            y: e.y,
                        },
                        e,
                    ],
                    [
                        s,
                        CanvasCoord {
                            x: s.x,
                            y: middle_y,
                        },
                        CanvasCoord {
                            x: e.x,
                            y: middle_y,
                        },
                        e,
                    ],
                    [
                        s,
                        CanvasCoord { x: right, y: s.y },
                        CanvasCoord { x: right, y: e.y },
                        e,
                    ],
                    [
                        s,
                        CanvasCoord { x: s.x, y: bottom },
                        CanvasCoord { x: e.x, y: bottom },
                        e,
                    ],
                    [
                        s,
                        CanvasCoord {
                            x: first.x.saturating_sub(2),
                            y: s.y,
                        },
                        CanvasCoord {
                            x: first.x.saturating_sub(2),
                            y: e.y,
                        },
                        e,
                    ],
                    [
                        s,
                        CanvasCoord {
                            x: s.x,
                            y: first.y.saturating_sub(2),
                        },
                        CanvasCoord {
                            x: e.x,
                            y: first.y.saturating_sub(2),
                        },
                        e,
                    ],
                ]
                .into_iter()
                .map(Some)
                .chain(local_channels)
                .flatten()
                {
                    execution.checkpoint(OperationPhase::Layout)?;
                    resources.charge_layout_work(1)?;
                    let mut clear = true;
                    for (from, to) in [
                        (start.attachment.contact, s),
                        (points[0], points[1]),
                        (points[1], points[2]),
                        (points[2], points[3]),
                        (e, end.attachment.contact),
                    ] {
                        if groups.segment(from, to, resources, execution)? == FrameSegment::Blocked
                        {
                            clear = false;
                            break;
                        }
                    }
                    if !clear {
                        continue;
                    }
                    let Some(path) = direct_path(
                        points,
                        start.attachment.outward,
                        end.attachment.outward.opposite(),
                        resources,
                        execution,
                    )?
                    else {
                        continue;
                    };
                    for host in terminal_label_hosts(&path, label, resources)? {
                        execution.checkpoint(OperationPhase::Layout)?;
                        let Some(plan) = plan_terminal_path(
                            &path,
                            attachments,
                            request.edge,
                            host,
                            request.charset,
                            resources,
                        )?
                        else {
                            continue;
                        };
                        let Some((score, plan)) =
                            admit_route_candidate(request, obstacles, plan, resources)?
                        else {
                            continue;
                        };
                        // Preserve enumeration order on ties without retaining a candidate list.
                        if best.as_ref().is_none_or(|(incumbent, incumbent_plan)| {
                            route_candidate_is_better(
                                forward,
                                (score, plan.attachments),
                                (*incumbent, incumbent_plan.attachments),
                            )
                        }) {
                            best = Some((score, plan));
                        }
                    }
                }
                // Keep an ordinary clear central route. Only blocked/congested preferred
                // contacts need another pair; flat routing retains its original full comparison.
                if preferred
                    && best
                        .as_ref()
                        .is_some_and(|(score, _)| !score.has_congestion())
                {
                    return Ok(best);
                }
            }
        }
    }
    Ok(best)
}

// At most nine preferred contact pairs, only in compound forward scenes. Existing full
// contour recovery remains responsible for constraints beyond these inexpensive candidates.
#[allow(clippy::too_many_arguments)]
fn preferred_ports(
    geometry: EndpointGeometry<'_>,
    request: EdgeRouteRequest<'_>,
    obstacles: RouteObstacles<'_>,
    forward: Option<StepDirection>,
    source: bool,
    resources: &ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<[Option<TerminalPort>; 3]> {
    let mut selected = [None; 3];
    let Some(forward) = forward else {
        return Ok(selected);
    };
    let outward = if source { forward } else { forward.opposite() };
    let mut count = 0;
    for port in terminal_ports(geometry, request, resources, execution)? {
        execution.checkpoint(OperationPhase::Layout)?;
        resources.charge_layout_work(1)?;
        if port.attachment.outward != outward {
            continue;
        }
        let usage = if !source {
            obstacles.scene.target_port_usage(port.attachment)
        } else if request.edge.start_marker != GraphEdgeMarker::Open {
            obstacles.scene.source_port_usage(port.attachment)
        } else {
            0
        };
        if usage == 0 {
            selected[count] = Some(port);
            count += 1;
            if count == selected.len() {
                break;
            }
        }
    }
    Ok(selected)
}

fn central_ports(
    geometry: EndpointGeometry<'_>,
    request: EdgeRouteRequest<'_>,
    resources: &ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<[Option<TerminalPort>; 4]> {
    let mut central = [None; 4];
    for port in terminal_ports(geometry, request, resources, execution)? {
        execution.checkpoint(OperationPhase::Layout)?;
        resources.charge_layout_work(1)?;
        for (slot, outward) in central.iter_mut().zip(DIRECTIONS) {
            if port.attachment.outward == outward {
                slot.get_or_insert(port);
            }
        }
    }
    Ok(central)
}

pub(in crate::graph::routing) fn direct_path<const N: usize>(
    points: [CanvasCoord; N],
    departure: StepDirection,
    arrival: StepDirection,
    resources: &ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<Option<Vec<CanvasCoord>>> {
    // Direct channels have at most three bends. Keep validation and scratch storage bounded
    // even if another internal caller instantiates this helper with a larger array.
    if N == 0 || N > 5 {
        return Ok(None);
    }
    let start = points[0];
    let mut length = 1;
    let mut first = None;
    let mut last = None;
    let mut segments = [(start, start); 4];
    let mut segment_count = 0;
    for pair in points.windows(2) {
        let (from, to) = (pair[0], pair[1]);
        if from == to {
            continue;
        }
        if from.x != to.x && from.y != to.y {
            return Ok(None);
        }
        let heading = if from.x < to.x {
            StepDirection::Right
        } else if from.x > to.x {
            StepDirection::Left
        } else if from.y < to.y {
            StepDirection::Down
        } else {
            StepDirection::Up
        };
        if last.is_some_and(|previous: StepDirection| previous.opposite() == heading) {
            return Ok(None);
        }
        first.get_or_insert(heading);
        last = Some(heading);
        segments[segment_count] = (from, to);
        segment_count += 1;
        let span = resources.checked_work_add(from.x.abs_diff(to.x), from.y.abs_diff(to.y))?;
        length = resources.checked_work_add(length, span)?;
    }
    if first.is_some_and(|heading| heading != departure)
        || last.is_some_and(|heading| heading != arrival)
        || (first.is_none() && departure != arrival)
    {
        return Ok(None);
    }
    // Collapse empty segments before checking adjacency: [a, b, b, c] still has two
    // adjacent segments, whereas a fourth nonempty segment can cross the first one.
    let segments = &segments[..segment_count];
    for (first, &(a, b)) in segments.iter().enumerate() {
        for &(c, d) in segments.iter().skip(first + 2) {
            if a.x.min(b.x) <= c.x.max(d.x)
                && c.x.min(d.x) <= a.x.max(b.x)
                && a.y.min(b.y) <= c.y.max(d.y)
                && c.y.min(d.y) <= a.y.max(b.y)
            {
                return Ok(None);
            }
        }
    }
    execution.checkpoint(OperationPhase::Layout)?;
    resources.grid_extent(length, 1)?;
    resources.charge_layout_work(length)?;
    let mut path = Vec::new();
    path.try_reserve_exact(length)
        .map_err(|_| layout_allocation_failed())?;
    path.push(start);
    for pair in points.windows(2) {
        let mut cursor = pair[0];
        let to = pair[1];
        while cursor != to {
            execution.checkpoint(OperationPhase::Layout)?;
            // Checked axis-aligned spans bound these unit steps.
            if cursor.x < to.x {
                cursor.x += 1;
            } else if cursor.x > to.x {
                cursor.x -= 1;
            } else if cursor.y < to.y {
                cursor.y += 1;
            } else {
                cursor.y -= 1;
            }
            path.push(cursor);
        }
    }
    Ok(Some(path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AsciiRenderOptions;
    use crate::graph::charset::GraphCharset;
    use crate::graph::layout::layout_graph;
    use crate::graph::model::{AsciiGraph, GraphDirection, GraphNodeShape};
    use crate::graph::routing::RouteOwner;
    use crate::graph::routing::occupancy::SceneOccupancy;
    use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy};

    #[test]
    fn five_point_channels_reject_crossings_and_keep_distinct_terminal_berths() {
        let policy = AsciiResourcePolicy::unbounded();
        let resources = ResourceContext::new(policy);
        let execution = AsciiExecution::for_test(&policy);
        let points = [
            CanvasCoord { x: 3, y: 2 },
            CanvasCoord { x: 9, y: 2 },
            CanvasCoord { x: 9, y: 8 },
            CanvasCoord { x: 5, y: 8 },
            CanvasCoord { x: 5, y: 4 },
        ];
        let path = direct_path(
            points,
            StepDirection::Right,
            StepDirection::Up,
            &resources,
            execution,
        )
        .unwrap()
        .unwrap();
        assert_eq!(path.first(), Some(&points[0]));
        assert_eq!(path.last(), Some(&points[4]));
        assert!(
            path.windows(2)
                .all(|pair| { pair[0].x.abs_diff(pair[1].x) + pair[0].y.abs_diff(pair[1].y) == 1 })
        );
        for end in [CanvasCoord { x: 5, y: 1 }, CanvasCoord { x: 5, y: 2 }] {
            assert!(
                direct_path(
                    [points[0], points[1], points[2], points[3], end],
                    StepDirection::Right,
                    StepDirection::Up,
                    &resources,
                    execution,
                )
                .unwrap()
                .is_none()
            );
        }
        assert!(
            direct_path(
                [points[0]; 6],
                StepDirection::Right,
                StepDirection::Right,
                &resources,
                execution
            )
            .unwrap()
            .is_none()
        );
        assert!(
            direct_path(
                [],
                StepDirection::Right,
                StepDirection::Right,
                &resources,
                execution
            )
            .unwrap()
            .is_none()
        );
    }

    #[test]
    fn collapsed_middle_channel_retains_a_continuous_straight_path() {
        let policy = AsciiResourcePolicy::unbounded();
        let resources = ResourceContext::new(policy);
        let path = direct_path(
            [
                CanvasCoord { x: 3, y: 8 },
                CanvasCoord { x: 5, y: 8 },
                CanvasCoord { x: 5, y: 8 },
                CanvasCoord { x: 10, y: 8 },
            ],
            StepDirection::Right,
            StepDirection::Right,
            &resources,
            AsciiExecution::for_test(&policy),
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            path,
            (3..=10)
                .map(|x| CanvasCoord { x, y: 8 })
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn direct_routes_use_actual_central_shape_contacts_deterministically() {
        for shape in [GraphNodeShape::Rect, GraphNodeShape::Diamond] {
            for transpose in [false, true] {
                let policy = AsciiResourcePolicy::unbounded();
                let execution = AsciiExecution::for_test(&policy);
                let mut resources = ResourceContext::new(policy);
                let options = AsciiRenderOptions::ascii();
                let charset = GraphCharset::for_options(&options);
                let mut graph = AsciiGraph::new(GraphDirection::TopDown);
                graph.add_node("a", "A");
                graph.add_node("b", "B");
                graph.add_edge("a", "b");
                let mut layout = layout_graph(&graph, &options);
                for (node, (x, y)) in layout.nodes.iter_mut().zip([(8, 8), (25, 22)]) {
                    node.x = if transpose { y } else { x };
                    node.y = if transpose { x } else { y };
                    node.width = if transpose { 5 } else { 9 };
                    node.height = if transpose { 9 } else { 5 };
                    node.shape = shape;
                }
                let occupancy =
                    SceneOccupancy::try_new(&[], &layout, &mut resources, "flowchart").unwrap();
                let owner = RouteOwner::for_test(0, "a", "b");
                let request = EdgeRouteRequest {
                    graph: &graph,
                    graph_layout: &layout,
                    edges: &graph.edges,
                    from: &layout.nodes[0],
                    to: &layout.nodes[1],
                    edge_index: 0,
                    edge: &graph.edges[0],
                    charset: &charset,
                };
                let obstacles = RouteObstacles {
                    scene: &occupancy,
                    routes: &[],
                    owner: &owner,
                };
                let first = plan_direct_route(request, obstacles, None, &mut resources, execution)
                    .unwrap()
                    .expect("central terminal routes must not depend on coarse projection");
                let again = plan_direct_route(request, obstacles, None, &mut resources, execution)
                    .unwrap()
                    .unwrap();
                assert_eq!(first, again);
                let attachments = first.1.attachments.unwrap();
                for (attachment, node) in [
                    (attachments.start, request.from),
                    (attachments.end, request.to),
                ] {
                    match attachment.outward {
                        StepDirection::Left | StepDirection::Right => {
                            assert_eq!(attachment.contact.y, node.center_y());
                        }
                        StepDirection::Up | StepDirection::Down => {
                            assert_eq!(attachment.contact.x, node.center_x());
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn direct_path_rejects_reversals_and_charges_before_expansion() {
        let policy = AsciiResourcePolicy::unbounded();
        let execution = AsciiExecution::for_test(&policy);
        let resources = ResourceContext::new(policy);
        let start = CanvasCoord { x: 2, y: 2 };
        let turn = CanvasCoord { x: 8, y: 2 };
        let end = CanvasCoord { x: 8, y: 9 };
        for (points, departure, arrival) in [
            (
                [start, turn, end, end],
                StepDirection::Left,
                StepDirection::Down,
            ),
            (
                [start, turn, end, end],
                StepDirection::Right,
                StepDirection::Up,
            ),
            (
                [start, turn, start, end],
                StepDirection::Right,
                StepDirection::Down,
            ),
        ] {
            assert!(
                direct_path(points, departure, arrival, &resources, execution)
                    .unwrap()
                    .is_none()
            );
        }
        let limited = ResourceContext::new(
            policy
                .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 13)
                .unwrap(),
        );
        let error = direct_path(
            [start, turn, end, end],
            StepDirection::Right,
            StepDirection::Down,
            &limited,
            execution,
        )
        .unwrap_err();
        assert!(
            matches!(error, crate::error::AsciiError::ResourceLimitExceeded(details)
            if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits && details.actual == 14)
        );
    }
}
