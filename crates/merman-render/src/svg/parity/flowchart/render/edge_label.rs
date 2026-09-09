//! Flowchart edge label renderer.

use super::super::*;
use crate::svg::parity::flowchart::util::HTML_LABEL_FOREIGN_OBJECT_OVERFLOW_ATTR;
use std::borrow::Cow;

type FlowchartEdgeLabelEmissionReceipt =
    crate::svg::parity::flowchart::render::node::emission::FlowchartNodeLabelEmissionReceipt;

fn edge_label_emission_receipt(
    source_typography_verified: bool,
    typography_applicable: bool,
    prepared: Option<&crate::flowchart::FlowchartSvgLabelRenderPlan<'_>>,
) -> FlowchartEdgeLabelEmissionReceipt {
    let receipt = if source_typography_verified {
        FlowchartEdgeLabelEmissionReceipt::verified()
    } else {
        FlowchartEdgeLabelEmissionReceipt::unverified()
    };
    receipt.with_prepared_typography_reach(
        typography_applicable,
        prepared.is_some_and(
            crate::flowchart::FlowchartSvgLabelRenderPlan::emitted_admitted_typography,
        ),
    )
}

#[allow(clippy::too_many_arguments)]
fn record_edge_label_emission(
    ctx: &FlowchartRenderCtx<'_>,
    edge_id: &str,
    compiled_styles: Option<&FlowchartCompiledStyles>,
    typed_font_stack_selected: bool,
    typed_font_size_selected: bool,
    padding_verified: bool,
    receipt: FlowchartEdgeLabelEmissionReceipt,
    sanitized_xhtml: Option<&str>,
) {
    let html_typography_statuses = sanitized_xhtml.map_or(
        (
            crate::flowchart::FlowchartSourceFacetStatus::Absent,
            crate::flowchart::FlowchartSourceFacetStatus::Absent,
        ),
        crate::svg::parity::flowchart::style::sanitized_xhtml_typography_statuses,
    );
    ctx.record_base_typography_label_emission(
        crate::flowchart::FlowchartBaseTypographyLabelEmission::new(
            ctx.svg_label_sidecar
                .and_then(|sidecar| sidecar.edge_owner(edge_id, ctx.swimlane_direction.is_some())),
            receipt.typography_applicable(),
            receipt.typography_verified(),
        )
        .with_source_facets(
            compiled_styles
                .map_or(
                    crate::flowchart::FlowchartSourceFacetStatus::Absent,
                    FlowchartCompiledStyles::source_font_stack_status,
                )
                .merge(html_typography_statuses.0),
            compiled_styles
                .map_or(
                    crate::flowchart::FlowchartSourceFacetStatus::Absent,
                    FlowchartCompiledStyles::source_font_size_status,
                )
                .merge(html_typography_statuses.1),
        )
        .with_target_selection(typed_font_stack_selected, typed_font_size_selected),
    );
    let source_residuals = compiled_styles.map_or_else(Vec::new, |compiled_styles| {
        sanitized_xhtml.map_or_else(
            || compiled_styles.label_source_residuals(edge_id, receipt),
            |sanitized_xhtml| {
                compiled_styles.emitted_html_label_source_residuals(
                    edge_id,
                    receipt.typography_verified(),
                    sanitized_xhtml,
                )
            },
        )
    });
    let prepared_reach = receipt.prepared_typography_reach();
    let font_stack = prepared_reach.map(|reach| {
        crate::flowchart::FlowchartThemeFacetEmission::new(
            crate::flowchart::FlowchartFacetPrecedence::new(
                compiled_styles.map_or(
                    crate::flowchart::FlowchartSourceFacetStatus::Absent,
                    |compiled_styles| compiled_styles.emitted_source_font_stack_status(receipt),
                ),
                ctx.node_typography_config_ownership.font_stack,
            ),
            typed_font_stack_selected && reach.is_verified(),
        )
    });
    let font_size = prepared_reach.map(|reach| {
        crate::flowchart::FlowchartThemeFacetEmission::new(
            crate::flowchart::FlowchartFacetPrecedence::new(
                compiled_styles.map_or(
                    crate::flowchart::FlowchartSourceFacetStatus::Absent,
                    |compiled_styles| compiled_styles.emitted_source_font_size_status(receipt),
                ),
                ctx.node_typography_config_ownership.font_size,
            ),
            typed_font_size_selected && reach.is_verified(),
        )
    });
    let padding_precedence = crate::flowchart::FlowchartFacetPrecedence::new(
        crate::flowchart::FlowchartSourceFacetStatus::Absent,
        false,
    );
    let padding = ctx
        .edge_theme
        .padding_selected(padding_precedence)
        .then_some(crate::flowchart::FlowchartThemeFacetEmission::new(
            padding_precedence,
            padding_verified,
        ));
    ctx.theme_evidence.record_edge_label_emission(
        ctx.edge_theme,
        crate::flowchart::FlowchartEdgeLabelThemeEmission {
            font_stack,
            font_size,
            padding,
        },
        &source_residuals,
    );
}

