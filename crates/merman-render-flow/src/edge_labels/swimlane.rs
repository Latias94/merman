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
        svg_label::write_svg_text_source_word_lines_with_style,
    },
    theme_svg::edge_label_background,
    xml::escape_attr_display,
};
use merman_theme_contract::v2::Color;

use super::FlowchartEdgeLabelText;
use crate::{
    document::FlowchartDocumentIds,
    projection::FlowchartRenderEdgeRef,
    style::{
        compiled::FlowchartCompiledStyles, first_label_rect_style,
        flowchart_is_source_spelled_label_style_key,
    },
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
    pub canonical: bool,
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
        let first_label_style =
            first_label_rect_style(default_edge_style, &edge.edge.style).map_or("", String::as_str);
        let compiled_label_styles = ctx.edge_style_plan.label_rect_for(edge.key)?;
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

        let prepared = (!paint.html_labels).then(|| {
            FlowchartSvgLabelRenderPlan::new(
                ctx.svg_label_sidecar,
                Some(owner),
                label_text,
                ctx.measurer,
                label_text_style,
                Some(paint.wrapping_width),
                true,
                FlowchartSvgWidthMode::Bbox,
            )
        });
        let (text_x, text_y, centered_lines) =
            if let Some(label) = prepared.as_ref().filter(|_| paint.canonical) {
                let lines = label.wrapped_lines();
                let bbox_y = ctx
                    .measurer
                    .measure_svg_create_text_bbox_y_offset_px(label.plain_text(), label_text_style);
                (0.0, -height / 2.0 - bbox_y, Some(lines))
            } else {
                (-width / 2.0, -height / 2.0, None)
            };
        let _ = write!(
            out,
            r#"<g class="label edgeLabel" id="{}" data-id="{}" data-et="edge-label" transform="translate({}, {})"><rect width="0.1" height="0.1"/><g class="label"{} transform="translate({}, {})"><rect/>"#,
            edge_label_dom_id,
            escape_attr_display(&edge.id),
            fmt_display(x),
            fmt_display(y),
            OptionalStyleAttr(swimlane_group_style.as_ref()),
            fmt_display(text_x),
            fmt_display(text_y),
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
        let swimlane_svg_text_style =
            swimlane_svg_edge_label_text_style(first_label_style, swimlane_adaptive_ink);
        let swimlane_svg_text_style = crate::svg_label::with_svg_font_fallback(
            &swimlane_svg_text_style,
            ctx.svg_font_fallback,
        );
        if let Some(lines) = centered_lines {
            write_svg_text_source_word_lines_with_style(
                out,
                &lines,
                &swimlane_svg_text_style,
                true,
                None,
            );
        } else {
            write_flowchart_svg_label_plan_with_style(
                out,
                prepared.as_ref().expect("prepared SVG label"),
                true,
                Some(swimlane_svg_text_style.as_ref()),
            );
        }
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
    use crate::{
        class_style::FlowchartClassDefinitions, document::FlowchartDocumentIdRequest,
        edge_labels::ordinary::flowchart_svg_edge_label_text_style, projection::FlowchartEdgeKey,
        style::FlowchartEdgeStylePlan,
    };
    use merman_model::{families::flowchart::FlowchartModel, presentation::PresentationConfig};
    use merman_render_core::{
        LayoutNode,
        resources::{OperationWorkMeter, RenderResourcePolicy},
        svg_css::SvgId,
        text::{DeterministicTextMeasurer, TextMeasurer, TextMetrics},
    };
    use serde_json::json;

    struct OffsetMeasurer(DeterministicTextMeasurer);

    impl TextMeasurer for OffsetMeasurer {
        fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
            self.0.measure(text, style)
        }

        fn measure_svg_create_text_bbox_y_offset_px(&self, _text: &str, _style: &TextStyle) -> f64 {
            4.0
        }
    }

    #[test]
    fn svg_rows_center_on_the_canonical_anchor_using_the_measured_bbox_origin() {
        let model: FlowchartModel = serde_json::from_value(json!({
            "nodes": [],
            "edges": [{"id": "handoff", "from": "a", "to": "b", "length": 1, "label": "Caption"}],
        }))
        .unwrap();
        let edge = FlowchartRenderEdgeRef {
            key: FlowchartEdgeKey::new(0),
            edge: &model.edges[0],
        };
        let config = PresentationConfig::default();
        let class_defs = FlowchartClassDefinitions::new(&model.class_defs);
        let work = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let styles = FlowchartEdgeStylePlan::prepare_with_class_definitions(
            &model,
            &config,
            true,
            &class_defs,
            &work,
        )
        .unwrap();
        let text_style = TextStyle::default();
        styles.bind_terminal_typography(&text_style, true).unwrap();
        let ids = FlowchartDocumentIds::prepare(FlowchartDocumentIdRequest {
            diagram_id: SvgId::new("swimlane").unwrap(),
            edge_order: &[edge],
            node_dom_index: &Default::default(),
            subgraph_index_by_id: &Default::default(),
            layout_nodes: &[LayoutNode {
                id: "label".into(),
                x: 0.0,
                y: 0.0,
                width: 80.0,
                height: 20.0,
                is_cluster: false,
                label_width: None,
                label_height: None,
            }],
            swimlane_nodes: None,
            swimlane_lanes: None,
        });
        let measurer = OffsetMeasurer(DeterministicTextMeasurer::default());
        for (label, rows) in [("Caption", 1), ("Caption<br/>Details", 2)] {
            for (canonical, transform) in
                [(true, "translate(0, -14)"), (false, "translate(-40, -10)")]
            {
                let text = FlowchartEdgeLabelText {
                    edge_style_plan: &styles,
                    svg_label_sidecar: None,
                    measurer: &measurer,
                    math_renderer: None,
                    text_style: &text_style,
                    svg_font_fallback: None,
                    config: &config,
                    default_text_color: "black",
                };
                let mut svg = String::new();
                SwimlaneEdgeLabel::bind(text, edge, label, &[])
                    .unwrap()
                    .write(
                        &mut svg,
                        SwimlaneEdgeLabelPaint {
                            node_id: "label",
                            document_ids: &ids,
                            position: LayoutPoint { x: 0.0, y: 0.0 },
                            size: Size::new(80.0, 20.0),
                            sampled_backdrop: None,
                            html_labels: false,
                            canonical,
                            wrap_mode: WrapMode::SvgLike,
                            wrapping_width: 200.0,
                        },
                    )
                    .unwrap();
                assert!(
                    svg.contains(&format!(r#"transform="{transform}"><rect/>"#)),
                    "{svg}"
                );
                assert_eq!(svg.matches("row text-outer-tspan").count(), rows, "{svg}");
                assert_eq!(svg.contains(r#"text-anchor="middle""#), canonical, "{svg}");
            }
        }
    }

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
