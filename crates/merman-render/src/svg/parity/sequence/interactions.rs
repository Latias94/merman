use super::super::*;
use super::activation::{build_sequence_activation_plan, render_sequence_activation_group};
use super::block_collection::{SequenceBlock, collect_sequence_blocks};
use super::block_geometry::frame_x_from_actors;
use super::blocks::{
    SequenceBlockRenderContext, render_critical_sequence_block, render_sectioned_sequence_block,
    render_simple_sequence_block,
};
use super::model::*;
use super::notes::render_sequence_note;
use super::settings::SequenceRenderSettings;
use rustc_hash::FxHashMap;

pub(super) fn render_sequence_interaction_overlays(
    out: &mut String,
    model: &SequenceSvgModel,
    nodes_by_id: &FxHashMap<&str, &LayoutNode>,
    edges_by_id: &FxHashMap<&str, &crate::model::LayoutEdge>,
    seq_cfg: &serde_json::Value,
    effective_config: &serde_json::Value,
    settings: &SequenceRenderSettings,
    measurer: &dyn TextMeasurer,
) {
    // Mermaid creates activation placeholders at ACTIVE_START and inserts the `<rect>` once the
    // corresponding ACTIVE_END is encountered. We store the final rect geometry during this
    // first pass and remember which message id should emit which activation group.
    let activation_plan =
        build_sequence_activation_plan(model, nodes_by_id, edges_by_id, seq_cfg, effective_config);

    let (blocks_by_end_id, blocks) = collect_sequence_blocks(model);

    let Some((frame_x1, frame_x2)) = frame_x_from_actors(model, nodes_by_id) else {
        return;
    };

    let mut actor_nodes_by_id: FxHashMap<&str, &LayoutNode> =
        FxHashMap::with_capacity_and_hasher(model.actors.len(), Default::default());
    for actor_id in &model.actor_order {
        let node_id = format!("actor-top-{actor_id}");
        let Some(n) = nodes_by_id.get(node_id.as_str()).copied() else {
            continue;
        };
        actor_nodes_by_id.insert(actor_id.as_str(), n);
    }

    let mut msg_endpoints: FxHashMap<&str, (&str, &str)> =
        FxHashMap::with_capacity_and_hasher(model.messages.len(), Default::default());
    for msg in &model.messages {
        let (Some(from), Some(to)) = (msg.from.as_deref(), msg.to.as_deref()) else {
            continue;
        };
        msg_endpoints.insert(msg.id.as_str(), (from, to));
    }

    let block_ctx = SequenceBlockRenderContext {
        default_frame_x1: frame_x1,
        default_frame_x2: frame_x2,
        msg_endpoints: &msg_endpoints,
        actor_nodes_by_id: &actor_nodes_by_id,
        edges_by_id,
        nodes_by_id,
        label_box_height: settings.label_box_height,
        box_text_margin: settings.box_text_margin,
        measurer,
        loop_text_style: &settings.loop_text_style,
    };

    for msg in &model.messages {
        render_sequence_activation_group(out, &activation_plan, &msg.id);
        render_sequence_note(
            out,
            msg,
            nodes_by_id,
            measurer,
            settings.actor_label_font_size,
            settings.wrap_padding,
            &settings.note_text_style,
        );

        let Some(idxs) = blocks_by_end_id.get(msg.id.as_str()) else {
            continue;
        };
        for idx in idxs {
            let Some(block) = blocks.get(*idx) else {
                continue;
            };
            match block {
                SequenceBlock::Alt { sections } => {
                    render_sectioned_sequence_block(out, "alt", sections, true, &block_ctx);
                }
                SequenceBlock::Par { sections } => {
                    render_sectioned_sequence_block(out, "par", sections, false, &block_ctx);
                }
                SequenceBlock::Loop {
                    raw_label,
                    message_ids,
                } => {
                    render_simple_sequence_block(out, "loop", raw_label, message_ids, &block_ctx);
                }
                SequenceBlock::Opt {
                    raw_label,
                    message_ids,
                } => {
                    render_simple_sequence_block(out, "opt", raw_label, message_ids, &block_ctx);
                }
                SequenceBlock::Break {
                    raw_label,
                    message_ids,
                } => {
                    render_simple_sequence_block(out, "break", raw_label, message_ids, &block_ctx);
                }
                SequenceBlock::Critical { sections } => {
                    render_critical_sequence_block(out, sections, &block_ctx);
                }
            }
        }
    }
}
