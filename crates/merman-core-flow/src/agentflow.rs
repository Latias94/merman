//! Agentflow statement scanning, validation, and semantic model construction.
//!
//! Agentflow shares presentation tokens with Flowchart and replays its own shapes,
//! containers, metadata, and edge operators.

use merman_core_preprocess::yaml::parse_mermaid_inline_yaml_controlled;
use merman_model::families::agentflow::{
    ALLOWED_SHAPES, AgentflowAuthoredStyles, AgentflowConnector, AgentflowDiagnostic,
    AgentflowDiagnosticPosition, AgentflowDiagramModel, AgentflowEdge, AgentflowEdgeSemantic,
    AgentflowNode, AgentflowSubGraph, AgentflowVertexKind,
};
use merman_model::families::flowchart::shapes;
use merman_model::text::is_ecmascript_whitespace;
use merman_model::warnings::DiagramWarningFact;
use merman_model::{
    MermaidConfig, OperationCancelled, OperationControl, OperationControlResult, SourceSpan,
};
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};

mod presentation;

pub struct ParsedAgentflow {
    pub model: AgentflowDiagramModel,
    #[cfg(feature = "presentation")]
    pub node_dom_indices: Option<HashMap<String, usize>>,
}

pub struct AgentflowSyntaxIssue {
    pub message: String,
    pub span: SourceSpan,
}

#[cfg(feature = "editor-facts")]
pub enum AgentflowObservation<'a> {
    Started,
    Header(SourceSpan),
    Identifier {
        name: &'a str,
        span: SourceSpan,
        kind: AgentflowIdentifierKind,
    },
}

#[cfg(feature = "editor-facts")]
pub enum AgentflowIdentifierKind {
    NodeDeclaration,
    NodeReference,
    NodeReferenceIfDeclared,
    ContainerDeclaration,
    ContainerReference,
    ClassDefinition,
}

#[cfg(any(test, feature = "test-support"))]
thread_local! {
    static AGENTFLOW_SYNTAX_CONSTRUCTION_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(any(test, feature = "test-support"))]
pub fn reset_agentflow_syntax_construction_count() {
    AGENTFLOW_SYNTAX_CONSTRUCTION_COUNT.set(0);
}

#[cfg(any(test, feature = "test-support"))]
pub fn agentflow_syntax_construction_count() -> usize {
    AGENTFLOW_SYNTAX_CONSTRUCTION_COUNT.get()
}

#[derive(Debug)]
enum ParseFailure {
    Syntax { message: String, span: SourceSpan },
    Cancelled(OperationCancelled),
}

/// Parses a preprocessed Agentflow source with its admitted configuration.
pub fn parse_agentflow_source_controlled(
    code: &str,
    config: &MermaidConfig,
    title: Option<&str>,
    control: &OperationControl,
) -> OperationControlResult<Result<ParsedAgentflow, AgentflowSyntaxIssue>> {
    parse_source(
        code,
        config,
        title,
        #[cfg(feature = "editor-facts")]
        None,
        #[cfg(feature = "presentation")]
        false,
        control,
    )
}

