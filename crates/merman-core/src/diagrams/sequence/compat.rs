use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};
use serde_json::Value;

use crate::family::EditorFactsDemand;
use crate::{OperationControl, OperationControlResult, ParseMetadata, Result};

use super::model::SequenceDiagramModel;
use super::parse::{
    SequenceSemanticSource, construct_sequence_semantic_source, sequence_wrap_enabled_from_config,
};
use super::{PLACEMENT_LEFT_OF, PLACEMENT_OVER, PLACEMENT_RIGHT_OF};

impl SequenceDiagramModel {
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

pub(crate) fn render_model_to_compat_json(
    model: &SequenceDiagramModel,
    meta: &ParseMetadata,
) -> Result<Value> {
    Ok(model.to_compat_json(&meta.diagram_type))
}

pub(crate) fn parse_sequence(code: &str, meta: &ParseMetadata) -> Result<Value> {
    let construction = construct_sequence_semantic_source(
        code,
        sequence_wrap_enabled_from_config(meta.effective_config.as_config()),
        EditorFactsDemand::Omit,
        &OperationControl::new(),
    )
    .expect("a private parse control cannot be cancelled");
    let source = construction
        .map_err(|failure| failure.into_parse_error_for_type(&meta.diagram_type, code.len()))?;
    Ok(source.db.into_model().to_compat_json(&meta.diagram_type))
}

pub(crate) fn parse_sequence_json_and_editor_facts(
    code: &str,
    meta: &ParseMetadata,
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
                editor_facts,
            )
        },
        |failure| failure.into_error_and_editor_facts_for_type(&meta.diagram_type, code.len()),
    );
    control.checkpoint()?;
    Ok(parsed)
}
