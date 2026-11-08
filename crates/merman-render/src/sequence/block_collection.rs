use super::SequenceOperationCheckpoints;
use super::block_geometry::{SequenceBlockGeometry, SequenceBlockLayout};
use crate::Result;
use crate::model::{LayoutEdge, LayoutNode};
use merman_model::families::sequence::{
    SequenceControlKind, SequenceControlRole, SequenceDiagramModel, SequenceMessage,
    SequenceMessageKind,
};
use rustc_hash::FxHashMap;

#[derive(Debug, Clone)]
pub(super) struct SequenceBlockSection<'a> {
    pub(super) label_id: &'a str,
    pub(super) raw_label: &'a str,
    pub(super) geometry: SequenceBlockGeometry<'a>,
    pub(super) separator_y: Option<f64>,
}

#[derive(Debug, Clone)]
pub(super) struct SequenceBlock<'a> {
    pub(super) sections: Vec<SequenceBlockSection<'a>>,
    pub(super) layout: Option<&'a SequenceBlockLayout>,
}

impl<'a> SequenceBlock<'a> {
    pub(super) fn geometry(
        &self,
        checkpoints: SequenceOperationCheckpoints<'_>,
    ) -> Result<SequenceBlockGeometry<'a>> {
        let mut geometry = SequenceBlockGeometry::empty();
        for (index, section) in self.sections.iter().enumerate() {
            checkpoints.checkpoint_loop(index)?;
            geometry.merge(section.geometry);
        }
        Ok(geometry)
    }

    fn include_geometry(&mut self, geometry: SequenceBlockGeometry<'a>) {
        if let Some(section) = self.sections.last_mut() {
            section.geometry.merge(geometry);
        }
    }
}

struct BlockStackEntry<'a> {
    kind: SequenceControlKind,
    block: SequenceBlock<'a>,
}

pub(super) fn collect_sequence_blocks<'a>(
    model: &'a SequenceDiagramModel,
    actor_nodes_by_id: &FxHashMap<&str, &LayoutNode>,
    edges_by_id: &FxHashMap<&str, &LayoutEdge>,
    nodes_by_id: &FxHashMap<&str, &LayoutNode>,
    block_layouts_by_id: &'a FxHashMap<String, SequenceBlockLayout>,
    checkpoints: SequenceOperationCheckpoints<'_>,
) -> Result<(Vec<Option<usize>>, Vec<SequenceBlock<'a>>)> {
    collect_sequence_blocks_with(
        model,
        block_layouts_by_id,
        |message| {
            SequenceBlockGeometry::from_message(
                message,
                actor_nodes_by_id,
                edges_by_id,
                nodes_by_id,
            )
        },
        checkpoints,
    )
}

fn collect_sequence_blocks_with<'a>(
    model: &'a SequenceDiagramModel,
    block_layouts_by_id: &'a FxHashMap<String, SequenceBlockLayout>,
    mut message_geometry: impl FnMut(&'a SequenceMessage) -> SequenceBlockGeometry<'a>,
    checkpoints: SequenceOperationCheckpoints<'_>,
) -> Result<(Vec<Option<usize>>, Vec<SequenceBlock<'a>>)> {
    let mut blocks_by_end_index = vec![None; model.messages.len()];
    let mut blocks = Vec::with_capacity(model.messages.len().div_ceil(2));
    let mut stack: Vec<BlockStackEntry<'a>> = Vec::new();
    for (message_index, message) in model.messages.iter().enumerate() {
        checkpoints.checkpoint_loop(message_index)?;
        if message.semantic_kind() == SequenceMessageKind::Note {
            if let Some(entry) = stack.last_mut() {
                entry.block.include_geometry(message_geometry(message));
            }
            continue;
        }
        if let Some(control) = message.control_semantics()
            && control.kind != SequenceControlKind::Rect
        {
            match control.role {
                SequenceControlRole::Start => stack.push(BlockStackEntry {
                    kind: control.kind,
                    block: SequenceBlock {
                        sections: vec![SequenceBlockSection {
                            label_id: &message.id,
                            raw_label: message.message_text(),
                            geometry: SequenceBlockGeometry::empty(),
                            separator_y: None,
                        }],
                        layout: block_layouts_by_id.get(&message.id),
                    },
                }),
                SequenceControlRole::Separator => {
                    if let Some(entry) = stack.last_mut()
                        && entry.kind.accepts_separator(control.kind)
                    {
                        entry.block.sections.push(SequenceBlockSection {
                            label_id: &message.id,
                            raw_label: message.message_text(),
                            geometry: SequenceBlockGeometry::empty(),
                            separator_y: entry
                                .block
                                .layout
                                .and_then(|layout| layout.section_ys_by_id.get(&message.id))
                                .copied(),
                        });
                    }
                }
                SequenceControlRole::End => {
                    if let Some(entry) = stack.pop() {
                        if let Some(parent) = stack.last_mut() {
                            parent
                                .block
                                .include_geometry(entry.block.geometry(checkpoints)?);
                        }
                        if entry.kind.accepts_end(control.kind) {
                            blocks_by_end_index[message_index] = Some(blocks.len());
                            blocks.push(entry.block);
                        }
                    }
                }
            }
            continue;
        }
        if message.from.is_some()
            && message.to.is_some()
            && let Some(entry) = stack.last_mut()
        {
            entry.block.include_geometry(message_geometry(message));
        }
    }
    checkpoints.checkpoint()?;
    Ok((blocks_by_end_index, blocks))
}

