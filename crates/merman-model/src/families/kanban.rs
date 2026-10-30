//! Canonical Kanban cards, managed metadata, and borrowed terminal views.

use crate::SourceSpan;
use serde::{Deserialize, Serialize};
use serde_json::Value;

mod complexity;
#[cfg(test)]
mod tests;
#[cfg(feature = "presentation-data")]
pub use complexity::model_complexity_from_kanban;
pub use complexity::model_complexity_from_kanban_semantic;

// Numeric node types shared with source construction and typed fallback output.
pub const NODE_TYPE_DEFAULT: i32 = 0;
pub const NODE_TYPE_ROUNDED_RECT: i32 = 1;
pub const NODE_TYPE_RECT: i32 = 2;
pub const NODE_TYPE_CIRCLE: i32 = 3;
pub const NODE_TYPE_CLOUD: i32 = 4;
pub const NODE_TYPE_BANG: i32 = 5;
pub const NODE_TYPE_HEXAGON: i32 = 6;

/// Authored Kanban sections, cards, and card metadata, without visual dimensions.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct KanbanDiagramModel {
    pub nodes: Vec<KanbanNodeModel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KanbanNodeModel {
    pub id: String,
    pub label: String,
    pub level: usize,
    pub node_type: i32,
    pub span: SourceSpan,
    pub is_group: bool,
    pub parent_id: Option<String>,
    pub ticket: Option<String>,
    pub priority: Option<String>,
    pub assigned: Option<String>,
    pub icon: Option<String>,
    pub css_classes: Option<String>,
    pub shape: Option<String>,
    /// Source-authored metadata, including properties unknown to the renderer.
    pub metadata: Option<crate::ManagedJson>,
}

#[derive(Debug, Clone, Copy)]
enum KanbanTerminalBacking<'a> {
    Canonical(&'a KanbanDiagramModel),
    #[cfg(feature = "presentation-data")]
    Full(&'a KanbanDiagramRenderModel),
}

/// Borrows the admitted board without constructing dimensions or copying its cards.
#[derive(Debug, Clone, Copy)]
pub struct KanbanTerminalView<'a>(KanbanTerminalBacking<'a>);

impl<'a> KanbanTerminalView<'a> {
    pub fn from_model(model: &'a KanbanDiagramModel) -> Self {
        Self(KanbanTerminalBacking::Canonical(model))
    }

    #[cfg(feature = "presentation-data")]
    pub fn from_render(model: &'a KanbanDiagramRenderModel) -> Self {
        Self(KanbanTerminalBacking::Full(model))
    }

    pub fn nodes(self) -> impl ExactSizeIterator<Item = KanbanTerminalNode<'a>> {
        let count = match self.0 {
            KanbanTerminalBacking::Canonical(model) => model.nodes.len(),
            #[cfg(feature = "presentation-data")]
            KanbanTerminalBacking::Full(model) => model.nodes.len(),
        };
        (0..count).map(move |index| self.node_at(index))
    }

    fn node_at(self, index: usize) -> KanbanTerminalNode<'a> {
        match self.0 {
            KanbanTerminalBacking::Canonical(model) => {
                let node = &model.nodes[index];
                KanbanTerminalNode {
                    id: &node.id,
                    label: &node.label,
                    is_group: node.is_group,
                    parent_id: node.parent_id.as_deref(),
                    ticket: node.ticket.as_deref(),
                    priority: node.priority.as_deref(),
                    assigned: node.assigned.as_deref(),
                    icon: node.icon.as_deref(),
                    authored: Some(node),
                }
            }
            #[cfg(feature = "presentation-data")]
            KanbanTerminalBacking::Full(model) => {
                let node = &model.nodes[index];
                KanbanTerminalNode {
                    id: &node.id,
                    label: &node.label,
                    is_group: node.is_group,
                    parent_id: node.parent_id.as_deref(),
                    ticket: node.ticket.as_deref(),
                    priority: node.priority.as_deref(),
                    assigned: node.assigned.as_deref(),
                    icon: node.icon.as_deref(),
                    authored: None,
                }
            }
        }
    }

    pub fn node(self, index: usize) -> Option<KanbanTerminalNode<'a>> {
        (index < self.nodes().len()).then(|| self.node_at(index))
    }

    pub fn complexity(self) -> crate::resources::ModelComplexity {
        match self.0 {
            KanbanTerminalBacking::Canonical(model) => model_complexity_from_kanban_semantic(model),
            #[cfg(feature = "presentation-data")]
            KanbanTerminalBacking::Full(model) => {
                let mut complexity = model_complexity_from_kanban(model);
                for node in &model.nodes {
                    for text in [
                        node.compatibility.css_classes.as_deref(),
                        node.compatibility.shape.as_deref(),
                    ]
                    .into_iter()
                    .flatten()
                    {
                        complexity.text_bytes = complexity.text_bytes.saturating_add(text.len());
                    }
                }
                complexity
            }
        }
    }
}

