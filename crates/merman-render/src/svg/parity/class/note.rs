use crate::entities::decode_entities_minimal_cow;
use crate::model::{Bounds, LayoutNode};
use crate::text::{MERMAID_CREATE_TEXT_DEFAULT_WIDTH_PX, TextMeasurer, TextStyle, WrapMode};
use std::fmt::Write as _;
use std::time::Duration;

use super::super::SvgDiagramId;
use super::super::timing::RenderTiming;
use super::super::{SvgOutput, escape_attr_display, fmt, theme_token};
use super::ClassSvgNote;
use super::bounds::{include_path_d, include_xywh};
use super::context::ClassEmitCheckpoint;
use super::label::{
    class_math_html_label, class_note_html_div_style, write_class_svg_text_markdown_with_style,
};
use super::node::ClassNodeRenderPosition;
use super::rough::{
    class_rough_hachure_rect_paths, class_rough_rect_stroke_path_and_bounds, class_rough_seed,
};

pub(super) struct ClassNoteRenderContext<'a> {
    pub text_paint: Option<&'a crate::class::ClassTextPaint>,
    pub diagram_id: SvgDiagramId<'a>,
    pub measurer: &'a dyn TextMeasurer,
    pub text_style: &'a TextStyle,
    pub line_height: f64,
    pub use_html_labels: bool,
    pub mermaid_config: &'a merman_core::MermaidConfig,
    pub math_renderer: Option<&'a (dyn crate::math::MathRenderer + Send + Sync)>,
    pub look: &'a str,
    pub hand_drawn_seed: roughr::core::RoughRandomness,
    pub timing: RenderTiming,
    pub emit: ClassEmitCheckpoint<'a>,
}

pub(super) struct ClassNoteRenderState<'a, O: SvgOutput> {
    pub out: &'a mut O,
    pub content_bounds: &'a mut Option<Bounds>,
}

#[derive(Debug, Default, Clone, Copy)]
pub(super) struct ClassNoteRenderStats {
    pub notes_sanitize: Duration,
    pub path_bounds: Duration,
    pub path_bounds_calls: usize,
    pub typography: crate::class::ClassTextTerminalFacts,
}

