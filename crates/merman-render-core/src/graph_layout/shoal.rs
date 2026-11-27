//! Solve a measured graph in its family's occurrence order.

use super::{
    Axis, BorderStitch, Endpoint, Marker, MeasuredGraph, Point, Size, SolvedEdge, SolvedGraph,
    SolvedGroup,
};
use crate::LayoutLabel;
use crate::layout_work::OperationLayoutWorkControl;
use crate::{Error, Result};
use merman_layout_shoal as shoal;
pub use shoal::{OrderHint, WidthPolicy};

#[derive(Clone, Copy)]
pub struct NodeHint {
    pub order: OrderHint,
    pub direction: shoal::Direction,
    pub position: Option<(shoal::PositionHint, u64)>,
}

#[derive(Default)]
pub struct Constraints<'a> {
    pub flow_relative_nodes: &'a [(usize, Axis)],
    pub edge_ports: &'a [(shoal::PortSideHint, shoal::PortSideHint)],
    pub lane_order: &'a [u32],
    pub lane_boundaries: &'a [u32],
    pub capture_ranks: bool,
    pub allow_turn: bool,
    pub flush_top_left_titles: bool,
}

#[derive(Default)]
pub struct Placement<'a> {
    pub width: WidthPolicy,
    pub spacing: Option<(f64, f64, f64)>,
    pub capture_ranks: bool,
    pub reuse_components: bool,
    pub hints: &'a [Option<NodeHint>],
    pub incident_signatures: &'a [u64],
    pub constraints: Constraints<'a>,
}

fn marker_metrics(marker: Marker) -> shoal::MarkerMetrics {
    shoal::MarkerMetrics {
        length: marker.trim + marker.depth,
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

pub fn project_input(
    measured: &MeasuredGraph,
    placement: &Placement<'_>,
    work: &mut OperationLayoutWorkControl,
) -> Result<shoal::Graph> {
    let mut graph = shoal::Graph::default();
    graph.direction = measured.direction;
    graph.nodes = project_rows(measured.nodes.iter().enumerate(), work, |(index, node)| {
        let mut spec = shoal::NodeSpec::new(node.size, node.seq);
        spec.parent = node.parent.map(shoal::GroupId);
        spec.contour = node.contour;
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
        let node = graph
            .nodes
            .get_mut(index)
            .ok_or_else(|| Error::InvalidModel {
                message: "Shoal relative axis hint does not match a measured node occurrence"
                    .into(),
            })?;
        node.flow_relative_axis = Some(axis);
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
    work.charge_adapter(placement.constraints.lane_boundaries.len())?;
    for &id in placement.constraints.lane_boundaries {
        let group = graph
            .groups
            .get_mut(id as usize)
            .ok_or_else(|| Error::InvalidModel {
                message: "Shoal lane boundary does not match a measured group occurrence".into(),
            })?;
        group.lane_boundary_only = true;
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
        spec.set_marker_runways([edge.start_marker, edge.end_marker].map(Marker::smooth_runway));
        if let Some(&(source, target)) = ports.get(index) {
            spec.source_port = source;
            spec.target_port = target;
        }
        Ok(spec)
    })?;
    Ok(graph)
}

impl Placement<'_> {
    pub fn options_for(&self, measured: &MeasuredGraph) -> shoal::Options {
        let mut options = shoal::Options::default();
        options.routing = measured.routing;
        options.global_orthogonal = measured.routing == shoal::RoutingStyle::Orthogonal;
        (
            options.spacing.node,
            options.spacing.rank,
            options.spacing.component,
        ) = self.spacing.unwrap_or((
            measured.node_spacing,
            measured.rank_spacing,
            measured.component_spacing,
        ));
        options.group_policy.inherit_crossed_direction = measured.inherit_crossed_group_direction;
        options.group_policy.flush_top_left_titles = self.constraints.flush_top_left_titles;
        options.width_policy = self.width;
        options.capture_node_ranks = self.capture_ranks || self.constraints.capture_ranks;
        options.reuse_unchanged_components = self.reuse_components;
        options
    }
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

/// Projects the result of solving the same measured graph. Shoal returns node, group, and edge
/// geometry in input order and lists truncated route occurrences in strictly increasing order.
pub fn project_output(
    measured: &MeasuredGraph,
    solved: shoal::LayoutGeometry,
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
                title_struck: geometry.title_struck,
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
                corridor_clearance: route.corridor_clearance,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::{OperationWorkMeter, RenderResourcePolicy};
    use std::sync::Arc;

    #[test]
    fn measured_marker_runway_keeps_the_setback_separate_from_the_footprint() {
        let marker = |trim, depth, width| Marker { trim, depth, width };
        for (marker, footprint, runway) in [
            (marker(0.0, 0.0, 0.0), 0.0, 0.0),
            (marker(0.0, 0.0, 10.0), 0.0, 6.0),
            (marker(4.0, 10.0, 10.0), 14.0, 14.0),
            (marker(12.0, 0.0, 10.0), 12.0, 24.0),
        ] {
            assert_eq!(marker_metrics(marker).length, footprint);
            assert_eq!(marker.smooth_runway(), runway);
        }
    }

    #[test]
    fn relative_axis_hints_must_refer_to_measured_nodes() {
        let graph = MeasuredGraph {
            direction: shoal::Direction::Down,
            routing: shoal::RoutingStyle::Curved,
            node_spacing: 0.0,
            rank_spacing: 0.0,
            component_spacing: 0.0,
            inherit_crossed_group_direction: false,
            nodes: Vec::new(),
            groups: Vec::new(),
            edges: Vec::new(),
        };
        let placement = Placement {
            constraints: Constraints {
                flow_relative_nodes: &[(0, Axis::Horizontal)],
                ..Constraints::default()
            },
            ..Placement::default()
        };
        let meter = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let mut work = OperationLayoutWorkControl::new(Arc::new(meter));
        assert!(matches!(
            project_input(&graph, &placement, &mut work),
            Err(Error::InvalidModel { .. })
        ));
    }
}