impl Serialize for KanbanTerminalView<'_> {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        match self.0 {
            KanbanTerminalBacking::Canonical(model) => {
                #[derive(Serialize)]
                struct Wire<'a> {
                    nodes: CanonicalTerminalNodes<'a>,
                }
                Wire {
                    nodes: CanonicalTerminalNodes(&model.nodes),
                }
                .serialize(serializer)
            }
            #[cfg(feature = "presentation-data")]
            KanbanTerminalBacking::Full(model) => {
                #[derive(Serialize)]
                struct Wire<'a> {
                    nodes: FullTerminalNodes<'a>,
                }
                Wire {
                    nodes: FullTerminalNodes(&model.nodes),
                }
                .serialize(serializer)
            }
        }
    }
}

struct CanonicalTerminalNodes<'a>(&'a [KanbanNodeModel]);

impl Serialize for CanonicalTerminalNodes<'_> {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Node<'a> {
            id: &'a str,
            label: &'a str,
            level: usize,
            node_type: i32,
            span: &'a SourceSpan,
            is_group: bool,
            parent_id: Option<&'a str>,
            ticket: Option<&'a str>,
            priority: Option<&'a str>,
            assigned: Option<&'a str>,
            icon: Option<&'a str>,
            css_classes: Option<&'a str>,
            shape: Option<&'a str>,
            metadata: Option<&'a Value>,
        }

        serializer.collect_seq(self.0.iter().map(|node| Node {
            id: &node.id,
            label: &node.label,
            level: node.level,
            node_type: node.node_type,
            span: &node.span,
            is_group: node.is_group,
            parent_id: node.parent_id.as_deref(),
            ticket: node.ticket.as_deref(),
            priority: node.priority.as_deref(),
            assigned: node.assigned.as_deref(),
            icon: node.icon.as_deref(),
            css_classes: node.css_classes.as_deref(),
            shape: node.shape.as_deref(),
            metadata: node.metadata.as_ref().map(crate::ManagedJson::as_value),
        }))
    }
}

#[cfg(feature = "presentation-data")]
struct FullTerminalNodes<'a>(&'a [KanbanRenderNode]);

#[cfg(feature = "presentation-data")]
impl Serialize for FullTerminalNodes<'_> {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct Node<'a> {
            #[serde(flatten)]
            node: &'a KanbanRenderNode,
            level: usize,
            #[serde(rename = "cssClasses", skip_serializing_if = "Option::is_none")]
            css_classes: Option<&'a str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            shape: Option<&'a str>,
        }
        serializer.collect_seq(self.0.iter().map(|node| Node {
            node,
            level: node.compatibility.level,
            css_classes: node.compatibility.css_classes.as_deref(),
            shape: node.compatibility.shape.as_deref(),
        }))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct KanbanTerminalNode<'a> {
    pub id: &'a str,
    pub label: &'a str,
    pub is_group: bool,
    pub parent_id: Option<&'a str>,
    pub ticket: Option<&'a str>,
    pub priority: Option<&'a str>,
    pub assigned: Option<&'a str>,
    pub icon: Option<&'a str>,
    authored: Option<&'a KanbanNodeModel>,
}

impl<'a> KanbanTerminalNode<'a> {
    /// Returns all original canonical card fields, including unknown authored metadata.
    pub fn authored(self) -> Option<&'a KanbanNodeModel> {
        self.authored
    }
}

#[cfg(feature = "presentation-data")]
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct KanbanDiagramRenderModel {
    #[serde(default)]
    pub nodes: Vec<KanbanRenderNode>,
}

#[cfg(feature = "presentation-data")]
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct KanbanRenderNode {
    pub id: String,
    pub label: String,
    #[serde(default, rename = "isGroup")]
    pub is_group: bool,
    #[serde(default, rename = "parentId")]
    pub parent_id: Option<String>,
    #[serde(default)]
    pub ticket: Option<String>,
    #[serde(default)]
    pub priority: Option<String>,
    #[serde(default)]
    pub assigned: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(skip)]
    compatibility: KanbanRenderNodeCompatibility,
}

#[cfg(feature = "presentation-data")]
#[derive(Debug, Clone, Default)]
/// Frozen full-output fields, including terminal fallback facts and projection dimensions.
/// These values do not grant source provenance or perform config resolution.
pub struct KanbanRenderNodeCompatibility {
    pub level: usize,
    pub width: i64,
    pub padding: i64,
    pub ticket: Option<String>,
    pub priority: Option<String>,
    pub assigned: Option<String>,
    pub icon: Option<String>,
    pub css_classes: Option<String>,
    pub shape: Option<String>,
}

#[cfg(feature = "presentation-data")]
impl KanbanRenderNode {
    /// Creates a render node with no parent or optional card metadata.
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            ..Self::default()
        }
    }

    /// Attaches the complete declared full-output record; typed serde keeps it out of the wire shape.
    pub fn with_compatibility(mut self, compatibility: KanbanRenderNodeCompatibility) -> Self {
        self.compatibility = compatibility;
        self
    }

    pub fn compatibility(&self) -> &KanbanRenderNodeCompatibility {
        &self.compatibility
    }
}
