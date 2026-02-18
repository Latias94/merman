//! Flowchart node label renderer.

use std::fmt::Write as _;

use crate::svg::parity::flowchart::label::{flowchart_label_html, flowchart_label_plain_text};
use crate::svg::parity::flowchart::style::FlowchartCompiledStyles;
use crate::svg::parity::flowchart::types::{FlowchartRenderCtx, FlowchartRenderDetails};
use crate::svg::parity::flowchart::util::{OptionalStyleXmlAttr, flowchart_html_contains_img_tag};
use crate::svg::parity::flowchart::write_flowchart_svg_text;
use crate::svg::parity::{escape_xml_display, fmt_display};

use super::super::root::flowchart_wrap_svg_text_lines;

pub(in crate::svg::parity::flowchart::render::node) fn render_flowchart_node_label(
    out: &mut String,
    ctx: &FlowchartRenderCtx<'_>,
    layout_node: &crate::model::LayoutNode,
    label_text: &str,
    label_type: &str,
    node_classes: &[String],
    node_styles: &[String],
    compiled_styles: &FlowchartCompiledStyles,
    label_dx: f64,
    label_dy: f64,
    compact_label_translate: bool,
    wrapped_in_a: bool,
    timing_enabled: bool,
    details: &mut FlowchartRenderDetails,
) {
    fn label_html_timed(
        timing_enabled: bool,
        details: &mut FlowchartRenderDetails,
        f: impl FnOnce() -> String,
    ) -> String {
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

    let label_text_plain = flowchart_label_plain_text(label_text, label_type, ctx.node_html_labels);
    let node_text_style = crate::flowchart::flowchart_effective_text_style_for_classes(
        &ctx.text_style,
        ctx.class_defs,
        node_classes,
        node_styles,
    );
    let mut metrics =
        if let (Some(w), Some(h)) = (layout_node.label_width, layout_node.label_height) {
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
                label_text,
                label_type,
                &node_text_style,
                Some(ctx.wrapping_width),
                ctx.node_wrap_mode,
            );
            let span_css_height_parity = node_classes.iter().any(|c| {
                ctx.class_defs.get(c.as_str()).is_some_and(|styles| {
                    styles.iter().any(|s| {
                        matches!(
                            s.split_once(':').map(|p| p.0.trim()),
                            Some("background" | "border")
                        )
                    })
                })
            });
            if span_css_height_parity {
                crate::text::flowchart_apply_mermaid_styled_node_height_parity(
                    &mut metrics,
                    &node_text_style,
                );
            }
            metrics
        };
    let label_has_visual_content = flowchart_html_contains_img_tag(label_text)
        || (label_type == "markdown" && label_text.contains("!["));
    if label_text_plain.trim().is_empty() && !label_has_visual_content {
        metrics.width = 0.0;
        metrics.height = 0.0;
    }
    if !ctx.node_html_labels {
        let _ = write!(
            out,
            r#"<g class="label" style="{}" transform="translate({}, {})"><rect/><g><rect class="background" style="stroke: none"/>"#,
            escape_xml_display(&compiled_styles.label_style),
            fmt_display(label_dx),
            fmt_display(-metrics.height / 2.0 + label_dy)
        );
        let wrapped = flowchart_wrap_svg_text_lines(
            ctx.measurer,
            &label_text_plain,
            &node_text_style,
            Some(ctx.wrapping_width),
            true,
        )
        .join("\n");
        write_flowchart_svg_text(out, &wrapped, true);
        out.push_str("</g></g></g>");
    } else {
        let label_html = label_html_timed(timing_enabled, details, || {
            flowchart_label_html(label_text, label_type, ctx.config)
        });
        let span_style_attr = OptionalStyleXmlAttr(compiled_styles.label_style.as_str());
        let needs_wrap = if ctx.node_wrap_mode == crate::text::WrapMode::HtmlLike {
            let has_inline_style_tags = ctx.node_html_labels && label_type != "markdown" && {
                let lower = label_text.to_ascii_lowercase();
                crate::text::flowchart_html_has_inline_style_tags(&lower)
            };

            let raw = if label_type == "markdown" {
                crate::text::measure_markdown_with_flowchart_bold_deltas(
                    ctx.measurer,
                    label_text,
                    &node_text_style,
                    None,
                    ctx.node_wrap_mode,
                )
                .width
            } else if has_inline_style_tags {
                crate::text::measure_html_with_flowchart_bold_deltas(
                    ctx.measurer,
                    label_text,
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
                "display: table; white-space: break-spaces; line-height: 1.5; max-width: 200px; text-align: center; width: {}px;",
                fmt_display(ctx.wrapping_width)
            );
        } else {
            div_style.push_str(
                "display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 200px; text-align: center;",
            );
        }
        if compact_label_translate {
            let _ = write!(
                out,
                r#"<g class="label" style="{}" transform="translate({},{})"><rect/><foreignObject width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" style="{}"><span class="nodeLabel"{}>{}</span></div></foreignObject></g></g>"#,
                escape_xml_display(&compiled_styles.label_style),
                fmt_display(-metrics.width / 2.0 + label_dx),
                fmt_display(-metrics.height / 2.0 + label_dy),
                fmt_display(metrics.width),
                fmt_display(metrics.height),
                escape_xml_display(&div_style),
                span_style_attr,
                label_html
            );
        } else {
            let _ = write!(
                out,
                r#"<g class="label" style="{}" transform="translate({}, {})"><rect/><foreignObject width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" style="{}"><span class="nodeLabel"{}>{}</span></div></foreignObject></g></g>"#,
                escape_xml_display(&compiled_styles.label_style),
                fmt_display(-metrics.width / 2.0 + label_dx),
                fmt_display(-metrics.height / 2.0 + label_dy),
                fmt_display(metrics.width),
                fmt_display(metrics.height),
                escape_xml_display(&div_style),
                span_style_attr,
                label_html
            );
        }
    }
    if wrapped_in_a {
        out.push_str("</a>");
    }
}
