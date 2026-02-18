//! Flowchart v2 curved trapezoid (Display).

use std::fmt::Write as _;

use crate::svg::parity::flowchart::escape_attr;
use crate::svg::parity::util;

use super::super::geom::{generate_circle_points, path_from_points};
use super::super::helpers;
use super::super::roughjs::roughjs_paths_for_svg_path;

pub(in crate::svg::parity::flowchart::render::node) fn render_curved_trapezoid(
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
    let min_width = 80.0;
    let min_height = 20.0;
    let w = ((metrics.width + 2.0 * p) * 1.25).max(min_width);
    let h = (metrics.height + 2.0 * p).max(min_height);
    let radius = h / 2.0;

    let total_width = w;
    let total_height = h;
    let rw = total_width - radius;
    let tw = total_height / 4.0;

    let mut points: Vec<(f64, f64)> = Vec::new();
    points.push((rw, 0.0));
    points.push((tw, 0.0));
    points.push((0.0, total_height / 2.0));
    points.push((tw, total_height));
    points.push((rw, total_height));
    points.extend(generate_circle_points(
        -rw,
        -total_height / 2.0,
        radius,
        50,
        270.0,
        90.0,
    ));

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
        r##"<g class="basic label-container" transform="translate({}, {})"><path d="{}" stroke="none" stroke-width="0" fill="{}" style=""/><path d="{}" stroke="{}" stroke-width="1.3" fill="none" stroke-dasharray="0 0" style=""/></g>"##,
        util::fmt(-w / 2.0),
        util::fmt(-h / 2.0),
        escape_attr(&fill_d),
        escape_attr(fill_color),
        escape_attr(&stroke_d),
        escape_attr(stroke_color),
    );
}
