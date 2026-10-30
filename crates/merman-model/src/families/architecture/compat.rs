//! Pure Mermaid-compatible JSON projection of declared family data.

use super::{
    ArchitectureDiagramModel, ArchitectureEdgeModel, ArchitectureNodeKind, ArchitectureNodeModel,
};
use crate::presentation::compat::{CompatJsonMetadata, CompatProjectionError};
use crate::{OperationControl, OperationControlResult};
use serde_json::{Map, Value, json};

pub fn project(
    model: &ArchitectureDiagramModel,
    meta: &CompatJsonMetadata<'_>,
) -> Result<Value, CompatProjectionError> {
    let control = OperationControl::new();
    project_controlled(model, meta, &control).expect("a private parse control cannot be cancelled")
}

pub fn project_controlled(
    model: &ArchitectureDiagramModel,
    meta: &CompatJsonMetadata<'_>,
    control: &OperationControl,
) -> OperationControlResult<Result<Value, CompatProjectionError>> {
    control.checkpoint()?;
    let mut config = crate::json::clone_value_nonrecursive(meta.config.as_value());
    if meta.authored_config.as_value().get("layout").is_none()
        && let Some(obj) = config.as_object_mut()
    {
        obj.insert("layout".to_string(), Value::String("dagre".to_string()));
    }
    control.checkpoint()?;

    let mut edges = Vec::with_capacity(model.edges.len());
    for (index, edge) in model.edges.iter().enumerate() {
        if index % 128 == 0 {
            control.checkpoint()?;
        }
        edges.push(architecture_render_edge_to_compat_json(edge));
    }
    let mut nodes = Vec::with_capacity(model.nodes.len());
    let mut services = Vec::new();
    let mut junctions = Vec::new();
    for (index, node) in model.nodes.iter().enumerate() {
        if index % 128 == 0 {
            control.checkpoint()?;
        }
        let value = architecture_render_node_to_compat_json(node, &edges, control)?;
        match node.node_type {
            ArchitectureNodeKind::Service => services.push(value.clone()),
            ArchitectureNodeKind::Junction => junctions.push(value.clone()),
        }
        nodes.push(value);
    }
    let mut groups = Vec::with_capacity(model.groups.len());
    for (index, group) in model.groups.iter().enumerate() {
        if index % 128 == 0 {
            control.checkpoint()?;
        }
        groups.push(json!({
            "id": group.id,
            "icon": group.icon,
            "title": group.title,
            "in": group.in_group,
        }));
    }
    let mut layout_hints = Vec::with_capacity(model.layout_hints.len());
    for (hint_index, hint) in model.layout_hints.iter().enumerate() {
        if hint_index % 128 == 0 {
            control.checkpoint()?;
        }
        let mut members = Vec::with_capacity(hint.members.len());
        for (member_index, member) in hint.members.iter().enumerate() {
            if member_index % 128 == 0 {
                control.checkpoint()?;
            }
            members.push(Value::String(member.clone()));
        }
        layout_hints.push(json!({
            "direction": hint.direction.as_str(),
            "members": members,
        }));
    }

    let mut out = Map::with_capacity(11);
    out.insert(
        "type".to_string(),
        Value::String(meta.diagram_type.to_owned()),
    );
    out.insert("title".to_string(), json!(&model.title));
    out.insert("accTitle".to_string(), json!(&model.acc_title));
    out.insert("accDescr".to_string(), json!(&model.acc_descr));
    out.insert("groups".to_string(), Value::Array(groups));
    out.insert("nodes".to_string(), Value::Array(nodes));
    out.insert("services".to_string(), Value::Array(services));
    out.insert("junctions".to_string(), Value::Array(junctions));
    out.insert("edges".to_string(), Value::Array(edges));
    out.insert("layoutHints".to_string(), Value::Array(layout_hints));
    out.insert("config".to_string(), config);
    control.checkpoint()?;
    Ok(Ok(Value::Object(out)))
}

fn architecture_render_edge_to_compat_json(edge: &ArchitectureEdgeModel) -> Value {
    json!({
        "lhsId": edge.lhs_id,
        "lhsDir": edge.lhs_dir.to_string(),
        "lhsInto": edge.lhs_into,
        "lhsGroup": edge.lhs_group,
        "rhsId": edge.rhs_id,
        "rhsDir": edge.rhs_dir.to_string(),
        "rhsInto": edge.rhs_into,
        "rhsGroup": edge.rhs_group,
        "title": edge.title,
    })
}

fn architecture_render_node_to_compat_json(
    node: &ArchitectureNodeModel,
    edges: &[Value],
    control: &OperationControl,
) -> OperationControlResult<Value> {
    let mut node_edges = Vec::with_capacity(node.edge_indices.len());
    for (index, edge_index) in node.edge_indices.iter().enumerate() {
        if index % 128 == 0 {
            control.checkpoint()?;
        }
        if let Some(edge) = edges.get(*edge_index) {
            node_edges.push(edge.clone());
        }
    }
    Ok(json!({
        "id": node.id,
        "type": node.node_type,
        "edges": node_edges,
        "icon": node.icon,
        "iconText": node.icon_text,
        "title": node.title,
        "in": node.in_group,
    }))
}
