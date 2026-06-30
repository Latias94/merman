use crate::diagrams::scan::strip_line_ending;
use crate::sanitize::sanitize_text;
use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
    EditorSemanticRole, EditorSemanticSymbol, Error, MermaidConfig, ParseMetadata, Result,
    SourceSpan,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum C4Text {
    Wrapped { text: Value },
    String(String),
    Value(Value),
}

impl Default for C4Text {
    fn default() -> Self {
        Self::String(String::new())
    }
}

impl C4Text {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Wrapped { text } => text.as_str().unwrap_or(""),
            Self::String(s) => s.as_str(),
            Self::Value(v) => v.as_str().unwrap_or(""),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct C4LayoutConfig {
    #[serde(default, rename = "c4ShapeInRow")]
    pub c4_shape_in_row: i64,
    #[serde(default, rename = "c4BoundaryInRow")]
    pub c4_boundary_in_row: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct C4ShapeRenderModel {
    pub alias: String,
    #[serde(default, rename = "parentBoundary")]
    pub parent_boundary: String,
    #[serde(default, rename = "typeC4Shape")]
    pub type_c4_shape: C4Text,
    #[serde(default)]
    pub label: C4Text,
    #[serde(default)]
    pub wrap: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sprite: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<Value>,
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    pub ty: Option<C4Text>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub techn: Option<C4Text>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub descr: Option<C4Text>,
    #[serde(default, rename = "bgColor", skip_serializing_if = "Option::is_none")]
    pub bg_color: Option<String>,
    #[serde(
        default,
        rename = "borderColor",
        skip_serializing_if = "Option::is_none"
    )]
    pub border_color: Option<String>,
    #[serde(default, rename = "fontColor", skip_serializing_if = "Option::is_none")]
    pub font_color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadowing: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shape: Option<Value>,
    #[serde(
        default,
        rename = "legendText",
        skip_serializing_if = "Option::is_none"
    )]
    pub legend_text: Option<Value>,
    #[serde(
        default,
        rename = "legendSprite",
        skip_serializing_if = "Option::is_none"
    )]
    pub legend_sprite: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct C4BoundaryRenderModel {
    pub alias: String,
    #[serde(default, rename = "parentBoundary")]
    pub parent_boundary: String,
    #[serde(default)]
    pub label: C4Text,
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    pub ty: Option<C4Text>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub descr: Option<C4Text>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wrap: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sprite: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<Value>,
    #[serde(default, rename = "nodeType", skip_serializing_if = "Option::is_none")]
    pub node_type: Option<String>,
    #[serde(default, rename = "bgColor", skip_serializing_if = "Option::is_none")]
    pub bg_color: Option<String>,
    #[serde(
        default,
        rename = "borderColor",
        skip_serializing_if = "Option::is_none"
    )]
    pub border_color: Option<String>,
    #[serde(default, rename = "fontColor", skip_serializing_if = "Option::is_none")]
    pub font_color: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct C4RelRenderModel {
    #[serde(rename = "from")]
    pub from_alias: String,
    #[serde(rename = "to")]
    pub to_alias: String,
    #[serde(rename = "type")]
    pub rel_type: String,
    #[serde(default)]
    pub label: C4Text,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub techn: Option<C4Text>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub descr: Option<C4Text>,
    #[serde(default)]
    pub wrap: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sprite: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<Value>,
    #[serde(default, rename = "offsetX", skip_serializing_if = "Option::is_none")]
    pub offset_x: Option<i64>,
    #[serde(default, rename = "offsetY", skip_serializing_if = "Option::is_none")]
    pub offset_y: Option<i64>,
    #[serde(default, rename = "lineColor", skip_serializing_if = "Option::is_none")]
    pub line_color: Option<String>,
    #[serde(default, rename = "textColor", skip_serializing_if = "Option::is_none")]
    pub text_color: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct C4DiagramRenderModel {
    #[serde(default, rename = "c4Type")]
    pub c4_type: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default, rename = "accTitle")]
    pub acc_title: Option<String>,
    #[serde(default, rename = "accDescr")]
    pub acc_descr: Option<String>,
    #[serde(default)]
    pub wrap: bool,
    #[serde(default)]
    pub layout: C4LayoutConfig,
    #[serde(default)]
    pub shapes: Vec<C4ShapeRenderModel>,
    #[serde(default)]
    pub boundaries: Vec<C4BoundaryRenderModel>,
    #[serde(default)]
    pub rels: Vec<C4RelRenderModel>,
}

impl C4DiagramRenderModel {
    pub(crate) fn sanitize_common_db_fields(&mut self, config: &crate::MermaidConfig) {
        crate::common_db::sanitize_optional_title(&mut self.title, config);
        crate::common_db::sanitize_optional_acc_title(&mut self.acc_title, config);
        crate::common_db::sanitize_optional_acc_descr(&mut self.acc_descr, config);
    }
}

#[derive(Debug, Default)]
struct C4Db {
    c4_type: String,
    title: String,
    acc_descr: String,
    wrap_enabled: bool,

    current_boundary: String,
    parent_boundary: String,
    boundary_stack: Vec<String>,

    boundaries: Vec<Map<String, Value>>,
    boundary_index: HashMap<String, usize>,

    shapes: Vec<Map<String, Value>>,
    shape_index: HashMap<String, usize>,

    rels: Vec<Map<String, Value>>,

    c4_shape_in_row: i64,
    c4_boundary_in_row: i64,
}

#[derive(Debug, Clone)]
struct SpannedText {
    text: String,
    span: SourceSpan,
}

#[derive(Debug, Clone)]
struct SpannedKvArg {
    key: String,
    value: SpannedText,
}

#[derive(Debug, Clone)]
enum SpannedArgValue {
    Text(SpannedText),
    KeyValue(SpannedKvArg),
}

#[derive(Debug, Clone)]
struct SpannedArg {
    value: SpannedArgValue,
}

impl SpannedArg {
    fn text(&self) -> &str {
        match &self.value {
            SpannedArgValue::Text(value) => value.text.as_str(),
            SpannedArgValue::KeyValue(value) => value.value.text.as_str(),
        }
    }

    fn span(&self) -> SourceSpan {
        match &self.value {
            SpannedArgValue::Text(value) => value.span,
            SpannedArgValue::KeyValue(value) => value.value.span,
        }
    }

    fn key(&self) -> Option<&str> {
        match &self.value {
            SpannedArgValue::Text(_) => None,
            SpannedArgValue::KeyValue(value) => Some(value.key.as_str()),
        }
    }
}

#[derive(Debug, Clone)]
struct SpannedMacroStmt {
    name: String,
    args: Vec<SpannedArg>,
    span: SourceSpan,
}

pub fn parse_c4(code: &str, meta: &ParseMetadata) -> Result<Value> {
    Ok(parse_c4_db(code, meta)?.to_model(meta))
}

pub fn parse_c4_model_for_render(code: &str, meta: &ParseMetadata) -> Result<C4DiagramRenderModel> {
    parse_c4_db(code, meta)?.to_render_model()
}

pub fn parse_c4_editor_facts(code: &str, _meta: &ParseMetadata) -> EditorSemanticFacts {
    let mut facts = EditorSemanticFacts::new();
    let mut header_seen = false;
    let mut offset = 0usize;
    let mut lines = code.split_inclusive('\n').peekable();

    while let Some(segment) = lines.next() {
        let line_start = offset;
        offset += segment.len();
        let raw_line = strip_line_ending(segment);
        let line = strip_inline_comment(raw_line);
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if !header_seen {
            if trimmed.starts_with("%%") {
                continue;
            }
            if is_c4_header(trimmed) {
                header_seen = true;
                continue;
            }
            return facts;
        }

        if trimmed.starts_with("%%") || trimmed == "{" || trimmed == "}" {
            continue;
        }

        if let Some(value) = parse_title_spanned_c4(&line, line_start) {
            facts.push_directive_prefix("title");
            push_c4_payload_fact(&mut facts, &value, "c4 title");
            continue;
        }

        if let Some(value) = parse_acc_title_spanned_c4(&line, line_start) {
            facts.push_directive_prefix("accTitle");
            push_c4_payload_fact(&mut facts, &value, "c4 accessibility title");
            continue;
        }

        if let Some(value) = parse_acc_description_stmt_spanned_c4(&line, line_start) {
            facts.push_directive_prefix("accDescription");
            push_c4_payload_fact(&mut facts, &value, "c4 accessibility description");
            continue;
        }

        if let Some((value, closed)) =
            parse_acc_descr_spanned_c4(&mut lines, &line, line_start, &mut offset)
        {
            facts.push_directive_prefix("accDescr");
            push_c4_payload_fact(&mut facts, &value, "c4 accessibility description");
            if !closed {
                facts.mark_recovered_with_diagnostic(
                    "unterminated C4 accDescr block",
                    Some(value.span),
                );
            }
            continue;
        }

        if parse_direction_stmt_facts_c4(&line, line_start, &mut facts) {
            continue;
        }

        let stmt_start = line_start + line.find(trimmed).unwrap_or(0);
        match parse_macro_stmt_spanned(trimmed, stmt_start) {
            Ok(Some(stmt)) => {
                if parse_macro_stmt_facts_c4(&stmt, &mut facts).is_err() {
                    facts.mark_recovered_with_diagnostic(
                        "unable to recover C4 statement semantics",
                        Some(stmt.span),
                    );
                }
            }
            Ok(None) => {
                let span = SourceSpan::new(stmt_start, stmt_start + trimmed.len());
                facts.mark_recovered_with_diagnostic("unsupported C4 statement", Some(span));
            }
            Err(_) => {
                let span = SourceSpan::new(stmt_start, stmt_start + trimmed.len());
                facts.mark_recovered_with_diagnostic("unable to parse C4 statement", Some(span));
            }
        }
    }

    facts
}

fn is_c4_header(line: &str) -> bool {
    matches!(
        line.trim(),
        "C4Context" | "C4Container" | "C4Component" | "C4Dynamic" | "C4Deployment"
    )
}

fn push_c4_entity_fact(
    facts: &mut EditorSemanticFacts,
    value: &SpannedText,
    detail: impl Into<String>,
) {
    if value.text.is_empty() {
        facts.push_expected_syntax(EditorExpectedSyntax::new(
            EditorExpectedSyntaxKind::NodeIdentifier,
            value.span,
        ));
        return;
    }

    facts.push_symbol(EditorSemanticSymbol::with_role(
        value.text.clone(),
        Some(detail.into()),
        EditorSemanticKind::Object,
        EditorSemanticRole::Entity,
        value.span,
        value.span,
    ));
}

fn push_c4_payload_fact(
    facts: &mut EditorSemanticFacts,
    value: &SpannedText,
    detail: impl Into<String>,
) {
    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::Payload,
        value.span,
    ));
    if value.text.is_empty() {
        return;
    }
    facts.push_symbol(EditorSemanticSymbol::payload(
        value.text.clone(),
        Some(detail.into()),
        EditorSemanticKind::String,
        value.span,
        value.span,
    ));
}

fn push_c4_payload_arg(facts: &mut EditorSemanticFacts, arg: &SpannedArg, fallback_detail: &str) {
    let detail = arg
        .key()
        .map(|key| format!("c4 {key}"))
        .unwrap_or_else(|| fallback_detail.to_string());
    let value = SpannedText {
        text: arg.text().to_string(),
        span: arg.span(),
    };
    push_c4_payload_fact(facts, &value, detail);
}

