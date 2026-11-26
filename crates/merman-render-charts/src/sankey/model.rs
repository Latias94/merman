use merman_render_core::Bounds;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SankeyNodeLayout {
    pub id: String,
    pub index: usize,
    pub depth: usize,
    pub height: usize,
    pub layer: usize,
    pub value: f64,
    pub x0: f64,
    pub x1: f64,
    pub y0: f64,
    pub y1: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SankeyLinkLayout {
    pub index: usize,
    pub source: String,
    pub target: String,
    pub value: f64,
    pub width: f64,
    pub y0: f64,
    pub y1: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SankeyDiagramLayout {
    pub bounds: Option<Bounds>,
    pub width: f64,
    pub height: f64,
    pub node_width: f64,
    pub node_padding: f64,
    pub nodes: Vec<SankeyNodeLayout>,
    pub links: Vec<SankeyLinkLayout>,
}
