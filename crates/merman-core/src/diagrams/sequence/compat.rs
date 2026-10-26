#[cfg(all(feature = "compat-json", feature = "editor-facts"))]
use super::parse::SequenceSemanticSource;
use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};
#[cfg(feature = "compat-json")]
use serde_json::Value;

#[cfg(all(feature = "compat-json", feature = "editor-facts"))]
use crate::OperationControlResult;
#[cfg(all(feature = "compat-json", feature = "editor-facts"))]
use crate::family::EditorFactsDemand;
#[cfg(feature = "compat-json")]
use crate::{OperationControl, PresentationMetadata, Result};

use super::model::SequenceDiagramModel;
#[cfg(feature = "compat-json")]
use super::parse::{construct_sequence_semantic_source, sequence_wrap_enabled_from_config};
use super::{PLACEMENT_LEFT_OF, PLACEMENT_OVER, PLACEMENT_RIGHT_OF};

impl SequenceDiagramModel {
    #[cfg(feature = "compat-json")]
    pub(crate) fn to_compat_json(&self, diagram_type: &str) -> Value {
        serde_json::to_value(self.compatibility_projection(diagram_type))
            .expect("the typed Sequence compatibility projection only contains JSON values")
    }

    /// Borrows the stable Mermaid compatibility JSON projection without constructing a `Value`
    /// tree first.
    #[doc(hidden)]
    pub fn compatibility_projection<'a>(
        &'a self,
        diagram_type: &'a str,
    ) -> SequenceCompatibilityProjection<'a> {
        SequenceCompatibilityProjection {
            model: self,
            diagram_type,
        }
    }
}

/// Borrowed serializer for Mermaid's public Sequence compatibility JSON shape.
///
/// This remains family-owned so streaming consumers do not duplicate field mappings or clone
/// arbitrary actor property trees before applying their own output budgets.
#[doc(hidden)]
#[derive(Debug, Clone, Copy)]
pub struct SequenceCompatibilityProjection<'a> {
    model: &'a SequenceDiagramModel,
    diagram_type: &'a str,
}

impl Serialize for SequenceCompatibilityProjection<'_> {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut root = serializer.serialize_struct("SequenceCompatibilityProjection", 12)?;
        root.serialize_field("type", self.diagram_type)?;
        root.serialize_field("title", &self.model.title)?;
        root.serialize_field("accTitle", &self.model.acc_title)?;
        root.serialize_field("accDescr", &self.model.acc_descr)?;
        root.serialize_field("actorOrder", &self.model.actor_order)?;
        root.serialize_field("actors", &self.model.actors)?;
        root.serialize_field("messages", &self.model.messages)?;
        root.serialize_field("notes", &self.model.notes)?;
        root.serialize_field("boxes", &self.model.boxes)?;
        root.serialize_field("createdActors", &self.model.created_actors)?;
        root.serialize_field("destroyedActors", &self.model.destroyed_actors)?;
        root.serialize_field("constants", &SEQUENCE_CONSTANTS)?;
        root.end()
    }
}

#[derive(Serialize)]
struct SequenceConstants {
    placement: SequencePlacementConstants,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SequencePlacementConstants {
    left_of: i32,
    right_of: i32,
    over: i32,
}

const SEQUENCE_CONSTANTS: SequenceConstants = SequenceConstants {
    placement: SequencePlacementConstants {
        left_of: PLACEMENT_LEFT_OF,
        right_of: PLACEMENT_RIGHT_OF,
        over: PLACEMENT_OVER,
    },
};

#[cfg(feature = "compat-json")]
pub(crate) fn render_model_to_compat_json(
    model: &SequenceDiagramModel,
    meta: &PresentationMetadata,
) -> Result<Value> {
    Ok(model.to_compat_json(&meta.diagram_type))
}

#[cfg(feature = "compat-json")]
pub(crate) fn parse_sequence(code: &str, meta: &PresentationMetadata) -> Result<Value> {
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
    Ok(source.db.into_model().to_compat_json(&meta.diagram_type))
}

#[cfg(all(feature = "compat-json", feature = "editor-facts"))]
pub(crate) fn parse_sequence_json_and_editor_facts(
    code: &str,
    meta: &PresentationMetadata,
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
                Ok(db.into_model().to_compat_json(&meta.diagram_type)),
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
        assert!(std::ptr::eq(projection.model, model));
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
            super::PLACEMENT_OVER
        );
        assert!(!model.notes.is_empty());
        assert!(!model.boxes.is_empty());
        assert!(!model.created_actors.is_empty());
        assert!(!model.destroyed_actors.is_empty());
    }
}
