//! Flowchart node label renderer.

use crate::diagram_theme::ThemeTarget;
use std::fmt::Write as _;

use crate::svg::parity::flowchart::label::{
    flowchart_label_html_with_prepared_math, flowchart_label_plain_text,
};
use crate::svg::parity::flowchart::style::{
    FlowchartCompiledStyles, flowchart_label_div_style_prefix,
};
use crate::svg::parity::flowchart::types::{FlowchartRenderCtx, FlowchartRenderDetails};
use crate::svg::parity::flowchart::util::{
    HTML_LABEL_FOREIGN_OBJECT_OVERFLOW_ATTR, OptionalStyleXmlAttr, flowchart_html_contains_img_tag,
};
use crate::svg::parity::flowchart::{
    write_flowchart_svg_label_plan_with_style, write_flowchart_svg_text_markdown_wrapped_with_style,
};
use crate::svg::parity::{escape_xml_display, fmt_display};

pub(super) struct FlowchartNodeLabelEmissionPlan<'a> {
    node_classes: &'a [String],
    node_styles: &'a [String],
    compiled_styles: &'a FlowchartCompiledStyles,
    typed_label_fill: Option<&'a str>,
}

impl<'a> FlowchartNodeLabelEmissionPlan<'a> {
    pub(super) const fn new(
        node_classes: &'a [String],
        node_styles: &'a [String],
        compiled_styles: &'a FlowchartCompiledStyles,
        typed_label_fill: Option<&'a str>,
    ) -> Self {
        Self {
            node_classes,
            node_styles,
            compiled_styles,
            typed_label_fill,
        }
    }

    fn typed_label_fill_style(&self) -> Option<String> {
        self.typed_label_fill.map(|fill| {
            let mut style = String::new();
            append_typed_label_fill(&mut style, fill);
            style
        })
    }

