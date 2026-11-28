//! Usecase contour clipping and axis-tagged Layered shafts.

use super::super::{
    UsecaseCssBinding, UsecaseEdgePlan, UsecasePrepared,
    layered::{Routes, allocation_error, buffer, invalid_projection},
};
use super::*;
use kurbo::{Point, Size};
use merman_model::OperationPhase;
use merman_render_core::{
    LayoutPoint,
    rough_svg::paths::roughjs_hand_drawn_stroke_path_for_svg_path,
    route_types::RoutePoint,
    svg_bounds::svg_path_bounds_from_d,
    svg_edge_parity::{
        layered_route::{self, Curve},
        path_numbers::json_stringify_axis_points_into,
        svg_path_length_from_d,
    },
};

pub(super) struct PaintedEdge {
    pub(super) json: String,
    pub(super) path: String,
    points: Vec<LayoutPoint>,
    length: Option<f64>,
    hopped: bool,
}

pub(super) fn paint_edges(
    prepared: &UsecasePrepared,
    routes: &Routes,
    config: &serde_json::Value,
    nodes: &HashMap<&str, &LayoutNode>,
    plans: &HashMap<&str, &UsecaseNodePlan>,
    bounds: &mut Bounds,
    options: &UsecaseSvgContext<'_>,
) -> Result<Vec<PaintedEdge>> {
    let work = options.work_meter;
    let css = &prepared.measured.css;
    let hand_drawn = css.edge_look == "handDrawn";
    let sketch = css.sketches_edges();
    let randomness = (hand_drawn || sketch)
        .then(|| operation_randomness(options.operation, css.hand_drawn_seed, "usecase-edge"));
    let edges = &prepared.layout().edges;
    if routes.edges.len() != edges.len() || edges.len() != prepared.measured.edges.len() {
        return Err(invalid_projection("route and paint edge slots differ"));
    }
    work.charge_emit_work(edges.len())?;
    let mut painted = buffer(edges.len())?;
    let mut route_points = Vec::new();
    let mut axes = Vec::new();
    let mut number = ryu_js::Buffer::new();
    for ((route, edge), plan) in routes.edges.iter().zip(edges).zip(&prepared.measured.edges) {
        work.charge_emit_work(route.points.len())?;
        route_points.clear();
        route_points
            .try_reserve(route.points.len())
            .map_err(|_| allocation_error())?;
        route_points.extend_from_slice(&route.points);
        let last = route_points
            .len()
            .checked_sub(1)
            .filter(|&last| last > 0)
            .ok_or_else(|| invalid_projection("missing shaft terminal"))?;
        for (index, id) in [(0, edge.from.as_str()), (last, edge.to.as_str())] {
            if let Some(outline) = plans[id].layered_ellipse {
                route_points[index] = clip_ellipse(route_points[index], nodes[id], outline, work)?;
            }
        }
        let mut points = buffer(route_points.len())?;
        axes.clear();
        axes.try_reserve(route_points.len())
            .map_err(|_| allocation_error())?;
        for point in &route_points {
            points.push(LayoutPoint {
                x: point.x,
                y: point.y,
            });
            axes.push(point.axis);
        }
        let mut json = String::new();
        json_stringify_axis_points_into(&mut json, &points, &axes, &mut number);
        work.charge_emit_work(json.len())?;
        let mut path = layered_route::path(
            &route_points,
            routes.curve,
            layered_route::marker_trim(plan.start_marker.as_deref()),
            layered_route::marker_trim(plan.end_marker.as_deref()),
            work,
        )?;
        if let Some(randomness) = &randomness {
            if sketch {
                path = constrained_rough_edge_path(&path, Default::default(), work, randomness)?;
            } else if let Some(drawn) =
                roughjs_hand_drawn_stroke_path_for_svg_path(&path, 0.3, work, randomness)
            {
                path = drawn;
            } else {
                work.checkpoint(OperationPhase::Emit)?;
            }
        }
        work.charge_emit_work(path.len())?;
        let length = (css.edge_look == "neo" && !plan.animated && randomness.is_none())
            .then(|| svg_path_length_from_d(&path))
            .flatten();
        painted.push(PaintedEdge {
            json,
            path,
            points,
            length,
            hopped: false,
        });
    }

    if !sketch
        && (!hand_drawn || routes.curve != Curve::Curved)
        && let Some(style) = layered_route::line_hop_style(config.pointer("/layered/lineHops"))
    {
        work.charge_emit_work(edges.len())?;
        let mut inputs = buffer(edges.len())?;
        inputs.extend(
            edges
                .iter()
                .zip(&painted)
                .map(|(edge, item)| line_hops::LineHopEdge {
                    id: &edge.id,
                    points: &item.points,
                    curve: Some(routes.curve.as_str()),
                    arrow_type_start: edge.start_marker.as_deref(),
                    arrow_type_end: edge.end_marker.as_deref(),
                }),
        );
        for (index, path) in layered_route::line_hops(&inputs, style, work)? {
            work.charge_emit_work(path.len())?;
            painted[index].path = path;
            painted[index].hopped = true;
        }
    }
    for (edge, paint) in edges.iter().zip(&painted) {
        work.charge_emit_work(paint.path.len())?;
        if let Some(path) = svg_path_bounds_from_d(&paint.path) {
            bounds.min_x = bounds.min_x.min(path.min_x);
            bounds.min_y = bounds.min_y.min(path.min_y);
            bounds.max_x = bounds.max_x.max(path.max_x);
            bounds.max_y = bounds.max_y.max(path.max_y);
        }
        if let Some(label) = &edge.label {
            let y = label.y + routes.label_y_offset;
            bounds.min_y = bounds.min_y.min(y - label.height / 2.0);
            bounds.max_y = bounds.max_y.max(y + label.height / 2.0);
        }
    }
    work.charge_emit_work(prepared.measured.nodes.len())?;
    for plan in prepared.measured.nodes.iter().filter(|plan| plan.package) {
        let node = nodes[plan.id.as_str()];
        let height = boundary_height(node, plan);
        bounds.min_y = bounds.min_y.min(node.y - height / 2.0);
        bounds.max_y = bounds.max_y.max(node.y + height / 2.0);
    }
    Ok(painted)
}

