//! Flowchart v2 image square shape.

use std::fmt::Write as _;

use crate::svg::parity::flowchart::types::{FlowchartRenderCtx, FlowchartRenderDetails};
use crate::svg::parity::flowchart::{flowchart_label_html, flowchart_label_plain_text};
use crate::svg::parity::{escape_xml_display, fmt_display};

use super::super::roughjs::roughjs_stroke_path_for_svg_path;

pub(in crate::svg::parity::flowchart::render::node) fn try_render_image_square(
    out: &mut String,
    ctx: &FlowchartRenderCtx<'_>,
    layout_node: &crate::model::LayoutNode,
    label_text: &str,
    label_type: &str,
    node_pos: Option<&str>,
    node_img: Option<&str>,
    node_asset_height: Option<f64>,
    node_asset_width: Option<f64>,
    node_constraint: Option<&str>,
    style: &str,
    fill_color: &str,
    stroke_color: &str,
    stroke_width: f32,
    stroke_dasharray: &str,
    hand_drawn_seed: u64,
    wrapped_in_a: bool,
    timing_enabled: bool,
    details: &mut FlowchartRenderDetails,
) -> bool {
    fn rough_timed<T>(
        timing_enabled: bool,
        details: &mut FlowchartRenderDetails,
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

    fn label_html_timed<T>(
        timing_enabled: bool,
        details: &mut FlowchartRenderDetails,
        f: impl FnOnce() -> T,
    ) -> T {
        if timing_enabled {
            details.node_label_html_calls += 1;
            let start = std::time::Instant::now();
            let out = f();
            details.node_label_html += start.elapsed();
            out
        } else {
            f()
        }
    }

    // Port of Mermaid `imageSquare.ts` (`image-shape default`).
    if let Some(img_href) = node_img.filter(|s| !s.trim().is_empty()) {
        let label_text_plain =
            flowchart_label_plain_text(label_text, label_type, ctx.node_html_labels);
        let has_label = !label_text_plain.trim().is_empty();
        let label_padding = if has_label { 8.0 } else { 0.0 };
        let top_label = node_pos == Some("t");

        let assumed_aspect_ratio = 1.0f64;
        let asset_h = node_asset_height.unwrap_or(60.0).max(1.0);
        let asset_w = node_asset_width.unwrap_or(asset_h).max(1.0);
        let aspect_ratio = if asset_h > 0.0 {
            asset_w / asset_h
        } else {
            assumed_aspect_ratio
        };

        let default_width = ctx.wrapping_width.max(0.0);
        let image_raw_width = asset_w.max(if has_label { default_width } else { 0.0 });

        let constraint_on = node_constraint == Some("on");
        let image_width = if constraint_on && node_asset_height.is_some() {
            asset_h * aspect_ratio
        } else {
            image_raw_width
        };
        let image_height = if constraint_on {
            if aspect_ratio != 0.0 {
                image_width / aspect_ratio
            } else {
                asset_h
            }
        } else {
            asset_h
        };

        let mut metrics = crate::flowchart::flowchart_label_metrics_for_layout(
            ctx.measurer,
            label_text,
            label_type,
            &ctx.text_style,
            Some(ctx.wrapping_width),
            ctx.node_wrap_mode,
        );
        if !has_label {
            metrics.width = 0.0;
            metrics.height = 0.0;
        }

        let outer_w = image_width.max(metrics.width);
        let outer_h = image_height + metrics.height + label_padding;

        let x0 = -image_width / 2.0;
        let y0 = -image_height / 2.0;
        // Mermaid `imageSquare` fills with a straight rect (not rough).
        let rect_fill_path = format!(
            "M{} {} L{} {} L{} {} L{} {}",
            fmt_display(x0),
            fmt_display(y0),
            fmt_display(x0 + image_width),
            fmt_display(y0),
            fmt_display(x0 + image_width),
            fmt_display(y0 + image_height),
            fmt_display(x0),
            fmt_display(y0 + image_height)
        );
        // Stroke uses RoughJS and must be a closed path so the left edge is included.
        let rect_stroke_path = format!(
            "M{} {} L{} {} L{} {} L{} {} L{} {}",
            fmt_display(x0),
            fmt_display(y0),
            fmt_display(x0 + image_width),
            fmt_display(y0),
            fmt_display(x0 + image_width),
            fmt_display(y0 + image_height),
            fmt_display(x0),
            fmt_display(y0 + image_height),
            fmt_display(x0),
            fmt_display(y0)
        );

        let icon_dy = if top_label {
            metrics.height / 2.0 + label_padding / 2.0
        } else {
            -metrics.height / 2.0 - label_padding / 2.0
        };
        let _ = write!(
            out,
            r#"<g transform="translate(0,{})">"#,
            fmt_display(icon_dy)
        );
        let _ = write!(
            out,
            r#"<path d="{}" stroke="none" stroke-width="0" fill="{}"/>"#,
            escape_xml_display(&rect_fill_path),
            escape_xml_display(fill_color)
        );
        if let Some(stroke_d) = rough_timed(timing_enabled, details, || {
            roughjs_stroke_path_for_svg_path(
                &rect_stroke_path,
                stroke_color,
                stroke_width,
                stroke_dasharray,
                hand_drawn_seed,
            )
        }) {
            let _ = write!(
                out,
                r#"<path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}"/>"#,
                escape_xml_display(&stroke_d),
                escape_xml_display(stroke_color),
                fmt_display(stroke_width as f64),
                escape_xml_display(stroke_dasharray)
            );
        }
        out.push_str("</g>");

        // Label group uses a background class in Mermaid's image/icon helpers.
        let label_html = label_html_timed(timing_enabled, details, || {
            flowchart_label_html(label_text, label_type, ctx.config)
        });
        let label_dy = if top_label {
            -image_height / 2.0 - metrics.height / 2.0 - label_padding / 2.0
        } else {
            image_height / 2.0 - metrics.height / 2.0 + label_padding / 2.0
        };
        let _ = write!(
            out,
            concat!(
                r#"<g class="label" style="" transform="translate({},{})">"#,
                r#"<rect/>"#,
                r#"<foreignObject width="{}" height="{}">"#,
                r#"<div xmlns="http://www.w3.org/1999/xhtml" class="labelBkg" "#,
                r#"style="display: table-cell; white-space: nowrap; line-height: 1.5; "#,
                r#"max-width: 200px; text-align: center;"><span class="nodeLabel">{}</span></div>"#,
                r#"</foreignObject></g>"#
            ),
            fmt_display(-metrics.width / 2.0),
            fmt_display(label_dy),
            fmt_display(metrics.width),
            fmt_display(metrics.height),
            label_html
        );

        let outer_x0 = -outer_w / 2.0;
        let outer_y0 = -outer_h / 2.0;
        let outer_path = format!(
            "M{} {} L{} {} L{} {} L{} {}",
            outer_x0,
            outer_y0,
            outer_x0 + outer_w,
            outer_y0,
            outer_x0 + outer_w,
            outer_y0 + outer_h,
            outer_x0,
            outer_y0 + outer_h
        );
        let _ = write!(
            out,
            r#"<g><path d="{}" stroke="none" stroke-width="0" fill="none"/></g>"#,
            escape_xml_display(&outer_path)
        );

        let img_translate_y = if top_label {
            outer_h / 2.0 - image_height
        } else {
            -outer_h / 2.0
        };
        let _ = write!(
            out,
            r#"<image href="{}" width="{}" height="{}" preserveAspectRatio="none" transform="translate({},{})"/>"#,
            escape_xml_display(img_href),
            fmt_display(image_width),
            fmt_display(image_height),
            fmt_display(-image_width / 2.0),
            fmt_display(img_translate_y)
        );

        out.push_str("</g>");
        if wrapped_in_a {
            out.push_str("</a>");
        }
        return true;
    } else {
        // Fall back to a normal node if the image URL is missing.
        let w = layout_node.width.max(1.0);
        let h = layout_node.height.max(1.0);
        let _ = write!(
            out,
            r#"<rect class="basic label-container" style="{}" x="{}" y="{}" width="{}" height="{}"/>"#,
            escape_xml_display(style),
            fmt_display(-w / 2.0),
            fmt_display(-h / 2.0),
            fmt_display(w),
            fmt_display(h)
        );
        // Keep default label rendering.
    }

    false
}
