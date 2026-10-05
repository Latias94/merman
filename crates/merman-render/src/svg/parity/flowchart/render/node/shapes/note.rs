//! Flowchart v2 note shape.

use crate::svg::parity::flowchart::escape_attr;
use crate::svg::parity::util;
use crate::svg::parity::{fmt, fmt_display};

use super::super::helpers;
use super::super::roughjs::{RoughRectSpec, roughjs_paths_for_rect};

pub(in crate::svg::parity::flowchart::render::node) fn render_note(
    out: &mut impl crate::svg::parity::SvgOutput,
    ctx: &crate::svg::parity::flowchart::types::FlowchartRenderCtx<'_>,
    common: &super::super::FlowchartNodeRenderCommon<'_>,
    details: &mut crate::svg::parity::flowchart::types::FlowchartRenderDetails,
) {
    let w = common.layout_node.width.max(1.0);
    let h = common.layout_node.height.max(1.0);
    let x = -w / 2.0;
    let y = -h / 2.0;

    let note_fill = util::theme_token(ctx.config.as_value(), "noteBkgColor", "#fff5ad");
    let note_stroke = util::theme_token(ctx.config.as_value(), "noteBorderColor", "#aaaa33");

    if let Some((fill_d, stroke_d)) = helpers::hand_drawn_path_pair_with_stroke(
        common.look_is_hand_drawn(),
        common.timing,
        details,
        &format!("M{} {} H{} V{} H{} Z", x, y, x + w, y + h, x),
        common.stroke_width,
        common.stroke_dasharray,
        common.work_meter,
        common.hand_drawn_seed,
    ) {
        let _ = write!(
            out,
            r#"<g class="basic label-container outer-path" style="{}">"#,
            escape_attr(common.rough_group_style)
        );
        let _ = write!(
            out,
            r#"<path d="{}" stroke="{}" stroke-width="4" fill="none" stroke-dasharray="0 0"/><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}"/></g>"#,
            escape_attr(&fill_d),
            escape_attr(&note_fill),
            escape_attr(&stroke_d),
            escape_attr(&note_stroke),
            fmt_display(common.stroke_width as f64),
            escape_attr(common.stroke_dasharray),
        );
    } else if common.look_is_hand_drawn() {
        let _ = write!(
            out,
            r#"<rect class="basic label-container outer-path" style="{}" x="{}" y="{}" width="{}" height="{}" fill="{}" stroke="{}" stroke-width="{}"/>"#,
            escape_attr(common.style),
            fmt(x),
            fmt(y),
            fmt(w),
            fmt(h),
            escape_attr(&note_fill),
            escape_attr(&note_stroke),
            fmt_display(common.stroke_width as f64),
        );
    } else if let Some((fill_d, stroke_d)) =
        super::super::helpers::timed_node_roughjs(common.timing, details, || {
            roughjs_paths_for_rect(RoughRectSpec {
                x,
                y,
                w,
                h,
                stroke_width: common.stroke_width,
                randomness: common.hand_drawn_seed,
            })
        })
    {
        let _ = write!(out, r#"<g class="basic label-container outer-path">"#);
        let _ = write!(
            out,
            r#"<path d="{}" stroke="none" stroke-width="0" fill="{}" style="{}"/>"#,
            escape_attr(&fill_d),
            escape_attr(&note_fill),
            escape_attr(common.style)
        );
        let _ = write!(
            out,
            r#"<path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}" style="{}"/>"#,
            escape_attr(&stroke_d),
            escape_attr(&note_stroke),
            fmt_display(common.stroke_width as f64),
            escape_attr(common.stroke_dasharray),
            escape_attr(common.style)
        );
        out.push_str("</g>");
    } else {
        // Fallback: basic rect.
        let _ = write!(
            out,
            r#"<rect class="basic label-container outer-path" style="{}" x="{}" y="{}" width="{}" height="{}"/>"#,
            escape_attr(common.style),
            fmt(x),
            fmt(y),
            fmt(w),
            fmt(h)
        );
    }
}
