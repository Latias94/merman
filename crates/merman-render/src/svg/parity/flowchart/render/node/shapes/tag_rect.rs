//! Flowchart v2 tagged rectangle (Tagged process).

use std::fmt::Write as _;

use crate::svg::parity::flowchart::escape_attr;

use super::super::geom::path_from_points;
use super::super::helpers;
use super::super::roughjs::roughjs_paths_for_svg_path;

pub(in crate::svg::parity::flowchart::render::node) fn render_tag_rect(
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
    let w = (metrics.width + 2.0 * p).max(layout_node.width.max(0.0));
    let h = (metrics.height + 2.0 * p).max(layout_node.height.max(0.0));
    let x = -w / 2.0;
    let y = -h / 2.0;
    let tag_w = 0.2 * h;
    let tag_h = 0.2 * h;

    let rect_points = vec![
        (x - tag_w / 2.0, y),
        (x + w + tag_w / 2.0, y),
        (x + w + tag_w / 2.0, y + h),
        (x - tag_w / 2.0, y + h),
    ];
    let tag_points = vec![
        (x + w - tag_w / 2.0, y + h),
        (x + w + tag_w / 2.0, y + h),
        (x + w + tag_w / 2.0, y + h - tag_h),
    ];

    let rect_path = path_from_points(&rect_points);
    let (rect_fill_d, rect_stroke_d) = rough_timed(timing_enabled, details, || {
        roughjs_paths_for_svg_path(
            &rect_path,
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
        r##"<g class="basic label-container"><g><path d="{}" stroke="none" stroke-width="0" fill="{}" style=""/><path d="{}" stroke="{}" stroke-width="1.3" fill="none" stroke-dasharray="0 0" style=""/></g><path d="{}" stroke="none" stroke-width="0" fill="{}" style=""/><path d="{}" stroke="{}" stroke-width="1.3" fill="none" stroke-dasharray="0 0" style=""/></g>"##,
        escape_attr(&rect_fill_d),
        escape_attr(fill_color),
        escape_attr(&rect_stroke_d),
        escape_attr(stroke_color),
        escape_attr(&tag_fill_d),
        escape_attr(fill_color),
        escape_attr(&tag_stroke_d),
        escape_attr(stroke_color),
    );
}
