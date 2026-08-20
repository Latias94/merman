use super::*;

#[derive(Debug, Clone, Copy)]
struct StateEdgeBoundaryNode {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

fn state_edge_outside_node(
    node: &StateEdgeBoundaryNode,
    point: &crate::model::LayoutPoint,
) -> bool {
    let dx = (point.x - node.x).abs();
    let dy = (point.y - node.y).abs();
    let w = node.width / 2.0;
    let h = node.height / 2.0;
    dx >= w || dy >= h
}

fn state_edge_rect_intersection(
    node: &StateEdgeBoundaryNode,
    inside_point: &crate::model::LayoutPoint,
    outside_point: &crate::model::LayoutPoint,
) -> crate::model::LayoutPoint {
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
        let mut res = crate::model::LayoutPoint {
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

    crate::model::LayoutPoint { x: ix, y: iy }
}

fn state_edge_cut_path_at_intersect(
    input: &[crate::model::LayoutPoint],
    boundary: &StateEdgeBoundaryNode,
) -> Vec<crate::model::LayoutPoint> {
    if input.is_empty() {
        return Vec::new();
    }
    let mut out: Vec<crate::model::LayoutPoint> = Vec::new();
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
    le: &crate::model::LayoutEdge,
    input: &[crate::model::LayoutPoint],
    origin_x: f64,
    origin_y: f64,
) -> Option<Vec<crate::model::LayoutPoint>> {
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
    let center = crate::model::LayoutPoint {
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
    point_a: &crate::model::LayoutPoint,
    point_b: &crate::model::LayoutPoint,
    distance: f64,
) -> crate::model::LayoutPoint {
    let x_diff = point_b.x - point_a.x;
    let y_diff = point_b.y - point_a.y;
    let length = (x_diff * x_diff + y_diff * y_diff).sqrt();
    let ratio = distance / length;
    crate::model::LayoutPoint {
        x: point_b.x - ratio * x_diff,
        y: point_b.y - ratio * y_diff,
    }
}

fn state_edge_is_corner_point(
    prev: &crate::model::LayoutPoint,
    curr: &crate::model::LayoutPoint,
    next: &crate::model::LayoutPoint,
) -> bool {
    (prev.x == curr.x
        && curr.y == next.y
        && (curr.x - next.x).abs() > 5.0
        && (curr.y - prev.y).abs() > 5.0)
        || (prev.y == curr.y
            && curr.x == next.x
            && (curr.x - prev.x).abs() > 5.0
            && (curr.y - next.y).abs() > 5.0)
}

fn state_edge_fix_corners(
    line_data: &[crate::model::LayoutPoint],
) -> Vec<crate::model::LayoutPoint> {
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
                new_corner_point = crate::model::LayoutPoint {
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
                new_corner_point = crate::model::LayoutPoint {
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

fn state_line_with_end_marker_offset_points(
    input: &[crate::model::LayoutPoint],
    arrow_type_end: Option<&str>,
) -> Vec<crate::model::LayoutPoint> {
    fn calculate_delta_and_angle(
        a: &crate::model::LayoutPoint,
        b: &crate::model::LayoutPoint,
    ) -> (f64, f64, f64) {
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

        out.push(crate::model::LayoutPoint {
            x: point.x + offset_x,
            y: point.y + offset_y,
        });
    }

    out
}

struct StatePreparedEdgeGeometry {
    data_points: Vec<crate::model::LayoutPoint>,
    label_path_points: Vec<crate::model::LayoutPoint>,
    rendered_d: String,
    points_were_explicitly_updated: bool,
}

fn state_edge_finish_geometry(
    data_points: Vec<crate::model::LayoutPoint>,
    label_path_points: Vec<crate::model::LayoutPoint>,
    arrow_type_end: Option<&str>,
    points_were_explicitly_updated: bool,
) -> StatePreparedEdgeGeometry {
    // Mermaid keeps `paths.updatedPath` before `fixCorners`, marker offsets, and curve encoding.
    // Only the rendered SVG path consumes these projected curve points.
    let mut points_for_curve = label_path_points
        .iter()
        .filter(|point| !point.y.is_nan())
        .cloned()
        .collect::<Vec<_>>();
    points_for_curve = state_edge_fix_corners(&points_for_curve);
    points_for_curve = state_line_with_end_marker_offset_points(&points_for_curve, arrow_type_end);

    StatePreparedEdgeGeometry {
        data_points,
        label_path_points,
        rendered_d: curve_basis_path_d(&points_for_curve),
        points_were_explicitly_updated,
    }
}

fn state_edge_prepare_geometry(
    ctx: &StateRenderCtx<'_>,
    le: &crate::model::LayoutEdge,
    arrow_type_end: Option<&str>,
    origin_x: f64,
    origin_y: f64,
) -> StatePreparedEdgeGeometry {
    let mut raw_local_points: Vec<crate::model::LayoutPoint> = Vec::new();
    for p in &le.points {
        raw_local_points.push(crate::model::LayoutPoint {
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
        data_points,
        label_path_points,
        arrow_type_end,
        points_were_explicitly_updated,
    )
}

#[allow(clippy::too_many_arguments)]
fn write_state_edge_path(
    out: &mut String,
    ctx: &StateRenderCtx<'_>,
    le: &crate::model::LayoutEdge,
    edge_id: &str,
    classes: &str,
    marker_end: Option<&str>,
    arrow_type_end: Option<&str>,
    origin_x: f64,
    origin_y: f64,
) {
    if le.points.len() < 2 {
        return;
    }

    let geometry = state_edge_prepare_geometry(ctx, le, arrow_type_end, origin_x, origin_y);
    let data_points = base64::engine::general_purpose::STANDARD
        .encode(serde_json::to_vec(&geometry.data_points).unwrap_or_default());
    let _ = write!(
        out,
        r#"<path d="{}" id="{}" class="{}" style="fill:none;;;fill:none" data-edge="true" data-et="edge" data-id="{}" data-points="{}" data-look="{}""#,
        geometry.rendered_d,
        state_scoped_dom_id(ctx, edge_id),
        escape_xml_display(classes),
        escape_xml_display(edge_id),
        data_points,
        escape_xml_display(state_data_look(ctx))
    );
    if let Some(marker_end) = marker_end {
        let _ = write!(out, r#" marker-end="{}""#, escape_xml_display(marker_end));
    }
    out.push_str("/>");
}

pub(super) fn render_state_edge_path(
    out: &mut String,
    ctx: &StateRenderCtx<'_>,
    edge: &StateSvgEdge,
    origin_x: f64,
    origin_y: f64,
) {
    let mut classes = "edge-thickness-normal edge-pattern-solid".to_string();
    for c in edge.classes.split_whitespace() {
        if c.trim().is_empty() {
            continue;
        }
        classes.push(' ');
        classes.push_str(c.trim());
    }

    let marker_end = match edge.arrow_type_end.trim() {
        "arrow_barb" | "arrow_barb_neo" => {
            Some(format!("url(#{}_stateDiagram-barbEnd)", ctx.diagram_id))
        }
        _ => None,
    };

    let Some(le) = ctx.layout_edges_by_id.get(edge.id.as_str()).copied() else {
        return;
    };
    write_state_edge_path(
        out,
        ctx,
        le,
        edge.id.as_str(),
        &classes,
        marker_end.as_deref(),
        Some(edge.arrow_type_end.as_str()),
        origin_x,
        origin_y,
    );
}

pub(super) fn render_state_edge_label(
    out: &mut String,
    ctx: &StateRenderCtx<'_>,
    edge: &StateSvgEdge,
    origin_x: f64,
    origin_y: f64,
) {
    fn edge_label_div_style(label_w: f64) -> String {
        // Mermaid uses `createText(..., { width: 200 })` for state edge labels and flips the XHTML
        // `<div>` container to wrapping mode when the label reaches the max width.
        let max_width = crate::text::MERMAID_CREATE_TEXT_DEFAULT_WIDTH_PX;
        if label_w >= max_width - 1e-3 {
            format!(
                "display: table; white-space: break-spaces; line-height: 1.5; max-width: {}px; text-align: center; width: {}px;",
                fmt_display(max_width),
                fmt_display(max_width),
            )
        } else {
            format!(
                "display: table-cell; white-space: nowrap; line-height: 1.5; max-width: {}px; text-align: center;",
                fmt_display(max_width),
            )
        }
    }

    fn write_empty_edge_label(out: &mut String, id: &str, html_labels: bool, html_style: &str) {
        if html_labels {
            let _ = write!(
                out,
                r#"<g class="edgeLabel"><g class="label" data-id="{}" transform="translate(0, 0)"><foreignObject width="0" height="0"><div xmlns="http://www.w3.org/1999/xhtml" class="labelBkg" style="{}"><span class="edgeLabel"></span></div></foreignObject></g></g>"#,
                escape_attr(id),
                html_style
            );
        } else {
            let _ = write!(
                out,
                r#"<g class="edgeLabel"><g class="label" data-id="{}" transform="translate(0, 0)"></g></g>"#,
                escape_attr(id)
            );
        }
    }

    fn write_visible_edge_label(
        out: &mut String,
        id: &str,
        label_text: &str,
        label_pos: crate::model::LayoutPoint,
        w: f64,
        h: f64,
        html_labels: bool,
    ) {
        let w = w.max(0.0);
        let h = h.max(0.0);
        if html_labels {
            let _ = write!(
                out,
                r#"<g class="edgeLabel" transform="translate({}, {})"><g class="label" data-id="{}" transform="translate({}, {})"><foreignObject width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" class="labelBkg" style="{}"><span class="edgeLabel">{}</span></div></foreignObject></g></g>"#,
                fmt_display(label_pos.x),
                fmt_display(label_pos.y),
                escape_attr(id),
                fmt_display(-w / 2.0),
                fmt_display(-h / 2.0),
                fmt_display(w),
                fmt_display(h),
                edge_label_div_style(w),
                state_edge_label_html(label_text)
            );
        } else {
            let label_dom = state_svg_text_label(label_text, true, None);
            let _ = write!(
                out,
                r#"<g class="edgeLabel" transform="translate({}, {})"><g class="label" data-id="{}" transform="translate({}, {})"><g><rect class="background" style="stroke: none" x="0" y="0" width="{}" height="{}"/><g transform="translate({}, {})">{}</g></g></g></g>"#,
                fmt_display(label_pos.x),
                fmt_display(label_pos.y),
                escape_attr(id),
                fmt_display(-w / 2.0),
                fmt_display(-h / 2.0),
                fmt_display(w),
                fmt_display(h),
                fmt_display(w / 2.0),
                fmt_display(h / 2.0),
                label_dom
            );
        }
    }

    let empty_edge_label_style = edge_label_div_style(0.0);
    let label_text = edge.label.trim();
    if label_text.is_empty() {
        write_empty_edge_label(
            out,
            &edge.id,
            ctx.html_labels,
            empty_edge_label_style.as_str(),
        );
        return;
    }

    let Some(le) = ctx.layout_edges_by_id.get(edge.id.as_str()).copied() else {
        return;
    };
    let Some(lbl) = le.label.as_ref() else {
        return;
    };

    let geometry = state_edge_prepare_geometry(
        ctx,
        le,
        Some(edge.arrow_type_end.as_str()),
        origin_x,
        origin_y,
    );
    let label_position = super::super::edge_label_geometry::position_edge_label(
        crate::model::LayoutPoint {
            x: lbl.x - origin_x,
            y: lbl.y - origin_y,
        },
        &geometry.label_path_points,
        &geometry.rendered_d,
        geometry.points_were_explicitly_updated,
    );
    let w = lbl.width.max(0.0);
    let h = lbl.height.max(0.0);

    write_visible_edge_label(
        out,
        &edge.id,
        label_text,
        label_position,
        w,
        h,
        ctx.html_labels,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_line_with_end_marker_offset_shortens_neo_barb_terminal_point() {
        let input = vec![
            crate::model::LayoutPoint { x: 0.0, y: 0.0 },
            crate::model::LayoutPoint { x: 10.0, y: 0.0 },
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
            crate::model::LayoutPoint { x: 0.0, y: 0.0 },
            crate::model::LayoutPoint { x: 0.0, y: 20.0 },
            crate::model::LayoutPoint { x: 20.0, y: 20.0 },
        ];
        let geometry = state_edge_finish_geometry(
            label_path_points.clone(),
            label_path_points.clone(),
            Some("arrow_barb_neo"),
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
        let position = crate::svg::parity::edge_label_geometry::position_edge_label(
            crate::model::LayoutPoint { x: 99.0, y: 99.0 },
            &geometry.label_path_points,
            &geometry.rendered_d,
            geometry.points_were_explicitly_updated,
        );
        assert_eq!((position.x, position.y), (0.0, 20.0));
    }
}
