mod db;
mod parse;
mod render_model;
mod utils;

#[cfg(test)]
mod tests;

pub(crate) use parse::parse_mindmap_json_and_editor_facts;
#[cfg(all(test, feature = "full"))]
pub(crate) use parse::{
    mindmap_syntax_construction_count, reset_mindmap_syntax_construction_count,
};
pub use parse::{parse_mindmap, parse_mindmap_editor_facts, parse_mindmap_model_for_render};
pub(crate) use render_model::render_model_to_compat_json;
pub use render_model::{
    MindmapDiagramRenderEdge, MindmapDiagramRenderModel, MindmapDiagramRenderNode,
};

const NODE_TYPE_DEFAULT: i32 = 0;
const NODE_TYPE_ROUNDED_RECT: i32 = 1;
const NODE_TYPE_RECT: i32 = 2;
const NODE_TYPE_CIRCLE: i32 = 3;
const NODE_TYPE_CLOUD: i32 = 4;
const NODE_TYPE_BANG: i32 = 5;
const NODE_TYPE_HEXAGON: i32 = 6;
