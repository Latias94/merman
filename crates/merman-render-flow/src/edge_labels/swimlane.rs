//! Complete Swimlane edge-label nodes on the caller's sink.

use std::{borrow::Cow, fmt::Write as _};

use kurbo::Size;
use merman_render_core::{
    Error, LayoutPoint, Result,
    html::HTML_LABEL_FOREIGN_OBJECT_OVERFLOW_ATTR,
    mermaid_style,
    number_format::canonical_number as fmt_display,
    svg_output::SvgOutput,
    text::{
        TextStyle, WrapMode, create_text_html::FlowchartLabelMetricsRequest,
        html_label_layout::measure_flowchart_html_label, literal_label::FlowchartSvgWidthMode,
    },
    theme_svg::edge_label_background,
    xml::{escape_attr_display, escape_xml_display},
};
use merman_theme_contract::v2::Color;

use super::FlowchartEdgeLabelText;
use crate::{
    document::FlowchartDocumentIds,
    projection::FlowchartRenderEdgeRef,
    style::{compiled::FlowchartCompiledStyles, flowchart_is_source_spelled_label_style_key},
    svg_label::{
        FlowchartSvgLabelOwner, FlowchartSvgLabelRenderPlan, OptionalStyleAttr,
        write_flowchart_svg_label_plan_with_style,
    },
};

pub struct SwimlaneEdgeLabel<'a> {
    ctx: FlowchartEdgeLabelText<'a>,
    edge: FlowchartRenderEdgeRef<'a>,
    label_text: &'a str,
    first_label_style: &'a str,
    compiled_label_styles: Option<&'a FlowchartCompiledStyles>,
    label_text_style: &'a TextStyle,
}

pub struct SwimlaneEdgeLabelPaint<'a> {
    pub node_id: &'a str,
    pub document_ids: &'a FlowchartDocumentIds,
    pub position: LayoutPoint,
    pub size: Size,
    pub sampled_backdrop: Option<Color>,
    pub html_labels: bool,
    pub wrap_mode: WrapMode,
    pub wrapping_width: f64,
}

impl<'a> SwimlaneEdgeLabel<'a> {
    /// Resolve the existing style rows before the facade checks geometry and samples its backdrop.
    pub fn bind(
        ctx: FlowchartEdgeLabelText<'a>,
        edge: FlowchartRenderEdgeRef<'a>,
        label_text: &'a str,
        default_edge_style: &'a [String],
    ) -> Result<Self> {
        let first_label_style = default_edge_style
            .first()
            .or_else(|| edge.edge.style.first());
        let compiled_label_styles = ctx.edge_style_plan.swimlane_label_for(edge.key)?;
        let first_label_style = first_label_style.map_or("", String::as_str);
        let label_text_style = ctx.metrics_style(edge.key, label_text.is_empty())?;
        Ok(Self {
            ctx,
            edge,
            label_text,
            first_label_style,
            compiled_label_styles,
            label_text_style,
        })
    }

    /// Append the complete synthetic label. The graph layer owns the next sink checkpoint.
    pub fn write(self, out: &mut impl SvgOutput, paint: SwimlaneEdgeLabelPaint<'_>) -> Result<()> {
        let Self {
            ctx,
            edge: FlowchartRenderEdgeRef { key, edge },
            label_text,
            first_label_style,
            compiled_label_styles,
            label_text_style,
        } = self;
        // Mermaid creates a fresh `labelRect` without copying `edge.labelType`.
        let label_type = "text";
        let owner = FlowchartSvgLabelOwner::SwimlaneEdgeLabel(key.semantic_index());
        let x = paint.position.x;
        let y = paint.position.y;
        let width = paint.size.width;
        let height = paint.size.height;
        let sampled_backdrop = paint.sampled_backdrop;
        let mut has_swimlane_fg =
            compiled_label_styles.is_some_and(FlowchartCompiledStyles::has_source_label_foreground);
        mermaid_style::visit_parsed_style_declarations(first_label_style, |decl| {
            if matches!(decl.property(), "color" | "fill") {
                has_swimlane_fg = true;
            }
        });
        let swimlane_adaptive_ink =
            sampled_backdrop
                .filter(|_| !has_swimlane_fg)
                .and_then(|backdrop| {
                    let default_ink = edge_label_background::literal_fill(ctx.default_text_color)?;
                    edge_label_background::adaptive_text_ink(backdrop, default_ink)
                });
        let trimmed_first_style = first_label_style.trim().trim_end_matches(';');
        let swimlane_group_style: Cow<'_, str> = match swimlane_adaptive_ink {
            Some(ink) if trimmed_first_style.is_empty() => Cow::Owned(format!("fill:{ink}")),
            Some(ink) => Cow::Owned(format!("{trimmed_first_style};fill:{ink}")),
            None => Cow::Borrowed(first_label_style),
        };
        let edge_label_dom_id = paint
            .document_ids
            .synthetic_label(paint.node_id)
            .ok_or_else(|| Error::InvalidModel {
                message: format!(
                    "missing prepared Swimlane edge-label DOM id for `{}`",
                    paint.node_id
                ),
            })?;

        let _ = write!(
            out,
            r#"<g class="label edgeLabel" id="{}" data-id="{}" data-et="edge-label" transform="translate({}, {})"><rect width="0.1" height="0.1"/><g class="label"{} transform="translate({}, {})"><rect/>"#,
            edge_label_dom_id,
            escape_xml_display(&edge.id),
            fmt_display(x),
            fmt_display(y),
            OptionalStyleAttr(swimlane_group_style.as_ref()),
            fmt_display(-width / 2.0),
            fmt_display(-height / 2.0),
        );

