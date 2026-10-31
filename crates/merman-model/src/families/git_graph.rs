#[cfg(feature = "compat-json")]
pub mod compat;

use crate::warnings::DiagramWarningFact;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GitGraphBranch {
    pub name: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GitGraphCommit {
    pub id: String,
    pub message: String,
    pub seq: i64,
    #[serde(rename = "type")]
    pub commit_type: i64,
    pub tags: Vec<String>,
    pub parents: Vec<String>,
    pub branch: String,
    #[serde(rename = "customType", skip_serializing_if = "Option::is_none")]
    pub custom_type: Option<i64>,
    #[serde(rename = "customId", skip_serializing_if = "Option::is_none")]
    pub custom_id: Option<bool>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
/// Canonical commits and branches of a GitGraph diagram.
pub struct GitGraphDiagramModel {
    #[serde(rename = "type")]
    pub diagram_type: String,
    pub commits: Vec<GitGraphCommit>,
    pub branches: Vec<GitGraphBranch>,
    #[serde(rename = "currentBranch")]
    pub current_branch: String,
    pub direction: String,
    pub title: Option<String>,
    #[serde(rename = "accTitle")]
    pub acc_title: Option<String>,
    #[serde(rename = "accDescr")]
    pub acc_descr: Option<String>,
    #[serde(
        default,
        rename = "warningFacts",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub warning_facts: Vec<DiagramWarningFact>,
}