fn push_c4_entity_arg(facts: &mut EditorSemanticFacts, arg: &SpannedArg, detail: &str) {
    let value = SpannedText {
        text: arg.text().to_string(),
        span: arg.span(),
    };
    push_c4_entity_fact(facts, &value, detail.to_string());
}

fn parse_title_spanned_c4(line: &str, line_start: usize) -> Option<SpannedText> {
    let trimmed = line.trim_start();
    let rest = trimmed.strip_prefix("title")?;
    let ws = rest.chars().next()?;
    if !ws.is_whitespace() {
        return None;
    }
    let value = rest.trim_start();
    let value_rel = line.find(value)?;
    Some(SpannedText {
        text: value.to_string(),
        span: SourceSpan::new(line_start + value_rel, line_start + value_rel + value.len()),
    })
}

fn parse_acc_title_spanned_c4(line: &str, line_start: usize) -> Option<SpannedText> {
    let trimmed = line.trim_start();
    let rest = trimmed.strip_prefix("accTitle")?.trim_start();
    let value = rest.strip_prefix(':')?.trim();
    let value_rel = line.find(value)?;
    Some(SpannedText {
        text: value.to_string(),
        span: SourceSpan::new(line_start + value_rel, line_start + value_rel + value.len()),
    })
}

fn parse_acc_description_stmt_spanned_c4(line: &str, line_start: usize) -> Option<SpannedText> {
    let trimmed = line.trim_start();
    let rest = trimmed.strip_prefix("accDescription")?;
    let ws = rest.chars().next()?;
    if !ws.is_whitespace() {
        return None;
    }
    let value = rest.trim_start();
    let value_rel = line.find(value)?;
    Some(SpannedText {
        text: value.to_string(),
        span: SourceSpan::new(line_start + value_rel, line_start + value_rel + value.len()),
    })
}

fn parse_acc_descr_spanned_c4<'a>(
    lines: &mut std::iter::Peekable<std::str::SplitInclusive<'a, char>>,
    line: &str,
    line_start: usize,
    offset: &mut usize,
) -> Option<(SpannedText, bool)> {
    let trimmed = line.trim_start();
    let rest = trimmed.strip_prefix("accDescr")?.trim_start();
    if let Some(after) = rest.strip_prefix(':') {
        let value = after.trim();
        let value_rel = line.find(value)?;
        return Some((
            SpannedText {
                text: value.to_string(),
                span: SourceSpan::new(line_start + value_rel, line_start + value_rel + value.len()),
            },
            true,
        ));
    }

    let rest = rest.strip_prefix('{')?;
    if let Some(end) = rest.find('}') {
        let value = rest[..end].trim();
        let value_rel = line.find(value)?;
        return Some((
            SpannedText {
                text: value.to_string(),
                span: SourceSpan::new(line_start + value_rel, line_start + value_rel + value.len()),
            },
            true,
        ));
    }

    let mut parts = Vec::new();
    let mut span_start = None;
    let mut span_end = None;

    let first = rest.trim();
    if !first.is_empty() {
        let rel = line.find(first)?;
        parts.push(first.to_string());
        span_start = Some(line_start + rel);
        span_end = Some(line_start + rel + first.len());
    }

    let mut closed = false;
    while let Some(segment) = lines.next() {
        let segment_start = *offset;
        *offset += segment.len();
        let next_line = strip_line_ending(segment);
        if let Some(close_pos) = next_line.find('}') {
            let before = next_line[..close_pos].trim();
            if !before.is_empty() {
                let rel = next_line.find(before)?;
                parts.push(before.to_string());
                span_start.get_or_insert(segment_start + rel);
                span_end = Some(segment_start + rel + before.len());
            }
            closed = true;
            break;
        }

        let text = next_line.trim();
        if text.is_empty() {
            continue;
        }
        let rel = next_line.find(text)?;
        parts.push(text.to_string());
        span_start.get_or_insert(segment_start + rel);
        span_end = Some(segment_start + rel + text.len());
    }

    let start = span_start.unwrap_or_else(|| line_start + line.find('{').unwrap_or(line.len()));
    let end = span_end.unwrap_or(start);
    Some((
        SpannedText {
            text: parts.join("\n"),
            span: SourceSpan::new(start, end),
        },
        closed,
    ))
}

fn parse_direction_stmt_facts_c4(
    line: &str,
    line_start: usize,
    facts: &mut EditorSemanticFacts,
) -> bool {
    let trimmed = line.trim_start();
    let Some(rest) = trimmed.strip_prefix("direction") else {
        return false;
    };
    if rest.chars().next().is_some_and(|ch| !ch.is_whitespace()) {
        return false;
    }

    let value = rest.trim_start();
    if value.is_empty() {
        let span_start = line_start + line.find("direction").unwrap_or(0) + "direction".len();
        facts.push_expected_syntax(EditorExpectedSyntax::new(
            EditorExpectedSyntaxKind::DirectionValue,
            SourceSpan::new(span_start, span_start),
        ));
        return true;
    }

    let token = value.split_whitespace().next().unwrap_or(value);
    let value_rel = line.find(token).unwrap_or(0);
    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::DirectionValue,
        SourceSpan::new(line_start + value_rel, line_start + value_rel + token.len()),
    ));
    true
}

fn parse_macro_stmt_facts_c4(
    stmt: &SpannedMacroStmt,
    facts: &mut EditorSemanticFacts,
) -> Result<()> {
    match stmt.name.as_str() {
        "Person" | "Person_Ext" | "System" | "SystemDb" | "SystemQueue" | "System_Ext"
        | "SystemDb_Ext" | "SystemQueue_Ext" => {
            if let Some(alias) = stmt.args.first() {
                push_c4_entity_arg(facts, alias, c4_shape_detail(&stmt.name));
            }
            if let Some(label) = stmt.args.get(1) {
                push_c4_payload_arg(facts, label, "c4 label");
            }
            if let Some(descr) = stmt.args.get(2) {
                push_c4_payload_arg(facts, descr, "c4 description");
            }
            for arg in stmt.args.iter().skip(3) {
                push_c4_payload_arg(facts, arg, "c4 value");
            }
            Ok(())
        }
        "Container" | "ContainerDb" | "ContainerQueue" | "Container_Ext" | "ContainerDb_Ext"
        | "ContainerQueue_Ext" | "Component" | "ComponentDb" | "ComponentQueue"
        | "Component_Ext" | "ComponentDb_Ext" | "ComponentQueue_Ext" => {
            if let Some(alias) = stmt.args.first() {
                push_c4_entity_arg(facts, alias, c4_shape_detail(&stmt.name));
            }
            if let Some(label) = stmt.args.get(1) {
                push_c4_payload_arg(facts, label, "c4 label");
            }
            if let Some(techn) = stmt.args.get(2) {
                push_c4_payload_arg(facts, techn, "c4 technology");
            }
            if let Some(descr) = stmt.args.get(3) {
                push_c4_payload_arg(facts, descr, "c4 description");
            }
            for arg in stmt.args.iter().skip(4) {
                push_c4_payload_arg(facts, arg, "c4 value");
            }
            Ok(())
        }
        "Boundary" | "Enterprise_Boundary" | "System_Boundary" | "Container_Boundary" => {
            if let Some(alias) = stmt.args.first() {
                push_c4_entity_arg(facts, alias, "c4 boundary");
            }
            if let Some(label) = stmt.args.get(1) {
                push_c4_payload_arg(facts, label, "c4 label");
            }
            if let Some(boundary_type) = stmt.args.get(2) {
                push_c4_payload_arg(facts, boundary_type, "c4 boundary type");
            }
            for arg in stmt.args.iter().skip(3) {
                push_c4_payload_arg(facts, arg, "c4 value");
            }
            Ok(())
        }
        "Node" | "Deployment_Node" | "Node_L" | "Node_R" => {
            if let Some(alias) = stmt.args.first() {
                push_c4_entity_arg(facts, alias, "c4 deployment node");
            }
            if let Some(label) = stmt.args.get(1) {
                push_c4_payload_arg(facts, label, "c4 label");
            }
            if let Some(node_type) = stmt.args.get(2) {
                push_c4_payload_arg(facts, node_type, "c4 node type");
            }
            if let Some(descr) = stmt.args.get(3) {
                push_c4_payload_arg(facts, descr, "c4 description");
            }
            for arg in stmt.args.iter().skip(4) {
                push_c4_payload_arg(facts, arg, "c4 value");
            }
            Ok(())
        }
        "Rel" | "BiRel" | "Rel_U" | "Rel_Up" | "Rel_D" | "Rel_Down" | "Rel_L" | "Rel_Left"
        | "Rel_R" | "Rel_Right" | "Rel_Back" => {
            let Some(from) = stmt.args.first() else {
                return Err(Error::diagram_parse_fallback(
                    "c4".to_string(),
                    "missing relation source".to_string(),
                ));
            };
            push_c4_entity_arg(facts, from, "c4 relation source");
            let Some(to) = stmt.args.get(1) else {
                return Err(Error::diagram_parse_fallback(
                    "c4".to_string(),
                    "missing relation target".to_string(),
                ));
            };
            push_c4_entity_arg(facts, to, "c4 relation target");
            if let Some(label) = stmt.args.get(2) {
                push_c4_payload_arg(facts, label, "c4 relation label");
            } else {
                return Ok(());
            }
            if let Some(techn) = stmt.args.get(3) {
                push_c4_payload_arg(facts, techn, "c4 technology");
            }
            if let Some(descr) = stmt.args.get(4) {
                push_c4_payload_arg(facts, descr, "c4 description");
            }
            for arg in stmt.args.iter().skip(5) {
                push_c4_payload_arg(facts, arg, "c4 value");
            }
            Ok(())
        }
        "RelIndex" => {
            let Some(index) = stmt.args.first() else {
                return Err(Error::diagram_parse_fallback(
                    "c4".to_string(),
                    "missing relation index".to_string(),
                ));
            };
            push_c4_payload_arg(facts, index, "c4 relation index");
            let Some(from) = stmt.args.get(1) else {
                return Err(Error::diagram_parse_fallback(
                    "c4".to_string(),
                    "missing relation source".to_string(),
                ));
            };
            push_c4_entity_arg(facts, from, "c4 relation source");
            let Some(to) = stmt.args.get(2) else {
                return Err(Error::diagram_parse_fallback(
                    "c4".to_string(),
                    "missing relation target".to_string(),
                ));
            };
            push_c4_entity_arg(facts, to, "c4 relation target");
            if let Some(label) = stmt.args.get(3) {
                push_c4_payload_arg(facts, label, "c4 relation label");
            } else {
                return Ok(());
            }
            if let Some(techn) = stmt.args.get(4) {
                push_c4_payload_arg(facts, techn, "c4 technology");
            }
            if let Some(descr) = stmt.args.get(5) {
                push_c4_payload_arg(facts, descr, "c4 description");
            }
            for arg in stmt.args.iter().skip(6) {
                push_c4_payload_arg(facts, arg, "c4 value");
            }
            Ok(())
        }
        "UpdateElementStyle" => {
            let Some(target) = stmt.args.first() else {
                return Err(Error::diagram_parse_fallback(
                    "c4".to_string(),
                    "missing style target".to_string(),
                ));
            };
            push_c4_entity_arg(facts, target, "c4 style target");
            for arg in stmt.args.iter().skip(1) {
                push_c4_payload_arg(facts, arg, "c4 style value");
            }
            Ok(())
        }
        "UpdateRelStyle" => {
            let Some(from) = stmt.args.first() else {
                return Err(Error::diagram_parse_fallback(
                    "c4".to_string(),
                    "missing relation style source".to_string(),
                ));
            };
            push_c4_entity_arg(facts, from, "c4 relation style source");
            let Some(to) = stmt.args.get(1) else {
                return Err(Error::diagram_parse_fallback(
                    "c4".to_string(),
                    "missing relation style target".to_string(),
                ));
            };
            push_c4_entity_arg(facts, to, "c4 relation style target");
            for arg in stmt.args.iter().skip(2) {
                push_c4_payload_arg(facts, arg, "c4 relation style value");
            }
            Ok(())
        }
        "UpdateLayoutConfig" => {
            for arg in &stmt.args {
                push_c4_payload_arg(facts, arg, "c4 layout value");
            }
            Ok(())
        }
        other => Err(Error::diagram_parse_fallback(
            "c4".to_string(),
            format!("unsupported C4 macro: {other}"),
        )),
    }
}

