use super::{GroupLayout, NodeLayout};
use crate::error::{AsciiError, Result};
use crate::graph::model::{AsciiGraph, GraphEdgeMarker, GraphGroupKind, GraphNodeShape};
use crate::graph::shape::{GraphNodeShapeSemantics, GraphNodeSide};
use crate::resource::{AsciiResourceLimitPhase, ResourceContext};
use rustc_hash::FxHashMap as HashMap;

// One endpoint reading-margin cell and one cell in which its route can turn.
const EXTERIOR_TERMINAL_CLEARANCE: usize = 2;

#[derive(Default)]
struct EndpointDemand {
    contacts: usize,
    markers: usize,
}

pub(super) fn reserve_exterior_routing(
    graph: &AsciiGraph,
    nodes: &mut [NodeLayout],
    groups: &mut [GroupLayout],
    resources: &ResourceContext,
) -> Result<(usize, usize)> {
    let mut demand = HashMap::<&str, EndpointDemand>::default();
    for edge in &graph.edges {
        resources.checkpoint()?;
        resources.charge_layout_work(1)?;
        if !edge.stroke.is_visible() {
            continue;
        }
        for (id, marker) in [(&edge.from, edge.start_marker), (&edge.to, edge.end_marker)] {
            resources.charge_layout_work(1)?;
            if !demand.contains_key(id.as_str()) {
                demand.try_reserve(1).map_err(|_| allocation_failed())?;
            }
            let count = demand.entry(id).or_default();
            count.contacts = resources.checked_work_add(count.contacts, 1)?;
            if marker != GraphEdgeMarker::Open {
                count.markers = resources.checked_work_add(count.markers, 1)?;
            }
        }
    }

    let mut offset = (0, 0);
    for node in nodes.iter() {
        resources.checkpoint()?;
        resources.charge_layout_work(1)?;
        if let Some(count) = demand.get(node.id.as_str())
            && count.markers > 1
        {
            // Open ends still occupy escape corridors beside independent marked ends.
            reserve_endpoint_exterior(node, count.contacts, &mut offset, resources)?;
        }
    }
    for group in groups.iter() {
        resources.checkpoint()?;
        resources.charge_layout_work(1)?;
        if group.kind != GraphGroupKind::Container {
            continue;
        }
        if let Some(count) = demand.get(group.id.as_str())
            && count.markers > 1
        {
            let node = NodeLayout {
                id: String::new(),
                label: crate::graph::label::GraphLabel::empty_with_profile(
                    crate::TerminalWidthProfile::Unicode,
                ),
                shape: GraphNodeShape::Rect,
                style: Default::default(),
                grid: super::GridCoord { x: 0, y: 0 },
                x: group.x,
                y: group.y,
                width: group.width,
                height: group.height,
            };
            reserve_endpoint_exterior(&node, count.contacts, &mut offset, resources)?;
        }
    }
    if offset == (0, 0) {
        return Ok(offset);
    }

    // Translate the whole scene, including open region spans.
    // Relative ranks, contours, labels, and containment remain unchanged.
    for node in nodes {
        resources.checkpoint()?;
        resources.charge_layout_work(1)?;
        node.x = resources.checked_grid_add(node.x, offset.0)?;
        node.y = resources.checked_grid_add(node.y, offset.1)?;
    }
    for group in groups {
        resources.checkpoint()?;
        resources.charge_layout_work(1)?;
        group.x = resources.checked_grid_add(group.x, offset.0)?;
        group.y = resources.checked_grid_add(group.y, offset.1)?;
        if let Some(span) = &mut group.divider_span {
            span.x_start = resources.checked_grid_add(span.x_start, offset.0)?;
            span.x_end = resources.checked_grid_add(span.x_end, offset.0)?;
        }
    }
    Ok(offset)
}

fn reserve_endpoint_exterior(
    node: &NodeLayout,
    demand: usize,
    offset: &mut (usize, usize),
    resources: &ResourceContext,
) -> Result<()> {
    let missing_x = EXTERIOR_TERMINAL_CLEARANCE.saturating_sub(node.x);
    let missing_y = EXTERIOR_TERMINAL_CLEARANCE.saturating_sub(node.y);
    if missing_x == 0 && missing_y == 0 {
        return Ok(());
    }
    let semantics = GraphNodeShapeSemantics::new(node.shape);
    let mut available = 0;
    for (side, first, last, reachable) in [
        (GraphNodeSide::Right, node.y, node.bottom(), true),
        (GraphNodeSide::Bottom, node.x, node.right(), true),
        (GraphNodeSide::Left, node.y, node.bottom(), missing_x == 0),
        (GraphNodeSide::Top, node.x, node.right(), missing_y == 0),
    ] {
        if !reachable {
            continue;
        }
        for intercept in resources.checked_grid_add(first, 1)?..last {
            resources.checkpoint()?;
            resources.charge_layout_work(1)?;
            if semantics.route_contact(node, side, intercept).is_some() {
                available = resources.checked_work_add(available, 1)?;
                if available >= demand {
                    return Ok(());
                }
            }
        }
    }
    offset.0 = offset.0.max(missing_x);
    offset.1 = offset.1.max(missing_y);
    Ok(())
}

