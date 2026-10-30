//! Pure Mermaid-compatible JSON projection of declared family data.

use super::FlowchartModel;
use crate::presentation::compat::{CompatJsonMetadata, CompatProjectionError};
use crate::warnings::{FLOWCHART_EXPLICIT_DIRECTION_WARNING_RULE_ID, legacy_warning_messages};
use serde_json::{Value, json};

pub fn project(
    model: &FlowchartModel,
    meta: &CompatJsonMetadata<'_>,
) -> Result<Value, CompatProjectionError> {
    let mut value =
        serde_json::to_value(model).expect("Flowchart typed model must remain JSON-serializable");
    let root = value.as_object_mut().ok_or_else(|| {
        CompatProjectionError::invalid_model(
            meta.diagram_type.to_string(),
            "flowchart typed model did not serialize to an object".to_string(),
        )
    })?;
    if let Some(edges) = root.get_mut("edges").and_then(Value::as_array_mut) {
        for edge in edges {
            if let Some(edge) = edge.as_object_mut() {
                edge.remove("startMarker");
                edge.remove("endMarker");
                edge.remove("strokeKind");
                edge.remove("visibility");
            }
        }
    }
    if let Some(nodes) = root.get_mut("nodes").and_then(Value::as_array_mut) {
        for node in nodes {
            if let Some(node) = node.as_object_mut() {
                node.remove("provenance");
            }
        }
    }
    root.insert(
        "type".to_string(),
        Value::String(meta.diagram_type.to_string()),
    );
    if model
        .warning_facts
        .iter()
        .any(|fact| fact.rule_id == FLOWCHART_EXPLICIT_DIRECTION_WARNING_RULE_ID)
    {
        root.insert("direction".to_string(), Value::Null);
    }
    if !model.warning_facts.is_empty() {
        root.insert(
            "warnings".to_string(),
            json!(legacy_warning_messages(&model.warning_facts)),
        );
    }
    Ok(value)
}