pub(super) fn boundary_height(node: &LayoutNode, plan: &UsecaseNodePlan) -> f64 {
    if plan.package {
        node.height
            .max(super::super::package_tab_size(&plan.label).1 + 40.0)
    } else {
        node.height
    }
}

pub(super) fn write_marker_variants(
    out: &mut impl SvgOutput,
    diagram_id: SvgId<'_>,
    css: &UsecaseCssBinding,
    edges: &[UsecaseEdgePlan],
    work: &OperationWorkMeter,
) -> Result<Vec<[Option<String>; 2]>> {
    use BaseEdgeMarkerKind::*;
    use merman_render_core::mermaid_style::is_safe_browser_css_color_value;
    use std::collections::HashSet;

    work.charge_emit_work(edges.len())?;
    let mut references = buffer(edges.len())?;
    let mut written = HashSet::new();
    for edge in edges {
        let mut sides = [None, None];
        if let Some(stroke) = edge
            .layered_marker_stroke
            .as_deref()
            .filter(|s| !s.trim().is_empty())
        {
            work.charge_emit_work(stroke.len())?;
            let color_id: String = stroke
                .encode_utf16()
                .map(|unit| {
                    u8::try_from(unit)
                        .ok()
                        .filter(u8::is_ascii_alphanumeric)
                        .map_or('_', char::from)
                })
                .collect();
            let safe_stroke = is_safe_browser_css_color_value(stroke).then_some(stroke);
            let margin = css.edge_look == "neo" && !edge.animated;
            for (start, arrow) in [
                (true, edge.start_marker.as_deref()),
                (false, edge.end_marker.as_deref()),
            ] {
                let (base, extension) = match (arrow, start, margin) {
                    (Some("extension"), true, false) => ("extensionStart", true),
                    (Some("extension"), false, false) => ("extensionEnd", true),
                    (Some("extension"), true, true) => ("extensionStart-margin", true),
                    (Some("extension"), false, true) => ("extensionEnd-margin", true),
                    (None, _, _) => continue,
                    _ => ("", false),
                };
                let marker = if extension {
                    None
                } else {
                    let kind = match (BaseEdgeMarkerKind::from_arrow(arrow, start), margin) {
                        (Some(PointEnd), true) => PointEndMargin,
                        (Some(PointStart), true) => PointStartMargin,
                        (Some(CircleEnd), true) => CircleEndMargin,
                        (Some(CircleStart), true) => CircleStartMargin,
                        (Some(CrossEnd), true) => CrossEndMargin,
                        (Some(CrossStart), true) => CrossStartMargin,
                        (Some(kind), _) => kind,
                        (None, _) => continue,
                    };
                    Some(kind)
                };
                let base = marker.map_or(base, BaseEdgeMarkerKind::suffix);
                let suffix = format!("{base}_{color_id}");
                if !written.contains(&suffix) {
                    written.try_reserve(1).map_err(|_| allocation_error())?;
                    if written.is_empty() {
                        out.push_str("<defs>");
                    }
                    if let Some(kind) = marker {
                        write_base_marker(
                            out,
                            diagram_id,
                            css.marker_style,
                            kind,
                            &suffix,
                            safe_stroke,
                        );
                    } else {
                        write_extension_marker(
                            out,
                            diagram_id,
                            &suffix,
                            start,
                            margin,
                            safe_stroke,
                        );
                    }
                    out.checkpoint()?;
                    written.insert(suffix.clone());
                }
                sides[usize::from(!start)] = Some(suffix);
            }
        }
        references.push(sides);
    }
    if !written.is_empty() {
        out.push_str("</defs>");
    }
    out.checkpoint()?;
    Ok(references)
}

