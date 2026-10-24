#[cfg(feature = "presentation")]
use crate::PresentationMetadata;
use crate::diagrams::scan::{LineCursor, leading_whitespace_len};
#[cfg(feature = "editor-facts")]
use crate::family::EditorFactsDemand;
use crate::sanitize::sanitize_text;
#[cfg(feature = "editor-facts")]
use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
    EditorSemanticSymbol,
};
use crate::{Error, MermaidConfig, OperationControl, OperationControlResult, Result, SourceSpan};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
#[cfg(test)]
use std::cell::Cell;

#[cfg(test)]
thread_local! {
    static C4_SYNTAX_CONSTRUCTION_COUNT: Cell<usize> = const { Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_c4_syntax_construction_count() {
    C4_SYNTAX_CONSTRUCTION_COUNT.set(0);
}

#[cfg(test)]
pub(crate) fn c4_syntax_construction_count() -> usize {
    C4_SYNTAX_CONSTRUCTION_COUNT.get()
}

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

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct C4Shape {
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

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct C4Boundary {
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadowing: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shape: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub techn: Option<Value>,
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

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct C4Relation {
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
pub struct C4DiagramModel {
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
    pub shapes: Vec<C4Shape>,
    #[serde(default)]
    pub boundaries: Vec<C4Boundary>,
    #[serde(default)]
    pub rels: Vec<C4Relation>,
}

impl C4DiagramModel {
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

    boundaries: Vec<C4Boundary>,

    shapes: Vec<C4Shape>,

    rels: Vec<C4RelationRecord>,

    c4_shape_in_row: i64,
    c4_boundary_in_row: i64,
}

#[derive(Debug, Clone)]
struct SpannedText {
    text: String,
    span: SourceSpan,
}

struct SpannedAccDescr {
    value: SpannedText,
    closed: bool,
}

#[derive(Debug, Clone)]
struct SpannedNamedArg {
    key: String,
    value: SpannedText,
}

#[derive(Debug, Clone)]
enum SpannedArgValue {
    Text(SpannedText),
    Named(SpannedNamedArg),
}

#[derive(Debug, Clone)]
struct SpannedArg {
    value: SpannedArgValue,
}

impl SpannedArg {
    #[cfg(feature = "editor-facts")]
    fn text(&self) -> &str {
        match &self.value {
            SpannedArgValue::Text(value) => value.text.as_str(),
            SpannedArgValue::Named(value) => value.value.text.as_str(),
        }
    }

    fn span(&self) -> SourceSpan {
        match &self.value {
            SpannedArgValue::Text(value) => value.span,
            SpannedArgValue::Named(value) => value.value.span,
        }
    }

    #[cfg(feature = "editor-facts")]
    fn key(&self) -> Option<&str> {
        match &self.value {
            SpannedArgValue::Text(_) => None,
            SpannedArgValue::Named(value) => Some(value.key.as_str()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum C4Arg {
    Text(String),
    Named { key: String, value: String },
}

impl C4Arg {
    fn from_spanned(arg: &SpannedArg) -> Self {
        match &arg.value {
            SpannedArgValue::Text(value) => Self::Text(value.text.clone()),
            SpannedArgValue::Named(named) => Self::Named {
                key: named.key.clone(),
                value: named.value.text.clone(),
            },
        }
    }

    fn value(&self) -> &str {
        match self {
            Self::Text(value) | Self::Named { value, .. } => value,
        }
    }

    fn key_or<'a>(&'a self, positional_key: &'a str) -> &'a str {
        match self {
            Self::Text(_) => positional_key,
            Self::Named { key, .. } => key,
        }
    }

    fn to_value(&self) -> Value {
        match self {
            Self::Text(value) => json!(value),
            Self::Named { key, value } => {
                let mut map = Map::new();
                map.insert(key.clone(), json!(value));
                Value::Object(map)
            }
        }
    }
}

#[derive(Debug, Clone)]
struct SpannedMacroStmt {
    name: String,
    args: Vec<SpannedArg>,
    span: SourceSpan,
    args_span: SourceSpan,
    has_lbrace: bool,
}

#[derive(Debug)]
enum C4SemanticStatement {
    SetTitle(String),
    SetAccDescription(String),
    Macro(SpannedMacroStmt),
    Boundary(C4BoundaryId),
}

#[derive(Debug, Clone, Copy)]
struct C4BoundaryId(usize);

#[derive(Debug)]
struct C4BoundaryRecord {
    declaration: SpannedMacroStmt,
    statements: Vec<C4SemanticStatement>,
}

#[derive(Debug)]
struct C4BoundaryFrame {
    boundary: C4BoundaryId,
    has_diagram_statement: bool,
    is_valid: bool,
}

impl C4BoundaryFrame {
    fn new(declaration: SpannedMacroStmt, records: &mut Vec<C4BoundaryRecord>) -> Self {
        let boundary = C4BoundaryId(records.len());
        // Incomplete and completed boundaries share flat ownership from construction onward.
        records.push(C4BoundaryRecord {
            declaration,
            statements: Vec::new(),
        });
        Self {
            boundary,
            has_diagram_statement: false,
            is_valid: true,
        }
    }
}

fn push_c4_semantic_statement(
    root: &mut Vec<C4SemanticStatement>,
    boundaries: &mut [C4BoundaryFrame],
    records: &mut [C4BoundaryRecord],
    statement: C4SemanticStatement,
    is_diagram_statement: bool,
) {
    if let Some(boundary) = boundaries.last_mut() {
        boundary.has_diagram_statement |= is_diagram_statement;
        records[boundary.boundary.0].statements.push(statement);
    } else {
        root.push(statement);
    }
}

fn c4_other_statement_is_allowed(
    boundaries: &mut [C4BoundaryFrame],
    issues: &mut Vec<C4ParseIssue>,
    diagram_type: &str,
    span: SourceSpan,
) -> bool {
    let Some(boundary) = boundaries.last_mut() else {
        return true;
    };
    if boundary.has_diagram_statement {
        return true;
    }
    if boundary.is_valid {
        issues.push(c4_parse_issue(
            Error::diagram_parse_exact(
                diagram_type.to_string(),
                "C4 boundary must start with a diagram statement".to_string(),
                span,
            ),
            span,
        ));
        boundary.is_valid = false;
    }
    false
}

struct C4SemanticSource {
    db: C4Db,
    #[cfg(feature = "editor-facts")]
    editor_facts: Option<EditorSemanticFacts>,
}

struct C4ParseIssue {
    error: Error,
    span: SourceSpan,
}

struct C4ParseOutcome {
    source: C4SemanticSource,
    issues: Vec<C4ParseIssue>,
}

impl C4ParseOutcome {
    fn into_strict_source(self) -> Result<C4SemanticSource> {
        if let Some(issue) = self.issues.into_iter().next() {
            return Err(issue.error);
        }
        Ok(self.source)
    }

    fn into_semantic_construction(
        self,
        #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    ) -> crate::family::SemanticConstruction<C4SemanticSource> {
        #[cfg(feature = "editor-facts")]
        {
            let mut this = self;

            if editor_demand == EditorFactsDemand::Omit {
                return crate::family::SemanticConstruction {
                    model: this.into_strict_source(),
                    editor_facts: None,
                };
            }
            let mut editor_facts = this
                .source
                .editor_facts
                .take()
                .expect("requested editor facts");
            let mut first_error = None;
            for issue in this.issues {
                editor_facts.mark_recovered_from_parse_error(
                    format!("c4 parser recovered after parse error: {}", issue.error),
                    Some(issue.span),
                );
                if first_error.is_none() {
                    first_error = Some(issue.error);
                }
            }
            crate::family::SemanticConstruction {
                model: match first_error {
                    Some(error) => Err(error),
                    None => Ok(this.source),
                },
                editor_facts: Some(editor_facts),
            }
        }
        #[cfg(not(feature = "editor-facts"))]
        {
            crate::family::SemanticConstruction {
                model: self.into_strict_source(),
            }
        }
    }

    #[cfg(feature = "compat-json")]
    #[cfg(feature = "editor-facts")]
    fn into_combined(self, meta: &PresentationMetadata) -> crate::family::CombinedSemanticParse {
        let construction = self.into_semantic_construction(EditorFactsDemand::Capture);
        let editor_facts = construction.editor_facts.unwrap_or_default();
        let construction = match construction.model {
            Ok(source) => Ok((source, editor_facts)),
            Err(error) => Err(crate::family::CombinedSemanticFailure::new(
                error,
                Some(editor_facts),
            )),
        };
        crate::family::CombinedSemanticParse::from_construction(
            construction,
            |(source, editor_facts)| (source.db.into_compat_json(meta), editor_facts),
            crate::family::CombinedSemanticFailure::into_parts,
        )
    }
}

#[cfg(feature = "compat-json")]
pub(crate) fn parse_c4(code: &str, meta: &PresentationMetadata) -> Result<Value> {
    let source = construct_c4_semantic_source_controlled(code, meta, &OperationControl::new())
        .expect("a private parse control cannot be cancelled")
        .into_strict_source()?;
    source.db.into_compat_json(meta)
}

#[cfg(all(test, feature = "presentation"))]
pub(crate) fn parse_c4_model_for_render(
    code: &str,
    meta: &PresentationMetadata,
) -> Result<C4DiagramModel> {
    construct_c4_semantic_source_controlled(code, meta, &OperationControl::new())
        .expect("a private parse control cannot be cancelled")
        .into_strict_source()?
        .db
        .into_model()
}

#[cfg(feature = "presentation")]
pub(crate) fn parse_c4_model_for_render_controlled(
    code: &str,
    meta: &PresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<Result<C4DiagramModel>> {
    let outcome = construct_c4_semantic_source_controlled(code, meta, control)?;
    Ok(outcome
        .into_strict_source()
        .and_then(|source| source.db.into_model()))
}

#[cfg(feature = "compat-json")]
#[cfg(feature = "editor-facts")]
pub(crate) fn parse_c4_json_and_editor_facts(
    code: &str,
    meta: &PresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<crate::family::CombinedSemanticParse> {
    Ok(construct_c4_source_controlled(
        code,
        &meta.diagram_type,
        meta.effective_config.as_config(),
        EditorFactsDemand::Capture,
        control,
    )?
    .into_combined(meta))
}

#[cfg(feature = "compat-json")]
pub(crate) fn render_model_to_compat_json(
    model: &C4DiagramModel,
    meta: &PresentationMetadata,
) -> Result<Value> {
    let mut out = Map::with_capacity(11);
    out.insert("type".to_string(), Value::String(meta.diagram_type.clone()));
    out.insert("c4Type".to_string(), Value::String(model.c4_type.clone()));
    out.insert("title".to_string(), json!(&model.title));
    out.insert("accTitle".to_string(), json!(&model.acc_title));
    out.insert("accDescr".to_string(), json!(&model.acc_descr));
    out.insert("wrap".to_string(), Value::Bool(model.wrap));
    out.insert("layout".to_string(), json!(&model.layout));
    out.insert("shapes".to_string(), json!(&model.shapes));
    let mut boundaries = json!(&model.boundaries);
    if let Some(boundaries) = boundaries.as_array_mut() {
        for boundary in boundaries {
            let Some(boundary) = boundary.as_object_mut() else {
                continue;
            };
            if boundary.get("alias").and_then(Value::as_str) == Some("global") {
                boundary.entry("tags".to_string()).or_insert(Value::Null);
                boundary.entry("link".to_string()).or_insert(Value::Null);
            }
        }
    }
    out.insert("boundaries".to_string(), boundaries);
    out.insert("rels".to_string(), json!(&model.rels));
    out.insert(
        "config".to_string(),
        crate::json_value::clone_value_nonrecursive(meta.effective_config.as_value()),
    );
    Ok(Value::Object(out))
}

fn is_c4_header(line: &str) -> bool {
    matches!(
        line.trim(),
        "C4Context" | "C4Container" | "C4Component" | "C4Dynamic" | "C4Deployment"
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg(feature = "editor-facts")]
enum C4EntityOccurrence {
    Definition,
    Reference,
}

#[cfg(feature = "editor-facts")]
fn push_c4_entity_fact(
    facts: &mut Option<EditorSemanticFacts>,
    value: &SpannedText,
    detail: impl Into<String>,
    occurrence: C4EntityOccurrence,
) {
    let Some(facts) = facts.as_mut() else {
        return;
    };
    if value.text.is_empty() {
        facts.push_expected_syntax(EditorExpectedSyntax::new(
            EditorExpectedSyntaxKind::NodeIdentifier,
            value.span,
        ));
        return;
    }

    let detail = Some(detail.into());
    let symbol = match occurrence {
        C4EntityOccurrence::Definition => EditorSemanticSymbol::new(
            value.text.clone(),
            detail,
            EditorSemanticKind::Object,
            value.span,
            value.span,
        ),
        C4EntityOccurrence::Reference => EditorSemanticSymbol::reference(
            value.text.clone(),
            detail,
            EditorSemanticKind::Object,
            value.span,
            value.span,
        ),
    };
    facts.push_symbol(symbol);
}

#[cfg(feature = "editor-facts")]
fn push_c4_payload_fact(
    facts: &mut Option<EditorSemanticFacts>,
    value: &SpannedText,
    detail: impl Into<String>,
) {
    let Some(facts) = facts.as_mut() else {
        return;
    };
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

#[cfg(feature = "editor-facts")]
fn push_c4_payload_arg(
    facts: &mut Option<EditorSemanticFacts>,
    arg: &SpannedArg,
    fallback_detail: &str,
) {
    if facts.is_none() {
        return;
    }
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

#[cfg(feature = "editor-facts")]
fn push_c4_entity_arg(
    facts: &mut Option<EditorSemanticFacts>,
    arg: &SpannedArg,
    detail: &str,
    occurrence: C4EntityOccurrence,
) {
    if facts.is_none() {
        return;
    }
    let value = SpannedText {
        text: arg.text().to_string(),
        span: arg.span(),
    };
    push_c4_entity_fact(facts, &value, detail.to_string(), occurrence);
}

fn parse_title_spanned_c4(line: &str, line_start: usize) -> Option<SpannedText> {
    let trimmed = line.trim_start();
    let rest = trimmed.strip_prefix("title")?;
    let ws = rest.chars().next()?;
    if !ws.is_whitespace() {
        return None;
    }
    let keyword_start = line_start + line.len() - trimmed.len();
    let rest_start = keyword_start + "title".len();
    let value = spanned_trimmed_c4(rest, rest_start);
    Some(value)
}

fn parse_acc_title_spanned_c4(line: &str, line_start: usize) -> Option<SpannedText> {
    let trimmed = line.trim_start();
    let after_keyword = trimmed.strip_prefix("accTitle")?;
    let whitespace = leading_whitespace_len(after_keyword);
    let rest = after_keyword.trim_start().strip_prefix(':')?;
    let keyword_start = line_start + line.len() - trimmed.len();
    let keyword_end = keyword_start + "accTitle".len();
    let colon = keyword_end + whitespace;
    let value = spanned_trimmed_c4(rest, colon + 1);
    Some(value)
}

fn parse_acc_description_stmt_spanned_c4(line: &str, line_start: usize) -> Option<SpannedText> {
    let trimmed = line.trim_start();
    let rest = trimmed.strip_prefix("accDescription")?;
    let ws = rest.chars().next()?;
    if !ws.is_whitespace() {
        return None;
    }
    let keyword_start = line_start + line.len() - trimmed.len();
    let rest_start = keyword_start + "accDescription".len();
    let value = spanned_trimmed_c4(rest, rest_start);
    Some(value)
}

fn spanned_trimmed_c4(source: &str, source_start: usize) -> SpannedText {
    let leading = leading_whitespace_len(source);
    let value = source[leading..].trim_end();
    let start = if value.is_empty() {
        source_start + source.len()
    } else {
        source_start + leading
    };
    SpannedText {
        text: value.to_string(),
        span: SourceSpan::new(start, start + value.len()),
    }
}

fn parse_acc_descr_spanned_c4(
    lines: &mut LineCursor<'_>,
    line: &str,
    line_start: usize,
    control: &OperationControl,
) -> OperationControlResult<Option<SpannedAccDescr>> {
    control.checkpoint()?;
    let trimmed = line.trim_start();
    let keyword_start = line_start + line.len() - trimmed.len();
    let keyword_end = keyword_start + "accDescr".len();
    let Some(after_keyword) = trimmed.strip_prefix("accDescr") else {
        return Ok(None);
    };
    let whitespace = leading_whitespace_len(after_keyword);
    let rest = &after_keyword[whitespace..];
    let rest_start = keyword_end + whitespace;
    if let Some(after) = rest.strip_prefix(':') {
        let value = spanned_trimmed_c4(after, rest_start + 1);
        return Ok(Some(SpannedAccDescr {
            value,
            closed: true,
        }));
    }

    let Some(rest) = rest.strip_prefix('{') else {
        return Ok(None);
    };
    let content_start = rest_start + 1;
    if let Some(end) = rest.find('}') {
        let value = spanned_trimmed_c4(&rest[..end], content_start);
        lines.resume_same_line_at(content_start + end + 1);
        return Ok(Some(SpannedAccDescr {
            value,
            closed: true,
        }));
    }

    let mut parts = Vec::new();
    let mut span_start = None;
    let mut span_end = None;

    let first = spanned_trimmed_c4(rest, content_start);
    if !first.text.is_empty() {
        parts.push(first.text);
        span_start = Some(first.span.start);
        span_end = Some(first.span.end);
    }

    let mut closed = false;
    while let Some((next_line, segment_start)) = lines.next_line() {
        control.checkpoint()?;
        if let Some(close_pos) = next_line.find('}') {
            let before = spanned_trimmed_c4(&next_line[..close_pos], segment_start);
            if !before.text.is_empty() {
                parts.push(before.text);
                span_start.get_or_insert(before.span.start);
                span_end = Some(before.span.end);
            }
            lines.resume_same_line_at(segment_start + close_pos + 1);
            closed = true;
            break;
        }

        let text = spanned_trimmed_c4(next_line, segment_start);
        if text.text.is_empty() {
            continue;
        }
        parts.push(text.text);
        span_start.get_or_insert(text.span.start);
        span_end = Some(text.span.end);
    }

    let start = span_start.unwrap_or(content_start);
    let end = span_end.unwrap_or(start);
    Ok(Some(SpannedAccDescr {
        value: SpannedText {
            text: parts.join("\n"),
            span: SourceSpan::new(start, end),
        },
        closed,
    }))
}

fn parse_direction_stmt_facts_c4(
    line: &str,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    #[cfg(feature = "editor-facts")] line_start: usize,
    #[cfg(feature = "editor-facts")] facts: &mut Option<EditorSemanticFacts>,
) -> Option<bool> {
    let trimmed = line.trim_start();
    let rest = trimmed.strip_prefix("direction")?;
    if rest.chars().next().is_some_and(|ch| !ch.is_whitespace()) {
        return None;
    }

    #[cfg(feature = "editor-facts")]
    let keyword_start = line_start + line.len() - trimmed.len();
    #[cfg(feature = "editor-facts")]
    let keyword_end = keyword_start + "direction".len();
    let whitespace = leading_whitespace_len(rest);
    let value = &rest[whitespace..];
    if value.is_empty() {
        #[cfg(feature = "editor-facts")]
        if let Some(facts) = facts.as_mut().filter(|_| editor_demand.captures_rich()) {
            facts.push_expected_syntax(EditorExpectedSyntax::new(
                EditorExpectedSyntaxKind::CardinalDirectionValue,
                SourceSpan::new(keyword_end, keyword_end),
            ));
        }
        return Some(false);
    }

    let token = value.split_whitespace().next().unwrap_or(value);
    #[cfg(feature = "editor-facts")]
    let value_start = keyword_end + whitespace;
    #[cfg(feature = "editor-facts")]
    let value_span = SourceSpan::new(value_start, value_start + token.len());
    #[cfg(feature = "editor-facts")]
    if let Some(facts) = facts.as_mut().filter(|_| editor_demand.captures_rich()) {
        facts.push_expected_syntax(EditorExpectedSyntax::new(
            EditorExpectedSyntaxKind::CardinalDirectionValue,
            value_span,
        ));
    }
    Some(matches!(token, "TB" | "BT" | "LR" | "RL"))
}

fn parse_macro_stmt_facts_c4(
    stmt: &SpannedMacroStmt,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    #[cfg(feature = "editor-facts")] facts: &mut Option<EditorSemanticFacts>,
) -> Result<()> {
    #[cfg(feature = "editor-facts")]
    let mut omitted_facts = None;
    #[cfg(feature = "editor-facts")]
    let facts = if editor_demand.captures_rich() {
        facts
    } else {
        &mut omitted_facts
    };
    match stmt.name.as_str() {
        "Person" | "Person_Ext" | "System" | "SystemDb" | "SystemQueue" | "System_Ext"
        | "SystemDb_Ext" | "SystemQueue_Ext" => {
            #[cfg(feature = "editor-facts")]
            if let Some(alias) = stmt.args.first() {
                push_c4_entity_arg(
                    facts,
                    alias,
                    c4_shape_detail(&stmt.name),
                    C4EntityOccurrence::Definition,
                );
            }
            #[cfg(feature = "editor-facts")]
            if let Some(label) = stmt.args.get(1) {
                push_c4_payload_arg(facts, label, "c4 label");
            }
            #[cfg(feature = "editor-facts")]
            if let Some(descr) = stmt.args.get(2) {
                push_c4_payload_arg(facts, descr, "c4 description");
            }
            #[cfg(feature = "editor-facts")]
            for arg in stmt.args.iter().skip(3) {
                push_c4_payload_arg(facts, arg, "c4 value");
            }
            Ok(())
        }
        "Container" | "ContainerDb" | "ContainerQueue" | "Container_Ext" | "ContainerDb_Ext"
        | "ContainerQueue_Ext" | "Component" | "ComponentDb" | "ComponentQueue"
        | "Component_Ext" | "ComponentDb_Ext" | "ComponentQueue_Ext" => {
            #[cfg(feature = "editor-facts")]
            if let Some(alias) = stmt.args.first() {
                push_c4_entity_arg(
                    facts,
                    alias,
                    c4_shape_detail(&stmt.name),
                    C4EntityOccurrence::Definition,
                );
            }
            #[cfg(feature = "editor-facts")]
            if let Some(label) = stmt.args.get(1) {
                push_c4_payload_arg(facts, label, "c4 label");
            }
            #[cfg(feature = "editor-facts")]
            if let Some(techn) = stmt.args.get(2) {
                push_c4_payload_arg(facts, techn, "c4 technology");
            }
            #[cfg(feature = "editor-facts")]
            if let Some(descr) = stmt.args.get(3) {
                push_c4_payload_arg(facts, descr, "c4 description");
            }
            #[cfg(feature = "editor-facts")]
            for arg in stmt.args.iter().skip(4) {
                push_c4_payload_arg(facts, arg, "c4 value");
            }
            Ok(())
        }
        "Boundary" | "Enterprise_Boundary" | "System_Boundary" | "Container_Boundary" => {
            #[cfg(feature = "editor-facts")]
            if let Some(alias) = stmt.args.first() {
                push_c4_entity_arg(facts, alias, "c4 boundary", C4EntityOccurrence::Definition);
            }
            #[cfg(feature = "editor-facts")]
            if let Some(label) = stmt.args.get(1) {
                push_c4_payload_arg(facts, label, "c4 label");
            }
            #[cfg(feature = "editor-facts")]
            if let Some(boundary_type) = stmt.args.get(2) {
                push_c4_payload_arg(facts, boundary_type, "c4 boundary type");
            }
            #[cfg(feature = "editor-facts")]
            for arg in stmt.args.iter().skip(3) {
                push_c4_payload_arg(facts, arg, "c4 value");
            }
            Ok(())
        }
        "Node" | "Deployment_Node" | "Node_L" | "Node_R" => {
            #[cfg(feature = "editor-facts")]
            if let Some(alias) = stmt.args.first() {
                push_c4_entity_arg(
                    facts,
                    alias,
                    "c4 deployment node",
                    C4EntityOccurrence::Definition,
                );
            }
            #[cfg(feature = "editor-facts")]
            if let Some(label) = stmt.args.get(1) {
                push_c4_payload_arg(facts, label, "c4 label");
            }
            #[cfg(feature = "editor-facts")]
            if let Some(node_type) = stmt.args.get(2) {
                push_c4_payload_arg(facts, node_type, "c4 node type");
            }
            #[cfg(feature = "editor-facts")]
            if let Some(descr) = stmt.args.get(3) {
                push_c4_payload_arg(facts, descr, "c4 description");
            }
            #[cfg(feature = "editor-facts")]
            for arg in stmt.args.iter().skip(4) {
                push_c4_payload_arg(facts, arg, "c4 value");
            }
            Ok(())
        }
        "Rel" | "BiRel" | "Rel_U" | "Rel_Up" | "Rel_D" | "Rel_Down" | "Rel_L" | "Rel_Left"
        | "Rel_R" | "Rel_Right" | "Rel_Back" => {
            let Some(_from) = stmt.args.first() else {
                return Err(Error::diagram_parse_fallback(
                    "c4".to_string(),
                    "missing relation source".to_string(),
                ));
            };
            #[cfg(feature = "editor-facts")]
            push_c4_entity_arg(
                facts,
                _from,
                "c4 relation source",
                C4EntityOccurrence::Reference,
            );
            let Some(_to) = stmt.args.get(1) else {
                return Err(Error::diagram_parse_fallback(
                    "c4".to_string(),
                    "missing relation target".to_string(),
                ));
            };
            #[cfg(feature = "editor-facts")]
            push_c4_entity_arg(
                facts,
                _to,
                "c4 relation target",
                C4EntityOccurrence::Reference,
            );
            #[cfg(feature = "editor-facts")]
            if let Some(label) = stmt.args.get(2) {
                push_c4_payload_arg(facts, label, "c4 relation label");
            } else {
                return Ok(());
            }
            #[cfg(feature = "editor-facts")]
            if let Some(techn) = stmt.args.get(3) {
                push_c4_payload_arg(facts, techn, "c4 technology");
            }
            #[cfg(feature = "editor-facts")]
            if let Some(descr) = stmt.args.get(4) {
                push_c4_payload_arg(facts, descr, "c4 description");
            }
            #[cfg(feature = "editor-facts")]
            for arg in stmt.args.iter().skip(5) {
                push_c4_payload_arg(facts, arg, "c4 value");
            }
            Ok(())
        }
        "RelIndex" => {
            let Some(_index) = stmt.args.first() else {
                return Err(Error::diagram_parse_fallback(
                    "c4".to_string(),
                    "missing relation index".to_string(),
                ));
            };
            #[cfg(feature = "editor-facts")]
            push_c4_payload_arg(facts, _index, "c4 relation index");
            let Some(_from) = stmt.args.get(1) else {
                return Err(Error::diagram_parse_fallback(
                    "c4".to_string(),
                    "missing relation source".to_string(),
                ));
            };
            #[cfg(feature = "editor-facts")]
            push_c4_entity_arg(
                facts,
                _from,
                "c4 relation source",
                C4EntityOccurrence::Reference,
            );
            let Some(_to) = stmt.args.get(2) else {
                return Err(Error::diagram_parse_fallback(
                    "c4".to_string(),
                    "missing relation target".to_string(),
                ));
            };
            #[cfg(feature = "editor-facts")]
            push_c4_entity_arg(
                facts,
                _to,
                "c4 relation target",
                C4EntityOccurrence::Reference,
            );
            #[cfg(feature = "editor-facts")]
            if let Some(label) = stmt.args.get(3) {
                push_c4_payload_arg(facts, label, "c4 relation label");
            } else {
                return Ok(());
            }
            #[cfg(feature = "editor-facts")]
            if let Some(techn) = stmt.args.get(4) {
                push_c4_payload_arg(facts, techn, "c4 technology");
            }
            #[cfg(feature = "editor-facts")]
            if let Some(descr) = stmt.args.get(5) {
                push_c4_payload_arg(facts, descr, "c4 description");
            }
            #[cfg(feature = "editor-facts")]
            for arg in stmt.args.iter().skip(6) {
                push_c4_payload_arg(facts, arg, "c4 value");
            }
            Ok(())
        }
        "UpdateElementStyle" => {
            let Some(_target) = stmt.args.first() else {
                return Err(Error::diagram_parse_fallback(
                    "c4".to_string(),
                    "missing style target".to_string(),
                ));
            };
            #[cfg(feature = "editor-facts")]
            push_c4_entity_arg(
                facts,
                _target,
                "c4 style target",
                C4EntityOccurrence::Reference,
            );
            #[cfg(feature = "editor-facts")]
            for arg in stmt.args.iter().skip(1) {
                push_c4_payload_arg(facts, arg, "c4 style value");
            }
            Ok(())
        }
        "UpdateRelStyle" => {
            let Some(_from) = stmt.args.first() else {
                return Err(Error::diagram_parse_fallback(
                    "c4".to_string(),
                    "missing relation style source".to_string(),
                ));
            };
            #[cfg(feature = "editor-facts")]
            push_c4_entity_arg(
                facts,
                _from,
                "c4 relation style source",
                C4EntityOccurrence::Reference,
            );
            let Some(_to) = stmt.args.get(1) else {
                return Err(Error::diagram_parse_fallback(
                    "c4".to_string(),
                    "missing relation style target".to_string(),
                ));
            };
            #[cfg(feature = "editor-facts")]
            push_c4_entity_arg(
                facts,
                _to,
                "c4 relation style target",
                C4EntityOccurrence::Reference,
            );
            #[cfg(feature = "editor-facts")]
            for arg in stmt.args.iter().skip(2) {
                push_c4_payload_arg(facts, arg, "c4 relation style value");
            }
            Ok(())
        }
        "UpdateLayoutConfig" => {
            #[cfg(feature = "editor-facts")]
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

fn semantic_c4_args(args: &[SpannedArg]) -> Vec<C4Arg> {
    args.iter().map(C4Arg::from_spanned).collect()
}

fn c4_missing_arg(stmt: &SpannedMacroStmt, message: &'static str) -> Error {
    let offset = stmt
        .args
        .last()
        .map(|arg| arg.span().end)
        .unwrap_or(stmt.args_span.start);
    Error::diagram_parse_insertion_point("c4".to_string(), message.to_string(), offset)
}

fn validate_c4_macro_args(stmt: &SpannedMacroStmt) -> Result<()> {
    match stmt.name.as_str() {
        name if is_boundary_macro(name) && stmt.args.is_empty() => {
            return Err(c4_missing_arg(stmt, "missing boundary alias"));
        }
        "Rel" | "BiRel" | "Rel_U" | "Rel_Up" | "Rel_D" | "Rel_Down" | "Rel_L" | "Rel_Left"
        | "Rel_R" | "Rel_Right" | "Rel_Back" => {
            if stmt.args.is_empty() {
                return Err(c4_missing_arg(stmt, "missing relation source"));
            }
            if stmt.args.len() < 2 {
                return Err(c4_missing_arg(stmt, "missing relation target"));
            }
        }
        "RelIndex" => {
            if stmt.args.is_empty() {
                return Err(c4_missing_arg(stmt, "missing relation index"));
            }
            if stmt.args.len() < 2 {
                return Err(c4_missing_arg(stmt, "missing relation source"));
            }
            if stmt.args.len() < 3 {
                return Err(c4_missing_arg(stmt, "missing relation target"));
            }
        }
        "UpdateElementStyle" if stmt.args.is_empty() => {
            return Err(c4_missing_arg(stmt, "missing style target"));
        }
        "UpdateRelStyle" => {
            if stmt.args.is_empty() {
                return Err(c4_missing_arg(stmt, "missing relation style source"));
            }
            if stmt.args.len() < 2 {
                return Err(c4_missing_arg(stmt, "missing relation style target"));
            }
        }
        _ => {}
    }
    Ok(())
}

#[cfg(feature = "editor-facts")]
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
    let args_start = stmt_start + paren + 1;
    let Some(end_paren) = find_c4_closing_paren(after) else {
        return Err(Error::diagram_parse_fallback(
            "c4".to_string(),
            format!("unterminated macro call: {t}"),
        ));
    };

    let args_raw = &after[..end_paren];
    let parsed_args = parse_args_csv_spanned(args_raw, args_start);

    let trailing_raw = &after[end_paren + 1..];
    let trailing_whitespace = leading_whitespace_len(trailing_raw);
    let rest = &trailing_raw[trailing_whitespace..];
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

    let args = parsed_args?;
    Ok(Some(SpannedMacroStmt {
        name,
        args,
        span: SourceSpan::new(stmt_start, stmt_start + t.len()),
        args_span: SourceSpan::new(
            stmt_start + paren + 1,
            stmt_start + paren + 1 + args_raw.len(),
        ),
        has_lbrace,
    }))
}

fn find_c4_closing_paren(input: &str) -> Option<usize> {
    let mut in_quotes = false;
    for (index, ch) in input.char_indices() {
        match ch {
            '"' => in_quotes = !in_quotes,
            ')' if !in_quotes => return Some(index),
            _ => {}
        }
    }
    None
}

fn parse_args_csv_spanned(input: &str, base_offset: usize) -> Result<Vec<SpannedArg>> {
    let mut out = Vec::new();
    let mut cur = input;
    let mut cursor = base_offset;
    loop {
        if cur.trim().is_empty() {
            break;
        }
        let (seg, comma) = split_next_arg(cur);
        out.push(parse_arg_spanned(seg, cursor)?);
        let Some((comma_offset, rest)) = comma else {
            break;
        };
        cursor += comma_offset + 1;
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
    let key_source = spanned_trimmed_c4(&rest[..eq], seg_base + 1);
    let key = key_source.text.as_str();
    if key.is_empty() {
        return Err(Error::diagram_parse_fallback(
            "c4".to_string(),
            format!("invalid attribute kv key: {seg}"),
        ));
    }
    let equals = seg_base + 1 + eq;

    let val_raw = rest[eq + 1..].trim_start();
    let leading_ws = rest[eq + 1..]
        .char_indices()
        .find(|(_, ch)| !ch.is_whitespace())
        .map(|(idx, _)| idx)
        .unwrap_or(rest[eq + 1..].len());
    let value = parse_quoted_spanned(val_raw, equals + 1 + leading_ws)?;
    Ok(Some(SpannedArg {
        value: SpannedArgValue::Named(SpannedNamedArg {
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
    let value_span = SourceSpan::new(input_base + 1, input_base + 1 + value.len());
    let closing_quote = value_span.end;
    let trailing_raw = &rest[end + 1..];
    let trailing = spanned_trimmed_c4(trailing_raw, closing_quote + 1);
    if !trailing.text.is_empty() {
        return Err(Error::diagram_parse_fallback(
            "c4".to_string(),
            format!("unexpected trailing tokens after string: {}", trailing.text),
        ));
    }
    Ok(SpannedText {
        text: value.to_string(),
        span: value_span,
    })
}

impl C4Db {
    fn new(config: &MermaidConfig) -> Self {
        // Mermaid captures the root `wrap` directive while parsing and stores it on
        // each C4 source object. The renderer's diagram-local `c4.wrap` setting is
        // applied later as the second half of the `object.wrap && conf.wrap` check.
        let wrap_enabled = config.get_bool("wrap").unwrap_or(false);
        let mut db = Self {
            wrap_enabled,
            c4_shape_in_row: 4,
            c4_boundary_in_row: 2,
            ..Default::default()
        };
        db.add_global_boundary();
        db
    }

    fn add_global_boundary(&mut self) {
        self.boundaries.push(C4Boundary {
            alias: "global".to_string(),
            label: C4Text::Wrapped {
                text: Value::from("global"),
            },
            ty: Some(C4Text::Wrapped {
                text: Value::from("global"),
            }),
            ..Default::default()
        });
    }

    fn ensure_shape(&mut self, alias: &str) -> &mut C4Shape {
        if let Some(index) = self.shapes.iter().position(|shape| shape.alias == alias) {
            return &mut self.shapes[index];
        }
        self.shapes.push(C4Shape {
            alias: alias.to_string(),
            ..Default::default()
        });
        self.shapes.last_mut().expect("shape was just inserted")
    }

    fn ensure_boundary(&mut self, alias: &str) -> &mut C4Boundary {
        if let Some(index) = self
            .boundaries
            .iter()
            .position(|boundary| boundary.alias == alias)
        {
            return &mut self.boundaries[index];
        }
        self.boundaries.push(C4Boundary {
            alias: alias.to_string(),
            ..Default::default()
        });
        self.boundaries
            .last_mut()
            .expect("boundary was just inserted")
    }

    fn ensure_relation(&mut self, from: &str, to: &str) -> &mut C4RelationRecord {
        if let Some(index) = self
            .rels
            .iter()
            .position(|rel| rel.model.from_alias == from && rel.model.to_alias == to)
        {
            return &mut self.rels[index];
        }
        self.rels.push(C4RelationRecord::default());
        self.rels.last_mut().expect("relation was just inserted")
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

    fn add_person_or_system(
        &mut self,
        type_c4_shape: &str,
        args: &[C4Arg],
        parent_boundary: &str,
    ) -> Result<()> {
        let alias = positional_arg_to_string(args.first())?;
        let label = c4_arg_value_or_empty(args.get(1));

        let wrap_enabled = self.wrap_enabled;
        let obj = self.ensure_shape(&alias);
        obj.assign("label", wrap_text(label));
        assign_text_argument(obj, "descr", args.get(2), "");
        assign_optional_argument(obj, "sprite", args.get(3));
        assign_optional_argument(obj, "tags", args.get(4));
        assign_optional_argument(obj, "link", args.get(5));
        obj.assign("typeC4Shape", wrap_text(json!(type_c4_shape)));
        obj.assign("parentBoundary", json!(parent_boundary));
        obj.assign("wrap", json!(wrap_enabled));
        Ok(())
    }

    fn add_container(
        &mut self,
        type_c4_shape: &str,
        args: &[C4Arg],
        parent_boundary: &str,
    ) -> Result<()> {
        let alias = positional_arg_to_string(args.first())?;
        let label = c4_arg_value_or_empty(args.get(1));

        let wrap_enabled = self.wrap_enabled;
        let obj = self.ensure_shape(&alias);
        obj.assign("label", wrap_text(label));
        assign_text_argument(obj, "techn", args.get(2), "");
        assign_text_argument(obj, "descr", args.get(3), "");
        assign_optional_argument(obj, "sprite", args.get(4));
        assign_optional_argument(obj, "tags", args.get(5));
        assign_optional_argument(obj, "link", args.get(6));
        obj.assign("wrap", json!(wrap_enabled));
        obj.assign("typeC4Shape", wrap_text(json!(type_c4_shape)));
        obj.assign("parentBoundary", json!(parent_boundary));
        Ok(())
    }

    fn add_component(
        &mut self,
        type_c4_shape: &str,
        args: &[C4Arg],
        parent_boundary: &str,
    ) -> Result<()> {
        self.add_container(type_c4_shape, args, parent_boundary)
    }

    fn add_person_or_system_boundary(
        &mut self,
        args: &[C4Arg],
        parent_boundary: &str,
    ) -> Result<String> {
        let alias = positional_arg_to_string(args.first())?;
        let label = c4_arg_value_or_empty(args.get(1));

        let wrap_enabled = self.wrap_enabled;
        let obj = self.ensure_boundary(&alias);
        obj.assign("label", wrap_text(label));
        assign_text_argument(obj, "type", args.get(2), "system");

        assign_optional_argument(obj, "tags", args.get(3));
        assign_optional_argument(obj, "link", args.get(4));

        obj.assign("parentBoundary", json!(parent_boundary));
        obj.assign("wrap", json!(wrap_enabled));

        Ok(alias)
    }

    fn add_container_boundary(&mut self, args: &[C4Arg], parent_boundary: &str) -> Result<String> {
        let alias = positional_arg_to_string(args.first())?;
        let label = c4_arg_value_or_empty(args.get(1));

        let wrap_enabled = self.wrap_enabled;
        let obj = self.ensure_boundary(&alias);
        obj.assign("label", wrap_text(label));
        assign_text_argument(obj, "type", args.get(2), "container");
        assign_optional_argument(obj, "tags", args.get(3));
        assign_optional_argument(obj, "link", args.get(4));
        obj.assign("parentBoundary", json!(parent_boundary));
        obj.assign("wrap", json!(wrap_enabled));

        Ok(alias)
    }

    fn add_deployment_node(
        &mut self,
        node_type: &str,
        args: &[C4Arg],
        parent_boundary: &str,
    ) -> Result<String> {
        let alias = positional_arg_to_string(args.first())?;
        let label = c4_arg_value_or_empty(args.get(1));

        let wrap_enabled = self.wrap_enabled;
        let obj = self.ensure_boundary(&alias);
        obj.assign("label", wrap_text(label));

        assign_text_argument(obj, "type", args.get(2), "node");
        assign_text_argument(obj, "descr", args.get(3), "");
        assign_optional_argument(obj, "tags", args.get(5));
        assign_optional_argument(obj, "link", args.get(6));

        obj.assign("nodeType", json!(node_type));
        obj.assign("parentBoundary", json!(parent_boundary));
        obj.assign("wrap", json!(wrap_enabled));

        Ok(alias)
    }

    fn add_rel(&mut self, rel_type: &str, args: &[C4Arg]) -> Result<()> {
        let from = positional_arg_to_string(args.first())?;
        let to = positional_arg_to_string(args.get(1))?;
        let Some(label) = args.get(2).map(C4Arg::to_value) else {
            return Ok(());
        };

        let wrap_enabled = self.wrap_enabled;
        let rel = self.ensure_relation(&from, &to);

        rel.assign("type", json!(rel_type));
        rel.assign("from", json!(from));
        rel.assign("to", json!(to));
        rel.assign("label", wrap_text(label));

        assign_text_argument(rel, "techn", args.get(3), "");
        assign_text_argument(rel, "descr", args.get(4), "");

        assign_optional_argument(rel, "sprite", args.get(5));
        assign_optional_argument(rel, "tags", args.get(6));
        assign_optional_argument(rel, "link", args.get(7));
        rel.assign("wrap", json!(wrap_enabled));
        Ok(())
    }

    fn update_el_style(&mut self, args: &[C4Arg]) -> Result<()> {
        let element_name = positional_arg_to_string(args.first())?;
        if let Some(target) = self
            .shapes
            .iter_mut()
            .find(|element| element.alias == element_name)
        {
            apply_element_style_arguments(target, args);
        } else if let Some(target) = self
            .boundaries
            .iter_mut()
            .find(|element| element.alias == element_name)
        {
            apply_element_style_arguments(target, args);
        }
        Ok(())
    }

    fn update_rel_style(&mut self, args: &[C4Arg]) -> Result<()> {
        let from = positional_arg_to_string(args.first())?;
        let to = positional_arg_to_string(args.get(1))?;

        let Some(target) = self
            .rels
            .iter_mut()
            .find(|rel| rel.model.from_alias == from && rel.model.to_alias == to)
        else {
            return Ok(());
        };

        apply_relation_style_argument(target, "textColor", args.get(2));
        apply_relation_style_argument(target, "lineColor", args.get(3));
        apply_relation_style_argument(target, "offsetX", args.get(4));
        apply_relation_style_argument(target, "offsetY", args.get(5));
        Ok(())
    }

    fn update_layout_config(&mut self, args: &[C4Arg]) -> Result<()> {
        if let Some(arg) = args.first()
            && let Some(parsed) = javascript_parse_int(arg.value())
            && parsed >= 1
        {
            self.c4_shape_in_row = parsed;
        }
        if let Some(arg) = args.get(1)
            && let Some(parsed) = javascript_parse_int(arg.value())
            && parsed >= 1
        {
            self.c4_boundary_in_row = parsed;
        }
        Ok(())
    }

    #[cfg(feature = "compat-json")]
    fn into_compat_json(self, meta: &PresentationMetadata) -> Result<Value> {
        render_model_to_compat_json(&self.into_model()?, meta)
    }

    fn into_model(self) -> Result<C4DiagramModel> {
        Ok(C4DiagramModel {
            c4_type: self.c4_type,
            title: (!self.title.is_empty()).then_some(self.title),
            acc_title: None,
            acc_descr: (!self.acc_descr.is_empty()).then_some(self.acc_descr),
            wrap: self.wrap_enabled,
            layout: C4LayoutConfig {
                c4_shape_in_row: self.c4_shape_in_row,
                c4_boundary_in_row: self.c4_boundary_in_row,
            },
            shapes: self.shapes,
            boundaries: self.boundaries,
            rels: self
                .rels
                .into_iter()
                .map(C4RelationRecord::into_model)
                .collect::<Result<_>>()?,
        })
    }
}

// C4 permits named attributes to rewrite fields outside their positional slots.
// Interpret that dynamic assignment once, at the typed database owner.
trait C4Attributes {
    fn assign(&mut self, key: &str, value: Value);
    fn has_text_attribute(&self, key: &str) -> bool;
}

impl C4Attributes for C4Shape {
    fn assign(&mut self, key: &str, value: Value) {
        match key {
            "alias" => self.alias = c4_string_argument(value),
            "parentBoundary" => self.parent_boundary = c4_string_argument(value),
            "typeC4Shape" => self.type_c4_shape = c4_text_from_value(value),
            "label" => self.label = c4_text_from_value(value),
            "wrap" => self.wrap = c4_truthy(&value),
            "type" => self.ty = Some(c4_text_from_value(value)),
            "techn" => self.techn = Some(c4_text_from_value(value)),
            "descr" => self.descr = Some(c4_text_from_value(value)),
            "sprite" => self.sprite = Some(value),
            "tags" => self.tags = Some(value),
            "link" => self.link = Some(value),
            "bgColor" => self.bg_color = Some(c4_string_argument(value)),
            "borderColor" => self.border_color = Some(c4_string_argument(value)),
            "fontColor" => self.font_color = Some(c4_string_argument(value)),
            "shadowing" => self.shadowing = Some(value),
            "shape" => self.shape = Some(value),
            "legendText" => self.legend_text = Some(value),
            "legendSprite" => self.legend_sprite = Some(value),
            _ => {}
        }
    }

    fn has_text_attribute(&self, key: &str) -> bool {
        match key {
            "type" => self.ty.is_some(),
            "techn" => self.techn.is_some(),
            "descr" => self.descr.is_some(),
            _ => false,
        }
    }
}

impl C4Attributes for C4Boundary {
    fn assign(&mut self, key: &str, value: Value) {
        match key {
            "alias" => self.alias = c4_string_argument(value),
            "parentBoundary" => self.parent_boundary = c4_string_argument(value),
            "label" => self.label = c4_text_from_value(value),
            "type" => self.ty = Some(c4_text_from_value(value)),
            "descr" => self.descr = Some(c4_text_from_value(value)),
            "wrap" => self.wrap = Some(c4_truthy(&value)),
            "sprite" => self.sprite = Some(value),
            "tags" => self.tags = Some(value),
            "link" => self.link = Some(value),
            "nodeType" => self.node_type = Some(c4_string_argument(value)),
            "bgColor" => self.bg_color = Some(c4_string_argument(value)),
            "borderColor" => self.border_color = Some(c4_string_argument(value)),
            "fontColor" => self.font_color = Some(c4_string_argument(value)),
            "shadowing" => self.shadowing = Some(value),
            "shape" => self.shape = Some(value),
            "techn" => self.techn = Some(value),
            "legendText" => self.legend_text = Some(value),
            "legendSprite" => self.legend_sprite = Some(value),
            _ => {}
        }
    }

    fn has_text_attribute(&self, key: &str) -> bool {
        match key {
            "type" => self.ty.is_some(),
            "techn" => self.techn.is_some(),
            "descr" => self.descr.is_some(),
            _ => false,
        }
    }
}

#[derive(Debug, Default)]
struct C4RelationRecord {
    model: C4Relation,
    // A named text argument can temporarily replace the required string type with {text: ...}.
    // Later redeclarations/style assignments may repair it before final model validation.
    invalid_type: Option<Value>,
}

impl C4RelationRecord {
    fn into_model(self) -> Result<C4Relation> {
        if let Some(value) = self.invalid_type {
            return Err(Error::diagram_parse_fallback(
                "c4".to_string(),
                format!("expected string field `type`, got {value:?}"),
            ));
        }
        Ok(self.model)
    }
}

impl C4Attributes for C4RelationRecord {
    fn assign(&mut self, key: &str, value: Value) {
        match key {
            "from" => self.model.from_alias = c4_string_argument(value),
            "to" => self.model.to_alias = c4_string_argument(value),
            "type" => match value {
                Value::String(value) => {
                    self.model.rel_type = value;
                    self.invalid_type = None;
                }
                value => self.invalid_type = Some(value),
            },
            "label" => self.model.label = c4_text_from_value(value),
            "techn" => self.model.techn = Some(c4_text_from_value(value)),
            "descr" => self.model.descr = Some(c4_text_from_value(value)),
            "wrap" => self.model.wrap = c4_truthy(&value),
            "sprite" => self.model.sprite = Some(value),
            "tags" => self.model.tags = Some(value),
            "link" => self.model.link = Some(value),
            "offsetX" => self.model.offset_x = value_as_i64(&value),
            "offsetY" => self.model.offset_y = value_as_i64(&value),
            "lineColor" => self.model.line_color = Some(c4_string_argument(value)),
            "textColor" => self.model.text_color = Some(c4_string_argument(value)),
            _ => {}
        }
    }

    fn has_text_attribute(&self, key: &str) -> bool {
        match key {
            "type" => true,
            "techn" => self.model.techn.is_some(),
            "descr" => self.model.descr.is_some(),
            _ => false,
        }
    }
}

fn c4_string_argument(value: Value) -> String {
    let Value::String(value) = value else {
        unreachable!("C4 string fields receive string macro arguments")
    };
    value
}

fn c4_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(value) => value.as_f64().is_some_and(|value| value != 0.0),
        Value::String(value) => !value.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

fn c4_text_from_value(value: Value) -> C4Text {
    match value {
        Value::Object(mut map) if map.contains_key("text") => C4Text::Wrapped {
            text: map.remove("text").expect("text key was checked"),
        },
        Value::String(value) => C4Text::String(value),
        value => C4Text::Value(value),
    }
}

fn wrap_text(value: Value) -> Value {
    json!({"text": value})
}

fn apply_element_style_arguments(target: &mut impl C4Attributes, args: &[C4Arg]) {
    for (key, arg) in [
        "bgColor",
        "fontColor",
        "borderColor",
        "shadowing",
        "shape",
        "sprite",
        "techn",
        "legendText",
        "legendSprite",
    ]
    .into_iter()
    .zip(args.iter().skip(1))
    {
        apply_update_argument(target, key, Some(arg));
    }
}

fn positional_arg_to_string(arg: Option<&C4Arg>) -> Result<String> {
    match arg {
        None => Ok(String::new()),
        Some(C4Arg::Text(value)) => Ok(value.clone()),
        Some(C4Arg::Named { key, .. }) => Err(Error::diagram_parse_fallback(
            "c4".to_string(),
            format!("expected positional string argument, got named argument `${key}`"),
        )),
    }
}

fn c4_arg_value_or_empty(arg: Option<&C4Arg>) -> Value {
    arg.map(C4Arg::to_value).unwrap_or_else(|| json!(""))
}

fn assign_text_argument(
    obj: &mut impl C4Attributes,
    positional_key: &str,
    arg: Option<&C4Arg>,
    missing_value: &str,
) {
    match arg {
        None => {
            // Boundary type defaults reset on redeclaration; text descriptions do not.
            if positional_key == "type" {
                obj.assign(positional_key, wrap_text(json!(missing_value)));
            } else {
                if !obj.has_text_attribute(positional_key) {
                    obj.assign(positional_key, wrap_text(json!(missing_value)));
                }
            }
        }
        Some(C4Arg::Text(value)) => {
            obj.assign(positional_key, wrap_text(json!(value)));
        }
        Some(C4Arg::Named { key, value }) => assign_named_argument(obj, key, value),
    }
}

fn assign_optional_argument(
    obj: &mut impl C4Attributes,
    positional_key: &str,
    arg: Option<&C4Arg>,
) {
    match arg {
        // A named argument may occupy an earlier positional slot.  A later
        // missing positional value must not clobber the named value that was
        // already applied to the object.
        None => {}
        Some(C4Arg::Text(value)) => {
            obj.assign(positional_key, json!(value));
        }
        Some(C4Arg::Named { key, value }) => assign_named_argument(obj, key, value),
    }
}

fn assign_named_argument(obj: &mut impl C4Attributes, key: &str, value: &str) {
    let value = json!(value);
    let value = if matches!(key, "label" | "descr" | "techn" | "type") {
        wrap_text(value)
    } else {
        value
    };
    obj.assign(key, value);
}

fn apply_update_argument(obj: &mut impl C4Attributes, positional_key: &str, arg: Option<&C4Arg>) {
    let Some(arg) = arg else {
        return;
    };

    obj.assign(arg.key_or(positional_key), json!(arg.value()));
}

fn apply_relation_style_argument(
    obj: &mut impl C4Attributes,
    positional_key: &str,
    arg: Option<&C4Arg>,
) {
    let Some(arg) = arg else {
        return;
    };

    let key = arg.key_or(positional_key);
    let value = match key {
        "offsetX" | "offsetY" => javascript_parse_int(arg.value())
            .map(Value::from)
            .unwrap_or(Value::Null),
        _ => json!(arg.value()),
    };
    obj.assign(key, value);
}

fn javascript_parse_int(input: &str) -> Option<i64> {
    let input = input.trim_start_matches(|ch: char| ch.is_whitespace() || ch == '\u{feff}');
    let (negative, unsigned) = match input.as_bytes().first() {
        Some(b'-') => (true, &input[1..]),
        Some(b'+') => (false, &input[1..]),
        _ => (false, input),
    };
    let (radix, digits) = unsigned
        .strip_prefix("0x")
        .or_else(|| unsigned.strip_prefix("0X"))
        .map_or((10, unsigned), |digits| (16, digits));
    let digit_count = digits
        .bytes()
        .take_while(|byte| match radix {
            16 => byte.is_ascii_hexdigit(),
            _ => byte.is_ascii_digit(),
        })
        .count();
    if digit_count == 0 {
        return None;
    }

    let magnitude = i128::from_str_radix(&digits[..digit_count], radix).ok()?;
    let signed = if negative { -magnitude } else { magnitude };
    i64::try_from(signed).ok()
}

fn value_as_i64(v: &Value) -> Option<i64> {
    match v {
        Value::Number(n) => n
            .as_i64()
            .or_else(|| n.as_u64().and_then(|value| i64::try_from(value).ok())),
        Value::String(s) => javascript_parse_int(s),
        _ => None,
    }
}

#[cfg(all(test, feature = "compat-json"))]
fn construct_c4_semantic_source(code: &str, meta: &PresentationMetadata) -> C4ParseOutcome {
    construct_c4_source_controlled(
        code,
        &meta.diagram_type,
        meta.effective_config.as_config(),
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Capture,
        &OperationControl::new(),
    )
    .expect("a private parse control cannot be cancelled")
}

#[cfg(feature = "presentation")]
fn construct_c4_semantic_source_controlled(
    code: &str,
    meta: &PresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<C4ParseOutcome> {
    construct_c4_source_controlled(
        code,
        &meta.diagram_type,
        meta.effective_config.as_config(),
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        control,
    )
}

pub(crate) fn parse_c4_semantic_model_controlled(
    code: &str,
    meta: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &OperationControl,
) -> OperationControlResult<crate::family::SemanticConstruction<C4DiagramModel>> {
    let construction = construct_c4_source_controlled(
        code,
        &meta.diagram_type,
        &meta.semantic_config,
        #[cfg(feature = "editor-facts")]
        editor_demand,
        control,
    )?
    .into_semantic_construction(
        #[cfg(feature = "editor-facts")]
        editor_demand,
    );
    Ok(crate::family::SemanticConstruction {
        model: construction
            .model
            .and_then(|source| source.db.into_model())
            .map(|mut model| {
                model.sanitize_common_db_fields(&meta.semantic_config);
                model
            }),
        #[cfg(feature = "editor-facts")]
        editor_facts: construction.editor_facts,
    })
}

fn construct_c4_source_controlled(
    code: &str,
    diagram_type: &str,
    config: &MermaidConfig,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &OperationControl,
) -> OperationControlResult<C4ParseOutcome> {
    control.checkpoint()?;
    #[cfg(test)]
    C4_SYNTAX_CONSTRUCTION_COUNT.set(C4_SYNTAX_CONSTRUCTION_COUNT.get() + 1);
    parse_c4_semantic_source(
        code,
        diagram_type,
        config,
        #[cfg(feature = "editor-facts")]
        editor_demand,
        control,
    )
}

fn parse_c4_semantic_source(
    code: &str,
    diagram_type: &str,
    config: &MermaidConfig,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &OperationControl,
) -> OperationControlResult<C4ParseOutcome> {
    control.checkpoint()?;
    let mut db = C4Db::new(config);
    #[cfg(feature = "editor-facts")]
    let mut editor_facts = editor_demand.needs_recovery().then(|| {
        #[cfg(test)]
        crate::editor::test_work::record_collection();
        EditorSemanticFacts::new()
    });
    let mut issues = Vec::new();
    let mut semantic_statements = Vec::new();
    let mut boundary_records = Vec::new();
    let mut boundary_frames = Vec::new();
    let mut pending_boundary = None;
    let mut saw_statement_after_header = false;

    let mut lines = LineCursor::new(code);
    let mut saw_header_line = false;
    while let Some((raw, line_start)) = lines.next_line() {
        control.checkpoint()?;
        let line = strip_inline_comment(raw);
        let header = line.trim();
        if header.is_empty() {
            continue;
        }
        saw_header_line = true;
        let start = line_start + line.len() - line.trim_start().len();
        let span = SourceSpan::new(start, start + header.len());
        if is_c4_header(header) {
            db.set_c4_type(header, config);
        } else {
            issues.push(c4_parse_issue(
                Error::diagram_parse_exact(
                    diagram_type.to_string(),
                    format!("unexpected C4 header: {header}"),
                    span,
                ),
                span,
            ));
        }
        break;
    }
    if !saw_header_line {
        let span = SourceSpan::new(code.len(), code.len());
        issues.push(c4_parse_issue(
            Error::diagram_parse_insertion_point(
                diagram_type.to_string(),
                "expected C4 header",
                span.start,
            ),
            span,
        ));
    }

    while let Some((raw, line_start)) = lines.next_line() {
        control.checkpoint()?;
        let raw = strip_inline_comment(raw);
        let t = raw.trim();
        if t.is_empty() {
            continue;
        }
        saw_statement_after_header = true;

        let statement_start = line_start + raw.len() - raw.trim_start().len();
        if let Some(declaration) = pending_boundary.take() {
            if t == "{" {
                boundary_frames.push(C4BoundaryFrame::new(declaration, &mut boundary_records));
                continue;
            }
            issues.push(c4_parse_issue(
                Error::diagram_parse_insertion_point(
                    diagram_type.to_string(),
                    "expected '{' after boundary",
                    declaration.span.end,
                ),
                SourceSpan::new(declaration.span.end, declaration.span.end),
            ));
        }

        if t == "}" {
            let closing_span = SourceSpan::new(statement_start, statement_start + 1);
            let Some(frame) = boundary_frames.pop() else {
                issues.push(c4_parse_issue(
                    Error::diagram_parse_exact(
                        diagram_type.to_string(),
                        "unexpected '}' without open C4 boundary".to_string(),
                        closing_span,
                    ),
                    closing_span,
                ));
                continue;
            };
            if !frame.has_diagram_statement {
                issues.push(c4_parse_issue(
                    Error::diagram_parse_exact(
                        diagram_type.to_string(),
                        "C4 boundary must contain at least one diagram statement".to_string(),
                        closing_span,
                    ),
                    closing_span,
                ));
                continue;
            }
            if !frame.is_valid {
                continue;
            }
            push_c4_semantic_statement(
                &mut semantic_statements,
                &mut boundary_frames,
                &mut boundary_records,
                C4SemanticStatement::Boundary(frame.boundary),
                true,
            );
            continue;
        }

        if let Some(title) = parse_title_spanned_c4(raw, line_start) {
            if c4_other_statement_is_allowed(
                &mut boundary_frames,
                &mut issues,
                diagram_type,
                title.span,
            ) {
                push_c4_semantic_statement(
                    &mut semantic_statements,
                    &mut boundary_frames,
                    &mut boundary_records,
                    C4SemanticStatement::SetTitle(title.text.clone()),
                    false,
                );
            }
            #[cfg(feature = "editor-facts")]
            if let Some(facts) = editor_facts
                .as_mut()
                .filter(|_| editor_demand.captures_rich())
            {
                facts.push_directive_prefix("title");
            }
            #[cfg(feature = "editor-facts")]
            if editor_demand.captures_rich() {
                push_c4_payload_fact(&mut editor_facts, &title, "c4 title");
            }
            continue;
        }

        if let Some(acc_title) = parse_acc_title_spanned_c4(raw, line_start) {
            // Mermaid's C4 grammar maps accTitle to the diagram title.
            if c4_other_statement_is_allowed(
                &mut boundary_frames,
                &mut issues,
                diagram_type,
                acc_title.span,
            ) {
                push_c4_semantic_statement(
                    &mut semantic_statements,
                    &mut boundary_frames,
                    &mut boundary_records,
                    C4SemanticStatement::SetTitle(acc_title.text.clone()),
                    false,
                );
            }
            #[cfg(feature = "editor-facts")]
            if let Some(facts) = editor_facts
                .as_mut()
                .filter(|_| editor_demand.captures_rich())
            {
                facts.push_directive_prefix("accTitle");
            }
            #[cfg(feature = "editor-facts")]
            if editor_demand.captures_rich() {
                push_c4_payload_fact(&mut editor_facts, &acc_title, "c4 accessibility title");
            }
            continue;
        }

        if let Some(acc_description) = parse_acc_description_stmt_spanned_c4(raw, line_start) {
            if c4_other_statement_is_allowed(
                &mut boundary_frames,
                &mut issues,
                diagram_type,
                acc_description.span,
            ) {
                push_c4_semantic_statement(
                    &mut semantic_statements,
                    &mut boundary_frames,
                    &mut boundary_records,
                    C4SemanticStatement::SetAccDescription(acc_description.text.clone()),
                    false,
                );
            }
            #[cfg(feature = "editor-facts")]
            if let Some(facts) = editor_facts
                .as_mut()
                .filter(|_| editor_demand.captures_rich())
            {
                facts.push_directive_prefix("accDescription");
            }
            #[cfg(feature = "editor-facts")]
            if editor_demand.captures_rich() {
                push_c4_payload_fact(
                    &mut editor_facts,
                    &acc_description,
                    "c4 accessibility description",
                );
            }
            continue;
        }

        if let Some(acc_descr) = parse_acc_descr_spanned_c4(&mut lines, raw, line_start, control)? {
            if c4_other_statement_is_allowed(
                &mut boundary_frames,
                &mut issues,
                diagram_type,
                acc_descr.value.span,
            ) {
                push_c4_semantic_statement(
                    &mut semantic_statements,
                    &mut boundary_frames,
                    &mut boundary_records,
                    C4SemanticStatement::SetAccDescription(acc_descr.value.text.clone()),
                    false,
                );
            }
            #[cfg(feature = "editor-facts")]
            if let Some(facts) = editor_facts
                .as_mut()
                .filter(|_| editor_demand.captures_rich())
            {
                facts.push_directive_prefix("accDescr");
            }
            #[cfg(feature = "editor-facts")]
            if editor_demand.captures_rich() {
                push_c4_payload_fact(
                    &mut editor_facts,
                    &acc_descr.value,
                    "c4 accessibility description",
                );
            }
            if !acc_descr.closed {
                let error = Error::diagram_parse_insertion_point(
                    diagram_type.to_string(),
                    "unterminated C4 accDescr block",
                    code.len(),
                );
                issues.push(c4_parse_issue(error, acc_descr.value.span));
            }
            continue;
        }

        let stmt_start = statement_start;
        let statement_span = SourceSpan::new(stmt_start, stmt_start + t.len());
        if let Some(_valid) = parse_direction_stmt_facts_c4(
            raw,
            #[cfg(feature = "editor-facts")]
            editor_demand,
            #[cfg(feature = "editor-facts")]
            line_start,
            #[cfg(feature = "editor-facts")]
            &mut editor_facts,
        ) {
            issues.push(c4_parse_issue(
                Error::diagram_parse_exact(
                    diagram_type.to_string(),
                    format!("unsupported C4 statement: {t}"),
                    statement_span,
                ),
                statement_span,
            ));
            continue;
        }
        let stmt = match parse_macro_stmt_spanned(t, stmt_start) {
            Ok(Some(stmt)) => stmt,
            Ok(None) => {
                issues.push(c4_parse_issue(
                    Error::diagram_parse_exact(
                        diagram_type.to_string(),
                        format!("unsupported C4 statement: {t}"),
                        statement_span,
                    ),
                    statement_span,
                ));
                continue;
            }
            Err(error) => {
                issues.push(c4_parse_issue(error, statement_span));
                continue;
            }
        };
        if let Err(error) = validate_c4_macro_args(&stmt) {
            issues.push(c4_parse_issue(error, stmt.span));
            continue;
        }
        if let Err(error) = parse_macro_stmt_facts_c4(
            &stmt,
            #[cfg(feature = "editor-facts")]
            editor_demand,
            #[cfg(feature = "editor-facts")]
            &mut editor_facts,
        ) {
            issues.push(c4_parse_issue(error, stmt.span));
            continue;
        }
        if stmt.has_lbrace && !is_boundary_macro(&stmt.name) {
            let lbrace = SourceSpan::new(stmt.span.end.saturating_sub(1), stmt.span.end);
            issues.push(c4_parse_issue(
                Error::diagram_parse_exact(
                    diagram_type.to_string(),
                    "unexpected '{' after non-boundary C4 statement".to_string(),
                    lbrace,
                ),
                lbrace,
            ));
            continue;
        }
        if is_boundary_macro(&stmt.name) {
            if stmt.has_lbrace {
                boundary_frames.push(C4BoundaryFrame::new(stmt, &mut boundary_records));
            } else {
                pending_boundary = Some(stmt);
            }
        } else {
            push_c4_semantic_statement(
                &mut semantic_statements,
                &mut boundary_frames,
                &mut boundary_records,
                C4SemanticStatement::Macro(stmt),
                true,
            );
        }
    }

    if let Some(declaration) = pending_boundary {
        issues.push(c4_parse_issue(
            Error::diagram_parse_insertion_point(
                diagram_type.to_string(),
                "expected '{' after boundary",
                declaration.span.end,
            ),
            SourceSpan::new(declaration.span.end, declaration.span.end),
        ));
    }

    while boundary_frames.pop().is_some() {
        let eof = SourceSpan::new(code.len(), code.len());
        issues.push(c4_parse_issue(
            Error::diagram_parse_insertion_point(
                diagram_type.to_string(),
                "expected '}' before end of C4 diagram",
                code.len(),
            ),
            eof,
        ));
    }

    if saw_header_line && !saw_statement_after_header {
        let eof = SourceSpan::new(code.len(), code.len());
        issues.push(c4_parse_issue(
            Error::diagram_parse_insertion_point(
                diagram_type.to_string(),
                "expected at least one C4 statement",
                code.len(),
            ),
            eof,
        ));
    }

    apply_c4_semantic_statements(
        &mut db,
        &semantic_statements,
        &boundary_records,
        diagram_type,
        config,
        &mut issues,
        control,
    )?;
    issues.sort_by_key(|issue| issue.span.start);

    control.checkpoint()?;
    Ok(C4ParseOutcome {
        source: C4SemanticSource {
            db,
            #[cfg(feature = "editor-facts")]
            editor_facts,
        },
        issues,
    })
}

fn apply_c4_semantic_statements(
    db: &mut C4Db,
    statements: &[C4SemanticStatement],
    boundaries: &[C4BoundaryRecord],
    diagram_type: &str,
    config: &MermaidConfig,
    issues: &mut Vec<C4ParseIssue>,
    control: &OperationControl,
) -> OperationControlResult<()> {
    struct ReplayFrame<'a> {
        statements: &'a [C4SemanticStatement],
        next_statement: usize,
        parent_boundary: String,
    }

    let mut frames = vec![ReplayFrame {
        statements,
        next_statement: 0,
        parent_boundary: "global".to_string(),
    }];
    while let Some(frame) = frames.last_mut() {
        control.checkpoint()?;
        if frame.next_statement == frame.statements.len() {
            frames.pop();
            continue;
        }
        let statement = &frame.statements[frame.next_statement];
        frame.next_statement += 1;
        let parent_boundary = frame.parent_boundary.clone();
        match statement {
            C4SemanticStatement::SetTitle(title) => {
                db.set_title(title, config);
            }
            C4SemanticStatement::SetAccDescription(description) => {
                db.set_acc_description(description);
            }
            C4SemanticStatement::Macro(statement) => {
                if let Err(error) = apply_c4_macro(db, statement, diagram_type, &parent_boundary) {
                    issues.push(c4_parse_issue(error, statement.span));
                }
            }
            C4SemanticStatement::Boundary(boundary) => {
                let C4BoundaryRecord {
                    declaration,
                    statements,
                } = &boundaries[boundary.0];
                match apply_c4_macro(db, declaration, diagram_type, &parent_boundary) {
                    Ok(Some(alias)) => {
                        frames.push(ReplayFrame {
                            statements,
                            next_statement: 0,
                            parent_boundary: alias,
                        });
                    }
                    Ok(None) => issues.push(c4_parse_issue(
                        Error::diagram_parse_exact(
                            diagram_type.to_string(),
                            "expected C4 boundary declaration".to_string(),
                            declaration.span,
                        ),
                        declaration.span,
                    )),
                    Err(error) => issues.push(c4_parse_issue(error, declaration.span)),
                }
            }
        }
    }
    Ok(())
}

fn apply_c4_macro(
    db: &mut C4Db,
    stmt: &SpannedMacroStmt,
    diagram_type: &str,
    parent_boundary: &str,
) -> Result<Option<String>> {
    let name = stmt.name.as_str();
    let mut args = semantic_c4_args(&stmt.args);

    if is_boundary_macro(name) {
        match name {
            "Enterprise_Boundary" => {
                args.insert(args.len().min(2), C4Arg::Text("ENTERPRISE".to_string()))
            }
            "System_Boundary" => args.insert(args.len().min(2), C4Arg::Text("SYSTEM".to_string())),
            "Container_Boundary" => {
                args.insert(args.len().min(2), C4Arg::Text("CONTAINER".to_string()))
            }
            _ => {}
        }

        let alias = match name {
            "Boundary" | "Enterprise_Boundary" | "System_Boundary" => {
                db.add_person_or_system_boundary(&args, parent_boundary)?
            }
            "Container_Boundary" => db.add_container_boundary(&args, parent_boundary)?,
            "Node" | "Deployment_Node" => db.add_deployment_node("node", &args, parent_boundary)?,
            "Node_L" => db.add_deployment_node("nodeL", &args, parent_boundary)?,
            "Node_R" => db.add_deployment_node("nodeR", &args, parent_boundary)?,
            other => {
                return Err(Error::diagram_parse_fallback(
                    diagram_type.to_string(),
                    format!("unsupported boundary macro: {other}"),
                ));
            }
        };

        return Ok(Some(alias));
    }

    match name {
        "Person" => db.add_person_or_system("person", &args, parent_boundary)?,
        "Person_Ext" => db.add_person_or_system("external_person", &args, parent_boundary)?,
        "System" => db.add_person_or_system("system", &args, parent_boundary)?,
        "SystemDb" => db.add_person_or_system("system_db", &args, parent_boundary)?,
        "SystemQueue" => db.add_person_or_system("system_queue", &args, parent_boundary)?,
        "System_Ext" => db.add_person_or_system("external_system", &args, parent_boundary)?,
        "SystemDb_Ext" => db.add_person_or_system("external_system_db", &args, parent_boundary)?,
        "SystemQueue_Ext" => {
            db.add_person_or_system("external_system_queue", &args, parent_boundary)?
        }
        "Container" => db.add_container("container", &args, parent_boundary)?,
        "ContainerDb" => db.add_container("container_db", &args, parent_boundary)?,
        "ContainerQueue" => db.add_container("container_queue", &args, parent_boundary)?,
        "Container_Ext" => db.add_container("external_container", &args, parent_boundary)?,
        "ContainerDb_Ext" => db.add_container("external_container_db", &args, parent_boundary)?,
        "ContainerQueue_Ext" => {
            db.add_container("external_container_queue", &args, parent_boundary)?
        }
        "Component" => db.add_component("component", &args, parent_boundary)?,
        "ComponentDb" => db.add_component("component_db", &args, parent_boundary)?,
        "ComponentQueue" => db.add_component("component_queue", &args, parent_boundary)?,
        "Component_Ext" => db.add_component("external_component", &args, parent_boundary)?,
        "ComponentDb_Ext" => db.add_component("external_component_db", &args, parent_boundary)?,
        "ComponentQueue_Ext" => {
            db.add_component("external_component_queue", &args, parent_boundary)?
        }
        "Rel" => db.add_rel("rel", &args)?,
        "BiRel" => db.add_rel("birel", &args)?,
        "Rel_U" | "Rel_Up" => db.add_rel("rel_u", &args)?,
        "Rel_D" | "Rel_Down" => db.add_rel("rel_d", &args)?,
        "Rel_L" | "Rel_Left" => db.add_rel("rel_l", &args)?,
        "Rel_R" | "Rel_Right" => db.add_rel("rel_r", &args)?,
        "Rel_Back" => db.add_rel("rel_b", &args)?,
        "RelIndex" => db.add_rel("rel", &args[1..])?,
        "UpdateElementStyle" => db.update_el_style(&args)?,
        "UpdateRelStyle" => db.update_rel_style(&args)?,
        "UpdateLayoutConfig" => db.update_layout_config(&args)?,
        other => {
            return Err(Error::diagram_parse_fallback(
                diagram_type.to_string(),
                format!("unsupported C4 macro: {other}"),
            ));
        }
    }
    Ok(None)
}

fn c4_parse_issue(error: Error, fallback: SourceSpan) -> C4ParseIssue {
    let span = match &error {
        Error::DiagramParse { diagnostic, .. } => diagnostic.span().unwrap_or(fallback),
        _ => fallback,
    };
    C4ParseIssue { error, span }
}

fn strip_inline_comment(line: &str) -> &str {
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
            return &line[..idx];
        }
        idx += 1;
    }
    line
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

fn split_next_arg(input: &str) -> (&str, Option<(usize, &str)>) {
    let mut in_quotes = false;
    for (i, c) in input.char_indices() {
        match c {
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => {
                return (&input[..i], Some((i, &input[i + 1..])));
            }
            _ => {}
        }
    }
    (input, None)
}

#[cfg(test)]
mod semantic_tests {
    use super::*;

    #[test]
    #[cfg(feature = "editor-facts")]
    fn recovery_capture_retains_c4_issues_without_macro_or_payload_facts() {
        let source = "C4Context\r\ntitle Example\r\nRel(first, )\r\nPerson(later, \"Later\")\r\n";
        let metadata = crate::SemanticMetadata {
            diagram_type: "c4".into(),
            config: MermaidConfig::empty_object(),
            semantic_config: MermaidConfig::empty_object(),
            title: None,
        };
        let capture = |demand| {
            parse_c4_semantic_model_controlled(source, &metadata, demand, &OperationControl::new())
                .unwrap()
        };
        let full = capture(EditorFactsDemand::Capture);
        let full_facts = full.editor_facts.unwrap();
        assert!(!full_facts.symbols.is_empty());
        crate::editor::test_work::reset();
        reset_c4_syntax_construction_count();
        let recovery = capture(EditorFactsDemand::Recovery);
        assert_eq!(
            recovery.model.unwrap_err().to_string(),
            full.model.unwrap_err().to_string()
        );
        let facts = recovery.editor_facts.unwrap();
        assert_eq!(facts.diagnostics, full_facts.diagnostics);
        assert!(!facts.diagnostics.is_empty());
        assert!(facts.symbols.is_empty());
        assert!(facts.directive_prefixes.is_empty());
        assert!(facts.expected_syntax.is_empty());
        let work = crate::editor::test_work::read();
        assert_eq!(
            (work.symbols, work.directives, work.expected_syntax),
            (0, 0, 0)
        );
        assert_eq!(c4_syntax_construction_count(), 1);
    }

    #[test]
    fn canonical_capture_retains_all_recovery_issues_and_later_symbols() {
        let source = concat!(
            "C4Context\r\n",
            "Rel(first, )\r\n",
            "Person(later, \"Later\")\r\n",
            "Rel(second, )\r\n",
        );
        let metadata = crate::SemanticMetadata {
            diagram_type: "c4".into(),
            config: MermaidConfig::empty_object(),
            semantic_config: MermaidConfig::empty_object(),
            title: None,
        };
        reset_c4_syntax_construction_count();
        let parsed = parse_c4_semantic_model_controlled(
            source,
            &metadata,
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Capture,
            &OperationControl::new(),
        )
        .unwrap();
        assert_eq!(c4_syntax_construction_count(), 1);
        assert!(
            parsed
                .model
                .unwrap_err()
                .to_string()
                .contains("missing relation target")
        );
        #[cfg(feature = "editor-facts")]
        {
            let facts = parsed.editor_facts.expect("requested facts");
            assert_eq!(
                facts.completeness,
                crate::EditorSemanticCompleteness::Recovered
            );
            assert_eq!(facts.diagnostics.len(), 2);
            assert!(
                facts.diagnostics[0].span.unwrap().start < facts.diagnostics[1].span.unwrap().start
            );
            assert!(facts.symbols.iter().any(|symbol| symbol.name == "later"));
            assert!(facts.symbols.iter().any(|symbol| symbol.name == "Later"));
        }
    }

    #[test]
    fn canonical_c4_keeps_typed_records_and_omits_editor_observation() {
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        reset_c4_syntax_construction_count();
        let source = concat!(
            "C4Context\n",
            "Boundary(bank, \"Bank\") {\n",
            "  Person(customer, \"Customer\", $tags=\"external\")\n",
            "  System(service, \"Service\")\n",
            "  Rel(customer, service, \"Uses\")\n",
            "}\n",
            "UpdateElementStyle(customer, $bgColor=\"red\")\n",
        );
        let parsed = crate::SemanticParser::new()
            .parse_diagram_sync(source)
            .unwrap();
        assert_eq!(parsed.metadata.diagram_type, "c4");
        let model = match parsed.model {
            crate::SemanticDiagramModel::C4(model) => model,
            #[allow(unreachable_patterns)]
            _ => panic!("expected C4 semantics"),
        };
        assert_eq!(model.shapes.len(), 2);
        assert_eq!(model.shapes[0].alias, "customer");
        assert_eq!(model.shapes[0].parent_boundary, "bank");
        assert_eq!(model.shapes[0].label.as_str(), "Customer");
        assert_eq!(model.shapes[0].tags, Some(Value::from("external")));
        assert_eq!(model.shapes[0].bg_color.as_deref(), Some("red"));
        assert_eq!(model.rels[0].from_alias, "customer");
        assert_eq!(model.rels[0].to_alias, "service");
        assert_eq!(model.boundaries[1].alias, "bank");
        assert_eq!(c4_syntax_construction_count(), 1);
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());

        #[cfg(feature = "presentation")]
        {
            let full = crate::MermaidEngine::new()
                .parse_diagram_for_render_model_with_type_sync(
                    "c4",
                    source,
                    crate::ParseOptions::strict(),
                )
                .unwrap()
                .unwrap();
            let crate::RenderSemanticModel::C4(full) = full.model() else {
                panic!("expected full C4 model");
            };
            assert_eq!(
                serde_json::to_value(&model).unwrap(),
                serde_json::to_value(full).unwrap()
            );
            #[cfg(feature = "editor-facts")]
            assert_eq!(crate::editor::test_work::read(), Default::default());
        }
    }

    #[test]
    fn canonical_c4_preserves_validation_and_cancellation_without_observers() {
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        for source in [
            "C4Dynamic\nRel(a)\n",
            "C4Context\nUnexpected(a, \"A\") {\n",
            "C4Context\ndirection LR\n",
        ] {
            let error = crate::SemanticParser::new()
                .parse_diagram_sync(source)
                .unwrap_err();
            assert!(matches!(error, Error::DiagramParse { .. }));
            #[cfg(feature = "compat-json")]
            {
                let full = crate::MermaidEngine::new()
                    .parse_diagram_sync(source, crate::ParseOptions::strict())
                    .unwrap_err();
                assert_eq!(error.to_string(), full.to_string());
            }
        }
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
        let control = OperationControl::new();
        control.cancel();
        let error = crate::SemanticParser::new()
            .parse_diagram_controlled_sync("C4Context\nPerson(p, \"P\")\n", &control)
            .unwrap_err();
        assert_eq!(error.reason, crate::CancelReason::Requested);
    }
}

#[cfg(all(test, feature = "compat-json"))]
mod tests {
    use super::*;
    #[cfg(feature = "editor-facts")]
    use crate::{EditorSemanticCompleteness, EditorSemanticRole};
    use crate::{
        MermaidConfig, MermaidEngine, ParseDiagnosticSpanKind, ParseOptions, PresentationMetadata,
        RenderSemanticModel,
    };
    use futures::executor::block_on;
    use serde_json::json;

    fn parse(text: &str) -> crate::ManagedSemanticJson {
        let engine = MermaidEngine::new();
        block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap()
            .model
    }

    fn parse_err(text: &str) -> crate::ParseDiagnostic {
        let engine = MermaidEngine::new();
        match block_on(engine.parse_diagram(text, ParseOptions::default())).unwrap_err() {
            Error::DiagramParse { diagnostic, .. } => diagnostic,
            other => panic!("expected C4 parse error, got {other:?}"),
        }
    }

    fn recovered_model_and_errors(text: &str) -> (Value, Vec<String>) {
        let outcome = construct_c4_semantic_source(text, &meta());
        let errors = outcome
            .issues
            .iter()
            .map(|issue| issue.error.to_string())
            .collect();
        let model = outcome.source.db.into_compat_json(&meta()).unwrap();
        (model, errors)
    }

    fn meta() -> PresentationMetadata {
        PresentationMetadata {
            diagram_type: "c4".to_string(),
            config: MermaidConfig::empty_object(),
            effective_config: crate::presentation::PresentationConfig::empty_object(),
            title: None,
        }
    }

    fn boundary_chain(depth: usize) -> String {
        let mut source = String::from("C4Context\n");
        for level in 0..depth {
            source.push_str(&format!("Boundary(b{level}, \"B{level}\") {{\n"));
        }
        source.push_str("System(leaf, \"Leaf\")\n");
        for _ in 0..depth {
            source.push_str("}\n");
        }
        source
    }

    #[test]
    fn c4_cancellation_during_boundary_closure_releases_partial_fragments() {
        std::thread::Builder::new()
            .name("c4-deep-closure-cancellation".into())
            .stack_size(2 * 1024 * 1024)
            .spawn(|| {
                const DEPTH: usize = 15_000;
                let source = boundary_chain(DEPTH);
                let control = OperationControl::new().for_phase(crate::OperationPhase::Parse);
                // Construction entry/header use three checkpoints; each macro uses two. Cancel
                // halfway through the closers, after completed inner boundaries belong to an
                // open frame. Nextest isolates this named unit in its own OS subprocess.
                control.cancel_after_checkpoints(3 + 2 * (DEPTH + 1) + DEPTH / 2);

                let cancelled =
                    match construct_c4_semantic_source_controlled(&source, &meta(), &control) {
                        Ok(_) => panic!("closure construction must observe cancellation"),
                        Err(cancelled) => cancelled,
                    };
                assert_eq!(cancelled.reason, crate::CancelReason::Requested);
                assert_eq!(cancelled.phase, crate::OperationPhase::Parse);
                assert_eq!(control.checkpoint().unwrap_err(), cancelled);
                assert!(
                    parse_c4_model_for_render("C4Context\nSystem(after, \"After\")\n", &meta())
                        .is_ok()
                );
                println!(
                    "depth={DEPTH} worker_stack_bytes=2097152 phase=after-small-operation status=done"
                );
            })
            .expect("spawn 2 MiB C4 closure-cancellation worker")
            .join()
            .expect("C4 closure cancellation releases completed inner frames");
    }

    #[test]
    fn c4_cancellation_during_boundary_replay_releases_partial_fragments() {
        std::thread::Builder::new()
            .name("c4-deep-replay-cancellation".into())
            .stack_size(2 * 1024 * 1024)
            .spawn(|| {
                const DEPTH: usize = 15_000;
                let source = boundary_chain(DEPTH);
                let control = OperationControl::new().for_phase(crate::OperationPhase::Parse);
                // Consume construction and all closers, then enter half the nested replay
                // frames. Nextest isolates this named unit in its own OS subprocess.
                control.cancel_after_checkpoints(3 + 2 * (DEPTH + 1) + DEPTH + DEPTH / 2);

                let cancelled =
                    match construct_c4_semantic_source_controlled(&source, &meta(), &control) {
                        Ok(_) => panic!("boundary replay must observe cancellation"),
                        Err(cancelled) => cancelled,
                    };
                assert_eq!(cancelled.reason, crate::CancelReason::Requested);
                assert_eq!(cancelled.phase, crate::OperationPhase::Parse);
                assert_eq!(control.checkpoint().unwrap_err(), cancelled);
                assert!(
                    parse_c4_model_for_render("C4Context\nSystem(after, \"After\")\n", &meta())
                        .is_ok()
                );
                println!(
                    "depth={DEPTH} worker_stack_bytes=2097152 phase=after-small-operation status=done"
                );
            })
            .expect("spawn 2 MiB C4 replay-cancellation worker")
            .join()
            .expect("C4 replay cancellation releases completed boundary records");
    }

    #[test]
    fn c4_flat_boundaries_preserve_declaration_context_and_statement_order() {
        let source = r#"C4Context
title Root title
System(before, "Before")
Boundary(outer, "Outer") {
  System(outer_before, "Outer Before")
  accTitle: Outer title
  Boundary(inner, "Inner") {
    System(inside, "Inside")
    accDescr: Inner description
    Rel(inside, before, "Inside relation")
  }
  System(outer_after, "Outer After")
}
Boundary(sibling, "Sibling") {
  System(sibling_shape, "Sibling Shape")
  title Sibling title
}
System(after, "After")
Rel(after, sibling_shape, "After relation")
"#;
        let model = parse_c4_model_for_render(source, &meta()).expect("valid boundary contexts");
        assert_eq!(model.title.as_deref(), Some("Sibling title"));
        assert_eq!(model.acc_title, None);
        assert_eq!(model.acc_descr.as_deref(), Some("Inner description"));
        assert_eq!(
            model
                .boundaries
                .iter()
                .map(|boundary| (boundary.alias.as_str(), boundary.parent_boundary.as_str()))
                .collect::<Vec<_>>(),
            [
                ("global", ""),
                ("outer", "global"),
                ("inner", "outer"),
                ("sibling", "global")
            ]
        );
        assert_eq!(
            model
                .shapes
                .iter()
                .map(|shape| (shape.alias.as_str(), shape.parent_boundary.as_str()))
                .collect::<Vec<_>>(),
            [
                ("before", "global"),
                ("outer_before", "outer"),
                ("inside", "inner"),
                ("outer_after", "outer"),
                ("sibling_shape", "sibling"),
                ("after", "global"),
            ]
        );
        assert_eq!(
            model
                .rels
                .iter()
                .map(|relation| relation.label.as_str())
                .collect::<Vec<_>>(),
            ["Inside relation", "After relation"]
        );
    }

    #[test]
    fn c4_completed_inner_boundary_is_discarded_with_unclosed_outer() {
        let source = r#"C4Context
title Root title
System(before, "Before")
Boundary(open, "Open") {
  Boundary(completed, "Completed") {
    System(inside, "Inside")
  }
  title Unfinished title
"#;
        let outcome = construct_c4_semantic_source(source, &meta());
        assert_eq!(outcome.issues.len(), 1);
        assert_eq!(
            outcome.issues[0].span,
            SourceSpan::new(source.len(), source.len())
        );
        assert!(
            outcome.issues[0]
                .error
                .to_string()
                .contains("expected '}' before end")
        );
        let model = outcome
            .source
            .db
            .into_model()
            .expect("safe recovered model");
        assert_eq!(model.boundaries.len(), 1);
        assert_eq!(model.shapes.len(), 1);
        assert_eq!(model.shapes[0].alias, "before");
        assert_eq!(model.title.as_deref(), Some("Root title"));
        #[cfg(feature = "editor-facts")]
        assert!(
            outcome
                .source
                .editor_facts
                .as_ref()
                .expect("requested editor facts")
                .symbols
                .iter()
                .any(|symbol| symbol.name == "inside")
        );
    }

    #[test]
    fn c4_empty_nested_boundary_does_not_publish_or_change_sibling_context() {
        let (model, errors) = recovered_model_and_errors(
            r#"C4Context
Boundary(outer, "Outer") {
  System(before, "Before")
  Boundary(empty, "Empty") {
  }
  Boundary(sibling, "Sibling") {
    System(inside, "Inside")
  }
  System(after, "After")
}
System(root_after, "Root After")
"#,
        );
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("boundary must contain"));
        assert_eq!(model["boundaries"].as_array().unwrap().len(), 3);
        assert_eq!(model["boundaries"][2]["alias"], json!("sibling"));
        assert_eq!(model["boundaries"][2]["parentBoundary"], json!("outer"));
        for (index, parent) in ["outer", "sibling", "outer", "global"].iter().enumerate() {
            assert_eq!(model["shapes"][index]["parentBoundary"], json!(parent));
        }
    }

    #[test]
    fn combined_parse_constructs_once_and_preserves_all_projections() {
        let text = concat!(
            "C4Context\r\n",
            "title Banking Context\r\n",
            "accTitle: Banking accessibility title\r\n",
            "accDescr {\r\n",
            "  Banking accessibility description\r\n",
            "}\r\n",
            "Boundary(bank, \"Bank\") {\r\n",
            "  Person(customer, \"Customer\", \"Uses the system\")\r\n",
            "  System(system, \"Internet Banking\", \"Core system\")\r\n",
            "}\r\n",
            "Rel(customer, system, \"Uses\", \"HTTPS\")\r\n",
        );
        let expected_json = parse_c4(text, &meta()).unwrap();
        let expected_model = parse_c4_model_for_render(text, &meta()).unwrap();

        reset_c4_syntax_construction_count();
        #[cfg(feature = "editor-facts")]
        let (json, facts) = crate::family::test_support::into_result(
            parse_c4_json_and_editor_facts(text, &meta(), &OperationControl::new()),
        )
        .unwrap();

        #[cfg(not(feature = "editor-facts"))]
        let json = parse_c4(text, &meta()).unwrap();
        assert_eq!(c4_syntax_construction_count(), 1);
        assert_eq!(json, expected_json);
        #[cfg(feature = "editor-facts")]
        assert!(!facts.symbols.is_empty());
        assert_eq!(
            render_model_to_compat_json(&expected_model, &meta()).unwrap(),
            json,
            "C4 typed compatibility projection drifted"
        );
        assert!(
            json["boundaries"][0]
                .get("link")
                .is_some_and(Value::is_null),
            "the global C4 boundary must preserve Mermaid's explicit link:null"
        );
        assert!(
            json["boundaries"][0]
                .get("tags")
                .is_some_and(Value::is_null),
            "the global C4 boundary must preserve Mermaid's explicit tags:null"
        );
        assert_eq!(json["title"].as_str(), expected_model.title.as_deref());
        assert_eq!(
            json["shapes"].as_array().unwrap().len(),
            expected_model.shapes.len()
        );
        assert_eq!(
            json["rels"].as_array().unwrap().len(),
            expected_model.rels.len()
        );

        #[cfg(feature = "editor-facts")]
        for (name, marker) in [
            ("bank", "Boundary(bank"),
            ("customer", "Person(customer"),
            ("system", "System(system"),
            ("Banking Context", "Banking Context"),
        ] {
            let marker_start = text.find(marker).unwrap();
            let start = marker_start + marker.find(name).unwrap();
            assert!(
                facts.symbols.iter().any(|symbol| {
                    symbol.name == name
                        && symbol.selection == SourceSpan::new(start, start + name.len())
                }) || facts.expected_syntax.iter().any(|expected| {
                    expected.span == SourceSpan::new(start, start + name.len())
                }),
                "missing exact C4 fact for {name:?}"
            );
        }
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn malformed_editor_input_recovers_from_one_construction() {
        let text = "C4Context\nPerson(customer, \"Customer\")\nNotAMacro customer\n";
        reset_c4_syntax_construction_count();
        let facts = crate::family::test_support::editor_facts(
            parse_c4_json_and_editor_facts,
            text,
            &meta(),
        );

        assert_eq!(c4_syntax_construction_count(), 1);
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
        assert!(facts.symbols.iter().any(|symbol| symbol.name == "customer"));
        assert_eq!(facts.diagnostics.len(), 1);
    }

    #[test]
    fn unterminated_acc_descr_reports_eof_and_reuses_partial_facts() {
        let text = "C4Context\naccDescr {\n  partial description\n";
        let error = parse_c4(text, &meta()).unwrap_err();
        let Error::DiagramParse { diagnostic, .. } = error else {
            panic!("expected C4 parse diagnostic");
        };
        assert_eq!(
            diagnostic.span_kind(),
            ParseDiagnosticSpanKind::InsertionPoint
        );
        assert_eq!(
            diagnostic.span(),
            Some(SourceSpan::new(text.len(), text.len()))
        );

        #[cfg(feature = "editor-facts")]
        {
            reset_c4_syntax_construction_count();
            let facts = crate::family::test_support::editor_facts(
                parse_c4_json_and_editor_facts,
                text,
                &meta(),
            );
            assert_eq!(c4_syntax_construction_count(), 1);
            assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
            assert!(
                facts
                    .symbols
                    .iter()
                    .any(|symbol| symbol.name == "partial description")
            );
            assert_eq!(facts.diagnostics.len(), 1);
            assert_eq!(
                facts.diagnostics[0].span,
                Some(SourceSpan::new(text.len(), text.len()))
            );
        }
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
    fn c4_redeclaring_shape_keeps_omitted_optional_fields_like_mermaid() {
        let model = parse(
            r#"C4Context
Person(customer, "First", "Original description", "users", "retail", "https://example.com")
Person(customer, "Second")
"#,
        );
        let shape = model["shapes"][0].as_object().unwrap();

        assert_eq!(shape["label"]["text"], json!("Second"));
        assert_eq!(shape["descr"]["text"], json!("Original description"));
        assert_eq!(shape["sprite"], json!("users"));
        assert_eq!(shape["tags"], json!("retail"));
        assert_eq!(shape["link"], json!("https://example.com"));
    }

    #[test]
    fn c4_redeclaring_container_and_component_keeps_omitted_optional_fields() {
        let model = parse(
            r#"C4Component
Container(container, "First container", "Rust", "Description", "database", "backend", "https://example.com/container")
Container(container, "Second container")
Component(component, "First component", "Rust", "Description", "server", "backend", "https://example.com/component")
Component(component, "Second component")
"#,
        );
        let shapes = model["shapes"].as_array().unwrap();

        assert_eq!(
            shapes.len(),
            2,
            "redeclaration must preserve insertion order"
        );
        for (shape, alias, label) in [
            (&shapes[0], "container", "Second container"),
            (&shapes[1], "component", "Second component"),
        ] {
            let shape = shape.as_object().unwrap();
            assert_eq!(shape["alias"], json!(alias));
            assert_eq!(shape["label"]["text"], json!(label));
            assert_eq!(shape["techn"]["text"], json!("Rust"));
            assert_eq!(shape["descr"]["text"], json!("Description"));
            assert!(shape.contains_key("sprite"));
            assert!(shape.contains_key("tags"));
            assert!(shape.contains_key("link"));
        }
    }

    #[test]
    fn c4_redeclaring_relation_keeps_omitted_optional_fields() {
        let model = parse(
            r#"C4Dynamic
Rel(a, b, "First", "HTTPS", "Description", "server", "backend", "https://example.com")
Rel(a, b, "Second")
"#,
        );
        let rel = model["rels"][0].as_object().unwrap();

        assert_eq!(model["rels"].as_array().unwrap().len(), 1);
        assert_eq!(rel["label"]["text"], json!("Second"));
        assert_eq!(rel["techn"]["text"], json!("HTTPS"));
        assert_eq!(rel["descr"]["text"], json!("Description"));
        assert!(rel.contains_key("sprite"));
        assert!(rel.contains_key("tags"));
        assert!(rel.contains_key("link"));
    }

    #[test]
    fn c4_declaration_named_arguments_cannot_override_structural_fields() {
        let model = parse(
            r#"C4Context
Boundary(b, "Boundary") {
  Person(p, "Person", "", $wrap="false", $parentBoundary="outside", $typeC4Shape="wrong")
}
"#,
        );
        let shape = &model["shapes"][0];

        assert_eq!(shape["alias"], json!("p"));
        assert_eq!(shape["parentBoundary"], json!("b"));
        assert_eq!(shape["typeC4Shape"]["text"], json!("person"));
        assert_eq!(shape["wrap"], json!(false));
    }

    #[test]
    fn c4_named_arguments_in_earlier_slots_are_not_clobbered() {
        let model = parse(
            r#"C4Context
Person(p, "Person", "Description", $tags="tag1,tag2", $link="https://example.com")
"#,
        );
        let shape = model["shapes"][0].as_object().unwrap();

        assert_eq!(shape["tags"], json!("tag1,tag2"));
        assert_eq!(shape["link"], json!("https://example.com"));
    }

    #[test]
    fn c4_named_attributes_use_field_types_independent_of_argument_slot() {
        // Mermaid 12 c4NamedAttributes.spec.ts: both text and optional slots use TEXT_FIELDS.
        for (header, statement, collection, index, key, expected) in [
            (
                "C4Context",
                r#"System(s, "S", $tags="cylinder")"#,
                "shapes",
                0,
                "tags",
                json!("cylinder"),
            ),
            (
                "C4Context",
                r#"Person(p, "P", $descr="description")"#,
                "shapes",
                0,
                "descr",
                json!({"text": "description"}),
            ),
            (
                "C4Container",
                r#"Container(c, "C", $sprite="database", $link="https://example.com")"#,
                "shapes",
                0,
                "sprite",
                json!("database"),
            ),
            (
                "C4Container",
                r#"Container(c, "C", $descr="description")"#,
                "shapes",
                0,
                "descr",
                json!({"text": "description"}),
            ),
            (
                "C4Component",
                r#"Component(c, "C", "Rust", "Description", $techn="Java")"#,
                "shapes",
                0,
                "techn",
                json!({"text": "Java"}),
            ),
            (
                "C4Container",
                r#"System_Boundary(b, "B", $type="custom") {
  Container(c, "C")
}"#,
                "boundaries",
                1,
                "type",
                json!({"text": "custom"}),
            ),
            (
                "C4Container",
                r#"Container_Boundary(b, "B", $type="custom") {
  Component(c, "C")
}"#,
                "boundaries",
                1,
                "type",
                json!({"text": "custom"}),
            ),
            (
                "C4Deployment",
                r#"Node(b, "B", $descr="description") {
  Container(c, "C")
}"#,
                "boundaries",
                1,
                "descr",
                json!({"text": "description"}),
            ),
            (
                "C4Dynamic",
                r#"Rel(a, b, "Uses", $tags="async")"#,
                "rels",
                0,
                "tags",
                json!("async"),
            ),
            (
                "C4Dynamic",
                r#"Rel(a, b, "Uses", $descr="description")"#,
                "rels",
                0,
                "descr",
                json!({"text": "description"}),
            ),
            (
                "C4Context",
                r#"System(s, "S", "Description", $label="Renamed")"#,
                "shapes",
                0,
                "label",
                json!({"text": "Renamed"}),
            ),
        ] {
            let source = format!("{header}\n{statement}\n");
            let model = parse(&source);
            assert_eq!(model[collection][index][key], expected, "{statement}");
        }

        let model = parse("C4Container\nContainer(c, \"C\", $techn=\"Java\")\n");
        assert_eq!(model["shapes"][0]["descr"], json!({"text": ""}));
    }

    #[test]
    fn c4_alias_updates_keep_first_match_lookup_semantics() {
        let model = parse(
            r#"C4Context
Person(a, "A")
UpdateElementStyle(a, $alias="b")
UpdateElementStyle(b, $bgColor="red")
Person(b, "B")
"#,
        );
        let shapes = model["shapes"].as_array().unwrap();

        assert_eq!(shapes.len(), 1);
        assert_eq!(shapes[0]["alias"], json!("b"));
        assert_eq!(shapes[0]["label"]["text"], json!("B"));
        assert_eq!(shapes[0]["bgColor"], json!("red"));
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
  Person(child, "Child")
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
    fn c4_update_rel_style_sparse_named_offsets_follow_their_keys() {
        let model = parse(
            r#"C4Dynamic
Rel(c2, c3, "Calls isAuthenticated() on")
UpdateRelStyle(c2, c3, $textColor="red", $offsetX="-40", $offsetY="60")
"#,
        );
        let rel = &model["rels"][0];

        assert_eq!(rel["textColor"], json!("red"));
        assert!(
            rel.get("lineColor").is_none(),
            "omitting lineColor must not consume a named offset"
        );
        assert_eq!(rel["offsetX"], json!(-40));
        assert_eq!(rel["offsetY"], json!(60));
    }

    #[test]
    fn c4_update_rel_style_named_fields_are_order_independent() {
        let model = parse(
            r#"C4Dynamic
Rel(a, b, "label")
UpdateRelStyle(a, b, $offsetX="-40", $textColor="red", $offsetY="60")
"#,
        );
        let rel = &model["rels"][0];

        assert_eq!(rel["textColor"], json!("red"));
        assert_eq!(rel["offsetX"], json!(-40));
        assert_eq!(rel["offsetY"], json!(60));
    }

    #[test]
    fn c4_update_rel_style_uses_javascript_parse_int_semantics() {
        let model = parse(
            r#"C4Dynamic
Rel(a, b, "positional")
Rel(c, d, "named")
UpdateRelStyle(a, b, "red", "blue", "   -40px", "+60.75")
UpdateRelStyle(c, d, $offsetX="not-a-number", $offsetY="0x10tail")
"#,
        );
        let positional = &model["rels"][0];
        let named = &model["rels"][1];

        assert_eq!(positional["textColor"], json!("red"));
        assert_eq!(positional["lineColor"], json!("blue"));
        assert_eq!(positional["offsetX"], json!(-40));
        assert_eq!(positional["offsetY"], json!(60));
        assert!(named.get("offsetX").is_none());
        assert_eq!(named["offsetY"], json!(16));
    }

    #[test]
    fn c4_update_rel_style_preserves_unknown_named_key_side_effects() {
        let model = parse(
            r#"C4Dynamic
Rel(a, b, "label")
UpdateRelStyle(a, b, $wrap="false")
UpdateRelStyle(a, b, $from="c")
UpdateRelStyle(c, b, $lineColor="red")
"#,
        );
        let rel = &model["rels"][0];

        assert_eq!(rel["from"], json!("c"));
        assert_eq!(rel["lineColor"], json!("red"));
        assert_eq!(rel["wrap"], json!(true));
    }

    #[test]
    fn relation_named_type_retains_validation_until_a_later_assignment() {
        let source = "C4Dynamic\nRel(a, b, \"label\", $type=\"custom\")\n";
        let error = parse_err(source);
        assert!(error.message().contains("expected string field `type`"));
        let repaired = format!("{source}UpdateRelStyle(a, b, $type=\"custom\")\n");
        assert_eq!(parse(&repaired)["rels"][0]["type"], json!("custom"));
    }

    #[test]
    fn relation_redeclaration_replaces_an_invalid_named_type() {
        let source = concat!(
            "C4Dynamic\n",
            "Rel(a, b, \"first\", $type=\"custom\")\n",
            "Rel(a, b, \"second\")\n",
        );
        let model = parse(source);
        assert_eq!(model["rels"][0]["type"], json!("rel"));
        assert_eq!(model["rels"][0]["label"]["text"], json!("second"));
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
  Person(p1, "P1")
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
    fn c4_non_boundary_macro_cannot_open_a_boundary_body() {
        let source = r#"C4Context
Person(p, "P") {
"#;
        let (model, errors) = recovered_model_and_errors(source);

        assert!(model["shapes"].as_array().unwrap().is_empty());
        assert!(
            errors
                .iter()
                .any(|error| error.contains("unexpected '{' after non-boundary"))
        );
        parse_err(source);
    }

    #[test]
    fn c4_boundary_must_start_with_a_diagram_statement() {
        let (model, errors) = recovered_model_and_errors(
            r#"C4Context
Boundary(b, "B") {
  title Invalid inside boundary
  Person(p, "P")
}
"#,
        );

        assert_eq!(model["boundaries"].as_array().unwrap().len(), 1);
        assert!(model["shapes"].as_array().unwrap().is_empty());
        assert!(model["title"].is_null());
        assert!(
            errors
                .iter()
                .any(|error| error.contains("must start with a diagram statement"))
        );
    }

    #[test]
    fn c4_boundary_without_validated_brace_does_not_mutate_db_or_parent() {
        let (model, errors) = recovered_model_and_errors(
            r#"C4Context
Boundary(bank, "Bank")
Person(after, "After")
"#,
        );

        assert!(
            errors
                .iter()
                .any(|error| error.contains("expected '{' after boundary"))
        );
        assert_eq!(model["boundaries"].as_array().unwrap().len(), 1);
        assert_eq!(model["shapes"][0]["alias"], json!("after"));
        assert_eq!(model["shapes"][0]["parentBoundary"], json!("global"));
    }

    #[test]
    fn c4_empty_boundary_is_rejected_and_rolled_back() {
        let (model, errors) = recovered_model_and_errors(
            r#"C4Context
Boundary(empty, "Empty") {
}
"#,
        );

        assert!(
            errors
                .iter()
                .any(|error| error.contains("boundary must contain"))
        );
        assert_eq!(model["boundaries"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn c4_unclosed_boundary_is_rejected_and_rolled_back_at_eof() {
        let source = r#"C4Context
Boundary(open, "Open") {
  Person(inside, "Inside")
"#;
        let (model, errors) = recovered_model_and_errors(source);

        assert!(
            errors
                .iter()
                .any(|error| error.contains("expected '}' before end"))
        );
        assert_eq!(model["boundaries"].as_array().unwrap().len(), 1);
        assert!(model["shapes"].as_array().unwrap().is_empty());

        #[cfg(feature = "editor-facts")]
        {
            let facts = crate::family::test_support::editor_facts(
                parse_c4_json_and_editor_facts,
                source,
                &meta(),
            );
            assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
            assert!(facts.symbols.iter().any(|symbol| symbol.name == "inside"));
        }
    }

    #[test]
    fn c4_render_model_fails_closed_for_boundary_structure_errors() {
        for source in [
            "C4Context\nBoundary(missing, \"Missing\")\nPerson(after, \"After\")\n",
            "C4Context\nBoundary(empty, \"Empty\") {\n}\n",
            "C4Context\nBoundary(open, \"Open\") {\nPerson(inside, \"Inside\")\n",
            "C4Context\nPerson(before, \"Before\")\n}\n",
        ] {
            assert!(
                parse_c4_model_for_render(source, &meta()).is_err(),
                "render projection accepted structurally invalid C4 source: {source:?}"
            );
        }
    }

    #[test]
    fn c4_unmatched_closing_brace_does_not_change_safe_sibling_parent() {
        let (model, errors) = recovered_model_and_errors(
            r#"C4Context
Boundary(bank, "Bank") {
  Person(inside, "Inside")
}
}
Person(after, "After")
"#,
        );

        assert!(errors.iter().any(|error| error.contains("unexpected '}'")));
        assert_eq!(model["shapes"][1]["alias"], json!("after"));
        assert_eq!(model["shapes"][1]["parentBoundary"], json!("global"));
    }

    #[test]
    fn c4_direction_after_header_is_rejected_but_kept_in_editor_facts() {
        let source = "C4Context\nPerson(customer, \"Customer\")\ndirection LR\n";
        let error = parse_c4(source, &meta()).expect_err("direction is not a C4 statement");
        assert!(error.to_string().contains("unsupported C4 statement"));

        #[cfg(feature = "editor-facts")]
        {
            let facts = crate::family::test_support::editor_facts(
                parse_c4_json_and_editor_facts,
                source,
                &meta(),
            );
            assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
            let lr = source.find("LR").unwrap();
            assert!(facts.expected_syntax.iter().any(|expected| {
                expected.kind == EditorExpectedSyntaxKind::CardinalDirectionValue
                    && expected.span == SourceSpan::new(lr, lr + "LR".len())
            }));
        }
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
    fn c4_container_boundary_with_only_alias_uses_container_default_type() {
        let model = parse(
            r#"C4Container
Container_Boundary("b") {
  Person(p, "P")
}
"#,
        );

        assert_eq!(model["boundaries"][1]["alias"], json!("b"));
        assert_eq!(model["boundaries"][1]["type"]["text"], json!("container"));
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
  Person(p1, "P1")
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
    fn c4_header_without_statements_is_rejected() {
        let diagnostic = parse_err("C4Context\n");
        assert!(
            diagnostic
                .message()
                .contains("expected at least one C4 statement")
        );
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
    fn c4_missing_relation_target_reports_local_insertion_point() {
        let text = "C4Context\nRel(a)\n";
        let diagnostic = parse_err(text);
        let insert = text.find(')').unwrap();
        assert_eq!(diagnostic.message(), "missing relation target");
        assert_eq!(diagnostic.span(), Some(SourceSpan::new(insert, insert)));
        assert_eq!(
            diagnostic.span_kind(),
            ParseDiagnosticSpanKind::InsertionPoint
        );
    }

    #[test]
    fn c4_missing_relation_style_target_reports_local_insertion_point() {
        let text = "C4Context\nUpdateRelStyle(a)\n";
        let diagnostic = parse_err(text);
        let insert = text.find(')').unwrap();
        assert_eq!(diagnostic.message(), "missing relation style target");
        assert_eq!(diagnostic.span(), Some(SourceSpan::new(insert, insert)));
        assert_eq!(
            diagnostic.span_kind(),
            ParseDiagnosticSpanKind::InsertionPoint
        );
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
        let engine = MermaidEngine::new();
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

        assert_eq!(parsed.metadata().diagram_type, "c4");
        match parsed.model() {
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
    #[cfg(feature = "editor-facts")]
    fn c4_editor_facts_expose_parser_backed_spans() {
        let engine = MermaidEngine::new();
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
            .parse_editor_semantic_facts_with_type_sync("c4", input)
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

        let roles_for = |name: &str| {
            facts
                .symbols
                .iter()
                .filter(|symbol| symbol.name == name)
                .map(|symbol| symbol.role)
                .collect::<Vec<_>>()
        };
        assert_eq!(roles_for("bank"), vec![EditorSemanticRole::Entity]);
        assert_eq!(
            roles_for("customer"),
            vec![
                EditorSemanticRole::Entity,
                EditorSemanticRole::Reference,
                EditorSemanticRole::Reference,
            ]
        );
        assert_eq!(
            roles_for("system"),
            vec![
                EditorSemanticRole::Entity,
                EditorSemanticRole::Reference,
                EditorSemanticRole::Reference,
                EditorSemanticRole::Reference,
            ]
        );

        let system_refs = facts
            .symbols
            .iter()
            .filter(|symbol| {
                symbol.name == "system" && symbol.role == EditorSemanticRole::Reference
            })
            .count();
        assert_eq!(
            system_refs, 3,
            "system relation and style occurrences must remain references"
        );

        let title_start = input.find("Banking Context").unwrap();
        assert!(facts.expected_syntax.iter().any(|expected| {
            expected.kind == EditorExpectedSyntaxKind::Payload
                && expected.span
                    == SourceSpan::new(title_start, title_start + "Banking Context".len())
        }));
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn c4_forward_relation_and_style_targets_remain_typed_references() {
        let input = concat!(
            "C4Context\n",
            "Rel(customer, system, \"Uses\")\n",
            "UpdateElementStyle(system, $bgColor=\"red\")\n",
            "UpdateRelStyle(customer, system, $lineColor=\"blue\")\n",
            "Person(customer, \"Customer\")\n",
            "System(system, \"System\")\n",
        );
        let facts = crate::family::test_support::editor_facts(
            parse_c4_json_and_editor_facts,
            input,
            &meta(),
        );
        let roles_for = |name: &str| {
            facts
                .symbols
                .iter()
                .filter(|symbol| symbol.name == name)
                .map(|symbol| symbol.role)
                .collect::<Vec<_>>()
        };

        assert_eq!(
            roles_for("customer"),
            vec![
                EditorSemanticRole::Reference,
                EditorSemanticRole::Reference,
                EditorSemanticRole::Entity,
            ]
        );
        assert_eq!(
            roles_for("system"),
            vec![
                EditorSemanticRole::Reference,
                EditorSemanticRole::Reference,
                EditorSemanticRole::Reference,
                EditorSemanticRole::Entity,
            ]
        );

        let relation_source = facts
            .symbols
            .iter()
            .find(|symbol| symbol.detail.as_deref() == Some("c4 relation source"))
            .expect("relation source");
        assert_eq!(relation_source.role, EditorSemanticRole::Reference);
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn c4_entity_occurrence_role_does_not_depend_on_display_detail() {
        let span = SourceSpan::new(0, "node".len());
        let value = SpannedText {
            text: "node".to_string(),
            span,
        };
        let mut facts = Some(EditorSemanticFacts::new());
        push_c4_entity_fact(
            &mut facts,
            &value,
            "reference-looking definition",
            C4EntityOccurrence::Definition,
        );
        push_c4_entity_fact(
            &mut facts,
            &value,
            "definition-looking reference",
            C4EntityOccurrence::Reference,
        );

        let facts = facts.unwrap();
        assert_eq!(facts.symbols[0].role, EditorSemanticRole::Entity);
        assert_eq!(facts.symbols[1].role, EditorSemanticRole::Reference);
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn c4_editor_facts_recover_unsupported_statements_without_losing_prior_facts() {
        let engine = MermaidEngine::new();
        let input = "C4Context\nPerson(customer, \"Customer\")\nNotAMacro customer\n";

        let facts = engine
            .parse_editor_semantic_facts_with_type_sync("c4", input)
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

    #[test]
    fn c4_recovery_keeps_later_safe_lines() {
        let source = concat!(
            "C4Context\r\n",
            "Rel(known, )\r\n",
            "Person(后续, \"Later\")\r\n",
            "UpdateLayoutConfig(3, 2)\r\n",
        );
        let error = parse_c4(source, &meta()).expect_err("strict C4 parse must return first error");
        assert!(error.to_string().contains("missing relation target"));

        #[cfg(feature = "editor-facts")]
        {
            let facts = crate::family::test_support::editor_facts(
                parse_c4_json_and_editor_facts,
                source,
                &meta(),
            );
            assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
            assert!(facts.symbols.iter().any(|symbol| symbol.name == "后续"));
            assert!(facts.symbols.iter().any(|symbol| symbol.name == "Later"));
        }
    }

    #[test]
    fn c4_missing_boundary_brace_does_not_consume_the_next_safe_line() {
        let source = concat!(
            "C4Context\n",
            "Boundary(bank, \"Bank\")\n",
            "Person(after, \"After\")\n",
        );
        let error =
            parse_c4(source, &meta()).expect_err("strict parse must require a boundary brace");
        assert!(error.to_string().contains("expected '{' after boundary"));

        #[cfg(feature = "editor-facts")]
        {
            let facts = crate::family::test_support::editor_facts(
                parse_c4_json_and_editor_facts,
                source,
                &meta(),
            );
            assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
            assert!(facts.symbols.iter().any(|symbol| symbol.name == "after"));
        }
    }
}
