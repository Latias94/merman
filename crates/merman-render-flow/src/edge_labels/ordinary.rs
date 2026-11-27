//! Complete ordinary Flow edge-label groups on the caller's sink.

use std::borrow::Cow;

use kurbo::Size;
use merman_render_core::{
    LayoutPoint, Result,
    html::HTML_LABEL_FOREIGN_OBJECT_OVERFLOW_ATTR,
    mermaid_style,
    number_format::canonical_number as fmt_display,
    svg_output::SvgOutput,
    text::{
        WrapMode,
        html_label_layout::{
            flowchart_html_has_inline_style_tags, flowchart_label_plain_text_for_layout,
        },
        inline_html::measure_html_with_inline_styles,
        literal_label::FlowchartSvgWidthMode,
        measure_markdown_with_inline_styles, trim_html_collapsible_ascii_whitespace,
    },
    theme_svg::edge_label_background,
    xml::escape_attr_display,
};
use merman_theme_contract::v2::Color;

use super::{
    FlowchartEdgeLabelText,
    svg::{
        FLOWCHART_EDGE_LABEL_WRAP_WIDTH, SvgEdgeLabel, SvgEdgeLabelText, write_empty_svg_edge_label,
    },
};
use crate::{
    projection::FlowchartRenderEdgeRef,
    style::compiled::flowchart_label_div_style_prefix,
    svg_label::{FlowchartSvgLabelOwner, FlowchartSvgLabelRenderPlan, OptionalStyleAttr},
};

/// Coordinates projected by the route owner into this edge's active SVG group.
pub enum FlowchartEdgeLabelPlacement {
    Layout {
        position: LayoutPoint,
        size: Size,
    },
    /// A visible unboxed label uses its prepared point.
    Fallback(Option<LayoutPoint>),
}

pub struct OrdinaryEdgeLabel<'a> {
    pub edge: FlowchartRenderEdgeRef<'a>,
    pub label_text: &'a str,
    pub placement: Option<FlowchartEdgeLabelPlacement>,
    pub sampled_backdrop: Option<Color>,
    pub html_labels: bool,
    pub html_label_inset: f64,
    pub wrap_mode: WrapMode,
}

