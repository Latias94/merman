//! Flowchart cluster renderer.

use super::super::*;
use crate::svg::parity::flowchart::util::HTML_LABEL_FOREIGN_OBJECT_OVERFLOW_ATTR;
use std::borrow::Cow;

const FLOWCHART_CLUSTER_TITLE_WRAP_WIDTH: f64 = 200.0;
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

fn flowchart_hand_drawn_seed(ctx: &FlowchartRenderCtx<'_>) -> u64 {
    ctx.config
        .as_value()
        .get("handDrawnSeed")
        .and_then(|v| v.as_u64())
        .unwrap_or(0)
}

#[allow(clippy::too_many_arguments)]
fn write_flowchart_cluster_shape(
    out: &mut String,
    ctx: &FlowchartRenderCtx<'_>,
    compiled_styles: &FlowchartCompiledStyles,
    rect_style: &str,
    left: f64,
    top: f64,
    rect_w: f64,
    rect_h: f64,
) {
    if flowchart_config_look(ctx.config) == "handDrawn" {
        let theme = PresentationTheme::new(ctx.config.as_value()).node_diagram();
        let fill = theme.cluster_bkg.as_str();
        let stroke = theme.cluster_border.as_str();
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
            flowchart_hand_drawn_seed(ctx),
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

    let _ = write!(
        out,
        r#"<rect style="{}" x="{}" y="{}" width="{}" height="{}"/>"#,
        escape_xml_display(rect_style),
        fmt_display(left),
        fmt_display(top),
        fmt_display(rect_w),
        fmt_display(rect_h)
    );
}

pub(in crate::svg::parity) fn render_flowchart_cluster(
    out: &mut String,
    ctx: &FlowchartRenderCtx<'_>,
    cluster: &LayoutCluster,
    origin_x: f64,
    origin_y: f64,
) {
    let Some(sg) = ctx.subgraphs_by_id.get(cluster.id.as_str()) else {
        return;
    };
    if sg.nodes.is_empty() && !super::flowchart_elk_renders_empty_subgraph_as_cluster(ctx) {
        return;
    }

    let compiled_styles = flowchart_compile_styles(ctx.class_defs, &sg.classes, &sg.styles, &[]);
    let rect_style = compiled_styles.node_style.trim();
    let label_style = compiled_styles.label_style.trim();

    let left = (cluster.x - cluster.width / 2.0) + ctx.tx - origin_x;
    let top = (cluster.y - cluster.height / 2.0) + ctx.ty - origin_y;
    let rect_w = cluster.width.max(1.0);
    let rect_h = cluster.height.max(1.0);
    let label_top = top + cluster.title_margin_top.max(0.0);
    let cluster_dom_id = if ctx.source_ported_elk_rendering {
        Cow::Borrowed("[object Object]")
    } else {
        Cow::Owned(format!("{}-{}", ctx.diagram_id, cluster.id))
    };

    let label_type = sg.label_type.as_deref().unwrap_or("text");

    let mut class_attr = String::new();
    for c in &sg.classes {
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
        let title_text = flowchart_label_plain_text(&cluster.title, label_type, false);
        let _ = write!(
            out,
            r#"<g class="{}" id="{}" data-look="{}">"#,
            escape_xml_display(&class_attr),
            escape_xml_display(&cluster_dom_id),
            escape_xml_display(data_look),
        );
        write_flowchart_cluster_shape(
            out,
            ctx,
            &compiled_styles,
            rect_style,
            left,
            top,
            rect_w,
            rect_h,
        );
        let _ = write!(
            out,
            r#"<g class="cluster-label" transform="translate({},{})"><g><rect class="background" style="stroke: none"/>"#,
            fmt_display(label_left),
            fmt_display(label_top)
        );
        if label_type == "markdown" {
            write_flowchart_svg_text_markdown(out, &cluster.title, true);
        } else {
            write_flowchart_svg_text(out, &title_text, true);
        }
        out.push_str("</g></g></g>");
        return;
    }

    let title_html =
        flowchart_label_html(&cluster.title, label_type, ctx.config, ctx.math_renderer);
    let label_w = cluster.title_label.width.max(0.0);
    let label_h = cluster.title_label.height.max(0.0);
    let label_left = left + rect_w / 2.0 - label_w / 2.0;

    let span_style_attr = OptionalStyleXmlAttr(label_style);
    let div_style = if label_type != "markdown" {
        "display: table-cell; white-space: nowrap; line-height: 1.5;".to_string()
    } else if label_w >= FLOWCHART_CLUSTER_TITLE_WRAP_WIDTH - 1e-3 {
        format!(
            "display: table; white-space: break-spaces; line-height: 1.5; max-width: {mw}px; text-align: center; width: {mw}px;",
            mw = fmt_display(FLOWCHART_CLUSTER_TITLE_WRAP_WIDTH)
        )
    } else {
        format!(
            "display: table-cell; white-space: nowrap; line-height: 1.5; max-width: {mw}px; text-align: center;",
            mw = fmt_display(FLOWCHART_CLUSTER_TITLE_WRAP_WIDTH)
        )
    };

    let _ = write!(
        out,
        r#"<g class="{}" id="{}" data-look="{}">"#,
        escape_xml_display(&class_attr),
        escape_xml_display(&cluster_dom_id),
        escape_xml_display(data_look),
    );
    write_flowchart_cluster_shape(
        out,
        ctx,
        &compiled_styles,
        rect_style,
        left,
        top,
        rect_w,
        rect_h,
    );
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
}
