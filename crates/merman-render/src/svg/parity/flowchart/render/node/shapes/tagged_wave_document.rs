//! Flowchart v2 tagged wave edged rectangle (Tagged document).

use std::fmt::Write as _;

use crate::svg::parity::flowchart::escape_attr;
use crate::svg::parity::util;

use super::super::geom::{generate_full_sine_wave_points, path_from_points};
use super::super::helpers;
use super::super::roughjs::roughjs_paths_for_svg_path;

pub(in crate::svg::parity::flowchart::render::node) fn render_tagged_wave_document(
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
    let wave_amplitude = h / 4.0;
    let tag_width = 0.2 * w;
    let tag_height = 0.2 * h;
    let final_h = h + wave_amplitude;

    // Mermaid shifts label to the left padding origin and up by waveAmplitude/2.
    *label_dx = -w / 2.0 + p + metrics.width / 2.0;
    *label_dy = -h / 2.0 + p - wave_amplitude / 2.0 + metrics.height / 2.0;

    let ext = (w / 2.0) * 0.1;
    let mut points: Vec<(f64, f64)> = Vec::new();
    points.push((-w / 2.0 - ext, final_h / 2.0));
    points.extend(generate_full_sine_wave_points(
        -w / 2.0 - ext,
        final_h / 2.0,
        w / 2.0 + ext,
        final_h / 2.0,
        wave_amplitude,
        0.8,
    ));
    points.push((w / 2.0 + ext, -final_h / 2.0));
    points.push((-w / 2.0 - ext, -final_h / 2.0));

    let x = -w / 2.0 + ext;
    let y = -final_h / 2.0 - tag_height * 0.4;
    let mut tag_points: Vec<(f64, f64)> = Vec::new();
    tag_points.push((x + w - tag_width, (y + h) * 1.4));
    tag_points.push((x + w, y + h - tag_height));
    tag_points.push((x + w, (y + h) * 0.9));
    tag_points.extend(generate_full_sine_wave_points(
        x + w,
        (y + h) * 1.3,
        x + w - tag_width,
        (y + h) * 1.5,
        -h * 0.03,
        0.5,
    ));

    let wave_rect_path = path_from_points(&points);
    let (wave_fill_d, wave_stroke_d) = rough_timed(timing_enabled, details, || {
        roughjs_paths_for_svg_path(
            &wave_rect_path,
            fill_color,
            stroke_color,
            1.3,
            "0 0",
            hand_drawn_seed,
        )
    })
    .unwrap_or_else(|| ("M0,0".to_string(), "M0,0".to_string()));

    let tag_path = path_from_points(&tag_points);
    let (tag_fill_d, tag_stroke_d) = rough_timed(timing_enabled, details, || {
        roughjs_paths_for_svg_path(
            &tag_path,
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
        r##"<g class="basic label-container" transform="translate(0,{})"><g><path d="{}" stroke="none" stroke-width="0" fill="{}" style=""/><path d="{}" stroke="{}" stroke-width="1.3" fill="none" stroke-dasharray="0 0" style=""/></g><path d="{}" stroke="none" stroke-width="0" fill="{}" style=""/><path d="{}" stroke="{}" stroke-width="1.3" fill="none" stroke-dasharray="0 0" style=""/></g>"##,
        util::fmt(-wave_amplitude / 2.0),
        escape_attr(&wave_fill_d),
        escape_attr(fill_color),
        escape_attr(&wave_stroke_d),
        escape_attr(stroke_color),
        escape_attr(&tag_fill_d),
        escape_attr(fill_color),
        escape_attr(&tag_stroke_d),
        escape_attr(stroke_color),
    );
}
