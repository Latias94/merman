//! Pure Mermaid-compatible JSON projection of declared family data.

use super::GitGraphDiagramModel;
use crate::presentation::compat::{CompatJsonMetadata, CompatProjectionError};
use crate::warnings::legacy_warning_messages;
use serde_json::{Map, Value, json};

pub fn project(
    model: &GitGraphDiagramModel,
    meta: &CompatJsonMetadata<'_>,
) -> Result<Value, CompatProjectionError> {
    let warnings = legacy_warning_messages(&model.warning_facts);
    let mut out = Map::with_capacity(11);
    out.insert(
        "type".to_string(),
        Value::String(model.diagram_type.clone()),
    );
    out.insert("commits".to_string(), json!(&model.commits));
    out.insert("branches".to_string(), json!(&model.branches));
    out.insert("currentBranch".to_string(), json!(&model.current_branch));
    out.insert("direction".to_string(), json!(&model.direction));
    if let Some(title) = &model.title {
        out.insert("title".to_string(), Value::String(title.clone()));
    }
    out.insert("accTitle".to_string(), json!(&model.acc_title));
    out.insert("accDescr".to_string(), json!(&model.acc_descr));
    out.insert("warningFacts".to_string(), json!(&model.warning_facts));
    out.insert("warnings".to_string(), json!(warnings));
    out.insert(
        "config".to_string(),
        crate::json::clone_value_nonrecursive(meta.config.as_value()),
    );
    Ok(Value::Object(out))
}
