#[cfg(feature = "compat-json")]
use std::collections::HashMap;

use serde::{Deserialize, Serialize};
#[cfg(feature = "compat-json")]
use serde_json::{Map, Number, Value, json};

use super::{
    NODE_TYPE_BANG, NODE_TYPE_CIRCLE, NODE_TYPE_CLOUD, NODE_TYPE_DEFAULT, NODE_TYPE_HEXAGON,
    NODE_TYPE_RECT, NODE_TYPE_ROUNDED_RECT,
};
#[cfg(feature = "compat-json")]
use crate::{Error, json_value::ManagedJson};
use crate::{MermaidConfig, OperationControl, OperationControlResult, ParseMetadata, Result};

const MINDMAP_SECTION_COUNT: usize = 11;

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

pub(super) fn model_into_render_model_controlled(
    model: super::db::MindmapDiagramModel,
    meta: &ParseMetadata,
    control: &OperationControl,
) -> OperationControlResult<Result<MindmapDiagramRenderModel>> {
    control.checkpoint()?;
    let Some(root_id) = model.nodes.first().map(|node| node.id) else {
        return Ok(Ok(MindmapDiagramRenderModel::default()));
    };
    let config = meta.effective_config.as_config();
    let width = super::utils::get_i64(config, "mindmap.maxNodeWidth").unwrap_or(200);
    let base_padding = super::utils::get_i64(config, "mindmap.padding").unwrap_or(10);
    let mut nodes = Vec::with_capacity(model.nodes.len());
    for (index, node) in model.nodes.into_iter().enumerate() {
        if index % 128 == 0 {
            control.checkpoint()?;
        }
        let padding = match node.node_type {
            NODE_TYPE_ROUNDED_RECT | NODE_TYPE_RECT | NODE_TYPE_HEXAGON => base_padding * 2,
            _ => base_padding,
        };
        nodes.push(MindmapProjectionNode {
            id: node.id,
            node_id: node.node_id,
            level: node.level,
            descr: node.descr,
            ty: node.node_type,
            children: node.children,
            width,
            padding,
            section: None,
            height: None,
            class: node.class,
            icon: node.icon,
            x: None,
            y: None,
            is_root: node.is_root,
        });
    }
    let mut projection = MindmapProjection { nodes };
    projection.assign_sections_controlled(
        root_id,
        None,
        config.get_str("mindmap.colorBy") == Some("depth"),
        control,
    )?;
    let nodes = projection.to_layout_nodes_for_render_controlled(root_id, config, control)?;
    let edges = projection.to_edges_for_render_controlled(root_id, config, control)?;
    control.checkpoint()?;
    Ok(Ok(MindmapDiagramRenderModel { nodes, edges }))
}

struct MindmapProjection {
    nodes: Vec<MindmapProjectionNode>,
}

fn mindmap_look(config: &MermaidConfig) -> String {
    config.get_str("look").unwrap_or("classic").to_string()
}

fn mindmap_default_shape(config: &MermaidConfig) -> &'static str {
    let theme = config
        .get_str("theme")
        .unwrap_or_default()
        .to_ascii_lowercase();
    if theme.contains("redux") {
        "rounded"
    } else {
        "defaultMindmapNode"
    }
}

