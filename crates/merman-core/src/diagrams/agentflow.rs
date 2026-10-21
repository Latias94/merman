//! Native semantic model for Mermaid's `agentflow-beta` diagram.
//!
//! Agentflow deliberately keeps its semantic projection separate from Flowchart.  The surface
//! syntax borrows Flowchart's node and edge spelling, while the meaning of shapes, containers,
//! metadata, and the three edge operators is domain specific.

use crate::family::{EditorFactsDemand, SemanticConstruction};
use crate::{
    EditorFamilySemantics, EditorRenamePolicy, EditorSemanticFacts, EditorSemanticKind,
    EditorSemanticSymbol, Error, OperationControl, OperationControlResult, SourceSpan, family,
};
#[cfg(feature = "presentation")]
use crate::{ParseMetadata, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};

mod presentation;
pub use presentation::{AgentflowAuthoredStyles, AgentflowEdgeAttributes, AgentflowNodeAttributes};
#[cfg(feature = "presentation")]
mod render_model;
#[cfg(feature = "presentation")]
pub use render_model::{
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
    #[cfg(feature = "presentation")]
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
    pub warning_facts: Vec<crate::DiagramWarningFact>,
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
    pub(crate) fn sanitize_common_db_fields(&mut self, config: &crate::MermaidConfig) {
        crate::common_db::sanitize_optional_title(&mut self.title, config);
        crate::common_db::sanitize_optional_acc_title(&mut self.acc_title, config);
        crate::common_db::sanitize_optional_acc_descr(&mut self.acc_descr, config);
    }
}

impl AgentflowDiagramModel {
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
    #[cfg(feature = "presentation")]
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