/// Append a complete label. The graph layer owns the following sink checkpoint.
pub fn write_ordinary_edge_label<'a>(
    out: &mut impl SvgOutput,
    ctx: FlowchartEdgeLabelText<'a>,
    request: OrdinaryEdgeLabel<'a>,
) -> Result<()> {
    let FlowchartRenderEdgeRef { key, edge } = request.edge;
    let label_text = request.label_text;
    let source_label_empty = label_text.is_empty();
    let sampled_backdrop = request.sampled_backdrop;
    let compiled_label_styles = ctx.edge_style_plan.edge_for(key)?;
    if source_label_empty && !request.html_labels {
        write_empty_svg_edge_label(out, &edge.id);
        return Ok(());
    }

    let label_type = edge.label_type.as_deref().unwrap_or("text");
    let edge_wrap_style = ctx.text_style;
    let edge_metrics_style = ctx.metrics_style(key, source_label_empty)?;
    let base_label_style = compiled_label_styles.label_style.as_str();
    let adaptive_label_ink = sampled_backdrop
        .filter(|_| {
            !compiled_label_styles.has_source_label_foreground()
                && !has_explicit_css_color(base_label_style)
        })
        .and_then(|backdrop| {
            let default_ink = edge_label_background::literal_fill(ctx.default_text_color)?;
            edge_label_background::adaptive_text_ink(backdrop, default_ink)
        });
    let effective_label_style: Cow<'_, str> = match adaptive_label_ink {
        Some(ink) if base_label_style.is_empty() => Cow::Owned(format!("color:{ink} !important;")),
        Some(ink) => Cow::Owned(format!("{base_label_style};color:{ink} !important;")),
        None => Cow::Borrowed(base_label_style),
    };
    let svg_edge_label_text_style = flowchart_svg_edge_label_text_style(
        base_label_style,
        adaptive_label_ink,
        source_label_empty,
    );
    let owner = FlowchartSvgLabelOwner::Edge(key.semantic_index());
    let prepared_svg_label = (!request.html_labels && label_type != "markdown").then(|| {
        FlowchartSvgLabelRenderPlan::new_with_metrics_style(
            ctx.svg_label_sidecar,
            Some(owner),
            label_text,
            ctx.measurer,
            edge_wrap_style,
            edge_metrics_style,
            Some(FLOWCHART_EDGE_LABEL_WRAP_WIDTH),
            true,
            FlowchartSvgWidthMode::Bbox,
        )
    });
    let label_text_plain: Cow<'_, str> = prepared_svg_label.as_ref().map_or_else(
        || {
            if source_label_empty {
                Cow::Borrowed("")
            } else {
                Cow::Owned(flowchart_label_plain_text_for_layout(
                    label_text,
                    label_type,
                    request.html_labels,
                ))
            }
        },
        |prepared| Cow::Borrowed(prepared.plain_text()),
    );
    let span_style_attr = OptionalStyleAttr(effective_label_style.as_ref());
    let div_style_prefix = flowchart_label_div_style_prefix(compiled_label_styles, false);
    if !request.html_labels {
        if let Some(placement) = request.placement {
            let text_is_empty =
                trim_html_collapsible_ascii_whitespace(&label_text_plain).is_empty();
            let (position, layout_size) = match placement {
                FlowchartEdgeLabelPlacement::Layout { position, size } => (
                    Some(position),
                    Some(if text_is_empty {
                        Size::new(4.0, 4.0)
                    } else {
                        size
                    }),
                ),
                FlowchartEdgeLabelPlacement::Fallback(position) if !text_is_empty => (
                    Some(position.expect("visible fallback label is prepared")),
                    None,
                ),
                FlowchartEdgeLabelPlacement::Fallback(_) => (None, None),
            };
            if let Some(position) = position {
                let text = match prepared_svg_label.as_ref() {
                    Some(FlowchartSvgLabelRenderPlan::Prepared { measured, .. }) => {
                        SvgEdgeLabelText::Wrapped(measured.wrapped_lines())
                    }
                    Some(FlowchartSvgLabelRenderPlan::Source { source, style, .. }) => {
                        SvgEdgeLabelText::Source {
                            source: source.as_ref(),
                            wrap_style: style,
                        }
                    }
                    None => SvgEdgeLabelText::Markdown(label_text),
                };
                SvgEdgeLabel {
                    edge_id: &edge.id,
                    position,
                    layout_size,
                    text,
                    plain_text: &label_text_plain,
                    css: svg_edge_label_text_style.as_ref(),
                    measurer: ctx.measurer,
                    metrics_style: edge_metrics_style,
                }
                .write(out);
                return Ok(());
            }
        }

        write_empty_svg_edge_label(out, &edge.id);
        return Ok(());
    }

    let label_html = if source_label_empty {
        Cow::Borrowed("")
    } else {
        ctx.label_html(owner, label_text, label_type)
    };
    let html_label_inset = request.html_label_inset;

    match request.placement {
        Some(FlowchartEdgeLabelPlacement::Layout { position, size }) => {
            let x = position.x;
            let y = position.y;

            let width = size.width.max(0.0);
            let height = size.height.max(0.0);
            let inset = if width > 0.0 && height > 0.0 {
                html_label_inset
            } else {
                0.0
            };
            let div_style = html_edge_label_div_style(&div_style_prefix, width, inset);
            let _ = write!(
                out,
                r#"<g class="edgeLabel" transform="translate({},{})"><g class="label" data-id="{}" transform="translate({},{})">"#,
                fmt_display(x),
                fmt_display(y),
                escape_attr_display(&edge.id),
                fmt_display(-width / 2.0),
                fmt_display(-height / 2.0),
            );
            let _ = write!(
                out,
                r#"<foreignObject width="{}" height="{}"{}><div xmlns="http://www.w3.org/1999/xhtml" class="labelBkg" style="{}"><span class="edgeLabel"{}>{}</span></div></foreignObject>"#,
                fmt_display(width),
                fmt_display(height),
                HTML_LABEL_FOREIGN_OBJECT_OVERFLOW_ATTR,
                escape_attr_display(&div_style),
                span_style_attr,
                label_html,
            );
            out.push_str("</g></g>");
            return Ok(());
        }

        Some(FlowchartEdgeLabelPlacement::Fallback(position))
            if !trim_html_collapsible_ascii_whitespace(&label_text_plain).is_empty() =>
        {
            let position = position.expect("visible fallback label is prepared");
            let (x, y) = (position.x, position.y);
            let has_inline_style_tags = if label_type == "markdown" {
                false
            } else {
                let lower = label_text.to_ascii_lowercase();
                flowchart_html_has_inline_style_tags(&lower)
            };

            let metrics = if label_type == "markdown" {
                measure_markdown_with_inline_styles(
                    ctx.measurer,
                    label_text,
                    edge_metrics_style,
                    Some(FLOWCHART_EDGE_LABEL_WRAP_WIDTH),
                    request.wrap_mode,
                )
            } else if has_inline_style_tags {
                measure_html_with_inline_styles(
                    ctx.measurer,
                    label_text,
                    edge_metrics_style,
                    Some(FLOWCHART_EDGE_LABEL_WRAP_WIDTH),
                    request.wrap_mode,
                )
            } else {
                ctx.measurer.measure_wrapped(
                    &label_text_plain,
                    edge_metrics_style,
                    Some(FLOWCHART_EDGE_LABEL_WRAP_WIDTH),
                    request.wrap_mode,
                )
            };
            let width = metrics.width.max(1.0) + 2.0 * html_label_inset;
            let height = metrics.height.max(1.0);
            let div_style = html_edge_label_div_style(&div_style_prefix, width, html_label_inset);
            let _ = write!(
                out,
                r#"<g class="edgeLabel" transform="translate({},{})"><g class="label" data-id="{}" transform="translate({},{})"><foreignObject width="{}" height="{}"{}><div xmlns="http://www.w3.org/1999/xhtml" class="labelBkg" style="{}"><span class="edgeLabel"{}>{}</span></div></foreignObject></g></g>"#,
                fmt_display(x),
                fmt_display(y),
                escape_attr_display(&edge.id),
                fmt_display(-width / 2.0),
                fmt_display(-height / 2.0),
                fmt_display(width),
                fmt_display(height),
                HTML_LABEL_FOREIGN_OBJECT_OVERFLOW_ATTR,
                escape_attr_display(&div_style),
                span_style_attr,
                label_html
            );
            return Ok(());
        }
        _ => {}
    }

    let _ = write!(
        out,
        r#"<g class="edgeLabel"><g class="label" data-id="{}" transform="translate(0,0)"><foreignObject width="0" height="0"><div xmlns="http://www.w3.org/1999/xhtml" class="labelBkg" style="{}display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 200px; text-align: center;"><span class="edgeLabel"{}></span></div></foreignObject></g></g>"#,
        escape_attr_display(&edge.id),
        escape_attr_display(&div_style_prefix),
        span_style_attr
    );
    Ok(())
}