fn shape_from_type(ty: i32, default_shape: &'static str) -> &'static str {
    match ty {
        NODE_TYPE_CIRCLE => "mindmapCircle",
        NODE_TYPE_RECT => "rect",
        NODE_TYPE_ROUNDED_RECT => "rounded",
        NODE_TYPE_CLOUD => "cloud",
        NODE_TYPE_BANG => "bang",
        NODE_TYPE_HEXAGON => "hexagon",
        NODE_TYPE_DEFAULT => default_shape,
        _ => "rect",
    }
}

#[derive(Debug, Clone)]
struct MindmapProjectionNode {
    id: i32,
    node_id: String,
    level: i32,
    descr: String,
    ty: i32,
    children: Vec<i32>,
    width: i64,
    padding: i64,
    section: Option<i32>,
    height: Option<i64>,
    class: Option<String>,
    icon: Option<String>,
    x: Option<f64>,
    y: Option<f64>,
    is_root: bool,
}

fn mindmap_node_css_classes(node: &MindmapProjectionNode) -> String {
    let mut css = vec!["mindmap-node".to_string()];
    if node.is_root {
        css.push("section-root".to_string());
        css.push("section--1".to_string());
    } else if let Some(section) = node.section {
        css.push(format!("section-{section}"));
    }
    if let Some(cls) = &node.class {
        css.push(cls.clone());
    }
    css.join(" ")
}

fn mindmap_render_node(
    node: &MindmapProjectionNode,
    look: &str,
    default_shape: &'static str,
) -> MindmapDiagramRenderNode {
    MindmapDiagramRenderNode {
        id: node.id.to_string(),
        dom_id: format!("node_{}", node.id),
        label: node.descr.clone(),
        // Mermaid's `flattenNodes()` marks every layout node as Markdown. The parser's quoted
        // description flag only controls sanitization while the node enters this DB.
        label_type: "markdown".to_string(),
        is_group: false,
        shape: shape_from_type(node.ty, default_shape).to_string(),
        width: node.width as f64,
        height: node.height.unwrap_or(0) as f64,
        padding: node.padding as f64,
        css_classes: mindmap_node_css_classes(node),
        css_styles: Vec::new(),
        look: look.to_string(),
        icon: node.icon.clone(),
        x: node.x,
        y: node.y,
        level: node.level as i64,
        node_id: node.node_id.clone(),
        node_type: node.ty,
        section: node.section,
    }
}

fn mindmap_edge_classes(parent: &MindmapProjectionNode, child: &MindmapProjectionNode) -> String {
    let mut classes = "edge".to_string();
    if let Some(section) = child.section {
        classes.push_str(&format!(" section-edge-{section}"));
    }
    let edge_depth = parent.level + 1;
    classes.push_str(&format!(" edge-depth-{edge_depth}"));
    classes
}

fn mindmap_render_edge(
    parent: &MindmapProjectionNode,
    child: &MindmapProjectionNode,
    look: &str,
) -> MindmapDiagramRenderEdge {
    MindmapDiagramRenderEdge {
        id: format!("edge_{}_{}", parent.id, child.id),
        start: parent.id.to_string(),
        end: child.id.to_string(),
        edge_type: "normal".to_string(),
        curve: "basis".to_string(),
        thickness: "normal".to_string(),
        look: look.to_string(),
        classes: mindmap_edge_classes(parent, child),
        depth: parent.level as i64,
        section: child.section,
    }
}

impl MindmapProjection {
    fn assign_sections_controlled(
        &mut self,
        node_id: i32,
        section: Option<i32>,
        color_by_depth: bool,
        control: &OperationControl,
    ) -> OperationControlResult<()> {
        let mut stack = vec![(node_id, section, 0usize)];
        let mut visited = 0usize;
        while let Some((node_id, section, depth)) = stack.pop() {
            if visited.is_multiple_of(128) {
                control.checkpoint()?;
            }
            visited = visited.saturating_add(1);
            let Ok(node_idx) = usize::try_from(node_id) else {
                continue;
            };
            let Some(node) = self.nodes.get_mut(node_idx) else {
                continue;
            };
            let node_level = node.level;
            if node_level == 0 {
                node.section = None;
            } else if color_by_depth {
                node.section = Some(((depth - 1) % MINDMAP_SECTION_COUNT) as i32);
            } else {
                node.section = section;
            }

            let child_count = node.children.len();
            for index in (0..child_count).rev() {
                if index % 128 == 0 {
                    control.checkpoint()?;
                }
                let child_id = self.nodes[node_idx].children[index];
                let child_section = if node_level == 0 {
                    Some((index % MINDMAP_SECTION_COUNT) as i32)
                } else {
                    section
                };
                stack.push((child_id, child_section, depth + 1));
            }
        }
        control.checkpoint()
    }

    fn to_layout_nodes_for_render_controlled(
        &self,
        root_id: i32,
        config: &MermaidConfig,
        control: &OperationControl,
    ) -> OperationControlResult<Vec<MindmapDiagramRenderNode>> {
        let mut out = Vec::new();
        let look = mindmap_look(config);
        let default_shape = mindmap_default_shape(config);
        let mut stack = vec![root_id];
        let mut visited = 0usize;
        while let Some(node_id) = stack.pop() {
            if visited.is_multiple_of(128) {
                control.checkpoint()?;
            }
            visited = visited.saturating_add(1);
            let Ok(node_idx) = usize::try_from(node_id) else {
                continue;
            };
            let Some(node) = self.nodes.get(node_idx) else {
                continue;
            };

            out.push(mindmap_render_node(node, &look, default_shape));

            for child in node.children.iter().rev() {
                stack.push(*child);
            }
        }
        control.checkpoint()?;
        Ok(out)
    }

    fn to_edges_for_render_controlled(
        &self,
        root_id: i32,
        config: &MermaidConfig,
        control: &OperationControl,
    ) -> OperationControlResult<Vec<MindmapDiagramRenderEdge>> {
        struct EdgeFrame {
            node_id: i32,
            next_child_index: usize,
        }

        let mut edges = Vec::new();
        let look = mindmap_look(config);
        let mut stack = vec![EdgeFrame {
            node_id: root_id,
            next_child_index: 0,
        }];
        let mut visited = 0usize;
        while let Some(frame) = stack.last_mut() {
            if visited.is_multiple_of(128) {
                control.checkpoint()?;
            }
            visited = visited.saturating_add(1);
            let Ok(node_idx) = usize::try_from(frame.node_id) else {
                stack.pop();
                continue;
            };
            let Some(node) = self.nodes.get(node_idx) else {
                stack.pop();
                continue;
            };
            let Some(child_id) = node.children.get(frame.next_child_index).copied() else {
                stack.pop();
                continue;
            };
            frame.next_child_index += 1;

            let Ok(child_idx) = usize::try_from(child_id) else {
                continue;
            };
            let Some(child) = self.nodes.get(child_idx) else {
                continue;
            };

            edges.push(mindmap_render_edge(node, child, &look));
            stack.push(EdgeFrame {
                node_id: child_id,
                next_child_index: 0,
            });
        }
        control.checkpoint()?;
        Ok(edges)
    }
}
#[cfg(feature = "compat-json")]
pub(crate) fn render_model_to_compat_json(
    model: &MindmapDiagramRenderModel,
    meta: &ParseMetadata,
) -> Result<Value> {
    let control = OperationControl::new();
    render_model_to_compat_json_controlled(model, meta, &control)
        .expect("a private parse control cannot be cancelled")
}

#[cfg(feature = "compat-json")]
pub(crate) fn render_model_to_compat_json_controlled(
    model: &MindmapDiagramRenderModel,
    meta: &ParseMetadata,
    control: &OperationControl,
) -> OperationControlResult<Result<Value>> {
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
    let config = ManagedJson::from_value(mindmap_compat_config(meta));
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
                "width": crate::json_value::number_value(node.width),
                "height": crate::json_value::number_value(node.height),
                "padding": crate::json_value::number_value(node.padding),
            }),
        );
    }

    let root_node = match mindmap_root_node_to_compat_json_controlled(model, meta, control)? {
        Ok(root) => root,
        Err(error) => return Ok(Err(error)),
    };
    let mut root = Map::with_capacity(12);
    root.insert("type".to_string(), Value::String(meta.diagram_type.clone()));
    root.insert("nodes".to_string(), Value::Array(nodes));
    root.insert("edges".to_string(), Value::Array(edges));
    root.insert("config".to_string(), config.into_unmanaged_value());
    root.insert("rootNode".to_string(), root_node.into_unmanaged_value());
    root.insert("markers".to_string(), json!(["point"]));
    root.insert("direction".to_string(), Value::String("TB".to_string()));
    root.insert("nodeSpacing".to_string(), Number::from(50).into());
    root.insert("rankSpacing".to_string(), Number::from(50).into());
    root.insert("shapes".to_string(), Value::Object(shapes));
    root.insert("diagramId".to_string(), Value::String(mindmap_diagram_id()));
    let root = ManagedJson::from_value(Value::Object(root));
    control.checkpoint()?;
    Ok(Ok(root.into_unmanaged_value()))
}