#[cfg(test)]
mod tests {
    use super::collect_sequence_blocks_with;
    use crate::resources::{OperationWorkMeter, RenderResourcePolicy};
    use crate::sequence::block_geometry::SequenceBlockGeometry;
    use merman_model::families::sequence::{
        SequenceDiagramModel, SequenceMessage, SequenceMessagePayload,
    };
    use rustc_hash::FxHashMap;
    use std::collections::BTreeMap;

    fn checkpoints(meter: &OperationWorkMeter) -> super::SequenceOperationCheckpoints<'_> {
        super::SequenceOperationCheckpoints::for_emit(meter)
    }

    fn message(
        id: String,
        message_type: i32,
        from: Option<&str>,
        to: Option<&str>,
    ) -> SequenceMessage {
        SequenceMessage {
            id,
            from: from.map(str::to_string),
            to: to.map(str::to_string),
            message_type,
            message: SequenceMessagePayload::Text(String::new()),
            wrap: false,
            activate: false,
            placement: None,
            central_connection: 0,
        }
    }

    fn model(messages: Vec<SequenceMessage>) -> SequenceDiagramModel {
        SequenceDiagramModel {
            acc_title: None,
            acc_descr: None,
            title: None,
            actor_order: Vec::new(),
            actors: BTreeMap::new(),
            boxes: Vec::new(),
            messages,
            notes: Vec::new(),
            created_actors: BTreeMap::new(),
            destroyed_actors: BTreeMap::new(),
            actor_lifecycles: None,
        }
    }

    #[test]
    fn deeply_nested_blocks_aggregate_each_message_once() {
        const DEPTH: usize = 2_048;
        const CONTENT_MESSAGES: usize = 2_048;

        let mut messages = Vec::with_capacity(DEPTH * 2 + CONTENT_MESSAGES);
        for index in 0..DEPTH {
            messages.push(message(format!("start-{index}"), 10, None, None));
        }
        for index in 0..CONTENT_MESSAGES {
            messages.push(message(format!("message-{index}"), 5, Some("A"), Some("B")));
        }
        for index in 0..DEPTH {
            messages.push(message(format!("end-{index}"), 11, None, None));
        }
        let model = model(messages);
        let block_layouts = FxHashMap::default();
        let mut y = 0.0;
        let meter = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());

        let (blocks_by_end_index, blocks) = collect_sequence_blocks_with(
            &model,
            &block_layouts,
            |_| {
                let geometry = SequenceBlockGeometry::test_y_range(y, y + 1.0);
                y += 1.0;
                geometry
            },
            checkpoints(&meter),
        )
        .unwrap();

        assert_eq!(blocks.len(), DEPTH);
        assert_eq!(blocks_by_end_index.iter().flatten().count(), DEPTH);
        for block in blocks {
            let geometry = block.geometry(checkpoints(&meter)).unwrap();
            assert_eq!(
                geometry.frame_y_range(),
                Some((0.0, CONTENT_MESSAGES as f64))
            );
        }
    }

    #[test]
    fn nested_block_geometry_stays_in_the_active_parent_section() {
        let model = model(vec![
            message("alt".to_string(), 12, None, None),
            message("first".to_string(), 5, Some("A"), Some("B")),
            message("loop".to_string(), 10, None, None),
            message("nested".to_string(), 5, Some("A"), Some("B")),
            message("loop-end".to_string(), 11, None, None),
            message("else".to_string(), 13, None, None),
            message("second".to_string(), 5, Some("A"), Some("B")),
            message("alt-end".to_string(), 14, None, None),
        ]);
        let block_layouts = FxHashMap::default();
        let mut y = 0.0;
        let meter = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());

        let (_, blocks) = collect_sequence_blocks_with(
            &model,
            &block_layouts,
            |_| {
                let geometry = SequenceBlockGeometry::test_y_range(y, y + 1.0);
                y += 1.0;
                geometry
            },
            checkpoints(&meter),
        )
        .unwrap();

        let sections = &blocks[1].sections;
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].geometry.frame_y_range(), Some((0.0, 2.0)));
        assert_eq!(sections[1].geometry.frame_y_range(), Some((2.0, 3.0)));
    }
}
