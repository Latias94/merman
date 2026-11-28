//! C4 statement scanning and source-ordered replay.

use super::db::{C4Arg, C4Db};
use super::{C4Diagnostic, C4Result};
#[cfg(feature = "editor-facts")]
use super::{C4EntityOccurrence, C4Observation, C4ObservedFailure};
use merman_core_preprocess::scan::LineCursor;
use merman_model::families::c4::{C4DiagramModel, C4RowPreferences};
use merman_model::text::is_ecmascript_whitespace;
use merman_model::{MermaidConfig, OperationControl, OperationControlResult, SourceSpan};

#[cfg(any(test, feature = "test-support"))]
use std::cell::Cell;

#[cfg(any(test, feature = "test-support"))]
thread_local! {
    static C4_SYNTAX_CONSTRUCTION_COUNT: Cell<usize> = const { Cell::new(0) };
}

#[cfg(any(test, feature = "test-support"))]
pub fn reset_c4_syntax_construction_count() {
    C4_SYNTAX_CONSTRUCTION_COUNT.set(0);
}

#[cfg(any(test, feature = "test-support"))]
pub fn c4_syntax_construction_count() -> usize {
    C4_SYNTAX_CONSTRUCTION_COUNT.get()
}

#[cfg(feature = "editor-facts")]
type Observer<'a> = dyn FnMut(C4Observation<'_>) + 'a;

fn leading_whitespace_len(text: &str) -> usize {
    text.len() - text.trim_start().len()
}

/// A source issue also retains the statement fallback span used by editor recovery.
#[derive(Debug)]
pub struct C4SourceIssue {
    pub diagnostic: C4Diagnostic,
    pub recovery_span: SourceSpan,
}

struct C4ParseOutcome {
    db: C4Db,
    issues: Vec<C4SourceIssue>,
}

/// Parses preprocessed C4 source using the caller's configuration and operation control.
pub fn parse_c4_model_controlled(
    code: &str,
    config: &MermaidConfig,
    control: &OperationControl,
) -> OperationControlResult<Result<C4DiagramModel, C4Diagnostic>> {
    Ok(parse_c4_model_with_rows_controlled(code, config, control)?.map(|(model, _)| model))
}

/// Captures authored display preferences during the same parse as the semantic C4 model.
pub fn parse_c4_model_with_rows_controlled(
    code: &str,
    config: &MermaidConfig,
    control: &OperationControl,
) -> OperationControlResult<Result<(C4DiagramModel, C4RowPreferences), C4Diagnostic>> {
    let outcome = construct_c4_source_controlled(
        code,
        config,
        #[cfg(feature = "editor-facts")]
        false,
        #[cfg(feature = "editor-facts")]
        None,
        control,
    )?;
    if let Some(issue) = outcome.issues.into_iter().next() {
        return Ok(Err(issue.diagnostic));
    }
    let rows = outcome.db.row_preferences;
    Ok(outcome.db.into_model().map(|model| (model, rows)))
}

/// Observes C4 source facts during the same parse and returns all ordered source issues on error.
#[cfg(feature = "editor-facts")]
pub fn parse_c4_model_observed_controlled(
    code: &str,
    config: &MermaidConfig,
    capture_rich: bool,
    observe: &mut dyn FnMut(C4Observation<'_>),
    control: &OperationControl,
) -> OperationControlResult<Result<C4DiagramModel, C4ObservedFailure>> {
    let outcome =
        construct_c4_source_controlled(code, config, capture_rich, Some(observe), control)?;
    let mut issues = outcome.issues.into_iter();
    if let Some(first) = issues.next() {
        return Ok(Err(C4ObservedFailure::Source {
            first,
            remaining: issues,
        }));
    }
    Ok(outcome.db.into_model().map_err(C4ObservedFailure::Model))
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
    issues: &mut Vec<C4SourceIssue>,
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
            C4Diagnostic::detected_exact(
                "C4 boundary must start with a diagram statement".to_string(),
                span,
            ),
            span,
        ));
        boundary.is_valid = false;
    }
    false
}

