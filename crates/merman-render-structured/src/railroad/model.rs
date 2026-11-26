use merman_render_core::Bounds;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RailroadDiagramLayout {
    pub bounds: Option<Bounds>,
    pub diagram_type: String,
    pub width: f64,
    pub height: f64,
    pub use_max_width: bool,
    pub rules: Vec<RailroadRuleLayout>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RailroadRuleLayout {
    pub name: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub baseline_y: f64,
    pub name_width: f64,
    pub definition_x: f64,
    pub start_marker_x: f64,
    pub end_marker_x: f64,
    pub marker_radius: f64,
    pub elements: Vec<RailroadElementLayout>,
    pub paths: Vec<RailroadPathLayout>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RailroadElementLayout {
    pub kind: String,
    pub label: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub text_x: f64,
    pub text_y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RailroadPathLayout {
    #[serde(default)]
    pub x: f64,
    #[serde(default)]
    pub y: f64,
    pub d: String,
}
