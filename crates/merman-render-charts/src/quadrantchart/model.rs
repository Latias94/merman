use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuadrantChartTextData {
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub fill: String,
    #[serde(rename = "fontSize")]
    pub font_size: f64,
    #[serde(default)]
    pub rotation: f64,
    #[serde(rename = "verticalPos")]
    pub vertical_pos: String,
    #[serde(rename = "horizontalPos")]
    pub horizontal_pos: String,
}

pub type QuadrantChartAxisLabelData = QuadrantChartTextData;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuadrantChartQuadrantData {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub fill: String,
    pub text: QuadrantChartTextData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuadrantChartBorderLineData {
    #[serde(rename = "strokeWidth")]
    pub stroke_width: f64,
    #[serde(rename = "strokeFill")]
    pub stroke_fill: String,
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuadrantChartPointData {
    pub x: f64,
    pub y: f64,
    pub fill: String,
    pub radius: f64,
    #[serde(rename = "strokeColor")]
    pub stroke_color: String,
    #[serde(rename = "strokeWidth")]
    pub stroke_width: String,
    pub text: QuadrantChartTextData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuadrantChartDiagramLayout {
    pub width: f64,
    pub height: f64,
    #[serde(default)]
    pub title: Option<QuadrantChartTextData>,
    pub quadrants: Vec<QuadrantChartQuadrantData>,
    #[serde(rename = "borderLines")]
    pub border_lines: Vec<QuadrantChartBorderLineData>,
    pub points: Vec<QuadrantChartPointData>,
    #[serde(rename = "axisLabels")]
    pub axis_labels: Vec<QuadrantChartAxisLabelData>,
}
