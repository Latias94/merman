//! Pure Mermaid-compatible JSON projection of declared family data.

use super::ErrorDiagramModel;
use crate::presentation::compat::{CompatJsonMetadata, CompatProjectionError};
use serde_json::{Value, json};

pub fn project(
    model: &ErrorDiagramModel,
    _meta: &CompatJsonMetadata<'_>,
) -> Result<Value, CompatProjectionError> {
    let mut output = json!({ "type": model.diagram_type });
    if let Some(message) = &model.error_message {
        output["errorMessage"] = json!(message);
    }
    Ok(output)
}