    #[cfg(feature = "presentation")]
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

#[derive(Debug)]
enum ParseFailure {
    Syntax { message: String, span: SourceSpan },
    Cancelled(crate::OperationCancelled),
}

struct Construction {
    model: AgentflowDiagramModel,
    #[cfg(feature = "presentation")]
    visuals: Option<AgentflowVisuals>,
    editor_facts: EditorSemanticFacts,
}

#[cfg(test)]
thread_local! {
    static AGENTFLOW_SYNTAX_CONSTRUCTION_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[derive(Debug, Clone, Default)]
struct Context {
    id: Option<String>,
    title: Option<String>,
    label_type: Option<&'static str>,
    metadata: Map<String, Value>,
    nodes: Vec<String>,
    direction: Option<String>,
    global: bool,
    declaration_span: Option<SourceSpan>,
}

/// Parse the compatibility JSON path used by the public Mermaid facade.
#[cfg(feature = "compat-json")]
pub(crate) fn parse_agentflow(code: &str, meta: &ParseMetadata) -> Result<Value> {
    parse_agentflow_with_warning_facts(code, meta).map(family::WarningSemanticParse::into_model)
}

#[cfg(feature = "compat-json")]
pub(crate) fn parse_agentflow_with_warning_facts(
    code: &str,
    meta: &ParseMetadata,
) -> Result<family::WarningSemanticParse> {
    let control = OperationControl::new();
    let construction = construct_source(
        code,
        &meta.diagram_type,
        meta.effective_config.as_config(),
        meta.title.as_deref(),
        EditorFactsDemand::Omit,
        false,
        &control,
    )
    .expect("a private operation control cannot be cancelled")
    .map_err(family::CombinedSemanticFailure::into_error)?;
    Ok(family::WarningSemanticParse::new(
        semantic_model_to_compat_json(&construction.model, &meta.diagram_type)?,
        construction.model.warning_facts,
    ))
}

pub(crate) fn parse_agentflow_semantic_model_controlled(
    code: &str,
    meta: &crate::SemanticMetadata,
    editor_demand: EditorFactsDemand,
    control: &OperationControl,
) -> OperationControlResult<SemanticConstruction<AgentflowDiagramModel>> {
    let construction = construct_source(
        code,
        &meta.diagram_type,
        &meta.semantic_config,
        meta.title.as_deref(),
        editor_demand,
        #[cfg(feature = "presentation")]
        false,
        control,
    )?;
    Ok(match construction {
        Ok(construction) => {
            let mut model = construction.model;
            model.sanitize_common_db_fields(&meta.semantic_config);
            SemanticConstruction {
                model: Ok(model),
                editor_facts: (editor_demand == EditorFactsDemand::Capture)
                    .then_some(construction.editor_facts),
            }
        }
        Err(failure) if editor_demand == EditorFactsDemand::Capture => {
            let (error, editor_facts) = failure.into_parts();
            SemanticConstruction {
                model: Err(error),
                editor_facts: Some(editor_facts),
            }
        }
        Err(failure) => SemanticConstruction {
            model: Err(failure.into_error()),
            editor_facts: None,
        },
    })
}

#[cfg(feature = "presentation")]
pub(crate) fn parse_agentflow_model_for_render_controlled(
    code: &str,
    meta: &ParseMetadata,
    control: &OperationControl,
) -> OperationControlResult<Result<AgentflowDiagramRenderModel>> {
    Ok(construct_source(
        code,
        &meta.diagram_type,
        meta.effective_config.as_config(),
        meta.title.as_deref(),
        EditorFactsDemand::Omit,
        true,
        control,
    )?
    .map(|construction| AgentflowDiagramRenderModel {
        model: construction.model,
        visuals: construction
            .visuals
            .expect("full rendering requests DOM indices"),
    })
    .map_err(family::CombinedSemanticFailure::into_error))
}

#[cfg(feature = "compat-json")]
pub(crate) fn parse_agentflow_json_and_editor_facts(
    code: &str,
    meta: &ParseMetadata,
    control: &OperationControl,
) -> OperationControlResult<family::CombinedSemanticParse> {
    let construction = construct_source(
        code,
        &meta.diagram_type,
        meta.effective_config.as_config(),
        meta.title.as_deref(),
        EditorFactsDemand::Capture,
        false,
        control,
    )?;
    Ok(
        family::CombinedSemanticParse::from_construction_with_warning_facts(
            construction,
            |construction| {
                (
                    semantic_model_to_compat_json(&construction.model, &meta.diagram_type),
                    construction.editor_facts,
                    construction.model.warning_facts,
                )
            },
            family::CombinedSemanticFailure::into_parts,
        ),
    )
}

#[cfg(feature = "compat-json")]
pub(crate) fn render_model_to_compat_json(
    model: &AgentflowDiagramRenderModel,
    meta: &ParseMetadata,
) -> Result<Value> {
    semantic_model_to_compat_json(&model.model, &meta.diagram_type)
}

#[cfg(feature = "compat-json")]
fn semantic_model_to_compat_json(
    model: &AgentflowDiagramModel,
    diagram_type: &str,
) -> Result<Value> {
    // Keep the established preserve_order removals without serializing discarded facts.
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Compatibility<'a> {
        presentation: (),
        direction: &'a str,
        diagnostics: &'a [AgentflowDiagnostic],
        warning_facts: (),
        vertices: &'a [AgentflowNode],
        edges: &'a [AgentflowEdge],
        sub_graphs: &'a [AgentflowSubGraph],
        connectors: &'a [AgentflowConnector],
        #[serde(skip_serializing_if = "Option::is_none")]
        title: Option<&'a str>,
        #[serde(rename = "accTitle", skip_serializing_if = "Option::is_none")]
        acc_title: Option<&'a str>,
        #[serde(rename = "accDescr", skip_serializing_if = "Option::is_none")]
        acc_descr: Option<&'a str>,
    }
    let mut value = serde_json::to_value(Compatibility {
        presentation: (),
        direction: &model.direction,
        diagnostics: &model.diagnostics,
        warning_facts: (),
        vertices: &model.vertices,
        edges: &model.edges,
        sub_graphs: &model.sub_graphs,
        connectors: &model.connectors,
        title: model.title.as_deref(),
        acc_title: model.acc_title.as_deref(),
        acc_descr: model.acc_descr.as_deref(),
    })
    .map_err(|error| Error::diagram_parse_fallback(diagram_type.to_string(), error.to_string()))?;
    if let Some(root) = value.as_object_mut() {
        root.remove("presentation");
        root.remove("warningFacts");
        root.insert("type".into(), Value::String(diagram_type.to_string()));
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

fn construct_source(
    code: &str,
    diagram_type: &str,
    config: &crate::MermaidConfig,
    title: Option<&str>,
    editor_demand: EditorFactsDemand,
    #[cfg(feature = "presentation")] capture_visuals: bool,
    control: &OperationControl,
) -> OperationControlResult<std::result::Result<Construction, family::CombinedSemanticFailure>> {
    control.checkpoint()?;
    #[cfg(test)]
    AGENTFLOW_SYNTAX_CONSTRUCTION_COUNT.set(AGENTFLOW_SYNTAX_CONSTRUCTION_COUNT.get() + 1);
    let mut parser = Parser::new(
        code,
        config,
        title,
        editor_demand,
        #[cfg(feature = "presentation")]
        capture_visuals,
        control,
    );
    let result = parser.parse()?;
    control.checkpoint()?;
    match result {
        Ok(model) => Ok(Ok(Construction {
            model,
            #[cfg(feature = "presentation")]
            visuals: parser.visuals,
            editor_facts: parser.facts.unwrap_or_default(),
        })),
        Err(ParseFailure::Cancelled(cancelled)) => Err(cancelled),
        Err(ParseFailure::Syntax { message, span }) => {
            if let Some(facts) = &mut parser.facts {
                facts.mark_recovered_from_parse_error(message.clone(), Some(span));
            }
            Ok(Err(family::CombinedSemanticFailure::new(
                Error::diagram_parse_exact(diagram_type.to_string(), message, span),
                parser.facts.unwrap_or_default(),
            )))
        }
    }
}

#[cfg(all(test, feature = "compat-json"))]
fn construct(
    code: &str,
    meta: &ParseMetadata,
    control: &OperationControl,
) -> OperationControlResult<std::result::Result<Construction, family::CombinedSemanticFailure>> {
    construct_source(
        code,
        &meta.diagram_type,
        meta.effective_config.as_config(),
        meta.title.as_deref(),
        EditorFactsDemand::Capture,
        false,
        control,
    )
}

struct Parser<'a> {
    source: &'a str,
    config: &'a crate::MermaidConfig,
    title: Option<&'a str>,
    control: &'a OperationControl,
    facts: Option<EditorSemanticFacts>,
    declared_entities: HashSet<String>,
    authored: AgentflowAuthoredStyles,
    #[cfg(feature = "presentation")]
    visuals: Option<AgentflowVisuals>,
    #[cfg(feature = "presentation")]
    vertex_counter: usize,
    diagnostic_spans: HashMap<String, SourceSpan>,
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
    owned_nodes: HashSet<String>,
    subgraph_count: usize,
    in_frontmatter: bool,
    acc_title: Option<String>,
    acc_descr: Option<String>,
}

impl<'a> Parser<'a> {
    fn new(
        source: &'a str,
        config: &'a crate::MermaidConfig,
        title: Option<&'a str>,
        editor_demand: EditorFactsDemand,
        #[cfg(feature = "presentation")] capture_visuals: bool,
        control: &'a OperationControl,
    ) -> Self {
        let facts = (editor_demand == EditorFactsDemand::Capture).then(|| {
            #[cfg(test)]
            crate::editor::test_work::record_collection();
            let mut facts = EditorSemanticFacts::new();
            facts.family_semantics = EditorFamilySemantics::new(EditorSemanticKind::Object);
            facts
        });
        Self {
            source,
            config,
            title,
            control,
            facts,
            declared_entities: HashSet::new(),
            authored: AgentflowAuthoredStyles::default(),
            #[cfg(feature = "presentation")]
            visuals: capture_visuals.then(AgentflowVisuals::default),
            #[cfg(feature = "presentation")]
            vertex_counter: 0,
            diagnostic_spans: HashMap::new(),
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
            owned_nodes: HashSet::new(),
            subgraph_count: 0,
            in_frontmatter: false,
            acc_title: None,
            acc_descr: None,
        }
    }

    fn parse(
        &mut self,
    ) -> OperationControlResult<std::result::Result<AgentflowDiagramModel, ParseFailure>> {
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
        if header_tokens.len() > 2 {
            return Ok(Err(self.failure(
                "unexpected content after agentflow header direction",
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
            if self.direction == "TD" {
                self.direction = "TB".to_string();
            }
        } else if self.source[header_start + header.len()..]
            .trim_start_matches([' ', '\t', '\r'])
            .starts_with(';')
        {
            return Ok(Err(self.failure(
                "a semicolon after the agentflow header requires a direction",
                SourceSpan::new(header_start, header_start + header.len()),
            )));
        }
        if let Some(facts) = &mut self.facts {
            facts.push_expected_syntax(crate::EditorExpectedSyntax::new(
                crate::EditorExpectedSyntaxKind::NodeIdentifier,
                SourceSpan::new(header_start, header_start + header.len()),
            ));
        }

        for (line_start, line) in split_top_level(self.source, b";\n") {
            self.control.checkpoint()?;
            let trimmed = line.trim().trim_start_matches('\u{feff}');
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
            // Direction precedes whitespace in Jison. Preserve the statement's raw
            // cursor so indentation cannot promote a later keyword ahead of it.
            if let Some(direction) = statement_direction(line) {
                if !direction_has_prior_token(line) {
                    if let Some(context) = self.contexts.last_mut() {
                        context.direction = Some(direction.to_string());
                    }
                    continue;
                }
                if !starts_accessibility_statement(line) && !keyword_with_whitespace(line, "click")
                {
                    return Ok(Err(self.failure(
                        "unexpected direction after agentflow statement token",
                        span_of(line_start, line, trimmed),
                    )));
                }
            }
            if let Some(rest) = trimmed.strip_prefix("accTitle").and_then(|rest| {
                rest.trim_start_matches(super::scan::is_ecmascript_whitespace)
                    .strip_prefix(':')
            }) {
                self.acc_title = Some(rest.trim().to_string());
                continue;
            }
            if let Some(rest) = trimmed.strip_prefix("accDescr").and_then(|rest| {
                rest.trim_start_matches(super::scan::is_ecmascript_whitespace)
                    .strip_prefix(':')
            }) {
                self.acc_descr = Some(rest.trim().to_string());
                continue;
            }
            if let Some(rest) = trimmed.strip_prefix("accDescr")
                && let Some(description) = rest.trim_start().strip_prefix('{')
            {
                let Some(description) = description.strip_suffix('}') else {
                    return Ok(Err(self.failure(
                        "unterminated accessibility description",
                        span_of(line_start, line, trimmed),
                    )));
                };
                self.acc_descr = Some(description.trim().to_string());
                continue;
            }
            if keyword_with_whitespace(trimmed, "click")
                && click_contains_unshielded_direction(trimmed)
            {
                return Ok(Err(self.failure(
                    "unexpected direction in agentflow click statement",
                    span_of(line_start, line, trimmed),
                )));
            }
            if trimmed == "end" {
                let Some(context) = self.contexts.pop() else {
                    return Ok(Err(self.failure(
                        "unexpected agentflow end",
                        span_of(line_start, line, trimmed),
                    )));
                };
                self.close_context(context, span_of(line_start, line, trimmed).end);
                continue;
            }
            if trimmed == "global" {
                self.contexts.push(Context {
                    global: true,
                    ..Default::default()
                });
                continue;
            }
            if let Some(rest) = trimmed.strip_prefix("flow")
                && rest.chars().next().is_none_or(char::is_whitespace)
            {
                let (id, title, label_type, metadata) = if rest.trim().is_empty() {
                    (None, None, Some("markdown"), Map::new())
                } else {
                    match parse_declaration(
                        rest,
                        line_start + line.find(trimmed).unwrap_or(0) + "flow".len(),
                        rest,
                        self.control,
                    ) {
                        Ok(Declaration {
                            id,
                            id_span,
                            label: title,
                            label_type,
                            metadata,
                            ..
                        }) => {
                            self.push_symbol(&id, EditorSemanticKind::Namespace, id_span, false);
                            (Some(id), title, label_type.or(Some("text")), metadata)
                        }
                        Err(error) => return Ok(Err(error)),
                    }
                };
                let declaration_span = id.as_ref().map(|_| span_of(line_start, line, trimmed));
                self.contexts.push(Context {
                    id,
                    title,
                    label_type,
                    metadata,
                    declaration_span,
                    ..Default::default()
                });
                continue;
            }
            if let Some(rest) = trimmed.strip_prefix("connector")
                && rest.chars().next().is_none_or(char::is_whitespace)
            {
                let declaration = match parse_declaration(
                    rest,
                    line_start + line.find(trimmed).unwrap_or(0) + "connector".len(),
                    rest,
                    self.control,
                ) {
                    Ok(value) => value,
                    Err(error) => return Ok(Err(error)),
                };
                let id = declaration.id.clone();
                let id_span = declaration.id_span;
                let first = !self.connector_index.contains_key(&id);
                if first {
                    // addConnector replaces a pre-existing ordinary vertex, preserving map order.
                    if let Some(&index) = self.node_index.get(&id) {
                        self.nodes[index] = AgentflowNode {
                            id: id.clone(),
                            label: Some(id.clone()),
                            shape: None,
                            vertex_kind: AgentflowVertexKind::Connector,
                            metadata: Map::new(),
                            parent_id: None,
                        };
                    }
                    self.authored.nodes.insert(id.clone(), Default::default());
                    #[cfg(feature = "presentation")]
                    if let Some(visuals) = &mut self.visuals {
                        visuals.node_dom_indices.remove(&id);
                    }
                    self.record_vertex_call(&id);
                }
                let title = declaration.label.filter(|title| !title.is_empty());
                let metadata = declaration.metadata;
                self.upsert_connector(id.clone(), title.clone(), Map::new());
                let node_index = self.upsert_node(Declaration {
                    id: id.clone(),
                    id_span,
                    label: title,
                    label_type: Some("text"),
                    syntax_shape: None,
                    metadata: Map::new(),
                    metadata_span: None,
                    authored_shape: None,
                    class: None,
                });
                self.nodes[node_index].vertex_kind = AgentflowVertexKind::Connector;
                if let Some(&index) = self.sub_graph_index.get(&id) {
                    self.sub_graphs[index].metadata.extend(metadata);
                } else {
                    let index = self.connector_index[&id];
                    self.connectors[index].metadata.extend(metadata);
                }
                self.record_members(std::iter::once(id.clone()));
                self.diagnostic_spans
                    .entry(id.clone())
                    .or_insert_with(|| span_of(line_start, line, trimmed));
                self.push_symbol(&id, EditorSemanticKind::Object, id_span, false);
                continue;
            }
            match self
                .parse_presentation_statement(trimmed, line_start + line.find(trimmed).unwrap_or(0))
            {
                Ok(true) => continue,
                Ok(false) => {}
                Err(error) => return Ok(Err(error)),
            }
            match self.parse_edge_statement(trimmed, line_start, line) {
                Ok(true) => continue,
                Ok(false) => {}
                Err(error) => return Ok(Err(error)),
            }
            if let Err(error) = self.parse_node_statement(trimmed, line_start, line) {
                return Ok(Err(error));
            }
        }
        if !self.contexts.is_empty() {
            return Ok(Err(self.failure(
                "unterminated agentflow container",
                SourceSpan::new(0, self.source.len()),
            )));
        }
        for graph in &self.sub_graphs {
            for child in &graph.nodes {
                if let Some(&index) = self.node_index.get(child) {
                    self.nodes[index].parent_id = Some(graph.id.clone());
                }
            }
        }
        let diagnostics = self
            .nodes
            .iter()
            .filter(|node| !self.sub_graph_index.contains_key(&node.id))
            .filter_map(shape_diagnostic)
            .collect();
        let mut model = AgentflowDiagramModel {
            authored: std::mem::take(&mut self.authored),
            warning_facts: Vec::new(),
            direction: std::mem::take(&mut self.direction),
            diagnostics,
            vertices: std::mem::take(&mut self.nodes)
                .into_iter()
                .filter(|node| {
                    !self.sub_graph_index.contains_key(&node.id)
                        && !self.connector_index.contains_key(&node.id)
                })
                .collect(),
            edges: std::mem::take(&mut self.edges),
            sub_graphs: std::mem::take(&mut self.sub_graphs),
            connectors: std::mem::take(&mut self.connectors),
            title: self.title.map(str::to_string),
            acc_title: self.acc_title.take(),
            acc_descr: self.acc_descr.take(),
        };
        model.diagnostics.extend(model.containment().diagnostics);
        for diagnostic in &mut model.diagnostics {
            diagnostic.position = self
                .diagnostic_spans
                .get(&diagnostic.node_id)
                .map(|span| diagnostic_position(self.source, *span));
        }
        model.warning_facts = model
            .diagnostics
            .iter()
            .map(|diagnostic| {
                let mut fact = crate::DiagramWarningFact::new(
                    diagnostic.rule_id(),
                    diagnostic.message.clone(),
                );
                fact.span = self.diagnostic_spans.get(&diagnostic.node_id).copied();
                fact
            })
            .collect();
        Ok(Ok(model))
    }

    fn parse_node_statement(
        &mut self,
        statement: &str,
        line_start: usize,
        line: &str,
    ) -> std::result::Result<(), ParseFailure> {
        let start = line_start + line.find(statement).unwrap_or(0);
        let ids = self.parse_node_group(statement, start, false)?;
        self.record_members(ids);
        Ok(())
    }

    fn parse_node(
        &mut self,
        statement: &str,
        line_start: usize,
        line: &str,
        reference: bool,
    ) -> std::result::Result<(String, bool), ParseFailure> {
        let declaration = parse_declaration(statement, line_start, line, self.control)?;
        let id = declaration.id.clone();
        let id_span = declaration.id_span;
        let class = declaration.class.clone();
        let mapped_element = if declaration.metadata_span.is_some() {
            statement.trim()
        } else {
            &statement[..find_top_level_marker(statement, ":::").unwrap_or(statement.len())]
        };
        self.diagnostic_spans
            .entry(id.clone())
            .or_insert_with(|| span_of(line_start, line, mapped_element.trim()));
        // The enclosing node group reduces metadata after parsing the next styled vertex.
        // AgentflowDB skips metadata counter increments for existing containers/connectors.
        let vertex_call = id != "connectors"
            && !self
                .edges
                .iter()
                .any(|edge| edge.id.as_deref() == Some(&id));
        let metadata_call = vertex_call
            && declaration.metadata_span.is_some()
            && !self.sub_graph_index.contains_key(&id)
            && !self.connector_index.contains_key(&id);
        if vertex_call {
            self.record_vertex_call(&id);
        }
        if declaration.metadata_span.is_some()
            && (self.sub_graph_index.contains_key(&id) || self.connector_index.contains_key(&id))
            && !self
                .edges
                .iter()
                .any(|edge| edge.id.as_deref() == Some(&id))
        {
            self.upsert_node(Declaration {
                id: id.clone(),
                id_span,
                label: declaration.label.clone(),
                label_type: declaration.label_type,
                syntax_shape: declaration.syntax_shape.clone(),
                metadata: Map::new(),
                metadata_span: None,
                authored_shape: None,
                class: None,
            });
        }
        if declaration.metadata_span.is_some()
            && let Some(&index) = self.sub_graph_index.get(&id)
        {
            self.sub_graphs[index].metadata.extend(declaration.metadata);
            self.push_symbol(&id, EditorSemanticKind::Namespace, id_span, true);
        } else if declaration.metadata_span.is_some()
            && let Some(&index) = self.connector_index.get(&id)
        {
            self.connectors[index].metadata.extend(declaration.metadata);
            self.push_symbol(&id, EditorSemanticKind::Object, id_span, true);
        } else if let Some(edge) = self
            .edges
            .iter_mut()
            .find(|edge| edge.id.as_deref() == Some(&id))
        {
            self.authored
                .attach_edge_metadata(&id, &declaration.metadata);
            edge.metadata.extend(declaration.metadata);
            self.push_symbol(&id, EditorSemanticKind::Object, id_span, true);
        } else {
            declaration.validate_shape()?;
            let reference = reference && self.declared_entities.contains(&id);
            self.upsert_node(declaration);
            self.push_symbol(&id, EditorSemanticKind::Object, id_span, reference);
        }
        if let Some(class) = class {
            self.assign_class(&id, &class);
        }
        Ok((id, metadata_call))
    }

    fn parse_edge_statement(
        &mut self,
        statement: &str,
        line_start: usize,
        line: &str,
    ) -> std::result::Result<bool, ParseFailure> {
        let Some(mut operator) = find_operator(statement, 0) else {
            return Ok(false);
        };
        let statement_start = line_start + line.find(statement).unwrap_or(0);
        let (source, mut edge_id) = split_edge_id(&statement[..operator.start]);
        let mut sources = self.parse_node_group(source, statement_start, true)?;
        let mut members = vec![sources.clone()];
        loop {
            let next = find_operator(statement, operator.end);
            let target_end = next
                .as_ref()
                .map_or(statement.len(), |operator| operator.start);
            let raw_targets = &statement[operator.end..target_end];
            let (target, next_edge_id) = if next.is_some() {
                split_edge_id(raw_targets)
            } else {
                (raw_targets, None)
            };
            let targets = self.parse_node_group(target, statement_start + operator.end, true)?;
            members.push(targets.clone());
            // Mermaid emits one link for every source/target pair, then uses the target group
            // as the source of the next link in a chain.
            for source in &sources {
                for target in &targets {
                    let mut edge = edge_for(
                        source,
                        target,
                        operator.semantic,
                        operator.label.clone(),
                        operator.length,
                    );
                    if Some(source) == sources.last() && Some(target) == targets.first() {
                        edge.id = edge_id.clone();
                    }
                    self.push_edge(
                        edge,
                        operator.label_type,
                        SourceSpan::new(statement_start, statement_start + statement.len()),
                    )?;
                }
            }
            let Some(next) = next else {
                // Grammar reductions prepend each target group to the statement's members.
                self.record_members(members.into_iter().rev().flatten());
                return Ok(true);
            };
            sources = targets;
            operator = next;
            edge_id = next_edge_id;
        }
    }

    fn parse_node_group(
        &mut self,
        group: &str,
        source_start: usize,
        reference: bool,
    ) -> std::result::Result<Vec<String>, ParseFailure> {
        let mut ids = Vec::new();
        let mut pending_metadata: Option<String> = None;
        for (offset, endpoint) in split_top_level(group, b"&") {
            if ids.len() % 128 == 0 {
                self.control.checkpoint().map_err(ParseFailure::Cancelled)?;
            }
            let start = source_start + offset;
            let (id, metadata_call) = self.parse_node(endpoint, start, endpoint, reference)?;
            if let Some(previous) = pending_metadata.take() {
                self.record_vertex_call(&previous);
            }
            pending_metadata = metadata_call.then(|| id.clone());
            ids.push(id);
        }
        if let Some(last) = pending_metadata {
            self.record_vertex_call(&last);
        }
        if (reference && ids.is_empty()) || group.trim_end().ends_with('&') {
            return Err(self.failure(
                if reference {
                    "expected edge endpoint"
                } else {
                    "expected node after &"
                },
                SourceSpan::new(source_start, source_start + group.len()),
            ));
        }
        Ok(ids)
    }

    fn push_edge(
        &mut self,
        mut edge: AgentflowEdge,
        label_type: Option<&str>,
        span: SourceSpan,
    ) -> std::result::Result<(), ParseFailure> {
        if self.edges.len().is_multiple_of(128) {
            self.control.checkpoint().map_err(ParseFailure::Cancelled)?;
        }
        let limit = self
            .config
            .as_value()
            .get("maxEdges")
            .and_then(Value::as_f64)
            .unwrap_or(500.0);
        if self.edges.len() as f64 >= limit {
            return Err(self.failure(
                format!(
                    "Edge limit exceeded. {} edges found, but the limit is {limit}.",
                    self.edges.len()
                ),
                span,
            ));
        }
        edge.is_user_defined_id = edge.id.is_some();
        if edge.id.as_ref().is_none_or(|id| {
            self.edges
                .iter()
                .any(|existing| existing.id.as_ref() == Some(id))
        }) {
            edge.is_user_defined_id = false;
            let count = self
                .edges
                .iter()
                .filter(|existing| existing.start == edge.start && existing.end == edge.end)
                .count();
            let counter = if count == 0 { 0 } else { count + 1 };
            edge.id = Some(format!("L_{}_{}_{counter}", edge.start, edge.end));
        }
        self.authored.add_edge(
            edge.id.as_deref().expect("edge id is assigned above"),
            label_type,
        );
        self.edges.push(edge);
        Ok(())
    }

    fn record_vertex_call(&mut self, _id: &str) {
        #[cfg(feature = "presentation")]
        if let Some(visuals) = &mut self.visuals {
            visuals
                .node_dom_indices
                .entry(_id.to_string())
                .or_insert(self.vertex_counter);
            self.vertex_counter += 1;
        }
    }

    fn upsert_node(&mut self, declaration: Declaration) -> usize {
        let Declaration {
            id,
            label,
            label_type,
            syntax_shape,
            metadata,
            ..
        } = declaration;
        let presentation = self.authored.nodes.entry(id.clone()).or_default();
        if label.is_some() {
            presentation.label_type = label_type.map(str::to_owned);
        }
        let metadata_label = metadata
            .get("label")
            .and_then(Value::as_str)
            .filter(|label| !label.is_empty());
        if metadata_label.is_some() {
            let label_type = metadata
                .get("labelType")
                .and_then(Value::as_str)
                .filter(|kind| matches!(*kind, "text" | "string" | "markdown"))
                .unwrap_or("markdown");
            presentation.label_type = Some(label_type.to_string());
        }
        let label = metadata_label.map(str::to_string).or(label);
        let shape = metadata
            .get("shape")
            .and_then(Value::as_str)
            .filter(|shape| !shape.is_empty())
            .map(resolve_shape)
            .or(syntax_shape);
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
        if let Some(&connector) = self.connector_index.get(&id) {
            self.nodes[index].vertex_kind = AgentflowVertexKind::Connector;
            self.connectors[connector].title =
                self.nodes[index].label.clone().filter(|label| label != &id);
        }
        index
    }

    fn record_members(&mut self, ids: impl IntoIterator<Item = String>) {
        if let Some(context) = self.contexts.last_mut() {
            context.nodes.extend(ids);
        }
    }

    fn close_context(&mut self, context: Context, end: usize) {
        if context.global {
            self.global_nodes.extend(context.nodes);
            for graph in &mut self.sub_graphs {
                graph
                    .nodes
                    .retain(|child| !self.global_nodes.contains(child));
            }
            self.owned_nodes
                .retain(|id| !self.global_nodes.contains(id));
            return;
        }
        let id = context
            .id
            .unwrap_or_else(|| format!("subGraph{}", self.subgraph_count));
        self.subgraph_count += 1;
        if let Some(span) = context.declaration_span {
            self.diagnostic_spans
                .entry(id.clone())
                .or_insert(SourceSpan::new(span.start, end));
        }
        let mut seen = HashSet::new();
        let nodes: Vec<_> = context
            .nodes
            .into_iter()
            .filter(|child| {
                child != &id
                    && !self.global_nodes.contains(child)
                    && !self.owned_nodes.contains(child)
                    && seen.insert(child.clone())
            })
            .collect();
        self.owned_nodes.extend(nodes.iter().cloned());
        let direction = context.direction.or_else(|| {
            self.config
                .as_value()
                .pointer("/flowchart/inheritDir")
                .and_then(Value::as_bool)
                .unwrap_or(false)
                .then(|| self.direction.clone())
        });
        let title = context.title.unwrap_or_default();
        // Duplicate containers retain the label type from their first declaration.
        self.authored
            .subgraph_label_types
            .entry(id.clone())
            .or_insert_with(|| context.label_type.unwrap_or("text").to_string());
        if let Some(&index) = self.sub_graph_index.get(&id) {
            let graph = &mut self.sub_graphs[index];
            graph.nodes.extend(nodes);
            if !title.is_empty() {
                graph.title = Some(title);
            }
            if direction.is_some() {
                graph.direction = direction;
            }
            graph.metadata.extend(context.metadata);
        } else {
            self.sub_graph_index
                .insert(id.clone(), self.sub_graphs.len());
            self.sub_graphs.push(AgentflowSubGraph {
                id: id.clone(),
                title: Some(title),
                nodes,
                sub_graph_type: "flow".into(),
                direction,
                metadata: context.metadata,
            });
        }
        self.record_members(std::iter::once(id));
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
        span: SourceSpan,
        reference: bool,
    ) {
        let Some(facts) = &mut self.facts else {
            return;
        };
        if !reference {
            self.declared_entities.insert(id.to_string());
        }
        let symbol = if reference {
            EditorSemanticSymbol::reference(id, None, kind, span, span)
        } else {
            EditorSemanticSymbol::new(id, None, kind, span, span)
        };
        facts.push_symbol(symbol.with_rename_policy(EditorRenamePolicy::AgentflowNodeId));
    }

    fn failure(&self, message: impl Into<String>, span: SourceSpan) -> ParseFailure {
        ParseFailure::Syntax {
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
    label_type: Option<&'static str>,
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
        is_user_defined_id: false,
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

#[cfg(feature = "presentation")]
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
        position: None,
        message: format!("shape \"{shape}\" {reason}, using \"roundedRect\""),
    })
}

fn diagnostic_position(source: &str, span: SourceSpan) -> AgentflowDiagnosticPosition {
    let start = span.start.min(source.len());
    let mut end = span.end.min(source.len());
    let element = &source[start..end];
    if element.ends_with('}') && find_metadata_start(element).is_some() {
        // Jison consumes the closing metadata brace without returning a token.
        end -= 1;
    }
    let line_column = |offset: usize| {
        let prefix = &source[..offset];
        (
            prefix.bytes().filter(|byte| *byte == b'\n').count() + 1,
            prefix
                .rsplit_once('\n')
                .map_or(prefix, |(_, line)| line)
                .encode_utf16()
                .count(),
        )
    };
    let (start_line, start_column) = line_column(start);
    let (end_line, end_column) = line_column(end);
    AgentflowDiagnosticPosition {
        start_line,
        start_column,
        end_line,
        end_column,
        // Mermaid 12 does not enable Jison ranges; its fallback indices are both zero.
        start_index: 0,
        end_index: 0,
    }
}

impl AgentflowDiagramModel {
    pub(crate) fn offset_diagnostic_positions(&mut self, line_offset: usize, column_offset: usize) {
        for diagnostic in &mut self.diagnostics {
            if let Some(position) = &mut diagnostic.position {
                position.offset(line_offset, column_offset);
            }
        }
    }
}

impl AgentflowDiagnosticPosition {
    fn offset(&mut self, line_offset: usize, column_offset: usize) {
        if self.start_line == 1 {
            self.start_column += column_offset;
        }
        if self.end_line == 1 {
            self.end_column += column_offset;
        }
        self.start_line += line_offset;
        self.end_line += line_offset;
    }
}

#[cfg(feature = "compat-json")]
pub(crate) fn offset_compatibility_diagnostic_positions(
    model: &mut Value,
    line_offset: usize,
    column_offset: usize,
) {
    if (line_offset == 0 && column_offset == 0) || model["type"] != "agentflow" {
        return;
    }
    let Some(diagnostics) = model.get_mut("diagnostics").and_then(Value::as_array_mut) else {
        return;
    };
    for diagnostic in diagnostics {
        let Some(position) = diagnostic.get_mut("position") else {
            continue;
        };
        for (line_key, column_key) in [("startLine", "startColumn"), ("endLine", "endColumn")] {
            let Some(line) = position[line_key].as_u64() else {
                continue;
            };
            if line == 1
                && let Some(column) = position[column_key].as_u64()
            {
                position[column_key] = Value::from(column + column_offset as u64);
            }
            position[line_key] = Value::from(line + line_offset as u64);
        }
    }
}

impl AgentflowDiagnostic {
    fn rule_id(&self) -> &'static str {
        match self.id.as_str() {
            "SHAPE_REMOVED" => crate::AGENTFLOW_SHAPE_REMOVED_WARNING_RULE_ID,
            "SHAPE_UNSUPPORTED" => crate::AGENTFLOW_SHAPE_UNSUPPORTED_WARNING_RULE_ID,
            "CONTAINMENT_VIOLATION" => crate::AGENTFLOW_CONTAINMENT_VIOLATION_WARNING_RULE_ID,
            _ => "merman.semantic.agentflow.diagnostic",
        }
    }
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

fn opens_quote(source: &str, index: usize) -> bool {
    matches!(source.as_bytes()[index], b'"' | b'\'')
        && source[..index]
            .chars()
            .next_back()
            .is_none_or(|ch| !ch.is_alphanumeric() && ch != '_')
}

/// Yield borrowed statements without splitting quoted labels, shapes or metadata blocks.
fn split_top_level<'a>(
    source: &'a str,
    separators: &'static [u8],
) -> impl Iterator<Item = (usize, &'a str)> {
    let mut cursor = 0;
    let mut physical_line_end = 0;
    let mut direction_scan_end = 0;
    std::iter::from_fn(move || {
        if cursor >= source.len() {
            return None;
        }
        let start = cursor;
        if separators == b";\n"
            && let Some(rest) = source[start..].trim_start().strip_prefix("accDescr")
            && let Some(body) = rest.trim_start().strip_prefix('{')
            && (starts_accessibility_statement(&source[start..])
                || statement_direction(source[start..].split('\n').next().unwrap_or_default())
                    .is_none())
        {
            // The accessibility lexer treats everything up to the first } as plain text.
            cursor = body
                .find('}')
                .map_or(source.len(), |end| source.len() - body.len() + end + 1);
            return Some((start, &source[start..cursor]));
        }
        if separators == b";\n" {
            let tail = &source[start..];
            if start >= physical_line_end {
                physical_line_end = start + tail.find('\n').unwrap_or(tail.len());
            }
            let line_end = physical_line_end - start;
            let line = &tail[..line_end];
            let trimmed = line.trim_start();
            let accessibility_line = ["accTitle", "accDescr"].into_iter().any(|keyword| {
                trimmed.strip_prefix(keyword).is_some_and(|rest| {
                    rest.trim_start_matches(super::scan::is_ecmascript_whitespace)
                        .starts_with(':')
                })
            });
            // Whole-line Jison rules run before the semicolon token. Keep the
            // header and earlier keyword rules in their existing token stream.
            let direction_line =
                !direction_has_prior_token(line) && start >= direction_scan_end && {
                    let matched = statement_direction(line).is_some();
                    if !matched {
                        direction_scan_end = physical_line_end;
                    }
                    matched
                };
            if accessibility_line || direction_line {
                cursor = start + line_end + usize::from(line_end < tail.len());
                return Some((start, line));
            }
            if let Some(suffix) = keyword_suffix(trimmed, "end") {
                // END consumes horizontal whitespace itself. A following END must
                // start at its keyword, rather than at the preceding separator.
                let token_end = start + line.len() - trimmed.len() + "end".len();
                cursor = token_end + suffix.len() - suffix.trim_start_matches([' ', '\t']).len();
                return Some((start, &source[start..token_end]));
            }
        }
        let mut depth = 0usize;
        let mut in_label = false;
        let mut quote = None;
        let mut escaped = false;
        while cursor < source.len() {
            let byte = source.as_bytes()[cursor];
            if let Some(delimiter) = quote {
                if escaped {
                    escaped = false;
                } else if byte == b'\\' {
                    escaped = true;
                } else if byte == delimiter {
                    quote = None;
                }
            } else if opens_quote(source, cursor) {
                quote = Some(byte);
            } else if in_label
                && source.as_bytes()[cursor..].starts_with(b"%%")
                && source.as_bytes().get(cursor + 2) != Some(&b'{')
            {
                cursor = source[cursor..]
                    .find('\n')
                    .map_or(source.len(), |end| cursor + end);
                continue;
            } else if depth == 0 && source.as_bytes()[cursor..].starts_with(b"%%") {
                let end = cursor;
                cursor = source[cursor..]
                    .find('\n')
                    .map_or(source.len(), |n| cursor + n + 1);
                return Some((start, &source[start..end]));
            } else if matches!(byte, b'[' | b'(' | b'{') {
                if depth == 0 {
                    in_label = byte != b'{'
                        || source.as_bytes().get(cursor.wrapping_sub(1)) != Some(&b'@');
                }
                depth += 1;
            } else if matches!(byte, b']' | b')' | b'}') {
                depth = depth.saturating_sub(1);
                in_label &= depth > 0;
            } else if depth == 0 && separators.contains(&byte) {
                let end = cursor;
                cursor += 1;
                return Some((start, &source[start..end]));
            }
            cursor += 1;
        }
        Some((start, &source[start..]))
    })
}

fn first_content_line(source: &str) -> Option<(usize, &str)> {
    let mut frontmatter = false;
    for (offset, line) in split_top_level(source, b";\n") {
        let trimmed = line.trim().trim_start_matches('\u{feff}');
        if trimmed == "---" {
            frontmatter = !frontmatter;
            continue;
        }
        if !frontmatter && !trimmed.is_empty() {
            return Some((offset + line.find(trimmed).unwrap_or(0), trimmed));
        }
    }
    None
}

fn span_of(line_start: usize, line: &str, value: &str) -> SourceSpan {
    let start = line.find(value).unwrap_or(0);
    SourceSpan::new(line_start + start, line_start + start + value.len())
}

fn find_operator(source: &str, from: usize) -> Option<Operator> {
    let bytes = source.as_bytes();
    let mut depth = 0usize;
    let mut in_label = false;
    let mut quote = None;
    let mut escaped = false;
    let mut index = from;
    while index < bytes.len() {
        let ch = bytes[index] as char;
        if let Some(q) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == q {
                quote = None;
            }
            index += 1;
            continue;
        }
        if opens_quote(source, index) {
            quote = Some(ch);
            index += 1;
            continue;
        }
        if in_label && bytes[index..].starts_with(b"%%") && bytes.get(index + 2) != Some(&b'{') {
            index = source[index..]
                .find('\n')
                .map_or(source.len(), |end| index + end);
            continue;
        }
        if matches!(ch, '[' | '(' | '{') {
            if depth == 0 {
                in_label = ch != '{' || bytes.get(index.wrapping_sub(1)) != Some(&b'@');
            }
            depth += 1;
            index += 1;
            continue;
        }
        if matches!(ch, ']' | ')' | '}') {
            depth = depth.saturating_sub(1);
            in_label &= depth > 0;
            index += 1;
            continue;
        }
        if depth == 0 && bytes[index] == b'-' {
            if let Some(mut operator) = link_at(source, index) {
                let end = operator.end;
                let label_start = end + source[end..].len() - source[end..].trim_start().len();
                if source[label_start..].starts_with('|')
                    && let Some(close) = source[label_start + 1..].find('|')
                {
                    let close = label_start + 1 + close;
                    let (label, label_type) = parse_label_text(&source[label_start + 1..close]);
                    operator.label = Some(label);
                    operator.label_type = Some(label_type);
                    operator.end = close + 1;
                }
                return Some(operator);
            }
            if source[index..].starts_with("--") {
                let label_start = index + 2;
                if let Some(arrow) = find_labeled_arrow(source, label_start) {
                    let label = source[label_start..arrow.start].trim();
                    return Some(Operator {
                        start: index,
                        end: arrow.end,
                        semantic: arrow.semantic,
                        length: arrow.length,
                        label: (!label.is_empty()).then(|| parse_label_text(label).0),
                        label_type: (!label.is_empty()).then(|| parse_label_text(label).1),
                    });
                }
            }
        }
        index += 1;
    }
    None
}

fn link_at(source: &str, start: usize) -> Option<Operator> {
    let bytes = &source.as_bytes()[start..];
    let (width, semantic, length) = if bytes.starts_with(b"--") {
        let dashes = bytes.iter().take_while(|&&byte| byte == b'-').count();
        let semantic = match bytes.get(dashes) {
            Some(b'>') => AgentflowEdgeSemantic::Sequence,
            Some(b'x') => AgentflowEdgeSemantic::Failure,
            _ => return None,
        };
        (dashes + 1, semantic, dashes - 1)
    } else if bytes.starts_with(b"-.") {
        let dots = bytes[1..].iter().take_while(|&&byte| byte == b'.').count();
        if bytes.get(dots + 1) != Some(&b'-') {
            return None;
        }
        (dots + 2, AgentflowEdgeSemantic::Reference, dots)
    } else {
        return None;
    };
    Some(Operator {
        start,
        end: start + width,
        semantic,
        length: length.min(10),
        label: None,
        label_type: None,
    })
}

fn find_labeled_arrow(source: &str, from: usize) -> Option<Operator> {
    source[from..]
        .char_indices()
        .filter(|(_, ch)| *ch == '-')
        .find_map(|(offset, _)| {
            link_at(source, from + offset)
                .filter(|operator| operator.semantic != AgentflowEdgeSemantic::Reference)
        })
}

struct Declaration {
    id: String,
    id_span: SourceSpan,
    label: Option<String>,
    label_type: Option<&'static str>,
    syntax_shape: Option<String>,
    metadata: Map<String, Value>,
    metadata_span: Option<SourceSpan>,
    authored_shape: Option<Value>,
    class: Option<String>,
}

impl Declaration {
    fn validate_shape(&self) -> std::result::Result<(), ParseFailure> {
        let Some(value) = self.authored_shape.as_ref().filter(|value| match value {
            Value::Null | Value::Bool(false) => false,
            Value::Number(number) => number.as_f64() != Some(0.0),
            Value::String(text) => !text.is_empty(),
            _ => true,
        }) else {
            return Ok(());
        };
        let error = match value.as_str() {
            None => Some(format!("No such shape: {value}.")),
            Some(shape) if shape != shape.to_lowercase() || shape.contains('_') => Some(format!(
                "No such shape: {shape}. Shape names should be lowercase."
            )),
            Some(shape) if !super::shapes::is_valid_pinned_shape(&resolve_shape(shape)) => {
                Some(format!("No such shape: {}.", resolve_shape(shape)))
            }
            _ => None,
        };
        match error {
            Some(message) => Err(ParseFailure::Syntax {
                message,
                span: self
                    .metadata_span
                    .expect("authored shape has a metadata span"),
            }),
            None => Ok(()),
        }
    }
}

fn parse_declaration(
    statement: &str,
    line_start: usize,
    line: &str,
    control: &OperationControl,
) -> std::result::Result<Declaration, ParseFailure> {
    let statement = statement.trim().trim_end_matches(';').trim();
    if statement.is_empty() {
        return Err(ParseFailure::Syntax {
            message: "expected declaration".to_string(),
            span: span_of(line_start, line, statement),
        });
    }
    let metadata_start = find_metadata_start(statement);
    let mut metadata_span = None;
    let (head, mut metadata) = if let Some(start) = metadata_start {
        let end = matching_brace(statement, start).ok_or_else(|| ParseFailure::Syntax {
            message: "unterminated agentflow metadata".to_string(),
            span: span_of(line_start, line, statement),
        })?;
        if !statement[end..].trim().is_empty() {
            return Err(ParseFailure::Syntax {
                message: "unexpected content after agentflow metadata".into(),
                span: span_of(line_start, line, &statement[end..]),
            });
        }
        metadata_span = Some(span_of(line_start, line, &statement[start..end]));
        (
            &statement[..start],
            parse_metadata(&statement[start + 2..end - 1], control)
                .map_err(ParseFailure::Cancelled)?
                .map_err(|message| ParseFailure::Syntax {
                    message,
                    span: span_of(line_start, line, &statement[start..end]),
                })?,
        )
    } else {
        (statement, Map::new())
    };
    let head = head.trim();
    let (head, class) = if let Some(start) = find_top_level_marker(head, ":::") {
        let class = head[start + 3..].trim();
        if !is_style_identifier(class) {
            return Err(ParseFailure::Syntax {
                message: "expected class identifier after :::".into(),
                span: span_of(line_start, line, &head[start..]),
            });
        }
        (&head[..start], Some(class.to_string()))
    } else {
        (head, None)
    };
    let id_end = head
        .find(['[', '(', '{', '<', '>', '|'])
        .unwrap_or(head.len());
    let id = head[..id_end]
        .split_whitespace()
        .next()
        .unwrap_or("")
        .trim()
        .to_string();
    if id.is_empty() || starts_reserved_identifier(&id) {
        return Err(ParseFailure::Syntax {
            message: "expected agentflow node identifier".to_string(),
            span: span_of(line_start, line, head),
        });
    }
    let remainder = head[id_end..].trim();
    let (label, shape, label_type) = parse_label(remainder);
    if head[..id_end].trim() != id || (!remainder.is_empty() && shape.is_none()) {
        return Err(ParseFailure::Syntax {
            message: "invalid agentflow declaration or unsupported edge operator".into(),
            span: span_of(line_start, line, statement),
        });
    }
    let authored_shape = metadata.get("shape").cloned();
    if let Some(Value::String(shape)) = metadata.get_mut("shape") {
        *shape = resolve_shape(shape);
    }
    let id_start = line_start + line.find(statement).unwrap_or(0);
    let id_span = SourceSpan::new(id_start, id_start + id.len());
    Ok(Declaration {
        id,
        id_span,
        label,
        label_type,
        syntax_shape: shape.map(resolve_shape),
        metadata,
        metadata_span,
        authored_shape,
        class,
    })
}

fn parse_label(remainder: &str) -> (Option<String>, Option<&'static str>, Option<&'static str>) {
    let cleaned = strip_label_comments(remainder);
    let remainder = cleaned.as_str();
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
            let (text, kind) = parse_label_text(body);
            return (Some(text), Some(shape), Some(kind));
        }
    }
    (None, None, None)
}

fn parse_label_text(value: &str) -> (String, &'static str) {
    let value = value.trim();
    if let Some(body) = value
        .strip_prefix("\"`")
        .and_then(|body| body.strip_suffix("`\""))
    {
        (body.trim().to_string(), "markdown")
    } else if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
        (unquote(value).trim().to_string(), "string")
    } else {
        (unquote(value), "text")
    }
}

