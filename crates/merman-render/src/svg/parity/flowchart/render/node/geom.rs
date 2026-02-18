//! Node geometry helpers (point generation and path assembly).
//!
//! These are ports of Mermaid rendering-element helpers and are used by multiple node shapes.

use std::fmt::Write as _;

pub(super) fn path_from_points(points: &[(f64, f64)]) -> String {
    let mut out = String::new();
    for (i, (x, y)) in points.iter().copied().enumerate() {
        let cmd = if i == 0 { 'M' } else { 'L' };
        let _ = write!(&mut out, "{cmd}{x},{y} ");
    }
    out.push('Z');
    out
}

pub(super) fn generate_circle_points(
    center_x: f64,
    center_y: f64,
    radius: f64,
    num_points: usize,
    start_angle_deg: f64,
    end_angle_deg: f64,
) -> Vec<(f64, f64)> {
    // Ported from Mermaid `generateCirclePoints(...)` in
    // `packages/mermaid/src/rendering-util/rendering-elements/shapes/util.ts`.
    //
    // Note: Mermaid pushes negated coordinates (`{ x: -x, y: -y }`).
    let start = start_angle_deg.to_radians();
    let end = end_angle_deg.to_radians();
    let angle_range = end - start;
    let step = angle_range / (num_points.saturating_sub(1).max(1) as f64);
    let mut pts: Vec<(f64, f64)> = Vec::with_capacity(num_points);
    for i in 0..num_points {
        let angle = start + (i as f64) * step;
        let x = center_x + radius * angle.cos();
        let y = center_y + radius * angle.sin();
        pts.push((-x, -y));
    }
    pts
}

pub(super) fn generate_full_sine_wave_points(
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    amplitude: f64,
    num_cycles: f64,
) -> Vec<(f64, f64)> {
    // Ported from Mermaid `generateFullSineWavePoints` (50 segments).
    let steps: usize = 50;
    let delta_x = x2 - x1;
    let delta_y = y2 - y1;
    let cycle_length = delta_x / num_cycles;
    let frequency = (2.0 * std::f64::consts::PI) / cycle_length;
    let mid_y = y1 + delta_y / 2.0;

    let mut points: Vec<(f64, f64)> = Vec::with_capacity(steps + 1);
    for i in 0..=steps {
        let t = (i as f64) / (steps as f64);
        let x = x1 + t * delta_x;
        let y = mid_y + amplitude * (frequency * (x - x1)).sin();
        points.push((x, y));
    }
    points
}

pub(super) fn arc_points(
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    rx: f64,
    ry: f64,
    clockwise: bool,
) -> Vec<(f64, f64)> {
    // Port of Mermaid `@11.12.2` `generateArcPoints(...)` in
    // `packages/mermaid/src/rendering-util/rendering-elements/shapes/roundedRect.ts`.
    let num_points: usize = 20;

    let mid_x = (x1 + x2) / 2.0;
    let mid_y = (y1 + y2) / 2.0;
    let angle = (y2 - y1).atan2(x2 - x1);

    let dx = (x2 - x1) / 2.0;
    let dy = (y2 - y1) / 2.0;
    let transformed_x = dx / rx;
    let transformed_y = dy / ry;
    let distance = (transformed_x * transformed_x + transformed_y * transformed_y).sqrt();
    if distance > 1.0 {
        return vec![(x1, y1), (x2, y2)];
    }

    let scaled_center_distance = (1.0 - distance * distance).sqrt();
    let sign = if clockwise { -1.0 } else { 1.0 };
    let center_x = mid_x + scaled_center_distance * ry * angle.sin() * sign;
    let center_y = mid_y - scaled_center_distance * rx * angle.cos() * sign;

    let start_angle = ((y1 - center_y) / ry).atan2((x1 - center_x) / rx);
    let end_angle = ((y2 - center_y) / ry).atan2((x2 - center_x) / rx);

    let mut angle_range = end_angle - start_angle;
    if clockwise && angle_range < 0.0 {
        angle_range += 2.0 * std::f64::consts::PI;
    }
    if !clockwise && angle_range > 0.0 {
        angle_range -= 2.0 * std::f64::consts::PI;
    }

    let mut points: Vec<(f64, f64)> = Vec::with_capacity(num_points);
    for i in 0..num_points {
        let t = i as f64 / (num_points - 1) as f64;
        let a = start_angle + t * angle_range;
        let x = center_x + rx * a.cos();
        let y = center_y + ry * a.sin();
        points.push((x, y));
    }
    points
}
