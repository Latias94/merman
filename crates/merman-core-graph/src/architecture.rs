//! Controlled Architecture syntax and semantic replay.

use merman_core_preprocess::scan::strip_line_ending;
use merman_model::families::architecture::{
    ArchitectureDiagramModel, ArchitectureEdgeModel, ArchitectureGroupModel,
    ArchitectureLayoutDirection, ArchitectureLayoutHintModel, ArchitectureNodeKind,
    ArchitectureNodeModel,
};
use merman_model::{OperationControl, OperationControlResult, SourceSpan};
use std::collections::HashMap;

mod parse;

pub use parse::parse_architecture_source_controlled;
#[cfg(feature = "editor-facts")]
pub use parse::{
    ArchitectureObservation, ArchitectureOccurrenceKind, ArchitectureOccurrenceRole,
    parse_architecture_source_observed_controlled,
};
#[cfg(any(test, feature = "test-support"))]
pub use parse::{
    architecture_syntax_construction_count, reset_architecture_syntax_construction_count,
};

type ArchitectureResult<T> = Result<T, ArchitectureIssue>;

/// A syntax or validation diagnostic in parser input coordinates.
#[derive(Debug, Clone)]
pub struct ArchitectureIssue {
    pub message: String,
    pub location: ArchitectureIssueLocation,
    use_detected_type: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum ArchitectureIssueLocation {
    Exact(SourceSpan),
    InsertionPoint(usize),
    Fallback,
}

impl ArchitectureIssue {
    fn new(message: impl Into<String>, location: ArchitectureIssueLocation) -> Self {
        Self {
            message: message.into(),
            location,
            use_detected_type: false,
        }
    }

    fn exact(message: impl Into<String>, span: SourceSpan) -> Self {
        Self::new(message, ArchitectureIssueLocation::Exact(span))
    }

    fn insertion_point(message: impl Into<String>, offset: usize) -> Self {
        Self::new(message, ArchitectureIssueLocation::InsertionPoint(offset))
    }

    fn fallback(message: impl Into<String>) -> Self {
        Self::new(message, ArchitectureIssueLocation::Fallback)
    }

    fn with_detected_type(mut self) -> Self {
        self.use_detected_type = true;
        self
    }

    /// Header and recovered syntax errors keep the caller's detected diagram type.
    pub fn diagram_type<'a>(&self, detected_type: &'a str) -> &'a str {
        if self.use_detected_type {
            detected_type
        } else {
            "architecture"
        }
    }

    #[cfg(feature = "editor-facts")]
    pub fn span(&self) -> Option<SourceSpan> {
        match self.location {
            ArchitectureIssueLocation::Exact(span) => Some(span),
            ArchitectureIssueLocation::InsertionPoint(offset) => {
                Some(SourceSpan::new(offset, offset))
            }
            ArchitectureIssueLocation::Fallback => None,
        }
    }
}

/// Validated Architecture source whose ordered database is ready for model projection.
#[derive(Debug)]
pub struct ArchitectureSource(ArchitectureDb);

impl ArchitectureSource {
    /// Projects the canonical model at the caller's existing operation boundary.
    pub fn to_model_controlled(
        &self,
        control: &OperationControl,
    ) -> OperationControlResult<ArchitectureDiagramModel> {
        self.0.semantic_model_controlled(control)
    }
}

#[derive(Debug, Clone)]
struct ArchitectureIdentifier {
    text: String,
    span: SourceSpan,
}

#[derive(Debug, Clone)]
struct ArchitectureGroup {
    id: String,
    icon: Option<String>,
    title: Option<String>,
    in_group: Option<String>,
}

#[derive(Debug, Clone)]
struct ArchitectureEdge {
    lhs_id: String,
    lhs_span: SourceSpan,
    lhs_dir: char,
    lhs_into: Option<bool>,
    lhs_group: Option<bool>,
    rhs_id: String,
    rhs_span: SourceSpan,
    rhs_dir: char,
    rhs_into: Option<bool>,
    rhs_group: Option<bool>,
    title: Option<String>,
}

