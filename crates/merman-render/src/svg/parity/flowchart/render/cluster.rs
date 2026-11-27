//! Flowchart cluster renderer.

use super::super::*;
use crate::flowchart::FLOWCHART_FIXED_LABEL_WRAP_WIDTH;
use crate::svg::parity::flowchart::{
    cluster_geometry::painted_cluster_frame, util::HTML_LABEL_FOREIGN_OBJECT_OVERFLOW_ATTR,
};

const FLOWCHART_CLUSTER_HAND_DRAWN_ROUGHNESS: f32 = 0.7;
const FLOWCHART_CLUSTER_HAND_DRAWN_FILL_WEIGHT: f32 = 3.0;
const FLOWCHART_CLUSTER_HAND_DRAWN_HACHURE_GAP: f32 = 1.5;
const TITLE_HALO_WIDTH: u8 = 5;

pub(in crate::svg::parity::flowchart) fn advance_rough_geometry(
    ctx: &FlowchartRenderCtx<'_>,
    cluster: &LayoutCluster,
    origin_x: f64,
    origin_y: f64,
) -> crate::Result<()> {
    ctx.work_meter
        .checkpoint(merman_model::OperationPhase::Layout)
        .map_err(crate::Error::from)?;
    if ctx.compatibility.look.as_str() != "handDrawn"
        || ctx
            .swimlane_lanes_by_id
            .get(cluster.id.as_str())
            .is_some_and(|lane| lane.parent_id.is_none())
        || (!ctx.subgraph_has_children(cluster.id.as_str())
            && !super::flowchart_elk_renders_empty_subgraph_as_cluster(ctx))
    {
        return Ok(());
    }
    let terminal = ctx.prepared_nodes.cluster(cluster.id.as_str())?;
    let frame = painted_cluster_frame(ctx, cluster);
    let width = frame.size.width;
    let height = frame.size.height;
    let left = frame.origin.x + ctx.tx - origin_x;
    let top = frame.origin.y + ctx.ty - origin_y;
    let agentflow = ctx.diagram_type == "agentflow";
    if agentflow {
        let path = rounded_rect_path_d(left, top, width, height, 10.0);
        let _ = super::node::roughjs::roughjs_prepare_hand_drawn_stroke_path_for_svg_path(
            super::node::roughjs::RoughGeometryTarget::Query,
            &path,
            FLOWCHART_CLUSTER_HAND_DRAWN_ROUGHNESS,
            ctx.work_meter,
            &ctx.hand_drawn_seed,
        );
    } else {
        let path = rounded_rect_path_d(left, top, width, height, 0.0);
        let stroke_width = parse_css_px_f32(terminal.compiled.stroke_width.as_ref(), 1.3);
        let dash = terminal
            .compiled
            .stroke_dasharray
            .as_deref()
            .unwrap_or("0 0")
            .trim();
        let _ = super::node::roughjs::roughjs_prepare_hachure_paths_for_svg_path(
            super::node::roughjs::RoughGeometryTarget::Query,
            &path,
            stroke_width,
            dash,
            FLOWCHART_CLUSTER_HAND_DRAWN_FILL_WEIGHT,
            FLOWCHART_CLUSTER_HAND_DRAWN_HACHURE_GAP,
            FLOWCHART_CLUSTER_HAND_DRAWN_ROUGHNESS,
            ctx.work_meter,
            &ctx.hand_drawn_seed,
        );
    }
    ctx.work_meter
        .checkpoint(merman_model::OperationPhase::Layout)
        .map_err(crate::Error::from)
}

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

fn cluster_svg_text_style(label_style: &str) -> String {
    if label_style.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    let mut color_value: Option<&str> = None;
    let mut has_fill = false;
    for decl in label_style.split(';') {
        let decl = decl.trim();
        if decl.is_empty() {
            continue;
        }
        if let Some((key, val)) = decl.split_once(':') {
            let key_trim = key.trim();
            if key_trim.eq_ignore_ascii_case("fill") {
                has_fill = true;
            } else if key_trim.eq_ignore_ascii_case("color") {
                let val_trim = val.trim();
                if !val_trim.is_empty() {
                    color_value = Some(val_trim);
                }
            }
        }
        if !out.is_empty() {
            out.push(';');
        }
        out.push_str(decl);
    }
    if !has_fill && let Some(val) = color_value {
        if !out.is_empty() {
            out.push(';');
        }
        out.push_str("fill:");
        out.push_str(val);
    }
    out
}

