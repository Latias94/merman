//! Layered's common renderer uses the provider's axis-tagged routes and its label anchors.

use kurbo::{Point, Size};
use merman_model::{OperationPhase, families::flowchart::FlowEdge};
use merman_render_core::{
    LayoutPoint,
    route_types::RoutePoint,
    svg_edge_parity::{layered_route, line_hops::LineHopEdge},
};

use super::intersect::intersect_node_outline;
use super::*;
use crate::{
    layered::FlowLayeredRouting,
    projection::{FlowchartEdgeKey, FlowchartEdgeTransportPlan},
};

fn draw_curve(
    ctx: &FlowchartRenderCtx<'_>,
    edge: &FlowEdge,
    routing: FlowLayeredRouting,
) -> layered_route::Curve {
    layered_route::Curve::for_edge_routing(
        routing,
        edge.interpolate
            .as_deref()
            .unwrap_or(ctx.default_edge_interpolate),
    )
}

pub(super) fn paint(
    request: FlowchartEdgePathGeomRequest<'_>,
    routing: FlowLayeredRouting,
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
    let Some(&FlowchartLayoutEdgeRef::Layered {
        edge: layout,
        route,
    }) = ctx.layout_edges_by_key.get(&key)
    else {
        return Ok(None);
    };
    if route.points.len() < 2 {
        return Ok(None);
    }
    let tx = ctx.tx - origin_x;
    let ty = ctx.ty - origin_y;
    let points = &mut scratch.layered_points;
    ctx.work_meter.charge_emit_work(route.points.len())?;
    points.clear();
    points
        .try_reserve(route.points.len())
        .map_err(|_| Error::LayoutAllocationFailed { backend: "layered" })?;
    points.extend(route.points.iter().map(|point| RoutePoint {
        x: point.x + tx,
        y: point.y + ty,
        ..*point
    }));

    let last = points.len() - 1;
    for (index, id, is_cluster) in [
        (0, &layout.from, layout.from_cluster.is_some()),
        (last, &layout.to, layout.to_cluster.is_some()),
    ] {
        if is_cluster {
            continue;
        }
        let Some(boundary) = boundary_for_node(ctx, id, origin_x, origin_y) else {
            continue;
        };
        points[index] = layered_route::clip_endpoint(
            points[index],
            Point::new(boundary.x, boundary.y),
            Size::new(boundary.width, boundary.height),
            |adjacent, origin| {
                Ok(Some(intersect_node_outline(
                    ctx, id, &boundary, adjacent, origin,
                )))
            },
            ctx.work_meter,
        )?;
    }
    let arrows = visible_arrows(edge);
    let d = layered_route::path(
        points,
        draw_curve(ctx, edge, routing),
        layered_route::marker_trim(arrows.0),
        layered_route::marker_trim(arrows.1),
        ctx.work_meter,
    )?;
    let (data_points, axes): (Vec<_>, Vec<_>) = points
        .iter()
        .map(|point| {
            (
                LayoutPoint {
                    x: point.x,
                    y: point.y,
                },
                point.axis,
            )
        })
        .unzip();
    if trace_enabled {
        let original = route
            .points
            .iter()
            .map(|point| LayoutPoint {
                x: point.x + tx,
                y: point.y + ty,
            })
            .collect::<Vec<_>>();
        record_flowchart_edge_trace(FlowchartEdgeTraceInput {
            ctx,
            edge,
            layout_edge: Some(layout),
            origin_x,
            origin_y,
            base_points: &original,
            points_after_intersect_for_trace: Some(&data_points),
            points_for_render: &data_points,
            points_for_data_points: &data_points,
        });
    }
    let measure_length = measure_neo_length
        && !trace_enabled
        && ctx.compatibility.look.is_neo()
        && !ctx.edge_style_plan.animation_for(key)?.is_active();
    let neo_length = if measure_length {
        svg_path_length_from_d(&d)
    } else {
        None
    };
    ctx.work_meter.checkpoint(OperationPhase::Emit)?;
    Ok(Some(FlowchartEdgePathGeom {
        pb: svg_path_bounds_from_d(&d),
        d,
        neo_length,
        data_points,
        axes: Some(axes),
        pre_hop_dash_mask_length: None,
        line_hop_applied: false,
        after_paint_d: None,
        original_label_path_points: Vec::new(),
        label_path_points: Vec::new(),
        label_anchor: layout.label.as_ref().map(|label| LayoutPoint {
            x: label.x + tx,
            y: label.y + ty,
        }),
        label_path_was_explicitly_updated: false,
        emitted_d_for_label: None,
        bounds_skipped_for_viewbox: false,
    }))
}

fn visible_arrows(edge: &FlowEdge) -> (Option<&'static str>, Option<&'static str>) {
    if edge.stroke.as_deref() == Some("invisible") {
        (None, None)
    } else {
        arrow_types_for_edge(edge.edge_type.as_deref())
    }
}

pub(in crate::svg) fn apply_layered_line_hops(
    ctx: &FlowchartRenderCtx<'_>,
    cache: &mut FxHashMap<FlowchartEdgeKey, FlowchartEdgePathCacheEntry>,
    render_edges: &[crate::svg::render_input::FlowchartRenderEdge<'_>],
    routing: FlowLayeredRouting,
) -> Result<()> {
    let Some(style) = layered_route::line_hop_style(
        ctx.config
            .get("layered")
            .and_then(|section| section.get("lineHops")),
    ) else {
        return Ok(());
    };
    let work = ctx.work_meter;
    work.charge_emit_work(render_edges.len())?;
    let ids = FlowchartEdgeTransportPlan::for_keyed_edges(
        render_edges
            .iter()
            .map(|edge| (edge.key, edge.edge.as_ref())),
    );
    let (owners, edges): (Vec<_>, Vec<_>) = render_edges
        .iter()
        .filter_map(|edge| {
            let entry = cache.get(&edge.key)?;
            let (arrow_type_start, arrow_type_end) = visible_arrows(&edge.edge);
            Some((
                edge.key,
                LineHopEdge {
                    id: ids.id(edge.key)?,
                    points: &entry.geom.data_points,
                    curve: Some(draw_curve(ctx, &edge.edge, routing).as_str()),
                    arrow_type_start,
                    arrow_type_end,
                },
            ))
        })
        .unzip();
    for (index, path) in layered_route::line_hops(&edges, style, work)? {
        work.charge_emit_work(path.len())?;
        if let Some(entry) = cache.get_mut(&owners[index]) {
            entry.geom.pb = svg_path_bounds_from_d(&path);
            entry.geom.after_paint_d = Some(path);
        }
    }
    Ok(())
}