fn allocation_failed() -> AsciiError {
    AsciiError::allocation_failed(AsciiResourceLimitPhase::LayoutWork.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::label::GraphLabel;
    use crate::graph::layout::{DividerSpan, GridCoord};
    use crate::graph::model::{GraphDirection, GraphEdgeAttrs, GraphEdgeStroke};
    use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy};
    use merman_model::{CancelReason, OperationControl, OperationPhase};

    fn source_node(shape: GraphNodeShape) -> NodeLayout {
        NodeLayout {
            id: "source".into(),
            label: GraphLabel::empty_with_profile(crate::TerminalWidthProfile::Unicode),
            shape,
            style: Default::default(),
            grid: GridCoord { x: 0, y: 0 },
            x: 0,
            y: 0,
            width: 10,
            height: 5,
        }
    }

    #[test]
    fn exterior_is_reserved_only_after_reachable_shape_contacts_are_exhausted() {
        for (shape, capacity) in [(GraphNodeShape::Rect, 11), (GraphNodeShape::Diamond, 2)] {
            let resources = ResourceContext::new(AsciiResourcePolicy::unbounded());
            let node = source_node(shape);
            let mut offset = (0, 0);
            reserve_endpoint_exterior(&node, capacity, &mut offset, &resources).unwrap();
            assert_eq!(offset, (0, 0));
            reserve_endpoint_exterior(&node, capacity + 1, &mut offset, &resources).unwrap();
            assert_eq!(offset, (2, 2));

            let mut node = node;
            node.x = 2;
            node.y = 2;
            offset = (0, 0);
            reserve_endpoint_exterior(&node, capacity + 1, &mut offset, &resources).unwrap();
            assert_eq!(
                offset,
                (0, 0),
                "an already available exterior needs no translation"
            );
        }
    }

    #[test]
    fn demand_ignores_invisible_and_open_markers_and_translation_preserves_groups() {
        for (stroke, markers, expected) in [
            (GraphEdgeStroke::Normal, 0, (0, 0)),
            (GraphEdgeStroke::Normal, 1, (0, 0)),
            (GraphEdgeStroke::Invisible, 12, (0, 0)),
            (GraphEdgeStroke::Normal, 2, (2, 2)),
            (GraphEdgeStroke::Normal, 12, (2, 2)),
        ] {
            let mut graph = AsciiGraph::new(GraphDirection::TopDown);
            for index in 0..12 {
                graph.add_edge_with_attrs(
                    "source",
                    "target",
                    GraphEdgeAttrs {
                        stroke,
                        start_marker: if index < markers {
                            GraphEdgeMarker::Point
                        } else {
                            GraphEdgeMarker::Open
                        },
                        end_marker: GraphEdgeMarker::Open,
                        ..Default::default()
                    },
                );
            }
            let resources = ResourceContext::new(AsciiResourcePolicy::unbounded());
            let mut nodes = vec![source_node(GraphNodeShape::Rect)];
            let original = nodes[0].clone();
            let mut groups = vec![GroupLayout {
                id: "region".into(),
                kind: GraphGroupKind::Divider,
                title: GraphLabel::empty_with_profile(crate::TerminalWidthProfile::Unicode),
                style: Default::default(),
                divider_span: Some(DividerSpan {
                    x_start: 0,
                    x_end: 9,
                }),
                x: 0,
                y: 5,
                width: 10,
                height: 1,
            }];
            assert_eq!(
                reserve_exterior_routing(&graph, &mut nodes, &mut groups, &resources).unwrap(),
                expected
            );
            assert_eq!((nodes[0].x, nodes[0].y), expected);
            assert_eq!(
                (nodes[0].width, nodes[0].height, nodes[0].grid),
                (original.width, original.height, original.grid)
            );
            assert_eq!((groups[0].x, groups[0].y), (expected.0, 5 + expected.1));
            assert_eq!(
                groups[0].divider_span.unwrap(),
                DividerSpan {
                    x_start: expected.0,
                    x_end: 9 + expected.0
                }
            );
        }
    }

    #[test]
    fn exterior_contact_scan_observes_exact_work_limits_and_cancellation() {
        let node = source_node(GraphNodeShape::Rect);
        let policy = AsciiResourcePolicy::unbounded();
        let measured = ResourceContext::new(policy);
        reserve_endpoint_exterior(&node, 12, &mut (0, 0), &measured).unwrap();
        let work = measured.layout_work_used();
        assert_eq!(work, 11);
        for (limit, succeeds) in [(work, true), (work - 1, false)] {
            let resources = ResourceContext::new(
                policy
                    .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, limit)
                    .unwrap(),
            );
            let result = reserve_endpoint_exterior(&node, 12, &mut (0, 0), &resources);
            if succeeds {
                result.unwrap();
            } else {
                assert!(
                    matches!(result, Err(AsciiError::ResourceLimitExceeded(error)) if error.limit == AsciiResourceLimitId::MaxLayoutWorkUnits)
                );
            }
        }
        let mut resources = ResourceContext::new(policy);
        let control = OperationControl::new();
        control.cancel_after_checkpoints(3);
        crate::operation::AsciiExecution::new(&control, &policy)
            .rebind_resource_context(&mut resources, OperationPhase::Layout);
        let result = reserve_endpoint_exterior(&node, 12, &mut (0, 0), &resources);
        assert!(
            matches!(result, Err(AsciiError::Cancelled(error)) if error.phase == OperationPhase::Layout && error.reason == CancelReason::Requested)
        );
    }
}
