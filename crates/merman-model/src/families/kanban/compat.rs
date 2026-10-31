//! Pure Mermaid-compatible JSON projection of declared family data.

use super::KanbanDiagramRenderModel;
use crate::MermaidConfig;
use crate::presentation::compat::{CompatJsonMetadata, CompatProjectionError};
use serde_json::{Map, Value, json};

pub fn project(
    model: &KanbanDiagramRenderModel,
    meta: &CompatJsonMetadata<'_>,
) -> Result<Value, CompatProjectionError> {
    let mut out = Map::with_capacity(6);
    out.insert(
        "type".to_string(),
        Value::String(meta.diagram_type.to_string()),
    );
    out.insert("sections".to_string(), kanban_sections_to_json(model));
    out.insert(
        "nodes".to_string(),
        Value::Array(kanban_nodes_to_json(model, meta.config)),
    );
    out.insert("edges".to_string(), Value::Array(Vec::new()));
    out.insert("other".to_string(), Value::Object(Map::new()));
    out.insert(
        "config".to_string(),
        crate::json::clone_value_nonrecursive(meta.config.as_value()),
    );
    Ok(Value::Object(out))
}

fn kanban_sections_to_json(model: &KanbanDiagramRenderModel) -> Value {
    Value::Array(
        model
            .nodes
            .iter()
            .filter(|node| node.is_group)
            .map(|node| {
                let compat = node.compatibility();
                let mut out = Map::new();
                out.insert("id".to_string(), json!(&node.id));
                out.insert("label".to_string(), json!(&node.label));
                out.insert("level".to_string(), json!(compat.level));
                out.insert("width".to_string(), json!(compat.width));
                out.insert("padding".to_string(), json!(compat.padding));
                out.insert("isGroup".to_string(), json!(false));
                for (key, value) in [
                    ("ticket", &compat.ticket),
                    ("priority", &compat.priority),
                    ("assigned", &compat.assigned),
                    ("icon", &compat.icon),
                    ("cssClasses", &compat.css_classes),
                    ("shape", &compat.shape),
                ] {
                    if let Some(value) = value {
                        out.insert(key.to_string(), json!(value));
                    }
                }
                Value::Object(out)
            })
            .collect(),
    )
}

fn kanban_nodes_to_json(model: &KanbanDiagramRenderModel, config: &MermaidConfig) -> Vec<Value> {
    let look = config.get_str("look").unwrap_or("classic");
    model.nodes.iter().map(|node| {
        if node.is_group {
            json!({"id": node.id, "label": node.label, "isGroup": true, "ticket": node.ticket,
                "shape": "kanbanSection", "level": node.compatibility().level, "look": look})
        } else {
            json!({"id": node.id, "parentId": node.parent_id, "label": node.label, "isGroup": false,
                "ticket": node.ticket, "priority": node.priority, "assigned": node.assigned, "icon": node.icon,
                "shape": "kanbanItem", "level": node.compatibility().level, "rx": 5, "ry": 5,
                "cssStyles": ["text-align: left"]})
        }
    }).collect()
}
