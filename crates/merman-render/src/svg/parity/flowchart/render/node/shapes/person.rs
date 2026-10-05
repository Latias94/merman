//! Flowchart v2 person shape.

use crate::svg::parity::flowchart::escape_attr;
use crate::svg::parity::{fmt, fmt_display};

use super::super::geom::{path_from_points, rounded_rect_points};
use super::super::roughjs::{roughjs_paths_for_circle, roughjs_paths_for_hand_drawn_svg_path};

pub(in crate::svg::parity::flowchart::render::node) fn render_person(
    out: &mut impl crate::svg::parity::SvgOutput,
    ctx: &crate::svg::parity::flowchart::types::FlowchartRenderCtx<'_>,
    common: &super::super::FlowchartNodeRenderCommon<'_>,
    label: &mut super::super::FlowchartNodeLabelState<'_>,
    details: &mut crate::svg::parity::flowchart::types::FlowchartRenderDetails,
) {
    let metrics = super::super::helpers::compute_node_label_metrics(
        ctx,
        Some(common.layout_node),
        label.text,
        label.label_type,
        common.node_classes,
        common.node_styles,
    );
    let p = ctx.node_padding.max(0.0);
    let w = (metrics.width + 2.0 * p)
        .max(common.layout_node.width.max(0.0))
        .max(100.0);
    let head_radius = (w * 0.23).clamp(16.0, 56.0);
    let overlap = head_radius * 0.27;
    let body_height = (metrics.height + 2.0 * p)
        .max((common.layout_node.height - (2.0 * head_radius - overlap)).max(0.0));
    let body_radius = (w * 0.177).min(body_height * 0.45);
    let total_height = body_height + 2.0 * head_radius - overlap;
    let top = -total_height / 2.0;
    let body_top = top + 2.0 * head_radius - overlap;
    let body_path = path_from_points(&rounded_rect_points(
        -w / 2.0,
        body_top,
        w,
        body_height,
        body_radius,
    ));
    let head_center_y = top + head_radius;

    if common.look_is_hand_drawn() {
        let _ = write!(
            out,
            r#"<g class="basic label-container" style="{}">"#,
            escape_attr(common.rough_group_style)
        );
    } else {
        out.push_str(r#"<g class="basic label-container">"#);
    }
    if common.look_is_hand_drawn() {
        if let Some((fill_d, stroke_d)) =
            super::super::helpers::timed_node_roughjs(common.timing, details, || {
                roughjs_paths_for_hand_drawn_svg_path(
                    &body_path,
                    common.stroke_width,
                    common.stroke_dasharray,
                    common.work_meter,
                    common.hand_drawn_seed,
                )
            })
        {
            let _ = write!(
                out,
                r#"<path d="{}" stroke="{}" stroke-width="4" fill="none" stroke-dasharray="0 0"/><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}"/>"#,
                escape_attr(&fill_d),
                escape_attr(common.fill_color),
                escape_attr(&stroke_d),
                escape_attr(common.stroke_color),
                fmt_display(common.stroke_width as f64),
                escape_attr(common.stroke_dasharray),
            );
        } else {
            let _ = write!(
                out,
                r#"<path d="{}" style="{}"/>"#,
                escape_attr(&body_path),
                escape_attr(common.style),
            );
        }
        if let Some((fill_d, stroke_d)) =
            super::super::helpers::timed_node_roughjs(common.timing, details, || {
                roughjs_paths_for_circle(
                    head_radius * 2.0,
                    common.stroke_width,
                    common.stroke_dasharray,
                    true,
                    common.work_meter,
                    common.hand_drawn_seed,
                )
            })
        {
            let _ = write!(
                out,
                r#"<g transform="translate(0,{})"><path d="{}" stroke="{}" stroke-width="4" fill="none" stroke-dasharray="0 0"/><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}"/></g>"#,
                fmt(head_center_y),
                escape_attr(&fill_d),
                escape_attr(common.fill_color),
                escape_attr(&stroke_d),
                escape_attr(common.stroke_color),
                fmt_display(common.stroke_width as f64),
                escape_attr(common.stroke_dasharray),
            );
        } else {
            let _ = write!(
                out,
                r#"<circle cx="0" cy="{}" r="{}" style="{}"/>"#,
                fmt(head_center_y),
                fmt(head_radius),
                escape_attr(common.style),
            );
        }
    } else {
        let _ = write!(
            out,
            r#"<rect x="{}" y="{}" width="{}" height="{}" rx="{}" ry="{}" style="{}"/><circle cx="0" cy="{}" r="{}" style="{}"/>"#,
            fmt(-w / 2.0),
            fmt(body_top),
            fmt(w),
            fmt(body_height),
            fmt(body_radius),
            fmt(body_radius),
            escape_attr(common.style),
            fmt(head_center_y),
            fmt(head_radius),
            escape_attr(common.style),
        );
    }
    out.push_str("</g>");

    let body_center_y = body_top + body_height / 2.0;
    label.dy = body_center_y;
}
