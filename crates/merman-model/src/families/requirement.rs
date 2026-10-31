//! Canonical requirement declarations, elements, relationships, and classes.

#[cfg(feature = "compat-json")]
pub mod compat;

use std::collections::BTreeMap;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequirementNode {
    pub name: String,
    #[serde(rename = "type")]
    pub node_type: String,
    #[serde(default, rename = "requirementId")]
    pub requirement_id: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub risk: String,
    #[serde(default, rename = "verifyMethod")]
    pub verify_method: String,
    #[serde(default)]
    pub css_styles: Vec<String>,
    #[serde(default)]
    pub classes: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequirementElement {
    pub name: String,
    #[serde(rename = "type")]
    pub element_type: String,
    #[serde(default, rename = "docRef")]
    pub doc_ref: String,
    #[serde(default)]
    pub css_styles: Vec<String>,
    #[serde(default)]
    pub classes: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RequirementRelationship {
    #[serde(rename = "type")]
    pub rel_type: String,
    pub src: String,
    pub dst: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequirementClass {
    pub id: String,
    #[serde(default)]
    pub styles: Vec<String>,
    #[serde(default, rename = "textStyles")]
    pub text_styles: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
/// Requirement declarations, relationships, and authored styles.
pub struct RequirementDiagramModel {
    #[serde(default, rename = "accTitle")]
    pub acc_title: Option<String>,
    #[serde(default, rename = "accDescr")]
    pub acc_descr: Option<String>,
    #[serde(default)]
    pub direction: String,
    #[serde(default)]
    pub requirements: Vec<RequirementNode>,
    #[serde(default)]
    pub elements: Vec<RequirementElement>,
    #[serde(default)]
    pub relationships: Vec<RequirementRelationship>,
    #[serde(default)]
    pub classes: BTreeMap<String, RequirementClass>,
}