/// Reports borrowed source observations from the same parse used to construct the model.
#[cfg(feature = "editor-facts")]
pub fn parse_agentflow_source_with_observer_controlled(
    code: &str,
    config: &MermaidConfig,
    title: Option<&str>,
    observe: &mut dyn FnMut(AgentflowObservation<'_>),
    control: &OperationControl,
) -> OperationControlResult<Result<ParsedAgentflow, AgentflowSyntaxIssue>> {
    parse_source(
        code,
        config,
        title,
        Some(observe),
        #[cfg(feature = "presentation")]
        false,
        control,
    )
}

/// Constructs the model and its render-only DOM ordinals without editor collection.
#[cfg(feature = "presentation")]
pub fn parse_agentflow_source_for_render_controlled(
    code: &str,
    config: &MermaidConfig,
    title: Option<&str>,
    control: &OperationControl,
) -> OperationControlResult<Result<ParsedAgentflow, AgentflowSyntaxIssue>> {
    parse_source(
        code,
        config,
        title,
        #[cfg(feature = "editor-facts")]
        None,
        true,
        control,
    )
}

fn parse_source(
    code: &str,
    config: &MermaidConfig,
    title: Option<&str>,
    #[cfg(feature = "editor-facts")] mut observe: Option<&mut dyn FnMut(AgentflowObservation<'_>)>,
    #[cfg(feature = "presentation")] capture_visuals: bool,
    control: &OperationControl,
) -> OperationControlResult<Result<ParsedAgentflow, AgentflowSyntaxIssue>> {
    control.checkpoint()?;
    #[cfg(any(test, feature = "test-support"))]
    AGENTFLOW_SYNTAX_CONSTRUCTION_COUNT.set(AGENTFLOW_SYNTAX_CONSTRUCTION_COUNT.get() + 1);
    #[cfg(feature = "editor-facts")]
    if let Some(observe) = observe.as_deref_mut() {
        observe(AgentflowObservation::Started);
    }
    let mut parser = Parser::new(
        code,
        config,
        title,
        #[cfg(feature = "editor-facts")]
        observe
            .as_mut()
            .map(|observe| &mut **observe as &mut dyn FnMut(AgentflowObservation<'_>)),
        #[cfg(feature = "presentation")]
        capture_visuals,
        control,
    );
    let result = parser.parse()?;
    control.checkpoint()?;
    match result {
        Ok(model) => Ok(Ok(ParsedAgentflow {
            model,
            #[cfg(feature = "presentation")]
            node_dom_indices: parser.visuals,
        })),
        Err(ParseFailure::Cancelled(cancelled)) => Err(cancelled),
        Err(ParseFailure::Syntax { message, span }) => {
            Ok(Err(AgentflowSyntaxIssue { message, span }))
        }
    }
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

struct Parser<'a> {
    source: &'a str,
    config: &'a MermaidConfig,
    title: Option<&'a str>,
    control: &'a OperationControl,
    #[cfg(feature = "editor-facts")]
    observe: Option<&'a mut dyn FnMut(AgentflowObservation<'_>)>,
    authored: AgentflowAuthoredStyles,
    #[cfg(feature = "presentation")]
    visuals: Option<HashMap<String, usize>>,
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
        config: &'a MermaidConfig,
        title: Option<&'a str>,
        #[cfg(feature = "editor-facts")] observe: Option<
            &'a mut dyn FnMut(AgentflowObservation<'_>),
        >,
        #[cfg(feature = "presentation")] capture_visuals: bool,
        control: &'a OperationControl,
    ) -> Self {
        Self {
            source,
            config,
            title,
            control,
            #[cfg(feature = "editor-facts")]
            observe,
            authored: AgentflowAuthoredStyles::default(),
            #[cfg(feature = "presentation")]
            visuals: capture_visuals.then(HashMap::new),
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
        let Some((header_statement_start, header_statement)) = first_content_statement(self.source)
        else {
            return Ok(Err(self.failure(
                "expected agentflow or agentflow-beta header",
                SourceSpan::new(0, self.source.len()),
            )));
        };
        let header = header_statement.trim_matches(is_ecmascript_whitespace);
        let header_span = span_of(header_statement_start, header_statement, header);
        let mut header_tokens = header
            .split(is_ecmascript_whitespace)
            .filter(|token| !token.is_empty());
        if !matches!(header_tokens.next(), Some("agentflow" | "agentflow-beta")) {
            return Ok(Err(self.failure(
                "expected agentflow or agentflow-beta header",
                header_span,
            )));
        }
        let direction = header_tokens.next();
        if header_tokens.next().is_some() {
            return Ok(Err(self.failure(
                "unexpected content after agentflow header direction",
                header_span,
            )));
        }
        let header_tail = &self.source[header_span.end..];
        if let Some(value) = direction {
            self.direction = match normalize_direction(value) {
                Some(direction) => direction,
                None => {
                    return Ok(Err(self.failure("invalid agentflow direction", header_span)));
                }
            };
            if self.direction == "TD" {
                self.direction = "TB".to_string();
            }
            // In INITIAL, the whole-line direction token precedes the separator rules.
            if statement_direction(header_tail).is_some() {
                return Ok(Err(self.failure(
                    "expected a separator after the agentflow header direction",
                    header_span,
                )));
            }
        } else {
            let separator =
                header_tail.trim_start_matches(|ch| ch != '\n' && is_ecmascript_whitespace(ch));
            if separator.starts_with(';') {
                return Ok(Err(self.failure(
                    "a semicolon after the agentflow header requires a direction",
                    header_span,
                )));
            }
            if separator.starts_with("%%") {
                return Ok(Err(self.failure(
                    "an inline comment after the agentflow header requires a direction",
                    header_span,
                )));
            }
        }
        #[cfg(feature = "editor-facts")]
        if let Some(observe) = &mut self.observe {
            observe(AgentflowObservation::Header(header_span));
        }

        for (line_start, line) in split_top_level(self.source, b";\n") {
            self.control.checkpoint()?;
            let trimmed = line.trim().trim_start_matches('\u{feff}');
            if line_start == header_statement_start
                || trimmed.is_empty()
                || trimmed.starts_with("%%")
            {
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
                // The stable keyword emits a node token after the header.
                let starts_with_agentflow = line.strip_prefix("agentflow").is_some_and(|suffix| {
                    suffix
                        .chars()
                        .next()
                        .is_none_or(|ch| ch == ';' || is_ecmascript_whitespace(ch))
                });
                if !starts_with_agentflow && !direction_has_prior_token(line) {
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
                rest.trim_start_matches(is_ecmascript_whitespace)
                    .strip_prefix(':')
            }) {
                self.acc_title = Some(rest.trim().to_string());
                continue;
            }
            if let Some(rest) = trimmed.strip_prefix("accDescr").and_then(|rest| {
                rest.trim_start_matches(is_ecmascript_whitespace)
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
                            #[cfg(feature = "editor-facts")]
                            id_span,
                            label: title,
                            label_type,
                            metadata,
                            ..
                        }) => {
                            #[cfg(feature = "editor-facts")]
                            self.observe_identifier(
                                &id,
                                AgentflowIdentifierKind::ContainerDeclaration,
                                id_span,
                            );
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
                        visuals.remove(&id);
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
                #[cfg(feature = "editor-facts")]
                self.observe_identifier(&id, AgentflowIdentifierKind::NodeDeclaration, id_span);
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
        model.diagnostics.extend(model.containment_diagnostics());
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
                let mut fact =
                    DiagramWarningFact::new(diagnostic.rule_id(), diagnostic.message.clone());
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
        #[cfg(feature = "editor-facts")] reference: bool,
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
            #[cfg(feature = "editor-facts")]
            self.observe_identifier(&id, AgentflowIdentifierKind::ContainerReference, id_span);
        } else if declaration.metadata_span.is_some()
            && let Some(&index) = self.connector_index.get(&id)
        {
            self.connectors[index].metadata.extend(declaration.metadata);
            #[cfg(feature = "editor-facts")]
            self.observe_identifier(&id, AgentflowIdentifierKind::NodeReference, id_span);
        } else if let Some(edge) = self
            .edges
            .iter_mut()
            .find(|edge| edge.id.as_deref() == Some(&id))
        {
            presentation::attach_edge_metadata(&mut self.authored, &id, &declaration.metadata);
            edge.metadata.extend(declaration.metadata);
            #[cfg(feature = "editor-facts")]
            self.observe_identifier(&id, AgentflowIdentifierKind::NodeReference, id_span);
        } else {
            declaration.validate_shape()?;
            self.upsert_node(declaration);
            #[cfg(feature = "editor-facts")]
            self.observe_identifier(
                &id,
                if reference {
                    AgentflowIdentifierKind::NodeReferenceIfDeclared
                } else {
                    AgentflowIdentifierKind::NodeDeclaration
                },
                id_span,
            );
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
            let (id, metadata_call) = self.parse_node(
                endpoint,
                start,
                endpoint,
                #[cfg(feature = "editor-facts")]
                reference,
            )?;
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
        presentation::add_edge(
            &mut self.authored,
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

    #[cfg(feature = "editor-facts")]
    fn observe_identifier(&mut self, name: &str, kind: AgentflowIdentifierKind, span: SourceSpan) {
        if let Some(observe) = &mut self.observe {
            observe(AgentflowObservation::Identifier { name, span, kind });
        }
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
        "input" | "lean_right" => "lean-right".to_string(),
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
                    rest.trim_start_matches(is_ecmascript_whitespace)
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

fn first_content_statement(source: &str) -> Option<(usize, &str)> {
    let mut frontmatter = false;
    for (offset, line) in split_top_level(source, b";\n") {
        let trimmed = line.trim_matches(is_ecmascript_whitespace);
        if trimmed == "---" {
            frontmatter = !frontmatter;
            continue;
        }
        if !frontmatter && !trimmed.is_empty() {
            return Some((offset, line));
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
            Some(shape) if !shapes::is_valid_pinned_shape(&resolve_shape(shape)) => {
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
    value
        .strip_prefix(keyword)
        .is_some_and(|suffix| suffix.chars().next().is_some_and(is_ecmascript_whitespace))
}

// Preserve the INITIAL cursor after each protected CLICK/HREF/string/callback rule.
fn click_contains_unshielded_direction(statement: &str) -> bool {
    let rest = statement["click".len()..].trim_start_matches(is_ecmascript_whitespace);
    let Some(id_end) = rest.find(is_ecmascript_whitespace) else {
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
            let suffix = suffix.trim_start_matches(is_ecmascript_whitespace);
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

#[cfg(feature = "editor-facts")]
pub fn is_valid_editor_node_id(candidate: &str) -> bool {
    crate::flowchart::is_valid_editor_node_id(candidate) && !starts_reserved_identifier(candidate)
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
                    let value = suffix.trim_start_matches(is_ecmascript_whitespace);
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
    let mut result = parse_mermaid_inline_yaml_controlled(body, control)?;
    if result.is_err() && body.contains('\n') {
        let stripped = strip_line_trailing_commas(body);
        if stripped != body
            && let Ok(value) = parse_mermaid_inline_yaml_controlled(&stripped, control)?
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
            while chars.peek().is_some_and(|ch| is_ecmascript_whitespace(*ch)) {
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
mod tests {
    use super::*;

    #[test]
    fn agentflow_quoted_markdown_preserves_percent_signs() {
        let markdown = "[\"`one %% literal`\"]";
        assert_eq!(strip_label_comments(markdown), markdown);
    }

    #[test]
    fn agentflow_edge_limit_is_reported_after_the_terminal_checkpoint() {
        let source = "agentflow-beta LR; A & B --> C & D";
        let config = MermaidConfig::from_value(serde_json::json!({"maxEdges": 3}));
        for successful_checkpoints in 0..=7 {
            reset_agentflow_syntax_construction_count();
            let control = OperationControl::new();
            control.cancel_after_checkpoints(successful_checkpoints);
            let result = parse_agentflow_source_controlled(source, &config, None, &control);
            assert_eq!(
                agentflow_syntax_construction_count(),
                usize::from(successful_checkpoints != 0),
            );
            if successful_checkpoints < 7 {
                assert!(result.is_err(), "checkpoint {successful_checkpoints}");
            } else {
                let Err(issue) = result.unwrap() else {
                    panic!("four expanded edges must exceed the limit of three");
                };
                assert_eq!(
                    issue.message,
                    "Edge limit exceeded. 3 edges found, but the limit is 3."
                );
                assert_eq!(&source[issue.span.start..issue.span.end], "A & B --> C & D");
            }
        }
    }
}
