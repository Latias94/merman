//! Basis curve route simplifications.
//!
//! Mermaid uses Dagre routes combined with D3 curve interpolators (notably `curveBasis`). For a
//! few edge/cluster cases Mermaid's emitted `d` command sequence assumes the route was simplified
//! prior to interpolation. These helpers keep our headless output aligned with upstream.

use super::super::*;

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

pub(in crate::svg::parity::flowchart) fn maybe_remove_redundant_cluster_run_point(
    points: &mut Vec<crate::model::LayoutPoint>,
) {
    if points.len() != 8 {
        return;
    }

    const EPS: f64 = 1e-9;
    let len = points.len();
    let mut best_run: Option<(usize, usize)> = None;

    // Find the longest axis-aligned run (same x or same y) of consecutive points.
    for axis in 0..2 {
        let mut i = 0usize;
        while i + 1 < len {
            let base = if axis == 0 { points[i].x } else { points[i].y };
            if ((if axis == 0 {
                points[i + 1].x
            } else {
                points[i + 1].y
            }) - base)
                .abs()
                > EPS
            {
                i += 1;
                continue;
            }

            let start = i;
            while i + 1 < len {
                let v = if axis == 0 {
                    points[i + 1].x
                } else {
                    points[i + 1].y
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
            points.remove(idx);
        }
    }
}

pub(in crate::svg::parity::flowchart) fn maybe_insert_midpoint_for_basis(
    points: &mut Vec<crate::model::LayoutPoint>,
    interpolate: &str,
    is_cluster_edge: bool,
    is_cyclic_special: bool,
) {
    // Mermaid's Dagre pipeline typically provides at least one intermediate point even for
    // straight-looking edges, resulting in `C` segments in the SVG `d`. To keep our output closer
    // to Mermaid's command sequence, re-insert a midpoint when our route collapses to two points
    // after normalization (but keep cluster-adjacent edges as-is: Mermaid uses straight segments
    // there).
    if points.len() == 2 && interpolate != "linear" && (!is_cluster_edge || is_cyclic_special) {
        let a = &points[0];
        let b = &points[1];
        points.insert(
            1,
            crate::model::LayoutPoint {
                x: (a.x + b.x) / 2.0,
                y: (a.y + b.y) / 2.0,
            },
        );
    }
}

pub(in crate::svg::parity::flowchart) fn maybe_pad_cyclic_special_basis_route(
    ctx: &FlowchartRenderCtx<'_>,
    edge: &crate::flowchart::FlowEdge,
    points: &mut Vec<crate::model::LayoutPoint>,
) {
    // Mermaid's cyclic self-loop helper edges (`*-cyclic-special-{1,2}`) sometimes use longer
    // routed point lists. When our layout collapses these helper edges to a short polyline, D3's
    // `basis` interpolation produces fewer cubic segments than Mermaid (`C` command count
    // mismatch in SVG `d`).
    //
    // Mermaid's behavior differs depending on whether the base node is a cluster and on the
    // cluster's effective direction. Recreate the command sequence by padding the polyline to at
    // least 5 points (so `curveBasis` emits 4 `C` segments) only for the variants that Mermaid
    // expands.

    maybe_pad_cyclic_special_basis_route_for_layout_clusters(
        &ctx.layout_clusters_by_id,
        edge,
        points,
    );
}

fn maybe_pad_cyclic_special_basis_route_for_layout_clusters(
    layout_clusters_by_id: &rustc_hash::FxHashMap<&str, &LayoutCluster>,
    edge: &crate::flowchart::FlowEdge,
    points: &mut Vec<crate::model::LayoutPoint>,
) {
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
            ensure_min_points(points, 5);
        } else if points.len() == 4 {
            // For non-expanded cyclic helper edges, Mermaid's command sequence matches the
            // 3-point `curveBasis` case (`C` count = 2). Avoid emitting the intermediate
            // 4-point variant (`C` count = 3).
            points.remove(1);
        }
    }
}
