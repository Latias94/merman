use serde_json::Value;
#[cfg(all(test, feature = "full"))]
use std::cell::Cell;

use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorLexemeKind, EditorLexemeModifier,
    EditorLexemeModifiers, EditorSemanticFacts, EditorSemanticKind, EditorSemanticSymbol, Error,
    ParseMetadata, Result, SourceSpan, editor::EditorLexemeJournal,
};

use super::db::{MindmapDb, MindmapParseConfig};
use super::render_model::MindmapDiagramRenderModel;
use super::utils::{NodeSpec, NodeSpecTrace, parse_node_spec, strip_inline_comment};
use crate::diagrams::scan::{split_indent, starts_with_case_insensitive};

#[cfg(all(test, feature = "full"))]
thread_local! {
    static MINDMAP_SYNTAX_CONSTRUCTION_COUNT: Cell<usize> = const { Cell::new(0) };
}

#[cfg(all(test, feature = "full"))]
pub(crate) fn reset_mindmap_syntax_construction_count() {
    MINDMAP_SYNTAX_CONSTRUCTION_COUNT.set(0);
}

#[cfg(all(test, feature = "full"))]
pub(crate) fn mindmap_syntax_construction_count() -> usize {
    MINDMAP_SYNTAX_CONSTRUCTION_COUNT.get()
}

pub fn parse_mindmap(code: &str, meta: &ParseMetadata) -> Result<Value> {
    let model = parse_mindmap_semantic_source(code, meta)?.into_render_model(meta)?;
    super::render_model_to_compat_json(&model, meta)
}

pub(crate) fn parse_mindmap_json_and_editor_facts(
    code: &str,
    meta: &ParseMetadata,
) -> Result<(Value, EditorSemanticFacts)> {
    let source = parse_mindmap_semantic_source(code, meta)?;
    let editor_facts = source.editor_facts.clone();
    let model = source.into_render_model(meta)?;
    let model = super::render_model_to_compat_json(&model, meta)?;
    Ok((model, editor_facts))
}

pub fn parse_mindmap_model_for_render(
    code: &str,
    meta: &ParseMetadata,
) -> Result<MindmapDiagramRenderModel> {
    parse_mindmap_semantic_source(code, meta)?.into_render_model(meta)
}

struct MindmapSemanticSource {
    db: MindmapDb,
    editor_facts: EditorSemanticFacts,
}

impl MindmapSemanticSource {
    fn into_render_model(self, meta: &ParseMetadata) -> Result<MindmapDiagramRenderModel> {
        let mut db = self.db;
        let Some(root_id) = db.get_mindmap().map(|n| n.id) else {
            return Ok(MindmapDiagramRenderModel::default());
        };

        db.assign_sections(root_id, None);

        Ok(MindmapDiagramRenderModel {
            nodes: db.to_layout_nodes_for_render(root_id, &meta.effective_config),
            edges: db.to_edges_for_render(root_id, &meta.effective_config),
        })
    }
}

struct MindmapSemanticFailure {
    error: Box<Error>,
    editor_facts: Box<EditorSemanticFacts>,
}

impl MindmapSemanticFailure {
    fn into_editor_facts(mut self) -> EditorSemanticFacts {
        let (message, span) = match self.error.as_ref() {
            Error::DiagramParse { diagnostic, .. } => {
                (diagnostic.message().to_string(), diagnostic.span())
            }
            error => (error.to_string(), None),
        };
        self.editor_facts.mark_recovered_from_parse_error(
            format!("mindmap parser recovered after parse error: {message}"),
            span,
        );
        *self.editor_facts
    }
}

fn parse_mindmap_semantic_source(
    code: &str,
    meta: &ParseMetadata,
) -> Result<MindmapSemanticSource> {
    construct_mindmap_semantic_source(code, meta).map_err(|failure| *failure.error)
}

fn construct_mindmap_semantic_source(
    code: &str,
    meta: &ParseMetadata,
) -> std::result::Result<MindmapSemanticSource, MindmapSemanticFailure> {
    #[cfg(all(test, feature = "full"))]
    MINDMAP_SYNTAX_CONSTRUCTION_COUNT.set(MINDMAP_SYNTAX_CONSTRUCTION_COUNT.get() + 1);

    let mut lexemes = EditorLexemeJournal::family_parser(code);
    let MindmapParseOutcome {
        parsed,
        first_error,
    } = parse_mindmap_lines(code, meta, &mut lexemes);
    let mut editor_facts = mindmap_editor_facts_from_parsed(&parsed);
    editor_facts.replace_family_lexemes(lexemes.finish());

    if let Some(error) = first_error {
        return Err(MindmapSemanticFailure {
            error: Box::new(error),
            editor_facts: Box::new(editor_facts),
        });
    }

    let db = match mindmap_db_from_events(parsed.events, meta) {
        Ok(db) => db,
        Err(error) => {
            return Err(MindmapSemanticFailure {
                error: Box::new(error),
                editor_facts: Box::new(editor_facts),
            });
        }
    };
    Ok(MindmapSemanticSource { db, editor_facts })
}

