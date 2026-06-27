use super::model::{ACTIVE_END_MESSAGE_TYPE, ACTIVE_START_MESSAGE_TYPE, NOTE_MESSAGE_TYPE};
use crate::error::{AsciiError, Result};
use merman_core::diagrams::sequence::SequenceDiagramRenderModel;

pub(super) fn validate_supported_sequence_model(model: &SequenceDiagramRenderModel) -> Result<()> {
    if model
        .actors
        .values()
        .any(|actor| !is_supported_sequence_actor_type(&actor.actor_type))
    {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "sequence",
            feature: "actor types",
        });
    }

    if model
        .actors
        .values()
        .any(|actor| !actor.properties.is_empty())
    {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "sequence",
            feature: "actor properties",
        });
    }

    let note_message_count = model
        .messages
        .iter()
        .filter(|message| message.message_type == NOTE_MESSAGE_TYPE)
        .count();
    if !model.notes.is_empty() && note_message_count < model.notes.len() {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "sequence",
            feature: "notes without drawable messages",
        });
    }

    if model
        .boxes
        .iter()
        .any(|sequence_box| sequence_box.actor_keys.is_empty())
    {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "sequence",
            feature: "empty boxes",
        });
    }

    let has_activation_events = model.messages.iter().any(|message| {
        matches!(
            message.message_type,
            ACTIVE_START_MESSAGE_TYPE | ACTIVE_END_MESSAGE_TYPE
        )
    });
    if model.messages.iter().any(|message| message.activate) && !has_activation_events {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "sequence",
            feature: "activations without state events",
        });
    }

    if model
        .messages
        .iter()
        .any(|message| message.message_type != NOTE_MESSAGE_TYPE && message.placement.is_some())
    {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "sequence",
            feature: "message placement",
        });
    }

    Ok(())
}

fn is_supported_sequence_actor_type(actor_type: &str) -> bool {
    matches!(
        actor_type,
        "participant"
            | "actor"
            | "boundary"
            | "control"
            | "entity"
            | "database"
            | "collections"
            | "queue"
    )
}
