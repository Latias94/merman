//! Project graph geometry into the current SVG root's coordinate system exactly once.

use super::*;
use crate::graph_layout::{RouteAxes, contour, ir::RoutePoint, route_paint};

pub(super) fn paint(
    request: FlowchartEdgePathGeomRequest<'_>,
    profiles: &[contour::ShapePortProfile],
    axes: &RouteAxes,
) -> Option<FlowchartEdgePathGeom> {
    let FlowchartEdgePathGeomRequest {
        ctx,
        key,
        edge,
        origin_x,
        origin_y,
        trace_enabled,
    } = request;
    let solved = ctx.layout_edges_by_key.get(&key)?;
    if axes.axes.len() != solved.points.len() || solved.points.len() < 2 {
        return None;
    }
    let mut points: Vec<_> = solved
        .points
        .iter()
        .zip(&axes.axes)
        .map(|(p, &axis)| RoutePoint {
            x: p.x + ctx.tx - origin_x,
            y: p.y + ctx.ty - origin_y,
            axis,
        })
        .collect();
    let center = |id: &str| {
        let bounds = boundary_for_node(ctx, id, origin_x, origin_y)?;
        Some(crate::model::LayoutPoint {
            x: bounds.x,
            y: bounds.y,
        })
    };
    contour::inset_endpoints(
        &mut points,
        (
            profiles.get(axes.endpoints.0)?,
            &center(&solved.from)?,
            axes.ports.0,
        ),
        (
            profiles.get(axes.endpoints.1)?,
            &center(&solved.to)?,
            axes.ports.1,
        ),
    );
    let style = route_paint::RouteStyle::from_interpolate(
        edge.interpolate
            .as_deref()
            .unwrap_or(ctx.default_edge_interpolate),
        ctx.edge_corner_radius,
    );
    let painted = route_paint::build_svg_path(
        &points,
        axes.ports,
        style,
        super::super::flowchart_marker_metrics(edge),
    )?;
    let data_points: Vec<_> = points
        .iter()
        .map(|p| crate::model::LayoutPoint { x: p.x, y: p.y })
        .collect();
    if trace_enabled {
        let base_points: Vec<_> = solved
            .points
            .iter()
            .map(|p| crate::model::LayoutPoint {
                x: p.x + ctx.tx - origin_x,
                y: p.y + ctx.ty - origin_y,
            })
            .collect();
        record_flowchart_edge_trace(FlowchartEdgeTraceInput {
            ctx,
            edge,
            layout_edge: solved,
            origin_x,
            origin_y,
            base_points: &base_points,
            points_after_intersect_for_trace: Some(&data_points),
            points_for_render: &data_points,
            points_for_data_points: &data_points,
        });
    }
    Some(FlowchartEdgePathGeom {
        d: painted.d,
        pb: Some(path_bounds::SvgPathBounds {
            min_x: painted.bounds.min_x,
            min_y: painted.bounds.min_y,
            max_x: painted.bounds.max_x,
            max_y: painted.bounds.max_y,
        }),
        data_points,
        pre_hop_dash_mask_length: None,
        line_hop_applied: false,
        original_label_path_points: Vec::new(),
        label_path_points: Vec::new(),
        label_anchor: solved.label.as_ref().map(|p| crate::model::LayoutPoint {
            x: p.x + ctx.tx - origin_x,
            y: p.y + ctx.ty - origin_y,
        }),
        label_path_was_explicitly_updated: false,
        emitted_d_for_label: None,
        bounds_skipped_for_viewbox: false,
    })
}
