use super::{TerminalPort, direct_path, port_intercept, terminal_ports};
use crate::error::Result;
use crate::graph::label::try_measure_edge_label;
use crate::graph::layout::CanvasCoord;
use crate::graph::model::{GraphEdgeMarker, GraphGroupKind};
use crate::graph::routing::geometry::{CompoundRouteGeometry, FrameSegment};
use crate::graph::routing::label::{RoutedLabelDescriptor, routed_label_placement_for_descriptor};
use crate::graph::routing::layout_allocation_failed;
use crate::graph::routing::path::StepDirection;
use crate::graph::routing::plan::orthogonal::plan_terminal_path;
use crate::graph::routing::plan::{
    EdgeRouteRequest, EndpointGeometry, LabelAnchor, NodeAttachments, PlannedRouteLabel, RoutePlan,
};
use crate::operation::AsciiExecution;
use crate::resource::ResourceContext;
use merman_model::OperationPhase;

const LOOP_LANES: usize = 5;
const LOOP_CONTACTS: usize = 4;
pub(in crate::graph::routing) const MAX_SELF_LOOP_CANDIDATES: usize =
    LOOP_LANES * LOOP_CONTACTS * LOOP_CONTACTS;

/// Return nested right/down/left/up channels in canonical edge order. Admission, marker
/// allocation, and label reservations remain scene decisions made by the caller.
pub(in crate::graph::routing) fn plan_self_loop_candidates(
    request: EdgeRouteRequest<'_>,
    label: Option<RoutedLabelDescriptor>,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<Vec<RoutePlan>> {
    execution.checkpoint(OperationPhase::Layout)?;
    let mut candidates = Vec::new();
    if request.from.id != request.to.id || !request.edge.stroke.is_visible() {
        return Ok(candidates);
    }
    for group in &request.graph_layout.groups {
        execution.checkpoint(OperationPhase::Layout)?;
        resources.charge_layout_work(1)?;
        if group.id == request.from.id && group.kind != GraphGroupKind::Container {
            return Ok(candidates);
        }
    }
    // A container loop belongs to its exterior. Its own frame cannot be an interior
    // endpoint, although an enclosing parent must still contain the complete channel.
    let geometry = EndpointGeometry {
        interior: false,
        ..EndpointGeometry::for_route(request.from, request.to, request.graph_layout, resources)?
    };
    let (envelope, parallel_index) = loop_envelope(request, label, resources, execution)?;
    let ports = terminal_ports(geometry, request, resources, execution)?;
    let private_source = request.edge.start_marker != GraphEdgeMarker::Open;
    let start_y = if !private_source {
        request.from.center_y()
    } else {
        request.from.center_y().saturating_sub(parallel_index)
    };
    let starts = loop_ports(
        &ports,
        StepDirection::Right,
        start_y,
        private_source,
        resources,
        execution,
    )?;
    let ends = loop_ports(
        &ports,
        StepDirection::Down,
        resources.checked_grid_add(request.from.center_x(), parallel_index)?,
        false,
        resources,
        execution,
    )?;
    candidates
        .try_reserve_exact(MAX_SELF_LOOP_CANDIDATES)
        .map_err(|_| layout_allocation_failed())?;
    for lane in 0..LOOP_LANES {
        let offset = resources.checked_grid_mul(lane, 2)?;
        let right = resources.checked_grid_add(envelope.x, offset)?;
        let bottom = resources.checked_grid_add(envelope.y, offset)?;
        for start in starts.into_iter().flatten() {
            for end in ends.into_iter().flatten() {
                execution.checkpoint(OperationPhase::Layout)?;
                resources.charge_layout_work(1)?;
                if start.attachment.contact == end.attachment.contact
                    || start.berth.x >= right
                    || end.berth.y >= bottom
                    || end.berth.x >= right
                    || start.berth.y >= bottom
                {
                    continue;
                }
                let attachments = NodeAttachments {
                    start: start.attachment,
                    end: end.attachment,
                };
                let points = [
                    start.berth,
                    CanvasCoord {
                        x: right,
                        y: start.berth.y,
                    },
                    CanvasCoord {
                        x: right,
                        y: bottom,
                    },
                    CanvasCoord {
                        x: end.berth.x,
                        y: bottom,
                    },
                    end.berth,
                ];
                let groups = CompoundRouteGeometry {
                    layout: request.graph_layout,
                    from: request.from,
                    to: request.to,
                    attachments: Some(attachments),
                };
                let mut clear = true;
                for (from, to) in std::iter::once((start.attachment.contact, start.berth))
                    .chain(points.windows(2).map(|pair| (pair[0], pair[1])))
                    .chain(std::iter::once((end.berth, end.attachment.contact)))
                {
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
                    StepDirection::Right,
                    StepDirection::Up,
                    resources,
                    execution,
                )?
                else {
                    continue;
                };
                // Only the bottom horizontal segment is private to this loop. Do not put
                // its label on the departure stroke shared by nested parallel loops.
                let host = if let Some(descriptor) = label {
                    if right - end.berth.x < resources.checked_grid_add(descriptor.width(), 1)? {
                        continue;
                    }
                    let Some(placement) =
                        routed_label_placement_for_descriptor(points[3], points[2], descriptor)
                    else {
                        continue;
                    };
                    let mut host = PlannedRouteLabel::new(descriptor, placement);
                    host.anchor = LabelAnchor::Segment {
                        start: points[3],
                        end: points[2],
                        route_segment: None,
                    };
                    Some(host)
                } else {
                    None
                };
                if let Some(mut plan) = plan_terminal_path(
                    &path,
                    attachments,
                    request.edge,
                    host,
                    request.charset,
                    resources,
                )? {
                    plan.attachments = Some(attachments);
                    candidates.push(plan);
                }
            }
        }
    }
    Ok(candidates)
}

fn loop_envelope(
    request: EdgeRouteRequest<'_>,
    label: Option<RoutedLabelDescriptor>,
    resources: &ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<(CanvasCoord, usize)> {
    let node = request.from;
    let mut parallel_index = 0;
    let mut label_width = label.map_or(0, RoutedLabelDescriptor::width);
    let mut reverse = false;
    for (index, edge) in request.edges.iter().enumerate() {
        execution.checkpoint(OperationPhase::Layout)?;
        resources.charge_layout_work(1)?;
        if !edge.stroke.is_visible() || edge.to != node.id {
            continue;
        }
        if edge.from == node.id {
            parallel_index = resources
                .checked_work_add(parallel_index, usize::from(index < request.edge_index))?;
            if let Some(raw) = &edge.label {
                label_width = label_width.max(
                    try_measure_edge_label(raw, request.charset.width_profile, resources)?
                        .map_or(0, |metrics| metrics.max_width),
                );
            }
        } else if !reverse {
            for source in &request.graph_layout.nodes {
                execution.checkpoint(OperationPhase::Layout)?;
                resources.charge_layout_work(1)?;
                if source.id == edge.from {
                    reverse = source.center_y() == node.center_y() && source.x > node.right();
                    break;
                }
            }
        }
    }
    let mut neighbor_x = None;
    for other in &request.graph_layout.nodes {
        execution.checkpoint(OperationPhase::Layout)?;
        resources.charge_layout_work(1)?;
        if other.x > node.right() && other.center_y() == node.center_y() {
            neighbor_x = Some(neighbor_x.map_or(other.x, |x: usize| x.min(other.x)));
        }
    }
    let right = if let Some(x) = neighbor_x {
        resources.checked_grid_add(node.right(), (x - node.right()) / 2)?
    } else {
        resources.checked_grid_add(node.right(), 2)?
    };
    // Every parallel loop reserves the longest full-width label before adding its lane
    // offset, including labels on later edges in the caller's canonical ordering.
    let right = if label_width == 0 {
        right
    } else {
        right.max(
            resources
                .checked_grid_add(resources.checked_grid_add(node.center_x(), label_width)?, 1)?,
        )
    };
    let offset = resources.checked_grid_mul(parallel_index, 2)?;
    Ok((
        CanvasCoord {
            x: resources.checked_grid_add(right, offset)?,
            y: resources.checked_grid_add(
                resources.checked_grid_add(node.bottom(), if reverse { 3 } else { 2 })?,
                offset,
            )?,
        },
        parallel_index,
    ))
}

fn loop_ports(
    ports: &[TerminalPort],
    outward: StepDirection,
    preferred: usize,
    private_source: bool,
    resources: &ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<[Option<TerminalPort>; LOOP_CONTACTS]> {
    let mut selected = [None; LOOP_CONTACTS];
    let mut fallback = None;
    for mut port in ports.iter().copied() {
        execution.checkpoint(OperationPhase::Layout)?;
        resources.charge_layout_work(1)?;
        if port.attachment.outward != outward {
            continue;
        }
        // terminal_ports resolves contours using an exterior bounding-box probe. Start
        // the loop itself one cell from the actual contact, including slanted contours.
        let contact = port.attachment.contact;
        port.berth = match outward {
            StepDirection::Right => CanvasCoord {
                x: resources.checked_grid_add(contact.x, 1)?,
                y: contact.y,
            },
            StepDirection::Down => CanvasCoord {
                x: contact.x,
                y: resources.checked_grid_add(contact.y, 1)?,
            },
            _ => continue,
        };
        let intercept = port_intercept(port);
        if intercept > preferred && !private_source {
            if fallback.is_none_or(|previous| intercept < port_intercept(previous)) {
                fallback = Some(port);
            }
            continue;
        }
        // Keep the canonical contact ahead of contacts above/left of it. A private
        // source can also use lower contacts when the upper shoulder has too few berths.
        let mut candidate = Some(port);
        for slot in &mut selected {
            resources.charge_layout_work(1)?;
            if candidate.is_some_and(|candidate| {
                let key = |port| {
                    let intercept = port_intercept(port);
                    (intercept > preferred, intercept.abs_diff(preferred))
                };
                slot.is_none_or(|previous| key(candidate) < key(previous))
            }) {
                std::mem::swap(slot, &mut candidate);
            }
        }
    }
    if selected[0].is_none() {
        selected[0] = fallback;
    }
    Ok(selected)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AsciiRenderOptions;
    use crate::graph::charset::GraphCharset;
    use crate::graph::layout::{GraphLayout, layout_graph};
    use crate::graph::model::{AsciiGraph, GraphDirection, GraphEdgeAttrs, GraphNodeShape};
    use crate::graph::routing::endpoint_layout;
    use crate::graph::routing::plan::MarkerEndpoint;
    use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy};
    use merman_model::{CancelReason, OperationControl};

    fn request<'a>(
        graph: &'a AsciiGraph,
        layout: &'a GraphLayout,
        charset: &'a GraphCharset,
        edge_index: usize,
    ) -> EdgeRouteRequest<'a> {
        EdgeRouteRequest {
            graph,
            graph_layout: layout,
            edges: &graph.edges,
            from: &layout.nodes[0],
            to: &layout.nodes[0],
            edge_index,
            edge: &graph.edges[edge_index],
            charset,
        }
    }

    #[test]
    fn loop_candidates_keep_actual_contacts_markers_and_complete_private_labels() {
        let policy = AsciiResourcePolicy::unbounded();
        for shape in [
            GraphNodeShape::Rect,
            GraphNodeShape::Diamond,
            GraphNodeShape::LeanRight,
            GraphNodeShape::LeanLeft,
        ] {
            let options = AsciiRenderOptions::unicode();
            let charset = GraphCharset::for_options(&options);
            let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
            graph.add_node("a", "A");
            graph.add_edge_with_attrs(
                "a",
                "a",
                GraphEdgeAttrs {
                    label: Some("完整重试标签<br/>again".into()),
                    start_marker: GraphEdgeMarker::Circle,
                    end_marker: GraphEdgeMarker::Cross,
                    ..GraphEdgeAttrs::default()
                },
            );
            let mut layout = layout_graph(&graph, &options);
            let node = &mut layout.nodes[0];
            node.x = 10;
            node.y = 10;
            node.width = 17;
            node.height = 7;
            node.shape = shape;
            let request = request(&graph, &layout, &charset, 0);
            let descriptor = RoutedLabelDescriptor::for_test(
                0,
                graph.edges[0].label.as_deref().unwrap(),
                charset.width_profile,
            );
            let mut resources = ResourceContext::new(policy);
            let execution = AsciiExecution::for_test(&policy);
            let plans =
                plan_self_loop_candidates(request, descriptor, &mut resources, execution).unwrap();
            assert!(!plans.is_empty(), "{shape:?}");
            assert!(plans.len() <= MAX_SELF_LOOP_CANDIDATES);
            if shape == GraphNodeShape::Rect {
                let contacts: rustc_hash::FxHashSet<_> = plans
                    .iter()
                    .map(|plan| plan.attachments.unwrap().start.contact)
                    .collect();
                assert_eq!(contacts.len(), LOOP_CONTACTS);
            }
            assert_eq!(
                plans,
                plan_self_loop_candidates(request, descriptor, &mut resources, execution).unwrap()
            );
            for plan in plans {
                let attachments = plan.attachments.unwrap();
                assert_ne!(attachments.start.contact, attachments.end.contact);
                let start = plan
                    .terminal_candidate(MarkerEndpoint::Start, "flowchart")
                    .unwrap();
                let end = plan
                    .terminal_candidate(MarkerEndpoint::End, "flowchart")
                    .unwrap();
                assert_eq!(
                    start.coord,
                    CanvasCoord {
                        x: attachments.start.contact.x + 1,
                        y: attachments.start.contact.y,
                    }
                );
                assert_eq!(
                    end.coord,
                    CanvasCoord {
                        x: attachments.end.contact.x,
                        y: attachments.end.contact.y + 1,
                    }
                );
                let label = &plan.labels[0];
                assert_eq!(Some(label.descriptor), descriptor);
                let LabelAnchor::Segment { start, end, .. } = label.anchor else {
                    panic!("a loop label needs its private bottom segment");
                };
                assert_eq!(start.y, end.y);
                assert_eq!(
                    start.y,
                    plan.cells.iter().map(|cell| cell.coord.y).max().unwrap()
                );
                assert!(start.x < label.placement.x());
                assert!(label.placement.x() + label.placement.width() <= end.x);
                let plan = plan
                    .with_marker_requests(
                        GraphEdgeMarker::Circle,
                        GraphEdgeMarker::Cross,
                        "flowchart",
                    )
                    .unwrap();
                assert!(plan.requests_marker(MarkerEndpoint::Start));
                assert!(plan.requests_marker(MarkerEndpoint::End));
            }
        }
    }

    #[test]
    fn parallel_channels_share_the_longest_label_envelope_and_skip_invisible_loops() {
        let options = AsciiRenderOptions::ascii();
        let charset = GraphCharset::for_options(&options);
        let policy = AsciiResourcePolicy::unbounded();
        let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
        graph.add_node("a", "A");
        for text in ["a", "invisible", "z is the longest retry label"] {
            graph.add_edge_with_attrs(
                "a",
                "a",
                GraphEdgeAttrs {
                    label: Some(text.into()),
                    ..GraphEdgeAttrs::default()
                },
            );
        }
        graph.edges[1].stroke = crate::graph::model::GraphEdgeStroke::Invisible;
        let layout = layout_graph(&graph, &options);
        let mut resources = ResourceContext::new(policy);
        let mut first_bounds = None;
        for (parallel_index, edge_index) in [0, 2].into_iter().enumerate() {
            let request = request(&graph, &layout, &charset, edge_index);
            let label = RoutedLabelDescriptor::for_test(
                edge_index,
                graph.edges[edge_index].label.as_deref().unwrap(),
                charset.width_profile,
            );
            let plans = plan_self_loop_candidates(
                request,
                label,
                &mut resources,
                AsciiExecution::for_test(&policy),
            )
            .unwrap();
            let first = &plans[0];
            let bounds = (
                first.cells.iter().map(|cell| cell.coord.x).max().unwrap(),
                first.cells.iter().map(|cell| cell.coord.y).max().unwrap(),
            );
            assert_eq!(
                first.attachments.unwrap().end.contact.x,
                request.from.center_x() + parallel_index
            );
            if let Some((x, y)) = first_bounds {
                assert_eq!(bounds, (x + 2, y + 2));
            } else {
                assert!(bounds.0 > request.from.center_x() + "z is the longest retry label".len());
                first_bounds = Some(bounds);
            }
        }
        assert!(
            plan_self_loop_candidates(
                request(&graph, &layout, &charset, 1),
                None,
                &mut resources,
                AsciiExecution::for_test(&policy)
            )
            .unwrap()
            .is_empty()
        );
    }

    #[test]
    fn container_loop_channels_stay_outside_their_frame_and_inside_the_parent() {
        let options = AsciiRenderOptions::ascii();
        let charset = GraphCharset::for_options(&options);
        let policy = AsciiResourcePolicy::unbounded();
        let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
        graph.add_node("a", "A");
        for (id, member) in [("g", "a"), ("p", "g")] {
            graph.add_group_with_style(id, id, None, vec![member.into()], Default::default());
        }
        graph.add_edge("g", "g");
        let mut layout = layout_graph(&graph, &options);
        for group in &mut layout.groups {
            (group.x, group.y, group.width, group.height) = if group.id == "g" {
                (10, 10, 20, 16)
            } else {
                (1, 1, 32, 28)
            };
        }
        let node = &mut layout.nodes[0];
        (node.x, node.y, node.width, node.height) = (14, 15, 5, 5);
        let endpoint = endpoint_layout(&layout, "g", &charset).unwrap();
        let request = EdgeRouteRequest {
            from: &endpoint,
            to: &endpoint,
            ..request(&graph, &layout, &charset, 0)
        };
        let mut resources = ResourceContext::new(policy);
        let plans = plan_self_loop_candidates(
            request,
            None,
            &mut resources,
            AsciiExecution::for_test(&policy),
        )
        .unwrap();
        assert!(!plans.is_empty());
        for plan in plans {
            let contacts = plan.attachments.unwrap();
            assert_ne!(contacts.start.contact, contacts.end.contact);
            assert_eq!(contacts.start.contact.x, endpoint.right());
            assert_eq!(contacts.end.contact.y, endpoint.bottom());
            for (_, cell) in plan.active_cells() {
                let CanvasCoord { x, y } = cell.coord;
                assert!(1 < x && x < 32 && 1 < y && y < 28);
                assert!(x >= endpoint.right() || y >= endpoint.bottom());
            }
        }
    }

    #[test]
    fn loop_factory_propagates_resource_exhaustion_and_mid_scan_cancellation() {
        let options = AsciiRenderOptions::ascii();
        let charset = GraphCharset::for_options(&options);
        let policy = AsciiResourcePolicy::unbounded();
        let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
        graph.add_node("a", "A");
        graph.add_edge("a", "a");
        let layout = layout_graph(&graph, &options);
        let request = request(&graph, &layout, &charset, 0);
        let limited_policy = policy
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 1)
            .unwrap();
        let mut limited = ResourceContext::new(limited_policy);
        let error = plan_self_loop_candidates(
            request,
            None,
            &mut limited,
            AsciiExecution::for_test(&limited_policy),
        )
        .unwrap_err();
        assert!(
            matches!(error, crate::AsciiError::ResourceLimitExceeded(details)
            if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits)
        );
        let control = OperationControl::new();
        control.cancel_after_checkpoints(4);
        let mut resources = ResourceContext::new(policy);
        let error = plan_self_loop_candidates(
            request,
            None,
            &mut resources,
            AsciiExecution::new(&control, &policy),
        )
        .unwrap_err();
        assert!(matches!(error, crate::AsciiError::Cancelled(details)
            if details.phase == OperationPhase::Layout && details.reason == CancelReason::Requested));
    }
}