fn is_c4_header(line: &str) -> bool {
    matches!(
        line.trim(),
        "C4Context" | "C4Container" | "C4Component" | "C4Dynamic" | "C4Deployment"
    )
}

#[cfg(feature = "editor-facts")]
fn observe_c4_payload(observe: &mut Observer<'_>, value: &SpannedText, detail: &'static str) {
    observe(C4Observation::Payload {
        text: &value.text,
        span: value.span,
        detail,
        argument_name: None,
    });
}

#[cfg(feature = "editor-facts")]
fn observe_c4_payload_arg(
    observe: &mut Option<&mut Observer<'_>>,
    arg: &SpannedArg,
    detail: &'static str,
) {
    if let Some(observe) = observe {
        observe(C4Observation::Payload {
            text: arg.text(),
            span: arg.span(),
            detail,
            argument_name: arg.key(),
        });
    }
}

#[cfg(feature = "editor-facts")]
fn observe_c4_entity_arg(
    observe: &mut Option<&mut Observer<'_>>,
    arg: &SpannedArg,
    detail: &'static str,
    occurrence: C4EntityOccurrence,
) {
    if let Some(observe) = observe {
        observe(C4Observation::Entity {
            text: arg.text(),
            span: arg.span(),
            detail,
            occurrence,
        });
    }
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

fn observe_c4_direction_statement(
    line: &str,
    #[cfg(feature = "editor-facts")] line_start: usize,
    #[cfg(feature = "editor-facts")] observe: &mut Option<&mut Observer<'_>>,
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
        if let Some(observe) = observe {
            observe(C4Observation::DirectionValue(SourceSpan::new(
                keyword_end,
                keyword_end,
            )));
        }
        return Some(false);
    }

    let token = value.split_whitespace().next().unwrap_or(value);
    #[cfg(feature = "editor-facts")]
    let value_start = keyword_end + whitespace;
    #[cfg(feature = "editor-facts")]
    let value_span = SourceSpan::new(value_start, value_start + token.len());
    #[cfg(feature = "editor-facts")]
    if let Some(observe) = observe {
        observe(C4Observation::DirectionValue(value_span));
    }
    Some(matches!(token, "TB" | "BT" | "LR" | "RL"))
}

