use super::super::*;
use super::SequenceEmitCheckpoints;
use super::activation::{SequenceActivationPlan, render_sequence_activation_group};
use super::blocks::{SequenceBlockRenderContext, render_sequence_block};
use super::model::*;
use super::notes::{SequenceNoteRenderContext, render_sequence_note};
use super::settings::SequenceRenderSettings;
use crate::sequence::{SequenceFramePlacement, SequenceStaticRectThemeReceipt};
use rustc_hash::FxHashMap;

pub(super) struct SequenceInteractionRenderContext<'a> {
    pub(super) frame_paint: &'a super::control_paint::SequenceControlPaint<'a>,
    pub(super) keyword_paint: &'a super::control_paint::SequenceControlPaint<'a>,
    pub(super) loop_text_shadow: &'a super::text_effect::SequenceTextShadow<'a>,
    pub(super) note_text_shadow: &'a super::text_effect::SequenceTextShadow<'a>,
    pub(super) note_paint: &'a super::notes::SequenceNotePaintPlan,
    pub(super) shadow_evidence: &'a crate::diagram_theme::SvgShadowEvidenceRecorder,
    pub(super) model: &'a SequenceSvgModel,
    pub(super) block_widths_by_id: &'a FxHashMap<String, f64>,
    pub(super) frames: &'a [crate::model::SequenceFrameLayout],
    pub(super) frame_placements: &'a [Option<SequenceFramePlacement>],
    pub(super) nodes_by_id: &'a FxHashMap<&'a str, &'a LayoutNode>,
    pub(super) math_sidecar: &'a crate::sequence::SequenceMathSidecar,
    pub(super) wrapped_note_text: &'a crate::sequence::SequencePreparedNoteLabels,
    pub(super) sanitize_config: &'a merman_model::presentation::PresentationConfig,
    pub(super) compat: &'a crate::sequence::SequenceCompatBinding,
    pub(super) settings: &'a SequenceRenderSettings,
    pub(super) typography: &'a crate::sequence::SequenceTypographyPlan,
    pub(super) block_label_box_metrics: crate::sequence::SequenceBlockLabelBoxMetrics,
    pub(super) measurer: &'a dyn TextMeasurer,
    pub(super) typography_receipt: &'a crate::sequence::SequenceTypographyThemeReceipt,
    pub(super) checkpoints: SequenceEmitCheckpoints<'a>,
}

pub(super) fn render_sequence_interaction_overlays(
    out: &mut impl SvgOutput,
    ctx: &SequenceInteractionRenderContext<'_>,
    activation_plan: &SequenceActivationPlan<'_>,
    note_theme_receipt: &mut SequenceStaticRectThemeReceipt,
    activation_theme_receipt: &mut SequenceStaticRectThemeReceipt,
) -> Result<()> {
    ctx.checkpoints.checkpoint()?;

    let block_ctx = SequenceBlockRenderContext {
        is_neo: ctx.compat.is_neo,
        text_shadow: ctx.loop_text_shadow,
        shadow_evidence: ctx.shadow_evidence,
        block_widths_by_id: ctx.block_widths_by_id,
        label_box_width: ctx.block_label_box_metrics.width(),
        label_box_drawn_width: ctx.block_label_box_metrics.drawn_width(),
        label_box_height: if ctx.block_label_box_metrics.typography_expanded() {
            ctx.block_label_box_metrics.terminal_height()
        } else {
            ctx.settings.label_box_height
        },
        box_margin: ctx.settings.box_margin,
        box_text_margin: ctx.settings.box_text_margin,
        wrap_padding: ctx.settings.wrap_padding,
        measurer: ctx.measurer,
        loop_text_style: &ctx.settings.loop_text_style,
        loop_typography: ctx.typography.loop_label(),
        typography_receipt: ctx.typography_receipt,
        frame_paint: ctx.frame_paint,
        keyword_paint: ctx.keyword_paint,
        sanitize_config: ctx.sanitize_config,
        math_sidecar: ctx.math_sidecar,
        checkpoints: ctx.checkpoints,
    };
    let note_wrap_widths = ctx.wrapped_note_text.wrap_widths();
    let note_ctx = SequenceNoteRenderContext {
        is_neo: ctx.compat.is_neo,
        text_shadow: ctx.note_text_shadow,
        paint: ctx.note_paint,
        shadow_evidence: ctx.shadow_evidence,
        nodes_by_id: ctx.nodes_by_id,
        wrapped_note_text: ctx.wrapped_note_text.view(
            ctx.model,
            &crate::sequence::SequenceWrappedSideNoteLabelContext {
                measurer: ctx.measurer,
                measurement_style: &ctx.settings.note_text_style,
                terminal_style: ctx.typography.note().terminal_text_style(),
                config: ctx.sanitize_config,
                default_width: note_wrap_widths.default_width,
                note_margin: note_wrap_widths.note_margin,
                wrap_padding: note_wrap_widths.wrap_padding,
                checkpoints: ctx.checkpoints.text(),
            },
        ),
        measurer: ctx.measurer,
        note_wrap_widths,
        note_margin: ctx.settings.note_margin,
        wrap_padding: ctx.settings.wrap_padding,
        note_text_style: &ctx.settings.note_text_style,
        note_typography: ctx.typography.note(),
        sanitize_config: ctx.sanitize_config,
        typography_receipt: ctx.typography_receipt,
        math_sidecar: ctx.math_sidecar,
        checkpoints: ctx.checkpoints,
    };

    for (message_index, msg) in ctx.model.messages.iter().enumerate() {
        ctx.checkpoints.checkpoint_loop(message_index)?;
        render_sequence_activation_group(
            out,
            activation_plan,
            &msg.id,
            ctx.compat,
            activation_theme_receipt,
            ctx.shadow_evidence,
        )?;
        render_sequence_note(out, message_index, msg, &note_ctx, note_theme_receipt)?;

        let Some(placement) = ctx.frame_placements.get(message_index).copied().flatten() else {
            continue;
        };
        match placement {
            SequenceFramePlacement::Draw(index) => {
                render_sequence_block(out, &msg.id, &ctx.frames[index], &block_ctx)?;
            }
            SequenceFramePlacement::Empty => ctx
                .typography_receipt
                .record_missing_text_effect(crate::sequence::SequenceTextSurface::ControlKeyword),
        }
    }
    ctx.checkpoints.checkpoint()
}