fn c4_shape_detail(name: &str) -> &'static str {
    match name {
        "Person" | "Person_Ext" => "c4 person",
        "System" | "SystemDb" | "SystemQueue" | "System_Ext" | "SystemDb_Ext"
        | "SystemQueue_Ext" => "c4 system",
        "Container" | "ContainerDb" | "ContainerQueue" | "Container_Ext" | "ContainerDb_Ext"
        | "ContainerQueue_Ext" => "c4 container",
        "Component" | "ComponentDb" | "ComponentQueue" | "Component_Ext" | "ComponentDb_Ext"
        | "ComponentQueue_Ext" => "c4 component",
        _ => "c4 element",
    }
}

fn parse_macro_stmt_spanned(t: &str, stmt_start: usize) -> Result<Option<SpannedMacroStmt>> {
    let t = t.trim_end();
    let Some(paren) = t.find('(') else {
        return Ok(None);
    };
    let name = t[..paren].trim().to_string();
    if name.is_empty() {
        return Ok(None);
    }

    let after = &t[paren + 1..];
    let Some(end_paren) = after.rfind(')') else {
        return Err(Error::diagram_parse_fallback(
            "c4".to_string(),
            format!("unterminated macro call: {t}"),
        ));
    };

    let args_raw = &after[..end_paren];
    let rest = after[end_paren + 1..].trim();
    let mut has_lbrace = false;
    if let Some(after) = rest.strip_prefix('{') {
        if after.trim().is_empty() {
            has_lbrace = true;
        } else {
            return Err(Error::diagram_parse_fallback(
                "c4".to_string(),
                format!("unexpected tokens after '{{' in macro: {t}"),
            ));
        }
    } else if !rest.is_empty() {
        return Err(Error::diagram_parse_fallback(
            "c4".to_string(),
            format!("unexpected trailing tokens in macro: {t}"),
        ));
    }

    let args = parse_args_csv_spanned(args_raw, stmt_start + paren + 1)?;
    let _ = has_lbrace;
    Ok(Some(SpannedMacroStmt {
        name,
        args,
        span: SourceSpan::new(stmt_start, stmt_start + t.len()),
    }))
}

fn parse_args_csv_spanned(input: &str, base_offset: usize) -> Result<Vec<SpannedArg>> {
    let mut out = Vec::new();
    let mut cur = input;
    let mut cursor = base_offset;
    loop {
        if cur.trim().is_empty() {
            break;
        }
        let (seg, rest) = split_next_arg(cur);
        out.push(parse_arg_spanned(seg, cursor)?);
        let Some(rest) = rest else {
            break;
        };
        cursor += seg.len() + 1;
        cur = rest;
    }
    Ok(out)
}

fn parse_arg_spanned(seg: &str, seg_base: usize) -> Result<SpannedArg> {
    let trimmed_start = seg
        .char_indices()
        .find(|(_, ch)| !ch.is_whitespace())
        .map(|(idx, _)| idx)
        .unwrap_or(seg.len());
    let trimmed_end = seg
        .char_indices()
        .rev()
        .find(|(_, ch)| !ch.is_whitespace())
        .map(|(idx, ch)| idx + ch.len_utf8())
        .unwrap_or(trimmed_start);
    let trimmed = &seg[trimmed_start..trimmed_end];
    let value_base = seg_base + trimmed_start;

    if trimmed.is_empty() {
        return Ok(SpannedArg {
            value: SpannedArgValue::Text(SpannedText {
                text: String::new(),
                span: SourceSpan::new(value_base, seg_base + trimmed_end),
            }),
        });
    }

    if let Some(value) = try_parse_kv_spanned(trimmed, value_base)? {
        return Ok(value);
    }

    if trimmed.starts_with('"') {
        return Ok(SpannedArg {
            value: SpannedArgValue::Text(parse_quoted_spanned(trimmed, value_base)?),
        });
    }

    Ok(SpannedArg {
        value: SpannedArgValue::Text(SpannedText {
            text: trimmed.to_string(),
            span: SourceSpan::new(value_base, value_base + trimmed.len()),
        }),
    })
}

fn try_parse_kv_spanned(seg: &str, seg_base: usize) -> Result<Option<SpannedArg>> {
    if !seg.starts_with('$') {
        return Ok(None);
    }
    let rest = &seg[1..];
    let Some(eq) = rest.find('=') else {
        return Err(Error::diagram_parse_fallback(
            "c4".to_string(),
            format!("invalid attribute kv: {seg}"),
        ));
    };
    let key = rest[..eq].trim();
    if key.is_empty() {
        return Err(Error::diagram_parse_fallback(
            "c4".to_string(),
            format!("invalid attribute kv key: {seg}"),
        ));
    }

    let val_raw = rest[eq + 1..].trim_start();
    let leading_ws = rest[eq + 1..]
        .char_indices()
        .find(|(_, ch)| !ch.is_whitespace())
        .map(|(idx, _)| idx)
        .unwrap_or(rest[eq + 1..].len());
    let value = parse_quoted_spanned(val_raw, seg_base + 1 + eq + 1 + leading_ws)?;
    Ok(Some(SpannedArg {
        value: SpannedArgValue::KeyValue(SpannedKvArg {
            key: key.to_string(),
            value,
        }),
    }))
}

fn parse_quoted_spanned(input: &str, input_base: usize) -> Result<SpannedText> {
    let input = input.trim();
    let Some(rest) = input.strip_prefix('"') else {
        return Err(Error::diagram_parse_fallback(
            "c4".to_string(),
            format!("expected quoted string, got: {input}"),
        ));
    };
    let Some(end) = rest.find('"') else {
        return Err(Error::diagram_parse_fallback(
            "c4".to_string(),
            "unterminated string".to_string(),
        ));
    };
    let value = &rest[..end];
    let trailing = rest[end + 1..].trim();
    if !trailing.is_empty() {
        return Err(Error::diagram_parse_fallback(
            "c4".to_string(),
            format!("unexpected trailing tokens after string: {trailing}"),
        ));
    }
    Ok(SpannedText {
        text: value.to_string(),
        span: SourceSpan::new(input_base + 1, input_base + 1 + value.len()),
    })
}

impl C4Db {
    fn new(config: &MermaidConfig) -> Self {
        let wrap_enabled = config.get_bool("wrap").unwrap_or(false);
        let mut db = Self {
            wrap_enabled,
            current_boundary: "global".to_string(),
            c4_shape_in_row: 4,
            c4_boundary_in_row: 2,
            ..Default::default()
        };
        db.boundary_stack.push(String::new());
        db.add_global_boundary();
        db
    }

    fn add_global_boundary(&mut self) {
        let mut obj = Map::new();
        obj.insert("alias".to_string(), json!("global"));
        obj.insert("label".to_string(), wrap_text(json!("global")));
        obj.insert("type".to_string(), wrap_text(json!("global")));
        obj.insert("tags".to_string(), Value::Null);
        obj.insert("link".to_string(), Value::Null);
        obj.insert("parentBoundary".to_string(), json!(""));
        self.boundary_index
            .insert("global".to_string(), self.boundaries.len());
        self.boundaries.push(obj);
    }

    fn ensure_shape(&mut self, alias: &str) -> &mut Map<String, Value> {
        let shapes = &mut self.shapes;
        let idx = *self
            .shape_index
            .entry(alias.to_string())
            .or_insert_with(|| {
                let idx = shapes.len();
                let mut obj = Map::new();
                obj.insert("alias".to_string(), json!(alias));
                shapes.push(obj);
                idx
            });
        &mut shapes[idx]
    }

    fn ensure_boundary(&mut self, alias: &str) -> &mut Map<String, Value> {
        let boundaries = &mut self.boundaries;
        let idx = *self
            .boundary_index
            .entry(alias.to_string())
            .or_insert_with(|| {
                let idx = boundaries.len();
                let mut obj = Map::new();
                obj.insert("alias".to_string(), json!(alias));
                boundaries.push(obj);
                idx
            });
        &mut boundaries[idx]
    }

    fn ensure_relation(&mut self, from: &str, to: &str) -> &mut Map<String, Value> {
        if let Some(idx) = self
            .rels
            .iter()
            .position(|r| r.get("from") == Some(&json!(from)) && r.get("to") == Some(&json!(to)))
        {
            return &mut self.rels[idx];
        }
        self.rels.push(Map::new());
        let idx = self.rels.len() - 1;
        &mut self.rels[idx]
    }

    fn set_c4_type(&mut self, raw: &str, config: &MermaidConfig) {
        self.c4_type = sanitize_text(raw, config);
    }

    fn set_title(&mut self, raw: &str, config: &MermaidConfig) {
        self.title = sanitize_text(raw, config);
    }

    fn set_acc_description(&mut self, raw: &str) {
        self.acc_descr = raw.to_string();
    }

    fn add_person_or_system(&mut self, type_c4_shape: &str, args: &[Value]) -> Result<()> {
        let alias = arg_to_string(args.first())?;
        let label = args.get(1).cloned().unwrap_or_else(|| json!(""));
        let descr = args.get(2).cloned();

        let current_boundary = self.current_boundary.clone();
        let wrap_enabled = self.wrap_enabled;
        let obj = self.ensure_shape(&alias);
        obj.insert("label".to_string(), wrap_text(label));
        apply_text_field_or_kv(obj, "descr", descr.unwrap_or_else(|| json!("")))?;
        obj.insert("typeC4Shape".to_string(), wrap_text(json!(type_c4_shape)));
        obj.insert("parentBoundary".to_string(), json!(current_boundary));
        obj.insert("wrap".to_string(), json!(wrap_enabled));

        apply_kv_value(obj, "sprite", args.get(3))?;
        apply_kv_value(obj, "tags", args.get(4))?;
        apply_kv_value(obj, "link", args.get(5))?;
        Ok(())
    }