fn padded_html_edge_label_background(
    padding: crate::flowchart::FlowchartEdgeLabelPadding,
    width: f64,
    height: f64,
) -> String {
    if padding.is_zero() || width <= 0.0 || height <= 0.0 {
        return String::new();
    }

    format!(
        r#"<rect class="background" x="{}" y="{}" width="{}" height="{}"/>"#,
        fmt_display(-width / 2.0),
        fmt_display(-height / 2.0),
        fmt_display(width),
        fmt_display(height),
    )
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct SvgEdgeLabelBox {
    translate_x: f64,
    translate_y: f64,
    background_x: f64,
    background_y: f64,
    background_width: f64,
    background_height: f64,
}

fn centered_svg_edge_label_box(
    layout_width: f64,
    layout_height: f64,
    text_bbox_y: f64,
) -> SvgEdgeLabelBox {
    const BACKGROUND_PADDING_PX: f64 = 2.0;

    let background_width = layout_width.max(0.0);
    let background_height = layout_height.max(0.0);
    let text_height = (background_height - 2.0 * BACKGROUND_PADDING_PX).max(0.0);
    SvgEdgeLabelBox {
        // Mermaid emits centered SVG text (`text-anchor="middle"`) and centers the text bbox,
        // not the padded label rectangle. A symmetric deterministic bbox therefore needs no
        // horizontal translation.
        translate_x: 0.0,
        translate_y: -(text_bbox_y + text_height / 2.0),
        background_x: -background_width / 2.0,
        background_y: text_bbox_y - BACKGROUND_PADDING_PX,
        background_width,
        background_height,
    }
}

fn flowchart_svg_edge_label_box(
    layout_width: f64,
    layout_height: f64,
    text_bbox_y: f64,
    padding: crate::flowchart::FlowchartEdgeLabelPadding,
) -> SvgEdgeLabelBox {
    if padding.is_zero() {
        return centered_svg_edge_label_box(layout_width, layout_height, text_bbox_y);
    }

    let layout_width = layout_width.max(0.0);
    let layout_height = layout_height.max(0.0);
    let content = padding.content_box(layout_width, layout_height);
    SvgEdgeLabelBox {
        translate_x: content.x,
        translate_y: content.y,
        background_x: -2.0 - padding.left(),
        background_y: text_bbox_y - 2.0 - padding.top(),
        background_width: layout_width,
        background_height: layout_height,
    }
}

fn position_flowchart_edge_label(
    dagre_anchor: crate::model::LayoutPoint,
    geom: &FlowchartEdgePathGeom,
    always_recompute: bool,
) -> crate::model::LayoutPoint {
    let rendered_d = geom
        .emitted_d_for_label
        .as_deref()
        .unwrap_or(geom.d.as_str());
    crate::svg::parity::edge_label_geometry::position_edge_label(
        dagre_anchor,
        &geom.label_path_points,
        rendered_d,
        geom.label_path_was_explicitly_updated || always_recompute,
    )
}

fn resolve_flowchart_edge_label_position(
    ctx: &FlowchartRenderCtx<'_>,
    key: crate::flowchart::FlowchartEdgeKey,
    layout_edge: &crate::model::LayoutEdge,
    label: &crate::model::LayoutLabel,
    origin_x: f64,
    origin_y: f64,
    edge_cache: &FxHashMap<crate::flowchart::FlowchartEdgeKey, FlowchartEdgePathCacheEntry>,
    always_recompute: bool,
) -> crate::model::LayoutPoint {
    let dagre_anchor = crate::model::LayoutPoint {
        x: label.x + ctx.tx - origin_x,
        y: label.y + ctx.ty - origin_y,
    };

    if let Some(geom) = edge_cache
        .get(&key)
        .filter(|entry| {
            (entry.origin_x - origin_x).abs() <= 1e-9 && (entry.origin_y - origin_y).abs() <= 1e-9
        })
        .map(|entry| &entry.geom)
    {
        return position_flowchart_edge_label(dagre_anchor, geom, always_recompute);
    }

    // Geometry caching only skips routes with fewer than two points. For that degenerate case,
    // `calcLabelPosition` either keeps the anchor (empty) or returns the sole waypoint.
    if always_recompute || layout_edge.to_cluster.is_some() || layout_edge.from_cluster.is_some() {
        let points = layout_edge
            .points
            .iter()
            .map(|point| crate::model::LayoutPoint {
                x: point.x + ctx.tx - origin_x,
                y: point.y + ctx.ty - origin_y,
            })
            .collect::<Vec<_>>();
        return crate::svg::parity::edge_label_geometry::position_edge_label(
            dagre_anchor.clone(),
            &points,
            "",
            true,
        );
    }

    dagre_anchor
}

pub(in crate::svg::parity) fn render_flowchart_edge_label(
    out: &mut impl crate::svg::parity::SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    edge_ref: super::super::render_input::FlowchartRenderEdgeRef<'_>,
    origin_x: f64,
    origin_y: f64,
    edge_cache: &FxHashMap<crate::flowchart::FlowchartEdgeKey, FlowchartEdgePathCacheEntry>,
) -> crate::Result<()> {
    let key = edge_ref.key;
    let edge = edge_ref.edge;
    let label_text = ctx
        .model
        .edge_label_for_render(key.semantic_index(), edge)
        .unwrap_or_default();
    let label_type = edge.label_type.as_deref().unwrap_or("text");
    let compiled_label_styles = ctx.edge_style_plan.edge_for(key)?;
    let font_stack_precedence = crate::flowchart::FlowchartFacetPrecedence::new(
        compiled_label_styles.source_font_stack_status(),
        ctx.node_typography_config_ownership.font_stack,
    );
    let font_size_precedence = crate::flowchart::FlowchartFacetPrecedence::new(
        compiled_label_styles.source_font_size_status(),
        ctx.node_typography_config_ownership.font_size,
    );
    let typed_font_stack_selected = ctx.edge_theme.font_stack_selected(font_stack_precedence);
    let typed_font_size_selected = ctx.edge_theme.font_size_selected(font_size_precedence);
    let edge_metrics_style = compiled_label_styles.effective_edge_label_text_style(&ctx.text_style);
    let svg_edge_label_text_style: Cow<'_, str> =
        if compiled_label_styles.label_style.contains("color:") {
            Cow::Owned(compiled_label_styles.label_style.replace("color:", "fill:"))
        } else {
            Cow::Borrowed(compiled_label_styles.label_style.as_str())
        };
    let owner = Some(crate::flowchart::FlowchartSvgLabelOwner::Edge(
        key.semantic_index(),
    ));
    let prepared_svg_label = (!ctx.edge_html_labels && label_type != "markdown").then(|| {
        crate::flowchart::FlowchartSvgLabelRenderPlan::new_with_metrics_style(
            ctx.svg_label_sidecar,
            owner,
            label_text,
            ctx.measurer,
            &ctx.text_style,
            edge_metrics_style.as_ref(),
            Some(FLOWCHART_EDGE_LABEL_WRAP_WIDTH),
            true,
            crate::flowchart::FlowchartSvgWidthMode::Bbox,
        )
    });
    let label_text_plain: Cow<'_, str> = prepared_svg_label.as_ref().map_or_else(
        || {
            Cow::Owned(flowchart_label_plain_text(
                label_text,
                label_type,
                ctx.edge_html_labels,
            ))
        },
        |prepared| Cow::Borrowed(prepared.plain_text()),
    );
    let span_style_attr = OptionalStyleXmlAttr(compiled_label_styles.label_style.as_str());
    let div_style_prefix = crate::svg::parity::flowchart::style::flowchart_label_div_style_prefix(
        compiled_label_styles,
        false,
    );
    let typography_applicable = !crate::flowchart::flowchart_label_text_is_empty_for_mode(
        &label_text_plain,
        ctx.edge_html_labels,
    );
    let record_label_emission = |source_typography_verified, sanitized_xhtml: Option<&str>| {
        let receipt = edge_label_emission_receipt(
            source_typography_verified,
            typography_applicable,
            prepared_svg_label.as_ref(),
        );
        record_edge_label_emission(
            ctx,
            edge.id.as_str(),
            Some(compiled_label_styles),
            typed_font_stack_selected,
            typed_font_size_selected,
            typography_applicable,
            receipt,
            sanitized_xhtml,
        );
    };

    fn fallback_midpoint(
        le: &crate::model::LayoutEdge,
        ctx: &FlowchartRenderCtx<'_>,
        origin_x: f64,
        origin_y: f64,
    ) -> (f64, f64) {
        let anchor = crate::model::LayoutPoint {
            x: ctx.tx - origin_x,
            y: ctx.ty - origin_y,
        };
        let points = le
            .points
            .iter()
            .map(|point| crate::model::LayoutPoint {
                x: point.x + ctx.tx - origin_x,
                y: point.y + ctx.ty - origin_y,
            })
            .collect::<Vec<_>>();
        let position =
            crate::svg::parity::edge_label_geometry::position_edge_label(anchor, &points, "", true);
        (position.x, position.y)
    }

    if !ctx.edge_html_labels {
        if let Some(le) = ctx.layout_edges_by_key.get(&key) {
            if let Some(lbl) = le.label.as_ref() {
                let position = resolve_flowchart_edge_label_position(
                    ctx, key, le, lbl, origin_x, origin_y, edge_cache, false,
                );
                let x = position.x;
                let y = position.y;

                if crate::flowchart::flowchart_label_text_is_empty_for_mode(
                    &label_text_plain,
                    false,
                ) {
                    if !label_text.is_empty() {
                        let (layout_w, layout_h) = ctx.edge_label_padding.padded_size(4.0, 4.0);
                        let label_box = flowchart_svg_edge_label_box(
                            layout_w,
                            layout_h,
                            ctx.measurer.measure_svg_create_text_bbox_y_offset_px(
                                &label_text_plain,
                                edge_metrics_style.as_ref(),
                            ),
                            ctx.edge_label_padding,
                        );
                        let _ = write!(
                            out,
                            r#"<g class="edgeLabel" transform="translate({},{})"><g class="label" data-id="{}" transform="translate({},{})"><g><rect class="background" style="" x="{}" y="{}" width="{}" height="{}"/>"#,
                            fmt_display(x),
                            fmt_display(y),
                            escape_xml_display(&edge.id),
                            fmt_display(label_box.translate_x),
                            fmt_display(label_box.translate_y),
                            fmt_display(label_box.background_x),
                            fmt_display(label_box.background_y),
                            fmt_display(label_box.background_width),
                            fmt_display(label_box.background_height),
                        );
                        if label_type == "markdown" {
                            write_flowchart_svg_text_markdown_wrapped_centered(
                                out,
                                label_text,
                                true,
                                ctx.measurer,
                                &ctx.text_style,
                                Some(FLOWCHART_EDGE_LABEL_WRAP_WIDTH),
                            );
                        } else {
                            write_flowchart_svg_label_plan_centered_with_style(
                                out,
                                prepared_svg_label.as_ref().expect(
                                    "non-Markdown SVG edge labels are prepared before emission",
                                ),
                                svg_edge_label_text_style.as_ref(),
                            );
                        }
                        out.push_str("</g></g></g>");
                        record_label_emission(label_type != "markdown", None);
                        return Ok(());
                    }
                } else {
                    let label_box = flowchart_svg_edge_label_box(
                        lbl.width,
                        lbl.height,
                        ctx.measurer.measure_svg_create_text_bbox_y_offset_px(
                            &label_text_plain,
                            edge_metrics_style.as_ref(),
                        ),
                        ctx.edge_label_padding,
                    );
                    let _ = write!(
                        out,
                        r#"<g class="edgeLabel" transform="translate({},{})"><g class="label" data-id="{}" transform="translate({},{})"><g><rect class="background" style="" x="{}" y="{}" width="{}" height="{}"/>"#,
                        fmt_display(x),
                        fmt_display(y),
                        escape_xml_display(&edge.id),
                        fmt_display(label_box.translate_x),
                        fmt_display(label_box.translate_y),
                        fmt_display(label_box.background_x),
                        fmt_display(label_box.background_y),
                        fmt_display(label_box.background_width),
                        fmt_display(label_box.background_height)
                    );
                    if label_type == "markdown" {
                        write_flowchart_svg_text_markdown_wrapped_centered(
                            out,
                            label_text,
                            true,
                            ctx.measurer,
                            &ctx.text_style,
                            Some(FLOWCHART_EDGE_LABEL_WRAP_WIDTH),
                        );
                    } else {
                        write_flowchart_svg_label_plan_centered_with_style(
                            out,
                            prepared_svg_label.as_ref().expect(
                                "non-Markdown SVG edge labels are prepared before emission",
                            ),
                            svg_edge_label_text_style.as_ref(),
                        );
                    }
                    out.push_str("</g></g></g>");
                    record_label_emission(label_type != "markdown", None);
                    return Ok(());
                }
            }

            if !crate::flowchart::flowchart_label_text_is_empty_for_mode(&label_text_plain, false) {
                let (x, y) = fallback_midpoint(le, ctx, origin_x, origin_y);
                let metrics = ctx.measurer.measure_wrapped(
                    &label_text_plain,
                    &ctx.text_style,
                    Some(FLOWCHART_EDGE_LABEL_WRAP_WIDTH),
                    crate::text::WrapMode::SvgLike,
                );
                let content_w = (metrics.width + 4.0).max(1.0);
                let content_h = (metrics.height + 4.0).max(1.0);
                let (w, h) = ctx.edge_label_padding.padded_size(content_w, content_h);
                let label_box = flowchart_svg_edge_label_box(
                    w,
                    h,
                    ctx.measurer.measure_svg_create_text_bbox_y_offset_px(
                        &label_text_plain,
                        edge_metrics_style.as_ref(),
                    ),
                    ctx.edge_label_padding,
                );
                let _ = write!(
                    out,
                    r#"<g class="edgeLabel" transform="translate({},{})"><g class="label" data-id="{}" transform="translate({},{})"><g><rect class="background" style="" x="{}" y="{}" width="{}" height="{}"/>"#,
                    fmt_display(x),
                    fmt_display(y),
                    escape_xml_display(&edge.id),
                    fmt_display(label_box.translate_x),
                    fmt_display(label_box.translate_y),
                    fmt_display(label_box.background_x),
                    fmt_display(label_box.background_y),
                    fmt_display(label_box.background_width),
                    fmt_display(label_box.background_height)
                );
                if label_type == "markdown" {
                    write_flowchart_svg_text_markdown_wrapped_centered(
                        out,
                        label_text,
                        true,
                        ctx.measurer,
                        &ctx.text_style,
                        Some(FLOWCHART_EDGE_LABEL_WRAP_WIDTH),
                    );
                } else {
                    write_flowchart_svg_label_plan_centered_with_style(
                        out,
                        prepared_svg_label
                            .as_ref()
                            .expect("non-Markdown SVG edge labels are prepared before emission"),
                        svg_edge_label_text_style.as_ref(),
                    );
                }
                out.push_str("</g></g></g>");
                record_label_emission(label_type != "markdown", None);
                return Ok(());
            }
        }

        let _ = write!(
            out,
            r#"<g class="edgeLabel"><g class="label" data-id="{}" transform="translate(0,0)">"#,
            escape_xml_display(&edge.id)
        );
        write_flowchart_empty_svg_text_centered(out, false);
        out.push_str("</g></g>");
        record_label_emission(false, None);
        return Ok(());
    }

    let label_html = if crate::flowchart::flowchart_label_is_empty_for_render(label_text) {
        std::borrow::Cow::Borrowed("")
    } else {
        let prepared_math = ctx
            .svg_label_sidecar
            .zip(owner)
            .map_or(Default::default(), |(sidecar, owner)| {
                sidecar.prepared_math_for_terminal(owner, label_text)
            });
        flowchart_label_html_with_prepared_math(
            label_text,
            label_type,
            ctx.config,
            ctx.math_renderer,
            prepared_math,
        )
    };

    if let Some(le) = ctx.layout_edges_by_key.get(&key) {
        if let Some(lbl) = le.label.as_ref() {
            let position = resolve_flowchart_edge_label_position(
                ctx, key, le, lbl, origin_x, origin_y, edge_cache, false,
            );
            let x = position.x;
            let y = position.y;

            let layout_w = lbl.width.max(0.0);
            let h = lbl.height.max(0.0);
            let content = ctx.edge_label_padding.content_box(layout_w, h);
            let background = padded_html_edge_label_background(ctx.edge_label_padding, layout_w, h);
            let wrapped_style = if content.width >= FLOWCHART_EDGE_LABEL_WRAP_WIDTH - 0.01 {
                format!(
                    "display: table; white-space: break-spaces; line-height: 1.5; max-width: {mw}px; text-align: center; width: {mw}px;",
                    mw = fmt_display(FLOWCHART_EDGE_LABEL_WRAP_WIDTH)
                )
            } else {
                "display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 200px; text-align: center;".to_string()
            };
            let div_style = if div_style_prefix.is_empty() {
                wrapped_style
            } else {
                format!("{div_style_prefix}{wrapped_style}")
            };
            let _ = write!(
                out,
                r#"<g class="edgeLabel" transform="translate({},{})">{}<g class="label" data-id="{}" transform="translate({},{})"><foreignObject width="{}" height="{}"{}><div xmlns="http://www.w3.org/1999/xhtml" class="labelBkg" style="{}"><span class="edgeLabel"{}>{}</span></div></foreignObject></g></g>"#,
                fmt_display(x),
                fmt_display(y),
                background,
                escape_xml_display(&edge.id),
                fmt_display(content.x),
                fmt_display(content.y),
                fmt_display(content.width),
                fmt_display(content.height),
                HTML_LABEL_FOREIGN_OBJECT_OVERFLOW_ATTR,
                escape_xml_display(&div_style),
                span_style_attr,
                label_html
            );
            record_label_emission(true, Some(&label_html));
            return Ok(());
        }

        if !crate::flowchart::flowchart_label_text_is_empty_for_mode(
            &label_text_plain,
            ctx.edge_html_labels,
        ) {
            let (x, y) = fallback_midpoint(le, ctx, origin_x, origin_y);
            let has_inline_style_tags = if label_type == "markdown" {
                false
            } else {
                let lower = label_text.to_ascii_lowercase();
                crate::text::flowchart_html_has_inline_style_tags(&lower)
            };

            let metrics = if label_type == "markdown" {
                crate::text::measure_markdown_with_inline_styles(
                    ctx.measurer,
                    label_text,
                    &ctx.text_style,
                    Some(FLOWCHART_EDGE_LABEL_WRAP_WIDTH),
                    ctx.edge_wrap_mode,
                )
            } else if has_inline_style_tags {
                crate::text::measure_html_with_inline_styles(
                    ctx.measurer,
                    label_text,
                    &ctx.text_style,
                    Some(FLOWCHART_EDGE_LABEL_WRAP_WIDTH),
                    ctx.edge_wrap_mode,
                )
            } else {
                ctx.measurer.measure_wrapped(
                    &label_text_plain,
                    &ctx.text_style,
                    Some(FLOWCHART_EDGE_LABEL_WRAP_WIDTH),
                    ctx.edge_wrap_mode,
                )
            };
            let content_w = metrics.width.max(1.0);
            let content_h = metrics.height.max(1.0);
            let (layout_w, h) = ctx.edge_label_padding.padded_size(content_w, content_h);
            let content = ctx.edge_label_padding.content_box(layout_w, h);
            let background = padded_html_edge_label_background(ctx.edge_label_padding, layout_w, h);
            let wrapped_style = if content.width >= FLOWCHART_EDGE_LABEL_WRAP_WIDTH - 0.01 {
                format!(
                    "display: table; white-space: break-spaces; line-height: 1.5; max-width: {mw}px; text-align: center; width: {mw}px;",
                    mw = fmt_display(FLOWCHART_EDGE_LABEL_WRAP_WIDTH)
                )
            } else {
                "display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 200px; text-align: center;".to_string()
            };
            let div_style = if div_style_prefix.is_empty() {
                wrapped_style
            } else {
                format!("{div_style_prefix}{wrapped_style}")
            };
            let _ = write!(
                out,
                r#"<g class="edgeLabel" transform="translate({},{})">{}<g class="label" data-id="{}" transform="translate({},{})"><foreignObject width="{}" height="{}"{}><div xmlns="http://www.w3.org/1999/xhtml" class="labelBkg" style="{}"><span class="edgeLabel"{}>{}</span></div></foreignObject></g></g>"#,
                fmt_display(x),
                fmt_display(y),
                background,
                escape_xml_display(&edge.id),
                fmt_display(content.x),
                fmt_display(content.y),
                fmt_display(content.width),
                fmt_display(content.height),
                HTML_LABEL_FOREIGN_OBJECT_OVERFLOW_ATTR,
                escape_xml_display(&div_style),
                span_style_attr,
                label_html
            );
            record_label_emission(true, Some(&label_html));
            return Ok(());
        }
    }

    let _ = write!(
        out,
        r#"<g class="edgeLabel"><g class="label" data-id="{}" transform="translate(0,0)"><foreignObject width="0" height="0"><div xmlns="http://www.w3.org/1999/xhtml" class="labelBkg" style="{}display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 200px; text-align: center;"><span class="edgeLabel"{}></span></div></foreignObject></g></g>"#,
        escape_xml_display(&edge.id),
        escape_xml_display(&div_style_prefix),
        span_style_attr
    );
    record_label_emission(true, None);
    Ok(())
}

