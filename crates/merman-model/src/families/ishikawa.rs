//! Flat Ishikawa cause records and controlled nested data projection.

#[cfg(feature = "compat-json")]
pub mod compat;

use crate::json::ManagedJson;
use crate::{OperationControl, OperationControlResult};
use serde_json::{Map, Value};

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct IshikawaNode {
    pub text: String,
    #[serde(default)]
    pub children: Vec<usize>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct IshikawaDiagramModel {
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub title: Option<String>,
    pub root: Option<usize>,
    /// Flat records; root and child IDs are indices into this vector.
    pub nodes: Vec<IshikawaNode>,
}

/// Expands reachable flat records into managed nested JSON without recursive ownership.
///
/// Invalid root IDs, child IDs, repeated children, and cycles return a local serde error.
/// Cancellation retains the shared operation error and safely disposes of partial projections.
pub fn project_root_controlled(
    model: &IshikawaDiagramModel,
    control: &OperationControl,
) -> OperationControlResult<serde_json::Result<ManagedJson>> {
    let Some(root) = model.root else {
        return Ok(Ok(ManagedJson::from_value(Value::Null)));
    };
    if root >= model.nodes.len() {
        return Ok(Err(<serde_json::Error as serde::ser::Error>::custom(
            "ishikawa root ID is out of range",
        )));
    }
    let mut completed = vec![None::<ManagedJson>; model.nodes.len()];
    let mut seen = vec![false; model.nodes.len()];
    let mut stack = vec![(root, false)];
    while let Some((id, expanded)) = stack.pop() {
        control.checkpoint()?;
        let node = &model.nodes[id];
        if expanded {
            let mut children = Vec::with_capacity(node.children.len());
            for &child in &node.children {
                control.checkpoint()?;
                let Some(value) = completed[child].take() else {
                    return Ok(Err(<serde_json::Error as serde::ser::Error>::custom(
                        "invalid ishikawa child relationship",
                    )));
                };
                children.push(value);
            }
            let mut out = Map::new();
            out.insert("text".to_string(), Value::String(node.text.clone()));
            out.insert(
                "children".to_string(),
                Value::Array(
                    children
                        .into_iter()
                        .map(ManagedJson::into_unmanaged_value)
                        .collect(),
                ),
            );
            completed[id] = Some(ManagedJson::from_value(Value::Object(out)));
        } else {
            if std::mem::replace(&mut seen[id], true) {
                return Ok(Err(<serde_json::Error as serde::ser::Error>::custom(
                    "cyclic or repeated ishikawa child relationship",
                )));
            }
            stack.push((id, true));
            for &child in node.children.iter().rev() {
                control.checkpoint()?;
                if child >= model.nodes.len() {
                    return Ok(Err(<serde_json::Error as serde::ser::Error>::custom(
                        "ishikawa child ID is out of range",
                    )));
                }
                stack.push((child, false));
            }
        }
    }
    control.checkpoint()?;
    Ok(Ok(completed[root]
        .take()
        .expect("root projection completed")))
}
