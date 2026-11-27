use merman_render_core::Bounds;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KanbanSectionLayout {
    pub id: String,
    pub label: String,
    pub index: i64,
    pub center_x: f64,
    pub center_y: f64,
    pub width: f64,
    pub rect_y: f64,
    pub rect_height: f64,
    pub rx: f64,
    pub ry: f64,
    pub label_width: f64,
    #[serde(skip)]
    pub label_height: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KanbanItemLayout {
    pub id: String,
    pub label: String,
    pub parent_id: String,
    pub center_x: f64,
    pub center_y: f64,
    pub width: f64,
    pub height: f64,
    pub rx: f64,
    pub ry: f64,
    #[serde(default)]
    pub ticket: Option<String>,
    #[serde(default)]
    pub assigned: Option<String>,
    #[serde(default)]
    pub priority: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KanbanDiagramLayout {
    pub bounds: Option<Bounds>,
    pub section_width: f64,
    pub padding: f64,
    pub max_label_height: f64,
    pub viewbox_padding: f64,
    #[serde(default = "super::config::default_use_max_width")]
    pub use_max_width: bool,
    pub sections: Vec<KanbanSectionLayout>,
    pub items: Vec<KanbanItemLayout>,
}
