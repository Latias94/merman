//! Flowchart v2 icon circle shape.

use crate::svg::parity::flowchart::{escape_attr, flowchart_label_plain_text};
use crate::svg::parity::{fmt, fmt_display};

const FRAME_PADDING: f64 = 20.0;
const HAND_DRAWN_FILL_WEIGHT: f64 = 4.0;

pub(in crate::svg::parity::flowchart::render::node) fn render_icon_circle(
    out: &mut impl crate::svg::parity::SvgOutput,
    ctx: &crate::svg::parity::flowchart::types::FlowchartRenderCtx<'_>,
    common: &super::super::FlowchartNodeRenderCommon<'_>,
    label: &super::super::FlowchartNodeLabelState<'_>,
    details: &mut crate::svg::parity::flowchart::types::FlowchartRenderDetails,
) -> crate::Result<super::super::emission::FlowchartNodeLabelEmissionReceipt> {
    // Port of Mermaid `iconCircle.ts` (`icon-shape default`). A populated nested icon SVG has an
    // explicit square viewport, while an empty icon group has a zero-sized browser `getBBox()`.
    let icon_name = common.node_icon.filter(|icon| !icon.trim().is_empty());
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

    let mut metrics = common
        .label_emission
        .metrics(ctx, Some(common.layout_node), label);
    if !has_label {
        metrics.width = 0.0;
        metrics.height = 0.0;
    }

    let label_bbox_w = metrics.width + if has_label { 4.0 } else { 0.0 };
    let label_bbox_h = metrics.height + if has_label { 4.0 } else { 0.0 };
    let icon_bbox_size = if icon_name.is_some() { icon_size } else { 0.0 };
    let diameter = icon_bbox_size * std::f64::consts::SQRT_2 + FRAME_PADDING * 2.0;
    let outer_w = diameter.max(label_bbox_w);
    let outer_h = diameter + label_bbox_h + label_padding;
    let icon_dy = if top_label {
        label_bbox_h / 2.0 + label_padding / 2.0
    } else {
        -label_bbox_h / 2.0 - label_padding / 2.0
    };

    let (fill_d, stroke_d) =
        match super::super::helpers::timed_node_roughjs(common.timing, details, || {
            super::super::roughjs::roughjs_paths_for_circle(
                diameter,
                common.fill_color,
                common.fill_color,
                common.stroke_width,
                common.stroke_dasharray,
                common.look_is_hand_drawn(),
                common.work_meter,
                common.hand_drawn_seed,
            )
        }) {
            Some(paths) => paths,
            None => {
                return Err(crate::Error::InvalidModel {
                    message: "Flowchart iconCircle frame generation failed".to_string(),
                });
            }
        };

    let _ = write!(out, r#"<g transform="translate(0,{})">"#, fmt(icon_dy));
    if common.look_is_hand_drawn() {
        let _ = write!(
            out,
            r#"<path d="{}" stroke="{}" stroke-width="{}" fill="none"/>"#,
            escape_attr(&fill_d),
            escape_attr(common.fill_color),
            fmt_display(HAND_DRAWN_FILL_WEIGHT),
        );
    } else {
        let _ = write!(
            out,
            r#"<path d="{}" stroke="none" stroke-width="0" fill="{}"/>"#,
            escape_attr(&fill_d),
            escape_attr(common.fill_color),
        );
    }
    let _ = write!(
        out,
        r#"<path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}"/>"#,
        escape_attr(&stroke_d),
        escape_attr(common.fill_color),
        fmt_display(common.stroke_width as f64),
        escape_attr(common.stroke_dasharray),
    );
    out.push_str("</g>");

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

    if let Some(icon_name) = icon_name {
        let icon_svg = super::super::helpers::icon_svg_or_placeholder(
            ctx,
            common.node_id,
            icon_name,
            icon_size,
        )?;
        let _ = write!(
            out,
            r#"<g transform="translate({},{})" style="color: {};"><g>{}</g></g>"#,
            fmt(-icon_size / 2.0),
            fmt(icon_dy - icon_size / 2.0),
            escape_attr(common.stroke_color),
            icon_svg,
        );
    }

    let outer_x = -outer_w / 2.0;
    let outer_y = -outer_h / 2.0;
    let outer_path = format!(
        "M{} {} L{} {} L{} {} L{} {}",
        fmt(outer_x),
        fmt(outer_y),
        fmt(outer_x + outer_w),
        fmt(outer_y),
        fmt(outer_x + outer_w),
        fmt(outer_y + outer_h),
        fmt(outer_x),
        fmt(outer_y + outer_h),
    );
    let _ = write!(
        out,
        r#"<g><path d="{}" stroke="none" stroke-width="0" fill="transparent"/></g>"#,
        escape_attr(&outer_path),
    );

    out.push_str("</g>");
    if common.wrapped_in_a {
        out.push_str("</a>");
    }
    Ok(label_receipt)
}
