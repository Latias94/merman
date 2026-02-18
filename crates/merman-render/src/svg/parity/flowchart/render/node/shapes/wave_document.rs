//! Flowchart v2 wave edged rectangle (Document).

use std::fmt::Write as _;

use crate::svg::parity::flowchart::escape_attr;
use crate::svg::parity::util;

use super::super::geom::{generate_full_sine_wave_points, path_from_points};
use super::super::helpers;
use super::super::roughjs::roughjs_paths_for_svg_path;

pub(in crate::svg::parity::flowchart::render::node) fn render_wave_document(
    out: &mut String,
    ctx: &crate::svg::parity::flowchart::types::FlowchartRenderCtx<'_>,
    layout_node: &crate::model::LayoutNode,
    label_text: &str,
    label_type: &str,
    node_classes: &[String],
    node_styles: &[String],
    fill_color: &str,
    stroke_color: &str,
    hand_drawn_seed: u64,
    timing_enabled: bool,
    details: &mut crate::svg::parity::flowchart::types::FlowchartRenderDetails,
    label_dx: &mut f64,
    label_dy: &mut f64,
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

    let metrics =
        helpers::compute_node_label_metrics(ctx, label_text, label_type, node_classes, node_styles);

    let p = ctx.node_padding;
    let w = (metrics.width + 2.0 * p).max(layout_node.width.max(0.0));
    let h = (metrics.height + 2.0 * p).max(layout_node.height.max(0.0));
    let wave_amplitude = h / 8.0;
    let final_h = h + wave_amplitude;

    // Mermaid keeps a minimum width (70px) for wave edged rectangles.
    let min_width = 70.0;
    let extra_w = ((min_width - w).max(0.0)) / 2.0;

    let mut points: Vec<(f64, f64)> = Vec::new();
    points.push((-w / 2.0 - extra_w, final_h / 2.0));
    points.extend(generate_full_sine_wave_points(
        -w / 2.0 - extra_w,
        final_h / 2.0,
        w / 2.0 + extra_w,
        final_h / 2.0,
        wave_amplitude,
        0.8,
    ));
    points.push((w / 2.0 + extra_w, -final_h / 2.0));
    points.push((-w / 2.0 - extra_w, -final_h / 2.0));

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
        r##"<g class="basic label-container" transform="translate(0,{})"><path d="{}" stroke="none" stroke-width="0" fill="{}" style=""/><path d="{}" stroke="{}" stroke-width="1.3" fill="none" stroke-dasharray="0 0" style=""/></g>"##,
        util::fmt(-wave_amplitude / 2.0),
        escape_attr(&fill_d),
        escape_attr(fill_color),
        escape_attr(&stroke_d),
        escape_attr(stroke_color),
    );

    // Mirror Mermaid `waveEdgedRectangle.ts` label placement.
    *label_dx = -w / 2.0 + p + metrics.width / 2.0;
    *label_dy = -h / 2.0 + p - wave_amplitude + metrics.height / 2.0;
}
