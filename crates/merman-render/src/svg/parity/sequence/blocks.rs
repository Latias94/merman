use super::super::*;
use super::block_collection::AltSection;
use super::block_geometry::SequenceBlockGeometry;
use super::block_text::{
    LoopTextPlacement, LoopTextRenderContext, display_block_label, write_loop_text_lines,
    write_section_title_lines,
};
use crate::model::SequenceBlockLayout;
use crate::sequence::sequence_block_label_wrap_width;
use rustc_hash::FxHashMap;

pub(super) struct SequenceBlockRenderContext<'a> {
    pub(super) default_frame_x1: f64,
    pub(super) default_frame_x2: f64,
    pub(super) block_widths_by_id: &'a FxHashMap<String, f64>,
    pub(super) actor_nodes_by_id: &'a FxHashMap<&'a str, &'a LayoutNode>,
    pub(super) label_box_width: f64,
    pub(super) wrap_padding: f64,
    pub(super) measurer: &'a dyn TextMeasurer,
    pub(super) loop_text_style: &'a TextStyle,
    pub(super) sanitize_config: &'a merman_core::MermaidConfig,
    pub(super) math_renderer: Option<&'a (dyn crate::math::MathRenderer + Send + Sync)>,
}

pub(super) struct SimpleSequenceBlock<'a> {
    pub(super) control_id: &'a str,
    pub(super) label_id: &'a str,
    pub(super) block_label: &'static str,
    pub(super) raw_label: &'a str,
    pub(super) geometry: SequenceBlockGeometry<'a>,
    pub(super) layout: Option<&'a SequenceBlockLayout>,
}

impl<'a> SequenceBlockRenderContext<'a> {
    fn loop_text_context(&self) -> LoopTextRenderContext<'_> {
        LoopTextRenderContext::new(
            self.measurer,
            self.loop_text_style,
            self.sanitize_config,
            self.math_renderer,
        )
    }

    fn label_wrap_width(&self, label_id: &str, fallback: Option<f64>) -> Option<f64> {
        self.block_widths_by_id
            .get(label_id)
            .map(|width| sequence_block_label_wrap_width(*width, self.wrap_padding))
            .or(fallback)
    }
}

fn write_control_structure_group_open(out: &mut String, control_id: &str) {
    let _ = write!(
        out,
        r#"<g data-et="control-structure" data-id="i{id}">"#,
        id = escape_attr(control_id)
    );
}

pub(super) fn write_block_frame(
    out: &mut String,
    frame_x1: f64,
    frame_x2: f64,
    frame_y1: f64,
    frame_y2: f64,
) {
    let _ = write!(
        out,
        r#"<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y1}" class="loopLine"/>"#,
        x1 = fmt(frame_x1),
        x2 = fmt(frame_x2),
        y1 = fmt(frame_y1)
    );
    let _ = write!(
        out,
        r#"<line x1="{x2}" y1="{y1}" x2="{x2}" y2="{y2}" class="loopLine"/>"#,
        x2 = fmt(frame_x2),
        y1 = fmt(frame_y1),
        y2 = fmt(frame_y2)
    );
    let _ = write!(
        out,
        r#"<line x1="{x1}" y1="{y2}" x2="{x2}" y2="{y2}" class="loopLine"/>"#,
        x1 = fmt(frame_x1),
        x2 = fmt(frame_x2),
        y2 = fmt(frame_y2)
    );
    let _ = write!(
        out,
        r#"<line x1="{x1}" y1="{y1}" x2="{x1}" y2="{y2}" class="loopLine"/>"#,
        x1 = fmt(frame_x1),
        y1 = fmt(frame_y1),
        y2 = fmt(frame_y2)
    );
}

pub(super) fn write_block_label_box(
    out: &mut String,
    frame_x1: f64,
    frame_y1: f64,
    label_box_width: f64,
    label: &str,
) {
    let x1 = frame_x1;
    let y1 = frame_y1;
    let x2 = x1 + label_box_width;
    let y2 = y1 + 13.0;
    let y3 = y1 + 20.0;
    let x3 = x2 - 8.4;
    let _ = write!(
        out,
        r#"<polygon points="{x1},{y1} {x2},{y1} {x2},{y2} {x3},{y3} {x1},{y3}" class="labelBox"/>"#,
        x1 = fmt(x1),
        y1 = fmt(y1),
        x2 = fmt(x2),
        y2 = fmt(y2),
        x3 = fmt(x3),
        y3 = fmt(y3)
    );
    let label_cx = (x1 + label_box_width / 2.0).round();
    let label_cy = y1 + 13.0;
    let _ = write!(
        out,
        r#"<text x="{x}" y="{y}" text-anchor="middle" dominant-baseline="middle" alignment-baseline="middle" class="labelText" style="font-size: 16px; font-weight: 400;">{label}</text>"#,
        x = fmt(label_cx),
        y = fmt(label_cy),
        label = escape_xml(label)
    );
}

