use merman_render_core::Bounds;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeViewNodeLayout {
    pub id: i64,
    pub level: i64,
    pub name: String,
    #[serde(rename = "nodeType")]
    pub node_type: String,
    #[serde(default, rename = "cssClass", skip_serializing_if = "Option::is_none")]
    pub css_class: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub depth: usize,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub label_x: f64,
    pub label_y: f64,
    pub label_width: f64,
    pub label_height: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description_x: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description_width: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeViewLineLayout {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
    pub stroke_width: f64,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeViewDiagramLayout {
    pub bounds: Option<Bounds>,
    pub total_width: f64,
    pub total_height: f64,
    pub row_indent: f64,
    pub padding_x: f64,
    pub padding_y: f64,
    pub line_thickness: f64,
    pub use_max_width: bool,
    pub label_font_size: f64,
    pub nodes: Vec<TreeViewNodeLayout>,
    pub lines: Vec<TreeViewLineLayout>,
}
