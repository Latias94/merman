use super::super::*;
use crate::flowchart::{
    FLOWCHART_FIXED_LABEL_WRAP_WIDTH, FlowchartClusterThemeEmission, FlowchartFacetPrecedence,
    FlowchartShapeFacetEmissionReceipt, FlowchartThemeFacetEmission,
    flowchart_label_is_empty_for_render,
};
use crate::model::SwimlaneLaneLayout;

const SWIMLANE_HAND_DRAWN_ROUGHNESS: f32 = 0.7;
const SWIMLANE_HAND_DRAWN_FILL_WEIGHT: f32 = 3.0;
const SWIMLANE_HAND_DRAWN_HACHURE_GAP: f32 = 1.5;

#[derive(Debug, Clone, Copy)]
struct SwimlaneShellEmissionReceipt {
    body: FlowchartShapeFacetEmissionReceipt,
    title: FlowchartShapeFacetEmissionReceipt,
}

impl SwimlaneShellEmissionReceipt {
    const fn source_facets(self) -> FlowchartShapeFacetEmissionReceipt {
        self.body.merge(self.title)
    }

    const fn fill_verified(self) -> bool {
        self.body.fill && self.title.fill
    }

    const fn stroke_verified(self) -> bool {
        self.body.stroke && self.title.stroke
    }
}

fn rough_style_from_node_style(node_style: &str, mut keep: impl FnMut(&str) -> bool) -> String {
    let mut out = String::new();
    for declaration in node_style.split(';') {
        let declaration = declaration.trim();
        let Some((key, _)) = declaration.split_once(':') else {
            continue;
        };
        if !keep(key.trim()) {
            continue;
        }
        if !out.is_empty() {
            out.push(';');
        }
        out.push_str(declaration);
    }
    out
}

fn parse_css_px_f32(value: Option<&String>, fallback: f32) -> f32 {
    value
        .and_then(|raw| raw.trim_end_matches("px").trim().parse::<f32>().ok())
        .unwrap_or(fallback)
}

#[allow(clippy::too_many_arguments)]
fn write_swimlane_rect(
    out: &mut impl crate::svg::parity::SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    compiled: &FlowchartCompiledStyles,
    class_name: &str,
    node_style: &str,
    typed_stroke_width: Option<f32>,
    paint_owner_id: impl std::fmt::Display,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    fill: Option<&str>,
    stroke: &str,
) -> FlowchartShapeFacetEmissionReceipt {
    if ctx.compatibility.look.as_str() == "handDrawn" {
        let stroke_width = parse_css_px_f32(
            compiled.stroke_width.as_ref(),
            typed_stroke_width.unwrap_or(1.3),
        );
        let stroke_dasharray = compiled.stroke_dasharray.as_deref().unwrap_or("0 0").trim();
        // RoughJS creates the outline before the fill. Generating both paths
        // and omitting the fill path for the body therefore preserves the
        // exact seeded outline used by `fill: none` upstream.
        if let Some((fill_d, stroke_d)) =
            super::super::render::node::roughjs::roughjs_hachure_paths_for_rect(
                x,
                y,
                width,
                height,
                stroke_width,
                stroke_dasharray,
                SWIMLANE_HAND_DRAWN_FILL_WEIGHT,
                SWIMLANE_HAND_DRAWN_HACHURE_GAP,
                SWIMLANE_HAND_DRAWN_ROUGHNESS,
                ctx.work_meter,
                &ctx.hand_drawn_seed,
            )
        {
            out.push_str("<g>");
            if let Some(fill) = fill {
                let background_style = rough_style_from_node_style(node_style, |key| key == "fill")
                    .replace("fill", "stroke");
                let _ = write!(
                    out,
                    r#"<path id="{}" d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="0 0"{} />"#,
                    escape_xml_display(&format!(
                        "{}-{}{}",
                        paint_owner_id,
                        class_name,
                        crate::svg::RENDERER_SEMANTIC_FILL_PATH_SUFFIX
                    )),
                    escape_xml_display(&fill_d),
                    escape_xml_display(fill),
                    fmt_display(SWIMLANE_HAND_DRAWN_FILL_WEIGHT as f64),
                    OptionalStyleXmlAttr(&background_style),
                );
            }
            let border_style =
                rough_style_from_node_style(node_style, |key| key.contains("stroke"));
            let _ = write!(
                out,
                r#"<path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}"{} /></g>"#,
                escape_xml_display(&stroke_d),
                escape_xml_display(stroke),
                fmt_display(stroke_width as f64),
                escape_xml_display(stroke_dasharray),
                OptionalStyleXmlAttr(&border_style),
            );
            return FlowchartShapeFacetEmissionReceipt {
                fill: fill.is_some(),
                stroke: true,
                stroke_width: true,
                stroke_dasharray: true,
                remaining_shape_style: false,
            };
        }
    }

    let _ = write!(
        out,
        r#"<rect class="{}" style="{}" x="{}" y="{}" width="{}" height="{}" fill="{}" stroke="{}"/>"#,
        escape_xml_display(class_name),
        escape_xml_display(node_style),
        fmt_display(x),
        fmt_display(y),
        fmt_display(width),
        fmt_display(height),
        escape_xml_display(fill.unwrap_or("none")),
        escape_xml_display(stroke),
    );
    FlowchartShapeFacetEmissionReceipt::all()
}

