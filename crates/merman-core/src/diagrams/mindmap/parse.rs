use super::MindmapDiagramModel;
#[cfg(feature = "presentation")]
use super::MindmapDiagramRenderModel;
use super::db::MindmapDb;
use super::utils::{
    NodeSpec, NodeSpecContinuation, is_statement_terminator, node_closing_delimiter_at_start,
    parse_node_spec, starts_node_spec, strip_inline_comment,
};
#[cfg(feature = "presentation")]
use crate::SourcePresentationMetadata;
use crate::diagrams::scan::{split_indent, starts_with_case_insensitive};
#[cfg(feature = "editor-facts")]
use crate::family::EditorFactsDemand;
#[cfg(feature = "editor-facts")]
use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
    EditorSemanticSymbol,
};
use crate::{Error, Result, SourceSpan, family::CombinedSemanticFailure};
#[cfg(feature = "compat-json")]
use serde_json::Value;
#[cfg(test)]
use std::cell::Cell;

#[cfg(test)]
thread_local! { static MINDMAP_SYNTAX_CONSTRUCTION_COUNT: Cell<usize> = const { Cell::new(0) }; }
#[cfg(test)]
pub(crate) fn reset_mindmap_syntax_construction_count() {
    MINDMAP_SYNTAX_CONSTRUCTION_COUNT.set(0);
}
#[cfg(test)]
pub(crate) fn mindmap_syntax_construction_count() -> usize {
    MINDMAP_SYNTAX_CONSTRUCTION_COUNT.get()
}

struct MindmapSemanticSource {
    model: MindmapDiagramModel,
    #[cfg(feature = "editor-facts")]
    editor_facts: Option<EditorSemanticFacts>,
}

