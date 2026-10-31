//! Pure Mermaid-compatible JSON projection of declared family data.

use super::QuadrantChartDiagramModel;
use crate::presentation::compat::{CompatJsonMetadata, CompatProjectionError};
use serde_json::{Map, Value, json};

pub fn project(
    model: &QuadrantChartDiagramModel,
    meta: &CompatJsonMetadata<'_>,
) -> Result<Value, CompatProjectionError> {
    let mut out = Map::with_capacity(9);
    out.insert(
        "type".to_string(),
        Value::String(meta.diagram_type.to_string()),
    );
    out.insert("title".to_string(), json!(&model.title));
    out.insert("accTitle".to_string(), json!(&model.acc_title));
    out.insert("accDescr".to_string(), json!(&model.acc_descr));
    out.insert("quadrants".to_string(), json!(&model.quadrants));
    out.insert("axes".to_string(), json!(&model.axes));
    out.insert("points".to_string(), json!(&model.points));
    out.insert("classes".to_string(), json!(&model.classes));
    out.insert(
        "config".to_string(),
        crate::json::clone_value_nonrecursive(meta.config.as_value()),
    );
    Ok(Value::Object(out))
}
