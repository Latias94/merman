use crate::diagrams::sequence::{
    SequenceActionBuilder, SequenceControlKind, SequenceMessageKind, SequenceParticipantKind,
};
use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
    EditorSemanticSymbol, Error, ParseMetadata, Result, SourceSpan,
};
use serde_json::Value;

/// Parses a ZenUML diagram into a Mermaid-like semantic model.
///
/// Upstream Mermaid integrates ZenUML via the `mermaid-zenuml` external diagram package, which
/// uses `@zenuml/core` in the browser. `merman` is headless and pure Rust, so for now we implement
/// a conservative compatibility mode: a small ZenUML subset lowers directly into the Sequence
/// semantic action model.
///
/// Rendering and editor facts share one source pass so the supported subset and LSP ranges cannot
/// drift into separate grammars.
pub fn parse_zenuml(code: &str, meta: &ParseMetadata) -> Result<Value> {
    Ok(parse_zenuml_semantic_source(code, meta)?.compat_json(meta))
}

pub fn parse_zenuml_model_for_render(
    code: &str,
    meta: &ParseMetadata,
) -> Result<crate::diagrams::sequence::SequenceDiagramRenderModel> {
    Ok(parse_zenuml_semantic_source(code, meta)?.model)
}

#[derive(Debug, Clone)]
struct ZenumlSpannedText {
    text: String,
    span: SourceSpan,
}

struct ZenumlSemanticSource {
    model: crate::diagrams::sequence::SequenceDiagramRenderModel,
    editor_facts: EditorSemanticFacts,
}

struct ZenumlSemanticFailure {
    error: Box<Error>,
    message: String,
    span: Option<SourceSpan>,
    editor_facts: Box<EditorSemanticFacts>,
}

impl ZenumlSemanticFailure {
    fn into_editor_facts(self) -> EditorSemanticFacts {
        let mut editor_facts = *self.editor_facts;
        editor_facts.mark_recovered_from_parse_error(
            format!(
                "zenuml parser recovered after parse error: {}",
                self.message
            ),
            self.span,
        );
        editor_facts
    }
}

struct ZenumlTranslation {
    sequence: SequenceActionBuilder,
    editor_facts: EditorSemanticFacts,
    first_error: Option<ZenumlSyntaxDiagnostic>,
}

struct ZenumlSyntaxDiagnostic {
    message: String,
    span: SourceSpan,
}

impl ZenumlSemanticSource {
    fn compat_json(&self, meta: &ParseMetadata) -> Value {
        self.model.to_compat_json(&meta.diagram_type)
    }
}

pub(crate) fn parse_zenuml_json_and_editor_facts(
    code: &str,
    meta: &ParseMetadata,
) -> Result<(Value, EditorSemanticFacts)> {
    let source = parse_zenuml_semantic_source(code, meta)?;
    let compat = source.compat_json(meta);
    Ok((compat, source.editor_facts))
}

pub fn parse_zenuml_editor_facts(code: &str, meta: &ParseMetadata) -> EditorSemanticFacts {
    match construct_zenuml_semantic_source(code, meta) {
        Ok(source) => source.editor_facts,
        Err(failure) => failure.into_editor_facts(),
    }
}

fn parse_zenuml_semantic_source(code: &str, meta: &ParseMetadata) -> Result<ZenumlSemanticSource> {
    construct_zenuml_semantic_source(code, meta).map_err(|failure| *failure.error)
}

fn construct_zenuml_semantic_source(
    code: &str,
    meta: &ParseMetadata,
) -> std::result::Result<ZenumlSemanticSource, ZenumlSemanticFailure> {
    #[cfg(test)]
    crate::diagrams::langium_common::record_family_syntax_construction("zenuml");

    let ZenumlTranslation {
        sequence,
        editor_facts,
        first_error,
    } = translate_zenuml_syntax(code);
    if let Some(diagnostic) = first_error {
        return Err(zenuml_failure(
            meta,
            diagnostic.message,
            Some(diagnostic.span),
            editor_facts,
        ));
    }

    match sequence.into_render_model(meta) {
        Ok(model) => Ok(ZenumlSemanticSource {
            model,
            editor_facts,
        }),
        Err(message) => Err(zenuml_failure(meta, message, None, editor_facts)),
    }
}

fn zenuml_value_after_keyword(
    line: &str,
    keyword: &str,
    stmt_start: usize,
) -> Option<ZenumlSpannedText> {
    let after_keyword = line.strip_prefix(keyword)?;
    let separator_len = after_keyword
        .chars()
        .next()
        .filter(|ch| ch.is_whitespace())?
        .len_utf8();
    let rest = &after_keyword[separator_len..];
    zenuml_trimmed_spanned(rest, stmt_start + keyword.len() + separator_len)
}

fn zenuml_value_after_keyword_ci(
    line: &str,
    keyword: &str,
    stmt_start: usize,
) -> Option<ZenumlSpannedText> {
    let prefix = line.get(0..keyword.len())?;
    if !prefix.eq_ignore_ascii_case(keyword) {
        return None;
    }
    zenuml_value_after_keyword(line, prefix, stmt_start)
}

fn parse_zenuml_creation(line: &str, stmt_start: usize) -> Option<ZenumlSpannedText> {
    let rest = line.strip_prefix("new ")?;
    parse_zenuml_identifier(rest, stmt_start + "new ".len())
}

struct ZenumlParticipantDecl {
    sequence_id: String,
    sequence_description: Option<String>,
    sequence_kind: SequenceParticipantKind,
    entity: ZenumlSpannedText,
    label: Option<ZenumlSpannedText>,
}