fn strip_label_comments(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut quote = None;
    let mut escaped = false;
    let mut comment = false;
    for (index, ch) in value.char_indices() {
        if comment {
            if ch != '\n' {
                continue;
            }
            comment = false;
        }
        if let Some(delimiter) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == delimiter {
                quote = None;
            }
        } else if opens_quote(value, index) {
            quote = Some(ch);
        } else if value[index..].starts_with("%%") && !value[index..].starts_with("%%{") {
            comment = true;
            continue;
        }
        out.push(ch);
    }
    out
}

fn split_edge_id(source: &str) -> (&str, Option<String>) {
    let source = source.trim_end();
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

fn is_style_identifier(value: &str) -> bool {
    !value.is_empty()
        && !value.contains(":::")
        && !value.contains("==")
        && value
            .chars()
            .all(|ch| ch.is_alphanumeric() || "!#$%&'*+.-/\\_`?:,=".contains(ch))
}

// These Jison tokens are excluded from idStringToken. Its word boundary is ASCII,
// so punctuation and non-ASCII letters terminate the reserved word as well.
fn starts_reserved_identifier(value: &str) -> bool {
    [
        "agentflow-beta",
        "flow",
        "connector",
        "global",
        "end",
        "style",
        "linkStyle",
        "interpolate",
        "classDef",
        "class",
        "_self",
        "_blank",
        "_parent",
        "_top",
    ]
    .into_iter()
    .any(|keyword| keyword_suffix(value, keyword).is_some())
}

fn keyword_suffix<'a>(value: &'a str, keyword: &str) -> Option<&'a str> {
    value.strip_prefix(keyword).filter(|suffix| {
        suffix
            .as_bytes()
            .first()
            .is_none_or(|byte| !byte.is_ascii_alphanumeric() && *byte != b'_')
    })
}

