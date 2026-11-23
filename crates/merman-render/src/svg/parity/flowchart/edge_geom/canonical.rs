//! Project graph geometry into the current SVG root's coordinate system exactly once.

use super::*;
use crate::graph_layout::{
    RouteAxes, contour,
    ir::{BorderStitch, Endpoint, RoutePoint},
    route_paint,
};
use std::borrow::Cow;

pub(super) fn paint(
    request: FlowchartEdgePathGeomRequest<'_>,
    profiles: &[contour::ShapePortProfile],
    axes: &RouteAxes,
    stitches: &[BorderStitch],
    clearance: &route_paint::Clearance,
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
    let center = |x, y| crate::model::LayoutPoint {
        x: x + ctx.tx - origin_x,
        y: y + ctx.ty - origin_y,
    };
    let group_contour = |id: u32| {
        let group = ctx.layout.clusters.get(id as usize)?;
        let radius = if ctx.diagram_type == "agentflow" {
            10.0
        } else {
            0.0
        };
        Some((
            contour::ShapePortProfile::rounded_rectangle(group.width, group.height, radius, radius),
            center(group.x, group.y),
        ))
    };
    let terminal = |endpoint, side, port, next| {
        Some(match endpoint {
            Endpoint::Node(id) => {
                let node = ctx.layout.nodes.get(id as usize)?;
                (
                    Cow::Borrowed(profiles.get(id as usize)?),
                    center(node.x, node.y),
                    side,
                )
            }
            Endpoint::Group(id) => {
                let (profile, center) = group_contour(id)?;
                (
                    Cow::Owned(profile),
                    center,
                    route_paint::group_lead_side(side, port, next),
                )
            }
        })
    };
    let last = points.len() - 1;
    let (source_profile, source_center, source_lead) =
        terminal(axes.endpoints.0, axes.ports.0, points[0], points[1])?;
    let (target_profile, target_center, target_lead) = terminal(
        axes.endpoints.1,
        axes.ports.1,
        points[last],
        points[last - 1],
    )?;
    contour::inset_endpoints(
        &mut points,
        (&source_profile, &source_center, axes.ports.0),
        (&target_profile, &target_center, axes.ports.1),
    );
    for stitch in stitches {
        let (profile, center) = group_contour(stitch.group)?;
        let port = points.get_mut(stitch.point)?;
        *port = profile.contact(&center, stitch.side, *port);
    }
    let style = route_paint::RouteStyle::from_interpolate(
        edge.interpolate
            .as_deref()
            .unwrap_or(ctx.default_edge_interpolate),
        ctx.edge_corner_radius,
    );
    let painted = route_paint::build_svg_path(
        &points,
        (source_lead, target_lead),
        style,
        super::super::flowchart_marker_metrics(edge),
        stitches,
        Some(
            clearance.for_edge(
                key.semantic_index(),
                axes.endpoints,
                kurbo::Vec2::new(origin_x - ctx.tx, origin_y - ctx.ty),
                ctx.edge_style_plan
                    .stroke_width_for(key)
                    .ok()
                    .and_then(|stroke| stroke.paint_outset())
                    .unwrap_or_default(),
            ),
        ),
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
