use crate::sanitize::sanitize_text;
use crate::{Error, MermaidConfig, Result};
use serde_json::{Map, Value, json};

use super::render_model::{MindmapDiagramRenderEdge, MindmapDiagramRenderNode};
use super::utils::get_i64;
use super::{
    NODE_TYPE_BANG, NODE_TYPE_CIRCLE, NODE_TYPE_CLOUD, NODE_TYPE_DEFAULT, NODE_TYPE_HEXAGON,
    NODE_TYPE_RECT, NODE_TYPE_ROUNDED_RECT,
};

#[derive(Debug, Clone)]
pub(super) struct MindmapNode {
    pub(super) id: i32,
    pub(super) node_id: String,
    pub(super) level: i32,
    pub(super) descr: String,
    pub(super) is_markdown: bool,
    pub(super) ty: i32,
    pub(super) children: Vec<i32>,
    pub(super) width: i64,
    pub(super) padding: i64,
    pub(super) section: Option<i32>,
    pub(super) height: Option<i64>,
    pub(super) class: Option<String>,
    pub(super) icon: Option<String>,
    pub(super) x: Option<f64>,
    pub(super) y: Option<f64>,
    pub(super) is_root: bool,
}

#[derive(Debug, Default)]
pub(super) struct MindmapDb {
    pub(super) nodes: Vec<MindmapNode>,
    base_level: Option<i32>,
}

impl MindmapDb {
    pub(super) fn clear(&mut self) {
        self.nodes.clear();
        self.base_level = None;
    }

    pub(super) fn get_mindmap(&self) -> Option<&MindmapNode> {
        self.nodes.first()
    }

    fn get_parent_index(&self, level: i32) -> Option<usize> {
        self.nodes.iter().rposition(|n| n.level < level)
    }

    pub(super) fn add_node(
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

    pub(super) fn decorate_last(
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

    pub(super) fn assign_sections(&mut self, node_id: i32, section: Option<i32>) {
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

    pub(super) fn to_root_node_value(&self, node_id: i32) -> Value {
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

    pub(super) fn to_layout_node_values(&self, root_id: i32) -> Vec<Value> {
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

    pub(super) fn to_layout_nodes_for_render(&self, root_id: i32) -> Vec<MindmapDiagramRenderNode> {
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

    pub(super) fn to_edge_values(&self, root_id: i32) -> Vec<Value> {
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

    pub(super) fn to_edges_for_render(&self, root_id: i32) -> Vec<MindmapDiagramRenderEdge> {
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
