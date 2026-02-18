//! Flowchart v2 hexagon shape.

use std::fmt::Write as _;

use crate::svg::parity::flowchart::{OptionalStyleAttr, escape_attr};
use crate::svg::parity::fmt_display;

use super::super::geom::path_from_points;
use super::super::roughjs::roughjs_paths_for_svg_path;

pub(in crate::svg::parity::flowchart::render::node) fn render_hexagon(
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
    let half_width = w / 2.0;
    let half_height = h / 2.0;
    let fixed_length = half_height / 2.0;
    let deduced_width = half_width - fixed_length;

    let pts: Vec<(f64, f64)> = vec![
        (-deduced_width, -half_height),
        (0.0, -half_height),
        (deduced_width, -half_height),
        (half_width, 0.0),
        (deduced_width, half_height),
        (0.0, half_height),
        (-deduced_width, half_height),
        (-half_width, 0.0),
    ];
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
        out.push_str(r#"<g class="basic label-container">"#);
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
            r#"<polygon points="{},{} {},{} {},{} {},{} {},{} {},{} {},{} {},{}" class="label-container" transform="translate({}, {})"{} />"#,
            fmt_display(-deduced_width),
            fmt_display(-half_height),
            fmt_display(0.0),
            fmt_display(-half_height),
            fmt_display(deduced_width),
            fmt_display(-half_height),
            fmt_display(half_width),
            fmt_display(0.0),
            fmt_display(deduced_width),
            fmt_display(half_height),
            fmt_display(0.0),
            fmt_display(half_height),
            fmt_display(-deduced_width),
            fmt_display(half_height),
            fmt_display(-half_width),
            fmt_display(0.0),
            fmt_display(0.0),
            fmt_display(0.0),
            OptionalStyleAttr(style)
        );
    }
}
