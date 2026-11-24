//! Solve a measured graph in its family's occurrence order.

use super::Marker;
use super::ir::{
    Axis, BorderStitch, Endpoint, MeasuredGraph, Point, Size, SolvedEdge, SolvedGraph, SolvedGroup,
};
use super::route_paint::LEAD;
use crate::layout_work::OperationLayoutWorkControl;
use crate::model::LayoutLabel;
use crate::{Error, Result};
use merman_layout_shoal as shoal;
pub(crate) use shoal::{OrderHint, WidthPolicy};

#[derive(Clone, Copy)]
pub(crate) struct NodeHint {
    pub(crate) order: OrderHint,
    pub(crate) direction: shoal::Direction,
    pub(crate) position: Option<(shoal::PositionHint, u64)>,
}

#[derive(Default)]
pub(crate) struct Constraints<'a> {
    pub(crate) flow_relative_nodes: &'a [(usize, Axis)],
    pub(crate) edge_ports: &'a [(shoal::PortSideHint, shoal::PortSideHint)],
    pub(crate) lane_order: &'a [u32],
    pub(crate) capture_ranks: bool,
    pub(crate) allow_turn: bool,
    pub(crate) inline_labels: bool,
    pub(crate) compact_ranks: bool,
}

#[derive(Default)]
pub(crate) struct Placement<'a> {
    pub(crate) width: WidthPolicy,
    pub(crate) capture_ranks: bool,
    pub(crate) reuse_components: bool,
    pub(crate) hints: &'a [Option<NodeHint>],
    pub(crate) incident_signatures: &'a [u64],
    pub(crate) constraints: Constraints<'a>,
}

#[cfg(all(
    test,
    feature = "diagram-flowchart",
    feature = "layout-dagre",
    feature = "layout-elk"
))]
#[path = "kernel_bench.rs"]
mod kernel_bench;

fn marker_metrics(marker: Marker) -> shoal::MarkerMetrics {
    shoal::MarkerMetrics {
        // The painter keeps a straight normal lead even at an unmarked end (zero width).
        length: marker.trim + marker.depth.max(LEAD),
        width: marker.width,
    }
}

fn endpoint(endpoint: Endpoint) -> shoal::Endpoint {
    match endpoint {
        Endpoint::Node(index) => shoal::Endpoint::Node(shoal::NodeId(index)),
        Endpoint::Group(index) => shoal::Endpoint::Group(shoal::GroupId(index)),
    }
}

fn physical_endpoint(endpoint: shoal::Endpoint) -> Endpoint {
    match endpoint {
        shoal::Endpoint::Node(id) => Endpoint::Node(id.0),
        shoal::Endpoint::Group(id) => Endpoint::Group(id.0),
    }
}

/// Admit one row per occurrence before reserving storage, then keep large transfers cancellable.
fn project_rows<I: ExactSizeIterator, T>(
    rows: I,
    work: &mut OperationLayoutWorkControl,
    mut project: impl FnMut(I::Item) -> Result<T>,
) -> Result<Vec<T>> {
    const CHECK_INTERVAL: usize = 128;
    work.charge_adapter(rows.len())?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(rows.len())
        .map_err(|_| work.map_shoal_error(shoal::LayoutError::AllocationLimit))?;
    for (index, row) in rows.enumerate() {
        if index.is_multiple_of(CHECK_INTERVAL) {
            work.charge_adapter(0)?;
        }
        output.push(project(row)?);
    }
    Ok(output)
}

fn project_input(
    measured: &MeasuredGraph,
    placement: &Placement<'_>,
    work: &mut OperationLayoutWorkControl,
) -> Result<shoal::Graph> {
    let mut graph = shoal::Graph::default();
    graph.direction = measured.direction;
    graph.nodes = project_rows(measured.nodes.iter().enumerate(), work, |(index, node)| {
        let mut spec = shoal::NodeSpec::new(node.size, node.seq);
        spec.parent = node.parent.map(shoal::GroupId);
        spec.port_span = node.port_span;
        spec.perpendicular_port_span = node.perpendicular_port_span;
        spec.anchor_y = node.anchor_y;
        if let Some(&signature) = placement.incident_signatures.get(index) {
            spec.set_current_incident_signature(Some(signature));
        }
        if let Some(hint) = placement.hints.get(index).copied().flatten() {
            spec.order_hint = Some(hint.order);
            spec.order_hint_direction = Some(hint.direction);
            if let Some((position, current_signature)) = hint.position {
                spec.set_position_hint(position, Some(current_signature));
            }
        }
        Ok(spec)
    })?;
    work.charge_adapter(placement.constraints.flow_relative_nodes.len())?;
    for &(index, axis) in placement.constraints.flow_relative_nodes {
        graph.nodes[index].flow_relative_axis = Some(axis);
    }
    graph.groups = project_rows(measured.groups.iter(), work, |group| {
        let mut spec = shoal::GroupSpec::new(group.seq);
        spec.parent = group.parent.map(shoal::GroupId);
        spec.direction = group.direction;
        spec.padding = shoal::Insets {
            top: group.padding.y0,
            right: group.padding.x1,
            bottom: group.padding.y1,
            left: group.padding.x0,
        };
        spec.min_size = group.min_size;
        spec.title = group.title;
        spec.title_position = group.title_position;
        Ok(spec)
    })?;
    if !placement.constraints.lane_order.is_empty() {
        graph.lane_order = project_rows(placement.constraints.lane_order.iter(), work, |&id| {
            Ok(shoal::GroupId(id))
        })?;
    }
    let ports = placement.constraints.edge_ports;
    if !ports.is_empty() && ports.len() != measured.edges.len() {
        return Err(Error::InvalidModel {
            message: "Shoal port hints do not match measured edge occurrences".into(),
        });
    }
    graph.edges = project_rows(measured.edges.iter().enumerate(), work, |(index, edge)| {
        let mut spec = shoal::EdgeSpec::new(endpoint(edge.source), endpoint(edge.target));
        spec.min_rank_span = edge.min_rank_span;
        spec.weight = edge.weight;
        spec.label = edge.label;
        spec.start_label = edge.start_label;
        spec.end_label = edge.end_label;
        spec.start_marker = marker_metrics(edge.start_marker);
        spec.end_marker = marker_metrics(edge.end_marker);
        if let Some(&(source, target)) = ports.get(index) {
            spec.source_port = source;
            spec.target_port = target;
        }
        Ok(spec)
    })?;
    Ok(graph)
}

