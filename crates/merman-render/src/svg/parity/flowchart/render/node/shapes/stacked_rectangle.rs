//! Flowchart v2 stacked rectangle shape.

use std::fmt::Write as _;

use crate::svg::parity::flowchart::escape_attr;
use crate::svg::parity::fmt_display;

use super::super::geom::path_from_points;
use super::super::roughjs::{roughjs_paths_for_svg_path, roughjs_stroke_path_for_svg_path};

pub(in crate::svg::parity::flowchart::render::node) fn render_stacked_rectangle(
    out: &mut String,
    common: &super::super::FlowchartNodeRenderCommon<'_>,
    details: &mut crate::svg::parity::flowchart::types::FlowchartRenderDetails,
) {
    let w = common.layout_node.width.max(1.0);
    let h = common.layout_node.height.max(1.0);
    let rect_offset = 5.0;
    let x = -w / 2.0;
    let y = -h / 2.0;

    let outer_points = vec![
        (x - rect_offset, y + rect_offset),
        (x - rect_offset, y + h + rect_offset),
        (x + w - rect_offset, y + h + rect_offset),
        (x + w - rect_offset, y + h),
        (x + w, y + h),
        (x + w, y + h - rect_offset),
        (x + w + rect_offset, y + h - rect_offset),
        (x + w + rect_offset, y - rect_offset),
        (x + rect_offset, y - rect_offset),
        (x + rect_offset, y),
        (x, y),
        (x, y + rect_offset),
    ];

    let inner_points = vec![
        (x, y + rect_offset),
        (x + w - rect_offset, y + rect_offset),
        (x + w - rect_offset, y + h),
        (x + w, y + h),
        (x + w, y),
        (x, y),
    ];

    let outer_path = path_from_points(&outer_points);
    let inner_path = path_from_points(&inner_points);

    out.push_str(r#"<g class="basic label-container">"#);
    out.push_str("<g>");
    if let Some((fill_d, stroke_d)) =
        super::super::helpers::timed_node_roughjs(common.timing_enabled, details, || {
            roughjs_paths_for_svg_path(
                &outer_path,
                common.fill_color,
                common.stroke_color,
                common.stroke_width,
                common.stroke_dasharray,
                common.hand_drawn_seed,
            )
        })
    {
        let _ = write!(
            out,
            r#"<path d="{}" stroke="none" stroke-width="0" fill="{}" style="{}"/>"#,
            escape_attr(&fill_d),
            escape_attr(common.fill_color),
            escape_attr(common.style)
        );
        let _ = write!(
            out,
            r#"<path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}" style="{}"/>"#,
            escape_attr(&stroke_d),
            escape_attr(common.stroke_color),
            fmt_display(common.stroke_width as f64),
            escape_attr(common.stroke_dasharray),
            escape_attr(common.style)
        );
    }
    out.push_str("</g>");
    if let Some(stroke_d) =
        super::super::helpers::timed_node_roughjs(common.timing_enabled, details, || {
            roughjs_stroke_path_for_svg_path(
                &inner_path,
                common.stroke_color,
                common.stroke_width,
                common.stroke_dasharray,
                common.hand_drawn_seed,
            )
        })
    {
        let _ = write!(
            out,
            r#"<path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}" style="{}"/>"#,
            escape_attr(&stroke_d),
            escape_attr(common.stroke_color),
            fmt_display(common.stroke_width as f64),
            escape_attr(common.stroke_dasharray),
            escape_attr(common.style)
        );
    }
    out.push_str("</g>");
}
