use super::super::*;
use super::SequenceEmitCheckpoints;
use super::block_text::{
    LoopTextPlacement, LoopTextRenderContext, display_block_label, write_loop_text_lines,
    write_section_title_lines,
};
use crate::model::SequenceBlockLayout;
use crate::sequence::{
    AltSection, SequenceBlockGeometry, resolved_block_frame_x, sequence_block_label_wrap_width,
    sequence_block_section_geometry,
};
use rustc_hash::FxHashMap;

pub(super) struct SequenceBlockRenderContext<'a> {
    pub(super) text_shadow: &'a super::text_effect::SequenceTextShadow<'a>,
    pub(super) shadow_evidence: &'a crate::diagram_theme::SvgShadowEvidenceRecorder,
    pub(super) default_frame_x1: f64,
    pub(super) default_frame_x2: f64,
    pub(super) block_widths_by_id: &'a FxHashMap<String, f64>,
    pub(super) actor_nodes_by_id: &'a FxHashMap<&'a str, &'a LayoutNode>,
    pub(super) label_box_width: f64,
    pub(super) label_box_height: f64,
    pub(super) wrap_padding: f64,
    pub(super) measurer: &'a dyn TextMeasurer,
    pub(super) loop_text_style: &'a TextStyle,
    pub(super) loop_typography: &'a crate::sequence::SequenceResolvedTypography,
    pub(super) typography_receipt: &'a crate::sequence::SequenceTypographyThemeReceipt,
    pub(super) loop_theme_receipt: &'a crate::sequence::SequenceLoopThemeReceipt,
    pub(super) math_sidecar: &'a crate::sequence::SequenceMathSidecar,
    pub(super) checkpoints: SequenceEmitCheckpoints<'a>,
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
        LoopTextRenderContext {
            measurer: self.measurer,
            style: self.loop_text_style,
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

pub(super) fn write_block_frame(
    out: &mut impl SvgOutput,
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
    out: &mut impl SvgOutput,
    frame_x1: f64,
    frame_y1: f64,
    label: &str,
    ctx: &SequenceBlockRenderContext<'_>,
) -> Result<()> {
    let label_box_width = ctx.label_box_width;
    let label_box_height = ctx.label_box_height;
    let typography = ctx.loop_typography;
    let typography_receipt = ctx.typography_receipt;
    let theme_receipt = ctx.loop_theme_receipt;
    let x1 = frame_x1;
    let y1 = frame_y1;
    let x2 = x1 + label_box_width;
    let y3 = y1 + label_box_height;
    let y2 = (y3 - 7.0).max(y1);
    let x3 = x2 - 8.4;
    theme_receipt.record_surface_candidate();
    let surface_emitted = write!(
        out,
        r#"<polygon points="{x1},{y1} {x2},{y1} {x2},{y2} {x3},{y3} {x1},{y3}" class="labelBox"/>"#,
        x1 = fmt(x1),
        y1 = fmt(y1),
        x2 = fmt(x2),
        y2 = fmt(y2),
        x3 = fmt(x3),
        y3 = fmt(y3)
    )
    .is_ok();
    if surface_emitted {
        theme_receipt.record_surface_emission();
    }
    let label_cx = (x1 + label_box_width / 2.0).round();
    let label_cy = y1 + (label_box_height / 2.0).max(13.0);
    let style = typography.terminal_style("", "font-size: 16px; font-weight: 400;".to_string());
    let shadow = if ctx.text_shadow.needs_bounds() {
        let mut terminal_style = typography.terminal_text_style().clone();
        if !typography.requires_resolved_emission() {
            terminal_style.font_size = 16.0;
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

pub(super) fn render_simple_sequence_block(
    out: &mut impl SvgOutput,
    block: SimpleSequenceBlock<'_>,
    ctx: &SequenceBlockRenderContext<'_>,
) -> Result<()> {
    ctx.checkpoints.checkpoint()?;
    let Some(layout) = block.layout else {
        ctx.typography_receipt
            .record_missing_text_effect(crate::sequence::SequenceTextSurface::ControlKeyword);
        return Ok(());
    };
    let Some((frame_x1, frame_x2, _min_left)) = resolved_block_frame_x(
        block.geometry,
        layout,
        ctx.actor_nodes_by_id,
        (ctx.default_frame_x1, ctx.default_frame_x2),
        None,
    ) else {
        ctx.typography_receipt
            .record_missing_text_effect(crate::sequence::SequenceTextSurface::ControlKeyword);
        return Ok(());
    };
    let frame_x2 = frame_x2.max(frame_x1 + ctx.label_box_width);

    let frame_y1 = layout.start_y;
    let frame_y2 = layout.stop_y;

    write_control_structure_group_open(out, block.control_id);
    write_block_frame(out, frame_x1, frame_x2, frame_y1, frame_y2);
    write_block_label_box(out, frame_x1, frame_y1, block.block_label, ctx)?;
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
        block.label_id,
        &label,
    )?;
    out.push_str("</g>");
    ctx.checkpoints.checkpoint()
}

fn section_separator_ys(
    sections: &[AltSection<'_>],
    checkpoints: SequenceEmitCheckpoints<'_>,
) -> Result<Option<Vec<f64>>> {
    let mut separator_ys = Vec::with_capacity(sections.len().saturating_sub(1));
    for (section_index, section) in sections.iter().skip(1).enumerate() {
        checkpoints.checkpoint_loop(section_index)?;
        let Some(separator_y) = section.separator_y else {
            return Ok(None);
        };
        separator_ys.push(separator_y);
    }
    checkpoints.checkpoint()?;
    Ok(Some(separator_ys))
}

pub(super) fn render_sectioned_sequence_block(
    out: &mut impl SvgOutput,
    control_id: &str,
    block_label: &str,
    sections: &[AltSection<'_>],
    layout: Option<&SequenceBlockLayout>,
    ctx: &SequenceBlockRenderContext<'_>,
) -> Result<()> {
    if sections.is_empty() {
        ctx.typography_receipt
            .record_missing_text_effect(crate::sequence::SequenceTextSurface::ControlKeyword);
        return Ok(());
    }

    let geometry = sequence_block_section_geometry(sections);
    let Some(layout) = layout else {
        ctx.typography_receipt
            .record_missing_text_effect(crate::sequence::SequenceTextSurface::ControlKeyword);
        return Ok(());
    };
    let Some(sep_ys) = section_separator_ys(sections, ctx.checkpoints)? else {
        ctx.typography_receipt
            .record_missing_text_effect(crate::sequence::SequenceTextSurface::ControlKeyword);
        return Ok(());
    };
    ctx.checkpoints.checkpoint()?;

    let Some((frame_x1, frame_x2, _min_left)) = resolved_block_frame_x(
        geometry,
        layout,
        ctx.actor_nodes_by_id,
        (ctx.default_frame_x1, ctx.default_frame_x2),
        None,
    ) else {
        ctx.typography_receipt
            .record_missing_text_effect(crate::sequence::SequenceTextSurface::ControlKeyword);
        return Ok(());
    };
    let frame_x2 = frame_x2.max(frame_x1 + ctx.label_box_width);

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
    for (separator_index, y) in sep_ys.iter().enumerate() {
        ctx.checkpoints.checkpoint_loop(separator_index)?;
        let _ = write!(
            out,
            r#"<line x1="{x1}" y1="{y}" x2="{x2}" y2="{y}" class="loopLine" style="stroke-dasharray: 3, 3;"/>"#,
            x1 = fmt(dash_x1),
            x2 = fmt(dash_x2),
            y = fmt(*y)
        );
    }

    // label box + label text
    write_block_label_box(out, frame_x1, frame_y1, block_label, ctx)?;

    // section labels
    let label_box_right = frame_x1 + ctx.label_box_width;
    let main_text_x = (label_box_right + frame_x2) / 2.0;
    let center_text_x = (frame_x1 + frame_x2) / 2.0;
    for (i, sec) in sections.iter().enumerate() {
        ctx.checkpoints.checkpoint_loop(i)?;
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
                sec.label_id,
                &label_text,
            )?;
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
            sec.label_id,
            &label_text,
        )?;
    }

    out.push_str("</g>");
    ctx.checkpoints.checkpoint()
}

pub(super) fn render_critical_sequence_block(
    out: &mut impl SvgOutput,
    control_id: &str,
    sections: &[AltSection<'_>],
    layout: Option<&SequenceBlockLayout>,
    ctx: &SequenceBlockRenderContext<'_>,
) -> Result<()> {
    if sections.is_empty() {
        ctx.typography_receipt
            .record_missing_text_effect(crate::sequence::SequenceTextSurface::ControlKeyword);
        return Ok(());
    }

    let geometry = sequence_block_section_geometry(sections);
    let Some(layout) = layout else {
        ctx.typography_receipt
            .record_missing_text_effect(crate::sequence::SequenceTextSurface::ControlKeyword);
        return Ok(());
    };
    let Some(sep_ys) = section_separator_ys(sections, ctx.checkpoints)? else {
        ctx.typography_receipt
            .record_missing_text_effect(crate::sequence::SequenceTextSurface::ControlKeyword);
        return Ok(());
    };
    ctx.checkpoints.checkpoint()?;

    let Some((frame_x1, frame_x2, _min_left)) = resolved_block_frame_x(
        geometry,
        layout,
        ctx.actor_nodes_by_id,
        (ctx.default_frame_x1, ctx.default_frame_x2),
        Some(sections.len()),
    ) else {
        ctx.typography_receipt
            .record_missing_text_effect(crate::sequence::SequenceTextSurface::ControlKeyword);
        return Ok(());
    };
    let frame_x2 = frame_x2.max(frame_x1 + ctx.label_box_width);

    let frame_y1 = layout.start_y;
    let frame_y2 = layout.stop_y;

    write_control_structure_group_open(out, control_id);

    // frame
    write_block_frame(out, frame_x1, frame_x2, frame_y1, frame_y2);

    // separators (dashed)
    let dash_x1 = frame_x1;
    let dash_x2 = frame_x2;
    for (separator_index, y) in sep_ys.iter().enumerate() {
        ctx.checkpoints.checkpoint_loop(separator_index)?;
        let _ = write!(
            out,
            r#"<line x1="{x1}" y1="{y}" x2="{x2}" y2="{y}" class="loopLine" style="stroke-dasharray: 3, 3;"/>"#,
            x1 = fmt(dash_x1),
            x2 = fmt(dash_x2),
            y = fmt(*y)
        );
    }

    // label box + label text
    write_block_label_box(out, frame_x1, frame_y1, "critical", ctx)?;

    // section labels
    let label_box_right = frame_x1 + ctx.label_box_width;
    let main_text_x = (label_box_right + frame_x2) / 2.0;
    let center_text_x = (frame_x1 + frame_x2) / 2.0;
    for (i, sec) in sections.iter().enumerate() {
        ctx.checkpoints.checkpoint_loop(i)?;
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
                sec.label_id,
                &label_text,
            )?;
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
            sec.label_id,
            &label_text,
        )?;
    }

    out.push_str("</g>");
    ctx.checkpoints.checkpoint()
}
