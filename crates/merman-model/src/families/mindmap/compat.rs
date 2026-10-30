//! Pure Mermaid-compatible Mindmap JSON with an explicit output identity.

use super::{MindmapDiagramRenderEdge, MindmapDiagramRenderModel, MindmapDiagramRenderNode};
use crate::presentation::compat::{CompatJsonMetadata, CompatProjectionError};
use crate::{ManagedJson, OperationControl, OperationControlResult};
use serde_json::{Map, Number, Value, json};
use std::collections::HashMap;

pub fn project(
    model: &MindmapDiagramRenderModel,
    meta: &CompatJsonMetadata<'_>,
) -> Result<Value, CompatProjectionError> {
    let control = OperationControl::new();
    project_controlled(model, meta, &control).expect("a private parse control cannot be cancelled")
}

pub fn project_controlled(
    model: &MindmapDiagramRenderModel,
    meta: &CompatJsonMetadata<'_>,
    control: &OperationControl,
) -> OperationControlResult<Result<Value, CompatProjectionError>> {
    control.checkpoint()?;
    let mut nodes = Vec::with_capacity(model.nodes.len());
    for (index, node) in model.nodes.iter().enumerate() {
        if index % 128 == 0 {
            control.checkpoint()?;
        }
        nodes.push(mindmap_node_to_compat_json(node));
    }
    let mut edges = Vec::with_capacity(model.edges.len());
    for (index, edge) in model.edges.iter().enumerate() {
        if index % 128 == 0 {
            control.checkpoint()?;
        }
        edges.push(mindmap_edge_to_compat_json(edge));
    }
    let config = ManagedJson::from_value(crate::json::clone_value_nonrecursive(
        meta.config.as_value(),
    ));
    control.checkpoint()?;

    if model.nodes.is_empty() {
        let mut root = Map::with_capacity(3);
        root.insert("nodes".to_string(), Value::Array(nodes));
        root.insert("edges".to_string(), Value::Array(edges));
        root.insert("config".to_string(), config.into_unmanaged_value());
        return Ok(Ok(Value::Object(root)));
    }

    let mut shapes = Map::new();
    for (index, node) in model.nodes.iter().enumerate() {
        if index % 128 == 0 {
            control.checkpoint()?;
        }
        shapes.insert(
            node.id.clone(),
            json!({
                "shape": node.shape,
                "width": crate::json::number_value(node.width),
                "height": crate::json::number_value(node.height),
                "padding": crate::json::number_value(node.padding),
            }),
        );
    }

    let root_node = match root_node_controlled(model, meta, control)? {
        Ok(root) => root,
        Err(error) => return Ok(Err(error)),
    };
    let mut root = Map::with_capacity(12);
    root.insert(
        "type".to_string(),
        Value::String(meta.diagram_type.to_string()),
    );
    root.insert("nodes".to_string(), Value::Array(nodes));
    root.insert("edges".to_string(), Value::Array(edges));
    root.insert("config".to_string(), config.into_unmanaged_value());
    root.insert("rootNode".to_string(), root_node.into_unmanaged_value());
    root.insert("markers".to_string(), json!(["point"]));
    root.insert("direction".to_string(), Value::String("TB".to_string()));
    root.insert("nodeSpacing".to_string(), Number::from(50).into());
    root.insert("rankSpacing".to_string(), Number::from(50).into());
    root.insert("shapes".to_string(), Value::Object(shapes));
    let mut root = ManagedJson::from_value(Value::Object(root));
    let Some(diagram_id) = meta.diagram_id.filter(|id| !id.is_empty()) else {
        return Ok(Err(invalid_mindmap_model(
            meta,
            "mindmap diagram id is missing",
        )));
    };
    root.as_value_mut()
        .as_object_mut()
        .expect("the Mindmap projection root is an object")
        .insert(
            "diagramId".to_string(),
            Value::String(diagram_id.to_string()),
        );
    control.checkpoint()?;
    Ok(Ok(root.into_unmanaged_value()))
}

