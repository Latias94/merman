use crate::sanitize::sanitize_text;
use crate::{Error, MermaidConfig, ParseMetadata, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::sync::atomic::{AtomicU64, Ordering};

const NODE_TYPE_DEFAULT: i32 = 0;
const NODE_TYPE_ROUNDED_RECT: i32 = 1;
const NODE_TYPE_RECT: i32 = 2;
const NODE_TYPE_CIRCLE: i32 = 3;
const NODE_TYPE_CLOUD: i32 = 4;
const NODE_TYPE_BANG: i32 = 5;
const NODE_TYPE_HEXAGON: i32 = 6;

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

#[derive(Debug, Clone)]
struct MindmapNode {
    id: i32,
    node_id: String,
    level: i32,
    descr: String,
    is_markdown: bool,
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

#[derive(Debug, Default)]
struct MindmapDb {
    nodes: Vec<MindmapNode>,
    base_level: Option<i32>,
}

impl MindmapDb {
    fn clear(&mut self) {
        self.nodes.clear();
        self.base_level = None;
    }

    fn get_mindmap(&self) -> Option<&MindmapNode> {
        self.nodes.first()
    }

    fn get_parent_index(&self, level: i32) -> Option<usize> {
        self.nodes.iter().rposition(|n| n.level < level)
    }

    fn add_node(
        &mut self,
        indent_level: i32,
        id_raw: &str,
        descr_raw: &str,
        descr_is_markdown: bool,
        ty: i32,
        diagram_type: &str,
        config: &MermaidConfig,
    ) -> Result<()> {
        let mut level = indent_level;
        let is_root;
        if self.nodes.is_empty() {
            self.base_level = Some(level);
            level = 0;
            is_root = true;
        } else if let Some(base) = self.base_level {
            level -= base;
            is_root = false;
        } else {
            is_root = false;
        }

        let mut padding = get_i64(config, "mindmap.padding").unwrap_or(10);
        let width = get_i64(config, "mindmap.maxNodeWidth").unwrap_or(200);

        match ty {
            NODE_TYPE_ROUNDED_RECT | NODE_TYPE_RECT | NODE_TYPE_HEXAGON => {
                padding *= 2;
            }
            _ => {}
        }

        let id = self.nodes.len() as i32;
        let node = MindmapNode {
            id,
            node_id: sanitize_text(id_raw, config),
            level,
            descr: if descr_is_markdown {
                descr_raw.to_string()
            } else {
                sanitize_text(descr_raw, config)
            },
            is_markdown: descr_is_markdown,
            ty,
            children: Vec::new(),
            width,
            padding,
            section: None,
            height: None,
            class: None,
            icon: None,
            x: None,
            y: None,
            is_root,
        };

        if let Some(parent_idx) = self.get_parent_index(level) {
            self.nodes[parent_idx].children.push(id);
            self.nodes.push(node);
            return Ok(());
        }

        if is_root {
            self.nodes.push(node);
            return Ok(());
        }

        Err(Error::DiagramParse {
            diagram_type: diagram_type.to_string(),
            message: format!(
                "There can be only one root. No parent could be found for (\"{}\")",
                node.descr
            ),
        })
    }

    fn decorate_last(
        &mut self,
        class: Option<String>,
        icon: Option<String>,
        config: &MermaidConfig,
    ) {
        let Some(last) = self.nodes.last_mut() else {
            return;
        };
        if let Some(icon) = icon {
            last.icon = Some(sanitize_text(&icon, config));
        }
        if let Some(class) = class {
            last.class = Some(sanitize_text(&class, config));
        }
    }

    fn assign_sections(&mut self, node_id: i32, section: Option<i32>) {
        let Ok(node_idx) = usize::try_from(node_id) else {
            return;
        };
        if node_idx >= self.nodes.len() {
            return;
        }
        let node_level = self.nodes[node_idx].level;
        if node_level == 0 {
            self.nodes[node_idx].section = None;
        } else {
            self.nodes[node_idx].section = section;
        }

        let children = self.nodes[node_idx].children.clone();
        for (index, child_id) in children.into_iter().enumerate() {
            let child_section = if node_level == 0 {
                Some(index as i32)
            } else {
                section
            };
            self.assign_sections(child_id, child_section);
        }
    }

    fn to_root_node_value(&self, node_id: i32) -> Value {
        let Ok(node_idx) = usize::try_from(node_id) else {
            return Value::Null;
        };
        let Some(node) = self.nodes.get(node_idx) else {
            return Value::Null;
        };

        let mut map = Map::new();
        map.insert("id".to_string(), json!(node.id));
        map.insert("nodeId".to_string(), json!(node.node_id));
        map.insert("level".to_string(), json!(node.level));
        map.insert("descr".to_string(), json!(node.descr));
        map.insert("type".to_string(), json!(node.ty));
        map.insert(
            "children".to_string(),
            Value::Array(
                node.children
                    .iter()
                    .map(|c| self.to_root_node_value(*c))
                    .collect(),
            ),
        );
        map.insert("width".to_string(), json!(node.width));
        map.insert("padding".to_string(), json!(node.padding));

        if let Some(section) = node.section {
            map.insert("section".to_string(), json!(section));
        }
        if let Some(height) = node.height {
            map.insert("height".to_string(), json!(height));
        }
        if let Some(class) = &node.class {
            map.insert("class".to_string(), json!(class));
        }
        if let Some(icon) = &node.icon {
            map.insert("icon".to_string(), json!(icon));
        }
        if let Some(x) = node.x {
            map.insert("x".to_string(), json!(x));
        }
        if let Some(y) = node.y {
            map.insert("y".to_string(), json!(y));
        }
        if node.is_root {
            map.insert("isRoot".to_string(), json!(true));
        }

        Value::Object(map)
    }

    fn to_layout_node_values(&self, root_id: i32) -> Vec<Value> {
        fn shape_from_type(ty: i32) -> &'static str {
            match ty {
                NODE_TYPE_CIRCLE => "mindmapCircle",
                NODE_TYPE_RECT => "rect",
                NODE_TYPE_ROUNDED_RECT => "rounded",
                NODE_TYPE_CLOUD => "cloud",
                NODE_TYPE_BANG => "bang",
                NODE_TYPE_HEXAGON => "hexagon",
                NODE_TYPE_DEFAULT => "defaultMindmapNode",
                _ => "rect",
            }
        }

        fn visit(db: &MindmapDb, node_id: i32, out: &mut Vec<Value>) {
            let Ok(node_idx) = usize::try_from(node_id) else {
                return;
            };
            let Some(node) = db.nodes.get(node_idx) else {
                return;
            };

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
            let css_classes = css.join(" ");

            let mut map = Map::new();
            map.insert("id".to_string(), json!(node.id.to_string()));
            map.insert("domId".to_string(), json!(format!("node_{}", node.id)));
            map.insert("label".to_string(), json!(node.descr));
            if node.is_markdown {
                map.insert("labelType".to_string(), json!("markdown"));
            }
            map.insert("isGroup".to_string(), json!(false));
            map.insert("shape".to_string(), json!(shape_from_type(node.ty)));
            map.insert("width".to_string(), json!(node.width));
            map.insert("height".to_string(), json!(node.height.unwrap_or(0)));
            map.insert("padding".to_string(), json!(node.padding));
            map.insert("cssClasses".to_string(), json!(css_classes));
            map.insert("cssStyles".to_string(), Value::Array(Vec::new()));
            map.insert("look".to_string(), json!("default"));

            if let Some(icon) = &node.icon {
                map.insert("icon".to_string(), json!(icon));
            }
            if let Some(x) = node.x {
                map.insert("x".to_string(), json!(x));
            }
            if let Some(y) = node.y {
                map.insert("y".to_string(), json!(y));
            }

            map.insert("level".to_string(), json!(node.level));
            map.insert("nodeId".to_string(), json!(node.node_id));
            map.insert("type".to_string(), json!(node.ty));
            if let Some(section) = node.section {
                map.insert("section".to_string(), json!(section));
            }

            out.push(Value::Object(map));

            for child in node.children.iter() {
                visit(db, *child, out);
            }
        }

        let mut out = Vec::new();
        visit(self, root_id, &mut out);
        out
    }

    fn to_layout_nodes_for_render(&self, root_id: i32) -> Vec<MindmapDiagramRenderNode> {
        fn shape_from_type(ty: i32) -> &'static str {
            match ty {
                NODE_TYPE_CIRCLE => "mindmapCircle",
                NODE_TYPE_RECT => "rect",
                NODE_TYPE_ROUNDED_RECT => "rounded",
                NODE_TYPE_CLOUD => "cloud",
                NODE_TYPE_BANG => "bang",
                NODE_TYPE_HEXAGON => "hexagon",
                NODE_TYPE_DEFAULT => "defaultMindmapNode",
                _ => "rect",
            }
        }

        fn visit(db: &MindmapDb, node_id: i32, out: &mut Vec<MindmapDiagramRenderNode>) {
            let Ok(node_idx) = usize::try_from(node_id) else {
                return;
            };
            let Some(node) = db.nodes.get(node_idx) else {
                return;
            };

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
            let css_classes = css.join(" ");

            out.push(MindmapDiagramRenderNode {
                id: node.id.to_string(),
                dom_id: format!("node_{}", node.id),
                label: node.descr.clone(),
                label_type: if node.is_markdown {
                    "markdown".to_string()
                } else {
                    String::new()
                },
                is_group: false,
                shape: shape_from_type(node.ty).to_string(),
                width: node.width as f64,
                height: node.height.unwrap_or(0) as f64,
                padding: node.padding as f64,
                css_classes,
                css_styles: Vec::new(),
                look: "default".to_string(),
                icon: node.icon.clone(),
                x: node.x,
                y: node.y,
                level: node.level as i64,
                node_id: node.node_id.clone(),
                node_type: node.ty,
                section: node.section,
            });

            for child in node.children.iter() {
                visit(db, *child, out);
            }
        }

        let mut out = Vec::new();
        visit(self, root_id, &mut out);
        out
    }

    fn to_edge_values(&self, root_id: i32) -> Vec<Value> {
        fn visit(db: &MindmapDb, node_id: i32, edges: &mut Vec<Value>) {
            let Ok(node_idx) = usize::try_from(node_id) else {
                return;
            };
            let Some(node) = db.nodes.get(node_idx) else {
                return;
            };
            for child_id in node.children.iter() {
                let Ok(child_idx) = usize::try_from(*child_id) else {
                    continue;
                };
                let Some(child) = db.nodes.get(child_idx) else {
                    continue;
                };

                let mut classes = "edge".to_string();
                if let Some(section) = child.section {
                    classes.push_str(&format!(" section-edge-{section}"));
                }
                let edge_depth = node.level + 1;
                classes.push_str(&format!(" edge-depth-{edge_depth}"));

                let mut map = Map::new();
                map.insert(
                    "id".to_string(),
                    json!(format!("edge_{}_{}", node.id, child.id)),
                );
                map.insert("start".to_string(), json!(node.id.to_string()));
                map.insert("end".to_string(), json!(child.id.to_string()));
                map.insert("type".to_string(), json!("normal"));
                map.insert("curve".to_string(), json!("basis"));
                map.insert("thickness".to_string(), json!("normal"));
                map.insert("look".to_string(), json!("default"));
                map.insert("classes".to_string(), json!(classes));
                map.insert("depth".to_string(), json!(node.level));
                if let Some(section) = child.section {
                    map.insert("section".to_string(), json!(section));
                }
                edges.push(Value::Object(map));

                visit(db, *child_id, edges);
            }
        }

        let mut edges = Vec::new();
        visit(self, root_id, &mut edges);
        edges
    }

    fn to_edges_for_render(&self, root_id: i32) -> Vec<MindmapDiagramRenderEdge> {
        fn visit(db: &MindmapDb, node_id: i32, edges: &mut Vec<MindmapDiagramRenderEdge>) {
            let Ok(node_idx) = usize::try_from(node_id) else {
                return;
            };
            let Some(node) = db.nodes.get(node_idx) else {
                return;
            };
            for child_id in node.children.iter() {
                let Ok(child_idx) = usize::try_from(*child_id) else {
                    continue;
                };
                let Some(child) = db.nodes.get(child_idx) else {
                    continue;
                };

                let mut classes = "edge".to_string();
                if let Some(section) = child.section {
                    classes.push_str(&format!(" section-edge-{section}"));
                }
                let edge_depth = node.level + 1;
                classes.push_str(&format!(" edge-depth-{edge_depth}"));

                edges.push(MindmapDiagramRenderEdge {
                    id: format!("edge_{}_{}", node.id, child.id),
                    start: node.id.to_string(),
                    end: child.id.to_string(),
                    edge_type: "normal".to_string(),
                    curve: "basis".to_string(),
                    thickness: "normal".to_string(),
                    look: "default".to_string(),
                    classes,
                    depth: node.level as i64,
                    section: child.section,
                });

                visit(db, *child_id, edges);
            }
        }

        let mut edges = Vec::new();
        visit(self, root_id, &mut edges);
        edges
    }
}

