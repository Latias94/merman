//! Flowchart node label renderer.

use std::borrow::Cow;
use std::fmt::Write as _;

use crate::style::compiled::{FlowchartCompiledStyles, flowchart_label_div_style_prefix};
use crate::svg::label::{append_native_label_color, flowchart_label_html_with_prepared_label};
use crate::svg::types::{FlowchartRenderCtx, FlowchartRenderDetails};
use crate::svg::util::{HTML_LABEL_FOREIGN_OBJECT_OVERFLOW_ATTR, OptionalStyleAttr};
use crate::svg::{escape_attr_display, fmt_display};
use crate::svg::{
    write_flowchart_svg_label_plan_with_style, write_flowchart_svg_text_markdown_wrapped_with_style,
};

pub(super) struct FlowchartNodeLabelEmissionPlan<'a> {
    source_text_style: &'a merman_render_core::text::TextStyle,
    compiled_styles: &'a FlowchartCompiledStyles,
    native_label_color: Option<&'a str>,
}

impl<'a> FlowchartNodeLabelEmissionPlan<'a> {
    pub(super) const fn new(
        source_text_style: &'a merman_render_core::text::TextStyle,
        compiled_styles: &'a FlowchartCompiledStyles,
        native_label_color: Option<&'a str>,
    ) -> Self {
        Self {
            source_text_style,
            compiled_styles,
            native_label_color,
        }
    }

    fn native_label_color_style(&self) -> Option<String> {
        self.native_label_color.map(|fill| {
            let mut style = String::new();
            append_native_label_color(&mut style, fill);
            style
        })
    }

    pub(super) fn metrics(
        &self,
        ctx: &FlowchartRenderCtx<'_>,
        layout_node: Option<&merman_render_core::LayoutNode>,
        label: &super::FlowchartNodeLabelState<'_>,
    ) -> merman_render_core::text::TextMetrics {
        super::helpers::compute_node_label_metrics_with_style(
            ctx,
            layout_node,
            label.text,
            label.label_type,
            self.source_text_style,
        )
    }

    fn final_style(&self) -> Cow<'_, str> {
        let Some(fill) = self.native_label_color else {
            return Cow::Borrowed(&self.compiled_styles.label_style);
        };
        let mut style = self.compiled_styles.label_style.clone();
        append_native_label_color(&mut style, fill);
        Cow::Owned(style)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn write_special_html_label(
        &self,
        out: &mut impl crate::svg::SvgOutput,
        ctx: &FlowchartRenderCtx<'_>,
        common: &super::FlowchartNodeRenderCommon<'_>,
        label: &super::FlowchartNodeLabelState<'_>,
        details: &mut FlowchartRenderDetails,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    ) {
        let owner = ctx.svg_label_sidecar.and_then(|sidecar| {
            sidecar.node_owner(common.node_id, ctx.swimlane_direction.is_some())
        });
        let final_style = self.final_style();
        let span_style_attr = OptionalStyleAttr(final_style.trim());
        let prepared_label = ctx
            .svg_label_sidecar
            .zip(owner)
            .map_or(Default::default(), |(sidecar, owner)| {
                sidecar.prepared_label_for_terminal(owner, label.text)
            });
        let label_html = super::helpers::timed_node_label_html(common.timing, details, || {
            flowchart_label_html_with_prepared_label(
                label.text,
                label.label_type,
                ctx.config,
                ctx.math_renderer,
                prepared_label,
            )
        });
        let mut div_style = final_style.as_ref().to_owned();
        if !div_style.is_empty() {
            div_style.push(';');
        }
        div_style.push_str(&super::helpers::asset_label_div_style(ctx, width));
        let _ = write!(
            out,
            r#"<g class="label" style="{}" transform="translate({},{})"><rect/><foreignObject width="{}" height="{}"{}><div xmlns="http://www.w3.org/1999/xhtml" class="labelBkg" style="{}"><span class="{}"{}>{}</span></div></foreignObject></g>"#,
            escape_attr_display(&final_style),
            fmt_display(x),
            fmt_display(y),
            fmt_display(width),
            fmt_display(height),
            HTML_LABEL_FOREIGN_OBJECT_OVERFLOW_ATTR,
            escape_attr_display(&div_style),
            super::helpers::flowchart_node_label_span_class(label.label_type),
            span_style_attr,
            label_html,
        );
    }
}

pub(in crate::svg::render::node) fn render_flowchart_node_label(
    out: &mut impl crate::svg::SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    common: &super::FlowchartNodeRenderCommon<'_>,
    label: &super::FlowchartNodeLabelState<'_>,
    details: &mut FlowchartRenderDetails,
    div_style_scratch: &mut String,
) {
    render_flowchart_node_label_with_wrapper(
        out,
        ctx,
        common,
        label,
        details,
        div_style_scratch,
        true,
    )
}