pub(crate) fn parse_mindmap_semantic_model_controlled(
    code: &str,
    metadata: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<crate::family::SemanticConstruction<MindmapDiagramModel>> {
    let construction = construct_mindmap_semantic_source_controlled(
        code,
        &metadata.diagram_type,
        &metadata.semantic_config,
        #[cfg(feature = "editor-facts")]
        editor_demand,
        control,
    )?;
    Ok(match construction {
        Ok(source) => crate::family::SemanticConstruction {
            model: Ok(source.model),
            #[cfg(feature = "editor-facts")]
            editor_facts: source.editor_facts,
        },
        Err(failure) => {
            #[cfg(feature = "editor-facts")]
            if editor_demand.needs_recovery() {
                let (error, facts) = failure.into_parts();
                return Ok(crate::family::SemanticConstruction {
                    model: Err(error),
                    editor_facts: Some(facts),
                });
            }
            crate::family::SemanticConstruction {
                model: Err(failure.into_error()),
                #[cfg(feature = "editor-facts")]
                editor_facts: None,
            }
        }
    })
}

#[cfg(feature = "compat-json")]
pub(crate) fn parse_mindmap(code: &str, meta: &SourcePresentationMetadata) -> Result<Value> {
    let model =
        parse_mindmap_model_for_render_controlled(code, meta, &crate::OperationControl::new())
            .expect("a private parse control cannot be cancelled")?;
    super::render_model_to_compat_json(&model, meta)
}

#[cfg(all(feature = "compat-json", feature = "editor-facts"))]
pub(crate) fn parse_mindmap_json_and_editor_facts(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<crate::family::CombinedSemanticParse> {
    control.checkpoint()?;
    let construction = match construct_mindmap_semantic_source_controlled(
        code,
        &meta.diagram_type,
        meta.effective_config.as_config(),
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Capture,
        control,
    )? {
        Ok(source) => {
            let model = match super::render_model::model_into_render_model_controlled(
                source.model,
                meta,
                control,
            )? {
                Ok(model) => super::render_model::render_model_to_compat_json_controlled(
                    &model, meta, control,
                )?,
                Err(error) => Err(error),
            };
            Ok((model, source.editor_facts.unwrap_or_default()))
        }
        Err(error) => Err(error),
    };
    let parsed = crate::family::CombinedSemanticParse::from_construction(
        construction,
        |parts| parts,
        CombinedSemanticFailure::into_parts,
    );
    control.checkpoint()?;
    Ok(parsed)
}

#[cfg(all(test, feature = "presentation"))]
pub(crate) fn parse_mindmap_model_for_render(
    code: &str,
    meta: &SourcePresentationMetadata,
) -> Result<MindmapDiagramRenderModel> {
    parse_mindmap_model_for_render_controlled(code, meta, &crate::OperationControl::new())
        .expect("a private parse control cannot be cancelled")
}

#[cfg(feature = "presentation")]
pub(crate) fn parse_mindmap_model_for_render_controlled(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<Result<MindmapDiagramRenderModel>> {
    match construct_mindmap_semantic_source_controlled(
        code,
        &meta.diagram_type,
        meta.effective_config.as_config(),
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        control,
    )? {
        Ok(source) => {
            super::render_model::model_into_render_model_controlled(source.model, meta, control)
        }
        Err(error) => Ok(Err(error.into_error())),
    }
}

fn mindmap_failure(
    error: Error,
    #[cfg(feature = "editor-facts")] facts: Option<EditorSemanticFacts>,
) -> CombinedSemanticFailure {
    CombinedSemanticFailure::parser_recovery(
        #[cfg(feature = "editor-facts")]
        "mindmap",
        error,
        #[cfg(feature = "editor-facts")]
        facts,
    )
}

fn construct_mindmap_semantic_source_controlled(
    code: &str,
    diagram_type: &str,
    config: &crate::MermaidConfig,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<
    std::result::Result<MindmapSemanticSource, CombinedSemanticFailure>,
> {
    control.checkpoint()?;
    #[cfg(test)]
    MINDMAP_SYNTAX_CONSTRUCTION_COUNT.set(MINDMAP_SYNTAX_CONSTRUCTION_COUNT.get() + 1);
    let MindmapParseOutcome {
        parsed,
        first_error,
    } = parse_mindmap_lines(
        code,
        diagram_type,
        #[cfg(feature = "editor-facts")]
        editor_demand,
        control,
    )?;
    control.checkpoint()?;
    #[cfg(feature = "editor-facts")]
    let editor_facts = match editor_demand {
        EditorFactsDemand::Omit => None,
        EditorFactsDemand::Recovery => Some(EditorSemanticFacts::new()),
        EditorFactsDemand::Capture => Some(mindmap_editor_facts_from_parsed(&parsed, control)?),
    };
    if let Some(error) = first_error {
        return Ok(Err(mindmap_failure(
            error,
            #[cfg(feature = "editor-facts")]
            editor_facts,
        )));
    }
    let db = match mindmap_db_from_events(parsed.events, diagram_type, config, control)? {
        Ok(db) => db,
        Err(error) => {
            return Ok(Err(mindmap_failure(
                error,
                #[cfg(feature = "editor-facts")]
                editor_facts,
            )));
        }
    };
    control.checkpoint()?;
    Ok(Ok(MindmapSemanticSource {
        model: db.into_semantic_model(),
        #[cfg(feature = "editor-facts")]
        editor_facts,
    }))
}

fn mindmap_db_from_events(
    events: Vec<MindmapParsedEvent>,
    diagram_type: &str,
    config: &crate::MermaidConfig,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<Result<MindmapDb>> {
    let mut db = MindmapDb::default();
    db.clear();

    for (index, event) in events.into_iter().enumerate() {
        if index % 128 == 0 {
            control.checkpoint()?;
        }
        match event {
            MindmapParsedEvent::Node(node) => {
                let selection = node.selection;
                if let Err(error) = db.add_node_controlled(
                    super::db::MindmapNodeInput {
                        indent_level: node.indent as i32,
                        id_raw: &node.id_raw,
                        descr_raw: &node.descr_raw,
                        descr_is_markdown: node.descr_is_markdown,
                        ty: node.ty,
                        diagram_type,
                    },
                    config,
                    control,
                )? {
                    return Ok(Err(error.with_exact_span_if_missing(selection)));
                }
            }
            MindmapParsedEvent::Class(class) => {
                db.decorate_last(Some(class.value), None, config);
            }
            MindmapParsedEvent::Icon(icon) => {
                db.decorate_last(None, Some(icon.value), config);
            }
        }
    }

    control.checkpoint()?;
    Ok(Ok(db))
}

#[derive(Debug, Clone)]
struct MindmapParsedNodeLine {
    indent: usize,
    id_raw: String,
    descr_raw: String,
    descr_is_markdown: bool,
    ty: i32,
    #[cfg(feature = "editor-facts")]
    span: SourceSpan,
    selection: SourceSpan,
    #[cfg(feature = "editor-facts")]
    payload_span: Option<SourceSpan>,
}

#[derive(Debug, Clone)]
struct MindmapParsedPayloadLine {
    value: String,
    #[cfg(feature = "editor-facts")]
    span: SourceSpan,
    #[cfg(feature = "editor-facts")]
    tokens: Vec<MindmapParsedPayloadToken>,
}

#[cfg(feature = "editor-facts")]
#[derive(Debug, Clone)]
struct MindmapParsedPayloadToken {
    value: String,
    selection: SourceSpan,
}

#[derive(Debug, Clone)]
enum MindmapParsedEvent {
    Node(MindmapParsedNodeLine),
    Class(MindmapParsedPayloadLine),
    Icon(MindmapParsedPayloadLine),
}

#[derive(Debug, Default)]
struct MindmapParsedLines {
    events: Vec<MindmapParsedEvent>,
    #[cfg(feature = "editor-facts")]
    directive_prefixes: Vec<String>,
    #[cfg(feature = "editor-facts")]
    capture_editor: bool,
}

struct MindmapParseOutcome {
    parsed: MindmapParsedLines,
    first_error: Option<Error>,
}

#[cfg(feature = "editor-facts")]
fn mindmap_editor_facts_from_parsed(
    parsed: &MindmapParsedLines,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<EditorSemanticFacts> {
    #[cfg(test)]
    crate::editor::test_work::record_collection();
    let mut facts = EditorSemanticFacts::new();
    for (index, prefix) in parsed.directive_prefixes.iter().enumerate() {
        if index % 128 == 0 {
            control.checkpoint()?;
        }
        facts.push_directive_prefix(prefix.clone());
    }
    for (index, event) in parsed.events.iter().enumerate() {
        if index % 128 == 0 {
            control.checkpoint()?;
        }
        match event {
            MindmapParsedEvent::Node(node) => {
                facts.push_expected_syntax(EditorExpectedSyntax::new(
                    EditorExpectedSyntaxKind::NodeIdentifier,
                    node.selection,
                ));
                if let Some(payload_span) = node.payload_span {
                    facts.push_expected_syntax(EditorExpectedSyntax::new(
                        EditorExpectedSyntaxKind::Payload,
                        #[cfg(feature = "editor-facts")]
                        payload_span,
                    ));
                    facts.push_symbol(EditorSemanticSymbol::payload(
                        node.descr_raw.clone(),
                        Some("mindmap node label".to_string()),
                        EditorSemanticKind::String,
                        #[cfg(feature = "editor-facts")]
                        payload_span,
                        #[cfg(feature = "editor-facts")]
                        payload_span,
                    ));
                }
                facts.push_symbol(EditorSemanticSymbol::new(
                    node.id_raw.clone(),
                    Some("mindmap node".to_string()),
                    EditorSemanticKind::Namespace,
                    node.span,
                    node.selection,
                ));
            }
            MindmapParsedEvent::Class(class) => {
                for (index, token) in class.tokens.iter().enumerate() {
                    if index % 128 == 0 {
                        control.checkpoint()?;
                    }
                    facts.push_symbol(EditorSemanticSymbol::payload(
                        token.value.clone(),
                        Some("mindmap class".to_string()),
                        EditorSemanticKind::Property,
                        class.span,
                        token.selection,
                    ));
                }
            }
            MindmapParsedEvent::Icon(icon) => {
                for (index, token) in icon.tokens.iter().enumerate() {
                    if index % 128 == 0 {
                        control.checkpoint()?;
                    }
                    facts.push_symbol(EditorSemanticSymbol::payload(
                        token.value.clone(),
                        Some("mindmap icon".to_string()),
                        EditorSemanticKind::String,
                        icon.span,
                        token.selection,
                    ));
                }
            }
        }
    }
    control.checkpoint()?;
    Ok(facts)
}

fn absolute_mindmap_span(base: usize, span: SourceSpan) -> SourceSpan {
    SourceSpan::new(base + span.start, base + span.end)
}

#[cfg(feature = "editor-facts")]
fn mindmap_payload_tokens(raw: &str, raw_start: usize) -> Vec<MindmapParsedPayloadToken> {
    let mut tokens = Vec::new();
    let mut cursor = 0usize;
    while cursor < raw.len() {
        let Some(ch) = raw[cursor..].chars().next() else {
            break;
        };
        if ch.is_whitespace() {
            cursor += ch.len_utf8();
            continue;
        }
        let start = cursor;
        cursor += ch.len_utf8();
        while cursor < raw.len() {
            let Some(ch) = raw[cursor..].chars().next() else {
                break;
            };
            if ch.is_whitespace() {
                break;
            }
            cursor += ch.len_utf8();
        }
        tokens.push(MindmapParsedPayloadToken {
            value: raw[start..cursor].to_string(),
            selection: SourceSpan::new(raw_start + start, raw_start + cursor),
        });
    }
    tokens
}

fn record_mindmap_error(
    first_error: &mut Option<Error>,
    diagram_type: &str,
    message: impl Into<String>,
    span: SourceSpan,
) {
    first_error.get_or_insert_with(|| {
        Error::diagram_parse_exact(diagram_type.to_string(), message.into(), span)
    });
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MindmapContinuationState {
    Node {
        syntax: NodeSpecContinuation,
        original_indent: usize,
    },
    Icon,
}

struct MindmapPendingSyntax {
    state: MindmapContinuationState,
    statement_span: SourceSpan,
    message: String,
}

enum MindmapLineOutcome {
    Done,
    NeedMoreInput(MindmapPendingSyntax),
}

struct PendingMindmapLine {
    text: String,
    start: usize,
    syntax: MindmapPendingSyntax,
}

fn handle_mindmap_line(
    line: &str,
    line_start: usize,
    parsed: &mut MindmapParsedLines,
    first_error: &mut Option<Error>,
    diagram_type: &str,
) -> MindmapLineOutcome {
    if line.trim().is_empty() {
        return MindmapLineOutcome::Done;
    }

    let (mut indent, rest) = split_indent(line);
    let rest_offset = line.len().saturating_sub(rest.len());
    let mut rest = rest.trim_end();
    let mut statement_start = line_start + rest_offset;
    // Empty ICON states emit no token; the following adjacent statement still owns this line.
    while starts_with_case_insensitive(rest, "::icon()") {
        rest = &rest["::icon()".len()..];
        statement_start += "::icon()".len();
        let (tail_indent, tail) = split_indent(rest);
        if tail_indent > 0 {
            if indent > 0 && !is_statement_terminator(rest) {
                record_mindmap_error(
                    first_error,
                    diagram_type,
                    "unexpected trailing input after mindmap icon directive",
                    SourceSpan::new(
                        statement_start + rest.len() - tail.len(),
                        statement_start + rest.len(),
                    ),
                );
                return MindmapLineOutcome::Done;
            }
            indent = tail_indent;
            statement_start += rest.len() - tail.len();
            rest = tail;
        }
    }
    if is_statement_terminator(rest) {
        return MindmapLineOutcome::Done;
    }
    let statement_span = SourceSpan::new(statement_start, statement_start + rest.len());

    if starts_with_case_insensitive(rest, "::icon(") {
        let after = &rest["::icon(".len()..];
        let Some(end) = after.find(')') else {
            return MindmapLineOutcome::NeedMoreInput(MindmapPendingSyntax {
                state: MindmapContinuationState::Icon,
                statement_span,
                message: "unterminated mindmap icon directive".to_string(),
            });
        };
        let icon = &after[..end];
        let tail = &after[end + 1..];
        if !is_statement_terminator(tail) {
            let tail_start = statement_start + "::icon(".len() + end + 1;
            let leading = tail.len() - tail.trim_start().len();
            record_mindmap_error(
                first_error,
                diagram_type,
                "unexpected trailing input after mindmap icon directive",
                SourceSpan::new(tail_start + leading, statement_span.end),
            );
            return MindmapLineOutcome::Done;
        }
        #[cfg(feature = "editor-facts")]
        let selection_start = statement_start + "::icon(".len();
        #[cfg(feature = "editor-facts")]
        let tokens = if parsed.capture_editor {
            parsed.directive_prefixes.push("::icon".to_string());
            vec![MindmapParsedPayloadToken {
                value: icon.to_string(),
                selection: SourceSpan::new(selection_start, selection_start + icon.len()),
            }]
        } else {
            Vec::new()
        };
        parsed
            .events
            .push(MindmapParsedEvent::Icon(MindmapParsedPayloadLine {
                value: icon.to_string(),
                #[cfg(feature = "editor-facts")]
                span: statement_span,
                #[cfg(feature = "editor-facts")]
                tokens,
            }));
        return MindmapLineOutcome::Done;
    }

    if let Some(after) = rest.strip_prefix(":::") {
        let class = after.trim();
        if class.is_empty() {
            record_mindmap_error(
                first_error,
                diagram_type,
                "mindmap class is empty",
                statement_span,
            );
            return MindmapLineOutcome::Done;
        }
        #[cfg(feature = "editor-facts")]
        let class_leading = after.len() - after.trim_start().len();
        #[cfg(feature = "editor-facts")]
        let class_start = statement_start + ":::".len() + class_leading;
        #[cfg(feature = "editor-facts")]
        let tokens = if parsed.capture_editor {
            parsed.directive_prefixes.push(":::".to_string());
            mindmap_payload_tokens(class, class_start)
        } else {
            Vec::new()
        };
        parsed
            .events
            .push(MindmapParsedEvent::Class(MindmapParsedPayloadLine {
                value: class.to_string(),
                #[cfg(feature = "editor-facts")]
                span: statement_span,
                #[cfg(feature = "editor-facts")]
                tokens,
            }));
        return MindmapLineOutcome::Done;
    }

    #[cfg(feature = "editor-facts")]
    let editor_demand = if parsed.capture_editor {
        EditorFactsDemand::Capture
    } else {
        EditorFactsDemand::Omit
    };
    match parse_node_spec(
        rest,
        #[cfg(feature = "editor-facts")]
        editor_demand,
    ) {
        Ok(NodeSpec {
            id_raw,
            descr_raw,
            ty,
            descr_is_markdown,
            selection,
            #[cfg(feature = "editor-facts")]
            trace,
        }) => {
            let selection = absolute_mindmap_span(statement_start, selection);
            #[cfg(feature = "editor-facts")]
            let payload_span = trace
                .and_then(|trace| trace.description_span)
                .map(|span| absolute_mindmap_span(statement_start, span));
            parsed
                .events
                .push(MindmapParsedEvent::Node(MindmapParsedNodeLine {
                    indent,
                    id_raw,
                    descr_raw,
                    descr_is_markdown,
                    ty,
                    #[cfg(feature = "editor-facts")]
                    span: statement_span,
                    selection,
                    #[cfg(feature = "editor-facts")]
                    payload_span,
                }));
            MindmapLineOutcome::Done
        }
        Err(error) if error.continuation.is_some() => {
            let continuation = error
                .continuation
                .expect("continuation branch requires typed syntax state");
            MindmapLineOutcome::NeedMoreInput(MindmapPendingSyntax {
                state: MindmapContinuationState::Node {
                    syntax: continuation,
                    original_indent: indent,
                },
                statement_span,
                message: error.message,
            })
        }
        Err(error) => {
            record_mindmap_error(first_error, diagram_type, error.message, statement_span);
            MindmapLineOutcome::Done
        }
    }
}

fn finish_pending_mindmap_line(
    pending: PendingMindmapLine,
    first_error: &mut Option<Error>,
    diagram_type: &str,
) {
    let MindmapPendingSyntax {
        statement_span,
        message,
        ..
    } = pending.syntax;
    record_mindmap_error(first_error, diagram_type, message, statement_span);
}

fn is_safe_mindmap_statement(line: &str) -> bool {
    let (_, rest) = split_indent(line);
    let rest = rest.trim_end();
    if is_statement_terminator(rest) {
        return false;
    }
    if starts_with_case_insensitive(rest, "::icon(") || rest.starts_with(":::") {
        return true;
    }

    starts_node_spec(rest)
}

fn pending_mindmap_line_should_synchronize(
    pending: &PendingMindmapLine,
    physical_line: &str,
) -> bool {
    let MindmapContinuationState::Node {
        syntax,
        original_indent,
    } = pending.syntax.state
    else {
        // ICON consumes every physical line until ')', regardless of node-like text or indent.
        return false;
    };
    let (indent, rest) = split_indent(physical_line);
    let rest = rest.trim_end();
    if rest.is_empty() || syntax.has_open_text() || indent > original_indent {
        return false;
    }
    // `(` also opens an anonymous sibling; only a standalone closer continues this node.
    if indent == original_indent
        && let Some(closing) = node_closing_delimiter_at_start(rest)
        && is_statement_terminator(&rest[closing.len()..])
    {
        return false;
    }
    indent <= original_indent && is_safe_mindmap_statement(physical_line)
}

fn process_mindmap_physical_line(
    physical_line: &str,
    line_start: usize,
    pending: &mut Option<PendingMindmapLine>,
    parsed: &mut MindmapParsedLines,
    first_error: &mut Option<Error>,
    diagram_type: &str,
) {
    if pending
        .as_ref()
        .is_some_and(|current| pending_mindmap_line_should_synchronize(current, physical_line))
    {
        let current = pending.take().expect("checked pending mindmap line");
        finish_pending_mindmap_line(current, first_error, diagram_type);
    }

    if let Some(mut current) = pending.take() {
        current.text.push('\n');
        current.text.push_str(physical_line);
        if current.syntax.state == MindmapContinuationState::Icon && !physical_line.contains(')') {
            let content = physical_line.trim_end();
            if !content.is_empty() {
                current.syntax.statement_span.end = line_start + content.len();
            }
            *pending = Some(current);
            return;
        }
        match handle_mindmap_line(
            &current.text,
            current.start,
            parsed,
            first_error,
            diagram_type,
        ) {
            MindmapLineOutcome::Done => {}
            MindmapLineOutcome::NeedMoreInput(syntax) => {
                current.syntax = syntax;
                *pending = Some(current);
            }
        }
        return;
    }

    if let MindmapLineOutcome::NeedMoreInput(syntax) =
        handle_mindmap_line(physical_line, line_start, parsed, first_error, diagram_type)
    {
        *pending = Some(PendingMindmapLine {
            text: physical_line.to_string(),
            start: line_start,
            syntax,
        });
    }
}

fn parse_mindmap_lines(
    code: &str,
    diagram_type: &str,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<MindmapParseOutcome> {
    control.checkpoint()?;
    let mut lines = code.split_inclusive('\n').peekable();
    let mut offset = 0usize;
    let mut parsed = MindmapParsedLines {
        #[cfg(feature = "editor-facts")]
        capture_editor: matches!(editor_demand, EditorFactsDemand::Capture),
        ..Default::default()
    };
    let mut first_error = None;
    let mut header_tail: Option<(&str, usize)> = None;
    let mut found_header = false;

    for segment in lines.by_ref() {
        control.checkpoint()?;
        let line_start = offset;
        offset += segment.len();
        let line = segment.strip_suffix('\n').unwrap_or(segment);
        let statement = strip_inline_comment(line);
        let trimmed = statement.trim();
        if trimmed.is_empty() {
            continue;
        }
        let leading = statement.len() - statement.trim_start().len();
        let keyword_start = line_start + leading;
        let is_header = trimmed.eq_ignore_ascii_case("mindmap");
        let has_tail = starts_with_case_insensitive(trimmed, "mindmap")
            && trimmed.len() > "mindmap".len()
            && trimmed["mindmap".len()..]
                .chars()
                .next()
                .is_some_and(char::is_whitespace);
        if is_header || has_tail {
            found_header = true;
            if has_tail {
                let tail_start = leading + "mindmap".len();
                header_tail = Some((&line[tail_start..], line_start + tail_start));
            }
            break;
        }
        record_mindmap_error(
            &mut first_error,
            diagram_type,
            "expected mindmap header",
            SourceSpan::new(keyword_start, keyword_start + trimmed.len()),
        );
        break;
    }

    if !found_header {
        if first_error.is_none() {
            first_error = Some(Error::diagram_parse_insertion_point(
                diagram_type.to_string(),
                "expected mindmap header",
                code.len(),
            ));
        }
        return Ok(MindmapParseOutcome {
            parsed,
            first_error,
        });
    }

    let mut pending = None;
    if let Some((tail, tail_start)) = header_tail {
        process_mindmap_physical_line(
            tail,
            tail_start,
            &mut pending,
            &mut parsed,
            &mut first_error,
            diagram_type,
        );
    }
    for segment in lines {
        control.checkpoint()?;
        let line_start = offset;
        offset += segment.len();
        let line = segment.strip_suffix('\n').unwrap_or(segment);
        process_mindmap_physical_line(
            line,
            line_start,
            &mut pending,
            &mut parsed,
            &mut first_error,
            diagram_type,
        );
    }
    if let Some(pending) = pending {
        finish_pending_mindmap_line(pending, &mut first_error, diagram_type);
    }

    control.checkpoint()?;
    Ok(MindmapParseOutcome {
        parsed,
        first_error,
    })
}

#[cfg(test)]
mod semantic_tests {
    use super::*;

    fn metadata() -> crate::SemanticMetadata {
        crate::SemanticMetadata {
            diagram_type: "mindmap".to_string(),
            config: crate::MermaidConfig::empty_object(),
            semantic_config: crate::MermaidConfig::empty_object(),
            title: None,
        }
    }

    #[test]
    fn semantic_bare_percent_labels_preserve_literal_text() {
        reset_mindmap_syntax_construction_count();
        let construction = parse_mindmap_semantic_model_controlled(
            "mindmap\n  Root %% literal\n    %% comment line\n    child[Label %% literal] %% comment\n",
            &metadata(),
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &crate::OperationControl::new(),
        )
        .unwrap();
        let model = construction.model.unwrap();
        assert_eq!(model.nodes.len(), 2);
        assert_eq!(model.nodes[0].node_id, "Root %% literal");
        assert_eq!(model.nodes[0].descr, "Root %% literal");
        assert_eq!(model.nodes[1].descr, "Label %% literal");
        assert_eq!(model.nodes[0].children, vec![1]);
        assert_eq!(mindmap_syntax_construction_count(), 1);
    }

    #[test]
    fn semantic_shape_uses_the_actual_generic_closing_token() {
        use super::super::{
            NODE_TYPE_BANG, NODE_TYPE_CIRCLE, NODE_TYPE_CLOUD, NODE_TYPE_HEXAGON, NODE_TYPE_RECT,
            NODE_TYPE_ROUNDED_RECT,
        };

        // Generic NODE_DEND is deliberately permissive in the selected Jison grammar.
        for (source, label, node_type) in [
            ("root(Label]", "Label", NODE_TYPE_CLOUD),
            ("(Label]", "Label", NODE_TYPE_CLOUD),
            ("root(Label))", "Label", NODE_TYPE_CLOUD),
            ("root(Label)", "Label", NODE_TYPE_ROUNDED_RECT),
            ("root[Label)", "Label", NODE_TYPE_RECT),
            ("root((Label]", "Label", NODE_TYPE_CIRCLE),
            ("root{{Label]", "Label", NODE_TYPE_HEXAGON),
            ("root))Label]", "Label", NODE_TYPE_BANG),
            ("root(\"Label ) ] ((\"]", "Label ) ] ((", NODE_TYPE_CLOUD),
            ("root(Label-)", "Label-", NODE_TYPE_ROUNDED_RECT),
            ("root(\"Label\"-)", "Label", NODE_TYPE_CLOUD),
        ] {
            let input = format!("mindmap\n  {source}\n");
            let model = parse_mindmap_semantic_model_controlled(
                &input,
                &metadata(),
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &crate::OperationControl::new(),
            )
            .unwrap()
            .model
            .unwrap_or_else(|error| panic!("{source}: {error}"));
            assert_eq!(model.nodes[0].descr, label, "{source}");
            assert_eq!(model.nodes[0].node_type, node_type, "{source}");
        }
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn semantic_recovery_preserves_same_indent_anonymous_sibling_spans() {
        for sibling in ["(Recovered)", "((Recovered))"] {
            let input = format!("mindmap\n  Root\n    broken[\n    {sibling}\n    after[After]\n");
            let construction = parse_mindmap_semantic_model_controlled(
                &input,
                &metadata(),
                EditorFactsDemand::Capture,
                &crate::OperationControl::new(),
            )
            .unwrap();
            let Error::DiagramParse { diagnostic, .. } = construction.model.unwrap_err() else {
                panic!("expected an incomplete node diagnostic");
            };
            let broken_start = input.find("broken[").unwrap();
            assert_eq!(
                diagnostic.span(),
                Some(SourceSpan::new(
                    broken_start,
                    broken_start + "broken[".len()
                ))
            );
            assert!(diagnostic.message().contains("unterminated node delimiter"));

            let facts = construction.editor_facts.unwrap();
            assert_eq!(
                facts.completeness,
                crate::EditorSemanticCompleteness::Recovered
            );
            let recovered = facts
                .symbols
                .iter()
                .find(|symbol| {
                    symbol.name == "Recovered" && symbol.detail.as_deref() == Some("mindmap node")
                })
                .expect("complete anonymous sibling must survive recovery");
            let selection_start = input.find("Recovered").unwrap();
            assert_eq!(
                recovered.selection,
                SourceSpan::new(selection_start, selection_start + "Recovered".len())
            );
            let statement_start = input.find(sibling).unwrap();
            assert_eq!(
                recovered.span,
                SourceSpan::new(statement_start, statement_start + sibling.len())
            );
            assert!(facts.symbols.iter().any(|symbol| symbol.name == "after"));
            assert!(!facts.symbols.iter().any(|symbol| symbol.name == "broken"));
        }
    }

    #[test]
    fn semantic_standalone_generic_closers_continue_unquoted_nodes() {
        for closing in ["(", "((", "]"] {
            for suffix in ["", " %% comment"] {
                let input = format!(
                    "mindmap\n  Root\n    child[\n      Label\n    {closing}{suffix}\n    after[After]\n"
                );
                let model = parse_mindmap_semantic_model_controlled(
                    &input,
                    &metadata(),
                    #[cfg(feature = "editor-facts")]
                    EditorFactsDemand::Omit,
                    &crate::OperationControl::new(),
                )
                .unwrap()
                .model
                .unwrap();
                assert_eq!(model.nodes.len(), 3);
                assert_eq!(model.nodes[0].children, vec![1, 2]);
                assert_eq!(model.nodes[1].node_id, "child");
                assert_eq!(model.nodes[1].descr, "\n      Label\n    ");
                assert_eq!(model.nodes[1].node_type, super::super::NODE_TYPE_RECT);
                assert_eq!(model.nodes[2].node_id, "after");
            }
        }
    }

    #[test]
    fn semantic_multiline_quotes_preserve_same_indent_node_shaped_text() {
        for (opening, closing, markdown) in [("\"", "\"", false), ("\"`", "`\"", true)] {
            let input = format!(
                "mindmap\n  Root\n    child[{opening}Start\n    (Literal)\n    End{closing}]\n    after[After]\n"
            );
            let model = parse_mindmap_semantic_model_controlled(
                &input,
                &metadata(),
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &crate::OperationControl::new(),
            )
            .unwrap()
            .model
            .unwrap();
            assert_eq!(model.nodes.len(), 3);
            assert_eq!(model.nodes[0].children, vec![1, 2]);
            assert_eq!(model.nodes[1].descr, "Start\n    (Literal)\n    End");
            assert_eq!(model.nodes[1].descr_is_markdown, markdown);
            assert_eq!(model.nodes[2].node_id, "after");
        }
    }

    #[test]
    fn semantic_icon_state_preserves_raw_payload_across_physical_lines() {
        for payload in [
            "fa\n  fa-star",
            "  fa\n\n  fa-star  ",
            "\n ",
            "  ",
            "fa\nliteral[Node]\n:::literal\n  fa-star",
        ] {
            let input = format!("mindmap\n  Root\n  ::icon({payload})\n    child[Child]\n");
            #[cfg(feature = "editor-facts")]
            crate::editor::test_work::reset();
            reset_mindmap_syntax_construction_count();
            let model = parse_mindmap_semantic_model_controlled(
                &input,
                &metadata(),
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &crate::OperationControl::new(),
            )
            .unwrap()
            .model
            .unwrap();
            assert_eq!(model.nodes.len(), 2, "{payload:?}");
            assert_eq!(model.nodes[0].icon.as_deref(), Some(payload));
            assert_eq!(model.nodes[0].children, vec![1]);
            assert_eq!(model.nodes[1].node_id, "child");
            assert_eq!(mindmap_syntax_construction_count(), 1);
            #[cfg(feature = "editor-facts")]
            assert_eq!(crate::editor::test_work::read(), Default::default());
        }
    }

    #[test]
    fn semantic_empty_icon_leaves_the_previous_decoration_intact() {
        for previous in [None, Some("star")] {
            let prior = previous
                .map(|icon| format!("  ::icon({icon})\n"))
                .unwrap_or_default();
            let input = format!("mindmap\n  Root\n{prior}  ::icon()\n    child[Child]\n");
            let model = parse_mindmap_semantic_model_controlled(
                &input,
                &metadata(),
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &crate::OperationControl::new(),
            )
            .unwrap()
            .model
            .unwrap();
            assert_eq!(model.nodes.len(), 2);
            assert_eq!(model.nodes[0].icon.as_deref(), previous);
            assert_eq!(model.nodes[0].children, vec![1]);
        }
    }

    #[test]
    fn semantic_empty_icon_prefixes_preserve_the_following_statement() {
        for (tail, expected_icon, expected_class) in [
            (":::after", None, Some("after")),
            ("::icon(other)", Some("other"), None),
        ] {
            let input = format!("mindmap\n  Root\n  ::icon()::ICON(){tail}\n");
            let model = parse_mindmap_semantic_model_controlled(
                &input,
                &metadata(),
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &crate::OperationControl::new(),
            )
            .unwrap()
            .model
            .unwrap();
            assert_eq!(model.nodes[0].icon.as_deref(), expected_icon);
            assert_eq!(model.nodes[0].class.as_deref(), expected_class);
        }
    }

    #[test]
    fn semantic_icon_requires_a_statement_terminator() {
        for payload in ["star", "fa\n  fa-star"] {
            for suffix in [" trailing", ":::class", "::icon(other)"] {
                let input = format!("mindmap\n  Root\n  ::icon({payload}){suffix}\n");
                let error = parse_mindmap_semantic_model_controlled(
                    &input,
                    &metadata(),
                    #[cfg(feature = "editor-facts")]
                    EditorFactsDemand::Omit,
                    &crate::OperationControl::new(),
                )
                .unwrap()
                .model
                .unwrap_err();
                let Error::DiagramParse { diagnostic, .. } = error else {
                    panic!("expected trailing icon input diagnostic");
                };
                let start = input.find(suffix).unwrap() + suffix.len() - suffix.trim_start().len();
                assert_eq!(
                    diagnostic.span(),
                    Some(SourceSpan::new(start, start + suffix.trim_start().len()))
                );
            }
            for suffix in ["", " %% comment"] {
                let input = format!("mindmap\n  Root\n  ::icon({payload}){suffix}\n");
                let model = parse_mindmap_semantic_model_controlled(
                    &input,
                    &metadata(),
                    #[cfg(feature = "editor-facts")]
                    EditorFactsDemand::Omit,
                    &crate::OperationControl::new(),
                )
                .unwrap()
                .model
                .unwrap();
                assert_eq!(
                    model.nodes[0].icon.as_deref(),
                    (!payload.is_empty()).then_some(payload)
                );
            }
        }
    }

    #[test]
    fn semantic_repaired_syntax_preserves_in_progress_cancellation() {
        let input = format!(
            "mindmap\n  Root %% literal\n{}",
            "    child(Label]\n".repeat(256)
        );
        let control = crate::OperationControl::new();
        control.cancel_after_checkpoints(4);
        assert!(
            parse_mindmap_semantic_model_controlled(
                &input,
                &metadata(),
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &control,
            )
            .is_err()
        );
        assert!(control.checkpoint().is_err());
    }

    #[test]
    fn semantic_tree_constructs_once_without_render_fields_or_editor_work() {
        reset_mindmap_syntax_construction_count();
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let model = parse_mindmap_semantic_model_controlled(
            "mindmap\n  root[Root]\n    child((Child))\n      leaf[\"`Markdown`\"]\n      ::icon(star)\n      :::highlight\n",
            &metadata(), #[cfg(feature = "editor-facts")] EditorFactsDemand::Omit, &crate::OperationControl::new(),
        ).unwrap().model.unwrap();
        assert_eq!(model.nodes.len(), 3);
        assert_eq!(model.nodes[0].node_id, "root");
        assert_eq!(model.nodes[0].children, vec![1]);
        assert_eq!(model.nodes[1].children, vec![2]);
        assert_eq!(model.nodes[1].node_type, super::super::NODE_TYPE_CIRCLE);
        assert_eq!(model.nodes[2].descr, "Markdown");
        assert!(model.nodes[2].descr_is_markdown);
        assert_eq!(model.nodes[2].icon.as_deref(), Some("star"));
        assert_eq!(model.nodes[2].class.as_deref(), Some("highlight"));
        assert_eq!(mindmap_syntax_construction_count(), 1);
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
        let value = serde_json::to_value(model).unwrap();
        for key in ["width", "height", "padding", "section", "shape", "look"] {
            assert!(
                value["nodes"][0].get(key).is_none(),
                "unexpected visual field {key}"
            );
        }
    }

    #[test]
    fn semantic_config_sanitizes_plain_descriptions_and_preserves_markdown_source() {
        let mut metadata = metadata();
        metadata.semantic_config = crate::MermaidConfig::from_value(serde_json::json!({
            "securityLevel": "strict", "themeVariables": {"primaryColor": "not-a-color"},
            "mindmap": {"padding": 40, "colorBy": "depth"}
        }));
        let model = parse_mindmap_semantic_model_controlled(
            "mindmap\n  root[\"<script>bad()</script>Safe\"]\n    leaf[\"`**Markdown**`\"]",
            &metadata,
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &crate::OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap();
        assert_eq!(model.nodes[0].descr, "Safe");
        assert_eq!(model.nodes[1].descr, "**Markdown**");
    }

    #[test]
    fn semantic_database_error_preserves_exact_selection_without_editor_work() {
        let input = "mindmap\n  root\n  second[Other]\n";
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let error = parse_mindmap_semantic_model_controlled(
            input,
            &metadata(),
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &crate::OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap_err();
        let Error::DiagramParse { diagnostic, .. } = &error else {
            panic!("expected parser diagnostic")
        };
        let offset = input.find("second").unwrap();
        assert_eq!(
            diagnostic.span(),
            Some(SourceSpan::new(offset, offset + "second".len()))
        );
        assert!(diagnostic.message().contains("There can be only one root"));
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
        #[cfg(feature = "presentation")]
        {
            let full = SourcePresentationMetadata {
                diagram_type: "mindmap".to_string(),
                config: crate::MermaidConfig::empty_object(),
                effective_config: crate::presentation::SourcePresentationConfig::empty_object(),
                title: None,
            };
            let full_error = parse_mindmap_model_for_render_controlled(
                input,
                &full,
                &crate::OperationControl::new(),
            )
            .unwrap()
            .unwrap_err();
            assert_eq!(error.to_string(), full_error.to_string());
        }
    }

    #[test]
    fn semantic_cancellation_stops_before_construction() {
        reset_mindmap_syntax_construction_count();
        let control = crate::OperationControl::new();
        control.cancel();
        assert!(
            parse_mindmap_semantic_model_controlled(
                "mindmap\n  root",
                &metadata(),
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &control
            )
            .is_err()
        );
        assert_eq!(mindmap_syntax_construction_count(), 0);
    }
    #[cfg(feature = "compat-json")]
    #[test]
    fn typed_and_json_paths_skip_editor_projection() {
        let full = SourcePresentationMetadata {
            diagram_type: "mindmap".to_string(),
            config: crate::MermaidConfig::empty_object(),
            effective_config: crate::presentation::SourcePresentationConfig::empty_object(),
            title: None,
        };
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        reset_mindmap_syntax_construction_count();
        parse_mindmap_model_for_render_controlled(
            "mindmap\n  root\n    child",
            &full,
            &crate::OperationControl::new(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(mindmap_syntax_construction_count(), 1);
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
        reset_mindmap_syntax_construction_count();
        parse_mindmap("mindmap\n  root\n    child", &full).unwrap();
        assert_eq!(mindmap_syntax_construction_count(), 1);
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
    }
}
