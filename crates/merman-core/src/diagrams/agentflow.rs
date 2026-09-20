//! Native semantic model for Mermaid's `agentflow-beta` diagram.
//!
//! Agentflow deliberately keeps its semantic projection separate from Flowchart.  The surface
//! syntax borrows Flowchart's node and edge spelling, while the meaning of shapes, containers,
//! metadata, and the three edge operators is domain specific.

use crate::{
    EditorFamilySemantics, EditorRenamePolicy, EditorSemanticFacts, EditorSemanticKind,
    EditorSemanticSymbol, Error, OperationControl, OperationControlResult, ParseMetadata, Result,
    SourceSpan, family,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::collections::{HashMap, HashSet};

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
}

/// Typed data consumed by the Agentflow renderer and projected to Mermaid-compatible JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentflowDiagramRenderModel {
    pub direction: String,
    #[serde(default)]
    pub diagnostics: Vec<AgentflowDiagnostic>,
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

impl AgentflowDiagramRenderModel {
    pub(crate) fn sanitize_common_db_fields(&mut self, config: &crate::MermaidConfig) {
        crate::common_db::sanitize_optional_title(&mut self.title, config);
        crate::common_db::sanitize_optional_acc_title(&mut self.acc_title, config);
        crate::common_db::sanitize_optional_acc_descr(&mut self.acc_descr, config);
    }

    /// Projects the domain graph into the shared flow layout model.
    ///
    /// Agentflow owns its semantic JSON and diagnostics, while Mermaid 12 uses the unified
    /// flow renderer for node geometry and ELK/Dagre routing. Keeping this adapter explicit
    /// prevents the compatibility model from silently acquiring Flowchart-only fields.
    pub fn to_flowchart_model(
        &self,
    ) -> Result<(
        crate::diagrams::flowchart::FlowchartModel,
        crate::diagrams::flowchart::FlowchartRenderContext,
    )> {
        let nodes = self
            .vertices
            .iter()
            .map(|node| {
                json!({
                    "id": node.id,
                    "label": node.label,
                    "layoutShape": normalized_render_shape(node.shape.as_deref()),
                    "shape": normalized_render_shape(node.shape.as_deref()),
                    "classes": [node.vertex_kind.css_class()],
                    "styles": []
                })
            })
            .chain(self.connectors.iter().map(|connector| {
                json!({
                    "id": connector.id,
                    "label": connector.title.as_deref().unwrap_or(&connector.id),
                    "layoutShape": "roundedRect",
                    "shape": "roundedRect",
                    "classes": ["af-kind-connector"], "styles": []
                })
            }))
            .collect::<Vec<_>>();
        let edges = self
            .edges
            .iter()
            .enumerate()
            .map(|(index, edge)| {
                let arrow = match edge.edge_semantic {
                    AgentflowEdgeSemantic::Sequence => "-->",
                    AgentflowEdgeSemantic::Reference => "-.-",
                    AgentflowEdgeSemantic::Failure => "--x",
                };
                json!({
                    "id": edge.id.clone().unwrap_or_else(|| format!("edge-{index}")),
                    "from": edge.start,
                    "to": edge.end,
                    "label": edge.label,
                    "type": edge.edge_type,
                    "arrow": arrow,
                    "stroke": edge.stroke,
                    "length": edge.length.max(1),
                    "classes": [],
                    "style": []
                })
            })
            .collect::<Vec<_>>();
        let subgraphs = self
            .sub_graphs
            .iter()
            .map(|graph| {
                json!({
                    "id": graph.id,
                    "title": graph.title.clone().unwrap_or_default(),
                    "dir": graph.direction,
                    "nodes": graph.nodes,
                    "classes": [],
                    "styles": [],
                    "metadata": graph.metadata
                })
            })
            .collect::<Vec<_>>();
        let mut model: crate::diagrams::flowchart::FlowchartModel = serde_json::from_value(json!({
            "keyword": "agentflow-beta",
            "direction": self.direction,
            "nodes": nodes,
            "edges": edges,
            "subgraphs": subgraphs,
            "classDefs": {},
            "vertexCalls": [],
            "tooltips": {}
        }))
        .map_err(|error| {
            Error::diagram_parse_fallback(
                "agentflow".to_string(),
                format!("agentflow flow layout projection failed: {error}"),
            )
        })?;
        let collapsed = self
            .sub_graphs
            .iter()
            .filter(|graph| graph.metadata.get("view").and_then(Value::as_str) == Some("collapsed"))
            .map(|graph| graph.id.clone())
            .collect();
        let context = crate::diagrams::flowchart::FlowchartRenderContext::new(
            Default::default(),
            Default::default(),
            collapsed,
            &model.subgraphs,
        );
        model.edges.retain_mut(|edge| {
            let authored_self_loop = edge.from == edge.to;
            if let Some(replacement) = context.collapsed_replacement(&edge.from) {
                edge.from = replacement.to_string();
            }
            if let Some(replacement) = context.collapsed_replacement(&edge.to) {
                edge.to = replacement.to_string();
            }
            // Agentflow retains authored loops even when their node is collapsed.
            authored_self_loop || edge.from != edge.to
        });
        Ok((model, context))
    }
}

#[derive(Debug)]
struct ParseFailure {
    message: String,
    span: SourceSpan,
}

struct Construction {
    model: AgentflowDiagramRenderModel,
    editor_facts: EditorSemanticFacts,
}

#[derive(Debug, Clone)]
struct Context {
    id: Option<String>,
    global: bool,
}

/// Parse the compatibility JSON path used by the public Mermaid facade.
pub(crate) fn parse_agentflow(code: &str, meta: &ParseMetadata) -> Result<Value> {
    let control = OperationControl::new();
    let construction = construct(code, meta, &control)
        .expect("a private operation control cannot be cancelled")
        .map_err(|failure| family::CombinedSemanticFailure::into_error(failure))?;
    render_model_to_compat_json(&construction.model, meta)
}

pub(crate) fn parse_agentflow_model_for_render_controlled(
    code: &str,
    meta: &ParseMetadata,
    control: &OperationControl,
) -> OperationControlResult<Result<AgentflowDiagramRenderModel>> {
    Ok(construct(code, meta, control)?
        .map(|construction| construction.model)
        .map_err(family::CombinedSemanticFailure::into_error))
}

pub(crate) fn parse_agentflow_json_and_editor_facts(
    code: &str,
    meta: &ParseMetadata,
    control: &OperationControl,
) -> OperationControlResult<family::CombinedSemanticParse> {
    let construction = construct(code, meta, control)?;
    Ok(family::CombinedSemanticParse::from_construction(
        construction,
        |construction| {
            (
                render_model_to_compat_json(&construction.model, meta),
                construction.editor_facts,
            )
        },
        family::CombinedSemanticFailure::into_parts,
    ))
}