fn parse_zenuml_participant_decl(line: &str, stmt_start: usize) -> Option<ZenumlParticipantDecl> {
    if let Some(rest) = line.strip_prefix('@') {
        let kind_len = rest.find(char::is_whitespace).unwrap_or(rest.len());
        let kind = &rest[..kind_len];
        let after_kind = &rest[kind_len..];
        let name = zenuml_trimmed_spanned(after_kind, stmt_start + 1 + kind_len)?;
        let sequence_kind = if kind.eq_ignore_ascii_case("actor") {
            SequenceParticipantKind::Actor
        } else {
            SequenceParticipantKind::Participant
        };
        return Some(ZenumlParticipantDecl {
            sequence_id: name.text.clone(),
            sequence_description: None,
            sequence_kind,
            entity: name,
            label: None,
        });
    }

    if let Some((id, label)) = split_zenuml_alias_decl(line, stmt_start) {
        return Some(ZenumlParticipantDecl {
            sequence_id: label.text.clone(),
            sequence_description: Some(id.text.clone()),
            sequence_kind: SequenceParticipantKind::Participant,
            entity: id,
            label: Some(label),
        });
    }

    if line
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
    {
        return Some(ZenumlParticipantDecl {
            sequence_id: line.to_string(),
            sequence_description: None,
            sequence_kind: SequenceParticipantKind::Participant,
            entity: ZenumlSpannedText {
                text: line.to_string(),
                span: SourceSpan::new(stmt_start, stmt_start + line.len()),
            },
            label: None,
        });
    }

    None
}

fn push_zenuml_participant_facts(
    facts: &mut EditorSemanticFacts,
    participant: &ZenumlParticipantDecl,
) {
    let detail = if participant.label.is_some() {
        "zenuml participant alias"
    } else {
        "zenuml participant"
    };
    push_zenuml_entity(
        facts,
        &participant.entity,
        detail,
        EditorSemanticKind::Event,
    );
    if let Some(label) = participant.label.as_ref() {
        push_zenuml_payload(
            facts,
            label,
            "zenuml participant label",
            EditorSemanticKind::String,
        );
    }
}

fn split_zenuml_alias_decl(
    line: &str,
    stmt_start: usize,
) -> Option<(ZenumlSpannedText, ZenumlSpannedText)> {
    let separator = line.find(" as ")?;
    let id_raw = &line[..separator];
    let label_start = separator + " as ".len();
    let label_raw = &line[label_start..];
    let id = zenuml_trimmed_spanned(id_raw, stmt_start)?;
    let label = zenuml_trimmed_spanned(label_raw, stmt_start + label_start)?;
    Some((id, label))
}

struct ZenumlAssignment {
    target: ZenumlSpannedText,
    actor: ZenumlSpannedText,
    method: ZenumlSpannedText,
}

fn parse_zenuml_assignment(line: &str, stmt_start: usize) -> Option<ZenumlAssignment> {
    let eq = line.find('=')?;
    let rhs = line[eq + 1..].trim_start();
    let rhs_start = stmt_start + eq + 1 + line[eq + 1..].len() - rhs.len();
    let (actor, method) = parse_zenuml_method_call(rhs, rhs_start)?;
    let var = line[..eq].split_whitespace().last()?;
    let rel = line[..eq].rfind(var)?;
    Some(ZenumlAssignment {
        target: ZenumlSpannedText {
            text: var.to_string(),
            span: SourceSpan::new(stmt_start + rel, stmt_start + rel + var.len()),
        },
        actor,
        method,
    })
}

fn push_zenuml_assignment_facts(facts: &mut EditorSemanticFacts, assignment: &ZenumlAssignment) {
    push_zenuml_entity(
        facts,
        &assignment.actor,
        "zenuml participant reference",
        EditorSemanticKind::Event,
    );
    push_zenuml_payload(
        facts,
        &assignment.method,
        "zenuml message",
        EditorSemanticKind::String,
    );
    push_zenuml_payload(
        facts,
        &assignment.target,
        "zenuml assignment target",
        EditorSemanticKind::Variable,
    );
}

struct ZenumlMessage {
    from: ZenumlSpannedText,
    to: ZenumlSpannedText,
    label: Option<ZenumlSpannedText>,
    reply: bool,
}

fn parse_zenuml_message(line: &str, stmt_start: usize) -> Option<ZenumlMessage> {
    let (lhs, label) = if let Some(colon) = line.find(':') {
        (
            &line[..colon],
            zenuml_trimmed_spanned(&line[colon + 1..], stmt_start + colon + 1),
        )
    } else {
        (line, None)
    };

    let (arrow_start, arrow_len, reply) = if let Some(index) = lhs.find("-->") {
        (index, "-->".len(), true)
    } else {
        let index = lhs.find("->")?;
        (index, "->".len(), false)
    };
    let from = zenuml_trimmed_spanned(&lhs[..arrow_start], stmt_start)?;
    let to_start = arrow_start + arrow_len;
    let to = zenuml_trimmed_spanned(&lhs[to_start..], stmt_start + to_start)?;
    Some(ZenumlMessage {
        from,
        to,
        label,
        reply,
    })
}

fn push_zenuml_message_facts(facts: &mut EditorSemanticFacts, message: &ZenumlMessage) {
    push_zenuml_entity(
        facts,
        &message.from,
        "zenuml participant reference",
        EditorSemanticKind::Event,
    );
    push_zenuml_entity(
        facts,
        &message.to,
        "zenuml participant reference",
        EditorSemanticKind::Event,
    );
    if let Some(label) = message.label.as_ref() {
        push_zenuml_payload(facts, label, "zenuml message", EditorSemanticKind::String);
    }
}

