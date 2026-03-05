//! Flowchart v2 icon square shape.

use std::fmt::Write as _;

use crate::svg::parity::flowchart::{
    escape_attr, flowchart_label_html, flowchart_label_plain_text,
};
use crate::svg::parity::fmt;

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

pub(in crate::svg::parity::flowchart::render::node) fn try_render_icon_square(
    out: &mut String,
    ctx: &crate::svg::parity::flowchart::types::FlowchartRenderCtx<'_>,
    label_text: &str,
    label_type: &str,
    node_icon: Option<&str>,
    node_pos: Option<&str>,
    node_asset_width: Option<f64>,
    node_asset_height: Option<f64>,
    _fill_color: &str,
    stroke_color: &str,
    _stroke_width: f32,
    _stroke_dasharray: &str,
    _hand_drawn_seed: u64,
    wrapped_in_a: bool,
) -> bool {
    // Port of Mermaid `iconSquare.ts` (`icon-shape default`).
    if let Some(_icon_name) = node_icon.filter(|s| !s.trim().is_empty()) {
        // Mermaid `labelHelper(...)` uses the flowchart `nodePadding` (15px) and returns `halfPadding`.
        let half_padding = (ctx.node_padding / 2.0).max(0.0);
        let label_text_plain =
            flowchart_label_plain_text(label_text, label_type, ctx.node_html_labels);
        let has_label = !label_text_plain.trim().is_empty();
        let label_padding = if has_label { 8.0 } else { 0.0 };
        let top_label = node_pos == Some("t");

        let asset_h = node_asset_height.unwrap_or(48.0).max(1.0);
        let asset_w = node_asset_width.unwrap_or(48.0).max(1.0);
        let icon_size = asset_h.max(asset_w);

        let height = icon_size + half_padding * 2.0;
        let width = icon_size + half_padding * 2.0;
        let x = -width / 2.0;
        let y = -height / 2.0;

        let mut metrics = crate::flowchart::flowchart_label_metrics_for_layout(
            ctx.measurer,
            label_text,
            label_type,
            &ctx.text_style,
            Some(ctx.wrapping_width),
            ctx.node_wrap_mode,
            ctx.config,
            ctx.math_renderer,
        );
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

        let rounded_rect = rounded_rect_path_d(x, y, width, height, 0.1);
        let (fill_d, stroke_d) = match super::super::roughjs::roughjs_paths_for_svg_path_single_set(
            &rounded_rect,
            _fill_color,
            _fill_color,
            1.3,
            "0 0",
            _hand_drawn_seed,
        ) {
            Some(v) => v,
            None => return false,
        };

        // Icon border/background (RoughJS `rc.path(...)`) — emitted before labels and outer bbox.
        // Mermaid uses `translate(0,18)` without a space after the comma.
        let _ = write!(out, r#"<g transform="translate(0,{})">"#, fmt(icon_dy));
        let _ = write!(
            out,
            r#"<path d="{}" stroke="none" stroke-width="0" fill="{}"/>"#,
            escape_attr(&fill_d),
            escape_attr(_fill_color),
        );
        let _ = write!(
            out,
            r#"<path d="{}" stroke="{}" stroke-width="1.3" fill="none" stroke-dasharray="0 0"/>"#,
            escape_attr(&stroke_d),
            escape_attr(_fill_color),
        );
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
        let label_html =
            flowchart_label_html(label_text, label_type, ctx.config, ctx.math_renderer);
        let label_y = if top_label {
            -outer_h / 2.0 + half_padding
        } else {
            outer_h / 2.0 - label_bbox_h - half_padding
        };
        let _ = write!(
            out,
            r#"<g class="label" style="" transform="translate({},{})"><rect/><foreignObject width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" class="labelBkg" style="display: table-cell; white-space: nowrap; line-height: 1.5; max-width: {}px; text-align: center;"><span class="nodeLabel">{}</span></div></foreignObject></g>"#,
            fmt(-label_bbox_w / 2.0),
            fmt(label_y),
            fmt(label_bbox_w),
            fmt(label_bbox_h),
            fmt(ctx.wrapping_width),
            label_html
        );

        // Outer bbox helper node (transparent fill, no stroke) — emitted after the label group.
        let _ = write!(
            out,
            r#"<g><path d="{}" stroke="none" stroke-width="0" fill="transparent"/></g>"#,
            escape_attr(&outer_path)
        );

        // Mermaid CLI baseline at 11.12.2 renders iconify-based icons via a browser-loaded icon
        // set. In our pinned baselines, the upstream renderer falls back to a placeholder icon SVG
        // (a blue square with a `?`). Mirror that placeholder output here.
        let icon_tx = -icon_size / 2.0;
        let icon_ty = icon_dy - icon_size / 2.0;
        let _ = write!(
            out,
            r#"<g transform="translate({},{})" style="color: {};"><g><svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}" viewBox="0 0 80 80"><g><rect width="80" height="80" style="fill: #087ebf; stroke-width: 0px;"/><text transform="translate(21.16 64.67)" style="fill: #fff; font-family: ArialMT, Arial; font-size: 67.75px;"><tspan x="0" y="0">?</tspan></text></g></svg></g></g>"#,
            fmt(icon_tx),
            fmt(icon_ty),
            escape_attr(stroke_color),
            fmt(icon_size),
            fmt(icon_size),
        );

        out.push_str("</g>");
        if wrapped_in_a {
            out.push_str("</a>");
        }
        return true;
    } else {
        // Fall back to a normal node if the icon name is missing.
    }

    false
}