pub(super) fn render_class_note_node<O: SvgOutput>(
    state: ClassNoteRenderState<'_, O>,
    note: &ClassSvgNote,
    layout_node: &LayoutNode,
    position: ClassNodeRenderPosition,
    ctx: &ClassNoteRenderContext<'_>,
) -> crate::Result<ClassNoteRenderStats> {
    let out = &mut *state.out;
    let content_bounds = &mut *state.content_bounds;
    let mut stats = ClassNoteRenderStats::default();

    let note_label_style = super::label::class_node_label_style(
        "text-align:left !important;white-space:nowrap !important",
        None,
    );
    let note_label_style = ctx.text_paint.map_or(note_label_style.clone(), |paint| {
        format!("{note_label_style};{}", paint.style())
    });
    let note_src = note.text.trim();
    let note_text = decode_entities_minimal_cow(note_src);
    let (label_w_raw, label_h_raw) = if ctx.use_html_labels {
        match (layout_node.label_width, layout_node.label_height) {
            (Some(w), Some(h)) => (w, h),
            _ => {
                let metrics = crate::class::class_html_measure_note_metrics(
                    ctx.measurer,
                    ctx.text_style,
                    note_src,
                    ctx.mermaid_config,
                );
                (metrics.width, metrics.height)
            }
        }
    } else {
        let mut metrics =
            ctx.measurer
                .measure_wrapped(&note_text, ctx.text_style, None, WrapMode::SvgLike);
        if let Some(width) = crate::class::class_svg_single_line_plain_label_width_px(
            note_text.as_ref(),
            ctx.measurer,
            ctx.text_style,
        ) {
            metrics.width = width;
        }
        (metrics.width, metrics.height)
    };
    let label_w = label_w_raw.max(1.0);
    let label_h = if ctx.use_html_labels {
        label_h_raw.max(ctx.line_height).max(1.0)
    } else {
        label_h_raw.max(1.0)
    };
    let w = layout_node.width.max(1.0);
    let h = layout_node.height.max(1.0);
    let left = -w / 2.0;
    let top = -h / 2.0;
    let label_x = -label_w / 2.0;
    let label_y = if ctx.use_html_labels {
        -label_h / 2.0
    } else {
        -label_h / 2.0
            - ctx
                .measurer
                .measure_svg_create_text_bbox_y_offset_px(note_text.as_ref(), ctx.text_style)
    };
    let hand_drawn = ctx.look == "handDrawn";
    let rough_seed = class_rough_seed(
        &ctx.hand_drawn_seed,
        ctx.diagram_id.semantic_str(),
        &note.id,
    );
    include_xywh(
        content_bounds,
        position.node_bounds_tx + left,
        position.node_bounds_ty + top,
        w,
        h,
    );
    include_xywh(
        content_bounds,
        position.node_bounds_tx + label_x,
        position.node_bounds_ty + label_y,
        label_w,
        label_h,
    );
    let path_bounds_start = ctx.timing.start();
    let note_fill = theme_token(ctx.mermaid_config.as_value(), "noteBkgColor", "#fff5ad");
    let note_stroke = theme_token(ctx.mermaid_config.as_value(), "noteBorderColor", "#aaaa33");
    let note_shape_style = format!("fill:{note_fill} !important;stroke:{note_stroke} !important");
    let (note_fill_d, note_stroke_d) = if hand_drawn {
        class_rough_hachure_rect_paths(
            left,
            top,
            w,
            h,
            &note_fill,
            &note_stroke,
            1.3,
            "0 0",
            &rough_seed,
        )
        .unwrap_or_else(|| {
            let (stroke_d, _) =
                class_rough_rect_stroke_path_and_bounds(left, top, w, h, &rough_seed);
            (String::new(), stroke_d)
        })
    } else {
        let (stroke_d, _) = class_rough_rect_stroke_path_and_bounds(left, top, w, h, &rough_seed);
        (String::new(), stroke_d)
    };
    include_path_d(
        content_bounds,
        &note_stroke_d,
        position.node_bounds_tx,
        position.node_bounds_ty,
    );
    if let Some(s) = path_bounds_start {
        stats.path_bounds += s.elapsed();
        stats.path_bounds_calls += 1;
    }

    let note_node_class = if hand_drawn {
        "rough-node undefined"
    } else {
        "node undefined"
    };
    let note_data_look_attr = format!(r#" data-look="{}""#, escape_attr_display(ctx.look));
    let note_label_class = "label noteLabel";
    let note_span_class = "nodeLabel markdown-node-label";
    let mut note_shape = String::new();
    if hand_drawn {
        let _ = write!(
            &mut note_shape,
            r##"<g class="basic label-container outer-path"><path d="{}" stroke="{}" stroke-width="4" fill="none" stroke-dasharray="0 0"/><path d="{}" stroke="{}" stroke-width="1.3" fill="none" stroke-dasharray="0 0"/></g>"##,
            escape_attr_display(&note_fill_d),
            escape_attr_display(&note_fill),
            escape_attr_display(&note_stroke_d),
            escape_attr_display(&note_stroke),
        );
    } else {
        let _ = write!(
            &mut note_shape,
            r##"<g class="basic label-container outer-path"><path d="M{} {} L{} {} L{} {} L{} {}" stroke="none" stroke-width="0" fill="{}" style="{}"/><path d="{}" stroke="{}" stroke-width="1.3" fill="none" stroke-dasharray="0 0" style="{}"/></g>"##,
            fmt(left),
            fmt(top),
            fmt(left + w),
            fmt(top),
            fmt(left + w),
            fmt(top + h),
            fmt(left),
            fmt(top + h),
            escape_attr_display(&note_fill),
            escape_attr_display(&note_shape_style),
            escape_attr_display(&note_stroke_d),
            escape_attr_display(&note_stroke),
            escape_attr_display(&note_shape_style),
        );
    }

    if ctx.use_html_labels {
        let note_div_style =
            class_note_html_div_style(label_w, MERMAID_CREATE_TEXT_DEFAULT_WIDTH_PX as i64);
        let _ = write!(out, r#"<g class="{}" id=""#, note_node_class);
        let _ = write!(out, "{}", ctx.diagram_id);
        ctx.emit.checkpoint()?;
        let _ = write!(
            out,
            r##"-{}"{} transform="translate({}, {})">{}<g class="{}" style="text-align:left !important;white-space:nowrap !important" transform="translate({}, {})"><rect/><foreignObject width="{}" height="{}"><div style="{}" xmlns="http://www.w3.org/1999/xhtml"><span style="{}" class="{}">"##,
            escape_attr_display(&note.id),
            note_data_look_attr,
            fmt(position.node_tx),
            fmt(position.node_ty),
            note_shape,
            note_label_class,
            fmt(label_x),
            fmt(label_y),
            fmt(label_w),
            fmt(label_h),
            escape_attr_display(&note_div_style),
            escape_attr_display(&note_label_style),
            note_span_class,
        );
        let sanitize_start = ctx.timing.start();
        let note_html = if let Some(math_html) =
            class_math_html_label(note_src, Some(ctx.mermaid_config), ctx.math_renderer)
        {
            stats.typography = crate::class::ClassTextTerminalFacts::unverified_text(note_src);
            math_html
        } else {
            let html = crate::class::class_note_html_fragment(note_src, ctx.mermaid_config);
            let note_html = format!("<p>{html}</p>");
            let facts = crate::text::VisibleTextStyleFacts::from_xhtml_fragment(&note_html);
            stats.typography =
                crate::class::ClassTextTerminalFacts::from_visible_style_facts(&facts);
            note_html
        };
        if let Some(s) = sanitize_start {
            stats.notes_sanitize += s.elapsed();
        }
        out.push_str(&note_html);
        out.push_str("</span></div></foreignObject></g></g>");
    } else {
        let _ = write!(out, r#"<g class="{}" id=""#, note_node_class);
        let _ = write!(out, "{}", ctx.diagram_id);
        ctx.emit.checkpoint()?;
        let _ = write!(
            out,
            r##"-{}"{} transform="translate({}, {})">{}<g class="{}" style="{}" transform="translate({}, {})"><rect/><g><rect class="background" style="stroke: none"/>"##,
            escape_attr_display(&note.id),
            note_data_look_attr,
            fmt(position.node_tx),
            fmt(position.node_ty),
            note_shape,
            note_label_class,
            escape_attr_display(&note_label_style),
            fmt(label_x),
            fmt(label_y),
        );
        write_class_svg_text_markdown_with_style(out, note_text.as_ref(), &note_label_style);
        out.push_str("</g></g></g>");
        let facts = crate::class::class_svg_label_visible_style_facts(note_text.as_ref());
        stats.typography = crate::class::ClassTextTerminalFacts::from_visible_style_facts(&facts);
    }

    if let Some(paint) = ctx.text_paint {
        stats.typography = paint.observe(stats.typography, &note_label_style);
    }
    Ok(stats)
}
