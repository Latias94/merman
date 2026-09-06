//! Flowchart v2 tagged rectangle (Tagged process).

use crate::svg::parity::flowchart::escape_attr;
use crate::svg::parity::util;

use super::super::geom::path_from_points;
use super::super::helpers;
use super::super::roughjs::{
    roughjs_paths_for_hand_drawn_solid_svg_path, roughjs_paths_for_svg_path,
};

pub(in crate::svg::parity::flowchart::render::node) fn render_tag_rect(
    out: &mut impl crate::svg::parity::SvgOutput,
    ctx: &crate::svg::parity::flowchart::types::FlowchartRenderCtx<'_>,
    common: &super::super::FlowchartNodeRenderCommon<'_>,
    label: &super::super::FlowchartNodeLabelState<'_>,
    details: &mut crate::svg::parity::flowchart::types::FlowchartRenderDetails,
) {
    let metrics = helpers::compute_node_label_metrics(
        ctx,
        Some(common.layout_node),
        label.text,
        label.label_type,
        common.node_classes,
        common.node_styles,
    );

    let p = ctx.node_padding;
    let w = metrics.width + 2.0 * p;
    let h = metrics.height + 2.0 * p;
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
    let tag_path = path_from_points(&tag_points);

    if common.look_is_hand_drawn() {
        let rect_paths = helpers::hand_drawn_path_pair(common, details, &rect_path);
        let tag_paths = super::super::helpers::timed_node_roughjs(common.timing, details, || {
            roughjs_paths_for_hand_drawn_solid_svg_path(
                &tag_path,
                common.fill_color,
                common.stroke_color,
                common.stroke_width,
                common.stroke_dasharray,
                common.hand_drawn_seed,
            )
        })
        .filter(|(fill_d, stroke_d)| !fill_d.is_empty() && !stroke_d.is_empty());

        if let (Some((rect_fill_d, rect_stroke_d)), Some((tag_fill_d, tag_stroke_d))) =
            (rect_paths, tag_paths)
        {
            let _ = write!(
                out,
                r##"<g class="basic label-container outer-path" style="{}"><g><path d="{}" stroke="{}" stroke-width="4" fill="none" stroke-dasharray="0 0"/><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}"/></g><path d="{}" stroke="none" stroke-width="0" fill="{}"/><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}"/></g>"##,
                escape_attr(common.rough_group_style),
                escape_attr(&rect_fill_d),
                escape_attr(common.fill_color),
                escape_attr(&rect_stroke_d),
                escape_attr(common.stroke_color),
                util::fmt_display(common.stroke_width as f64),
                escape_attr(common.stroke_dasharray),
                escape_attr(&tag_fill_d),
                escape_attr(common.fill_color),
                escape_attr(&tag_stroke_d),
                escape_attr(common.stroke_color),
                util::fmt_display(common.stroke_width as f64),
                escape_attr(common.stroke_dasharray),
            );
            return;
        }

        // Hand-drawn RoughJS only accepts a strict color subset. Keep the full tag and body
        // geometry visible when either path cannot be admitted.
        let _ = write!(
            out,
            r##"<g class="basic label-container outer-path"><g><path d="{}" fill="{}" stroke="{}" stroke-width="{}" style="{}"/></g><path d="{}" fill="{}" stroke="{}" stroke-width="{}" style="{}"/></g>"##,
            escape_attr(&rect_path),
            escape_attr(common.fill_color),
            escape_attr(common.stroke_color),
            util::fmt_display(common.stroke_width as f64),
            escape_attr(common.style),
            escape_attr(&tag_path),
            escape_attr(common.fill_color),
            escape_attr(common.stroke_color),
            util::fmt_display(common.stroke_width as f64),
            escape_attr(common.style),
        );
        return;
    }

    let (rect_fill_d, rect_stroke_d) =
        super::super::helpers::timed_node_roughjs(common.timing, details, || {
            roughjs_paths_for_svg_path(
                &rect_path,
                common.fill_color,
                common.stroke_color,
                common.stroke_width,
                common.stroke_dasharray,
                common.hand_drawn_seed,
            )
        })
        .unwrap_or_else(|| ("M0,0".to_string(), "M0,0".to_string()));

    let (tag_fill_d, tag_stroke_d) =
        super::super::helpers::timed_node_roughjs(common.timing, details, || {
            roughjs_paths_for_svg_path(
                &tag_path,
                common.fill_color,
                common.stroke_color,
                common.stroke_width,
                common.stroke_dasharray,
                common.hand_drawn_seed,
            )
        })
        .unwrap_or_else(|| ("M0,0".to_string(), "M0,0".to_string()));

    let _ = write!(
        out,
        r##"<g class="basic label-container outer-path"><g><path d="{}" stroke="none" stroke-width="0" fill="{}" style="{}"/><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}" style="{}"/></g><path d="{}" stroke="none" stroke-width="0" fill="{}" style="{}"/><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}" style="{}"/></g>"##,
        escape_attr(&rect_fill_d),
        escape_attr(common.fill_color),
        escape_attr(common.style),
        escape_attr(&rect_stroke_d),
        escape_attr(common.stroke_color),
        util::fmt_display(common.stroke_width as f64),
        escape_attr(common.stroke_dasharray),
        escape_attr(common.style),
        escape_attr(&tag_fill_d),
        escape_attr(common.fill_color),
        escape_attr(common.style),
        escape_attr(&tag_stroke_d),
        escape_attr(common.stroke_color),
        util::fmt_display(common.stroke_width as f64),
        escape_attr(common.stroke_dasharray),
        escape_attr(common.style),
    );
}
