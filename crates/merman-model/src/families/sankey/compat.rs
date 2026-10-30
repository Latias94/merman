//! Pure Mermaid-compatible JSON projection of declared family data.

use super::SankeyDiagramModel;
use crate::presentation::compat::{CompatJsonMetadata, CompatProjectionError};
use serde_json::{Map, Value, json};

pub fn project(
    model: &SankeyDiagramModel,
    meta: &CompatJsonMetadata<'_>,
) -> Result<Value, CompatProjectionError> {
    let mut out = Map::with_capacity(3);
    out.insert(
        "type".to_string(),
        Value::String(meta.diagram_type.to_string()),
    );
    out.insert("graph".to_string(), json!(&model.graph));
    out.insert(
        "config".to_string(),
        crate::json::clone_value_nonrecursive(meta.config.as_value()),
    );
    Ok(Value::Object(out))
}
