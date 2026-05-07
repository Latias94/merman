use crate::entities::decode_entities_minimal_cow;
use crate::model::{Bounds, ClassNodeRowMetrics, LayoutNode};
use crate::text::{TextMeasurer, TextStyle};
use merman_core::models::class_diagram::ClassMember;
use std::fmt::Write as _;
use std::time::Duration;

use super::super::{escape_attr_display, escape_xml_into, fmt, fmt_into};
use super::ClassSvgNode;
use super::bounds::{include_path_bounds, include_xywh};
use super::label::{
    class_html_div_style, class_html_label_max_width_px, class_html_label_metrics,
    class_html_title_metrics, render_class_html_label,
};
use super::rough::{
    class_rough_line_double_path_and_bounds, class_rough_rect_stroke_path_and_bounds,
    class_rough_seed,
};

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

pub(super) struct ClassNodeDividerContext<'a> {
    pub node_style_attr: &'a str,
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

pub(super) struct ClassSvgNodeLabelRun {
    pub text: String,
    pub style: String,
    pub metrics: crate::text::TextMetrics,
    pub y_offset: f64,
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

pub(super) struct ClassHtmlNodeBodyContext<'a> {
    pub measurer: &'a dyn TextMeasurer,
    pub text_style: &'a TextStyle,
    pub html_calc_text_style: &'a TextStyle,
    pub line_height: f64,
    pub class_padding: f64,
    pub hide_empty_members_box: bool,
    pub node_style_attr: &'a str,
    pub node_stroke: &'a str,
    pub node_stroke_width: &'a str,
    pub node_stroke_dasharray: &'a str,
    pub timing_enabled: bool,
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

pub(super) fn render_class_node_dividers(
    state: ClassNodeRenderState<'_>,
    position: ClassNodeRenderPosition,
    left: f64,
    right: f64,
    divider_ys: [f64; 2],
    rough_seed: u64,
    ctx: &ClassNodeDividerContext<'_>,
) -> ClassNodeRenderStats {
    let out = &mut *state.out;
    let content_bounds = &mut *state.content_bounds;
    let mut stats = ClassNodeRenderStats::default();

    for y in divider_ys {
        let _ = write!(
            out,
            r#"<g class="divider" style="{}">"#,
            escape_attr_display(ctx.node_style_attr)
        );
        let (d, d_pb) = class_rough_line_double_path_and_bounds(left, y, right, y, rough_seed);
        let path_bounds_start = ctx.timing_enabled.then(std::time::Instant::now);
        include_path_bounds(
            content_bounds,
            &d_pb,
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
            escape_attr_display(&d),
            escape_attr_display(ctx.node_stroke),
            escape_attr_display(ctx.node_stroke_width),
            escape_attr_display(ctx.node_stroke_dasharray),
            escape_attr_display(ctx.node_style_attr),
        );
        out.push_str("</g>");
    }

    stats
}

pub(super) fn render_class_html_node_body(
    state: ClassNodeRenderState<'_>,
    position: ClassNodeRenderPosition,
    node: &ClassSvgNode,
    geometry: ClassNodeBoxGeometry,
    class_row_metrics: Option<&ClassNodeRowMetrics>,
    ctx: &ClassHtmlNodeBodyContext<'_>,
) -> ClassNodeRenderStats {
    let out = &mut *state.out;
    let content_bounds = &mut *state.content_bounds;
    let padding = ctx.class_padding.max(0.0);
    let gap = padding;
    let members_rows = node.members.len();
    let methods_rows = node.methods.len();
    let render_extra_box = members_rows == 0 && methods_rows == 0 && !ctx.hide_empty_members_box;
    let content_bbox_height = if render_extra_box {
        (geometry.h - 4.0 * padding).max(0.0)
    } else if members_rows == 0 && methods_rows == 0 {
        (geometry.h - padding).max(0.0)
    } else {
        (geometry.h - 2.0 * padding).max(0.0)
    };
    let content_top = -content_bbox_height / 2.0;
    let text_translate_y = if render_extra_box {
        content_top
    } else if members_rows == 0 && methods_rows == 0 {
        content_top + padding * 1.5
    } else {
        content_top + padding
    };

    let title_text = decode_entities_minimal_cow(node.text.trim());
    let mut title_max_width_px = crate::class::class_html_create_text_width_px(
        title_text.as_ref(),
        ctx.measurer,
        ctx.html_calc_text_style,
    );
    let title_calc_max_width_px = title_max_width_px;
    let mut title_metrics = class_html_title_metrics(
        ctx.measurer,
        ctx.text_style,
        title_text.as_ref(),
        title_max_width_px,
    );
    if title_text.chars().count() > 4 && title_metrics.width > 0.0 {
        title_metrics.width =
            crate::text::round_to_1_64_px((title_metrics.width - (1.0 / 64.0)).max(0.0));
    }
    if let Some(width) = crate::class::class_html_known_rendered_width_override_px(
        title_text.as_ref(),
        ctx.text_style,
        true,
    ) {
        title_metrics.width = width;
    }
    if title_text.chars().count() == 1
        && !(title_text.contains('*') || title_text.contains('_') || title_text.contains('`'))
    {
        let rendered_title_max_width_px = class_html_label_max_width_px(title_metrics.width, true);
        title_max_width_px = if crate::class::class_html_known_calc_text_width_override_px(
            title_text.as_ref(),
            ctx.html_calc_text_style,
        )
        .is_some()
        {
            title_calc_max_width_px.min(rendered_title_max_width_px)
        } else {
            rendered_title_max_width_px
        };
    }
    let title_width = title_metrics.width.max(1.0);
    let title_height = title_metrics.height.max(ctx.line_height).max(1.0);
    let title_x = -title_width / 2.0;

    let annotation_text = node.annotations.first().map(|annotation| {
        let decoded = decode_entities_minimal_cow(annotation.trim());
        let mut label = String::new();
        label.push('\u{00AB}');
        label.push_str(decoded.as_ref());
        label.push('\u{00BB}');
        label
    });
    let annotation_metrics = annotation_text.as_deref().map(|text| {
        let max_width_px = crate::class::class_html_create_text_width_px(
            text,
            ctx.measurer,
            ctx.html_calc_text_style,
        );
        class_html_label_metrics(ctx.measurer, ctx.text_style, text, max_width_px, "")
    });
    let annotation_width = annotation_metrics
        .as_ref()
        .map(|metrics| metrics.width.max(1.0))
        .unwrap_or(0.0);
    let annotation_height = annotation_metrics
        .as_ref()
        .map(|metrics| metrics.height.max(ctx.line_height).max(1.0))
        .unwrap_or(0.0);
    let annotation_group_x = if annotation_width > 0.0 {
        -annotation_width / 2.0
    } else {
        0.0
    };
    let annotation_group_y = text_translate_y;
    let title_y = annotation_height + text_translate_y;

    let html_rows_ctx = ClassHtmlNodeRowsContext {
        measurer: ctx.measurer,
        text_style: ctx.text_style,
        html_calc_text_style: ctx.html_calc_text_style,
        line_height: ctx.line_height,
    };
    let members_rows_rendered = measure_class_html_node_rows(
        &node.members,
        class_row_metrics.map(|rows| rows.members.as_slice()),
        &html_rows_ctx,
    );
    let members_group_raw_height = members_rows_rendered.raw_height;
    let members_group_y = annotation_height + title_height + gap * 2.0 + text_translate_y;

    let methods_offset_base = if members_group_raw_height > 0.0 {
        members_group_raw_height + gap * 4.0
    } else {
        gap / 2.0 + gap * 4.0
    };
    let methods_rows_rendered = measure_class_html_node_rows(
        &node.methods,
        class_row_metrics.map(|rows| rows.methods.as_slice()),
        &html_rows_ctx,
    );
    let methods_group_y = annotation_height + title_height + methods_offset_base + text_translate_y;

    let members_group_width = members_rows_rendered
        .rows
        .iter()
        .fold(0.0_f64, |acc, row| acc.max(row.metrics.width.max(1.0)));
    let methods_group_width = methods_rows_rendered
        .rows
        .iter()
        .fold(0.0_f64, |acc, row| acc.max(row.metrics.width.max(1.0)));
    let mut content_bbox_min_x = 0.0_f64;
    let mut content_bbox_max_x = 0.0_f64;
    for centered_width in [annotation_width, title_width] {
        if centered_width > 0.0 {
            content_bbox_min_x = content_bbox_min_x.min(-centered_width / 2.0);
            content_bbox_max_x = content_bbox_max_x.max(centered_width / 2.0);
        }
    }
    for left_aligned_width in [members_group_width, methods_group_width] {
        if left_aligned_width > 0.0 {
            content_bbox_max_x = content_bbox_max_x.max(left_aligned_width);
        }
    }
    let content_bbox_width = (content_bbox_max_x - content_bbox_min_x).max(0.0);
    let members_x = -content_bbox_width / 2.0;

    let divider_adjust = if render_extra_box { padding / 2.0 } else { 0.0 };
    let divider1_y = (annotation_height - divider_adjust)
        + (title_height - divider_adjust)
        + content_top
        + padding;
    let divider2_y = (annotation_height - divider_adjust)
        + (title_height - divider_adjust)
        + (members_group_raw_height - divider_adjust)
        + content_top
        + padding
        + gap * 2.0;

    if let Some(annotation_text) = annotation_text.as_deref() {
        let annotation_max_width_px = crate::class::class_html_create_text_width_px(
            annotation_text,
            ctx.measurer,
            ctx.html_calc_text_style,
        );
        let annotation_div_style =
            class_html_div_style(annotation_width.max(1.0), annotation_max_width_px);
        let _ = write!(
            out,
            r#"<g class="annotation-group text" transform="translate({}, {})">"#,
            fmt(annotation_group_x),
            fmt(annotation_group_y)
        );
        render_class_html_node_label_group(
            out,
            &ClassHtmlNodeLabelGroupSpec {
                label_style: "",
                translate_y: -annotation_height / 2.0,
                width: annotation_width.max(1.0),
                height: annotation_height.max(1.0),
                div_style: annotation_div_style.as_str(),
                text: annotation_text,
                include_p: true,
                extra_span_class: Some("markdown-node-label"),
                span_style: Some(ctx.node_style_attr),
            },
        );
        out.push_str("</g>");
    } else {
        let _ = write!(
            out,
            r#"<g class="annotation-group text" transform="translate(0, {})"/>"#,
            fmt(annotation_group_y)
        );
    }

    let title_div_style = class_html_div_style(title_width, title_max_width_px);
    let _ = write!(
        out,
        r#"<g class="label-group text" transform="translate({}, {})">"#,
        fmt(title_x),
        fmt(title_y)
    );
    render_class_html_node_label_group(
        out,
        &ClassHtmlNodeLabelGroupSpec {
            label_style: "font-weight: bolder",
            translate_y: -12.0,
            width: title_width,
            height: title_height,
            div_style: title_div_style.as_str(),
            text: title_text.as_ref(),
            include_p: true,
            extra_span_class: Some("markdown-node-label"),
            span_style: Some(ctx.node_style_attr),
        },
    );
    out.push_str("</g>");

    render_class_html_node_rows_group(
        out,
        "members-group text",
        members_x,
        members_group_y,
        &members_rows_rendered,
        ctx.line_height,
        ctx.node_style_attr,
    );

    render_class_html_node_rows_group(
        out,
        "methods-group text",
        members_x,
        methods_group_y,
        &methods_rows_rendered,
        ctx.line_height,
        ctx.node_style_attr,
    );

    if ctx.hide_empty_members_box && members_rows == 0 && methods_rows == 0 {
        ClassNodeRenderStats::default()
    } else {
        render_class_node_dividers(
            ClassNodeRenderState {
                out,
                content_bounds,
            },
            position,
            geometry.left,
            geometry.left + geometry.w,
            [divider1_y, divider2_y],
            geometry.rough_seed,
            &ClassNodeDividerContext {
                node_style_attr: ctx.node_style_attr,
                node_stroke: ctx.node_stroke,
                node_stroke_width: ctx.node_stroke_width,
                node_stroke_dasharray: ctx.node_stroke_dasharray,
                timing_enabled: ctx.timing_enabled,
            },
        )
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

pub(super) fn render_class_svg_node_runs_group(
    out: &mut String,
    group_class: &str,
    group_x: f64,
    group_y: f64,
    runs: &[ClassSvgNodeLabelRun],
) {
    if runs.is_empty() {
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
    for run in runs {
        let t_y = -run.metrics.height.max(0.0) / (2.0 * run.metrics.line_count.max(1) as f64)
            + run.y_offset;
        let _ = write!(
            out,
            r#"<g class="label" style="{}" transform="translate(0,{})"><g><rect class="background" style="stroke: none"/>"#,
            escape_attr_display(run.style.as_str()),
            fmt(t_y)
        );
        crate::svg::parity::flowchart::write_flowchart_svg_text_markdown(
            out,
            run.text.as_str(),
            true,
        );
        out.push_str("</g></g>");
    }
    out.push_str("</g>");
}

pub(super) fn render_class_svg_title_group(
    out: &mut String,
    group_x: f64,
    group_y: f64,
    title_lines: &[String],
    title_metrics: &crate::text::TextMetrics,
) {
    let _ = write!(
        out,
        r#"<g class="label-group text" transform="translate({}, {})">"#,
        fmt(group_x),
        fmt(group_y)
    );
    let t_y = -title_metrics.height.max(0.0) / (2.0 * title_metrics.line_count.max(1) as f64);
    let _ = write!(
        out,
        r#"<g class="label" style="font-weight: bolder" transform="translate(0,{})"><g><rect class="background" style="stroke: none"/><text y="-10.1" style="">"#,
        fmt(t_y)
    );
    for (idx, line) in title_lines.iter().enumerate() {
        if idx == 0 {
            out.push_str(
                r#"<tspan class="text-outer-tspan" x="0" y="-0.1em" dy="1.1em" font-weight="">"#,
            );
        } else {
            let y_em = if idx == 1 {
                "1em".to_string()
            } else {
                format!("{:.1}em", 1.0 + (idx as f64 - 1.0) * 1.1)
            };
            let _ = write!(
                out,
                r#"<tspan class="text-outer-tspan" x="0" y="{}" dy="1.1em" font-weight="">"#,
                y_em
            );
        }
        out.push_str(r#"<tspan font-style="normal" class="text-inner-tspan" font-weight="">"#);
        escape_xml_into(out, line);
        out.push_str("</tspan></tspan>");
    }
    out.push_str("</text></g></g></g>");
}
