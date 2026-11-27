use super::super::*;
use crate::flowchart::FLOWCHART_FIXED_LABEL_WRAP_WIDTH;
use crate::model::SwimlaneLaneLayout;
use crate::svg::parity::flowchart::util::HTML_LABEL_FOREIGN_OBJECT_OVERFLOW_ATTR;

const SWIMLANE_HAND_DRAWN_ROUGHNESS: f32 = 0.7;
const SWIMLANE_HAND_DRAWN_FILL_WEIGHT: f32 = 3.0;
const SWIMLANE_HAND_DRAWN_HACHURE_GAP: f32 = 1.5;

fn rough_style_from_node_style<'a>(
    node_style: &'a str,
    mut property: impl FnMut(&crate::mermaid_style::ParsedStyleDeclaration<'a>) -> Option<&'a str>,
) -> String {
    let mut out = String::new();
    crate::mermaid_style::visit_parsed_style_declarations(node_style, |declaration| {
        let Some(property) = property(&declaration) else {
            return;
        };
        if !out.is_empty() {
            out.push(';');
        }
        out.push_str(property);
        out.push(':');
        out.push_str(declaration.source_value());
    });
    out
}

fn parse_css_px_f32(value: Option<&String>, fallback: f32) -> f32 {
    value
        .and_then(|raw| raw.trim_end_matches("px").trim().parse::<f32>().ok())
        .unwrap_or(fallback)
}

pub(super) fn lane_color_slot(ctx: &FlowchartRenderCtx<'_>, id: &str) -> Option<usize> {
    super::super::agentflow::container_color_slot(ctx, id).or_else(|| {
        let palette_len = ctx.palette.color_count();
        (!ctx.subgraphs_by_id.contains_key(id) && palette_len > 0).then(|| {
            // The synthetic lane follows the highest visible source container color ordinal.
            ctx.model
                .subgraphs
                .iter()
                .filter(|subgraph| ctx.model.collapsed_replacement(&subgraph.id).is_none())
                .filter_map(|subgraph| ctx.model.subgraph_color_ordinal(&subgraph.id))
                .max()
                .map_or(0, |ordinal| ordinal.saturating_add(1))
                % palette_len
        })
    })
}

#[allow(clippy::too_many_arguments)]
fn write_swimlane_rect(
    out: &mut impl crate::svg::parity::SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    compiled: &FlowchartCompiledStyles,
    class_name: &str,
    node_style: &str,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    fill: Option<&str>,
    stroke: &str,
) {
    if ctx.compatibility.look.as_str() == "handDrawn" {
        let stroke_width = parse_css_px_f32(compiled.stroke_width.as_ref(), 1.3);
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
            let _ = write!(out, r#"<g class="{}">"#, escape_xml_display(class_name));
            if let Some(fill) = fill {
                let background_style = rough_style_from_node_style(node_style, |decl| {
                    (decl.property() == "fill").then_some("stroke")
                });
                let _ = write!(
                    out,
                    r#"<path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="0 0"{} />"#,
                    escape_xml_display(&fill_d),
                    escape_attr_display(fill),
                    fmt_display(SWIMLANE_HAND_DRAWN_FILL_WEIGHT as f64),
                    OptionalStyleAttr(&background_style),
                );
            }
            let border_style = rough_style_from_node_style(node_style, |decl| {
                decl.property()
                    .starts_with("stroke")
                    .then(|| decl.property_css())
            });
            let _ = write!(
                out,
                r#"<path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}"{} /></g>"#,
                escape_xml_display(&stroke_d),
                escape_attr_display(stroke),
                fmt_display(stroke_width as f64),
                escape_attr_display(stroke_dasharray),
                OptionalStyleAttr(&border_style),
            );

            return;
        }
    }

    let _ = write!(
        out,
        r#"<rect class="{}" style="{}" x="{}" y="{}" width="{}" height="{}" fill="{}" stroke="{}"/>"#,
        escape_xml_display(class_name),
        escape_attr_display(node_style),
        fmt_display(x),
        fmt_display(y),
        fmt_display(width),
        fmt_display(height),
        escape_attr_display(fill.unwrap_or("none")),
        escape_attr_display(stroke),
    );
}

