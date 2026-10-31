//! Canonical Sankey graph data.

#[cfg(feature = "compat-json")]
pub mod compat;

use serde_json::Value;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SankeyNode {
    pub id: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SankeyLink {
    pub source: String,
    pub target: String,
    pub value: Value,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct SankeyGraph {
    #[serde(default)]
    pub nodes: Vec<SankeyNode>,
    #[serde(default)]
    pub links: Vec<SankeyLink>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct SankeyDiagramModel {
    #[serde(default)]
    pub graph: SankeyGraph,
}
