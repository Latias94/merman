use super::super::*;
use super::SequenceEmitCheckpoints;
use super::block_text::{
    LoopTextPlacement, LoopTextRenderContext, display_block_label, write_loop_text_lines,
    write_section_title_lines,
};
use crate::model::SequenceFrameLayout;
use crate::sequence::{
    sequence_block_label_wrap_width, sequence_drawn_text_first_y, sequence_drawn_text_y,
};
use rustc_hash::FxHashMap;

pub(super) struct SequenceBlockRenderContext<'a> {
    pub(super) is_neo: bool,
    pub(super) text_shadow: &'a super::text_effect::SequenceTextShadow<'a>,
    pub(super) shadow_evidence: &'a crate::diagram_theme::SvgShadowEvidenceRecorder,
    pub(super) block_widths_by_id: &'a FxHashMap<String, f64>,
    pub(super) label_box_width: f64,
    pub(super) label_box_height: f64,
    pub(super) box_margin: f64,
    pub(super) box_text_margin: f64,
    pub(super) wrap_padding: f64,
    pub(super) measurer: &'a dyn TextMeasurer,
    pub(super) loop_text_style: &'a TextStyle,
    pub(super) loop_typography: &'a crate::sequence::SequenceResolvedTypography,
    pub(super) typography_receipt: &'a crate::sequence::SequenceTypographyThemeReceipt,
    pub(super) frame_paint: &'a super::control_paint::SequenceControlPaint<'a>,
    pub(super) keyword_paint: &'a super::control_paint::SequenceControlPaint<'a>,
    pub(super) math_sidecar: &'a crate::sequence::SequenceMathSidecar,
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
            typography_receipt: self.typography_receipt,
            math_sidecar: self.math_sidecar,
            checkpoints: self.checkpoints,
            text_shadow: self.text_shadow,
            shadow_evidence: self.shadow_evidence,
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
    let application = ctx.frame_paint.begin_terminal(out, coordinates)?;
    let [x1, y1, x2, y2] = coordinates;
    let _ = write!(
        out,
        r#"<line x1="{}" y1="{}" x2="{}" y2="{}" class="loopLine""#,
        fmt(x1),
        fmt(y1),
        fmt(x2),
        fmt(y2)
    );
    ctx.frame_paint
        .write_attributes(out, application.as_ref(), separator);
    out.push_str("/>");
    out.checkpoint()?;
    ctx.frame_paint
        .finish_terminal(application.as_ref(), ctx.shadow_evidence);
    Ok(())
}

fn write_block_frame(
    out: &mut impl SvgOutput,
    x1: f64,
    x2: f64,
    y1: f64,
    y2: f64,
    ctx: &SequenceBlockRenderContext<'_>,
) -> Result<()> {
    for coordinates in [
        [x1, y1, x2, y1],
        [x2, y1, x2, y2],
        [x1, y2, x2, y2],
        [x1, y1, x1, y2],
    ] {
        write_control_line(out, coordinates, false, ctx)?;
    }
    Ok(())
}

pub(super) fn write_block_label_box(
    out: &mut impl SvgOutput,
    frame_x1: f64,
    frame_y1: f64,
    label: &str,
    ctx: &SequenceBlockRenderContext<'_>,
) -> Result<()> {
    let label_box_width = ctx.label_box_width;
    let neo_height = if ctx.is_neo { 15.0 } else { 0.0 };
    let label_box_height = ctx.label_box_height + neo_height;
    let label_box_height = if label_box_height == 0.0 {
        20.0
    } else {
        label_box_height
    };
    let typography = ctx.loop_typography;
    let typography_receipt = ctx.typography_receipt;
    let x1 = frame_x1;
    let y1 = frame_y1;
    let x2 = x1 + label_box_width;
    let y3 = y1 + label_box_height;
    let y2 = y3 - 7.0;
    let x3 = x2 - 8.4;
    let application = ctx.keyword_paint.begin_terminal(
        out,
        [
            x1.min(x2).min(x3),
            y1.min(y2).min(y3),
            x1.max(x2).max(x3),
            y1.max(y2).max(y3),
        ],
    )?;
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
    ctx.keyword_paint
        .write_attributes(out, application.as_ref(), false);
    out.push_str("/>");
    out.checkpoint()?;
    ctx.keyword_paint
        .finish_terminal(application.as_ref(), ctx.shadow_evidence);
    let label_cx = (x1 + label_box_width / 2.0).round();
    let label_cy = sequence_drawn_text_y(
        sequence_drawn_text_first_y(y1 + label_box_height / 2.0, ctx.box_text_margin),
        ctx.box_text_margin,
        0.0,
    );
    let style = typography.terminal_style(
        "",
        super::settings::sequence_text_style_attribute(ctx.loop_text_style),
    );
    let shadow = if ctx.text_shadow.needs_bounds() {
        let mut terminal_style = typography.terminal_text_style().clone();
        if !typography.requires_resolved_emission() {
            terminal_style.font_size = ctx.loop_text_style.font_size;
        }
        ctx.text_shadow.write_definition(
            out,
            label,
            label_cx,
            label_cy,
            super::text_effect::TextShadowBaseline::Middle,
            &terminal_style,
            ctx.measurer,
        )?
    } else {
        None
    };
    let filter = shadow
        .as_ref()
        .map(|s| format!(" filter=\"{}\"", escape_attr(&s.filter)))
        .unwrap_or_default();
    let _ = write!(
        out,
        r#"<text x="{x}" y="{y}" text-anchor="middle" dominant-baseline="middle" alignment-baseline="middle" class="labelText" style="{style}"{filter}>{label}</text>"#,
        x = fmt(label_cx),
        y = fmt(label_cy),
        style = escape_attr_display(&style),
        label = escape_xml(label)
    );
    typography_receipt.record_terminal_text(crate::sequence::SequenceTextSurface::ControlKeyword);
    out.checkpoint()?;
    ctx.text_shadow.record_terminal(
        shadow.as_ref(),
        false,
        ctx.shadow_evidence,
        typography_receipt,
        crate::sequence::SequenceTextSurface::ControlKeyword,
    );
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
    write_block_frame(out, frame_x1, frame_x2, frame_y1, frame_y2, ctx)?;
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
