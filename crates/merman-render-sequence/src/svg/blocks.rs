use super::SequenceEmitCheckpoints;
use super::block_text::{
    LoopTextPlacement, LoopTextRenderContext, display_block_label, write_loop_text_lines,
    write_section_title_lines,
};
use super::prelude::*;
use crate::frame::SequenceFrameLayout;
use crate::preparation::{
    sequence_block_label_wrap_width, sequence_drawn_text_first_y, sequence_drawn_text_y,
};
use rustc_hash::FxHashMap;

pub(super) struct SequenceBlockRenderContext<'a> {
    pub(super) is_neo: bool,
    pub(super) block_widths_by_id: &'a FxHashMap<String, f64>,
    pub(super) label_box_width: f64,
    pub(super) label_box_drawn_width: f64,
    pub(super) label_box_height: f64,
    pub(super) box_margin: f64,
    pub(super) box_text_margin: f64,
    pub(super) wrap_padding: f64,
    pub(super) measurer: &'a dyn TextMeasurer,
    pub(super) loop_text_style: &'a TextStyle,
    pub(super) loop_typography: &'a crate::preparation::SequenceResolvedTypography,
    pub(super) frame_paint: &'a super::control_paint::SequenceControlPaint,
    pub(super) keyword_paint: &'a super::control_paint::SequenceControlPaint,
    pub(super) math_sidecar: &'a crate::preparation::SequenceMathSidecar,
    pub(super) sanitize_config: &'a merman_model::presentation::PresentationConfig,
    pub(super) checkpoints: SequenceEmitCheckpoints<'a>,
}

impl<'a> SequenceBlockRenderContext<'a> {
    fn loop_text_context(&self) -> LoopTextRenderContext<'_> {
        LoopTextRenderContext {
            measurer: self.measurer,
            style: self.loop_text_style,
            config: self.sanitize_config,
            margin: self.box_text_margin,
            typography: self.loop_typography,
            math_sidecar: self.math_sidecar,
            checkpoints: self.checkpoints,
        }
    }

    fn label_wrap_width(&self, label_id: &str, fallback: Option<f64>) -> Option<f64> {
        self.block_widths_by_id
            .get(label_id)
            .map(|width| sequence_block_label_wrap_width(*width, self.wrap_padding))
            .or(fallback)
    }
}

fn write_control_structure_group_open(out: &mut impl SvgOutput, control_id: &str) {
    let _ = write!(
        out,
        r#"<g data-et="control-structure" data-id="i{id}">"#,
        id = escape_attr(control_id)
    );
}

fn write_control_line(
    out: &mut impl SvgOutput,
    coordinates: [f64; 4],
    separator: bool,
    ctx: &SequenceBlockRenderContext<'_>,
) -> Result<()> {
    ctx.frame_paint.include_bounds(out, coordinates)?;
    let [x1, y1, x2, y2] = coordinates;
    let _ = write!(
        out,
        r#"<line x1="{}" y1="{}" x2="{}" y2="{}" class="loopLine""#,
        fmt(x1),
        fmt(y1),
        fmt(x2),
        fmt(y2)
    );
    ctx.frame_paint.write_attributes(out, separator);
    out.push_str("/>");
    out.checkpoint()?;
    Ok(())
}

pub(super) fn block_frame_lines([x1, y1, x2, y2]: [f64; 4]) -> [[f64; 4]; 4] {
    [
        [x1, y1, x2, y1],
        [x2, y1, x2, y2],
        [x1, y2, x2, y2],
        [x1, y1, x1, y2],
    ]
}

pub(super) struct BlockLabelBox {
    pub(super) points: [[f64; 2]; 5],
    pub(super) text_position: [f64; 2],
}

impl BlockLabelBox {
    pub(super) fn new(x1: f64, y1: f64, width: f64, height: f64, margin: f64) -> Self {
        let height = if height == 0.0 { 20.0 } else { height };
        let x2 = x1 + width;
        let y3 = y1 + height;
        let y2 = y3 - 7.0;
        let x3 = x2 - 8.4;
        Self {
            points: [[x1, y1], [x2, y1], [x2, y2], [x3, y3], [x1, y3]],
            text_position: [
                (x1 + width / 2.0).round(),
                sequence_drawn_text_y(
                    sequence_drawn_text_first_y(y1 + height / 2.0, margin),
                    margin,
                    0.0,
                ),
            ],
        }
    }

    pub(super) fn bounds(&self) -> [f64; 4] {
        let [[x1, y1], [x2, _], [_, y2], [x3, y3], _] = self.points;
        [
            x1.min(x2).min(x3),
            y1.min(y2).min(y3),
            x1.max(x2).max(x3),
            y1.max(y2).max(y3),
        ]
    }
}

