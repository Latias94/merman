//! Pure Mermaid-compatible JSON projection of declared family data.

use super::ErDiagramModel;
use crate::presentation::compat::{CompatJsonMetadata, CompatProjectionError};
use serde_json::{Value, json};

pub fn project(
    model: &ErDiagramModel,
    meta: &CompatJsonMetadata<'_>,
) -> Result<Value, CompatProjectionError> {
    let mut value = serde_json::to_value(model).map_err(|e| {
        CompatProjectionError::invalid_model(meta.diagram_type.to_string(), e.to_string())
    })?;
    let Value::Object(obj) = &mut value else {
        return Ok(value);
    };

    obj.insert("type".to_string(), json!(meta.diagram_type));
    obj.insert(
        "constants".to_string(),
        json!({
            "cardinality": {
                "zeroOrOne": "ZERO_OR_ONE",
                "zeroOrMore": "ZERO_OR_MORE",
                "oneOrMore": "ONE_OR_MORE",
                "onlyOne": "ONLY_ONE",
                "mdParent": "MD_PARENT",
            },
            "identification": {
                "nonIdentifying": "NON_IDENTIFYING",
                "identifying": "IDENTIFYING",
            }
        }),
    );

    Ok(value)
}
