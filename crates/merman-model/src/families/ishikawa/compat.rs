//! Pure Mermaid-compatible JSON projection of declared family data.

use super::{IshikawaDiagramModel, project_root_controlled};
use crate::json::ManagedJson;
use crate::presentation::compat::{CompatJsonMetadata, CompatProjectionError};
use crate::{OperationControl, OperationControlResult};
use serde_json::{Map, Value, json};

pub fn project(
    model: &IshikawaDiagramModel,
    meta: &CompatJsonMetadata<'_>,
) -> Result<Value, CompatProjectionError> {
    project_controlled(model, meta, &OperationControl::new())
        .expect("a private parse control cannot be cancelled")
}

pub fn project_controlled(
    model: &IshikawaDiagramModel,
    meta: &CompatJsonMetadata<'_>,
    control: &OperationControl,
) -> OperationControlResult<Result<Value, CompatProjectionError>> {
    let root = match project_root_controlled(model, control)? {
        Ok(root) => root,
        Err(error) => {
            return Ok(Err(CompatProjectionError::invalid_model(
                meta.diagram_type,
                error.to_string(),
            )));
        }
    };
    let mut nodes = Vec::with_capacity(model.nodes.len());
    let mut stack = model
        .root
        .map(|root| vec![(root, 0usize)])
        .unwrap_or_default();
    while let Some((id, depth)) = stack.pop() {
        control.checkpoint()?;
        let node = &model.nodes[id];
        nodes.push(json!({"text": node.text, "depth": depth}));
        for &child in node.children.iter().rev() {
            control.checkpoint()?;
            stack.push((child, depth.saturating_add(1)));
        }
    }
    let mut out = Map::new();
    out.insert(
        "type".to_string(),
        Value::String(meta.diagram_type.to_owned()),
    );
    out.insert("title".to_string(), json!(&model.title));
    out.insert("accTitle".to_string(), json!(&model.acc_title));
    out.insert("accDescr".to_string(), json!(&model.acc_descr));
    out.insert("root".to_string(), root.into_unmanaged_value());
    out.insert("nodes".to_string(), Value::Array(nodes));
    let out = ManagedJson::from_value(Value::Object(out));
    control.checkpoint()?;
    Ok(Ok(out.into_unmanaged_value()))
}
