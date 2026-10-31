#[cfg(feature = "compat-json")]
use serde_json::Value;

#[cfg(test)]
use super::NODE_TYPE_DEFAULT;
use super::{MindmapDisplayPolicy, NODE_TYPE_HEXAGON, NODE_TYPE_RECT, NODE_TYPE_ROUNDED_RECT};
#[cfg(feature = "compat-json")]
use crate::Error;
use crate::{
    MermaidConfig, OperationControl, OperationControlResult, Result, SourcePresentationMetadata,
};

use merman_model::families::mindmap::{
    MindmapDiagramRenderEdge, MindmapDiagramRenderModel, MindmapDiagramRenderNode,
};

pub(super) fn model_into_render_model_controlled(
    model: super::MindmapDiagramModel,
    meta: &SourcePresentationMetadata,
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
    let display = MindmapDisplayPolicy::from_config(config);
    projection.assign_sections_controlled(root_id, None, display, control)?;
    let nodes =
        projection.to_layout_nodes_for_render_controlled(root_id, config, display, control)?;
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
    display: MindmapDisplayPolicy,
) -> MindmapDiagramRenderNode {
    MindmapDiagramRenderNode {
        id: node.id.to_string(),
        dom_id: format!("node_{}", node.id),
        label: node.descr.clone(),
        // Mermaid's `flattenNodes()` marks every layout node as Markdown. The parser's quoted
        // description flag only controls sanitization while the node enters this DB.
        label_type: "markdown".to_string(),
        is_group: false,
        shape: display.shape(node.ty).to_string(),
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
        display: MindmapDisplayPolicy,
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
            node.section = display.section(node_level, section, depth);

            let child_count = node.children.len();
            for index in (0..child_count).rev() {
                if index % 128 == 0 {
                    control.checkpoint()?;
                }
                let child_id = self.nodes[node_idx].children[index];
                let child_section = display.child_section(node_level, index, section);
                stack.push((child_id, child_section, depth + 1));
            }
        }
        control.checkpoint()
    }

    fn to_layout_nodes_for_render_controlled(
        &self,
        root_id: i32,
        config: &MermaidConfig,
        display: MindmapDisplayPolicy,
        control: &OperationControl,
    ) -> OperationControlResult<Vec<MindmapDiagramRenderNode>> {
        let mut out = Vec::new();
        let look = mindmap_look(config);
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

            out.push(mindmap_render_node(node, &look, display));

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
    meta: &SourcePresentationMetadata,
) -> Result<Value> {
    render_model_to_compat_json_controlled(model, meta, &OperationControl::new())
        .expect("a private parse control cannot be cancelled")
}

#[cfg(feature = "compat-json")]
pub(crate) fn render_model_to_compat_json_controlled(
    model: &MindmapDiagramRenderModel,
    meta: &SourcePresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<Result<Value>> {
    let diagram_id = merman_model::families::mindmap::diagram_id_from_context(
        &crate::runtime::active_operation_context(),
    );
    let mut projection = meta.compat_json_metadata();
    projection.diagram_id = Some(&diagram_id);
    merman_model::families::mindmap::compat::project_controlled(model, &projection, control)
        .map(|result| result.map_err(Error::from))
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

        let metadata = SourcePresentationMetadata {
            diagram_type: "mindmap".to_string(),
            config: config.clone(),
            effective_config: crate::presentation::SourcePresentationConfig::from_value(
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