fn validate_and_observe_c4_macro(
    stmt: &SpannedMacroStmt,
    #[cfg(feature = "editor-facts")] observe: &mut Option<&mut Observer<'_>>,
) -> C4Result<()> {
    match stmt.name.as_str() {
        "Person" | "Person_Ext" | "System" | "SystemDb" | "SystemQueue" | "System_Ext"
        | "SystemDb_Ext" | "SystemQueue_Ext" => {
            #[cfg(feature = "editor-facts")]
            if let Some(alias) = stmt.args.first() {
                observe_c4_entity_arg(
                    observe,
                    alias,
                    c4_shape_detail(&stmt.name),
                    C4EntityOccurrence::Definition,
                );
            }
            #[cfg(feature = "editor-facts")]
            if let Some(label) = stmt.args.get(1) {
                observe_c4_payload_arg(observe, label, "c4 label");
            }
            #[cfg(feature = "editor-facts")]
            if let Some(descr) = stmt.args.get(2) {
                observe_c4_payload_arg(observe, descr, "c4 description");
            }
            #[cfg(feature = "editor-facts")]
            for arg in stmt.args.iter().skip(3) {
                observe_c4_payload_arg(observe, arg, "c4 value");
            }
            Ok(())
        }
        "Container" | "ContainerDb" | "ContainerQueue" | "Container_Ext" | "ContainerDb_Ext"
        | "ContainerQueue_Ext" | "Component" | "ComponentDb" | "ComponentQueue"
        | "Component_Ext" | "ComponentDb_Ext" | "ComponentQueue_Ext" => {
            #[cfg(feature = "editor-facts")]
            if let Some(alias) = stmt.args.first() {
                observe_c4_entity_arg(
                    observe,
                    alias,
                    c4_shape_detail(&stmt.name),
                    C4EntityOccurrence::Definition,
                );
            }
            #[cfg(feature = "editor-facts")]
            if let Some(label) = stmt.args.get(1) {
                observe_c4_payload_arg(observe, label, "c4 label");
            }
            #[cfg(feature = "editor-facts")]
            if let Some(techn) = stmt.args.get(2) {
                observe_c4_payload_arg(observe, techn, "c4 technology");
            }
            #[cfg(feature = "editor-facts")]
            if let Some(descr) = stmt.args.get(3) {
                observe_c4_payload_arg(observe, descr, "c4 description");
            }
            #[cfg(feature = "editor-facts")]
            for arg in stmt.args.iter().skip(4) {
                observe_c4_payload_arg(observe, arg, "c4 value");
            }
            Ok(())
        }
        "Boundary" | "Enterprise_Boundary" | "System_Boundary" | "Container_Boundary" => {
            #[cfg(feature = "editor-facts")]
            if let Some(alias) = stmt.args.first() {
                observe_c4_entity_arg(
                    observe,
                    alias,
                    "c4 boundary",
                    C4EntityOccurrence::Definition,
                );
            }
            #[cfg(feature = "editor-facts")]
            if let Some(label) = stmt.args.get(1) {
                observe_c4_payload_arg(observe, label, "c4 label");
            }
            #[cfg(feature = "editor-facts")]
            if let Some(boundary_type) = stmt.args.get(2) {
                observe_c4_payload_arg(observe, boundary_type, "c4 boundary type");
            }
            #[cfg(feature = "editor-facts")]
            for arg in stmt.args.iter().skip(3) {
                observe_c4_payload_arg(observe, arg, "c4 value");
            }
            Ok(())
        }
        "Node" | "Deployment_Node" | "Node_L" | "Node_R" => {
            #[cfg(feature = "editor-facts")]
            if let Some(alias) = stmt.args.first() {
                observe_c4_entity_arg(
                    observe,
                    alias,
                    "c4 deployment node",
                    C4EntityOccurrence::Definition,
                );
            }
            #[cfg(feature = "editor-facts")]
            if let Some(label) = stmt.args.get(1) {
                observe_c4_payload_arg(observe, label, "c4 label");
            }
            #[cfg(feature = "editor-facts")]
            if let Some(node_type) = stmt.args.get(2) {
                observe_c4_payload_arg(observe, node_type, "c4 node type");
            }
            #[cfg(feature = "editor-facts")]
            if let Some(descr) = stmt.args.get(3) {
                observe_c4_payload_arg(observe, descr, "c4 description");
            }
            #[cfg(feature = "editor-facts")]
            for arg in stmt.args.iter().skip(4) {
                observe_c4_payload_arg(observe, arg, "c4 value");
            }
            Ok(())
        }
        "Rel" | "BiRel" | "Rel_U" | "Rel_Up" | "Rel_D" | "Rel_Down" | "Rel_L" | "Rel_Left"
        | "Rel_R" | "Rel_Right" | "Rel_Back" => {
            let Some(_from) = stmt.args.first() else {
                return Err(C4Diagnostic::fallback(
                    "missing relation source".to_string(),
                ));
            };
            #[cfg(feature = "editor-facts")]
            observe_c4_entity_arg(
                observe,
                _from,
                "c4 relation source",
                C4EntityOccurrence::Reference,
            );
            let Some(_to) = stmt.args.get(1) else {
                return Err(C4Diagnostic::fallback(
                    "missing relation target".to_string(),
                ));
            };
            #[cfg(feature = "editor-facts")]
            observe_c4_entity_arg(
                observe,
                _to,
                "c4 relation target",
                C4EntityOccurrence::Reference,
            );
            #[cfg(feature = "editor-facts")]
            if let Some(label) = stmt.args.get(2) {
                observe_c4_payload_arg(observe, label, "c4 relation label");
            } else {
                return Ok(());
            }
            #[cfg(feature = "editor-facts")]
            if let Some(techn) = stmt.args.get(3) {
                observe_c4_payload_arg(observe, techn, "c4 technology");
            }
            #[cfg(feature = "editor-facts")]
            if let Some(descr) = stmt.args.get(4) {
                observe_c4_payload_arg(observe, descr, "c4 description");
            }
            #[cfg(feature = "editor-facts")]
            for arg in stmt.args.iter().skip(5) {
                observe_c4_payload_arg(observe, arg, "c4 value");
            }
            Ok(())
        }
        "RelIndex" => {
            let Some(_index) = stmt.args.first() else {
                return Err(C4Diagnostic::fallback("missing relation index".to_string()));
            };
            #[cfg(feature = "editor-facts")]
            observe_c4_payload_arg(observe, _index, "c4 relation index");
            let Some(_from) = stmt.args.get(1) else {
                return Err(C4Diagnostic::fallback(
                    "missing relation source".to_string(),
                ));
            };
            #[cfg(feature = "editor-facts")]
            observe_c4_entity_arg(
                observe,
                _from,
                "c4 relation source",
                C4EntityOccurrence::Reference,
            );
            let Some(_to) = stmt.args.get(2) else {
                return Err(C4Diagnostic::fallback(
                    "missing relation target".to_string(),
                ));
            };
            #[cfg(feature = "editor-facts")]
            observe_c4_entity_arg(
                observe,
                _to,
                "c4 relation target",
                C4EntityOccurrence::Reference,
            );
            #[cfg(feature = "editor-facts")]
            if let Some(label) = stmt.args.get(3) {
                observe_c4_payload_arg(observe, label, "c4 relation label");
            } else {
                return Ok(());
            }
            #[cfg(feature = "editor-facts")]
            if let Some(techn) = stmt.args.get(4) {
                observe_c4_payload_arg(observe, techn, "c4 technology");
            }
            #[cfg(feature = "editor-facts")]
            if let Some(descr) = stmt.args.get(5) {
                observe_c4_payload_arg(observe, descr, "c4 description");
            }
            #[cfg(feature = "editor-facts")]
            for arg in stmt.args.iter().skip(6) {
                observe_c4_payload_arg(observe, arg, "c4 value");
            }
            Ok(())
        }
        "UpdateElementStyle" => {
            let Some(_target) = stmt.args.first() else {
                return Err(C4Diagnostic::fallback("missing style target".to_string()));
            };
            #[cfg(feature = "editor-facts")]
            observe_c4_entity_arg(
                observe,
                _target,
                "c4 style target",
                C4EntityOccurrence::Reference,
            );
            #[cfg(feature = "editor-facts")]
            for arg in stmt.args.iter().skip(1) {
                observe_c4_payload_arg(observe, arg, "c4 style value");
            }
            Ok(())
        }
        "UpdateRelStyle" => {
            let Some(_from) = stmt.args.first() else {
                return Err(C4Diagnostic::fallback(
                    "missing relation style source".to_string(),
                ));
            };
            #[cfg(feature = "editor-facts")]
            observe_c4_entity_arg(
                observe,
                _from,
                "c4 relation style source",
                C4EntityOccurrence::Reference,
            );
            let Some(_to) = stmt.args.get(1) else {
                return Err(C4Diagnostic::fallback(
                    "missing relation style target".to_string(),
                ));
            };
            #[cfg(feature = "editor-facts")]
            observe_c4_entity_arg(
                observe,
                _to,
                "c4 relation style target",
                C4EntityOccurrence::Reference,
            );
            #[cfg(feature = "editor-facts")]
            for arg in stmt.args.iter().skip(2) {
                observe_c4_payload_arg(observe, arg, "c4 relation style value");
            }
            Ok(())
        }
        "UpdateLayoutConfig" => {
            #[cfg(feature = "editor-facts")]
            for arg in &stmt.args {
                observe_c4_payload_arg(observe, arg, "c4 layout value");
            }
            Ok(())
        }
        other => Err(C4Diagnostic::fallback(format!(
            "unsupported C4 macro: {other}"
        ))),
    }
}