fn parse_zenuml_method_call(
    line: &str,
    stmt_start: usize,
) -> Option<(ZenumlSpannedText, ZenumlSpannedText)> {
    let separator = line.find('.')?;
    let actor_raw = &line[..separator];
    let method_start = separator + '.'.len_utf8();
    let method_raw = &line[method_start..];
    let actor = zenuml_trimmed_spanned(actor_raw, stmt_start)?;
    let method = zenuml_trimmed_spanned(method_raw, stmt_start + method_start)?;
    Some((actor, method))
}

fn parse_zenuml_identifier(input: &str, input_start: usize) -> Option<ZenumlSpannedText> {
    let trimmed = input.trim_start();
    let leading = input.len().saturating_sub(trimmed.len());
    let mut end = 0usize;
    for (idx, ch) in trimmed.char_indices() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '.' {
            end = idx + ch.len_utf8();
            continue;
        }
        break;
    }
    if end == 0 {
        return None;
    }
    Some(ZenumlSpannedText {
        text: trimmed[..end].to_string(),
        span: SourceSpan::new(input_start + leading, input_start + leading + end),
    })
}

fn push_zenuml_payload_tail(
    facts: &mut EditorSemanticFacts,
    line: &str,
    stmt_start: usize,
    detail: &str,
) {
    let payload_start = line
        .find(|ch: char| ch.is_whitespace() || ch == '(')
        .unwrap_or(line.len());
    if payload_start >= line.len() {
        return;
    }
    if let Some(payload) =
        zenuml_trimmed_spanned(&line[payload_start..], stmt_start + payload_start)
    {
        push_zenuml_payload(facts, &payload, detail, EditorSemanticKind::String);
    }
}

fn zenuml_trimmed_spanned(raw: &str, raw_start: usize) -> Option<ZenumlSpannedText> {
    let leading = raw.len().saturating_sub(raw.trim_start().len());
    let trailing = raw.trim_end().len();
    if leading >= trailing {
        return None;
    }
    Some(ZenumlSpannedText {
        text: raw[leading..trailing].to_string(),
        span: SourceSpan::new(raw_start + leading, raw_start + trailing),
    })
}

fn push_zenuml_entity(
    facts: &mut EditorSemanticFacts,
    text: &ZenumlSpannedText,
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
    facts.push_symbol(EditorSemanticSymbol::new(
        text.text.clone(),
        Some(detail.to_string()),
        kind,
        text.span,
        text.span,
    ));
}

fn push_zenuml_payload(
    facts: &mut EditorSemanticFacts,
    text: &ZenumlSpannedText,
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
        text.text.clone(),
        Some(detail.to_string()),
        kind,
        text.span,
        text.span,
    ));
}

fn record_zenuml_syntax_error(
    first_error: &mut Option<ZenumlSyntaxDiagnostic>,
    statement: &str,
    statement_start: usize,
) {
    if first_error.is_none() {
        *first_error = Some(ZenumlSyntaxDiagnostic {
            message: format!("unsupported zenuml statement: {statement}"),
            span: SourceSpan::new(statement_start, statement_start + statement.len()),
        });
    }
}

fn zenuml_failure(
    meta: &ParseMetadata,
    message: impl Into<String>,
    span: Option<SourceSpan>,
    editor_facts: EditorSemanticFacts,
) -> ZenumlSemanticFailure {
    let message = message.into();
    let error = match span {
        Some(span) => Error::diagram_parse_exact(meta.diagram_type.clone(), &message, span),
        None => Error::diagram_parse_fallback(meta.diagram_type.clone(), &message),
    };
    ZenumlSemanticFailure {
        error: Box::new(error),
        message,
        span,
        editor_facts: Box::new(editor_facts),
    }
}