    fn add_container(&mut self, type_c4_shape: &str, args: &[Value]) -> Result<()> {
        let alias = arg_to_string(args.first())?;
        let label = args.get(1).cloned().unwrap_or_else(|| json!(""));
        let techn = args.get(2).cloned();
        let descr = args.get(3).cloned();

        let current_boundary = self.current_boundary.clone();
        let wrap_enabled = self.wrap_enabled;
        let obj = self.ensure_shape(&alias);
        obj.insert("label".to_string(), wrap_text(label));
        apply_text_field_or_kv(obj, "techn", techn.unwrap_or_else(|| json!("")))?;
        apply_text_field_or_kv(obj, "descr", descr.unwrap_or_else(|| json!("")))?;
        obj.insert("typeC4Shape".to_string(), wrap_text(json!(type_c4_shape)));
        obj.insert("parentBoundary".to_string(), json!(current_boundary));
        obj.insert("wrap".to_string(), json!(wrap_enabled));

        apply_kv_value(obj, "sprite", args.get(4))?;
        apply_kv_value(obj, "tags", args.get(5))?;
        apply_kv_value(obj, "link", args.get(6))?;
        Ok(())
    }

    fn add_component(&mut self, type_c4_shape: &str, args: &[Value]) -> Result<()> {
        self.add_container(type_c4_shape, args)
    }

    fn add_person_or_system_boundary(&mut self, args: Vec<Value>) -> Result<()> {
        let alias = arg_to_string(args.first())?;
        let label = args.get(1).cloned().unwrap_or_else(|| json!(""));
        let boundary_type = args.get(2).cloned();
        let tags = args.get(3).cloned();
        let link = args.get(4).cloned();

        let current_boundary = self.current_boundary.clone();
        let wrap_enabled = self.wrap_enabled;
        let obj = self.ensure_boundary(&alias);
        obj.insert("label".to_string(), wrap_text(label));
        let ty = boundary_type.unwrap_or_else(|| json!("system"));
        apply_text_field_or_kv(obj, "type", ty)?;

        apply_kv_value(obj, "tags", tags.as_ref())?;
        apply_kv_value(obj, "link", link.as_ref())?;

        obj.insert("parentBoundary".to_string(), json!(current_boundary));
        obj.insert("wrap".to_string(), json!(wrap_enabled));

        self.parent_boundary = self.current_boundary.clone();
        self.current_boundary = alias;
        self.boundary_stack.push(self.parent_boundary.clone());

        Ok(())
    }

    fn add_container_boundary(&mut self, args: Vec<Value>) -> Result<()> {
        self.add_person_or_system_boundary(args)
    }

    fn add_deployment_node(&mut self, node_type: &str, args: Vec<Value>) -> Result<()> {
        let alias = arg_to_string(args.first())?;
        let label = args.get(1).cloned().unwrap_or_else(|| json!(""));
        let node_label_type = args.get(2).cloned();
        let descr = args.get(3).cloned();
        let tags = args.get(5).cloned();
        let link = args.get(6).cloned();

        let current_boundary = self.current_boundary.clone();
        let wrap_enabled = self.wrap_enabled;
        let obj = self.ensure_boundary(&alias);
        obj.insert("label".to_string(), wrap_text(label));

        let ty = node_label_type.unwrap_or_else(|| json!("node"));
        apply_text_field_or_kv(obj, "type", ty)?;
        apply_text_field_or_kv(obj, "descr", descr.unwrap_or_else(|| json!("")))?;
        apply_kv_value(obj, "tags", tags.as_ref())?;
        apply_kv_value(obj, "link", link.as_ref())?;

        obj.insert("nodeType".to_string(), json!(node_type));
        obj.insert("parentBoundary".to_string(), json!(current_boundary));
        obj.insert("wrap".to_string(), json!(wrap_enabled));

        self.parent_boundary = self.current_boundary.clone();
        self.current_boundary = alias;
        self.boundary_stack.push(self.parent_boundary.clone());

        Ok(())
    }

    fn pop_boundary_parse_stack(&mut self) {
        self.current_boundary = self.parent_boundary.clone();
        self.boundary_stack.pop();
        self.parent_boundary = self.boundary_stack.pop().unwrap_or_default();
        self.boundary_stack.push(self.parent_boundary.clone());
    }

    fn add_rel(&mut self, rel_type: &str, args: Vec<Value>) -> Result<()> {
        let from = arg_to_string(args.first())?;
        let to = arg_to_string(args.get(1))?;
        let Some(label) = args.get(2).cloned() else {
            return Ok(());
        };

        let wrap_enabled = self.wrap_enabled;
        let rel = self.ensure_relation(&from, &to);

        rel.insert("type".to_string(), json!(rel_type));
        rel.insert("from".to_string(), json!(from));
        rel.insert("to".to_string(), json!(to));
        rel.insert("label".to_string(), wrap_text(label));

        let techn = args.get(3).cloned().unwrap_or_else(|| json!(""));
        apply_text_field_or_kv(rel, "techn", techn)?;
        let descr = args.get(4).cloned().unwrap_or_else(|| json!(""));
        apply_text_field_or_kv(rel, "descr", descr)?;

        apply_kv_value(rel, "sprite", args.get(5))?;
        apply_kv_value(rel, "tags", args.get(6))?;
        apply_kv_value(rel, "link", args.get(7))?;
        rel.insert("wrap".to_string(), json!(wrap_enabled));
        Ok(())
    }

    fn update_el_style(&mut self, args: Vec<Value>) -> Result<()> {
        let element_name = arg_to_string(args.first())?;
        let Some(target) = self
            .shape_index
            .get(&element_name)
            .and_then(|idx| self.shapes.get_mut(*idx))
            .or_else(|| {
                self.boundary_index
                    .get(&element_name)
                    .and_then(|idx| self.boundaries.get_mut(*idx))
            })
        else {
            return Ok(());
        };

        apply_kv_value(target, "bgColor", args.get(1))?;
        apply_kv_value(target, "fontColor", args.get(2))?;
        apply_kv_value(target, "borderColor", args.get(3))?;
        apply_kv_value(target, "shadowing", args.get(4))?;
        apply_kv_value(target, "shape", args.get(5))?;
        apply_kv_value(target, "sprite", args.get(6))?;
        apply_kv_value(target, "techn", args.get(7))?;
        apply_kv_value(target, "legendText", args.get(8))?;
        apply_kv_value(target, "legendSprite", args.get(9))?;
        Ok(())
    }

    fn update_rel_style(&mut self, args: Vec<Value>) -> Result<()> {
        let from = arg_to_string(args.first())?;
        let to = arg_to_string(args.get(1))?;

        let Some(target) = self
            .rels
            .iter_mut()
            .find(|r| r.get("from") == Some(&json!(from)) && r.get("to") == Some(&json!(to)))
        else {
            return Ok(());
        };

        apply_kv_value(target, "textColor", args.get(2))?;
        apply_kv_value(target, "lineColor", args.get(3))?;
        if let Some(v) = args.get(4) {
            apply_int_kv(target, "offsetX", v)?;
        }
        if let Some(v) = args.get(5) {
            apply_int_kv(target, "offsetY", v)?;
        }

        // Mermaid C4 macros accept named parameters (e.g. `$offsetY="-40"`) and users commonly
        // omit earlier optional keys (like `$textColor` / `$lineColor`). Our arg vector is a
        // heterogeneous list of single-key objects, so the above positional probes can still
        // insert `offsetX/offsetY` as strings via `apply_kv_value`. Normalize any parsed numeric
        // strings back to integers to match upstream Mermaid behavior.
        for key in ["offsetX", "offsetY"] {
            let Some(v) = target.get(key) else {
                continue;
            };
            if let Some(parsed) = value_as_i64(v) {
                target.insert(key.to_string(), json!(parsed));
            }
        }
        Ok(())
    }

    fn update_layout_config(&mut self, args: Vec<Value>) -> Result<()> {
        if let Some(v) = args.first()
            && let Some(parsed) = value_as_i64(v)
            && parsed >= 1
        {
            self.c4_shape_in_row = parsed;
        }
        if let Some(v) = args.get(1)
            && let Some(parsed) = value_as_i64(v)
            && parsed >= 1
        {
            self.c4_boundary_in_row = parsed;
        }
        Ok(())
    }

    fn to_model(&self, meta: &ParseMetadata) -> Value {
        let mut layout = Map::with_capacity(2);
        layout.insert(
            "c4ShapeInRow".to_string(),
            Value::Number(self.c4_shape_in_row.into()),
        );
        layout.insert(
            "c4BoundaryInRow".to_string(),
            Value::Number(self.c4_boundary_in_row.into()),
        );

        let mut out = Map::with_capacity(12);
        out.insert("type".to_string(), Value::String(meta.diagram_type.clone()));
        out.insert("c4Type".to_string(), Value::String(self.c4_type.clone()));
        out.insert(
            "title".to_string(),
            if self.title.is_empty() {
                Value::Null
            } else {
                Value::String(self.title.clone())
            },
        );
        out.insert("accTitle".to_string(), Value::Null);
        out.insert(
            "accDescr".to_string(),
            if self.acc_descr.is_empty() {
                Value::Null
            } else {
                Value::String(self.acc_descr.clone())
            },
        );
        out.insert("wrap".to_string(), Value::Bool(self.wrap_enabled));
        out.insert("layout".to_string(), Value::Object(layout));
        out.insert(
            "shapes".to_string(),
            Value::Array(
                self.shapes
                    .iter()
                    .map(|m| Value::Object(m.clone()))
                    .collect(),
            ),
        );
        out.insert(
            "boundaries".to_string(),
            Value::Array(
                self.boundaries
                    .iter()
                    .map(|m| Value::Object(m.clone()))
                    .collect(),
            ),
        );
        out.insert(
            "rels".to_string(),
            Value::Array(self.rels.iter().map(|m| Value::Object(m.clone())).collect()),
        );
        out.insert(
            "config".to_string(),
            crate::config::clone_value_nonrecursive(meta.effective_config.as_value()),
        );
        Value::Object(out)
    }

    fn to_render_model(&self) -> Result<C4DiagramRenderModel> {
        let shapes = self
            .shapes
            .iter()
            .map(c4_shape_render_model_from_map)
            .collect::<Result<Vec<_>>>()?;
        let boundaries = self
            .boundaries
            .iter()
            .map(c4_boundary_render_model_from_map)
            .collect::<Result<Vec<_>>>()?;
        let rels = self
            .rels
            .iter()
            .map(c4_rel_render_model_from_map)
            .collect::<Result<Vec<_>>>()?;

        Ok(C4DiagramRenderModel {
            c4_type: self.c4_type.clone(),
            title: (!self.title.is_empty()).then(|| self.title.clone()),
            acc_title: None,
            acc_descr: (!self.acc_descr.is_empty()).then(|| self.acc_descr.clone()),
            wrap: self.wrap_enabled,
            layout: C4LayoutConfig {
                c4_shape_in_row: self.c4_shape_in_row,
                c4_boundary_in_row: self.c4_boundary_in_row,
            },
            shapes,
            boundaries,
            rels,
        })
    }
}

fn wrap_text(v: Value) -> Value {
    json!({ "text": v })
}

fn c4_required_string(obj: &Map<String, Value>, key: &str) -> Result<String> {
    match obj.get(key) {
        Some(Value::String(s)) => Ok(s.clone()),
        Some(other) => Err(Error::diagram_parse_fallback(
            "c4".to_string(),
            format!("expected string field `{key}`, got {other:?}"),
        )),
        None => Err(Error::diagram_parse_fallback(
            "c4".to_string(),
            format!("missing required field `{key}`"),
        )),
    }
}

fn c4_optional_string(obj: &Map<String, Value>, key: &str) -> Result<Option<String>> {
    match obj.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(other) => Err(Error::diagram_parse_fallback(
            "c4".to_string(),
            format!("expected optional string field `{key}`, got {other:?}"),
        )),
    }
}