pub(crate) fn solve(
    measured: &MeasuredGraph,
    placement: Placement<'_>,
    scratch: &mut shoal::LayoutScratch,
    work: &mut OperationLayoutWorkControl,
) -> Result<SolvedGraph> {
    let mut options = shoal::Options::default();
    options.routing = measured.routing;
    options.global_orthogonal = measured.routing == shoal::RoutingStyle::Orthogonal;
    options.inline_labels = placement.constraints.inline_labels;
    options.compact_ranks = placement.constraints.compact_ranks;
    options.spacing.node = measured.node_spacing;
    options.spacing.rank = measured.rank_spacing;
    options.spacing.component = measured.component_spacing;
    options.group_policy.inherit_crossed_direction = measured.inherit_crossed_group_direction;
    options.width_policy = placement.width;
    options.capture_node_ranks = placement.capture_ranks || placement.constraints.capture_ranks;
    options.reuse_unchanged_components = placement.reuse_components;
    #[cfg(all(
        test,
        feature = "diagram-flowchart",
        feature = "layout-dagre",
        feature = "layout-elk"
    ))]
    kernel_bench::capture(measured, options);
    let graph = project_input(measured, &placement, work)?;
    let solved = shoal::layout_with_control(&graph, &options, scratch, work)
        .map_err(|error| work.map_shoal_error(error))?;
    project_output(measured, solved, work)
}

fn placed_label(
    size: Option<Size>,
    center: Option<Point>,
    kind: &str,
    index: usize,
) -> Result<Option<LayoutLabel>> {
    let Some(size) = size else { return Ok(None) };
    let center = center.ok_or_else(|| Error::InvalidModel {
        message: format!("Shoal omitted the {kind} anchor at input occurrence {index}"),
    })?;
    Ok(Some(LayoutLabel {
        x: center.x,
        y: center.y,
        width: size.width,
        height: size.height,
    }))
}

pub(super) fn project_output(
    measured: &MeasuredGraph,
    solved: shoal::LayoutOutput,
    work: &mut OperationLayoutWorkControl,
) -> Result<SolvedGraph> {
    if measured.nodes.len() != solved.nodes.len()
        || measured.groups.len() != solved.groups.len()
        || measured.edges.len() != solved.edges.len()
    {
        return Err(Error::InvalidModel {
            message: format!(
                "Shoal returned {} nodes, {} groups and {} edges for an input with {}, {} and {}",
                solved.nodes.len(),
                solved.groups.len(),
                solved.edges.len(),
                measured.nodes.len(),
                measured.groups.len(),
                measured.edges.len(),
            ),
        });
    }
    let groups = project_rows(
        measured.groups.iter().zip(solved.groups).enumerate(),
        work,
        |(index, (group, geometry))| {
            Ok(SolvedGroup {
                parent: geometry.parent.map(|id| id.0),
                rect: geometry.rect,
                title: placed_label(group.title, geometry.title_center, "group title", index)?,
                direction: geometry.effective_direction,
            })
        },
    )?;
    let mut truncated = solved.truncated_routes.into_iter().peekable();
    let edges = project_rows(
        measured.edges.iter().zip(solved.edges).enumerate(),
        work,
        |(index, (edge, route))| {
            let (source, target) = truncated
                .next_if(|row| row.edge.0 as usize == index)
                .map_or((edge.source, edge.target), |row| {
                    (physical_endpoint(row.source), physical_endpoint(row.target))
                });
            Ok(SolvedEdge {
                source,
                target,
                points: route.points,
                ports: route.ports,
                label: placed_label(edge.label, route.label_center, "edge label", index)?,
                start_label: placed_label(
                    edge.start_label,
                    route.start_label_center,
                    "edge start label",
                    index,
                )?,
                end_label: placed_label(
                    edge.end_label,
                    route.end_label_center,
                    "edge end label",
                    index,
                )?,
                reversed_for_ranking: route.reversed_for_ranking,
            })
        },
    )?;
    Ok(SolvedGraph {
        nodes: solved.nodes,
        node_ranks: solved.node_ranks,
        groups,
        edges,
        border_stitches: project_rows(solved.border_stitches.into_iter(), work, |stitch| {
            Ok(BorderStitch {
                edge: stitch.edge.0 as usize,
                point: stitch.point,
                group: stitch.group.0,
                side: stitch.side,
            })
        })?,
        direction: solved.direction,
        bounds: solved.bounds,
    })
}
