//! Prepared control-frame geometry and source/emission order projection.

mod block_collection;
mod block_geometry;

use merman_model::OperationPhase;
use merman_model::families::sequence::{SequenceControlKind, SequenceDiagramModel};
use merman_render_core::resources::OperationWorkMeter;
use merman_render_core::{LayoutEdge, LayoutNode, Result};
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

use block_collection::collect_sequence_blocks;
pub use block_geometry::SequenceBlockLayout;
use block_geometry::{frame_x_from_actors, resolved_block_frame_x};

pub const SEQUENCE_CHECKPOINT_INTERVAL: usize = 64;
pub const SEQUENCE_SELF_MESSAGE_FRAME_EXTRA_Y_PX: f64 = 60.0;
pub const SEQUENCE_FRAME_SIDE_PAD_PX: f64 = 11.0;
pub const SEQUENCE_FRAME_GEOM_PAD_PX: f64 = 10.0;

#[derive(Clone, Copy)]
struct FrameCheckpoints<'a>(&'a OperationWorkMeter);

impl FrameCheckpoints<'_> {
    fn checkpoint(self) -> Result<()> {
        self.0
            .checkpoint(OperationPhase::Layout)
            .map_err(Into::into)
    }

    fn checkpoint_loop(self, iteration: usize) -> Result<()> {
        if iteration.is_multiple_of(SEQUENCE_CHECKPOINT_INTERVAL) {
            self.checkpoint()?;
        }
        Ok(())
    }
}

/// A control-frame divider. Its horizontal endpoints are the enclosing frame's edges.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SequenceFrameSectionLayout {
    /// ID of the semantic `else`, `and`, or `option` message.
    pub id: String,
    pub label: String,
    pub y: f64,
}

/// Final drawing bounds of a visible Sequence control frame, in diagram coordinates.
///
/// `x` and `y` are the top-left corner, unlike the center-based participant nodes. Frames
/// retain source order, including nested frames. Background `rect` blocks remain in `nodes`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SequenceFrameLayout {
    /// ID of the semantic message that opens the frame.
    pub id: String,
    pub kind: SequenceControlKind,
    /// Semantic label before presentation-specific brackets, wrapping, and entity decoding.
    pub label: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sections: Vec<SequenceFrameSectionLayout>,
}

pub struct SequenceFrameGeometry {
    pub frames: Vec<SequenceFrameLayout>,
    /// Frame indices in message emission order, independent of source-ordered geometry.
    pub placements: Vec<Option<usize>>,
}

/// Resolves final control-frame geometry from the prepared layout and the source messages.
///
/// The caller admits this linear pass on the operation work meter before constructing the input.
pub fn prepare_sequence_frame_geometry(
    model: &SequenceDiagramModel,
    nodes: &[LayoutNode],
    edges: &[LayoutEdge],
    block_layouts_by_id: &FxHashMap<String, SequenceBlockLayout>,
    label_box_width: f64,
    work_meter: &OperationWorkMeter,
) -> Result<SequenceFrameGeometry> {
    #[cfg(feature = "benchmark-stages")]
    let _stage =
        merman_model::benchmark_stages::enter(merman_model::benchmark_stages::Stage::LayoutSolve);
    let checkpoints = FrameCheckpoints(work_meter);
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
        result.placements[end_index] = Some(result.frames.len());
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