fn c4_optional_bool(obj: &Map<String, Value>, key: &str) -> Result<Option<bool>> {
    match obj.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Bool(v)) => Ok(Some(*v)),
        Some(other) => Err(Error::diagram_parse_fallback(
            "c4".to_string(),
            format!("expected optional bool field `{key}`, got {other:?}"),
        )),
    }
}

fn c4_optional_value(obj: &Map<String, Value>, key: &str) -> Option<Value> {
    obj.get(key)
        .and_then(|v| if v.is_null() { None } else { Some(v.clone()) })
}

fn c4_text_from_value(v: &Value) -> C4Text {
    match v {
        Value::Object(map) => {
            if let Some(text) = map.get("text") {
                C4Text::Wrapped { text: text.clone() }
            } else {
                C4Text::Value(v.clone())
            }
        }
        Value::String(s) => C4Text::String(s.clone()),
        other => C4Text::Value(other.clone()),
    }
}

fn c4_text_or_default(obj: &Map<String, Value>, key: &str) -> C4Text {
    obj.get(key).map(c4_text_from_value).unwrap_or_default()
}

fn c4_optional_text(obj: &Map<String, Value>, key: &str) -> Option<C4Text> {
    obj.get(key).and_then(|v| {
        if v.is_null() {
            None
        } else {
            Some(c4_text_from_value(v))
        }
    })
}

fn c4_optional_i64(obj: &Map<String, Value>, key: &str) -> Option<i64> {
    obj.get(key).and_then(value_as_i64)
}

fn c4_shape_render_model_from_map(obj: &Map<String, Value>) -> Result<C4ShapeRenderModel> {
    Ok(C4ShapeRenderModel {
        alias: c4_required_string(obj, "alias")?,
        parent_boundary: c4_optional_string(obj, "parentBoundary")?.unwrap_or_default(),
        type_c4_shape: c4_text_or_default(obj, "typeC4Shape"),
        label: c4_text_or_default(obj, "label"),
        wrap: c4_optional_bool(obj, "wrap")?.unwrap_or(false),
        sprite: c4_optional_value(obj, "sprite"),
        tags: c4_optional_value(obj, "tags"),
        link: c4_optional_value(obj, "link"),
        ty: c4_optional_text(obj, "type"),
        techn: c4_optional_text(obj, "techn"),
        descr: c4_optional_text(obj, "descr"),
        bg_color: c4_optional_string(obj, "bgColor")?,
        border_color: c4_optional_string(obj, "borderColor")?,
        font_color: c4_optional_string(obj, "fontColor")?,
        shadowing: c4_optional_value(obj, "shadowing"),
        shape: c4_optional_value(obj, "shape"),
        legend_text: c4_optional_value(obj, "legendText"),
        legend_sprite: c4_optional_value(obj, "legendSprite"),
    })
}

fn c4_boundary_render_model_from_map(obj: &Map<String, Value>) -> Result<C4BoundaryRenderModel> {
    Ok(C4BoundaryRenderModel {
        alias: c4_required_string(obj, "alias")?,
        parent_boundary: c4_optional_string(obj, "parentBoundary")?.unwrap_or_default(),
        label: c4_text_or_default(obj, "label"),
        ty: c4_optional_text(obj, "type"),
        descr: c4_optional_text(obj, "descr"),
        wrap: c4_optional_bool(obj, "wrap")?,
        sprite: c4_optional_value(obj, "sprite"),
        tags: c4_optional_value(obj, "tags"),
        link: c4_optional_value(obj, "link"),
        node_type: c4_optional_string(obj, "nodeType")?,
        bg_color: c4_optional_string(obj, "bgColor")?,
        border_color: c4_optional_string(obj, "borderColor")?,
        font_color: c4_optional_string(obj, "fontColor")?,
    })
}

fn c4_rel_render_model_from_map(obj: &Map<String, Value>) -> Result<C4RelRenderModel> {
    Ok(C4RelRenderModel {
        from_alias: c4_required_string(obj, "from")?,
        to_alias: c4_required_string(obj, "to")?,
        rel_type: c4_required_string(obj, "type")?,
        label: c4_text_or_default(obj, "label"),
        techn: c4_optional_text(obj, "techn"),
        descr: c4_optional_text(obj, "descr"),
        wrap: c4_optional_bool(obj, "wrap")?.unwrap_or(false),
        sprite: c4_optional_value(obj, "sprite"),
        tags: c4_optional_value(obj, "tags"),
        link: c4_optional_value(obj, "link"),
        offset_x: c4_optional_i64(obj, "offsetX"),
        offset_y: c4_optional_i64(obj, "offsetY"),
        line_color: c4_optional_string(obj, "lineColor")?,
        text_color: c4_optional_string(obj, "textColor")?,
    })
}

fn arg_to_string(v: Option<&Value>) -> Result<String> {
    match v {
        None => Ok(String::new()),
        Some(Value::String(s)) => Ok(s.clone()),
        Some(other) => Err(Error::diagram_parse_fallback(
            "c4".to_string(),
            format!("expected string argument, got {other:?}"),
        )),
    }
}

fn apply_text_field_or_kv(obj: &mut Map<String, Value>, default_key: &str, v: Value) -> Result<()> {
    match v {
        Value::Object(map) => {
            let mut iter = map.into_iter();
            let Some((k, vv)) = iter.next() else {
                obj.insert(default_key.to_string(), wrap_text(json!("")));
                return Ok(());
            };
            let s = match vv {
                Value::String(s) => s,
                other => {
                    return Err(Error::diagram_parse_fallback(
                        "c4".to_string(),
                        format!("expected string in attribute kv, got {other:?}"),
                    ));
                }
            };
            obj.insert(k, wrap_text(json!(s)));
            Ok(())
        }
        Value::String(s) => {
            obj.insert(default_key.to_string(), wrap_text(json!(s)));
            Ok(())
        }
        other => Err(Error::diagram_parse_fallback(
            "c4".to_string(),
            format!("invalid text field value: {other:?}"),
        )),
    }
}

fn apply_kv_value(
    obj: &mut Map<String, Value>,
    default_key: &str,
    v: Option<&Value>,
) -> Result<()> {
    let Some(v) = v else {
        return Ok(());
    };

    match v {
        Value::Object(map) => {
            let mut iter = map.clone().into_iter();
            let Some((k, vv)) = iter.next() else {
                return Ok(());
            };
            obj.insert(k, vv);
            Ok(())
        }
        Value::String(s) => {
            obj.insert(default_key.to_string(), json!(s));
            Ok(())
        }
        other => Err(Error::diagram_parse_fallback(
            "c4".to_string(),
            format!("invalid kv value: {other:?}"),
        )),
    }
}

fn apply_int_kv(obj: &mut Map<String, Value>, key: &str, v: &Value) -> Result<()> {
    let Some(parsed) = value_as_i64(v) else {
        return Ok(());
    };
    obj.insert(key.to_string(), json!(parsed));
    Ok(())
}

fn value_as_i64(v: &Value) -> Option<i64> {
    match v {
        Value::Number(n) => n.as_i64().or_else(|| n.as_u64().map(|v| v as i64)),
        Value::String(s) => s.trim().parse::<i64>().ok(),
        Value::Object(map) => map.values().next().and_then(value_as_i64),
        _ => None,
    }
}

fn parse_c4_db(code: &str, meta: &ParseMetadata) -> Result<C4Db> {
    let mut db = C4Db::new(&meta.effective_config);

    let mut lines = code.lines().peekable();
    let header = next_non_empty_line(&mut lines).ok_or_else(|| {
        Error::diagram_parse_fallback(meta.diagram_type.clone(), "expected C4 header".to_string())
    })?;
    let header = header.trim();

    match header {
        "C4Context" | "C4Container" | "C4Component" | "C4Dynamic" | "C4Deployment" => {}
        _ => {
            return Err(Error::diagram_parse_fallback(
                meta.diagram_type.clone(),
                format!("unexpected C4 header: {header}"),
            ));
        }
    }
    db.set_c4_type(header, &meta.effective_config);

    while let Some(raw) = lines.next() {
        let raw = strip_inline_comment(raw);
        let t = raw.trim();
        if t.is_empty() {
            continue;
        }

        if t == "}" {
            db.pop_boundary_parse_stack();
            continue;
        }

        if let Some(title) = try_parse_title(t) {
            db.set_title(&title, &meta.effective_config);
            continue;
        }

        if let Some(acc) = try_parse_acc_description_stmt(t) {
            db.set_acc_description(&acc);
            continue;
        }

        if try_parse_acc_title_as_title(t, &mut db, &meta.effective_config) {
            continue;
        }

        if let Some(v) = try_parse_acc_descr(t, &mut lines)? {
            db.set_acc_description(&v);
            continue;
        }

        if is_direction_stmt(t) {
            continue;
        }

        let Some((name, args, has_lbrace)) = parse_macro_stmt(t)? else {
            return Err(Error::diagram_parse_fallback(
                meta.diagram_type.clone(),
                format!("unsupported C4 statement: {t}"),
            ));
        };

        if is_boundary_macro(&name) {
            let mut args = args;
            match name.as_str() {
                "Enterprise_Boundary" => args.insert(2, json!("ENTERPRISE")),
                "System_Boundary" => args.insert(2, json!("SYSTEM")),
                "Container_Boundary" => args.insert(2, json!("CONTAINER")),
                _ => {}
            }

            match name.as_str() {
                "Boundary" | "Enterprise_Boundary" | "System_Boundary" => {
                    db.add_person_or_system_boundary(args)?;
                }
                "Container_Boundary" => {
                    db.add_container_boundary(args)?;
                }
                "Node" | "Deployment_Node" => {
                    db.add_deployment_node("node", args)?;
                }
                "Node_L" => {
                    db.add_deployment_node("nodeL", args)?;
                }
                "Node_R" => {
                    db.add_deployment_node("nodeR", args)?;
                }
                other => {
                    return Err(Error::diagram_parse_fallback(
                        meta.diagram_type.clone(),
                        format!("unsupported boundary macro: {other}"),
                    ));
                }
            }

            if !has_lbrace {
                consume_required_lbrace(&mut lines)?;
            }
            continue;
        }

        match name.as_str() {
            "Person" => db.add_person_or_system("person", &args)?,
            "Person_Ext" => db.add_person_or_system("external_person", &args)?,
            "System" => db.add_person_or_system("system", &args)?,
            "SystemDb" => db.add_person_or_system("system_db", &args)?,
            "SystemQueue" => db.add_person_or_system("system_queue", &args)?,
            "System_Ext" => db.add_person_or_system("external_system", &args)?,
            "SystemDb_Ext" => db.add_person_or_system("external_system_db", &args)?,
            "SystemQueue_Ext" => db.add_person_or_system("external_system_queue", &args)?,

            "Container" => db.add_container("container", &args)?,
            "ContainerDb" => db.add_container("container_db", &args)?,
            "ContainerQueue" => db.add_container("container_queue", &args)?,
            "Container_Ext" => db.add_container("external_container", &args)?,
            "ContainerDb_Ext" => db.add_container("external_container_db", &args)?,
            "ContainerQueue_Ext" => db.add_container("external_container_queue", &args)?,

            "Component" => db.add_component("component", &args)?,
            "ComponentDb" => db.add_component("component_db", &args)?,
            "ComponentQueue" => db.add_component("component_queue", &args)?,
            "Component_Ext" => db.add_component("external_component", &args)?,
            "ComponentDb_Ext" => db.add_component("external_component_db", &args)?,
            "ComponentQueue_Ext" => db.add_component("external_component_queue", &args)?,

            "Rel" => db.add_rel("rel", args)?,
            "BiRel" => db.add_rel("birel", args)?,
            "Rel_U" | "Rel_Up" => db.add_rel("rel_u", args)?,
            "Rel_D" | "Rel_Down" => db.add_rel("rel_d", args)?,
            "Rel_L" | "Rel_Left" => db.add_rel("rel_l", args)?,
            "Rel_R" | "Rel_Right" => db.add_rel("rel_r", args)?,
            "Rel_Back" => db.add_rel("rel_b", args)?,
            "RelIndex" => {
                let args = args.into_iter().skip(1).collect::<Vec<_>>();
                db.add_rel("rel", args)?;
            }

            "UpdateElementStyle" => db.update_el_style(args)?,
            "UpdateRelStyle" => db.update_rel_style(args)?,
            "UpdateLayoutConfig" => db.update_layout_config(args)?,

            other => {
                return Err(Error::diagram_parse_fallback(
                    meta.diagram_type.clone(),
                    format!("unsupported C4 macro: {other}"),
                ));
            }
        }
    }

    Ok(db)
}

