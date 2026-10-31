use super::SequenceDiagramModel;
use crate::resources::{JsonValueComplexity, ModelComplexity, json_value_complexity};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SequenceComplexity {
    pub messages: usize,
    pub block_depth: usize,
    pub items: usize,
    pub text_bytes: usize,
    pub nesting_depth: usize,
}

impl SequenceComplexity {
    pub fn from_model(model: &SequenceDiagramModel) -> Self {
        const ACTOR_PROPERTY_VALUE_DEPTH: usize = 4;
        const NOTE_ACTOR_VALUE_DEPTH: usize = 3;

        let mut depth = 0usize;
        let mut block_depth = 0usize;
        for message in &model.messages {
            match message.message_type {
                10 | 12 | 15 | 19 | 22 | 27 | 30 | 32 => {
                    depth = depth.saturating_add(1);
                    block_depth = block_depth.max(depth);
                }
                11 | 14 | 16 | 21 | 23 | 29 | 31 => {
                    depth = depth.saturating_sub(1);
                }
                _ => {}
            }
        }
        let common_text_bytes = [
            model.acc_title.as_deref(),
            model.acc_descr.as_deref(),
            model.title.as_deref(),
        ]
        .into_iter()
        .flatten()
        .fold(0usize, |total, value| total.saturating_add(value.len()));
        let actor_order_text_bytes = model
            .actor_order
            .iter()
            .fold(0usize, |total, value| total.saturating_add(value.len()));
        let mut arbitrary_json = JsonValueComplexity::default();
        let actor_text_bytes = model.actors.iter().fold(0usize, |total, (key, actor)| {
            let links = json_map_complexity(&actor.links, ACTOR_PROPERTY_VALUE_DEPTH);
            let properties = json_map_complexity(&actor.properties, ACTOR_PROPERTY_VALUE_DEPTH);
            arbitrary_json.merge(links);
            arbitrary_json.merge(properties);
            total
                .saturating_add(key.len())
                .saturating_add(actor.name.len())
                .saturating_add(actor.description.len())
                .saturating_add(actor.actor_type.len())
                .saturating_add(links.text_bytes)
                .saturating_add(properties.text_bytes)
        });
        let box_text_bytes = model.boxes.iter().fold(0usize, |total, sequence_box| {
            sequence_box.actor_keys.iter().fold(
                total
                    .saturating_add(sequence_box.fill.len())
                    .saturating_add(sequence_box.name.as_deref().map_or(0, str::len)),
                |subtotal, actor| subtotal.saturating_add(actor.len()),
            )
        });
        let message_text_bytes = model.messages.iter().fold(0usize, |total, message| {
            total
                .saturating_add(message.id.len())
                .saturating_add(message.from.as_deref().map_or(0, str::len))
                .saturating_add(message.to.as_deref().map_or(0, str::len))
                .saturating_add(message.message_text().len())
        });
        let note_text_bytes = model.notes.iter().fold(0usize, |total, note| {
            let actor = json_value_complexity(&note.actor, NOTE_ACTOR_VALUE_DEPTH);
            arbitrary_json.merge(actor);
            total
                .saturating_add(actor.text_bytes)
                .saturating_add(note.message.len())
        });
        let lifecycle_text_bytes = model
            .created_actors
            .keys()
            .chain(model.destroyed_actors.keys())
            .fold(0usize, |total, value| total.saturating_add(value.len()));
        Self {
            messages: model.messages.len(),
            block_depth,
            items: model
                .actors
                .len()
                .saturating_add(model.boxes.len())
                .saturating_add(model.messages.len())
                .saturating_add(model.notes.len())
                .saturating_add(arbitrary_json.items),
            text_bytes: common_text_bytes
                .saturating_add(actor_order_text_bytes)
                .saturating_add(actor_text_bytes)
                .saturating_add(box_text_bytes)
                .saturating_add(message_text_bytes)
                .saturating_add(note_text_bytes)
                .saturating_add(lifecycle_text_bytes),
            nesting_depth: block_depth.max(arbitrary_json.nesting_depth),
        }
    }

    pub fn as_model_complexity(self) -> ModelComplexity {
        ModelComplexity::new(self.items, self.text_bytes, self.nesting_depth)
    }
}

fn json_map_complexity(
    map: &serde_json::Map<String, serde_json::Value>,
    value_depth: usize,
) -> JsonValueComplexity {
    let mut complexity = JsonValueComplexity {
        items: map.len(),
        text_bytes: map
            .keys()
            .fold(0usize, |total, key| total.saturating_add(key.len())),
        nesting_depth: 0,
        container_depth: 0,
    };
    for value in map.values() {
        complexity.merge(json_value_complexity(value, value_depth));
    }
    complexity
}
