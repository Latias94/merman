use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Number, Value, json};

use crate::{Error, ParseMetadata, Result};

static MINDMAP_DIAGRAM_ID_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MindmapDiagramRenderModel {
    #[serde(default)]
    pub nodes: Vec<MindmapDiagramRenderNode>,
    #[serde(default)]
    pub edges: Vec<MindmapDiagramRenderEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MindmapDiagramRenderNode {
    pub id: String,
    #[serde(rename = "domId")]
    pub dom_id: String,
    pub label: String,
    #[serde(default, rename = "labelType")]
    pub label_type: String,
    #[serde(default, rename = "isGroup")]
    pub is_group: bool,
    pub shape: String,
    #[serde(default)]
    pub width: f64,
    #[serde(default)]
    pub height: f64,
    #[serde(default)]
    pub padding: f64,
    #[serde(rename = "cssClasses")]
    pub css_classes: String,
    #[serde(default, rename = "cssStyles")]
    pub css_styles: Vec<String>,
    #[serde(default)]
    pub look: String,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub x: Option<f64>,
    #[serde(default)]
    pub y: Option<f64>,
    #[serde(default)]
    pub level: i64,
    #[serde(default, rename = "nodeId")]
    pub node_id: String,
    #[serde(default, rename = "type")]
    pub node_type: i32,
    #[serde(default)]
    pub section: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MindmapDiagramRenderEdge {
    pub id: String,
    pub start: String,
    pub end: String,
    #[serde(default, rename = "type")]
    pub edge_type: String,
    #[serde(default)]
    pub curve: String,
    #[serde(default)]
    pub thickness: String,
    #[serde(default)]
    pub look: String,
    #[serde(default)]
    pub classes: String,
    #[serde(default)]
    pub depth: i64,
    #[serde(default)]
    pub section: Option<i32>,
}

pub(crate) fn render_model_to_compat_json(
    model: &MindmapDiagramRenderModel,
    meta: &ParseMetadata,
) -> Result<Value> {
    let nodes = model
        .nodes
        .iter()
        .map(mindmap_node_to_compat_json)
        .collect();
    let edges = model
        .edges
        .iter()
        .map(mindmap_edge_to_compat_json)
        .collect();
    let config = mindmap_compat_config(meta);

    if model.nodes.is_empty() {
        let mut root = Map::with_capacity(3);
        root.insert("nodes".to_string(), Value::Array(nodes));
        root.insert("edges".to_string(), Value::Array(edges));
        root.insert("config".to_string(), config);
        return Ok(Value::Object(root));
    }

    let mut shapes = Map::new();
    for node in &model.nodes {
        shapes.insert(
            node.id.clone(),
            json!({
                "shape": node.shape,
                "width": crate::compatibility_json::number_value(node.width),
                "height": crate::compatibility_json::number_value(node.height),
                "padding": crate::compatibility_json::number_value(node.padding),
            }),
        );
    }

    let diagram_id = MINDMAP_DIAGRAM_ID_COUNTER.fetch_add(1, Ordering::Relaxed) + 1;
    let mut root = Map::with_capacity(12);
    root.insert("type".to_string(), Value::String(meta.diagram_type.clone()));
    root.insert("nodes".to_string(), Value::Array(nodes));
    root.insert("edges".to_string(), Value::Array(edges));
    root.insert("config".to_string(), config);
    root.insert(
        "rootNode".to_string(),
        mindmap_root_node_to_compat_json(model, meta)?,
    );
    root.insert("markers".to_string(), json!(["point"]));
    root.insert("direction".to_string(), Value::String("TB".to_string()));
    root.insert("nodeSpacing".to_string(), Number::from(50).into());
    root.insert("rankSpacing".to_string(), Number::from(50).into());
    root.insert("shapes".to_string(), Value::Object(shapes));
    root.insert(
        "diagramId".to_string(),
        Value::String(format!("mindmap-{diagram_id}")),
    );
    Ok(Value::Object(root))
}

fn mindmap_compat_config(meta: &ParseMetadata) -> Value {
    let mut config = crate::config::clone_value_nonrecursive(meta.effective_config.as_value());
    if meta.config.as_value().get("layout").is_none()
        && let Some(object) = config.as_object_mut()
    {
        object.insert(
            "layout".to_string(),
            Value::String("cose-bilkent".to_string()),
        );
    }
    config
}

fn mindmap_node_to_compat_json(node: &MindmapDiagramRenderNode) -> Value {
    let mut out = Map::with_capacity(19);
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
    out.insert(
        "width".to_string(),
        crate::compatibility_json::number_value(node.width),
    );
    out.insert(
        "height".to_string(),
        crate::compatibility_json::number_value(node.height),
    );
    out.insert(
        "padding".to_string(),
        crate::compatibility_json::number_value(node.padding),
    );
    out.insert(
        "cssClasses".to_string(),
        Value::String(node.css_classes.clone()),
    );
    out.insert("cssStyles".to_string(), json!(&node.css_styles));
    out.insert("look".to_string(), Value::String(node.look.clone()));
    if let Some(icon) = &node.icon {
        out.insert("icon".to_string(), Value::String(icon.clone()));
    }
    if let Some(x) = node.x {
        out.insert("x".to_string(), crate::compatibility_json::number_value(x));
    }
    if let Some(y) = node.y {
        out.insert("y".to_string(), crate::compatibility_json::number_value(y));
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

fn mindmap_root_node_to_compat_json(
    model: &MindmapDiagramRenderModel,
    meta: &ParseMetadata,
) -> Result<Value> {
    let mut node_index = HashMap::with_capacity(model.nodes.len());
    for (index, node) in model.nodes.iter().enumerate() {
        if node_index.insert(node.id.as_str(), index).is_some() {
            return Err(invalid_mindmap_model(
                meta,
                format!("duplicate mindmap node id `{}`", node.id),
            ));
        }
    }

    let root_index = model
        .nodes
        .iter()
        .position(|node| node.level == 0)
        .ok_or_else(|| invalid_mindmap_model(meta, "mindmap root node is missing"))?;
    let mut children = vec![Vec::new(); model.nodes.len()];
    for edge in &model.edges {
        let Some(&parent) = node_index.get(edge.start.as_str()) else {
            return Err(invalid_mindmap_model(
                meta,
                format!("mindmap edge `{}` references missing start node", edge.id),
            ));
        };
        let Some(&child) = node_index.get(edge.end.as_str()) else {
            return Err(invalid_mindmap_model(
                meta,
                format!("mindmap edge `{}` references missing end node", edge.id),
            ));
        };
        children[parent].push(child);
    }

    let mut values = vec![None; model.nodes.len()];
    let mut stack = vec![(root_index, false)];
    while let Some((index, visited)) = stack.pop() {
        if visited {
            let node = &model.nodes[index];
            let child_values = children[index]
                .iter()
                .map(|child| values[*child].take().unwrap_or(Value::Null))
                .collect();
            values[index] = Some(mindmap_root_record(node, child_values, meta)?);
        } else {
            stack.push((index, true));
            for child in children[index].iter().rev() {
                stack.push((*child, false));
            }
        }
    }

    Ok(values[root_index].take().unwrap_or(Value::Null))
}

fn mindmap_root_record(
    node: &MindmapDiagramRenderNode,
    children: Vec<Value>,
    meta: &ParseMetadata,
) -> Result<Value> {
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
    out.insert("children".to_string(), Value::Array(children));
    out.insert(
        "width".to_string(),
        crate::compatibility_json::number_value(node.width),
    );
    out.insert(
        "padding".to_string(),
        crate::compatibility_json::number_value(node.padding),
    );
    if let Some(section) = node.section {
        out.insert("section".to_string(), Number::from(section).into());
    }
    if node.height != 0.0 {
        out.insert(
            "height".to_string(),
            crate::compatibility_json::number_value(node.height),
        );
    }
    if let Some(class) = mindmap_custom_class(node) {
        out.insert("class".to_string(), Value::String(class));
    }
    if let Some(icon) = &node.icon {
        out.insert("icon".to_string(), Value::String(icon.clone()));
    }
    if let Some(x) = node.x {
        out.insert("x".to_string(), crate::compatibility_json::number_value(x));
    }
    if let Some(y) = node.y {
        out.insert("y".to_string(), crate::compatibility_json::number_value(y));
    }
    if node.level == 0 {
        out.insert("isRoot".to_string(), Value::Bool(true));
    }
    Ok(Value::Object(out))
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

fn invalid_mindmap_model(meta: &ParseMetadata, message: impl Into<String>) -> Error {
    Error::diagram_parse_fallback(meta.diagram_type.clone(), message.into())
}