fn mindmap_db_from_events(
    events: Vec<MindmapParsedEvent>,
    meta: &ParseMetadata,
) -> Result<MindmapDb> {
    let mut db = MindmapDb::default();
    db.clear();
    let parse_config = MindmapParseConfig::from_config(&meta.effective_config);

    for event in events {
        match event {
            MindmapParsedEvent::Node(node) => {
                let selection = node.selection;
                db.add_node(
                    super::db::MindmapNodeInput {
                        indent_level: node.indent as i32,
                        id_raw: &node.id_raw,
                        descr_raw: &node.descr_raw,
                        descr_is_markdown: node.descr_is_markdown,
                        ty: node.ty,
                        diagram_type: &meta.diagram_type,
                    },
                    &meta.effective_config,
                    parse_config,
                )
                .map_err(|error| error.with_exact_span_if_missing(selection))?;
            }
            MindmapParsedEvent::Class(class) => {
                db.decorate_last(Some(class.value), None, &meta.effective_config);
            }
            MindmapParsedEvent::Icon(icon) => {
                db.decorate_last(None, Some(icon.value), &meta.effective_config);
            }
        }
    }

    Ok(db)
}

pub fn parse_mindmap_editor_facts(code: &str, meta: &ParseMetadata) -> EditorSemanticFacts {
    match construct_mindmap_semantic_source(code, meta) {
        Ok(source) => source.editor_facts,
        Err(failure) => failure.into_editor_facts(),
    }
}

#[derive(Debug, Clone)]
struct MindmapParsedNodeLine {
    indent: usize,
    id_raw: String,
    descr_raw: String,
    descr_is_markdown: bool,
    ty: i32,
    span: SourceSpan,
    selection: SourceSpan,
    payload_span: Option<SourceSpan>,
}

#[derive(Debug, Clone)]
struct MindmapParsedPayloadLine {
    value: String,
    span: SourceSpan,
    tokens: Vec<MindmapParsedPayloadToken>,
}

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
    directive_prefixes: Vec<String>,
}

struct MindmapParseOutcome {
    parsed: MindmapParsedLines,
    first_error: Option<Error>,
}

