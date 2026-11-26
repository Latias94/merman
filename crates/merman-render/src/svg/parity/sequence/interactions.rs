use super::super::*;
use super::SequenceEmitCheckpoints;
use super::blocks::{SequenceBlockRenderContext, render_sequence_block};
use super::model::*;
use super::notes::{SequenceNoteRenderContext, render_sequence_note};
use super::settings::SequenceRenderSettings;
use merman_render_sequence::activation::SequenceActivationPlan;
use rustc_hash::FxHashMap;

pub(super) struct SequenceInteractionRenderContext<'a> {
    pub(super) frame_paint: &'a super::control_paint::SequenceControlPaint,
    pub(super) keyword_paint: &'a super::control_paint::SequenceControlPaint,
    pub(super) note_paint: &'a super::notes::SequenceNotePaintPlan,
    pub(super) model: &'a SequenceSvgModel,
    pub(super) block_widths_by_id: &'a FxHashMap<String, f64>,
    pub(super) frames: &'a [crate::model::SequenceFrameLayout],
    pub(super) frame_placements: &'a [Option<usize>],
    pub(super) nodes_by_id: &'a FxHashMap<&'a str, &'a LayoutNode>,
    pub(super) math_sidecar: &'a crate::sequence::SequenceMathSidecar,
    pub(super) sanitize_config: &'a merman_model::presentation::PresentationConfig,
    pub(super) compat: &'a crate::sequence::SequenceCompatBinding,
    pub(super) settings: &'a SequenceRenderSettings,
    pub(super) typography: &'a crate::sequence::SequenceTypographyPlan,
    pub(super) block_label_box_metrics: crate::sequence::SequenceBlockLabelBoxMetrics,
    pub(super) measurer: &'a dyn TextMeasurer,
    pub(super) checkpoints: SequenceEmitCheckpoints<'a>,
}

pub(super) fn render_sequence_interaction_overlays(
    out: &mut impl SvgOutput,
    ctx: &SequenceInteractionRenderContext<'_>,
    activation_plan: &SequenceActivationPlan<'_>,
) -> Result<()> {
    ctx.checkpoints.checkpoint()?;

    let block_ctx = SequenceBlockRenderContext {
        is_neo: ctx.compat.is_neo,
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
        frame_paint: ctx.frame_paint,
        keyword_paint: ctx.keyword_paint,
        sanitize_config: ctx.sanitize_config,
        math_sidecar: ctx.math_sidecar,
        checkpoints: ctx.checkpoints,
    };
    let note_ctx = SequenceNoteRenderContext {
        is_neo: ctx.compat.is_neo,
        paint: ctx.note_paint,
        nodes_by_id: ctx.nodes_by_id,
        measurer: ctx.measurer,
        note_margin: ctx.settings.note_margin,
        wrap_padding: ctx.settings.wrap_padding,
        note_text_style: &ctx.settings.note_text_style,
        note_typography: ctx.typography.note(),
        sanitize_config: ctx.sanitize_config,
        math_sidecar: ctx.math_sidecar,
        checkpoints: ctx.checkpoints,
    };

    for (message_index, msg) in ctx.model.messages.iter().enumerate() {
        ctx.checkpoints.checkpoint_loop(message_index)?;
        activation_plan.write_group(out, &msg.id)?;
        render_sequence_note(out, message_index, msg, &note_ctx)?;

        let Some(index) = ctx.frame_placements.get(message_index).copied().flatten() else {
            continue;
        };
        render_sequence_block(out, &msg.id, &ctx.frames[index], &block_ctx)?;
    }
    ctx.checkpoints.checkpoint()
}