#[derive(Debug, Clone)]
struct ArchitectureLayoutHint {
    direction: ArchitectureLayoutDirection,
    members: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArchitectureNodeType {
    Service,
    Junction,
}

#[derive(Debug, Clone)]
struct ArchitectureNode {
    id: String,
    ty: ArchitectureNodeType,
    edges: Vec<usize>,
    icon: Option<String>,
    icon_text: Option<String>,
    title: Option<String>,
    in_group: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RegisteredIdType {
    Node,
    Group,
}

impl std::fmt::Display for RegisteredIdType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RegisteredIdType::Node => write!(f, "node"),
            RegisteredIdType::Group => write!(f, "group"),
        }
    }
}

#[derive(Debug, Default)]
struct ArchitectureDb {
    title: String,
    acc_title: String,
    acc_descr: String,

    nodes: HashMap<String, ArchitectureNode>,
    node_order: Vec<String>,
    groups: HashMap<String, ArchitectureGroup>,
    group_order: Vec<String>,
    edges: Vec<ArchitectureEdge>,
    layout_hints: Vec<ArchitectureLayoutHint>,
    registered_ids: HashMap<String, RegisteredIdType>,
}

impl ArchitectureDb {
    #[cfg(feature = "editor-facts")]
    fn editor_kind_for_id(&self, id: &str) -> Option<ArchitectureOccurrenceKind> {
        if self.groups.contains_key(id) {
            return Some(ArchitectureOccurrenceKind::Group);
        }
        self.nodes.get(id).map(|node| match node.ty {
            ArchitectureNodeType::Service => ArchitectureOccurrenceKind::Service,
            ArchitectureNodeType::Junction => ArchitectureOccurrenceKind::Junction,
        })
    }

    fn set_title(&mut self, title: String) {
        self.title = title;
    }

    fn set_acc_title(&mut self, title: String) {
        self.acc_title = title;
    }

    fn set_acc_descr(&mut self, descr: String) {
        self.acc_descr = descr;
    }

    fn semantic_model_controlled(
        &self,
        control: &OperationControl,
    ) -> OperationControlResult<ArchitectureDiagramModel> {
        control.checkpoint()?;
        let title = (!self.title.is_empty()).then(|| self.title.clone());
        let acc_title = (!self.acc_title.is_empty()).then(|| self.acc_title.clone());
        let acc_descr = (!self.acc_descr.is_empty()).then(|| self.acc_descr.clone());

        let mut nodes = Vec::with_capacity(self.node_order.len());
        for (index, id) in self.node_order.iter().enumerate() {
            if index % 128 == 0 {
                control.checkpoint()?;
            }
            let Some(n) = self.nodes.get(id) else {
                continue;
            };
            nodes.push(ArchitectureNodeModel {
                id: n.id.clone(),
                node_type: match n.ty {
                    ArchitectureNodeType::Service => ArchitectureNodeKind::Service,
                    ArchitectureNodeType::Junction => ArchitectureNodeKind::Junction,
                },
                edge_indices: n.edges.clone(),
                icon: n.icon.clone(),
                icon_text: n.icon_text.clone(),
                title: n.title.clone(),
                in_group: n.in_group.clone(),
            });
        }

        let mut groups = Vec::with_capacity(self.group_order.len());
        for (index, id) in self.group_order.iter().enumerate() {
            if index % 128 == 0 {
                control.checkpoint()?;
            }
            let Some(g) = self.groups.get(id) else {
                continue;
            };
            groups.push(ArchitectureGroupModel {
                id: g.id.clone(),
                icon: g.icon.clone(),
                title: g.title.clone(),
                in_group: g.in_group.clone(),
            });
        }

        let mut edges = Vec::with_capacity(self.edges.len());
        for (index, e) in self.edges.iter().enumerate() {
            if index % 128 == 0 {
                control.checkpoint()?;
            }
            edges.push(ArchitectureEdgeModel {
                lhs_id: e.lhs_id.clone(),
                lhs_dir: e.lhs_dir,
                lhs_into: e.lhs_into,
                lhs_group: e.lhs_group,
                rhs_id: e.rhs_id.clone(),
                rhs_dir: e.rhs_dir,
                rhs_into: e.rhs_into,
                rhs_group: e.rhs_group,
                title: e.title.clone(),
            });
        }

        let mut layout_hints = Vec::with_capacity(self.layout_hints.len());
        for (index, hint) in self.layout_hints.iter().enumerate() {
            if index % 128 == 0 {
                control.checkpoint()?;
            }
            layout_hints.push(ArchitectureLayoutHintModel {
                direction: hint.direction,
                members: hint.members.clone(),
            });
        }

        control.checkpoint()?;
        Ok(ArchitectureDiagramModel {
            title,
            acc_title,
            acc_descr,
            nodes,
            groups,
            edges,
            layout_hints,
        })
    }

