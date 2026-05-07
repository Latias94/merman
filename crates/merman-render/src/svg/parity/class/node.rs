use crate::entities::decode_entities_minimal_cow;
use crate::model::{Bounds, LayoutNode};
use crate::text::{TextMeasurer, TextStyle};
use merman_core::models::class_diagram::ClassMember;
use std::fmt::Write as _;
use std::time::Duration;

use super::super::{escape_attr_display, fmt, fmt_into};
use super::ClassSvgNode;
use super::bounds::{include_path_bounds, include_xywh};
use super::label::{
    class_html_div_style, class_html_label_max_width_px, class_html_label_metrics,
    render_class_html_label,
};
use super::rough::{class_rough_rect_stroke_path_and_bounds, class_rough_seed};

#[derive(Debug, Clone, Copy)]
pub(super) struct ClassNodeRenderPosition {
    pub node_tx: f64,
    pub node_ty: f64,
    pub node_bounds_tx: f64,
    pub node_bounds_ty: f64,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ClassNodeBoxGeometry {
    pub w: f64,
    pub h: f64,
    pub left: f64,
    pub rough_seed: u64,
}

pub(super) struct ClassNodeRenderState<'a> {
    pub out: &'a mut String,
    pub content_bounds: &'a mut Option<Bounds>,
}

pub(super) struct ClassNodeBasicContainerContext<'a> {
    pub diagram_id: &'a str,
    pub node_style_attr: &'a str,
    pub node_fill: &'a str,
    pub node_stroke: &'a str,
    pub node_stroke_width: &'a str,
    pub node_stroke_dasharray: &'a str,
    pub timing_enabled: bool,
}

#[derive(Debug, Default, Clone, Copy)]
pub(super) struct ClassNodeRenderStats {
    pub path_bounds: Duration,
    pub path_bounds_calls: usize,
}

pub(super) struct ClassNodeBasicContainerResult {
    pub geometry: ClassNodeBoxGeometry,
    pub stats: ClassNodeRenderStats,
}

pub(super) struct ClassHtmlNodeRow {
    pub text: String,
    pub row_style: String,
    pub metrics: crate::text::TextMetrics,
    pub max_width_px: i64,
    pub y: f64,
}

pub(super) struct ClassHtmlNodeRows {
    pub rows: Vec<ClassHtmlNodeRow>,
    pub raw_height: f64,
}

pub(super) struct ClassHtmlNodeRowsContext<'a> {
    pub measurer: &'a dyn TextMeasurer,
    pub text_style: &'a TextStyle,
    pub html_calc_text_style: &'a TextStyle,
    pub line_height: f64,
}

pub(super) struct ClassHtmlNodeLabelGroupSpec<'a> {
    pub label_style: &'a str,
    pub translate_y: f64,
    pub width: f64,
    pub height: f64,
    pub div_style: &'a str,
    pub text: &'a str,
    pub include_p: bool,
    pub extra_span_class: Option<&'a str>,
    pub span_style: Option<&'a str>,
}