fn mindmap_node_to_compat_json(node: &MindmapDiagramRenderNode) -> Value {
    let mut out = Map::with_capacity(20);
    out.insert("id".to_string(), Value::String(node.id.clone()));
    out.insert("domId".to_string(), Value::String(node.dom_id.clone()));
    out.insert("label".to_string(), Value::String(node.label.clone()));
    if !node.label_type.is_empty() {
        out.insert(
            "labelType".to_string(),
            Value::String(node.label_type.clone()),
        );
    }
    out.insert("isGroup".to_string(), Value::Bool(node.is_group));
    out.insert("shape".to_string(), Value::String(node.shape.clone()));
    out.insert("width".to_string(), crate::json::number_value(node.width));
    out.insert("height".to_string(), crate::json::number_value(node.height));
    out.insert(
        "padding".to_string(),
        crate::json::number_value(node.padding),
    );
    out.insert(
        "cssClasses".to_string(),
        Value::String(node.css_classes.clone()),
    );
    out.insert("cssStyles".to_string(), json!(&node.css_styles));
    out.insert("centerLabel".to_string(), Value::Bool(true));
    out.insert("look".to_string(), Value::String(node.look.clone()));
    if let Some(icon) = &node.icon {
        out.insert("icon".to_string(), Value::String(icon.clone()));
    }
    if let Some(x) = node.x {
        out.insert("x".to_string(), crate::json::number_value(x));
    }
    if let Some(y) = node.y {
        out.insert("y".to_string(), crate::json::number_value(y));
    }
    out.insert("level".to_string(), Number::from(node.level).into());
    out.insert("nodeId".to_string(), Value::String(node.node_id.clone()));
    out.insert("type".to_string(), Number::from(node.node_type).into());
    if let Some(section) = node.section {
        out.insert("section".to_string(), Number::from(section).into());
    }
    Value::Object(out)
}

fn mindmap_edge_to_compat_json(edge: &MindmapDiagramRenderEdge) -> Value {
    let mut out = Map::with_capacity(10);
    out.insert("id".to_string(), Value::String(edge.id.clone()));
    out.insert("start".to_string(), Value::String(edge.start.clone()));
    out.insert("end".to_string(), Value::String(edge.end.clone()));
    out.insert("type".to_string(), Value::String(edge.edge_type.clone()));
    out.insert("curve".to_string(), Value::String(edge.curve.clone()));
    out.insert(
        "thickness".to_string(),
        Value::String(edge.thickness.clone()),
    );
    out.insert("look".to_string(), Value::String(edge.look.clone()));
    out.insert("classes".to_string(), Value::String(edge.classes.clone()));
    out.insert("depth".to_string(), Number::from(edge.depth).into());
    if let Some(section) = edge.section {
        out.insert("section".to_string(), Number::from(section).into());
    }
    Value::Object(out)
}

pub fn root_node_controlled(
    model: &MindmapDiagramRenderModel,
    meta: &CompatJsonMetadata<'_>,
    control: &OperationControl,
) -> OperationControlResult<Result<ManagedJson, CompatProjectionError>> {
    let mut node_index = HashMap::with_capacity(model.nodes.len());
    for (index, node) in model.nodes.iter().enumerate() {
        if index % 128 == 0 {
            control.checkpoint()?;
        }
        if node_index.insert(node.id.as_str(), index).is_some() {
            return Ok(Err(invalid_mindmap_model(
                meta,
                format!("duplicate mindmap node id `{}`", node.id),
            )));
        }
    }

    let mut root_index = None;
    for (index, node) in model.nodes.iter().enumerate() {
        if index % 128 == 0 {
            control.checkpoint()?;
        }
        if node.level == 0 {
            root_index = Some(index);
            break;
        }
    }
    let Some(root_index) = root_index else {
        return Ok(Err(invalid_mindmap_model(
            meta,
            "mindmap root node is missing",
        )));
    };
    let mut children = vec![Vec::new(); model.nodes.len()];
    let mut has_incoming = vec![false; model.nodes.len()];
    control.checkpoint()?;
    for (index, edge) in model.edges.iter().enumerate() {
        if index % 128 == 0 {
            control.checkpoint()?;
        }
        let Some(&parent) = node_index.get(edge.start.as_str()) else {
            return Ok(Err(invalid_mindmap_model(
                meta,
                format!("mindmap edge `{}` references missing start node", edge.id),
            )));
        };
        let Some(&child) = node_index.get(edge.end.as_str()) else {
            return Ok(Err(invalid_mindmap_model(
                meta,
                format!("mindmap edge `{}` references missing end node", edge.id),
            )));
        };
        // Mermaid assigns each internal node to exactly one parent when it constructs the
        // tree. Authored nodeId and label text may repeat; the internal id may not be shared.
        if std::mem::replace(&mut has_incoming[child], true) {
            return Ok(Err(invalid_mindmap_model(
                meta,
                format!("mindmap node `{}` has more than one parent edge", edge.end),
            )));
        }
        children[parent].push(child);
    }

    let mut values = vec![None::<ManagedJson>; model.nodes.len()];
    let mut active = vec![false; model.nodes.len()];
    let mut stack = vec![(root_index, false)];
    let mut processed = 0usize;
    while let Some((index, expanded)) = stack.pop() {
        if processed.is_multiple_of(128) {
            control.checkpoint()?;
        }
        processed = processed.saturating_add(1);
        if expanded {
            active[index] = false;
            let node = &model.nodes[index];
            let mut child_values = Vec::with_capacity(children[index].len());
            for &child in &children[index] {
                control.checkpoint()?;
                let Some(value) = values[child].take() else {
                    return Ok(Err(invalid_mindmap_model(
                        meta,
                        "mindmap child projection is incomplete",
                    )));
                };
                child_values.push(value);
            }
            let value = match mindmap_root_record(node, child_values, meta) {
                Ok(value) => value,
                Err(error) => return Ok(Err(error)),
            };
            values[index] = Some(value);
        } else {
            if std::mem::replace(&mut active[index], true) {
                return Ok(Err(invalid_mindmap_model(
                    meta,
                    "cyclic mindmap child relationship",
                )));
            }
            stack.push((index, true));
            for child in children[index].iter().rev() {
                control.checkpoint()?;
                stack.push((*child, false));
            }
        }
    }

    control.checkpoint()?;
    Ok(values[root_index]
        .take()
        .ok_or_else(|| invalid_mindmap_model(meta, "mindmap root projection is incomplete")))
}

