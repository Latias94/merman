use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
    EditorSemanticSymbol, Error, ParseMetadata, Result, SourceSpan,
};
use serde_json::Value;

/// Parses a ZenUML diagram into a Mermaid-like semantic model.
///
/// Upstream Mermaid integrates ZenUML via the `mermaid-zenuml` external diagram package, which
/// uses `@zenuml/core` in the browser. `merman` is headless and pure Rust, so for now we implement
/// a conservative compatibility mode: a small ZenUML subset is translated into Mermaid
/// `sequenceDiagram` syntax and then parsed by the existing sequence parser.
///
/// Rendering still goes through that translation seam, while editor facts are collected directly
/// from the original ZenUML source so LSP ranges stay source-mapped for the supported subset.
pub fn parse_zenuml(code: &str, meta: &ParseMetadata) -> Result<Value> {
    let sequence_code = translate_zenuml_to_sequence(code, meta)?;
    crate::diagrams::sequence::parse_sequence(&sequence_code, meta)
}

pub fn parse_zenuml_model_for_render(
    code: &str,
    meta: &ParseMetadata,
) -> Result<crate::diagrams::sequence::SequenceDiagramRenderModel> {
    let sequence_code = translate_zenuml_to_sequence(code, meta)?;
    crate::diagrams::sequence::parse_sequence_model_for_render(&sequence_code, meta)
}

#[derive(Debug, Clone)]
struct ZenumlSpannedText {
    text: String,
    span: SourceSpan,
}

pub fn parse_zenuml_editor_facts(code: &str, meta: &ParseMetadata) -> EditorSemanticFacts {
    let mut facts = collect_zenuml_editor_facts_from_source(code);
    if let Err(err) = translate_zenuml_to_sequence(code, meta) {
        facts.mark_recovered_with_diagnostic(
            format!("zenuml parser recovered after parse error: {err}"),
            Some(SourceSpan::new(0, code.len())),
        );
    }
    facts
}

fn collect_zenuml_editor_facts_from_source(code: &str) -> EditorSemanticFacts {
    let mut facts = EditorSemanticFacts::new();
    let mut offset = 0usize;
    let mut header_seen = false;

    for segment in code.split_inclusive('\n') {
        let line_start = offset;
        offset += segment.len();
        let line = segment.trim_end_matches(['\n', '\r']);
        let mut rest = line.trim_start();
        if rest.is_empty() {
            continue;
        }

        if !header_seen && rest.to_ascii_lowercase().starts_with("zenuml") {
            header_seen = true;
            continue;
        }
        header_seen = true;

        while let Some(after) = rest.strip_prefix('}') {
            rest = after.trim_start();
        }
        if rest.is_empty() || rest.starts_with("//") {
            continue;
        }
        if rest.eq_ignore_ascii_case("@return") || rest.eq_ignore_ascii_case("@reply") {
            continue;
        }

        let stmt_start = line_start + line.find(rest).unwrap_or(0);

        if let Some(value) = zenuml_value_after_keyword(rest, "title", stmt_start) {
            facts.push_directive_prefix("title");
            push_zenuml_payload(
                &mut facts,
                value,
                "zenuml title",
                EditorSemanticKind::String,
            );
            continue;
        }
        if let Some(value) = zenuml_value_after_keyword_ci(rest, "accTitle", stmt_start) {
            facts.push_directive_prefix("accTitle");
            push_zenuml_payload(
                &mut facts,
                value,
                "zenuml accessibility title",
                EditorSemanticKind::String,
            );
            continue;
        }
        if let Some(value) = zenuml_value_after_keyword_ci(rest, "accDescr", stmt_start) {
            facts.push_directive_prefix("accDescr");
            push_zenuml_payload(
                &mut facts,
                value,
                "zenuml accessibility description",
                EditorSemanticKind::String,
            );
            continue;
        }

        if push_zenuml_assignment_facts(&mut facts, rest, stmt_start) {
            continue;
        }

        let rest_no_brace = strip_zenuml_trailing_open_brace(rest).unwrap_or(rest);
        if push_zenuml_block_or_call_facts(&mut facts, rest_no_brace.trim(), stmt_start).is_some() {
            continue;
        }

        if let Some(created) = parse_zenuml_creation(rest, stmt_start) {
            push_zenuml_entity(
                &mut facts,
                created,
                "zenuml participant",
                EditorSemanticKind::Event,
            );
            push_zenuml_payload_tail(&mut facts, rest, stmt_start, "zenuml creation payload");
            continue;
        }

        if push_zenuml_participant_decl_facts(&mut facts, rest, stmt_start) {
            continue;
        }

        if push_zenuml_message_facts(&mut facts, rest, stmt_start) {
            continue;
        }

        if let Some(value) = zenuml_value_after_keyword(rest, "return", stmt_start) {
            push_zenuml_payload(
                &mut facts,
                value,
                "zenuml return payload",
                EditorSemanticKind::String,
            );
            continue;
        }

        facts.mark_recovered();
    }

    facts
}

