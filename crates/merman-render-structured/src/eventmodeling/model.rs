use merman_render_core::Bounds;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventModelingSwimlaneLayout {
    pub index: i64,
    pub label: String,
    pub namespace: Option<String>,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventModelingBoxLayout {
    pub index: usize,
    pub frame_name: String,
    pub frame_kind: String,
    pub model_entity_type: String,
    pub entity_identifier: String,
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub fill: String,
    pub stroke: String,
    pub swimlane_index: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventModelingRelationLayout {
    pub source_frame: String,
    pub target_frame: String,
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
    pub stroke: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventModelingDiagramLayout {
    pub bounds: Option<Bounds>,
    pub total_width: f64,
    pub total_height: f64,
    pub viewbox_x: f64,
    pub viewbox_y: f64,
    pub padding: f64,
    pub use_max_width: bool,
    pub swimlanes: Vec<EventModelingSwimlaneLayout>,
    pub boxes: Vec<EventModelingBoxLayout>,
    pub relations: Vec<EventModelingRelationLayout>,
}
