//! Flowchart v2 manual input/file shapes.

use std::fmt::Write as _;

use crate::svg::parity::{escape_xml_display, fmt, fmt_display};

use super::super::geom::path_from_points;
use super::super::roughjs::roughjs_paths_for_svg_path;

pub(in crate::svg::parity::flowchart::render::node) fn render_manual_file(
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

    let h = layout_node.height.max(1.0);
    let pts = vec![(0.0, -h), (h, -h), (h / 2.0, 0.0)];
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
        let _ = write!(
            out,
            r#"<g transform="translate({}, {})">"#,
            fmt_display(-h / 2.0),
            fmt_display(h / 2.0)
        );
        let _ = write!(
            out,
            r#"<path d="{}" stroke="none" stroke-width="0" fill="{}" style="{}"/>"#,
            escape_xml_display(&fill_d),
            escape_xml_display(fill_color),
            escape_xml_display(style)
        );
        let _ = write!(
            out,
            r#"<path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}" style="{}"/>"#,
            escape_xml_display(&stroke_d),
            escape_xml_display(stroke_color),
            fmt_display(stroke_width as f64),
            escape_xml_display(stroke_dasharray),
            escape_xml_display(style)
        );
        out.push_str("</g>");
    }
}

pub(in crate::svg::parity::flowchart::render::node) fn render_manual_input(
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
    let x = -w / 2.0;
    let y = -h / 2.0;
    let points = vec![(x, y), (x, y + h), (x + w, y + h), (x + w, y - h / 2.0)];
    let path_data = path_from_points(&points);
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
        let _ = write!(
            out,
            r#"<g class="basic label-container" transform="translate(0, {})">"#,
            fmt(h / 4.0)
        );
        let _ = write!(
            out,
            r#"<path d="{}" stroke="none" stroke-width="0" fill="{}" style="{}"/>"#,
            escape_xml_display(&fill_d),
            escape_xml_display(fill_color),
            escape_xml_display(style)
        );
        let _ = write!(
            out,
            r#"<path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}" style="{}"/>"#,
            escape_xml_display(&stroke_d),
            escape_xml_display(stroke_color),
            fmt_display(stroke_width as f64),
            escape_xml_display(stroke_dasharray),
            escape_xml_display(style)
        );
        out.push_str("</g>");
    }
}
