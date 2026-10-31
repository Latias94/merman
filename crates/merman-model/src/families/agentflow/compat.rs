//! Pure Mermaid-compatible JSON projection of declared family data.

use super::{
    AgentflowConnector, AgentflowDiagnostic, AgentflowDiagramModel, AgentflowDiagramRenderModel,
    AgentflowEdge, AgentflowNode, AgentflowSubGraph,
};
use crate::presentation::compat::{CompatJsonMetadata, CompatProjectionError};
use serde::Serialize;
use serde_json::Value;

pub fn project(
    model: &AgentflowDiagramRenderModel,
    meta: &CompatJsonMetadata<'_>,
) -> Result<Value, CompatProjectionError> {
    project_semantic(&model.model, meta)
}

pub fn project_semantic(
    model: &AgentflowDiagramModel,
    meta: &CompatJsonMetadata<'_>,
) -> Result<Value, CompatProjectionError> {
    let diagram_type = meta.diagram_type;
    // Keep the established preserve_order removals without serializing discarded facts.
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Compatibility<'a> {
        presentation: (),
        direction: &'a str,
        diagnostics: &'a [AgentflowDiagnostic],
        warning_facts: (),
        vertices: &'a [AgentflowNode],
        edges: &'a [AgentflowEdge],
        sub_graphs: &'a [AgentflowSubGraph],
        connectors: &'a [AgentflowConnector],
        #[serde(skip_serializing_if = "Option::is_none")]
        title: Option<&'a str>,
        #[serde(rename = "accTitle", skip_serializing_if = "Option::is_none")]
        acc_title: Option<&'a str>,
        #[serde(rename = "accDescr", skip_serializing_if = "Option::is_none")]
        acc_descr: Option<&'a str>,
    }
    let mut value = serde_json::to_value(Compatibility {
        presentation: (),
        direction: &model.direction,
        diagnostics: &model.diagnostics,
        warning_facts: (),
        vertices: &model.vertices,
        edges: &model.edges,
        sub_graphs: &model.sub_graphs,
        connectors: &model.connectors,
        title: model.title.as_deref(),
        acc_title: model.acc_title.as_deref(),
        acc_descr: model.acc_descr.as_deref(),
    })
    .map_err(|error| {
        CompatProjectionError::invalid_model(diagram_type.to_string(), error.to_string())
    })?;
    if let Some(root) = value.as_object_mut() {
        root.remove("presentation");
        root.remove("warningFacts");
        root.insert("type".into(), Value::String(diagram_type.to_string()));
        for collection in ["vertices", "subGraphs", "connectors"] {
            if let Some(items) = root.get_mut(collection).and_then(Value::as_array_mut) {
                for item in items {
                    if let Some(item) = item.as_object_mut() {
                        item.remove("parentId");
                        if let Some(metadata) =
                            item.get_mut("metadata").and_then(Value::as_object_mut)
                        {
                            metadata.retain(|key, _| {
                                !matches!(
                                    key.as_str(),
                                    "shape"
                                        | "view"
                                        | "icon"
                                        | "img"
                                        | "form"
                                        | "pos"
                                        | "w"
                                        | "h"
                                        | "class"
                                        | "style"
                                        | "labelType"
                                )
                            });
                            if metadata.is_empty() {
                                item.remove("metadata");
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(value)
}