    fn add_service(
        &mut self,
        id: ArchitectureIdentifier,
        icon: Option<String>,
        icon_text: Option<String>,
        title: Option<String>,
        in_group: Option<ArchitectureIdentifier>,
    ) -> ArchitectureResult<()> {
        let id_text = id.text;
        let id_span = id.span;
        if let Some(existing) = self.registered_ids.get(&id_text) {
            return Err(ArchitectureIssue::exact(
                format!("The service id [{id_text}] is already in use by another {existing}"),
                id_span,
            ));
        }

        if let Some(parent) = &in_group {
            if id_text == parent.text {
                return Err(ArchitectureIssue::exact(
                    format!("The service [{id_text}] cannot be placed within itself"),
                    parent.span,
                ));
            }
            let Some(parent_type) = self.registered_ids.get(&parent.text).copied() else {
                return Err(ArchitectureIssue::exact(
                    format!(
                        "The service [{id_text}]'s parent does not exist. Please make sure the parent is created before this service"
                    ),
                    parent.span,
                ));
            };
            if parent_type == RegisteredIdType::Node {
                return Err(ArchitectureIssue::exact(
                    format!("The service [{id_text}]'s parent is not a group"),
                    parent.span,
                ));
            }
        }

        let in_group = in_group.map(|parent| parent.text);
        self.registered_ids
            .insert(id_text.clone(), RegisteredIdType::Node);
        if !self.nodes.contains_key(&id_text) {
            self.node_order.push(id_text.clone());
        }
        self.nodes.insert(
            id_text.clone(),
            ArchitectureNode {
                id: id_text,
                ty: ArchitectureNodeType::Service,
                edges: Vec::new(),
                icon,
                icon_text,
                title,
                in_group,
            },
        );
        Ok(())
    }

    fn add_junction(
        &mut self,
        id: ArchitectureIdentifier,
        in_group: Option<ArchitectureIdentifier>,
    ) -> ArchitectureResult<()> {
        let id_text = id.text;
        if let Some(existing) = self.registered_ids.get(&id_text) {
            return Err(ArchitectureIssue::exact(
                format!("The junction id [{id_text}] is already in use by another {existing}"),
                id.span,
            ));
        }

        if let Some(parent) = &in_group {
            if id_text == parent.text {
                return Err(ArchitectureIssue::exact(
                    format!("The junction [{id_text}] cannot be placed within itself"),
                    parent.span,
                ));
            }
            let Some(parent_type) = self.registered_ids.get(&parent.text).copied() else {
                return Err(ArchitectureIssue::exact(
                    format!(
                        "The junction [{id_text}]'s parent does not exist. Please make sure the parent is created before this junction"
                    ),
                    parent.span,
                ));
            };
            if parent_type == RegisteredIdType::Node {
                return Err(ArchitectureIssue::exact(
                    format!("The junction [{id_text}]'s parent is not a group"),
                    parent.span,
                ));
            }
        }

        let in_group = in_group.map(|parent| parent.text);
        self.registered_ids
            .insert(id_text.clone(), RegisteredIdType::Node);
        if !self.nodes.contains_key(&id_text) {
            self.node_order.push(id_text.clone());
        }
        self.nodes.insert(
            id_text.clone(),
            ArchitectureNode {
                id: id_text,
                ty: ArchitectureNodeType::Junction,
                edges: Vec::new(),
                icon: None,
                icon_text: None,
                title: None,
                in_group,
            },
        );
        Ok(())
    }

