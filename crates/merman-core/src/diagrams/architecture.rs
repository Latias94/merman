use crate::diagrams::scan::strip_line_ending;
use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorRenameDomain, EditorSemanticFacts,
    EditorSemanticKind, EditorSemanticSymbol, Error, ParseMetadata, Result, SourceSpan,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ArchitectureLayoutDirection {
    Row,
    Column,
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
    fn clear(&mut self) {
        *self = Self::default();
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

    fn render_model(&self) -> ArchitectureDiagramRenderModel {
        let title = (!self.title.trim().is_empty()).then(|| self.title.clone());
        let acc_title = (!self.acc_title.trim().is_empty()).then(|| self.acc_title.clone());
        let acc_descr = (!self.acc_descr.trim().is_empty()).then(|| self.acc_descr.clone());

        let nodes: Vec<ArchitectureRenderNode> = self
            .node_order
            .iter()
            .filter_map(|id| self.nodes.get(id))
            .map(|n| ArchitectureRenderNode {
                id: n.id.clone(),
                node_type: match n.ty {
                    ArchitectureNodeType::Service => ArchitectureRenderNodeType::Service,
                    ArchitectureNodeType::Junction => ArchitectureRenderNodeType::Junction,
                },
                edge_indices: n.edges.clone(),
                icon: n.icon.clone(),
                icon_text: n.icon_text.clone(),
                title: n.title.clone(),
                in_group: n.in_group.clone(),
            })
            .collect();

        let groups: Vec<ArchitectureRenderGroup> = self
            .group_order
            .iter()
            .filter_map(|id| self.groups.get(id))
            .map(|g| ArchitectureRenderGroup {
                id: g.id.clone(),
                icon: g.icon.clone(),
                title: g.title.clone(),
                in_group: g.in_group.clone(),
            })
            .collect();

        let edges: Vec<ArchitectureRenderEdge> = self
            .edges
            .iter()
            .map(|e| ArchitectureRenderEdge {
                lhs_id: e.lhs_id.clone(),
                lhs_dir: e.lhs_dir,
                lhs_into: e.lhs_into,
                lhs_group: e.lhs_group,
                rhs_id: e.rhs_id.clone(),
                rhs_dir: e.rhs_dir,
                rhs_into: e.rhs_into,
                rhs_group: e.rhs_group,
                title: e.title.clone(),
            })
            .collect();

        ArchitectureDiagramRenderModel {
            title,
            acc_title,
            acc_descr,
            nodes,
            groups,
            edges,
            layout_hints: self.layout_hints_json_model(),
        }
    }

    fn add_service(
        &mut self,
        id: ArchitectureIdentifier,
        icon: Option<String>,
        icon_text: Option<String>,
        title: Option<String>,
        in_group: Option<ArchitectureIdentifier>,
    ) -> Result<()> {
        let id_text = id.text;
        let id_span = id.span;
        if let Some(existing) = self.registered_ids.get(&id_text) {
            return Err(Error::diagram_parse_exact(
                "architecture",
                format!("The service id [{id_text}] is already in use by another {existing}"),
                id_span,
            ));
        }

        if let Some(parent) = &in_group {
            if id_text == parent.text {
                return Err(Error::diagram_parse_exact(
                    "architecture",
                    format!("The service [{id_text}] cannot be placed within itself"),
                    parent.span,
                ));
            }
            let Some(parent_type) = self.registered_ids.get(&parent.text).copied() else {
                return Err(Error::diagram_parse_exact(
                    "architecture",
                    format!(
                        "The service [{id_text}]'s parent does not exist. Please make sure the parent is created before this service"
                    ),
                    parent.span,
                ));
            };
            if parent_type == RegisteredIdType::Node {
                return Err(Error::diagram_parse_exact(
                    "architecture",
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

    fn add_junction(&mut self, id: ArchitectureIdentifier, in_group: Option<String>) {
        let id = id.text;
        self.registered_ids
            .insert(id.clone(), RegisteredIdType::Node);
        if !self.nodes.contains_key(&id) {
            self.node_order.push(id.clone());
        }
        self.nodes.insert(
            id.clone(),
            ArchitectureNode {
                id,
                ty: ArchitectureNodeType::Junction,
                edges: Vec::new(),
                icon: None,
                icon_text: None,
                title: None,
                in_group,
            },
        );
    }

    fn add_group(
        &mut self,
        id: ArchitectureIdentifier,
        icon: Option<String>,
        title: Option<String>,
        in_group: Option<ArchitectureIdentifier>,
    ) -> Result<()> {
        let id_text = id.text;
        let id_span = id.span;
        if let Some(existing) = self.registered_ids.get(&id_text) {
            return Err(Error::diagram_parse_exact(
                "architecture",
                format!("The group id [{id_text}] is already in use by another {existing}"),
                id_span,
            ));
        }

        if let Some(parent) = &in_group {
            if id_text == parent.text {
                return Err(Error::diagram_parse_exact(
                    "architecture",
                    format!("The group [{id_text}] cannot be placed within itself"),
                    parent.span,
                ));
            }
            let Some(parent_type) = self.registered_ids.get(&parent.text).copied() else {
                return Err(Error::diagram_parse_exact(
                    "architecture",
                    format!(
                        "The group [{id_text}]'s parent does not exist. Please make sure the parent is created before this group"
                    ),
                    parent.span,
                ));
            };
            if parent_type == RegisteredIdType::Node {
                return Err(Error::diagram_parse_exact(
                    "architecture",
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

    fn add_edge(&mut self, edge: ArchitectureEdge) -> Result<()> {
        if !is_dir(edge.lhs_dir) {
            return Err(Error::diagram_parse_fallback(
                "architecture".to_string(),
                format!(
                    "Invalid direction given for left hand side of edge {}--{}. Expected (L,R,T,B) got {}",
                    edge.lhs_id, edge.rhs_id, edge.lhs_dir
                ),
            ));
        }
        if !is_dir(edge.rhs_dir) {
            return Err(Error::diagram_parse_fallback(
                "architecture".to_string(),
                format!(
                    "Invalid direction given for right hand side of edge {}--{}. Expected (L,R,T,B) got {}",
                    edge.lhs_id, edge.rhs_id, edge.rhs_dir
                ),
            ));
        }

        if !self.nodes.contains_key(&edge.lhs_id) && !self.groups.contains_key(&edge.lhs_id) {
            return Err(Error::diagram_parse_exact(
                "architecture",
                format!(
                    "The left-hand id [{}] does not yet exist. Please create the service/group before declaring an edge to it.",
                    edge.lhs_id
                ),
                edge.lhs_span,
            ));
        }
        if !self.nodes.contains_key(&edge.rhs_id) && !self.groups.contains_key(&edge.rhs_id) {
            return Err(Error::diagram_parse_exact(
                "architecture",
                format!(
                    "The right-hand id [{}] does not yet exist. Please create the service/group before declaring an edge to it.",
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
            return Err(Error::diagram_parse_exact(
                "architecture",
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
            return Err(Error::diagram_parse_exact(
                "architecture",
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

    fn add_layout_hint(
        &mut self,
        direction: ArchitectureLayoutDirection,
        members: Vec<ArchitectureIdentifier>,
    ) -> Result<()> {
        if members.len() < 2 {
            return Err(Error::diagram_parse_fallback(
                "architecture".to_string(),
                format!(
                    "An align directive requires at least two members; got {}",
                    members.len()
                ),
            ));
        }

        let mut seen = std::collections::HashSet::new();
        let mut member_texts = Vec::with_capacity(members.len());
        for member in members {
            if self.registered_ids.get(&member.text).copied() != Some(RegisteredIdType::Node) {
                return Err(Error::diagram_parse_exact(
                    "architecture",
                    format!(
                        "align {} references [{}], which is not a service or junction",
                        direction.as_str(),
                        member.text
                    ),
                    member.span,
                ));
            }
            if !seen.insert(member.text.clone()) {
                return Err(Error::diagram_parse_exact(
                    "architecture",
                    format!(
                        "align {} lists [{}] more than once",
                        direction.as_str(),
                        member.text
                    ),
                    member.span,
                ));
            }
            member_texts.push(member.text);
        }

        self.layout_hints.push(ArchitectureLayoutHint {
            direction,
            members: member_texts,
        });
        Ok(())
    }

    fn edges_json(&self) -> Vec<Value> {
        self.edges
            .iter()
            .map(|e| {
                json!({
                    "lhsId": e.lhs_id,
                    "lhsDir": e.lhs_dir.to_string(),
                    "lhsInto": e.lhs_into,
                    "lhsGroup": e.lhs_group,
                    "rhsId": e.rhs_id,
                    "rhsDir": e.rhs_dir.to_string(),
                    "rhsInto": e.rhs_into,
                    "rhsGroup": e.rhs_group,
                    "title": e.title,
                })
            })
            .collect()
    }

    fn groups_json(&self) -> Vec<Value> {
        self.group_order
            .iter()
            .filter_map(|id| self.groups.get(id))
            .map(|g| {
                json!({
                    "id": g.id,
                    "icon": g.icon,
                    "title": g.title,
                    "in": g.in_group,
                })
            })
            .collect()
    }

    fn nodes_json(&self) -> Vec<Value> {
        self.node_order
            .iter()
            .filter_map(|id| self.nodes.get(id))
            .map(|n| {
                let edges: Vec<Value> = n
                    .edges
                    .iter()
                    .filter_map(|idx| self.edges.get(*idx))
                    .map(|e| {
                        json!({
                            "lhsId": e.lhs_id,
                            "lhsDir": e.lhs_dir.to_string(),
                            "lhsInto": e.lhs_into,
                            "lhsGroup": e.lhs_group,
                            "rhsId": e.rhs_id,
                            "rhsDir": e.rhs_dir.to_string(),
                            "rhsInto": e.rhs_into,
                            "rhsGroup": e.rhs_group,
                            "title": e.title,
                        })
                    })
                    .collect();

                let ty = match n.ty {
                    ArchitectureNodeType::Service => "service",
                    ArchitectureNodeType::Junction => "junction",
                };

                json!({
                    "id": n.id,
                    "type": ty,
                    "edges": edges,
                    "icon": n.icon,
                    "iconText": n.icon_text,
                    "title": n.title,
                    "in": n.in_group,
                })
            })
            .collect()
    }

    fn services_json(&self) -> Vec<Value> {
        self.nodes_json()
            .into_iter()
            .filter(|n| n.get("type").and_then(|v| v.as_str()) == Some("service"))
            .collect()
    }

    fn junctions_json(&self) -> Vec<Value> {
        self.nodes_json()
            .into_iter()
            .filter(|n| n.get("type").and_then(|v| v.as_str()) == Some("junction"))
            .collect()
    }

    fn layout_hints_json(&self) -> Vec<Value> {
        self.layout_hints
            .iter()
            .map(|hint| {
                json!({
                    "direction": hint.direction.as_str(),
                    "members": hint.members,
                })
            })
            .collect()
    }

    fn layout_hints_json_model(&self) -> Vec<ArchitectureRenderLayoutHint> {
        self.layout_hints
            .iter()
            .map(|hint| ArchitectureRenderLayoutHint {
                direction: hint.direction,
                members: hint.members.clone(),
            })
            .collect()
    }
}

fn is_dir(c: char) -> bool {
    matches!(c, 'L' | 'R' | 'T' | 'B')
}

impl ArchitectureLayoutDirection {
    fn as_str(self) -> &'static str {
        match self {
            ArchitectureLayoutDirection::Row => "row",
            ArchitectureLayoutDirection::Column => "column",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "row" => Some(Self::Row),
            "column" => Some(Self::Column),
            _ => None,
        }
    }
}

fn strip_inline_comment(line: &str) -> &str {
    let mut in_quote = false;
    let mut quote_char: Option<char> = None;
    let mut it = line.char_indices().peekable();
    while let Some((idx, ch)) = it.next() {
        if in_quote {
            if ch == '\\' {
                it.next();
                continue;
            }
            if Some(ch) == quote_char {
                in_quote = false;
                quote_char = None;
            }
            continue;
        }
        if ch == '"' || ch == '\'' {
            in_quote = true;
            quote_char = Some(ch);
            continue;
        }
        if ch == '%' && it.peek().is_some_and(|(_, next)| *next == '%') {
            return &line[..idx];
        }
    }
    line
}

fn starts_with_kw(line: &str, kw: &str) -> bool {
    let t = line.trim_start();
    if !t.starts_with(kw) {
        return false;
    }
    let rest = &t[kw.len()..];
    rest.is_empty() || rest.chars().next().is_some_and(|c| c.is_whitespace())
}

fn parse_title_stmt(line: &str) -> Option<String> {
    if !starts_with_kw(line, "title") {
        return None;
    }
    let t = line.trim_start();
    let rest = &t["title".len()..];
    let rest = rest.strip_prefix(|c: char| c.is_whitespace()).unwrap_or("");
    Some(rest.to_string())
}

fn parse_acc_title_stmt(line: &str) -> Option<String> {
    let t = line.trim_start();
    if !t.starts_with("accTitle") {
        return None;
    }
    let rest = &t["accTitle".len()..];
    let rest = rest.trim_start();
    let rest = rest.strip_prefix(':')?;
    Some(rest.trim().to_string())
}

fn parse_acc_descr_stmt_single(line: &str) -> Option<String> {
    let t = line.trim_start();
    if !t.starts_with("accDescr") {
        return None;
    }
    let rest = &t["accDescr".len()..];
    let rest = rest.trim_start();
    let rest = rest.strip_prefix(':')?;
    Some(rest.trim().to_string())
}

#[derive(Debug, Clone, Copy)]
struct ArchitectureSourceLine<'a> {
    text: &'a str,
    start: usize,
}

#[derive(Debug)]
struct ArchitectureLineCursor<'a> {
    source: &'a str,
    offset: usize,
}

impl<'a> ArchitectureLineCursor<'a> {
    fn new(source: &'a str) -> Self {
        Self { source, offset: 0 }
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

fn parse_acc_descr_block(
    lines: &mut ArchitectureLineCursor<'_>,
    first_line: &str,
) -> Option<String> {
    let t = first_line.trim_start();
    if !t.starts_with("accDescr") {
        return None;
    }
    let rest = t["accDescr".len()..].trim_start();
    let rest = rest.strip_prefix('{')?;

    let mut buf = String::new();
    if let Some(end) = rest.find('}') {
        buf.push_str(&rest[..end]);
        return Some(buf.trim().to_string());
    }
    buf.push_str(rest);
    buf.push('\n');

    while let Some(line) = lines.next() {
        if let Some(end) = line.text.find('}') {
            buf.push_str(&line.text[..end]);
            break;
        }
        buf.push_str(line.text);
        buf.push('\n');
    }
    Some(buf.trim().to_string())
}

#[derive(Debug, Clone)]
struct SpannedText {
    text: String,
    span: SourceSpan,
}

struct SpanParser<'a> {
    input: &'a str,
    pos: usize,
    base_offset: usize,
}

impl<'a> SpanParser<'a> {
    fn new(input: &'a str, base_offset: usize) -> Self {
        Self {
            input,
            pos: 0,
            base_offset,
        }
    }

    fn is_eof(&self) -> bool {
        self.pos >= self.input.len()
    }

    fn peek_char(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }

    fn bump(&mut self) -> Option<char> {
        let ch = self.peek_char()?;
        self.pos += ch.len_utf8();
        Some(ch)
    }

    fn skip_ws(&mut self) {
        while self.peek_char().is_some_and(char::is_whitespace) {
            self.bump();
        }
    }

    fn consume_literal(&mut self, literal: &str) -> bool {
        self.skip_ws();
        if !self.input[self.pos..].starts_with(literal) {
            return false;
        }
        self.pos += literal.len();
        true
    }

    fn consume_keyword(&mut self, kw: &str) -> bool {
        self.skip_ws();
        if !self.input[self.pos..].starts_with(kw) {
            return false;
        }
        let after = &self.input[self.pos + kw.len()..];
        if !after
            .chars()
            .next()
            .is_none_or(|ch| ch.is_whitespace() || ch == ':' || ch == '[' || ch == '(')
        {
            return false;
        }
        self.pos += kw.len();
        true
    }

    fn parse_id(&mut self) -> Option<SpannedText> {
        self.skip_ws();
        let start = self.pos;
        let mut last_word_end: Option<usize> = None;
        let mut seen_any = false;
        while let Some(ch) = self.peek_char() {
            let is_word = ch.is_ascii_alphanumeric() || ch == '_';
            let is_allowed = is_word || ch == '-';
            if !seen_any {
                if !is_word {
                    return None;
                }
                seen_any = true;
                self.bump();
                last_word_end = Some(self.pos);
                continue;
            }
            if !is_allowed {
                break;
            }
            self.bump();
            if is_word {
                last_word_end = Some(self.pos);
            }
        }
        let end = last_word_end?;
        self.pos = end;
        Some(SpannedText {
            text: self.input[start..end].to_string(),
            span: SourceSpan::new(self.base_offset + start, self.base_offset + end),
        })
    }

    fn parse_bracketed(&mut self, open: char, close: char) -> Option<SpannedText> {
        self.skip_ws();
        if self.peek_char()? != open {
            return None;
        }
        self.bump();
        let start = self.pos;
        while let Some(ch) = self.peek_char() {
            if ch == close {
                break;
            }
            self.bump();
        }
        if self.peek_char()? != close {
            return None;
        }
        let end = self.pos;
        self.bump();

        let raw = &self.input[start..end];
        let leading = raw.len() - raw.trim_start().len();
        let trailing = raw.len() - raw.trim_end().len();
        let inner_start = start + leading;
        let inner_end = end.saturating_sub(trailing);
        Some(SpannedText {
            text: raw.trim().to_string(),
            span: SourceSpan::new(self.base_offset + inner_start, self.base_offset + inner_end),
        })
    }

    fn parse_quoted(&mut self) -> Option<SpannedText> {
        self.skip_ws();
        let quote = self.peek_char()?;
        if quote != '"' && quote != '\'' {
            return None;
        }
        self.bump();
        let start = self.pos;
        let mut escaped = false;
        while let Some(ch) = self.peek_char() {
            if escaped {
                escaped = false;
                self.bump();
                continue;
            }
            if ch == '\\' {
                escaped = true;
                self.bump();
                continue;
            }
            if ch == quote {
                break;
            }
            self.bump();
        }
        if self.peek_char()? != quote {
            return None;
        }
        let end = self.pos;
        self.bump();
        Some(SpannedText {
            text: self.input[start..end].to_string(),
            span: SourceSpan::new(self.base_offset + start, self.base_offset + end),
        })
    }

    fn consume_group_modifier(&mut self) {
        self.skip_ws();
        if self.input[self.pos..].starts_with("{group}") {
            self.pos += "{group}".len();
        }
    }
}

fn parse_architecture_editor_id(
    parser: &mut SpanParser<'_>,
    facts: &mut EditorSemanticFacts,
) -> std::result::Result<SpannedText, ()> {
    let Some(id) = parser.parse_id() else {
        return Err(());
    };
    if is_architecture_reserved_id(&id.text) {
        facts.mark_recovered_with_diagnostic(
            architecture_reserved_id_message(&id.text),
            Some(id.span),
        );
        return Err(());
    }
    Ok(id)
}

fn push_architecture_entity(
    facts: &mut EditorSemanticFacts,
    text: SpannedText,
    detail: &str,
    kind: EditorSemanticKind,
) {
    if text.text.is_empty() {
        return;
    }
    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::NodeIdentifier,
        text.span,
    ));
    facts.push_symbol(
        EditorSemanticSymbol::new(
            text.text,
            Some(detail.to_string()),
            kind,
            text.span,
            text.span,
        )
        .with_rename_domain(EditorRenameDomain::ArchitectureIdentifier),
    );
}

fn push_architecture_payload(
    facts: &mut EditorSemanticFacts,
    text: SpannedText,
    detail: &str,
    kind: EditorSemanticKind,
) {
    if text.text.is_empty() {
        return;
    }
    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::Payload,
        text.span,
    ));
    facts.push_symbol(EditorSemanticSymbol::payload(
        text.text,
        Some(detail.to_string()),
        kind,
        text.span,
        text.span,
    ));
}

fn value_after_keyword_span(line: &str, keyword: &str, base_offset: usize) -> Option<SpannedText> {
    let leading = line.len() - line.trim_start().len();
    let trimmed = &line[leading..];
    if !trimmed.starts_with(keyword) {
        return None;
    }
    let rest_start = leading + keyword.len();
    let after_keyword = &line[rest_start..];
    let rest = after_keyword.strip_prefix(|ch: char| ch.is_whitespace())?;
    let rest_start = rest_start + after_keyword.len() - rest.len();
    let value_leading = rest.len() - rest.trim_start().len();
    let value_without_leading = &rest[value_leading..];
    let value_trailing = value_without_leading.len() - value_without_leading.trim_end().len();
    let value = &value_without_leading[..value_without_leading.len() - value_trailing];
    if value.is_empty() {
        return None;
    }
    let rel = rest_start + value_leading;
    Some(SpannedText {
        text: value.to_string(),
        span: SourceSpan::new(base_offset + rel, base_offset + rel + value.len()),
    })
}

fn value_after_colon_span(line: &str, keyword: &str, base_offset: usize) -> Option<SpannedText> {
    let leading = line.len() - line.trim_start().len();
    let trimmed = &line[leading..];
    if !trimmed.starts_with(keyword) {
        return None;
    }
    let rest_start = leading + keyword.len();
    let rest = &line[rest_start..];
    let rest_leading = rest.len() - rest.trim_start().len();
    let colon_start = rest_start + rest_leading;
    let value_raw = line[colon_start..].strip_prefix(':')?;
    let value_leading = value_raw.len() - value_raw.trim_start().len();
    let value_without_leading = &value_raw[value_leading..];
    let value_trailing = value_without_leading.len() - value_without_leading.trim_end().len();
    let value = &value_without_leading[..value_without_leading.len() - value_trailing];
    if value.is_empty() {
        return None;
    }
    let rel = colon_start + 1 + value_leading;
    Some(SpannedText {
        text: value.to_string(),
        span: SourceSpan::new(base_offset + rel, base_offset + rel + value.len()),
    })
}

fn parse_architecture_stmt_facts(
    stmt: &str,
    stmt_start: usize,
    facts: &mut EditorSemanticFacts,
) -> std::result::Result<(), ()> {
    if let Some(title) = value_after_keyword_span(stmt, "title", stmt_start) {
        facts.push_directive_prefix("title");
        push_architecture_payload(
            facts,
            title,
            "architecture title",
            EditorSemanticKind::String,
        );
        return Ok(());
    }
    if let Some(title) = value_after_colon_span(stmt, "accTitle", stmt_start) {
        facts.push_directive_prefix("accTitle");
        push_architecture_payload(
            facts,
            title,
            "architecture accessibility title",
            EditorSemanticKind::String,
        );
        return Ok(());
    }
    if let Some(descr) = value_after_colon_span(stmt, "accDescr", stmt_start) {
        facts.push_directive_prefix("accDescr");
        push_architecture_payload(
            facts,
            descr,
            "architecture accessibility description",
            EditorSemanticKind::String,
        );
        return Ok(());
    }
    if stmt.trim_start().starts_with("accDescr") {
        facts.push_directive_prefix("accDescr");
        return Ok(());
    }

    let mut parser = SpanParser::new(stmt, stmt_start);
    if parser.consume_keyword("group") {
        let id = parse_architecture_editor_id(&mut parser, facts)?;
        push_architecture_entity(
            facts,
            id,
            "architecture group",
            EditorSemanticKind::Namespace,
        );
        if let Some(icon) = parser.parse_bracketed('(', ')') {
            push_architecture_payload(
                facts,
                icon,
                "architecture group icon",
                EditorSemanticKind::String,
            );
        }
        if let Some(title) = parser.parse_bracketed('[', ']') {
            push_architecture_payload(
                facts,
                title,
                "architecture group title",
                EditorSemanticKind::String,
            );
        }
        if parser.consume_keyword("in") {
            let parent = parse_architecture_editor_id(&mut parser, facts)?;
            push_architecture_entity(
                facts,
                parent,
                "architecture group parent",
                EditorSemanticKind::Namespace,
            );
        }
        return parser.is_eof().then_some(()).ok_or(());
    }

    let mut parser = SpanParser::new(stmt, stmt_start);
    if parser.consume_keyword("service") {
        let id = parse_architecture_editor_id(&mut parser, facts)?;
        push_architecture_entity(
            facts,
            id,
            "architecture service",
            EditorSemanticKind::Variable,
        );
        if let Some(icon) = parser.parse_bracketed('(', ')') {
            push_architecture_payload(
                facts,
                icon,
                "architecture service icon",
                EditorSemanticKind::String,
            );
        } else if let Some(icon_text) = parser.parse_quoted() {
            push_architecture_payload(
                facts,
                icon_text,
                "architecture service icon text",
                EditorSemanticKind::String,
            );
        }
        if let Some(title) = parser.parse_bracketed('[', ']') {
            push_architecture_payload(
                facts,
                title,
                "architecture service title",
                EditorSemanticKind::String,
            );
        }
        if parser.consume_keyword("in") {
            let parent = parse_architecture_editor_id(&mut parser, facts)?;
            push_architecture_entity(
                facts,
                parent,
                "architecture service parent",
                EditorSemanticKind::Namespace,
            );
        }
        return parser.is_eof().then_some(()).ok_or(());
    }

    let mut parser = SpanParser::new(stmt, stmt_start);
    if parser.consume_keyword("junction") {
        let id = parse_architecture_editor_id(&mut parser, facts)?;
        push_architecture_entity(
            facts,
            id,
            "architecture junction",
            EditorSemanticKind::Object,
        );
        if parser.consume_keyword("in") {
            let parent = parse_architecture_editor_id(&mut parser, facts)?;
            push_architecture_entity(
                facts,
                parent,
                "architecture junction parent",
                EditorSemanticKind::Namespace,
            );
        }
        return parser.is_eof().then_some(()).ok_or(());
    }

    let mut parser = SpanParser::new(stmt, stmt_start);
    if parser.consume_keyword("align") {
        let Some(direction) = parser.parse_id() else {
            return Err(());
        };
        if ArchitectureLayoutDirection::parse(&direction.text).is_none() {
            return Err(());
        }
        push_architecture_payload(
            facts,
            direction,
            "architecture alignment direction",
            EditorSemanticKind::String,
        );
        let mut count = 0usize;
        while !parser.is_eof() {
            let member = parse_architecture_editor_id(&mut parser, facts)?;
            push_architecture_entity(
                facts,
                member,
                "architecture alignment member",
                EditorSemanticKind::Variable,
            );
            count += 1;
        }
        return (count >= 1).then_some(()).ok_or(());
    }

    let mut parser = SpanParser::new(stmt, stmt_start);
    let lhs = parse_architecture_editor_id(&mut parser, facts)?;
    push_architecture_entity(
        facts,
        lhs,
        "architecture edge endpoint",
        EditorSemanticKind::Variable,
    );
    parser.consume_group_modifier();
    if !parser.consume_literal(":") {
        return Err(());
    }
    parser.skip_ws();
    if !parser.peek_char().is_some_and(is_arch_dir) {
        return Err(());
    }
    parser.bump();
    parser.skip_ws();
    if parser.peek_char().is_some_and(|ch| ch == '<' || ch == '>') {
        parser.bump();
    }
    parser.skip_ws();
    if parser.input[parser.pos..].starts_with("--") {
        parser.pos += 2;
    } else if parser.input[parser.pos..].starts_with('-') {
        parser.pos += 1;
        let Some(title) = parser.parse_bracketed('[', ']') else {
            return Err(());
        };
        push_architecture_payload(
            facts,
            title,
            "architecture edge title",
            EditorSemanticKind::String,
        );
        if !parser.consume_literal("-") {
            return Err(());
        }
    } else {
        return Err(());
    }
    parser.skip_ws();
    if parser.peek_char().is_some_and(|ch| ch == '<' || ch == '>') {
        parser.bump();
    }
    parser.skip_ws();
    if !parser.peek_char().is_some_and(is_arch_dir) {
        return Err(());
    }
    parser.bump();
    if !parser.consume_literal(":") {
        return Err(());
    }
    parser.skip_ws();
    if parser.peek_char() == Some(':') {
        parser.bump();
    }
    let rhs = parse_architecture_editor_id(&mut parser, facts)?;
    push_architecture_entity(
        facts,
        rhs,
        "architecture edge endpoint",
        EditorSemanticKind::Variable,
    );
    parser.consume_group_modifier();
    parser.is_eof().then_some(()).ok_or(())
}

pub fn parse_architecture_editor_facts(code: &str, _meta: &ParseMetadata) -> EditorSemanticFacts {
    let mut facts = EditorSemanticFacts::new();
    let mut offset = 0usize;
    let mut header_seen = false;

    for segment in code.split_inclusive('\n') {
        let line_start = offset;
        offset += segment.len();
        let raw_line = strip_line_ending(segment);
        let line = strip_inline_comment(raw_line);
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if !header_seen {
            if let Some(rest) = trimmed.strip_prefix("architecture-beta") {
                header_seen = true;
                let rest = rest.trim_start();
                if !rest.is_empty() {
                    let rel = line.find(rest).unwrap_or(0);
                    if parse_architecture_stmt_facts(rest, line_start + rel, &mut facts).is_err() {
                        facts.mark_recovered();
                    }
                }
                continue;
            }
            return facts;
        }

        if parse_architecture_stmt_facts(
            trimmed,
            line_start + line.find(trimmed).unwrap_or(0),
            &mut facts,
        )
        .is_err()
        {
            facts.mark_recovered();
        }
    }

    facts
}

fn take_id_prefix(input: &str) -> Option<(&str, &str)> {
    let mut last_word_end: Option<usize> = None;
    let mut seen_any = false;
    for (idx, ch) in input.char_indices() {
        let is_word = ch.is_ascii_alphanumeric() || ch == '_';
        let is_allowed = is_word || ch == '-';
        if !seen_any {
            if !is_word {
                return None;
            }
            seen_any = true;
            last_word_end = Some(idx + ch.len_utf8());
            continue;
        }
        if !is_allowed {
            break;
        }
        if is_word {
            last_word_end = Some(idx + ch.len_utf8());
        }
    }
    let end = last_word_end?;
    Some((&input[..end], &input[end..]))
}

fn is_architecture_reserved_id(id: &str) -> bool {
    matches!(id, "align" | "row" | "column")
}

pub(crate) fn is_valid_editor_identifier(candidate: &str) -> bool {
    take_id_prefix(candidate).is_some_and(|(id, rest)| {
        rest.is_empty() && id == candidate && !is_architecture_reserved_id(id)
    })
}

fn architecture_reserved_id_message(id: &str) -> String {
    format!("reserved architecture keyword [{id}] cannot be used as an id")
}

fn take_bracketed(input: &str, open: char, close: char) -> Option<(String, &str)> {
    let mut it = input.char_indices();
    let (_, first) = it.next()?;
    if first != open {
        return None;
    }
    for (idx, ch) in it {
        if ch == close {
            let inner = input[1..idx].to_string();
            return Some((inner, &input[idx + close.len_utf8()..]));
        }
    }
    None
}

fn architecture_suffix_start(line: &str, line_start: usize, suffix: &str) -> usize {
    debug_assert!(line.len() >= suffix.len());
    line_start + line.len().saturating_sub(suffix.len())
}

fn architecture_insertion_at_suffix(
    message: impl Into<String>,
    line: &str,
    line_start: usize,
    suffix: &str,
) -> Error {
    let suffix = suffix.trim_start();
    Error::diagram_parse_insertion_point(
        "architecture",
        message,
        architecture_suffix_start(line, line_start, suffix),
    )
}

fn architecture_exact_token(
    message: impl Into<String>,
    line: &str,
    line_start: usize,
    token_suffix: &str,
    token_len: usize,
) -> Error {
    let start = architecture_suffix_start(line, line_start, token_suffix);
    Error::diagram_parse_exact(
        "architecture",
        message,
        SourceSpan::new(start, start + token_len),
    )
}

fn architecture_trailing_input(line: &str, line_start: usize, rest: &str) -> Error {
    let trailing = rest.trim_start();
    let unexpected = trailing.trim_end();
    let start = architecture_suffix_start(line, line_start, trailing);
    Error::diagram_parse_exact(
        "architecture",
        "unexpected trailing input",
        SourceSpan::new(start, start + unexpected.len()),
    )
}

fn architecture_id_from_suffix(
    line: &str,
    line_start: usize,
    id: &str,
    suffix: &str,
) -> Result<ArchitectureIdentifier> {
    let suffix_start = architecture_suffix_start(line, line_start, suffix);
    let span = SourceSpan::new(suffix_start, suffix_start + id.len());
    if is_architecture_reserved_id(id) {
        return Err(Error::diagram_parse_exact(
            "architecture",
            architecture_reserved_id_message(id),
            span,
        ));
    }
    Ok(ArchitectureIdentifier {
        text: id.to_string(),
        span,
    })
}

fn parse_group_stmt(db: &mut ArchitectureDb, line: &str, line_start: usize) -> Result<bool> {
    if !starts_with_kw(line, "group") {
        return Ok(false);
    }
    let t = line.trim_start();
    let mut rest = t["group".len()..].trim_start();
    let Some((id, tail)) = take_id_prefix(rest) else {
        return Err(architecture_insertion_at_suffix(
            "invalid group id",
            line,
            line_start,
            rest,
        ));
    };
    let id = architecture_id_from_suffix(line, line_start, id, rest)?;
    rest = tail.trim_start();

    let mut icon = None;
    if let Some((i, tail)) = take_bracketed(rest, '(', ')') {
        icon = Some(i.trim().to_string());
        rest = tail.trim_start();
    }

    let mut title = None;
    if let Some((t, tail)) = take_bracketed(rest, '[', ']') {
        title = Some(t.trim().to_string());
        rest = tail.trim_start();
    }

    let mut in_group = None;
    if starts_with_kw(rest, "in") {
        rest = rest.trim_start()["in".len()..].trim_start();
        let Some((parent, tail)) = take_id_prefix(rest) else {
            return Err(architecture_insertion_at_suffix(
                "invalid group parent id",
                line,
                line_start,
                rest,
            ));
        };
        in_group = Some(architecture_id_from_suffix(line, line_start, parent, rest)?);
        rest = tail.trim_start();
    }

    if !rest.trim().is_empty() {
        return Err(architecture_trailing_input(line, line_start, rest));
    }

    db.add_group(id, icon, title, in_group)?;
    Ok(true)
}

fn take_quoted(input: &str) -> Option<(String, &str)> {
    let mut it = input.char_indices();
    let (_, q) = it.next()?;
    if q != '"' && q != '\'' {
        return None;
    }
    let mut escaped = false;
    for (idx, ch) in it {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == q {
            let inner = input[1..idx].to_string();
            return Some((inner, &input[idx + q.len_utf8()..]));
        }
    }
    None
}

fn parse_service_stmt(db: &mut ArchitectureDb, line: &str, line_start: usize) -> Result<bool> {
    if !starts_with_kw(line, "service") {
        return Ok(false);
    }
    let t = line.trim_start();
    let mut rest = t["service".len()..].trim_start();
    let Some((id, tail)) = take_id_prefix(rest) else {
        return Err(architecture_insertion_at_suffix(
            "invalid service id",
            line,
            line_start,
            rest,
        ));
    };
    let id = architecture_id_from_suffix(line, line_start, id, rest)?;
    rest = tail.trim_start();

    let mut icon = None;
    let mut icon_text = None;
    if let Some((i, tail)) = take_bracketed(rest, '(', ')') {
        icon = Some(i.trim().to_string());
        rest = tail.trim_start();
    } else if let Some((s, tail)) = take_quoted(rest) {
        icon_text = Some(s);
        rest = tail.trim_start();
    }

    let mut title = None;
    if let Some((t, tail)) = take_bracketed(rest, '[', ']') {
        title = Some(t.trim().to_string());
        rest = tail.trim_start();
    }

    let mut in_group = None;
    if starts_with_kw(rest, "in") {
        rest = rest.trim_start()["in".len()..].trim_start();
        let Some((parent, tail)) = take_id_prefix(rest) else {
            return Err(architecture_insertion_at_suffix(
                "invalid service parent id",
                line,
                line_start,
                rest,
            ));
        };
        in_group = Some(architecture_id_from_suffix(line, line_start, parent, rest)?);
        rest = tail.trim_start();
    }

    if !rest.trim().is_empty() {
        return Err(architecture_trailing_input(line, line_start, rest));
    }

    db.add_service(id, icon, icon_text, title, in_group)?;
    Ok(true)
}

fn parse_junction_stmt(db: &mut ArchitectureDb, line: &str, line_start: usize) -> Result<bool> {
    if !starts_with_kw(line, "junction") {
        return Ok(false);
    }
    let t = line.trim_start();
    let mut rest = t["junction".len()..].trim_start();
    let Some((id, tail)) = take_id_prefix(rest) else {
        return Err(architecture_insertion_at_suffix(
            "invalid junction id",
            line,
            line_start,
            rest,
        ));
    };
    let id = architecture_id_from_suffix(line, line_start, id, rest)?;
    rest = tail.trim_start();

    let mut in_group = None;
    if starts_with_kw(rest, "in") {
        rest = rest.trim_start()["in".len()..].trim_start();
        let Some((parent, tail)) = take_id_prefix(rest) else {
            return Err(architecture_insertion_at_suffix(
                "invalid junction parent id",
                line,
                line_start,
                rest,
            ));
        };
        in_group = Some(architecture_id_from_suffix(line, line_start, parent, rest)?.text);
        rest = tail.trim_start();
    }

    if !rest.trim().is_empty() {
        return Err(architecture_trailing_input(line, line_start, rest));
    }

    db.add_junction(id, in_group);
    Ok(true)
}

fn parse_id_with_optional_group_modifier<'a>(
    line: &str,
    line_start: usize,
    input: &'a str,
) -> Result<(ArchitectureIdentifier, Option<bool>, &'a str)> {
    let input = input.trim_start();
    let Some((id, rest)) = take_id_prefix(input) else {
        return Err(architecture_insertion_at_suffix(
            "invalid id",
            line,
            line_start,
            input,
        ));
    };
    let mut rest = rest;
    let mut group = None;
    if rest.starts_with("{group}") {
        group = Some(true);
        rest = &rest["{group}".len()..];
    }
    Ok((
        architecture_id_from_suffix(line, line_start, id, input)?,
        group,
        rest,
    ))
}

fn is_arch_dir(ch: char) -> bool {
    matches!(ch, 'L' | 'R' | 'T' | 'B')
}

fn parse_edge_stmt(db: &mut ArchitectureDb, line: &str, line_start: usize) -> Result<bool> {
    let mut rest = line.trim_start();
    if rest.is_empty() {
        return Ok(false);
    }
    if starts_with_kw(rest, "group")
        || starts_with_kw(rest, "service")
        || starts_with_kw(rest, "junction")
        || starts_with_kw(rest, "align")
        || starts_with_kw(rest, "title")
        || starts_with_kw(rest, "accTitle")
        || starts_with_kw(rest, "accDescr")
    {
        return Ok(false);
    }

    let (lhs_id, lhs_group, tail) = parse_id_with_optional_group_modifier(line, line_start, rest)?;
    rest = tail.trim_start();

    let mut lhs_into = None;
    let mut rhs_into = None;
    let mut title = None;

    rest = rest.strip_prefix(':').ok_or_else(|| {
        architecture_insertion_at_suffix("expected ':' for lhs port", line, line_start, rest)
    })?;
    rest = rest.trim_start();
    let lhs_dir: char = rest.chars().next().ok_or_else(|| {
        architecture_insertion_at_suffix("expected lhs direction", line, line_start, rest)
    })?;
    if !is_arch_dir(lhs_dir) {
        return Err(architecture_exact_token(
            "invalid lhs direction",
            line,
            line_start,
            rest,
            lhs_dir.len_utf8(),
        ));
    }
    rest = &rest[lhs_dir.len_utf8()..];

    rest = rest.trim_start();
    if let Some(ch) = rest.chars().next()
        && (ch == '<' || ch == '>')
    {
        lhs_into = Some(true);
        rest = &rest[ch.len_utf8()..];
    }

    rest = rest.trim_start();
    if rest.starts_with("--") {
        rest = &rest[2..];
    } else if rest.starts_with('-') {
        rest = &rest[1..];
        rest = rest.trim_start();
        let (t, tail) = take_bracketed(rest, '[', ']').ok_or_else(|| {
            architecture_insertion_at_suffix("expected edge title", line, line_start, rest)
        })?;
        title = Some(t.trim().to_string());
        rest = tail.trim_start();
        rest = rest.strip_prefix('-').ok_or_else(|| {
            architecture_insertion_at_suffix(
                "expected '-' after edge title",
                line,
                line_start,
                rest,
            )
        })?;
    } else {
        return Ok(false);
    }

    rest = rest.trim_start();
    if let Some(ch) = rest.chars().next()
        && (ch == '<' || ch == '>')
    {
        rhs_into = Some(true);
        rest = &rest[ch.len_utf8()..];
    }

    rest = rest.trim_start();
    let rhs_dir: char = rest.chars().next().ok_or_else(|| {
        architecture_insertion_at_suffix("expected rhs direction", line, line_start, rest)
    })?;
    if !is_arch_dir(rhs_dir) {
        return Err(architecture_exact_token(
            "invalid rhs direction",
            line,
            line_start,
            rest,
            rhs_dir.len_utf8(),
        ));
    }
    rest = &rest[rhs_dir.len_utf8()..];

    rest = rest.trim_start();
    rest = rest.strip_prefix(':').ok_or_else(|| {
        architecture_insertion_at_suffix("expected ':' for rhs port", line, line_start, rest)
    })?;

    rest = rest.trim_start();
    if rest.starts_with(':') {
        rest = &rest[1..];
        rest = rest.trim_start();
    }
    let (rhs_id, rhs_group, tail) = parse_id_with_optional_group_modifier(line, line_start, rest)?;
    rest = tail.trim_start();

    if !rest.is_empty() {
        return Err(architecture_trailing_input(line, line_start, rest));
    }

    db.add_edge(ArchitectureEdge {
        lhs_id: lhs_id.text,
        lhs_span: lhs_id.span,
        lhs_dir,
        lhs_into,
        lhs_group,
        rhs_id: rhs_id.text,
        rhs_span: rhs_id.span,
        rhs_dir,
        rhs_into,
        rhs_group,
        title,
    })?;

    Ok(true)
}

fn parse_align_stmt(db: &mut ArchitectureDb, line: &str, line_start: usize) -> Result<bool> {
    if !starts_with_kw(line, "align") {
        return Ok(false);
    }
    let t = line.trim_start();
    let mut rest = t["align".len()..].trim_start();
    let Some((direction_text, tail)) = take_id_prefix(rest) else {
        return Err(architecture_insertion_at_suffix(
            "invalid align direction",
            line,
            line_start,
            rest,
        ));
    };
    let Some(direction) = ArchitectureLayoutDirection::parse(direction_text) else {
        return Err(architecture_exact_token(
            "invalid align direction",
            line,
            line_start,
            rest,
            direction_text.len(),
        ));
    };
    rest = tail.trim_start();

    let mut members = Vec::new();
    while !rest.trim().is_empty() {
        let Some((member, tail)) = take_id_prefix(rest) else {
            return Err(architecture_insertion_at_suffix(
                "invalid align member id",
                line,
                line_start,
                rest,
            ));
        };
        members.push(architecture_id_from_suffix(line, line_start, member, rest)?);
        rest = tail.trim_start();
    }

    db.add_layout_hint(direction, members)?;
    Ok(true)
}

pub fn parse_architecture(code: &str, meta: &ParseMetadata) -> Result<Value> {
    let mut db = ArchitectureDb::default();
    db.clear();

    let mut lines = ArchitectureLineCursor::new(code);
    let mut found_header = false;
    let mut header_tail: Option<(String, usize)> = None;
    while let Some(line) = lines.next() {
        let (trimmed, trimmed_start) = trimmed_statement_with_offset(line.text, line.start);
        if trimmed.is_empty() {
            continue;
        }
        if let Some(rest_with_ws) = trimmed.strip_prefix("architecture-beta") {
            let rest = rest_with_ws.trim_start();
            if !rest.is_empty() {
                let leading = rest_with_ws.len() - rest_with_ws.trim_start().len();
                header_tail = Some((
                    rest.to_string(),
                    trimmed_start + "architecture-beta".len() + leading,
                ));
            }
            found_header = true;
            break;
        }
        break;
    }

    if !found_header {
        return Err(Error::diagram_parse_fallback(
            meta.diagram_type.clone(),
            "expected architecture-beta header".to_string(),
        ));
    }

    let mut process_line =
        |raw: &str, raw_start: usize, lines: &mut ArchitectureLineCursor<'_>| -> Result<()> {
            let (trimmed, trimmed_start) = trimmed_statement_with_offset(raw, raw_start);
            if trimmed.is_empty() {
                return Ok(());
            }

            if let Some(v) = parse_title_stmt(trimmed) {
                db.set_title(v);
                return Ok(());
            }
            if let Some(v) = parse_acc_title_stmt(trimmed) {
                db.set_acc_title(v);
                return Ok(());
            }
            if let Some(v) = parse_acc_descr_stmt_single(trimmed) {
                db.set_acc_descr(v);
                return Ok(());
            }
            if let Some(v) = parse_acc_descr_block(lines, trimmed) {
                db.set_acc_descr(v);
                return Ok(());
            }

            if parse_group_stmt(&mut db, trimmed, trimmed_start)? {
                return Ok(());
            }
            if parse_service_stmt(&mut db, trimmed, trimmed_start)? {
                return Ok(());
            }
            if parse_junction_stmt(&mut db, trimmed, trimmed_start)? {
                return Ok(());
            }
            if parse_align_stmt(&mut db, trimmed, trimmed_start)? {
                return Ok(());
            }
            if parse_edge_stmt(&mut db, trimmed, trimmed_start)? {
                return Ok(());
            }

            Err(Error::diagram_parse_fallback(
                meta.diagram_type.clone(),
                format!("unrecognized statement: {trimmed}"),
            ))
        };

    if let Some((tail, tail_start)) = &header_tail {
        process_line(tail, *tail_start, &mut lines)?;
    }

    while let Some(line) = lines.next() {
        process_line(line.text, line.start, &mut lines)?;
    }

    let mut config = crate::config::clone_value_nonrecursive(meta.effective_config.as_value());
    if meta.config.as_value().get("layout").is_none()
        && let Some(obj) = config.as_object_mut()
    {
        obj.insert("layout".to_string(), Value::String("dagre".to_string()));
    }

    let groups = db.groups_json();
    let nodes = db.nodes_json();
    let services = db.services_json();
    let junctions = db.junctions_json();
    let edges = db.edges_json();
    let layout_hints = db.layout_hints_json();

    let mut out = serde_json::Map::with_capacity(10);
    out.insert("type".to_string(), Value::String(meta.diagram_type.clone()));
    out.insert(
        "title".to_string(),
        if db.title.is_empty() {
            Value::Null
        } else {
            Value::String(db.title.clone())
        },
    );
    out.insert(
        "accTitle".to_string(),
        if db.acc_title.is_empty() {
            Value::Null
        } else {
            Value::String(db.acc_title.clone())
        },
    );
    out.insert(
        "accDescr".to_string(),
        if db.acc_descr.is_empty() {
            Value::Null
        } else {
            Value::String(db.acc_descr.clone())
        },
    );
    out.insert("groups".to_string(), Value::Array(groups));
    out.insert("nodes".to_string(), Value::Array(nodes));
    out.insert("services".to_string(), Value::Array(services));
    out.insert("junctions".to_string(), Value::Array(junctions));
    out.insert("edges".to_string(), Value::Array(edges));
    out.insert("layoutHints".to_string(), Value::Array(layout_hints));
    out.insert("config".to_string(), config);
    Ok(Value::Object(out))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchitectureDiagramRenderModel {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default, rename = "accTitle")]
    pub acc_title: Option<String>,
    #[serde(default, rename = "accDescr")]
    pub acc_descr: Option<String>,
    #[serde(default)]
    pub nodes: Vec<ArchitectureRenderNode>,
    #[serde(default)]
    pub groups: Vec<ArchitectureRenderGroup>,
    #[serde(default)]
    pub edges: Vec<ArchitectureRenderEdge>,
    #[serde(default, rename = "layoutHints")]
    pub layout_hints: Vec<ArchitectureRenderLayoutHint>,
}

impl ArchitectureDiagramRenderModel {
    pub(crate) fn sanitize_common_db_fields(&mut self, config: &crate::MermaidConfig) {
        crate::common_db::sanitize_optional_title(&mut self.title, config);
        crate::common_db::sanitize_optional_acc_title(&mut self.acc_title, config);
        crate::common_db::sanitize_optional_acc_descr(&mut self.acc_descr, config);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArchitectureRenderNodeType {
    #[serde(rename = "service")]
    Service,
    #[serde(rename = "junction")]
    Junction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchitectureRenderNode {
    pub id: String,
    #[serde(rename = "type")]
    pub node_type: ArchitectureRenderNodeType,
    #[serde(default)]
    pub edge_indices: Vec<usize>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default, rename = "iconText")]
    pub icon_text: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default, rename = "in")]
    pub in_group: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchitectureRenderGroup {
    pub id: String,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default, rename = "in")]
    pub in_group: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchitectureRenderEdge {
    #[serde(rename = "lhsId")]
    pub lhs_id: String,
    #[serde(rename = "lhsDir")]
    pub lhs_dir: char,
    #[serde(default, rename = "lhsInto")]
    pub lhs_into: Option<bool>,
    #[serde(default, rename = "lhsGroup")]
    pub lhs_group: Option<bool>,
    #[serde(rename = "rhsId")]
    pub rhs_id: String,
    #[serde(rename = "rhsDir")]
    pub rhs_dir: char,
    #[serde(default, rename = "rhsInto")]
    pub rhs_into: Option<bool>,
    #[serde(default, rename = "rhsGroup")]
    pub rhs_group: Option<bool>,
    #[serde(default)]
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchitectureRenderLayoutHint {
    pub direction: ArchitectureLayoutDirection,
    #[serde(default)]
    pub members: Vec<String>,
}

pub fn parse_architecture_model_for_render(
    code: &str,
    meta: &ParseMetadata,
) -> Result<ArchitectureDiagramRenderModel> {
    let mut db = ArchitectureDb::default();
    db.clear();

    let mut lines = ArchitectureLineCursor::new(code);
    let mut found_header = false;
    let mut header_tail: Option<(String, usize)> = None;
    while let Some(line) = lines.next() {
        let (trimmed, trimmed_start) = trimmed_statement_with_offset(line.text, line.start);
        if trimmed.is_empty() {
            continue;
        }
        if let Some(rest_with_ws) = trimmed.strip_prefix("architecture-beta") {
            let rest = rest_with_ws.trim_start();
            if !rest.is_empty() {
                let leading = rest_with_ws.len() - rest_with_ws.trim_start().len();
                header_tail = Some((
                    rest.to_string(),
                    trimmed_start + "architecture-beta".len() + leading,
                ));
            }
            found_header = true;
            break;
        }
        break;
    }

    if !found_header {
        return Err(Error::diagram_parse_fallback(
            meta.diagram_type.clone(),
            "expected architecture-beta header".to_string(),
        ));
    }

    let mut process_line =
        |raw: &str, raw_start: usize, lines: &mut ArchitectureLineCursor<'_>| -> Result<()> {
            let (trimmed, trimmed_start) = trimmed_statement_with_offset(raw, raw_start);
            if trimmed.is_empty() {
                return Ok(());
            }

            if let Some(v) = parse_title_stmt(trimmed) {
                db.set_title(v);
                return Ok(());
            }
            if let Some(v) = parse_acc_title_stmt(trimmed) {
                db.set_acc_title(v);
                return Ok(());
            }
            if let Some(v) = parse_acc_descr_stmt_single(trimmed) {
                db.set_acc_descr(v);
                return Ok(());
            }
            if let Some(v) = parse_acc_descr_block(lines, trimmed) {
                db.set_acc_descr(v);
                return Ok(());
            }

            if parse_group_stmt(&mut db, trimmed, trimmed_start)? {
                return Ok(());
            }
            if parse_service_stmt(&mut db, trimmed, trimmed_start)? {
                return Ok(());
            }
            if parse_junction_stmt(&mut db, trimmed, trimmed_start)? {
                return Ok(());
            }
            if parse_align_stmt(&mut db, trimmed, trimmed_start)? {
                return Ok(());
            }
            if parse_edge_stmt(&mut db, trimmed, trimmed_start)? {
                return Ok(());
            }

            Err(Error::diagram_parse_fallback(
                meta.diagram_type.clone(),
                format!("unrecognized statement: {trimmed}"),
            ))
        };

    if let Some((tail, tail_start)) = &header_tail {
        process_line(tail, *tail_start, &mut lines)?;
    }

    while let Some(line) = lines.next() {
        process_line(line.text, line.start, &mut lines)?;
    }

    Ok(db.render_model())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        EditorSemanticCompleteness, Engine, MermaidConfig, ParseDiagnosticSpanKind, ParseOptions,
    };
    use futures::executor::block_on;

    fn parse(text: &str) -> Value {
        let engine = Engine::new();
        block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap()
            .model
    }

    fn parse_err(text: &str) -> crate::ParseDiagnostic {
        let engine = Engine::new();
        match block_on(engine.parse_diagram(text, ParseOptions::default())).unwrap_err() {
            Error::DiagramParse { diagnostic, .. } => diagnostic,
            other => panic!("expected architecture parse error, got {other:?}"),
        }
    }

    fn test_meta() -> ParseMetadata {
        ParseMetadata {
            diagram_type: "architecture".to_string(),
            config: MermaidConfig::default(),
            effective_config: MermaidConfig::default(),
            title: None,
        }
    }

    fn payload_selection(facts: &EditorSemanticFacts, detail: &str, name: &str) -> SourceSpan {
        facts
            .symbols
            .iter()
            .find(|symbol| symbol.detail.as_deref() == Some(detail) && symbol.name == name)
            .unwrap_or_else(|| panic!("missing payload symbol {detail:?} {name:?}"))
            .selection
    }

    #[test]
    fn architecture_accepts_header_only() {
        let _ = parse("architecture-beta");
    }

    #[test]
    fn architecture_accepts_simple_service() {
        let model = parse("architecture-beta\n  service db\n");
        assert_eq!(model["services"].as_array().unwrap().len(), 1);
        assert_eq!(model["services"][0]["id"].as_str().unwrap(), "db");
    }

    #[test]
    fn architecture_rejects_reserved_keywords_as_entity_ids_with_exact_spans() {
        for (entity, suffix) in [
            ("service", "(server)[X]"),
            ("group", "(cloud)[X]"),
            ("junction", ""),
        ] {
            for reserved in ["align", "row", "column"] {
                let text = format!("architecture-beta\n  {entity} {reserved}{suffix}\n");
                let diagnostic = parse_err(&text);
                let offset = text.find(reserved).unwrap();

                assert_eq!(
                    diagnostic.message(),
                    format!("reserved architecture keyword [{reserved}] cannot be used as an id")
                );
                assert_eq!(
                    diagnostic.span(),
                    Some(SourceSpan::new(offset, offset + reserved.len()))
                );
                assert_eq!(diagnostic.span_kind(), ParseDiagnosticSpanKind::Exact);
            }
        }
    }

    #[test]
    fn architecture_rejects_reserved_keywords_in_id_reference_positions() {
        for (text, reserved) in [
            (
                "architecture-beta\n  group root\n  service child in row\n",
                "row",
            ),
            (
                "architecture-beta\n  service source\n  source:L -- R:column\n",
                "column",
            ),
            (
                "architecture-beta\n  service source\n  align row source align\n",
                "align",
            ),
        ] {
            let diagnostic = parse_err(text);
            let offset = text.rfind(reserved).unwrap();

            assert_eq!(
                diagnostic.message(),
                format!("reserved architecture keyword [{reserved}] cannot be used as an id")
            );
            assert_eq!(
                diagnostic.span(),
                Some(SourceSpan::new(offset, offset + reserved.len()))
            );
            assert_eq!(diagnostic.span_kind(), ParseDiagnosticSpanKind::Exact);
        }
    }

    #[test]
    fn architecture_accepts_ids_that_only_start_with_reserved_keywords() {
        let model = parse(
            "architecture-beta\n  service rowspan(server)[Rowspan]\n  group columnar(cloud)[Columnar]\n  junction alignment\n",
        );

        assert_eq!(model["services"][0]["id"], "rowspan");
        assert_eq!(model["groups"][0]["id"], "columnar");
        assert_eq!(model["junctions"][0]["id"], "alignment");
    }

    #[test]
    fn architecture_editor_facts_report_reserved_entity_ids() {
        for (entity, suffix) in [
            ("service", "(server)[X]"),
            ("group", "(cloud)[X]"),
            ("junction", ""),
        ] {
            for reserved in ["align", "row", "column"] {
                let text = format!("architecture-beta\n  {entity} {reserved}{suffix}\n");
                let offset = text.find(reserved).unwrap();
                let facts = parse_architecture_editor_facts(&text, &test_meta());

                assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
                assert_eq!(facts.diagnostics.len(), 1);
                assert_eq!(
                    facts.diagnostics[0].message,
                    format!("reserved architecture keyword [{reserved}] cannot be used as an id")
                );
                assert_eq!(
                    facts.diagnostics[0].span,
                    Some(SourceSpan::new(offset, offset + reserved.len()))
                );
                assert!(!facts.symbols.iter().any(|symbol| symbol.name == reserved));
            }
        }
    }

    #[test]
    fn architecture_editor_facts_report_reserved_id_references() {
        for (text, reserved) in [
            (
                "architecture-beta\n  group root\n  service child in row\n",
                "row",
            ),
            (
                "architecture-beta\n  service source\n  source:L -- R:column\n",
                "column",
            ),
            (
                "architecture-beta\n  service source\n  align row source align\n",
                "align",
            ),
        ] {
            let offset = text.rfind(reserved).unwrap();
            let facts = parse_architecture_editor_facts(text, &test_meta());

            assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
            assert_eq!(facts.diagnostics.len(), 1);
            assert_eq!(
                facts.diagnostics[0].message,
                format!("reserved architecture keyword [{reserved}] cannot be used as an id")
            );
            assert_eq!(
                facts.diagnostics[0].span,
                Some(SourceSpan::new(offset, offset + reserved.len()))
            );
        }
    }

    #[test]
    fn architecture_title_on_first_line() {
        let model = parse("architecture-beta title Simple Architecture Diagram");
        assert_eq!(
            model["title"].as_str().unwrap(),
            "Simple Architecture Diagram"
        );
    }

    #[test]
    fn architecture_title_on_another_line() {
        let model = parse("architecture-beta\n  title Simple Architecture Diagram\n");
        assert_eq!(
            model["title"].as_str().unwrap(),
            "Simple Architecture Diagram"
        );
    }

    #[test]
    fn architecture_editor_payload_spans_point_to_values_when_values_match_keywords() {
        let text = "architecture-beta\n  title title\n  accTitle: accTitle\n  accDescr: accDescr\n";
        let facts = parse_architecture_editor_facts(text, &test_meta());

        for (detail, name, needle) in [
            ("architecture title", "title", "title title"),
            (
                "architecture accessibility title",
                "accTitle",
                "accTitle: accTitle",
            ),
            (
                "architecture accessibility description",
                "accDescr",
                "accDescr: accDescr",
            ),
        ] {
            let value_start = text.find(needle).unwrap() + needle.rfind(name).unwrap();
            assert_eq!(
                payload_selection(&facts, detail, name),
                SourceSpan::new(value_start, value_start + name.len()),
                "wrong span for {detail}"
            );
        }
    }

    #[test]
    fn architecture_accessibility_title_and_descr() {
        let model = parse(
            "architecture-beta\n  accTitle: Accessibility Title\n  accDescr: Accessibility Description\n",
        );
        assert_eq!(model["accTitle"].as_str().unwrap(), "Accessibility Title");
        assert_eq!(
            model["accDescr"].as_str().unwrap(),
            "Accessibility Description"
        );
    }

    #[test]
    fn architecture_multiline_acc_descr() {
        let model = parse("architecture-beta\n  accDescr {\n    Accessibility Description\n  }\n");
        assert_eq!(
            model["accDescr"].as_str().unwrap(),
            "Accessibility Description"
        );
    }

    #[test]
    fn architecture_edge_with_ports_is_parsed() {
        let model =
            parse("architecture-beta\n  service db\n  service server\n  db:L -- R:server\n");
        let edges = model["edges"].as_array().unwrap();
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0]["lhsId"].as_str().unwrap(), "db");
        assert_eq!(edges[0]["lhsDir"].as_str().unwrap(), "L");
        assert_eq!(edges[0]["rhsId"].as_str().unwrap(), "server");
        assert_eq!(edges[0]["rhsDir"].as_str().unwrap(), "R");
    }

    #[test]
    fn architecture_edge_with_title_is_parsed() {
        let model = parse("architecture-beta\n  service a\n  service b\n  a:L -[Label]- R:b\n");
        let edges = model["edges"].as_array().unwrap();
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0]["title"].as_str().unwrap(), "Label");
        assert_eq!(edges[0]["lhsDir"].as_str().unwrap(), "L");
        assert_eq!(edges[0]["rhsDir"].as_str().unwrap(), "R");
    }

    #[test]
    fn architecture_align_layout_hints_are_parsed() {
        let model = parse(
            "architecture-beta\n  group api(cloud)[API]\n  service db1(database)[DB1] in api\n  service db2(database)[DB2] in api\n  service db3(database)[DB3] in api\n  junction join\n  align row db1 db2 db3\n  align column db2 join\n",
        );
        assert_eq!(
            model["layoutHints"],
            serde_json::json!([
                {"direction": "row", "members": ["db1", "db2", "db3"]},
                {"direction": "column", "members": ["db2", "join"]}
            ])
        );
    }

    #[test]
    fn architecture_align_editor_facts_preserve_spans() {
        let text = "architecture-beta\n  service rowspan(server)[Rowspan]\n  service columnar(server)[Columnar]\n  align row rowspan columnar\n";
        let facts = parse_architecture_editor_facts(text, &test_meta());

        let row_start = text.find("align row").unwrap() + "align ".len();
        assert_eq!(
            payload_selection(&facts, "architecture alignment direction", "row"),
            SourceSpan::new(row_start, row_start + "row".len())
        );

        for member in ["rowspan", "columnar"] {
            let member_start = text.rfind(member).unwrap();
            assert_eq!(
                facts
                    .symbols
                    .iter()
                    .find(|symbol| {
                        symbol.detail.as_deref() == Some("architecture alignment member")
                            && symbol.name == member
                    })
                    .unwrap_or_else(|| panic!("missing alignment member symbol {member}"))
                    .selection,
                SourceSpan::new(member_start, member_start + member.len())
            );
        }
    }

    #[test]
    fn architecture_align_rejects_unknown_member_with_exact_span() {
        let text = "architecture-beta\n  service a(server)[A]\n  service b(server)[B]\n  align row a b ghost\n";
        let diagnostic = parse_err(text);
        let offset = text.find("ghost").unwrap();

        assert!(diagnostic.message().contains("ghost"));
        assert_eq!(
            diagnostic.span(),
            Some(SourceSpan::new(offset, offset + "ghost".len()))
        );
        assert_eq!(diagnostic.span_kind(), ParseDiagnosticSpanKind::Exact);
    }

    #[test]
    fn architecture_align_rejects_duplicate_member_with_exact_span() {
        let text = "architecture-beta\n  service a(server)[A]\n  align row a a\n";
        let diagnostic = parse_err(text);
        let offset = text.rfind("a").unwrap();

        assert!(diagnostic.message().contains("more than once"));
        assert_eq!(diagnostic.span(), Some(SourceSpan::new(offset, offset + 1)));
        assert_eq!(diagnostic.span_kind(), ParseDiagnosticSpanKind::Exact);
    }

    #[test]
    fn architecture_rejects_legacy_edge_shorthand() {
        let engine = Engine::new();
        let err = block_on(engine.parse_diagram(
            "architecture-beta\n  service a\n  service b\n  a (T--B) b\n",
            ParseOptions::default(),
        ))
        .unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("expected ':' for lhs port") || msg.contains("unrecognized"));
    }

    #[test]
    fn architecture_invalid_service_id_reports_insertion_point() {
        let text = "architecture-beta\n  service -bad\n";
        let diagnostic = parse_err(text);
        let offset = text.find("-bad").unwrap();

        assert_eq!(diagnostic.message(), "invalid service id");
        assert_eq!(diagnostic.span(), Some(SourceSpan::new(offset, offset)));
        assert_eq!(
            diagnostic.span_kind(),
            ParseDiagnosticSpanKind::InsertionPoint
        );
    }

    #[test]
    fn architecture_invalid_edge_direction_reports_exact_token_span() {
        let text = "architecture-beta\n  service a\n  service b\n  a:X -- R:b\n";
        let diagnostic = parse_err(text);
        let offset = text.find('X').unwrap();

        assert_eq!(diagnostic.message(), "invalid lhs direction");
        assert_eq!(diagnostic.span(), Some(SourceSpan::new(offset, offset + 1)));
        assert_eq!(diagnostic.span_kind(), ParseDiagnosticSpanKind::Exact);
    }

    #[test]
    fn architecture_trailing_group_input_reports_exact_token_span() {
        let text = "architecture-beta\n  group core extra\n";
        let diagnostic = parse_err(text);
        let offset = text.find("extra").unwrap();

        assert_eq!(diagnostic.message(), "unexpected trailing input");
        assert_eq!(
            diagnostic.span(),
            Some(SourceSpan::new(offset, offset + "extra".len()))
        );
        assert_eq!(diagnostic.span_kind(), ParseDiagnosticSpanKind::Exact);
    }

    #[test]
    fn architecture_duplicate_service_reports_exact_id_span() {
        let text = "architecture-beta\n  service api\n  service api\n";
        let diagnostic = parse_err(text);
        let offset = text.rfind("api").unwrap();

        assert!(diagnostic.message().contains("already in use"));
        assert_eq!(
            diagnostic.span(),
            Some(SourceSpan::new(offset, offset + "api".len()))
        );
        assert_eq!(diagnostic.span_kind(), ParseDiagnosticSpanKind::Exact);
    }

    #[test]
    fn architecture_unknown_parent_reports_exact_reference_span() {
        let text = "architecture-beta\n  service api in missing\n";
        let diagnostic = parse_err(text);
        let offset = text.find("missing").unwrap();

        assert!(diagnostic.message().contains("parent does not exist"));
        assert_eq!(
            diagnostic.span(),
            Some(SourceSpan::new(offset, offset + "missing".len()))
        );
        assert_eq!(diagnostic.span_kind(), ParseDiagnosticSpanKind::Exact);
    }

    #[test]
    fn architecture_unknown_edge_endpoint_reports_exact_reference_span() {
        let text = "architecture-beta\n  service api\n  api:L -- R:missing\n";
        let diagnostic = parse_err(text);
        let offset = text.find("missing").unwrap();

        assert!(diagnostic.message().contains("right-hand id"));
        assert_eq!(
            diagnostic.span(),
            Some(SourceSpan::new(offset, offset + "missing".len()))
        );
        assert_eq!(diagnostic.span_kind(), ParseDiagnosticSpanKind::Exact);
    }
}
