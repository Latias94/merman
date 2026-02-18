//! Cyclic-special helpers.
//!
//! Mermaid expands flowchart self-loop edges into helper nodes and `*-cyclic-special-*` segments.
//! In strict SVG DOM parity mode we sometimes need to mirror tiny DOM/layout measurement artifacts
//! for those helper-node endpoints.

use super::super::*;

fn ceil_grid(v: f64, scale: f64) -> f64 {
    if !(v.is_finite() && scale.is_finite() && scale > 0.0) {
        return v;
    }
    (v * scale).ceil() / scale
}

fn frac_scaled(v: f64, scale: f64) -> Option<f64> {
    if !(v.is_finite() && scale.is_finite() && scale > 0.0) {
        return None;
    }
    let scaled = v * scale;
    let frac = scaled - scaled.floor();
    if frac.is_finite() { Some(frac) } else { None }
}

fn should_promote(frac: f64) -> bool {
    frac.is_finite() && frac > 1e-4 && frac < 1e-3
}

fn is_close_to_rounded(v: f64, digits: u32) -> Option<f64> {
    if !v.is_finite() {
        return None;
    }
    let pow10 = 10_f64.powi(digits as i32);
    let rounded = (v * pow10).round() / pow10;
    if (v - rounded).abs() <= 5e-6 {
        Some(rounded)
    } else {
        None
    }
}

pub(in crate::svg::parity::flowchart) fn normalized_boundary_for_node(
    ctx: &FlowchartRenderCtx<'_>,
    node_id: &str,
    origin_x: f64,
    origin_y: f64,
    eps: f64,
    step: f64,
) -> Option<super::BoundaryNode> {
    let n = ctx.layout_nodes_by_id.get(node_id)?;
    let mut x = n.x + ctx.tx - origin_x;
    let mut y = n.y + ctx.ty - origin_y;
    let mut width = n.width;
    let mut height = n.height;

    // Cluster rectangles go through DOM/layout measurement pipelines upstream and commonly land
    // on an f32 lattice. Mirror that for cyclic-special endpoint intersections to match strict
    // `data-points` parity.
    if n.is_cluster {
        x = (x as f32) as f64;
        y = (y as f32) as f64;
        width = (width as f32) as f64;
        height = (height as f32) as f64;
    }

    let x_frac_40960 = frac_scaled(x, 40960.0);
    let promote_x_40960 = x_frac_40960.is_some_and(should_promote);
    let x_on_40960_grid = x_frac_40960.is_some_and(|f| f.abs() <= 1e-12);
    if promote_x_40960 {
        // Mermaid uses tiny `labelRect` helper nodes for cyclic-special edges. Those nodes carry a
        // tiny per-node offset in upstream output:
        // - `...---1` nodes are slightly smaller (`-step`)
        // - `...---2` nodes align to the promoted tick
        x = if node_id.contains("---") {
            if node_id.ends_with("---1") {
                ceil_grid(x, 40960.0) - step
            } else {
                ceil_grid(x, 40960.0)
            }
        } else {
            ceil_grid(x, 40960.0)
        };
    }

    if node_id.contains("---") && (y - y.round()).abs() <= 1e-6 {
        let scale = 40960.0;
        if let Some(frac) = frac_scaled(y, scale) {
            if should_promote(frac) || frac.abs() <= 1e-12 {
                let scaled = y * scale;
                let base = scaled.floor();
                let tick = if frac.abs() <= 1e-12 {
                    (base + 1.0) / scale
                } else {
                    scaled.ceil() / scale
                };
                y = tick + eps;
            }
        }
    } else if let Some(rounded) = is_close_to_rounded(y, 1) {
        let f32_candidate = (rounded as f32) as f64;
        y = if f32_candidate >= y {
            f32_candidate
        } else {
            ceil_grid(y, 81920.0) + eps
        };
    } else if let Some(rounded) = is_close_to_rounded(y, 2) {
        let as_int = (rounded * 100.0).round() as i64;
        if as_int % 10 == 5 {
            let rounded_f32 = (rounded as f32) as f64;
            let promote_40960 =
                frac_scaled(y, 40960.0).is_some_and(|f| should_promote(f) || f.abs() <= 1e-12);
            if promote_40960 || (y - rounded_f32).abs() <= 1e-9 {
                // Node centers for these helper nodes go through a different DOM/measurement
                // lattice than edge points: upstream ends up with an additional `eps` shift
                // relative to the `data-points` y-normalization rules. This only affects endpoint
                // intersection x-coordinates (we keep original y in output).
                let scale = if node_id.contains("---") && x_on_40960_grid {
                    81920.0
                } else {
                    40960.0
                };
                y = ceil_grid(y, scale) + eps + 2.0 * step;
            }
        }
    }

    Some(super::BoundaryNode {
        x,
        y,
        width,
        height,
    })
}
