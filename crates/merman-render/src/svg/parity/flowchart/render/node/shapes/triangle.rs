//! Flowchart v2 triangle (Extract).

use crate::flowchart::flowchart_effective_text_style_for_node_classes;
use crate::svg::parity::flowchart::escape_attr;
use crate::svg::parity::util;

use super::super::geom::path_from_points;
use super::super::helpers;
use super::super::roughjs::{roughjs_paths_for_hand_drawn_svg_path, roughjs_paths_for_svg_path};

pub(in crate::svg::parity::flowchart::render::node) fn render_triangle_extract(
    out: &mut impl crate::svg::parity::SvgOutput,
    ctx: &crate::svg::parity::flowchart::types::FlowchartRenderCtx<'_>,
    common: &super::super::FlowchartNodeRenderCommon<'_>,
    label: &mut super::super::FlowchartNodeLabelState<'_>,
    details: &mut crate::svg::parity::flowchart::types::FlowchartRenderDetails,
) {
    let metrics = helpers::compute_node_label_metrics(
        ctx,
        None,
        label.text,
        label.label_type,
        common.node_classes,
        common.node_styles,
    );

    let p = ctx.node_padding;
    let w = metrics.width + p;
    let h = w + metrics.height;
    let tw = w + metrics.height;
    let pts = vec![(0.0, 0.0), (tw, 0.0), (tw / 2.0, -h)];
    let path_data = path_from_points(&pts);
    if common.look_is_hand_drawn() {
        let rough_paths = super::super::helpers::timed_node_roughjs(common.timing, details, || {
            roughjs_paths_for_hand_drawn_svg_path(
                &path_data,
                common.fill_color,
                common.stroke_color,
                common.stroke_width,
                common.stroke_dasharray,
                common.hand_drawn_seed,
            )
        });
        if let Some((fill_d, stroke_d)) =
            rough_paths.filter(|(fill_d, stroke_d)| !fill_d.is_empty() && !stroke_d.is_empty())
        {
            let _ = write!(
                out,
                r#"<g transform="translate({},{})" class="outer-path" style="{}"><path d="{}" stroke="{}" stroke-width="4" fill="none" stroke-dasharray="0 0"/><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}"/></g>"#,
                util::fmt(-h / 2.0),
                util::fmt(h / 2.0),
                escape_attr(common.rough_group_style),
                escape_attr(&fill_d),
                escape_attr(common.fill_color),
                escape_attr(&stroke_d),
                escape_attr(common.stroke_color),
                util::fmt(common.stroke_width as f64),
                escape_attr(common.stroke_dasharray),
            );
        } else {
            // Keep unsupported CSS colors on the complete classic geometry. Do not emit a
            // partial hand-drawn shape with empty paths when RoughJS admission fails.
            let _ = write!(
                out,
                r#"<path d="{}" class="outer-path" transform="translate({}, {})" style="{}"/>"#,
                escape_attr(&path_data),
                util::fmt(-h / 2.0),
                util::fmt(h / 2.0),
                escape_attr(common.style),
            );
        }
    } else if let Some((fill_d, stroke_d)) =
        super::super::helpers::timed_node_roughjs(common.timing, details, || {
            roughjs_paths_for_svg_path(
                &path_data,
                common.fill_color,
                common.stroke_color,
                common.stroke_width,
                common.stroke_dasharray,
                common.hand_drawn_seed,
            )
        })
        .filter(|(fill_d, stroke_d)| !fill_d.is_empty() && !stroke_d.is_empty())
    {
        let _ = write!(
            out,
            r#"<g transform="translate({},{})" class="outer-path"><path d="{}" stroke="none" stroke-width="0" fill="{}" style="{}"/><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}" style="{}"/></g>"#,
            util::fmt(-h / 2.0),
            util::fmt(h / 2.0),
            escape_attr(&fill_d),
            escape_attr(common.fill_color),
            escape_attr(common.style),
            escape_attr(&stroke_d),
            escape_attr(common.stroke_color),
            util::fmt(common.stroke_width as f64),
            escape_attr(common.stroke_dasharray),
            escape_attr(common.style),
        );
    } else {
        let _ = write!(
            out,
            r#"<path d="{}" class="outer-path" transform="translate({}, {})" style="{}"/>"#,
            escape_attr(&path_data),
            util::fmt(-h / 2.0),
            util::fmt(h / 2.0),
            escape_attr(common.style),
        );
    }

    let node_text_style = flowchart_effective_text_style_for_node_classes(
        &ctx.text_style,
        ctx.class_defs,
        common.node_classes,
        common.node_styles,
    );
    let bbox_y_offset = if ctx.node_html_labels {
        0.0
    } else {
        ctx.measurer
            .measure_svg_create_text_bbox_y_offset_px(label.text, &node_text_style)
    };
    let padding_term = if ctx.node_html_labels { p / 2.0 } else { p };
    label.dy = h / 2.0 - metrics.height / 2.0 - padding_term + bbox_y_offset;
}
