use super::super::*;
use super::SequenceEmitCheckpoints;
use super::geometry::node_left_top;
use super::math_label::{
    record_sequence_katex_terminal_emission, sequence_katex_label,
    write_sequence_katex_foreign_object,
};
use crate::sequence::{
    SequenceMathHeightMode, SequenceStaticRectThemeReceipt, sequence_note_final_wrapped_lines,
    sequence_text_line_step_px,
};
use merman_core::diagrams::sequence::{SequenceMessage, SequenceMessageKind};
use rustc_hash::FxHashMap;

use crate::diagram_theme::{SvgFilterRegion, SvgShadowEffect, SvgShadowEvidenceRecorder};
use std::collections::BTreeMap;

#[derive(Default)]
pub(super) struct SequenceNotePaintPlan {
    stroke_width: Option<f32>,
    radius: Option<f32>,
    effect: Option<SvgShadowEffect>,
    shadows: BTreeMap<String, (String, SvgFilterRegion)>,
    pub(super) bounds: Option<Bounds>,
}

impl SequenceNotePaintPlan {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn prepare(
        model: &super::model::SequenceSvgModel,
        nodes: &FxHashMap<&str, &LayoutNode>,
        stroke_width: Option<f32>,
        radius: Option<f32>,
        effect: Option<SvgShadowEffect>,
        receipt: &mut SequenceStaticRectThemeReceipt,
        options: &SvgExecution<'_>,
    ) -> Result<Self> {
        let mut plan = Self {
            stroke_width,
            radius,
            effect,
            ..Self::default()
        };
        if plan.stroke_width.is_none() && plan.effect.is_none() {
            return Ok(plan);
        }
        // Notes have the SVG 1px default; actor strokeWidth does not apply here.
        let width = f64::from(stroke_width.unwrap_or(1.0));
        for (index, message) in model.messages.iter().enumerate() {
            options.work_meter().charge(1)?;
            if message.semantic_kind() != SequenceMessageKind::Note {
                continue;
            }
            let node_id = format!("note-{}", message.id);
            let Some(node) = nodes.get(node_id.as_str()) else {
                continue;
            };
            let (left, top) = node_left_top(node);
            let mut bounds = Bounds {
                min_x: left - width / 2.0,
                min_y: top - width / 2.0,
                max_x: left + node.width + width / 2.0,
                max_y: top + node.height + width / 2.0,
            };
            if let Some(effect) = &plan.effect {
                options
                    .work_meter()
                    .charge(effect.stages().len().saturating_mul(3))?;
                if let Some(materialized) = effect.materialize_rect(
                    &options.theme_resource_policy(),
                    node.width,
                    node.height,
                    width,
                )? {
                    let region = materialized.region();
                    let [x, y, w, h] = region.as_array().map(f64::from);
                    // Use the same outward-rounded region as the filter writer.
                    bounds = Bounds {
                        min_x: left + x * node.width,
                        min_y: top + y * node.height,
                        max_x: left + (x + w) * node.width,
                        max_y: top + (y + h) * node.height,
                    };
                    plan.shadows.insert(
                        node_id,
                        (
                            format!(
                                "{}-note-{index}-theme-effect-{}",
                                options.diagram_id_or("merman"),
                                effect.id()
                            ),
                            region,
                        ),
                    );
                } else {
                    receipt.effect_unhandled = true;
                }
            }
            if let Some(total) = &mut plan.bounds {
                total.min_x = total.min_x.min(bounds.min_x);
                total.min_y = total.min_y.min(bounds.min_y);
                total.max_x = total.max_x.max(bounds.max_x);
                total.max_y = total.max_y.max(bounds.max_y);
            } else {
                plan.bounds = Some(bounds);
            }
        }
        Ok(plan)
    }

    pub(super) fn len(&self) -> usize {
        self.shadows.len()
    }
}

pub(super) struct SequenceNoteRenderContext<'a> {
    pub(super) text_shadow: &'a super::text_effect::SequenceTextShadow<'a>,
    pub(super) paint: &'a SequenceNotePaintPlan,
    pub(super) shadow_evidence: &'a SvgShadowEvidenceRecorder,
    pub(super) nodes_by_id: &'a FxHashMap<&'a str, &'a LayoutNode>,
    pub(super) measurer: &'a dyn TextMeasurer,
    pub(super) legacy_label_font_size: f64,
    pub(super) wrap_padding: f64,
    pub(super) note_text_style: &'a TextStyle,
    pub(super) note_typography: &'a crate::sequence::SequenceResolvedTypography,
    pub(super) typography_receipt: &'a crate::sequence::SequenceTypographyThemeReceipt,
    pub(super) math_sidecar: &'a crate::sequence::SequenceMathSidecar,
    pub(super) checkpoints: SequenceEmitCheckpoints<'a>,
}

