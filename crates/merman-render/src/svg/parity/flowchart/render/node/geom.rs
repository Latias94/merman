//! Node geometry helpers (point generation and path assembly).
//!
//! These are ports of Mermaid rendering-element helpers and are used by multiple node shapes.

pub(super) use crate::svg::parity::roughjs_common::closed_path_d_from_points as path_from_points;

/// Builds a bounded polygonal approximation of a rounded rectangle.
///
/// RoughJS accepts arbitrary SVG paths, but its path flattener recursively expands arc segments.
/// Flowchart object shapes are user-controlled, so keeping this hand-drawn input as a finite
/// polygon avoids an unbounded recursion path while preserving the rounded silhouette.
pub(super) fn rounded_rect_points(
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    radius: f64,
) -> Vec<(f64, f64)> {
    const ARC_SEGMENTS: usize = 8;

    let radius = radius.min(width / 2.0).min(height / 2.0).max(0.0);
    if radius == 0.0 {
        return vec![
            (x, y),
            (x + width, y),
            (x + width, y + height),
            (x, y + height),
        ];
    }

    let mut points = Vec::with_capacity(4 * (ARC_SEGMENTS + 1));
    let append_arc = |points: &mut Vec<(f64, f64)>, cx: f64, cy: f64, start: f64, end: f64| {
        for step in 0..=ARC_SEGMENTS {
            if !points.is_empty() && step == 0 {
                continue;
            }
            let t = step as f64 / ARC_SEGMENTS as f64;
            let angle = start + (end - start) * t;
            points.push((cx + radius * angle.cos(), cy + radius * angle.sin()));
        }
    };

    // Start at the top-left tangent and walk clockwise in SVG's y-down coordinate system.
    points.push((x + radius, y));
    points.push((x + width - radius, y));
    append_arc(
        &mut points,
        x + width - radius,
        y + radius,
        -std::f64::consts::FRAC_PI_2,
        0.0,
    );
    append_arc(
        &mut points,
        x + width - radius,
        y + height - radius,
        0.0,
        std::f64::consts::FRAC_PI_2,
    );
    append_arc(
        &mut points,
        x + radius,
        y + height - radius,
        std::f64::consts::FRAC_PI_2,
        std::f64::consts::PI,
    );
    append_arc(
        &mut points,
        x + radius,
        y + radius,
        std::f64::consts::PI,
        std::f64::consts::PI * 1.5,
    );
    points
}

pub(in crate::svg::parity::flowchart) use merman_render_flow::node_body::document_shapes::generate_full_sine_wave_points;