    fn text_style<'b>(
        &'b self,
        ctx: &'b FlowchartRenderCtx<'_>,
    ) -> std::borrow::Cow<'b, crate::text::TextStyle> {
        let base = if ctx.node_wrap_mode == crate::text::WrapMode::HtmlLike {
            &ctx.html_label_text_style
        } else {
            &ctx.text_style
        };
        let source = crate::flowchart::flowchart_effective_text_style_for_node_classes(
            base,
            ctx.class_defs,
            self.node_classes,
            self.node_styles,
        );
        ctx.svg_label_sidecar
            .map_or(
                crate::flowchart::FlowchartLabelWeights::default(),
                |sidecar| sidecar.label_weights(),
            )
            .apply_cow(ThemeTarget::NodeLabel, source)
    }

    pub(super) fn metrics(
        &self,
        ctx: &FlowchartRenderCtx<'_>,
        layout_node: Option<&crate::model::LayoutNode>,
        label: &super::FlowchartNodeLabelState<'_>,
    ) -> crate::text::TextMetrics {
        let text_style = self.text_style(ctx);
        super::helpers::compute_node_label_metrics_with_style(
            ctx,
            layout_node,
            label.text,
            label.label_type,
            text_style.as_ref(),
        )
    }

    fn final_style(
        &self,
        ctx: &FlowchartRenderCtx<'_>,
        text_style: &crate::text::TextStyle,
        prepared: Option<&crate::flowchart::FlowchartSvgLabelRenderPlan<'_>>,
    ) -> String {
        let mut style = prepared
            .and_then(|plan| {
                plan.merge_emission_font_style(Some(self.compiled_styles.label_style.as_str()))
            })
            .unwrap_or_else(|| self.compiled_styles.label_style.clone());
        if let Some(fill) = self.typed_label_fill {
            append_typed_label_fill(&mut style, fill);
        }
        let weights = ctx.svg_label_sidecar.map_or(
            crate::flowchart::FlowchartLabelWeights::default(),
            |sidecar| sidecar.label_weights(),
        );
        if weights.get(ThemeTarget::NodeLabel).is_some() {
            weights.append_style(ThemeTarget::NodeLabel, text_style, &mut style);
        }
        style
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn write_special_html_label(
        &self,
        out: &mut impl crate::svg::parity::SvgOutput,
        ctx: &FlowchartRenderCtx<'_>,
        common: &super::FlowchartNodeRenderCommon<'_>,
        label: &super::FlowchartNodeLabelState<'_>,
        details: &mut FlowchartRenderDetails,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    ) -> super::emission::FlowchartNodeLabelEmissionReceipt {
        let text_style = self.text_style(ctx);
        let owner = ctx.svg_label_sidecar.and_then(|sidecar| {
            sidecar.node_owner(common.node_id, ctx.swimlane_direction.is_some())
        });
        let prepared_svg_label =
            (!ctx.node_html_labels && label.label_type != "markdown").then(|| {
                crate::flowchart::FlowchartSvgLabelRenderPlan::new(
                    ctx.svg_label_sidecar,
                    owner,
                    label.text,
                    ctx.measurer,
                    text_style.as_ref(),
                    Some(ctx.wrapping_width),
                    true,
                    crate::flowchart::flowchart_node_svg_width_mode(
                        label.text,
                        label.label_type,
                        ctx.node_wrap_mode,
                        common.shape,
                    ),
                )
            });
        let final_style = self.final_style(ctx, &text_style, prepared_svg_label.as_ref());
        let span_style_attr = OptionalStyleXmlAttr(final_style.as_str());
        let prepared_math = ctx
            .svg_label_sidecar
            .zip(owner)
            .map_or(Default::default(), |(sidecar, owner)| {
                sidecar.prepared_math_for_terminal(owner, label.text)
            });
        let label_html = super::helpers::timed_node_label_html(common.timing, details, || {
            flowchart_label_html_with_prepared_math(
                label.text,
                label.label_type,
                ctx.config,
                ctx.math_renderer,
                prepared_math,
            )
        });
        let mut div_style = final_style.clone();
        if !div_style.is_empty() {
            div_style.push(';');
        }
        let _ = write!(
            &mut div_style,
            "display: table-cell; white-space: nowrap; line-height: 1.5; max-width: {}px; text-align: center;",
            fmt_display(ctx.wrapping_width)
        );
        let _ = write!(
            out,
            r#"<g class="label" style="{}" transform="translate({},{})"><rect/><foreignObject width="{}" height="{}"{}><div xmlns="http://www.w3.org/1999/xhtml" class="labelBkg" style="{}"><span class="{}"{}>{}</span></div></foreignObject></g>"#,
            escape_xml_display(&final_style),
            fmt_display(x),
            fmt_display(y),
            fmt_display(width),
            fmt_display(height),
            HTML_LABEL_FOREIGN_OBJECT_OVERFLOW_ATTR,
            escape_xml_display(&div_style),
            super::helpers::flowchart_node_label_span_class(label.label_type),
            span_style_attr,
            label_html,
        );
        let typography_applicable = flowchart_node_label_typography_is_applicable(
            &flowchart_label_plain_text(label.text, label.label_type, ctx.node_html_labels),
            ctx.node_html_labels,
        );
        let (html_font_stack, html_font_size) =
            crate::svg::parity::flowchart::style::sanitized_xhtml_typography_statuses(
                label_html.as_ref(),
            );
        let weight_verified = label_weight_is_verified(
            ctx,
            label,
            Some(label_html.as_ref()),
            &["label", "nodeLabel", "labelBkg"],
            self.node_classes,
        );
        super::emission::FlowchartNodeLabelEmissionReceipt::verified()
            .with_font_weight_reach(typography_applicable, weight_verified)
            .with_background_area(width > 0.0 && height > 0.0)
            .with_prepared_typography_reach(
                typography_applicable,
                prepared_svg_label.as_ref().is_some_and(
                    crate::flowchart::FlowchartSvgLabelRenderPlan::emitted_admitted_typography,
                ),
            )
            .with_label_fill_reach(typography_applicable, true)
            .with_html_typography_statuses(html_font_stack, html_font_size)
            .with_text_paint_facts(ctx.text_surface_paint.generic_text.requested().then(|| {
                crate::flowchart::FlowchartTextPaintFacts::from_visible(
                    &crate::text::VisibleTextStyleFacts::from_xhtml_fragment(label_html.as_ref()),
                )
            }))
    }
}

pub(in crate::svg::parity::flowchart::render::node) fn render_flowchart_node_label(
    out: &mut impl crate::svg::parity::SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    common: &super::FlowchartNodeRenderCommon<'_>,
    label: &super::FlowchartNodeLabelState<'_>,
    details: &mut FlowchartRenderDetails,
) -> super::emission::FlowchartNodeLabelEmissionReceipt {
    render_flowchart_node_label_with_wrapper(out, ctx, common, label, details, true)
}