pub fn parse_mindmap(code: &str, meta: &ParseMetadata) -> Result<Value> {
    parse_mindmap_impl(code, meta, MindmapEmit::Full)
}

pub fn parse_mindmap_for_render(code: &str, meta: &ParseMetadata) -> Result<Value> {
    parse_mindmap_impl(code, meta, MindmapEmit::RenderOnly)
}

pub fn parse_mindmap_model_for_render(
    code: &str,
    meta: &ParseMetadata,
) -> Result<MindmapDiagramRenderModel> {
    let mut db = parse_mindmap_db(code, meta)?;
    let Some(root_id) = db.get_mindmap().map(|n| n.id) else {
        return Ok(MindmapDiagramRenderModel::default());
    };

    db.assign_sections(root_id, None);

    Ok(MindmapDiagramRenderModel {
        nodes: db.to_layout_nodes_for_render(root_id),
        edges: db.to_edges_for_render(root_id),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MindmapEmit {
    Full,
    RenderOnly,
}

fn parse_mindmap_db(code: &str, meta: &ParseMetadata) -> Result<MindmapDb> {
    let mut db = MindmapDb::default();
    db.clear();

    let mut lines = code.lines();
    let mut found_header = false;
    let mut header_tail: Option<String> = None;
    for line in lines.by_ref() {
        let t = strip_inline_comment(line);
        let trimmed = t.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.eq_ignore_ascii_case("mindmap") {
            found_header = true;
            break;
        }
        if starts_with_case_insensitive(trimmed, "mindmap")
            && trimmed.len() > "mindmap".len()
            && trimmed["mindmap".len()..]
                .chars()
                .next()
                .is_some_and(|c| c.is_whitespace())
        {
            found_header = true;
            let after_keyword = &trimmed["mindmap".len()..];
            let indent = after_keyword
                .chars()
                .take_while(|c| c.is_whitespace())
                .count();
            let rest = after_keyword.trim_start();
            if !rest.is_empty() {
                header_tail = Some(format!("{}{}", " ".repeat(indent), rest));
            }
            break;
        }
        break;
    }

    if !found_header {
        return Err(Error::DiagramParse {
            diagram_type: meta.diagram_type.clone(),
            message: "expected mindmap header".to_string(),
        });
    }

    enum HandleOutcome {
        Done,
        NeedMoreInput,
    }

    let mut handle_line = |line: &str| -> Result<HandleOutcome> {
        let line = strip_inline_comment(line);
        if line.trim().is_empty() {
            return Ok(HandleOutcome::Done);
        }

        let (indent, rest) = split_indent(line);
        let rest = rest.trim_end();
        if rest.is_empty() {
            return Ok(HandleOutcome::Done);
        }

        if starts_with_case_insensitive(rest, "::icon(") {
            let after = &rest["::icon(".len()..];
            let Some(end) = after.find(')') else {
                return Ok(HandleOutcome::Done);
            };
            let icon = after[..end].to_string();
            db.decorate_last(None, Some(icon), &meta.effective_config);
            return Ok(HandleOutcome::Done);
        }

        if let Some(after) = rest.strip_prefix(":::") {
            db.decorate_last(Some(after.trim().to_string()), None, &meta.effective_config);
            return Ok(HandleOutcome::Done);
        }

        let (id_raw, descr_raw, ty, descr_is_markdown) = match parse_node_spec(rest) {
            Ok(v) => v,
            Err(message) if message == "unterminated node delimiter" => {
                return Ok(HandleOutcome::NeedMoreInput);
            }
            Err(message) => {
                return Err(Error::DiagramParse {
                    diagram_type: meta.diagram_type.clone(),
                    message,
                });
            }
        };
        db.add_node(
            indent as i32,
            &id_raw,
            &descr_raw,
            descr_is_markdown,
            ty,
            &meta.diagram_type,
            &meta.effective_config,
        )?;
        Ok(HandleOutcome::Done)
    };

    let mut pending: Option<String> = None;
    let mut push_and_try = |physical_line: &str| -> Result<()> {
        match pending.as_mut() {
            Some(buf) => {
                buf.push('\n');
                buf.push_str(physical_line);
            }
            None => pending = Some(physical_line.to_string()),
        }

        let current = pending.as_deref().unwrap_or_default();
        match handle_line(current)? {
            HandleOutcome::Done => {
                pending = None;
            }
            HandleOutcome::NeedMoreInput => {}
        }
        Ok(())
    };

    if let Some(tail) = &header_tail {
        push_and_try(tail)?;
    }
    for line in lines {
        push_and_try(line)?;
    }
    if let Some(buf) = pending {
        let line = strip_inline_comment(&buf);
        if !line.trim().is_empty() {
            return Err(Error::DiagramParse {
                diagram_type: meta.diagram_type.clone(),
                message: "unterminated node delimiter".to_string(),
            });
        }
    }

    Ok(db)
}

fn parse_mindmap_impl(code: &str, meta: &ParseMetadata, emit: MindmapEmit) -> Result<Value> {
    let mut db = parse_mindmap_db(code, meta)?;

    let Some(root_id) = db.get_mindmap().map(|n| n.id) else {
        if emit == MindmapEmit::Full {
            let mut final_config = meta.effective_config.as_value().clone();
            if meta.config.as_value().get("layout").is_none() {
                if let Some(obj) = final_config.as_object_mut() {
                    obj.insert(
                        "layout".to_string(),
                        Value::String("cose-bilkent".to_string()),
                    );
                }
            }

            return Ok(json!({
                "nodes": [],
                "edges": [],
                "config": final_config,
            }));
        }

        return Ok(json!({
            "nodes": [],
            "edges": [],
        }));
    };

    db.assign_sections(root_id, None);

    let nodes = db.to_layout_node_values(root_id);
    let edges = db.to_edge_values(root_id);

    if emit == MindmapEmit::RenderOnly {
        return Ok(json!({
            "nodes": nodes,
            "edges": edges,
        }));
    }

    let mut final_config = meta.effective_config.as_value().clone();
    if meta.config.as_value().get("layout").is_none() {
        if let Some(obj) = final_config.as_object_mut() {
            obj.insert(
                "layout".to_string(),
                Value::String("cose-bilkent".to_string()),
            );
        }
    }

    let mut shapes = Map::new();
    for n in nodes.iter() {
        let Some(node) = n.as_object() else {
            continue;
        };
        let Some(id) = node.get("id").and_then(|v| v.as_str()) else {
            continue;
        };
        let shape = node.get("shape").cloned().unwrap_or(Value::Null);
        let width = node.get("width").cloned().unwrap_or(Value::Null);
        let height = node.get("height").cloned().unwrap_or(Value::Null);
        let padding = node.get("padding").cloned().unwrap_or(Value::Null);
        shapes.insert(
            id.to_string(),
            json!({
                "shape": shape,
                "width": width,
                "height": height,
                "padding": padding,
            }),
        );
    }

    let diagram_id = MINDMAP_DIAGRAM_ID_COUNTER.fetch_add(1, Ordering::Relaxed) + 1;

    Ok(json!({
        "type": meta.diagram_type,
        "nodes": nodes,
        "edges": edges,
        "config": final_config,
        "rootNode": db.to_root_node_value(root_id),
        "markers": ["point"],
        "direction": "TB",
        "nodeSpacing": 50,
        "rankSpacing": 50,
        "shapes": Value::Object(shapes),
        // Mermaid uses a random UUID v4 here. For performance and determinism, keep a cheap
        // monotonic id that is unique within the current process. Snapshot tests normalize this
        // field to "<dynamic>".
        "diagramId": format!("mindmap-{diagram_id}"),
    }))
}

fn starts_with_case_insensitive(haystack: &str, needle: &str) -> bool {
    if haystack.len() < needle.len() {
        return false;
    }
    haystack
        .as_bytes()
        .iter()
        .take(needle.len())
        .copied()
        .map(|b| b.to_ascii_lowercase())
        .eq(needle
            .as_bytes()
            .iter()
            .copied()
            .map(|b| b.to_ascii_lowercase()))
}

fn split_indent(line: &str) -> (usize, &str) {
    let mut indent_chars = 0usize;
    let mut byte_idx = line.len();
    for (idx, ch) in line.char_indices() {
        if ch.is_whitespace() {
            indent_chars += 1;
            continue;
        }
        byte_idx = idx;
        break;
    }
    if indent_chars == 0 {
        byte_idx = 0;
    } else if byte_idx == line.len() {
        byte_idx = line.len();
    }
    (indent_chars, &line[byte_idx..])
}

fn strip_inline_comment(line: &str) -> &str {
    let mut in_quote = false;
    let mut in_backtick_quote = false;

    let mut it = line.char_indices().peekable();
    while let Some((idx, ch)) = it.next() {
        if in_backtick_quote {
            if ch == '`' && it.peek().is_some_and(|(_, next)| *next == '"') {
                in_backtick_quote = false;
                it.next();
            }
            continue;
        }

        if in_quote {
            if ch == '"' {
                in_quote = false;
            }
            continue;
        }

        if ch == '"' {
            if it.peek().is_some_and(|(_, next)| *next == '`') {
                in_backtick_quote = true;
                it.next();
                continue;
            }
            in_quote = true;
            continue;
        }

        if ch == '%' && it.peek().is_some_and(|(_, next)| *next == '%') {
            return &line[..idx];
        }
    }

    line
}

fn parse_node_spec(input: &str) -> std::result::Result<(String, String, i32, bool), String> {
    let input = input.trim_end();
    if input.is_empty() {
        return Err("expected node".to_string());
    }

    if let Some((start, end)) = node_delimiter_pair_at_start(input) {
        let (inner, tail) = extract_delimited(input, start, end)?;
        if !tail.trim().is_empty() {
            return Err("unexpected trailing input".to_string());
        }
        let (descr, descr_is_markdown) = unquote_node_descr(inner);
        let ty = node_type_for(start, end);
        return Ok((descr.clone(), descr, ty, descr_is_markdown));
    }

    let (id_raw, rest) = split_node_id(input);
    let id_raw = id_raw.to_string();
    let rest = rest.trim_end();
    if rest.is_empty() {
        return Ok((id_raw.clone(), id_raw, NODE_TYPE_DEFAULT, false));
    }

    let Some((start, end)) = node_delimiter_pair_at_start(rest) else {
        return Err("expected node delimiter".to_string());
    };

    let (inner, tail) = extract_delimited(rest, start, end)?;
    if !tail.trim().is_empty() {
        return Err("unexpected trailing input".to_string());
    }

    let (descr, descr_is_markdown) = unquote_node_descr(inner);
    let ty = node_type_for(start, end);
    Ok((id_raw, descr, ty, descr_is_markdown))
}

fn split_node_id(input: &str) -> (&str, &str) {
    let bytes = input.as_bytes();
    for (idx, b) in bytes.iter().enumerate() {
        match b {
            b'(' | b')' | b'[' | b'{' | b'}' => return (&input[..idx], &input[idx..]),
            _ => {}
        }
    }
    (input, "")
}

fn node_delimiter_pair_at_start(input: &str) -> Option<(&'static str, &'static str)> {
    let pairs: &[(&str, &str)] = &[
        ("(-", "-)"),
        ("-)", "(-"),
        ("((", "))"),
        ("))", "(("),
        ("{{", "}}"),
        ("[", "]"),
        (")", "("),
        ("(", ")"),
    ];

    for (start, end) in pairs {
        if input.starts_with(start) {
            return Some((*start, *end));
        }
    }
    None
}

fn extract_delimited<'a>(
    input: &'a str,
    start: &str,
    end: &str,
) -> std::result::Result<(&'a str, &'a str), String> {
    if !input.starts_with(start) {
        return Err("expected delimiter start".to_string());
    }
    let mut in_quote = false;
    let mut in_backtick_quote = false;

    let start_len = start.len();
    let mut it = input[start_len..].char_indices().peekable();
    while let Some((off, ch)) = it.next() {
        let idx = start_len + off;

        if in_backtick_quote {
            if ch == '`' && it.peek().is_some_and(|(_, next)| *next == '"') {
                in_backtick_quote = false;
                it.next();
            }
            continue;
        }

        if in_quote {
            if ch == '"' {
                in_quote = false;
            }
            continue;
        }

        if ch == '"' {
            if it.peek().is_some_and(|(_, next)| *next == '`') {
                in_backtick_quote = true;
                it.next();
                continue;
            }
            in_quote = true;
            continue;
        }

        if input[idx..].starts_with(end) {
            let inner = &input[start_len..idx];
            let tail = &input[idx + end.len()..];
            return Ok((inner, tail));
        }
    }

    Err("unterminated node delimiter".to_string())
}

fn unquote_node_descr(raw: &str) -> (String, bool) {
    // Mermaid mindmap uses a special `"` + backtick quote form for Markdown strings, e.g.:
    //   id1["`**Root** with\nsecond line`"]
    if let Some(inner) = raw.strip_prefix("\"`").and_then(|s| s.strip_suffix("`\"")) {
        return (inner.to_string(), true);
    }
    if let Some(inner) = raw.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
        return (inner.to_string(), false);
    }
    (raw.to_string(), false)
}

fn node_type_for(start: &str, end: &str) -> i32 {
    match start {
        "[" => NODE_TYPE_RECT,
        "(" => {
            if end == ")" {
                NODE_TYPE_ROUNDED_RECT
            } else {
                NODE_TYPE_CLOUD
            }
        }
        "((" => NODE_TYPE_CIRCLE,
        ")" => NODE_TYPE_CLOUD,
        "))" => NODE_TYPE_BANG,
        "{{" => NODE_TYPE_HEXAGON,
        _ => NODE_TYPE_DEFAULT,
    }
}

fn get_i64(cfg: &MermaidConfig, dotted_path: &str) -> Option<i64> {
    let mut cur = cfg.as_value();
    for segment in dotted_path.split('.') {
        cur = cur.as_object()?.get(segment)?;
    }
    cur.as_i64().or_else(|| cur.as_f64().map(|f| f as i64))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Engine, ParseOptions};
    use futures::executor::block_on;

    fn parse(text: &str) -> Value {
        let engine = Engine::new();
        block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap()
            .model
    }

    fn root_descr(model: &Value) -> &str {
        model["rootNode"]["descr"].as_str().unwrap()
    }

    #[test]
    fn mindmap_simple_root() {
        let model = parse("mindmap\n    root");
        assert_eq!(root_descr(&model), "root");
    }

    #[test]
    fn mindmap_simple_root_shaped_without_id() {
        let model = parse("mindmap\n    (root)");
        assert_eq!(root_descr(&model), "root");
        assert_eq!(model["rootNode"]["nodeId"].as_str().unwrap(), "root");
    }

    #[test]
    fn mindmap_hierarchy_two_children() {
        let model = parse("mindmap\n    root\n      child1\n      child2\n");
        assert_eq!(root_descr(&model), "root");
        assert_eq!(model["rootNode"]["children"].as_array().unwrap().len(), 2);
        assert_eq!(
            model["rootNode"]["children"][0]["descr"].as_str().unwrap(),
            "child1"
        );
        assert_eq!(
            model["rootNode"]["children"][1]["descr"].as_str().unwrap(),
            "child2"
        );
    }

    #[test]
    fn mindmap_deeper_hierarchy() {
        let model = parse("mindmap\n    root\n      child1\n        leaf1\n      child2");
        let mm = &model["rootNode"];
        assert_eq!(mm["descr"].as_str().unwrap(), "root");
        let children = mm["children"].as_array().unwrap();
        assert_eq!(children.len(), 2);
        assert_eq!(children[0]["descr"].as_str().unwrap(), "child1");
        assert_eq!(
            children[0]["children"][0]["descr"].as_str().unwrap(),
            "leaf1"
        );
        assert_eq!(children[1]["descr"].as_str().unwrap(), "child2");
    }

    #[test]
    fn mindmap_multiple_roots_is_error() {
        let engine = Engine::new();
        let err = block_on(
            engine.parse_diagram("mindmap\n    root\n    fakeRoot", ParseOptions::default()),
        )
        .unwrap_err();
        assert!(
            err.to_string().contains(
                "There can be only one root. No parent could be found for (\"fakeRoot\")"
            )
        );
    }

    #[test]
    fn mindmap_real_root_in_wrong_place_is_error() {
        let engine = Engine::new();
        let text = "mindmap\n          root\n        fakeRoot\n    realRootWrongPlace";
        let err = block_on(engine.parse_diagram(text, ParseOptions::default())).unwrap_err();
        assert!(
            err.to_string().contains(
                "There can be only one root. No parent could be found for (\"fakeRoot\")"
            )
        );
    }

    #[test]
    fn mindmap_node_id_and_label_and_type_rect() {
        let model = parse("mindmap\n    root[The root]\n");
        assert_eq!(model["rootNode"]["nodeId"].as_str().unwrap(), "root");
        assert_eq!(root_descr(&model), "The root");
        assert_eq!(
            model["rootNode"]["type"].as_i64().unwrap(),
            NODE_TYPE_RECT as i64
        );
    }

    #[test]
    fn mindmap_child_node_id_and_type_rounded_rect() {
        let model = parse("mindmap\n    root\n      theId(child1)");
        let child = &model["rootNode"]["children"][0];
        assert_eq!(child["descr"].as_str().unwrap(), "child1");
        assert_eq!(child["nodeId"].as_str().unwrap(), "theId");
        assert_eq!(
            child["type"].as_i64().unwrap(),
            NODE_TYPE_ROUNDED_RECT as i64
        );
    }

    #[test]
    fn mindmap_node_types_circle_cloud_bang_hexagon() {
        let circle = parse("mindmap\n root((the root))");
        assert_eq!(
            circle["rootNode"]["type"].as_i64().unwrap(),
            NODE_TYPE_CIRCLE as i64
        );
        assert_eq!(circle["rootNode"]["descr"].as_str().unwrap(), "the root");

        let cloud = parse("mindmap\n root)the root(");
        assert_eq!(
            cloud["rootNode"]["type"].as_i64().unwrap(),
            NODE_TYPE_CLOUD as i64
        );
        assert_eq!(cloud["rootNode"]["descr"].as_str().unwrap(), "the root");

        let bang = parse("mindmap\n root))the root((");
        assert_eq!(
            bang["rootNode"]["type"].as_i64().unwrap(),
            NODE_TYPE_BANG as i64
        );
        assert_eq!(bang["rootNode"]["descr"].as_str().unwrap(), "the root");

        let hex = parse("mindmap\n root{{the root}}");
        assert_eq!(
            hex["rootNode"]["type"].as_i64().unwrap(),
            NODE_TYPE_HEXAGON as i64
        );
        assert_eq!(hex["rootNode"]["descr"].as_str().unwrap(), "the root");
    }

    #[test]
    fn mindmap_icon_and_class_decorations() {
        let model = parse("mindmap\n    root[The root]\n    :::m-4 p-8\n    ::icon(bomb)\n");
        assert_eq!(model["rootNode"]["class"].as_str().unwrap(), "m-4 p-8");
        assert_eq!(model["rootNode"]["icon"].as_str().unwrap(), "bomb");
    }

    #[test]
    fn mindmap_can_set_icon_then_class_or_class_then_icon() {
        let model = parse("mindmap\n    root[The root]\n    :::m-4 p-8\n    ::icon(bomb)\n");
        assert_eq!(model["rootNode"]["class"].as_str().unwrap(), "m-4 p-8");
        assert_eq!(model["rootNode"]["icon"].as_str().unwrap(), "bomb");

        let model = parse("mindmap\n    root[The root]\n    ::icon(bomb)\n    :::m-4 p-8\n");
        assert_eq!(model["rootNode"]["class"].as_str().unwrap(), "m-4 p-8");
        assert_eq!(model["rootNode"]["icon"].as_str().unwrap(), "bomb");
    }

    #[test]
    fn mindmap_quoted_descriptions_can_contain_delimiters() {
        let model = parse("mindmap\n    root[\"String containing []\"]");
        assert_eq!(model["rootNode"]["nodeId"].as_str().unwrap(), "root");
        assert_eq!(
            model["rootNode"]["descr"].as_str().unwrap(),
            "String containing []"
        );

        let model = parse(
            "mindmap\n    root[\"String containing []\"]\n      child1[\"String containing ()\"]",
        );
        assert_eq!(model["rootNode"]["children"].as_array().unwrap().len(), 1);
        assert_eq!(
            model["rootNode"]["children"][0]["descr"].as_str().unwrap(),
            "String containing ()"
        );
    }

    #[test]
    fn mindmap_child_after_class_assignment_is_attached_to_last_node() {
        let model = parse(
            "mindmap\n  root(Root)\n    Child(Child)\n    :::hot\n      a(a)\n      b[New Stuff]",
        );
        let mm = &model["rootNode"];
        assert_eq!(mm["nodeId"].as_str().unwrap(), "root");
        let child = &mm["children"][0];
        assert_eq!(child["nodeId"].as_str().unwrap(), "Child");
        assert_eq!(child["children"].as_array().unwrap().len(), 2);
        assert_eq!(child["children"][0]["nodeId"].as_str().unwrap(), "a");
        assert_eq!(child["children"][1]["nodeId"].as_str().unwrap(), "b");
    }

    #[test]
    fn mindmap_comment_end_of_line_is_ignored() {
        let model = parse(
            "mindmap\n  root(Root)\n    Child(Child)\n      a(a) %% This is a comment\n      b[New Stuff]\n",
        );
        let child = &model["rootNode"]["children"][0];
        assert_eq!(child["nodeId"].as_str().unwrap(), "Child");
        assert_eq!(child["children"].as_array().unwrap().len(), 2);
        assert_eq!(child["children"][1]["nodeId"].as_str().unwrap(), "b");
    }

    #[test]
    fn mindmap_rows_above_declaration_are_ignored() {
        let model = parse("\n \n\nmindmap\nroot\n A\n \n\n B");
        assert_eq!(model["rootNode"]["nodeId"].as_str().unwrap(), "root");
        assert_eq!(model["rootNode"]["children"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn mindmap_leading_comment_lines_before_declaration_are_ignored() {
        let model = parse("%% comment\n\nmindmap\nroot\n A\n B");
        assert_eq!(model["rootNode"]["nodeId"].as_str().unwrap(), "root");
        assert_eq!(model["rootNode"]["children"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn mindmap_root_without_indent_child_with_indent() {
        let model = parse("mindmap\nroot\n      theId(child1)");
        let mm = &model["rootNode"];
        assert_eq!(mm["nodeId"].as_str().unwrap(), "root");
        assert_eq!(mm["children"].as_array().unwrap().len(), 1);
        let child = &mm["children"][0];
        assert_eq!(child["descr"].as_str().unwrap(), "child1");
        assert_eq!(child["nodeId"].as_str().unwrap(), "theId");
    }

    #[test]
    fn mindmap_rows_with_only_spaces_do_not_interfere() {
        let model = parse("mindmap\nroot\n A\n \n\n B");
        let mm = &model["rootNode"];
        assert_eq!(mm["nodeId"].as_str().unwrap(), "root");
        assert_eq!(mm["children"].as_array().unwrap().len(), 2);
        assert_eq!(mm["children"][0]["nodeId"].as_str().unwrap(), "A");
        assert_eq!(mm["children"][1]["nodeId"].as_str().unwrap(), "B");
    }

    #[test]
    fn mindmap_meaningless_empty_rows_do_not_interfere() {
        let model =
            parse("mindmap\n  root(Root)\n    Child(Child)\n      a(a)\n\n      b[New Stuff]");
        let mm = &model["rootNode"];
        assert_eq!(mm["nodeId"].as_str().unwrap(), "root");
        let child = &mm["children"][0];
        assert_eq!(child["nodeId"].as_str().unwrap(), "Child");
        assert_eq!(child["children"].as_array().unwrap().len(), 2);
        assert_eq!(child["children"][1]["nodeId"].as_str().unwrap(), "b");
    }

    #[test]
    fn mindmap_header_can_share_line_with_root_node() {
        let model = parse("mindmap root\n  child1\n");
        let mm = &model["rootNode"];
        assert_eq!(mm["descr"].as_str().unwrap(), "root");
        assert_eq!(mm["children"].as_array().unwrap().len(), 1);
        assert_eq!(mm["children"][0]["descr"].as_str().unwrap(), "child1");
    }

    #[test]
    fn mindmap_multiline_markdown_string_node_description_is_parsed() {
        let model = parse(
            "mindmap\n    id1[\"`**Root** with\n\
a second line\n\
Unicode works too: 🤓`\"]\n      id2[\"`The dog in **the** hog... a *very long text* that wraps to a new line`\"]\n      id3[Regular labels still works]\n",
        );
        let root = &model["rootNode"];
        assert_eq!(root["nodeId"].as_str().unwrap(), "id1");
        let descr = root["descr"].as_str().unwrap();
        assert!(descr.contains("Root"));
        assert!(descr.contains("a second line"));
        assert!(descr.contains("🤓"));
    }

    #[test]
    fn mindmap_get_data_empty_when_no_nodes() {
        let model = parse("mindmap\n");
        assert_eq!(model["nodes"].as_array().unwrap().len(), 0);
        assert_eq!(model["edges"].as_array().unwrap().len(), 0);
        assert!(model.get("rootNode").is_none());
        assert!(model.get("config").is_some());
    }

    #[test]
    fn mindmap_get_data_basic_nodes_edges_and_layout_defaults() {
        let model = parse("mindmap\nroot(Root Node)\n child1(Child 1)\n child2(Child 2)\n");

        assert_eq!(model["nodes"].as_array().unwrap().len(), 3);
        assert_eq!(model["edges"].as_array().unwrap().len(), 2);
        assert_eq!(model["config"]["layout"].as_str().unwrap(), "cose-bilkent");
        assert!(model["diagramId"].as_str().unwrap().starts_with("mindmap-"));

        let root = model["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"].as_str() == Some("0"))
            .unwrap();
        assert_eq!(root["label"].as_str().unwrap(), "Root Node");
        assert_eq!(root["level"].as_i64().unwrap(), 0);

        let edge_0_1 = model["edges"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["start"].as_str() == Some("0") && e["end"].as_str() == Some("1"))
            .unwrap();
        assert_eq!(edge_0_1["depth"].as_i64().unwrap(), 0);
    }

    #[test]
    fn mindmap_get_data_assigns_section_classes_to_nodes_and_edges() {
        let model = parse("mindmap\nA\n a0\n  aa0\n a1\n  aaa\n a2\n");
        let nodes = model["nodes"].as_array().unwrap();

        let node_a = nodes
            .iter()
            .find(|n| n["label"].as_str() == Some("A"))
            .unwrap();
        let node_a0 = nodes
            .iter()
            .find(|n| n["label"].as_str() == Some("a0"))
            .unwrap();
        let node_aa0 = nodes
            .iter()
            .find(|n| n["label"].as_str() == Some("aa0"))
            .unwrap();
        let node_a1 = nodes
            .iter()
            .find(|n| n["label"].as_str() == Some("a1"))
            .unwrap();
        let node_aaa = nodes
            .iter()
            .find(|n| n["label"].as_str() == Some("aaa"))
            .unwrap();
        let node_a2 = nodes
            .iter()
            .find(|n| n["label"].as_str() == Some("a2"))
            .unwrap();

        assert!(node_a.get("section").is_none());
        assert_eq!(
            node_a["cssClasses"].as_str().unwrap(),
            "mindmap-node section-root section--1"
        );
        assert_eq!(node_a0["section"].as_i64().unwrap(), 0);
        assert_eq!(node_aa0["section"].as_i64().unwrap(), 0);
        assert_eq!(node_a1["section"].as_i64().unwrap(), 1);
        assert_eq!(node_aaa["section"].as_i64().unwrap(), 1);
        assert_eq!(node_a2["section"].as_i64().unwrap(), 2);

        let edges = model["edges"].as_array().unwrap();
        assert_eq!(edges.len(), 5);

        let edge_0_1 = edges
            .iter()
            .find(|e| e["start"].as_str() == Some("0") && e["end"].as_str() == Some("1"))
            .unwrap();
        let edge_1_2 = edges
            .iter()
            .find(|e| e["start"].as_str() == Some("1") && e["end"].as_str() == Some("2"))
            .unwrap();
        let edge_0_3 = edges
            .iter()
            .find(|e| e["start"].as_str() == Some("0") && e["end"].as_str() == Some("3"))
            .unwrap();
        let edge_3_4 = edges
            .iter()
            .find(|e| e["start"].as_str() == Some("3") && e["end"].as_str() == Some("4"))
            .unwrap();
        let edge_0_5 = edges
            .iter()
            .find(|e| e["start"].as_str() == Some("0") && e["end"].as_str() == Some("5"))
            .unwrap();

        assert_eq!(
            edge_0_1["classes"].as_str().unwrap(),
            "edge section-edge-0 edge-depth-1"
        );
        assert_eq!(
            edge_1_2["classes"].as_str().unwrap(),
            "edge section-edge-0 edge-depth-2"
        );
        assert_eq!(
            edge_0_3["classes"].as_str().unwrap(),
            "edge section-edge-1 edge-depth-1"
        );
        assert_eq!(
            edge_3_4["classes"].as_str().unwrap(),
            "edge section-edge-1 edge-depth-2"
        );
        assert_eq!(
            edge_0_5["classes"].as_str().unwrap(),
            "edge section-edge-2 edge-depth-1"
        );

        assert_eq!(edge_0_1["section"].as_i64().unwrap(), 0);
        assert_eq!(edge_1_2["section"].as_i64().unwrap(), 0);
        assert_eq!(edge_0_3["section"].as_i64().unwrap(), 1);
        assert_eq!(edge_3_4["section"].as_i64().unwrap(), 1);
        assert_eq!(edge_0_5["section"].as_i64().unwrap(), 2);
    }

    #[test]
    fn mindmap_get_data_edge_ids_are_unique() {
        let model = parse("mindmap\nroot\n child1\n child2\n child3\n");
        let edges = model["edges"].as_array().unwrap();
        assert_eq!(edges.len(), 3);

        let ids: Vec<&str> = edges.iter().map(|e| e["id"].as_str().unwrap()).collect();
        let unique: std::collections::BTreeSet<&str> = ids.iter().copied().collect();
        assert_eq!(unique.len(), ids.len());
    }

    #[test]
    fn mindmap_get_data_missing_optional_properties_are_absent() {
        let model = parse("mindmap\nroot\n");
        let nodes = model["nodes"].as_array().unwrap();
        assert_eq!(nodes.len(), 1);
        let node = nodes[0].as_object().unwrap();

        assert!(node.get("section").is_none());
        assert_eq!(
            node.get("cssClasses").and_then(|v| v.as_str()).unwrap(),
            "mindmap-node section-root section--1"
        );
        assert!(node.get("icon").is_none());
        assert!(node.get("x").is_none());
        assert!(node.get("y").is_none());
    }

    #[test]
    fn mindmap_get_data_preserves_custom_classes_while_adding_section_classes() {
        let model = parse(
            "mindmap\nroot(Root Node)\n:::custom-root-class\n child(Child Node)\n :::custom-child-class\n",
        );

        let nodes = model["nodes"].as_array().unwrap();
        let root = nodes
            .iter()
            .find(|n| n["label"].as_str() == Some("Root Node"))
            .unwrap();
        let child = nodes
            .iter()
            .find(|n| n["label"].as_str() == Some("Child Node"))
            .unwrap();

        assert_eq!(
            root["cssClasses"].as_str().unwrap(),
            "mindmap-node section-root section--1 custom-root-class"
        );
        assert_eq!(
            child["cssClasses"].as_str().unwrap(),
            "mindmap-node section-0 custom-child-class"
        );
    }

    #[test]
    fn mindmap_padding_doubles_for_rect_like_nodes() {
        let model = parse("mindmap\nroot[Root]\n");
        let node = model["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"].as_str() == Some("0"))
            .unwrap();
        assert_eq!(node["type"].as_i64().unwrap(), NODE_TYPE_RECT as i64);
        assert_eq!(node["padding"].as_i64().unwrap(), 20);
    }

    #[test]
    fn mindmap_empty_rows_and_comments_do_not_interfere() {
        let model = parse(
            "mindmap\n  root(Root)\n    Child(Child)\n      a(a)\n\n      %% This is a comment\n      b[New Stuff]\n",
        );
        let child = &model["rootNode"]["children"][0];
        assert_eq!(child["nodeId"].as_str().unwrap(), "Child");
        assert_eq!(child["children"].as_array().unwrap().len(), 2);
        assert_eq!(child["children"][1]["nodeId"].as_str().unwrap(), "b");
    }
}
