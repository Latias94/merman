//! Flowchart edge bbox/path helpers.
//!
//! This module computes the edge path `d` and its bounds (bbox). It is used by the flowchart
//! renderer for tasks like cluster label placement and viewBox sizing.

use super::*;
use crate::svg::parity;

pub(super) fn flowchart_edge_path_d_for_bbox(
    layout_edges_by_id: &FxHashMap<&str, &crate::model::LayoutEdge>,
    layout_clusters_by_id: &FxHashMap<&str, &LayoutCluster>,
    translate_x: f64,
    translate_y: f64,
    default_edge_interpolate: &str,
    edge_html_labels: bool,
    edge: &crate::flowchart::FlowEdge,
) -> Option<(String, parity::path_bounds::SvgPathBounds)> {
    flowchart_edge_path_d_for_bbox_impl(
        layout_edges_by_id,
        layout_clusters_by_id,
        translate_x,
        translate_y,
        default_edge_interpolate,
        edge_html_labels,
        edge,
    )
}

fn flowchart_edge_path_d_for_bbox_impl(
    layout_edges_by_id: &FxHashMap<&str, &crate::model::LayoutEdge>,
    layout_clusters_by_id: &FxHashMap<&str, &LayoutCluster>,
    translate_x: f64,
    translate_y: f64,
    default_edge_interpolate: &str,
    edge_html_labels: bool,
    edge: &crate::flowchart::FlowEdge,
) -> Option<(String, parity::path_bounds::SvgPathBounds)> {
    let le = layout_edges_by_id.get(edge.id.as_str()).copied()?;
    if le.points.len() < 2 {
        return None;
    }

    let mut local_points: Vec<crate::model::LayoutPoint> = Vec::new();
    for p in &le.points {
        local_points.push(crate::model::LayoutPoint {
            x: p.x + translate_x,
            y: p.y + translate_y,
        });
    }

    #[derive(Debug, Clone, Copy)]
    struct BoundaryNode {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    }

    fn outside_node(node: &BoundaryNode, point: &crate::model::LayoutPoint) -> bool {
        let dx = (point.x - node.x).abs();
        let dy = (point.y - node.y).abs();
        let w = node.width / 2.0;
        let h = node.height / 2.0;
        dx >= w || dy >= h
    }

    fn rect_intersection(
        node: &BoundaryNode,
        outside_point: &crate::model::LayoutPoint,
        inside_point: &crate::model::LayoutPoint,
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

    fn cut_path_at_intersect_into(
        input: &[crate::model::LayoutPoint],
        boundary: &BoundaryNode,
        out: &mut Vec<crate::model::LayoutPoint>,
    ) {
        out.clear();
        if input.is_empty() {
            return;
        }
        out.reserve(input.len() + 1);

        let mut last_point_outside = input[0].clone();
        let mut is_inside = false;
        const EPS: f64 = 1e-9;

        for point in input {
            if !outside_node(boundary, point) && !is_inside {
                let inter = rect_intersection(boundary, &last_point_outside, point);
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
                    out.push(crate::model::LayoutPoint {
                        x: point.x,
                        y: point.y,
                    });
                }
            }
        }
    }

    fn dedup_consecutive_points_into(
        input: &[crate::model::LayoutPoint],
        out: &mut Vec<crate::model::LayoutPoint>,
    ) {
        out.clear();
        if input.is_empty() {
            return;
        }
        const EPS: f64 = 1e-9;
        out.reserve(input.len());
        for p in input {
            if out
                .last()
                .is_some_and(|prev| (prev.x - p.x).abs() <= EPS && (prev.y - p.y).abs() <= EPS)
            {
                continue;
            }
            out.push(crate::model::LayoutPoint { x: p.x, y: p.y });
        }
    }

    fn cut_path_at_intersect(
        input: &[crate::model::LayoutPoint],
        boundary: &BoundaryNode,
    ) -> Vec<crate::model::LayoutPoint> {
        let mut out: Vec<crate::model::LayoutPoint> = Vec::new();
        cut_path_at_intersect_into(input, boundary, &mut out);
        out
    }

    fn dedup_consecutive_points(
        input: &[crate::model::LayoutPoint],
    ) -> Vec<crate::model::LayoutPoint> {
        let mut out: Vec<crate::model::LayoutPoint> = Vec::new();
        dedup_consecutive_points_into(input, &mut out);
        out
    }

    fn boundary_for_cluster(
        layout_clusters_by_id: &FxHashMap<&str, &LayoutCluster>,
        cluster_id: &str,
        translate_x: f64,
        translate_y: f64,
    ) -> Option<BoundaryNode> {
        let n = layout_clusters_by_id.get(cluster_id).copied()?;
        Some(BoundaryNode {
            x: n.x + translate_x,
            y: n.y + translate_y,
            width: n.width,
            height: n.height,
        })
    }

    let is_cyclic_special = edge.id.contains("-cyclic-special-");
    let local_points = dedup_consecutive_points(&local_points);
    let mut points_for_render = local_points.clone();
    if let Some(tc) = le.to_cluster.as_deref() {
        if let Some(boundary) =
            boundary_for_cluster(layout_clusters_by_id, tc, translate_x, translate_y)
        {
            points_for_render = cut_path_at_intersect(&points_for_render, &boundary);
        }
    }
    if let Some(fc) = le.from_cluster.as_deref() {
        if let Some(boundary) =
            boundary_for_cluster(layout_clusters_by_id, fc, translate_x, translate_y)
        {
            let mut rev = points_for_render.clone();
            rev.reverse();
            rev = cut_path_at_intersect(&rev, &boundary);
            rev.reverse();
            points_for_render = rev;
        }
    }

    let interpolate = edge
        .interpolate
        .as_deref()
        .unwrap_or(default_edge_interpolate);
    let is_basis = !matches!(
        interpolate,
        "linear"
            | "natural"
            | "step"
            | "stepAfter"
            | "stepBefore"
            | "cardinal"
            | "monotoneX"
            | "monotoneY"
    );

    let label_text = edge.label.as_deref().unwrap_or_default();
    let label_type = edge.label_type.as_deref().unwrap_or("text");
    let label_text_plain = flowchart_label_plain_text(label_text, label_type, edge_html_labels);
    let has_label_text = !label_text_plain.trim().is_empty();
    let is_cluster_edge = le.to_cluster.is_some() || le.from_cluster.is_some();

    fn all_triples_collinear(input: &[crate::model::LayoutPoint]) -> bool {
        if input.len() <= 2 {
            return true;
        }
        const EPS: f64 = 1e-9;
        for i in 1..input.len().saturating_sub(1) {
            let a = &input[i - 1];
            let b = &input[i];
            let c = &input[i + 1];
            let abx = b.x - a.x;
            let aby = b.y - a.y;
            let bcx = c.x - b.x;
            let bcy = c.y - b.y;
            if (abx * bcy - aby * bcx).abs() > EPS {
                return false;
            }
        }
        true
    }

    if is_basis
        && !has_label_text
        && !is_cyclic_special
        && edge.length <= 1
        && points_for_render.len() > 4
    {
        let fully_collinear = all_triples_collinear(&points_for_render);

        fn count_non_collinear_triples(input: &[crate::model::LayoutPoint]) -> usize {
            if input.len() < 3 {
                return 0;
            }
            const EPS: f64 = 1e-9;
            let mut count = 0usize;
            for i in 1..input.len().saturating_sub(1) {
                let a = &input[i - 1];
                let b = &input[i];
                let c = &input[i + 1];
                let abx = b.x - a.x;
                let aby = b.y - a.y;
                let bcx = c.x - b.x;
                let bcy = c.y - b.y;
                if (abx * bcy - aby * bcx).abs() > EPS {
                    count += 1;
                }
            }
            count
        }

        fn has_short_segment(input: &[crate::model::LayoutPoint], max_len: f64) -> bool {
            if input.len() < 2 {
                return false;
            }
            let max_len2 = max_len * max_len;
            for win in input.windows(2) {
                let a = &win[0];
                let b = &win[1];
                let dx = b.x - a.x;
                let dy = b.y - a.y;
                let d2 = dx * dx + dy * dy;
                if d2.is_finite() && d2 > 0.0 && d2 <= max_len2 {
                    return true;
                }
            }
            false
        }

        // Only collapse when the route includes a short clipped segment (usually introduced by
        // boundary cuts). If the straight run is made up of "normal" rank-to-rank steps, Mermaid
        // keeps those points and the `curveBasis` command sequence includes the extra `C`
        // segments.
        if !fully_collinear
            && count_non_collinear_triples(&points_for_render) <= 1
            && has_short_segment(&points_for_render, 10.0)
        {
            let a = points_for_render[0].clone();
            let mid = points_for_render[points_for_render.len() / 2].clone();
            let b = points_for_render[points_for_render.len() - 1].clone();
            points_for_render.clear();
            points_for_render.extend([a, mid, b]);
        }
    }

    if is_basis && is_cluster_edge && points_for_render.len() == 8 {
        const EPS: f64 = 1e-9;
        let len = points_for_render.len();
        let mut best_run: Option<(usize, usize)> = None;

        for axis in 0..2 {
            let mut i = 0usize;
            while i + 1 < len {
                let base = if axis == 0 {
                    points_for_render[i].x
                } else {
                    points_for_render[i].y
                };
                if (if axis == 0 {
                    points_for_render[i + 1].x
                } else {
                    points_for_render[i + 1].y
                } - base)
                    .abs()
                    > EPS
                {
                    i += 1;
                    continue;
                }

                let start = i;
                while i + 1 < len {
                    let v = if axis == 0 {
                        points_for_render[i + 1].x
                    } else {
                        points_for_render[i + 1].y
                    };
                    if (v - base).abs() > EPS {
                        break;
                    }
                    i += 1;
                }
                let end = i;
                if end + 1 - start >= 6 {
                    best_run = match best_run {
                        Some((bs, be)) if (be + 1 - bs) >= (end + 1 - start) => Some((bs, be)),
                        _ => Some((start, end)),
                    };
                }
                i += 1;
            }
        }

        if let Some((start, end)) = best_run {
            let idx = end.saturating_sub(1);
            if idx > start && idx > 0 && idx + 1 < len {
                points_for_render.remove(idx);
            }
        }
    }

    if is_basis
        && is_cyclic_special
        && edge.id.contains("-cyclic-special-mid")
        && points_for_render.len() > 3
    {
        points_for_render = vec![
            points_for_render[0].clone(),
            points_for_render[points_for_render.len() / 2].clone(),
            points_for_render[points_for_render.len() - 1].clone(),
        ];
    }
    if points_for_render.len() == 1 {
        points_for_render = local_points.clone();
    }

    if is_basis
        && points_for_render.len() == 2
        && interpolate != "linear"
        && (!is_cluster_edge || is_cyclic_special)
    {
        let a = &points_for_render[0];
        let b = &points_for_render[1];
        points_for_render.insert(
            1,
            crate::model::LayoutPoint {
                x: (a.x + b.x) / 2.0,
                y: (a.y + b.y) / 2.0,
            },
        );
    }

    if is_basis && is_cyclic_special {
        fn ensure_min_points(points: &mut Vec<crate::model::LayoutPoint>, min_len: usize) {
            if points.len() >= min_len || points.len() < 2 {
                return;
            }
            while points.len() < min_len {
                let mut best_i = 0usize;
                let mut best_d2 = -1.0f64;
                for i in 0..points.len().saturating_sub(1) {
                    let a = &points[i];
                    let b = &points[i + 1];
                    let dx = b.x - a.x;
                    let dy = b.y - a.y;
                    let d2 = dx * dx + dy * dy;
                    if d2 > best_d2 {
                        best_d2 = d2;
                        best_i = i;
                    }
                }
                let a = points[best_i].clone();
                let b = points[best_i + 1].clone();
                points.insert(
                    best_i + 1,
                    crate::model::LayoutPoint {
                        x: (a.x + b.x) / 2.0,
                        y: (a.y + b.y) / 2.0,
                    },
                );
            }
        }

        let cyclic_variant = if edge.id.ends_with("-cyclic-special-1") {
            Some(1u8)
        } else if edge.id.ends_with("-cyclic-special-2") {
            Some(2u8)
        } else {
            None
        };

        if let Some(variant) = cyclic_variant {
            let base_id = edge
                .id
                .split("-cyclic-special-")
                .next()
                .unwrap_or(edge.id.as_str());

            let should_expand = match layout_clusters_by_id.get(base_id) {
                Some(cluster) if cluster.effective_dir == "TB" || cluster.effective_dir == "TD" => {
                    variant == 1
                }
                Some(_) => variant == 2,
                None => variant == 2,
            };

            if should_expand {
                ensure_min_points(&mut points_for_render, 5);
            } else if points_for_render.len() == 4 {
                points_for_render.remove(1);
            }
        }
    }

    let mut line_data: Vec<crate::model::LayoutPoint> = points_for_render
        .iter()
        .filter(|p| !p.y.is_nan())
        .cloned()
        .collect();

    if !line_data.is_empty() {
        const CORNER_DIST: f64 = 5.0;
        let mut corner_positions: Vec<usize> = Vec::new();
        for i in 1..line_data.len().saturating_sub(1) {
            let prev = &line_data[i - 1];
            let curr = &line_data[i];
            let next = &line_data[i + 1];

            let is_corner_xy = prev.x == curr.x
                && curr.y == next.y
                && (curr.x - next.x).abs() > CORNER_DIST
                && (curr.y - prev.y).abs() > CORNER_DIST;
            let is_corner_yx = prev.y == curr.y
                && curr.x == next.x
                && (curr.x - prev.x).abs() > CORNER_DIST
                && (curr.y - next.y).abs() > CORNER_DIST;

            if is_corner_xy || is_corner_yx {
                corner_positions.push(i);
            }
        }

        if !corner_positions.is_empty() {
            fn find_adjacent_point(
                point_a: &crate::model::LayoutPoint,
                point_b: &crate::model::LayoutPoint,
                distance: f64,
            ) -> crate::model::LayoutPoint {
                let x_diff = point_b.x - point_a.x;
                let y_diff = point_b.y - point_a.y;
                let len = (x_diff * x_diff + y_diff * y_diff).sqrt();
                if len == 0.0 {
                    return point_b.clone();
                }
                let ratio = distance / len;
                crate::model::LayoutPoint {
                    x: point_b.x - ratio * x_diff,
                    y: point_b.y - ratio * y_diff,
                }
            }

            let a = (2.0_f64).sqrt() * 2.0;
            let mut new_line_data: Vec<crate::model::LayoutPoint> = Vec::new();
            for i in 0..line_data.len() {
                if !corner_positions.contains(&i) {
                    new_line_data.push(line_data[i].clone());
                    continue;
                }

                let prev = &line_data[i - 1];
                let next = &line_data[i + 1];
                let corner = &line_data[i];
                let new_prev = find_adjacent_point(prev, corner, CORNER_DIST);
                let new_next = find_adjacent_point(next, corner, CORNER_DIST);
                let x_diff = new_next.x - new_prev.x;
                let y_diff = new_next.y - new_prev.y;

                new_line_data.push(new_prev.clone());

                let mut new_corner = corner.clone();
                if (next.x - prev.x).abs() > 10.0 && (next.y - prev.y).abs() >= 10.0 {
                    let r = CORNER_DIST;
                    if corner.x == new_prev.x {
                        new_corner = crate::model::LayoutPoint {
                            x: if x_diff < 0.0 {
                                new_prev.x - r + a
                            } else {
                                new_prev.x + r - a
                            },
                            y: if y_diff < 0.0 {
                                new_prev.y - a
                            } else {
                                new_prev.y + a
                            },
                        };
                    } else {
                        new_corner = crate::model::LayoutPoint {
                            x: if x_diff < 0.0 {
                                new_prev.x - a
                            } else {
                                new_prev.x + a
                            },
                            y: if y_diff < 0.0 {
                                new_prev.y - r + a
                            } else {
                                new_prev.y + r - a
                            },
                        };
                    }
                }

                new_line_data.push(new_corner);
                new_line_data.push(new_next);
            }
            line_data = new_line_data;
        }
    }

    fn marker_offset_for(arrow_type: Option<&str>) -> Option<f64> {
        match arrow_type {
            Some("arrow_point") => Some(4.0),
            Some("dependency") => Some(6.0),
            Some("lollipop") => Some(13.5),
            Some("aggregation" | "extension" | "composition") => Some(17.25),
            _ => None,
        }
    }

    fn calculate_delta_and_angle(
        a: &crate::model::LayoutPoint,
        b: &crate::model::LayoutPoint,
    ) -> (f64, f64, f64) {
        let delta_x = b.x - a.x;
        let delta_y = b.y - a.y;
        let angle = (delta_y / delta_x).atan();
        (angle, delta_x, delta_y)
    }

    fn line_with_offset_points(
        input: &[crate::model::LayoutPoint],
        arrow_type_start: Option<&str>,
        arrow_type_end: Option<&str>,
    ) -> Vec<crate::model::LayoutPoint> {
        if input.len() < 2 {
            return input.to_vec();
        }

        let start = &input[0];
        let end = &input[input.len() - 1];

        let x_direction_is_left = start.x < end.x;
        let y_direction_is_down = start.y < end.y;
        let extra_room = 1.0;

        let start_marker_height = marker_offset_for(arrow_type_start);
        let end_marker_height = marker_offset_for(arrow_type_end);

        let mut out = Vec::with_capacity(input.len());
        for (i, p) in input.iter().enumerate() {
            let mut ox = 0.0;
            let mut oy = 0.0;

            if i == 0 {
                if let Some(h) = start_marker_height {
                    let (angle, delta_x, delta_y) = calculate_delta_and_angle(&input[0], &input[1]);
                    ox = h * angle.cos() * if delta_x >= 0.0 { 1.0 } else { -1.0 };
                    oy = h * angle.sin().abs() * if delta_y >= 0.0 { 1.0 } else { -1.0 };
                }
            } else if i == input.len() - 1 {
                if let Some(h) = end_marker_height {
                    let (angle, delta_x, delta_y) =
                        calculate_delta_and_angle(&input[input.len() - 1], &input[input.len() - 2]);
                    ox = h * angle.cos() * if delta_x >= 0.0 { 1.0 } else { -1.0 };
                    oy = h * angle.sin().abs() * if delta_y >= 0.0 { 1.0 } else { -1.0 };
                }
            }

            if let Some(h) = end_marker_height {
                let diff_x = (p.x - end.x).abs();
                let diff_y = (p.y - end.y).abs();
                if diff_x < h && diff_x > 0.0 && diff_y < h {
                    let mut adjustment = h + extra_room - diff_x;
                    adjustment *= if !x_direction_is_left { -1.0 } else { 1.0 };
                    ox -= adjustment;
                }
            }
            if let Some(h) = start_marker_height {
                let diff_x = (p.x - start.x).abs();
                let diff_y = (p.y - start.y).abs();
                if diff_x < h && diff_x > 0.0 && diff_y < h {
                    let mut adjustment = h + extra_room - diff_x;
                    adjustment *= if !x_direction_is_left { -1.0 } else { 1.0 };
                    ox += adjustment;
                }
            }

            if let Some(h) = end_marker_height {
                let diff_y = (p.y - end.y).abs();
                let diff_x = (p.x - end.x).abs();
                if diff_y < h && diff_y > 0.0 && diff_x < h {
                    let mut adjustment = h + extra_room - diff_y;
                    adjustment *= if !y_direction_is_down { -1.0 } else { 1.0 };
                    oy -= adjustment;
                }
            }
            if let Some(h) = start_marker_height {
                let diff_y = (p.y - start.y).abs();
                let diff_x = (p.x - start.x).abs();
                if diff_y < h && diff_y > 0.0 && diff_x < h {
                    let mut adjustment = h + extra_room - diff_y;
                    adjustment *= if !y_direction_is_down { -1.0 } else { 1.0 };
                    oy += adjustment;
                }
            }

            out.push(crate::model::LayoutPoint {
                x: p.x + ox,
                y: p.y + oy,
            });
        }
        out
    }

    let arrow_type_start = match edge.edge_type.as_deref() {
        Some("double_arrow_point") => Some("arrow_point"),
        Some("double_arrow_circle") => Some("arrow_circle"),
        Some("double_arrow_cross") => Some("arrow_cross"),
        _ => None,
    };
    let arrow_type_end = match edge.edge_type.as_deref() {
        Some("arrow_open") => None,
        Some("arrow_cross") => Some("arrow_cross"),
        Some("arrow_circle") => Some("arrow_circle"),
        Some("double_arrow_point" | "arrow_point") => Some("arrow_point"),
        Some("double_arrow_circle") => Some("arrow_circle"),
        Some("double_arrow_cross") => Some("arrow_cross"),
        _ => Some("arrow_point"),
    };
    let line_data = line_with_offset_points(&line_data, arrow_type_start, arrow_type_end);

    let (d, pb) = match interpolate {
        "linear" => parity::curve::curve_linear_path_d_and_bounds(&line_data),
        "natural" => parity::curve::curve_natural_path_d_and_bounds(&line_data),
        "bumpY" => parity::curve::curve_bump_y_path_d_and_bounds(&line_data),
        "catmullRom" => parity::curve::curve_catmull_rom_path_d_and_bounds(&line_data),
        "step" => parity::curve::curve_step_path_d_and_bounds(&line_data),
        "stepAfter" => parity::curve::curve_step_after_path_d_and_bounds(&line_data),
        "stepBefore" => parity::curve::curve_step_before_path_d_and_bounds(&line_data),
        "cardinal" => parity::curve::curve_cardinal_path_d_and_bounds(&line_data, 0.0),
        "monotoneX" => parity::curve::curve_monotone_path_d_and_bounds(&line_data, false),
        "monotoneY" => parity::curve::curve_monotone_path_d_and_bounds(&line_data, true),
        _ => parity::curve::curve_basis_path_d_and_bounds(&line_data),
    };
    let pb = pb?;
    Some((d, pb))
}
