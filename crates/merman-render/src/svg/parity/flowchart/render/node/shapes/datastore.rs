//! Flowchart v2 datastore shape.

use crate::svg::parity::flowchart::escape_attr;
use crate::svg::parity::fmt;

use super::super::helpers;
use super::super::roughjs::{roughjs_hand_drawn_line_path, roughjs_paths_for_hand_drawn_rect};

pub(in crate::svg::parity::flowchart::render::node) fn render_datastore(
    out: &mut impl crate::svg::parity::SvgOutput,
    common: &super::super::FlowchartNodeRenderCommon<'_>,
    details: &mut crate::svg::parity::flowchart::types::FlowchartRenderDetails,
) {
    let w = common.layout_node.width.max(1.0);
    let h = common.layout_node.height.max(1.0);

    if common.look_is_hand_drawn() {
        let rough_paths = helpers::timed_node_roughjs(common.timing, details, || {
            let (fill_d, _) = roughjs_paths_for_hand_drawn_rect(
                -w / 2.0,
                -h / 2.0,
                w,
                h,
                common.fill_color,
                common.stroke_color,
                common.stroke_width,
                common.stroke_dasharray,
                common.hand_drawn_seed,
            )?;
            let top_d = roughjs_hand_drawn_line_path(
                -w / 2.0,
                -h / 2.0,
                w / 2.0,
                -h / 2.0,
                common.stroke_color,
                common.stroke_width,
                common.stroke_dasharray,
                common.hand_drawn_seed,
            )?;
            let bottom_d = roughjs_hand_drawn_line_path(
                -w / 2.0,
                h / 2.0,
                w / 2.0,
                h / 2.0,
                common.stroke_color,
                common.stroke_width,
                common.stroke_dasharray,
                common.hand_drawn_seed,
            )?;
            Some((fill_d, top_d, bottom_d))
        });

        if let Some((fill_d, top_d, bottom_d)) = rough_paths {
            let _ = write!(
                out,
                r#"<g class="basic label-container" style="{}"><path d="{}" stroke="{}" stroke-width="4" fill="none" stroke-dasharray="0 0"/><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}"/><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}"/></g>"#,
                escape_attr(common.rough_group_style),
                escape_attr(&fill_d),
                escape_attr(common.fill_color),
                escape_attr(&top_d),
                escape_attr(common.stroke_color),
                common.stroke_width,
                escape_attr(common.stroke_dasharray),
                escape_attr(&bottom_d),
                escape_attr(common.stroke_color),
                common.stroke_width,
                escape_attr(common.stroke_dasharray),
            );
            return;
        }
    }

    let _ = write!(
        out,
        r#"<rect class="basic label-container" style="{}" rx="0" ry="0" x="{}" y="{}" width="{}" height="{}" stroke-dasharray="{} {}"/>"#,
        escape_attr(common.style),
        fmt(-w / 2.0),
        fmt(-h / 2.0),
        fmt(w),
        fmt(h),
        fmt(w),
        fmt(h)
    );
}
