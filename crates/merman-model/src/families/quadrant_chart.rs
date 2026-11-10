//! Canonical QuadrantChart points, axes, quadrants and styles.

#[cfg(feature = "compat-json")]
pub mod compat;

use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuadrantChartStyles {
    /// JavaScript `parseInt` result, including binary64 rounding and overflow to infinity.
    #[serde(default, with = "crate::number::serde_optional_number")]
    pub radius: Option<f64>,
    pub color: Option<String>,
    pub stroke_color: Option<String>,
    pub stroke_width: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuadrantChartPointModel {
    pub text: String,
    /// Coordinate after JavaScript Number coercion; a nonnumeric lexer token yields NaN.
    #[serde(with = "crate::number::serde_number")]
    pub x: f64,
    /// Coordinate after JavaScript Number coercion; a nonnumeric lexer token yields NaN.
    #[serde(with = "crate::number::serde_number")]
    pub y: f64,
    pub class_name: Option<String>,
    pub styles: QuadrantChartStyles,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuadrantChartQuadrantsModel {
    pub quadrant1_text: String,
    pub quadrant2_text: String,
    pub quadrant3_text: String,
    pub quadrant4_text: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuadrantChartAxesModel {
    pub x_axis_left_text: String,
    pub x_axis_right_text: String,
    pub y_axis_bottom_text: String,
    pub y_axis_top_text: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct QuadrantChartDiagramModel {
    pub title: Option<String>,
    #[serde(rename = "accTitle")]
    pub acc_title: Option<String>,
    #[serde(rename = "accDescr")]
    pub acc_descr: Option<String>,
    pub quadrants: QuadrantChartQuadrantsModel,
    pub axes: QuadrantChartAxesModel,
    pub points: Vec<QuadrantChartPointModel>,
    pub classes: BTreeMap<String, QuadrantChartStyles>,
}