fn translate_zenuml_syntax(code: &str) -> ZenumlTranslation {
    let mut sequence = SequenceActionBuilder::new();
    let mut editor_facts = EditorSemanticFacts::new();
    let mut first_error = None;

    let mut saw_header = false;
    let mut pending_comments: Vec<String> = Vec::new();
    let mut pending_return_annotator: bool = false;

    #[derive(Debug, Clone)]
    enum BlockKind {
        Loop,
        Opt,
        Par { branch_started: bool },
        IfAlt,
        TryAlt,
        SyncCall { actor: String },
    }

    fn starts_with_word_ci(haystack: &str, word: &str) -> bool {
        haystack
            .get(0..word.len())
            .is_some_and(|p| p.eq_ignore_ascii_case(word))
            && haystack
                .get(word.len()..word.len() + 1)
                .is_none_or(|c| c.chars().all(|ch| ch.is_ascii_whitespace() || ch == '('))
    }

    fn strip_trailing_open_brace(line: &str) -> Option<&str> {
        let trimmed = line.trim_end();
        trimmed.strip_suffix('{').map(str::trim_end)
    }

    fn flush_pending_comments_as_notes(
        pending: &mut Vec<String>,
        sequence: &mut SequenceActionBuilder,
        from: &str,
        to: &str,
    ) {
        if pending.is_empty() {
            return;
        }
        for c in pending.drain(..) {
            let text = c.trim();
            if text.is_empty() {
                continue;
            }
            sequence.note_over(from.to_string(), to.to_string(), text.to_string());
        }
    }

    let mut stack: Vec<BlockKind> = Vec::new();

    fn par_maybe_and(stack: &mut [BlockKind], sequence: &mut SequenceActionBuilder) {
        let Some(BlockKind::Par { branch_started }) = stack.last_mut() else {
            return;
        };
        if *branch_started {
            sequence.control(SequenceControlKind::ParAnd, Some(String::new()));
        } else {
            *branch_started = true;
        }
    }

    fn close_brace(rest: &str, stack: &mut Vec<BlockKind>, sequence: &mut SequenceActionBuilder) {
        let Some(top) = stack.last() else {
            return;
        };

        // For `if { ... } else { ... }` and `try { ... } catch { ... }`, the brace before the next
        // branch must *not* close the translated Mermaid fragment.
        match top {
            BlockKind::IfAlt if rest.starts_with("else") => {
                return;
            }
            BlockKind::TryAlt if (rest.starts_with("catch") || rest.starts_with("finally")) => {
                return;
            }
            BlockKind::SyncCall { .. } => {}
            _ => {}
        }

        let closed = stack.pop();
        match closed {
            Some(BlockKind::SyncCall { actor }) => {
                sequence.deactivate(actor);
            }
            Some(BlockKind::Loop) => {
                sequence.control(SequenceControlKind::LoopEnd, None);
            }
            Some(BlockKind::Opt) => {
                sequence.control(SequenceControlKind::OptEnd, None);
            }
            Some(BlockKind::Par { .. }) => {
                sequence.control(SequenceControlKind::ParEnd, None);
            }
            Some(BlockKind::IfAlt | BlockKind::TryAlt) => {
                sequence.control(SequenceControlKind::AltEnd, None);
            }
            None => {}
        }
    }

    let mut source_offset = 0usize;
    'lines: for segment in code.split_inclusive('\n') {
        let line_start = source_offset;
        source_offset += segment.len();
        let raw = segment.trim_end_matches(['\n', '\r']);
        let mut line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let mut statement_start = line_start + raw.find(line).unwrap_or_default();

        if !saw_header && line.to_ascii_lowercase().starts_with("zenuml") {
            saw_header = true;
            continue;
        }

        // ZenUML renders `// ...` comments above the following messages/fragments.
        // - a comment on a participant will not be rendered
        // - a comment on a message should be rendered
        if let Some(c) = line.strip_prefix("//") {
            pending_comments.push(c.trim().to_string());
            continue;
        }

        // ZenUML reply annotators:
        //   @return
        //   @reply
        //
        // These affect the next message. We approximate this by forcing the next message to use
        // a Mermaid-style "return" arrow (`-->>`) regardless of the original arrow.
        if line.eq_ignore_ascii_case("@return") || line.eq_ignore_ascii_case("@reply") {
            pending_return_annotator = true;
            continue;
        }

        // Handle leading close braces, including `} else {` and `} catch {` forms.
        loop {
            let trimmed = line.trim_start();
            if !trimmed.starts_with('}') {
                line = trimmed;
                break;
            }
            let after_brace = &trimmed[1..];
            let rest = after_brace.trim_start();
            statement_start += 1 + after_brace.len().saturating_sub(rest.len());
            close_brace(rest, &mut stack, &mut sequence);
            line = rest;
            if line.is_empty() {
                break;
            }
        }
        if line.is_empty() {
            continue;
        }

        // Pass through common metadata directives as-is when possible.
        if line.to_ascii_lowercase().starts_with("title ") {
            editor_facts.push_directive_prefix("title");
            if let Some(value) = zenuml_value_after_keyword_ci(line, "title", statement_start) {
                push_zenuml_payload(
                    &mut editor_facts,
                    &value,
                    "zenuml title",
                    EditorSemanticKind::String,
                );
                sequence.set_title(value.text);
            }
            pending_comments.clear();
            continue;
        }
        if line.to_ascii_lowercase().starts_with("acctitle ") {
            editor_facts.push_directive_prefix("accTitle");
            if let Some(value) = zenuml_value_after_keyword_ci(line, "accTitle", statement_start) {
                push_zenuml_payload(
                    &mut editor_facts,
                    &value,
                    "zenuml accessibility title",
                    EditorSemanticKind::String,
                );
                sequence.set_acc_title(value.text);
            }
            pending_comments.clear();
            continue;
        }
        if line.to_ascii_lowercase().starts_with("accdescr ") {
            editor_facts.push_directive_prefix("accDescr");
            if let Some(value) = zenuml_value_after_keyword_ci(line, "accDescr", statement_start) {
                push_zenuml_payload(
                    &mut editor_facts,
                    &value,
                    "zenuml accessibility description",
                    EditorSemanticKind::String,
                );
                sequence.set_acc_descr(value.text);
            }
            pending_comments.clear();
            continue;
        }

        // Branch continuations for `if` and `try` structures.
        if let Some(prefix) = strip_trailing_open_brace(line) {
            let p = prefix.trim();
            if starts_with_word_ci(p, "else if") {
                let Some((_, cond)) = p.split_once('(') else {
                    record_zenuml_syntax_error(&mut first_error, line, statement_start);
                    continue 'lines;
                };
                let Some((cond, _)) = cond.rsplit_once(')') else {
                    record_zenuml_syntax_error(&mut first_error, line, statement_start);
                    continue 'lines;
                };
                push_zenuml_payload_tail(
                    &mut editor_facts,
                    p,
                    statement_start,
                    "zenuml else if payload",
                );
                let label = format!("if({})", cond.trim());
                sequence.control(SequenceControlKind::AltElse, Some(label));
                pending_comments.clear();
                continue;
            }
            if starts_with_word_ci(p, "else") {
                sequence.control(SequenceControlKind::AltElse, Some(String::new()));
                pending_comments.clear();
                continue;
            }
            if starts_with_word_ci(p, "catch") {
                push_zenuml_payload_tail(
                    &mut editor_facts,
                    p,
                    statement_start,
                    "zenuml catch payload",
                );
                sequence.control(SequenceControlKind::AltElse, Some("catch".to_string()));
                pending_comments.clear();
                continue;
            }
            if starts_with_word_ci(p, "finally") {
                push_zenuml_payload_tail(
                    &mut editor_facts,
                    p,
                    statement_start,
                    "zenuml finally payload",
                );
                sequence.control(SequenceControlKind::AltElse, Some("finally".to_string()));
                pending_comments.clear();
                continue;
            }
        }

        // Block openings.
        if let Some(prefix) = strip_trailing_open_brace(line) {
            let p = prefix.trim();

            if starts_with_word_ci(p, "while") {
                push_zenuml_payload_tail(
                    &mut editor_facts,
                    p,
                    statement_start,
                    "zenuml while payload",
                );
                par_maybe_and(&mut stack, &mut sequence);
                sequence.control(SequenceControlKind::LoopStart, Some(p.to_string()));
                stack.push(BlockKind::Loop);
                pending_comments.clear();
                continue;
            }
            if starts_with_word_ci(p, "for")
                || starts_with_word_ci(p, "foreach")
                || starts_with_word_ci(p, "loop")
            {
                push_zenuml_payload_tail(
                    &mut editor_facts,
                    p,
                    statement_start,
                    "zenuml loop payload",
                );
                par_maybe_and(&mut stack, &mut sequence);
                sequence.control(SequenceControlKind::LoopStart, Some(p.to_string()));
                stack.push(BlockKind::Loop);
                pending_comments.clear();
                continue;
            }
            if starts_with_word_ci(p, "opt") {
                push_zenuml_payload_tail(
                    &mut editor_facts,
                    p,
                    statement_start,
                    "zenuml opt payload",
                );
                par_maybe_and(&mut stack, &mut sequence);
                let label = p.strip_prefix("opt").unwrap_or("").trim();
                sequence.control(SequenceControlKind::OptStart, Some(label.to_string()));
                stack.push(BlockKind::Opt);
                pending_comments.clear();
                continue;
            }
            if starts_with_word_ci(p, "par") {
                push_zenuml_payload_tail(
                    &mut editor_facts,
                    p,
                    statement_start,
                    "zenuml par payload",
                );
                par_maybe_and(&mut stack, &mut sequence);
                let label = p.strip_prefix("par").unwrap_or("").trim();
                sequence.control(SequenceControlKind::ParStart, Some(label.to_string()));
                stack.push(BlockKind::Par {
                    branch_started: false,
                });
                pending_comments.clear();
                continue;
            }
            if starts_with_word_ci(p, "if") {
                par_maybe_and(&mut stack, &mut sequence);
                let Some((_, cond)) = p.split_once('(') else {
                    record_zenuml_syntax_error(&mut first_error, line, statement_start);
                    continue 'lines;
                };
                let Some((cond, _)) = cond.rsplit_once(')') else {
                    record_zenuml_syntax_error(&mut first_error, line, statement_start);
                    continue 'lines;
                };
                push_zenuml_payload_tail(
                    &mut editor_facts,
                    p,
                    statement_start,
                    "zenuml if payload",
                );
                sequence.control(
                    SequenceControlKind::AltStart,
                    Some(format!("if({})", cond.trim())),
                );
                stack.push(BlockKind::IfAlt);
                pending_comments.clear();
                continue;
            }
            if starts_with_word_ci(p, "try") {
                par_maybe_and(&mut stack, &mut sequence);
                sequence.control(SequenceControlKind::AltStart, Some("try".to_string()));
                stack.push(BlockKind::TryAlt);
                pending_comments.clear();
                continue;
            }

            // Sync message / method-call blocks:
            //   A.SyncMessage(with, parameters) { ... }
            //
            // Translate to a self-message plus explicit activation scope.
            if let Some((actor, method)) = parse_zenuml_method_call(p, statement_start) {
                push_zenuml_entity(
                    &mut editor_facts,
                    &actor,
                    "zenuml participant reference",
                    EditorSemanticKind::Event,
                );
                push_zenuml_payload(
                    &mut editor_facts,
                    &method,
                    "zenuml message",
                    EditorSemanticKind::String,
                );
                par_maybe_and(&mut stack, &mut sequence);
                flush_pending_comments_as_notes(
                    &mut pending_comments,
                    &mut sequence,
                    &actor.text,
                    &actor.text,
                );
                sequence.message(
                    actor.text.clone(),
                    actor.text.clone(),
                    SequenceMessageKind::Solid,
                    method.text,
                );
                sequence.activate(actor.text.clone());
                stack.push(BlockKind::SyncCall { actor: actor.text });
                continue;
            }
        }

        // Creation messages:
        //   new A1
        //   new A2(with, parameters)
        if let Some(rest) = line.strip_prefix("new ") {
            let rest = rest.trim();
            let Some(created) = parse_zenuml_creation(line, statement_start) else {
                record_zenuml_syntax_error(&mut first_error, line, statement_start);
                continue 'lines;
            };
            push_zenuml_entity(
                &mut editor_facts,
                &created,
                "zenuml participant",
                EditorSemanticKind::Event,
            );
            push_zenuml_payload_tail(
                &mut editor_facts,
                line,
                statement_start,
                "zenuml creation payload",
            );

            par_maybe_and(&mut stack, &mut sequence);
            pending_comments.clear();

            // If the creation has arguments, keep the full text as the label (description).
            let description = (rest != created.text).then(|| rest.to_string());
            sequence.create_participant(
                created.text,
                description,
                SequenceParticipantKind::Participant,
            );
            continue;
        }

        // Participants.
        if let Some(decl) = parse_zenuml_participant_decl(line, statement_start) {
            push_zenuml_participant_facts(&mut editor_facts, &decl);
            par_maybe_and(&mut stack, &mut sequence);
            sequence.participant(
                decl.sequence_id,
                decl.sequence_description,
                decl.sequence_kind,
            );
            // ZenUML comment on a participant is not rendered.
            pending_comments.clear();
            continue;
        }

        // Reply assignments must be handled before generic `Actor.Method(...)` parsing, because
        // an assignment line contains a `.` and would otherwise be misinterpreted as a sync call.
        if let Some(assignment) = parse_zenuml_assignment(line, statement_start) {
            push_zenuml_assignment_facts(&mut editor_facts, &assignment);
            par_maybe_and(&mut stack, &mut sequence);
            flush_pending_comments_as_notes(
                &mut pending_comments,
                &mut sequence,
                &assignment.actor.text,
                &assignment.actor.text,
            );
            sequence.message(
                assignment.actor.text.clone(),
                assignment.actor.text,
                SequenceMessageKind::Solid,
                format!("{} => {}", assignment.method.text, assignment.target.text),
            );
            pending_return_annotator = false;
            continue;
        }

        // Sync messages without blocks:
        //   A.SyncMessage
        //   A.SyncMessage(with, parameters)
        if let Some((actor, method)) = parse_zenuml_method_call(line, statement_start) {
            push_zenuml_entity(
                &mut editor_facts,
                &actor,
                "zenuml participant reference",
                EditorSemanticKind::Event,
            );
            push_zenuml_payload(
                &mut editor_facts,
                &method,
                "zenuml message",
                EditorSemanticKind::String,
            );
            par_maybe_and(&mut stack, &mut sequence);
            flush_pending_comments_as_notes(
                &mut pending_comments,
                &mut sequence,
                &actor.text,
                &actor.text,
            );
            sequence.message(
                actor.text.clone(),
                actor.text,
                SequenceMessageKind::Solid,
                method.text,
            );
            continue;
        }

        // Return statements inside sync call blocks.
        if let Some(rest) = line.strip_prefix("return ") {
            let Some(actor) = stack.last().and_then(|b| match b {
                BlockKind::SyncCall { actor } => Some(actor.clone()),
                _ => None,
            }) else {
                record_zenuml_syntax_error(&mut first_error, line, statement_start);
                continue 'lines;
            };
            if let Some(value) = zenuml_value_after_keyword(line, "return", statement_start) {
                push_zenuml_payload(
                    &mut editor_facts,
                    &value,
                    "zenuml return payload",
                    EditorSemanticKind::String,
                );
            }
            par_maybe_and(&mut stack, &mut sequence);
            flush_pending_comments_as_notes(&mut pending_comments, &mut sequence, &actor, &actor);
            sequence.message(
                actor.clone(),
                actor,
                SequenceMessageKind::Dotted,
                rest.trim().to_string(),
            );
            pending_return_annotator = false;
            continue;
        }

        if let Some(message) = parse_zenuml_message(line, statement_start) {
            push_zenuml_message_facts(&mut editor_facts, &message);
            par_maybe_and(&mut stack, &mut sequence);
            flush_pending_comments_as_notes(
                &mut pending_comments,
                &mut sequence,
                &message.from.text,
                &message.to.text,
            );
            let force_reply = pending_return_annotator;
            pending_return_annotator = false;
            let kind = if message.reply || force_reply {
                SequenceMessageKind::Dotted
            } else {
                SequenceMessageKind::Solid
            };
            sequence.message(
                message.from.text,
                message.to.text,
                kind,
                message.label.map_or_else(String::new, |label| label.text),
            );
            continue;
        }

        record_zenuml_syntax_error(&mut first_error, line, statement_start);
    }

    if !saw_header && first_error.is_none() {
        first_error = Some(ZenumlSyntaxDiagnostic {
            message: "expected zenuml header".to_string(),
            span: SourceSpan::new(0, 0),
        });
    }
    if !stack.is_empty() && first_error.is_none() {
        first_error = Some(ZenumlSyntaxDiagnostic {
            message: "unterminated zenuml block; expected '}'".to_string(),
            span: SourceSpan::new(code.len(), code.len()),
        });
    }

    ZenumlTranslation {
        sequence,
        editor_facts,
        first_error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        EditorSemanticCompleteness, EditorSemanticRole, Engine, MermaidConfig, ParseOptions,
        RenderSemanticModel, SourceSpan,
    };

    fn meta() -> ParseMetadata {
        ParseMetadata {
            diagram_type: "zenuml".to_string(),
            config: MermaidConfig::empty_object(),
            effective_config: MermaidConfig::empty_object(),
            title: None,
        }
    }

    #[test]
    fn zenuml_basic_translates_to_sequence_model() {
        let engine = Engine::new();
        let input = "zenuml\n  Alice->Bob: Hello\n  Bob-->Alice: Reply\n";
        let parsed =
            futures::executor::block_on(engine.parse_diagram(input, ParseOptions::lenient()))
                .unwrap()
                .unwrap();
        assert_eq!(parsed.meta.diagram_type, "zenuml");
        assert!(parsed.model.get("messages").is_some());
    }

    #[test]
    fn zenuml_participants_and_fragments_translate_to_sequence_model() {
        let engine = Engine::new();
        let input = r#"zenuml
title Demo
Bob
Alice
Alice->Bob: Hi Bob
while(true) {
  Bob->Alice: Hi Alice
}
if(is_sick) {
  Bob->Alice: Not so good :(
} else {
  Bob->Alice: Feeling fresh
}
opt {
  Bob->Alice: Thanks
}
par {
  Alice->Bob: Hello guys!
  Alice->John: Hello guys!
}
"#;
        let parsed =
            futures::executor::block_on(engine.parse_diagram(input, ParseOptions::lenient()))
                .unwrap()
                .unwrap();
        assert_eq!(parsed.meta.diagram_type, "zenuml");
        assert!(parsed.model.get("messages").is_some());
    }

    #[test]
    fn zenuml_reply_message_forms_translate() {
        let engine = Engine::new();
        let input = r#"zenuml
SomeType a = A.SyncMessage()
a = A.SyncMessage()
A.SyncMessage() {
  return result
}
@return
A->B: ok
"#;
        let parsed =
            futures::executor::block_on(engine.parse_diagram(input, ParseOptions::lenient()))
                .unwrap()
                .unwrap();
        assert_eq!(parsed.meta.diagram_type, "zenuml");
        assert!(parsed.model.get("messages").is_some());
    }

    #[test]
    fn zenuml_render_model_uses_sequence_typed_variant_without_changing_json_parse() {
        let engine = Engine::new();
        let input = r#"zenuml
title Login Flow
Alice->Bob: Login
Bob-->Alice: Ack
"#;

        let parsed = engine
            .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
            .unwrap()
            .unwrap();

        assert_eq!(parsed.meta.diagram_type, "zenuml");
        match parsed.model {
            RenderSemanticModel::Sequence(model) => {
                assert_eq!(model.title.as_deref(), Some("Login Flow"));
                assert_eq!(model.messages.len(), 2);
                assert_eq!(model.messages[0].from.as_deref(), Some("Alice"));
                assert_eq!(model.messages[0].to.as_deref(), Some("Bob"));
                assert_eq!(model.messages[0].message_text(), "Login");
            }
            other => {
                panic!("zenuml render parse should return sequence typed model, got {other:?}")
            }
        }

        let parsed_json = engine
            .parse_diagram_sync(input, ParseOptions::strict())
            .unwrap()
            .unwrap();
        assert_eq!(parsed_json.meta.diagram_type, "zenuml");
        assert!(parsed_json.model.get("messages").is_some());
        assert_eq!(parsed_json.model["title"], serde_json::json!("Login Flow"));
    }

    #[test]
    fn zenuml_editor_facts_expose_source_mapped_spans() {
        let engine = Engine::new();
        let input = r#"zenuml
title Login Flow
accTitle Login accessibility title
accDescr Login accessibility description
@Actor Alice
Bob
A as API
Alice->Bob: Login
SomeType result = A.SyncMessage()
new Session(with, params)
"#;

        let facts = engine
            .parse_editor_semantic_facts_with_type_sync("zenuml", input, ParseOptions::strict())
            .unwrap()
            .unwrap();

        assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
        for prefix in ["title", "accTitle", "accDescr"] {
            assert!(
                facts
                    .directive_prefixes
                    .iter()
                    .any(|candidate| candidate == prefix),
                "missing ZenUML directive prefix {prefix}"
            );
        }
        for entity in ["Alice", "Bob", "A", "Session"] {
            assert!(
                facts.symbols.iter().any(|symbol| {
                    symbol.name == entity && symbol.role == EditorSemanticRole::Entity
                }),
                "missing ZenUML entity fact for {entity}"
            );
        }
        for payload in [
            "Login Flow",
            "Login accessibility title",
            "Login accessibility description",
            "API",
            "Login",
            "SyncMessage()",
            "result",
            "Session(with, params)",
        ] {
            assert!(
                facts.symbols.iter().any(|symbol| {
                    symbol.name == payload && symbol.role == EditorSemanticRole::Payload
                }),
                "missing ZenUML payload fact for {payload}"
            );
        }

        let login_start = input.find("Alice->Bob: Login").unwrap() + "Alice->Bob: ".len();
        assert!(facts.symbols.iter().any(|symbol| {
            symbol.name == "Login"
                && symbol.role == EditorSemanticRole::Payload
                && symbol.span == SourceSpan::new(login_start, login_start + "Login".len())
        }));
    }

    #[test]
    fn zenuml_editor_facts_recover_unsupported_statements_without_losing_prior_facts() {
        let engine = Engine::new();
        let input = "zenuml\nAlice\nUnsupported ? statement\nAlice->Bob: Hi\n";

        let facts = engine
            .parse_editor_semantic_facts_with_type_sync("zenuml", input, ParseOptions::strict())
            .unwrap()
            .unwrap();

        assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
        assert!(!facts.diagnostics.is_empty());
        assert!(
            facts.symbols.iter().any(|symbol| {
                symbol.name == "Alice" && symbol.role == EditorSemanticRole::Entity
            })
        );
        assert!(
            facts.symbols.iter().any(|symbol| {
                symbol.name == "Hi" && symbol.role == EditorSemanticRole::Payload
            })
        );
    }

    #[test]
    fn zenuml_combined_parse_constructs_once_and_matches_standalone_entrypoints() {
        let input = r#"zenuml
title Login Flow
@Actor Alice
Bob
Alice->Bob: Login
if(accepted) {
  Bob-->Alice: Ack
}
"#;
        let meta = meta();

        crate::diagrams::langium_common::reset_family_syntax_construction_count("zenuml");
        crate::diagrams::sequence::reset_sequence_syntax_construction_count();
        let (combined_json, combined_editor) =
            parse_zenuml_json_and_editor_facts(input, &meta).unwrap();
        assert_eq!(
            crate::diagrams::langium_common::family_syntax_construction_count("zenuml"),
            1,
            "one combined request must construct ZenUML syntax once"
        );
        assert_eq!(
            crate::diagrams::sequence::sequence_syntax_construction_count(),
            0,
            "ZenUML must lower directly to Sequence semantic actions without invoking its parser"
        );

        assert_eq!(combined_json, parse_zenuml(input, &meta).unwrap());
        assert_eq!(combined_editor, parse_zenuml_editor_facts(input, &meta));
    }

    #[test]
    fn zenuml_typed_and_json_projections_share_one_sequence_model() {
        let input = "zenuml\ntitle Login Flow\nAlice->Bob: Login\nBob-->Alice: Ack\n";
        let compat = parse_zenuml(input, &meta()).unwrap();
        let typed = parse_zenuml_model_for_render(input, &meta()).unwrap();

        assert_eq!(compat, typed.to_compat_json("zenuml"));
        assert_eq!(typed.title.as_deref(), Some("Login Flow"));
        assert_eq!(typed.messages.len(), 2);
    }

    #[test]
    fn zenuml_direct_actions_match_equivalent_sequence_semantics() {
        let input = r#"zenuml
title Demo
accTitle Screen reader title
accDescr Screen reader description
@Actor Alice
B as Bob
new Session(with, params)
Alice->Session: create
// work note
Session.Work {
  return done
}
while(active) {
  Alice->Bob: loop message
}
if(ok) {
  Bob->Alice: accepted
} else {
  Bob-->Alice: rejected
}
opt optional {
  Alice->Bob: maybe
}
par parallel {
  Alice->Bob: first
  Alice->Session: second
}
"#;
        let equivalent_sequence = r#"sequenceDiagram
title Demo
accTitle: Screen reader title
accDescr: Screen reader description
actor Alice
participant Bob as B
create participant Session as Session(with, params)
Alice->>Session: create
Note over Session,Session: work note
Session->>Session: Work
activate Session
Session-->>Session: done
deactivate Session
loop while(active)
Alice->>Bob: loop message
end
alt if(ok)
Bob->>Alice: accepted
else
Bob-->>Alice: rejected
end
opt optional
Alice->>Bob: maybe
end
par parallel
Alice->>Bob: first
and
Alice->>Session: second
end
"#;

        let direct = parse_zenuml_model_for_render(input, &meta()).unwrap();
        let parsed_sequence = crate::diagrams::sequence::parse_sequence_model_for_render(
            equivalent_sequence,
            &meta(),
        )
        .unwrap();

        assert_eq!(
            direct.to_compat_json("zenuml"),
            parsed_sequence.to_compat_json("zenuml")
        );
    }

    #[test]
    fn zenuml_malformed_recovery_reuses_statement_facts_and_exact_original_span() {
        let input = "zenuml\nAlice\nUnsupported ? statement\nAlice->Bob: Hi\n";
        let invalid_start = input.find("Unsupported ? statement").unwrap();
        let invalid_span = SourceSpan::new(
            invalid_start,
            invalid_start + "Unsupported ? statement".len(),
        );

        let error = parse_zenuml(input, &meta()).expect_err("strict parser must reject the line");
        let Error::DiagramParse { diagnostic, .. } = error else {
            panic!("expected structured ZenUML parse error");
        };
        assert!(
            diagnostic
                .message()
                .contains("unsupported zenuml statement")
        );
        assert_eq!(diagnostic.span(), Some(invalid_span));
        assert_eq!(
            diagnostic.span_kind(),
            crate::ParseDiagnosticSpanKind::Exact
        );

        let facts = parse_zenuml_editor_facts(input, &meta());
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
        assert!(facts.diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("unsupported zenuml statement")
                && diagnostic.span == Some(invalid_span)
        }));
        for name in ["Alice", "Hi"] {
            assert!(
                facts.symbols.iter().any(|symbol| symbol.name == name),
                "same-pass recovery lost {name}"
            );
        }
        assert!(
            !facts
                .symbols
                .iter()
                .any(|symbol| { symbol.name.contains("Unsupported") || symbol.name.contains('?') })
        );
    }

    #[test]
    fn zenuml_unclosed_block_fails_at_eof_and_preserves_recovered_facts() {
        let input = "zenuml\nAlice\nif(ready) {\n  Alice->Bob: Hi\n";
        let eof = SourceSpan::new(input.len(), input.len());

        let error = parse_zenuml(input, &meta()).expect_err("unclosed block must fail strictly");
        let Error::DiagramParse { diagnostic, .. } = error else {
            panic!("expected structured ZenUML parse error");
        };
        assert!(diagnostic.message().contains("unterminated zenuml block"));
        assert_eq!(diagnostic.span(), Some(eof));

        let facts = parse_zenuml_editor_facts(input, &meta());
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
        assert!(facts.diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("unterminated zenuml block") && diagnostic.span == Some(eof)
        }));
        for name in ["Alice", "Bob", "Hi"] {
            assert!(
                facts.symbols.iter().any(|symbol| symbol.name == name),
                "same-pass recovery lost {name}"
            );
        }
    }

    #[test]
    fn zenuml_repeated_values_keep_role_specific_original_spans() {
        let input = "zenuml\nA as A\nA->A:A\nA.A {\n}\n";
        let facts = parse_zenuml_editor_facts(input, &meta());

        let alias_line = input.find("A as A").unwrap();
        let message_line = input.find("A->A:A").unwrap();
        let call_line = input.rfind("A.A {").unwrap();
        for (detail, span) in [
            (
                "zenuml participant alias",
                SourceSpan::new(alias_line, alias_line + 1),
            ),
            (
                "zenuml participant label",
                SourceSpan::new(alias_line + 5, alias_line + 6),
            ),
            (
                "zenuml participant reference",
                SourceSpan::new(message_line + 3, message_line + 4),
            ),
            (
                "zenuml message",
                SourceSpan::new(message_line + 5, message_line + 6),
            ),
            (
                "zenuml message",
                SourceSpan::new(call_line + 2, call_line + 3),
            ),
        ] {
            assert!(facts.symbols.iter().any(|symbol| {
                symbol.name == "A"
                    && symbol.detail.as_deref() == Some(detail)
                    && symbol.selection == span
            }));
        }
    }
}
