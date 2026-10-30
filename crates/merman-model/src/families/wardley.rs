//! Canonical Wardley map vocabulary and authored map data.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WardleySourceStrategy {
    Build,
    Buy,
    Outsource,
    Market,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WardleyFlowDirection {
    Forward,
    Backward,
    Bidirectional,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WardleyPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WardleyNode {
    pub id: String,
    pub label: String,
    pub x: f64,
    pub y: f64,
    #[serde(default, rename = "className", skip_serializing_if = "Option::is_none")]
    pub class_name: Option<String>,
    #[serde(
        default,
        rename = "labelOffsetX",
        skip_serializing_if = "Option::is_none"
    )]
    pub label_offset_x: Option<i64>,
    #[serde(
        default,
        rename = "labelOffsetY",
        skip_serializing_if = "Option::is_none"
    )]
    pub label_offset_y: Option<i64>,
    #[serde(default, rename = "inPipeline", skip_serializing_if = "is_false")]
    pub in_pipeline: bool,
    #[serde(default, rename = "isPipelineParent", skip_serializing_if = "is_false")]
    pub is_pipeline_parent: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inertia: Option<bool>,
    #[serde(
        default,
        rename = "sourceStrategy",
        skip_serializing_if = "Option::is_none"
    )]
    pub source_strategy: Option<WardleySourceStrategy>,
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WardleyLink {
    pub source: String,
    pub target: String,
    #[serde(default)]
    pub dashed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flow: Option<WardleyFlowDirection>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WardleyTrend {
    #[serde(rename = "nodeId")]
    pub node_id: String,
    #[serde(rename = "targetX")]
    pub target_x: f64,
    #[serde(rename = "targetY")]
    pub target_y: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WardleyPipeline {
    #[serde(rename = "nodeId")]
    pub node_id: String,
    #[serde(default, rename = "componentIds")]
    pub component_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WardleyAnnotation {
    pub number: u64,
    #[serde(default)]
    pub coordinates: Vec<WardleyPoint>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WardleyNote {
    pub text: String,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WardleyAccelerator {
    pub name: String,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WardleyDeaccelerator {
    pub name: String,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct WardleyAxes {
    #[serde(default, rename = "xLabel", skip_serializing_if = "Option::is_none")]
    pub x_label: Option<String>,
    #[serde(default, rename = "yLabel", skip_serializing_if = "Option::is_none")]
    pub y_label: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stages: Vec<String>,
    #[serde(
        default,
        rename = "stageBoundaries",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub stage_boundaries: Vec<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WardleySize {
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
/// Authored map meaning; node and annotation positions retain visibility/evolution percentages.
pub struct WardleyDiagramModel {
    #[serde(default, rename = "accTitle", skip_serializing_if = "Option::is_none")]
    pub acc_title: Option<String>,
    #[serde(default, rename = "accDescr", skip_serializing_if = "Option::is_none")]
    pub acc_descr: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default)]
    pub nodes: Vec<WardleyNode>,
    #[serde(default)]
    pub links: Vec<WardleyLink>,
    #[serde(default)]
    pub trends: Vec<WardleyTrend>,
    #[serde(default)]
    pub pipelines: Vec<WardleyPipeline>,
    #[serde(default)]
    pub annotations: Vec<WardleyAnnotation>,
    #[serde(default)]
    pub notes: Vec<WardleyNote>,
    #[serde(default)]
    pub accelerators: Vec<WardleyAccelerator>,
    #[serde(default)]
    pub deaccelerators: Vec<WardleyDeaccelerator>,
    #[serde(
        default,
        rename = "annotationsBox",
        skip_serializing_if = "Option::is_none"
    )]
    pub annotations_box: Option<WardleyPoint>,
    #[serde(default)]
    pub axes: WardleyAxes,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<WardleySize>,
}
