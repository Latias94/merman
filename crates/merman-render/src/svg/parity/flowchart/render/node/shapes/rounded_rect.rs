//! Flowchart v2 rounded rectangle shape.

use std::fmt::Write as _;

use crate::svg::parity::flowchart::escape_attr;
use crate::svg::parity::{fmt, fmt_display};

use super::super::geom::{arc_points, path_from_points};
use super::super::roughjs::roughjs_paths_for_svg_path;

pub(in crate::svg::parity::flowchart::render::node) fn render_rounded_rect(
    out: &mut String,
    layout_node: &crate::model::LayoutNode,
    style: &str,
    fill_color: &str,
    stroke_color: &str,
    stroke_width: f32,
    stroke_dasharray: &str,
    hand_drawn_seed: u64,
    timing_enabled: bool,
    details: &mut crate::svg::parity::flowchart::types::FlowchartRenderDetails,
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

    let w = layout_node.width.max(1.0);
    let h = layout_node.height.max(1.0);
    let radius = 5.0;
    let taper = 5.0;

    let mut pts: Vec<(f64, f64)> = Vec::new();
    pts.push((-w / 2.0 + taper, -h / 2.0));
    pts.push((w / 2.0 - taper, -h / 2.0));
    pts.extend(arc_points(
        w / 2.0 - taper,
        -h / 2.0,
        w / 2.0,
        -h / 2.0 + taper,
        radius,
        radius,
        true,
    ));
    pts.push((w / 2.0, -h / 2.0 + taper));
    pts.push((w / 2.0, h / 2.0 - taper));
    pts.extend(arc_points(
        w / 2.0,
        h / 2.0 - taper,
        w / 2.0 - taper,
        h / 2.0,
        radius,
        radius,
        true,
    ));
    pts.push((w / 2.0 - taper, h / 2.0));
    pts.push((-w / 2.0 + taper, h / 2.0));
    pts.extend(arc_points(
        -w / 2.0 + taper,
        h / 2.0,
        -w / 2.0,
        h / 2.0 - taper,
        radius,
        radius,
        true,
    ));
    pts.push((-w / 2.0, h / 2.0 - taper));
    pts.push((-w / 2.0, -h / 2.0 + taper));
    pts.extend(arc_points(
        -w / 2.0,
        -h / 2.0 + taper,
        -w / 2.0 + taper,
        -h / 2.0,
        radius,
        radius,
        true,
    ));
    let path_data = path_from_points(&pts);

    if let Some((fill_d, stroke_d)) = rough_timed(timing_enabled, details, || {
        roughjs_paths_for_svg_path(
            &path_data,
            fill_color,
            stroke_color,
            stroke_width,
            stroke_dasharray,
            hand_drawn_seed,
        )
    }) {
        out.push_str(r#"<g class="basic label-container outer-path">"#);
        let _ = write!(
            out,
            r#"<path d="{}" stroke="none" stroke-width="0" fill="{}" style="{}"/>"#,
            escape_attr(&fill_d),
            escape_attr(fill_color),
            escape_attr(style)
        );
        let _ = write!(
            out,
            r#"<path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}" style="{}"/>"#,
            escape_attr(&stroke_d),
            escape_attr(stroke_color),
            fmt_display(stroke_width as f64),
            escape_attr(stroke_dasharray),
            escape_attr(style)
        );
        out.push_str("</g>");
    } else {
        let _ = write!(
            out,
            r#"<rect class="basic label-container" style="{}" x="{}" y="{}" width="{}" height="{}" rx="5" ry="5"/>"#,
            escape_attr(style),
            fmt(-w / 2.0),
            fmt(-h / 2.0),
            fmt(w),
            fmt(h)
        );
    }
}
