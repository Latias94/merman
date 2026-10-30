//! Canonical Agentflow graph, authored style facts, and pure containment views.

#[cfg(feature = "compat-json")]
pub mod compat;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};

mod authored;
pub use authored::{AgentflowAuthoredStyles, AgentflowEdgeAttributes, AgentflowNodeAttributes};
#[cfg(feature = "presentation-data")]
mod presentation;
#[cfg(feature = "presentation-data")]
pub use presentation::{
    AgentflowDiagramRenderModel, AgentflowRenderLabelSources, AgentflowVisuals,
};

/// Semantic kind derived from the authored shape or declaration keyword.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentflowVertexKind {
    Tool,
    Action,
    Input,
    Refdoc,
    Decision,
    Connector,
    Task,
}

impl AgentflowVertexKind {
    #[cfg(feature = "presentation-data")]
    fn css_class(self) -> &'static str {
        match self {
            Self::Tool => "af-kind-tool",
            Self::Task => "af-kind-task",
            Self::Input => "af-kind-input",
            Self::Refdoc => "af-kind-refdoc",
            Self::Action => "af-kind-action",
            Self::Decision => "af-kind-decision",
            Self::Connector => "af-kind-connector",
        }
    }
}

/// The three operators admitted by Mermaid 12 Agentflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentflowEdgeSemantic {
    Sequence,
    Reference,
    Failure,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentflowNode {
    pub id: String,
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shape: Option<String>,
    pub vertex_kind: AgentflowVertexKind,
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub metadata: Map<String, Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentflowEdge {
    pub start: String,
    pub end: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip)]
    pub is_user_defined_id: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub edge_semantic: AgentflowEdgeSemantic,
    #[serde(default, rename = "type")]
    pub edge_type: String,
    #[serde(default)]
    pub stroke: String,
    #[serde(default)]
    pub length: usize,
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub metadata: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentflowSubGraph {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default)]
    pub nodes: Vec<String>,
    #[serde(default, rename = "type")]
    pub sub_graph_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub metadata: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentflowConnector {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub metadata: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentflowDiagnostic {
    pub id: String,
    pub severity: String,
    pub message: String,
    pub node_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<AgentflowDiagnosticPosition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentflowDiagnosticPosition {
    pub start_line: usize,
    pub start_column: usize,
    pub end_line: usize,
    pub end_column: usize,
    pub start_index: usize,
    pub end_index: usize,
}

/// Authored Agentflow graph, including CSS, links, and label interpretation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentflowDiagramModel {
    /// Source-ordered style, interaction, and label facts; no computed DOM identifiers.
    #[serde(default)]
    pub authored: AgentflowAuthoredStyles,
    pub direction: String,
    #[serde(default)]
    pub diagnostics: Vec<AgentflowDiagnostic>,
    #[serde(default)]
    pub warning_facts: Vec<crate::warnings::DiagramWarningFact>,
    #[serde(default)]
    pub vertices: Vec<AgentflowNode>,
    #[serde(default)]
    pub edges: Vec<AgentflowEdge>,
    #[serde(default)]
    pub sub_graphs: Vec<AgentflowSubGraph>,
    #[serde(default)]
    pub connectors: Vec<AgentflowConnector>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, rename = "accTitle", skip_serializing_if = "Option::is_none")]
    pub acc_title: Option<String>,
    #[serde(default, rename = "accDescr", skip_serializing_if = "Option::is_none")]
    pub acc_descr: Option<String>,
}

impl AgentflowDiagramModel {
    /// Derives containment warnings without changing authored membership or source positions.
    pub fn containment_diagnostics(&self) -> Vec<AgentflowDiagnostic> {
        self.containment().diagnostics
    }

