//! Flowchart v2 bow tie rectangle (Stored data).

use std::fmt::Write as _;

use crate::svg::parity::flowchart::escape_attr;
use crate::svg::parity::util;

use super::super::geom::{arc_points, path_from_points};
use super::super::helpers;
use super::super::roughjs::roughjs_paths_for_svg_path;

pub(in crate::svg::parity::flowchart::render::node) fn render_bow_tie_rect(
    out: &mut String,
    ctx: &crate::svg::parity::flowchart::types::FlowchartRenderCtx<'_>,
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
    let w = metrics.width + p + 20.0;
    let h = metrics.height + p;
    let ry = h / 2.0;
    let rx = ry / (2.5 + h / 50.0);

    let mut points: Vec<(f64, f64)> = Vec::new();
    points.push((w / 2.0, -h / 2.0));
    points.push((-w / 2.0, -h / 2.0));
    points.extend(arc_points(
        -w / 2.0,
        -h / 2.0,
        -w / 2.0,
        h / 2.0,
        rx,
        ry,
        false,
    ));
    points.push((w / 2.0, h / 2.0));
    points.extend(arc_points(
        w / 2.0,
        h / 2.0,
        w / 2.0,
        -h / 2.0,
        rx,
        ry,
        true,
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
        r##"<g class="basic label-container" transform="translate({}, 0)"><path d="{}" stroke="none" stroke-width="0" fill="{}" style=""/><path d="{}" stroke="{}" stroke-width="1.3" fill="none" stroke-dasharray="0 0" style=""/></g>"##,
        util::fmt(rx / 2.0),
        escape_attr(&fill_d),
        escape_attr(fill_color),
        escape_attr(&stroke_d),
        escape_attr(stroke_color),
    );
}
