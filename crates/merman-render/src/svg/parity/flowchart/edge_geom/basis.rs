//! Basis curve route simplifications.
//!
//! Mermaid uses Dagre routes combined with D3 curve interpolators (notably `curveBasis`). For a
//! few edge/cluster cases Mermaid's emitted `d` command sequence assumes the route was simplified
//! prior to interpolation. These helpers keep our headless output aligned with upstream.

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

pub(in crate::svg::parity::flowchart) fn maybe_collapse_straight_except_one_endpoint(
    points: &mut Vec<crate::model::LayoutPoint>,
) {
    if points.len() <= 4 {
        return;
    }

    let fully_collinear = all_triples_collinear(points);
    if fully_collinear {
        return;
    }

    // Only collapse when the route includes a short clipped segment (usually introduced by
    // boundary cuts). If the straight run is made up of "normal" rank-to-rank steps, Mermaid
    // keeps those points and the `curveBasis` command sequence includes the extra `C` segments.
    if count_non_collinear_triples(points) <= 1 && has_short_segment(points, 10.0) {
        let a = points[0].clone();
        let mid = points[points.len() / 2].clone();
        let b = points[points.len() - 1].clone();
        points.clear();
        points.extend([a, mid, b]);
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