pub(super) fn write_block_label_box(
    out: &mut impl SvgOutput,
    frame_x1: f64,
    frame_y1: f64,
    label: &str,
    ctx: &SequenceBlockRenderContext<'_>,
) -> Result<()> {
    let neo_height = if ctx.is_neo { 15.0 } else { 0.0 };
    let label_box = BlockLabelBox::new(
        frame_x1,
        frame_y1,
        ctx.label_box_drawn_width,
        ctx.label_box_height + neo_height,
        ctx.box_text_margin,
    );
    let typography = ctx.loop_typography;
    let [[x1, y1], [x2, _], [_, y2], [x3, y3], _] = label_box.points;
    ctx.keyword_paint.include_bounds(out, label_box.bounds())?;
    let _ = write!(
        out,
        r#"<polygon points="{x1},{y1} {x2},{y1} {x2},{y2} {x3},{y3} {x1},{y3}" class="labelBox""#,
        x1 = fmt(x1),
        y1 = fmt(y1),
        x2 = fmt(x2),
        y2 = fmt(y2),
        x3 = fmt(x3),
        y3 = fmt(y3)
    );
    ctx.keyword_paint.write_attributes(out, false);
    out.push_str("/>");
    out.checkpoint()?;
    let [label_cx, label_cy] = label_box.text_position;
    let style = typography.terminal_style(
        "",
        super::settings::sequence_text_style_attribute(ctx.loop_text_style),
    );
    let _ = write!(
        out,
        r#"<text x="{x}" y="{y}" text-anchor="middle" dominant-baseline="middle" alignment-baseline="middle" class="labelText" style="{style}">{label}</text>"#,
        x = fmt(label_cx),
        y = fmt(label_cy),
        style = escape_attr_display(&style),
        label = escape_xml(label)
    );
    out.checkpoint()?;
    Ok(())
}

pub(super) fn render_sequence_block(
    out: &mut impl SvgOutput,
    control_id: &str,
    frame: &SequenceFrameLayout,
    ctx: &SequenceBlockRenderContext<'_>,
) -> Result<()> {
    ctx.checkpoints.checkpoint()?;
    let frame_x1 = frame.x;
    let frame_x2 = frame.x + frame.width;
    let frame_y1 = frame.y;
    let frame_y2 = frame.y + frame.height;
    write_control_structure_group_open(out, control_id);
    for coordinates in block_frame_lines([frame_x1, frame_y1, frame_x2, frame_y2]) {
        write_control_line(out, coordinates, false, ctx)?;
    }
    for (index, section) in frame.sections.iter().enumerate() {
        ctx.checkpoints.checkpoint_loop(index)?;
        write_control_line(out, [frame_x1, section.y, frame_x2, section.y], true, ctx)?;
    }
    // Mermaid labels both parallel forms as "par" in SVG, while the DTO retains their exact kind.
    let keyword = if frame.kind == merman_model::families::sequence::SequenceControlKind::ParOver {
        "par"
    } else {
        frame.kind.keyword()
    };
    write_block_label_box(out, frame_x1, frame_y1, keyword, ctx)?;
    let label_box_right = frame_x1 + ctx.label_box_width;
    let main_text_x = (label_box_right + frame_x2) / 2.0;
    let center_text_x = (frame_x1 + frame_x2) / 2.0;
    let loop_text_ctx = ctx.loop_text_context();
    let label = display_block_label(&frame.label, true).unwrap_or_else(|| "\u{200B}".to_owned());
    write_loop_text_lines(
        out,
        &loop_text_ctx,
        LoopTextPlacement {
            x: main_text_x,
            math_center_x: center_text_x,
            y0: frame_y1 + ctx.box_margin + ctx.box_text_margin,
            block_start_y: frame_y1,
            max_width: ctx.label_wrap_width(&frame.id, Some((frame_x2 - label_box_right).max(0.0))),
            use_tspan: true,
        },
        &frame.id,
        &label,
    )?;
    for (index, section) in frame.sections.iter().enumerate() {
        ctx.checkpoints.checkpoint_loop(index)?;
        let Some(label) = display_block_label(&section.label, false) else {
            continue;
        };
        write_section_title_lines(
            out,
            &loop_text_ctx,
            center_text_x,
            section.y + ctx.box_margin + ctx.box_text_margin,
            section.y,
            ctx.label_wrap_width(&section.id, None),
            &section.id,
            &label,
        )?;
    }
    out.push_str("</g>");
    ctx.checkpoints.checkpoint()
}