fn keyword_with_whitespace(value: &str, keyword: &str) -> bool {
    value.strip_prefix(keyword).is_some_and(|suffix| {
        suffix
            .chars()
            .next()
            .is_some_and(super::scan::is_ecmascript_whitespace)
    })
}

// Preserve the INITIAL cursor after each protected CLICK/HREF/string/callback rule.
fn click_contains_unshielded_direction(statement: &str) -> bool {
    let rest = statement["click".len()..].trim_start_matches(super::scan::is_ecmascript_whitespace);
    let Some(id_end) = rest.find(super::scan::is_ecmascript_whitespace) else {
        return false;
    };
    let mut rest = &rest[id_end..];
    rest = &rest[rest.chars().next().map_or(0, char::len_utf8)..];
    loop {
        if let Some(body) = rest.strip_prefix('"') {
            let Some(end) = body.find('"') else {
                return false;
            };
            rest = &body[end + 1..];
        } else if keyword_with_whitespace(rest, "href") {
            rest = &rest["href".len()..];
            rest = &rest[rest.chars().next().map_or(0, char::len_utf8)..];
        } else if keyword_with_whitespace(rest, "call") {
            let Some(open) = rest.find('(') else {
                return false;
            };
            let Some(close) = rest[open + 1..].find(')') else {
                return false;
            };
            rest = &rest[open + 1 + close + 1..];
        } else {
            return statement_direction(rest).is_some();
        }
    }
}

fn starts_accessibility_statement(value: &str) -> bool {
    ["accTitle", "accDescr"].into_iter().any(|keyword| {
        value.strip_prefix(keyword).is_some_and(|suffix| {
            let suffix = suffix.trim_start_matches(super::scan::is_ecmascript_whitespace);
            suffix.starts_with(':') || (keyword == "accDescr" && suffix.starts_with('{'))
        })
    })
}

fn direction_has_prior_token(value: &str) -> bool {
    starts_reserved_identifier(value)
        || starts_accessibility_statement(value)
        || value.starts_with('"')
        || keyword_suffix(value, "default").is_some()
        || ["click", "call", "href"]
            .into_iter()
            .any(|keyword| keyword_with_whitespace(value, keyword))
}

pub(crate) fn is_valid_editor_node_id(candidate: &str) -> bool {
    crate::diagrams::flowchart::is_valid_editor_node_id(candidate)
        && !starts_reserved_identifier(candidate)
}

fn statement_direction(statement: &str) -> Option<&'static str> {
    let prefix_end = statement
        .find(['\n', '\r', '\u{2028}', '\u{2029}'])
        .unwrap_or(statement.len());
    ["TB", "BT", "RL", "LR", "TD"]
        .into_iter()
        .find(|direction| {
            statement[..prefix_end]
                .char_indices()
                .rev()
                .any(|(offset, _)| {
                    let Some(suffix) = statement[offset..].strip_prefix("direction") else {
                        return false;
                    };
                    let value = suffix.trim_start_matches(super::scan::is_ecmascript_whitespace);
                    value.len() < suffix.len() && value.starts_with(direction)
                })
        })
}

fn is_identifier(value: &str) -> bool {
    !value.is_empty() && value.chars().all(|ch| !ch.is_whitespace() && ch != '"')
}

fn find_metadata_start(value: &str) -> Option<usize> {
    find_top_level_marker(value, "@{")
}

