//! Measured `labelRect` helpers in the common Grid renderer's node layer.

use std::fmt::Write as _;

use merman_render_core::{
    Error, LayoutLabel, LayoutPoint, Result,
    html::HTML_LABEL_FOREIGN_OBJECT_OVERFLOW_ATTR,
    mermaid_style::visit_parsed_style_declarations,
    number_format::canonical_number as fmt,
    svg_output::SvgOutput,
    text::literal_label::FlowchartSvgWidthMode,
    theme_svg::{
        cssom_color_value,
        edge_label_background::{adaptive_text_ink, literal_fill},
    },
    xml::escape_attr_display,
};

use crate::{
    graph::FlowchartHierarchyPlan,
    projection::FlowchartRenderEdgeRef,
    style::{compiled::FlowchartCompiledStyles, first_label_rect_style},
    svg::FlowchartRenderCtx,
    svg_label::{
        FlowchartSvgLabelOwner, FlowchartSvgLabelRenderPlan, OptionalStyleAttr,
        flowchart_label_html_with_prepared_label, write_flowchart_svg_label_plan_with_style,
    },
};

use super::edge_label::render_native_label_background;

pub(super) fn write_grid_edge_label_nodes(
    out: &mut impl SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    edges: &[FlowchartRenderEdgeRef<'_>],
    hierarchy: &FlowchartHierarchyPlan<'_>,
) -> Result<()> {
    ctx.work_meter.charge_emit_work(edges.len())?;
    for &edge in edges {
        ctx.checkpoint_emit()?;
        if edge.key.self_loop_segment_order().is_some() {
            continue;
        }
        let Some(label) = ctx
            .layout_edges_by_key
            .get(&edge.key)
            .and_then(|layout_edge| layout_edge.label())
        else {
            continue;
        };
        let Some(source) = ctx
            .model
            .edge_label_for_occurrence(edge.key, edge.edge)
            .filter(|source| !source.is_empty())
        else {
            continue;
        };
        write_grid_edge_label_node(out, ctx, edge, label, source, hierarchy)?;
        out.checkpoint()?;
        ctx.checkpoint_emit()?;
    }
    Ok(())
}

fn write_grid_edge_label_node(
    out: &mut impl SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    edge: FlowchartRenderEdgeRef<'_>,
    label: &LayoutLabel,
    source: &str,
    hierarchy: &FlowchartHierarchyPlan<'_>,
) -> Result<()> {
    let id = ctx
        .document_ids()
        .edge(edge.key)
        .ok_or_else(|| Error::InvalidModel {
            message: format!(
                "missing prepared Grid edge-label DOM owner for `{}`",
                edge.id
            ),
        })?;
    let position = LayoutPoint {
        x: label.x + ctx.tx,
        y: label.y + ctx.ty,
    };
    // The helper has node-label settings and a plain labelType. Upstream copies just the first
    // default or linkStyle declaration from the edge's label style array.
    let owner = FlowchartSvgLabelOwner::Edge(edge.key.semantic_index());
    let first_style =
        first_label_rect_style(ctx.default_edge_style, &edge.style).map_or("", String::as_str);
    ctx.work_meter.charge_emit_work(
        first_style.len().div_ceil(64) + source.len().div_ceil(64) + edge.id.len().div_ceil(64),
    )?;
    let mut style = GridLabelStyle::from_source(first_style, ctx.node_html_labels);
    let backdrop = render_native_label_background(
        out,
        ctx,
        hierarchy,
        position.clone(),
        (0.0, 0.0),
        (label.width, label.height),
    )?;
    if let Some(backdrop) = backdrop
        && !style.has_source_ink
        && !ctx
            .edge_style_plan
            .label_rect_for(edge.key)?
            .is_some_and(FlowchartCompiledStyles::has_source_label_foreground)
        && let Some(ink) = literal_fill(&ctx.compatibility.text_color)
            .and_then(|ink| adaptive_text_ink(backdrop, ink))
    {
        style.add_adaptive_ink(ctx.node_html_labels, ink);
    }
    let _ = write!(
        out,
        r#"<g class="label edgeLabel" id="{id}-label" data-id="{}" data-et="edge-label" transform="translate({}, {})"><rect width="0.1" height="0.1"/>"#,
        escape_attr_display(&edge.id),
        fmt(position.x),
        fmt(position.y),
    );
    out.checkpoint()?;

    if ctx.node_html_labels {
        let prepared = ctx.svg_label_sidecar.map_or(Default::default(), |sidecar| {
            sidecar.prepared_label_for_terminal(owner, source)
        });
        let rendered = flowchart_label_html_with_prepared_label(
            source,
            "text",
            ctx.config,
            ctx.math_renderer,
            prepared,
        );
        // A wrapped helper retains its measured box. The node wrap limit only styles the div.
        let wrapping_width = fmt(ctx.wrapping_width);
        let wraps = label.width == ctx.wrapping_width;
        let (display, white_space) = if wraps {
            ("table", "break-spaces")
        } else {
            ("table-cell", "nowrap")
        };
        let _ = write!(
            style.div,
            "display: {display}; white-space: {white_space}; line-height: 1.5; max-width: {wrapping_width}px; text-align: center;"
        );
        if wraps {
            let _ = write!(style.div, " width: {wrapping_width}px;");
        }
        write_inner_frame(out, label, &style.group, 0.0);
        let _ = write!(
            out,
            r#"<foreignObject width="{}" height="{}"{}><div xmlns="http://www.w3.org/1999/xhtml" style="{}"><span class="nodeLabel"{}>{}</span></div></foreignObject>"#,
            fmt(label.width),
            fmt(label.height),
            HTML_LABEL_FOREIGN_OBJECT_OVERFLOW_ATTR,
            escape_attr_display(&style.div),
            OptionalStyleAttr(&style.text),
            rendered,
        );
    } else {
        let wrap_style = ctx
            .edge_style_plan
            .grid_label_rect_svg_wrap_style(edge.key, ctx.text_style)?;
        let metrics_style = ctx
            .edge_style_plan
            .grid_label_rect_svg_bbox_style(edge.key, &wrap_style)?;
        let prepared = FlowchartSvgLabelRenderPlan::new(
            ctx.svg_label_sidecar,
            Some(owner),
            source,
            ctx.measurer,
            &wrap_style,
            Some(ctx.wrapping_width),
            true,
            FlowchartSvgWidthMode::Bbox,
        );
        let bbox_top = ctx
            .measurer
            .measure_svg_create_text_bbox_y_offset_px(prepared.plain_text(), &metrics_style);
        write_inner_frame(out, label, &style.group, bbox_top);
        out.push_str(r#"<g><rect class="background" style="stroke: none"/>"#);
        write_flowchart_svg_label_plan_with_style(out, &prepared, true, Some(&style.text));
        out.push_str("</g>");
    }
    out.push_str("</g></g>");
    Ok(())
}

fn write_inner_frame(out: &mut impl SvgOutput, label: &LayoutLabel, style: &str, bbox_top: f64) {
    let _ = write!(
        out,
        r#"<g class="label"{} transform="translate({}, {})"><rect/>"#,
        OptionalStyleAttr(style),
        fmt(-label.width / 2.0),
        fmt(-label.height / 2.0 - bbox_top),
    );
}

#[derive(Default)]
struct GridLabelStyle {
    group: String,
    text: String,
    div: String,
    has_source_ink: bool,
}

impl GridLabelStyle {
    fn from_source(source: &str, html: bool) -> Self {
        let mut style = Self::default();
        let mut mapped_fill = false;
        visit_parsed_style_declarations(source, |decl| {
            let property = decl.property();
            let value = decl.source_value();
            append_declaration(&mut style.group, decl.property_css(), value);
            if html {
                let css = if property == "fill" && !mapped_fill {
                    mapped_fill = true;
                    "color"
                } else {
                    decl.property_css()
                };
                append_declaration(&mut style.text, css, value);
                if property == "color" || css == "color" {
                    style.has_source_ink = true;
                    let color = cssom_color_value(decl.value());
                    let priority = if decl.important() { " !important" } else { "" };
                    let _ = write!(style.div, "color: {color}{priority}; ");
                } else if !matches!(
                    property,
                    "display" | "white-space" | "line-height" | "max-width" | "text-align"
                ) {
                    let _ = write!(style.div, "{css}: {value}; ");
                }
            } else {
                let css = match property {
                    "stroke" | "stroke-width" | "fill" => return,
                    "color" => {
                        style.has_source_ink = true;
                        "fill"
                    }
                    _ => decl.property_css(),
                };
                append_declaration(&mut style.text, css, value);
            }
        });
        style
    }

    fn add_adaptive_ink(&mut self, html: bool, ink: &str) {
        if html {
            if !self.text.is_empty() {
                self.text.push(';');
            }
            let _ = write!(self.text, "color:{ink} !important");
            let _ = write!(self.div, "color: {ink} !important; ");
        } else {
            append_declaration(&mut self.text, "fill", ink);
        }
    }
}

fn append_declaration(style: &mut String, property: &str, value: &str) {
    if !style.is_empty() {
        style.push(';');
    }
    let _ = write!(style, "{property}:{value}");
}

#[cfg(all(test, feature = "diagram-flowchart"))]
mod tests;
