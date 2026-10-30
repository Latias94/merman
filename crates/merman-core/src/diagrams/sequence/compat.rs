#[cfg(all(feature = "compat-json", feature = "editor-facts"))]
use super::parse::SequenceSemanticSource;
#[cfg(feature = "compat-json")]
use serde_json::Value;

#[cfg(all(feature = "compat-json", feature = "editor-facts"))]
use crate::OperationControlResult;
#[cfg(all(feature = "compat-json", feature = "editor-facts"))]
use crate::family::EditorFactsDemand;
#[cfg(feature = "compat-json")]
use crate::{OperationControl, Result, SourcePresentationMetadata};

#[cfg(feature = "compat-json")]
use super::model::SequenceDiagramModel;
#[cfg(feature = "compat-json")]
use super::parse::{construct_sequence_semantic_source, sequence_wrap_enabled_from_config};

#[cfg(feature = "compat-json")]
pub(crate) fn render_model_to_compat_json(
    model: &SequenceDiagramModel,
    meta: &SourcePresentationMetadata,
) -> Result<Value> {
    merman_model::families::sequence::compat::project(model, &meta.compat_json_metadata())
        .map_err(crate::Error::from)
}

#[cfg(feature = "compat-json")]
pub(crate) fn parse_sequence(code: &str, meta: &SourcePresentationMetadata) -> Result<Value> {
    let construction = construct_sequence_semantic_source(
        code,
        sequence_wrap_enabled_from_config(meta.effective_config.as_config()),
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        &OperationControl::new(),
    )
    .expect("a private parse control cannot be cancelled");
    let source = construction
        .map_err(|failure| failure.into_parse_error_for_type(&meta.diagram_type, code.len()))?;
    render_model_to_compat_json(&source.db.into_model(), meta)
}

#[cfg(all(feature = "compat-json", feature = "editor-facts"))]
pub(crate) fn parse_sequence_json_and_editor_facts(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<crate::family::CombinedSemanticParse> {
    let construction = construct_sequence_semantic_source(
        code,
        sequence_wrap_enabled_from_config(meta.effective_config.as_config()),
        EditorFactsDemand::Capture,
        control,
    )?;
    let parsed = crate::family::CombinedSemanticParse::from_construction(
        construction,
        |SequenceSemanticSource { db, editor_facts }| {
            (
                render_model_to_compat_json(&db.into_model(), meta),
                editor_facts.expect("Sequence combined parse requests editor facts"),
            )
        },
        |failure| failure.into_error_and_editor_facts_for_type(&meta.diagram_type, code.len()),
    );
    control.checkpoint()?;
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    #[test]
    fn borrowed_compatibility_serializer_preserves_base_owner_records() {
        let source = concat!(
            "sequenceDiagram\ntitle: Delivery\naccTitle: Delivery sequence\naccDescr: Description\n",
            "autonumber 10 5\nparticipant Alice\nactor Bob\n",
            "box rgb(240,240,240) Team\nparticipant Carol\nend\n",
            "Alice->>Bob: Request\nNote over Alice,Bob: Review\n",
            "create participant Worker\nBob->>Worker: Start\ndestroy Worker\nWorker-->>Bob: Done\n",
        );
        let parsed = crate::SemanticParser::new()
            .parse_diagram_sync(source)
            .unwrap();
        let crate::SemanticDiagramModel::Sequence(model) = &parsed.model else {
            panic!("expected canonical Sequence");
        };
        let projection = model.compatibility_projection(&parsed.metadata.diagram_type);
        let value = serde_json::to_value(projection).unwrap();
        assert_eq!(value["type"], parsed.metadata.diagram_type);
        assert_eq!(value["title"], serde_json::to_value(&model.title).unwrap());
        assert_eq!(
            value["accTitle"],
            serde_json::to_value(&model.acc_title).unwrap()
        );
        assert_eq!(
            value["accDescr"],
            serde_json::to_value(&model.acc_descr).unwrap()
        );
        assert_eq!(
            value["actorOrder"],
            serde_json::to_value(&model.actor_order).unwrap()
        );
        assert_eq!(
            value["actors"],
            serde_json::to_value(&model.actors).unwrap()
        );
        assert_eq!(
            value["messages"],
            serde_json::to_value(&model.messages).unwrap()
        );
        assert_eq!(value["notes"], serde_json::to_value(&model.notes).unwrap());
        assert_eq!(value["boxes"], serde_json::to_value(&model.boxes).unwrap());
        assert_eq!(
            value["createdActors"],
            serde_json::to_value(&model.created_actors).unwrap()
        );
        assert_eq!(
            value["destroyedActors"],
            serde_json::to_value(&model.destroyed_actors).unwrap()
        );
        assert_eq!(
            value["constants"]["placement"]["over"],
            super::super::PLACEMENT_OVER
        );
        assert!(!model.notes.is_empty());
        assert!(!model.boxes.is_empty());
        assert!(!model.created_actors.is_empty());
        assert!(!model.destroyed_actors.is_empty());
    }
}
