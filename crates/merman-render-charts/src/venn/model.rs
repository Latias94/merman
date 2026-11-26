use merman_render_core::Bounds;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VennCircleLayout {
    pub set: String,
    pub x: f64,
    pub y: f64,
    pub radius: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VennAreaLayout {
    pub sets: Vec<String>,
    pub size: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub text_x: f64,
    pub text_y: f64,
    pub text_disjoint: bool,
    #[serde(default)]
    pub circles: Vec<VennCircleLayout>,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VennTextDebugCellLayout {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VennTextAreaLayout {
    pub sets: Vec<String>,
    pub center_x: f64,
    pub center_y: f64,
    pub inner_radius: f64,
    pub font_size: f64,
    #[serde(default)]
    pub debug_cells: Vec<VennTextDebugCellLayout>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VennTextNodeLayout {
    pub sets: Vec<String>,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VennDiagramLayout {
    pub bounds: Option<Bounds>,
    pub width: f64,
    pub height: f64,
    pub diagram_height: f64,
    pub title_height: f64,
    pub scale: f64,
    pub padding: f64,
    pub use_max_width: bool,
    pub use_debug_layout: bool,
    #[serde(default)]
    pub areas: Vec<VennAreaLayout>,
    #[serde(default)]
    pub text_areas: Vec<VennTextAreaLayout>,
    #[serde(default)]
    pub text_nodes: Vec<VennTextNodeLayout>,
}
