//! Flowchart v2 node shapes that do not emit a label group.

use std::fmt::Write as _;

use crate::svg::parity::flowchart::escape_attr;
use crate::svg::parity::util;

use super::super::geom::path_from_points;
use super::super::roughjs::{
    roughjs_circle_path_d, roughjs_paths_for_rect, roughjs_paths_for_svg_path,
};

pub(in crate::svg::parity::flowchart::render::node) fn try_render_flowchart_v2_no_label(
    out: &mut String,
    ctx: &crate::svg::parity::flowchart::types::FlowchartRenderCtx<'_>,
    shape: &str,
    layout_node: &crate::model::LayoutNode,
    fill_color: &str,
    stroke_color: &str,
    hand_drawn_seed: u64,
    timing_enabled: bool,
    details: &mut crate::svg::parity::flowchart::types::FlowchartRenderDetails,
) -> bool {
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

    match shape {
        // Flowchart v2 "rendering-elements" aliases for state diagram start/end nodes.
        // Mermaid ignores `node.label` for these shapes and does not emit a label group.
        "sm-circ" | "small-circle" | "start" => {
            out.push_str(r#"<circle class="state-start" r="7" width="14" height="14"/>"#);
            true
        }
        "fr-circ" | "framed-circle" | "stop" => {
            let line_color = util::theme_color(ctx.config.as_value(), "lineColor", "#333333");
            let inner_fill =
                util::config_string(ctx.config.as_value(), &["themeVariables", "stateBorder"])
                    .unwrap_or_else(|| ctx.node_border_color.clone());

            let outer_d = rough_timed(timing_enabled, details, || {
                roughjs_circle_path_d(14.0, hand_drawn_seed)
            })
            .unwrap_or_else(|| "M0,0".to_string());
            let inner_d = rough_timed(timing_enabled, details, || {
                roughjs_circle_path_d(5.0, hand_drawn_seed)
            })
            .unwrap_or_else(|| "M0,0".to_string());

            let _ = write!(
                out,
                r##"<g><path d="{}" stroke="none" stroke-width="0" fill="{}" style=""/><path d="{}" stroke="{}" stroke-width="2" fill="none" stroke-dasharray="0 0" style=""/><g><path d="{}" stroke="none" stroke-width="0" fill="{}" style=""/><path d="{}" stroke="{}" stroke-width="2" fill="none" stroke-dasharray="0 0" style=""/></g></g>"##,
                outer_d,
                escape_attr(ctx.node_fill_color.as_str()),
                outer_d,
                escape_attr(&line_color),
                inner_d,
                escape_attr(&inner_fill),
                inner_d,
                escape_attr(&inner_fill),
            );
            true
        }
        // Flowchart v2 fork/join (no label; uses `lineColor` fill/stroke).
        "fork" | "join" => {
            // Mermaid inflates Dagre dimensions after `updateNodeBounds(...)` but does not
            // re-render the bar at the inflated size. Render the canonical shape dimensions.
            let (w, h) = if layout_node.width >= layout_node.height {
                (70.0, 10.0)
            } else {
                (10.0, 70.0)
            };
            let line_color = util::theme_color(ctx.config.as_value(), "lineColor", "#333333");
            let (fill_d, stroke_d) = rough_timed(timing_enabled, details, || {
                roughjs_paths_for_rect(
                    -w / 2.0,
                    -h / 2.0,
                    w,
                    h,
                    &line_color,
                    &line_color,
                    1.3,
                    hand_drawn_seed,
                )
            })
            .unwrap_or_else(|| ("M0,0".to_string(), "M0,0".to_string()));
            let _ = write!(
                out,
                r##"<g><path d="{}" stroke="none" stroke-width="0" fill="{}" style=""/><path d="{}" stroke="{}" stroke-width="1.3" fill="none" stroke-dasharray="0 0" style=""/></g>"##,
                fill_d,
                escape_attr(&line_color),
                stroke_d,
                escape_attr(&line_color),
            );
            true
        }
        // Flowchart v2 lightning bolt (Communication link). Mermaid clears `node.label` and does
        // not emit a label group.
        "bolt" => {
            // Mermaid uses `width = max(35, node.width)` and `height = max(35, node.height)`,
            // then draws a 2*height tall bolt and translates it by `(-width/2, -height)`.
            let width = layout_node.width.max(35.0);
            let height = (layout_node.height / 2.0).max(35.0);
            let gap = 7.0;

            let points: Vec<(f64, f64)> = vec![
                (width, 0.0),
                (0.0, height + gap / 2.0),
                (width - 2.0 * gap, height + gap / 2.0),
                (0.0, 2.0 * height),
                (width, height - gap / 2.0),
                (2.0 * gap, height - gap / 2.0),
            ];
            let path_data = path_from_points(&points);
            let (fill_d, stroke_d) = rough_timed(timing_enabled, details, || {
                roughjs_paths_for_svg_path(
                    &path_data,
                    fill_color,
                    stroke_color,
                    1.3,
                    "0 0",
                    hand_drawn_seed,
                )
            })
            .unwrap_or_else(|| ("M0,0".to_string(), "M0,0".to_string()));
            let _ = write!(
                out,
                r#"<g transform="translate({},{})"><path d="{}" stroke="none" stroke-width="0" fill="{}" style=""/><path d="{}" stroke="{}" stroke-width="1.3" fill="none" stroke-dasharray="0 0" style=""/></g>"#,
                util::fmt(-width / 2.0),
                util::fmt(-height),
                escape_attr(&fill_d),
                escape_attr(fill_color),
                escape_attr(&stroke_d),
                escape_attr(stroke_color),
            );
            true
        }
        // Flowchart v2 filled circle (junction). Mermaid clears `node.label` and does not emit a
        // label group. Note that even in non-handDrawn mode Mermaid still uses RoughJS circle
        // paths (roughness=0), which have a slightly asymmetric bbox in Chromium.
        "f-circ" => {
            let border =
                util::config_string(ctx.config.as_value(), &["themeVariables", "nodeBorder"])
                    .unwrap_or_else(|| ctx.node_border_color.clone());

            let d = rough_timed(timing_enabled, details, || {
                roughjs_circle_path_d(14.0, hand_drawn_seed)
            })
            .unwrap_or_else(|| "M0,0".into());
            let _ = write!(
                out,
                r##"<g><path d="{}" stroke="none" stroke-width="0" fill="{}" style="fill: {} !important;"/><path d="{}" stroke="{}" stroke-width="1.3" fill="none" stroke-dasharray="0 0" style="fill: {} !important;"/></g>"##,
                escape_attr(&d),
                escape_attr(fill_color),
                escape_attr(&border),
                escape_attr(&d),
                escape_attr(stroke_color),
                escape_attr(&border),
            );
            true
        }
        // Flowchart v2 crossed circle (summary). Mermaid clears `node.label` and does not emit a
        // label group.
        "cross-circ" => {
            // Mermaid uses `radius = max(30, node.width)` before `updateNodeBounds(...)`. In
            // practice `node.width` is usually unset here, so radius=30.
            let radius = 30.0;

            let circle_d = rough_timed(timing_enabled, details, || {
                roughjs_circle_path_d(radius * 2.0, hand_drawn_seed)
            })
            .unwrap_or_else(|| "M0,0".into());

            // Port of Mermaid `createLine(r)` in `crossedCircle.ts`.
            let x_axis_45 = (std::f64::consts::PI / 4.0).cos();
            let y_axis_45 = (std::f64::consts::PI / 4.0).sin();
            let point_q1 = (radius * x_axis_45, radius * y_axis_45);
            let point_q2 = (-radius * x_axis_45, radius * y_axis_45);
            let point_q3 = (-radius * x_axis_45, -radius * y_axis_45);
            let point_q4 = (radius * x_axis_45, -radius * y_axis_45);
            let line_path = format!(
                "M {},{} L {},{} M {},{} L {},{}",
                point_q2.0,
                point_q2.1,
                point_q4.0,
                point_q4.1,
                point_q1.0,
                point_q1.1,
                point_q3.0,
                point_q3.1
            );
            let (line_fill_d, line_stroke_d) = rough_timed(timing_enabled, details, || {
                roughjs_paths_for_svg_path(
                    &line_path,
                    fill_color,
                    stroke_color,
                    1.3,
                    "0 0",
                    hand_drawn_seed,
                )
            })
            .unwrap_or_else(|| ("".to_string(), "M0,0".to_string()));

            let _ = write!(
                out,
                r##"<g><path d="{}" stroke="none" stroke-width="0" fill="{}" style=""/><path d="{}" stroke="{}" stroke-width="1.3" fill="none" stroke-dasharray="0 0" style=""/><g><path d="{}" stroke="none" stroke-width="0" fill="{}" style=""/><path d="{}" stroke="{}" stroke-width="1.3" fill="none" stroke-dasharray="0 0" style=""/></g></g>"##,
                escape_attr(&circle_d),
                escape_attr(fill_color),
                escape_attr(&circle_d),
                escape_attr(stroke_color),
                escape_attr(&line_fill_d),
                escape_attr(fill_color),
                escape_attr(&line_stroke_d),
                escape_attr(stroke_color),
            );
            true
        }
        _ => false,
    }
}
