use crate::error::{AsciiError, Result};
use crate::operation::AsciiExecution;
use crate::resource::AsciiResourceLimitPhase;
use merman_core::OperationPhase;
use merman_core::diagrams::sequence::{
    SequenceCentralDecoration, SequenceDiagramRenderModel, SequenceMessage, SequenceMessageKind,
    SequenceMessagePayload, SequenceNote,
};
use std::collections::HashSet;

pub(super) fn validate_supported_sequence_model(
    model: &SequenceDiagramRenderModel,
    execution: AsciiExecution<'_>,
) -> Result<()> {
    validate_actor_order(model, execution)?;
    validate_message_payloads(model, execution)?;
    validate_message_record_shapes(model, execution)?;
    validate_autonumber_values(model, execution)?;
    validate_central_records(model, execution)?;

    for actor in model.actors.values() {
        execution.checkpoint(OperationPhase::Semantic)?;
        if !is_supported_sequence_actor_type(&actor.actor_type) {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: "sequence",
                feature: "actor types",
            });
        }
    }

    validate_note_projection(model, execution)?;
    validate_activated_signals(model, execution)?;

    for message in &model.messages {
        execution.checkpoint(OperationPhase::Semantic)?;
        if message.semantic_kind() != SequenceMessageKind::Note && message.placement.is_some() {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: "sequence",
                feature: "message placement",
            });
        }
    }

    Ok(())
}

fn validate_message_payloads(
    model: &SequenceDiagramRenderModel,
    execution: AsciiExecution<'_>,
) -> Result<()> {
    for message in &model.messages {
        execution.checkpoint(OperationPhase::Semantic)?;
        if matches!(message.semantic_kind(), SequenceMessageKind::Autonumber)
            != matches!(message.message, SequenceMessagePayload::Autonumber(_))
        {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: "sequence",
                feature: "message payload shape",
            });
        }
    }
    Ok(())
}

fn validate_message_record_shapes(
    model: &SequenceDiagramRenderModel,
    execution: AsciiExecution<'_>,
) -> Result<()> {
    for message in &model.messages {
        execution.checkpoint(OperationPhase::Semantic)?;
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
                    && (message
                        .control_semantics()
                        .is_some_and(|semantics| semantics.consumes_text())
                        || text_is_empty)
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
        if !endpoints_match || !semantic_fields_match {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: "sequence",
                feature: "message record shape",
            });
        }
    }
    Ok(())
}

fn validate_autonumber_values(
    model: &SequenceDiagramRenderModel,
    execution: AsciiExecution<'_>,
) -> Result<()> {
    for message in &model.messages {
        execution.checkpoint(OperationPhase::Semantic)?;
        let SequenceMessagePayload::Autonumber(autonumber) = &message.message else {
            continue;
        };
        if autonumber.start.is_some_and(|value| !value.is_finite())
            || autonumber.step.is_some_and(|value| !value.is_finite())
        {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: "sequence",
                feature: "autonumber values",
            });
        }
    }
    Ok(())
}

fn validate_central_records(
    model: &SequenceDiagramRenderModel,
    execution: AsciiExecution<'_>,
) -> Result<()> {
    let mut index = 0;
    while index < model.messages.len() {
        execution.checkpoint(OperationPhase::Semantic)?;
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
                Some((SequenceCentralDecoration::Target, message.to.as_deref())),
                None,
            ],
            Some(SequenceCentralDecoration::Source) => [
                Some((SequenceCentralDecoration::Source, message.from.as_deref())),
                None,
            ],
            Some(SequenceCentralDecoration::Both) => [
                Some((SequenceCentralDecoration::Target, message.to.as_deref())),
                Some((SequenceCentralDecoration::Source, message.from.as_deref())),
            ],
            None => {
                index += 1;
                continue;
            }
        };

        let record_count = expected.iter().flatten().count();
        for (offset, (decoration, actor)) in expected.into_iter().flatten().enumerate() {
            execution.checkpoint(OperationPhase::Semantic)?;
            let record = model.messages.get(index + offset + 1);
            if !record.is_some_and(|record| {
                record.semantic_kind() == SequenceMessageKind::CentralDecorationRecord
                    && record.central_record_decoration() == Some(decoration)
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

fn validate_actor_order(
    model: &SequenceDiagramRenderModel,
    execution: AsciiExecution<'_>,
) -> Result<()> {
    if model.actor_order.is_empty() {
        return Ok(());
    }

    if model.actor_order.len() != model.actors.len() {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "sequence",
            feature: "actor order",
        });
    }

    execution.checkpoint(OperationPhase::Semantic)?;
    let mut seen = HashSet::new();
    seen.try_reserve(model.actor_order.len())
        .map_err(|_| AsciiError::allocation_failed(AsciiResourceLimitPhase::LayoutWork.as_str()))?;
    for actor_id in &model.actor_order {
        execution.checkpoint(OperationPhase::Semantic)?;
        if !model.actors.contains_key(actor_id) || !seen.insert(actor_id.as_str()) {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: "sequence",
                feature: "actor order",
            });
        }
    }

    Ok(())
}

fn validate_note_projection(
    model: &SequenceDiagramRenderModel,
    execution: AsciiExecution<'_>,
) -> Result<()> {
    if model.notes.is_empty() {
        return Ok(());
    }

    let mut next_message = 0usize;
    for note in &model.notes {
        execution.checkpoint(OperationPhase::Semantic)?;
        if !next_note_message(model, &mut next_message, execution)?
            .is_some_and(|message| note_matches_message(note, message))
        {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: "sequence",
                feature: "note model consistency",
            });
        }
    }
    if next_note_message(model, &mut next_message, execution)?.is_some() {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "sequence",
            feature: "note model consistency",
        });
    }

    Ok(())
}

fn next_note_message<'a>(
    model: &'a SequenceDiagramRenderModel,
    next_message: &mut usize,
    execution: AsciiExecution<'_>,
) -> Result<Option<&'a SequenceMessage>> {
    while let Some(message) = model.messages.get(*next_message) {
        execution.checkpoint(OperationPhase::Semantic)?;
        *next_message += 1;
        if message.semantic_kind() == SequenceMessageKind::Note {
            return Ok(Some(message));
        }
    }
    Ok(None)
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

fn validate_activated_signals(
    model: &SequenceDiagramRenderModel,
    execution: AsciiExecution<'_>,
) -> Result<()> {
    for (index, message) in model.messages.iter().enumerate() {
        execution.checkpoint(OperationPhase::Semantic)?;
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