pub(super) fn render_simple_sequence_block(
    out: &mut String,
    block: SimpleSequenceBlock<'_>,
    ctx: &SequenceBlockRenderContext<'_>,
) {
    if block.geometry.frame_y_range().is_none() {
        return;
    }
    let Some(layout) = block.layout else {
        return;
    };

    let (frame_x1, frame_x2, _min_left) = block
        .geometry
        .frame_x(ctx.actor_nodes_by_id)
        .unwrap_or((ctx.default_frame_x1, ctx.default_frame_x2, f64::INFINITY));

    let frame_y1 = layout.start_y;
    let frame_y2 = layout.stop_y;

    write_control_structure_group_open(out, block.control_id);
    write_block_frame(out, frame_x1, frame_x2, frame_y1, frame_y2);
    write_block_label_box(
        out,
        frame_x1,
        frame_y1,
        ctx.label_box_width,
        block.block_label,
    );
    let label_box_right = frame_x1 + ctx.label_box_width;
    let text_x = (label_box_right + frame_x2) / 2.0;
    let text_y = frame_y1 + 18.0;
    let label =
        display_block_label(block.raw_label, true).unwrap_or_else(|| "\u{200B}".to_string());
    let max_w = ctx.label_wrap_width(block.label_id, Some((frame_x2 - label_box_right).max(0.0)));
    let loop_text_ctx = ctx.loop_text_context();
    write_loop_text_lines(
        out,
        &loop_text_ctx,
        LoopTextPlacement {
            x: text_x,
            y0: text_y,
            block_start_y: frame_y1,
            max_width: max_w,
            use_tspan: true,
        },
        &label,
    );
    out.push_str("</g>");
}

fn section_geometry<'a>(sections: &[AltSection<'a>]) -> SequenceBlockGeometry<'a> {
    sections
        .iter()
        .fold(SequenceBlockGeometry::empty(), |geometry, section| {
            geometry.merged(section.geometry)
        })
}

