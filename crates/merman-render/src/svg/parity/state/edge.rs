use super::*;
use crate::generated::state_text_overrides_11_12_2 as state_text_overrides;

#[derive(Debug, Clone, Copy)]
struct StateEdgeBoundaryNode {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

fn state_edge_dedup_consecutive_points(
    input: &[crate::model::LayoutPoint],
) -> Vec<crate::model::LayoutPoint> {
    if input.len() <= 1 {
        return input.to_vec();
    }
    const EPS: f64 = 1e-9;
    let mut out: Vec<crate::model::LayoutPoint> = Vec::with_capacity(input.len());
    for p in input {
        if out
            .last()
            .is_some_and(|prev| (prev.x - p.x).abs() <= EPS && (prev.y - p.y).abs() <= EPS)
        {
            continue;
        }
        out.push(p.clone());
    }
    out
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
    if le.from != le.to || le.from_cluster.is_some() || le.to_cluster.is_some() || input.len() < 3 {
        return None;
    }
    if ctx.layout_clusters_by_id.contains_key(le.from.as_str()) {
        return None;
    }
    if ctx
        .nodes_by_id
        .get(le.from.as_str())
        .copied()
        .is_some_and(|node| node.is_group && node.shape != "noteGroup")
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

fn state_edge_prepare_points(
    ctx: &StateRenderCtx<'_>,
    le: &crate::model::LayoutEdge,
    edge_id: &str,
    arrow_type_end: Option<&str>,
    origin_x: f64,
    origin_y: f64,
) -> (
    Vec<crate::model::LayoutPoint>,
    Vec<crate::model::LayoutPoint>,
) {
    let mut raw_local_points: Vec<crate::model::LayoutPoint> = Vec::new();
    for p in &le.points {
        raw_local_points.push(crate::model::LayoutPoint {
            x: p.x - origin_x,
            y: p.y - origin_y,
        });
    }
    let local_points =
        state_edge_clip_self_loop_points_to_node(ctx, le, &raw_local_points, origin_x, origin_y)
            .unwrap_or(raw_local_points);

    let is_cyclic_special = edge_id.contains("-cyclic-special-");
    let mut points_for_curve = if is_cyclic_special {
        state_edge_dedup_consecutive_points(&local_points)
    } else {
        local_points.clone()
    };

    // Match Mermaid `dagre-wrapper/edges.js insertEdge`: cut the path at cluster boundaries when the
    // edge connects to a cluster.
    if let Some(tc) = le.to_cluster.as_deref()
        && let Some(boundary) = state_edge_boundary_for_cluster(ctx, tc, origin_x, origin_y)
    {
        points_for_curve = state_edge_cut_path_at_intersect(&points_for_curve, &boundary);
    }
    if let Some(fc) = le.from_cluster.as_deref()
        && let Some(boundary) = state_edge_boundary_for_cluster(ctx, fc, origin_x, origin_y)
    {
        let mut rev = points_for_curve;
        rev.reverse();
        rev = state_edge_cut_path_at_intersect(&rev, &boundary);
        rev.reverse();
        points_for_curve = rev;
    }

    if is_cyclic_special {
        if edge_id.contains("-cyclic-special-mid") && points_for_curve.len() > 3 {
            points_for_curve = vec![
                points_for_curve[0].clone(),
                points_for_curve[points_for_curve.len() / 2].clone(),
                points_for_curve[points_for_curve.len() - 1].clone(),
            ];
        }
        if points_for_curve.len() == 4 {
            // Mermaid's cyclic-special helper edges frequently collapse the 4-point basis
            // case into the 3-point command sequence (`C` count = 2).
            points_for_curve.remove(1);
        }
        if edge_id.ends_with("-cyclic-special-2") && points_for_curve.len() == 6 {
            // Some cyclic-special-2 helper edges are routed with 6 points but Mermaid's path
            // command sequence matches the 5-point `curveBasis` case (`C` count = 4).
            points_for_curve.remove(1);
        }
    }
    points_for_curve = state_edge_fix_corners(&points_for_curve);
    points_for_curve = state_line_with_end_marker_offset_points(&points_for_curve, arrow_type_end);

    (local_points, points_for_curve)
}

fn state_edge_encode_path(
    ctx: &StateRenderCtx<'_>,
    le: &crate::model::LayoutEdge,
    edge_id: &str,
    arrow_type_end: Option<&str>,
    origin_x: f64,
    origin_y: f64,
) -> (String, String) {
    let (local_points, points_for_curve) =
        state_edge_prepare_points(ctx, le, edge_id, arrow_type_end, origin_x, origin_y);

    let data_points = base64::engine::general_purpose::STANDARD
        .encode(serde_json::to_vec(&local_points).unwrap_or_default());
    let d = curve_basis_path_d(&points_for_curve);
    (d, data_points)
}

fn state_self_loop_node_bounds(
    ctx: &StateRenderCtx<'_>,
    node_id: &str,
) -> Option<StateEdgeBoundaryNode> {
    if let Some(cluster) = ctx.layout_clusters_by_id.get(node_id).copied() {
        return Some(StateEdgeBoundaryNode {
            x: cluster.x,
            y: cluster.y,
            width: cluster.width,
            height: cluster.height,
        });
    }
    ctx.layout_nodes_by_id
        .get(node_id)
        .copied()
        .map(|node| StateEdgeBoundaryNode {
            x: node.x,
            y: node.y,
            width: node.width,
            height: node.height,
        })
}

fn state_self_loop_default_side(ctx: &StateRenderCtx<'_>, node_id: &str) -> &'static str {
    let dir = ctx
        .nodes_by_id
        .get(node_id)
        .and_then(|node| node.dir.as_deref())
        .unwrap_or("TB");
    match dir {
        "BT" => "bottom",
        "LR" => "right",
        "RL" => "left",
        _ => "top",
    }
}

fn state_self_loop_side(
    ctx: &StateRenderCtx<'_>,
    node_id: &str,
    node: &StateEdgeBoundaryNode,
) -> &'static str {
    let special_id1 = format!("{node_id}---{node_id}---1");
    let special_id2 = format!("{node_id}---{node_id}---2");
    let mut hints: Vec<crate::model::LayoutPoint> = Vec::new();

    for id in [special_id1.as_str(), special_id2.as_str()] {
        if let Some(dummy) = ctx.layout_nodes_by_id.get(id).copied() {
            hints.push(crate::model::LayoutPoint {
                x: dummy.x,
                y: dummy.y,
            });
        }
    }

    if hints.is_empty() {
        let id1 = format!("{node_id}-cyclic-special-1");
        let idm = format!("{node_id}-cyclic-special-mid");
        let id2 = format!("{node_id}-cyclic-special-2");
        for id in [id1.as_str(), idm.as_str(), id2.as_str()] {
            if let Some(edge) = ctx.layout_edges_by_id.get(id).copied() {
                hints.extend(edge.points.iter().cloned());
            }
        }
    }

    if hints.is_empty() {
        return state_self_loop_default_side(ctx, node_id);
    }

    let len = hints.len() as f64;
    let center_x = hints.iter().map(|p| p.x).sum::<f64>() / len;
    let center_y = hints.iter().map(|p| p.y).sum::<f64>() / len;
    let dx = center_x - node.x;
    let dy = center_y - node.y;
    if dx.abs() > dy.abs() {
        if dx > 0.0 { "right" } else { "left" }
    } else if dy.abs() > 0.0 {
        if dy > 0.0 { "bottom" } else { "top" }
    } else {
        state_self_loop_default_side(ctx, node_id)
    }
}

fn state_self_loop_points(
    node: &StateEdgeBoundaryNode,
    side: &str,
    label_width: f64,
) -> Vec<crate::model::LayoutPoint> {
    let x = node.x;
    let y = node.y;
    let half_width = node.width / 2.0;
    let half_height = node.height / 2.0;
    let max_span = 36.0_f64.max(100.0_f64.min(node.width * 0.8));
    let span = label_width.max(node.width * 0.35).clamp(36.0, max_span);
    let depth = node.width.min(node.height) * 0.45;
    let depth = depth.clamp(24.0, 48.0);

    match side {
        "bottom" => {
            let bottom = y + half_height;
            vec![
                crate::model::LayoutPoint {
                    x: x - span / 2.0,
                    y: bottom,
                },
                crate::model::LayoutPoint {
                    x: x - span / 2.0,
                    y: bottom + depth,
                },
                crate::model::LayoutPoint {
                    x: x + span / 2.0,
                    y: bottom + depth,
                },
                crate::model::LayoutPoint {
                    x: x + span / 2.0,
                    y: bottom,
                },
            ]
        }
        "right" => {
            let right = x + half_width;
            vec![
                crate::model::LayoutPoint {
                    x: right,
                    y: y - span / 2.0,
                },
                crate::model::LayoutPoint {
                    x: right + depth,
                    y: y - span / 2.0,
                },
                crate::model::LayoutPoint {
                    x: right + depth,
                    y: y + span / 2.0,
                },
                crate::model::LayoutPoint {
                    x: right,
                    y: y + span / 2.0,
                },
            ]
        }
        "left" => {
            let left = x - half_width;
            vec![
                crate::model::LayoutPoint {
                    x: left,
                    y: y - span / 2.0,
                },
                crate::model::LayoutPoint {
                    x: left - depth,
                    y: y - span / 2.0,
                },
                crate::model::LayoutPoint {
                    x: left - depth,
                    y: y + span / 2.0,
                },
                crate::model::LayoutPoint {
                    x: left,
                    y: y + span / 2.0,
                },
            ]
        }
        _ => {
            let top = y - half_height;
            vec![
                crate::model::LayoutPoint {
                    x: x - span / 2.0,
                    y: top,
                },
                crate::model::LayoutPoint {
                    x: x - span / 2.0,
                    y: top - depth,
                },
                crate::model::LayoutPoint {
                    x: x + span / 2.0,
                    y: top - depth,
                },
                crate::model::LayoutPoint {
                    x: x + span / 2.0,
                    y: top,
                },
            ]
        }
    }
}

fn state_self_loop_label(
    points: &[crate::model::LayoutPoint],
    side: &str,
    width: f64,
    height: f64,
) -> crate::model::LayoutLabel {
    let gap = 4.0;
    let x_min = points
        .iter()
        .map(|p| p.x)
        .fold(f64::INFINITY, |acc, x| acc.min(x));
    let x_max = points
        .iter()
        .map(|p| p.x)
        .fold(f64::NEG_INFINITY, |acc, x| acc.max(x));
    let y_min = points
        .iter()
        .map(|p| p.y)
        .fold(f64::INFINITY, |acc, y| acc.min(y));
    let y_max = points
        .iter()
        .map(|p| p.y)
        .fold(f64::NEG_INFINITY, |acc, y| acc.max(y));
    let x_center = (x_min + x_max) / 2.0;
    let y_center = (y_min + y_max) / 2.0;

    let (x, y) = match side {
        "bottom" => (x_center, y_max + height / 2.0 + gap),
        "right" => (x_max + width / 2.0 + gap, y_center),
        "left" => (x_min - width / 2.0 - gap, y_center),
        _ => (x_center, y_min - height / 2.0 - gap),
    };

    crate::model::LayoutLabel {
        x,
        y,
        width,
        height,
    }
}

fn state_self_loop_layout_edge(
    ctx: &StateRenderCtx<'_>,
    edge: &StateSvgEdge,
) -> Option<crate::model::LayoutEdge> {
    if edge.start != edge.end {
        return None;
    }
    let node_id = edge.start.as_str();
    let node = state_self_loop_node_bounds(ctx, node_id)?;
    let mid_id = format!("{node_id}-cyclic-special-mid");
    let mid_label = ctx
        .layout_edges_by_id
        .get(mid_id.as_str())
        .and_then(|edge| edge.label.clone())
        .or_else(|| {
            ctx.layout_edges_by_id
                .get(edge.id.as_str())
                .and_then(|edge| edge.label.clone())
        });
    let label_width = mid_label.as_ref().map(|label| label.width).unwrap_or(0.0);
    let label_height = mid_label.as_ref().map(|label| label.height).unwrap_or(0.0);
    let side = state_self_loop_side(ctx, node_id, &node);
    let points = state_self_loop_points(&node, side, label_width);
    let label = state_self_loop_label(&points, side, label_width, label_height);
    Some(crate::model::LayoutEdge {
        id: edge.id.clone(),
        from: edge.start.clone(),
        to: edge.end.clone(),
        from_cluster: None,
        to_cluster: None,
        points,
        label: Some(label),
        start_label_left: None,
        start_label_right: None,
        end_label_left: None,
        end_label_right: None,
        start_marker: None,
        end_marker: None,
        stroke_dasharray: None,
    })
}

pub(super) fn render_state_edge_path(
    out: &mut String,
    ctx: &StateRenderCtx<'_>,
    edge: &StateSvgEdge,
    origin_x: f64,
    origin_y: f64,
) {
    let data_look = state_data_look(ctx);
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

    if edge.start == edge.end {
        let Some(le) = state_self_loop_layout_edge(ctx, edge) else {
            return;
        };
        if le.points.len() < 2 {
            return;
        }

        let (d, data_points) = state_edge_encode_path(
            ctx,
            &le,
            edge.id.as_str(),
            marker_end.as_ref().map(|_| edge.arrow_type_end.as_str()),
            origin_x,
            origin_y,
        );
        let _ = write!(
            out,
            r#"<path d="{}" id="{}" class="{}" style="fill:none;;;fill:none" data-edge="true" data-et="edge" data-id="{}" data-points="{}" data-look="{}""#,
            d,
            escape_xml_display(&state_scoped_dom_id(ctx, edge.id.as_str())),
            escape_xml_display(&classes),
            escape_xml_display(edge.id.as_str()),
            data_points,
            escape_xml_display(data_look)
        );
        if let Some(m) = marker_end {
            let _ = write!(out, r#" marker-end="{}""#, escape_xml_display(&m));
        }
        out.push_str("/>");
        return;
    }

    let Some(le) = ctx.layout_edges_by_id.get(edge.id.as_str()).copied() else {
        return;
    };
    if le.points.len() < 2 {
        return;
    }

    let (d, data_points) = state_edge_encode_path(
        ctx,
        le,
        edge.id.as_str(),
        Some(edge.arrow_type_end.as_str()),
        origin_x,
        origin_y,
    );

    let _ = write!(
        out,
        r#"<path d="{}" id="{}" class="{}" style="fill:none;;;fill:none" data-edge="true" data-et="edge" data-id="{}" data-points="{}" data-look="{}""#,
        d,
        escape_xml_display(&state_scoped_dom_id(ctx, edge.id.as_str())),
        escape_xml_display(&classes),
        escape_xml_display(&edge.id),
        data_points,
        escape_xml_display(data_look)
    );
    if let Some(m) = marker_end {
        let _ = write!(out, r#" marker-end="{}""#, escape_xml_display(&m));
    }
    out.push_str("/>");
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
        let max_width = state_text_overrides::state_edge_label_max_width_px();
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

    fn mermaid_round_number(num: f64, precision: i32) -> f64 {
        let factor = 10_f64.powi(precision);
        (num * factor).round() / factor
    }

    fn mermaid_distance(
        point: &crate::model::LayoutPoint,
        prev: Option<&crate::model::LayoutPoint>,
    ) -> f64 {
        let Some(prev) = prev else {
            return 0.0;
        };
        ((point.x - prev.x).powi(2) + (point.y - prev.y).powi(2)).sqrt()
    }

    fn mermaid_calculate_point(
        points: &[crate::model::LayoutPoint],
        distance_to_traverse: f64,
    ) -> Option<crate::model::LayoutPoint> {
        let mut prev: Option<&crate::model::LayoutPoint> = None;
        let mut remaining = distance_to_traverse;
        for point in points {
            if let Some(prev_point) = prev {
                let vector_distance = mermaid_distance(point, Some(prev_point));
                if vector_distance == 0.0 {
                    return Some(prev_point.clone());
                }
                if vector_distance < remaining {
                    remaining -= vector_distance;
                } else {
                    let distance_ratio = remaining / vector_distance;
                    if distance_ratio <= 0.0 {
                        return Some(prev_point.clone());
                    }
                    if distance_ratio >= 1.0 {
                        return Some(point.clone());
                    }
                    if distance_ratio > 0.0 && distance_ratio < 1.0 {
                        return Some(crate::model::LayoutPoint {
                            x: mermaid_round_number(
                                (1.0 - distance_ratio) * prev_point.x + distance_ratio * point.x,
                                5,
                            ),
                            y: mermaid_round_number(
                                (1.0 - distance_ratio) * prev_point.y + distance_ratio * point.y,
                                5,
                            ),
                        });
                    }
                }
            }
            prev = Some(point);
        }
        None
    }

    fn mermaid_calc_label_position(
        points: &[crate::model::LayoutPoint],
    ) -> Option<crate::model::LayoutPoint> {
        if points.is_empty() {
            return None;
        }
        if points.len() == 1 {
            return Some(points[0].clone());
        }

        let mut total_distance: f64 = 0.0;
        let mut prev: Option<&crate::model::LayoutPoint> = None;
        for point in points {
            total_distance += mermaid_distance(point, prev);
            prev = Some(point);
        }

        let remaining_distance = total_distance / 2.0;
        mermaid_calculate_point(points, remaining_distance)
    }

    let empty_edge_label_style = edge_label_div_style(0.0);
    let label_text = edge.label.trim();
    if edge.start == edge.end {
        if label_text.is_empty() {
            write_empty_edge_label(
                out,
                &edge.id,
                ctx.html_labels,
                empty_edge_label_style.as_str(),
            );
            return;
        }

        if let Some(le) = state_self_loop_layout_edge(ctx, edge)
            && let Some(lbl) = le.label.as_ref()
        {
            write_visible_edge_label(
                out,
                &edge.id,
                label_text,
                crate::model::LayoutPoint {
                    x: lbl.x - origin_x,
                    y: lbl.y - origin_y,
                },
                lbl.width,
                lbl.height,
                ctx.html_labels,
            );
        }
        return;
    }

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

    let mut cx = lbl.x - origin_x;
    let mut cy = lbl.y - origin_y;

    // Mermaid `rendering-elements/edges.js insertEdge` sets `paths.updatedPath` when:
    // - cluster cutting happened (`toCluster` / `fromCluster`)
    // - or the mid point would not be present in the rendered `d` string (curveBasis does not
    //   pass through all control points; labels anchored on those points drift).
    //
    // `positionEdgeLabel` then recomputes the label center from `utils.calcLabelPosition(...)`
    // *only when* `updatedPath` exists. Otherwise it keeps Dagre's `edge.x/y` unchanged.
    let (_local_points, points_for_curve) = state_edge_prepare_points(
        ctx,
        le,
        edge.id.as_str(),
        Some(edge.arrow_type_end.as_str()),
        origin_x,
        origin_y,
    );

    fn mermaid_is_label_coordinate_in_path(
        point: &crate::model::LayoutPoint,
        d_attr: &str,
    ) -> bool {
        let rounded_x = point.x.round() as i64;
        let rounded_y = point.y.round() as i64;

        let bytes = d_attr.as_bytes();
        let mut i = 0usize;
        while i < bytes.len() {
            let b = bytes[i];
            let is_start = b.is_ascii_digit() || b == b'-' || b == b'.';
            if !is_start {
                i += 1;
                continue;
            }

            let start = i;
            i += 1;
            while i < bytes.len() {
                let b = bytes[i];
                if b.is_ascii_digit() || b == b'.' {
                    i += 1;
                    continue;
                }
                break;
            }

            let token = &d_attr[start..i];
            if let Ok(v) = token.parse::<f64>() {
                let r = v.round() as i64;
                if r == rounded_x || r == rounded_y {
                    return true;
                }
            }
        }

        false
    }

    let mut points_has_changed = le.to_cluster.is_some() || le.from_cluster.is_some();
    if !points_has_changed && !points_for_curve.is_empty() {
        let d_attr = curve_basis_path_d(&points_for_curve);
        let mid = &points_for_curve[points_for_curve.len() / 2];
        if !mermaid_is_label_coordinate_in_path(mid, &d_attr) {
            points_has_changed = true;
        }
    }

    if points_has_changed && let Some(pos) = mermaid_calc_label_position(&points_for_curve) {
        cx = pos.x;
        cy = pos.y;
    }
    let w = lbl.width.max(0.0);
    let h = lbl.height.max(0.0);

    write_visible_edge_label(
        out,
        &edge.id,
        label_text,
        crate::model::LayoutPoint { x: cx, y: cy },
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
}
