//! Project graph geometry into the current SVG root's coordinate system exactly once.

use super::*;
use crate::graph_layout::{
    RouteTopology,
    axis_clearance::AxisClearance,
    contour, edge_paint_error,
    ir::{BorderStitch, Endpoint, RoutePoint},
    route_paint,
};
use rustc_hash::FxHashSet;
use std::borrow::Cow;

fn obstacles(ctx: &FlowchartRenderCtx<'_>) -> Vec<(kurbo::Rect, route_paint::ObstacleOwner)> {
    use route_paint::ObstacleOwner;

    let layout = ctx.layout;
    let mut boxes = Vec::with_capacity(layout.nodes.len() + layout.clusters.len());
    let label_box = |label: &crate::model::LayoutLabel| {
        kurbo::Rect::from_center_size((label.x, label.y), (label.width, label.height))
    };
    boxes.extend(layout.nodes.iter().enumerate().map(|(index, node)| {
        (
            kurbo::Rect::from_center_size((node.x, node.y), (node.width, node.height)),
            ObstacleOwner::Node(index as u32),
        )
    }));
    boxes.extend(layout.clusters.iter().enumerate().map(|(index, group)| {
        (
            label_box(&group.title_label),
            ObstacleOwner::GroupTitle(index as u32),
        )
    }));
    boxes.extend(
        super::super::types::FlowchartLayoutEdgeRef::in_layout_order(layout).filter_map(
            |(key, edge)| {
                edge.label().map(|label| {
                    (
                        label_box(label),
                        ObstacleOwner::EdgeLabel(key.semantic_index()),
                    )
                })
            },
        ),
    );
    if let crate::graph_layout::RouteContract::CanonicalAxis(axis) = &layout.route_contract {
        let mut marker_boxes = FxHashSet::default();
        for (key, edge) in super::super::types::FlowchartLayoutEdgeRef::in_layout_order(layout) {
            let index = key.semantic_index();
            let (Some(Some(route)), Some(source), [first, .., last]) = (
                axis.routes.get(index),
                ctx.model.edges.get(index),
                edge.points(),
            ) else {
                continue;
            };
            let marks = super::super::flowchart_marker_metrics(source);
            let Some((&(start_axis, start_kind), &(end_axis, end_kind))) =
                route.knots.first().zip(route.knots.last())
            else {
                continue;
            };
            for (endpoint, point, side, marker) in [
                (
                    route.endpoints.0,
                    RoutePoint {
                        x: first.x,
                        y: first.y,
                        axis: start_axis,
                        kind: start_kind,
                    },
                    route.ports.0,
                    marks.0,
                ),
                (
                    route.endpoints.1,
                    RoutePoint {
                        x: last.x,
                        y: last.y,
                        axis: end_axis,
                        kind: end_kind,
                    },
                    route.ports.1,
                    marks.1,
                ),
            ] {
                if let Some((rect, owner @ ObstacleOwner::Marker(_, contact))) =
                    route_paint::marker_obstacle(index, endpoint, point, side, marker)
                {
                    // Every owning edge queries with this exact contact, so one copy of an
                    // identical footprint preserves both shared and foreign marker avoidance.
                    let bounds = [rect.x0, rect.y0, rect.x1, rect.y1].map(f64::to_bits);
                    if marker_boxes.insert((contact, bounds)) {
                        boxes.push((rect, owner));
                    }
                }
            }
        }
    }
    boxes
}

