//! Flowchart v2 icon square shape.

use std::fmt::Write as _;

use crate::svg::parity::flowchart::{escape_attr, flowchart_label_plain_text};
use crate::svg::parity::fmt;

use super::super::helpers;

fn rounded_rect_path_d(x: f64, y: f64, w: f64, h: f64, r: f64) -> String {
    // Port of Mermaid `createRoundedRectPathD(...)` (`roundedRectPath.ts`).
    let mut out = String::new();
    let _ = write!(
        &mut out,
        "M {} {} H {} A {} {} 0 0 1 {} {} V {} A {} {} 0 0 1 {} {} H {} A {} {} 0 0 1 {} {} V {} A {} {} 0 0 1 {} {} Z",
        fmt(x + r),
        fmt(y),
        fmt(x + w - r),
        fmt(r),
        fmt(r),
        fmt(x + w),
        fmt(y + r),
        fmt(y + h - r),
        fmt(r),
        fmt(r),
        fmt(x + w - r),
        fmt(y + h),
        fmt(x + r),
        fmt(r),
        fmt(r),
        fmt(x),
        fmt(y + h - r),
        fmt(y + r),
        fmt(r),
        fmt(r),
        fmt(x + r),
        fmt(y),
    );
    out
}

pub(in crate::svg::parity::flowchart::render::node) fn render_icon_square(
    out: &mut impl crate::svg::parity::SvgOutput,
    ctx: &crate::svg::parity::flowchart::types::FlowchartRenderCtx<'_>,
    common: &super::super::FlowchartNodeRenderCommon<'_>,
    label: &super::super::FlowchartNodeLabelState<'_>,
    details: &mut crate::svg::parity::flowchart::types::FlowchartRenderDetails,
) -> crate::Result<super::super::emission::FlowchartNodeLabelEmissionReceipt> {
    render_icon_rect_frame(out, ctx, common, label, details, 0.1, None)
}

pub(in crate::svg::parity::flowchart::render::node) fn render_icon_rounded(
    out: &mut impl crate::svg::parity::SvgOutput,
    ctx: &crate::svg::parity::flowchart::types::FlowchartRenderCtx<'_>,
    common: &super::super::FlowchartNodeRenderCommon<'_>,
    label: &super::super::FlowchartNodeLabelState<'_>,
    details: &mut crate::svg::parity::flowchart::types::FlowchartRenderDetails,
) -> crate::Result<super::super::emission::FlowchartNodeLabelEmissionReceipt> {
    render_icon_rect_frame(out, ctx, common, label, details, 5.0, Some("icon-shape2"))
}

