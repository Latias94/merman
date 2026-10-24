#[cfg(feature = "editor-facts")]
use crate::family::EditorFactsDemand;
use crate::{OperationControl, OperationControlResult, PresentationMetadata, Result};

use super::model::SequenceDiagramModel;
use super::parse::{construct_sequence_semantic_source, sequence_wrap_enabled_from_config};

pub(crate) fn parse_sequence_model_for_render_controlled(
    code: &str,
    meta: &PresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<Result<SequenceDiagramModel>> {
    let construction = construct_sequence_semantic_source(
        code,
        sequence_wrap_enabled_from_config(meta.effective_config.as_config()),
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        control,
    )?;
    match construction {
        Ok(source) => Ok(Ok(source.db.into_model())),
        Err(failure) => Ok(Err(
            failure.into_parse_error_for_type(&meta.diagram_type, code.len())
        )),
    }
}
