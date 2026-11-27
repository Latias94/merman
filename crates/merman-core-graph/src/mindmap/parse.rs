use super::db::{MindmapDb, MindmapNodeInput};
use super::utils::{
    NodeSpec, NodeSpecContinuation, is_statement_terminator, node_closing_delimiter_at_start,
    parse_node_spec, starts_node_spec, strip_inline_comment,
};
use merman_core_preprocess::scan::starts_with_case_insensitive;
use merman_model::config::MermaidConfig;
use merman_model::families::mindmap::MindmapDiagramModel;
use merman_model::{OperationControl, OperationControlResult, SourceSpan};

#[derive(Debug)]
pub enum MindmapSourceIssue {
    Exact {
        message: String,
        span: SourceSpan,
    },
    InsertionPoint {
        message: &'static str,
        offset: usize,
    },
}

/// The recovered events from one source scan and their first syntax issue.
pub struct MindmapSyntax {
    parsed: MindmapParsedLines,
    first_error: Option<MindmapSourceIssue>,
}

/// A validated tree built from one Mindmap statement stream.
pub struct MindmapSource(MindmapDiagramModel);

impl MindmapSource {
    pub fn into_model(self) -> MindmapDiagramModel {
        self.0
    }

    #[cfg(feature = "presentation")]
    pub fn into_render_model_controlled(
        self,
        config: &MermaidConfig,
        control: &OperationControl,
    ) -> OperationControlResult<merman_model::families::mindmap::MindmapDiagramRenderModel> {
        super::render_model::model_into_render_model_controlled(self.0, config, control)
    }
}