fn cluster_base_fill<'a>(
    ctx: &'a FlowchartRenderCtx<'_>,
    terminal: &'a super::super::node_inventory::FlowchartPreparedCluster,
    id: &str,
) -> &'a str {
    if ctx.diagram_type == "agentflow"
        && let Some(slot) = super::super::agentflow::container_color_slot(ctx, id)
        && let Some(background) = ctx.palette.background_color(slot)
    {
        return background;
    }
    &terminal.fill
}

fn write_flowchart_cluster_shape(
    out: &mut impl crate::svg::parity::SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    terminal: &super::super::node_inventory::FlowchartPreparedCluster,
    id: &str,
    frame: super::super::cluster_geometry::PaintedClusterFrame,
) {
    let kurbo::Point { x: left, y: top } = frame.origin;
    let kurbo::Size {
        width: rect_w,
        height: rect_h,
    } = frame.size;
    let compiled_styles = &terminal.compiled;
    let rect_style = terminal.rect_style.as_str();
    let fill = cluster_base_fill(ctx, terminal, id);
    let stroke = terminal.stroke.as_str();
    let agentflow = ctx.diagram_type == "agentflow";
    let canonical = ctx.layout.route_contract.is_canonical_axis();
    if let Some(native) = ctx.native {
        let radius = native
            .form
            .container_radius
            .min(rect_w.min(rect_h) / 2.0)
            .max(0.0);
        let _ = write!(
            out,
            r#"<rect x="{}" y="{}" width="{}" height="{}" rx="{}" style="fill:{};stroke:{};stroke-width:{}px"#,
            fmt_display(left),
            fmt_display(top),
            fmt_display(rect_w),
            fmt_display(rect_h),
            fmt_display(radius),
            escape_attr(fill),
            escape_attr(stroke),
            fmt_display(native.form.stroke_width),
        );
        // The native defaults are CSS tokens. Only source declarations need Mermaid entity recovery.
        if !rect_style.is_empty() {
            let _ = write!(out, ";{}", escape_xml_display(rect_style));
        }
        out.push_str(r#""/>"#);
        return;
    }
    if agentflow {
        let stroke = &ctx.compatibility.agentflow_container_stroke;
        if ctx.compatibility.look.as_str() == "handDrawn" {
            let path = rounded_rect_path_d(left, top, rect_w, rect_h, 10.0);
            if let Some(stroke_d) =
                super::node::roughjs::roughjs_hand_drawn_stroke_path_for_svg_path(
                    &path,
                    FLOWCHART_CLUSTER_HAND_DRAWN_ROUGHNESS,
                    ctx.work_meter,
                    &ctx.hand_drawn_seed,
                )
            {
                let stroke_dasharray = compiled_styles.stroke_dasharray.as_deref().unwrap_or("0 0");
                let _ = write!(
                    out,
                    r#"<g><path d="{}" stroke="{}" stroke-width="0.75" fill="none" stroke-dasharray="{}"/></g>"#,
                    escape_xml_display(&stroke_d),
                    escape_xml_display(stroke),
                    escape_xml_display(stroke_dasharray),
                );

                return;
            }
        }
        let _ = write!(
            out,
            r#"<rect rx="10" ry="10" x="{}" y="{}" width="{}" height="{}" fill="none" stroke="{}" stroke-width="0.75px""#,
            fmt_display(left),
            fmt_display(top),
            fmt_display(rect_w),
            fmt_display(rect_h),
            escape_xml_display(stroke),
        );
        if canonical {
            let fill = compiled_styles.fill.as_deref().unwrap_or(fill);
            let _ = write!(
                out,
                r#" style="fill:{};fill-opacity:{}""#,
                escape_attr(fill),
                fmt_display(compiled_styles.fill_opacity),
            );
        }
        out.push_str("/>");
        return;
    }

    if ctx.compatibility.look.as_str() == "handDrawn" {
        let stroke_width = parse_css_px_f32(compiled_styles.stroke_width.as_ref(), 1.3);
        let stroke_dasharray = compiled_styles
            .stroke_dasharray
            .as_deref()
            .unwrap_or("0 0")
            .trim();
        let path = rounded_rect_path_d(left, top, rect_w, rect_h, 0.0);

        if let Some((fill_d, stroke_d)) = super::node::roughjs::roughjs_hachure_paths_for_svg_path(
            &path,
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
                r#"<g><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="0 0"{} /><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}"{} /></g>"#,
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

            return;
        }
    }

    out.push_str(r#"<rect style=""#);
    if canonical && ctx.palette.color_count() > 0 {
        // The compatibility surface must agree in browsers and raster exports. The browser's
        // palette selector can otherwise override the clusterBkg fill used during export.
        let _ = write!(out, "fill:{};", escape_attr(fill));
    }
    let _ = write!(
        out,
        r#"{}" x="{}" y="{}" width="{}" height="{}"/>"#,
        escape_xml_display(rect_style),
        fmt_display(left),
        fmt_display(top),
        fmt_display(rect_w),
        fmt_display(rect_h)
    );
}

fn title_halo_color(ctx: &FlowchartRenderCtx<'_>, id: &str) -> crate::Result<crate::theme::Color> {
    use crate::theme::{Color, edge_label_bg::literal_fill};

    let canvas = ctx.native.map(|native| native.canvas).or_else(|| {
        ctx.config
            .get_str("themeVariables.background")
            .and_then(literal_fill)
    });
    let mut surface = Color::TRANSPARENT;
    let mut current = Some(id);
    while let Some(id) = current {
        ctx.work_meter.charge(1)?;
        let terminal = ctx.prepared_nodes.cluster(id)?;
        let fill = terminal
            .compiled
            .fill
            .as_deref()
            .and_then(literal_fill)
            .or_else(|| literal_fill(cluster_base_fill(ctx, terminal, id)))
            .unwrap_or(Color::TRANSPARENT);
        let layer = fill.composite(Color::TRANSPARENT, terminal.compiled.fill_opacity);
        surface = surface.composite(layer, 1.0);
        if surface.channels()[3] == u8::MAX {
            return Ok(surface);
        }
        current = ctx.parent.get(id).copied();
    }
    let white = Color::rgb(255, 255, 255);
    Ok(surface.composite(canvas.unwrap_or(white).composite(white, 1.0), 1.0))
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::svg::parity::flowchart) enum FlowchartClusterPass {
    Complete,
    Background,
    Label,
}

pub(in crate::svg::parity) fn render_flowchart_cluster(
    out: &mut impl crate::svg::parity::SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    cluster: &LayoutCluster,
    origin_x: f64,
    origin_y: f64,
) -> crate::Result<()> {
    render_flowchart_cluster_pass(
        out,
        ctx,
        cluster,
        origin_x,
        origin_y,
        FlowchartClusterPass::Complete,
    )
}

pub(in crate::svg::parity::flowchart) fn render_flowchart_cluster_pass(
    out: &mut impl crate::svg::parity::SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    cluster: &LayoutCluster,
    origin_x: f64,
    origin_y: f64,
    pass: FlowchartClusterPass,
) -> crate::Result<()> {
    if let Some(lane) = ctx.swimlane_lanes_by_id.get(cluster.id.as_str())
        && super::super::swimlane::is_lane_or_pool_cluster(ctx, cluster.id.as_str())
    {
        if pass == FlowchartClusterPass::Label {
            return Ok(());
        }
        super::super::swimlane::render_swimlane_cluster(
            out, ctx, cluster, lane, origin_x, origin_y,
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

    let terminal = ctx.prepared_nodes.cluster(cluster.id.as_str())?;
    let compiled_styles = &terminal.compiled;
    let label_style = compiled_styles.label_style.trim();
    let classes = ctx.model.effective_subgraph_css(subgraph_index, sg).0;

    let elk_geometry = ctx.elk_cluster_geometry.get(cluster.id.as_str());
    let mut frame = painted_cluster_frame(ctx, cluster);
    frame.origin.x = frame.origin.x + ctx.tx - origin_x;
    frame.origin.y = frame.origin.y + ctx.ty - origin_y;
    let Some(cluster_dom_id) = ctx.document_ids().cluster(cluster.id.as_str()) else {
        return Err(crate::Error::InvalidModel {
            message: format!(
                "missing prepared Flowchart DOM id for cluster `{}`",
                cluster.id
            ),
        });
    };
    ctx.checkpoint_emit()?;

    let label_type = sg.label_type.as_deref().unwrap_or("text");
    let render_title = ctx.model.subgraph_title_for_render(subgraph_index, sg);
    let title_owner = ctx
        .svg_label_sidecar
        .and_then(|sidecar| sidecar.subgraph_title_owner(cluster.id.as_str()));
    let title_text_style = terminal.title_text_style.as_ref();
    // ELK paints Markdown after the final frame is known. clusters.js passes node.width to
    // createText, so these paint metrics differ from the wrapped pre-layout placeholder.
    let markdown_wrap_width = if ctx.uses_elk_adapter_dom {
        cluster.width.max(1.0)
    } else {
        FLOWCHART_FIXED_LABEL_WRAP_WIDTH
    };
    let painted_markdown = elk_geometry.map(|geometry| geometry.label).or_else(|| {
        (ctx.uses_elk_adapter_dom && label_type == "markdown").then(|| {
            crate::flowchart::flowchart_label_metrics_for_layout(
                crate::flowchart::FlowchartLabelMetricsRequest {
                    measurer: ctx.measurer,
                    raw_label: render_title,
                    label_type,
                    style: title_text_style,
                    max_width_px: Some(markdown_wrap_width),
                    wrap_mode: ctx.edge_wrap_mode,
                    config: ctx.config,
                    math_renderer: ctx.math_renderer,
                },
            )
        })
    });
    let label_w = painted_markdown
        .map_or(cluster.title_label.width, |metrics| metrics.width)
        .max(0.0);
    let label_h = painted_markdown
        .map_or(cluster.title_label.height, |metrics| metrics.height)
        .max(0.0);
    let (label_left, label_top) = if ctx.layout.route_contract.group_parents().is_some() {
        (
            cluster.title_label.x + ctx.tx - origin_x - label_w / 2.0,
            cluster.title_label.y + ctx.ty - origin_y - label_h / 2.0,
        )
    } else {
        (
            cluster.x + ctx.tx - origin_x - label_w / 2.0,
            frame.origin.y + cluster.title_margin_top.max(0.0),
        )
    };

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
    if ctx.diagram_type == "agentflow" {
        class_attr.push_str(" flow-cluster");
    }
    let color_attr = super::super::agentflow::container_color_slot(ctx, &cluster.id)
        .map(|slot| format!(r#" data-color-id="color-{slot}""#))
        .unwrap_or_default();
    let data_look = ctx.compatibility.look.as_str();
    let struck = ctx.layout.route_contract.group_title_is_struck(&cluster.id);

    if pass == FlowchartClusterPass::Background {
        let _ = write!(
            out,
            r#"<g class="{}" data-merman-theme-depth="container" data-look="{}"{color_attr}"#,
            escape_attr(&class_attr),
            escape_attr(data_look),
        );
        if struck {
            let _ = write!(
                out,
                r#" id="{cluster_dom_id}" data-id="{}" data-et="cluster""#,
                escape_xml_display(&cluster.id),
            );
        }
        out.push('>');
        write_flowchart_cluster_shape(out, ctx, terminal, &cluster.id, frame);
        out.push_str("</g>");
        return out.checkpoint();
    }

    let halo = struck
        .then(|| title_halo_color(ctx, &cluster.id))
        .transpose()?;
    let overlay = struck && pass == FlowchartClusterPass::Label;
    let _ = write!(out, r#"<g class="{}""#, escape_xml_display(&class_attr));
    if !overlay {
        let _ = write!(
            out,
            r#" id="{cluster_dom_id}" data-id="{}" data-et="cluster""#,
            escape_xml_display(&cluster.id),
        );
    }
    let _ = write!(
        out,
        r#" data-look="{}"{color_attr}>"#,
        escape_xml_display(data_look),
    );
    if pass == FlowchartClusterPass::Complete {
        write_flowchart_cluster_shape(out, ctx, terminal, &cluster.id, frame);
    }
    let _ = write!(
        out,
        r#"<g class="cluster-label" transform="translate({},{})""#,
        fmt_display(label_left),
        fmt_display(label_top),
    );
    if overlay {
        let _ = write!(out, r#" data-merman-cluster-title-for="{cluster_dom_id}""#);
    }
    if let Some(color) = halo {
        let _ = write!(
            out,
            r#" data-merman-title-struck="true" stroke="{color}" stroke-width="{TITLE_HALO_WIDTH}" stroke-linejoin="round" paint-order="stroke""#,
        );
    }
    out.push('>');
    // A native text branch keeps the title and its outline when raster export strips HTML.
    let mut svg_fallback = false;
    if ctx.edge_html_labels {
        let prepared_label = ctx
            .svg_label_sidecar
            .zip(title_owner)
            .map_or(Default::default(), |(sidecar, owner)| {
                sidecar.prepared_label_for_terminal(owner, render_title)
            });
        let title_html = flowchart_label_html_with_prepared_label(
            render_title,
            label_type,
            ctx.config,
            ctx.math_renderer,
            prepared_label,
        );
        svg_fallback = halo.is_some()
            && !matches!(
                prepared_label,
                crate::flowchart::FlowchartPreparedLabelResolution::PreparedMath(_)
            );
        if svg_fallback {
            out.push_str("<switch>");
        }
        let halo_style = halo.map(|color| {
            if label_style.is_empty() {
                format!("-webkit-text-stroke:{TITLE_HALO_WIDTH}px {color};paint-order:stroke fill")
            } else {
                format!(
                    "{label_style};-webkit-text-stroke:{TITLE_HALO_WIDTH}px {color};paint-order:stroke fill"
                )
            }
        });
        // CSS color hex tokens like `#1F2335;` must not run through Mermaid `#entity;` text
        // decoding (which turns `#1F2335;` into `&amp;1F2335;` and drops the browser halo).
        let span_style_attr = OptionalStyleAttr(halo_style.as_deref().unwrap_or(label_style));
        // Min-content expansion does not undo the wrap selected by the first measurement.
        let markdown_uses_wrapped_box = label_w >= markdown_wrap_width - 1e-3;
        let div_style = if label_type != "markdown" {
            "display: table-cell; white-space: nowrap; line-height: 1.5;".to_string()
        } else if markdown_uses_wrapped_box {
            format!(
                "display: table; white-space: break-spaces; line-height: 1.5; max-width: {mw}px; text-align: center; width: {mw}px;",
                mw = fmt_display(markdown_wrap_width)
            )
        } else {
            format!(
                "display: table-cell; white-space: nowrap; line-height: 1.5; max-width: {mw}px; text-align: center;",
                mw = fmt_display(markdown_wrap_width)
            )
        };
        let _ = write!(
            out,
            r#"<foreignObject width="{}" height="{}"{}><div xmlns="http://www.w3.org/1999/xhtml" style="{}"><span class="nodeLabel"{}>{}</span></div></foreignObject>"#,
            fmt_display(label_w),
            fmt_display(label_h),
            HTML_LABEL_FOREIGN_OBJECT_OVERFLOW_ATTR,
            escape_xml_display(&div_style),
            span_style_attr,
            title_html
        );
    }
    if !ctx.edge_html_labels || svg_fallback {
        let svg_text_style = cluster_svg_text_style(label_style);
        out.push_str(r#"<g><rect class="background" style="stroke: none"/>"#);
        if label_type == "markdown" {
            if ctx.uses_elk_adapter_dom || svg_fallback {
                if !svg_text_style.is_empty() {
                    write_flowchart_svg_text_markdown_wrapped_with_style(
                        out,
                        render_title,
                        &svg_text_style,
                        ctx.measurer,
                        title_text_style,
                        Some(markdown_wrap_width),
                    );
                } else {
                    write_flowchart_svg_text_markdown_wrapped(
                        out,
                        render_title,
                        true,
                        ctx.measurer,
                        title_text_style,
                        Some(markdown_wrap_width),
                    );
                }
            } else {
                write_flowchart_svg_text_markdown(out, render_title, true);
            }
        } else {
            let prepared = crate::flowchart::FlowchartSvgLabelRenderPlan::new(
                ctx.svg_label_sidecar,
                title_owner,
                render_title,
                ctx.measurer,
                title_text_style,
                None,
                true,
                crate::flowchart::FlowchartSvgWidthMode::Bbox,
            );
            if ctx.layout.route_contract.group_parents().is_some()
                && matches!(label_type, "text" | "string")
            {
                crate::svg::parity::label::write_measured_svg_title(
                    out,
                    &prepared.wrapped_lines(),
                    &svg_text_style,
                    None,
                    cluster.title_label.width,
                );
            } else {
                write_flowchart_svg_label_plan_with_style(
                    out,
                    &prepared,
                    true,
                    (!svg_text_style.is_empty()).then_some(svg_text_style.as_str()),
                );
            }
        }
        out.push_str("</g>");
        if svg_fallback {
            out.push_str("</switch>");
        }
    }
    out.push_str("</g></g>");
    Ok(())
}
