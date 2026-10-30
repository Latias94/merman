//! Canonical Venn subsets, text nodes and styles.

#[cfg(feature = "compat-json")]
pub mod compat;

use std::collections::BTreeMap;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct VennSubsetModel {
    pub sets: Vec<String>,
    pub size: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct VennTextNodeModel {
    pub sets: Vec<String>,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct VennStyleEntryModel {
    pub targets: Vec<String>,
    pub styles: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct VennDiagramModel {
    #[serde(default, rename = "accTitle")]
    pub acc_title: Option<String>,
    #[serde(default, rename = "accDescr")]
    pub acc_descr: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub subsets: Vec<VennSubsetModel>,
    #[serde(default, rename = "textNodes")]
    pub text_nodes: Vec<VennTextNodeModel>,
    #[serde(default, rename = "styleEntries")]
    pub style_entries: Vec<VennStyleEntryModel>,
}