fn zenuml_value_after_keyword(
    line: &str,
    keyword: &str,
    stmt_start: usize,
) -> Option<ZenumlSpannedText> {
    let rest = line.strip_prefix(keyword)?;
    let rest = rest.strip_prefix(|ch: char| ch.is_whitespace())?;
    zenuml_trimmed_spanned(rest, stmt_start + line.find(rest).unwrap_or(0))
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

fn strip_zenuml_trailing_open_brace(line: &str) -> Option<&str> {
    line.trim_end().strip_suffix('{').map(str::trim_end)
}

fn push_zenuml_block_or_call_facts(
    facts: &mut EditorSemanticFacts,
    line: &str,
    stmt_start: usize,
) -> Option<()> {
    for keyword in [
        "while", "for", "foreach", "forEach", "loop", "opt", "par", "if", "else if", "catch",
        "finally",
    ] {
        if starts_with_zenuml_word_ci(line, keyword) {
            let detail = format!("zenuml {keyword} payload");
            push_zenuml_payload_tail(facts, line, stmt_start, &detail);
            return Some(());
        }
    }
    if starts_with_zenuml_word_ci(line, "else") || starts_with_zenuml_word_ci(line, "try") {
        return Some(());
    }

    if let Some((actor, method)) = parse_zenuml_method_call(line, stmt_start) {
        push_zenuml_entity(
            facts,
            actor,
            "zenuml participant reference",
            EditorSemanticKind::Event,
        );
        push_zenuml_payload(facts, method, "zenuml message", EditorSemanticKind::String);
        return Some(());
    }

    None
}

fn starts_with_zenuml_word_ci(haystack: &str, word: &str) -> bool {
    haystack
        .get(0..word.len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(word))
        && haystack
            .get(word.len()..word.len() + 1)
            .is_none_or(|c| c.chars().all(|ch| ch.is_ascii_whitespace() || ch == '('))
}

fn parse_zenuml_creation(line: &str, stmt_start: usize) -> Option<ZenumlSpannedText> {
    let rest = line.strip_prefix("new ")?;
    let rest_start = stmt_start + line.find(rest).unwrap_or(0);
    parse_zenuml_identifier(rest, rest_start)
}

fn push_zenuml_participant_decl_facts(
    facts: &mut EditorSemanticFacts,
    line: &str,
    stmt_start: usize,
) -> bool {
    if let Some(rest) = line.strip_prefix('@') {
        let kind_len = rest.find(char::is_whitespace).unwrap_or(rest.len());
        let after_kind = &rest[kind_len..];
        let Some(name) = zenuml_trimmed_spanned(after_kind, stmt_start + 1 + kind_len) else {
            return false;
        };
        push_zenuml_entity(facts, name, "zenuml participant", EditorSemanticKind::Event);
        return true;
    }

    if let Some((id, label)) = split_zenuml_alias_decl(line, stmt_start) {
        push_zenuml_entity(
            facts,
            id,
            "zenuml participant alias",
            EditorSemanticKind::Event,
        );
        push_zenuml_payload(
            facts,
            label,
            "zenuml participant label",
            EditorSemanticKind::String,
        );
        return true;
    }

    if line
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
    {
        push_zenuml_entity(
            facts,
            ZenumlSpannedText {
                text: line.to_string(),
                span: SourceSpan::new(stmt_start, stmt_start + line.len()),
            },
            "zenuml participant",
            EditorSemanticKind::Event,
        );
        return true;
    }

    false
}

fn split_zenuml_alias_decl(
    line: &str,
    stmt_start: usize,
) -> Option<(ZenumlSpannedText, ZenumlSpannedText)> {
    let (id_raw, label_raw) = line.split_once(" as ")?;
    let id = zenuml_trimmed_spanned(id_raw, stmt_start)?;
    let label_start = stmt_start + line.find(label_raw).unwrap_or(line.len());
    let label = zenuml_trimmed_spanned(label_raw, label_start)?;
    Some((id, label))
}

fn push_zenuml_assignment_facts(
    facts: &mut EditorSemanticFacts,
    line: &str,
    stmt_start: usize,
) -> bool {
    let Some(eq) = line.find('=') else {
        return false;
    };
    let rhs = line[eq + 1..].trim_start();
    let rhs_start = stmt_start + eq + 1 + line[eq + 1..].len() - rhs.len();
    let Some((actor, method)) = parse_zenuml_method_call(rhs, rhs_start) else {
        return false;
    };
    push_zenuml_entity(
        facts,
        actor,
        "zenuml participant reference",
        EditorSemanticKind::Event,
    );
    push_zenuml_payload(facts, method, "zenuml message", EditorSemanticKind::String);
    if let Some(var) = line[..eq].split_whitespace().last()
        && let Some(rel) = line[..eq].rfind(var)
    {
        push_zenuml_payload(
            facts,
            ZenumlSpannedText {
                text: var.to_string(),
                span: SourceSpan::new(stmt_start + rel, stmt_start + rel + var.len()),
            },
            "zenuml assignment target",
            EditorSemanticKind::Variable,
        );
    }
    true
}

fn push_zenuml_message_facts(
    facts: &mut EditorSemanticFacts,
    line: &str,
    stmt_start: usize,
) -> bool {
    let Some((lhs, label)) = line
        .split_once(':')
        .map_or(Some((line, None)), |(a, b)| Some((a, Some(b))))
    else {
        return false;
    };

    let Some((from_raw, arrow, to_raw)) = split_zenuml_arrow(lhs) else {
        return false;
    };
    let Some(from) = zenuml_trimmed_spanned(from_raw, stmt_start + lhs.find(from_raw).unwrap_or(0))
    else {
        return false;
    };
    let to_start = stmt_start
        + lhs
            .find(to_raw)
            .unwrap_or(from.span.end.saturating_sub(stmt_start));
    let Some(to) = zenuml_trimmed_spanned(to_raw, to_start) else {
        return false;
    };
    if arrow != "->" && arrow != "-->" {
        return false;
    }

    push_zenuml_entity(
        facts,
        from,
        "zenuml participant reference",
        EditorSemanticKind::Event,
    );
    push_zenuml_entity(
        facts,
        to,
        "zenuml participant reference",
        EditorSemanticKind::Event,
    );
    if let Some(label) = label {
        let label_start = stmt_start + line.find(label).unwrap_or(line.len());
        if let Some(label) = zenuml_trimmed_spanned(label, label_start) {
            push_zenuml_payload(facts, label, "zenuml message", EditorSemanticKind::String);
        }
    }
    true
}

fn split_zenuml_arrow(lhs: &str) -> Option<(&str, &str, &str)> {
    if let Some((from, to)) = lhs.split_once("-->") {
        return Some((from, "-->", to));
    }
    if let Some((from, to)) = lhs.split_once("->") {
        return Some((from, "->", to));
    }
    None
}

fn parse_zenuml_method_call(
    line: &str,
    stmt_start: usize,
) -> Option<(ZenumlSpannedText, ZenumlSpannedText)> {
    let (actor_raw, method_raw) = line.split_once('.')?;
    let actor = zenuml_trimmed_spanned(actor_raw, stmt_start)?;
    let method_start = stmt_start + line.find(method_raw).unwrap_or(line.len());
    let method = zenuml_trimmed_spanned(method_raw, method_start)?;
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
        push_zenuml_payload(facts, payload, detail, EditorSemanticKind::String);
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
    text: ZenumlSpannedText,
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
        text.text,
        Some(detail.to_string()),
        kind,
        text.span,
        text.span,
    ));
}