pub(crate) fn render_model_to_compat_json(
    model: &AgentflowDiagramRenderModel,
    meta: &ParseMetadata,
) -> Result<Value> {
    let mut value = serde_json::to_value(model).map_err(|error| {
        Error::diagram_parse_fallback(meta.diagram_type.clone(), error.to_string())
    })?;
    if let Some(root) = value.as_object_mut() {
        root.insert("type".into(), Value::String(meta.diagram_type.clone()));
        for collection in ["vertices", "subGraphs", "connectors"] {
            if let Some(items) = root.get_mut(collection).and_then(Value::as_array_mut) {
                for item in items {
                    if let Some(item) = item.as_object_mut() {
                        item.remove("parentId");
                        if let Some(metadata) =
                            item.get_mut("metadata").and_then(Value::as_object_mut)
                        {
                            metadata.retain(|key, _| {
                                !matches!(
                                    key.as_str(),
                                    "shape"
                                        | "view"
                                        | "icon"
                                        | "img"
                                        | "form"
                                        | "pos"
                                        | "w"
                                        | "h"
                                        | "class"
                                        | "style"
                                        | "labelType"
                                )
                            });
                            if metadata.is_empty() {
                                item.remove("metadata");
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(value)
}

fn construct(
    code: &str,
    meta: &ParseMetadata,
    control: &OperationControl,
) -> OperationControlResult<std::result::Result<Construction, family::CombinedSemanticFailure>> {
    control.checkpoint()?;
    let mut parser = Parser::new(code, meta, control);
    let result = parser.parse()?;
    control.checkpoint()?;
    match result {
        Ok(model) => Ok(Ok(Construction {
            model,
            editor_facts: parser.facts,
        })),
        Err(failure) => {
            let mut facts = parser.facts;
            facts.mark_recovered_from_parse_error(failure.message.clone(), Some(failure.span));
            Ok(Err(family::CombinedSemanticFailure::new(
                Error::diagram_parse_exact(
                    meta.diagram_type.clone(),
                    failure.message,
                    failure.span,
                ),
                facts,
            )))
        }
    }
}

struct Parser<'a> {
    source: &'a str,
    meta: &'a ParseMetadata,
    control: &'a OperationControl,
    facts: EditorSemanticFacts,
    direction: String,
    nodes: Vec<AgentflowNode>,
    node_index: HashMap<String, usize>,
    edges: Vec<AgentflowEdge>,
    sub_graphs: Vec<AgentflowSubGraph>,
    sub_graph_index: HashMap<String, usize>,
    connectors: Vec<AgentflowConnector>,
    connector_index: HashMap<String, usize>,
    contexts: Vec<Context>,
    global_nodes: HashSet<String>,
    in_frontmatter: bool,
    acc_title: Option<String>,
    acc_descr: Option<String>,
}

impl<'a> Parser<'a> {
    fn new(source: &'a str, meta: &'a ParseMetadata, control: &'a OperationControl) -> Self {
        let mut facts = EditorSemanticFacts::new();
        facts.family_semantics = EditorFamilySemantics::new(EditorSemanticKind::Object);
        Self {
            source,
            meta,
            control,
            facts,
            direction: "TB".to_string(),
            nodes: Vec::new(),
            node_index: HashMap::new(),
            edges: Vec::new(),
            sub_graphs: Vec::new(),
            sub_graph_index: HashMap::new(),
            connectors: Vec::new(),
            connector_index: HashMap::new(),
            contexts: Vec::new(),
            global_nodes: HashSet::new(),
            in_frontmatter: false,
            acc_title: None,
            acc_descr: None,
        }
    }

    fn parse(
        &mut self,
    ) -> OperationControlResult<std::result::Result<AgentflowDiagramRenderModel, ParseFailure>>
    {
        let Some((header_start, header)) = first_content_line(self.source) else {
            return Ok(Err(self.failure(
                "expected agentflow-beta header",
                SourceSpan::new(0, self.source.len()),
            )));
        };
        let header_tokens: Vec<&str> = header.split_whitespace().collect();
        if header_tokens.first().copied() != Some("agentflow-beta") {
            return Ok(Err(self.failure(
                "expected agentflow-beta header",
                SourceSpan::new(header_start, header_start + header.len()),
            )));
        }
        if let Some(value) = header_tokens.get(1) {
            self.direction = match normalize_direction(value) {
                Some(direction) => direction,
                None => {
                    return Ok(Err(self.failure(
                        "invalid agentflow direction",
                        SourceSpan::new(header_start, header_start + header.len()),
                    )));
                }
            };
        }
        self.facts
            .push_expected_syntax(crate::EditorExpectedSyntax::new(
                crate::EditorExpectedSyntaxKind::NodeIdentifier,
                SourceSpan::new(header_start, header_start + header.len()),
            ));

        let mut offset = 0usize;
        let mut lines = self.source.split_inclusive('\n');
        while let Some(raw) = lines.next() {
            self.control.checkpoint()?;
            let line_start = offset;
            offset += raw.len();
            let initial_line = raw
                .strip_suffix('\n')
                .unwrap_or(raw)
                .strip_suffix('\r')
                .unwrap_or(raw);
            let mut logical_line = initial_line.to_string();
            while let Some(metadata_start) = find_metadata_start(&logical_line)
                && matching_brace(&logical_line, metadata_start).is_none()
            {
                let Some(next_raw) = lines.next() else {
                    break;
                };
                offset += next_raw.len();
                logical_line.push('\n');
                logical_line.push_str(next_raw.strip_suffix('\n').unwrap_or(next_raw));
            }
            let line = logical_line;
            let trimmed = strip_inline_comment(&line)
                .trim()
                .trim_start_matches('\u{feff}');
            if trimmed.is_empty() || trimmed.starts_with("%%") || trimmed == header {
                continue;
            }
            if trimmed == "---" {
                self.in_frontmatter = !self.in_frontmatter;
                continue;
            }
            if self.in_frontmatter || trimmed.starts_with("---") {
                continue;
            }
            if let Some(rest) = trimmed.strip_prefix("direction ") {
                let direction = match normalize_direction(rest.trim()) {
                    Some(direction) => direction,
                    None => {
                        return Ok(Err(self.failure(
                            "invalid agentflow direction",
                            span_of(line_start, &line, trimmed),
                        )));
                    }
                };
                if let Some(index) = self.contexts.iter().rev().find_map(|context| {
                    context
                        .id
                        .as_ref()
                        .and_then(|id| self.sub_graph_index.get(id).copied())
                }) {
                    self.sub_graphs[index].direction = Some(direction);
                } else {
                    self.direction = direction;
                }
                continue;
            }
            if let Some(rest) = trimmed.strip_prefix("accTitle:") {
                self.acc_title = Some(rest.trim().to_string());
                continue;
            }
            if let Some(rest) = trimmed.strip_prefix("accDescr:") {
                self.acc_descr = Some(rest.trim().to_string());
                continue;
            }
            if trimmed == "end" {
                if self.contexts.pop().is_none() {
                    return Ok(Err(self.failure(
                        "unexpected agentflow end",
                        span_of(line_start, &line, trimmed),
                    )));
                }
                continue;
            }
            if trimmed == "global" {
                self.contexts.push(Context {
                    id: None,
                    global: true,
                });
                continue;
            }
            if let Some(rest) = trimmed.strip_prefix("flow")
                && rest.chars().next().is_none_or(char::is_whitespace)
            {
                let (id, label, metadata) = if rest.trim().is_empty() {
                    (format!("flow-{}", self.sub_graphs.len()), None, Map::new())
                } else {
                    match parse_declaration(rest.trim(), line_start, &line) {
                        Ok(value) => value,
                        Err(error) => return Ok(Err(error)),
                    }
                };
                let index = self.upsert_subgraph(id.clone(), label, metadata);
                self.assign_subgraph_parent(&id);
                self.contexts.push(Context {
                    id: Some(id.clone()),
                    global: false,
                });
                self.push_symbol(&id, EditorSemanticKind::Namespace, line_start, &line, false);
                self.sub_graph_index.insert(id, index);
                continue;
            }
            if let Some(rest) = trimmed.strip_prefix("connector")
                && rest.chars().next().is_none_or(char::is_whitespace)
            {
                let (id, label, metadata) = match parse_declaration(rest.trim(), line_start, &line)
                {
                    Ok(value) => value,
                    Err(error) => return Ok(Err(error)),
                };
                let node_index = self.upsert_node(
                    id.clone(),
                    label.clone(),
                    metadata.clone(),
                    line_start,
                    &line,
                    false,
                );
                self.nodes[node_index].vertex_kind = AgentflowVertexKind::Connector;
                self.assign_parent(&id, node_index);
                let index = self.upsert_connector(id.clone(), label, metadata);
                self.push_symbol(&id, EditorSemanticKind::Object, line_start, &line, false);
                self.connector_index.insert(id, index);
                continue;
            }
            match self.parse_edge_statement(trimmed, line_start, &line) {
                Ok(true) => continue,
                Ok(false) => {}
                Err(error) => return Ok(Err(error)),
            }
            if let Err(error) = self.parse_node_statement(trimmed, line_start, &line) {
                return Ok(Err(error));
            }
        }
        if !self.contexts.is_empty() {
            return Ok(Err(self.failure(
                "unterminated agentflow container",
                SourceSpan::new(0, self.source.len()),
            )));
        }
        Ok(Ok(AgentflowDiagramRenderModel {
            direction: self.direction.clone(),
            diagnostics: self
                .nodes
                .iter()
                .filter(|node| !self.sub_graph_index.contains_key(&node.id))
                .filter_map(|node| shape_diagnostic(node))
                .collect(),
            vertices: self
                .nodes
                .iter()
                .filter(|node| {
                    !self.sub_graph_index.contains_key(&node.id)
                        && !self.connector_index.contains_key(&node.id)
                })
                .cloned()
                .collect(),
            edges: self.edges.clone(),
            sub_graphs: self.sub_graphs.clone(),
            connectors: self.connectors.clone(),
            title: self.meta.title.clone(),
            acc_title: self.acc_title.clone(),
            acc_descr: self.acc_descr.clone(),
        }))
    }

    fn parse_node_statement(
        &mut self,
        statement: &str,
        line_start: usize,
        line: &str,
    ) -> std::result::Result<(), ParseFailure> {
        let (id, label, metadata) = parse_declaration(statement, line_start, line)?;
        if id.is_empty() {
            return Err(self.failure(
                "expected agentflow node identifier",
                span_of(line_start, line, statement),
            ));
        }
        if label.is_none()
            && let Some(sub_graph_index) = self.sub_graph_index.get(&id).copied()
        {
            self.sub_graphs[sub_graph_index].metadata.extend(metadata);
            self.push_symbol(&id, EditorSemanticKind::Namespace, line_start, line, true);
            return Ok(());
        }
        if label.is_none()
            && let Some(connector_index) = self.connector_index.get(&id).copied()
        {
            self.connectors[connector_index].metadata.extend(metadata);
            self.push_symbol(&id, EditorSemanticKind::Object, line_start, line, true);
            return Ok(());
        }
        if label.is_none()
            && let Some(edge_id) = id.strip_suffix('@').or(Some(id.as_str()))
            && let Some(edge) = self
                .edges
                .iter_mut()
                .find(|edge| edge.id.as_deref() == Some(edge_id))
        {
            edge.metadata.extend(metadata);
            self.push_symbol(edge_id, EditorSemanticKind::Object, line_start, line, true);
            return Ok(());
        }
        let index = self.upsert_node(id.clone(), label, metadata, line_start, line, false);
        self.assign_parent(&id, index);
        Ok(())
    }

    fn parse_edge_statement(
        &mut self,
        statement: &str,
        line_start: usize,
        line: &str,
    ) -> std::result::Result<bool, ParseFailure> {
        let Some(first) = find_operator(statement, 0) else {
            return Ok(false);
        };
        let source = statement[..first.start].trim();
        if source.is_empty() {
            return Err(self.failure("expected edge source", span_of(line_start, line, statement)));
        }
        let (source, explicit_edge_id) = split_edge_id(source);
        loop {
            let (source_id, source_label, source_meta) =
                parse_declaration(source, line_start, line)?;
            let source_idx = self.upsert_node(
                source_id.clone(),
                source_label,
                source_meta,
                line_start,
                line,
                true,
            );
            self.assign_parent(&source_id, source_idx);
            let rhs_start = first.end;
            let Some(next) = find_operator(statement, rhs_start) else {
                let target = statement[rhs_start..].trim();
                if target.is_empty() {
                    return Err(
                        self.failure("expected edge target", span_of(line_start, line, statement))
                    );
                }
                let (target_id, target_label, target_meta) =
                    parse_declaration(target, line_start, line)?;
                let target_idx = self.upsert_node(
                    target_id.clone(),
                    target_label,
                    target_meta,
                    line_start,
                    line,
                    true,
                );
                self.assign_parent(&target_id, target_idx);
                let mut edge = edge_for(
                    &source_id,
                    &target_id,
                    first.semantic,
                    first.label.clone(),
                    first.length,
                );
                edge.id = explicit_edge_id.clone();
                self.push_edge(edge);
                self.push_symbol(
                    &target_id,
                    EditorSemanticKind::Object,
                    line_start,
                    line,
                    true,
                );
                return Ok(true);
            };
            let target = statement[rhs_start..next.start].trim();
            if target.is_empty() {
                return Err(
                    self.failure("expected edge target", span_of(line_start, line, statement))
                );
            }
            let (target_id, target_label, target_meta) =
                parse_declaration(target, line_start, line)?;
            let target_idx = self.upsert_node(
                target_id.clone(),
                target_label,
                target_meta,
                line_start,
                line,
                true,
            );
            self.assign_parent(&target_id, target_idx);
            let mut edge = edge_for(
                &source_id,
                &target_id,
                first.semantic,
                first.label.clone(),
                first.length,
            );
            edge.id = explicit_edge_id.clone();
            self.push_edge(edge);
            self.push_symbol(
                &target_id,
                EditorSemanticKind::Object,
                line_start,
                line,
                true,
            );
            return self.parse_edge_chain_tail(statement, next, line_start, line, target_id);
        }
    }

    fn parse_edge_chain_tail(
        &mut self,
        statement: &str,
        op: Operator,
        line_start: usize,
        line: &str,
        mut source_id: String,
    ) -> std::result::Result<bool, ParseFailure> {
        let mut current = op;
        loop {
            let next = find_operator(statement, current.end);
            let target_end = next
                .as_ref()
                .map_or(statement.len(), |operator| operator.start);
            let raw_target = statement[current.end..target_end].trim();
            if raw_target.is_empty() {
                return Err(
                    self.failure("expected edge target", span_of(line_start, line, statement))
                );
            }
            let (id, label, metadata) = parse_declaration(raw_target, line_start, line)?;
            let index = self.upsert_node(id.clone(), label, metadata, line_start, line, true);
            self.assign_parent(&id, index);
            self.push_edge(edge_for(
                &source_id,
                &id,
                current.semantic,
                current.label.clone(),
                current.length,
            ));
            if let Some(next) = next {
                source_id = id;
                current = next;
                continue;
            }
            return Ok(true);
        }
    }

    fn push_edge(&mut self, mut edge: AgentflowEdge) {
        if edge.id.as_ref().is_none_or(|id| {
            self.edges
                .iter()
                .any(|existing| existing.id.as_ref() == Some(id))
        }) {
            let count = self
                .edges
                .iter()
                .filter(|existing| existing.start == edge.start && existing.end == edge.end)
                .count();
            let counter = if count == 0 { 0 } else { count + 1 };
            edge.id = Some(format!("L_{}_{}_{counter}", edge.start, edge.end));
        }
        self.edges.push(edge);
    }

    fn upsert_node(
        &mut self,
        id: String,
        label: Option<String>,
        metadata: Map<String, Value>,
        line_start: usize,
        line: &str,
        reference: bool,
    ) -> usize {
        if self.contexts.last().is_some_and(|context| context.global) {
            self.global_nodes.insert(id.clone());
            for graph in &mut self.sub_graphs {
                graph.nodes.retain(|child| child != &id);
            }
            if let Some(index) = self.node_index.get(&id).copied() {
                self.nodes[index].parent_id = None;
            }
        }
        let label = metadata
            .get("label")
            .and_then(Value::as_str)
            .map(str::to_string)
            .or(label);
        let shape = metadata
            .get("shape")
            .and_then(Value::as_str)
            .map(resolve_shape);
        let kind = vertex_kind(shape.as_deref());
        let index = if let Some(index) = self.node_index.get(&id).copied() {
            let node = &mut self.nodes[index];
            if label.is_some() {
                node.label = label;
            }
            if shape.is_some() {
                node.shape = shape;
                node.vertex_kind = kind;
            }
            node.metadata.extend(metadata);
            index
        } else {
            let index = self.nodes.len();
            self.nodes.push(AgentflowNode {
                id: id.clone(),
                label: Some(label.unwrap_or_else(|| id.clone())),
                shape,
                vertex_kind: kind,
                metadata,
                parent_id: None,
            });
            self.node_index.insert(id.clone(), index);
            index
        };
        self.push_symbol(&id, EditorSemanticKind::Object, line_start, line, reference);
        index
    }

    fn assign_parent(&mut self, id: &str, index: usize) {
        if self.global_nodes.contains(id) {
            return;
        }
        let Some(context) = self.contexts.iter().rev().find(|ctx| !ctx.global) else {
            return;
        };
        if context.id.as_deref() == Some(id) {
            return;
        }
        let belongs_here = self.nodes[index].parent_id.is_none();
        if belongs_here {
            self.nodes[index].parent_id = context.id.clone();
        }
        if belongs_here
            && let Some(parent_id) = &context.id
            && let Some(sub_index) = self.sub_graph_index.get(parent_id).copied()
            && !self.sub_graphs[sub_index]
                .nodes
                .iter()
                .any(|node| node == id)
        {
            self.sub_graphs[sub_index].nodes.push(id.to_string());
        }
    }

    fn assign_subgraph_parent(&mut self, id: &str) {
        let Some(context) = self.contexts.iter().rev().find(|ctx| !ctx.global) else {
            return;
        };
        let Some(parent_id) = &context.id else {
            return;
        };
        let Some(parent_index) = self.sub_graph_index.get(parent_id).copied() else {
            return;
        };
        if !self.sub_graphs[parent_index]
            .nodes
            .iter()
            .any(|node| node == id)
        {
            self.sub_graphs[parent_index].nodes.push(id.to_string());
        }
    }

    fn upsert_subgraph(
        &mut self,
        id: String,
        title: Option<String>,
        metadata: Map<String, Value>,
    ) -> usize {
        if let Some(index) = self.sub_graph_index.get(&id).copied() {
            let graph = &mut self.sub_graphs[index];
            if title.is_some() {
                graph.title = title;
            }
            graph.metadata.extend(metadata);
            return index;
        }
        let index = self.sub_graphs.len();
        self.sub_graphs.push(AgentflowSubGraph {
            id: id.clone(),
            title,
            nodes: Vec::new(),
            sub_graph_type: "flow".to_string(),
            direction: None,
            metadata,
        });
        self.sub_graph_index.insert(id, index);
        index
    }

    fn upsert_connector(
        &mut self,
        id: String,
        title: Option<String>,
        metadata: Map<String, Value>,
    ) -> usize {
        if let Some(index) = self.connector_index.get(&id).copied() {
            let connector = &mut self.connectors[index];
            if title.is_some() {
                connector.title = title;
            }
            connector.metadata.extend(metadata);
            return index;
        }
        let index = self.connectors.len();
        self.connectors.push(AgentflowConnector {
            id: id.clone(),
            title,
            metadata,
        });
        self.connector_index.insert(id, index);
        index
    }

    fn push_symbol(
        &mut self,
        id: &str,
        kind: EditorSemanticKind,
        line_start: usize,
        line: &str,
        reference: bool,
    ) {
        let Some(relative) = line.find(id) else {
            return;
        };
        let span = SourceSpan::new(line_start + relative, line_start + relative + id.len());
        let symbol = if reference {
            EditorSemanticSymbol::reference(id, None, kind, span, span)
        } else {
            EditorSemanticSymbol::new(id, None, kind, span, span)
        };
        self.facts
            .push_symbol(symbol.with_rename_policy(EditorRenamePolicy::FlowchartNodeId));
    }

    fn failure(&self, message: impl Into<String>, span: SourceSpan) -> ParseFailure {
        ParseFailure {
            message: message.into(),
            span,
        }
    }
}

#[derive(Debug, Clone)]
struct Operator {
    start: usize,
    end: usize,
    semantic: AgentflowEdgeSemantic,
    length: usize,
    label: Option<String>,
}

fn edge_for(
    start: &str,
    end: &str,
    semantic: AgentflowEdgeSemantic,
    label: Option<String>,
    length: usize,
) -> AgentflowEdge {
    let (edge_type, stroke) = match semantic {
        AgentflowEdgeSemantic::Sequence => ("arrow_point", "normal"),
        AgentflowEdgeSemantic::Reference => ("arrow_open", "dotted"),
        AgentflowEdgeSemantic::Failure => ("arrow_cross", "normal"),
    };
    AgentflowEdge {
        start: start.to_string(),
        end: end.to_string(),
        id: None,
        label,
        edge_semantic: semantic,
        edge_type: edge_type.to_string(),
        stroke: stroke.to_string(),
        length: length.max(1),
        metadata: Map::new(),
    }
}

fn normalize_direction(value: &str) -> Option<String> {
    let value = value.trim().trim_end_matches(';').to_ascii_uppercase();
    matches!(value.as_str(), "TB" | "TD" | "BT" | "LR" | "RL").then_some(value)
}

fn resolve_shape(value: &str) -> String {
    match value {
        "task" => "roundedRect".to_string(),
        "tool" => "subroutine".to_string(),
        "input" => "lean-right".to_string(),
        "refdoc" => "lin-doc".to_string(),
        "action" => "hexagon".to_string(),
        "round" => "rect".to_string(),
        "decision" => "diamond".to_string(),
        value => value.to_string(),
    }
}

const REMOVED_SHAPES: &[&str] = &[
    "doc",
    "stadium",
    "terminal",
    "circle",
    "trapezoid",
    "inv_trapezoid",
    "inv-trapezoid",
    "doublecircle",
    "double-circle",
    "typeDeclaration",
    "procs",
    "lean_left",
    "lean-left",
    "in-out",
    "cylinder",
    "ellipse",
    "odd",
    "tag-rect",
    "tagged-rectangle",
    "delay",
    "half-rounded-rectangle",
    "lin-rect",
    "lined-rectangle",
    "win-pane",
    "window-pane",
    "curv-trap",
    "curved-trapezoid",
];
const ALLOWED_SHAPES: &[&str] = &[
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

fn normalized_render_shape(shape: Option<&str>) -> &str {
    match shape {
        Some(shape) if ALLOWED_SHAPES.contains(&shape) => shape,
        _ => "roundedRect",
    }
}

fn shape_diagnostic(node: &AgentflowNode) -> Option<AgentflowDiagnostic> {
    let shape = node.shape.as_deref()?;
    if matches!(shape, "square" | "squareRect" | "rect") || ALLOWED_SHAPES.contains(&shape) {
        return None;
    }
    let (id, severity, reason) = if REMOVED_SHAPES.contains(&shape) {
        ("SHAPE_REMOVED", "error", "was removed in v0.8.1")
    } else {
        ("SHAPE_UNSUPPORTED", "warning", "is not supported")
    };
    Some(AgentflowDiagnostic {
        id: id.into(),
        severity: severity.into(),
        node_id: node.id.clone(),
        message: format!("shape \"{shape}\" {reason}, using \"roundedRect\""),
    })
}

fn vertex_kind(shape: Option<&str>) -> AgentflowVertexKind {
    match shape.unwrap_or("roundedRect") {
        "subroutine" | "subprocess" | "subproc" | "framed-rectangle" => AgentflowVertexKind::Tool,
        "hexagon" | "hex" => AgentflowVertexKind::Action,
        "lean-right" | "lean_right" => AgentflowVertexKind::Input,
        "lin-doc" | "lined-document" => AgentflowVertexKind::Refdoc,
        "diamond" => AgentflowVertexKind::Decision,
        _ => AgentflowVertexKind::Task,
    }
}

fn first_content_line(source: &str) -> Option<(usize, &str)> {
    let mut offset = 0;
    let mut frontmatter = false;
    for raw in source.split_inclusive('\n') {
        let line = raw
            .strip_suffix('\n')
            .unwrap_or(raw)
            .strip_suffix('\r')
            .unwrap_or(raw);
        let trimmed = line.trim().trim_start_matches('\u{feff}');
        if trimmed == "---" {
            frontmatter = !frontmatter;
            offset += raw.len();
            continue;
        }
        if !frontmatter && trimmed.starts_with("agentflow-beta") {
            return Some((offset + line.find(trimmed).unwrap_or(0), trimmed));
        }
        offset += raw.len();
    }
    None
}

fn span_of(line_start: usize, line: &str, value: &str) -> SourceSpan {
    let start = line.find(value).unwrap_or(0);
    SourceSpan::new(line_start + start, line_start + start + value.len())
}

fn strip_inline_comment(value: &str) -> &str {
    let bytes = value.as_bytes();
    let mut quote = None;
    let mut depth = 0usize;
    let mut index = 0usize;
    while index + 1 < bytes.len() {
        let ch = bytes[index] as char;
        if let Some(q) = quote {
            if ch == q && (index == 0 || bytes[index - 1] != b'\\') {
                quote = None;
            }
            index += 1;
            continue;
        }
        if ch == '"' || ch == '\'' {
            quote = Some(ch);
            index += 1;
            continue;
        }
        if matches!(ch, '[' | '(' | '{') {
            depth += 1;
        } else if matches!(ch, ']' | ')' | '}') {
            depth = depth.saturating_sub(1);
        } else if depth == 0 && bytes[index] == b'%' && bytes[index + 1] == b'%' {
            return &value[..index];
        }
        index += 1;
    }
    value
}

fn find_operator(source: &str, from: usize) -> Option<Operator> {
    let bytes = source.as_bytes();
    let mut depth = 0usize;
    let mut quote = None;
    let mut index = from;
    while index < bytes.len() {
        let ch = bytes[index] as char;
        if let Some(q) = quote {
            if ch == q && (index == 0 || bytes[index - 1] != b'\\') {
                quote = None;
            }
            index += 1;
            continue;
        }
        if ch == '"' || ch == '\'' {
            quote = Some(ch);
            index += 1;
            continue;
        }
        if matches!(ch, '[' | '(' | '{') {
            depth += 1;
            index += 1;
            continue;
        }
        if matches!(ch, ']' | ')' | '}') {
            depth = depth.saturating_sub(1);
            index += 1;
            continue;
        }
        if depth == 0 && bytes[index] == b'-' {
            if source[index..].starts_with("-.-") {
                let mut end = index + 3;
                let mut label = None;
                if source[end..].starts_with('|')
                    && let Some(close) = source[end + 1..].find('|')
                {
                    let close = end + 1 + close;
                    label = Some(unquote(source[end + 1..close].trim()));
                    end = close + 1;
                }
                return Some(Operator {
                    start: index,
                    end,
                    semantic: AgentflowEdgeSemantic::Reference,
                    length: 1,
                    label,
                });
            }
            if source[index..].starts_with("-->") || source[index..].starts_with("--x") {
                let semantic = if source[index..].starts_with("--x") {
                    AgentflowEdgeSemantic::Failure
                } else {
                    AgentflowEdgeSemantic::Sequence
                };
                let mut end = index + 3;
                let mut label = None;
                if source[end..].starts_with('|')
                    && let Some(close) = source[end + 1..].find('|')
                {
                    let close = end + 1 + close;
                    label = Some(unquote(source[end + 1..close].trim()));
                    end = close + 1;
                }
                return Some(Operator {
                    start: index,
                    end,
                    semantic,
                    length: 1,
                    label,
                });
            }
            if source[index..].starts_with("--") {
                let label_start = index + 2;
                if let Some((arrow_start, arrow_end, semantic)) =
                    find_labeled_arrow(source, label_start)
                {
                    let label = source[label_start..arrow_start].trim();
                    return Some(Operator {
                        start: index,
                        end: arrow_end,
                        semantic,
                        length: 1,
                        label: (!label.is_empty()).then(|| unquote(label)),
                    });
                }
            }
        }
        index += 1;
    }
    None
}

fn find_labeled_arrow(source: &str, from: usize) -> Option<(usize, usize, AgentflowEdgeSemantic)> {
    for (offset, ch) in source[from..].char_indices() {
        if ch != '-' {
            continue;
        }
        let index = from + offset;
        if source[index..].starts_with("-->") || source[index..].starts_with("--x") {
            let semantic = if source[index..].starts_with("--x") {
                AgentflowEdgeSemantic::Failure
            } else {
                AgentflowEdgeSemantic::Sequence
            };
            return Some((index, index + 3, semantic));
        }
    }
    None
}

fn parse_declaration(
    statement: &str,
    line_start: usize,
    line: &str,
) -> std::result::Result<(String, Option<String>, Map<String, Value>), ParseFailure> {
    let statement = statement.trim().trim_end_matches(';').trim();
    if statement.is_empty() {
        return Err(ParseFailure {
            message: "expected declaration".to_string(),
            span: span_of(line_start, line, statement),
        });
    }
    let metadata_start = find_metadata_start(statement);
    let (head, metadata) = if let Some(start) = metadata_start {
        let end = matching_brace(statement, start).ok_or_else(|| ParseFailure {
            message: "unterminated agentflow metadata".to_string(),
            span: span_of(line_start, line, statement),
        })?;
        (
            &statement[..start],
            parse_metadata(&statement[start + 2..end - 1]).map_err(|message| ParseFailure {
                message,
                span: span_of(line_start, line, &statement[start..end]),
            })?,
        )
    } else {
        (statement, Map::new())
    };
    let head = head.trim();
    let id_end = head
        .find(['[', '(', '{', '<', '>', '|'])
        .unwrap_or(head.len());
    let id = head[..id_end]
        .split_whitespace()
        .next()
        .unwrap_or("")
        .trim()
        .to_string();
    if id.is_empty() {
        return Err(ParseFailure {
            message: "expected agentflow node identifier".to_string(),
            span: span_of(line_start, line, head),
        });
    }
    let remainder = head[id_end..].trim();
    let mut metadata = metadata;
    let (label, shape) = parse_label(remainder);
    if head[..id_end].trim() != id || (!remainder.is_empty() && shape.is_none()) {
        return Err(ParseFailure {
            message: "invalid agentflow declaration or unsupported edge operator".into(),
            span: span_of(line_start, line, statement),
        });
    }
    if let Some(shape) = shape {
        metadata
            .entry("shape")
            .or_insert_with(|| Value::String(shape.into()));
    }
    Ok((id, label, metadata))
}

fn parse_label(remainder: &str) -> (Option<String>, Option<&'static str>) {
    // Match longer delimiters first; their meaning is defined by agentflow.jison.
    for (open, close, shape) in [
        ("(((", ")))", "doublecircle"),
        ("[[", "]]", "subroutine"),
        ("{{", "}}", "hexagon"),
        ("((", "))", "circle"),
        ("([", "])", "stadium"),
        ("[(", ")]", "cylinder"),
        ("[/", "/]", "lean_right"),
        ("[\\", "\\]", "lean_left"),
        ("[/", "\\]", "trapezoid"),
        ("[\\", "/]", "inv_trapezoid"),
        ("[", "]", "square"),
        ("(", ")", "round"),
        ("{", "}", "diamond"),
        (">", "]", "odd"),
    ] {
        if let Some(body) = remainder
            .strip_prefix(open)
            .and_then(|body| body.strip_suffix(close))
        {
            return (Some(unquote(body.trim())), Some(shape));
        }
    }
    (None, None)
}

fn split_edge_id(source: &str) -> (&str, Option<String>) {
    let Some(separator) = source.rfind(char::is_whitespace) else {
        return (source, None);
    };
    let (prefix, candidate) = source.split_at(separator);
    let candidate = candidate.trim_start();
    let Some(edge_id) = candidate.strip_suffix('@') else {
        return (source, None);
    };
    if edge_id.is_empty() || !is_identifier(edge_id) {
        return (source, None);
    }
    (prefix.trim_end(), Some(edge_id.to_string()))
}

fn is_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    chars
        .next()
        .is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_')
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
}

fn find_metadata_start(value: &str) -> Option<usize> {
    let mut depth = 0usize;
    let mut quote = None;
    for (index, ch) in value.char_indices() {
        if let Some(q) = quote {
            if ch == q {
                quote = None;
            }
            continue;
        }
        if ch == '"' || ch == '\'' {
            quote = Some(ch);
            continue;
        }
        if ch == '[' || ch == '(' {
            depth += 1;
        }
        if ch == ']' || ch == ')' {
            depth = depth.saturating_sub(1);
        }
        if ch == '@' && value[index..].starts_with("@{") && depth == 0 {
            return Some(index);
        }
    }
    None
}

fn matching_brace(value: &str, start: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut quote = None;
    for (index, ch) in value[start..].char_indices() {
        let index = start + index;
        if let Some(q) = quote {
            if ch == q {
                quote = None;
            }
            continue;
        }
        if ch == '"' || ch == '\'' {
            quote = Some(ch);
            continue;
        }
        if ch == '{' {
            depth += 1;
        }
        if ch == '}' {
            depth = depth.checked_sub(1)?;
            if depth == 0 {
                return Some(index + 1);
            }
        }
    }
    None
}

fn parse_metadata(body: &str) -> std::result::Result<Map<String, Value>, String> {
    let mut result = Map::new();
    let normalized = normalize_block_scalars(body);
    for part in split_metadata(&normalized) {
        let Some((key, value)) = split_once_unquoted(part, ':') else {
            return Err(format!("invalid agentflow metadata entry: {part}"));
        };
        let key = unquote(key.trim());
        if key.is_empty() {
            return Err("agentflow metadata key cannot be empty".to_string());
        }
        result.insert(
            key,
            strip_prototype_keys(parse_metadata_value(value.trim())),
        );
    }
    Ok(result)
}

fn normalize_block_scalars(body: &str) -> String {
    let lines: Vec<&str> = body.lines().collect();
    let mut output = Vec::with_capacity(lines.len());
    let mut index = 0usize;
    while index < lines.len() {
        let line = lines[index];
        let Some((key, value)) = split_once_unquoted(line.trim(), ':') else {
            output.push(line.to_string());
            index += 1;
            continue;
        };
        let marker = value.trim();
        if marker != "|" && marker != ">" {
            output.push(line.to_string());
            index += 1;
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        let mut content = Vec::new();
        index += 1;
        while index < lines.len() {
            let candidate = lines[index];
            let candidate_trimmed = candidate.trim();
            let candidate_indent = candidate.len() - candidate.trim_start().len();
            if !candidate_trimmed.is_empty()
                && candidate_indent <= indent
                && split_once_unquoted(candidate_trimmed, ':').is_some()
            {
                break;
            }
            content.push(candidate.trim());
            index += 1;
        }
        let joined = if marker == "|" {
            format!("{}\n", content.join("\n"))
        } else {
            format!("{}\n", content.join(" "))
        };
        let escaped = joined
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n");
        output.push(format!("{}: \"{}\"", key.trim(), escaped));
    }
    output.join("\n")
}

fn strip_prototype_keys(value: Value) -> Value {
    match value {
        Value::Array(values) => {
            Value::Array(values.into_iter().map(strip_prototype_keys).collect())
        }
        Value::Object(object) => Value::Object(
            object
                .into_iter()
                .filter(|(key, _)| {
                    !matches!(key.as_str(), "__proto__" | "constructor" | "prototype")
                })
                .map(|(key, value)| (key, strip_prototype_keys(value)))
                .collect(),
        ),
        other => other,
    }
}

fn split_metadata(value: &str) -> Vec<&str> {
    let mut result = Vec::new();
    let mut start = 0;
    let mut depth = 0usize;
    let mut quote = None;
    for (index, ch) in value.char_indices() {
        if let Some(q) = quote {
            if ch == q {
                quote = None;
            }
            continue;
        }
        if ch == '"' || ch == '\'' {
            quote = Some(ch);
            continue;
        }
        if matches!(ch, '[' | '{' | '(') {
            depth += 1;
        }
        if matches!(ch, ']' | '}' | ')') {
            depth = depth.saturating_sub(1);
        }
        if (ch == ',' || ch == '\n') && depth == 0 {
            if !value[start..index].trim().is_empty() {
                result.push(value[start..index].trim());
            }
            start = index + ch.len_utf8();
        }
    }
    if !value[start..].trim().is_empty() {
        result.push(value[start..].trim());
    }
    result
}

fn split_once_unquoted(value: &str, separator: char) -> Option<(&str, &str)> {
    let mut quote = None;
    let mut depth = 0usize;
    for (index, ch) in value.char_indices() {
        if let Some(q) = quote {
            if ch == q {
                quote = None;
            }
            continue;
        }
        if ch == '"' || ch == '\'' {
            quote = Some(ch);
            continue;
        }
        if matches!(ch, '[' | '{' | '(') {
            depth += 1;
        }
        if matches!(ch, ']' | '}' | ')') {
            depth = depth.saturating_sub(1);
        }
        if ch == separator && depth == 0 {
            return Some((&value[..index], &value[index + separator.len_utf8()..]));
        }
    }
    None
}

fn parse_metadata_value(value: &str) -> Value {
    if value.eq_ignore_ascii_case("true") {
        return Value::Bool(true);
    }
    if value.eq_ignore_ascii_case("false") {
        return Value::Bool(false);
    }
    if value.eq_ignore_ascii_case("null") {
        return Value::Null;
    }
    if let Ok(number) = value.parse::<i64>() {
        return Value::Number(number.into());
    }
    if let Ok(number) = value.parse::<f64>() {
        if let Some(number) = serde_json::Number::from_f64(number) {
            return Value::Number(number);
        }
    }
    if (value.starts_with('[') && value.ends_with(']'))
        || (value.starts_with('{') && value.ends_with('}'))
    {
        if let Ok(parsed) = json5::from_str::<Value>(value) {
            return parsed;
        }
    }
    Value::String(unquote(value))
}

fn unquote(value: &str) -> String {
    let value = value.trim();
    if value.len() >= 2
        && ((value.starts_with('"') && value.ends_with('"'))
            || (value.starts_with('\'') && value.ends_with('\'')))
    {
        let body = &value[1..value.len() - 1];
        return body
            .replace("\\\"", "\"")
            .replace("\\'", "'")
            .replace("\\n", "\n");
    }
    value.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MermaidConfig, OperationControl};

    fn meta() -> ParseMetadata {
        ParseMetadata {
            diagram_type: "agentflow".to_string(),
            config: MermaidConfig::empty_object(),
            effective_config: MermaidConfig::empty_object(),
            title: None,
        }
    }

    #[test]
    fn parses_domain_shapes_edges_and_nested_flow() {
        let source = "agentflow-beta LR\nflow agent[\"Agent\"]\n  input[\"Question\"]@{ shape: input, value: \"hello\" }\n  tool[\"Search\"]@{ shape: tool, params: \"q :: String\" }\n  input --> tool\nend\n";
        let model =
            parse_agentflow_model_for_render_controlled(source, &meta(), &OperationControl::new())
                .unwrap()
                .unwrap();
        assert_eq!(model.direction, "LR");
        assert_eq!(model.vertices[0].vertex_kind, AgentflowVertexKind::Input);
        assert_eq!(model.vertices[1].vertex_kind, AgentflowVertexKind::Tool);
        assert_eq!(
            model.edges[0].edge_semantic,
            AgentflowEdgeSemantic::Sequence
        );
        assert_eq!(model.sub_graphs[0].nodes, vec!["input", "tool"]);
        assert_eq!(model.vertices[0].metadata["value"], "hello");
    }

    #[test]
    fn keeps_unknown_metadata_and_assigns_edge_semantics() {
        let source = "agentflow-beta TB\na[\"A\"]@{ shape: decision, custom: 4 }\nb[\"B\"]@{shape: action}\na -- yes --> b\nb -.- a\na --x b\n";
        let model =
            parse_agentflow_model_for_render_controlled(source, &meta(), &OperationControl::new())
                .unwrap()
                .unwrap();
        assert_eq!(model.vertices[0].vertex_kind, AgentflowVertexKind::Decision);
        assert_eq!(model.vertices[0].metadata["custom"], 4);
        assert_eq!(
            model
                .edges
                .iter()
                .map(|edge| edge.edge_semantic)
                .collect::<Vec<_>>(),
            vec![
                AgentflowEdgeSemantic::Sequence,
                AgentflowEdgeSemantic::Reference,
                AgentflowEdgeSemantic::Failure
            ]
        );
        assert_eq!(model.edges[0].label.as_deref(), Some("yes"));
    }

    #[test]
    fn parses_multiline_metadata_and_container_attachment() {
        let source = "agentflow-beta TB\nflow worker[\"Worker\"]\n  task[\"Task\"]@{\n    shape: task\n    instruction: \"do the work\"\n  }\nend\nworker@{ view: collapsed, algorithm: \"elk.layered\" }\n";
        let model =
            parse_agentflow_model_for_render_controlled(source, &meta(), &OperationControl::new())
                .unwrap()
                .unwrap();
        assert_eq!(model.vertices[0].metadata["instruction"], "do the work");
        assert_eq!(model.sub_graphs[0].metadata["view"], "collapsed");
        assert_eq!(model.sub_graphs[0].metadata["algorithm"], "elk.layered");
    }

    #[test]
    fn preserves_global_nodes_nested_containment_and_pipe_labels() {
        let source = "agentflow-beta TB\nglobal\n  shared[\"Shared\"]@{ shape: refdoc }\nend\nflow outer[\"Outer\"]\n  flow inner[\"Inner\"]\n    task[\"Task\"]\n  end\n  task -->|\"done\"| shared\nend\n";
        let model =
            parse_agentflow_model_for_render_controlled(source, &meta(), &OperationControl::new())
                .unwrap()
                .unwrap();
        assert_eq!(model.vertices[0].id, "shared");
        assert_eq!(model.vertices[0].parent_id, None);
        assert_eq!(model.sub_graphs[0].nodes, vec!["inner"]);
        assert_eq!(model.sub_graphs[1].nodes, vec!["task"]);
        assert_eq!(model.edges[0].label.as_deref(), Some("done"));
    }

    #[test]
    fn preserves_explicit_edge_ids_and_edge_metadata_attachments() {
        let source = "agentflow-beta TB\na e1@--> b\ne1@{ instruction: \"hand off\", weight: 5 }\n";
        let model =
            parse_agentflow_model_for_render_controlled(source, &meta(), &OperationControl::new())
                .unwrap()
                .unwrap();
        assert_eq!(model.edges[0].id.as_deref(), Some("e1"));
        assert_eq!(model.edges[0].metadata["instruction"], "hand off");
        assert_eq!(model.edges[0].metadata["weight"], 5);
        assert_eq!(model.vertices.len(), 2);
    }

    #[test]
    fn preserves_literal_metadata_block_scalars() {
        let source =
            "agentflow-beta TB\na@{\n  instruction: |\n    do a thing,\n    then stop\n}\n";
        let model =
            parse_agentflow_model_for_render_controlled(source, &meta(), &OperationControl::new())
                .unwrap()
                .unwrap();
        assert_eq!(
            model.vertices[0].metadata["instruction"],
            "do a thing,\nthen stop\n"
        );
    }

    #[test]
    fn accepts_bom_and_crlf_source() {
        let source = "\u{feff}agentflow-beta TB\r\na[\"A\"] --> b[\"B\"]\r\n";
        let model =
            parse_agentflow_model_for_render_controlled(source, &meta(), &OperationControl::new())
                .unwrap()
                .unwrap();
        assert_eq!(model.vertices.len(), 2);
        assert_eq!(model.edges.len(), 1);
    }
    #[test]
    fn unicode_edges_and_shorthand_shapes_preserve_domain_meaning() {
        let model = parse_agentflow_model_for_render_controlled(
            "agentflow-beta\n判断{是否继续} -- 成功 --> 工具[[搜索]]\n工具 --x 失败\n",
            &meta(),
            &OperationControl::new(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(model.vertices[0].vertex_kind, AgentflowVertexKind::Decision);
        assert_eq!(model.vertices[1].vertex_kind, AgentflowVertexKind::Tool);
        assert_eq!(model.vertices[1].label.as_deref(), Some("搜索"));
        assert_eq!(model.edges[0].label.as_deref(), Some("成功"));
        assert!(model.edges.iter().all(|edge| edge.length == 1));
    }

    #[test]
    fn semantic_projection_strips_presentation_but_keeps_domain_metadata() {
        let json = parse_agentflow("agentflow-beta\nflow f@{ view: collapsed, instruction: run }\n a@{ shape: decision, icon: test, instruction: decide }\nend\na --> b\n", &meta()).unwrap();
        assert_eq!(json["vertices"][0]["shape"], "diamond");
        assert_eq!(
            json["vertices"][0]["metadata"],
            json!({"instruction": "decide"})
        );
        assert!(json["vertices"][0].get("parentId").is_none());
        assert_eq!(json["subGraphs"][0]["type"], "flow");
        assert_eq!(
            json["subGraphs"][0]["metadata"],
            json!({"instruction": "run"})
        );
        assert_eq!(json["edges"][0]["type"], "arrow_point");
    }

    #[test]
    fn connector_and_collapsed_flow_remain_renderable_without_semantic_duplication() {
        let model = parse_agentflow_model_for_render_controlled(
            "agentflow-beta\nconnector api[API]\nflow outer@{ view: collapsed }\n flow inner\n  a --> b\n end\nend\nb --> api\n",
            &meta(), &OperationControl::new()).unwrap().unwrap();
        assert!(model.vertices.iter().all(|node| node.id != "api"));
        assert_eq!(model.connectors.len(), 1);
        let (flow, context) = model.to_flowchart_model().unwrap();
        let connector = flow.nodes.iter().find(|node| node.id == "api").unwrap();
        assert_eq!(connector.label.as_deref(), Some("API"));
        assert!(context.is_subgraph_collapsed("outer"));
        assert_eq!(context.collapsed_replacement("a"), Some("outer"));
        assert_eq!(context.collapsed_replacement("inner"), Some("outer"));
    }

    #[test]
    fn global_declaration_after_flow_releases_existing_membership() {
        let model = parse_agentflow_model_for_render_controlled(
            "agentflow-beta LR\nflow f\n direction TB\n a --> b\nend\nglobal\n a\nend\n",
            &meta(),
            &OperationControl::new(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(model.direction, "LR");
        assert_eq!(model.sub_graphs[0].direction.as_deref(), Some("TB"));
        assert_eq!(model.sub_graphs[0].nodes, ["b"]);
        assert_eq!(model.vertices[0].parent_id, None);
    }

    #[test]
    fn unsupported_shapes_keep_source_meaning_and_use_upstream_render_fallback() {
        let model = parse_agentflow_model_for_render_controlled(
            "agentflow-beta\na((Circle))\nb@{ shape: unknown }\nc@{shape: roundedRect}\n",
            &meta(),
            &OperationControl::new(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(model.vertices[0].shape.as_deref(), Some("circle"));
        assert_eq!(model.diagnostics[0].id, "SHAPE_REMOVED");
        assert_eq!(model.diagnostics[1].id, "SHAPE_UNSUPPORTED");
        assert_eq!(model.diagnostics.len(), 2);
        let (flow, _) = model.to_flowchart_model().unwrap();
        assert!(
            flow.nodes
                .iter()
                .all(|node| node.shape.as_deref() == Some("roundedRect"))
        );
    }
    #[test]
    fn collapsing_redirects_edges_without_dropping_authored_self_loops() {
        let model = parse_agentflow_model_for_render_controlled(
            "agentflow-beta\nflow f@{view: collapsed}\n a --> b\n a --> a\nend\na --> external\n",
            &meta(),
            &OperationControl::new(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(model.edges[0].id.as_deref(), Some("L_a_b_0"));
        let (flow, _) = model.to_flowchart_model().unwrap();
        assert_eq!(flow.edges.len(), 2);
        assert_eq!((&*flow.edges[0].from, &*flow.edges[0].to), ("f", "f"));
        assert_eq!(
            (&*flow.edges[1].from, &*flow.edges[1].to),
            ("f", "external")
        );
        assert_eq!(flow.nodes[0].classes, ["af-kind-task"]);
    }
    #[test]
    fn invalid_edges_and_unterminated_labels_do_not_silently_drop_source() {
        for source in ["agentflow-beta\na ==> b\n", "agentflow-beta\na[missing\n"] {
            assert!(parse_agentflow(source, &meta()).is_err());
        }
    }
}