pub(in crate::svg::parity::flowchart) fn render_swimlane_cluster(
    out: &mut impl crate::svg::parity::SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    cluster: &LayoutCluster,
    cluster_theme: &crate::flowchart::FlowchartClusterThemeStyle,
    lane: &SwimlaneLaneLayout,
    origin_x: f64,
    origin_y: f64,
) -> crate::Result<()> {
    ctx.checkpoint_emit()?;
    let subgraph = ctx.subgraphs_by_id.get(cluster.id.as_str()).copied();
    let subgraph_index = ctx.subgraph_indices_by_id.get(cluster.id.as_str()).copied();
    let (class_names, styles) = subgraph
        .zip(subgraph_index)
        .map(|(subgraph, subgraph_index)| {
            ctx.model.effective_subgraph_css(subgraph_index, subgraph)
        })
        .unwrap_or_default();
    let compiled = flowchart_compile_styles(ctx.class_defs, class_names, styles, &[]);
    let fill_precedence = FlowchartFacetPrecedence::new(
        compiled.source_fill_status(),
        ctx.cluster_fill_config_override,
    );
    let stroke_precedence = FlowchartFacetPrecedence::new(
        compiled.source_stroke_status(),
        ctx.cluster_stroke_config_override,
    );
    let stroke_width_precedence =
        FlowchartFacetPrecedence::new(compiled.source_stroke_width_status(), false);
    let typed_stroke_width = cluster_theme.stroke_width_value(stroke_width_precedence);
    let mut node_style = compiled.node_style.trim().to_string();
    cluster_theme.append_inline_style(
        &mut node_style,
        fill_precedence,
        stroke_precedence,
        stroke_width_precedence,
    );
    let label_style = compiled.label_style.trim();
    let render_title =
        subgraph
            .zip(subgraph_index)
            .map_or(lane.title.as_str(), |(subgraph, subgraph_index)| {
                ctx.model
                    .subgraph_title_for_render(subgraph_index, subgraph)
            });
    let direction = ctx
        .swimlane_direction
        .ok_or_else(|| crate::Error::InvalidModel {
            message: format!(
                "missing Swimlane direction while rendering lane `{}`",
                lane.id
            ),
        })?;
    let geometry = super::swimlane_terminal_geometry(lane, direction);
    let label_width = lane.title_label_width.max(0.0);
    let label_height = lane.title_label_height.max(0.0);
    let title_owner = ctx
        .svg_label_sidecar
        .and_then(|sidecar| sidecar.swimlane_group_title_owner(cluster.id.as_str()));
    let mut title_receipt =
        super::super::render::node::emission::FlowchartNodeLabelEmissionReceipt::verified()
            .with_prepared_typography_reach(
                !flowchart_label_is_empty_for_render(render_title),
                false,
            );

    let width = geometry.width;
    let height = geometry.height;
    let lane_top = lane.y - height / 2.0 + ctx.ty - origin_y;
    let lane_bottom = lane.y + height / 2.0 + ctx.ty - origin_y;
    let lane_left = lane.x - width / 2.0 + ctx.tx - origin_x;
    let is_lr = direction == crate::model::SwimlaneDirection::Lr;

    let typed_lane_fill = cluster_theme.fill_value(fill_precedence, true);
    let lane_fill = typed_lane_fill.unwrap_or(&ctx.cluster_fill_color);
    let typed_lane_stroke = cluster_theme.stroke_value(stroke_precedence, true);
    let lane_stroke = typed_lane_stroke.unwrap_or(&ctx.cluster_stroke_color);
    // The body has no compatibility fill path in hand-drawn output. Only a direct typed fill may
    // add one; a typed stroke owns the outline independently and must not recolor the body.
    let body_fill = typed_lane_fill;
    let mut classes = String::from("cluster swimlane");
    for class in class_names {
        let class = class.trim();
        if !class.is_empty() {
            classes.push(' ');
            classes.push_str(class);
        }
    }
    let lane_dom_id =
        ctx.document_ids
            .lane(&lane.id)
            .ok_or_else(|| crate::Error::InvalidModel {
                message: format!("missing prepared Swimlane DOM id for lane `{}`", lane.id),
            })?;
    let _ = write!(
        out,
        r#"<g class="{}" id="{}" data-id="{}" data-et="cluster""#,
        escape_xml_display(&classes),
        lane_dom_id,
        escape_xml_display(&lane.id),
    );
    let data_look = ctx.compatibility.look.as_str();
    let color_slot = super::super::agentflow::container_color_slot(ctx, &lane.id)
        .or_else(|| (subgraph.is_none() && !ctx.compatibility.palette.is_empty()).then_some(0));
    let _ = write!(out, r#" data-look="{}""#, escape_xml_display(data_look));
    if let Some(color_slot) = color_slot {
        let _ = write!(out, r#" data-color-id="color-{color_slot}""#);
    }
    out.push('>');

    let (shell_receipt, label_x, label_y, label_transform) = if is_lr {
        let title_width = geometry.title_band_width;
        let body_x = lane_left + title_width;
        let body_width = (width - title_width).max(0.0);
        let body_receipt = write_swimlane_rect(
            out,
            ctx,
            &compiled,
            "swimlane-body",
            &node_style,
            typed_stroke_width,
            lane_dom_id,
            body_x,
            lane_top,
            body_width,
            height,
            body_fill,
            lane_stroke,
        );
        let title_receipt = write_swimlane_rect(
            out,
            ctx,
            &compiled,
            "swimlane-title",
            &node_style,
            typed_stroke_width,
            lane_dom_id,
            lane_left,
            lane_top,
            title_width,
            height,
            Some(lane_fill),
            lane_stroke,
        );
        let center_x = geometry.title_label.x + ctx.tx - origin_x;
        let center_y = geometry.title_label.y + ctx.ty - origin_y;
        (
            SwimlaneShellEmissionReceipt {
                body: body_receipt,
                title: title_receipt,
            },
            0.0,
            0.0,
            format!(
                "translate({}, {}) rotate(-90) translate({}, {})",
                fmt_display(center_x),
                fmt_display(center_y),
                fmt_display(-label_width / 2.0),
                fmt_display(-label_height / 2.0),
            ),
        )
    } else {
        let title_height = geometry.title_band_height;
        let body_y = lane_top + title_height;
        let body_height = (lane_bottom - body_y).max(0.0);
        let body_receipt = write_swimlane_rect(
            out,
            ctx,
            &compiled,
            "swimlane-body",
            &node_style,
            typed_stroke_width,
            lane_dom_id,
            lane_left,
            body_y,
            width,
            body_height,
            body_fill,
            lane_stroke,
        );
        let title_receipt = write_swimlane_rect(
            out,
            ctx,
            &compiled,
            "swimlane-title",
            &node_style,
            typed_stroke_width,
            lane_dom_id,
            lane_left,
            lane_top,
            width,
            title_height,
            Some(lane_fill),
            lane_stroke,
        );
        (
            SwimlaneShellEmissionReceipt {
                body: body_receipt,
                title: title_receipt,
            },
            geometry.title_label.x - label_width / 2.0 + ctx.tx - origin_x,
            geometry.title_label.y - label_height / 2.0 + ctx.ty - origin_y,
            String::new(),
        )
    };
    let source_residuals = compiled.emitted_shape_source_residuals_with_receipt(
        lane.id.as_str(),
        shell_receipt.source_facets(),
    );
    ctx.theme_evidence.record_cluster_emission(
        cluster_theme,
        FlowchartClusterThemeEmission {
            fill: FlowchartThemeFacetEmission::new(fill_precedence, shell_receipt.fill_verified()),
            stroke: FlowchartThemeFacetEmission::new(
                stroke_precedence,
                shell_receipt.stroke_verified(),
            ),
            stroke_width: FlowchartThemeFacetEmission::new(
                stroke_width_precedence,
                shell_receipt.body.stroke_width && shell_receipt.title.stroke_width,
            ),
        },
        &source_residuals,
        ctx.work_meter,
    )?;

    let mut html_typography_statuses = (
        crate::flowchart::FlowchartSourceFacetStatus::Absent,
        crate::flowchart::FlowchartSourceFacetStatus::Absent,
    );
    if ctx.swimlane_title_html_labels {
        let prepared_math = ctx
            .svg_label_sidecar
            .and_then(|sidecar| {
                sidecar
                    .swimlane_group_title_owner(cluster.id.as_str())
                    .map(|owner| sidecar.prepared_math_for_terminal(owner, render_title))
            })
            .unwrap_or_default();
        let title_html = flowchart_label_html_with_prepared_math(
            render_title,
            "markdown",
            ctx.config,
            ctx.math_renderer,
            prepared_math,
        );
        html_typography_statuses =
            crate::svg::parity::flowchart::style::sanitized_xhtml_typography_statuses(
                title_html.as_ref(),
            );
        let transform = if is_lr {
            label_transform
        } else {
            format!(
                "translate({}, {})",
                fmt_display(label_x),
                fmt_display(label_y)
            )
        };
        let div_style = format!(
            "display: table-cell; white-space: nowrap; line-height: 1.5; max-width: {}px; text-align: center;",
            fmt_display(width),
        );
        let _ = write!(
            out,
            r#"<g class="cluster-label swimlane-label" transform="{}"><foreignObject width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" style="{}"><span class="nodeLabel"{}>{}</span></div></foreignObject></g>"#,
            escape_xml_display(&transform),
            fmt_display(label_width),
            fmt_display(label_height),
            escape_xml_display(&div_style),
            OptionalStyleXmlAttr(label_style),
            title_html,
        );
        if ctx.text_surface_paint.requested() {
            ctx.text_surface_paint.record_label(
                lane.id.as_str(),
                &crate::text::VisibleTextStyleFacts::from_xhtml_fragment(title_html.as_ref()),
                compiled.source_label_foreground_status(),
                ctx.work_meter,
            )?;
        }
        ctx.theme_evidence
            .record_source_residuals(&compiled.emitted_html_label_source_residuals(
                lane.id.as_str(),
                true,
                &title_html,
            ));
    } else {
        let transform = if is_lr {
            label_transform
        } else {
            format!(
                "translate({}, {})",
                fmt_display(label_x),
                fmt_display(label_y)
            )
        };
        let _ = write!(
            out,
            r#"<g class="cluster-label swimlane-label" transform="{}"><g><rect class="background" style="stroke: none"/>"#,
            escape_xml_display(&transform),
        );
        let title_text_style = crate::flowchart::flowchart_effective_text_style_for_classes(
            &ctx.text_style,
            ctx.class_defs,
            class_names,
            styles,
        );
        let prepared = crate::flowchart::FlowchartSvgLabelRenderPlan::new(
            ctx.svg_label_sidecar,
            title_owner,
            render_title,
            ctx.measurer,
            title_text_style.as_ref(),
            Some(FLOWCHART_FIXED_LABEL_WRAP_WIDTH),
            true,
            crate::flowchart::FlowchartSvgWidthMode::Bbox,
        );
        let prepared_title = prepared.is_prepared();
        if prepared_title {
            write_flowchart_svg_label_plan(out, &prepared, true);
        } else {
            write_flowchart_svg_text_markdown(out, render_title, true);
        }
        title_receipt = title_receipt.with_prepared_typography_reach(
            !flowchart_label_is_empty_for_render(render_title),
            prepared_title && prepared.emitted_admitted_typography(),
        );
        out.push_str("</g></g>");
        if ctx.text_surface_paint.requested() {
            ctx.text_surface_paint.record_label(
                lane.id.as_str(),
                &crate::text::VisibleTextStyleFacts::from_svg_markdown_projection(render_title),
                super::super::css::cluster_title_class_foreground(
                    ctx.class_defs,
                    class_names,
                    ctx.work_meter,
                )?,
                ctx.work_meter,
            )?;
        }

        ctx.theme_evidence.record_source_residuals(
            &compiled.emitted_label_source_residuals(lane.id.as_str(), prepared_title),
        );
    }
    ctx.record_base_typography_label_emission(
        crate::flowchart::FlowchartBaseTypographyLabelEmission::new(
            title_owner,
            title_receipt.typography_applicable(),
            title_receipt.typography_verified(),
        )
        .with_source_facets(
            compiled
                .source_font_stack_status()
                .merge(html_typography_statuses.0),
            compiled
                .source_font_size_status()
                .merge(html_typography_statuses.1),
        ),
    );
    out.push_str("</g>");
    Ok(())
}
