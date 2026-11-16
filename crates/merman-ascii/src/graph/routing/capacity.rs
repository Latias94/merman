//! Necessary capacity bound for the current fixed-terminal presentation only.
//!
//! Count the complete canonical scene after joint branch allocation, before route search.
//! A terminal admits at most `MAX_MARKER_CANDIDATES` consecutive marker cells on its outward
//! ray. Retaining every admissible contact and direction deliberately overestimates capacity:
//! obstacles, overlapping rays, labels, and heuristic failures cannot tighten this certificate.

use super::allocation::BranchAllocation;
use super::layout_allocation_failed;
use super::path::StepDirection;
use super::plan::{MAX_MARKER_CANDIDATES, MarkerEndpoint, NodeAttachment};
use crate::error::{AsciiError, Result};
use crate::graph::charset::GraphCharset;
use crate::graph::layout::{CanvasCoord, GraphLayout, NodeLayout};
use crate::graph::model::{AsciiGraph, AsciiGraphEdge, GraphEdgeMarker, GraphNodeShape};
use crate::operation::AsciiExecution;
use crate::resource::ResourceContext;
use merman_model::OperationPhase;
use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};

/// Checks only fixed contours, using the same canonical edge slice as `allocation`.
///
/// Success is not evidence that routing will succeed. Failure certifies only that independent
/// marker demand exceeds an upper bound for these current contacts and bounded candidate rays;
/// it makes no claim about other presentations, layouts, or general graph impossibility.
pub(super) fn preflight_fixed_port_capacity(
    graph: &AsciiGraph,
    layout: &GraphLayout,
    canonical_edges: &[AsciiGraphEdge],
    allocation: &BranchAllocation,
    charset: &GraphCharset,
    resources: &ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<()> {
    execution.checkpoint(OperationPhase::Layout)?;
    let resources = execution.resource_context(resources, OperationPhase::Layout);
    let demands = marker_demands(layout, canonical_edges, allocation, &resources)?;
    for (node, required) in layout.nodes.iter().zip(demands) {
        resources.charge_layout_work(1)?;
        // No certificate is needed for low pressure. In particular ordinary shared fans
        // consume one marker, regardless of their number of independently identified edges.
        if required <= MAX_MARKER_CANDIDATES {
            continue;
        }
        let Some(available) = fixed_port_upper_bound(node, charset, &resources)? else {
            continue;
        };
        if required > available {
            // Error IDs are authored text too: admit copying work and reserve fallibly.
            resources.charge_layout_work(node.id.len())?;
            let mut endpoint = String::new();
            endpoint
                .try_reserve_exact(node.id.len())
                .map_err(|_| layout_allocation_failed())?;
            resources.checkpoint()?;
            endpoint.push_str(&node.id);
            resources.checkpoint()?;
            return Err(AsciiError::PortCapacityExceeded {
                diagram_type: graph.diagram_type(),
                endpoint,
                required,
                available,
            });
        }
    }
    resources.checkpoint()
}

fn marker_demands(
    layout: &GraphLayout,
    canonical_edges: &[AsciiGraphEdge],
    allocation: &BranchAllocation,
    resources: &ResourceContext,
) -> Result<Vec<usize>> {
    resources.charge_layout_work_product(layout.nodes.len(), 2)?;
    let mut indices = HashMap::default();
    indices
        .try_reserve(layout.nodes.len())
        .map_err(|_| layout_allocation_failed())?;
    let mut demands = Vec::new();
    demands
        .try_reserve_exact(layout.nodes.len())
        .map_err(|_| layout_allocation_failed())?;
    demands.resize(layout.nodes.len(), 0);
    for (index, node) in layout.nodes.iter().enumerate() {
        resources.charge_layout_work(1)?;
        resources.charge_layout_work(node.id.len())?;
        // Match endpoint_layout: the first node wins, but a group shadows any node ID.
        indices.entry(node.id.as_str()).or_insert(index);
    }
    for group in &layout.groups {
        resources.charge_layout_work(1)?;
        resources.charge_layout_work(group.id.len())?;
        indices.remove(group.id.as_str());
    }

    let mut shared_markers = HashSet::default();
    for (edge_index, edge) in canonical_edges.iter().enumerate() {
        resources.charge_layout_work(1)?;
        if !edge.stroke.is_visible() {
            continue;
        }
        for (endpoint, id, marker) in [
            (MarkerEndpoint::Start, edge.from.as_str(), edge.start_marker),
            (MarkerEndpoint::End, edge.to.as_str(), edge.end_marker),
        ] {
            if marker == GraphEdgeMarker::Open {
                continue;
            }
            resources.charge_layout_work(id.len())?;
            let Some(&node_index) = indices.get(id) else {
                // Missing endpoints and group routing retain their existing diagnostics.
                continue;
            };
            if let Some(branch) = allocation.get(edge_index)
                && branch.shared_endpoint == endpoint
            {
                resources.charge_layout_work(1)?;
                shared_markers
                    .try_reserve(1)
                    .map_err(|_| layout_allocation_failed())?;
                if !shared_markers.insert((branch.group, endpoint == MarkerEndpoint::End)) {
                    continue;
                }
            }
            // Parallel, reciprocal, and both ends of self-loops remain independent unless
            // this precise endpoint has an explicit shared-marker allocation above.
            demands[node_index] = resources.checked_work_add(demands[node_index], 1)?;
        }
    }
    Ok(demands)
}

fn fixed_port_upper_bound(
    node: &NodeLayout,
    charset: &GraphCharset,
    resources: &ResourceContext,
) -> Result<Option<usize>> {
    if !matches!(
        node.shape,
        GraphNodeShape::Text
            | GraphNodeShape::StateStart
            | GraphNodeShape::StateEnd
            | GraphNodeShape::Choice
            | GraphNodeShape::ForkJoinHorizontal
            | GraphNodeShape::ForkJoinVertical
    ) || node.width == 0
        || node.height == 0
        // Non-Text fixed contours have non-corner contacts. Do not infer a certificate
        // for degenerate geometry outside those contours' normal sizing contract.
        || (node.shape != GraphNodeShape::Text && (node.width < 3 || node.height < 3))
    {
        return Ok(None);
    }
    // Admit both exclusive extents before shape helpers use x + width - 1 / y + height - 1.
    let after_right = resources.checked_grid_add(node.x, node.width)?;
    let after_bottom = resources.checked_grid_add(node.y, node.height)?;
    let right = after_right - 1;
    let bottom = after_bottom - 1;
    let mut ports = 0;
    for outward in [
        StepDirection::Right,
        StepDirection::Down,
        StepDirection::Left,
        StepDirection::Up,
    ] {
        resources.charge_layout_work(1)?;
        let (first, last, probe) = match outward {
            StepDirection::Left => (node.y, bottom, node.x.checked_sub(1)),
            StepDirection::Right => (node.y, bottom, Some(after_right)),
            StepDirection::Up => (node.x, right, node.y.checked_sub(1)),
            StepDirection::Down => (node.x, right, Some(after_bottom)),
        };
        let Some(probe) = probe else {
            continue;
        };
        // Match terminal_ports' side domain, without storing ports or enumerating port pairs.
        // Resolving each contact also respects Choice's restricted contour.
        for intercept in first..=last {
            resources.charge_layout_work(1)?;
            if node.shape != GraphNodeShape::Text && (intercept == first || intercept == last) {
                continue;
            }
            let berth = match outward {
                StepDirection::Left | StepDirection::Right => CanvasCoord {
                    x: probe,
                    y: intercept,
                },
                StepDirection::Up | StepDirection::Down => CanvasCoord {
                    x: intercept,
                    y: probe,
                },
            };
            if NodeAttachment::resolve(node, berth, outward.opposite(), charset).is_some() {
                ports = resources.checked_work_add(ports, 1)?;
            }
        }
    }
    resources
        .checked_work_mul(ports, MAX_MARKER_CANDIDATES)
        .map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AsciiRenderOptions;
    use crate::graph::label::GraphLabel;
    use crate::graph::layout::GridCoord;
    use crate::graph::model::{GraphDirection, GraphEdgeMarker, GraphEdgeStroke, GraphNodeStyle};
    use crate::resource::{AsciiResourceLimitCause, AsciiResourceLimitId, AsciiResourcePolicy};
    use merman_model::{CancelReason, OperationControl};

    fn node(
        id: &str,
        shape: GraphNodeShape,
        (x, y): (usize, usize),
        (width, height): (usize, usize),
    ) -> NodeLayout {
        NodeLayout {
            id: id.to_string(),
            label: GraphLabel::new("x"),
            shape,
            style: GraphNodeStyle::default(),
            grid: GridCoord { x: 0, y: 0 },
            x,
            y,
            width,
            height,
        }
    }

    fn parallel_heads(
        count: usize,
    ) -> (AsciiGraph, GraphLayout, Vec<AsciiGraphEdge>, GraphCharset) {
        let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
        graph.add_node_with_shape_and_style(
            "source",
            "x",
            GraphNodeShape::Text,
            GraphNodeStyle::default(),
        );
        graph.add_node("target", "x");
        for _ in 0..count {
            graph.add_edge("source", "target");
        }
        for edge in &mut graph.edges {
            edge.start_marker = GraphEdgeMarker::Circle;
            edge.end_marker = GraphEdgeMarker::Open;
        }
        let layout = GraphLayout {
            nodes: vec![
                node("source", GraphNodeShape::Text, (8, 8), (1, 1)),
                node("target", GraphNodeShape::Rect, (32, 8), (5, 3)),
            ],
            groups: Vec::new(),
            group_background_order: Vec::new(),
        };
        // The preflight must use the supplied complete canonical edges, not graph.edges.
        let edges = std::mem::take(&mut graph.edges);
        (
            graph,
            layout,
            edges,
            GraphCharset::for_options(&AsciiRenderOptions::ascii()),
        )
    }

    fn check(
        graph: &AsciiGraph,
        layout: &GraphLayout,
        edges: &[AsciiGraphEdge],
        allocation: &BranchAllocation,
        charset: &GraphCharset,
    ) -> Result<()> {
        let policy = AsciiResourcePolicy::unbounded();
        preflight_fixed_port_capacity(
            graph,
            layout,
            edges,
            allocation,
            charset,
            &ResourceContext::new(policy),
            AsciiExecution::for_test(&policy),
        )
    }

    #[test]
    fn thirty_three_parallel_source_heads_exceed_four_directions_times_eight() {
        let (graph, layout, edges, charset) = parallel_heads(33);
        let policy = AsciiResourcePolicy::unbounded();
        let allocation = BranchAllocation::try_new(
            &graph,
            &layout,
            &edges,
            &charset,
            &ResourceContext::new(policy),
            AsciiExecution::for_test(&policy),
        )
        .unwrap();
        assert!(
            edges
                .iter()
                .enumerate()
                .all(|(index, _)| allocation.get(index).is_none())
        );
        assert_eq!(
            check(&graph, &layout, &edges, &allocation, &charset),
            Err(AsciiError::PortCapacityExceeded {
                diagram_type: "flowchart",
                endpoint: "source".to_string(),
                required: 33,
                available: 32,
            }),
        );
        assert_eq!(
            check(
                &graph,
                &layout,
                &edges[..32],
                &BranchAllocation::default(),
                &charset
            ),
            Ok(())
        );
        assert!(graph.edges.is_empty());
    }

    #[test]
    fn reciprocal_and_both_self_loop_endpoints_preserve_marker_multiplicity() {
        let (graph, layout, mut edges, charset) = parallel_heads(33);
        for edge in &mut edges[..16] {
            std::mem::swap(&mut edge.from, &mut edge.to);
            std::mem::swap(&mut edge.start_marker, &mut edge.end_marker);
        }
        let allocation = BranchAllocation::default();
        assert!(matches!(
            check(&graph, &layout, &edges, &allocation, &charset),
            Err(AsciiError::PortCapacityExceeded {
                required: 33,
                available: 32,
                ..
            })
        ));
        edges.truncate(17);
        for edge in &mut edges {
            edge.from = "source".to_string();
            edge.to = "source".to_string();
            edge.start_marker = GraphEdgeMarker::Circle;
            edge.end_marker = GraphEdgeMarker::Circle;
        }
        assert!(matches!(
            check(&graph, &layout, &edges, &allocation, &charset),
            Err(AsciiError::PortCapacityExceeded {
                required: 34,
                available: 32,
                ..
            })
        ));
        for (index, edge) in edges.iter_mut().enumerate() {
            if index % 2 == 0 {
                edge.stroke = GraphEdgeStroke::Invisible;
            } else {
                edge.start_marker = GraphEdgeMarker::Open;
                edge.end_marker = GraphEdgeMarker::Open;
            }
        }
        let resources = ResourceContext::new(AsciiResourcePolicy::unbounded());
        assert_eq!(
            marker_demands(&layout, &edges, &allocation, &resources).unwrap(),
            [0, 0]
        );
        assert_eq!(
            check(&graph, &layout, &edges, &allocation, &charset),
            Ok(())
        );
    }

    #[test]
    fn compatible_branch_and_merge_markers_count_once_only_at_the_shared_endpoint() {
        for count in [16, 33] {
            for merge in [false, true] {
                let (mut graph, mut layout, _, charset) = parallel_heads(0);
                graph.nodes.truncate(1);
                layout.nodes.truncate(1);
                for index in 0..count {
                    let id = format!("target-{index}");
                    graph.add_node(&id, "x");
                    graph.add_edge("source", &id);
                    layout
                        .nodes
                        .push(node(&id, GraphNodeShape::Text, (32, 8 + index * 4), (1, 1)));
                }
                for edge in &mut graph.edges {
                    edge.start_marker = GraphEdgeMarker::Circle;
                    edge.end_marker = GraphEdgeMarker::Circle;
                    if merge {
                        std::mem::swap(&mut edge.from, &mut edge.to);
                    }
                }
                if merge {
                    for node in &mut layout.nodes {
                        node.x = 64 - node.x - node.width;
                    }
                }
                let policy = AsciiResourcePolicy::unbounded();
                let resources = ResourceContext::new(policy);
                let allocation = BranchAllocation::try_new(
                    &graph,
                    &layout,
                    &graph.edges,
                    &charset,
                    &resources,
                    AsciiExecution::for_test(&policy),
                )
                .unwrap();
                for index in 0..count {
                    let branch = allocation
                        .get(index)
                        .expect("compatible channels must allocate");
                    assert_eq!(
                        branch.shared_endpoint,
                        if merge {
                            MarkerEndpoint::End
                        } else {
                            MarkerEndpoint::Start
                        }
                    );
                }
                let demands =
                    marker_demands(&layout, &graph.edges, &allocation, &resources).unwrap();
                assert_eq!(demands[0], 1, "the common marker counts once");
                assert!(
                    demands[1..].iter().all(|demand| *demand == 1),
                    "private endpoints must each retain their own marker"
                );
                assert_eq!(
                    check(&graph, &layout, &graph.edges, &allocation, &charset),
                    Ok(())
                );
            }
        }
    }

    #[test]
    fn fixed_contours_count_resolved_contacts_and_small_graphs_remain_admissible() {
        let (graph, mut layout, edges, charset) = parallel_heads(1);
        let resources = ResourceContext::new(AsciiResourcePolicy::unbounded());
        for (shape, size, expected) in [
            (GraphNodeShape::Text, (1, 1), 32),
            (GraphNodeShape::Text, (3, 2), 80),
            (GraphNodeShape::Choice, (5, 3), 32),
            (GraphNodeShape::StateStart, (5, 3), 64),
            (GraphNodeShape::StateEnd, (5, 3), 64),
            (GraphNodeShape::ForkJoinHorizontal, (7, 3), 96),
            (GraphNodeShape::ForkJoinVertical, (3, 7), 96),
        ] {
            layout.nodes[0] = node("source", shape, (8, 8), size);
            assert_eq!(
                fixed_port_upper_bound(&layout.nodes[0], &charset, &resources).unwrap(),
                Some(expected)
            );
            assert_eq!(
                check(
                    &graph,
                    &layout,
                    &edges,
                    &BranchAllocation::default(),
                    &charset
                ),
                Ok(())
            );
        }
        layout.nodes[0].shape = GraphNodeShape::Rect;
        assert_eq!(
            fixed_port_upper_bound(&layout.nodes[0], &charset, &resources).unwrap(),
            None
        );
        layout.nodes[0] = node("source", GraphNodeShape::Text, (1, 1), (1, usize::MAX));
        assert!(matches!(
            fixed_port_upper_bound(&layout.nodes[0], &charset, &resources),
            Err(AsciiError::ResourceLimitExceeded(details))
                if details.cause == AsciiResourceLimitCause::ArithmeticOverflow
        ));
    }

    #[test]
    fn capacity_preflight_admits_exact_work_and_rejects_one_unit_less() {
        let (graph, layout, edges, charset) = parallel_heads(33);
        let allocation = BranchAllocation::default();
        let policy = AsciiResourcePolicy::unbounded();
        let resources = ResourceContext::new(policy);
        let expected = preflight_fixed_port_capacity(
            &graph,
            &layout,
            &edges,
            &allocation,
            &charset,
            &resources,
            AsciiExecution::for_test(&policy),
        )
        .unwrap_err();
        let work = resources.layout_work_used();
        assert!(work > 1);
        let exact = policy
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, work)
            .unwrap();
        assert_eq!(
            preflight_fixed_port_capacity(
                &graph,
                &layout,
                &edges,
                &allocation,
                &charset,
                &ResourceContext::new(exact),
                AsciiExecution::for_test(&exact),
            ),
            Err(expected)
        );
        let below = policy
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, work - 1)
            .unwrap();
        assert!(matches!(preflight_fixed_port_capacity(
            &graph, &layout, &edges, &allocation, &charset, &ResourceContext::new(below), AsciiExecution::for_test(&below),
        ), Err(AsciiError::ResourceLimitExceeded(details))
            if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits
                && details.cause == AsciiResourceLimitCause::Ceiling
                && details.actual == work && details.max == work - 1));
    }

    #[test]
    fn capacity_preflight_honors_entry_and_in_scan_cancellation() {
        let (graph, layout, edges, charset) = parallel_heads(33);
        let allocation = BranchAllocation::default();
        let policy = AsciiResourcePolicy::unbounded();
        for after in [1, 20] {
            let resources = ResourceContext::new(policy);
            let control = OperationControl::new();
            control.cancel_after_checkpoints(after);
            assert!(matches!(preflight_fixed_port_capacity(
                &graph, &layout, &edges, &allocation, &charset, &resources, AsciiExecution::new(&control, &policy),
            ), Err(AsciiError::Cancelled(details))
                if details.phase == OperationPhase::Layout && details.reason == CancelReason::Requested));
            if after == 1 {
                assert_eq!(resources.layout_work_used(), 0);
            } else {
                assert!(resources.layout_work_used() > 0);
            }
        }
    }
}
