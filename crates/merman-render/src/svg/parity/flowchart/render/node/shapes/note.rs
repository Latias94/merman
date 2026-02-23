//! Flowchart v2 note shape.

use std::fmt::Write as _;

use crate::svg::parity::flowchart::escape_attr;
use crate::svg::parity::util;
use crate::svg::parity::{fmt, fmt_display};

use super::super::roughjs::roughjs_paths_for_rect;

pub(in crate::svg::parity::flowchart::render::node) fn render_note(
    out: &mut String,
    ctx: &crate::svg::parity::flowchart::types::FlowchartRenderCtx<'_>,
    layout_node: &crate::model::LayoutNode,
    style: &str,
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

    let note_fill = util::theme_color(ctx.config.as_value(), "noteBkgColor", "#fff5ad");
    let note_stroke = util::theme_color(ctx.config.as_value(), "noteBorderColor", "#aaaa33");

    if let Some((fill_d, stroke_d)) = rough_timed(timing_enabled, details, || {
        roughjs_paths_for_rect(
            x,
            y,
            w,
            h,
            &note_fill,
            &note_stroke,
            stroke_width,
            hand_drawn_seed,
        )
    }) {
        let _ = write!(out, r#"<g class="basic label-container">"#);
        let _ = write!(
            out,
            r#"<path d="{}" stroke="none" stroke-width="0" fill="{}" style="{}"/>"#,
            escape_attr(&fill_d),
            escape_attr(&note_fill),
            escape_attr(style)
        );
        let _ = write!(
            out,
            r#"<path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}" style="{}"/>"#,
            escape_attr(&stroke_d),
            escape_attr(&note_stroke),
            fmt_display(stroke_width as f64),
            escape_attr(stroke_dasharray),
            escape_attr(style)
        );
        out.push_str("</g>");
    } else {
        // Fallback: basic rect.
        let _ = write!(
            out,
            r#"<rect class="basic label-container" style="{}" x="{}" y="{}" width="{}" height="{}"/>"#,
            escape_attr(style),
            fmt(x),
            fmt(y),
            fmt(w),
            fmt(h)
        );
    }
}