fn push_zenuml_payload(
    facts: &mut EditorSemanticFacts,
    text: ZenumlSpannedText,
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

fn translate_zenuml_to_sequence(code: &str, meta: &ParseMetadata) -> Result<String> {
    let mut out: Vec<String> = vec!["sequenceDiagram".to_string()];

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

    fn translate_participant_decl(line: &str) -> Option<String> {
        // Participant order control:
        //   Bob
        //   Alice
        //
        // Annotators:
        //   @Actor Alice
        //   @Database Bob
        //
        // Aliases:
        //   A as Alice
        //   J as John
        let l = line.trim();
        if l.is_empty() {
            return None;
        }

        if let Some(rest) = l.strip_prefix('@') {
            let (kind, name) = rest.split_once(' ')?;
            let kind = kind.trim();
            let name = name.trim();
            if name.is_empty() {
                return None;
            }
            // Mermaid `sequenceDiagram` supports a limited set of participant kinds in our
            // headless parser today. Keep this translation conservative so fixtures can be
            // snapshot-gated deterministically.
            let kw = if kind.eq_ignore_ascii_case("actor") {
                "actor"
            } else {
                // `@Database`, `@Boundary`, etc. are represented as standard participants.
                "participant"
            };
            return Some(format!("{kw} {name}"));
        }

        if let Some((id, label)) = l.split_once(" as ") {
            let id = id.trim();
            let label = label.trim();
            if id.is_empty() || label.is_empty() {
                return None;
            }
            // ZenUML uses `A as Alice` where `A` is used in messages and `Alice` is the label.
            // Mermaid sequence supports `participant Alice as A` (label first, alias second).
            return Some(format!("participant {label} as {id}"));
        }

        // Bare participant/actor declaration (single token).
        if l.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
        {
            return Some(format!("participant {l}"));
        }

        None
    }

    fn translate_assignment(line: &str) -> Option<(String, String, String)> {
        // Minimal supported syntax (ZenUML docs "Reply message"):
        //   a = A.SyncMessage()
        //   SomeType a = A.SyncMessage()
        //
        // Returns (var, actor, call_text).
        let (lhs, rhs) = line.split_once('=')?;
        let lhs = lhs.trim();
        let rhs = rhs.trim();
        if lhs.is_empty() || rhs.is_empty() {
            return None;
        }

        let var = lhs.split_whitespace().last()?.trim();
        if var.is_empty() {
            return None;
        }

        let (actor, call) = rhs.split_once('.')?;
        let actor = actor.trim();
        let call = call.trim();
        if actor.is_empty() || call.is_empty() {
            return None;
        }

        Some((var.to_string(), actor.to_string(), call.to_string()))
    }

    fn translate_message_line(line: &str) -> Option<String> {
        // Minimal supported syntax:
        //   Alice->Bob: Hello
        //   Bob-->Alice: Reply
        //
        // Map to Mermaid sequence syntax:
        //   Alice->>Bob: Hello
        //   Bob-->>Alice: Reply
        let (lhs, label) = if let Some((a, b)) = line.split_once(':') {
            (a.trim(), Some(b.trim()))
        } else {
            (line.trim(), None)
        };

        let (from, arrow, to) = if let Some((a, b)) = lhs.split_once("-->") {
            (a.trim(), "-->", b.trim())
        } else if let Some((a, b)) = lhs.split_once("->") {
            (a.trim(), "->", b.trim())
        } else {
            return None;
        };

        if from.is_empty() || to.is_empty() {
            return None;
        }

        let seq_arrow = match arrow {
            "-->" => "-->>",
            "->" => "->>",
            _ => return None,
        };

        let mut out = String::new();
        out.push_str(from);
        out.push_str(seq_arrow);
        out.push_str(to);
        if let Some(lbl) = label
            && !lbl.is_empty()
        {
            out.push_str(": ");
            out.push_str(lbl);
        }
        Some(out)
    }

    fn flush_pending_comments_as_notes(
        pending: &mut Vec<String>,
        out: &mut Vec<String>,
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
            // ZenUML comments are rendered above messages/fragments. Approximate this behavior
            // by emitting a Mermaid sequence note spanning the message participants.
            out.push(format!("Note over {from},{to}: {text}"));
        }
    }

    let mut stack: Vec<BlockKind> = Vec::new();

    fn par_maybe_and(stack: &mut [BlockKind], out: &mut Vec<String>) {
        let Some(BlockKind::Par { branch_started }) = stack.last_mut() else {
            return;
        };
        if *branch_started {
            out.push("and".to_string());
        } else {
            *branch_started = true;
        }
    }

    fn close_brace(rest: &str, stack: &mut Vec<BlockKind>, out: &mut Vec<String>) {
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
                out.push(format!("deactivate {actor}"));
            }
            Some(_) => {
                out.push("end".to_string());
            }
            None => {}
        }
    }

    for raw in code.lines() {
        let mut line = raw.trim();
        if line.is_empty() {
            continue;
        }

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
            let rest = trimmed[1..].trim_start();
            close_brace(rest, &mut stack, &mut out);
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
            out.push(line.to_string());
            pending_comments.clear();
            continue;
        }
        if line.to_ascii_lowercase().starts_with("acctitle ") {
            out.push(line.to_string());
            pending_comments.clear();
            continue;
        }
        if line.to_ascii_lowercase().starts_with("accdescr ") {
            out.push(line.to_string());
            pending_comments.clear();
            continue;
        }

        // Branch continuations for `if` and `try` structures.
        if let Some(prefix) = strip_trailing_open_brace(line) {
            let p = prefix.trim();
            if starts_with_word_ci(p, "else if") {
                let Some((_, cond)) = p.split_once('(') else {
                    return Err(Error::DiagramParse {
                        diagram_type: meta.diagram_type.clone(),
                        message: format!("unsupported zenuml statement: {line}"),
                    });
                };
                let Some((cond, _)) = cond.rsplit_once(')') else {
                    return Err(Error::DiagramParse {
                        diagram_type: meta.diagram_type.clone(),
                        message: format!("unsupported zenuml statement: {line}"),
                    });
                };
                let label = format!("if({})", cond.trim());
                out.push(format!("else {label}"));
                pending_comments.clear();
                continue;
            }
            if starts_with_word_ci(p, "else") {
                out.push("else".to_string());
                pending_comments.clear();
                continue;
            }
            if starts_with_word_ci(p, "catch") {
                out.push("else catch".to_string());
                pending_comments.clear();
                continue;
            }
            if starts_with_word_ci(p, "finally") {
                out.push("else finally".to_string());
                pending_comments.clear();
                continue;
            }
        }

        // Block openings.
        if let Some(prefix) = strip_trailing_open_brace(line) {
            let p = prefix.trim();

            if starts_with_word_ci(p, "while") {
                par_maybe_and(&mut stack, &mut out);
                out.push(format!("loop {p}"));
                stack.push(BlockKind::Loop);
                pending_comments.clear();
                continue;
            }
            if starts_with_word_ci(p, "for")
                || starts_with_word_ci(p, "foreach")
                || starts_with_word_ci(p, "forEach")
                || starts_with_word_ci(p, "loop")
            {
                par_maybe_and(&mut stack, &mut out);
                out.push(format!("loop {p}"));
                stack.push(BlockKind::Loop);
                pending_comments.clear();
                continue;
            }
            if starts_with_word_ci(p, "opt") {
                par_maybe_and(&mut stack, &mut out);
                let label = p.strip_prefix("opt").unwrap_or("").trim();
                if label.is_empty() {
                    out.push("opt".to_string());
                } else {
                    out.push(format!("opt {label}"));
                }
                stack.push(BlockKind::Opt);
                pending_comments.clear();
                continue;
            }
            if starts_with_word_ci(p, "par") {
                par_maybe_and(&mut stack, &mut out);
                let label = p.strip_prefix("par").unwrap_or("").trim();
                if label.is_empty() {
                    out.push("par".to_string());
                } else {
                    out.push(format!("par {label}"));
                }
                stack.push(BlockKind::Par {
                    branch_started: false,
                });
                pending_comments.clear();
                continue;
            }
            if starts_with_word_ci(p, "if") {
                par_maybe_and(&mut stack, &mut out);
                let Some((_, cond)) = p.split_once('(') else {
                    return Err(Error::DiagramParse {
                        diagram_type: meta.diagram_type.clone(),
                        message: format!("unsupported zenuml statement: {line}"),
                    });
                };
                let Some((cond, _)) = cond.rsplit_once(')') else {
                    return Err(Error::DiagramParse {
                        diagram_type: meta.diagram_type.clone(),
                        message: format!("unsupported zenuml statement: {line}"),
                    });
                };
                out.push(format!("alt if({})", cond.trim()));
                stack.push(BlockKind::IfAlt);
                pending_comments.clear();
                continue;
            }
            if starts_with_word_ci(p, "try") {
                par_maybe_and(&mut stack, &mut out);
                out.push("alt try".to_string());
                stack.push(BlockKind::TryAlt);
                pending_comments.clear();
                continue;
            }

            // Sync message / method-call blocks:
            //   A.SyncMessage(with, parameters) { ... }
            //
            // Translate to a self-message plus explicit activation scope.
            if let Some((actor, method)) = p.split_once('.') {
                let actor = actor.trim();
                let method = method.trim();
                if !actor.is_empty() && !method.is_empty() {
                    par_maybe_and(&mut stack, &mut out);
                    flush_pending_comments_as_notes(&mut pending_comments, &mut out, actor, actor);
                    out.push(format!("{actor}->>{actor}: {method}"));
                    out.push(format!("activate {actor}"));
                    stack.push(BlockKind::SyncCall {
                        actor: actor.to_string(),
                    });
                    continue;
                }
            }
        }

        // Creation messages:
        //   new A1
        //   new A2(with, parameters)
        if let Some(rest) = line.strip_prefix("new ") {
            let rest = rest.trim();
            if rest.is_empty() {
                return Err(Error::DiagramParse {
                    diagram_type: meta.diagram_type.clone(),
                    message: format!("unsupported zenuml statement: {line}"),
                });
            }

            // Extract a stable id for Mermaid sequence: the leading identifier token.
            let chars = rest.chars();
            let mut id = String::new();
            for ch in chars {
                if ch.is_ascii_alphanumeric() || ch == '_' || ch == '.' {
                    id.push(ch);
                } else {
                    break;
                }
            }
            if id.is_empty() {
                return Err(Error::DiagramParse {
                    diagram_type: meta.diagram_type.clone(),
                    message: format!("unsupported zenuml statement: {line}"),
                });
            }

            par_maybe_and(&mut stack, &mut out);
            pending_comments.clear();

            // If the creation has arguments, keep the full text as the label (description).
            if rest != id {
                out.push(format!("create participant {id} as {rest}"));
            } else {
                out.push(format!("create participant {id}"));
            }
            continue;
        }

        // Participants.
        if let Some(decl) = translate_participant_decl(line) {
            par_maybe_and(&mut stack, &mut out);
            out.push(decl);
            // ZenUML comment on a participant is not rendered.
            pending_comments.clear();
            continue;
        }

        // Reply assignments must be handled before generic `Actor.Method(...)` parsing, because
        // an assignment line contains a `.` and would otherwise be misinterpreted as a sync call.
        if let Some((var, actor, call)) = translate_assignment(line) {
            par_maybe_and(&mut stack, &mut out);
            flush_pending_comments_as_notes(&mut pending_comments, &mut out, &actor, &actor);
            out.push(format!("{actor}->>{actor}: {call} => {var}"));
            pending_return_annotator = false;
            continue;
        }

        // Sync messages without blocks:
        //   A.SyncMessage
        //   A.SyncMessage(with, parameters)
        if let Some((actor, method)) = line.split_once('.') {
            let actor = actor.trim();
            let method = method.trim();
            if !actor.is_empty() && !method.is_empty() {
                par_maybe_and(&mut stack, &mut out);
                flush_pending_comments_as_notes(&mut pending_comments, &mut out, actor, actor);
                out.push(format!("{actor}->>{actor}: {method}"));
                continue;
            }
        }

        // Return statements inside sync call blocks.
        if let Some(rest) = line.strip_prefix("return ") {
            let Some(actor) = stack.last().and_then(|b| match b {
                BlockKind::SyncCall { actor } => Some(actor.clone()),
                _ => None,
            }) else {
                return Err(Error::DiagramParse {
                    diagram_type: meta.diagram_type.clone(),
                    message: format!("unsupported zenuml statement: {line}"),
                });
            };
            par_maybe_and(&mut stack, &mut out);
            flush_pending_comments_as_notes(&mut pending_comments, &mut out, &actor, &actor);
            out.push(format!("{actor}-->>{actor}: {}", rest.trim()));
            pending_return_annotator = false;
            continue;
        }

        if let Some(mut seq_line) = translate_message_line(line) {
            par_maybe_and(&mut stack, &mut out);
            let (lhs, _) = if let Some((a, b)) = line.split_once(':') {
                (a.trim(), Some(b.trim()))
            } else {
                (line.trim(), None)
            };
            let (from, to) = if let Some((a, b)) = lhs.split_once("-->") {
                (a.trim(), b.trim())
            } else if let Some((a, b)) = lhs.split_once("->") {
                (a.trim(), b.trim())
            } else {
                ("", "")
            };
            flush_pending_comments_as_notes(&mut pending_comments, &mut out, from, to);
            if pending_return_annotator {
                // Convert `->>` to `-->>` for return/reply.
                seq_line = seq_line.replace("->>", "-->>");
                pending_return_annotator = false;
            }
            out.push(seq_line);
            continue;
        }

        return Err(Error::DiagramParse {
            diagram_type: meta.diagram_type.clone(),
            message: format!("unsupported zenuml statement: {line}"),
        });
    }

    Ok(out.join("\n"))
}

#[cfg(test)]
mod tests {
    use crate::{
        EditorSemanticCompleteness, EditorSemanticRole, Engine, ParseOptions, RenderSemanticModel,
        SourceSpan,
    };

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
}
