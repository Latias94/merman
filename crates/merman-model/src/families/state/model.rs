use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

use super::{StateDocument, StateDocumentId};

/// Canonical State meaning and graph, independent of SVG presentation defaults.
#[derive(Debug, Clone, Default)]
pub struct StateDiagramModel {
    pub document: StateDocument,
    pub direction: String,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub nodes: Vec<StateNode>,
    pub edges: Vec<StateEdge>,
    pub relations: Vec<StateRelation>,
    pub links: HashMap<String, StateLinks>,
    pub states: HashMap<String, StateRecord>,
    pub style_classes: IndexMap<String, StateStyleClass>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StateNode {
    pub id: String,
    pub label: Option<Value>,
    pub description: Option<Vec<String>>,
    pub is_group: bool,
    #[serde(rename = "type")]
    pub node_type: Option<String>,
    pub parent_id: Option<String>,
    pub css_classes: String,
    pub css_compiled_styles: Vec<String>,
    pub css_styles: Vec<String>,
    pub dir: Option<String>,
    /// Whether the nested direction was authored, rather than supplied by Mermaid's default.
    pub explicit_dir: Option<bool>,
    pub shape: String,
    pub position: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StateEdge {
    pub id: String,
    pub start: String,
    pub end: String,
    /// Includes `note-edge` for the generated connection to a note.
    pub classes: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StateStyleClass {
    pub id: String,
    #[serde(default)]
    pub styles: Vec<String>,
    #[serde(default, rename = "textStyles")]
    pub text_styles: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct StateRecord {
    pub id: String,
    pub state_type: String,
    pub descriptions: Vec<String>,
    /// References this state's statements in the owning model's flat document store.
    pub doc: Option<StateDocumentId>,
    pub note: Option<StateNote>,
    pub classes: Vec<String>,
    pub styles: Vec<String>,
    pub text_styles: Vec<String>,
    pub start: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StateRelation {
    pub id1: String,
    pub id2: String,
    #[serde(default, rename = "relationTitle")]
    pub relation_title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StateNote {
    #[serde(default)]
    pub position: Option<String>,
    #[serde(default)]
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StateLink {
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub tooltip: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StateLinks {
    One(StateLink),
    Many(Vec<StateLink>),
}