fn section_separator_ys(sections: &[AltSection<'_>]) -> Option<Vec<f64>> {
    sections
        .iter()
        .skip(1)
        .map(|section| section.separator_y)
        .collect()
}

pub(super) fn render_sectioned_sequence_block(
    out: &mut String,
    control_id: &str,
    block_label: &str,
    sections: &[AltSection<'_>],
    layout: Option<&SequenceBlockLayout>,
    ctx: &SequenceBlockRenderContext<'_>,
) {
    if sections.is_empty() {
        return;
    }

    let geometry = section_geometry(sections);
    if geometry.frame_y_range().is_none() {
        return;
    }
    let Some(layout) = layout else {
        return;
    };
    let Some(sep_ys) = section_separator_ys(sections) else {
        return;
    };

    let (frame_x1, frame_x2, _min_left) = geometry.frame_x(ctx.actor_nodes_by_id).unwrap_or((
        ctx.default_frame_x1,
        ctx.default_frame_x2,
        f64::INFINITY,
    ));

    let frame_y1 = layout.start_y;
    let frame_y2 = layout.stop_y;

    write_control_structure_group_open(out, control_id);

    // frame
    write_block_frame(out, frame_x1, frame_x2, frame_y1, frame_y2);

    // separators (dashed)
    // Keep separator endpoints identical to the frame endpoints to match upstream
    // Mermaid output and avoid sub-pixel gaps at the frame border.
    let dash_x1 = frame_x1;
    let dash_x2 = frame_x2;
    for y in &sep_ys {
        let _ = write!(
            out,
            r#"<line x1="{x1}" y1="{y}" x2="{x2}" y2="{y}" class="loopLine" style="stroke-dasharray: 3, 3;"/>"#,
            x1 = fmt(dash_x1),
            x2 = fmt(dash_x2),
            y = fmt(*y)
        );
    }

    // label box + label text
    write_block_label_box(out, frame_x1, frame_y1, ctx.label_box_width, block_label);

    // section labels
    let label_box_right = frame_x1 + ctx.label_box_width;
    let main_text_x = (label_box_right + frame_x2) / 2.0;
    let center_text_x = (frame_x1 + frame_x2) / 2.0;
    for (i, sec) in sections.iter().enumerate() {
        let Some(label_text) = display_block_label(sec.raw_label, i == 0) else {
            continue;
        };
        if i == 0 {
            let y = frame_y1 + 18.0;
            let max_w =
                ctx.label_wrap_width(sec.label_id, Some((frame_x2 - label_box_right).max(0.0)));
            let loop_text_ctx = ctx.loop_text_context();
            write_loop_text_lines(
                out,
                &loop_text_ctx,
                LoopTextPlacement {
                    x: main_text_x,
                    y0: y,
                    block_start_y: frame_y1,
                    max_width: max_w,
                    use_tspan: true,
                },
                &label_text,
            );
            continue;
        }
        let y = sep_ys.get(i - 1).copied().unwrap_or(frame_y1) + 18.0;
        let loop_text_ctx = ctx.loop_text_context();
        write_section_title_lines(
            out,
            &loop_text_ctx,
            center_text_x,
            y,
            sep_ys.get(i - 1).copied().unwrap_or(frame_y1),
            ctx.label_wrap_width(sec.label_id, None),
            &label_text,
        );
    }

    out.push_str("</g>");
}

pub(super) fn render_critical_sequence_block(
    out: &mut String,
    control_id: &str,
    sections: &[AltSection<'_>],
    layout: Option<&SequenceBlockLayout>,
    ctx: &SequenceBlockRenderContext<'_>,
) {
    if sections.is_empty() {
        return;
    }

    let geometry = section_geometry(sections);
    if geometry.frame_y_range().is_none() {
        return;
    }
    let Some(layout) = layout else {
        return;
    };
    let Some(sep_ys) = section_separator_ys(sections) else {
        return;
    };

    let (mut frame_x1, frame_x2, min_left) = geometry.frame_x(ctx.actor_nodes_by_id).unwrap_or((
        ctx.default_frame_x1,
        ctx.default_frame_x2,
        f64::INFINITY,
    ));
    if sections.len() > 1 && min_left.is_finite() {
        // Mermaid's `critical` w/ `option` sections widens the frame to the left.
        frame_x1 = frame_x1.min(min_left - 9.0);
    }

    let frame_y1 = layout.start_y;
    let frame_y2 = layout.stop_y;

    write_control_structure_group_open(out, control_id);

    // frame
    write_block_frame(out, frame_x1, frame_x2, frame_y1, frame_y2);

    // separators (dashed)
    let dash_x1 = frame_x1;
    let dash_x2 = frame_x2;
    for y in &sep_ys {
        let _ = write!(
            out,
            r#"<line x1="{x1}" y1="{y}" x2="{x2}" y2="{y}" class="loopLine" style="stroke-dasharray: 3, 3;"/>"#,
            x1 = fmt(dash_x1),
            x2 = fmt(dash_x2),
            y = fmt(*y)
        );
    }

    // label box + label text
    write_block_label_box(out, frame_x1, frame_y1, ctx.label_box_width, "critical");

    // section labels
    let label_box_right = frame_x1 + ctx.label_box_width;
    let main_text_x = (label_box_right + frame_x2) / 2.0;
    let center_text_x = (frame_x1 + frame_x2) / 2.0;
    for (i, sec) in sections.iter().enumerate() {
        let Some(label_text) = display_block_label(sec.raw_label, i == 0) else {
            continue;
        };
        if i == 0 {
            let y = frame_y1 + 18.0;
            let max_w =
                ctx.label_wrap_width(sec.label_id, Some((frame_x2 - label_box_right).max(0.0)));
            let loop_text_ctx = ctx.loop_text_context();
            write_loop_text_lines(
                out,
                &loop_text_ctx,
                LoopTextPlacement {
                    x: main_text_x,
                    y0: y,
                    block_start_y: frame_y1,
                    max_width: max_w,
                    use_tspan: true,
                },
                &label_text,
            );
            continue;
        }
        let y = sep_ys.get(i - 1).copied().unwrap_or(frame_y1) + 18.0;
        let loop_text_ctx = ctx.loop_text_context();
        write_section_title_lines(
            out,
            &loop_text_ctx,
            center_text_x,
            y,
            sep_ys.get(i - 1).copied().unwrap_or(frame_y1),
            ctx.label_wrap_width(sec.label_id, None),
            &label_text,
        );
    }

    out.push_str("</g>");
}