    // Mirror agentflowDb.getData: resolve collapse first, then admit parents in reverse
    // completion order. Keep the authored membership in the semantic model unchanged.
    fn containment(&self) -> AgentflowContainment<'_> {
        let by_id: HashMap<_, _> = self
            .sub_graphs
            .iter()
            .map(|graph| (graph.id.as_str(), graph))
            .collect();
        let mut result = AgentflowContainment::default();
        for graph in &self.sub_graphs {
            if graph.metadata.get("view").and_then(Value::as_str) != Some("collapsed")
                || result
                    .collapsed_replacements
                    .contains_key(graph.id.as_str())
            {
                continue;
            }
            let mut visited = HashSet::from([graph.id.as_str()]);
            let mut pending: Vec<_> = graph.nodes.iter().rev().map(String::as_str).collect();
            while let Some(id) = pending.pop() {
                if !visited.insert(id) {
                    continue;
                }
                result.collapsed_replacements.insert(id, &graph.id);
                if let Some(child_graph) = by_id.get(id) {
                    pending.extend(child_graph.nodes.iter().rev().map(String::as_str));
                }
            }
        }
        for graph in self.sub_graphs.iter().rev() {
            if result
                .collapsed_replacements
                .contains_key(graph.id.as_str())
            {
                continue;
            }
            for child in &graph.nodes {
                let mut ancestor = Some(graph.id.as_str());
                while let Some(id) = ancestor {
                    if id == child {
                        break;
                    }
                    ancestor = result.parents.get(id).copied();
                }
                if ancestor.is_some() {
                    result.diagnostics.push(AgentflowDiagnostic {
                        id: "CONTAINMENT_VIOLATION".into(), severity: "warning".into(),
                        message: format!("Container \"{}\" cannot contain \"{}\" because \"{}\" already contains it. The nesting that would close the loop is dropped.", graph.id, child, child),
                        node_id: child.clone(),
                        position: None,
                    });
                } else {
                    result.parents.insert(child, &graph.id);
                }
            }
        }
        result
    }
}

#[derive(Default)]
struct AgentflowContainment<'a> {
    collapsed_replacements: HashMap<&'a str, &'a str>,
    parents: HashMap<&'a str, &'a str>,
    diagnostics: Vec<AgentflowDiagnostic>,
}

impl<'a> AgentflowContainment<'a> {
    #[cfg(feature = "presentation-data")]
    fn rendered_edge_endpoints(&self, edge: &'a AgentflowEdge) -> Option<(&'a str, &'a str)> {
        let from = self
            .collapsed_replacements
            .get(edge.start.as_str())
            .copied()
            .unwrap_or(&edge.start);
        let to = self
            .collapsed_replacements
            .get(edge.end.as_str())
            .copied()
            .unwrap_or(&edge.end);
        (edge.start == edge.end || from != to).then_some((from, to))
    }

    #[cfg(feature = "presentation-data")]
    fn color_ordinals(&self, graphs: &[AgentflowSubGraph]) -> Vec<(String, usize)> {
        let mut children: HashMap<&str, Vec<&str>> = HashMap::new();
        for graph in graphs {
            if let Some(parent) = self.parents.get(graph.id.as_str()) {
                children.entry(parent).or_default().push(&graph.id);
            }
        }
        let mut order = Vec::with_capacity(graphs.len());
        let mut seen = HashSet::new();
        let mut pending: Vec<&str> = graphs
            .iter()
            .rev()
            .filter(|graph| !self.parents.contains_key(graph.id.as_str()))
            .map(|graph| graph.id.as_str())
            .collect();
        while let Some(id) = pending.pop() {
            if !seen.insert(id) {
                continue;
            }
            order.push((id.to_string(), order.len()));
            if let Some(children) = children.get(id) {
                pending.extend(children.iter().rev().copied());
            }
        }
        // getData emits containers in reverse completion order; unreachable ones follow.
        for graph in graphs.iter().rev() {
            if seen.insert(&graph.id) {
                order.push((graph.id.clone(), order.len()));
            }
        }
        order
    }
}

/// Shapes supported by the Agentflow domain and its shared flow adapter.
pub const ALLOWED_SHAPES: &[&str] = &[
    "roundedRect",
    "subroutine",
    "subprocess",
    "subproc",
    "framed-rectangle",
    "lean-right",
    "diamond",
    "lin-doc",
    "lined-document",
    "hexagon",
    "hex",
    "connector",
    "collapsedGroup",
];

#[cfg(feature = "presentation-data")]
fn normalized_render_shape(shape: Option<&str>) -> &str {
    match shape {
        Some(shape) if ALLOWED_SHAPES.contains(&shape) => shape,
        _ => "roundedRect",
    }
}

impl AgentflowDiagnostic {
    /// Returns the stable model warning rule for this diagnostic.
    pub fn rule_id(&self) -> &'static str {
        match self.id.as_str() {
            "SHAPE_REMOVED" => crate::warnings::AGENTFLOW_SHAPE_REMOVED_WARNING_RULE_ID,
            "SHAPE_UNSUPPORTED" => crate::warnings::AGENTFLOW_SHAPE_UNSUPPORTED_WARNING_RULE_ID,
            "CONTAINMENT_VIOLATION" => {
                crate::warnings::AGENTFLOW_CONTAINMENT_VIOLATION_WARNING_RULE_ID
            }
            _ => "merman.semantic.agentflow.diagnostic",
        }
    }
}
