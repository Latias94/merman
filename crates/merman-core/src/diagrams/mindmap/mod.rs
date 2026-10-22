mod db;
mod display;
mod parse;
#[cfg(feature = "presentation")]
mod render_model;
mod terminal;
mod utils;

#[cfg(all(test, feature = "compat-json"))]
mod tests;

pub use db::{MindmapDiagramModel, MindmapNodeModel};
pub use display::MindmapDisplayPolicy;
#[cfg(feature = "compat-json")]
pub(crate) use parse::parse_mindmap;
#[cfg(all(feature = "compat-json", feature = "editor-facts"))]
pub(crate) use parse::parse_mindmap_json_and_editor_facts;
#[cfg(all(test, feature = "presentation"))]
pub(crate) use parse::parse_mindmap_model_for_render;
#[cfg(feature = "presentation")]
pub(crate) use parse::parse_mindmap_model_for_render_controlled;
pub(crate) use parse::parse_mindmap_semantic_model_controlled;
#[cfg(test)]
pub(crate) use parse::{
    mindmap_syntax_construction_count, reset_mindmap_syntax_construction_count,
};
#[cfg(feature = "presentation")]
pub use render_model::{
    MindmapDiagramRenderEdge, MindmapDiagramRenderModel, MindmapDiagramRenderNode,
};
#[cfg(feature = "compat-json")]
pub(crate) use render_model::{
    render_model_to_compat_json, render_model_to_compat_json_controlled,
};
pub use terminal::{
    MindmapTerminalEdge, MindmapTerminalNode, MindmapTerminalNodeId, MindmapTerminalView,
};

const NODE_TYPE_DEFAULT: i32 = 0;
const NODE_TYPE_ROUNDED_RECT: i32 = 1;
const NODE_TYPE_RECT: i32 = 2;
const NODE_TYPE_CIRCLE: i32 = 3;
const NODE_TYPE_CLOUD: i32 = 4;
const NODE_TYPE_BANG: i32 = 5;
const NODE_TYPE_HEXAGON: i32 = 6;