pub(super) fn path_style(
    plan: &UsecaseEdgePlan,
    paint: &PaintedEdge,
    work: &OperationWorkMeter,
) -> Result<String> {
    work.charge_emit_work(plan.source_style.len().max(1))?;
    let mut style = String::new();
    if let Some(length) = paint.length {
        let units = if plan.dotted {
            (length / 4.0).max(0.0) as usize
        } else {
            1
        };
        work.charge_emit_work(
            units
                .checked_mul(4)
                .ok_or_else(|| work.arithmetic_overflow())?,
        )?;
        edge_path::write_neo_edge_mask(
            &mut style,
            length,
            plan.start_marker.as_deref(),
            plan.end_marker.as_deref(),
            plan.dotted,
            false,
        );
    } else if plan.dotted {
        style.push_str("stroke-dasharray:3;");
    }
    style.push_str(&plan.source_style);
    if paint.hopped {
        Ok(line_hops::rewrite_style_after_line_hop_with_phase(
            &style,
            &paint.path,
            OperationPhase::Emit,
            work,
        )?
        .into_owned())
    } else {
        Ok(style)
    }
}

fn clip_ellipse(
    point: RoutePoint,
    node: &LayoutNode,
    outline: Size,
    work: &OperationWorkMeter,
) -> Result<RoutePoint> {
    let center = Point::new(node.x, node.y);
    let size = Size::new(node.width, node.height);
    let (rx, ry) = (outline.width / 2.0, outline.height / 2.0);
    let (rx2, ry2) = (rx * rx, ry * ry);
    layered_route::clip_endpoint(
        point,
        center,
        size,
        |target, origin| {
            if let Some(origin) = origin {
                let offset = origin - center;
                let delta = target - origin;
                let a = delta.x * delta.x / rx2 + delta.y * delta.y / ry2;
                let b = 2.0 * offset.x * delta.x / rx2 + 2.0 * offset.y * delta.y / ry2;
                let c = offset.x * offset.x / rx2 + offset.y * offset.y / ry2 - 1.0;
                let discriminant = b * b - 4.0 * a * c;
                if a > 0.0 && discriminant >= 0.0 {
                    let root = discriminant.sqrt();
                    let far = (-b + root) / (2.0 * a);
                    let near = (-b - root) / (2.0 * a);
                    if let Some(parameter) =
                        [far, near].into_iter().find(|t| (0.0..=1.0).contains(t))
                    {
                        return Ok(Some(origin + delta * parameter));
                    }
                }
            }
            let offset = target - center;
            let det = (rx2 * offset.y * offset.y + ry2 * offset.x * offset.x).sqrt();
            Ok((det > 0.0).then(|| center + offset * (rx * ry / det)))
        },
        work,
    )
}