pub(in crate::svg::parity::flowchart) fn render_swimlane_cluster(
    out: &mut impl crate::svg::parity::SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    cluster: &LayoutCluster,
    lane: &SwimlaneLaneLayout,
    origin_x: f64,
    origin_y: f64,
) -> crate::Result<()> {
    ctx.checkpoint_emit()?;
    if !super::is_lane_or_pool_cluster(ctx, lane.id.as_str()) {
        return Err(crate::Error::InvalidModel {
            message: format!("Swimlane shell `{}` is neither a lane nor a pool", lane.id),
        });
    }
    let subgraph = ctx.subgraphs_by_id.get(cluster.id.as_str()).copied();
    let subgraph_index = ctx.subgraph_indices_by_id.get(cluster.id.as_str()).copied();
    let class_names = subgraph
        .zip(subgraph_index)
        .map(|(subgraph, index)| ctx.model.effective_subgraph_css(index, subgraph).0)
        .unwrap_or_default();
    let terminal = ctx.prepared_nodes.cluster(cluster.id.as_str())?;
    let compiled = &terminal.compiled;
    let node_style = terminal.rect_style.as_str();
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
    let canonical = ctx.layout.route_contract.group_parents().is_some();
    let geometry = super::swimlane_terminal_geometry(lane, direction, canonical);
    let label_width = lane.title_label_width.max(0.0);
    let label_height = lane.title_label_height.max(0.0);
    let title_owner = ctx
        .svg_label_sidecar
        .and_then(|sidecar| sidecar.swimlane_group_title_owner(cluster.id.as_str()));
    let width = geometry.width;
    let (body, title) = geometry.frame_parts((lane.x, lane.y), direction, canonical);
    use crate::model::SwimlaneDirection::{Lr, Rl};
    let horizontal = direction == Lr || (canonical && direction == Rl);

    let lane_fill = terminal.fill.as_str();
    let lane_stroke = terminal.stroke.as_str();
    let is_pool = super::uses_pool_shell(ctx, lane.id.as_str());
    let body_class = if is_pool {
        "pool-body swimlane-body"
    } else {
        "swimlane-body"
    };
    let title_class = if is_pool {
        "pool-title swimlane-title"
    } else {
        "swimlane-title"
    };
    let label_class = if is_pool {
        "pool-label swimlane-label"
    } else {
        "swimlane-label"
    };
    let mut classes = String::from(if is_pool {
        "cluster pool swimlane"
    } else {
        "cluster swimlane"
    });
    for class in class_names {
        let class = class.trim();
        if !class.is_empty() {
            classes.push(' ');
            classes.push_str(class);
        }
    }
    let lane_dom_id =
        ctx.document_ids()
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
    let _ = write!(out, r#" data-look="{}""#, escape_xml_display(data_look));
    if let Some(color_slot) = lane_color_slot(ctx, &lane.id) {
        let _ = write!(out, r#" data-color-id="color-{color_slot}""#);
    }
    out.push('>');

    for (rect, class, fill) in [
        (body, body_class, None),
        (title, title_class, Some(lane_fill)),
    ] {
        write_swimlane_rect(
            out,
            ctx,
            compiled,
            class,
            node_style,
            rect.x0 + ctx.tx - origin_x,
            rect.y0 + ctx.ty - origin_y,
            rect.width().max(0.0),
            rect.height().max(0.0),
            fill,
            lane_stroke,
        );
    }

    let (label_x, label_y, label_transform) = if horizontal {
        let center_x = geometry.title_label.x + ctx.tx - origin_x;
        let center_y = geometry.title_label.y + ctx.ty - origin_y;
        (
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
        (
            geometry.title_label.x - label_width / 2.0 + ctx.tx - origin_x,
            geometry.title_label.y - label_height / 2.0 + ctx.ty - origin_y,
            String::new(),
        )
    };
    if ctx.swimlane_title_html_labels {
        let prepared_label = ctx
            .svg_label_sidecar
            .and_then(|sidecar| {
                sidecar
                    .swimlane_group_title_owner(cluster.id.as_str())
                    .map(|owner| sidecar.prepared_label_for_terminal(owner, render_title))
            })
            .unwrap_or_default();
        let title_html = flowchart_label_html_with_prepared_label(
            render_title,
            "markdown",
            ctx.config,
            ctx.math_renderer,
            prepared_label,
        );
        let transform = if horizontal {
            label_transform
        } else {
            format!(
                "translate({}, {})",
                fmt_display(label_x),
                fmt_display(label_y)
            )
        };
        let wrap_width = FLOWCHART_FIXED_LABEL_WRAP_WIDTH;
        let uses_wrapped_box = label_width >= wrap_width - 1e-3;
        let div_style = if uses_wrapped_box {
            format!(
                "display: table; white-space: break-spaces; line-height: 1.5; max-width: {mw}px; text-align: center; width: {mw}px;",
                mw = fmt_display(wrap_width),
            )
        } else {
            format!(
                "display: table-cell; white-space: nowrap; line-height: 1.5; max-width: {}px; text-align: center;",
                fmt_display(width.max(label_width)),
            )
        };
        let overflow_attr = if label_width > 0.0 || label_height > 0.0 {
            HTML_LABEL_FOREIGN_OBJECT_OVERFLOW_ATTR
        } else {
            ""
        };
        let _ = write!(
            out,
            r#"<g class="cluster-label {}" transform="{}"><foreignObject width="{}" height="{}"{}><div xmlns="http://www.w3.org/1999/xhtml" style="{}"><span class="nodeLabel"{}>{}</span></div></foreignObject></g>"#,
            label_class,
            escape_xml_display(&transform),
            fmt_display(label_width),
            fmt_display(label_height),
            overflow_attr,
            escape_attr_display(&div_style),
            OptionalStyleAttr(label_style),
            title_html,
        );
    } else {
        let transform = if horizontal {
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
            r#"<g class="cluster-label {}" transform="{}"><g><rect class="background" style="stroke: none"/>"#,
            label_class,
            escape_xml_display(&transform),
        );
        let title_text_style = ctx
            .svg_label_sidecar
            .zip(title_owner)
            .and_then(|(sidecar, owner)| sidecar.node_source_style(owner))
            .unwrap_or(&terminal.title_text_style)
            .as_ref();
        let mut svg_style = String::new();
        if canonical {
            svg_style.push_str(label_style);
            let default_ink = ctx.native.map(|native| native.on_emphasis.to_string());
            let ink = super::svg_label_ink(
                ctx,
                &lane.id,
                compiled,
                default_ink
                    .as_deref()
                    .unwrap_or(&ctx.compatibility.title_color),
                title_text_style,
            )?;
            if let Some(ink) = ink.as_deref() {
                super::super::label::append_native_label_color(&mut svg_style, ink);
            }
        }
        let prepared = crate::flowchart::FlowchartSvgLabelRenderPlan::new(
            ctx.svg_label_sidecar,
            title_owner,
            render_title,
            ctx.measurer,
            title_text_style,
            Some(FLOWCHART_FIXED_LABEL_WRAP_WIDTH),
            true,
            crate::flowchart::FlowchartSvgWidthMode::Bbox,
        );
        let zero_width_title = lane.width == 0.0;
        if zero_width_title {
            let rows = crate::flowchart::zero_width_lane_title_rows(render_title);
            crate::svg::parity::label::write_svg_markdown_word_lines_with_style(
                out,
                &rows,
                Some(""),
                crate::text::SvgTextEntityMode::DecodedModel,
            );
        } else if prepared.is_prepared() {
            if svg_style.is_empty() {
                write_flowchart_svg_label_plan(out, &prepared, true);
            } else {
                write_flowchart_svg_label_plan_with_style(out, &prepared, true, Some(&svg_style));
            }
        } else if canonical {
            write_flowchart_svg_text_markdown_wrapped_with_style(
                out,
                render_title,
                &svg_style,
                ctx.measurer,
                title_text_style,
                Some(FLOWCHART_FIXED_LABEL_WRAP_WIDTH),
            );
        } else {
            write_flowchart_svg_text_markdown(out, render_title, true);
        }
        out.push_str("</g></g>");
    }
    out.push_str("</g>");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::rough_style_from_node_style;

    #[test]
    fn rough_title_style_rewrites_only_the_property_and_keeps_quoted_css_intact() {
        let style = r#"fill:"R&amp;D;fill" !important;stroke:#123456"#;
        assert_eq!(
            rough_style_from_node_style(style, |decl| {
                (decl.property() == "fill").then_some("stroke")
            }),
            r#"stroke:"R&amp;D;fill" !important"#,
        );
    }
}