    fn add_group(
        &mut self,
        id: ArchitectureIdentifier,
        icon: Option<String>,
        title: Option<String>,
        in_group: Option<ArchitectureIdentifier>,
    ) -> ArchitectureResult<()> {
        let id_text = id.text;
        let id_span = id.span;
        if let Some(existing) = self.registered_ids.get(&id_text) {
            return Err(ArchitectureIssue::exact(
                format!("The group id [{id_text}] is already in use by another {existing}"),
                id_span,
            ));
        }

        if let Some(parent) = &in_group {
            if id_text == parent.text {
                return Err(ArchitectureIssue::exact(
                    format!("The group [{id_text}] cannot be placed within itself"),
                    parent.span,
                ));
            }
            let Some(parent_type) = self.registered_ids.get(&parent.text).copied() else {
                return Err(ArchitectureIssue::exact(
                    format!(
                        "The group [{id_text}]'s parent does not exist. Please make sure the parent is created before this group"
                    ),
                    parent.span,
                ));
            };
            if parent_type == RegisteredIdType::Node {
                return Err(ArchitectureIssue::exact(
                    format!("The group [{id_text}]'s parent is not a group"),
                    parent.span,
                ));
            }
        }

        let in_group = in_group.map(|parent| parent.text);
        self.registered_ids
            .insert(id_text.clone(), RegisteredIdType::Group);
        if !self.groups.contains_key(&id_text) {
            self.group_order.push(id_text.clone());
        }
        self.groups.insert(
            id_text.clone(),
            ArchitectureGroup {
                id: id_text,
                icon,
                title,
                in_group,
            },
        );
        Ok(())
    }

    fn add_edge(&mut self, edge: ArchitectureEdge) -> ArchitectureResult<()> {
        if !is_dir(edge.lhs_dir) {
            return Err(ArchitectureIssue::fallback(format!(
                "Invalid direction given for left hand side of edge {}--{}. Expected (L,R,T,B) got {}",
                edge.lhs_id, edge.rhs_id, edge.lhs_dir
            )));
        }
        if !is_dir(edge.rhs_dir) {
            return Err(ArchitectureIssue::fallback(format!(
                "Invalid direction given for right hand side of edge {}--{}. Expected (L,R,T,B) got {}",
                edge.lhs_id, edge.rhs_id, edge.rhs_dir
            )));
        }

        if !self.nodes.contains_key(&edge.lhs_id) && !self.groups.contains_key(&edge.lhs_id) {
            return Err(ArchitectureIssue::exact(
                format!(
                    "The left-hand id [{}] does not yet exist. Please create the service/group before declaring an edge to it.",
                    edge.lhs_id
                ),
                edge.lhs_span,
            ));
        }
        if !self.nodes.contains_key(&edge.rhs_id) && !self.groups.contains_key(&edge.rhs_id) {
            return Err(ArchitectureIssue::exact(
                format!(
                    "The right-hand id [{}] does not yet exist. Please create the service/group before declaring an edge to it.",
                    edge.rhs_id
                ),
                edge.rhs_span,
            ));
        }
        if self.groups.contains_key(&edge.lhs_id) {
            return Err(ArchitectureIssue::exact(
                format!(
                    "The left-hand id [{}] is a group; architecture edges require a service or junction endpoint.",
                    edge.lhs_id
                ),
                edge.lhs_span,
            ));
        }
        if self.groups.contains_key(&edge.rhs_id) {
            return Err(ArchitectureIssue::exact(
                format!(
                    "The right-hand id [{}] is a group; architecture edges require a service or junction endpoint.",
                    edge.rhs_id
                ),
                edge.rhs_span,
            ));
        }

        if edge.lhs_group == Some(true)
            && let (Some(lhs), Some(rhs)) =
                (self.nodes.get(&edge.lhs_id), self.nodes.get(&edge.rhs_id))
            && let (Some(lhs_parent), Some(rhs_parent)) = (&lhs.in_group, &rhs.in_group)
            && lhs_parent == rhs_parent
        {
            return Err(ArchitectureIssue::exact(
                format!(
                    "The left-hand id [{}] is modified to traverse the group boundary, but the edge does not pass through two groups.",
                    edge.lhs_id
                ),
                edge.lhs_span,
            ));
        }
        if edge.rhs_group == Some(true)
            && let (Some(lhs), Some(rhs)) =
                (self.nodes.get(&edge.lhs_id), self.nodes.get(&edge.rhs_id))
            && let (Some(lhs_parent), Some(rhs_parent)) = (&lhs.in_group, &rhs.in_group)
            && lhs_parent == rhs_parent
        {
            return Err(ArchitectureIssue::exact(
                format!(
                    "The right-hand id [{}] is modified to traverse the group boundary, but the edge does not pass through two groups.",
                    edge.rhs_id
                ),
                edge.rhs_span,
            ));
        }

        let edge_idx = self.edges.len();
        self.edges.push(edge);
        let lhs_id = self.edges[edge_idx].lhs_id.clone();
        let rhs_id = self.edges[edge_idx].rhs_id.clone();
        if self.nodes.contains_key(&lhs_id) && self.nodes.contains_key(&rhs_id) {
            if let Some(lhs) = self.nodes.get_mut(&lhs_id) {
                lhs.edges.push(edge_idx);
            }
            if let Some(rhs) = self.nodes.get_mut(&rhs_id) {
                rhs.edges.push(edge_idx);
            }
        }
        Ok(())
    }