/// A borrowed occurrence in the parser input's byte coordinates.
#[cfg(feature = "editor-facts")]
pub enum MindmapObservation<'statement> {
    DirectivePrefix(&'statement str),
    Node {
        id: &'statement str,
        span: SourceSpan,
        selection: SourceSpan,
        label: Option<(&'statement str, SourceSpan)>,
    },
    Class {
        text: &'statement str,
        span: SourceSpan,
        selection: SourceSpan,
    },
    Icon {
        text: &'statement str,
        span: SourceSpan,
        selection: SourceSpan,
    },
}

impl MindmapSyntax {
    /// Visits the captured editor occurrences before grammar or database errors are returned.
    #[cfg(feature = "editor-facts")]
    pub fn observe_editor_facts(
        &self,
        observe: &mut dyn FnMut(MindmapObservation<'_>),
        control: &OperationControl,
    ) -> OperationControlResult<()> {
        for (index, prefix) in self.parsed.directive_prefixes.iter().enumerate() {
            if index % 128 == 0 {
                control.checkpoint()?;
            }
            observe(MindmapObservation::DirectivePrefix(prefix));
        }
        for (index, event) in self.parsed.events.iter().enumerate() {
            if index % 128 == 0 {
                control.checkpoint()?;
            }
            match event {
                MindmapParsedEvent::Node(node) => observe(MindmapObservation::Node {
                    id: &node.id_raw,
                    span: node.span,
                    selection: node.selection,
                    label: node
                        .payload_span
                        .map(|span| (node.descr_raw.as_str(), span)),
                }),
                MindmapParsedEvent::Class(class) => {
                    for (index, token) in class.tokens.iter().enumerate() {
                        if index % 128 == 0 {
                            control.checkpoint()?;
                        }
                        observe(MindmapObservation::Class {
                            text: &token.value,
                            span: class.span,
                            selection: token.selection,
                        });
                    }
                }
                MindmapParsedEvent::Icon(icon) => {
                    for (index, token) in icon.tokens.iter().enumerate() {
                        if index % 128 == 0 {
                            control.checkpoint()?;
                        }
                        observe(MindmapObservation::Icon {
                            text: &token.value,
                            span: icon.span,
                            selection: token.selection,
                        });
                    }
                }
            }
        }
        control.checkpoint()
    }

    /// Consumes the events and constructs the canonical tree after syntax validation.
    pub fn into_source(
        self,
        config: &MermaidConfig,
        control: &OperationControl,
    ) -> OperationControlResult<Result<MindmapSource, MindmapSourceIssue>> {
        if let Some(error) = self.first_error {
            return Ok(Err(error));
        }
        let db = match mindmap_db_from_events(self.parsed.events, config, control)? {
            Ok(db) => db,
            Err(error) => return Ok(Err(error)),
        };
        control.checkpoint()?;
        Ok(Ok(MindmapSource(db.into_semantic_model())))
    }
}

fn mindmap_db_from_events(
    events: Vec<MindmapParsedEvent>,
    config: &MermaidConfig,
    control: &OperationControl,
) -> OperationControlResult<Result<MindmapDb, MindmapSourceIssue>> {
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
                    MindmapNodeInput {
                        indent_level: node.indent as i32,
                        id_raw: &node.id_raw,
                        descr_raw: &node.descr_raw,
                        descr_is_markdown: node.descr_is_markdown,
                        ty: node.ty,
                    },
                    config,
                    control,
                )? {
                    return Ok(Err(MindmapSourceIssue::Exact {
                        message: error,
                        span: selection,
                    }));
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

fn split_indent(line: &str) -> (usize, &str) {
    let mut indent = 0;
    let mut start = line.len();
    for (offset, ch) in line.char_indices() {
        if ch.is_whitespace() {
            indent += 1;
            continue;
        }
        start = offset;
        break;
    }
    (indent, &line[start..])
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
    first_error: &mut Option<MindmapSourceIssue>,
    message: impl Into<String>,
    span: SourceSpan,
) {
    first_error.get_or_insert_with(|| MindmapSourceIssue::Exact {
        message: message.into(),
        span,
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
    first_error: &mut Option<MindmapSourceIssue>,
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
            record_mindmap_error(first_error, "mindmap class is empty", statement_span);
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

    match parse_node_spec(
        rest,
        #[cfg(feature = "editor-facts")]
        parsed.capture_editor,
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
            record_mindmap_error(first_error, error.message, statement_span);
            MindmapLineOutcome::Done
        }
    }
}

fn finish_pending_mindmap_line(
    pending: PendingMindmapLine,
    first_error: &mut Option<MindmapSourceIssue>,
) {
    let MindmapPendingSyntax {
        statement_span,
        message,
        ..
    } = pending.syntax;
    record_mindmap_error(first_error, message, statement_span);
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
    first_error: &mut Option<MindmapSourceIssue>,
) {
    if pending
        .as_ref()
        .is_some_and(|current| pending_mindmap_line_should_synchronize(current, physical_line))
    {
        let current = pending.take().expect("checked pending mindmap line");
        finish_pending_mindmap_line(current, first_error);
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
        match handle_mindmap_line(&current.text, current.start, parsed, first_error) {
            MindmapLineOutcome::Done => {}
            MindmapLineOutcome::NeedMoreInput(syntax) => {
                current.syntax = syntax;
                *pending = Some(current);
            }
        }
        return;
    }

    if let MindmapLineOutcome::NeedMoreInput(syntax) =
        handle_mindmap_line(physical_line, line_start, parsed, first_error)
    {
        *pending = Some(PendingMindmapLine {
            text: physical_line.to_string(),
            start: line_start,
            syntax,
        });
    }
}

/// Scans source once, retaining recovered events and the first syntax issue.
pub fn parse_mindmap_syntax_controlled(
    code: &str,
    control: &OperationControl,
) -> OperationControlResult<MindmapSyntax> {
    scan_mindmap_syntax(
        code,
        #[cfg(feature = "editor-facts")]
        false,
        control,
    )
}

/// Scans the same source and optionally retains editor facts for later replay.
#[cfg(feature = "editor-facts")]
pub fn parse_mindmap_syntax_observed_controlled(
    code: &str,
    capture_editor: bool,
    control: &OperationControl,
) -> OperationControlResult<MindmapSyntax> {
    scan_mindmap_syntax(code, capture_editor, control)
}

fn scan_mindmap_syntax(
    code: &str,
    #[cfg(feature = "editor-facts")] capture_editor: bool,
    control: &OperationControl,
) -> OperationControlResult<MindmapSyntax> {
    control.checkpoint()?;
    let mut lines = code.split_inclusive('\n').peekable();
    let mut offset = 0usize;
    let mut parsed = MindmapParsedLines {
        #[cfg(feature = "editor-facts")]
        capture_editor,
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
            "expected mindmap header",
            SourceSpan::new(keyword_start, keyword_start + trimmed.len()),
        );
        break;
    }

    if !found_header {
        if first_error.is_none() {
            first_error = Some(MindmapSourceIssue::InsertionPoint {
                message: "expected mindmap header",
                offset: code.len(),
            });
        }
        return Ok(MindmapSyntax {
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
        );
    }
    if let Some(pending) = pending {
        finish_pending_mindmap_line(pending, &mut first_error);
    }

    control.checkpoint()?;
    Ok(MindmapSyntax {
        parsed,
        first_error,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_shape_uses_the_actual_generic_closing_token() {
        use merman_model::families::mindmap::{
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
            let control = OperationControl::new();
            let model = parse_mindmap_syntax_controlled(&input, &control)
                .unwrap()
                .into_source(&MermaidConfig::empty_object(), &control)
                .unwrap()
                .unwrap_or_else(|error| panic!("{source}: {error:?}"))
                .into_model();
            assert_eq!(model.nodes[0].descr, label, "{source}");
            assert_eq!(model.nodes[0].node_type, node_type, "{source}");
        }
    }
}
