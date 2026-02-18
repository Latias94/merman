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

    fn boundary_for_cluster(
        layout_clusters_by_id: &FxHashMap<&str, &LayoutCluster>,
        cluster_id: &str,
        translate_x: f64,
        translate_y: f64,
    ) -> Option<super::edge_geom::BoundaryNode> {
        let n = layout_clusters_by_id.get(cluster_id).copied()?;
        Some(super::edge_geom::BoundaryNode {
            x: n.x + translate_x,
            y: n.y + translate_y,
            width: n.width,
            height: n.height,
        })
    }

    let is_cyclic_special = edge.id.contains("-cyclic-special-");

    let mut points_for_render: Vec<crate::model::LayoutPoint> = Vec::new();
    super::edge_geom::dedup_consecutive_points_into(&local_points, &mut points_for_render);
    if let Some(tc) = le.to_cluster.as_deref() {
        if let Some(boundary) =
            boundary_for_cluster(layout_clusters_by_id, tc, translate_x, translate_y)
        {
            let mut tmp: Vec<crate::model::LayoutPoint> = Vec::new();
            super::edge_geom::cut_path_at_intersect_into(&points_for_render, &boundary, &mut tmp);
            points_for_render = tmp;
        }
    }
    if let Some(fc) = le.from_cluster.as_deref() {
        if let Some(boundary) =
            boundary_for_cluster(layout_clusters_by_id, fc, translate_x, translate_y)
        {
            let mut rev = points_for_render.clone();
            rev.reverse();
            let mut tmp: Vec<crate::model::LayoutPoint> = Vec::new();
            super::edge_geom::cut_path_at_intersect_into(&rev, &boundary, &mut tmp);
            rev = tmp;
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

    let line_data =
        super::edge_geom::line_with_offset_for_edge_type(&line_data, edge.edge_type.as_deref());

    let (d, pb, _skipped_bounds_for_viewbox) =
        super::edge_geom::curve_path_d_and_bounds(&line_data, interpolate, 0.0, 0.0, None);
    let pb = pb?;
    Some((d, pb))
}