pub(super) fn render_class_node_shell_open(
    out: &mut String,
    node: &ClassSvgNode,
    position: ClassNodeRenderPosition,
) -> bool {
    let tooltip = node.tooltip.as_deref().unwrap_or("").trim();
    let has_tooltip = !tooltip.is_empty();

    let link = node
        .link
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let include_href = link.is_some_and(|s| {
        let lower = s.to_ascii_lowercase();
        !lower.starts_with("javascript:") && lower != "about:blank"
    });
    let have_callback = node.have_callback;

    if let Some(link) = link {
        out.push_str("<a");
        if include_href {
            out.push_str(r#" xlink:href=""#);
            super::super::util::escape_attr_into(out, link);
            out.push('"');
        }
        if have_callback {
            out.push_str(r#" class="null clickable""#);
        }
        out.push_str(r#" transform="translate("#);
        fmt_into(out, position.node_tx);
        out.push_str(", ");
        fmt_into(out, position.node_ty);
        out.push_str(r#")">"#);
    }

    out.push_str(r#"<g class=""#);
    out.push_str("node ");
    super::super::util::escape_attr_into(out, node.css_classes.trim());
    out.push_str(r#"" id=""#);
    super::super::util::escape_attr_into(out, &node.dom_id);
    out.push('"');
    if has_tooltip {
        out.push_str(r#" title=""#);
        super::super::util::escape_attr_into(out, tooltip);
        out.push('"');
    }
    if link.is_none() {
        out.push_str(r#" transform="translate("#);
        fmt_into(out, position.node_tx);
        out.push_str(", ");
        fmt_into(out, position.node_ty);
        out.push_str(r#")""#);
    }
    out.push('>');

    link.is_some()
}

pub(super) fn render_class_node_basic_container(
    state: ClassNodeRenderState<'_>,
    node: &ClassSvgNode,
    layout_node: &LayoutNode,
    position: ClassNodeRenderPosition,
    ctx: &ClassNodeBasicContainerContext<'_>,
) -> ClassNodeBasicContainerResult {
    let out = &mut *state.out;
    let content_bounds = &mut *state.content_bounds;
    let mut stats = ClassNodeRenderStats::default();

    out.push_str(r#"<g class="basic label-container">"#);
    let w = layout_node.width.max(1.0);
    let h = layout_node.height.max(1.0);
    let left = -w / 2.0;
    let top = -h / 2.0;
    let rough_seed = class_rough_seed(ctx.diagram_id, &node.dom_id);
    let _ = write!(
        out,
        r#"<path d="M{} {} L{} {} L{} {} L{} {}" stroke="none" stroke-width="0" fill="{}" style="{}"/>"#,
        fmt(left),
        fmt(top),
        fmt(left + w),
        fmt(top),
        fmt(left + w),
        fmt(top + h),
        fmt(left),
        fmt(top + h),
        escape_attr_display(ctx.node_fill),
        escape_attr_display(ctx.node_style_attr)
    );
    let (stroke_d, stroke_pb) =
        class_rough_rect_stroke_path_and_bounds(left, top, w, h, rough_seed);
    include_xywh(
        content_bounds,
        position.node_bounds_tx + left,
        position.node_bounds_ty + top,
        w,
        h,
    );
    let path_bounds_start = ctx.timing_enabled.then(std::time::Instant::now);
    include_path_bounds(
        content_bounds,
        &stroke_pb,
        position.node_bounds_tx,
        position.node_bounds_ty,
    );
    if let Some(s) = path_bounds_start {
        stats.path_bounds += s.elapsed();
        stats.path_bounds_calls += 1;
    }
    let _ = write!(
        out,
        r#"<path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}" style="{}"/>"#,
        escape_attr_display(&stroke_d),
        escape_attr_display(ctx.node_stroke),
        escape_attr_display(ctx.node_stroke_width),
        escape_attr_display(ctx.node_stroke_dasharray),
        escape_attr_display(ctx.node_style_attr),
    );
    out.push_str("</g>");

    ClassNodeBasicContainerResult {
        geometry: ClassNodeBoxGeometry {
            w,
            h,
            left,
            rough_seed,
        },
        stats,
    }
}

pub(super) fn measure_class_html_node_rows(
    members: &[ClassMember],
    row_metrics: Option<&[crate::text::TextMetrics]>,
    ctx: &ClassHtmlNodeRowsContext<'_>,
) -> ClassHtmlNodeRows {
    let mut raw_height = 0.0;
    let mut rows = Vec::with_capacity(members.len());
    for (idx, member) in members.iter().enumerate() {
        let text = decode_entities_minimal_cow(member.display_text.trim()).into_owned();
        let mut max_width_px = crate::class::class_html_create_text_width_px(
            text.as_str(),
            ctx.measurer,
            ctx.html_calc_text_style,
        );
        let metrics = row_metrics
            .and_then(|rows| rows.get(idx).cloned())
            .unwrap_or_else(|| {
                class_html_label_metrics(
                    ctx.measurer,
                    ctx.text_style,
                    text.as_str(),
                    max_width_px,
                    member.css_style.as_str(),
                )
            });
        if metrics.width > 0.0
            && metrics.width < 60.0
            && !(text.contains('*') || text.contains('_') || text.contains('`'))
        {
            max_width_px = class_html_label_max_width_px(metrics.width, false);
        }
        if let Some(width) = crate::class::class_html_known_calc_text_width_override_px(
            text.as_str(),
            ctx.html_calc_text_style,
        ) {
            max_width_px = width + 50;
        }
        let row_height = metrics.height.max(ctx.line_height).max(1.0);
        let y = raw_height - row_height / 2.0;
        raw_height += row_height;
        rows.push(ClassHtmlNodeRow {
            text,
            row_style: member.css_style.trim().to_string(),
            metrics,
            max_width_px,
            y,
        });
    }

    ClassHtmlNodeRows { rows, raw_height }
}

pub(super) fn render_class_html_node_label_group(
    out: &mut String,
    spec: &ClassHtmlNodeLabelGroupSpec<'_>,
) {
    let _ = write!(
        out,
        r#"<g class="label" style="{}" transform="translate(0,{})"><foreignObject width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" style="{}">"#,
        escape_attr_display(spec.label_style),
        fmt(spec.translate_y),
        fmt(spec.width),
        fmt(spec.height),
        escape_attr_display(spec.div_style)
    );
    render_class_html_label(
        out,
        "nodeLabel",
        spec.text,
        spec.include_p,
        spec.extra_span_class,
        spec.span_style,
    );
    out.push_str("</div></foreignObject></g>");
}

pub(super) fn render_class_html_node_rows_group(
    out: &mut String,
    group_class: &str,
    group_x: f64,
    group_y: f64,
    rows_rendered: &ClassHtmlNodeRows,
    line_height: f64,
    node_style_attr: &str,
) {
    if rows_rendered.rows.is_empty() {
        let _ = write!(
            out,
            r#"<g class="{}" transform="translate({}, {})"/>"#,
            group_class,
            fmt(group_x),
            fmt(group_y)
        );
        return;
    }

    let _ = write!(
        out,
        r#"<g class="{}" transform="translate({}, {})">"#,
        group_class,
        fmt(group_x),
        fmt(group_y)
    );
    for row in &rows_rendered.rows {
        let div_style = class_html_div_style(row.metrics.width.max(1.0), row.max_width_px);
        render_class_html_node_label_group(
            out,
            &ClassHtmlNodeLabelGroupSpec {
                label_style: row.row_style.as_str(),
                translate_y: row.y,
                width: row.metrics.width.max(1.0),
                height: row.metrics.height.max(line_height).max(1.0),
                div_style: div_style.as_str(),
                text: row.text.as_str(),
                include_p: true,
                extra_span_class: Some("markdown-node-label"),
                span_style: Some(node_style_attr),
            },
        );
    }
    out.push_str("</g>");
}