fn find_top_level_marker(value: &str, marker: &str) -> Option<usize> {
    let mut depth = 0usize;
    let mut quote = None;
    let mut escaped = false;
    let mut comment = false;
    for (index, ch) in value.char_indices() {
        if comment {
            comment = ch != '\n';
            continue;
        }
        if let Some(q) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' && q == '"' {
                escaped = true;
            } else if ch == q {
                quote = None;
            }
            continue;
        }
        if opens_quote(value, index) {
            quote = Some(ch);
            continue;
        }
        if depth > 0 && value[index..].starts_with("%%") && !value[index..].starts_with("%%{") {
            comment = true;
            continue;
        }
        if depth == 0 && value[index..].starts_with(marker) {
            return Some(index);
        }
        if matches!(ch, '[' | '(' | '{') {
            depth += 1;
        }
        if matches!(ch, ']' | ')' | '}') {
            depth = depth.saturating_sub(1);
        }
    }
    None
}

fn matching_brace(value: &str, start: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut quote = None;
    let mut escaped = false;
    for (index, ch) in value[start..].char_indices() {
        let index = start + index;
        if let Some(q) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' && q == '"' {
                escaped = true;
            } else if ch == q {
                quote = None;
            }
            continue;
        }
        if opens_quote(value, index) {
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

fn parse_metadata(
    body: &str,
    control: &OperationControl,
) -> OperationControlResult<std::result::Result<Map<String, Value>, String>> {
    let normalized = normalize_metadata_quoted_newlines(body);
    let body = normalized.as_str();
    let mut result = crate::inline_config::parse_mermaid_inline_object_controlled(body, control)?;
    if result.is_err() && body.contains('\n') {
        let stripped = strip_line_trailing_commas(body);
        if stripped != body
            && let Ok(value) =
                crate::inline_config::parse_mermaid_inline_object_controlled(&stripped, control)?
        {
            result = Ok(value);
        }
    }
    Ok(result.and_then(|value| match strip_prototype_keys(value) {
        Value::Null => Ok(Map::new()),
        Value::Object(object) => Ok(object),
        _ => Err("agentflow metadata must be an object".to_string()),
    }))
}

// The shapeDataStr lexer rewrites LF plus following ECMAScript whitespace
// before YAML decoding, including strings used for non-presentation metadata.
fn normalize_metadata_quoted_newlines(body: &str) -> String {
    let mut normalized = String::with_capacity(body.len());
    let mut double_quoted = false;
    let mut chars = body.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '"' {
            double_quoted = !double_quoted;
        }
        if double_quoted && ch == '\n' {
            normalized.push_str("<br/>");
            while chars
                .peek()
                .is_some_and(|ch| super::scan::is_ecmascript_whitespace(*ch))
            {
                chars.next();
            }
        } else {
            normalized.push(ch);
        }
    }
    normalized
}

