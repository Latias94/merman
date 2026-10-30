//! Canonical architecture nodes, groups, edges, and layout hints.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ArchitectureLayoutDirection {
    Row,
    Column,
}

impl ArchitectureLayoutDirection {
    /// Returns the direction's canonical vocabulary value.
    pub fn as_str(self) -> &'static str {
        match self {
            ArchitectureLayoutDirection::Row => "row",
            ArchitectureLayoutDirection::Column => "column",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ArchitectureDiagramModel {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default, rename = "accTitle")]
    pub acc_title: Option<String>,
    #[serde(default, rename = "accDescr")]
    pub acc_descr: Option<String>,
    #[serde(default)]
    pub nodes: Vec<ArchitectureNodeModel>,
    #[serde(default)]
    pub groups: Vec<ArchitectureGroupModel>,
    #[serde(default)]
    pub edges: Vec<ArchitectureEdgeModel>,
    #[serde(default, rename = "layoutHints")]
    pub layout_hints: Vec<ArchitectureLayoutHintModel>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArchitectureNodeKind {
    #[serde(rename = "service")]
    Service,
    #[serde(rename = "junction")]
    Junction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchitectureNodeModel {
    pub id: String,
    #[serde(rename = "type")]
    pub node_type: ArchitectureNodeKind,
    #[serde(default)]
    pub edge_indices: Vec<usize>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default, rename = "iconText")]
    pub icon_text: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default, rename = "in")]
    pub in_group: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchitectureGroupModel {
    pub id: String,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default, rename = "in")]
    pub in_group: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchitectureEdgeModel {
    #[serde(rename = "lhsId")]
    pub lhs_id: String,
    #[serde(rename = "lhsDir")]
    pub lhs_dir: char,
    #[serde(default, rename = "lhsInto")]
    pub lhs_into: Option<bool>,
    #[serde(default, rename = "lhsGroup")]
    pub lhs_group: Option<bool>,
    #[serde(rename = "rhsId")]
    pub rhs_id: String,
    #[serde(rename = "rhsDir")]
    pub rhs_dir: char,
    #[serde(default, rename = "rhsInto")]
    pub rhs_into: Option<bool>,
    #[serde(default, rename = "rhsGroup")]
    pub rhs_group: Option<bool>,
    #[serde(default)]
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchitectureLayoutHintModel {
    pub direction: ArchitectureLayoutDirection,
    #[serde(default)]
    pub members: Vec<String>,
}
