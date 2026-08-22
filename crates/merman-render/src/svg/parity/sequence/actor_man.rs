use super::super::*;
use super::SequenceEmitCheckpoints;
use super::actor_man_glyphs::{
    ActorManBottomGlyphMetrics, write_actor_man_bottom_glyph, write_actor_man_top_glyph,
};
use super::actor_shapes::is_actor_man_variant;
use super::model::SequenceSvgModel;
use rustc_hash::FxHashMap;

pub(super) fn render_sequence_actor_man_tops(
    out: &mut String,
    model: &SequenceSvgModel,
    nodes_by_id: &FxHashMap<&str, &LayoutNode>,
    actor_height: f64,
    diagram_id: SvgDiagramId<'_>,
    checkpoints: SequenceEmitCheckpoints<'_>,
) -> Result<()> {
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
            continue;
        };
        write_actor_man_top_glyph(
            out,
            actor_type,
            actor_id,
            &actor.description,
            n,
            actor_idx,
            actor_height,
            diagram_id,
        );
        checkpoints.checkpoint()?;
    }
    checkpoints.checkpoint()
}

pub(super) fn render_sequence_actor_man_bottoms(
    out: &mut String,
    model: &SequenceSvgModel,
    nodes_by_id: &FxHashMap<&str, &LayoutNode>,
    actor_height: f64,
    label_box_height: f64,
    diagram_id: SvgDiagramId<'_>,
    checkpoints: SequenceEmitCheckpoints<'_>,
) -> Result<()> {
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
            continue;
        };
        footer_actors.push((actor_id, actor_type, actor.description.as_str(), n));
    }
    checkpoints.checkpoint()?;
    footer_actors.sort_by(|a, b| b.3.x.total_cmp(&a.3.x));

    checkpoints.checkpoint()?;
    for (actor_index, (actor_id, actor_type, label, n)) in footer_actors.into_iter().enumerate() {
        checkpoints.checkpoint_loop(actor_index)?;
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
        );
        checkpoints.checkpoint()?;
    }
    checkpoints.checkpoint()
}
