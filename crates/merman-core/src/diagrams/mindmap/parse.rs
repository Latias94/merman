use serde_json::Value;
#[cfg(all(test, feature = "full"))]
use std::cell::Cell;

use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticCompleteness,
    EditorSemanticDiagnostic, EditorSemanticFacts, EditorSemanticKind, EditorSemanticSymbol, Error,
    ParseMetadata, Result, SourceSpan,
    editor::{editor_recovery_fallback_span, ensure_editor_recovery_from_error},
};

use super::db::{MindmapDb, MindmapParseConfig};
use super::render_model::MindmapDiagramRenderModel;
use super::utils::{NodeSpec, parse_node_spec, strip_inline_comment};
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
    let editor_facts = source.editor_facts();
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
    parsed: MindmapParsedLines,
}

impl MindmapSemanticSource {
    fn editor_facts(&self) -> EditorSemanticFacts {
        mindmap_editor_facts_from_parsed(&self.parsed)
    }

    fn into_db(self, meta: &ParseMetadata) -> Result<MindmapDb> {
        mindmap_db_from_events(self.parsed.events, meta)
    }

    fn into_render_model(self, meta: &ParseMetadata) -> Result<MindmapDiagramRenderModel> {
        let mut db = self.into_db(meta)?;
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

fn parse_mindmap_semantic_source(
    code: &str,
    meta: &ParseMetadata,
) -> Result<MindmapSemanticSource> {
    Ok(MindmapSemanticSource {
        parsed: parse_mindmap_lines(code, meta, false)?,
    })
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
    match parse_mindmap_semantic_source(code, meta) {
        Ok(source) => {
            let facts = source.editor_facts();
            match source.into_db(meta) {
                Ok(_) => facts,
                Err(error) => ensure_editor_recovery_from_error(
                    facts,
                    &error,
                    editor_recovery_fallback_span(code),
                ),
            }
        }
        Err(error) => {
            let facts = parse_mindmap_lines(code, meta, true)
                .map(|parsed| mindmap_editor_facts_from_parsed(&parsed))
                .unwrap_or_default();
            ensure_editor_recovery_from_error(facts, &error, editor_recovery_fallback_span(code))
        }
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
    completeness: EditorSemanticCompleteness,
    diagnostics: Vec<EditorSemanticDiagnostic>,
}

fn mindmap_editor_facts_from_parsed(parsed: &MindmapParsedLines) -> EditorSemanticFacts {
    let mut facts = EditorSemanticFacts {
        completeness: parsed.completeness,
        symbols: Vec::new(),
        directive_prefixes: Vec::new(),
        diagnostics: parsed.diagnostics.clone(),
        expected_syntax: Vec::new(),
    };
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
                facts.push_symbol(EditorSemanticSymbol::payload(
                    class.value.clone(),
                    Some("mindmap class".to_string()),
                    EditorSemanticKind::Property,
                    class.span,
                    class.selection,
                ));
            }
            MindmapParsedEvent::Icon(icon) => {
                facts.push_symbol(EditorSemanticSymbol::payload(
                    icon.value.clone(),
                    Some("mindmap icon".to_string()),
                    EditorSemanticKind::String,
                    icon.span,
                    icon.selection,
                ));
            }
        }
    }
    facts
}