fn html_edge_label_div_style(prefix: &str, width: f64, inset: f64) -> String {
    let wraps = width - 2.0 * inset >= FLOWCHART_EDGE_LABEL_WRAP_WIDTH - 0.01;
    if inset > 0.0 {
        let white_space = if wraps { "break-spaces" } else { "nowrap" };
        // Keep the content wrap limit for the SVG text fallback. Center the span even when
        // its browser font is slightly wider than the layout measurement.
        format!(
            "{prefix}display: flex; justify-content: center; white-space: {white_space}; line-height: 1.5; max-width: {mw}px; text-align: center; width: calc(100% - {insets}px); box-sizing: content-box; padding-inline: {inset}px;",
            mw = fmt_display(FLOWCHART_EDGE_LABEL_WRAP_WIDTH),
            insets = fmt_display(2.0 * inset),
            inset = fmt_display(inset),
        )
    } else if wraps {
        format!(
            "{prefix}display: table; white-space: break-spaces; line-height: 1.5; max-width: {mw}px; text-align: center; width: {mw}px;",
            mw = fmt_display(FLOWCHART_EDGE_LABEL_WRAP_WIDTH)
        )
    } else {
        format!(
            "{prefix}display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 200px; text-align: center;"
        )
    }
}

fn has_explicit_css_color(style: &str) -> bool {
    let mut found = false;
    mermaid_style::visit_parsed_style_declarations(style, |decl| {
        if decl.property() == "color" {
            found = true;
        }
    });
    found
}

pub(super) fn flowchart_svg_edge_label_text_style<'a>(
    base_label_style: &'a str,
    adaptive_ink: Option<&'static str>,
    source_label_empty: bool,
) -> Cow<'a, str> {
    if source_label_empty {
        return Cow::Borrowed(base_label_style);
    }
    let has_color = has_explicit_css_color(base_label_style);
    if !has_color && adaptive_ink.is_none() {
        return Cow::Borrowed(base_label_style);
    }
    let mut out = String::with_capacity(base_label_style.len() + 16);
    mermaid_style::visit_parsed_style_declarations(base_label_style, |decl| {
        if !out.is_empty() {
            out.push(';');
        }
        if decl.property() == "color" {
            out.push_str("fill:");
            out.push_str(decl.source_value());
        } else {
            out.push_str(decl.property_css());
            out.push(':');
            out.push_str(decl.source_value());
        }
    });
    if let Some(ink) = adaptive_ink {
        if !out.is_empty() {
            out.push(';');
        }
        out.push_str("fill:");
        out.push_str(ink);
    }
    Cow::Owned(out)
}
