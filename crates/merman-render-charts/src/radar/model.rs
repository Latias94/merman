use merman_render_core::{Bounds, LayoutPoint};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RadarAxisLayout {
    pub label: String,
    pub angle: f64,
    pub line_x2: f64,
    pub line_y2: f64,
    pub label_x: f64,
    pub label_y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RadarGraticuleShapeLayout {
    pub kind: String,
    #[serde(default, with = "super::point_list::radius")]
    pub r: Option<f64>,
    #[serde(default, with = "super::point_list")]
    pub points: Vec<LayoutPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RadarCurveLayout {
    pub label: String,
    pub class_index: i64,
    #[serde(default, with = "super::point_list")]
    pub points: Vec<LayoutPoint>,
    pub path_d: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RadarLegendItemLayout {
    pub label: String,
    pub class_index: i64,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RadarDiagramLayout {
    pub bounds: Option<Bounds>,
    pub svg_width: f64,
    pub svg_height: f64,
    pub center_x: f64,
    pub center_y: f64,
    pub radius: f64,
    pub axis_label_factor: f64,
    pub title_y: f64,
    #[serde(default)]
    pub axes: Vec<RadarAxisLayout>,
    #[serde(default)]
    pub graticules: Vec<RadarGraticuleShapeLayout>,
    #[serde(default)]
    pub curves: Vec<RadarCurveLayout>,
    #[serde(default)]
    pub legend_items: Vec<RadarLegendItemLayout>,
}
