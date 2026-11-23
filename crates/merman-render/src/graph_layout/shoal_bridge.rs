//! Solve a measured graph in its family's occurrence order.

use super::Marker;
use super::ir::{
    BorderStitch, Endpoint, MeasuredGraph, Point, Size, SolvedEdge, SolvedGraph, SolvedGroup,
};
use super::route_paint::LEAD;
use crate::layout_work::OperationLayoutWorkControl;
use crate::model::LayoutLabel;
use crate::{Error, Result};
use merman_layout_shoal as shoal;
pub(crate) use shoal::LayoutScratch;

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
    work: &mut OperationLayoutWorkControl,
) -> Result<shoal::Graph> {
    let mut graph = shoal::Graph::default();
    graph.direction = measured.direction;
    graph.nodes = project_rows(measured.nodes.iter(), work, |node| {
        let mut spec = shoal::NodeSpec::new(node.size, node.seq);
        spec.parent = node.parent.map(shoal::GroupId);
        spec.port_span = node.port_span;
        spec.perpendicular_port_span = node.perpendicular_port_span;
        spec.anchor_y = node.anchor_y;
        Ok(spec)
    })?;
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
    graph.edges = project_rows(measured.edges.iter(), work, |edge| {
        let mut spec = shoal::EdgeSpec::new(endpoint(edge.source), endpoint(edge.target));
        spec.min_rank_span = edge.min_rank_span;
        spec.weight = edge.weight;
        spec.label = edge.label;
        spec.start_label = edge.start_label;
        spec.end_label = edge.end_label;
        spec.start_marker = marker_metrics(edge.start_marker);
        spec.end_marker = marker_metrics(edge.end_marker);
        Ok(spec)
    })?;
    Ok(graph)
}

pub(crate) fn solve(
    measured: &MeasuredGraph,
    scratch: &mut LayoutScratch,
    work: &mut OperationLayoutWorkControl,
) -> Result<SolvedGraph> {
    let mut options = shoal::Options::default();
    options.routing = measured.routing;
    options.spacing.node = measured.node_spacing;
    options.spacing.rank = measured.rank_spacing;
    options.spacing.component = measured.component_spacing;
    let solved =
        shoal::layout_with_control(&project_input(measured, work)?, &options, scratch, work)
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
            })
        },
    )?;
    Ok(SolvedGraph {
        nodes: solved.nodes,
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
