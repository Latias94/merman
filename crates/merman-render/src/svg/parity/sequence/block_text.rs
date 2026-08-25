use super::super::*;
use super::SequenceEmitCheckpoints;
use super::math_label::{
    record_sequence_katex_terminal_emission, sequence_katex_label,
    write_sequence_katex_foreign_object,
};
use crate::sequence::{
    SequenceMathHeightMode, bracketize_sequence_block_label, sequence_text_line_step_px,
};

pub(super) struct LoopTextRenderContext<'a> {
    pub(super) measurer: &'a dyn TextMeasurer,
    pub(super) style: &'a TextStyle,
    typography: &'a crate::sequence::SequenceResolvedTypography,
    typography_receipt: &'a crate::sequence::SequenceTypographyThemeReceipt,
    math_sidecar: &'a crate::sequence::SequenceMathSidecar,
    checkpoints: SequenceEmitCheckpoints<'a>,
}

pub(super) struct LoopTextPlacement {
    pub(super) x: f64,
    pub(super) y0: f64,
    pub(super) block_start_y: f64,
    pub(super) max_width: Option<f64>,
    pub(super) use_tspan: bool,
}

impl<'a> LoopTextRenderContext<'a> {
    pub(super) fn new(
        measurer: &'a dyn TextMeasurer,
        style: &'a TextStyle,
        typography: &'a crate::sequence::SequenceResolvedTypography,
        typography_receipt: &'a crate::sequence::SequenceTypographyThemeReceipt,
        math_sidecar: &'a crate::sequence::SequenceMathSidecar,
        checkpoints: SequenceEmitCheckpoints<'a>,
    ) -> Self {
        Self {
            measurer,
            style,
            typography,
            typography_receipt,
            math_sidecar,
            checkpoints,
        }
    }

    fn katex_label(
        &self,
        occurrence_id: &str,
    ) -> Option<super::math_label::SequenceKatexLabel<'_>> {
        let prepared = self.math_sidecar.terminal_for_occurrence(
            &crate::sequence::SequenceMathOccurrence::BlockLabel(occurrence_id.to_owned()),
        );
        sequence_katex_label(prepared, self.style, SequenceMathHeightMode::Draw)
    }
}

pub(super) fn display_block_label(raw_label: &str, always_show: bool) -> Option<String> {
    let decoded = merman_core::entities::decode_mermaid_entities_to_unicode(raw_label);
    let t = decoded.as_ref().trim();
    if t.is_empty() {
        if always_show {
            // Mermaid renders empty block labels as a zero-width space inside `<tspan>`.
            Some("\u{200B}".to_string())
        } else {
            None
        }
    } else {
        Some(bracketize_sequence_block_label(t))
    }
}

pub(super) fn wrap_svg_text_lines(
    text: &str,
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
    max_width: Option<f64>,
    checkpoints: SequenceEmitCheckpoints<'_>,
) -> Result<Vec<String>> {
    let lines = if let Some(width) = max_width {
        crate::sequence::wrap_sequence_label_like_mermaid_lines(
            text,
            measurer,
            style,
            width,
            checkpoints.text(),
        )?
    } else {
        let split_lines = crate::text::split_html_br_lines(text);
        let mut lines = Vec::with_capacity(split_lines.len());
        for line in split_lines {
            checkpoints.checkpoint()?;
            lines.push(line.to_string());
        }
        lines
    };
    if lines.is_empty() {
        Ok(vec!["".to_string()])
    } else {
        Ok(lines)
    }
}

pub(super) fn write_loop_text_lines(
    out: &mut impl SvgOutput,
    ctx: &LoopTextRenderContext<'_>,
    placement: LoopTextPlacement,
    occurrence_id: &str,
    text: &str,
) -> Result<()> {
    ctx.checkpoints.checkpoint()?;
    if let Some(katex) = ctx.katex_label(occurrence_id) {
        let x = (placement.x - katex.width / 2.0).round();
        write_sequence_katex_foreign_object(out, &katex, x, placement.block_start_y.round());
        record_sequence_katex_terminal_emission(
            ctx.typography_receipt,
            crate::sequence::SequenceTextSurface::ControlPrimaryTitle,
            &katex,
        );
        return ctx.checkpoints.checkpoint();
    }

    let line_step = sequence_text_line_step_px(ctx.style.font_size);
    let lines = wrap_svg_text_lines(
        text,
        ctx.measurer,
        ctx.style,
        placement.max_width,
        ctx.checkpoints,
    )?;
    for (i, line) in lines.into_iter().enumerate() {
        ctx.checkpoints.checkpoint_loop(i)?;
        let y = placement.y0 + (i as f64) * line_step;
        let legacy_style = format!(
            "font-size: {}px; font-weight: 400;",
            fmt(ctx.style.font_size)
        );
        let style = ctx.typography.terminal_style("", legacy_style);
        if placement.use_tspan {
            let _ = write!(
                out,
                r#"<text x="{x}" y="{y}" text-anchor="middle" class="loopText" style="{style}"><tspan x="{x}">{text}</tspan></text>"#,
                x = fmt(placement.x),
                y = fmt(y),
                style = escape_attr_display(&style),
                text = escape_xml(&line)
            );
        } else {
            let _ = write!(
                out,
                r#"<text x="{x}" y="{y}" text-anchor="middle" class="loopText" style="{style}">{text}</text>"#,
                x = fmt(placement.x),
                y = fmt(y),
                style = escape_attr_display(&style),
                text = escape_xml(&line)
            );
        }
        ctx.typography_receipt
            .record_terminal_text(crate::sequence::SequenceTextSurface::ControlPrimaryTitle);
    }
    ctx.checkpoints.checkpoint()
}

pub(super) fn write_section_title_lines(
    out: &mut impl SvgOutput,
    ctx: &LoopTextRenderContext<'_>,
    x: f64,
    y0: f64,
    section_start_y: f64,
    max_width: Option<f64>,
    occurrence_id: &str,
    text: &str,
) -> Result<()> {
    ctx.checkpoints.checkpoint()?;
    if let Some(katex) = ctx.katex_label(occurrence_id) {
        let x = (x - katex.width / 2.0).round();
        let y = (section_start_y - katex.height).round();
        write_sequence_katex_foreign_object(out, &katex, x, y);
        record_sequence_katex_terminal_emission(
            ctx.typography_receipt,
            crate::sequence::SequenceTextSurface::ControlSectionTitle,
            &katex,
        );
        return ctx.checkpoints.checkpoint();
    }

    let line_step = sequence_text_line_step_px(ctx.style.font_size);
    let lines = wrap_svg_text_lines(text, ctx.measurer, ctx.style, max_width, ctx.checkpoints)?;
    for (i, line) in lines.into_iter().enumerate() {
        ctx.checkpoints.checkpoint_loop(i)?;
        let y = y0 + (i as f64) * line_step;
        let legacy_style = format!(
            "font-size: {}px; font-weight: 400;",
            fmt(ctx.style.font_size)
        );
        let style = ctx.typography.terminal_style("", legacy_style);
        let _ = write!(
            out,
            r#"<text x="{x}" y="{y}" text-anchor="middle" class="sectionTitle" style="{style}">{text}</text>"#,
            x = fmt(x),
            y = fmt(y),
            style = escape_attr_display(&style),
            text = escape_xml(&line)
        );
        ctx.typography_receipt
            .record_terminal_text(crate::sequence::SequenceTextSurface::ControlSectionTitle);
    }
    ctx.checkpoints.checkpoint()
}
