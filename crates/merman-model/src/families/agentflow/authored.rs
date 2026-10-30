use crate::families::flowchart::FlowEdgeDefaults;
#[cfg(feature = "presentation-data")]
use crate::families::flowchart::FlowchartModel;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Source-authored style, interaction, and label facts, excluded from Mermaid compatibility JSON.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentflowAuthoredStyles {
    #[serde(default)]
    pub nodes: HashMap<String, AgentflowNodeAttributes>,
    #[serde(default)]
    pub edges: HashMap<String, AgentflowEdgeAttributes>,
    #[serde(default)]
    pub subgraph_classes: HashMap<String, Vec<String>>,
    #[serde(default)]
    pub subgraph_label_types: HashMap<String, String>,
    #[serde(default)]
    pub class_defs: IndexMap<String, Vec<String>>,
    #[serde(default)]
    pub edge_defaults: Option<FlowEdgeDefaults>,
    #[serde(default)]
    pub tooltips: HashMap<String, String>,
}

/// Authored label kind, classes, CSS, and admitted interaction for a node.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentflowNodeAttributes {
    #[serde(default)]
    pub label_type: Option<String>,
    pub classes: Vec<String>,
    pub styles: Vec<String>,
    pub link: Option<String>,
    pub link_target: Option<String>,
    pub have_callback: bool,
}

/// Authored label kind, classes, CSS, curve, and animation for an edge.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentflowEdgeAttributes {
    #[serde(default)]
    pub label_type: Option<String>,
    pub classes: Vec<String>,
    pub styles: Vec<String>,
    pub interpolate: Option<String>,
    pub animate: Option<bool>,
    pub animation: Option<String>,
}

impl AgentflowAuthoredStyles {
    /// Borrows the canonical CSS declaration lists without building a Flowchart model.
    #[doc(hidden)]
    pub fn declaration_styles(&self) -> impl Iterator<Item = &[String]> {
        self.class_defs
            .values()
            .map(Vec::as_slice)
            .chain(self.nodes.values().map(|node| node.styles.as_slice()))
            .chain(self.edges.values().map(|edge| edge.styles.as_slice()))
            .chain(self.edge_defaults.iter().map(|edge| edge.style.as_slice()))
    }

    pub fn node_label_type(&self, id: &str) -> Option<&str> {
        self.nodes
            .get(id)
            .map(|node| node.label_type.as_deref().unwrap_or("text"))
    }

    pub fn edge_label_type(&self, id: &str) -> Option<&str> {
        self.edges
            .get(id)
            .and_then(|edge| edge.label_type.as_deref())
    }

    #[cfg(feature = "presentation-data")]
    pub(super) fn apply_to_flowchart(&self, model: &mut FlowchartModel) {
        model.class_defs = self.class_defs.clone();
        model.edge_defaults = self.edge_defaults.clone();
        model.tooltips = self
            .tooltips
            .iter()
            .map(|(id, text)| (id.clone(), text.clone()))
            .collect();
        for node in &mut model.nodes {
            if let Some(presentation) = self.nodes.get(&node.id) {
                node.label_type = self.node_label_type(&node.id).map(str::to_owned);
                node.classes.extend(presentation.classes.iter().cloned());
                node.styles = presentation.styles.clone();
                node.link = presentation.link.clone();
                node.link_target = presentation.link_target.clone();
                node.have_callback = presentation.have_callback;
            }
        }
        for edge in &mut model.edges {
            if let Some(presentation) = self.edges.get(&edge.id) {
                edge.label_type = self.edge_label_type(&edge.id).map(str::to_owned);
                edge.classes = presentation.classes.clone();
                edge.style = presentation.styles.clone();
                edge.interpolate = presentation.interpolate.clone();
                edge.animate = presentation.animate;
                edge.animation = presentation.animation.clone();
            }
        }
        for graph in &mut model.subgraphs {
            graph.label_type = self.subgraph_label_types.get(&graph.id).cloned();
            // addNodeFromVertex overrides container CSS when the id also has a vertex record.
            if let Some(vertex) = self.nodes.get(&graph.id) {
                graph.classes = vertex.classes.clone();
                graph.styles = vertex.styles.clone();
            } else if let Some(classes) = self.subgraph_classes.get(&graph.id) {
                graph.classes = classes.clone();
            }
        }
    }
}