pub(in crate::svg::parity::flowchart) fn render_swimlane_edge_label_node(
    out: &mut impl crate::svg::parity::SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    node_id: &str,
    edge_ref: super::super::render_input::FlowchartRenderEdgeRef<'_>,
    origin_x: f64,
    origin_y: f64,
    edge_cache: &FxHashMap<crate::flowchart::FlowchartEdgeKey, FlowchartEdgePathCacheEntry>,
) -> crate::Result<()> {
    let key = edge_ref.key;
    let edge = edge_ref.edge;
    let label_text = ctx
        .model
        .edge_label_for_render(key.semantic_index(), edge)
        .unwrap_or_default();
    // Mermaid's Swimlane adapter creates a fresh `labelRect` without copying `edge.labelType`.
    // The node renderer therefore follows ordinary non-Markdown createText semantics.
    let label_type = "text";
    let first_label_style = ctx
        .default_edge_style
        .first()
        .or_else(|| edge.style.first());
    let compiled_label_styles = ctx.edge_style_plan.swimlane_label_for(key)?;
    let first_label_style = first_label_style.map_or("", String::as_str);
    let label_text_style = ctx
        .edge_style_plan
        .swimlane_edge_label_text_style_for(key, &ctx.text_style)?;
    let font_stack_precedence = crate::flowchart::FlowchartFacetPrecedence::new(
        compiled_label_styles.map_or(
            crate::flowchart::FlowchartSourceFacetStatus::Absent,
            FlowchartCompiledStyles::source_font_stack_status,
        ),
        ctx.node_typography_config_ownership.font_stack,
    );
    let font_size_precedence = crate::flowchart::FlowchartFacetPrecedence::new(
        compiled_label_styles.map_or(
            crate::flowchart::FlowchartSourceFacetStatus::Absent,
            FlowchartCompiledStyles::source_font_size_status,
        ),
        ctx.node_typography_config_ownership.font_size,
    );
    let typed_font_stack_selected = ctx.edge_theme.font_stack_selected(font_stack_precedence);
    let typed_font_size_selected = ctx.edge_theme.font_size_selected(font_size_precedence);
    let label_text_plain = flowchart_label_plain_text(label_text, label_type, ctx.node_html_labels);
    let typography_applicable = !crate::flowchart::flowchart_label_text_is_empty_for_mode(
        &label_text_plain,
        ctx.node_html_labels,
    );
    let record_label_emission =
        |source_typography_verified,
         prepared: Option<&crate::flowchart::FlowchartSvgLabelRenderPlan<'_>>,
         sanitized_xhtml: Option<&str>| {
            let receipt = edge_label_emission_receipt(
                source_typography_verified,
                typography_applicable,
                prepared,
            );
            record_edge_label_emission(
                ctx,
                edge.id.as_str(),
                compiled_label_styles,
                typed_font_stack_selected,
                typed_font_size_selected,
                typography_applicable,
                receipt,
                sanitized_xhtml,
            );
        };
    ctx.theme_evidence.record_source_residuals(
        &compiled_label_styles.map_or_else(Vec::new, |styles| {
            styles.emitted_shape_source_residuals(edge.id.as_str(), true)
        }),
    );

    let Some(layout_edge) = ctx.layout_edges_by_key.get(&key) else {
        record_label_emission(false, None, None);
        return Ok(());
    };
    let Some(label) = layout_edge.label.as_ref() else {
        record_label_emission(false, None, None);
        return Ok(());
    };

    // Swimlane's private `positionEdgeLabel` recomputes whenever `insertEdge` returns a paths
    // object, independent of the ordinary updated-path heuristic.
    let position = resolve_flowchart_edge_label_position(
        ctx,
        key,
        layout_edge,
        label,
        origin_x,
        origin_y,
        edge_cache,
        true,
    );
    let x = position.x;
    let y = position.y;
    let width = label.width.max(0.0);
    let height = label.height.max(0.0);
    let content = ctx.edge_label_padding.content_box(width, height);
    let background = padded_html_edge_label_background(ctx.edge_label_padding, width, height);
    let edge_label_dom_id =
        ctx.document_ids
            .synthetic_label(node_id)
            .ok_or_else(|| crate::Error::InvalidModel {
                message: format!("missing prepared Swimlane edge-label DOM id for `{node_id}`"),
            })?;

    let _ = write!(
        out,
        r#"<g class="label edgeLabel" id="{}" data-id="{}" data-et="edge-label" transform="translate({}, {})"><rect width="0.1" height="0.1"/>{}<g class="label"{} transform="translate({}, {})"><rect/>"#,
        edge_label_dom_id,
        escape_xml_display(&edge.id),
        fmt_display(x),
        fmt_display(y),
        background,
        OptionalStyleXmlAttr(first_label_style),
        fmt_display(content.x),
        fmt_display(content.y),
    );

    if ctx.node_html_labels {
        let owner = Some(crate::flowchart::FlowchartSvgLabelOwner::SwimlaneEdgeLabel(
            key.semantic_index(),
        ));
        let prepared_math = ctx
            .svg_label_sidecar
            .zip(owner)
            .map_or(Default::default(), |(sidecar, owner)| {
                sidecar.prepared_math_for_terminal(owner, label_text)
            });
        let label_html = flowchart_label_html_with_prepared_math(
            label_text,
            label_type,
            ctx.config,
            ctx.math_renderer,
            prepared_math,
        );
        // `createText` maps only the first literal `fill:` occurrence to `color:` before applying
        // the copied label style to the HTML span and div. Later fixed declarations override the
        // corresponding inline properties exactly as Mermaid's chained D3 `.style()` calls do.
        let html_label_style = first_label_style.replacen("fill:", "color:", 1);
        let mut div_style = String::new();
        if !html_label_style.trim().is_empty() {
            div_style.push_str(html_label_style.trim());
            div_style.push(';');
        }
        let _ = write!(
            &mut div_style,
            "display: table-cell; white-space: nowrap; line-height: 1.5; max-width: {}px; text-align: center;",
            fmt_display(ctx.wrapping_width),
        );
        let _ = write!(
            out,
            r#"<foreignObject width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" style="{}"><span class="nodeLabel"{}>{}</span></div></foreignObject></g></g>"#,
            fmt_display(content.width),
            fmt_display(content.height),
            escape_xml_display(&div_style),
            OptionalStyleXmlAttr(&html_label_style),
            label_html,
        );
        record_label_emission(true, None, Some(&label_html));
        return Ok(());
    }

    out.push_str(r#"<g><rect class="background" style="stroke: none"/>"#);
    let owner = Some(crate::flowchart::FlowchartSvgLabelOwner::SwimlaneEdgeLabel(
        key.semantic_index(),
    ));
    let prepared = crate::flowchart::FlowchartSvgLabelRenderPlan::new(
        ctx.svg_label_sidecar,
        owner,
        label_text,
        ctx.measurer,
        label_text_style.as_ref(),
        Some(ctx.wrapping_width),
        true,
        crate::flowchart::FlowchartSvgWidthMode::Bbox,
    );
    write_flowchart_svg_label_plan(out, &prepared, true);
    out.push_str("</g></g></g>");
    record_label_emission(true, Some(&prepared), None);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: f64, y: f64) -> crate::model::LayoutPoint {
        crate::model::LayoutPoint { x, y }
    }

    fn geom(d: &str, points: Vec<crate::model::LayoutPoint>) -> FlowchartEdgePathGeom {
        FlowchartEdgePathGeom {
            d: d.to_string(),
            pb: None,
            data_points: points.clone(),
            data_points_b64: String::new(),
            original_path_length: None,
            path_length: None,
            line_hop_applied: false,
            label_path_points: points,
            label_path_was_explicitly_updated: false,
            emitted_d_for_label: None,
            bounds_skipped_for_viewbox: false,
        }
    }

    #[test]
    fn svg_edge_label_box_centers_text_and_its_padded_background() {
        let label_box = centered_svg_edge_label_box(104.0, 24.0, 3.0);
        assert_eq!(label_box.translate_x, 0.0);
        assert_eq!(label_box.translate_y, -13.0);
        assert_eq!(label_box.background_x, -52.0);
        assert_eq!(label_box.background_y, 1.0);
        assert_eq!(label_box.background_width, 104.0);
        assert_eq!(label_box.background_height, 24.0);

        let translated_background_center_y =
            label_box.translate_y + label_box.background_y + label_box.background_height / 2.0;
        assert_eq!(translated_background_center_y, 0.0);
    }

    #[test]
    fn ordinary_dagre_keeps_anchor_until_insert_edge_marks_path_updated() {
        let anchor = point(4.0, 5.0);
        let mut geometry = geom(
            "M0,0L10,0L20,0",
            vec![point(0.0, 0.0), point(10.0, 0.0), point(20.0, 0.0)],
        );

        let unchanged = position_flowchart_edge_label(anchor.clone(), &geometry, false);
        assert_eq!((unchanged.x, unchanged.y), (4.0, 5.0));

        geometry.label_path_was_explicitly_updated = true;
        let updated = position_flowchart_edge_label(anchor, &geometry, false);
        assert_eq!((updated.x, updated.y), (10.0, 0.0));
    }

    #[test]
    fn swimlane_always_recomputes_from_returned_waypoints() {
        let geometry = geom(
            "M0,0L10,0L20,0",
            vec![point(0.0, 0.0), point(10.0, 0.0), point(20.0, 0.0)],
        );

        let positioned = position_flowchart_edge_label(point(4.0, 5.0), &geometry, true);
        assert_eq!((positioned.x, positioned.y), (10.0, 0.0));
    }

    #[test]
    fn rough_actual_emitted_path_drives_updated_path_detection() {
        let anchor = point(4.0, 5.0);
        let mut geometry = geom(
            "M0,0L10,20L30,20",
            vec![point(0.0, 0.0), point(10.0, 20.0), point(30.0, 20.0)],
        );

        let logical_curve = position_flowchart_edge_label(anchor.clone(), &geometry, false);
        assert_eq!((logical_curve.x, logical_curve.y), (4.0, 5.0));

        geometry.emitted_d_for_label = Some("M1.1,2.2C3.3,4.4,5.5,6.6,7.7,8.8".to_string());
        let rough_curve = position_flowchart_edge_label(anchor, &geometry, false);
        assert_ne!((rough_curve.x, rough_curve.y), (4.0, 5.0));
    }

    #[test]
    fn shared_geometry_handles_empty_single_degenerate_and_polyline_paths() {
        let anchor = point(4.0, 5.0);
        let empty = geom("", Vec::new());
        let positioned = position_flowchart_edge_label(anchor.clone(), &empty, true);
        assert_eq!((positioned.x, positioned.y), (4.0, 5.0));

        let single = geom("M2,3", vec![point(2.0, 3.0)]);
        let positioned = position_flowchart_edge_label(anchor.clone(), &single, true);
        assert_eq!((positioned.x, positioned.y), (2.0, 3.0));

        let degenerate = geom("M2,3L2,3", vec![point(2.0, 3.0), point(2.0, 3.0)]);
        let positioned = position_flowchart_edge_label(anchor.clone(), &degenerate, true);
        assert_eq!((positioned.x, positioned.y), (2.0, 3.0));

        let polyline = geom(
            "M0,0L6,0L6,8",
            vec![point(0.0, 0.0), point(6.0, 0.0), point(6.0, 8.0)],
        );
        let positioned = position_flowchart_edge_label(anchor, &polyline, true);
        assert_eq!((positioned.x, positioned.y), (6.0, 1.0));
    }
}
