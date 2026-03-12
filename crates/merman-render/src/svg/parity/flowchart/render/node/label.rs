//! Flowchart node label renderer.

use std::fmt::Write as _;

use crate::svg::parity::flowchart::label::{flowchart_label_html, flowchart_label_plain_text};
use crate::svg::parity::flowchart::style::FlowchartCompiledStyles;
use crate::svg::parity::flowchart::types::{FlowchartRenderCtx, FlowchartRenderDetails};
use crate::svg::parity::flowchart::util::{OptionalStyleXmlAttr, flowchart_html_contains_img_tag};
use crate::svg::parity::flowchart::write_flowchart_svg_text;
use crate::svg::parity::flowchart::write_flowchart_svg_text_markdown;
use crate::svg::parity::{escape_xml_display, fmt_display};

use super::super::root::flowchart_wrap_svg_text_lines;

pub(in crate::svg::parity::flowchart::render::node) fn render_flowchart_node_label(
    out: &mut String,
    ctx: &FlowchartRenderCtx<'_>,
    common: &super::FlowchartNodeRenderCommon<'_>,
    label: &super::FlowchartNodeLabelState<'_>,
    compiled_styles: &FlowchartCompiledStyles,
    details: &mut FlowchartRenderDetails,
) {
    let label_text_plain =
        flowchart_label_plain_text(label.text, label.label_type, ctx.node_html_labels);
    let node_text_style = crate::flowchart::flowchart_effective_text_style_for_node_classes(
        &ctx.text_style,
        ctx.class_defs,
        common.node_classes,
        common.node_styles,
    );
    let has_literal_backticks = label.label_type != "markdown" && label.text.contains('`');
    let renders_markdown_like = label.label_type == "markdown"
        || (label.label_type != "markdown"
            && !has_literal_backticks
            && (label.text.contains("**")
                || label.text.contains("__")
                || label.text.contains('*')
                || label.text.contains('_')));
    let mut label_dy = label.dy;
    if !ctx.node_html_labels
        && renders_markdown_like
        && crate::text::mermaid_markdown_to_lines(label.text, true).len() > 1
        && matches!(
            common.shape,
            "doc"
                | "document"
                | "lin-cyl"
                | "disk"
                | "lined-cylinder"
                | "tag-doc"
                | "tagged-document"
                | "docs"
                | "documents"
                | "st-doc"
                | "stacked-document"
                | "div-rect"
                | "div-proc"
                | "divided-rectangle"
                | "divided-process"
                | "win-pane"
                | "internal-storage"
                | "window-pane"
        )
    {
        // Mermaid shape renderers override `labelHelper(...)`'s default centering using
        // `-bbox.y`. Chromium reports these wrapped SVG markdown labels with a small positive
        // `getBBox().y`, so model that render-time offset here instead of baking literal `-1`s
        // into individual shapes.
        label_dy -= crate::text::svg_create_text_bbox_y_offset_px(&node_text_style);
    }
    let mut metrics = if let (Some(w), Some(h)) = (
        common.layout_node.label_width,
        common.layout_node.label_height,
    ) {
        // Layout already had to measure labels to compute node sizes. Carry those metrics forward so
        // render does not repeat expensive HTML/markdown measurement work.
        crate::text::TextMetrics {
            width: w,
            height: h,
            line_count: 0,
        }
    } else {
        let mut metrics = crate::flowchart::flowchart_label_metrics_for_layout(
            ctx.measurer,
            label.text,
            label.label_type,
            &node_text_style,
            Some(ctx.wrapping_width),
            ctx.node_wrap_mode,
            ctx.config,
            ctx.math_renderer,
        );
        let span_css_height_parity = crate::flowchart::flowchart_node_has_span_css_height_parity(
            ctx.class_defs,
            common.node_classes,
        );
        if span_css_height_parity {
            crate::text::flowchart_apply_mermaid_styled_node_height_parity(
                &mut metrics,
                &node_text_style,
            );
        }
        metrics
    };
    let label_has_visual_content = flowchart_html_contains_img_tag(label.text)
        || (label.label_type == "markdown" && label.text.contains("!["));
    if label_text_plain.trim().is_empty() && !label_has_visual_content {
        metrics.width = 0.0;
        metrics.height = 0.0;
    }
    if !ctx.node_html_labels {
        let _ = write!(
            out,
            r#"<g class="label" style="{}" transform="translate({},{})"><rect/><g><rect class="background" style="stroke: none"/>"#,
            escape_xml_display(&compiled_styles.label_style),
            fmt_display(label.dx),
            fmt_display(-metrics.height / 2.0 + label_dy)
        );
        if label.label_type == "markdown" {
            write_flowchart_svg_text_markdown(out, label.text, true);
        } else {
            let wrapped = flowchart_wrap_svg_text_lines(
                ctx.measurer,
                &label_text_plain,
                &node_text_style,
                Some(ctx.wrapping_width),
                true,
            )
            .join("\n");
            write_flowchart_svg_text(out, &wrapped, true);
        }
        out.push_str("</g></g></g>");
    } else {
        let label_html =
            super::helpers::timed_node_label_html(common.timing_enabled, details, || {
                flowchart_label_html(label.text, label.label_type, ctx.config, ctx.math_renderer)
            });
        let span_style_attr = OptionalStyleXmlAttr(compiled_styles.label_style.as_str());
        let is_math_html_label = ctx.node_wrap_mode == crate::text::WrapMode::HtmlLike
            && label.text.contains("$$")
            && ctx.math_renderer.is_some();

        let needs_wrap = if ctx.node_wrap_mode == crate::text::WrapMode::HtmlLike {
            if is_math_html_label {
                metrics.width >= ctx.wrapping_width - 0.01
            } else {
                let has_inline_style_tags =
                    ctx.node_html_labels && label.label_type != "markdown" && {
                        let lower = label.text.to_ascii_lowercase();
                        crate::text::flowchart_html_has_inline_style_tags(&lower)
                    };

                let raw = if label.label_type == "markdown" {
                    crate::text::measure_markdown_with_flowchart_bold_deltas(
                        ctx.measurer,
                        label.text,
                        &node_text_style,
                        None,
                        ctx.node_wrap_mode,
                    )
                    .width
                } else if has_inline_style_tags {
                    crate::text::measure_html_with_flowchart_bold_deltas(
                        ctx.measurer,
                        label.text,
                        &node_text_style,
                        None,
                        ctx.node_wrap_mode,
                    )
                    .width
                } else {
                    ctx.measurer
                        .measure_wrapped(
                            &label_text_plain,
                            &node_text_style,
                            None,
                            ctx.node_wrap_mode,
                        )
                        .width
                };
                raw > ctx.wrapping_width
            }
        } else {
            false
        };

        fn parse_hex_rgb_u8(v: &str) -> Option<(u8, u8, u8)> {
            let v = v.trim();
            let hex = v.strip_prefix('#')?;
            match hex.len() {
                6 => {
                    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                    Some((r, g, b))
                }
                3 => {
                    let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
                    let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
                    let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
                    Some((r, g, b))
                }
                _ => None,
            }
        }

        let mut div_style = String::new();
        if let Some(color) = compiled_styles.label_color.as_deref() {
            let color = color.trim();
            if !color.is_empty() {
                if let Some((r, g, b)) = parse_hex_rgb_u8(color) {
                    let _ = write!(&mut div_style, "color: rgb({r}, {g}, {b}) !important; ");
                } else {
                    div_style.push_str("color: ");
                    div_style.push_str(&color.to_ascii_lowercase());
                    div_style.push_str(" !important; ");
                }
            }
        }
        if let Some(v) = compiled_styles.label_font_size.as_deref() {
            let v = v.trim();
            if !v.is_empty() {
                let _ = write!(&mut div_style, "font-size: {v} !important; ");
            }
        }
        if let Some(v) = compiled_styles.label_font_weight.as_deref() {
            let v = v.trim();
            if !v.is_empty() {
                let _ = write!(&mut div_style, "font-weight: {v} !important; ");
            }
        }
        if let Some(v) = compiled_styles.label_font_family.as_deref() {
            let v = v.trim();
            if !v.is_empty() {
                let _ = write!(&mut div_style, "font-family: {v} !important; ");
            }
        }
        if let Some(v) = compiled_styles.label_opacity.as_deref() {
            let v = v.trim();
            if !v.is_empty() {
                let _ = write!(&mut div_style, "opacity: {v} !important; ");
            }
        }
        if needs_wrap {
            let _ = write!(
                &mut div_style,
                "display: table; white-space: break-spaces; line-height: 1.5; max-width: {mw}px; text-align: center; width: {mw}px;",
                mw = fmt_display(ctx.wrapping_width)
            );
        } else {
            let _ = write!(
                &mut div_style,
                "display: table-cell; white-space: nowrap; line-height: 1.5; max-width: {mw}px; text-align: center;",
                mw = fmt_display(ctx.wrapping_width)
            );
        }
        let _ = write!(
            out,
            r#"<g class="label" style="{}" transform="translate({},{})"><rect/><foreignObject width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" style="{}"><span class="nodeLabel"{}>{}</span></div></foreignObject></g></g>"#,
            escape_xml_display(&compiled_styles.label_style),
            fmt_display(-metrics.width / 2.0 + label.dx),
            fmt_display(-metrics.height / 2.0 + label_dy),
            fmt_display(metrics.width),
            fmt_display(metrics.height),
            escape_xml_display(&div_style),
            span_style_attr,
            label_html
        );
    }
    if common.wrapped_in_a {
        out.push_str("</a>");
    }
}
