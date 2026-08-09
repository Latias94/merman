use crate::error::{AsciiError, Result};
use crate::resource::AsciiResourceLimitPhase;
use merman_core::diagrams::sequence::{
    SequenceCentralDecoration, SequenceDiagramRenderModel, SequenceMessage, SequenceMessageKind,
    SequenceMessagePayload, SequenceNote,
};
use std::collections::HashSet;

const CENTRAL_TARGET_MESSAGE_TYPE: i32 = 59;
const CENTRAL_SOURCE_MESSAGE_TYPE: i32 = 60;

pub(super) fn validate_supported_sequence_model(model: &SequenceDiagramRenderModel) -> Result<()> {
    validate_actor_order(model)?;
    validate_message_payloads(model)?;
    validate_message_record_shapes(model)?;
    validate_autonumber_values(model)?;
    validate_central_records(model)?;

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

    validate_note_projection(model)?;
    validate_activated_signals(model)?;

    if model.messages.iter().any(|message| {
        message.semantic_kind() != SequenceMessageKind::Note && message.placement.is_some()
    }) {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "sequence",
            feature: "message placement",
        });
    }

    Ok(())
}

fn validate_message_payloads(model: &SequenceDiagramRenderModel) -> Result<()> {
    let mismatched = model.messages.iter().any(|message| {
        matches!(message.semantic_kind(), SequenceMessageKind::Autonumber)
            != matches!(message.message, SequenceMessagePayload::Autonumber(_))
    });
    if mismatched {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "sequence",
            feature: "message payload shape",
        });
    }
    Ok(())
}

fn validate_message_record_shapes(model: &SequenceDiagramRenderModel) -> Result<()> {
    if model.messages.iter().any(|message| {
        let kind = message.semantic_kind();
        let text_is_empty =
            matches!(&message.message, SequenceMessagePayload::Text(text) if text.is_empty());
        let endpoints_match = match kind {
            SequenceMessageKind::Signal => message.from.is_some() && message.to.is_some(),
            SequenceMessageKind::Note => {
                message.from.is_some() && message.to.is_some() && message.placement.is_some()
            }
            SequenceMessageKind::ActivationStart | SequenceMessageKind::ActivationEnd => {
                message.from.is_some() && message.to.is_none() && text_is_empty
            }
            SequenceMessageKind::Autonumber => message.from.is_none() && message.to.is_none(),
            SequenceMessageKind::Control => {
                message.from.is_none()
                    && message.to.is_none()
                    && (control_message_consumes_text(message.message_type) || text_is_empty)
            }
            SequenceMessageKind::CentralDecorationRecord => {
                message.from.is_some() && message.to.is_none() && text_is_empty
            }
            SequenceMessageKind::Unknown => true,
        };
        let semantic_fields_match = match kind {
            SequenceMessageKind::Signal => true,
            SequenceMessageKind::Note => !message.activate && message.central_connection == 0,
            SequenceMessageKind::Control => !message.activate && message.central_connection == 0,
            SequenceMessageKind::ActivationStart
            | SequenceMessageKind::ActivationEnd
            | SequenceMessageKind::CentralDecorationRecord => {
                !message.activate && message.placement.is_none() && message.central_connection == 0
            }
            SequenceMessageKind::Autonumber => {
                !message.wrap
                    && !message.activate
                    && message.placement.is_none()
                    && message.central_connection == 0
            }
            SequenceMessageKind::Unknown => true,
        };
        !endpoints_match || !semantic_fields_match
    }) {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "sequence",
            feature: "message record shape",
        });
    }
    Ok(())
}

fn control_message_consumes_text(message_type: i32) -> bool {
    !matches!(message_type, 11 | 14 | 16 | 21 | 23 | 29 | 31)
}

fn validate_autonumber_values(model: &SequenceDiagramRenderModel) -> Result<()> {
    if model.messages.iter().any(|message| {
        let SequenceMessagePayload::Autonumber(autonumber) = &message.message else {
            return false;
        };
        autonumber.start.is_some_and(|value| !value.is_finite())
            || autonumber.step.is_some_and(|value| !value.is_finite())
    }) {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "sequence",
            feature: "autonumber values",
        });
    }
    Ok(())
}