pub(super) fn render_sequence_note(
    out: &mut impl SvgOutput,
    message_index: usize,
    msg: &SequenceMessage,
    ctx: &SequenceNoteRenderContext<'_>,
    theme_receipt: &mut SequenceStaticRectThemeReceipt,
) -> Result<()> {
    if msg.semantic_kind() != SequenceMessageKind::Note {
        return Ok(());
    }
    ctx.checkpoints.checkpoint()?;

    let id = &msg.id;
    let raw = msg.message_text();
    let node_id = format!("note-{id}");
    let Some(n) = ctx.nodes_by_id.get(node_id.as_str()).copied() else {
        ctx.typography_receipt
            .record_missing_text_effect(crate::sequence::SequenceTextSurface::NoteLabel);
        return Ok(());
    };
    let (x, y) = node_left_top(n);
    let cx = x + (n.width / 2.0);
    let text_y = y + 5.0;
    let line_step = sequence_text_line_step_px(ctx.note_text_style.font_size);
    let shadow = ctx
        .paint
        .shadows
        .get(&node_id)
        .zip(ctx.paint.effect.as_ref());
    let filter = shadow.map(|((id, region), effect)| {
        super::super::shadow::write_theme_shadow_application(out, id, effect, *region)
    });
    let _ = write!(out, r#"<g data-et="note" data-id="i{}">"#, escape_attr(id));
    let _ = write!(
        &mut *out,
        r##"<rect x="{x}" y="{y}" fill="#EDF2AE" stroke="#666" width="{w}" height="{h}" class="note""##,
        x = fmt(x),
        y = fmt(y),
        w = fmt(n.width),
        h = fmt(n.height)
    );
    if let Some(width) = ctx.paint.stroke_width {
        let _ = write!(out, r#" stroke-width="{}""#, fmt(f64::from(width)));
    }
    if let Some(radius) = ctx.paint.radius {
        let _ = write!(out, r#" rx="{r}" ry="{r}""#, r = fmt(f64::from(radius)));
    }
    if let Some(filter) = &filter {
        let _ = write!(out, r#" filter="{}""#, escape_attr(filter));
    }
    out.push_str("/>");
    theme_receipt.record_rect_emission();
    if let Some(((id, region), effect)) = shadow {
        ctx.shadow_evidence.record_application(effect, id, *region);
        theme_receipt.record_effect_emission();
    } else if theme_receipt.effect_cleared {
        theme_receipt.record_effect_emission();
    }
    let prepared_math =
        ctx.math_sidecar
            .terminal_for_occurrence(&crate::sequence::SequenceMathOccurrence::Note(
                message_index,
            ));
    if let Some(katex) = sequence_katex_label(
        prepared_math,
        ctx.note_text_style,
        SequenceMathHeightMode::Draw,
    ) {
        write_sequence_katex_foreign_object(
            out,
            &katex,
            (x + n.width / 2.0 - katex.width / 2.0).round(),
            (y + n.height / 2.0 - katex.height / 2.0).round(),
        );
        record_sequence_katex_terminal_emission(
            ctx.typography_receipt,
            crate::sequence::SequenceTextSurface::NoteLabel,
            &katex,
        );
        ctx.text_shadow.record_terminal(
            None,
            false,
            ctx.shadow_evidence,
            ctx.typography_receipt,
            crate::sequence::SequenceTextSurface::NoteLabel,
        );
    } else if msg.wrap {
        // Mermaid@11.12.2 (Sequence) wraps notes *after* placement width is known:
        //   noteModel.message = wrapLabel(msg.message, noteModel.width - 2*wrapPadding, noteFont)
        //
        // Layout already computed the note box width (`n.width`) to match Mermaid's
        // `noteModel.width`, so wrap to `n.width - 2*wrapPadding` here.
        let lines = sequence_note_final_wrapped_lines(
            raw,
            n.width,
            2.0 * ctx.wrap_padding,
            ctx.measurer,
            ctx.note_text_style,
            ctx.checkpoints.text(),
        )?;
        render_sequence_note_lines(
            out,
            lines.iter().map(String::as_str),
            cx,
            text_y,
            line_step,
            ctx.legacy_label_font_size,
            Some(ctx.note_typography),
            Some(ctx.typography_receipt),
            Some(ctx),
            ctx.checkpoints,
        )?;
    } else {
        render_sequence_note_lines(
            out,
            crate::text::split_html_br_lines(raw),
            cx,
            text_y,
            line_step,
            ctx.legacy_label_font_size,
            Some(ctx.note_typography),
            Some(ctx.typography_receipt),
            Some(ctx),
            ctx.checkpoints,
        )?;
    }
    out.push_str("</g>");
    ctx.checkpoints.checkpoint()
}

fn render_sequence_note_lines<'a>(
    out: &mut impl SvgOutput,
    lines: impl IntoIterator<Item = &'a str>,
    cx: f64,
    text_y: f64,
    line_step: f64,
    legacy_label_font_size: f64,
    typography: Option<&crate::sequence::SequenceResolvedTypography>,
    typography_receipt: Option<&crate::sequence::SequenceTypographyThemeReceipt>,
    paint_context: Option<&SequenceNoteRenderContext<'_>>,
    checkpoints: SequenceEmitCheckpoints<'_>,
) -> Result<()> {
    for (i, line) in lines.into_iter().enumerate() {
        checkpoints.checkpoint_loop(i)?;
        let decoded = merman_core::entities::decode_mermaid_entities_to_unicode(line);
        let text = if decoded.as_ref().is_empty() {
            "\u{200B}"
        } else {
            decoded.as_ref()
        };
        let y = text_y + (i as f64) * line_step;
        let legacy_style = format!(
            "font-size: {}px; font-weight: 400;",
            fmt(legacy_label_font_size)
        );
        let style = typography.map_or(legacy_style.clone(), |typography| {
            typography.terminal_style("", legacy_style)
        });
        // SVG whitespace and the zero-width placeholder have no painted glyphs.
        // Keep the line for layout, but do not create an empty native filter group.
        let paintless = paint_context.is_some_and(|ctx| ctx.text_shadow.is_paintless(text));
        let shadow = paint_context
            .filter(|_| !paintless)
            .map(|ctx| {
                ctx.text_shadow.write_definition(
                    out,
                    text,
                    cx,
                    y,
                    super::text_effect::TextShadowBaseline::NoteMiddle,
                    ctx.note_typography.terminal_text_style(),
                    ctx.measurer,
                )
            })
            .transpose()?
            .flatten();
        let filter = shadow
            .as_ref()
            .map(|shadow| format!(" filter=\"{}\"", escape_attr(&shadow.filter)))
            .unwrap_or_default();
        let _ = write!(
            &mut *out,
            r#"<text x="{x}" y="{y}" text-anchor="middle" dominant-baseline="middle" alignment-baseline="middle" class="noteText" dy="1em" style="{style}"{filter}><tspan x="{x}">{text}</tspan></text>"#,
            x = fmt(cx),
            y = fmt(y),
            style = escape_attr_display(&style),
            text = escape_xml(text)
        );
        if let Some(receipt) = typography_receipt {
            receipt.record_terminal_text(crate::sequence::SequenceTextSurface::NoteLabel);
        }
        out.checkpoint()?;
        if let Some(ctx) = paint_context {
            ctx.text_shadow.record_terminal(
                shadow.as_ref(),
                paintless,
                ctx.shadow_evidence,
                ctx.typography_receipt,
                crate::sequence::SequenceTextSurface::NoteLabel,
            );
        }
    }
    checkpoints.checkpoint()
}

#[cfg(test)]
mod tests {
    use crate::resources::{OperationWorkMeter, RenderResourcePolicy};

    #[test]
    fn empty_note_rows_render_the_upstream_zero_width_space() {
        let mut out = String::new();
        let meter = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        super::render_sequence_note_lines(
            &mut out,
            ["first", "", "last"],
            50.0,
            10.0,
            19.0,
            16.0,
            None,
            None,
            None,
            super::SequenceEmitCheckpoints::for_emit(&meter),
        )
        .unwrap();

        assert!(out.contains("<tspan x=\"50\">\u{200b}</tspan>"), "{out}");
    }
}
