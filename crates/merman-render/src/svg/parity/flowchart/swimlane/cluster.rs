use super::super::*;
use crate::flowchart::{FlowchartLabelMetricsRequest, flowchart_label_metrics_for_layout};
use crate::model::{SwimlaneDirection, SwimlaneLaneLayout};

fn lane_label_metrics(
    ctx: &FlowchartRenderCtx<'_>,
    lane: &SwimlaneLaneLayout,
    label_type: &str,
) -> crate::text::TextMetrics {
    if lane.title.is_empty() {
        return crate::text::TextMetrics {
            width: 0.0,
            height: 0.0,
            line_count: 0,
        };
    }

    let style = if ctx.edge_html_labels {
        &ctx.html_label_text_style
    } else {
        &ctx.text_style
    };
    let wrap_mode = if ctx.edge_html_labels {
        WrapMode::HtmlLike
    } else {
        WrapMode::SvgLike
    };
    let mut metrics = flowchart_label_metrics_for_layout(FlowchartLabelMetricsRequest {
        measurer: ctx.measurer,
        raw_label: &lane.title,
        label_type,
        style,
        max_width_px: Some(lane.width.max(1.0)),
        wrap_mode,
        config: ctx.config,
        math_renderer: ctx.math_renderer,
    });

    let plain = flowchart_label_plain_text(&lane.title, label_type, ctx.edge_html_labels);
    if label_type != "markdown" && !lane.title.contains('<') && !lane.title.contains("$$") {
        if ctx.edge_html_labels {
            metrics.width = ctx
                .measurer
                .measure_svg_text_bounding_client_rect_width_px(&plain, style)
                .max(0.0);
        } else {
            metrics.width = ctx
                .measurer
                .measure_svg_tspan_text_bbox_width_px(&plain, style)
                .max(0.0);
            metrics.height = ctx
                .measurer
                .measure_svg_tspan_text_bbox_height_px(&plain, style)
                .max(0.0);
        }
    }
    metrics
}

pub(in crate::svg::parity::flowchart) fn render_swimlane_cluster(
    out: &mut String,
    ctx: &FlowchartRenderCtx<'_>,
    cluster: &LayoutCluster,
    lane: &SwimlaneLaneLayout,
    origin_x: f64,
    origin_y: f64,
) {
    let subgraph = ctx.subgraphs_by_id.get(cluster.id.as_str()).copied();
    let class_names = subgraph.map_or(&[][..], |subgraph| subgraph.classes.as_slice());
    let styles = subgraph.map_or(&[][..], |subgraph| subgraph.styles.as_slice());
    let compiled = flowchart_compile_styles(ctx.class_defs, class_names, styles, &[]);
    let node_style = compiled.node_style.trim();
    let label_style = compiled.label_style.trim();
    let label_type = subgraph
        .and_then(|subgraph| subgraph.label_type.as_deref())
        .unwrap_or("text");
    let label_metrics = lane_label_metrics(ctx, lane, label_type);
    let label_width = label_metrics.width.max(0.0);
    let label_height = label_metrics.height.max(0.0);

    let padding = lane.padding.max(0.0);
    let width = lane.width.max(label_width + padding);
    let height = lane.height.max(0.0);
    let lane_top = lane.y - height / 2.0 + ctx.ty - origin_y;
    let lane_bottom = lane.y + height / 2.0 + ctx.ty - origin_y;
    let lane_left = lane.x - width / 2.0 + ctx.tx - origin_x;
    let content_top = lane
        .content_top
        .map(|value| value + ctx.ty - origin_y)
        .unwrap_or(lane_top + height / 3.0);
    let is_lr = ctx.swimlane_direction == Some(SwimlaneDirection::Lr);
    let title_padding_y = if is_lr { 4.0 } else { 0.0 };
    let desired_title_size = label_height + 2.0 * title_padding_y;

    let theme = PresentationTheme::new(ctx.config.as_value()).node_diagram();
    let mut classes = String::from("cluster swimlane");
    for class in class_names {
        let class = class.trim();
        if !class.is_empty() {
            classes.push(' ');
            classes.push_str(class);
        }
    }
    let _ = write!(
        out,
        r#"<g class="{}" id="{}" data-id="{}" data-et="cluster""#,
        escape_xml_display(&classes),
        escape_xml_display(&lane.id),
        escape_xml_display(&lane.id),
    );
    if subgraph.is_some() {
        let _ = write!(
            out,
            r#" data-look="{}""#,
            escape_xml_display(flowchart_config_look(ctx.config)),
        );
    }
    out.push('>');

    let (label_x, label_y, label_transform) = if is_lr {
        let title_width = desired_title_size.max(label_height + 2.0 * title_padding_y);
        let body_x = lane_left + title_width;
        let body_width = (width - title_width).max(0.0);
        let _ = write!(
            out,
            r#"<rect class="swimlane-body" style="{}" x="{}" y="{}" width="{}" height="{}" fill="none" stroke="{}"/>"#,
            escape_xml_display(node_style),
            fmt_display(body_x),
            fmt_display(lane_top),
            fmt_display(body_width),
            fmt_display(height),
            escape_xml_display(&theme.cluster_border),
        );
        let _ = write!(
            out,
            r#"<rect class="swimlane-title" style="{}" x="{}" y="{}" width="{}" height="{}" fill="{}" stroke="{}"/>"#,
            escape_xml_display(node_style),
            fmt_display(lane_left),
            fmt_display(lane_top),
            fmt_display(title_width),
            fmt_display(height),
            escape_xml_display(&theme.cluster_bkg),
            escape_xml_display(&theme.cluster_border),
        );
        let center_x = lane_left + title_width / 2.0;
        let center_y = lane.y + ctx.ty - origin_y;
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
        let header_max_height = (content_top - lane_top).max(0.0);
        let title_height = desired_title_size.min(header_max_height);
        let body_y = lane_top + title_height;
        let body_height = (lane_bottom - body_y).max(0.0);
        let _ = write!(
            out,
            r#"<rect class="swimlane-body" style="{}" x="{}" y="{}" width="{}" height="{}" fill="none" stroke="{}"/>"#,
            escape_xml_display(node_style),
            fmt_display(lane_left),
            fmt_display(body_y),
            fmt_display(width),
            fmt_display(body_height),
            escape_xml_display(&theme.cluster_border),
        );
        let _ = write!(
            out,
            r#"<rect class="swimlane-title" style="{}" x="{}" y="{}" width="{}" height="{}" fill="{}" stroke="{}"/>"#,
            escape_xml_display(node_style),
            fmt_display(lane_left),
            fmt_display(lane_top),
            fmt_display(width),
            fmt_display(title_height),
            escape_xml_display(&theme.cluster_bkg),
            escape_xml_display(&theme.cluster_border),
        );
        (
            lane.x - label_width / 2.0 + ctx.tx - origin_x,
            lane_top + (title_height - label_height) / 2.0,
            String::new(),
        )
    };

    if ctx.edge_html_labels {
        let title_html =
            flowchart_label_html(&lane.title, label_type, ctx.config, ctx.math_renderer);
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
        if label_type == "markdown" {
            write_flowchart_svg_text_markdown(out, &lane.title, true);
        } else {
            let plain = flowchart_label_plain_text(&lane.title, label_type, false);
            write_flowchart_svg_text(out, &plain, true);
        }
        out.push_str("</g></g>");
    }
    out.push_str("</g>");
}
