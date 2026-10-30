//! Canonical Mindmap data, display rules, and borrowed terminal views.

mod display;
mod terminal;
pub use display::MindmapDisplayPolicy;
pub use terminal::{
    MindmapTerminalEdge, MindmapTerminalNode, MindmapTerminalNodeId, MindmapTerminalView,
};
#[cfg(feature = "presentation-data")]
mod presentation;
#[cfg(feature = "presentation-data")]
pub use presentation::{
    MindmapDiagramRenderEdge, MindmapDiagramRenderModel, MindmapDiagramRenderNode,
};

/// Canonical authored Mindmap tree. Shared display rules derive shapes and sections;
/// render dimensions and DOM fields remain presentation data.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct MindmapDiagramModel {
    pub nodes: Vec<MindmapNodeModel>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MindmapNodeModel {
    pub id: i32,
    pub node_id: String,
    pub level: i32,
    pub descr: String,
    pub descr_is_markdown: bool,
    #[serde(rename = "type")]
    pub node_type: i32,
    pub children: Vec<i32>,
    pub class: Option<String>,
    pub icon: Option<String>,
    pub is_root: bool,
}

pub const NODE_TYPE_DEFAULT: i32 = 0;
pub const NODE_TYPE_ROUNDED_RECT: i32 = 1;
pub const NODE_TYPE_RECT: i32 = 2;
pub const NODE_TYPE_CIRCLE: i32 = 3;
pub const NODE_TYPE_CLOUD: i32 = 4;
pub const NODE_TYPE_BANG: i32 = 5;
pub const NODE_TYPE_HEXAGON: i32 = 6;
