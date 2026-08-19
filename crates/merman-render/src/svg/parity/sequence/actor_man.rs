use super::super::*;
use super::actor_man_glyphs::{
    ActorManBottomGlyphMetrics, write_actor_man_bottom_glyph, write_actor_man_top_glyph,
};
use super::actor_shapes::is_actor_man_variant;
use super::math_label::sequence_katex_label;
use super::model::SequenceSvgModel;
use rustc_hash::FxHashMap;

pub(super) fn render_sequence_actor_man_tops(
    out: &mut impl SvgOutput,
    model: &SequenceSvgModel,
    nodes_by_id: &FxHashMap<&str, &LayoutNode>,
    actor_height: f64,
    diagram_id: &str,
    actor_text_style: &TextStyle,
    math_sidecar: &crate::sequence::SequenceMathSidecar,
    typography: &crate::sequence::SequenceResolvedTypography,
    typography_receipt: &crate::sequence::SequenceTypographyThemeReceipt,
) {
    // Actor-man variants (actor/boundary/control/entity) are emitted after `<defs>`.
    for (actor_idx, actor_id) in model.actor_order.iter().enumerate() {
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
            typography,
            typography_receipt,
        );
    }
}

pub(super) fn render_sequence_actor_man_bottoms(
    out: &mut impl SvgOutput,
    model: &SequenceSvgModel,
    nodes_by_id: &FxHashMap<&str, &LayoutNode>,
    actor_height: f64,
    label_box_height: f64,
    diagram_id: &str,
    actor_text_style: &TextStyle,
    math_sidecar: &crate::sequence::SequenceMathSidecar,
    typography: &crate::sequence::SequenceResolvedTypography,
    typography_receipt: &crate::sequence::SequenceTypographyThemeReceipt,
) {
    // Actor-man footers (actor/boundary/control/entity) are emitted after messages.
    let last_idx = model.actor_order.len().saturating_sub(1);
    let mut footer_actors = model
        .actor_order
        .iter()
        .enumerate()
        .filter_map(|(actor_index, actor_id)| {
            let actor = model.actors.get(actor_id)?;
            let actor_type = actor.actor_type.as_str();
            if !is_actor_man_variant(actor_type) {
                return None;
            }
            let node_id = format!("actor-bottom-{actor_id}");
            let n = nodes_by_id.get(node_id.as_str()).copied()?;
            Some((
                actor_index,
                actor_id,
                actor_type,
                actor.description.as_str(),
                n,
            ))
        })
        .collect::<Vec<_>>();
    footer_actors.sort_by(|a, b| b.4.x.total_cmp(&a.4.x));

    for (actor_index, actor_id, actor_type, label, n) in footer_actors {
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
            typography,
            typography_receipt,
        );
    }
}
