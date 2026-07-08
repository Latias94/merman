use serde_json::{Map, Value, json};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticCompleteness,
    EditorSemanticDiagnostic, EditorSemanticFacts, EditorSemanticKind, EditorSemanticSymbol, Error,
    ParseMetadata, Result, SourceSpan,
};

use super::db::{MindmapDb, MindmapParseConfig};
use super::render_model::MindmapDiagramRenderModel;
use super::utils::{NodeSpec, parse_node_spec, strip_inline_comment};
use crate::diagrams::scan::{split_indent, starts_with_case_insensitive};

static MINDMAP_DIAGRAM_ID_COUNTER: AtomicU64 = AtomicU64::new(0);

pub fn parse_mindmap(code: &str, meta: &ParseMetadata) -> Result<Value> {
    parse_mindmap_impl(code, meta)
}

pub fn parse_mindmap_model_for_render(
    code: &str,
    meta: &ParseMetadata,
) -> Result<MindmapDiagramRenderModel> {
    let mut db = parse_mindmap_db(code, meta)?;
    let Some(root_id) = db.get_mindmap().map(|n| n.id) else {
        return Ok(MindmapDiagramRenderModel::default());
    };

    db.assign_sections(root_id, None);

    Ok(MindmapDiagramRenderModel {
        nodes: db.to_layout_nodes_for_render(root_id, &meta.effective_config),
        edges: db.to_edges_for_render(root_id, &meta.effective_config),
    })
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

fn parse_mindmap_db(code: &str, meta: &ParseMetadata) -> Result<MindmapDb> {
    let mut db = MindmapDb::default();
    db.clear();
    let parse_config = MindmapParseConfig::from_config(&meta.effective_config);
    let parsed = parse_mindmap_lines(code, meta, false)?;

    for event in parsed.events {
        match event {
            MindmapParsedEvent::Node(node) => {
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
                )?;
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
    let parsed = match parse_mindmap_lines(code, meta, true) {
        Ok(parsed) => parsed,
        Err(_) => return EditorSemanticFacts::new(),
    };
    let mut facts = EditorSemanticFacts {
        completeness: parsed.completeness,
        span_coordinate_space: Default::default(),
        symbols: Vec::new(),
        directive_prefixes: Vec::new(),
        diagnostics: parsed.diagnostics,
        expected_syntax: Vec::new(),
    };
    for prefix in parsed.directive_prefixes {
        facts.push_directive_prefix(prefix);
    }
    for event in parsed.events {
        match event {
            MindmapParsedEvent::Node(node) => {
                let MindmapParsedNodeLine {
                    indent: _,
                    id_raw,
                    descr_raw,
                    descr_is_markdown: _,
                    ty: _,
                    span,
                    selection,
                    payload_span,
                } = node;
                facts.push_expected_syntax(EditorExpectedSyntax::new(
                    EditorExpectedSyntaxKind::NodeIdentifier,
                    selection,
                ));
                if let Some(payload_span) = payload_span {
                    facts.push_expected_syntax(EditorExpectedSyntax::new(
                        EditorExpectedSyntaxKind::Payload,
                        payload_span,
                    ));
                    facts.push_symbol(EditorSemanticSymbol::payload(
                        descr_raw,
                        Some("mindmap node label".to_string()),
                        EditorSemanticKind::String,
                        payload_span,
                        payload_span,
                    ));
                }
                facts.push_symbol(EditorSemanticSymbol::new(
                    id_raw,
                    Some("mindmap node".to_string()),
                    EditorSemanticKind::Namespace,
                    span,
                    selection,
                ));
            }
            MindmapParsedEvent::Class(class) => {
                facts.push_symbol(EditorSemanticSymbol::payload(
                    class.value,
                    Some("mindmap class".to_string()),
                    EditorSemanticKind::Property,
                    class.span,
                    class.selection,
                ));
            }
            MindmapParsedEvent::Icon(icon) => {
                facts.push_symbol(EditorSemanticSymbol::payload(
                    icon.value,
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

fn parse_mindmap_impl(code: &str, meta: &ParseMetadata) -> Result<Value> {
    let mut db = parse_mindmap_db(code, meta)?;

    let Some(root_id) = db.get_mindmap().map(|n| n.id) else {
        let mut final_config =
            crate::config::clone_value_nonrecursive(meta.effective_config.as_value());
        if meta.config.as_value().get("layout").is_none()
            && let Some(obj) = final_config.as_object_mut()
        {
            obj.insert(
                "layout".to_string(),
                Value::String("cose-bilkent".to_string()),
            );
        }

        let mut out = Map::with_capacity(3);
        out.insert("nodes".to_string(), Value::Array(Vec::new()));
        out.insert("edges".to_string(), Value::Array(Vec::new()));
        out.insert("config".to_string(), final_config);
        return Ok(Value::Object(out));
    };

    db.assign_sections(root_id, None);

    let nodes = db.to_layout_node_values(root_id, &meta.effective_config);
    let edges = db.to_edge_values(root_id, &meta.effective_config);

    let mut final_config =
        crate::config::clone_value_nonrecursive(meta.effective_config.as_value());
    if meta.config.as_value().get("layout").is_none()
        && let Some(obj) = final_config.as_object_mut()
    {
        obj.insert(
            "layout".to_string(),
            Value::String("cose-bilkent".to_string()),
        );
    }

    let mut shapes = Map::new();
    for n in nodes.iter() {
        let Some(node) = n.as_object() else {
            continue;
        };
        let Some(id) = node.get("id").and_then(|v| v.as_str()) else {
            continue;
        };
        let shape = node.get("shape").cloned().unwrap_or(Value::Null);
        let width = node.get("width").cloned().unwrap_or(Value::Null);
        let height = node.get("height").cloned().unwrap_or(Value::Null);
        let padding = node.get("padding").cloned().unwrap_or(Value::Null);
        shapes.insert(
            id.to_string(),
            json!({
                "shape": shape,
                "width": width,
                "height": height,
                "padding": padding,
            }),
        );
    }

    let diagram_id = MINDMAP_DIAGRAM_ID_COUNTER.fetch_add(1, Ordering::Relaxed) + 1;

    let mut out = Map::new();
    out.insert("type".to_string(), Value::String(meta.diagram_type.clone()));
    out.insert("nodes".to_string(), Value::Array(nodes));
    out.insert("edges".to_string(), Value::Array(edges));
    out.insert("config".to_string(), final_config);
    out.insert("rootNode".to_string(), db.to_root_node_value(root_id));
    out.insert(
        "markers".to_string(),
        Value::Array(vec![Value::String("point".to_string())]),
    );
    out.insert("direction".to_string(), Value::String("TB".to_string()));
    out.insert("nodeSpacing".to_string(), json!(50));
    out.insert("rankSpacing".to_string(), json!(50));
    out.insert("shapes".to_string(), Value::Object(shapes));
    // Mermaid uses a random UUID v4 here. For performance and determinism, keep a cheap
    // monotonic id that is unique within the current process. Snapshot tests normalize this
    // field to "<dynamic>".
    out.insert(
        "diagramId".to_string(),
        Value::String(format!("mindmap-{diagram_id}")),
    );
    Ok(Value::Object(out))
}
