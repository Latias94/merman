//! Flowchart cluster renderer.

use super::super::*;
use crate::flowchart::{
    FLOWCHART_FIXED_LABEL_WRAP_WIDTH, FlowchartClusterThemeEmission, FlowchartFacetPrecedence,
    FlowchartShapeFacetEmissionReceipt, FlowchartThemeFacetEmission,
};
use crate::svg::parity::flowchart::util::HTML_LABEL_FOREIGN_OBJECT_OVERFLOW_ATTR;

const FLOWCHART_CLUSTER_HAND_DRAWN_ROUGHNESS: f32 = 0.7;
const FLOWCHART_CLUSTER_HAND_DRAWN_FILL_WEIGHT: f32 = 3.0;
const FLOWCHART_CLUSTER_HAND_DRAWN_HACHURE_GAP: f32 = 5.2;

fn rounded_rect_path_d(x: f64, y: f64, w: f64, h: f64, r: f64) -> String {
    let mut out = String::new();
    let _ = write!(
        &mut out,
        "M {} {} H {} A {} {} 0 0 1 {} {} V {} A {} {} 0 0 1 {} {} H {} A {} {} 0 0 1 {} {} V {} A {} {} 0 0 1 {} {} Z",
        fmt_display(x + r),
        fmt_display(y),
        fmt_display(x + w - r),
        fmt_display(r),
        fmt_display(r),
        fmt_display(x + w),
        fmt_display(y + r),
        fmt_display(y + h - r),
        fmt_display(r),
        fmt_display(r),
        fmt_display(x + w - r),
        fmt_display(y + h),
        fmt_display(x + r),
        fmt_display(r),
        fmt_display(r),
        fmt_display(x),
        fmt_display(y + h - r),
        fmt_display(y + r),
        fmt_display(r),
        fmt_display(r),
        fmt_display(x + r),
        fmt_display(y),
    );
    out
}

fn rough_style_from_node_style(node_style: &str, mut keep: impl FnMut(&str) -> bool) -> String {
    let mut out = String::new();
    for decl in node_style.split(';') {
        let decl = decl.trim();
        let Some((key, _)) = decl.split_once(':') else {
            continue;
        };
        if !keep(key.trim()) {
            continue;
        }
        if !out.is_empty() {
            out.push(';');
        }
        out.push_str(decl);
    }
    out
}

fn cluster_rough_background_style(node_style: &str) -> String {
    rough_style_from_node_style(node_style, |key| key == "fill").replace("fill", "stroke")
}

fn cluster_rough_border_style(node_style: &str) -> String {
    rough_style_from_node_style(node_style, |key| key.contains("stroke"))
}

fn parse_css_px_f32(v: Option<&String>, fallback: f32) -> f32 {
    v.and_then(|raw| raw.trim_end_matches("px").trim().parse::<f32>().ok())
        .unwrap_or(fallback)
}

#[allow(clippy::too_many_arguments)]
fn write_flowchart_cluster_shape(
    out: &mut impl crate::svg::parity::SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    compiled_styles: &FlowchartCompiledStyles,
    rect_style: &str,
    fill_path_id: &str,
    fill: &str,
    stroke: &str,
    left: f64,
    top: f64,
    rect_w: f64,
    rect_h: f64,
) -> FlowchartShapeFacetEmissionReceipt {
    if flowchart_config_look(ctx.config) == "handDrawn" {
        let stroke_width = parse_css_px_f32(compiled_styles.stroke_width.as_ref(), 1.3);
        let stroke_dasharray = compiled_styles
            .stroke_dasharray
            .as_deref()
            .unwrap_or("0 0")
            .trim();
        let path = rounded_rect_path_d(left, top, rect_w, rect_h, 0.0);

        if let Some((fill_d, stroke_d)) = super::node::roughjs::roughjs_hachure_paths_for_svg_path(
            &path,
            fill,
            stroke,
            stroke_width,
            stroke_dasharray,
            FLOWCHART_CLUSTER_HAND_DRAWN_FILL_WEIGHT,
            FLOWCHART_CLUSTER_HAND_DRAWN_HACHURE_GAP,
            FLOWCHART_CLUSTER_HAND_DRAWN_ROUGHNESS,
            ctx.work_meter,
            &ctx.hand_drawn_seed,
        ) {
            let background_style = cluster_rough_background_style(rect_style);
            let border_style = cluster_rough_border_style(rect_style);
            let _ = write!(
                out,
                r#"<g><path id="{}" d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="0 0"{} /><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}"{} /></g>"#,
                escape_xml_display(fill_path_id),
                escape_xml_display(&fill_d),
                escape_xml_display(fill),
                fmt_display(FLOWCHART_CLUSTER_HAND_DRAWN_FILL_WEIGHT as f64),
                OptionalStyleXmlAttr(&background_style),
                escape_xml_display(&stroke_d),
                escape_xml_display(stroke),
                fmt_display(stroke_width as f64),
                escape_xml_display(stroke_dasharray),
                OptionalStyleXmlAttr(&border_style),
            );
            return FlowchartShapeFacetEmissionReceipt {
                fill: true,
                stroke: true,
                stroke_width: true,
                stroke_dasharray: true,
                remaining_shape_style: false,
            };
        }
    }

    let _ = write!(
        out,
        r#"<rect style="{}" x="{}" y="{}" width="{}" height="{}"/>"#,
        escape_xml_display(rect_style),
        fmt_display(left),
        fmt_display(top),
        fmt_display(rect_w),
        fmt_display(rect_h)
    );
    FlowchartShapeFacetEmissionReceipt::all()
}

