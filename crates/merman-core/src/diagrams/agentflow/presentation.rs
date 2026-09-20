//! Presentation directives share Flowchart spelling but follow Agentflow database ordering.

use super::{Declaration, ParseFailure, Parser};
use crate::diagrams::flowchart::{
    self, ClickAction, FlowEdgeDefaults, FlowchartModel, LinkStylePos, Tok,
};
use crate::{EditorSemanticKind, SourceSpan};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashMap;

/// Retained rendering facts, excluded from Agentflow's compatibility semantic projection.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentflowPresentation {
    #[serde(default)]
    pub(super) nodes: HashMap<String, NodePresentation>,
    #[serde(default)]
    edges: HashMap<String, EdgePresentation>,
    #[serde(default)]
    subgraph_classes: HashMap<String, Vec<String>>,
    #[serde(default)]
    class_defs: IndexMap<String, Vec<String>>,
    #[serde(default)]
    edge_defaults: Option<FlowEdgeDefaults>,
    #[serde(default)]
    tooltips: HashMap<String, String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(super) struct NodePresentation {
    classes: Vec<String>,
    styles: Vec<String>,
    link: Option<String>,
    link_target: Option<String>,
    have_callback: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct EdgePresentation {
    classes: Vec<String>,
    styles: Vec<String>,
    interpolate: Option<String>,
    animate: Option<bool>,
    animation: Option<String>,
}

impl AgentflowPresentation {
    pub(super) fn add_edge(&mut self, id: &str) {
        if let Some(interpolate) = self
            .edge_defaults
            .as_ref()
            .and_then(|defaults| defaults.interpolate.as_ref())
        {
            self.edges.entry(id.to_string()).or_default().interpolate = Some(interpolate.clone());
        }
    }

    pub(super) fn attach_edge_metadata(&mut self, id: &str, metadata: &Map<String, Value>) {
        let edge = self.edges.entry(id.to_string()).or_default();
        if let Some(value) = metadata.get("curve").and_then(Value::as_str) {
            edge.interpolate = Some(value.to_string());
        }
        if let Some(value) = metadata.get("animate").and_then(Value::as_bool) {
            edge.animate = Some(value);
        }
        if let Some(value) = metadata.get("animation").and_then(Value::as_str) {
            edge.animation = Some(value.to_string());
        }
    }

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
                node.classes.extend(presentation.classes.iter().cloned());
                node.styles = presentation.styles.clone();
                node.link = presentation.link.clone();
                node.link_target = presentation.link_target.clone();
                node.have_callback = presentation.have_callback;
            }
        }
        for edge in &mut model.edges {
            if let Some(presentation) = self.edges.get(&edge.id) {
                edge.classes = presentation.classes.clone();
                edge.style = presentation.styles.clone();
                edge.interpolate = presentation.interpolate.clone();
                edge.animate = presentation.animate;
                edge.animation = presentation.animation.clone();
            }
        }
        for graph in &mut model.subgraphs {
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

impl Parser<'_> {
    pub(super) fn assign_class(&mut self, ids: &str, class: &str) {
        for id in ids.split(',') {
            if let Some(node) = self.presentation.nodes.get_mut(id) {
                node.classes.push(class.to_string());
            }
            if self.edges.iter().any(|edge| edge.id.as_deref() == Some(id)) {
                self.presentation
                    .edges
                    .entry(id.to_string())
                    .or_default()
                    .classes
                    .push(class.to_string());
            }
            if self.sub_graph_index.contains_key(id) {
                self.presentation
                    .subgraph_classes
                    .entry(id.to_string())
                    .or_default()
                    .push(class.to_string());
            }
        }
    }

    pub(super) fn parse_presentation_statement(
        &mut self,
        source: &str,
        start: usize,
    ) -> Result<bool, ParseFailure> {
        let Some(result) = flowchart::lex_presentation_statement(source) else {
            return Ok(false);
        };
        let (_, token, end) = result.map_err(|error| {
            self.failure(
                error.message,
                error
                    .span
                    .map(|span| SourceSpan::new(start + span.start, start + span.end))
                    .unwrap_or(SourceSpan::new(start, start + source.len())),
            )
        })?;
        if !source[end..].trim().is_empty() {
            return Err(self.failure(
                "unexpected content after presentation directive",
                SourceSpan::new(start + end, start + source.len()),
            ));
        }
        match token {
            Tok::StyleStmt(style) => {
                if style.styles.is_empty() || !super::is_style_identifier(&style.target) {
                    return Err(
                        self.failure("expected style value", SourceSpan::new(start, start + end))
                    );
                }
                // style calls addVertex; unlike class it creates an unknown node, but its statement
                // contributes no members to the enclosing flow. Edge ids return before node mutation.
                if !self
                    .edges
                    .iter()
                    .any(|edge| edge.id.as_deref() == Some(&style.target))
                {
                    self.upsert_node(
                        Declaration {
                            id: style.target.clone(),
                            label: None,
                            syntax_shape: None,
                            metadata: Map::new(),
                            metadata_span: None,
                            authored_shape: None,
                            class: None,
                        },
                        start,
                        source,
                        true,
                    );
                    self.presentation
                        .nodes
                        .get_mut(&style.target)
                        .unwrap()
                        .styles
                        .extend(style.styles);
                }
            }
            Tok::ClassDefStmt(class) => {
                if class.styles.is_empty()
                    || class.ids.iter().any(|id| !super::is_style_identifier(id))
                {
                    return Err(self.failure(
                        "expected class style value",
                        SourceSpan::new(start, start + end),
                    ));
                }
                for id in class.ids {
                    self.control.checkpoint().map_err(ParseFailure::Cancelled)?;
                    self.presentation
                        .class_defs
                        .entry(id.clone())
                        .or_default()
                        .extend(class.styles.iter().cloned());
                    self.push_symbol(&id, EditorSemanticKind::Class, start, source, false);
                }
            }
            Tok::ClassAssignStmt(class) => {
                if !super::is_style_identifier(&class.class_name)
                    || class
                        .targets
                        .iter()
                        .any(|id| !super::is_style_identifier(id))
                {
                    return Err(self.failure(
                        "invalid class identifier",
                        SourceSpan::new(start, start + end),
                    ));
                }
                for target in class.targets {
                    self.control.checkpoint().map_err(ParseFailure::Cancelled)?;
                    self.assign_class(&target, &class.class_name);
                    self.push_symbol(&target, EditorSemanticKind::Object, start, source, true);
                }
            }
            Tok::ClickStmt(click) => {
                for id in click.ids.iter().flat_map(|ids| ids.split(',')) {
                    self.control.checkpoint().map_err(ParseFailure::Cancelled)?;
                    if let Some(tooltip) = &click.tooltip {
                        self.presentation.tooltips.insert(
                            id.to_string(),
                            crate::sanitize::sanitize_text(tooltip, &self.meta.effective_config),
                        );
                    }
                    self.assign_class(id, "clickable");
                    if let Some(node) = self.presentation.nodes.get_mut(id) {
                        match &click.action {
                            ClickAction::Link { href, target } => {
                                node.link =
                                    crate::utils::format_url(href, &self.meta.effective_config);
                                node.link_target = target.clone();
                            }
                            ClickAction::Callback => {
                                if self
                                    .meta
                                    .effective_config
                                    .as_value()
                                    .get("securityLevel")
                                    .and_then(Value::as_str)
                                    == Some("loose")
                                {
                                    node.have_callback = true;
                                }
                            }
                        }
                    }
                    self.push_symbol(id, EditorSemanticKind::Object, start, source, true);
                }
            }
            Tok::LinkStyleStmt(style) => {
                if style.styles.is_empty() && style.interpolate.is_none() {
                    return Err(self.failure(
                        "expected link style value",
                        SourceSpan::new(start, start + end),
                    ));
                }
                for position in style.positions {
                    self.control.checkpoint().map_err(ParseFailure::Cancelled)?;
                    match position {
                        LinkStylePos::Default => {
                            let defaults =
                                self.presentation.edge_defaults.get_or_insert_with(|| {
                                    FlowEdgeDefaults {
                                        interpolate: None,
                                        style: Vec::new(),
                                    }
                                });
                            if style.interpolate.is_some() {
                                defaults.interpolate = style.interpolate.clone();
                            }
                            if !style.styles.is_empty() {
                                defaults.style = style.styles.clone();
                            }
                        }
                        LinkStylePos::Index(index) => {
                            let Some(edge) = self.edges.get(index) else {
                                return Err(self.failure(format!("The index {index} for linkStyle is out of bounds. Valid indices for linkStyle are between 0 and {}. (Help: Ensure that the index is within the range of existing edges.)", self.edges.len() as i128 - 1), SourceSpan::new(start, start + end)));
                            };
                            let id = edge.id.as_ref().expect("push_edge assigns an id");
                            let edge = self.presentation.edges.entry(id.clone()).or_default();
                            if style.interpolate.is_some() {
                                edge.interpolate = style.interpolate.clone();
                            }
                            if !style.styles.is_empty() {
                                edge.styles = style.styles.clone();
                                if !edge.styles.iter().any(|style| style.starts_with("fill")) {
                                    edge.styles.push("fill:none".into());
                                }
                            }
                        }
                    }
                }
            }
            _ => unreachable!("presentation lexer returns only presentation tokens"),
        }
        Ok(true)
    }
}
