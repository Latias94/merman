use super::super::*;
use super::actor_man_glyphs::{
    ActorManBottomGlyphMetrics, write_actor_man_bottom_glyph, write_actor_man_top_glyph,
};
use super::actor_shapes::is_actor_man_variant;
use super::math_label::sequence_katex_label;

pub(super) fn render_sequence_actor_man_tops(
    out: &mut impl SvgOutput,
    model: &super::model::SequenceSvgModel,
    nodes_by_id: &rustc_hash::FxHashMap<&str, &LayoutNode>,
    actor_height: f64,
    diagram_id: SvgDiagramId<'_>,
    label_ctx: &super::actor_shapes::ActorLabelContext<'_>,
) -> Result<()> {
    let actor_text_style = label_ctx.style;
    let math_sidecar = label_ctx.math_sidecar;
    let checkpoints = label_ctx.checkpoints;
    // Actor-man variants (actor/boundary/control/entity) are emitted after `<defs>`.
    for (actor_idx, actor_id) in model.actor_order.iter().enumerate() {
        checkpoints.checkpoint_loop(actor_idx)?;
        let Some(actor) = model.actors.get(actor_id) else {
            continue;
        };
        let actor_type = actor.actor_type.as_str();
        if !is_actor_man_variant(actor_type) {
            continue;
        }
        let node_id = format!("actor-top-{actor_id}");
        let Some(n) = nodes_by_id.get(node_id.as_str()).copied() else {
            label_ctx
                .typography_receipt
                .record_missing_text_effect(crate::sequence::SequenceTextSurface::ParticipantLabel);
            continue;
        };
        let prepared_math = math_sidecar
            .terminal_for_occurrence(&crate::sequence::SequenceMathOccurrence::Actor(actor_idx));
        let math_label = sequence_katex_label(
            prepared_math,
            actor_text_style,
            crate::sequence::SequenceMathHeightMode::Actor,
        );
        write_actor_man_top_glyph(
            out,
            actor_type,
            actor_id,
            &actor.description,
            n,
            actor_idx,
            actor_height,
            diagram_id,
            math_label,
            label_ctx,
        )?;
        checkpoints.checkpoint()?;
    }
    checkpoints.checkpoint()
}

pub(super) fn render_sequence_actor_man_bottoms(
    out: &mut impl SvgOutput,
    model: &super::model::SequenceSvgModel,
    nodes_by_id: &rustc_hash::FxHashMap<&str, &LayoutNode>,
    actor_height: f64,
    label_box_height: f64,
    diagram_id: SvgDiagramId<'_>,
    label_ctx: &super::actor_shapes::ActorLabelContext<'_>,
) -> Result<()> {
    let actor_text_style = label_ctx.style;
    let math_sidecar = label_ctx.math_sidecar;
    let checkpoints = label_ctx.checkpoints;
    // Actor-man footers (actor/boundary/control/entity) are emitted after messages.
    let last_idx = model.actor_order.len().saturating_sub(1);
    let mut footer_actors = Vec::with_capacity(model.actor_order.len());
    for (actor_index, actor_id) in model.actor_order.iter().enumerate() {
        checkpoints.checkpoint_loop(actor_index)?;
        let Some(actor) = model.actors.get(actor_id) else {
            continue;
        };
        let actor_type = actor.actor_type.as_str();
        if !is_actor_man_variant(actor_type) {
            continue;
        }
        let node_id = format!("actor-bottom-{actor_id}");
        let Some(n) = nodes_by_id.get(node_id.as_str()).copied() else {
            label_ctx
                .typography_receipt
                .record_missing_text_effect(crate::sequence::SequenceTextSurface::ParticipantLabel);
            continue;
        };
        footer_actors.push((
            actor_index,
            actor_id,
            actor_type,
            actor.description.as_str(),
            n,
        ));
    }
    checkpoints.checkpoint()?;
    footer_actors.sort_by(|a, b| b.4.x.total_cmp(&a.4.x));

    checkpoints.checkpoint()?;
    for (emission_index, (actor_index, actor_id, actor_type, label, n)) in
        footer_actors.into_iter().enumerate()
    {
        checkpoints.checkpoint_loop(emission_index)?;
        let prepared_math = math_sidecar
            .terminal_for_occurrence(&crate::sequence::SequenceMathOccurrence::Actor(actor_index));
        let math_label = sequence_katex_label(
            prepared_math,
            actor_text_style,
            crate::sequence::SequenceMathHeightMode::Actor,
        );
        write_actor_man_bottom_glyph(
            out,
            actor_type,
            actor_id,
            label,
            n,
            last_idx,
            ActorManBottomGlyphMetrics {
                actor_height,
                label_box_height,
            },
            diagram_id,
            math_label,
            label_ctx,
        )?;
        checkpoints.checkpoint()?;
    }
    checkpoints.checkpoint()
}