fn record_cluster_shape_emission(
    ctx: &FlowchartRenderCtx<'_>,
    cluster_theme: &crate::flowchart::FlowchartClusterThemeStyle,
    compiled_styles: &FlowchartCompiledStyles,
    cluster_id: &str,
    receipt: FlowchartShapeFacetEmissionReceipt,
    fill_precedence: FlowchartFacetPrecedence,
    stroke_precedence: FlowchartFacetPrecedence,
) -> crate::Result<()> {
    let source_residuals =
        compiled_styles.emitted_shape_source_residuals_with_receipt(cluster_id, receipt);
    ctx.theme_evidence.record_cluster_emission(
        cluster_theme,
        FlowchartClusterThemeEmission {
            fill: FlowchartThemeFacetEmission::new(fill_precedence, receipt.fill),
            stroke: FlowchartThemeFacetEmission::new(stroke_precedence, receipt.stroke),
        },
        &source_residuals,
        ctx.work_meter,
    )?;
    Ok(())
}

pub(in crate::svg::parity) fn render_flowchart_cluster(
    out: &mut impl crate::svg::parity::SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    cluster: &LayoutCluster,
    cluster_theme: &crate::flowchart::FlowchartClusterThemeStyle,
    origin_x: f64,
    origin_y: f64,
) -> crate::Result<()> {
    if let Some(lane) = ctx.swimlane_lanes_by_id.get(cluster.id.as_str())
        && lane.parent_id.is_none()
    {
        super::super::swimlane::render_swimlane_cluster(
            out,
            ctx,
            cluster,
            cluster_theme,
            lane,
            origin_x,
            origin_y,
        )?;
        return Ok(());
    }

    let Some(sg) = ctx.subgraphs_by_id.get(cluster.id.as_str()) else {
        return Err(crate::Error::InvalidModel {
            message: format!(
                "Flowchart layout cluster `{}` has no semantic subgraph owner",
                cluster.id
            ),
        });
    };
    let Some(subgraph_index) = ctx.subgraph_indices_by_id.get(cluster.id.as_str()).copied() else {
        return Err(crate::Error::InvalidModel {
            message: format!(
                "missing semantic Flowchart subgraph index for cluster `{}`",
                cluster.id
            ),
        });
    };
    if !ctx.subgraph_has_children(cluster.id.as_str())
        && !super::flowchart_elk_renders_empty_subgraph_as_cluster(ctx)
    {
        return Ok(());
    }

    let (classes, styles) = ctx.model.effective_subgraph_css(subgraph_index, sg);
    let compiled_styles = flowchart_compile_styles(ctx.class_defs, classes, styles, &[]);
    let fill_precedence = FlowchartFacetPrecedence::new(
        compiled_styles.source_fill_status(),
        ctx.cluster_fill_config_override,
    );
    let stroke_precedence = FlowchartFacetPrecedence::new(
        compiled_styles.source_stroke_status(),
        ctx.cluster_stroke_config_override,
    );
    let mut rect_style = compiled_styles.node_style.trim().to_string();
    cluster_theme.append_inline_style(&mut rect_style, fill_precedence, stroke_precedence);
    let fill = cluster_theme
        .fill_value(fill_precedence, true)
        .unwrap_or(&ctx.cluster_fill_color);
    let stroke = cluster_theme
        .stroke_value(stroke_precedence, true)
        .unwrap_or(&ctx.cluster_stroke_color);
    let label_style = compiled_styles.label_style.trim();

    let left = (cluster.x - cluster.width / 2.0) + ctx.tx - origin_x;
    let top = (cluster.y - cluster.height / 2.0) + ctx.ty - origin_y;
    let rect_w = cluster.width.max(1.0);
    let rect_h = cluster.height.max(1.0);
    let label_top = top + cluster.title_margin_top.max(0.0);
    let Some(cluster_dom_id) = ctx.document_ids.cluster(cluster.id.as_str()) else {
        return Err(crate::Error::InvalidModel {
            message: format!(
                "missing prepared Flowchart DOM id for cluster `{}`",
                cluster.id
            ),
        });
    };
    let fill_path_id = format!(
        "{}{}",
        cluster_dom_id,
        crate::svg::RENDERER_SEMANTIC_FILL_PATH_SUFFIX
    );
    ctx.checkpoint_emit()?;

    let label_type = sg.label_type.as_deref().unwrap_or("text");
    let render_title = ctx.model.subgraph_title_for_render(subgraph_index, sg);
    let title_owner = ctx
        .svg_label_sidecar
        .and_then(|sidecar| sidecar.subgraph_title_owner(cluster.id.as_str()));
    let title_receipt = super::node::emission::FlowchartNodeLabelEmissionReceipt::verified()
        .with_prepared_typography_reach(
            !crate::flowchart::flowchart_label_is_empty_for_render(render_title),
            false,
        );

    let mut class_attr = String::new();
    for c in classes {
        let c = c.trim();
        if c.is_empty() {
            continue;
        }
        if !class_attr.is_empty() {
            class_attr.push(' ');
        }
        class_attr.push_str(c);
    }
    if !class_attr.is_empty() {
        class_attr.push(' ');
    }
    class_attr.push_str("cluster");
    let data_look = flowchart_config_look(ctx.config);

    // Mermaid renders subgraph titles using the same `flowchart.htmlLabels` toggle as edge labels.
    if !ctx.edge_html_labels {
        let label_w = cluster.title_label.width.max(0.0);
        let label_left = left + rect_w / 2.0 - label_w / 2.0;
        let _ = write!(
            out,
            r#"<g class="{}" id="{}" data-id="{}" data-et="cluster" data-look="{}">"#,
            escape_xml_display(&class_attr),
            cluster_dom_id,
            escape_xml_display(&cluster.id),
            escape_xml_display(data_look),
        );
        let shape_source_receipt = write_flowchart_cluster_shape(
            out,
            ctx,
            &compiled_styles,
            &rect_style,
            &fill_path_id,
            fill,
            stroke,
            left,
            top,
            rect_w,
            rect_h,
        );
        record_cluster_shape_emission(
            ctx,
            cluster_theme,
            &compiled_styles,
            cluster.id.as_str(),
            shape_source_receipt,
            fill_precedence,
            stroke_precedence,
        )?;
        let _ = write!(
            out,
            r#"<g class="cluster-label" transform="translate({},{})"><g><rect class="background" style="stroke: none"/>"#,
            fmt_display(label_left),
            fmt_display(label_top)
        );
        if label_type == "markdown" {
            write_flowchart_svg_text_markdown(out, render_title, true);
        } else {
            let title_text_style = crate::flowchart::flowchart_effective_text_style_for_classes(
                &ctx.text_style,
                ctx.class_defs,
                classes,
                styles,
            );
            let prepared = crate::flowchart::FlowchartSvgLabelRenderPlan::new(
                ctx.svg_label_sidecar,
                title_owner,
                render_title,
                ctx.measurer,
                title_text_style.as_ref(),
                None,
                true,
                crate::flowchart::FlowchartSvgWidthMode::Bbox,
            );
            write_flowchart_svg_label_plan(out, &prepared, true);
        }
        out.push_str("</g></g></g>");
        if ctx.title_paint.requested() {
            let facts = if label_type == "markdown" {
                crate::text::VisibleTextStyleFacts::from_svg_markdown_projection(render_title)
            } else {
                crate::text::VisibleTextStyleFacts::plain_text(
                    if crate::flowchart::flowchart_label_is_empty_for_render(render_title) {
                        ""
                    } else {
                        render_title
                    },
                )
            };
            // Assigned-class CSS still reaches tspans even though inline label_style is omitted.
            ctx.title_paint.record_label(
                cluster.id.as_str(),
                &facts,
                super::super::css::cluster_title_class_foreground(
                    ctx.class_defs,
                    classes,
                    ctx.work_meter,
                )?,
                ctx.work_meter,
            )?;
        }

        ctx.theme_evidence.record_source_residuals(
            &compiled_styles
                .emitted_label_source_residuals(cluster.id.as_str(), label_type != "markdown"),
        );
        ctx.record_base_typography_label_emission(
            crate::flowchart::FlowchartBaseTypographyLabelEmission::new(
                title_owner,
                title_receipt.typography_applicable(),
                title_receipt.typography_verified(),
            )
            .with_source_facets(
                compiled_styles.source_font_stack_status(),
                compiled_styles.source_font_size_status(),
            ),
        );
        return Ok(());
    }

    let prepared_math = ctx
        .svg_label_sidecar
        .zip(title_owner)
        .map_or(Default::default(), |(sidecar, owner)| {
            sidecar.prepared_math_for_terminal(owner, render_title)
        });
    let title_html = flowchart_label_html_with_prepared_math(
        render_title,
        label_type,
        ctx.config,
        ctx.math_renderer,
        prepared_math,
    );
    let label_w = cluster.title_label.width.max(0.0);
    let label_h = cluster.title_label.height.max(0.0);
    let label_left = left + rect_w / 2.0 - label_w / 2.0;

    let span_style_attr = OptionalStyleXmlAttr(label_style);
    let div_style = if label_type != "markdown" {
        "display: table-cell; white-space: nowrap; line-height: 1.5;".to_string()
    } else if label_w >= FLOWCHART_FIXED_LABEL_WRAP_WIDTH - 1e-3 {
        format!(
            "display: table; white-space: break-spaces; line-height: 1.5; max-width: {mw}px; text-align: center; width: {mw}px;",
            mw = fmt_display(FLOWCHART_FIXED_LABEL_WRAP_WIDTH)
        )
    } else {
        format!(
            "display: table-cell; white-space: nowrap; line-height: 1.5; max-width: {mw}px; text-align: center;",
            mw = fmt_display(FLOWCHART_FIXED_LABEL_WRAP_WIDTH)
        )
    };

    let _ = write!(
        out,
        r#"<g class="{}" id="{}" data-id="{}" data-et="cluster" data-look="{}">"#,
        escape_xml_display(&class_attr),
        cluster_dom_id,
        escape_xml_display(&cluster.id),
        escape_xml_display(data_look),
    );
    let shape_source_receipt = write_flowchart_cluster_shape(
        out,
        ctx,
        &compiled_styles,
        &rect_style,
        &fill_path_id,
        fill,
        stroke,
        left,
        top,
        rect_w,
        rect_h,
    );
    record_cluster_shape_emission(
        ctx,
        cluster_theme,
        &compiled_styles,
        cluster.id.as_str(),
        shape_source_receipt,
        fill_precedence,
        stroke_precedence,
    )?;
    let _ = write!(
        out,
        r#"<g class="cluster-label" transform="translate({},{})"><foreignObject width="{}" height="{}"{}><div xmlns="http://www.w3.org/1999/xhtml" style="{}"><span class="nodeLabel"{}>{}</span></div></foreignObject></g></g>"#,
        fmt_display(label_left),
        fmt_display(label_top),
        fmt_display(label_w),
        fmt_display(label_h),
        HTML_LABEL_FOREIGN_OBJECT_OVERFLOW_ATTR,
        escape_xml_display(&div_style),
        span_style_attr,
        title_html
    );
    if ctx.title_paint.requested() {
        ctx.title_paint.record_label(
            cluster.id.as_str(),
            &crate::text::VisibleTextStyleFacts::from_xhtml_fragment(title_html.as_ref()),
            compiled_styles.source_label_foreground_status(),
            ctx.work_meter,
        )?;
    }
    ctx.theme_evidence.record_source_residuals(
        &compiled_styles.emitted_html_label_source_residuals(
            cluster.id.as_str(),
            true,
            &title_html,
        ),
    );
    let html_typography_statuses =
        crate::svg::parity::flowchart::style::sanitized_xhtml_typography_statuses(
            title_html.as_ref(),
        );
    ctx.record_base_typography_label_emission(
        crate::flowchart::FlowchartBaseTypographyLabelEmission::new(
            title_owner,
            title_receipt.typography_applicable(),
            title_receipt.typography_verified(),
        )
        .with_source_facets(
            compiled_styles
                .source_font_stack_status()
                .merge(html_typography_statuses.0),
            compiled_styles
                .source_font_size_status()
                .merge(html_typography_statuses.1),
        ),
    );
    Ok(())
}