fn strip_inline_comment(line: &str) -> String {
    let mut in_quotes = false;
    let mut idx = 0usize;
    let bytes = line.as_bytes();
    while idx < bytes.len() {
        let b = bytes[idx];
        if b == b'"' {
            in_quotes = !in_quotes;
            idx += 1;
            continue;
        }
        if !in_quotes && b == b'%' && idx + 1 < bytes.len() && bytes[idx + 1] == b'%' {
            return line[..idx].to_string();
        }
        idx += 1;
    }
    line.to_string()
}

fn is_direction_stmt(t: &str) -> bool {
    let mut it = t.split_whitespace();
    let Some(first) = it.next() else {
        return false;
    };
    if first != "direction" {
        return false;
    }
    matches!(it.next(), Some("TB" | "BT" | "LR" | "RL"))
}

fn next_non_empty_line<'a>(
    lines: &mut std::iter::Peekable<std::str::Lines<'a>>,
) -> Option<&'a str> {
    lines
        .by_ref()
        .find(|&l| !l.trim().is_empty())
        .map(|v| v as _)
}

fn try_parse_title(t: &str) -> Option<String> {
    if t.starts_with("title ") && t.len() >= 6 {
        return Some(t[6..].trim_end().to_string());
    }
    None
}

fn try_parse_acc_description_stmt(t: &str) -> Option<String> {
    if t.starts_with("accDescription ") && t.len() >= 15 {
        return Some(t[15..].trim_end().to_string());
    }
    None
}

fn try_parse_acc_title_as_title(t: &str, db: &mut C4Db, config: &MermaidConfig) -> bool {
    let t = t.trim_start();
    if !t.starts_with("accTitle") {
        return false;
    }
    let rest = &t["accTitle".len()..];
    let rest = rest.trim_start();
    if !rest.starts_with(':') {
        return false;
    }
    let val = rest[1..].trim();
    db.set_title(val, config);
    true
}

fn try_parse_acc_descr<'a>(
    t: &str,
    lines: &mut std::iter::Peekable<std::str::Lines<'a>>,
) -> Result<Option<String>> {
    let t = t.trim_start();
    if !t.starts_with("accDescr") {
        return Ok(None);
    }

    let rest = &t["accDescr".len()..];
    let rest = rest.trim_start();
    if let Some(after) = rest.strip_prefix(':') {
        let val = after.trim();
        return Ok(Some(val.to_string()));
    }

    if let Some(rest) = rest.strip_prefix('{') {
        let mut buf = String::new();

        // Mermaid's lexer consumes whitespace after '{' (`accDescr\s*"{"\s*`),
        // and the parser applies a single `.trim()` to the whole token.
        let mut after = rest.to_string();
        if let Some(end) = after.find('}') {
            after.truncate(end);
            return Ok(Some(after.trim().to_string()));
        }
        let after = after.trim_start();
        if !after.is_empty() {
            buf.push_str(after);
        }

        for raw in lines.by_ref() {
            if let Some(pos) = raw.find('}') {
                let part = &raw[..pos];
                if !buf.is_empty() {
                    buf.push('\n');
                }
                buf.push_str(part);
                break;
            }

            if !buf.is_empty() {
                buf.push('\n');
            }
            buf.push_str(raw);
        }

        return Ok(Some(buf.trim().to_string()));
    }

    Ok(None)
}

fn is_boundary_macro(name: &str) -> bool {
    matches!(
        name,
        "Boundary"
            | "Enterprise_Boundary"
            | "System_Boundary"
            | "Container_Boundary"
            | "Node"
            | "Deployment_Node"
            | "Node_L"
            | "Node_R"
    )
}

fn consume_required_lbrace(lines: &mut std::iter::Peekable<std::str::Lines<'_>>) -> Result<()> {
    while let Some(peek) = lines.peek().copied() {
        if peek.trim().is_empty() {
            lines.next();
            continue;
        }
        if peek.trim() == "{" {
            lines.next();
            return Ok(());
        }
        return Err(Error::diagram_parse_fallback(
            "c4".to_string(),
            "expected '{' after boundary".to_string(),
        ));
    }
    Err(Error::diagram_parse_fallback(
        "c4".to_string(),
        "expected '{' after boundary".to_string(),
    ))
}

fn parse_macro_stmt(t: &str) -> Result<Option<(String, Vec<Value>, bool)>> {
    let t = t.trim_end();
    let Some(paren) = t.find('(') else {
        return Ok(None);
    };
    let name = t[..paren].trim().to_string();
    if name.is_empty() {
        return Ok(None);
    }

    let after = &t[paren + 1..];
    let Some(end_paren) = after.rfind(')') else {
        return Err(Error::diagram_parse_fallback(
            "c4".to_string(),
            format!("unterminated macro call: {t}"),
        ));
    };

    let args_raw = &after[..end_paren];
    let rest = after[end_paren + 1..].trim();
    let mut has_lbrace = false;
    if let Some(after) = rest.strip_prefix('{') {
        if after.trim().is_empty() {
            has_lbrace = true;
        } else {
            return Err(Error::diagram_parse_fallback(
                "c4".to_string(),
                format!("unexpected tokens after '{{' in macro: {t}"),
            ));
        }
    } else if !rest.is_empty() {
        return Err(Error::diagram_parse_fallback(
            "c4".to_string(),
            format!("unexpected trailing tokens in macro: {t}"),
        ));
    }

    let args = parse_args_csv(args_raw)?;
    Ok(Some((name, args, has_lbrace)))
}

fn parse_args_csv(input: &str) -> Result<Vec<Value>> {
    let mut out = Vec::new();
    let mut cur = input;
    loop {
        if cur.trim().is_empty() {
            break;
        }
        let (seg, rest) = split_next_arg(cur);
        out.push(parse_arg(seg.trim())?);
        let Some(rest) = rest else {
            break;
        };
        cur = rest;
    }
    Ok(out)
}

fn split_next_arg(input: &str) -> (&str, Option<&str>) {
    let mut in_quotes = false;
    for (i, c) in input.char_indices() {
        match c {
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => {
                return (&input[..i], Some(&input[i + 1..]));
            }
            _ => {}
        }
    }
    (input, None)
}

fn parse_arg(seg: &str) -> Result<Value> {
    if seg.is_empty() {
        return Ok(json!(""));
    }

    if let Some(v) = try_parse_kv(seg)? {
        return Ok(v);
    }

    if seg.starts_with('"') {
        let s = parse_quoted(seg)?;
        return Ok(json!(s));
    }

    Ok(json!(seg.trim()))
}

fn try_parse_kv(seg: &str) -> Result<Option<Value>> {
    let seg = seg.trim_start();
    if !seg.starts_with('$') {
        return Ok(None);
    }
    let rest = &seg[1..];
    let Some(eq) = rest.find('=') else {
        return Err(Error::diagram_parse_fallback(
            "c4".to_string(),
            format!("invalid attribute kv: {seg}"),
        ));
    };
    let key = rest[..eq].trim();
    let val_raw = rest[eq + 1..].trim_start();
    if key.is_empty() {
        return Err(Error::diagram_parse_fallback(
            "c4".to_string(),
            format!("invalid attribute kv key: {seg}"),
        ));
    }
    let val = parse_quoted(val_raw)?;
    let mut map = Map::new();
    map.insert(key.to_string(), json!(val));
    Ok(Some(Value::Object(map)))
}