fn validate_central_records(model: &SequenceDiagramRenderModel) -> Result<()> {
    let mut index = 0;
    while index < model.messages.len() {
        let message = &model.messages[index];
        if message.semantic_kind() == SequenceMessageKind::CentralDecorationRecord {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: "sequence",
                feature: "central connection records",
            });
        }
        if message.semantic_kind() != SequenceMessageKind::Signal {
            index += 1;
            continue;
        }

        let expected = match message.central_decoration() {
            Some(SequenceCentralDecoration::None) => {
                index += 1;
                continue;
            }
            Some(SequenceCentralDecoration::Target) => [
                Some((CENTRAL_TARGET_MESSAGE_TYPE, message.to.as_deref())),
                None,
            ],
            Some(SequenceCentralDecoration::Source) => [
                Some((CENTRAL_SOURCE_MESSAGE_TYPE, message.from.as_deref())),
                None,
            ],
            Some(SequenceCentralDecoration::Both) => [
                Some((CENTRAL_TARGET_MESSAGE_TYPE, message.to.as_deref())),
                Some((CENTRAL_SOURCE_MESSAGE_TYPE, message.from.as_deref())),
            ],
            None => {
                index += 1;
                continue;
            }
        };

        let record_count = expected.iter().flatten().count();
        for (offset, (message_type, actor)) in expected.into_iter().flatten().enumerate() {
            let record = model.messages.get(index + offset + 1);
            if !record.is_some_and(|record| {
                record.semantic_kind() == SequenceMessageKind::CentralDecorationRecord
                    && record.message_type == message_type
                    && record.from.as_deref() == actor
            }) {
                return Err(AsciiError::UnsupportedFeature {
                    diagram_type: "sequence",
                    feature: "central connection records",
                });
            }
        }
        index += 1 + record_count;
    }
    Ok(())
}

fn validate_actor_order(model: &SequenceDiagramRenderModel) -> Result<()> {
    if model.actor_order.is_empty() {
        return Ok(());
    }

    if model.actor_order.len() != model.actors.len() {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "sequence",
            feature: "actor order",
        });
    }

    let mut seen = HashSet::new();
    seen.try_reserve(model.actor_order.len())
        .map_err(|_| AsciiError::allocation_failed(AsciiResourceLimitPhase::LayoutWork.as_str()))?;
    for actor_id in &model.actor_order {
        if !model.actors.contains_key(actor_id) || !seen.insert(actor_id.as_str()) {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: "sequence",
                feature: "actor order",
            });
        }
    }

    Ok(())
}

fn validate_note_projection(model: &SequenceDiagramRenderModel) -> Result<()> {
    if model.notes.is_empty() {
        return Ok(());
    }

    let mut note_messages = model
        .messages
        .iter()
        .filter(|message| message.semantic_kind() == SequenceMessageKind::Note);
    let matches = model.notes.iter().all(|note| {
        note_messages
            .next()
            .is_some_and(|message| note_matches_message(note, message))
    });
    if !matches || note_messages.next().is_some() {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "sequence",
            feature: "note model consistency",
        });
    }

    Ok(())
}

fn note_matches_message(note: &SequenceNote, message: &SequenceMessage) -> bool {
    if message.message_text() != note.message
        || message.wrap != note.wrap
        || message.placement != Some(note.placement)
    {
        return false;
    }

    if let Some(actor) = note.actor.as_str() {
        return message.from.as_deref() == Some(actor) && message.to.as_deref() == Some(actor);
    }

    let Some(actors) = note.actor.as_array().filter(|actors| actors.len() == 2) else {
        return false;
    };
    let Some(from) = actors[0].as_str() else {
        return false;
    };
    let Some(to) = actors[1].as_str() else {
        return false;
    };
    message.from.as_deref() == Some(from) && message.to.as_deref() == Some(to)
}

fn validate_activated_signals(model: &SequenceDiagramRenderModel) -> Result<()> {
    for (index, message) in model.messages.iter().enumerate() {
        if message.semantic_kind() != SequenceMessageKind::Signal
            || !message.activate
            || message.central_decoration() != Some(SequenceCentralDecoration::None)
        {
            continue;
        }

        let target = message.to.as_deref();
        let state_event = model.messages.get(index + 1);
        if !state_event.is_some_and(|event| {
            event.semantic_kind() == SequenceMessageKind::ActivationStart
                && event.from.as_deref() == target
                && target.is_some()
        }) {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: "sequence",
                feature: "activation state events",
            });
        }
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
