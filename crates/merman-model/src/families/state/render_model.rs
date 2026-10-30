use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

use super::complexity::StateRecords;
use super::{StateDocument, StateLinks, StateRecord, StateRelation, StateStyleClass};
use crate::resources::ModelComplexity;

#[derive(Debug, Clone, Default)]
pub struct StateDiagramRenderModel {
    /// Canonical statements and document relationships; nested JSON is an explicit projection.
    pub document: StateDocument,
    pub direction: String,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub nodes: Vec<StateDiagramRenderNode>,
    pub edges: Vec<StateDiagramRenderEdge>,
    pub relations: Vec<StateRelation>,
    pub links: HashMap<String, StateLinks>,
    pub states: HashMap<String, StateRecord>,
    pub style_classes: IndexMap<String, StateStyleClass>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StateModelFields<'a> {
    direction: &'a str,
    #[serde(rename = "accTitle")]
    acc_title: &'a Option<String>,
    #[serde(rename = "accDescr")]
    acc_descr: &'a Option<String>,
    nodes: &'a [StateDiagramRenderNode],
    edges: &'a [StateDiagramRenderEdge],
    relations: &'a [StateRelation],
    links: &'a HashMap<String, StateLinks>,
    states: StateRecords<'a>,
    #[serde(rename = "styleClasses")]
    style_classes: &'a IndexMap<String, StateStyleClass>,
}

impl StateDiagramRenderModel {
    fn fields(&self) -> StateModelFields<'_> {
        StateModelFields {
            direction: &self.direction,
            acc_title: &self.acc_title,
            acc_descr: &self.acc_descr,
            nodes: &self.nodes,
            edges: &self.edges,
            relations: &self.relations,
            links: &self.links,
            states: StateRecords {
                states: &self.states,
                include_documents: false,
            },
            style_classes: &self.style_classes,
        }
    }

    pub fn model_complexity(&self) -> ModelComplexity {
        let mut result = ModelComplexity::from_serializable(&self.fields());
        let documents = self.document.complexities();
        for state in self.states.values() {
            if let Some(doc) = state.doc.and_then(|id| documents.get(id.0)).copied() {
                result.items = result.items.saturating_add(doc.items);
                result.text_bytes = result.text_bytes.saturating_add(doc.text_bytes);
                // The old wire shape places a document array below root, states map and state.
                result.nesting_depth = result.nesting_depth.max(2usize.saturating_add(doc.depth));
            }
        }
        result
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StateDiagramRenderNode {
    pub id: String,
    #[serde(default, rename = "labelStyle")]
    pub label_style: String,
    #[serde(default)]
    pub label: Option<Value>,
    #[serde(default)]
    pub description: Option<Vec<String>>,
    #[serde(default, rename = "domId")]
    pub dom_id: String,
    #[serde(default, rename = "isGroup")]
    pub is_group: bool,
    #[serde(default, rename = "type")]
    pub node_type: Option<String>,
    #[serde(default, rename = "parentId")]
    pub parent_id: Option<String>,
    #[serde(default, rename = "cssClasses")]
    pub css_classes: String,
    #[serde(default, rename = "cssCompiledStyles")]
    pub css_compiled_styles: Vec<String>,
    #[serde(default, rename = "cssStyles")]
    pub css_styles: Vec<String>,
    #[serde(default)]
    pub dir: Option<String>,
    // Kept as parser-internal provenance for the ASCII adapter. Mermaid 11.17.2 no longer
    // exposes this rollback-era field in the render model or compatibility JSON.
    #[serde(skip)]
    pub explicit_dir: Option<bool>,
    #[serde(default)]
    pub padding: Option<f64>,
    #[serde(default)]
    pub rx: Option<f64>,
    #[serde(default)]
    pub ry: Option<f64>,
    pub shape: String,
    #[serde(default)]
    pub position: Option<String>,
    /// Palette slot assigned to composite state containers by Mermaid's depth-first data fetcher.
    #[serde(default, rename = "colorIndex")]
    pub color_index: Option<usize>,
    /// Width at which state labels wrap; populated from `state.wrappingWidth`.
    #[serde(default, rename = "wrappingWidth")]
    pub wrapping_width: Option<f64>,
    /// Minimum label width for non-container state nodes; populated from `state.minNodeWidth`.
    #[serde(default, rename = "minWidth")]
    pub min_width: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StateDiagramRenderEdge {
    pub id: String,
    pub start: String,
    pub end: String,
    #[serde(default)]
    pub classes: String,
    #[serde(default, rename = "arrowTypeEnd")]
    pub arrow_type_end: String,
    #[serde(default)]
    pub label: String,
}