fn parse_quoted(input: &str) -> Result<String> {
    let input = input.trim();
    let Some(rest) = input.strip_prefix('"') else {
        return Err(Error::diagram_parse_fallback(
            "c4".to_string(),
            format!("expected quoted string, got: {input}"),
        ));
    };
    let Some(end) = rest.find('"') else {
        return Err(Error::diagram_parse_fallback(
            "c4".to_string(),
            "unterminated string".to_string(),
        ));
    };
    let val = &rest[..end];
    let trailing = rest[end + 1..].trim();
    if !trailing.is_empty() {
        return Err(Error::diagram_parse_fallback(
            "c4".to_string(),
            format!("unexpected trailing tokens after string: {trailing}"),
        ));
    }
    Ok(val.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Engine, ParseOptions, RenderSemanticModel};
    use futures::executor::block_on;
    use serde_json::json;

    fn parse(text: &str) -> Value {
        let engine = Engine::new();
        block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap()
            .model
    }

    #[test]
    fn c4_trailing_whitespace_after_statements_is_accepted() {
        let whitespace = " ";
        let model = parse(&format!(
            "C4Context{whitespace}\n\
title System Context diagram for Internet Banking System{whitespace}\n\
Person(customerA, \"Banking Customer A\", \"A customer of the bank, with personal bank accounts.\"){whitespace}\n"
        ));
        assert_eq!(model["c4Type"], json!("C4Context"));
        assert_eq!(model["shapes"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn c4_parameter_names_that_are_keywords_are_allowed() {
        let model = parse(
            r#"C4Context
title title
Person(Person, "Person", "Person")
"#,
        );
        assert_eq!(model["title"], json!("title"));
        assert_eq!(model["shapes"][0]["alias"], json!("Person"));
        assert_eq!(model["shapes"][0]["label"]["text"], json!("Person"));
        assert_eq!(model["shapes"][0]["descr"]["text"], json!("Person"));
    }

    #[test]
    fn c4_allows_default_in_parameters() {
        let model = parse(
            r#"C4Context
Person(default, "default", "default")
"#,
        );
        assert_eq!(model["shapes"][0]["alias"], json!("default"));
        assert_eq!(model["shapes"][0]["label"]["text"], json!("default"));
        assert_eq!(model["shapes"][0]["descr"]["text"], json!("default"));
    }

    #[test]
    fn c4_person_is_parsed() {
        let model = parse(
            r#"C4Context
title System Context diagram for Internet Banking System
Person(customerA, "Banking Customer A", "A customer of the bank, with personal bank accounts.")
"#,
        );
        assert_eq!(model["shapes"].as_array().unwrap().len(), 1);
        assert_eq!(model["shapes"][0]["alias"], json!("customerA"));
        assert_eq!(
            model["shapes"][0]["label"]["text"],
            json!("Banking Customer A")
        );
        assert_eq!(
            model["shapes"][0]["descr"]["text"],
            json!("A customer of the bank, with personal bank accounts.")
        );
        assert_eq!(model["shapes"][0]["parentBoundary"], json!("global"));
        assert_eq!(model["shapes"][0]["typeC4Shape"]["text"], json!("person"));
        assert_eq!(model["shapes"][0]["wrap"], json!(false));
    }

    #[test]
    fn c4_boundary_is_parsed() {
        let model = parse(
            r#"C4Context
title System Context diagram for Internet Banking System
Boundary(b1, "BankBoundary") {
System(SystemAA, "Internet Banking System")
}
"#,
        );

        assert_eq!(model["boundaries"].as_array().unwrap().len(), 2);
        assert_eq!(model["boundaries"][1]["alias"], json!("b1"));
        assert_eq!(
            model["boundaries"][1]["label"]["text"],
            json!("BankBoundary")
        );
        assert_eq!(model["boundaries"][1]["parentBoundary"], json!("global"));
        assert_eq!(model["boundaries"][1]["type"]["text"], json!("system"));

        assert_eq!(model["shapes"].as_array().unwrap().len(), 1);
        assert_eq!(model["shapes"][0]["parentBoundary"], json!("b1"));
    }

    #[test]
    fn c4_person_ext_is_parsed() {
        let model = parse(
            r#"C4Context
Person_Ext(customerA, "Banking Customer A", "A customer of the bank, with personal bank accounts.")
"#,
        );
        assert_eq!(
            model["shapes"][0]["typeC4Shape"]["text"],
            json!("external_person")
        );
    }

    #[test]
    fn c4_system_variants_are_parsed() {
        let cases = [
            ("System", "system"),
            ("SystemDb", "system_db"),
            ("SystemQueue", "system_queue"),
            ("System_Ext", "external_system"),
            ("SystemDb_Ext", "external_system_db"),
            ("SystemQueue_Ext", "external_system_queue"),
        ];
        for (macro_name, kind) in cases {
            let model = parse(&format!(
                "C4Context\n\
{macro_name}(SystemAA, \"Internet Banking System\", \"Allows customers to view information about their bank accounts, and make payments.\")\n"
            ));
            assert_eq!(model["shapes"][0]["typeC4Shape"]["text"], json!(kind));
        }
    }

    #[test]
    fn c4_container_variants_are_parsed() {
        let cases = [
            ("Container", "container"),
            ("ContainerDb", "container_db"),
            ("ContainerQueue", "container_queue"),
            ("Container_Ext", "external_container"),
            ("ContainerDb_Ext", "external_container_db"),
            ("ContainerQueue_Ext", "external_container_queue"),
        ];
        for (macro_name, kind) in cases {
            let model = parse(&format!(
                "C4Context\n\
{macro_name}(ContainerAA, \"Internet Banking Container\", \"Technology\", \"Allows customers to view information about their bank accounts, and make payments.\")\n"
            ));
            assert_eq!(model["shapes"][0]["typeC4Shape"]["text"], json!(kind));
            assert_eq!(model["shapes"][0]["techn"]["text"], json!("Technology"));
        }
    }

    #[test]
    fn c4_label_can_be_kv_object() {
        let model = parse(
            r#"C4Context
Person(customerA, $sprite="users")
"#,
        );
        assert_eq!(
            model["shapes"][0]["label"]["text"]["sprite"],
            json!("users")
        );
    }

    #[test]
    fn c4_rel_is_deduped_by_from_to_like_mermaid_db() {
        let model = parse(
            r#"C4Context
Rel(a, b, "first")
Rel(a, b, "second")
"#,
        );
        assert_eq!(model["rels"].as_array().unwrap().len(), 1);
        assert_eq!(model["rels"][0]["label"]["text"], json!("second"));
    }

    #[test]
    fn c4_relindex_ignores_index_arg() {
        let model = parse(
            r#"C4Context
RelIndex(123, a, b, "label")
"#,
        );
        assert_eq!(model["rels"].as_array().unwrap().len(), 1);
        assert_eq!(model["rels"][0]["from"], json!("a"));
        assert_eq!(model["rels"][0]["to"], json!("b"));
        assert_eq!(model["rels"][0]["label"]["text"], json!("label"));
    }

    #[test]
    fn c4_wrap_directive_sets_wrap_true_on_nodes() {
        let model = parse(
            r#"%%{wrap}%%
C4Context
Person(a, "A", "D")
"#,
        );
        assert_eq!(model["wrap"], json!(true));
        assert_eq!(model["shapes"][0]["wrap"], json!(true));
    }

    #[test]
    fn c4_update_element_style_updates_shape_fields() {
        let model = parse(
            r#"C4Context
Person(a, "A", "D")
UpdateElementStyle(a, $bgColor="red", $borderColor="blue")
"#,
        );
        assert_eq!(model["shapes"][0]["bgColor"], json!("red"));
        assert_eq!(model["shapes"][0]["borderColor"], json!("blue"));
    }

    #[test]
    fn c4_update_element_style_can_target_boundaries() {
        let model = parse(
            r#"C4Context
Boundary(b1, "B") {
}
UpdateElementStyle(b1, $bgColor="red")
"#,
        );
        assert_eq!(model["boundaries"][1]["bgColor"], json!("red"));
    }

    #[test]
    fn c4_update_rel_style_updates_rel_fields() {
        let model = parse(
            r#"C4Context
Rel(a, b, "label")
UpdateRelStyle(a, b, $textColor="red", $lineColor="blue", $offsetX="10", $offsetY="20")
"#,
        );
        assert_eq!(model["rels"][0]["textColor"], json!("red"));
        assert_eq!(model["rels"][0]["lineColor"], json!("blue"));
        assert_eq!(model["rels"][0]["offsetX"], json!(10));
        assert_eq!(model["rels"][0]["offsetY"], json!(20));
    }

    #[test]
    fn c4_update_layout_config_enforces_minimum_one() {
        let model = parse(
            r#"C4Context
UpdateLayoutConfig(0, 0)
"#,
        );
        assert_eq!(model["layout"]["c4ShapeInRow"], json!(4));
        assert_eq!(model["layout"]["c4BoundaryInRow"], json!(2));

        let model = parse(
            r#"C4Context
UpdateLayoutConfig(3, 2)
"#,
        );
        assert_eq!(model["layout"]["c4ShapeInRow"], json!(3));
        assert_eq!(model["layout"]["c4BoundaryInRow"], json!(2));
    }

    #[test]
    fn c4_deployment_node_ignores_sprite_param_like_mermaid_db() {
        let model = parse(
            r#"C4Deployment
Node(n1, "Node", "type", "descr", $sprite="users") {
}
"#,
        );
        assert_eq!(model["boundaries"].as_array().unwrap().len(), 2);
        assert!(model["boundaries"][1].get("sprite").is_none());
    }

    #[test]
    fn c4_boundary_brace_can_be_on_next_line() {
        let model = parse(
            r#"C4Context
Boundary(b1, "B")
{
  Person(p1, "P")
}
"#,
        );
        assert_eq!(model["boundaries"].as_array().unwrap().len(), 2);
        assert_eq!(model["boundaries"][1]["alias"], json!("b1"));
        assert_eq!(model["shapes"].as_array().unwrap().len(), 1);
        assert_eq!(model["shapes"][0]["parentBoundary"], json!("b1"));
    }

    #[test]
    fn c4_nested_boundaries_keep_parent_boundary_correct() {
        let model = parse(
            r#"C4Context
Enterprise_Boundary(ent, "Enterprise") {
  System_Boundary(sys, "System") {
    Person(p1, "P")
  }
  Person(p2, "P2")
}
"#,
        );

        assert_eq!(model["boundaries"].as_array().unwrap().len(), 3);
        assert_eq!(model["boundaries"][1]["alias"], json!("ent"));
        assert_eq!(model["boundaries"][1]["type"]["text"], json!("ENTERPRISE"));
        assert_eq!(model["boundaries"][1]["parentBoundary"], json!("global"));

        assert_eq!(model["boundaries"][2]["alias"], json!("sys"));
        assert_eq!(model["boundaries"][2]["type"]["text"], json!("SYSTEM"));
        assert_eq!(model["boundaries"][2]["parentBoundary"], json!("ent"));

        assert_eq!(model["shapes"].as_array().unwrap().len(), 2);
        assert_eq!(model["shapes"][0]["alias"], json!("p1"));
        assert_eq!(model["shapes"][0]["parentBoundary"], json!("sys"));
        assert_eq!(model["shapes"][1]["alias"], json!("p2"));
        assert_eq!(model["shapes"][1]["parentBoundary"], json!("ent"));
    }

    #[test]
    fn c4_container_boundary_injects_container_type() {
        let model = parse(
            r#"C4Container
Container_Boundary(cb, "CB") {
  Container(c1, "C1", "Tech", "Desc")
}
"#,
        );
        assert_eq!(model["boundaries"].as_array().unwrap().len(), 2);
        assert_eq!(model["boundaries"][1]["alias"], json!("cb"));
        assert_eq!(model["boundaries"][1]["type"]["text"], json!("CONTAINER"));
        assert_eq!(model["shapes"].as_array().unwrap().len(), 1);
        assert_eq!(model["shapes"][0]["parentBoundary"], json!("cb"));
    }

    #[test]
    fn c4_nested_nodes_push_and_pop_like_boundaries() {
        let model = parse(
            r#"C4Deployment
Node(n1, "N1") {
  Node_L(n2, "N2") {
    Person(p1, "P1")
  }
  Person(p2, "P2")
}
"#,
        );
        assert_eq!(model["boundaries"].as_array().unwrap().len(), 3);
        assert_eq!(model["boundaries"][1]["alias"], json!("n1"));
        assert_eq!(model["boundaries"][1]["nodeType"], json!("node"));
        assert_eq!(model["boundaries"][2]["alias"], json!("n2"));
        assert_eq!(model["boundaries"][2]["nodeType"], json!("nodeL"));
        assert_eq!(model["boundaries"][2]["parentBoundary"], json!("n1"));

        assert_eq!(model["shapes"].as_array().unwrap().len(), 2);
        assert_eq!(model["shapes"][0]["alias"], json!("p1"));
        assert_eq!(model["shapes"][0]["parentBoundary"], json!("n2"));
        assert_eq!(model["shapes"][1]["alias"], json!("p2"));
        assert_eq!(model["shapes"][1]["parentBoundary"], json!("n1"));
    }

    #[test]
    fn c4_update_layout_config_accepts_kv_objects() {
        let model = parse(
            r#"C4Context
UpdateLayoutConfig($c4ShapeInRow="1", $c4BoundaryInRow="1")
"#,
        );
        assert_eq!(model["layout"]["c4ShapeInRow"], json!(1));
        assert_eq!(model["layout"]["c4BoundaryInRow"], json!(1));
    }

    #[test]
    fn c4_update_macros_are_noop_when_target_missing() {
        let model = parse(
            r#"C4Context
UpdateElementStyle(missing, $bgColor="red")
UpdateRelStyle(a, b, $textColor="red")
"#,
        );
        assert_eq!(model["shapes"].as_array().unwrap().len(), 0);
        assert_eq!(model["rels"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn c4_techn_and_descr_can_be_kv_objects() {
        let model = parse(
            r#"C4Context
Container(c1, "C1", $techn="Rust", $descr="Fast")
"#,
        );
        assert_eq!(model["shapes"].as_array().unwrap().len(), 1);
        assert_eq!(model["shapes"][0]["techn"]["text"], json!("Rust"));
        assert_eq!(model["shapes"][0]["descr"]["text"], json!("Fast"));
    }

    #[test]
    fn c4_boundary_type_can_be_kv_object() {
        let model = parse(
            r#"C4Context
Boundary(b1, "B", $type="company") {
}
"#,
        );
        assert_eq!(model["boundaries"].as_array().unwrap().len(), 2);
        assert_eq!(model["boundaries"][1]["type"]["text"], json!("company"));
    }

    #[test]
    fn c4_empty_args_are_allowed() {
        let model = parse(
            r#"C4Context
Person(a, , "D")
"#,
        );
        assert_eq!(model["shapes"].as_array().unwrap().len(), 1);
        assert_eq!(model["shapes"][0]["label"]["text"], json!(""));
        assert_eq!(model["shapes"][0]["descr"]["text"], json!("D"));
    }

    #[test]
    fn c4_rel_direction_macros_are_parsed() {
        let model = parse(
            r#"C4Context
Rel(a, b, "l1")
BiRel(a, b, "l2")
Rel_Up(a, b, "l3")
Rel_U(a, b, "l4")
Rel_Down(a, b, "l5")
Rel_D(a, b, "l6")
Rel_Left(a, b, "l7")
Rel_L(a, b, "l8")
Rel_Right(a, b, "l9")
Rel_R(a, b, "l10")
Rel_Back(a, b, "l11")
"#,
        );
        let rels = model["rels"].as_array().unwrap();
        assert_eq!(rels.len(), 1, "rels are deduped by (from,to)");
        assert_eq!(model["rels"][0]["from"], json!("a"));
        assert_eq!(model["rels"][0]["to"], json!("b"));
        assert_eq!(model["rels"][0]["type"], json!("rel_b"));
        assert_eq!(model["rels"][0]["label"]["text"], json!("l11"));
    }

    #[test]
    fn c4_rel_without_label_is_ignored_like_mermaid_db() {
        let model = parse(
            r#"C4Context
Rel(a, b)
Rel(a, b, )
"#,
        );
        assert_eq!(model["rels"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn c4_rel_inline_comment_is_ignored_but_not_inside_quotes() {
        let model = parse(
            r#"C4Context
Rel(a, b, "label %% not a comment") %% actual comment
"#,
        );
        assert_eq!(model["rels"].as_array().unwrap().len(), 1);
        assert_eq!(
            model["rels"][0]["label"]["text"],
            json!("label %% not a comment")
        );
    }

    #[test]
    fn c4_label_supports_sprite_link_tags_via_kv_objects() {
        let model = parse(
            r#"C4Context
Person(p1, $sprite="users")
System(s1, $link="https://github.com/mermaidjs")
Container(c1, $tags="tag1,tag2")
"#,
        );
        assert_eq!(model["shapes"].as_array().unwrap().len(), 3);
        assert_eq!(
            model["shapes"][0]["label"]["text"]["sprite"],
            json!("users")
        );
        assert_eq!(
            model["shapes"][1]["label"]["text"]["link"],
            json!("https://github.com/mermaidjs")
        );
        assert_eq!(
            model["shapes"][2]["label"]["text"]["tags"],
            json!("tag1,tag2")
        );
    }

    #[test]
    fn c4_sprite_link_tags_can_be_provided_as_positional_fields() {
        let model = parse(
            r#"C4Context
Person(p1, "P", "D", $sprite="users", $tags="tag1,tag2", $link="https://example.com")
"#,
        );
        assert_eq!(model["shapes"].as_array().unwrap().len(), 1);
        assert_eq!(model["shapes"][0]["sprite"], json!("users"));
        assert_eq!(model["shapes"][0]["tags"], json!("tag1,tag2"));
        assert_eq!(model["shapes"][0]["link"], json!("https://example.com"));
    }

    #[test]
    fn c4_boundary_supports_sprite_link_tags_via_kv_objects_or_positional_fields() {
        let model = parse(
            r#"C4Context
Boundary(b1, $link="https://example.com") {
  Person(p1, "P1")
}
Boundary(b2, "B2", "company", $tags="tag1,tag2", $link="https://example.com") {
  Person(p2, "P2")
}
"#,
        );
        assert_eq!(model["boundaries"].as_array().unwrap().len(), 3);
        assert_eq!(
            model["boundaries"][1]["label"]["text"]["link"],
            json!("https://example.com")
        );
        assert_eq!(model["boundaries"][2]["type"]["text"], json!("company"));
        assert_eq!(model["boundaries"][2]["tags"], json!("tag1,tag2"));
        assert_eq!(model["boundaries"][2]["link"], json!("https://example.com"));
    }

    #[test]
    fn c4_update_element_style_applies_all_supported_fields() {
        let model = parse(
            r#"C4Context
Person(p1, "P1")
Boundary(b1, "B1") {
  Person(p2, "P2")
}
UpdateElementStyle(p1, $bgColor="red", $fontColor="white", $borderColor="black", $shadowing="true", $shape="rounded", $sprite="users", $techn="Rust", $legendText="Legend", $legendSprite="book")
UpdateElementStyle(b1, $bgColor="blue")
"#,
        );
        assert_eq!(model["shapes"].as_array().unwrap().len(), 2);
        assert_eq!(model["shapes"][0]["bgColor"], json!("red"));
        assert_eq!(model["shapes"][0]["fontColor"], json!("white"));
        assert_eq!(model["shapes"][0]["borderColor"], json!("black"));
        assert_eq!(model["shapes"][0]["shadowing"], json!("true"));
        assert_eq!(model["shapes"][0]["shape"], json!("rounded"));
        assert_eq!(model["shapes"][0]["sprite"], json!("users"));
        assert_eq!(model["shapes"][0]["techn"], json!("Rust"));
        assert_eq!(model["shapes"][0]["legendText"], json!("Legend"));
        assert_eq!(model["shapes"][0]["legendSprite"], json!("book"));

        assert_eq!(model["boundaries"].as_array().unwrap().len(), 2);
        assert_eq!(model["boundaries"][1]["bgColor"], json!("blue"));
    }

    #[test]
    fn c4_acc_title_is_mapped_to_title_like_mermaid_grammar() {
        let model = parse(
            r#"C4Context
accTitle: A11y title
"#,
        );
        assert_eq!(model["title"], json!("A11y title"));
        assert!(model["accTitle"].is_null());
    }

    #[test]
    fn c4_acc_descr_multiline_collapses_newline_whitespace_like_common_db() {
        let model = parse(
            r#"C4Context
accDescr{
first
  second
third
}
"#,
        );
        assert_eq!(model["accDescr"], json!("first\nsecond\nthird"));
    }

    #[test]
    fn c4_render_model_uses_typed_variant_without_changing_json_parse() {
        let engine = Engine::new();
        let input = r#"C4Context
title Banking Context
Person(customer, "Customer", "Uses the system")
System(system, "Internet Banking", "Core system")
Rel(customer, system, "Uses", "HTTPS")
"#;

        let parsed = engine
            .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
            .unwrap()
            .unwrap();

        assert_eq!(parsed.meta.diagram_type, "c4");
        match parsed.model {
            RenderSemanticModel::C4(model) => {
                assert_eq!(model.c4_type, "C4Context");
                assert_eq!(model.title.as_deref(), Some("Banking Context"));
                assert_eq!(model.shapes.len(), 2);
                assert_eq!(model.shapes[0].label.as_str(), "Customer");
                assert_eq!(model.rels.len(), 1);
                assert_eq!(model.rels[0].label.as_str(), "Uses");
            }
            other => panic!("c4 render parse should return typed model, got {other:?}"),
        }

        let parsed_json = engine
            .parse_diagram_sync(input, ParseOptions::strict())
            .unwrap()
            .unwrap();
        assert_eq!(parsed_json.model["type"], json!("c4"));
        assert_eq!(parsed_json.model["c4Type"], json!("C4Context"));
        assert_eq!(
            parsed_json.model["shapes"][0]["label"]["text"],
            json!("Customer")
        );
        assert_eq!(parsed_json.model["rels"][0]["label"]["text"], json!("Uses"));
    }

    #[test]
    fn c4_editor_facts_expose_parser_backed_spans() {
        let engine = Engine::new();
        let input = r#"C4Context
title Banking Context
accTitle: Banking accessibility title
accDescr: Banking accessibility description
Boundary(bank, "Bank") {
  Person(customer, "Customer", "Uses the system")
  System(system, "Internet Banking", "Core system")
}
Rel(customer, system, "Uses", "HTTPS")
UpdateElementStyle(system, $bgColor="red")
UpdateRelStyle(customer, system, $lineColor="blue")
"#;

        let facts = engine
            .parse_editor_semantic_facts_with_type_sync("c4", input, ParseOptions::strict())
            .unwrap()
            .unwrap();

        assert!(facts.directive_prefixes.iter().any(|p| p == "title"));
        assert!(facts.directive_prefixes.iter().any(|p| p == "accTitle"));
        assert!(facts.directive_prefixes.iter().any(|p| p == "accDescr"));
        for entity in ["bank", "customer", "system"] {
            assert!(
                facts.symbols.iter().any(|symbol| {
                    symbol.name == entity
                        && symbol.kind == EditorSemanticKind::Object
                        && symbol.role == EditorSemanticRole::Entity
                }),
                "missing C4 entity fact for {entity}"
            );
        }
        for payload in [
            "Banking Context",
            "Banking accessibility title",
            "Banking accessibility description",
            "Customer",
            "Core system",
            "Uses",
            "HTTPS",
            "red",
            "blue",
        ] {
            assert!(
                facts.symbols.iter().any(|symbol| {
                    symbol.name == payload
                        && symbol.kind == EditorSemanticKind::String
                        && symbol.role == EditorSemanticRole::Payload
                }),
                "missing C4 payload fact for {payload}"
            );
        }

        let system_refs = facts
            .symbols
            .iter()
            .filter(|symbol| symbol.name == "system" && symbol.role == EditorSemanticRole::Entity)
            .count();
        assert_eq!(
            system_refs, 4,
            "system should appear in definition, relation target, element style, and relation style"
        );

        let title_start = input.find("Banking Context").unwrap();
        assert!(facts.expected_syntax.iter().any(|expected| {
            expected.kind == EditorExpectedSyntaxKind::Payload
                && expected.span
                    == SourceSpan::new(title_start, title_start + "Banking Context".len())
        }));
    }

    #[test]
    fn c4_editor_facts_recover_unsupported_statements_without_losing_prior_facts() {
        let engine = Engine::new();
        let input = "C4Context\nPerson(customer, \"Customer\")\nNotAMacro customer\n";

        let facts = engine
            .parse_editor_semantic_facts_with_type_sync("c4", input, ParseOptions::strict())
            .unwrap()
            .unwrap();

        assert_eq!(
            facts.completeness,
            crate::EditorSemanticCompleteness::Recovered
        );
        assert!(facts.symbols.iter().any(|symbol| {
            symbol.name == "customer" && symbol.role == EditorSemanticRole::Entity
        }));
        assert!(!facts.diagnostics.is_empty());
    }
}