        if paint.html_labels {
            let label_html = ctx.label_html(owner, label_text, label_type);
            let needs_wrap = paint.wrap_mode == WrapMode::HtmlLike
                && measure_flowchart_html_label(
                    FlowchartLabelMetricsRequest {
                        measurer: ctx.measurer,
                        raw_label: label_text,
                        label_type,
                        style: label_text_style,
                        max_width_px: None,
                        wrap_mode: paint.wrap_mode,
                        config: ctx.config,
                        math_renderer: None,
                    },
                    &label_html,
                    compiled_label_styles.map_or(
                        Default::default(),
                        FlowchartCompiledStyles::html_label_geometry,
                    ),
                    true,
                )
                .width
                    > paint.wrapping_width;
            // `createText` maps only the first `fill:` declaration to `color:` before applying
            // the copied label style to the HTML span and div. Later fixed declarations override the
            // corresponding inline properties exactly as Mermaid's chained D3 `.style()` calls do.
            let html_label_style =
                swimlane_html_edge_label_style(first_label_style, swimlane_adaptive_ink);
            let mut div_style = String::new();
            let trimmed_html_style = html_label_style.trim().trim_end_matches(';');
            if !trimmed_html_style.is_empty() {
                div_style.push_str(trimmed_html_style);
                div_style.push(';');
            }
            if needs_wrap {
                let _ = write!(
                    &mut div_style,
                    "display: table; white-space: break-spaces; line-height: 1.5; max-width: {width}px; text-align: center; width: {width}px;",
                    width = fmt_display(paint.wrapping_width),
                );
            } else {
                let _ = write!(
                    &mut div_style,
                    "display: table-cell; white-space: nowrap; line-height: 1.5; max-width: {}px; text-align: center;",
                    fmt_display(paint.wrapping_width),
                );
            }
            let _ = write!(
                out,
                r#"<foreignObject width="{}" height="{}"{}><div xmlns="http://www.w3.org/1999/xhtml" style="{}"><span class="nodeLabel"{}>{}</span></div></foreignObject></g></g>"#,
                fmt_display(width),
                fmt_display(height),
                HTML_LABEL_FOREIGN_OBJECT_OVERFLOW_ATTR,
                escape_attr_display(&div_style),
                OptionalStyleAttr(&html_label_style),
                label_html,
            );
            return Ok(());
        }

        out.push_str(r#"<g><rect class="background" style="stroke: none"/>"#);
        let prepared = FlowchartSvgLabelRenderPlan::new(
            ctx.svg_label_sidecar,
            Some(owner),
            label_text,
            ctx.measurer,
            label_text_style,
            Some(paint.wrapping_width),
            true,
            FlowchartSvgWidthMode::Bbox,
        );
        let swimlane_svg_text_style =
            swimlane_svg_edge_label_text_style(first_label_style, swimlane_adaptive_ink);
        write_flowchart_svg_label_plan_with_style(
            out,
            &prepared,
            true,
            (!swimlane_svg_text_style.is_empty()).then_some(swimlane_svg_text_style.as_str()),
        );
        out.push_str("</g></g></g>");
        Ok(())
    }
}

fn swimlane_html_edge_label_style(
    first_label_style: &str,
    adaptive_ink: Option<&'static str>,
) -> String {
    let mut out = String::with_capacity(first_label_style.len() + 24);
    let mut replaced_fill = false;
    mermaid_style::visit_parsed_style_declarations(first_label_style, |decl| {
        if !out.is_empty() {
            out.push(';');
        }
        if !replaced_fill && decl.property() == "fill" {
            replaced_fill = true;
            out.push_str("color:");
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
        let _ = write!(&mut out, "color:{ink} !important");
    }
    out
}

fn swimlane_svg_edge_label_text_style(
    first_label_style: &str,
    adaptive_ink: Option<&'static str>,
) -> String {
    let mut out = String::with_capacity(first_label_style.len() + 16);
    mermaid_style::visit_parsed_style_declarations(first_label_style, |decl| {
        let property = decl.property();
        if property == "color" || property == "fill" {
            if !out.is_empty() {
                out.push(';');
            }
            out.push_str("fill:");
            out.push_str(decl.source_value());
        } else if flowchart_is_source_spelled_label_style_key(property) {
            if !out.is_empty() {
                out.push(';');
            }
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
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edge_labels::ordinary::flowchart_svg_edge_label_text_style;

    #[test]
    fn cssparser_label_style_rewrites_preserve_quoted_semicolons_and_avoid_double_semicolons() {
        assert_eq!(
            flowchart_svg_edge_label_text_style(
                r#"font-family:"Foo;color:red" !important;color:#ff0 !important"#,
                None,
                false,
            ),
            r#"font-family:"Foo;color:red" !important;fill:#ff0 !important"#
        );
        assert_eq!(
            flowchart_svg_edge_label_text_style(
                r#"font-family:"Foo;color:red" !important"#,
                Some("#FFFFFF"),
                false,
            ),
            r#"font-family:"Foo;color:red" !important;fill:#FFFFFF"#
        );
        assert_eq!(
            swimlane_html_edge_label_style(r#"font-family:"Foo;fill:red";FILL:#333;"#, None,),
            r#"font-family:"Foo;fill:red";color:#333"#
        );
        assert_eq!(
            swimlane_html_edge_label_style("font-size:14px;", Some("#FFFFFF")),
            "font-size:14px;color:#FFFFFF !important"
        );
        assert_eq!(
            swimlane_svg_edge_label_text_style(r#"font-family:"Foo;color:red";COLOR:yellow"#, None,),
            r#"font-family:"Foo;color:red";fill:yellow"#
        );
        assert_eq!(
            swimlane_svg_edge_label_text_style("stroke:#2563eb", Some("#FFFFFF")),
            "fill:#FFFFFF"
        );
    }
}