fn semantic_c4_args(args: &[SpannedArg]) -> Vec<C4Arg> {
    args.iter().map(C4Arg::from_spanned).collect()
}

fn c4_missing_arg(stmt: &SpannedMacroStmt, message: &'static str) -> C4Diagnostic {
    let offset = stmt
        .args
        .last()
        .map(|arg| arg.span().end)
        .unwrap_or(stmt.args_span.start);
    C4Diagnostic::insertion_point(message.to_string(), offset)
}

fn validate_c4_macro_args(stmt: &SpannedMacroStmt) -> C4Result<()> {
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

fn c4_macro_statement_source<'a>(
    code: &'a str,
    stmt_start: usize,
    control: &OperationControl,
) -> OperationControlResult<&'a str> {
    let source = &code[stmt_start..];
    let bytes = source.as_bytes();
    let mut in_quotes = false;
    for (index, &byte) in bytes.iter().enumerate() {
        if index > 0 && index.is_multiple_of(1024) {
            control.checkpoint()?;
        }
        match byte {
            b'"' => in_quotes = !in_quotes,
            b'\n' if in_quotes => control.checkpoint()?,
            b'\n' => return Ok(&source[..index]),
            b'%' if !in_quotes && bytes.get(index + 1) == Some(&b'%') => {
                return Ok(&source[..index]);
            }
            _ => {}
        }
    }
    Ok(source)
}

