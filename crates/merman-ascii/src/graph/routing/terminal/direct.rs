use super::{DIRECTIONS, TerminalPort, terminal_ports};
use crate::error::Result;
use crate::graph::layout::CanvasCoord;
use crate::graph::model::GraphGroupKind;
use crate::graph::routing::geometry::{CompoundRouteGeometry, FrameSegment};
use crate::graph::routing::label::RoutedLabelDescriptor;
use crate::graph::routing::occupancy::RouteCandidateScore;
use crate::graph::routing::path::{RouteObstacles, StepDirection};
use crate::graph::routing::plan::orthogonal::{plan_terminal_path, terminal_label_hosts};
use crate::graph::routing::plan::{EdgeRouteRequest, EndpointGeometry, NodeAttachments, RoutePlan};
use crate::graph::routing::{admit_route_candidate, layout_allocation_failed};
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
    let from =
        EndpointGeometry::for_route(request.from, request.to, request.graph_layout, resources)?;
    let to =
        EndpointGeometry::for_route(request.to, request.from, request.graph_layout, resources)?;
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
    let mut best: Option<(RouteCandidateScore, RoutePlan)> = None;
    for start in starts.into_iter().flatten() {
        for end in ends.into_iter().flatten() {
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
            ] {
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
                    if groups.segment(from, to, resources, execution)? == FrameSegment::Blocked {
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
                    if best
                        .as_ref()
                        .is_none_or(|(incumbent, _)| score < *incumbent)
                    {
                        best = Some((score, plan));
                    }
                }
            }
        }
    }
    Ok(best)
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

fn direct_path(
    points: [CanvasCoord; 4],
    departure: StepDirection,
    arrival: StepDirection,
    resources: &ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<Option<Vec<CanvasCoord>>> {
    let start = points[0];
    let mut length = 1;
    let mut first = None;
    let mut last = None;
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
        let span = resources.checked_work_add(from.x.abs_diff(to.x), from.y.abs_diff(to.y))?;
        length = resources.checked_work_add(length, span)?;
    }
    if first.is_some_and(|heading| heading != departure)
        || last.is_some_and(|heading| heading != arrival)
        || (first.is_none() && departure != arrival)
    {
        return Ok(None);
    }
    if points[0] != points[1]
        && points[2] != points[3]
        && points[0].x.min(points[1].x) <= points[2].x.max(points[3].x)
        && points[2].x.min(points[3].x) <= points[0].x.max(points[1].x)
        && points[0].y.min(points[1].y) <= points[2].y.max(points[3].y)
        && points[2].y.min(points[3].y) <= points[0].y.max(points[1].y)
    {
        return Ok(None);
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
