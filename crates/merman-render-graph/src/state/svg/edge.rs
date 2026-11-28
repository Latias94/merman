use super::*;
use base64::Engine as _;
#[cfg(feature = "layout-shoal")]
use merman_render_core::svg_bounds::svg_path_bounds_from_d;
#[cfg(test)]
use merman_render_core::svg_edge_parity::curve::curve_basis_path_d;
#[cfg(feature = "layout-shoal")]
use merman_render_core::svg_output::cutout;
use merman_render_core::{
    svg_edge_parity::{
        curve::interpolated_path_d,
        edge_label_geometry::position_edge_label,
        edge_path::{render_path, write_neo_edge_mask},
        line_hops::rewrite_style_after_line_hop,
        svg_path_length_from_d,
    },
    svg_output::{FALLBACK_BACKGROUND_FILL_DATA_ATTR, FALLBACK_OCCURRENCE_DATA_ATTR},
    text::{MERMAID_CREATE_TEXT_DEFAULT_WIDTH_PX, line_break::mermaid_html_breaks_to_newlines},
    theme_svg::edge_label_background::{adaptive_text_ink, literal_fill, write_backplate},
    xml::escape_attr_into,
};
use merman_theme_contract::v2::{Color, EdgeLabelBackground};

#[derive(Debug, Clone, Copy)]
struct StateEdgeBoundaryNode {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

fn state_edge_outside_node(node: &StateEdgeBoundaryNode, point: &LayoutPoint) -> bool {
    let dx = (point.x - node.x).abs();
    let dy = (point.y - node.y).abs();
    let w = node.width / 2.0;
    let h = node.height / 2.0;
    dx >= w || dy >= h
}

fn state_edge_rect_intersection(
    node: &StateEdgeBoundaryNode,
    inside_point: &LayoutPoint,
    outside_point: &LayoutPoint,
) -> LayoutPoint {
    let x = node.x;
    let y = node.y;
    let w = node.width / 2.0;
    let h = node.height / 2.0;

    let q_abs = (outside_point.y - inside_point.y).abs();
    let r_abs = (outside_point.x - inside_point.x).abs();

    if (y - outside_point.y).abs() * w > (x - outside_point.x).abs() * h {
        let q = if inside_point.y < outside_point.y {
            outside_point.y - h - y
        } else {
            y - h - outside_point.y
        };
        let r = if q_abs == 0.0 {
            0.0
        } else {
            (r_abs * q) / q_abs
        };
        let mut res = LayoutPoint {
            x: if inside_point.x < outside_point.x {
                inside_point.x + r
            } else {
                inside_point.x - r_abs + r
            },
            y: if inside_point.y < outside_point.y {
                inside_point.y + q_abs - q
            } else {
                inside_point.y - q_abs + q
            },
        };

        if r.abs() <= 1e-9 {
            res.x = outside_point.x;
            res.y = outside_point.y;
        }
        if r_abs == 0.0 {
            res.x = outside_point.x;
        }
        if q_abs == 0.0 {
            res.y = outside_point.y;
        }
        return res;
    }

    let r = if inside_point.x < outside_point.x {
        outside_point.x - w - x
    } else {
        x - w - outside_point.x
    };
    let q = if r_abs == 0.0 {
        0.0
    } else {
        (q_abs * r) / r_abs
    };
    let mut ix = if inside_point.x < outside_point.x {
        inside_point.x + r_abs - r
    } else {
        inside_point.x - r_abs + r
    };
    let mut iy = if inside_point.y < outside_point.y {
        inside_point.y + q
    } else {
        inside_point.y - q
    };

    if r.abs() <= 1e-9 {
        ix = outside_point.x;
        iy = outside_point.y;
    }
    if r_abs == 0.0 {
        ix = outside_point.x;
    }
    if q_abs == 0.0 {
        iy = outside_point.y;
    }

    LayoutPoint { x: ix, y: iy }
}

fn state_edge_cut_path_at_intersect(
    input: &[LayoutPoint],
    boundary: &StateEdgeBoundaryNode,
) -> Vec<LayoutPoint> {
    if input.is_empty() {
        return Vec::new();
    }
    let mut out: Vec<LayoutPoint> = Vec::new();
    let mut last_point_outside = input[0].clone();
    let mut is_inside = false;
    const EPS: f64 = 1e-9;

    for point in input {
        if !state_edge_outside_node(boundary, point) && !is_inside {
            // Mermaid's dagre-wrapper cuts an edge as it *enters* a cluster boundary.
            // `state_edge_rect_intersection` expects the point *inside* the rectangle first.
            let inter = state_edge_rect_intersection(boundary, point, &last_point_outside);
            if !out
                .iter()
                .any(|p| (p.x - inter.x).abs() <= EPS && (p.y - inter.y).abs() <= EPS)
            {
                out.push(inter);
            }
            is_inside = true;
        } else {
            last_point_outside = point.clone();
            if !is_inside {
                out.push(point.clone());
            }
        }
    }
    out
}

fn state_edge_boundary_for_cluster(
    ctx: &StateRenderCtx<'_>,
    cluster_id: &str,
    ox: f64,
    oy: f64,
) -> Option<StateEdgeBoundaryNode> {
    let mut resolved = cluster_id;
    if !ctx.layout_clusters_by_id.contains_key(resolved) {
        // Mermaid's state diagram edges sometimes annotate cluster endpoints as `state-<id>-<n>`
        // while the cluster itself is keyed by `<id>`.
        if let Some(rest) = resolved.strip_prefix("state-")
            && let Some((base, suffix)) = rest.rsplit_once('-')
            && !base.is_empty()
            && !suffix.is_empty()
            && suffix.bytes().all(|b| b.is_ascii_digit())
        {
            resolved = base;
        }
    }

    let n = ctx.layout_clusters_by_id.get(resolved).copied()?;
    Some(StateEdgeBoundaryNode {
        x: n.x - ox,
        y: n.y - oy,
        width: n.width,
        height: n.height,
    })
}

fn state_edge_boundary_for_layout_node(
    ctx: &StateRenderCtx<'_>,
    node_id: &str,
    ox: f64,
    oy: f64,
) -> Option<StateEdgeBoundaryNode> {
    let n = ctx.layout_nodes_by_id.get(node_id).copied()?;
    Some(StateEdgeBoundaryNode {
        x: n.x - ox,
        y: n.y - oy,
        width: n.width,
        height: n.height,
    })
}

fn state_edge_clip_self_loop_points_to_node(
    ctx: &StateRenderCtx<'_>,
    le: &LayoutEdge,
    input: &[LayoutPoint],
    origin_x: f64,
    origin_y: f64,
) -> Option<Vec<LayoutPoint>> {
    if le.from != le.to || input.len() < 3 {
        return None;
    }

    // Mermaid clips only when insertEdge receives endpoint nodes with an intersect callback.
    // A direct self-loop on a composite without cluster endpoint metadata receives the cluster
    // itself, so its Dagre points must remain intact. Explicit cluster endpoints still carry the
    // helper nodes that own the intersection callbacks.
    if le.from_cluster.is_none()
        && le.to_cluster.is_none()
        && (ctx.layout_clusters_by_id.contains_key(le.from.as_str())
            || ctx
                .nodes_by_id
                .get(le.from.as_str())
                .copied()
                .is_some_and(|node| node.is_group && node.shape != "noteGroup"))
    {
        return None;
    }

    let boundary = state_edge_boundary_for_layout_node(ctx, le.from.as_str(), origin_x, origin_y)?;
    let center = LayoutPoint {
        x: boundary.x,
        y: boundary.y,
    };
    let inner = &input[1..input.len() - 1];
    if inner.is_empty() {
        return None;
    }

    let mut out = Vec::with_capacity(inner.len() + 2);
    out.push(state_edge_rect_intersection(&boundary, &center, &inner[0]));
    out.extend(inner.iter().cloned());
    out.push(state_edge_rect_intersection(
        &boundary,
        &center,
        &inner[inner.len() - 1],
    ));
    Some(out)
}

fn state_edge_find_adjacent_point(
    point_a: &LayoutPoint,
    point_b: &LayoutPoint,
    distance: f64,
) -> LayoutPoint {
    let x_diff = point_b.x - point_a.x;
    let y_diff = point_b.y - point_a.y;
    let length = (x_diff * x_diff + y_diff * y_diff).sqrt();
    let ratio = distance / length;
    LayoutPoint {
        x: point_b.x - ratio * x_diff,
        y: point_b.y - ratio * y_diff,
    }
}

fn state_edge_is_corner_point(prev: &LayoutPoint, curr: &LayoutPoint, next: &LayoutPoint) -> bool {
    (prev.x == curr.x
        && curr.y == next.y
        && (curr.x - next.x).abs() > 5.0
        && (curr.y - prev.y).abs() > 5.0)
        || (prev.y == curr.y
            && curr.x == next.x
            && (curr.x - prev.x).abs() > 5.0
            && (curr.y - next.y).abs() > 5.0)
}

fn state_edge_fix_corners(line_data: &[LayoutPoint]) -> Vec<LayoutPoint> {
    if line_data.len() < 3 {
        return line_data.to_vec();
    }

    let mut out = Vec::with_capacity(line_data.len());
    for (idx, point) in line_data.iter().enumerate() {
        let is_corner = idx > 0
            && idx + 1 < line_data.len()
            && state_edge_is_corner_point(&line_data[idx - 1], point, &line_data[idx + 1]);

        if !is_corner {
            out.push(point.clone());
            continue;
        }

        let prev_point = &line_data[idx - 1];
        let next_point = &line_data[idx + 1];
        let corner_point = point;
        let new_prev_point = state_edge_find_adjacent_point(prev_point, corner_point, 5.0);
        let new_next_point = state_edge_find_adjacent_point(next_point, corner_point, 5.0);
        let x_diff = new_next_point.x - new_prev_point.x;
        let y_diff = new_next_point.y - new_prev_point.y;
        out.push(new_prev_point.clone());

        let mut new_corner_point = corner_point.clone();
        if (next_point.x - prev_point.x).abs() > 10.0 && (next_point.y - prev_point.y).abs() >= 10.0
        {
            let a = std::f64::consts::SQRT_2 * 2.0;
            let r = 5.0;
            if corner_point.x == new_prev_point.x {
                new_corner_point = LayoutPoint {
                    x: if x_diff < 0.0 {
                        new_prev_point.x - r + a
                    } else {
                        new_prev_point.x + r - a
                    },
                    y: if y_diff < 0.0 {
                        new_prev_point.y - a
                    } else {
                        new_prev_point.y + a
                    },
                };
            } else {
                new_corner_point = LayoutPoint {
                    x: if x_diff < 0.0 {
                        new_prev_point.x - a
                    } else {
                        new_prev_point.x + a
                    },
                    y: if y_diff < 0.0 {
                        new_prev_point.y - r + a
                    } else {
                        new_prev_point.y + r - a
                    },
                };
            }
        }

        out.push(new_corner_point);
        out.push(new_next_point);
    }
    out
}

fn state_marker_offset_for(arrow_type_end: Option<&str>) -> Option<f64> {
    match arrow_type_end {
        Some("arrow_barb_neo") => Some(5.5),
        _ => None,
    }
}

pub(super) fn state_rounded_end_marker_offset_points(
    input: &[LayoutPoint],
    arrow_type_end: Option<&str>,
) -> Vec<LayoutPoint> {
    let Some(offset) = state_marker_offset_for(arrow_type_end) else {
        return input.to_vec();
    };
    if input.len() < 2 {
        return input.to_vec();
    }

    // Mermaid's rounded-edge branch applies markerOffsets to the terminal point
    // before generating the rounded path. It moves only that point; the linear
    // branch below additionally handles short terminal stubs.
    let mut out = input.to_vec();
    let end = &input[input.len() - 1];
    let previous = &input[input.len() - 2];
    let angle = (end.y - previous.y).atan2(end.x - previous.x);
    out[input.len() - 1] = LayoutPoint {
        x: end.x - offset * angle.cos(),
        y: end.y - offset * angle.sin(),
    };
    out
}

pub(super) fn state_line_with_end_marker_offset_points(
    input: &[LayoutPoint],
    arrow_type_end: Option<&str>,
) -> Vec<LayoutPoint> {
    fn calculate_delta_and_angle(a: &LayoutPoint, b: &LayoutPoint) -> (f64, f64, f64) {
        let delta_x = b.x - a.x;
        let delta_y = b.y - a.y;
        let angle = (delta_y / delta_x).atan();
        (angle, delta_x, delta_y)
    }

    let Some(end_marker_height) = state_marker_offset_for(arrow_type_end) else {
        return input.to_vec();
    };
    if input.len() < 2 {
        return input.to_vec();
    }

    let start = &input[0];
    let end = &input[input.len() - 1];
    let x_direction_is_left = start.x < end.x;
    let y_direction_is_down = start.y < end.y;
    let extra_room = 1.0;

    let mut out = Vec::with_capacity(input.len());
    for (idx, point) in input.iter().enumerate() {
        let mut offset_x = 0.0;
        let mut offset_y = 0.0;

        if idx == input.len() - 1 {
            let (angle, delta_x, delta_y) =
                calculate_delta_and_angle(&input[input.len() - 1], &input[input.len() - 2]);
            offset_x = end_marker_height * angle.cos() * if delta_x >= 0.0 { 1.0 } else { -1.0 };
            offset_y =
                end_marker_height * angle.sin().abs() * if delta_y >= 0.0 { 1.0 } else { -1.0 };
        }

        let diff_x = (point.x - end.x).abs();
        let diff_y = (point.y - end.y).abs();
        if diff_x < end_marker_height && diff_x > 0.0 && diff_y < end_marker_height {
            let mut adjustment = end_marker_height + extra_room - diff_x;
            adjustment *= if !x_direction_is_left { -1.0 } else { 1.0 };
            offset_x -= adjustment;
        }
        if diff_y < end_marker_height && diff_y > 0.0 && diff_x < end_marker_height {
            let mut adjustment = end_marker_height + extra_room - diff_y;
            adjustment *= if !y_direction_is_down { -1.0 } else { 1.0 };
            offset_y -= adjustment;
        }

        out.push(LayoutPoint {
            x: point.x + offset_x,
            y: point.y + offset_y,
        });
    }

    out
}

pub(super) struct StatePreparedEdgeGeometry {
    original_label_path_points: Vec<LayoutPoint>,
    pub(super) data_points: Vec<LayoutPoint>,
    label_path_points: Vec<LayoutPoint>,
    rendered_d: String,
    points_were_explicitly_updated: bool,
}

fn state_edge_finish_geometry(
    original_label_path_points: Vec<LayoutPoint>,
    data_points: Vec<LayoutPoint>,
    label_path_points: Vec<LayoutPoint>,
    arrow_type_end: Option<&str>,
    curve: &str,
    points_were_explicitly_updated: bool,
) -> StatePreparedEdgeGeometry {
    // Mermaid keeps `paths.updatedPath` before `fixCorners`, marker offsets, and curve encoding.
    // Only the rendered SVG path consumes these projected curve points.
    let mut points_for_curve = label_path_points
        .iter()
        .filter(|point| !point.y.is_nan())
        .cloned()
        .collect::<Vec<_>>();
    if curve == "basis" {
        points_for_curve = state_edge_fix_corners(&points_for_curve);
    }
    points_for_curve = state_line_with_end_marker_offset_points(&points_for_curve, arrow_type_end);

    StatePreparedEdgeGeometry {
        original_label_path_points,
        data_points,
        label_path_points,
        rendered_d: interpolated_path_d(&points_for_curve, curve),
        points_were_explicitly_updated,
    }
}

pub(super) fn state_elk_edge_curve(ctx: &StateRenderCtx<'_>, edge: &LayoutEdge) -> &'static str {
    ctx.style_plan
        .compatibility()
        .native_edge_curve()
        .unwrap_or(if edge.points.is_empty() {
            "linear"
        } else {
            "rounded"
        })
}

pub(super) fn state_edge_prepare_geometry(
    ctx: &StateRenderCtx<'_>,
    le: &LayoutEdge,
    arrow_type_end: Option<&str>,
    origin_x: f64,
    origin_y: f64,
) -> StatePreparedEdgeGeometry {
    if ctx.uses_elk_adapter_dom {
        let points = ctx
            .elk_edge_paths
            .get(&le.id)
            .map(Vec::as_slice)
            .unwrap_or(&le.points);
        let data_points: Vec<_> = points
            .iter()
            .map(|point| LayoutPoint {
                x: point.x - origin_x,
                y: point.y - origin_y,
            })
            .collect();
        let curve = state_elk_edge_curve(ctx, le);
        let mut curve_points = data_points.clone();
        let marker_end = if curve == "rounded" {
            curve_points = state_rounded_end_marker_offset_points(&curve_points, arrow_type_end);
            None
        } else {
            arrow_type_end
        };
        let rendered_d = render_path(&mut curve_points, curve, None, marker_end);
        return StatePreparedEdgeGeometry {
            original_label_path_points: data_points.clone(),
            label_path_points: data_points.clone(),
            data_points,
            rendered_d,
            points_were_explicitly_updated: false,
        };
    }
    let mut raw_local_points: Vec<LayoutPoint> = Vec::new();
    for p in &le.points {
        raw_local_points.push(LayoutPoint {
            x: p.x - origin_x,
            y: p.y - origin_y,
        });
    }
    // `data-points` is captured immediately after endpoint clipping. A later cluster cut changes
    // the label path and rendered `d`, but deliberately does not rewrite this attribute.
    let data_points =
        state_edge_clip_self_loop_points_to_node(ctx, le, &raw_local_points, origin_x, origin_y)
            .unwrap_or_else(|| raw_local_points.clone());
    let mut label_path_points = data_points.clone();
    let mut points_were_explicitly_updated = false;

    // Match Mermaid `rendering-elements/edges.js insertEdge`: `toCluster` restarts from the
    // original edge points, while `fromCluster` continues from the current cut path.
    if let Some(tc) = le.to_cluster.as_deref()
        && let Some(boundary) = state_edge_boundary_for_cluster(ctx, tc, origin_x, origin_y)
    {
        label_path_points = state_edge_cut_path_at_intersect(&raw_local_points, &boundary);
        points_were_explicitly_updated = true;
    }
    if let Some(fc) = le.from_cluster.as_deref()
        && let Some(boundary) = state_edge_boundary_for_cluster(ctx, fc, origin_x, origin_y)
    {
        let mut rev = label_path_points;
        rev.reverse();
        rev = state_edge_cut_path_at_intersect(&rev, &boundary);
        rev.reverse();
        label_path_points = rev;
        points_were_explicitly_updated = true;
    }

    state_edge_finish_geometry(
        raw_local_points,
        data_points,
        label_path_points,
        arrow_type_end,
        ctx.style_plan
            .compatibility()
            .native_edge_curve()
            .unwrap_or("basis"),
        points_were_explicitly_updated,
    )
}

#[cfg(feature = "layout-shoal")]
fn state_edge_stroke_outset(ctx: &StateRenderCtx<'_>) -> f64 {
    ctx.style_plan
        .compatibility()
        .terminal_stroke_width_value(crate::state::StateTerminalSurface::Transition)
        / 2.0
}

#[cfg(feature = "layout-shoal")]
fn state_canonical_edge_bounds(
    geometry: &StateCanonicalEdge<'_>,
    stroke_outset: f64,
) -> kurbo::Rect {
    let bounds = &geometry.bounds;
    let path_bounds = geometry
        .rough_d
        .as_deref()
        .and_then(svg_path_bounds_from_d)
        .map_or_else(
            || kurbo::Rect::new(bounds.min_x, bounds.min_y, bounds.max_x, bounds.max_y),
            |bounds| kurbo::Rect::new(bounds.min_x, bounds.min_y, bounds.max_x, bounds.max_y),
        );
    let mut bounds = path_bounds.inflate(stroke_outset, stroke_outset);
    if geometry.end_marker.depth > 0.0 {
        let [_, _, shoulder, reference] = &geometry.marker_points;
        let reference = kurbo::Point::new(reference.x, reference.y);
        let forward = (reference - kurbo::Point::new(shoulder.x, shoulder.y)).normalize();
        let across = kurbo::Vec2::new(-forward.y, forward.x);
        let ref_x = f64::from(STATE_BARB_TIP_X) - geometry.end_marker.trim;
        let ref_y = f64::from(STATE_BARB_CENTER_Y);
        for dx in [-ref_x, f64::from(STATE_BARB_VIEWPORT_WIDTH) - ref_x] {
            for dy in [-ref_y, ref_y] {
                bounds = bounds.union_pt(reference + forward * dx + across * dy);
            }
        }
    }
    bounds
}

#[derive(Default)]
pub(super) struct StateEdgePathScratch {
    #[cfg(feature = "layout-shoal")]
    canonical: CanonicalPaintScratch,
    json: Vec<u8>,
    #[cfg(feature = "layout-layered")]
    layered_json: String,
    base64: String,
    style: String,
    classes: String,
    marker_end: String,
    ryu: ryu_js::Buffer,
}

/// Formats `&[LayoutPoint]` into `out` byte-for-byte identically to `serde_json::to_writer`,
/// avoiding per-coordinate `serde` dispatch overhead on the common finite decimal range.
#[inline]
fn write_serde_f64_json(out: &mut Vec<u8>, value: f64, ryu: &mut ryu_js::Buffer) {
    if value == 0.0 {
        if value.is_sign_negative() {
            out.extend_from_slice(b"-0.0");
        } else {
            out.extend_from_slice(b"0.0");
        }
        return;
    }
    let abs = value.abs();
    if value.is_finite() && (1e-4..1e15).contains(&abs) {
        let formatted = ryu.format_finite(value);
        out.extend_from_slice(formatted.as_bytes());
        if !formatted.as_bytes().contains(&b'.') {
            out.extend_from_slice(b".0");
        }
    } else if serde_json::to_writer(&mut *out, &value).is_err() {
        out.extend_from_slice(b"null");
    }
}

fn write_serde_points_json(out: &mut Vec<u8>, points: &[LayoutPoint], ryu: &mut ryu_js::Buffer) {
    out.clear();
    out.reserve(points.len().saturating_mul(28).max(128));
    out.push(b'[');
    for (index, point) in points.iter().enumerate() {
        if index > 0 {
            out.push(b',');
        }
        out.extend_from_slice(br#"{"x":"#);
        write_serde_f64_json(out, point.x, ryu);
        out.extend_from_slice(br#","y":"#);
        write_serde_f64_json(out, point.y, ryu);
        out.push(b'}');
    }
    out.push(b']');
}

#[allow(clippy::too_many_arguments)]
fn write_state_edge_path(
    out: &mut impl SvgOutput,
    ctx: &StateRenderCtx<'_>,
    le: &LayoutEdge,
    edge_id: &str,
    classes: &str,
    marker_end: Option<&str>,
    arrow_type_end: Option<&str>,
    origin_x: f64,
    origin_y: f64,
    scratch: &mut StateEdgePathScratch,
    #[cfg(feature = "layout-shoal")] ordinal: usize,
    #[cfg(feature = "layout-shoal")] canonical_bounds: &mut Option<kurbo::Rect>,
) -> Result<()> {
    if !ctx.uses_elk_adapter_dom && !ctx.uses_canonical_routes() && le.points.len() < 2 {
        return Ok(());
    }

    let work_meter = ctx.work_meter;
    let look = state_data_look(ctx);
    let compatibility = ctx.style_plan.compatibility();
    let sketch =
        compatibility.native_edge_style == Some(merman_theme_contract::v2::EdgeStyle::Sketch);
    #[cfg(feature = "layout-shoal")]
    let mut label_cutout = false;
    #[cfg(feature = "layout-shoal")]
    let canonical = match ctx.canonical_routes {
        Some(paint) => {
            if look == "neo" && !sketch {
                scratch.canonical.route.measure_length();
            }
            let origin = kurbo::Vec2::new(origin_x, origin_y);
            let outset = state_edge_stroke_outset(ctx);
            let randomness = (sketch || state_effective_look(ctx) == "handDrawn")
                .then_some(&ctx.hand_drawn_seed);
            let geometry = paint(le, -origin, outset, randomness, &mut scratch.canonical)?;
            let bounds = state_canonical_edge_bounds(&geometry, outset);
            if let Some(label) = le
                .label
                .as_ref()
                .filter(|l| l.width > 0.0 && l.height > 0.0)
            {
                // State roots can translate independently. Both rectangles are local to the edge.
                let gap = kurbo::Rect::from_center_size(
                    (label.x - origin_x, label.y - origin_y),
                    (label.width, label.height),
                );
                if bounds.intersect(gap).area() > 0.0 {
                    cutout::write_defs(
                        out,
                        format_args!("{}_state-edge-label-{ordinal}", ctx.diagram_id),
                        bounds,
                        1.0,
                        &[gap],
                        work_meter,
                    )?;
                    label_cutout = true;
                }
            }
            let bounds = bounds + origin;
            *canonical_bounds = Some(canonical_bounds.map_or(bounds, |cur| cur.union(bounds)));
            Some(geometry)
        }
        None => None,
    };

    #[cfg(feature = "layout-grid")]
    let grid = match &ctx.grid_edges {
        Some(edges) => match edges.get(edge_id) {
            Some(paint) => Some(paint),
            None => return Ok(()),
        },
        None => None,
    };
    #[cfg(feature = "layout-layered")]
    let layered = match &ctx.layered_edges {
        Some(edges) => match edges.get(edge_id) {
            Some(paint) => Some(paint),
            None => return Ok(()),
        },
        None => None,
    };
    let legacy;
    let (base_d, paint_d, points) = 'geometry: {
        #[cfg(feature = "layout-layered")]
        if let Some(paint) = layered {
            break 'geometry (
                paint.path.as_str(),
                paint.path.as_str(),
                paint.points.as_slice(),
            );
        }
        #[cfg(feature = "layout-grid")]
        if let Some(paint) = grid {
            break 'geometry (
                paint.path.as_str(),
                paint.path.as_str(),
                paint.points.as_slice(),
            );
        }

        #[cfg(feature = "layout-shoal")]
        if let Some(geometry) = &canonical {
            let path_d = geometry.d.as_str();
            break 'geometry (
                path_d,
                geometry.rough_d.as_deref().unwrap_or(path_d),
                geometry.data_points,
            );
        }
        legacy = state_edge_prepare_geometry(ctx, le, arrow_type_end, origin_x, origin_y);
        let path_d = legacy.rendered_d.as_str();
        break 'geometry (path_d, path_d, legacy.data_points.as_slice());
    };
    let hopped_d = ctx.elk_line_hop_paths.get(edge_id).map(String::as_str);
    #[cfg(feature = "layout-grid")]
    let hopped_d = hopped_d.or_else(|| grid.and_then(|paint| paint.hopped.as_deref()));
    #[cfg(feature = "layout-layered")]
    let hopped_d = hopped_d.or_else(|| layered.and_then(|paint| paint.hopped.as_deref()));
    let rendered_d = hopped_d.unwrap_or(paint_d);
    let layered_hand_drawn =
        ctx.uses_layered() && state_effective_look(ctx) == "handDrawn" && hopped_d.is_none();
    let rough_d =
        if (sketch || layered_hand_drawn) && !ctx.uses_canonical_routes() && !rendered_d.is_empty()
        {
            Some(merman_render_core::rough_svg::constrained_rough_edge_path(
                rendered_d,
                Default::default(),
                work_meter,
                &ctx.hand_drawn_seed,
            )?)
        } else {
            None
        };
    let rendered_d = rough_d.as_deref().unwrap_or(rendered_d);
    #[cfg(feature = "layout-layered")]
    let axis_bytes = if let Some(paint) = layered {
        work_meter.charge_emit_work(points.len())?;
        scratch.layered_json.clear();
        merman_render_core::svg_edge_parity::path_numbers::json_stringify_axis_points_into(
            &mut scratch.layered_json,
            points,
            paint.axes,
            &mut scratch.ryu,
        );
        work_meter.charge_emit_work(scratch.layered_json.len())?;
        Some(scratch.layered_json.as_bytes())
    } else {
        None
    };
    #[cfg(not(feature = "layout-layered"))]
    let axis_bytes = None;
    let point_bytes = if let Some(bytes) = axis_bytes {
        bytes
    } else {
        write_serde_points_json(&mut scratch.json, points, &mut scratch.ryu);
        scratch.json.as_slice()
    };
    scratch.base64.clear();
    base64::engine::general_purpose::STANDARD.encode_string(point_bytes, &mut scratch.base64);
    let original_style = &mut scratch.style;
    original_style.clear();
    if look == "neo" && !sketch {
        #[cfg(feature = "layout-shoal")]
        let length = if ctx.canonical_routes.is_some() {
            scratch
                .canonical
                .route
                .measured_length()
                .map(|length| f64::from(length as f32))
                .filter(|length| length.is_finite())
        } else {
            None
        };
        #[cfg(not(feature = "layout-shoal"))]
        let length: Option<f64> = None;
        if let Some(length) = length.or_else(|| svg_path_length_from_d(base_d)) {
            write_neo_edge_mask(
                original_style,
                length,
                None,
                arrow_type_end,
                classes
                    .split_whitespace()
                    .any(|class| class == "edge-pattern-dashed"),
                false,
            );
        }
    }
    original_style.push_str(if ctx.uses_canonical_routes() {
        "fill:none"
    } else {
        "fill:none;;;fill:none"
    });
    let style = if let Some(path) = hopped_d {
        if ctx.uses_layered() {
            merman_render_core::svg_edge_parity::line_hops::rewrite_style_after_line_hop_with_phase(
                original_style,
                path,
                merman_model::OperationPhase::Emit,
                work_meter,
            )?
        } else {
            rewrite_style_after_line_hop(original_style, path, work_meter)?
        }
    } else {
        std::borrow::Cow::Borrowed(original_style.as_str())
    };
    out.push_str(r#"<path d=""#);
    out.push_str(rendered_d);
    out.push_str(r#"" id=""#);
    let _ = write!(out, "{}", state_scoped_dom_id(ctx, edge_id));
    out.push_str(r#"" class=""#);
    escape_attr_into(out, classes);
    out.push_str(r#"" style=""#);
    escape_attr_into(out, &style);
    out.push_str(r#"" data-edge="true" data-et="edge" data-id=""#);
    escape_attr_into(out, edge_id);
    out.push_str(r#"" data-points=""#);
    out.push_str(&scratch.base64);
    out.push_str(r#"" data-look=""#);
    escape_attr_into(out, look);
    out.push('"');
    #[cfg(feature = "layout-shoal")]
    if label_cutout {
        cutout::write_reference(
            out,
            format_args!("{}_state-edge-label-{ordinal}", ctx.diagram_id),
        );
    }
    if let Some(marker_end) = marker_end {
        out.push_str(r#" marker-end=""#);
        escape_attr_into(out, marker_end);
        out.push('"');
    }
    out.push_str("/>");
    out.checkpoint()?;
    Ok(())
}

pub(super) fn render_state_edge_path(
    out: &mut impl SvgOutput,
    ctx: &StateRenderCtx<'_>,
    edge: &StateSvgEdge,
    (origin_x, origin_y): (f64, f64),
    scratch: &mut StateEdgePathScratch,
    #[cfg(feature = "layout-shoal")] ordinal: usize,
    #[cfg(feature = "layout-shoal")] canonical_bounds: &mut Option<kurbo::Rect>,
) -> Result<()> {
    let Some(le) = ctx.layout_edges_by_id.get(edge.id.as_str()).copied() else {
        return Ok(());
    };
    let mut classes = std::mem::take(&mut scratch.classes);
    classes.clear();
    if (ctx.uses_elk_adapter_dom || ctx.uses_canonical_routes() || ctx.uses_flat_routes())
        && edge
            .classes
            .split_whitespace()
            .any(|class| class == "note-edge")
    {
        classes.push_str("edge-thickness-normal edge-pattern-dashed");
    } else {
        classes.push_str("edge-thickness-normal edge-pattern-solid");
    }
    for c in edge.classes.split_whitespace() {
        let trimmed = c.trim();
        if trimmed.is_empty() {
            continue;
        }
        classes.push(' ');
        classes.push_str(trimmed);
    }

    let mut marker_end_buf = std::mem::take(&mut scratch.marker_end);
    marker_end_buf.clear();
    let has_marker_end = match edge.arrow_type_end.trim() {
        "arrow_barb" | "arrow_barb_neo" => {
            let neo = if ctx.uses_canonical_routes() || ctx.uses_flat_routes() {
                ctx.style_plan.compatibility().is_neo()
            } else {
                state_data_look(ctx) == "neo"
            };
            let suffix = if neo { "-margin" } else { "" };
            let _ = write!(
                marker_end_buf,
                "url(#{}_stateDiagram-barbEnd{suffix})",
                ctx.diagram_id
            );
            true
        }
        _ => false,
    };

    let result = write_state_edge_path(
        out,
        ctx,
        le,
        edge.id.as_str(),
        &classes,
        has_marker_end.then_some(marker_end_buf.as_str()),
        Some(edge.arrow_type_end.as_str()),
        origin_x,
        origin_y,
        scratch,
        #[cfg(feature = "layout-shoal")]
        ordinal,
        #[cfg(feature = "layout-shoal")]
        canonical_bounds,
    );
    scratch.classes = classes;
    scratch.marker_end = marker_end_buf;
    result
}

pub(super) fn state_edge_label_fill(
    ctx: &StateRenderCtx<'_>,
    paint: crate::state::compatibility::StateEdgeLabelPaint,
    center: kurbo::Point,
) -> Result<Color> {
    Ok(match paint.mode {
        EdgeLabelBackground::Auto => {
            ctx.work_meter
                .charge_emit_work(ctx.composite_paints.len())?;
            ctx.composite_paints
                .values()
                .fold(paint.canvas, |backdrop, composite| {
                    composite.composite_at(center, backdrop)
                })
        }
        EdgeLabelBackground::Canvas => paint.canvas,
        EdgeLabelBackground::Pill => paint.node,
        EdgeLabelBackground::Transparent => Color::TRANSPARENT,
    })
}

pub(super) fn render_state_edge_label(
    out: &mut impl SvgOutput,
    ctx: &StateRenderCtx<'_>,
    edge: &StateSvgEdge,
    origin_x: f64,
    origin_y: f64,
) -> Result<()> {
    fn fallback_background_metadata_attr(id: &str, fill: Option<&str>) -> String {
        let Some(fill) = fill else {
            return String::new();
        };
        format!(
            r#" {}="state-transition-label-background:{}" {}="{}""#,
            FALLBACK_OCCURRENCE_DATA_ATTR,
            escape_attr(id),
            FALLBACK_BACKGROUND_FILL_DATA_ATTR,
            escape_attr(fill),
        )
    }

    fn edge_label_div_style(
        label_w: f64,
        prefix: &str,
        inset_backdrop: Option<(f64, &str)>,
    ) -> String {
        // Mermaid uses `createText(..., { width: 200 })` for state edge labels and flips the XHTML
        // `<div>` container to wrapping mode when the label reaches the max width.
        let max_width = MERMAID_CREATE_TEXT_DEFAULT_WIDTH_PX;
        if let Some((inset, background)) = inset_backdrop {
            let insets = 2.0 * inset;
            let white_space = if label_w - insets >= max_width - 1e-3 {
                "break-spaces"
            } else {
                "nowrap"
            };
            format!(
                "{prefix}background-color: {background}; display: flex; justify-content: center; white-space: {white_space}; line-height: 1.5; max-width: {max_width}px; text-align: center; width: calc(100% - {insets}px); box-sizing: content-box; padding-inline: {inset}px;",
                max_width = fmt_display(max_width),
                insets = fmt_display(insets),
                inset = fmt_display(inset),
            )
        } else if label_w >= max_width - 1e-3 {
            format!(
                "{}display: table; white-space: break-spaces; line-height: 1.5; max-width: {}px; text-align: center; width: {}px;",
                prefix,
                fmt_display(max_width),
                fmt_display(max_width),
            )
        } else {
            format!(
                "{}display: table-cell; white-space: nowrap; line-height: 1.5; max-width: {}px; text-align: center;",
                prefix,
                fmt_display(max_width),
            )
        }
    }

    fn write_empty_edge_label(
        out: &mut impl SvgOutput,
        id: &str,
        html_labels: bool,
        html_style: &str,
        fallback_background_metadata_attr: &str,
    ) {
        if html_labels {
            let _ = write!(
                out,
                r#"<g class="edgeLabel"><g class="label" data-id="{}" transform="translate(0, 0)"><foreignObject{} width="0" height="0"><div xmlns="http://www.w3.org/1999/xhtml" class="labelBkg" style="{}"><span class="edgeLabel"></span></div></foreignObject></g></g>"#,
                escape_attr(id),
                fallback_background_metadata_attr,
                escape_attr(html_style)
            );
        } else {
            let _ = write!(
                out,
                r#"<g class="edgeLabel"><g class="label" data-id="{}" transform="translate(0, 0)"></g></g>"#,
                escape_attr(id)
            );
        }
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "The SVG writer takes geometry and resolved label styles separately."
    )]
    fn write_visible_edge_label(
        out: &mut impl SvgOutput,
        id: &str,
        label_text: &str,
        measured_geometry: Option<&crate::state::StateNativeLabelGeometry>,
        w: f64,
        h: f64,
        html_labels: bool,
        canonical: bool,
        uses_native_background: bool,
        label_style: &str,
        label_div_prefix: &str,
        html_inset_backdrop: Option<(f64, &str)>,
        fallback_background_fill: Option<&str>,
        svg_background_style: &str,
    ) {
        let w = w.max(0.0);
        let h = h.max(0.0);
        let measured_geometry = (!html_labels).then_some(measured_geometry).flatten();
        let (w, h) = if let Some(geometry) = measured_geometry {
            debug_assert!((geometry.width_px() - w).abs() <= 1e-6);
            debug_assert!((geometry.height_px() - h).abs() <= 1e-6);
            (geometry.width_px(), geometry.height_px())
        } else {
            (w, h)
        };
        if html_labels {
            let fallback_background_metadata_attr =
                fallback_background_metadata_attr(id, fallback_background_fill);
            let span_style_attr = if label_style.trim().is_empty() {
                String::new()
            } else {
                format!(r#" style="{}""#, escape_attr(label_style))
            };
            let _ = write!(
                out,
                r#"<g class="label" data-id="{}" transform="translate({}, {})"><foreignObject{} width="{}" height="{}"{}><div xmlns="http://www.w3.org/1999/xhtml" class="labelBkg" style="{}"><span class="edgeLabel"{}>{}</span></div></foreignObject></g>"#,
                escape_attr(id),
                fmt_display(-w / 2.0),
                fmt_display(-h / 2.0),
                fallback_background_metadata_attr,
                fmt_display(w),
                fmt_display(h),
                html_label_overflow_attr(canonical, w, h),
                escape_attr(&edge_label_div_style(
                    w,
                    label_div_prefix,
                    html_inset_backdrop,
                )),
                span_style_attr,
                state_edge_label_html(label_text)
            );
        } else {
            let (label_dom, text_origin_y) = if let Some(geometry) = measured_geometry {
                (
                    state_measured_svg_text_label(
                        geometry,
                        true,
                        (!label_style.is_empty()).then_some(label_style),
                    ),
                    0.0,
                )
            } else {
                (
                    state_svg_text_label(
                        label_text,
                        true,
                        (!label_style.is_empty()).then_some(label_style),
                    ),
                    h / 2.0,
                )
            };
            let background_fill = if uses_native_background {
                r#" fill="none""#
            } else {
                ""
            };
            let _ = write!(
                out,
                r#"<g class="label" data-id="{}" transform="translate({}, {})"><g><rect class="background"{background_fill} style="stroke: none;{}" x="0" y="0" width="{}" height="{}"/><g transform="translate({}, {})">{}</g></g></g>"#,
                escape_attr(id),
                fmt_display(-w / 2.0),
                fmt_display(-h / 2.0),
                escape_attr(svg_background_style),
                fmt_display(w),
                fmt_display(h),
                fmt_display(w / 2.0),
                fmt_display(text_origin_y),
                label_dom
            );
        }
    }

    let label_text = if ctx.uses_elk_adapter_dom {
        mermaid_html_breaks_to_newlines(&edge.label)
    } else {
        std::borrow::Cow::Borrowed(edge.label.as_str())
    };
    let label_text = if ctx.uses_layered() {
        label_text.trim_matches(merman_model::text::is_ecmascript_whitespace)
    } else {
        label_text.trim()
    };
    if label_text.is_empty() {
        if ctx.uses_elk_adapter_dom || ctx.uses_layered() {
            return Ok(());
        }
        write_empty_edge_label(
            out,
            &edge.id,
            ctx.html_labels,
            &edge_label_div_style(0.0, "", None),
            "",
        );
        return Ok(());
    }
    let Some(le) = ctx.layout_edges_by_id.get(edge.id.as_str()).copied() else {
        return Ok(());
    };
    let Some(lbl) = le.label.as_ref() else {
        return Ok(());
    };

    let label_position = LayoutPoint {
        x: lbl.x - origin_x,
        y: lbl.y - origin_y,
    };
    let label_position = if ctx.uses_canonical_routes() || ctx.uses_layered() {
        label_position
    } else {
        let geometry = state_edge_prepare_geometry(
            ctx,
            le,
            Some(edge.arrow_type_end.as_str()),
            origin_x,
            origin_y,
        );
        position_edge_label(
            label_position,
            Some(&geometry.original_label_path_points),
            &geometry.label_path_points,
            &geometry.rendered_d,
            geometry.points_were_explicitly_updated,
        )
    };
    let w = lbl.width.max(0.0);
    let h = lbl.height.max(0.0);
    let html_inset_backdrop =
        if ctx.html_labels && ctx.uses_canonical_routes() && w > 0.0 && h > 0.0 {
            let inset = ctx.style_plan.shoal_html_edge_label_inset();
            (inset > 0.0).then_some((
                inset,
                ctx.style_plan
                    .compatibility()
                    .edge_label_background
                    .as_str(),
            ))
        } else {
            None
        };

    let _ = write!(
        out,
        r#"<g class="edgeLabel" transform="translate({}, {})">"#,
        fmt_display(label_position.x),
        fmt_display(label_position.y),
    );
    let native_background = ctx.style_plan.compatibility().native_edge_label_paint;
    let mut adaptive_label_style = String::new();
    if let Some(paint) = native_background {
        let center = kurbo::Point::new(label_position.x + origin_x, label_position.y + origin_y);
        let fill = state_edge_label_fill(ctx, paint, center)?;
        if let Some(default_ink) =
            literal_fill(&ctx.style_plan.compatibility().transition_label_color)
            && let Some(ink) = adaptive_text_ink(fill, default_ink)
        {
            adaptive_label_style = if ctx.html_labels {
                format!("color:{ink} !important;")
            } else {
                format!("fill:{ink};")
            };
        }
        let bounds = kurbo::Rect::from_center_size((0.0, 0.0), (w, h));
        let _ = write_backplate(out, bounds, paint.mode, fill, paint.border);
        out.checkpoint()?;
    }
    write_visible_edge_label(
        out,
        &edge.id,
        label_text,
        ctx.label_sidecar
            .measured_native_geometry(crate::state::StateLabelOwner::Edge(&edge.id)),
        w,
        h,
        ctx.html_labels,
        ctx.uses_canonical_routes(),
        native_background.is_some(),
        &adaptive_label_style,
        "",
        html_inset_backdrop,
        None,
        "",
    );
    out.push_str("</g>");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_rounded_end_marker_offset_moves_only_the_terminal_point() {
        let input = vec![
            LayoutPoint { x: 0.0, y: 0.0 },
            LayoutPoint { x: 0.0, y: 20.0 },
            LayoutPoint { x: 20.0, y: 20.0 },
        ];

        let output = state_rounded_end_marker_offset_points(&input, Some("arrow_barb_neo"));

        assert_eq!(output.len(), input.len());
        assert_eq!((output[0].x, output[0].y), (0.0, 0.0));
        assert_eq!((output[1].x, output[1].y), (0.0, 20.0));
        assert!((output[2].x - 14.5).abs() <= 1e-9);
        assert!((output[2].y - 20.0).abs() <= 1e-9);
    }

    #[test]
    fn state_line_with_end_marker_offset_shortens_neo_barb_terminal_point() {
        let input = vec![
            LayoutPoint { x: 0.0, y: 0.0 },
            LayoutPoint { x: 10.0, y: 0.0 },
        ];

        let output = state_line_with_end_marker_offset_points(&input, Some("arrow_barb_neo"));

        assert_eq!(output.len(), 2);
        assert!((output[0].x - 0.0).abs() <= 1e-9);
        assert!((output[0].y - 0.0).abs() <= 1e-9);
        assert!((output[1].x - 4.5).abs() <= 1e-9);
        assert!((output[1].y - 0.0).abs() <= 1e-9);
    }

    #[test]
    fn state_updated_path_labels_use_points_before_curve_projection() {
        let label_path_points = vec![
            LayoutPoint { x: 0.0, y: 0.0 },
            LayoutPoint { x: 0.0, y: 20.0 },
            LayoutPoint { x: 20.0, y: 20.0 },
        ];
        let geometry = state_edge_finish_geometry(
            label_path_points.clone(),
            label_path_points.clone(),
            label_path_points.clone(),
            Some("arrow_barb_neo"),
            "basis",
            true,
        );

        assert_eq!(geometry.label_path_points.len(), label_path_points.len());
        for (actual, expected) in geometry.label_path_points.iter().zip(&label_path_points) {
            assert_eq!((actual.x, actual.y), (expected.x, expected.y));
        }
        assert_ne!(
            geometry.rendered_d,
            curve_basis_path_d(&geometry.label_path_points),
            "fixCorners and marker offsets should affect only the rendered curve"
        );
        let position = position_edge_label(
            LayoutPoint { x: 99.0, y: 99.0 },
            Some(&geometry.original_label_path_points),
            &geometry.label_path_points,
            &geometry.rendered_d,
            geometry.points_were_explicitly_updated,
        );
        assert_eq!((position.x, position.y), (99.0, 99.0));
    }

    #[test]
    fn state_updated_path_labels_preserve_offsets_from_the_unclipped_route() {
        let original = vec![
            LayoutPoint { x: -20.0, y: 0.0 },
            LayoutPoint { x: 0.0, y: 0.0 },
            LayoutPoint { x: 0.0, y: 20.0 },
            LayoutPoint { x: 20.0, y: 20.0 },
        ];
        let clipped = original[1..].to_vec();
        let geometry = state_edge_finish_geometry(
            original,
            clipped.clone(),
            clipped,
            Some("arrow_barb_neo"),
            "basis",
            true,
        );
        let position = position_edge_label(
            LayoutPoint { x: 99.0, y: 99.0 },
            Some(&geometry.original_label_path_points),
            &geometry.label_path_points,
            &geometry.rendered_d,
            geometry.points_were_explicitly_updated,
        );
        assert_eq!((position.x, position.y), (99.0, 109.0));
    }
}
