use super::SequenceLayoutCheckpoints;
use super::block_collection::collect_sequence_blocks;
use super::block_geometry::{SequenceBlockLayout, frame_x_from_actors, resolved_block_frame_x};
use crate::Result;
use crate::model::{LayoutEdge, LayoutNode, SequenceFrameLayout, SequenceFrameSectionLayout};
use merman_model::families::sequence::{SequenceControlKind, SequenceDiagramModel};
use rustc_hash::FxHashMap;

/// Emission order is independent of source-ordered public geometry. Empty blocks still have an
/// emission occurrence so typography evidence can report their missing terminal text effects.
#[derive(Debug, Clone, Copy)]
pub(crate) enum SequenceFramePlacement {
    Draw(usize),
    Empty,
}

pub(super) struct SequenceFrameGeometry {
    pub(super) frames: Vec<SequenceFrameLayout>,
    pub(super) placements: Vec<Option<SequenceFramePlacement>>,
}

pub(super) fn prepare_sequence_frame_geometry(
    model: &SequenceDiagramModel,
    nodes: &[LayoutNode],
    edges: &[LayoutEdge],
    block_layouts_by_id: &FxHashMap<String, SequenceBlockLayout>,
    label_box_width: f64,
    checkpoints: SequenceLayoutCheckpoints<'_>,
) -> Result<SequenceFrameGeometry> {
    let mut nodes_by_id = FxHashMap::default();
    for (index, node) in nodes.iter().enumerate() {
        checkpoints.checkpoint_loop(index)?;
        nodes_by_id.insert(node.id.as_str(), node);
    }
    let mut edges_by_id = FxHashMap::default();
    for (index, edge) in edges.iter().enumerate() {
        checkpoints.checkpoint_loop(index)?;
        edges_by_id.insert(edge.id.as_str(), edge);
    }
    let mut actor_nodes_by_id = FxHashMap::default();
    for (index, actor_id) in model.actor_order.iter().enumerate() {
        checkpoints.checkpoint_loop(index)?;
        let node_id = format!("actor-top-{actor_id}");
        if let Some(node) = nodes_by_id.get(node_id.as_str()).copied() {
            actor_nodes_by_id.insert(actor_id.as_str(), node);
        }
    }

    let mut result = SequenceFrameGeometry {
        frames: Vec::new(),
        placements: Vec::new(),
    };
    let Some(default_frame) = frame_x_from_actors(model, &nodes_by_id, checkpoints)? else {
        return Ok(result);
    };
    let (blocks_by_end_index, blocks) = collect_sequence_blocks(
        model,
        &actor_nodes_by_id,
        &edges_by_id,
        &nodes_by_id,
        block_layouts_by_id,
        checkpoints,
    )?;
    result.placements.resize(model.messages.len(), None);
    let mut blocks_by_start_id = FxHashMap::default();
    for (end_index, block_index) in blocks_by_end_index.into_iter().enumerate() {
        checkpoints.checkpoint_loop(end_index)?;
        let Some(block_index) = block_index else {
            continue;
        };
        result.placements[end_index] = Some(SequenceFramePlacement::Empty);
        if let Some(first) = blocks[block_index].sections.first() {
            blocks_by_start_id.insert(first.label_id, (end_index, block_index));
        }
    }
    for (index, message) in model.messages.iter().enumerate() {
        checkpoints.checkpoint_loop(index)?;
        let Some((end_index, block_index)) = blocks_by_start_id.remove(message.id.as_str()) else {
            continue;
        };
        let Some(control) = message.control_semantics() else {
            continue;
        };
        let block = &blocks[block_index];
        let Some(layout) = block.layout else { continue };
        let critical_sections =
            (control.kind == SequenceControlKind::Critical).then_some(block.sections.len());
        let Some((left, right, _)) = resolved_block_frame_x(
            block.geometry(checkpoints)?,
            layout,
            &actor_nodes_by_id,
            default_frame,
            critical_sections,
        ) else {
            continue;
        };
        let mut dividers = Vec::with_capacity(block.sections.len().saturating_sub(1));
        for (index, section) in block.sections.iter().skip(1).enumerate() {
            checkpoints.checkpoint_loop(index)?;
            let Some(y) = section.separator_y else { break };
            dividers.push(SequenceFrameSectionLayout {
                id: section.label_id.to_owned(),
                label: section.raw_label.to_owned(),
                y,
            });
        }
        if dividers.len() != block.sections.len().saturating_sub(1) {
            continue;
        }
        result.placements[end_index] = Some(SequenceFramePlacement::Draw(result.frames.len()));
        result.frames.push(SequenceFrameLayout {
            id: message.id.clone(),
            kind: control.kind,
            label: message.message_text().to_owned(),
            x: left,
            y: layout.start_y,
            width: right.max(left + label_box_width) - left,
            height: layout.stop_y - layout.start_y,
            sections: dividers,
        });
    }
    checkpoints.checkpoint()?;
    Ok(result)
}