fn parse_macro_stmt_spanned(t: &str, stmt_start: usize) -> C4Result<Option<SpannedMacroStmt>> {
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
        return Err(C4Diagnostic::fallback(format!(
            "unterminated macro call: {t}"
        )));
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
            return Err(C4Diagnostic::fallback(format!(
                "unexpected tokens after '{{' in macro: {t}"
            )));
        }
    } else if !rest.is_empty() {
        return Err(C4Diagnostic::fallback(format!(
            "unexpected trailing tokens in macro: {t}"
        )));
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

fn parse_args_csv_spanned(input: &str, base_offset: usize) -> C4Result<Vec<SpannedArg>> {
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

fn parse_arg_spanned(seg: &str, seg_base: usize) -> C4Result<SpannedArg> {
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

    if trimmed.contains('\n') {
        return Err(C4Diagnostic::fallback(
            "unexpected newline in unquoted C4 argument".to_string(),
        ));
    }

    Ok(SpannedArg {
        value: SpannedArgValue::Text(SpannedText {
            text: trimmed.to_string(),
            span: SourceSpan::new(value_base, value_base + trimmed.len()),
        }),
    })
}

fn try_parse_kv_spanned(seg: &str, seg_base: usize) -> C4Result<Option<SpannedArg>> {
    if !seg.starts_with('$') {
        return Ok(None);
    }
    let rest = &seg[1..];
    let Some(eq) = rest.find('=') else {
        return Err(C4Diagnostic::fallback(format!(
            "invalid attribute kv: {seg}"
        )));
    };
    let key_source = spanned_trimmed_c4(&rest[..eq], seg_base + 1);
    let key = key_source.text.as_str();
    if key.is_empty() || rest[..eq].contains('\n') {
        return Err(C4Diagnostic::fallback(format!(
            "invalid attribute kv key: {seg}"
        )));
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

fn parse_quoted_spanned(input: &str, input_base: usize) -> C4Result<SpannedText> {
    let input = input.trim();
    let Some(rest) = input.strip_prefix('"') else {
        return Err(C4Diagnostic::fallback(format!(
            "expected quoted string, got: {input}"
        )));
    };
    let Some(end) = rest.find('"') else {
        return Err(C4Diagnostic::fallback("unterminated string".to_string()));
    };
    let value = &rest[..end];
    let value_span = SourceSpan::new(input_base + 1, input_base + 1 + value.len());
    let closing_quote = value_span.end;
    let trailing_raw = &rest[end + 1..];
    let trailing = spanned_trimmed_c4(trailing_raw, closing_quote + 1);
    if !trailing.text.is_empty() {
        return Err(C4Diagnostic::fallback(format!(
            "unexpected trailing tokens after string: {}",
            trailing.text
        )));
    }
    Ok(SpannedText {
        // The grammar trims STR and STR_VALUE, but editor ranges still own the authored body.
        text: value.trim_matches(is_ecmascript_whitespace).to_string(),
        span: value_span,
    })
}

fn construct_c4_source_controlled(
    code: &str,
    config: &MermaidConfig,
    #[cfg(feature = "editor-facts")] capture_rich: bool,
    #[cfg(feature = "editor-facts")] observe: Option<&mut Observer<'_>>,
    control: &OperationControl,
) -> OperationControlResult<C4ParseOutcome> {
    control.checkpoint()?;
    #[cfg(any(test, feature = "test-support"))]
    C4_SYNTAX_CONSTRUCTION_COUNT.set(C4_SYNTAX_CONSTRUCTION_COUNT.get() + 1);
    parse_c4_semantic_source(
        code,
        config,
        #[cfg(feature = "editor-facts")]
        capture_rich,
        #[cfg(feature = "editor-facts")]
        observe,
        control,
    )
}

fn parse_c4_semantic_source(
    code: &str,
    config: &MermaidConfig,
    #[cfg(feature = "editor-facts")] capture_rich: bool,
    #[cfg(feature = "editor-facts")] mut observer: Option<&mut Observer<'_>>,
    control: &OperationControl,
) -> OperationControlResult<C4ParseOutcome> {
    control.checkpoint()?;
    let mut db = C4Db::new(config);
    #[cfg(feature = "editor-facts")]
    let mut observe = {
        if let Some(observer) = observer.as_deref_mut() {
            observer(C4Observation::CaptureStarted);
        }
        observer.as_deref_mut().filter(|_| capture_rich)
    };
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
                C4Diagnostic::detected_exact(format!("unexpected C4 header: {header}"), span),
                span,
            ));
        }
        break;
    }
    if !saw_header_line {
        let span = SourceSpan::new(code.len(), code.len());
        issues.push(c4_parse_issue(
            C4Diagnostic::detected_insertion_point("expected C4 header", span.start),
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
                C4Diagnostic::detected_insertion_point(
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
                    C4Diagnostic::detected_exact(
                        "unexpected '}' without open C4 boundary".to_string(),
                        closing_span,
                    ),
                    closing_span,
                ));
                continue;
            };
            if !frame.has_diagram_statement {
                issues.push(c4_parse_issue(
                    C4Diagnostic::detected_exact(
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
            if c4_other_statement_is_allowed(&mut boundary_frames, &mut issues, title.span) {
                push_c4_semantic_statement(
                    &mut semantic_statements,
                    &mut boundary_frames,
                    &mut boundary_records,
                    C4SemanticStatement::SetTitle(title.text.clone()),
                    false,
                );
            }
            #[cfg(feature = "editor-facts")]
            if let Some(observe) = observe.as_deref_mut() {
                observe(C4Observation::DirectivePrefix("title"));
                observe_c4_payload(observe, &title, "c4 title");
            }
            continue;
        }

        if let Some(acc_title) = parse_acc_title_spanned_c4(raw, line_start) {
            // Mermaid's C4 grammar maps accTitle to the diagram title.
            if c4_other_statement_is_allowed(&mut boundary_frames, &mut issues, acc_title.span) {
                push_c4_semantic_statement(
                    &mut semantic_statements,
                    &mut boundary_frames,
                    &mut boundary_records,
                    C4SemanticStatement::SetTitle(acc_title.text.clone()),
                    false,
                );
            }
            #[cfg(feature = "editor-facts")]
            if let Some(observe) = observe.as_deref_mut() {
                observe(C4Observation::DirectivePrefix("accTitle"));
                observe_c4_payload(observe, &acc_title, "c4 accessibility title");
            }
            continue;
        }

        if let Some(acc_description) = parse_acc_description_stmt_spanned_c4(raw, line_start) {
            if c4_other_statement_is_allowed(
                &mut boundary_frames,
                &mut issues,
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
            if let Some(observe) = observe.as_deref_mut() {
                observe(C4Observation::DirectivePrefix("accDescription"));
                observe_c4_payload(observe, &acc_description, "c4 accessibility description");
            }
            continue;
        }

        if let Some(acc_descr) = parse_acc_descr_spanned_c4(&mut lines, raw, line_start, control)? {
            if c4_other_statement_is_allowed(
                &mut boundary_frames,
                &mut issues,
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
            if let Some(observe) = observe.as_deref_mut() {
                observe(C4Observation::DirectivePrefix("accDescr"));
                observe_c4_payload(observe, &acc_descr.value, "c4 accessibility description");
            }
            if !acc_descr.closed {
                let error = C4Diagnostic::detected_insertion_point(
                    "unterminated C4 accDescr block",
                    code.len(),
                );
                issues.push(c4_parse_issue(error, acc_descr.value.span));
            }
            continue;
        }

        let stmt_start = statement_start;
        let statement_span = SourceSpan::new(stmt_start, stmt_start + t.len());
        if let Some(_valid) = observe_c4_direction_statement(
            raw,
            #[cfg(feature = "editor-facts")]
            line_start,
            #[cfg(feature = "editor-facts")]
            &mut observe,
        ) {
            issues.push(c4_parse_issue(
                C4Diagnostic::detected_exact(
                    format!("unsupported C4 statement: {t}"),
                    statement_span,
                ),
                statement_span,
            ));
            continue;
        }
        // Only quoted macro tokens can carry a physical LF into the next line. Borrow the
        // original span so comments, CRLF and UTF-8 offsets survive without reconstruction.
        let macro_source = if t.contains('(') {
            c4_macro_statement_source(code, stmt_start, control)?
        } else {
            t
        };
        let stmt = match parse_macro_stmt_spanned(macro_source, stmt_start) {
            Ok(Some(stmt)) => stmt,
            Ok(None) => {
                issues.push(c4_parse_issue(
                    C4Diagnostic::detected_exact(
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
        // Commit lookahead only after syntax succeeds; malformed strings must not consume
        // later safe lines that recovery can still parse independently.
        while lines.offset() < stmt.span.end {
            if lines.next_line().is_none() {
                break;
            }
            control.checkpoint()?;
        }
        if let Err(error) = validate_c4_macro_args(&stmt) {
            issues.push(c4_parse_issue(error, stmt.span));
            continue;
        }
        if let Err(error) = validate_and_observe_c4_macro(
            &stmt,
            #[cfg(feature = "editor-facts")]
            &mut observe,
        ) {
            issues.push(c4_parse_issue(error, stmt.span));
            continue;
        }
        if stmt.has_lbrace && !is_boundary_macro(&stmt.name) {
            let lbrace = SourceSpan::new(stmt.span.end.saturating_sub(1), stmt.span.end);
            issues.push(c4_parse_issue(
                C4Diagnostic::detected_exact(
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
            C4Diagnostic::detected_insertion_point(
                "expected '{' after boundary",
                declaration.span.end,
            ),
            SourceSpan::new(declaration.span.end, declaration.span.end),
        ));
    }

    while boundary_frames.pop().is_some() {
        let eof = SourceSpan::new(code.len(), code.len());
        issues.push(c4_parse_issue(
            C4Diagnostic::detected_insertion_point(
                "expected '}' before end of C4 diagram",
                code.len(),
            ),
            eof,
        ));
    }

    if saw_header_line && !saw_statement_after_header {
        let eof = SourceSpan::new(code.len(), code.len());
        issues.push(c4_parse_issue(
            C4Diagnostic::detected_insertion_point(
                "expected at least one C4 statement",
                code.len(),
            ),
            eof,
        ));
    }

    #[cfg(feature = "editor-facts")]
    let observe_overwrites = observer.filter(|_| db.is_dynamic());
    apply_c4_semantic_statements(
        &mut db,
        &semantic_statements,
        &boundary_records,
        config,
        &mut issues,
        #[cfg(feature = "editor-facts")]
        observe_overwrites,
        control,
    )?;
    issues.sort_by_key(|issue| issue.recovery_span.start);

    control.checkpoint()?;
    Ok(C4ParseOutcome { db, issues })
}

fn apply_c4_semantic_statements(
    db: &mut C4Db,
    statements: &[C4SemanticStatement],
    boundaries: &[C4BoundaryRecord],
    config: &MermaidConfig,
    issues: &mut Vec<C4SourceIssue>,
    #[cfg(feature = "editor-facts")] mut observe_overwrites: Option<&mut Observer<'_>>,
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
                if let Err(error) = apply_c4_macro(
                    db,
                    statement,
                    &parent_boundary,
                    #[cfg(feature = "editor-facts")]
                    observe_overwrites.as_deref_mut(),
                ) {
                    issues.push(c4_parse_issue(error, statement.span));
                }
            }
            C4SemanticStatement::Boundary(boundary) => {
                let C4BoundaryRecord {
                    declaration,
                    statements,
                } = &boundaries[boundary.0];
                match apply_c4_macro(
                    db,
                    declaration,
                    &parent_boundary,
                    #[cfg(feature = "editor-facts")]
                    None,
                ) {
                    Ok(Some(alias)) => {
                        frames.push(ReplayFrame {
                            statements,
                            next_statement: 0,
                            parent_boundary: alias,
                        });
                    }
                    Ok(None) => issues.push(c4_parse_issue(
                        C4Diagnostic::detected_exact(
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
    parent_boundary: &str,
    #[cfg(feature = "editor-facts")] observe_overwrites: Option<&mut Observer<'_>>,
) -> C4Result<Option<String>> {
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
                return Err(C4Diagnostic::detected_fallback(format!(
                    "unsupported boundary macro: {other}"
                )));
            }
        };

        return Ok(Some(alias));
    }

    let add_rel = |db: &mut C4Db, rel_type: &str, args: &[C4Arg]| -> C4Result<()> {
        let overwritten = db.add_rel(rel_type, args)?;
        #[cfg(feature = "editor-facts")]
        if overwritten && let Some(observe) = observe_overwrites {
            observe(C4Observation::RelationOverwritten {
                from: args[0].value(),
                to: args[1].value(),
                span: stmt.span,
            });
        }
        #[cfg(not(feature = "editor-facts"))]
        let _ = overwritten;
        Ok(())
    };

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
        "Rel" => add_rel(db, "rel", &args)?,
        "BiRel" => add_rel(db, "birel", &args)?,
        "Rel_U" | "Rel_Up" => add_rel(db, "rel_u", &args)?,
        "Rel_D" | "Rel_Down" => add_rel(db, "rel_d", &args)?,
        "Rel_L" | "Rel_Left" => add_rel(db, "rel_l", &args)?,
        "Rel_R" | "Rel_Right" => add_rel(db, "rel_r", &args)?,
        "Rel_Back" => add_rel(db, "rel_b", &args)?,
        "RelIndex" => add_rel(db, "rel", &args[1..])?,
        "UpdateElementStyle" => db.update_el_style(&args)?,
        "UpdateRelStyle" => db.update_rel_style(&args)?,
        "UpdateLayoutConfig" => db.update_layout_config(&args)?,
        other => {
            return Err(C4Diagnostic::detected_fallback(format!(
                "unsupported C4 macro: {other}"
            )));
        }
    }
    Ok(None)
}

fn c4_parse_issue(diagnostic: C4Diagnostic, fallback: SourceSpan) -> C4SourceIssue {
    C4SourceIssue {
        recovery_span: diagnostic.span().unwrap_or(fallback),
        diagnostic,
    }
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
mod tests;