fn mindmap_editor_facts_from_parsed(parsed: &MindmapParsedLines) -> EditorSemanticFacts {
    let mut facts = EditorSemanticFacts::new();
    for prefix in &parsed.directive_prefixes {
        facts.push_directive_prefix(prefix.clone());
    }
    for event in &parsed.events {
        match event {
            MindmapParsedEvent::Node(node) => {
                facts.push_expected_syntax(EditorExpectedSyntax::new(
                    EditorExpectedSyntaxKind::NodeIdentifier,
                    node.selection,
                ));
                if let Some(payload_span) = node.payload_span {
                    facts.push_expected_syntax(EditorExpectedSyntax::new(
                        EditorExpectedSyntaxKind::Payload,
                        payload_span,
                    ));
                    facts.push_symbol(EditorSemanticSymbol::payload(
                        node.descr_raw.clone(),
                        Some("mindmap node label".to_string()),
                        EditorSemanticKind::String,
                        payload_span,
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
                for token in &class.tokens {
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
                for token in &icon.tokens {
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
    facts
}

fn push_mindmap_lexeme(
    lexemes: &mut EditorLexemeJournal<'_>,
    kind: EditorLexemeKind,
    span: SourceSpan,
) {
    push_mindmap_lexeme_with_modifiers(lexemes, kind, EditorLexemeModifiers::NONE, span);
}

fn push_mindmap_lexeme_with_modifiers(
    lexemes: &mut EditorLexemeJournal<'_>,
    kind: EditorLexemeKind,
    modifiers: EditorLexemeModifiers,
    span: SourceSpan,
) {
    if span.start < span.end {
        lexemes.push(kind, modifiers, span);
    }
}

fn absolute_mindmap_span(base: usize, span: SourceSpan) -> SourceSpan {
    SourceSpan::new(base + span.start, base + span.end)
}

fn record_mindmap_node_trace(
    trace: &NodeSpecTrace,
    base: usize,
    lexemes: &mut EditorLexemeJournal<'_>,
) {
    for delimiter in [
        trace.shape_opening,
        trace.text_opening,
        trace.text_closing,
        trace.shape_closing,
    ]
    .into_iter()
    .flatten()
    {
        push_mindmap_lexeme(
            lexemes,
            EditorLexemeKind::Delimiter,
            absolute_mindmap_span(base, delimiter),
        );
    }
    if let Some(id) = trace.id_span {
        push_mindmap_lexeme_with_modifiers(
            lexemes,
            EditorLexemeKind::Identifier,
            EditorLexemeModifiers::from_modifier(EditorLexemeModifier::Definition),
            absolute_mindmap_span(base, id),
        );
    }
    if trace.explicit_id
        && let Some(description) = trace.description_span
    {
        push_mindmap_lexeme(
            lexemes,
            EditorLexemeKind::String,
            absolute_mindmap_span(base, description),
        );
    }
}

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
    meta: &ParseMetadata,
    message: impl Into<String>,
    span: SourceSpan,
) {
    first_error.get_or_insert_with(|| {
        Error::diagram_parse_exact(meta.diagram_type.clone(), message.into(), span)
    });
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MindmapLineOutcome {
    Done,
    NeedMoreInput,
}

fn handle_mindmap_line(
    line: &str,
    line_start: usize,
    allow_continuation: bool,
    parsed: &mut MindmapParsedLines,
    first_error: &mut Option<Error>,
    lexemes: &mut EditorLexemeJournal<'_>,
    meta: &ParseMetadata,
) -> MindmapLineOutcome {
    if line.trim().is_empty() {
        return MindmapLineOutcome::Done;
    }

    let (indent, rest) = split_indent(line);
    let rest_offset = line.len().saturating_sub(rest.len());
    let rest = rest.trim_end();
    if rest.is_empty() {
        return MindmapLineOutcome::Done;
    }
    let statement_start = line_start + rest_offset;
    let statement_span = SourceSpan::new(statement_start, statement_start + rest.len());

    if starts_with_case_insensitive(rest, "::icon(") {
        let keyword = SourceSpan::new(statement_start, statement_start + "::icon".len());
        let opening = SourceSpan::new(keyword.end, keyword.end + 1);
        push_mindmap_lexeme(lexemes, EditorLexemeKind::Keyword, keyword);
        push_mindmap_lexeme(lexemes, EditorLexemeKind::Delimiter, opening);
        let after = &rest["::icon(".len()..];
        let Some(end) = after.find(')') else {
            record_mindmap_error(
                first_error,
                meta,
                "unterminated mindmap icon directive",
                statement_span,
            );
            return MindmapLineOutcome::Done;
        };
        let icon_raw = &after[..end];
        let icon = icon_raw.trim();
        let closing_start = statement_start + "::icon(".len() + end;
        push_mindmap_lexeme(
            lexemes,
            EditorLexemeKind::Delimiter,
            SourceSpan::new(closing_start, closing_start + 1),
        );
        if icon.is_empty() {
            record_mindmap_error(first_error, meta, "mindmap icon is empty", statement_span);
            return MindmapLineOutcome::Done;
        }
        let icon_leading = icon_raw.len() - icon_raw.trim_start().len();
        let selection_start = statement_start + "::icon(".len() + icon_leading;
        let token = MindmapParsedPayloadToken {
            value: icon.to_string(),
            selection: SourceSpan::new(selection_start, selection_start + icon.len()),
        };
        push_mindmap_lexeme_with_modifiers(
            lexemes,
            EditorLexemeKind::Identifier,
            EditorLexemeModifiers::from_modifier(EditorLexemeModifier::Reference),
            token.selection,
        );
        parsed.directive_prefixes.push("::icon".to_string());
        parsed
            .events
            .push(MindmapParsedEvent::Icon(MindmapParsedPayloadLine {
                value: icon.to_string(),
                span: statement_span,
                tokens: vec![token],
            }));
        return MindmapLineOutcome::Done;
    }

    if let Some(after) = rest.strip_prefix(":::") {
        push_mindmap_lexeme(
            lexemes,
            EditorLexemeKind::Delimiter,
            SourceSpan::new(statement_start, statement_start + ":::".len()),
        );
        let class = after.trim();
        if class.is_empty() {
            record_mindmap_error(first_error, meta, "mindmap class is empty", statement_span);
            return MindmapLineOutcome::Done;
        }
        let class_leading = after.len() - after.trim_start().len();
        let class_start = statement_start + ":::".len() + class_leading;
        let tokens = mindmap_payload_tokens(class, class_start);
        for token in &tokens {
            push_mindmap_lexeme_with_modifiers(
                lexemes,
                EditorLexemeKind::Identifier,
                EditorLexemeModifiers::from_modifier(EditorLexemeModifier::Reference),
                token.selection,
            );
        }
        parsed.directive_prefixes.push(":::".to_string());
        parsed
            .events
            .push(MindmapParsedEvent::Class(MindmapParsedPayloadLine {
                value: class.to_string(),
                span: statement_span,
                tokens,
            }));
        return MindmapLineOutcome::Done;
    }

    let rest = strip_inline_comment(rest).trim_end();
    if rest.is_empty() {
        return MindmapLineOutcome::Done;
    }
    let statement_span = SourceSpan::new(statement_start, statement_start + rest.len());
    match parse_node_spec(rest) {
        Ok(NodeSpec {
            id_raw,
            descr_raw,
            ty,
            descr_is_markdown,
            trace,
        }) => {
            record_mindmap_node_trace(&trace, statement_start, lexemes);
            let selection = trace
                .id_span
                .map(|span| absolute_mindmap_span(statement_start, span))
                .unwrap_or(statement_span);
            let payload_span = trace
                .description_span
                .map(|span| absolute_mindmap_span(statement_start, span));
            parsed
                .events
                .push(MindmapParsedEvent::Node(MindmapParsedNodeLine {
                    indent,
                    id_raw,
                    descr_raw,
                    descr_is_markdown,
                    ty,
                    span: statement_span,
                    selection,
                    payload_span,
                }));
            MindmapLineOutcome::Done
        }
        Err(error) if error.can_continue && allow_continuation => MindmapLineOutcome::NeedMoreInput,
        Err(error) => {
            record_mindmap_node_trace(&error.trace, statement_start, lexemes);
            record_mindmap_error(first_error, meta, error.message, statement_span);
            MindmapLineOutcome::Done
        }
    }
}

fn parse_mindmap_lines(
    code: &str,
    meta: &ParseMetadata,
    lexemes: &mut EditorLexemeJournal<'_>,
) -> MindmapParseOutcome {
    let mut lines = code.split_inclusive('\n').peekable();
    let mut offset = 0usize;
    let mut parsed = MindmapParsedLines::default();
    let mut first_error = None;
    let mut header_tail: Option<(&str, usize)> = None;
    let mut found_header = false;

    for segment in lines.by_ref() {
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
            push_mindmap_lexeme(
                lexemes,
                EditorLexemeKind::Keyword,
                SourceSpan::new(keyword_start, keyword_start + "mindmap".len()),
            );
            if has_tail {
                let tail_start = leading + "mindmap".len();
                header_tail = Some((&line[tail_start..], line_start + tail_start));
            }
            break;
        }
        record_mindmap_error(
            &mut first_error,
            meta,
            "expected mindmap header",
            SourceSpan::new(keyword_start, keyword_start + trimmed.len()),
        );
        break;
    }

    if !found_header {
        if first_error.is_none() {
            first_error = Some(Error::diagram_parse_insertion_point(
                meta.diagram_type.clone(),
                "expected mindmap header",
                code.len(),
            ));
        }
        return MindmapParseOutcome {
            parsed,
            first_error,
        };
    }

    struct PendingMindmapLine {
        text: String,
        start: usize,
    }

    let mut pending = None;
    let push_line = |physical_line: &str,
                     line_start: usize,
                     pending: &mut Option<PendingMindmapLine>,
                     parsed: &mut MindmapParsedLines,
                     first_error: &mut Option<Error>,
                     lexemes: &mut EditorLexemeJournal<'_>| {
        match pending.as_mut() {
            Some(PendingMindmapLine { text, .. }) => {
                text.push('\n');
                text.push_str(physical_line);
            }
            None => {
                *pending = Some(PendingMindmapLine {
                    text: physical_line.to_string(),
                    start: line_start,
                });
            }
        }
        let current = pending.as_ref().expect("pending line was initialized");
        if handle_mindmap_line(
            &current.text,
            current.start,
            true,
            parsed,
            first_error,
            lexemes,
            meta,
        ) == MindmapLineOutcome::Done
        {
            *pending = None;
        }
    };

    if let Some((tail, tail_start)) = header_tail {
        push_line(
            tail,
            tail_start,
            &mut pending,
            &mut parsed,
            &mut first_error,
            lexemes,
        );
    }
    for segment in lines {
        let line_start = offset;
        offset += segment.len();
        let line = segment.strip_suffix('\n').unwrap_or(segment);
        push_line(
            line,
            line_start,
            &mut pending,
            &mut parsed,
            &mut first_error,
            lexemes,
        );
    }
    if let Some(PendingMindmapLine { text, start }) = pending {
        handle_mindmap_line(
            &text,
            start,
            false,
            &mut parsed,
            &mut first_error,
            lexemes,
            meta,
        );
    }

    MindmapParseOutcome {
        parsed,
        first_error,
    }
}
