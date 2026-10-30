//! Pure Mermaid-compatible JSON projection of declared family data.

use super::RailroadDiagramModel;
use crate::presentation::compat::{CompatJsonMetadata, CompatProjectionError};
use serde_json::{Value, json};

pub fn project(
    model: &RailroadDiagramModel,
    meta: &CompatJsonMetadata<'_>,
) -> Result<Value, CompatProjectionError> {
    Ok(json!({
        "type": meta.diagram_type,
        "title": &model.title,
        "accTitle": &model.acc_title,
        "accDescr": &model.acc_descr,
        "rules": &model.rules,
    }))
}