pub(super) fn paint(
    request: FlowchartEdgePathGeomRequest<'_>,
    profiles: &[contour::ShapePortProfile],
    route: &RouteTopology,
    stitches: &[BorderStitch],
    clearance: &AxisClearance,
    scratch: &mut FlowchartEdgeDataPointsScratch,
) -> Result<FlowchartEdgePathGeom> {
    let FlowchartEdgePathGeomRequest {
        ctx,
        key,
        edge,
        origin_x,
        origin_y,
        trace_enabled,
    } = request;
    let invalid = || edge_paint_error(&edge.id, route_paint::RoutePaintError::InvalidGeometry);
    let solved = ctx.layout_edges_by_key.get(&key).ok_or_else(invalid)?;
    let solved_points = solved.points();
    if route.knots.len() != solved_points.len() || solved_points.len() < 2 {
        return Err(invalid());
    }
    let points = &mut scratch.canonical_points;
    points.clear();
    points.reserve_exact(solved_points.len());
    points.extend(
        solved_points
            .iter()
            .zip(&route.knots)
            .map(|(point, &(axis, kind))| RoutePoint {
                x: point.x + ctx.tx - origin_x,
                y: point.y + ctx.ty - origin_y,
                axis,
                kind,
            }),
    );
    let center = |x, y| crate::model::LayoutPoint {
        x: x + ctx.tx - origin_x,
        y: y + ctx.ty - origin_y,
    };
    let group_contour = |id: u32| {
        let group = ctx.layout.clusters.get(id as usize)?;
        let radius = super::super::cluster_geometry::cluster_container_radius(
            ctx,
            group.width,
            group.height,
        );
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
    let box_contacts = (points[0], points[last]);
    let (source_profile, source_center, source_lead) =
        terminal(route.endpoints.0, route.ports.0, points[0], points[1]).ok_or_else(invalid)?;
    let (target_profile, target_center, target_lead) = terminal(
        route.endpoints.1,
        route.ports.1,
        points[last],
        points[last - 1],
    )
    .ok_or_else(invalid)?;
    contour::inset_endpoints(
        points,
        (&source_profile, &source_center, route.ports.0),
        (&target_profile, &target_center, route.ports.1),
    );
    for stitch in stitches {
        let (profile, center) = group_contour(stitch.group).ok_or_else(invalid)?;
        let port = points.get_mut(stitch.point).ok_or_else(invalid)?;
        *port = profile.contact(&center, stitch.side, *port);
    }
    let style = route_paint::RouteStyle::from_interpolate(
        edge.interpolate
            .as_deref()
            .unwrap_or(ctx.default_edge_interpolate),
        ctx.edge_corner_radius,
    );
    let stroke_outset = ctx.edge_style_plan.stroke_outset_for(key)?;
    let clearance = if style == route_paint::RouteStyle::Curved
        && stroke_outset.is_some_and(|outset| route.corridor_clearance > outset)
        && (points[0], points[last]) == box_contacts
        && stitches.is_empty()
    {
        None
    } else {
        let layout_contact = |index: usize| RoutePoint {
            x: solved_points[index].x,
            y: solved_points[index].y,
            ..points[index]
        };
        Some(
            clearance
                .resolve_with(ctx.work_meter, || obstacles(ctx))?
                .for_edge(
                    key.semantic_index(),
                    route.endpoints,
                    kurbo::Vec2::new(origin_x - ctx.tx, origin_y - ctx.ty),
                    stroke_outset.unwrap_or_default(),
                )
                .with_terminal_contacts((layout_contact(0), layout_contact(last))),
        )
    };
    if ctx.compatibility.look.is_neo() && !ctx.edge_style_plan.animation_for(key)?.is_active() {
        scratch.canonical_paint.measure_length();
    }
    let painted = route_paint::build_svg_path_with_scratch(
        points,
        (source_lead, target_lead),
        style,
        super::super::flowchart_marker_metrics(edge),
        route_paint::CurveTerminals::Smooth,
        stitches,
        clearance,
        &mut scratch.canonical_paint,
    )
    .map_err(|error| edge_paint_error(&edge.id, error))?;
    let data_points: Vec<_> = points
        .iter()
        .map(|p| crate::model::LayoutPoint { x: p.x, y: p.y })
        .collect();
    if trace_enabled {
        let base_points: Vec<_> = solved_points
            .iter()
            .map(|p| crate::model::LayoutPoint {
                x: p.x + ctx.tx - origin_x,
                y: p.y + ctx.ty - origin_y,
            })
            .collect();
        record_flowchart_edge_trace(FlowchartEdgeTraceInput {
            ctx,
            edge,
            layout_edge: solved.materialized(),
            origin_x,
            origin_y,
            base_points: &base_points,
            points_after_intersect_for_trace: Some(&data_points),
            points_for_render: &data_points,
            points_for_data_points: &data_points,
        });
    }
    Ok(FlowchartEdgePathGeom {
        d: painted.d,
        neo_length: scratch
            .canonical_paint
            .measured_length()
            .map(|length| f64::from(length as f32))
            .filter(|length| length.is_finite()),
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
        label_anchor: solved.label().map(|p| crate::model::LayoutPoint {
            x: p.x + ctx.tx - origin_x,
            y: p.y + ctx.ty - origin_y,
        }),
        label_path_was_explicitly_updated: false,
        emitted_d_for_label: None,
        bounds_skipped_for_viewbox: false,
    })
}
