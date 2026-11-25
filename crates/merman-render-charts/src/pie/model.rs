use merman_render_core::Bounds;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PieSliceLayout {
    pub label: String,
    pub value: f64,
    pub start_angle: f64,
    pub end_angle: f64,
    pub is_full_circle: bool,
    pub percent: i64,
    pub text_x: f64,
    pub text_y: f64,
    pub fill: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PieLegendItemLayout {
    pub label: String,
    pub value: f64,
    pub fill: String,
    pub y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PieDiagramLayout {
    pub bounds: Option<Bounds>,
    #[serde(default)]
    pub title: Option<String>,
    pub center_x: f64,
    pub center_y: f64,
    pub radius: f64,
    pub outer_radius: f64,
    pub legend_x: f64,
    pub legend_start_y: f64,
    pub legend_step_y: f64,
    pub slices: Vec<PieSliceLayout>,
    pub legend_items: Vec<PieLegendItemLayout>,
}
