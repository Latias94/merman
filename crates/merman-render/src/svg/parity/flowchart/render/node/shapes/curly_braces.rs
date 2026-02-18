//! Flowchart v2 curly brace / comment shapes.

use std::fmt::Write as _;

use crate::svg::parity::flowchart::escape_attr;
use crate::svg::parity::fmt;

use super::super::geom::path_from_points;
use super::super::roughjs::roughjs_stroke_path_for_svg_path;

pub(in crate::svg::parity::flowchart::render::node) fn render_curly_brace_comment(
    out: &mut String,
    shape: &str,
    layout_node: &crate::model::LayoutNode,
    stroke_color: &str,
    hand_drawn_seed: u64,
    timing_enabled: bool,
    details: &mut crate::svg::parity::flowchart::types::FlowchartRenderDetails,
    compact_label_translate: &mut bool,
    label_dx: &mut f64,
) {
    fn rough_timed<T>(
        timing_enabled: bool,
        details: &mut crate::svg::parity::flowchart::types::FlowchartRenderDetails,
        f: impl FnOnce() -> T,
    ) -> T {
        if timing_enabled {
            details.node_roughjs_calls += 1;
            let start = std::time::Instant::now();
            let out = f();
            details.node_roughjs += start.elapsed();
            out
        } else {
            f()
        }
    }

    fn circle_points(
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

    let out_w = layout_node.width.max(1.0);
    let out_h = layout_node.height.max(1.0);

    // Mermaid's `label.attr('transform', ...)` for curly brace shapes renders without a
    // space after the comma (e.g. `translate(-34.265625,-12)`).
    *compact_label_translate = true;

    // Radius depends on the *inner* height in Mermaid (`h = bbox.height + padding`).
    // Solve `radius = max(5, (out_h - 2*radius) * 0.1)` by a few fixed-point iterations.
    let mut radius: f64 = 5.0;
    for _ in 0..3 {
        let inner_h = (out_h - 2.0 * radius).max(0.0);
        let next = (inner_h * 0.1).max(5.0);
        if (next - radius).abs() < 1e-9 {
            break;
        }
        radius = next;
    }
    let h = (out_h - 2.0 * radius).max(0.0);

    let w = match shape {
        "comment" | "brace" | "brace-l" => (out_w - 2.0 * radius) / 1.1,
        "brace-r" | "braces" => out_w - 3.0 * radius,
        _ => out_w - 3.0 * radius,
    };

    let (group_tx, local_label_dx) = match shape {
        "comment" | "brace" | "brace-l" => (radius, -radius / 2.0),
        "brace-r" => (-radius, 0.0),
        "braces" => (radius - radius / 4.0, 0.0),
        _ => (0.0, 0.0),
    };
    *label_dx = local_label_dx;

    let mut stroke_d = |d: &str| {
        rough_timed(timing_enabled, details, || {
            roughjs_stroke_path_for_svg_path(d, stroke_color, 1.3, "0 0", hand_drawn_seed)
        })
        .unwrap_or_else(|| "M0,0".to_string())
    };

    if shape == "braces" {
        // Mermaid `curlyBraces.ts`: two visible brace paths + one invisible rect path.
        let left_points: Vec<(f64, f64)> = [
            circle_points(w / 2.0, -h / 2.0, radius, 30, -90.0, 0.0, true),
            vec![(-w / 2.0 - radius, radius)],
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
            vec![(-w / 2.0 - radius, -h / 2.0)],
            circle_points(w / 2.0, h / 2.0, radius, 20, 0.0, 90.0, true),
        ]
        .into_iter()
        .flatten()
        .collect();
        let right_points: Vec<(f64, f64)> = [
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

        let left_path = path_from_points(&left_points)
            .trim_end_matches('Z')
            .to_string();
        let right_path = path_from_points(&right_points)
            .trim_end_matches('Z')
            .to_string();
        let rect_path = path_from_points(&rect_points);

        let left_d = stroke_d(&left_path);
        let right_d = stroke_d(&right_path);
        let rect_d = stroke_d(&rect_path);

        let _ = write!(
            out,
            concat!(
                r##"<g class="text" transform="translate({}, 0)"><g>"##,
                r##"<path d="{}" stroke="{}" stroke-width="1.3" fill="none" stroke-dasharray="0 0" style=""/>"##,
                r##"</g><g>"##,
                r##"<path d="{}" stroke="{}" stroke-width="1.3" fill="none" stroke-dasharray="0 0" style=""/>"##,
                r##"</g><g stroke-opacity="0">"##,
                r##"<path d="{}" stroke="{}" stroke-width="1.3" fill="none" stroke-dasharray="0 0" style=""/>"##,
                r##"</g></g>"##
            ),
            fmt(group_tx),
            escape_attr(&left_d),
            escape_attr(stroke_color),
            escape_attr(&right_d),
            escape_attr(stroke_color),
            escape_attr(&rect_d),
            escape_attr(stroke_color),
        );
    } else {
        // Mermaid `curlyBraceLeft.ts` / `curlyBraceRight.ts`.
        let (negate, points, rect_points) = if shape == "brace-r" {
            let points: Vec<(f64, f64)> = [
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
            ]
            .into_iter()
            .flatten()
            .collect();
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
            (false, points, rect_points)
        } else {
            let points: Vec<(f64, f64)> = [
                circle_points(w / 2.0, -h / 2.0, radius, 30, -90.0, 0.0, true),
                vec![(-w / 2.0 - radius, radius)],
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
                vec![(-w / 2.0 - radius, -h / 2.0)],
                circle_points(w / 2.0, h / 2.0, radius, 20, 0.0, 90.0, true),
            ]
            .into_iter()
            .flatten()
            .collect();
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
            (true, points, rect_points)
        };
        let _ = negate;

        let brace_path = path_from_points(&points).trim_end_matches('Z').to_string();
        let rect_path = path_from_points(&rect_points);
        let brace_d = stroke_d(&brace_path);
        let rect_d = stroke_d(&rect_path);
        let _ = write!(
            out,
            concat!(
                r##"<g class="text" transform="translate({}, 0)"><g>"##,
                r##"<path d="{}" stroke="{}" stroke-width="1.3" fill="none" stroke-dasharray="0 0" style=""/>"##,
                r##"</g><g stroke-opacity="0">"##,
                r##"<path d="{}" stroke="{}" stroke-width="1.3" fill="none" stroke-dasharray="0 0" style=""/>"##,
                r##"</g></g>"##
            ),
            fmt(group_tx),
            escape_attr(&brace_d),
            escape_attr(stroke_color),
            escape_attr(&rect_d),
            escape_attr(stroke_color),
        );
    }
}
