//! Grid edge paint over the layout provider's original points and label anchors.

use kurbo::Point;
use merman_model::OperationPhase;
use merman_render_core::{
    LayoutPoint,
    graph_layout::{
        Size,
        mermaid_grid::routing::{
            outline::{GridOutline, GridOutlineQuery, clip_grid_endpoints},
            paint::{GridPaintedEdge, grid_line_hop_overrides},
        },
    },
    svg_edge_parity::line_hops::LineHopEdge,
};

use super::intersect::intersect_grid_node_outline;
use super::*;
use crate::{
    edge_geometry::LegacyEdgeCurve,
    projection::{FlowchartEdgeKey, FlowchartEdgeTransportPlan},
    svg::render_input::FlowchartRenderEdge,
};

pub(super) fn paint(
    request: FlowchartEdgePathGeomRequest<'_>,
    settings: FlowchartGridPaint,
    scratch: &mut FlowchartEdgeDataPointsScratch,
    measure_neo_length: bool,
) -> Result<Option<FlowchartEdgePathGeom>> {
    let FlowchartEdgePathGeomRequest {
        ctx,
        key,
        edge,
        origin_x,
        origin_y,
        trace_enabled,
    } = request;
    let Some(layout_edge) = ctx
        .layout_edges_by_key
        .get(&key)
        .and_then(|edge| edge.materialized())
    else {
        return Ok(None);
    };
    if layout_edge.points.len() < 2 {
        return Ok(None);
    }

    let base = &mut scratch.grid_points;
    base.clear();
    base.try_reserve(layout_edge.points.len())
        .map_err(|_| Error::LayoutAllocationFailed { backend: "grid" })?;
    for point in &layout_edge.points {
        let point = Point::new(point.x + ctx.tx - origin_x, point.y + ctx.ty - origin_y);
        if !point.x.is_finite() || !point.y.is_finite() {
            return Err(Error::InvalidModel {
                message: format!("Grid edge `{}` has non-finite paint points", edge.id),
            });
        }
        base.push(point);
    }
    let source = boundary_for_node(ctx, &layout_edge.from, origin_x, origin_y);
    let target = boundary_for_node(ctx, &layout_edge.to, origin_x, origin_y);
    let intersect = |id, boundary, query| {
        let point = intersect_grid_node_outline(ctx, id, boundary, query);
        Ok(Some(Point::new(point.x, point.y)))
    };
    let intersect_source = |query| match &source {
        Some(boundary) => intersect(layout_edge.from.as_str(), boundary, query),
        None => Ok(None),
    };
    let intersect_target = |query| match &target {
        Some(boundary) => intersect(layout_edge.to.as_str(), boundary, query),
        None => Ok(None),
    };
    let clipped = clip_grid_endpoints(
        base,
        source.as_ref().map(|node| outline(node, &intersect_source)),
        target.as_ref().map(|node| outline(node, &intersect_target)),
        ctx.work_meter,
    )?;
    ctx.work_meter.charge_emit_work(clipped.len())?;
    let data_points = layout_points(&clipped);
    let measure_length = measure_neo_length
        && !trace_enabled
        && ctx.compatibility.look.is_neo()
        && !ctx.edge_style_plan.animation_for(key)?.is_active();
    let geometry = LegacyEdgeCurve::from_clipped_points(
        &data_points,
        settings.curve,
        settings.curve == "linear",
        edge.edge_type.as_deref(),
    )
    .into_geometry(settings.corner_radius, measure_length);

    if trace_enabled {
        let original_points = layout_points(base);
        record_flowchart_edge_trace(FlowchartEdgeTraceInput {
            ctx,
            edge,
            layout_edge: Some(layout_edge),
            origin_x,
            origin_y,
            base_points: &original_points,
            points_after_intersect_for_trace: Some(&data_points),
            points_for_render: &data_points,
            points_for_data_points: &data_points,
        });
    }
    ctx.work_meter.checkpoint(OperationPhase::Emit)?;
    Ok(Some(FlowchartEdgePathGeom {
        d: geometry.d,
        pb: geometry.bounds,
        neo_length: geometry.centerline_length,
        original_label_path_points: Vec::new(),
        label_path_points: Vec::new(),
        data_points,
        pre_hop_dash_mask_length: None,
        line_hop_applied: false,
        grid_hopped_d: None,
        // The shared label resolver starts from the materialized label anchor, as in common.
        label_anchor: None,
        label_path_was_explicitly_updated: false,
        emitted_d_for_label: None,
        bounds_skipped_for_viewbox: false,
    }))
}

fn outline<'a>(
    boundary: &BoundaryNode,
    intersect: &'a dyn Fn(GridOutlineQuery) -> Result<Option<Point>>,
) -> GridOutline<'a> {
    GridOutline {
        center: Some(Point::new(boundary.x, boundary.y)),
        size: Some(Size {
            width: boundary.width,
            height: boundary.height,
        }),
        intersect,
    }
}

fn layout_points(points: &[Point]) -> Vec<LayoutPoint> {
    points
        .iter()
        .map(|point| LayoutPoint {
            x: point.x,
            y: point.y,
        })
        .collect()
}

pub(in crate::svg) fn apply_grid_line_hops(
    cache: &mut FxHashMap<FlowchartEdgeKey, FlowchartEdgePathCacheEntry>,
    render_edges: &[FlowchartRenderEdge<'_>],
    settings: FlowchartGridPaint,
    work: &OperationWorkMeter,
) -> Result<()> {
    if settings.line_hops == GridLineHops::Disabled {
        return Ok(());
    }
    work.charge_emit_work(render_edges.len())?;
    let identities = FlowchartEdgeTransportPlan::for_keyed_edges(
        render_edges
            .iter()
            .map(|edge| (edge.key, edge.edge.as_ref())),
    );
    let (owners, painted): (Vec<_>, Vec<_>) = render_edges
        .iter()
        .filter_map(|edge| {
            let entry = cache.get(&edge.key)?;
            let id = identities.id(edge.key)?;
            let (arrow_type_start, arrow_type_end) =
                arrow_types_for_edge(edge.edge.edge_type.as_deref());
            Some((
                edge.key,
                GridPaintedEdge {
                    geometry: LineHopEdge {
                        id,
                        points: &entry.geom.data_points,
                        curve: Some(settings.curve),
                        arrow_type_start,
                        arrow_type_end,
                    },
                    original_path: &entry.geom.d,
                    corner_radius: Some(settings.corner_radius),
                },
            ))
        })
        .unzip();
    for (index, path) in grid_line_hop_overrides(settings.line_hops, &painted, work)? {
        work.charge_emit_work(path.len())?;
        if let Some(entry) = cache.get_mut(&owners[index]) {
            entry.geom.pb = svg_path_bounds_from_d(&path);
            entry.geom.grid_hopped_d = Some(path);
        }
    }
    Ok(())
}