pub(in crate::svg::parity::flowchart::render::node) fn render_flowchart_node_label_before_tail(
    out: &mut impl crate::svg::parity::SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    common: &super::FlowchartNodeRenderCommon<'_>,
    label: &super::FlowchartNodeLabelState<'_>,
    details: &mut FlowchartRenderDetails,
) -> super::emission::FlowchartNodeLabelEmissionReceipt {
    render_flowchart_node_label_with_wrapper(out, ctx, common, label, details, false)
}

fn render_flowchart_node_label_with_wrapper(
    out: &mut impl crate::svg::parity::SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    common: &super::FlowchartNodeRenderCommon<'_>,
    label: &super::FlowchartNodeLabelState<'_>,
    details: &mut FlowchartRenderDetails,
    close_node_wrapper: bool,
) -> super::emission::FlowchartNodeLabelEmissionReceipt {
    let node_text_style = common.label_emission.text_style(ctx);
    let owner = ctx
        .svg_label_sidecar
        .and_then(|sidecar| sidecar.node_owner(common.node_id, ctx.swimlane_direction.is_some()));
    let prepared_svg_label = (!ctx.node_html_labels && label.label_type != "markdown").then(|| {
        crate::flowchart::FlowchartSvgLabelRenderPlan::new(
            ctx.svg_label_sidecar,
            owner,
            label.text,
            ctx.measurer,
            node_text_style.as_ref(),
            Some(ctx.wrapping_width),
            true,
            crate::flowchart::flowchart_node_svg_width_mode(
                label.text,
                label.label_type,
                ctx.node_wrap_mode,
                common.shape,
            ),
        )
    });
    let label_text_plain = prepared_svg_label.as_ref().map_or_else(
        || flowchart_label_plain_text(label.text, label.label_type, ctx.node_html_labels),
        |prepared| prepared.plain_text().to_string(),
    );
    let mut label_dy = label.dy;
    if !ctx.node_html_labels
        && flowchart_node_label_uses_markdown_bbox(label.label_type, label.text)
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
        label_dy -= ctx
            .measurer
            .measure_svg_create_text_bbox_y_offset_px(label.text, &node_text_style);
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
        crate::flowchart::flowchart_label_metrics_for_layout(
            crate::flowchart::FlowchartLabelMetricsRequest {
                measurer: ctx.measurer,
                raw_label: label.text,
                label_type: label.label_type,
                style: &node_text_style,
                max_width_px: Some(ctx.wrapping_width),
                wrap_mode: ctx.node_wrap_mode,
                config: ctx.config,
                math_renderer: ctx.math_renderer,
            },
        )
    };
    let label_has_visual_content = flowchart_html_contains_img_tag(label.text)
        || (label.label_type == "markdown" && label.text.contains("!["));
    if crate::flowchart::flowchart_label_text_is_empty_for_mode(
        &label_text_plain,
        ctx.node_html_labels,
    ) && !label_has_visual_content
    {
        metrics.width = 0.0;
        metrics.height = 0.0;
    }
    let label_group_class = if common.shape == "note" {
        "label noteLabel"
    } else {
        "label"
    };
    let mut label_effect_emitted = false;
    let mut text_paint_facts = None;
    let mut html_weight_verified = true;
    let mut html_typography_statuses = (
        crate::flowchart::FlowchartSourceFacetStatus::Absent,
        crate::flowchart::FlowchartSourceFacetStatus::Absent,
    );
    if !ctx.node_html_labels {
        if ctx.text_surface_paint.generic_text.requested() {
            let facts = if label.label_type == "markdown" {
                crate::text::VisibleTextStyleFacts::from_svg_markdown_projection(label.text)
            } else {
                crate::text::VisibleTextStyleFacts::plain_text(&label_text_plain)
            };
            text_paint_facts = Some(crate::flowchart::FlowchartTextPaintFacts::from_visible(
                &facts,
            ));
        }
        let label_group_style = prepared_svg_label.as_ref().map_or_else(
            || {
                common
                    .label_emission
                    .final_style(ctx, &node_text_style, None)
            },
            |plan| {
                common
                    .label_emission
                    .final_style(ctx, &node_text_style, Some(plan))
            },
        );
        let _ = write!(
            out,
            r#"<g class="{}" style="{}" transform="translate({},{})"><rect/><g><rect class="background" style="stroke: none"/>"#,
            label_group_class,
            escape_xml_display(&label_group_style),
            fmt_display(label.dx),
            fmt_display(-metrics.height / 2.0 + label_dy)
        );
        let effect = ctx
            .label_effects
            .get()
            .and_then(|plan| plan.node(common.node_id));
        let effect_id = effect.map(|_| {
            format!(
                "{}-theme-effect-label",
                ctx.document_ids
                    .node(common.node_id)
                    .expect("prepared node id")
            )
        });
        if let (Some(effect), Some(id)) = (effect, effect_id.as_deref()) {
            label_effect_emitted =
                effect.open(out, id, (label.dx, -metrics.height / 2.0 + label_dy));
        }
        if label.label_type == "markdown" {
            write_flowchart_svg_text_markdown_wrapped_with_style(
                out,
                label.text,
                &label_group_style,
                ctx.measurer,
                &node_text_style,
                Some(ctx.wrapping_width),
            );
        } else {
            let typed_label_fill_style = common.label_emission.typed_label_fill_style();
            write_flowchart_svg_label_plan_with_style(
                out,
                prepared_svg_label
                    .as_ref()
                    .expect("non-Markdown SVG labels are prepared before emission"),
                true,
                typed_label_fill_style.as_deref(),
            );
        }
        if label_effect_emitted {
            effect
                .expect("opened effect")
                .close(out, ctx, effect_id.as_deref().unwrap());
        }
        out.push_str("</g></g>");
    } else {
        let prepared_math = ctx
            .svg_label_sidecar
            .zip(owner)
            .map_or(Default::default(), |(sidecar, owner)| {
                sidecar.prepared_math_for_terminal(owner, label.text)
            });
        let label_html = super::helpers::timed_node_label_html(common.timing, details, || {
            flowchart_label_html_with_prepared_math(
                label.text,
                label.label_type,
                ctx.config,
                ctx.math_renderer,
                prepared_math,
            )
        });
        if ctx.text_surface_paint.generic_text.requested() {
            text_paint_facts = Some(crate::flowchart::FlowchartTextPaintFacts::from_visible(
                &crate::text::VisibleTextStyleFacts::from_xhtml_fragment(label_html.as_ref()),
            ));
        }
        html_weight_verified = label_weight_is_verified(
            ctx,
            label,
            Some(label_html.as_ref()),
            if common.shape == "note" {
                &["label", "nodeLabel", "noteLabel"]
            } else {
                &["label", "nodeLabel"]
            },
            common.node_classes,
        );
        html_typography_statuses =
            crate::svg::parity::flowchart::style::sanitized_xhtml_typography_statuses(
                label_html.as_ref(),
            );
        let final_style = common
            .label_emission
            .final_style(ctx, &node_text_style, None);
        let span_style_attr = OptionalStyleXmlAttr(final_style.as_str());
        let is_math_html_label = ctx.node_wrap_mode == crate::text::WrapMode::HtmlLike
            && label.text.contains("$$")
            && (matches!(
                prepared_math,
                crate::flowchart::FlowchartPreparedMathResolution::Prepared(_)
            ) || ctx.math_renderer.is_some());

        let needs_wrap = if ctx.node_wrap_mode == crate::text::WrapMode::HtmlLike {
            if is_math_html_label {
                metrics.width >= ctx.wrapping_width - 0.01
            } else {
                let has_inline_style_tags =
                    ctx.node_html_labels && label.label_type != "markdown" && {
                        let lower = label_html.to_ascii_lowercase();
                        crate::text::flowchart_html_has_inline_style_tags(&lower)
                    };

                let raw = if label.label_type == "markdown" {
                    crate::text::measure_markdown_with_inline_styles(
                        ctx.measurer,
                        label.text,
                        &node_text_style,
                        None,
                        ctx.node_wrap_mode,
                    )
                    .width
                } else if has_inline_style_tags {
                    crate::text::measure_html_with_inline_styles(
                        ctx.measurer,
                        &label_html,
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

        let mut div_style =
            flowchart_label_div_style_prefix(common.label_emission.compiled_styles, true);
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
            r#"<g class="{}" style="{}" transform="translate({},{})"><rect/><foreignObject width="{}" height="{}"{}><div xmlns="http://www.w3.org/1999/xhtml" style="{}"><span class="{}"{}>{}</span></div></foreignObject></g>"#,
            label_group_class,
            escape_xml_display(&final_style),
            fmt_display(-metrics.width / 2.0 + label.dx),
            fmt_display(-metrics.height / 2.0 + label_dy),
            fmt_display(metrics.width),
            fmt_display(metrics.height),
            HTML_LABEL_FOREIGN_OBJECT_OVERFLOW_ATTR,
            escape_xml_display(&div_style),
            super::helpers::flowchart_node_label_span_class(label.label_type),
            span_style_attr,
            label_html
        );
    }
    if close_node_wrapper {
        out.push_str("</g>");
        if common.wrapped_in_a {
            out.push_str("</a>");
        }
    }
    let typography_applicable =
        flowchart_node_label_typography_is_applicable(&label_text_plain, ctx.node_html_labels);
    super::emission::FlowchartNodeLabelEmissionReceipt::verified()
        .with_effect_reach(typography_applicable, label_effect_emitted)
        .with_font_weight_reach(
            typography_applicable,
            html_weight_verified && label_weight_is_verified(ctx, label, None, &[], &[]),
        )
        .with_prepared_typography_reach(
            typography_applicable,
            prepared_svg_label.as_ref().is_some_and(
                crate::flowchart::FlowchartSvgLabelRenderPlan::emitted_admitted_typography,
            ),
        )
        .with_label_fill_reach(typography_applicable, true)
        .with_html_typography_statuses(html_typography_statuses.0, html_typography_statuses.1)
        .with_text_paint_facts(text_paint_facts)
}

fn append_typed_label_fill(style: &mut String, fill: &str) {
    if !style.is_empty() {
        style.push(';');
    }
    let _ = write!(
        style,
        "fill:{fill} !important;color:{} !important",
        if fill == "none" { "transparent" } else { fill }
    );
}

fn flowchart_node_label_typography_is_applicable(text: &str, html_labels: bool) -> bool {
    !crate::flowchart::flowchart_label_text_is_empty_for_mode(text, html_labels)
}

fn flowchart_node_label_uses_markdown_bbox(label_type: &str, text: &str) -> bool {
    // Mermaid 11.16's labelHelper passes `markdown: true` only for the parser-owned label type.
    label_type == "markdown" && crate::text::mermaid_markdown_to_lines(text, true).len() > 1
}

#[cfg(test)]
mod tests {
    use super::{
        flowchart_node_label_typography_is_applicable, flowchart_node_label_uses_markdown_bbox,
    };

    #[test]
    fn typography_applicability_uses_mermaid_visible_whitespace_semantics() {
        assert!(!flowchart_node_label_typography_is_applicable(
            " \t\n", false
        ));
        assert!(flowchart_node_label_typography_is_applicable(
            "\u{00a0}", false
        ));
        assert!(flowchart_node_label_typography_is_applicable(
            "\u{2007}", false
        ));
        assert!(flowchart_node_label_typography_is_applicable(
            "\u{202f}", false
        ));
    }

    #[test]
    fn markdown_bbox_selection_uses_the_parser_label_type() {
        assert!(!flowchart_node_label_uses_markdown_bbox(
            "text",
            "ordinary_name\nsecond_line"
        ));
        assert!(!flowchart_node_label_uses_markdown_bbox(
            "text",
            "*unfinished\nsecond"
        ));
        assert!(flowchart_node_label_uses_markdown_bbox(
            "markdown",
            "**first**\n_second_"
        ));
        assert!(!flowchart_node_label_uses_markdown_bbox(
            "markdown", "one line"
        ));
    }
}

fn label_weight_is_verified(
    ctx: &FlowchartRenderCtx<'_>,
    label: &super::FlowchartNodeLabelState<'_>,
    html: Option<&str>,
    shell_classes: &[&str],
    node_classes: &[String],
) -> bool {
    ctx.svg_label_sidecar.is_some_and(|sidecar| {
        let weights = sidecar.label_weights();
        weights.config_is_verified() && weights.get(ThemeTarget::NodeLabel).is_some()
    }) && label.label_type != "markdown"
        && !label.text.contains("$$")
        && html.is_none_or(|html| {
            let ancestor_classes = ["root", "nodes", "node", "default"]
                .into_iter()
                .chain(node_classes.iter().map(String::as_str));
            crate::svg::parity::flowchart::style::html_label_font_weight_status(
                ctx.class_defs,
                shell_classes,
                ancestor_classes,
                html,
            ) == crate::flowchart::FlowchartSourceFacetStatus::Absent
        })
}
