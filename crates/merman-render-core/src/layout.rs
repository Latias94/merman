//! Shared graph layout projections for renderer families.

use crate::LayoutPoint;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutLabel {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutNode {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub is_cluster: bool,
    #[serde(skip)]
    pub label_width: Option<f64>,
    #[serde(skip)]
    pub label_height: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutCluster {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// Mermaid cluster "diff" value used during cluster positioning.
    pub diff: f64,
    /// Mermaid cluster "offsetY" value: title bbox height minus half padding.
    pub offset_y: f64,
    pub title: String,
    pub title_label: LayoutLabel,
    pub requested_dir: Option<String>,
    pub effective_dir: String,
    pub padding: f64,
    pub title_margin_top: f64,
    pub title_margin_bottom: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutEdge {
    pub id: String,
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub from_cluster: Option<String>,
    #[serde(default)]
    pub to_cluster: Option<String>,
    pub points: Vec<LayoutPoint>,
    pub label: Option<LayoutLabel>,
    #[serde(default)]
    pub start_label_left: Option<LayoutLabel>,
    #[serde(default)]
    pub start_label_right: Option<LayoutLabel>,
    #[serde(default)]
    pub end_label_left: Option<LayoutLabel>,
    #[serde(default)]
    pub end_label_right: Option<LayoutLabel>,
    /// Optional SVG marker id for marker-start (e.g. ER cardinality markers).
    #[serde(default)]
    pub start_marker: Option<String>,
    /// Optional SVG marker id for marker-end (e.g. ER cardinality markers).
    #[serde(default)]
    pub end_marker: Option<String>,
    /// Optional SVG dash pattern (e.g. identifying vs non-identifying ER relationships).
    #[serde(default)]
    pub stroke_dasharray: Option<String>,
}
