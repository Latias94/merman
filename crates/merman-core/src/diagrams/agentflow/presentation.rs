use super::{AgentflowAuthoredStyles, Declaration, ParseFailure, Parser};
use crate::SourceSpan;
use crate::diagrams::flowchart::{self, ClickAction, FlowEdgeDefaults, LinkStylePos, Tok};
#[cfg(feature = "editor-facts")]
use crate::{EditorSemanticKind, EditorSemanticSymbol};
use serde_json::{Map, Value};

pub(super) fn add_edge(authored: &mut AgentflowAuthoredStyles, id: &str, label_type: Option<&str>) {
    authored.edges.entry(id.to_string()).or_default().label_type =
        Some(label_type.unwrap_or("text").to_string());
    if let Some(interpolate) = authored
        .edge_defaults
        .as_ref()
        .and_then(|defaults| defaults.interpolate.as_ref())
    {
        authored
            .edges
            .entry(id.to_string())
            .or_default()
            .interpolate = Some(interpolate.clone());
    }
}

pub(super) fn attach_edge_metadata(
    authored: &mut AgentflowAuthoredStyles,
    id: &str,
    metadata: &Map<String, Value>,
) {
    let edge = authored.edges.entry(id.to_string()).or_default();
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

impl Parser<'_> {
    pub(super) fn assign_class(&mut self, ids: &str, class: &str) {
        for id in ids.split(',') {
            if let Some(node) = self.authored.nodes.get_mut(id) {
                node.classes.push(class.to_string());
            }
            if self.edges.iter().any(|edge| edge.id.as_deref() == Some(id)) {
                self.authored
                    .edges
                    .entry(id.to_string())
                    .or_default()
                    .classes
                    .push(class.to_string());
            }
            if self.sub_graph_index.contains_key(id) {
                self.authored
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
                let Some(target_span) = style.target_span else {
                    return Err(
                        self.failure("expected style target", SourceSpan::new(start, start + end))
                    );
                };
                let id_span = SourceSpan::new(start + target_span.start, start + target_span.end);
                // style calls addVertex; unlike class it creates an unknown node, but its statement
                // contributes no members to the enclosing flow. Edge ids return before node mutation.
                if !self
                    .edges
                    .iter()
                    .any(|edge| edge.id.as_deref() == Some(&style.target))
                {
                    if style.target != "connectors" {
                        self.record_vertex_call(&style.target);
                    }
                    self.upsert_node(Declaration {
                        id: style.target.clone(),
                        id_span,
                        label: None,
                        label_type: None,
                        syntax_shape: None,
                        metadata: Map::new(),
                        metadata_span: None,
                        authored_shape: None,
                        class: None,
                    });
                    #[cfg(feature = "editor-facts")]
                    {
                        let reference = self.declared_entities.contains(&style.target);
                        self.push_symbol(
                            &style.target,
                            EditorSemanticKind::Object,
                            id_span,
                            reference,
                        );
                    }
                    self.authored
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
                #[cfg(feature = "editor-facts")]
                let entries = class.ids.into_iter().zip(class.id_spans);
                #[cfg(not(feature = "editor-facts"))]
                let entries = class.ids;
                for entry in entries {
                    #[cfg(feature = "editor-facts")]
                    let (id, source_span) = entry;
                    #[cfg(not(feature = "editor-facts"))]
                    let id = entry;
                    self.control.checkpoint().map_err(ParseFailure::Cancelled)?;
                    self.authored
                        .class_defs
                        .entry(id.clone())
                        .or_default()
                        .extend(class.styles.iter().cloned());
                    #[cfg(feature = "editor-facts")]
                    if let Some(facts) = &mut self.facts {
                        let span =
                            SourceSpan::new(start + source_span.start, start + source_span.end);
                        facts.push_symbol(EditorSemanticSymbol::class_definition(
                            id,
                            None,
                            EditorSemanticKind::Class,
                            span,
                            span,
                        ));
                    }
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
                #[cfg(feature = "editor-facts")]
                let entries = class.targets.into_iter().zip(class.target_spans);
                #[cfg(not(feature = "editor-facts"))]
                let entries = class.targets;
                for entry in entries {
                    #[cfg(feature = "editor-facts")]
                    let (target, source_span) = entry;
                    #[cfg(not(feature = "editor-facts"))]
                    let target = entry;
                    self.control.checkpoint().map_err(ParseFailure::Cancelled)?;
                    self.assign_class(&target, &class.class_name);
                    #[cfg(feature = "editor-facts")]
                    {
                        let span =
                            SourceSpan::new(start + source_span.start, start + source_span.end);
                        self.push_symbol(&target, EditorSemanticKind::Object, span, true);
                    }
                }
            }
            Tok::ClickStmt(click) => {
                #[cfg(feature = "editor-facts")]
                let entries = click.ids.iter().zip(&click.id_spans);
                #[cfg(not(feature = "editor-facts"))]
                let entries = &click.ids;
                for entry in entries {
                    #[cfg(feature = "editor-facts")]
                    let (ids, group_span) = entry;
                    #[cfg(not(feature = "editor-facts"))]
                    let ids = entry;
                    #[cfg(feature = "editor-facts")]
                    let mut offset = 0;
                    for id in ids.split(',') {
                        #[cfg(feature = "editor-facts")]
                        let span = {
                            let span = SourceSpan::new(
                                start + group_span.start + offset,
                                start + group_span.start + offset + id.len(),
                            );
                            offset += id.len() + 1;
                            span
                        };
                        self.control.checkpoint().map_err(ParseFailure::Cancelled)?;
                        if let Some(tooltip) = &click.tooltip {
                            self.authored.tooltips.insert(
                                id.to_string(),
                                crate::sanitize::sanitize_text(tooltip, self.config),
                            );
                        }
                        self.assign_class(id, "clickable");
                        if let Some(node) = self.authored.nodes.get_mut(id) {
                            match &click.action {
                                ClickAction::Link { href, target } => {
                                    node.link = crate::utils::format_url(href, self.config);
                                    node.link_target = target.clone();
                                }
                                ClickAction::Callback => {
                                    if self
                                        .config
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
                        #[cfg(feature = "editor-facts")]
                        self.push_symbol(id, EditorSemanticKind::Object, span, true);
                    }
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
                            let defaults = self.authored.edge_defaults.get_or_insert_with(|| {
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
                            let edge = self.authored.edges.entry(id.clone()).or_default();
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