/// Mermaid retries invalid block YAML once after removing syntactically trailing commas.
fn strip_line_trailing_commas(body: &str) -> String {
    let mut out = String::with_capacity(body.len());
    let mut quote = None;
    let mut flow_depth = 0usize;
    let mut scalar_indent = None;
    for (line_index, line) in body.split('\n').enumerate() {
        if line_index != 0 {
            out.push('\n');
        }
        let indent = line.len() - line.trim_start().len();
        if scalar_indent.is_some_and(|base| line.trim().is_empty() || indent > base) {
            out.push_str(line);
            continue;
        }
        scalar_indent = None;
        let mut escaped = false;
        let mut comment = line.len();
        for (index, ch) in line.char_indices() {
            if let Some(q) = quote {
                if escaped {
                    escaped = false;
                } else if ch == '\\' && q == '"' {
                    escaped = true;
                } else if ch == q {
                    quote = None;
                }
            } else if ch == '"' || ch == '\'' {
                quote = Some(ch);
            } else if ch == '#'
                && (index == 0 || matches!(line.as_bytes()[index - 1], b' ' | b'\t'))
            {
                comment = index;
                break;
            } else if matches!(ch, '[' | '{') {
                flow_depth += 1;
            } else if matches!(ch, ']' | '}') {
                flow_depth = flow_depth.saturating_sub(1);
            }
        }
        if quote.is_some() || flow_depth > 0 {
            out.push_str(line);
            continue;
        }
        let code = &line[..comment];
        let trimmed = code.trim_end_matches([' ', '\t']);
        if code.rsplit_once(':').is_some_and(|(_, value)| {
            let value = value.trim();
            value.starts_with(['|', '>'])
                && value[1..]
                    .chars()
                    .all(|ch| ch.is_ascii_digit() || ch == '+' || ch == '-')
        }) {
            scalar_indent = Some(indent);
            out.push_str(line);
        } else if let Some(code) = trimmed.strip_suffix(',') {
            out.push_str(code);
            out.push_str(&line[trimmed.len()..]);
        } else {
            out.push_str(line);
        }
    }
    out
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

fn unquote(value: &str) -> String {
    let value = value.trim();
    // Jison STR strips the delimiters without interpreting backslash escapes.
    value
        .strip_prefix('"')
        .and_then(|body| body.strip_suffix('"'))
        .unwrap_or(value)
        .to_string()
}

#[cfg(test)]
mod semantic_tests {
    use super::*;
    use serde_json::json;

    const SOURCE: &str = concat!(
        "agentflow-beta LR\n",
        "flow team[\"`**Team**`\"]\n",
        " A[\"`**Task**`\"]@{shape: task, instruction: run} -->|\"`**Call**`\"| B\n",
        "end\n",
        "connector api[API]\n",
        "B -.- api\n",
        "classDef hot fill:red\n",
        "class A hot\n",
        "style A stroke:blue\n",
        "click A href \"https://example.com\" \"Open\" _blank\n",
    );

    #[test]
    fn canonical_agentflow_capture_retains_success_and_failure_facts() {
        for (source, valid) in [
            ("agentflow-beta\nA --> B\n", true),
            ("agentflow-beta\nA --> B\nC ==> D\n", false),
        ] {
            let metadata = crate::SemanticMetadata {
                diagram_type: "agentflow".to_string(),
                config: crate::MermaidConfig::empty_object(),
                semantic_config: crate::MermaidConfig::empty_object(),
                title: None,
            };
            AGENTFLOW_SYNTAX_CONSTRUCTION_COUNT.set(0);
            crate::editor::test_work::reset();
            let parsed = parse_agentflow_semantic_model_controlled(
                source,
                &metadata,
                EditorFactsDemand::Capture,
                &OperationControl::new(),
            )
            .unwrap();
            assert_eq!(parsed.model.is_ok(), valid);
            let facts = parsed.editor_facts.expect("same-pass Agentflow facts");
            assert!(facts.symbols.iter().any(|symbol| symbol.name == "A"));
            assert_eq!(AGENTFLOW_SYNTAX_CONSTRUCTION_COUNT.get(), 1);
            assert_eq!(crate::editor::test_work::read().collections, 1);
            assert_eq!(
                facts.completeness,
                if valid {
                    crate::EditorSemanticCompleteness::Complete
                } else {
                    crate::EditorSemanticCompleteness::Recovered
                }
            );
            assert_eq!(facts.diagnostics.is_empty(), valid);
        }
    }

    #[test]
    fn canonical_agentflow_keeps_authored_facts_without_dom_or_editor_work() {
        crate::editor::test_work::reset();
        AGENTFLOW_SYNTAX_CONSTRUCTION_COUNT.set(0);
        let parsed = crate::SemanticParser::new()
            .parse_diagram_sync(SOURCE)
            .unwrap();
        let model = match parsed.model {
            crate::SemanticDiagramModel::Agentflow(model) => model,
            #[allow(unreachable_patterns)]
            _ => panic!("expected Agentflow semantics"),
        };
        assert_eq!(model.direction, "LR");
        assert_eq!(model.vertices[0].id, "A");
        assert_eq!(model.vertices[0].vertex_kind, AgentflowVertexKind::Task);
        assert_eq!(model.vertices[0].shape.as_deref(), Some("roundedRect"));
        assert_eq!(model.vertices[0].metadata["instruction"], "run");
        assert_eq!(model.vertices[0].parent_id.as_deref(), Some("team"));
        assert_eq!(
            model.edges[1].edge_semantic,
            AgentflowEdgeSemantic::Reference
        );
        assert_eq!(model.connectors[0].id, "api");
        assert_eq!(model.sub_graphs[0].nodes, ["B", "A"]);
        let authored = serde_json::to_value(&model.authored).unwrap();
        assert_eq!(authored["nodes"]["A"]["label_type"], "markdown");
        assert_eq!(
            authored["nodes"]["A"]["classes"],
            json!(["hot", "clickable"])
        );
        assert_eq!(authored["nodes"]["A"]["styles"], json!(["stroke:blue"]));
        assert_eq!(authored["nodes"]["A"]["link"], "https://example.com/");
        assert_eq!(authored["nodes"]["A"]["link_target"], "_blank");
        assert_eq!(authored["subgraph_label_types"]["team"], "markdown");
        assert!(
            authored["nodes"]
                .as_object()
                .unwrap()
                .values()
                .all(|node| node.get("dom_index").is_none())
        );
        assert_eq!(AGENTFLOW_SYNTAX_CONSTRUCTION_COUNT.get(), 1);
        assert_eq!(crate::editor::test_work::read(), Default::default());

        #[cfg(feature = "presentation")]
        {
            AGENTFLOW_SYNTAX_CONSTRUCTION_COUNT.set(0);
            let full = crate::Engine::new()
                .parse_diagram_for_render_model_with_type_sync(
                    "agentflow",
                    SOURCE,
                    crate::ParseOptions::strict(),
                )
                .unwrap()
                .unwrap();
            let crate::RenderSemanticModel::Agentflow(full) = full.model() else {
                panic!("expected full Agentflow model");
            };
            assert_eq!(
                serde_json::to_value(&model).unwrap(),
                serde_json::to_value(&full.model).unwrap()
            );
            assert_eq!(AGENTFLOW_SYNTAX_CONSTRUCTION_COUNT.get(), 1);
            assert_eq!(crate::editor::test_work::read(), Default::default());
        }
    }

    #[test]
    fn canonical_agentflow_preserves_security_limits_diagnostics_and_cancellation() {
        crate::editor::test_work::reset();
        let source = "agentflow-beta\nA((Circle))\nclick A call test(\"one\", two) \"Run\"\n";
        for (security, expected_callback) in [("strict", false), ("loose", true)] {
            let parser = crate::SemanticParser::new().with_site_config(
                crate::MermaidConfig::from_value(json!({"securityLevel": security})),
            );
            let parsed = parser.parse_diagram_sync(source).unwrap();
            let model = match parsed.model {
                crate::SemanticDiagramModel::Agentflow(model) => model,
                #[allow(unreachable_patterns)]
                _ => panic!("expected Agentflow semantics"),
            };
            assert_eq!(model.diagnostics[0].id, "SHAPE_REMOVED");
            assert!(model.diagnostics[0].position.is_some());
            assert!(model.warning_facts[0].span.is_some());
            assert_eq!(model.authored.nodes["A"].have_callback, expected_callback);
        }
        let source_override = format!("%%{{init: {{securityLevel: 'loose'}}}}%%\n{source}");
        let parsed = crate::SemanticParser::new()
            .parse_diagram_sync(&source_override)
            .unwrap();
        let model = match parsed.model {
            crate::SemanticDiagramModel::Agentflow(model) => model,
            #[allow(unreachable_patterns)]
            _ => panic!("expected Agentflow semantics"),
        };
        assert!(!model.authored.nodes["A"].have_callback);

        for source in [
            "agentflow-beta\nA ==> B\n",
            "agentflow-beta\nA@{shape: unknown}\n",
        ] {
            let error = crate::SemanticParser::new()
                .parse_diagram_sync(source)
                .unwrap_err();
            assert!(matches!(error, Error::DiagramParse { .. }));
            #[cfg(feature = "compat-json")]
            assert_eq!(
                error.to_string(),
                crate::Engine::new()
                    .parse_diagram_sync(source, crate::ParseOptions::strict())
                    .unwrap_err()
                    .to_string()
            );
        }
        let error = crate::SemanticParser::new()
            .with_site_config(crate::MermaidConfig::from_value(json!({"maxEdges": 1})))
            .parse_diagram_sync("agentflow-beta\nA --> B --> C\n")
            .unwrap_err();
        assert!(error.to_string().contains("Edge limit exceeded"));
        assert_eq!(crate::editor::test_work::read(), Default::default());
        let control = OperationControl::new();
        control.cancel();
        let error = crate::SemanticParser::new()
            .parse_diagram_controlled_sync(SOURCE, &control)
            .unwrap_err();
        assert_eq!(error.reason, crate::CancelReason::Requested);
    }

    #[cfg(feature = "presentation")]
    #[test]
    fn full_agentflow_serialization_keeps_presentation_shape_and_dom_ordinals() {
        let parsed = crate::Engine::new()
            .parse_diagram_for_render_model_with_type_sync(
                "agentflow",
                SOURCE,
                crate::ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let crate::RenderSemanticModel::Agentflow(model) = parsed.model() else {
            panic!("expected full Agentflow model");
        };
        let value = serde_json::to_value(model).unwrap();
        assert!(value.get("authored").is_none());
        assert!(value.get("model").is_none());
        assert!(value.get("visuals").is_none());
        assert_eq!(value["presentation"]["nodes"]["A"]["dom_index"], 0);
        assert_eq!(
            value["presentation"]["nodes"]["A"]["label_type"],
            "markdown"
        );
        let roundtrip: AgentflowDiagramRenderModel = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(serde_json::to_value(&roundtrip).unwrap(), value);
        let (flow, context) = roundtrip.to_flowchart_model();
        let node = flow.nodes.iter().find(|node| node.id == "A").unwrap();
        assert_eq!(node.classes, ["af-kind-task", "hot", "clickable"]);
        assert_eq!(node.styles, ["stroke:blue"]);
        assert_eq!(context.node_dom_index("A"), Some(0));
    }

    #[cfg(feature = "compat-json")]
    #[test]
    fn agentflow_observes_editor_facts_only_for_combined_request() {
        let meta = ParseMetadata {
            diagram_type: "agentflow".into(),
            config: crate::MermaidConfig::empty_object(),
            effective_config: crate::presentation::PresentationConfig::empty_object(),
            title: None,
        };
        crate::editor::test_work::reset();
        AGENTFLOW_SYNTAX_CONSTRUCTION_COUNT.set(0);
        let json = parse_agentflow(SOURCE, &meta).unwrap();
        assert!(json.get("authored").is_none());
        assert!(json.get("presentation").is_none());
        assert!(json["vertices"][0].get("parentId").is_none());
        assert_eq!(
            json["vertices"][0]["metadata"],
            json!({"instruction": "run"})
        );
        assert_eq!(AGENTFLOW_SYNTAX_CONSTRUCTION_COUNT.get(), 1);
        assert_eq!(crate::editor::test_work::read(), Default::default());
        AGENTFLOW_SYNTAX_CONSTRUCTION_COUNT.set(0);
        let combined =
            parse_agentflow_json_and_editor_facts(SOURCE, &meta, &OperationControl::new()).unwrap();
        let (combined_model, editor_facts, _) = combined.into_parts();
        assert_eq!(combined_model.unwrap().as_value(), &json);
        assert!(editor_facts.symbols.iter().any(|symbol| symbol.name == "A"));
        assert_eq!(crate::editor::test_work::read().collections, 1);
        assert_eq!(AGENTFLOW_SYNTAX_CONSTRUCTION_COUNT.get(), 1);
    }
}

#[cfg(all(test, feature = "compat-json"))]
mod tests {
    use super::*;
    use crate::{MermaidConfig, OperationControl};
    use serde_json::json;

    fn meta() -> ParseMetadata {
        ParseMetadata {
            diagram_type: "agentflow".to_string(),
            config: MermaidConfig::empty_object(),
            effective_config: crate::presentation::PresentationConfig::empty_object(),
            title: None,
        }
    }

    #[test]
    fn agentflow_direction_is_scoped_to_flow_and_consumes_its_physical_line() {
        let source = "agentflow-beta TB\ndirection LR\nflow F\ndirection RL; A --> B\nend\nC; D\n";
        let model = parse_agentflow(source, &meta()).unwrap();
        assert_eq!(model["direction"], "TB");
        assert_eq!(model["subGraphs"][0]["direction"], "RL");
        assert_eq!(model["subGraphs"][0]["nodes"], json!([]));
        assert_eq!(model["edges"], json!([]));
        assert_eq!(model["vertices"].as_array().unwrap().len(), 2);
        assert_eq!(model["vertices"][0]["id"], "C");
        assert_eq!(model["vertices"][1]["id"], "D");
    }

    #[test]
    fn agentflow_accessibility_lines_preserve_semicolons_and_percent_signs() {
        let source =
            "agentflow-beta TB\naccTitle: hello; world %% literal\naccDescr: describe; all\nA\n";
        let model = parse_agentflow(source, &meta()).unwrap();
        assert_eq!(model["accTitle"], "hello; world %% literal");
        assert_eq!(model["accDescr"], "describe; all");
        assert_eq!(model["vertices"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn agentflow_plain_quoted_labels_preserve_literal_backslashes() {
        let source = r#"agentflow-beta TB
flow F["C:\new"]
A["C:\new"] -->|"C:\new"| B
end
"#;
        let model = parse_agentflow(source, &meta()).unwrap();
        assert_eq!(model["vertices"][0]["label"], r"C:\new");
        assert_eq!(model["edges"][0]["label"], r"C:\new");
        assert_eq!(model["subGraphs"][0]["title"], r"C:\new");
    }

    #[test]
    fn agentflow_metadata_double_quoted_newlines_match_shape_data_lexer() {
        let source =
            "agentflow-beta TB\nA@{\n label: \"first\n second\",\n instruction: \"do\n next\"\n}\n";
        let model = parse_agentflow(source, &meta()).unwrap();
        assert_eq!(model["vertices"][0]["label"], "first<br/>second");
        assert_eq!(
            model["vertices"][0]["metadata"]["instruction"],
            "do<br/>next"
        );
        let source = "agentflow-beta TB\nA@{\n instruction: |\n  first\n  second\n}\n";
        let model = parse_agentflow(source, &meta()).unwrap();
        assert_eq!(
            model["vertices"][0]["metadata"]["instruction"],
            "first\nsecond\n"
        );
    }

    #[test]
    fn agentflow_markdown_labels_preserve_types_in_the_render_projection() {
        let source = r#"agentflow-beta TB
flow F["`**group**`"]
A["`**bold**`"] -->|"`**edge**`"| B["**plain**"]
B -- "`**split**`" --> C[plain]
end
connector K["`**connector**`"]
"#;
        let model =
            parse_agentflow_model_for_render_controlled(source, &meta(), &OperationControl::new())
                .unwrap()
                .unwrap();
        let (flow, _) = model.to_flowchart_model();
        for (id, label, kind) in [
            ("A", "**bold**", "markdown"),
            ("B", "**plain**", "string"),
            ("C", "plain", "text"),
            ("K", "**connector**", "text"),
        ] {
            let node = flow.nodes.iter().find(|node| node.id == id).unwrap();
            assert_eq!(node.label.as_deref(), Some(label), "{id}");
            assert_eq!(node.label_type.as_deref(), Some(kind), "{id}");
        }
        for (index, label) in ["**edge**", "**split**"].into_iter().enumerate() {
            assert_eq!(flow.edges[index].label.as_deref(), Some(label));
            assert_eq!(flow.edges[index].label_type.as_deref(), Some("markdown"));
        }
        assert_eq!(flow.subgraphs[0].title, "**group**");
        assert_eq!(flow.subgraphs[0].label_type.as_deref(), Some("markdown"));
        let semantic = parse_agentflow(source, &meta()).unwrap();
        assert_eq!(semantic["vertices"][0]["label"], "**bold**");
        for collection in ["vertices", "edges", "subGraphs", "connectors"] {
            assert!(
                semantic[collection]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|item| item.get("labelType").is_none())
            );
        }
    }

    #[test]
    fn agentflow_metadata_label_types_follow_upstream_default_and_update_rules() {
        let source = r#"agentflow-beta TB
Default@{label: "**default**"}
Text@{label: "**text**", labelType: text}
String@{label: "**string**", labelType: string}
Markdown@{label: "**markdown**", labelType: markdown}
Invalid@{label: "**invalid**", labelType: other}
Kept["`**kept**`"]
Kept@{label: "", labelType: text}
Kept@{labelType: string}
Default["plain"]
A -->|"`edge`"| B
L_A_B_0@{labelType: text}
flow Group["`**first**`"]
X
end
flow Group["**second**"]
Y
end
Group@{labelType: text}
"#;
        let model =
            parse_agentflow_model_for_render_controlled(source, &meta(), &OperationControl::new())
                .unwrap()
                .unwrap();
        let (flow, _) = model.to_flowchart_model();
        for (id, kind) in [
            ("Default", "string"),
            ("Text", "text"),
            ("String", "string"),
            ("Markdown", "markdown"),
            ("Invalid", "markdown"),
            ("Kept", "markdown"),
        ] {
            let node = flow.nodes.iter().find(|node| node.id == id).unwrap();
            assert_eq!(node.label_type.as_deref(), Some(kind), "{id}");
        }
        assert_eq!(
            flow.nodes
                .iter()
                .find(|node| node.id == "Kept")
                .unwrap()
                .label
                .as_deref(),
            Some("**kept**")
        );
        assert_eq!(flow.edges[0].label_type.as_deref(), Some("markdown"));
        assert_eq!(flow.subgraphs[0].title, "**second**");
        assert_eq!(flow.subgraphs[0].label_type.as_deref(), Some("markdown"));
    }

    #[test]
    fn agentflow_header_requires_a_separator_after_direction() {
        for source in [
            "agentflow-beta TB extra\nA\n",
            "agentflow-beta LR A --> B\n",
        ] {
            assert!(parse_agentflow(source, &meta()).is_err(), "{source}");
        }
        for source in [
            "agentflow-beta TB; A\n",
            "agentflow-beta TB %% comment\nA\n",
        ] {
            assert!(parse_agentflow(source, &meta()).is_ok(), "{source}");
        }
    }

    #[test]
    fn agentflow_accessibility_accepts_lexer_whitespace_before_colon() {
        let source = "agentflow-beta TB\naccTitle \t: Heading\naccDescr\u{3000}: direction LR describes the graph\nA\n";
        let model = parse_agentflow(source, &meta()).unwrap();
        assert_eq!(model["accTitle"], "Heading");
        assert_eq!(model["accDescr"], "direction LR describes the graph");
        assert_eq!(model["direction"], "TB");
    }

    #[test]
    fn agentflow_label_comments_preserve_quoted_percent_signs_and_source_spans() {
        let markdown = "[\"`one %% literal`\"]";
        assert_eq!(strip_label_comments(markdown), markdown);
        for (label, expected) in [
            ("[one %%comment\n two]", "one \n two"),
            ("[one %% ] --> X @{ ignored\n two]", "one \n two"),
            ("[\"one %% literal\"]", "one %% literal"),
        ] {
            let source = format!("agentflow-beta TB\nA{label}\n");
            let model = parse_agentflow(&source, &meta()).unwrap();
            assert_eq!(model["vertices"][0]["label"], expected, "{source}");
            assert_eq!(model["vertices"].as_array().unwrap().len(), 1, "{source}");
        }
        assert!(parse_agentflow("agentflow-beta TB\nA[one %% hidden close]\n", &meta()).is_err());
    }

    #[test]
    fn agentflow_reserved_identifiers_follow_ascii_keyword_boundaries() {
        for id in [
            "end-user",
            "end注文",
            "global-user",
            "style-guide",
            "flow-user",
            "connector-user",
        ] {
            for statement in [id.to_string(), format!("A --> {id}")] {
                assert!(
                    parse_agentflow(&format!("agentflow-beta TB\n{statement}\n"), &meta()).is_err(),
                    "{statement}"
                );
            }
        }
        for id in [
            "end_user",
            "endUser",
            "End-user",
            "global_user",
            "flowUser",
            "friend-end",
        ] {
            let parsed = parse_agentflow(&format!("agentflow-beta TB\n{id}\n"), &meta()).unwrap();
            assert_eq!(parsed["vertices"][0]["id"], id);
        }
    }

    #[test]
    fn agentflow_direction_statements_follow_ordered_jison_rules() {
        for (statement, expected) in [
            ("direction\tLR", "LR"),
            ("direction\u{3000}RL", "RL"),
            ("direction LRignored", "LR"),
            ("prefixdirection BT", "BT"),
            ("direction LR direction TB", "TB"),
        ] {
            let source = format!("agentflow-beta TB\nflow F\n{statement}\nA\nend\n");
            let model = parse_agentflow_model_for_render_controlled(
                &source,
                &meta(),
                &OperationControl::new(),
            )
            .unwrap()
            .unwrap();
            assert_eq!(
                model.sub_graphs[0].direction.as_deref(),
                Some(expected),
                "{statement}"
            );
        }
    }

    #[test]
    fn direction_does_not_hide_higher_priority_agentflow_statements() {
        for statement in [
            r#"connector C["direction LR"]"#,
            "classDef c fill:direction LR",
            "style A fill:direction LR",
            "global direction LR",
            r#"click A href "https://example.test" "direction LR""#,
            r#"default["direction LR"]"#,
            r#""direction LR""#,
        ] {
            let source = format!("agentflow-beta TB\n{statement}\n");
            assert!(parse_agentflow(&source, &meta()).is_err(), "{source}");
        }
    }

    #[test]
    fn direction_after_end_applies_to_the_remaining_context() {
        let source = "agentflow-beta TB\nflow Outer\nflow Inner\nA\nend direction LR\nend\n";
        let model =
            parse_agentflow_model_for_render_controlled(source, &meta(), &OperationControl::new())
                .unwrap()
                .unwrap();
        assert_eq!(
            model
                .sub_graphs
                .iter()
                .find(|graph| graph.id == "Outer")
                .unwrap()
                .direction
                .as_deref(),
            Some("LR")
        );
        assert_eq!(
            model
                .sub_graphs
                .iter()
                .find(|graph| graph.id == "Inner")
                .unwrap()
                .direction
                .as_deref(),
            None
        );
    }

    #[test]
    fn consecutive_end_tokens_close_each_container_before_direction() {
        let source = "agentflow-beta TB\nflow Outer\nflow Inner\nA\nend end direction LR\n";
        let model =
            parse_agentflow_model_for_render_controlled(source, &meta(), &OperationControl::new())
                .unwrap()
                .unwrap();
        assert_eq!(model.sub_graphs.len(), 2);
        assert_eq!(
            model
                .sub_graphs
                .iter()
                .find(|graph| graph.id == "Inner")
                .unwrap()
                .direction,
            None
        );
        assert!(parse_agentflow(&format!("{source}end\n"), &meta()).is_err());
    }

    #[test]
    fn click_direction_text_preserves_the_link_instead_of_becoming_a_direction() {
        let source = "agentflow-beta TB\nA\nclick A href \"https://example.test/direction LR\"\n";
        let model =
            parse_agentflow_model_for_render_controlled(source, &meta(), &OperationControl::new())
                .unwrap()
                .unwrap();
        let (flow, _) = model.to_flowchart_model();
        assert_eq!(
            flow.nodes[0].link.as_deref(),
            Some("https://example.test/direction%20LR")
        );
    }

    #[test]
    fn indented_end_direction_preserves_the_open_container() {
        for indent in ["  ", "\u{00a0}"] {
            let source = format!("agentflow-beta TB\nflow O\nA\n{indent}end direction LR\nend\n");
            let model = parse_agentflow_model_for_render_controlled(
                &source,
                &meta(),
                &OperationControl::new(),
            )
            .unwrap()
            .unwrap();
            assert_eq!(model.sub_graphs.len(), 1, "{source}");
            assert_eq!(model.sub_graphs[0].id, "O");
            assert_eq!(
                model.sub_graphs[0].direction.as_deref(),
                Some("LR"),
                "{source}"
            );
            assert_eq!(model.vertices[0].id, "A");
        }
    }

    #[test]
    fn indented_direction_precedes_click_and_classdef_keywords() {
        for statement in [
            r#"  click A href "https://example.test/direction LR""#,
            "  classDef c fill:direction LR",
        ] {
            let source = format!("agentflow-beta TB\nA\n{statement}\n");
            let model = parse_agentflow_model_for_render_controlled(
                &source,
                &meta(),
                &OperationControl::new(),
            )
            .unwrap()
            .unwrap();
            let (flow, _) = model.to_flowchart_model();
            assert_eq!(flow.nodes.len(), 1, "{source}");
            assert_eq!(flow.nodes[0].id, "A");
            assert_eq!(flow.nodes[0].link, None, "{source}");
            assert!(flow.class_defs.is_empty(), "{source}");
        }
    }

    #[test]
    fn editor_edge_endpoints_declare_entities_once_and_preserve_occurrence_spans() {
        use crate::EditorSemanticRole::{Entity, Reference};

        let source = "agentflow-beta\nflow Team\nA[Worker]-->B-->A-->A\nend\n";
        let metadata = meta();
        let control = OperationControl::new();
        let mut parser = Parser::new(
            source,
            metadata.effective_config.as_config(),
            metadata.title.as_deref(),
            EditorFactsDemand::Capture,
            false,
            &control,
        );
        parser.parse().unwrap().unwrap();
        let symbols = &parser.facts.as_ref().unwrap().symbols;
        assert_eq!(
            symbols
                .iter()
                .map(|symbol| (symbol.name.as_str(), symbol.role))
                .collect::<Vec<_>>(),
            vec![
                ("Team", Entity),
                ("A", Entity),
                ("B", Entity),
                ("A", Reference),
                ("A", Reference),
            ]
        );
        let edge_start = source.find("A[Worker]").unwrap();
        assert_eq!(
            symbols[1..]
                .iter()
                .map(|symbol| symbol.selection)
                .collect::<Vec<_>>(),
            [0, 12, 16, 20]
                .map(|offset| SourceSpan::new(edge_start + offset, edge_start + offset + 1))
        );
        for symbol in symbols {
            assert_eq!(
                &source[symbol.selection.start..symbol.selection.end],
                symbol.name
            );
        }
    }

    #[test]
    fn editor_container_identifiers_do_not_select_keyword_prefixes_or_duplicate_symbols() {
        use crate::EditorSemanticRole::{Entity, Reference};

        let source = "agentflow-beta\nflow f[f]\nconnector c[c]\nc-->f\nend\nf@{view: collapsed}\n";
        let metadata = meta();
        let control = OperationControl::new();
        let mut parser = Parser::new(
            source,
            metadata.effective_config.as_config(),
            metadata.title.as_deref(),
            EditorFactsDemand::Capture,
            false,
            &control,
        );
        parser.parse().unwrap().unwrap();
        let symbols = &parser.facts.as_ref().unwrap().symbols;
        assert_eq!(
            symbols
                .iter()
                .map(|symbol| (symbol.name.as_str(), symbol.role))
                .collect::<Vec<_>>(),
            vec![
                ("f", Entity),
                ("c", Entity),
                ("c", Reference),
                ("f", Reference),
                ("f", Reference),
            ]
        );
        assert_eq!(symbols[0].selection.start, source.find("f[f]").unwrap());
        assert_eq!(symbols[1].selection.start, source.find("c[c]").unwrap());
        assert_eq!(symbols[4].kind, EditorSemanticKind::Namespace);
        for symbol in symbols {
            assert_eq!(
                &source[symbol.selection.start..symbol.selection.end],
                symbol.name
            );
        }
    }

    #[test]
    fn editor_presentation_symbols_use_lexer_spans_and_separate_class_definitions() {
        use crate::EditorSemanticRole::{ClassDefinition, Entity, Reference};

        let source = "agentflow-beta\nclassDef c fill:red\nstyle s fill:blue\nc-->s\nclass c,c c\nclick c,c \"https://example.com\"\n";
        let metadata = meta();
        let control = OperationControl::new();
        let mut parser = Parser::new(
            source,
            metadata.effective_config.as_config(),
            metadata.title.as_deref(),
            EditorFactsDemand::Capture,
            false,
            &control,
        );
        parser.parse().unwrap().unwrap();
        let symbols = &parser.facts.as_ref().unwrap().symbols;
        assert_eq!(
            symbols
                .iter()
                .map(|symbol| (symbol.name.as_str(), symbol.role))
                .collect::<Vec<_>>(),
            vec![
                ("c", ClassDefinition),
                ("s", Entity),
                ("c", Entity),
                ("s", Reference),
                ("c", Reference),
                ("c", Reference),
                ("c", Reference),
                ("c", Reference),
            ]
        );
        let expected_starts = [
            source.find("c fill:red").unwrap(),
            source.find("s fill:blue").unwrap(),
            source.find("c-->s").unwrap(),
            source.find("c-->s").unwrap() + 4,
            source.find("class c,c").unwrap() + 6,
            source.find("class c,c").unwrap() + 8,
            source.find("click c,c").unwrap() + 6,
            source.find("click c,c").unwrap() + 8,
        ];
        for (symbol, start) in symbols.iter().zip(expected_starts) {
            assert_eq!(symbol.selection, SourceSpan::new(start, start + 1));
            assert_eq!(&source[start..start + 1], symbol.name);
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
        assert_eq!(model.sub_graphs[0].nodes, vec!["task"]);
        assert_eq!(model.sub_graphs[1].nodes, vec!["inner"]);
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
        let (flow, _) = model.to_flowchart_model();
        assert!(flow.edges[0].is_user_defined_id);
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
    fn statement_separators_preserve_quoted_and_metadata_content() {
        let model = parse_agentflow_model_for_render_controlled(
            "agentflow-beta LR; flow f[Worker]; a[\"甲; %% literal\"]@{instruction: \"read; write\"}; a --> b; end; %% ignored; c\nconnector api[API]; b --> api",
            &meta(), &OperationControl::new(),
        ).unwrap().unwrap();
        assert_eq!(model.vertices.len(), 2);
        assert_eq!(model.vertices[0].label.as_deref(), Some("甲; %% literal"));
        assert_eq!(model.vertices[0].metadata["instruction"], "read; write");
        assert_eq!(model.sub_graphs[0].nodes, ["a", "b"]);
        assert_eq!(model.edges.len(), 2);
        assert_eq!(model.connectors[0].id, "api");
    }

    #[test]
    fn endpoint_groups_expand_in_order_with_one_explicit_id() {
        let model = parse_agentflow_model_for_render_controlled(
            "agentflow-beta TD; a[A & label] & b 边@---> c & d next@--x e; c -..- |reference| e",
            &meta(),
            &OperationControl::new(),
        )
        .unwrap()
        .unwrap();
        let links = model
            .edges
            .iter()
            .map(|edge| (edge.start.as_str(), edge.end.as_str()))
            .collect::<Vec<_>>();
        assert_eq!(
            links,
            [
                ("a", "c"),
                ("a", "d"),
                ("b", "c"),
                ("b", "d"),
                ("c", "e"),
                ("d", "e"),
                ("c", "e")
            ]
        );
        assert_eq!(model.edges[2].id.as_deref(), Some("边"));
        assert_eq!(model.edges[5].id.as_deref(), Some("next"));
        assert_eq!(model.edges[0].length, 2);
        assert_eq!(model.edges[6].length, 2);
        assert_eq!(model.edges[6].label.as_deref(), Some("reference"));
        assert_eq!(model.direction, "TB");
        assert_eq!(model.vertices[0].label.as_deref(), Some("A & label"));
        let (flow, _) = model.to_flowchart_model();
        assert_eq!(
            flow.edges
                .iter()
                .filter(|edge| edge.is_user_defined_id)
                .count(),
            2
        );
    }

    #[test]
    fn endpoint_expansion_obeys_the_secure_edge_limit() {
        let mut meta = meta();
        meta.effective_config =
            crate::presentation::PresentationConfig::from_value(json!({"maxEdges": 3}));
        let result = parse_agentflow_model_for_render_controlled(
            "agentflow-beta\na & b --> c & d",
            &meta,
            &OperationControl::new(),
        )
        .unwrap();
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Edge limit exceeded")
        );
    }

    #[test]
    fn metadata_uses_yaml_nesting_and_preserves_literal_commas() {
        let model = parse_agentflow_model_for_render_controlled(
            r#"agentflow-beta
a[Don't stop]@{
  shape: task,
  instruction: "don't \"stop\"", # trailing comma
  details:
    retries: 3
    enabled: true
  lines: |
    keep, commas,
}
a --> b
"#,
            &meta(),
            &OperationControl::new(),
        )
        .unwrap()
        .unwrap();
        let node = &model.vertices[0];
        assert_eq!(node.label.as_deref(), Some("Don't stop"));
        assert_eq!(node.metadata["instruction"], "don't \"stop\"");
        assert_eq!(
            node.metadata["details"],
            json!({"retries": 3, "enabled": true})
        );
        assert_eq!(node.metadata["lines"], "keep, commas,\n");
        assert_eq!(model.edges.len(), 1);
    }

    #[test]
    fn parser_retains_upstream_vertex_registration_ordinals() {
        // Replayed with the pinned Mermaid 12 AgentflowDB, including Jison's delayed metadata
        // reductions and connector replacement. Final edges cannot reconstruct these ordinals.
        let cases: &[(&str, &[(&str, usize)])] = &[
            (
                "agentflow-beta\nA\nA\nB --> C --> D\n",
                &[("A", 0), ("B", 2), ("C", 3), ("D", 4)],
            ),
            (
                "agentflow-beta\nA@{shape: task} & B@{shape: tool} --> C & D\nE\n",
                &[("A", 0), ("B", 1), ("C", 4), ("D", 5), ("E", 6)],
            ),
            (
                "agentflow-beta\nA[Old]@{shape: task}\nstyle A fill:red\nconnector A[API]\nconnector A[Again]\nA@{instruction: call}\nB\n",
                &[("A", 3), ("B", 5)],
            ),
            (
                "agentflow-beta\nflow F\n A --> B\nend\nF@{view: collapsed}\nA e@--> B\ne@{animate: true}\nC\n",
                &[("A", 0), ("B", 1), ("F", 2), ("C", 5)],
            ),
            (
                "agentflow-beta\nconnector api@{instruction: call}\nstyle X fill:red\napi --> Y\nZ\n",
                &[("api", 0), ("X", 1), ("Y", 3), ("Z", 4)],
            ),
        ];
        for (source, expected) in cases {
            let model = parse_agentflow_model_for_render_controlled(
                source,
                &meta(),
                &OperationControl::new(),
            )
            .unwrap()
            .unwrap();
            let serialized = serde_json::to_value(&model).unwrap();
            let model: AgentflowDiagramRenderModel = serde_json::from_value(serialized).unwrap();
            let (_, context) = model.to_flowchart_model();
            for &(id, index) in *expected {
                assert_eq!(context.node_dom_index(id), Some(index), "{source}: {id}");
            }
        }
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
        let (flow, context) = model.to_flowchart_model();
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
            "agentflow-beta\na((Circle))\nb@{ shape: cloud }\nc@{shape: task}\n",
            &meta(),
            &OperationControl::new(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(model.vertices[0].shape.as_deref(), Some("circle"));
        assert_eq!(model.diagnostics[0].id, "SHAPE_REMOVED");
        assert_eq!(model.diagnostics[1].id, "SHAPE_UNSUPPORTED");
        assert_eq!(model.diagnostics.len(), 2);
        let (flow, _) = model.to_flowchart_model();
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
        let (flow, _) = model.to_flowchart_model();
        assert_eq!(flow.edges.len(), 2);
        assert_eq!((&*flow.edges[0].from, &*flow.edges[0].to), ("f", "f"));
        assert_eq!(
            (&*flow.edges[1].from, &*flow.edges[1].to),
            ("f", "external")
        );
        assert_eq!(flow.nodes[0].classes, ["af-kind-task"]);
    }
    #[test]
    fn metadata_shape_validation_rejects_unknown_names_at_the_authored_block() {
        for shape in ["unknown", "roundedRect", "lean_right", "123", "true", "[]"] {
            let source = format!("agentflow-beta\n甲@{{ shape: {shape} }}\n");
            let failure = construct(&source, &meta(), &OperationControl::new())
                .unwrap()
                .err()
                .unwrap();
            let (error, _) = failure.into_parts();
            assert!(
                error.to_string().contains("No such shape:"),
                "{shape}: {error}"
            );
        }
        let source = "agentflow-beta\n甲@{ shape: unknown }\n";
        let metadata = meta();
        let control = OperationControl::new();
        let mut parser = Parser::new(
            source,
            metadata.effective_config.as_config(),
            metadata.title.as_deref(),
            EditorFactsDemand::Capture,
            false,
            &control,
        );
        let Err(ParseFailure::Syntax { span, .. }) = parser.parse().unwrap() else {
            panic!("unknown shape must fail with a source span");
        };
        assert_eq!(&source[span.start..span.end], "@{ shape: unknown }");
    }

    #[test]
    fn shape_aliases_are_validated_before_resolution_and_container_metadata_is_not_a_vertex() {
        for (authored, resolved) in [
            ("task", "roundedRect"),
            ("tool", "subroutine"),
            ("input", "lean-right"),
            ("round", "rect"),
        ] {
            let source = format!("agentflow-beta\na@{{ shape: {authored} }}\n");
            let model = parse_agentflow(&source, &meta()).unwrap();
            assert_eq!(model["vertices"][0]["shape"], resolved);
        }
        let model = parse_agentflow_model_for_render_controlled(
            "agentflow-beta\nflow f@{ shape: unknown }\n a[Task]\nend\nf@{ shape: 123 } --> b\n",
            &meta(),
            &OperationControl::new(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(model.sub_graphs[0].metadata["shape"], 123);
        assert_eq!(model.vertices[0].shape.as_deref(), Some("square"));
        for shape in ["null", "false", "0", "''"] {
            assert!(
                parse_agentflow(
                    &format!("agentflow-beta\na@{{ shape: {shape} }}\n"),
                    &meta()
                )
                .is_ok()
            );
        }
    }

    #[test]
    fn containers_complete_inside_out_and_keep_preorder_colors() {
        let model = parse_agentflow_model_for_render_controlled(
            "agentflow-beta LR\nflow outer[Outer]\n a\n flow\n  a --> b\n end\n flow inner\n  c\n end\nend\nflow\n d\nend\n",
            &meta(), &OperationControl::new(),
        ).unwrap().unwrap();
        assert_eq!(
            model
                .sub_graphs
                .iter()
                .map(|graph| graph.id.as_str())
                .collect::<Vec<_>>(),
            ["subGraph0", "inner", "outer", "subGraph3"]
        );
        assert_eq!(model.sub_graphs[0].nodes, ["b", "a"]);
        assert_eq!(model.sub_graphs[2].nodes, ["subGraph0", "inner"]);
        assert_eq!(model.sub_graphs[0].title.as_deref(), Some(""));
        assert_eq!(model.vertices[0].parent_id.as_deref(), Some("subGraph0"));
        let (_, context) = model.to_flowchart_model();
        for (ordinal, id) in ["outer", "subGraph0", "inner", "subGraph3"]
            .into_iter()
            .enumerate()
        {
            assert_eq!(context.subgraph_color_ordinal(id), Some(ordinal));
        }
    }

    #[test]
    fn duplicate_containers_merge_at_completion_and_increment_anonymous_ids() {
        let model = parse_agentflow_model_for_render_controlled(
            "agentflow-beta\nflow same[First]\n a\nend\nflow same\n direction LR\n b\nend\nflow\n c\nend\n",
            &meta(), &OperationControl::new(),
        ).unwrap().unwrap();
        assert_eq!(model.sub_graphs.len(), 2);
        assert_eq!(model.sub_graphs[0].nodes, ["a", "b"]);
        assert_eq!(model.sub_graphs[0].title.as_deref(), Some("First"));
        assert_eq!(model.sub_graphs[0].direction.as_deref(), Some("LR"));
        assert_eq!(model.sub_graphs[1].id, "subGraph2");
    }

    #[test]
    fn global_blocks_exempt_direct_members_without_exempting_nested_children() {
        let model = parse_agentflow_model_for_render_controlled(
            "agentflow-beta\nflow outer\n global\n  flow inner\n   a\n  end\n  b\n end\n a --> b\nend\n",
            &meta(), &OperationControl::new(),
        ).unwrap().unwrap();
        assert_eq!(model.sub_graphs[0].id, "inner");
        assert_eq!(model.sub_graphs[0].nodes, ["a"]);
        assert!(model.sub_graphs[1].nodes.is_empty());
        assert_eq!(model.vertices[0].parent_id.as_deref(), Some("inner"));
        assert_eq!(model.vertices[1].parent_id, None);
    }

    #[test]
    fn edge_chains_prepend_target_groups_to_container_membership() {
        let model = parse_agentflow_model_for_render_controlled(
            "agentflow-beta\nflow f\n a & b --> c & d --> e\nend\n",
            &meta(),
            &OperationControl::new(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(model.sub_graphs[0].nodes, ["e", "c", "d", "a", "b"]);
    }

    #[test]
    fn endpoint_metadata_uses_the_same_dispatch_as_standalone_declarations() {
        let model = parse_agentflow_model_for_render_controlled(
            "agentflow-beta\nflow f\n a\nend\nconnector api[API]\nf@{ view: collapsed } --> api@{ instruction: call }\n",
            &meta(), &OperationControl::new(),
        ).unwrap().unwrap();
        assert_eq!(model.sub_graphs[0].metadata["view"], "collapsed");
        assert_eq!(model.connectors[0].metadata["instruction"], "call");
        let (_, context) = model.to_flowchart_model();
        assert!(context.is_subgraph_collapsed("f"));
        for source in [
            "agentflow-beta\na@{ instruction: first } b@{ instruction: second }\n",
            "agentflow-beta\na@{ instruction: run } garbage\n",
        ] {
            assert!(parse_agentflow(source, &meta()).is_err());
        }
    }

    #[test]
    fn multiline_accessibility_description_does_not_create_a_vertex() {
        let model = parse_agentflow_model_for_render_controlled(
            "agentflow-beta\naccDescr {\n This diagram describes a workflow: [draft; \"pending\n}\na --> b\n",
            &meta(),
            &OperationControl::new(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            model.acc_descr.as_deref(),
            Some("This diagram describes a workflow: [draft; \"pending")
        );
        assert_eq!(model.vertices.len(), 2);
    }

    #[test]
    fn containment_cycles_keep_semantics_and_drop_only_the_render_cycle() {
        let model = parse_agentflow_model_for_render_controlled(
            "agentflow-beta\nflow A\n a --> B\nend\nflow B\n b --> A\nend\n",
            &meta(),
            &OperationControl::new(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(model.sub_graphs[0].nodes, ["B", "a"]);
        assert_eq!(model.sub_graphs[1].nodes, ["A", "b"]);
        assert_eq!(model.diagnostics.len(), 1);
        assert_eq!(model.diagnostics[0].id, "CONTAINMENT_VIOLATION");
        assert_eq!(model.diagnostics[0].node_id, "B");
        let (flow, context) = model.to_flowchart_model();
        assert_eq!(flow.subgraphs[0].nodes, ["a"]);
        assert_eq!(flow.subgraphs[1].nodes, ["A", "b"]);
        assert_eq!(context.subgraph_color_ordinal("B"), Some(0));
        assert_eq!(context.subgraph_color_ordinal("A"), Some(1));
        let mut collapsed = model;
        for graph in &mut collapsed.sub_graphs {
            graph.metadata.insert("view".into(), json!("collapsed"));
        }
        let (_, context) = collapsed.to_flowchart_model();
        assert_eq!(context.collapsed_replacement("A"), None);
        assert_eq!(context.collapsed_replacement("B"), Some("A"));
        assert_eq!(context.collapsed_replacement("a"), Some("A"));
        assert_eq!(context.collapsed_replacement("b"), Some("A"));
    }

    #[test]
    fn presentation_directives_preserve_order_without_changing_domain_membership() {
        let source = "agentflow-beta\nclass a hot\nclassDef hot fill:#ff0000,color:#123456\nclassDef hot stroke:#0000ff\nflow f[Worker]\n a[Task]:::hot --> b\n style ghost fill:#00ff00\nend\nclass f hot\nstyle a stroke-width:3px\nstyle a fill:#ffff00\nclick a href \"https://example.com\" \"Open task\" _blank\n";
        let model =
            parse_agentflow_model_for_render_controlled(source, &meta(), &OperationControl::new())
                .unwrap()
                .unwrap();
        assert_eq!(model.sub_graphs[0].nodes, ["b", "a"]);
        assert!(
            model
                .vertices
                .iter()
                .any(|node| node.id == "ghost" && node.parent_id.is_none())
        );
        let semantic = render_model_to_compat_json(&model, &meta()).unwrap();
        assert!(semantic.get("presentation").is_none());
        let (flow, _) = model.to_flowchart_model();
        assert_eq!(
            flow.class_defs["hot"],
            ["fill:#ff0000", "color:#123456", "stroke:#0000ff"]
        );
        let node = flow.nodes.iter().find(|node| node.id == "a").unwrap();
        assert_eq!(node.classes, ["af-kind-task", "hot", "clickable"]);
        assert_eq!(node.styles, ["stroke-width:3px", "fill:#ffff00"]);
        assert_eq!(node.link.as_deref(), Some("https://example.com/"));
        assert_eq!(node.link_target.as_deref(), Some("_blank"));
        assert_eq!(flow.tooltips["a"], "Open task");
        assert_eq!(flow.subgraphs[0].classes, ["hot"]);
    }

    #[test]
    fn edge_style_defaults_and_metadata_follow_source_mutation_order() {
        let model = parse_agentflow_model_for_render_controlled(
            "agentflow-beta\na e@--> b\nlinkStyle default stroke:#ff0000\nlinkStyle 0 interpolate basis stroke:#0000ff\ne@{ animate: true, animation: fast, curve: linear }\nclassDef marked stroke-width:4px\nclass e marked\n",
            &meta(), &OperationControl::new(),
        ).unwrap().unwrap();
        let (flow, _) = model.to_flowchart_model();
        assert_eq!(
            flow.edge_defaults.as_ref().unwrap().style,
            ["stroke:#ff0000"]
        );
        assert_eq!(flow.edges[0].style, ["stroke:#0000ff", "fill:none"]);
        assert_eq!(flow.edges[0].classes, ["marked"]);
        assert_eq!(flow.edges[0].interpolate.as_deref(), Some("linear"));
        assert_eq!(flow.edges[0].animate, Some(true));
        assert_eq!(flow.edges[0].animation.as_deref(), Some("fast"));
        assert_eq!(model.edges[0].metadata["curve"], "linear");
        assert!(
            parse_agentflow("agentflow-beta\na --> b\nlinkStyle 1 stroke:red\n", &meta()).is_err()
        );
    }

    #[test]
    fn first_connector_declaration_replaces_vertex_state_and_later_declarations_preserve_it() {
        let model = parse_agentflow_model_for_render_controlled(
            "agentflow-beta\na[Old]:::red@{ shape: cloud, instruction: old }\nstyle a fill:red\nconnector a[API]\nclass a blue\nconnector a[Updated]\na[Final]@{ instruction: call }\n",
            &meta(), &OperationControl::new(),
        ).unwrap().unwrap();
        assert!(model.vertices.is_empty());
        assert_eq!(model.connectors[0].title.as_deref(), Some("Final"));
        assert_eq!(
            model.connectors[0].metadata,
            serde_json::from_value(json!({"instruction":"call"})).unwrap()
        );
        let (flow, _) = model.to_flowchart_model();
        assert_eq!(flow.nodes[0].classes, ["af-kind-connector", "blue"]);
        assert!(flow.nodes[0].styles.is_empty());
        for source in [
            "agentflow-beta\na:::red[ignored]\n",
            "agentflow-beta\na\nclass a red[ignored]\n",
        ] {
            assert!(parse_agentflow(source, &meta()).is_err());
        }
    }

    #[test]
    fn default_curve_is_captured_when_an_edge_is_created() {
        let model = parse_agentflow_model_for_render_controlled(
            "agentflow-beta\nlinkStyle default interpolate basis\na --> b\nlinkStyle default interpolate linear\nb --> c\n",
            &meta(), &OperationControl::new(),
        ).unwrap().unwrap();
        let (flow, _) = model.to_flowchart_model();
        assert_eq!(flow.edges[0].interpolate.as_deref(), Some("basis"));
        assert_eq!(flow.edges[1].interpolate.as_deref(), Some("linear"));
    }

    #[test]
    fn container_vertex_styles_override_container_classes_and_inline_groups_keep_classes() {
        let model = parse_agentflow_model_for_render_controlled(
            "agentflow-beta\nflow f\n a:::red & b:::blue --> c:::green\nend\nclassDef first fill:red\nclass f first\nstyle f stroke:blue\n",
            &meta(), &OperationControl::new(),
        ).unwrap().unwrap();
        let (flow, _) = model.to_flowchart_model();
        assert_eq!(flow.subgraphs[0].styles, ["stroke:blue"]);
        assert!(flow.subgraphs[0].classes.is_empty());
        for (node, class) in flow.nodes.iter().zip(["red", "blue", "green"]) {
            assert_eq!(node.classes[1], class);
        }
        assert!(model.vertices.iter().all(|node| node.id != "f"));
    }

    #[test]
    fn click_callbacks_respect_security_and_directives_reject_unconsumed_content() {
        for (security, callback) in [("strict", false), ("loose", true)] {
            let mut metadata = meta();
            metadata.effective_config = crate::presentation::PresentationConfig::from_value(
                json!({"securityLevel":security}),
            );
            let model = parse_agentflow_model_for_render_controlled(
                "agentflow-beta\na\nclick a call test(\"one\", two) \"Run\"\nclick unknown \"https://example.com\"\n",
                &metadata, &OperationControl::new(),
            ).unwrap().unwrap();
            let (flow, _) = model.to_flowchart_model();
            assert_eq!(flow.nodes.len(), 1);
            assert_eq!(flow.nodes[0].have_callback, callback);
            assert_eq!(flow.tooltips["a"], "Run");
        }
        for statement in [
            "classDef hot",
            "classDef hot[ignored] fill:red",
            "style a[ignored] fill:red",
            "style a",
            "linkStyle default",
            "class a hot garbage",
            "click a \"https://example.com\" garbage",
            "click a callback \"tip\" garbage",
            "click a \"https://example.com\" _unknown",
        ] {
            assert!(
                parse_agentflow(&format!("agentflow-beta\na\n{statement}\n"), &meta()).is_err(),
                "{statement}"
            );
        }
    }

    #[test]
    fn invalid_edges_and_unterminated_labels_do_not_silently_drop_source() {
        for source in ["agentflow-beta\na ==> b\n", "agentflow-beta\na[missing\n"] {
            assert!(parse_agentflow(source, &meta()).is_err());
        }
    }
}