fn render_icon_rect_frame(
    out: &mut impl crate::svg::parity::SvgOutput,
    ctx: &crate::svg::parity::flowchart::types::FlowchartRenderCtx<'_>,
    common: &super::super::FlowchartNodeRenderCommon<'_>,
    label: &super::super::FlowchartNodeLabelState<'_>,
    details: &mut crate::svg::parity::flowchart::types::FlowchartRenderDetails,
    corner_radius: f64,
    frame_class: Option<&str>,
) -> crate::Result<super::super::emission::FlowchartNodeLabelEmissionReceipt> {
    // Port of Mermaid `iconSquare.ts` and `iconRounded.ts` (`icon-shape default`).
    let icon_name = common.node_icon.filter(|icon| !icon.trim().is_empty());
    // Mermaid `labelHelper(...)` uses the flowchart `nodePadding` (15px) and returns `halfPadding`.
    let half_padding = (ctx.node_padding / 2.0).max(0.0);
    let label_text_plain =
        flowchart_label_plain_text(label.text, label.label_type, ctx.node_html_labels);
    let has_label = !crate::flowchart::flowchart_label_text_is_empty_for_mode(
        &label_text_plain,
        ctx.node_html_labels,
    );
    let label_padding = if has_label { 8.0 } else { 0.0 };
    let top_label = common.node_pos == Some("t");

    let asset_h = common.node_asset_height.unwrap_or(48.0);
    let asset_w = common.node_asset_width.unwrap_or(48.0);
    let icon_size = asset_h.max(asset_w);

    let height = icon_size + half_padding * 2.0;
    let width = icon_size + half_padding * 2.0;
    let x = -width / 2.0;
    let y = -height / 2.0;
    let mut metrics = common
        .label_emission
        .metrics(ctx, Some(common.layout_node), label);
    if !has_label {
        metrics.width = 0.0;
        metrics.height = 0.0;
    }

    // Mermaid's `labelHelper(...)` wraps icon labels in `.labelBkg` (2px padding).
    let label_bbox_w = metrics.width + if has_label { 4.0 } else { 0.0 };
    let label_bbox_h = metrics.height + if has_label { 4.0 } else { 0.0 };

    let outer_w = width.max(label_bbox_w);
    let outer_h = height + label_bbox_h + label_padding;

    let icon_dy = if top_label {
        label_bbox_h / 2.0 + label_padding / 2.0
    } else {
        -label_bbox_h / 2.0 - label_padding / 2.0
    };

    let rounded_rect = rounded_rect_path_d(x, y, width, height, corner_radius);
    let hand_drawn = common.look_is_hand_drawn();
    let frame_paths = if hand_drawn {
        helpers::hand_drawn_path_pair_with_colors(
            true,
            common.timing,
            details,
            &rounded_rect,
            common.fill_color,
            common.fill_color,
            1.3,
            "0 0",
            common.hand_drawn_seed,
        )
    } else {
        super::super::helpers::timed_node_roughjs(common.timing, details, || {
            super::super::roughjs::roughjs_paths_for_svg_path_single_set(
                &rounded_rect,
                common.fill_color,
                common.fill_color,
                1.3,
                "0 0",
                common.hand_drawn_seed,
            )
        })
    };
    let raw_hand_drawn_frame = hand_drawn && frame_paths.is_none();
    let (fill_d, stroke_d) = match frame_paths {
        Some(v) => v,
        None if hand_drawn => (rounded_rect.clone(), rounded_rect.clone()),
        None => {
            return Err(crate::Error::InvalidModel {
                message: "Flowchart icon frame generation failed".to_string(),
            });
        }
    };

    // Icon border/background (RoughJS `rc.path(...)`) — emitted before labels and outer bbox.
    // Mermaid uses `translate(0,18)` without a space after the comma.
    if let Some(frame_class) = frame_class {
        let _ = write!(
            out,
            r#"<g class="{}" transform="translate(0,{})"{}>"#,
            escape_attr(frame_class),
            fmt(icon_dy),
            if hand_drawn {
                format!(r#" style="{}""#, escape_attr(common.rough_group_style))
            } else {
                String::new()
            }
        );
    } else {
        let _ = write!(
            out,
            r#"<g transform="translate(0,{})"{}>"#,
            fmt(icon_dy),
            if hand_drawn {
                format!(r#" style="{}""#, escape_attr(common.rough_group_style))
            } else {
                String::new()
            }
        );
    }
    if raw_hand_drawn_frame {
        let _ = write!(
            out,
            r#"<path d="{}" fill="{}" stroke="{}" stroke-width="1.3" style="{}"/>"#,
            escape_attr(&rounded_rect),
            escape_attr(common.fill_color),
            escape_attr(common.fill_color),
            escape_attr(common.style),
        );
    } else if hand_drawn {
        let _ = write!(
            out,
            r#"<path d="{}" stroke="{}" stroke-width="4" fill="none" stroke-dasharray="0 0"/><path d="{}" stroke="{}" stroke-width="1.3" fill="none" stroke-dasharray="0 0"/>"#,
            escape_attr(&fill_d),
            escape_attr(common.fill_color),
            escape_attr(&stroke_d),
            escape_attr(common.fill_color),
        );
    } else {
        let _ = write!(
            out,
            r#"<path d="{}" stroke="none" stroke-width="0" fill="{}"/>"#,
            escape_attr(&fill_d),
            escape_attr(common.fill_color),
        );
        let _ = write!(
            out,
            r#"<path d="{}" stroke="{}" stroke-width="1.3" fill="none" stroke-dasharray="0 0"/>"#,
            escape_attr(&stroke_d),
            escape_attr(common.fill_color),
        );
    }
    out.push_str("</g>");

    let outer_x0 = -outer_w / 2.0;
    let outer_y0 = -outer_h / 2.0;
    let outer_path = format!(
        "M{} {} L{} {} L{} {} L{} {}",
        fmt(outer_x0),
        fmt(outer_y0),
        fmt(outer_x0 + outer_w),
        fmt(outer_y0),
        fmt(outer_x0 + outer_w),
        fmt(outer_y0 + outer_h),
        fmt(outer_x0),
        fmt(outer_y0 + outer_h)
    );
    let label_y = if top_label {
        -outer_h / 2.0
    } else {
        outer_h / 2.0 - label_bbox_h
    };
    let label_receipt = common.label_emission.write_special_html_label(
        out,
        ctx,
        common,
        label,
        details,
        -label_bbox_w / 2.0,
        label_y,
        label_bbox_w,
        label_bbox_h,
    );

    // Outer bbox helper node (transparent fill, no stroke) — emitted after the label group.
    let _ = write!(
        out,
        r#"<g><path d="{}" stroke="none" stroke-width="0" fill="transparent"/></g>"#,
        escape_attr(&outer_path)
    );

    if let Some(icon_name) = icon_name {
        let icon_tx = -icon_size / 2.0;
        let icon_ty = icon_dy - icon_size / 2.0;
        let icon_svg = super::super::helpers::icon_svg_or_placeholder(
            ctx,
            common.node_id,
            icon_name,
            icon_size,
        )?;
        let _ = write!(
            out,
            r#"<g transform="translate({},{})" style="color: {};"><g>{}</g></g>"#,
            fmt(icon_tx),
            fmt(icon_ty),
            escape_attr(common.stroke_color),
            icon_svg,
        );
    }

    out.push_str("</g>");
    if common.wrapped_in_a {
        out.push_str("</a>");
    }
    Ok(label_receipt)
}