pub(in crate::svg::render::node) fn render_flowchart_node_label_before_tail(
    out: &mut impl crate::svg::SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    common: &super::FlowchartNodeRenderCommon<'_>,
    label: &super::FlowchartNodeLabelState<'_>,
    details: &mut FlowchartRenderDetails,
    div_style_scratch: &mut String,
) {
    render_flowchart_node_label_with_wrapper(
        out,
        ctx,
        common,
        label,
        details,
        div_style_scratch,
        false,
    )
}

fn render_flowchart_node_label_with_wrapper(
    out: &mut impl crate::svg::SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    common: &super::FlowchartNodeRenderCommon<'_>,
    label: &super::FlowchartNodeLabelState<'_>,
    details: &mut FlowchartRenderDetails,
    div_style_scratch: &mut String,
    close_node_wrapper: bool,
) {
    let node_text_style = common.label_emission.source_text_style;
    let wrapping_width = crate::bpmn::wrapping_width(Some(common.shape), ctx.wrapping_width);
    let owner = ctx
        .svg_label_sidecar
        .and_then(|sidecar| sidecar.node_owner(common.node_id, ctx.swimlane_direction.is_some()));
    let prepared_svg_label = (!ctx.node_html_labels && label.label_type != "markdown").then(|| {
        crate::svg_label::FlowchartSvgLabelRenderPlan::new(
            ctx.svg_label_sidecar,
            owner,
            label.text,
            ctx.measurer,
            node_text_style,
            Some(wrapping_width),
            true,
            crate::node_geometry::flowchart_node_svg_width_mode(
                label.text,
                label.label_type,
                ctx.node_wrap_mode,
                common.shape,
            ),
        )
    });
    let mut label_dy = label.dy;
    if !ctx.node_html_labels && crate::bpmn::BpmnShape::resolve(common.shape).is_some() {
        label_dy -= ctx
            .measurer
            .measure_svg_create_text_bbox_y_offset_px(label.text, node_text_style);
    } else if !ctx.node_html_labels
        && flowchart_node_label_uses_markdown_bbox(label.label_type, label.text)
        && matches!(
            common.shape,
            "doc"
                | "document"
                | "lin-doc"
                | "lined-document"
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
                | "st-rect"
                | "procs"
                | "processes"
                | "stacked-rectangle"
                | "lin-rect"
                | "lined-rectangle"
                | "lined-process"
                | "lin-proc"
                | "shaded-process"
                | "brace"
                | "brace-l"
                | "comment"
                | "brace-r"
                | "braces"
        )
    {
        // Mermaid shape renderers override `labelHelper(...)`'s default centering using
        // `-bbox.y`. Chromium reports these wrapped SVG markdown labels with a small positive
        // `getBBox().y`, so model that render-time offset here instead of baking literal `-1`s
        // into individual shapes.
        label_dy -= ctx
            .measurer
            .measure_svg_create_text_bbox_y_offset_px(label.text, node_text_style);
    }
    let metrics = super::helpers::compute_node_label_metrics(
        ctx,
        common.node_id,
        Some(common.layout_node),
        label.text,
        label.label_type,
        common.node_classes,
        common.node_styles,
    );
    // Only authored FlowDB nodes carry minWidth; subgraph titles retain their own sizing.
    let min_width = ctx
        .nodes_by_id
        .get(common.node_id)
        .filter(|_| !ctx.subgraphs_by_id.contains_key(common.node_id))
        .map_or(0.0, |node| {
            crate::node_geometry::flowchart_node_label_min_width(
                label.text,
                node.layout_shape.as_deref(),
                ctx.config,
            )
        });
    let label_group_class = if common.shape == "note" {
        "label noteLabel"
    } else {
        "label"
    };
    if !ctx.node_html_labels {
        let label_group_style = common.label_emission.final_style();
        let _ = write!(
            out,
            r#"<g class="{}" style="{}" transform="translate({},{})"><rect/><g><rect class="background" style="stroke: none"/>"#,
            label_group_class,
            escape_attr_display(&label_group_style),
            fmt_display(label.dx),
            fmt_display(-metrics.height / 2.0 + label_dy)
        );
        if label.label_type == "markdown" {
            write_flowchart_svg_text_markdown_wrapped_with_style(
                out,
                label.text,
                &label_group_style,
                ctx.measurer,
                node_text_style,
                Some(wrapping_width),
            );
        } else {
            let native_label_color_style = common.label_emission.native_label_color_style();
            write_flowchart_svg_label_plan_with_style(
                out,
                prepared_svg_label
                    .as_ref()
                    .expect("non-Markdown SVG labels are prepared before emission"),
                true,
                native_label_color_style.as_deref(),
            );
        }
        out.push_str("</g></g>");
    } else {
        let prepared_label = ctx
            .svg_label_sidecar
            .zip(owner)
            .map_or(Default::default(), |(sidecar, owner)| {
                sidecar.prepared_label_for_terminal(owner, label.text)
            });
        let label_html = super::helpers::timed_node_label_html(common.timing, details, || {
            flowchart_label_html_with_prepared_label(
                label.text,
                label.label_type,
                ctx.config,
                ctx.math_renderer,
                prepared_label,
            )
        });
        let final_style = common.label_emission.final_style();
        let span_style_attr = OptionalStyleAttr(final_style.trim());
        let is_math_html_label = ctx.node_wrap_mode == merman_render_core::text::WrapMode::HtmlLike
            && label.text.contains("$$")
            && (matches!(
                prepared_label,
                crate::svg_label::FlowchartPreparedLabelResolution::PreparedMath(_)
            ) || ctx.math_renderer.is_some());
        let label_box = (ctx.node_wrap_mode == merman_render_core::text::WrapMode::HtmlLike
            && !is_math_html_label)
            .then(|| {
                merman_render_core::text::html_label_layout::flowchart_html_label_box(
                    ctx.measurer,
                    &label_html,
                )
            })
            .flatten();

        let mut label_below_minimum = false;
        let needs_wrap = if ctx.node_wrap_mode == merman_render_core::text::WrapMode::HtmlLike {
            if is_math_html_label {
                let natural = if min_width > 0.0 {
                    crate::svg::label::flowchart_label_metrics_for_layout(
                        merman_render_core::text::create_text_html::FlowchartLabelMetricsRequest {
                            measurer: ctx.measurer,
                            raw_label: label.text,
                            label_type: label.label_type,
                            style: node_text_style,
                            max_width_px: Some(wrapping_width),
                            wrap_mode: ctx.node_wrap_mode,
                            config: ctx.config,
                            math_renderer: ctx.math_renderer,
                        },
                    )
                } else {
                    metrics
                };
                label_below_minimum = natural.width < min_width;
                natural.width >= wrapping_width - 0.01
            } else if let Some(width) =
                label_box.and_then(|dimensions| dimensions.width_in_label(Some(wrapping_width)))
            {
                label_below_minimum = width.outer < min_width;
                width.wrap.is_some()
            } else {
                let raw = crate::svg::label::flowchart_html_label_metrics_with_geometry(
                    merman_render_core::text::create_text_html::FlowchartLabelMetricsRequest {
                        measurer: ctx.measurer,
                        raw_label: label.text,
                        label_type: label.label_type,
                        style: node_text_style,
                        max_width_px: None,
                        wrap_mode: ctx.node_wrap_mode,
                        config: ctx.config,
                        math_renderer: None,
                    },
                    &label_html,
                    common.label_emission.compiled_styles.html_label_geometry(),
                )
                .width;
                label_below_minimum = raw < min_width;
                raw > wrapping_width
            }
        } else {
            false
        };

        let compiled_styles = common.label_emission.compiled_styles;
        let mut authored_div_style;
        let div_style = if !compiled_styles.has_label_div_declarations() {
            div_style_scratch.clear();
            div_style_scratch
        } else {
            authored_div_style = flowchart_label_div_style_prefix(compiled_styles, true);
            &mut authored_div_style
        };
        if needs_wrap {
            let _ = write!(
                div_style,
                "display: table; white-space: break-spaces; line-height: 1.5; max-width: {mw}px; text-align: center; width: {mw}px;",
                mw = fmt_display(wrapping_width)
            );
        } else if label_below_minimum {
            let _ = write!(
                div_style,
                "display: table; white-space: nowrap; line-height: 1.5; max-width: {mw}px; text-align: center; width: {min}px;",
                mw = fmt_display(wrapping_width),
                min = fmt_display(min_width),
            );
        } else {
            let _ = write!(
                div_style,
                "display: table-cell; white-space: nowrap; line-height: 1.5; max-width: {mw}px; text-align: center;",
                mw = fmt_display(wrapping_width)
            );
        }
        let _ = write!(
            out,
            r#"<g class="{}" style="{}" transform="translate({},{})"><rect/>"#,
            label_group_class,
            escape_attr_display(&final_style),
            fmt_display(-metrics.width / 2.0 + label.dx),
            fmt_display(-metrics.height / 2.0 + label_dy),
        );
        let _ = write!(
            out,
            r#"<foreignObject width="{}" height="{}"{}><div xmlns="http://www.w3.org/1999/xhtml" style="{}"><span class="{}"{}>{}</span></div></foreignObject>"#,
            fmt_display(metrics.width),
            fmt_display(metrics.height),
            if label_box.is_some() {
                ""
            } else {
                HTML_LABEL_FOREIGN_OBJECT_OVERFLOW_ATTR
            },
            escape_attr_display(div_style),
            super::helpers::flowchart_node_label_span_class(label.label_type),
            span_style_attr,
            label_html
        );
        out.push_str("</g>");
    }
    if close_node_wrapper {
        out.push_str("</g>");
        if common.wrapped_in_a {
            out.push_str("</a>");
        }
    }
}

fn flowchart_node_label_uses_markdown_bbox(label_type: &str, text: &str) -> bool {
    // Mermaid 11.16's labelHelper passes `markdown: true` only for the parser-owned label type.
    label_type == "markdown"
        && merman_render_core::text::mermaid_markdown_to_lines(text, true).len() > 1
}

#[cfg(test)]
mod tests {
    use super::flowchart_node_label_uses_markdown_bbox;

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
