//! Sampling for the visible brace paths and the contact polygon.

use super::flowchart_brace_content_dimensions;

pub fn circle_points(
    center_x: f64,
    center_y: f64,
    radius: f64,
    num_points: usize,
    start_deg: f64,
    end_deg: f64,
    negate: bool,
) -> Vec<(f64, f64)> {
    let start = start_deg.to_radians();
    let end = end_deg.to_radians();
    let angle_range = end - start;
    let angle_step = if num_points > 1 {
        angle_range / (num_points as f64 - 1.0)
    } else {
        0.0
    };
    let mut out: Vec<(f64, f64)> = Vec::with_capacity(num_points);
    for i in 0..num_points {
        let a = start + (i as f64) * angle_step;
        let x = center_x + radius * a.cos();
        let y = center_y + radius * a.sin();
        if negate {
            out.push((-x, -y));
        } else {
            out.push((x, y));
        }
    }
    out
}

pub fn curly_brace_comment_intersection_points(
    shape: &str,
    label_w: f64,
    label_h: f64,
    padding: f64,
    look_is_neo: bool,
) -> Vec<(f64, f64)> {
    let (w, h) = flowchart_brace_content_dimensions(shape, label_w, label_h, padding, look_is_neo);
    let radius = (h * 0.1).max(5.0);
    // Keep the pinned rectPoints separate from the visible brace paths: their
    // central indentation and sampling differ, and intersections need no SVG.
    if shape == "braces" {
        let rect_points: Vec<(f64, f64)> = [
            vec![(w / 2.0, -h / 2.0 - radius), (-w / 2.0, -h / 2.0 - radius)],
            circle_points(w / 2.0, -h / 2.0, radius, 20, -90.0, 0.0, true),
            vec![(-w / 2.0 - radius, -radius)],
            circle_points(
                w / 2.0 + radius * 2.0,
                -radius,
                radius,
                20,
                -180.0,
                -270.0,
                true,
            ),
            circle_points(
                w / 2.0 + radius * 2.0,
                radius,
                radius,
                20,
                -90.0,
                -180.0,
                true,
            ),
            vec![(-w / 2.0 - radius, h / 2.0)],
            circle_points(w / 2.0, h / 2.0, radius, 20, 0.0, 90.0, true),
            vec![
                (-w / 2.0, h / 2.0 + radius),
                (w / 2.0 - radius - radius / 2.0, h / 2.0 + radius),
            ],
            circle_points(
                -w / 2.0 + radius + radius / 2.0,
                -h / 2.0,
                radius,
                20,
                -90.0,
                -180.0,
                true,
            ),
            vec![(w / 2.0 - radius / 2.0, radius)],
            circle_points(
                -w / 2.0 - radius / 2.0,
                -radius,
                radius,
                20,
                0.0,
                90.0,
                true,
            ),
            circle_points(
                -w / 2.0 - radius / 2.0,
                radius,
                radius,
                20,
                -90.0,
                0.0,
                true,
            ),
            vec![(w / 2.0 - radius / 2.0, -radius)],
            circle_points(
                -w / 2.0 + radius + radius / 2.0,
                h / 2.0,
                radius,
                30,
                -180.0,
                -270.0,
                true,
            ),
        ]
        .into_iter()
        .flatten()
        .collect();
        rect_points
    } else if shape == "brace-r" {
        let rect_points: Vec<(f64, f64)> = [
            vec![(-w / 2.0, -h / 2.0 - radius), (w / 2.0, -h / 2.0 - radius)],
            circle_points(w / 2.0, -h / 2.0, radius, 20, -90.0, 0.0, false),
            vec![(w / 2.0 + radius, -radius)],
            circle_points(
                w / 2.0 + radius * 2.0,
                -radius,
                radius,
                20,
                -180.0,
                -270.0,
                false,
            ),
            circle_points(
                w / 2.0 + radius * 2.0,
                radius,
                radius,
                20,
                -90.0,
                -180.0,
                false,
            ),
            vec![(w / 2.0 + radius, h / 2.0)],
            circle_points(w / 2.0, h / 2.0, radius, 20, 0.0, 90.0, false),
            vec![(w / 2.0, h / 2.0 + radius), (-w / 2.0, h / 2.0 + radius)],
        ]
        .into_iter()
        .flatten()
        .collect();
        rect_points
    } else {
        let rect_points: Vec<(f64, f64)> = [
            vec![(w / 2.0, -h / 2.0 - radius), (-w / 2.0, -h / 2.0 - radius)],
            circle_points(w / 2.0, -h / 2.0, radius, 20, -90.0, 0.0, true),
            vec![(-w / 2.0 - radius, -radius)],
            circle_points(w / 2.0 + w * 0.1, -radius, radius, 20, -180.0, -270.0, true),
            circle_points(w / 2.0 + w * 0.1, radius, radius, 20, -90.0, -180.0, true),
            vec![(-w / 2.0 - radius, h / 2.0)],
            circle_points(w / 2.0, h / 2.0, radius, 20, 0.0, 90.0, true),
            vec![(-w / 2.0, h / 2.0 + radius), (w / 2.0, h / 2.0 + radius)],
        ]
        .into_iter()
        .flatten()
        .collect();
        rect_points
    }
}