fn parse_mindmap_lines(
    code: &str,
    meta: &ParseMetadata,
    recover: bool,
) -> Result<MindmapParsedLines> {
    #[cfg(all(test, feature = "full"))]
    MINDMAP_SYNTAX_CONSTRUCTION_COUNT.set(MINDMAP_SYNTAX_CONSTRUCTION_COUNT.get() + 1);

    let mut lines = code.split_inclusive('\n').peekable();
    let mut offset = 0usize;
    let mut found_header = false;
    let mut header_tail: Option<String> = None;
    let mut header_tail_offset = 0usize;
    for line in lines.by_ref() {
        let line_start = offset;
        offset += line.len();
        let line_no_newline = line.strip_suffix('\n').unwrap_or(line);
        let t = strip_inline_comment(line_no_newline);
        let trimmed = t.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.eq_ignore_ascii_case("mindmap") {
            found_header = true;
            break;
        }
        if starts_with_case_insensitive(trimmed, "mindmap")
            && trimmed.len() > "mindmap".len()
            && trimmed["mindmap".len()..]
                .chars()
                .next()
                .is_some_and(|c| c.is_whitespace())
        {
            found_header = true;
            let trimmed_offset = t.len().saturating_sub(t.trim_start().len());
            let after_keyword = &trimmed["mindmap".len()..];
            let indent = after_keyword
                .chars()
                .take_while(|c| c.is_whitespace())
                .count();
            let rest = after_keyword.trim_start();
            if !rest.is_empty() {
                header_tail = Some(format!("{}{}", " ".repeat(indent), rest));
                let rest_offset_in_trimmed =
                    "mindmap".len() + after_keyword.len().saturating_sub(rest.len());
                header_tail_offset = line_start + trimmed_offset + rest_offset_in_trimmed - indent;
            }
            break;
        }
        break;
    }

    if !found_header {
        return Err(Error::diagram_parse_fallback(
            meta.diagram_type.clone(),
            "expected mindmap header".to_string(),
        ));
    }

    let mut out = MindmapParsedLines::default();

    enum HandleOutcome {
        Done,
        NeedMoreInput,
    }

    let handle_line =
        |line: &str, line_start: usize, out: &mut MindmapParsedLines| -> Result<HandleOutcome> {
            if line.trim().is_empty() {
                return Ok(HandleOutcome::Done);
            }

            let (indent, rest) = split_indent(line);
            let rest_offset = line.len().saturating_sub(rest.len());
            let rest = rest.trim_end();
            if rest.is_empty() {
                return Ok(HandleOutcome::Done);
            }

            if starts_with_case_insensitive(rest, "::icon(") {
                let statement_span = SourceSpan::new(
                    line_start + rest_offset,
                    line_start + rest_offset + rest.len(),
                );
                let after = &rest["::icon(".len()..];
                let Some(end) = after.find(')') else {
                    return Ok(HandleOutcome::Done);
                };
                let icon = after[..end].trim();
                if icon.is_empty() {
                    return Ok(HandleOutcome::Done);
                }
                let icon_leading = after[..end].len() - after[..end].trim_start().len();
                let selection_start = line_start + rest_offset + "::icon(".len() + icon_leading;
                out.directive_prefixes.push("::icon".to_string());
                out.events
                    .push(MindmapParsedEvent::Icon(MindmapParsedPayloadLine {
                        value: icon.to_string(),
                        span: statement_span,
                        selection: SourceSpan::new(selection_start, selection_start + icon.len()),
                    }));
                return Ok(HandleOutcome::Done);
            }

            if let Some(after) = rest.strip_prefix(":::") {
                // Mermaid mindmap does not treat `%% ...` as an inline comment inside `:::` class
                // directives (the entire remainder is interpreted as space-separated class names).
                let statement_span = SourceSpan::new(
                    line_start + rest_offset,
                    line_start + rest_offset + rest.len(),
                );
                let class = after.trim();
                if class.is_empty() {
                    return Ok(HandleOutcome::Done);
                }
                let class_leading = after.len() - after.trim_start().len();
                let selection_start = line_start + rest_offset + ":::".len() + class_leading;
                out.directive_prefixes.push(":::".to_string());
                out.events
                    .push(MindmapParsedEvent::Class(MindmapParsedPayloadLine {
                        value: class.to_string(),
                        span: statement_span,
                        selection: SourceSpan::new(selection_start, selection_start + class.len()),
                    }));
                return Ok(HandleOutcome::Done);
            }

            let rest = strip_inline_comment(rest).trim_end();
            if rest.is_empty() {
                return Ok(HandleOutcome::Done);
            }

            let NodeSpec {
                id_raw,
                descr_raw,
                ty,
                descr_is_markdown,
                id_span,
                payload_span,
            } = match parse_node_spec(rest) {
                Ok(v) => v,
                Err(message) if message == "unterminated node delimiter" => {
                    return Ok(HandleOutcome::NeedMoreInput);
                }
                Err(message) => {
                    if recover {
                        out.completeness = EditorSemanticCompleteness::Recovered;
                        out.diagnostics.push(EditorSemanticDiagnostic::new(
                            format!("mindmap parser recovered from {message}"),
                            Some(SourceSpan::new(
                                line_start + rest_offset,
                                line_start + rest_offset + rest.len(),
                            )),
                        ));
                        return Ok(HandleOutcome::Done);
                    }
                    return Err(Error::diagram_parse_fallback(
                        meta.diagram_type.clone(),
                        message,
                    ));
                }
            };
            let span = SourceSpan::new(
                line_start + rest_offset,
                line_start + rest_offset + rest.len(),
            );
            out.events
                .push(MindmapParsedEvent::Node(MindmapParsedNodeLine {
                    indent,
                    id_raw,
                    descr_raw,
                    descr_is_markdown,
                    ty,
                    span,
                    selection: SourceSpan::new(
                        line_start + rest_offset + id_span.start,
                        line_start + rest_offset + id_span.end,
                    ),
                    payload_span: payload_span.map(|span| {
                        SourceSpan::new(
                            line_start + rest_offset + span.start,
                            line_start + rest_offset + span.end,
                        )
                    }),
                }));
            Ok(HandleOutcome::Done)
        };

    struct PendingMindmapLine {
        text: String,
        start: usize,
    }

    let mut pending: Option<PendingMindmapLine> = None;
    let mut push_and_try =
        |physical_line: &str, line_start: usize, out: &mut MindmapParsedLines| -> Result<()> {
            match pending.as_mut() {
                Some(PendingMindmapLine { text, .. }) => {
                    let buf = text;
                    buf.push('\n');
                    buf.push_str(physical_line);
                }
                None => {
                    pending = Some(PendingMindmapLine {
                        text: physical_line.to_string(),
                        start: line_start,
                    })
                }
            }

            let (current, current_start) = pending
                .as_ref()
                .map(|p| (p.text.as_str(), p.start))
                .unwrap_or(("", line_start));
            match handle_line(current, current_start, out)? {
                HandleOutcome::Done => {
                    pending = None;
                }
                HandleOutcome::NeedMoreInput => {}
            }
            Ok(())
        };

    if let Some(tail) = &header_tail {
        push_and_try(tail, header_tail_offset, &mut out)?;
    }
    for line in lines {
        let line_start = offset;
        offset += line.len();
        let line_no_newline = line.strip_suffix('\n').unwrap_or(line);
        push_and_try(line_no_newline, line_start, &mut out)?;
    }
    if let Some(PendingMindmapLine { text, start }) = pending {
        let line = strip_inline_comment(&text);
        if !line.trim().is_empty() {
            if recover {
                out.completeness = EditorSemanticCompleteness::Recovered;
                let leading = line.len().saturating_sub(line.trim_start().len());
                let trimmed = line.trim_end();
                out.diagnostics.push(EditorSemanticDiagnostic::new(
                    "mindmap parser recovered from unterminated node delimiter",
                    Some(SourceSpan::new(start + leading, start + trimmed.len())),
                ));
                return Ok(out);
            }
            return Err(Error::diagram_parse_fallback(
                meta.diagram_type.clone(),
                "unterminated node delimiter".to_string(),
            ));
        }
    }

    Ok(out)
}
