//! Flowchart v2 paper tape (flag) shape.

use std::fmt::Write as _;

use crate::svg::parity::flowchart::escape_attr;
use crate::svg::parity::fmt_display;

use super::super::geom::{generate_full_sine_wave_points, path_from_points};
use super::super::roughjs::roughjs_paths_for_svg_path;

pub(in crate::svg::parity::flowchart::render::node) fn render_paper_tape(
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

    let min_width = 100.0;
    let min_height = 50.0;

    let base_width = layout_node.width.max(1.0);
    let base_height = layout_node.height.max(1.0);
    let aspect_ratio = base_width / base_height.max(1e-9);

    let mut w = base_width;
    let mut h = base_height;
    if w > h * aspect_ratio {
        h = w / aspect_ratio;
    } else {
        w = h * aspect_ratio;
    }
    w = w.max(min_width);
    h = h.max(min_height);

    let wave_amplitude = (h * 0.2).min(h / 4.0);
    let final_h = h + wave_amplitude * 2.0;

    let mut points: Vec<(f64, f64)> = Vec::new();
    points.push((-w / 2.0, final_h / 2.0));
    points.extend(generate_full_sine_wave_points(
        -w / 2.0,
        final_h / 2.0,
        w / 2.0,
        final_h / 2.0,
        wave_amplitude,
        1.0,
    ));
    points.push((w / 2.0, -final_h / 2.0));
    points.extend(generate_full_sine_wave_points(
        w / 2.0,
        -final_h / 2.0,
        -w / 2.0,
        -final_h / 2.0,
        wave_amplitude,
        -1.0,
    ));

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
    }
}