#[cfg(feature = "compat-json")]
fn mindmap_diagram_id() -> String {
    let mut hex = crate::runtime::generated_id_hex("mindmap.diagram-id", 0, 32).into_bytes();
    hex[12] = b'4';
    let variant = hex_digit(hex[16]);
    hex[16] = b"89ab"[(variant & 0x03) as usize];

    let hex = String::from_utf8(hex).expect("generated hexadecimal ID is valid UTF-8");
    format!(
        "mindmap-{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

#[cfg(feature = "compat-json")]
fn hex_digit(value: u8) -> u8 {
    match value {
        b'0'..=b'9' => value - b'0',
        b'a'..=b'f' => value - b'a' + 10,
        _ => unreachable!("generated ID contains only lowercase hexadecimal digits"),
    }
}

#[cfg(feature = "compat-json")]
fn mindmap_compat_config(meta: &ParseMetadata) -> Value {
    let mut config = crate::json_value::clone_value_nonrecursive(meta.effective_config.as_value());
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

#[cfg(feature = "compat-json")]
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
    out.insert(
        "width".to_string(),
        crate::json_value::number_value(node.width),
    );
    out.insert(
        "height".to_string(),
        crate::json_value::number_value(node.height),
    );
    out.insert(
        "padding".to_string(),
        crate::json_value::number_value(node.padding),
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
        out.insert("x".to_string(), crate::json_value::number_value(x));
    }
    if let Some(y) = node.y {
        out.insert("y".to_string(), crate::json_value::number_value(y));
    }
    out.insert("level".to_string(), Number::from(node.level).into());
    out.insert("nodeId".to_string(), Value::String(node.node_id.clone()));
    out.insert("type".to_string(), Number::from(node.node_type).into());
    if let Some(section) = node.section {
        out.insert("section".to_string(), Number::from(section).into());
    }
    Value::Object(out)
}

#[cfg(feature = "compat-json")]
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

#[cfg(feature = "compat-json")]
pub(super) fn mindmap_root_node_to_compat_json_controlled(
    model: &MindmapDiagramRenderModel,
    meta: &ParseMetadata,
    control: &OperationControl,
) -> OperationControlResult<Result<ManagedJson>> {
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
        children[parent].push(child);
    }

    let mut values = vec![None::<ManagedJson>; model.nodes.len()];
    let mut stack = vec![(root_index, false)];
    let mut processed = 0usize;
    while let Some((index, expanded)) = stack.pop() {
        if processed.is_multiple_of(128) {
            control.checkpoint()?;
        }
        processed = processed.saturating_add(1);
        if expanded {
            let node = &model.nodes[index];
            let mut child_values = Vec::with_capacity(children[index].len());
            for &child in &children[index] {
                control.checkpoint()?;
                child_values.push(values[child].take().unwrap_or_default());
            }
            let value = match mindmap_root_record(node, child_values, meta) {
                Ok(value) => value,
                Err(error) => return Ok(Err(error)),
            };
            values[index] = Some(value);
        } else {
            stack.push((index, true));
            for child in children[index].iter().rev() {
                control.checkpoint()?;
                stack.push((*child, false));
            }
        }
    }

    control.checkpoint()?;
    Ok(Ok(values[root_index].take().unwrap_or_default()))
}

#[cfg(feature = "compat-json")]
fn mindmap_root_record(
    node: &MindmapDiagramRenderNode,
    children: Vec<ManagedJson>,
    meta: &ParseMetadata,
) -> Result<ManagedJson> {
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
    out.insert(
        "width".to_string(),
        crate::json_value::number_value(node.width),
    );
    out.insert(
        "padding".to_string(),
        crate::json_value::number_value(node.padding),
    );
    if let Some(section) = node.section {
        out.insert("section".to_string(), Number::from(section).into());
    }
    if node.height != 0.0 {
        out.insert(
            "height".to_string(),
            crate::json_value::number_value(node.height),
        );
    }
    if let Some(class) = mindmap_custom_class(node) {
        out.insert("class".to_string(), Value::String(class));
    }
    if let Some(icon) = &node.icon {
        out.insert("icon".to_string(), Value::String(icon.clone()));
    }
    if let Some(x) = node.x {
        out.insert("x".to_string(), crate::json_value::number_value(x));
    }
    if let Some(y) = node.y {
        out.insert("y".to_string(), crate::json_value::number_value(y));
    }
    if node.level == 0 {
        out.insert("isRoot".to_string(), Value::Bool(true));
    }
    Ok(ManagedJson::from_value(Value::Object(out)))
}

#[cfg(feature = "compat-json")]
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

#[cfg(feature = "compat-json")]
fn invalid_mindmap_model(meta: &ParseMetadata, message: impl Into<String>) -> Error {
    Error::diagram_parse_fallback(meta.diagram_type.clone(), message.into())
}

#[cfg(test)]
mod tests {
    use super::super::db::{MindmapDb, MindmapNodeInput};
    use super::*;

    #[test]
    fn root_child_sections_wrap_after_eleven_slots() {
        let config = MermaidConfig::empty_object();
        let mut db = MindmapDb::default();
        db.add_node(
            MindmapNodeInput {
                indent_level: 0,
                id_raw: "root",
                descr_raw: "root",
                descr_is_markdown: false,
                ty: NODE_TYPE_DEFAULT,
                diagram_type: "mindmap",
            },
            &config,
        )
        .expect("root node");

        for index in 0..15 {
            let id = format!("child-{index}");
            db.add_node(
                MindmapNodeInput {
                    indent_level: 1,
                    id_raw: &id,
                    descr_raw: &id,
                    descr_is_markdown: false,
                    ty: NODE_TYPE_DEFAULT,
                    diagram_type: "mindmap",
                },
                &config,
            )
            .expect("root child");
        }

        let metadata = ParseMetadata {
            diagram_type: "mindmap".to_string(),
            config: config.clone(),
            effective_config: crate::presentation::PresentationConfig::from_value(
                crate::json_value::clone_value_nonrecursive(config.as_value()),
            ),
            title: None,
        };
        let model = model_into_render_model_controlled(
            db.into_semantic_model(),
            &metadata,
            &OperationControl::new(),
        )
        .unwrap()
        .unwrap();
        let sections: Vec<_> = model
            .nodes
            .iter()
            .skip(1)
            .map(|node| node.section)
            .collect();

        assert_eq!(
            sections,
            vec![
                Some(0),
                Some(1),
                Some(2),
                Some(3),
                Some(4),
                Some(5),
                Some(6),
                Some(7),
                Some(8),
                Some(9),
                Some(10),
                Some(0),
                Some(1),
                Some(2),
                Some(3),
            ]
        );
    }
}