fn mindmap_root_record(
    node: &MindmapDiagramRenderNode,
    children: Vec<ManagedJson>,
    meta: &CompatJsonMetadata<'_>,
) -> Result<ManagedJson, CompatProjectionError> {
    let id = node.id.parse::<i64>().map_err(|_| {
        invalid_mindmap_model(
            meta,
            format!("invalid numeric mindmap node id `{}`", node.id),
        )
    })?;
    let mut out = Map::with_capacity(15);
    out.insert("id".to_string(), Number::from(id).into());
    out.insert("nodeId".to_string(), Value::String(node.node_id.clone()));
    out.insert("level".to_string(), Number::from(node.level).into());
    out.insert("descr".to_string(), Value::String(node.label.clone()));
    out.insert("type".to_string(), Number::from(node.node_type).into());
    out.insert(
        "children".to_string(),
        Value::Array(
            children
                .into_iter()
                .map(ManagedJson::into_unmanaged_value)
                .collect(),
        ),
    );
    out.insert("width".to_string(), crate::json::number_value(node.width));
    out.insert(
        "padding".to_string(),
        crate::json::number_value(node.padding),
    );
    if let Some(section) = node.section {
        out.insert("section".to_string(), Number::from(section).into());
    }
    if node.height != 0.0 {
        out.insert("height".to_string(), crate::json::number_value(node.height));
    }
    if let Some(class) = mindmap_custom_class(node) {
        out.insert("class".to_string(), Value::String(class));
    }
    if let Some(icon) = &node.icon {
        out.insert("icon".to_string(), Value::String(icon.clone()));
    }
    if let Some(x) = node.x {
        out.insert("x".to_string(), crate::json::number_value(x));
    }
    if let Some(y) = node.y {
        out.insert("y".to_string(), crate::json::number_value(y));
    }
    if node.level == 0 {
        out.insert("isRoot".to_string(), Value::Bool(true));
    }
    Ok(ManagedJson::from_value(Value::Object(out)))
}

fn mindmap_custom_class(node: &MindmapDiagramRenderNode) -> Option<String> {
    let prefix = if node.level == 0 {
        "mindmap-node section-root section--1".to_string()
    } else if let Some(section) = node.section {
        format!("mindmap-node section-{section}")
    } else {
        "mindmap-node".to_string()
    };
    node.css_classes
        .strip_prefix(&prefix)
        .map(str::trim)
        .filter(|class| !class.is_empty())
        .map(str::to_string)
}

fn invalid_mindmap_model(
    meta: &CompatJsonMetadata<'_>,
    message: impl Into<String>,
) -> CompatProjectionError {
    CompatProjectionError::invalid_model(meta.diagram_type, message.into())
}
