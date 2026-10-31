//! Pure Mermaid-compatible JSON projection of declared family data.

use super::RequirementDiagramModel;
use crate::presentation::compat::{CompatJsonMetadata, CompatProjectionError};
use serde_json::{Map, Value, json};

pub fn project(
    model: &RequirementDiagramModel,
    meta: &CompatJsonMetadata<'_>,
) -> Result<Value, CompatProjectionError> {
    let mut out = Map::with_capacity(9);
    out.insert(
        "type".to_string(),
        Value::String(meta.diagram_type.to_string()),
    );
    out.insert("accTitle".to_string(), json!(&model.acc_title));
    out.insert("accDescr".to_string(), json!(&model.acc_descr));
    out.insert("direction".to_string(), json!(&model.direction));
    out.insert("requirements".to_string(), json!(&model.requirements));
    out.insert("elements".to_string(), json!(&model.elements));
    out.insert("relationships".to_string(), json!(&model.relationships));
    out.insert("classes".to_string(), json!(&model.classes));
    out.insert(
        "config".to_string(),
        crate::json::clone_value_nonrecursive(meta.config.as_value()),
    );
    Ok(Value::Object(out))
}
