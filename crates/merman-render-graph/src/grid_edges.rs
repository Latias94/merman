//! Grid shaft and line-hop paint for measured rectangular outlines.

use merman_render_core::{
    Error, LayoutEdge, LayoutPoint, Result,
    graph_layout::mermaid_grid::{
        GridLineHops, GridPaint,
        routing::paint::{GridPaintedEdge, grid_line_hop_overrides},
    },
    resources::OperationWorkMeter,
    svg_edge_parity::{curve, edge_path, line_hops},
};
use std::collections::HashMap;

pub(crate) fn shaft(
    points: &[LayoutPoint],
    settings: GridPaint,
    work: &OperationWorkMeter,
) -> Result<String> {
    work.charge_emit_work(points.len())?;
    // Measured contacts already lie on the painted rectangular outline.
    Ok(match settings.curve {
        "linear" => curve::curve_linear_path_d(points),
        "rounded" => curve::curve_rounded_path_d(points, settings.corner_radius),
        interpolate => {
            let mut adjusted = points.to_vec();
            edge_path::maybe_fix_corners(&mut adjusted);
            curve::interpolated_path_d(&adjusted, interpolate)
        }
    })
}

pub(crate) fn line_hop_paths<'a>(
    edges: &'a [LayoutEdge],
    settings: GridPaint,
    work: &OperationWorkMeter,
) -> Result<HashMap<&'a str, String>> {
    if settings.line_hops == GridLineHops::Disabled {
        return Ok(HashMap::new());
    }
    work.charge_emit_work(edges.len())?;
    let mut painted = Vec::new();
    painted
        .try_reserve_exact(edges.len())
        .map_err(|_| allocation_error())?;
    for edge in edges {
        if edge.points.len() >= 2 {
            painted.push(GridPaintedEdge {
                geometry: line_hops::LineHopEdge {
                    id: &edge.id,
                    points: &edge.points,
                    curve: Some(settings.curve),
                    // ER cardinality shapes use the unshifted attachment coordinates.
                    arrow_type_start: None,
                    arrow_type_end: None,
                },
                corner_radius: Some(settings.corner_radius),
                // The shared pass uses this only when no curve is supplied.
                original_path: "",
            });
        }
    }
    let overrides = grid_line_hop_overrides(settings.line_hops, &painted, work)?;
    let mut paths = HashMap::new();
    paths
        .try_reserve(overrides.len())
        .map_err(|_| allocation_error())?;
    for (index, path) in overrides {
        work.charge_emit_work(path.len())?;
        paths.insert(painted[index].geometry.id, path);
    }
    Ok(paths)
}

fn allocation_error() -> Error {
    Error::LayoutAllocationFailed { backend: "grid" }
}