    fn add_layout_hint_controlled(
        &mut self,
        direction: ArchitectureLayoutDirection,
        members: Vec<ArchitectureIdentifier>,
        control: &OperationControl,
    ) -> OperationControlResult<ArchitectureResult<()>> {
        control.checkpoint()?;
        if members.len() < 2 {
            return Ok(Err(ArchitectureIssue::fallback(format!(
                "An align directive requires at least two members; got {}",
                members.len()
            ))));
        }

        let mut seen = std::collections::HashSet::new();
        let mut member_texts = Vec::with_capacity(members.len());
        for (index, member) in members.into_iter().enumerate() {
            if index % 128 == 0 {
                control.checkpoint()?;
            }
            if self.registered_ids.get(&member.text).copied() != Some(RegisteredIdType::Node) {
                return Ok(Err(ArchitectureIssue::exact(
                    format!(
                        "align {} references [{}], which is not a service or junction",
                        direction.as_str(),
                        member.text
                    ),
                    member.span,
                )));
            }
            if !seen.insert(member.text.clone()) {
                return Ok(Err(ArchitectureIssue::exact(
                    format!(
                        "align {} lists [{}] more than once",
                        direction.as_str(),
                        member.text
                    ),
                    member.span,
                )));
            }
            member_texts.push(member.text);
        }

        self.layout_hints.push(ArchitectureLayoutHint {
            direction,
            members: member_texts,
        });
        control.checkpoint()?;
        Ok(Ok(()))
    }
}

fn is_dir(c: char) -> bool {
    matches!(c, 'L' | 'R' | 'T' | 'B')
}

fn strip_inline_comment(line: &str) -> &str {
    &line[..parse::statement_end(line)]
}

struct ArchitectureSourceLine<'a> {
    text: &'a str,
    start: usize,
}

#[derive(Debug)]
struct ArchitectureLineCursor<'a> {
    source: &'a str,
    offset: usize,
    lookahead: parse::ArchitectureTerminalLookahead,
}

impl<'a> ArchitectureLineCursor<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            offset: 0,
            lookahead: parse::ArchitectureTerminalLookahead::default(),
        }
    }

    fn next(&mut self) -> Option<ArchitectureSourceLine<'a>> {
        if self.offset >= self.source.len() {
            return None;
        }

        let start = self.offset;
        let rest = &self.source[start..];
        let end = if let Some(newline) = rest.find('\n') {
            start + newline + 1
        } else {
            self.source.len()
        };
        self.offset = end;

        Some(ArchitectureSourceLine {
            text: strip_line_ending(&self.source[start..end]),
            start,
        })
    }
}

fn trimmed_statement_with_offset(raw: &str, raw_start: usize) -> (&str, usize) {
    let line = strip_inline_comment(raw);
    let leading = line.len() - line.trim_start().len();
    (line.trim(), raw_start + leading)
}

fn is_architecture_reserved_id(id: &str) -> bool {
    matches!(
        id,
        "architecture-beta" | "group" | "service" | "junction" | "in" | "align" | "row" | "column"
    ) || id.starts_with("title")
        || id
            .as_bytes()
            .first()
            .is_some_and(|first| matches!(first, b'L' | b'R' | b'T' | b'B'))
}

#[cfg(feature = "editor-facts")]
pub fn is_valid_editor_identifier(candidate: &str) -> bool {
    let mut last_was_word = false;
    for (index, ch) in candidate.chars().enumerate() {
        let is_word = ch.is_ascii_alphanumeric() || ch == '_';
        if index == 0 && !is_word {
            return false;
        }
        if !is_word && ch != '-' {
            return false;
        }
        last_was_word = is_word;
    }
    !candidate.is_empty() && last_was_word && !is_architecture_reserved_id(candidate)
}

fn architecture_reserved_id_message(id: &str) -> String {
    format!("reserved architecture keyword [{id}] cannot be used as an id")
}
