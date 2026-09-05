use super::{
    ClassAssignStmt, ClassDefStmt, ClickAction, ClickStmt, LabeledText, LexError, LinkStylePos,
    LinkStyleStmt, StyleStmt, TitleKind, ast::FlowchartClickEditorEvidence,
};
use crate::SourceSpan;

pub(super) fn parse_node_label_text(raw: &str) -> std::result::Result<LabeledText, LexError> {
    let quoted = raw.starts_with('"') && raw.ends_with('"');

    let (text, kind) = super::parse_label_text(raw);

    match kind {
        TitleKind::Text => {
            // Mermaid Jison-based flowchart lexer treats these as structural tokens (PS/PE/SQE/etc)
            // and will throw parse errors if they appear inside TEXT.
            if text.contains('"')
                || text.contains('(')
                || text.contains(')')
                || text.contains('[')
                || text.contains(']')
                || text.contains('{')
                || text.contains('}')
                || text.contains('|')
            {
                return Err(LexError::new(
                    "Invalid text label: contains structural characters; quote it to use them",
                ));
            }
            if contains_bound_at(raw) {
                return Err(LexError::new(
                    "Invalid text label: unquoted `@` must be preceded by whitespace",
                ));
            }
        }
        TitleKind::String => {
            // Mermaid's Jison string state closes on every double quote; backslashes are literal.
            if quoted {
                let inner = raw.get(1..raw.len().saturating_sub(1)).unwrap_or_default();
                if inner.contains('"') {
                    return Err(LexError::new(
                        "Invalid string label: contains nested quotes".to_string(),
                    ));
                }
            }
        }
        TitleKind::Markdown => {}
    }

    Ok(LabeledText {
        text,
        kind,
        span: None,
        selection: None,
    })
}

fn contains_bound_at(raw: &str) -> bool {
    raw.char_indices().any(|(index, ch)| {
        ch == '@'
            && raw[..index]
                .chars()
                .next_back()
                .is_none_or(|previous| !super::is_ecmascript_trim_char(previous))
    })
}

pub(super) fn parse_edge_text(raw: &str) -> std::result::Result<LabeledText, LexError> {
    if raw.is_empty() {
        return Err(LexError::new(
            "Flowchart edge label payload cannot be empty",
        ));
    }

    if raw.starts_with("\"`") {
        if raw.len() < 4 || !raw.ends_with("`\"") {
            return Err(LexError::new(
                "Invalid Markdown string in flowchart edge label",
            ));
        }
        let inner = &raw[2..raw.len() - 2];
        if inner.is_empty() || inner.contains(['`', '"']) {
            return Err(LexError::new(
                "Invalid Markdown string in flowchart edge label",
            ));
        }
    } else if raw.starts_with('"') {
        if raw.len() < 2 || !raw.ends_with('"') {
            return Err(LexError::new(
                "Invalid edge label: quoted strings cannot be mixed with unquoted text",
            ));
        }
        let inner = &raw[1..raw.len() - 1];
        if inner.is_empty() || inner.contains('"') {
            return Err(LexError::new(
                "Invalid edge label: expected exactly one quoted string token",
            ));
        }
    } else if raw.contains('"') {
        return Err(LexError::new(
            "Invalid edge label: quoted strings cannot be mixed with unquoted text",
        ));
    }

    let (text, kind) = super::parse_edge_label_text(raw);

    Ok(LabeledText {
        text,
        kind,
        span: None,
        selection: None,
    })
}

pub(super) fn parse_rect_border_label(raw: &str) -> (&'static str, &str, usize) {
    // Mermaid supports a special "rect" variant via `[|borders:...|Label]`.
    // The `[|` token is recognized lexically before FlowDB trim. Leading whitespace therefore
    // keeps this on the ordinary square-label path and is not allowed to synthesize a rect.
    let Some(rest) = raw.strip_prefix('|') else {
        return ("square", raw, 0);
    };
    let Some((prefix, label)) = rest.split_once('|') else {
        return ("square", raw, 0);
    };
    if prefix.starts_with("borders:") {
        let offset = 1 + prefix.len() + 1;
        return ("rect", label, offset);
    }
    ("square", raw, 0)
}

pub(super) fn find_unquoted_delim(input: &str, start: usize, delim: &str) -> Option<usize> {
    let bytes = input.as_bytes();
    let len = bytes.len();
    let delim_bytes = delim.as_bytes();
    let mut pos = start;

    while pos + delim_bytes.len() <= len {
        if bytes[pos..pos + delim_bytes.len()] == *delim_bytes {
            return Some(pos);
        }

        // Mermaid's flowchart lexer stays in a label-specific text state until the shape closer,
        // so newlines and semicolons inside node labels are label text rather than statement ends.
        match bytes[pos] {
            b'"' => {
                pos += 1;
                while pos < len {
                    if bytes[pos] == b'"' {
                        pos += 1;
                        break;
                    }
                    pos += 1;
                }
            }
            _ => pos += 1,
        }
    }

    None
}

fn split_first_word(s: &str) -> Option<(&str, &str)> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return None;
    }
    let bytes = trimmed.as_bytes();
    let mut i = 0;
    while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    let first = &trimmed[..i];
    let rest = &trimmed[i..];
    Some((first, rest))
}

fn parse_styles_list(s: &str) -> Vec<String> {
    // Used by `classDef` / `style` statements. Mermaid normalizes these style tokens by trimming
    // whitespace around each comma-separated entry.
    let placeholder = "\u{0000}";
    let replaced = s.replace("\\,", placeholder);
    replaced
        .split(',')
        .map(|p| p.replace(placeholder, ","))
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

fn parse_linkstyle_styles_list(s: &str) -> Vec<String> {
    // Mermaid's Jison grammar preserves whitespace inside each style token (e.g. `, stroke: ...`
    // becomes `" stroke: ..."`) and downstream FlowDB joins the style list verbatim via
    // `styles.join(';')` (see `flow.jison` + `flowDb.updateLink(...)`).
    //
    // Keep the raw spacing (except for filtering out all-whitespace entries).
    let placeholder = "\u{0000}";
    let replaced = s.replace("\\,", placeholder);
    replaced
        .split(',')
        .map(|p| p.replace(placeholder, ","))
        .filter(|p| !p.trim().is_empty())
        .collect()
}

pub(super) fn parse_style_stmt(rest: &str) -> std::result::Result<StyleStmt, LexError> {
    let Some((target, styles_raw)) = split_first_word(rest) else {
        return Err(LexError::new("Invalid style statement".to_string()));
    };
    let styles = parse_styles_list(styles_raw);
    Ok(StyleStmt {
        target: target.trim().to_string(),
        target_span: None,
        styles,
        styles_text: None,
        styles_span: None,
        editor_evidence: Default::default(),
    })
}

pub(super) fn parse_classdef_stmt(rest: &str) -> std::result::Result<ClassDefStmt, LexError> {
    let Some((ids_raw, styles_raw)) = split_first_word(rest) else {
        return Err(LexError::new("Invalid classDef statement".to_string()));
    };
    let ids = ids_raw
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>();
    let styles = parse_styles_list(styles_raw);
    Ok(ClassDefStmt {
        ids,
        id_spans: Vec::new(),
        styles,
        styles_text: None,
        styles_span: None,
        editor_evidence: Default::default(),
    })
}

pub(super) fn parse_class_assign_stmt(
    rest: &str,
) -> std::result::Result<ClassAssignStmt, LexError> {
    let Some((targets_raw, class_raw)) = split_first_word(rest) else {
        return Err(LexError::new("Invalid class statement".to_string()));
    };
    let targets = targets_raw
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>();
    let class_name = class_raw.trim().to_string();
    if class_name.is_empty() {
        return Err(LexError::new("Invalid class statement".to_string()));
    }
    Ok(ClassAssignStmt {
        targets,
        target_spans: Vec::new(),
        class_name,
        class_name_span: None,
        editor_evidence: Default::default(),
    })
}

pub(super) fn attach_style_stmt_spans(stmt: &mut StyleStmt, rest: &str, rest_start: usize) {
    let Some((target, styles)) = split_first_word_with_span(rest, rest_start) else {
        return;
    };
    stmt.target_span = Some(target.span);
    if let Some(styles) = trim_spanned_slice(styles) {
        stmt.styles_text = Some(styles.text.to_string());
        stmt.styles_span = Some(styles.span);
    }
}

pub(super) fn attach_classdef_stmt_spans(stmt: &mut ClassDefStmt, rest: &str, rest_start: usize) {
    let Some((ids, styles)) = split_first_word_with_span(rest, rest_start) else {
        return;
    };
    stmt.id_spans = split_comma_value_spans(ids.text, ids.span.start);
    if let Some(styles) = trim_spanned_slice(styles) {
        stmt.styles_text = Some(styles.text.to_string());
        stmt.styles_span = Some(styles.span);
    }
}

pub(super) fn attach_class_assign_stmt_spans(
    stmt: &mut ClassAssignStmt,
    rest: &str,
    rest_start: usize,
) {
    let Some((targets, class_name)) = split_first_word_with_span(rest, rest_start) else {
        return;
    };
    stmt.target_spans = split_comma_value_spans(targets.text, targets.span.start);
    stmt.class_name_span = trim_spanned_slice(class_name).map(|class_name| class_name.span);
}

#[derive(Clone, Copy)]
struct SpannedSlice<'a> {
    text: &'a str,
    span: SourceSpan,
}

fn split_first_word_with_span(
    rest: &str,
    rest_start: usize,
) -> Option<(SpannedSlice<'_>, SpannedSlice<'_>)> {
    let leading = rest.len().saturating_sub(rest.trim_start().len());
    let trimmed = &rest[leading..];
    if trimmed.is_empty() {
        return None;
    }

    let mut first_len = 0usize;
    while first_len < trimmed.len() && !trimmed.as_bytes()[first_len].is_ascii_whitespace() {
        first_len += 1;
    }

    let first_start = rest_start + leading;
    let rest_after_first_start = first_start + first_len;
    Some((
        SpannedSlice {
            text: &trimmed[..first_len],
            span: SourceSpan::new(first_start, rest_after_first_start),
        },
        SpannedSlice {
            text: &trimmed[first_len..],
            span: SourceSpan::new(rest_after_first_start, rest_start + rest.len()),
        },
    ))
}

fn trim_spanned_slice(slice: SpannedSlice<'_>) -> Option<SpannedSlice<'_>> {
    let leading = slice
        .text
        .len()
        .saturating_sub(slice.text.trim_start().len());
    let text = &slice.text[leading..];
    let trimmed_len = text.trim_end().len();
    if trimmed_len == 0 {
        return None;
    }
    let start = slice.span.start + leading;
    Some(SpannedSlice {
        text: &text[..trimmed_len],
        span: SourceSpan::new(start, start + trimmed_len),
    })
}

fn split_comma_value_spans(text: &str, text_start: usize) -> Vec<SourceSpan> {
    let mut out = Vec::new();
    let mut value_start = 0usize;
    let bytes = text.as_bytes();

    for idx in 0..=bytes.len() {
        if idx != bytes.len() && bytes[idx] != b',' {
            continue;
        }

        let value = &text[value_start..idx];
        if let Some(value) = trim_spanned_slice(SpannedSlice {
            text: value,
            span: SourceSpan::new(text_start + value_start, text_start + idx),
        }) {
            out.push(value.span);
        }
        value_start = idx.saturating_add(1);
    }

    out
}

struct ClickParse<'a> {
    s: &'a str,
    base: usize,
    i: usize,
}

impl<'a> ClickParse<'a> {
    fn new(s: &'a str, base: usize) -> Self {
        Self { s, base, i: 0 }
    }

    fn skip_ws(&mut self) {
        while self.i < self.s.len() && self.s.as_bytes()[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.s.as_bytes().get(self.i).copied()
    }

    fn take_word(&mut self) -> Option<String> {
        self.skip_ws();
        let start = self.i;
        while self.i < self.s.len() && !self.s.as_bytes()[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
        if self.i == start {
            return None;
        }
        Some(self.s[start..self.i].to_string())
    }

    fn take_quoted(&mut self) -> Option<String> {
        self.skip_ws();
        if self.peek()? != b'"' {
            return None;
        }
        self.i += 1;
        let start = self.i;
        while self.i < self.s.len() && self.s.as_bytes()[self.i] != b'"' {
            self.i += 1;
        }
        let out = self.s[start..self.i].to_string();
        if self.i < self.s.len() && self.s.as_bytes()[self.i] == b'"' {
            self.i += 1;
        }
        Some(out)
    }

    fn rest(&self) -> &str {
        &self.s[self.i..]
    }

    fn source_span(&self, start: usize, end: usize) -> SourceSpan {
        SourceSpan::new(self.base + start, self.base + end)
    }
}

fn click_following_span(parser: &ClickParse<'_>, boundary_end: usize) -> Option<SourceSpan> {
    (parser.i > boundary_end).then(|| {
        parser.source_span(
            boundary_end.saturating_add(1).min(parser.s.len()),
            parser.s.len(),
        )
    })
}

fn click_action_prefix(word: &str) -> bool {
    !word.is_empty() && ("href".starts_with(word) || "call".starts_with(word))
}

fn invalid_click_statement(evidence: FlowchartClickEditorEvidence) -> LexError {
    evidence.iter().fold(
        LexError::new("Invalid click statement".to_string()),
        |error, expected| error.expecting(expected.kind, expected.span),
    )
}

pub(super) fn parse_click_stmt(
    rest: &str,
    rest_start: usize,
) -> std::result::Result<ClickStmt, LexError> {
    let mut p = ClickParse::new(rest, rest_start);
    p.skip_ws();
    let id_start = p.i;
    let Some(id) = p.take_word() else {
        return Err(LexError::new("Invalid click statement".to_string()));
    };
    let ids = vec![id];
    let id_spans = vec![p.source_span(id_start, p.i)];

    let target_end = p.i;
    p.skip_ws();
    let after_target = click_following_span(&p, target_end);
    let tooltip: Option<String>;
    let action: ClickAction;

    if p.rest().starts_with("href")
        && p.rest()
            .as_bytes()
            .get(4)
            .is_none_or(|b| b.is_ascii_whitespace())
    {
        let action_start = p.i;
        let _ = p.take_word();
        let action_end = p.i;
        let action_span = p.source_span(action_start, action_end);
        p.skip_ws();
        let interaction_evidence = FlowchartClickEditorEvidence::new(
            Some(action_span),
            click_following_span(&p, action_end),
        );
        let Some(link) = p.take_quoted() else {
            return Err(invalid_click_statement(interaction_evidence));
        };
        let maybe_tt = p.take_quoted();
        let maybe_target = p.take_word().filter(|w| w.starts_with('_'));
        tooltip = maybe_tt;
        action = ClickAction::Link {
            href: link,
            target: maybe_target,
        };
        return Ok(ClickStmt {
            ids,
            id_spans,
            tooltip,
            action,
            editor_evidence: Default::default(),
            interaction_evidence,
            recovery_error: None,
        });
    }

    if p.rest().starts_with("call")
        && p.rest()
            .as_bytes()
            .get(4)
            .is_none_or(|b| b.is_ascii_whitespace())
    {
        let action_start = p.i;
        let _ = p.take_word();
        let action_end = p.i;
        p.skip_ws();
        let interaction_evidence = FlowchartClickEditorEvidence::new(
            Some(p.source_span(action_start, action_end)),
            click_following_span(&p, action_end),
        );
        let start = p.i;
        while p.i < p.s.len() {
            let b = p.s.as_bytes()[p.i];
            if b.is_ascii_whitespace() || b == b'(' {
                break;
            }
            p.i += 1;
        }
        if p.i == start {
            return Err(invalid_click_statement(interaction_evidence));
        }
        p.skip_ws();
        if p.peek() == Some(b'(') {
            p.i += 1;
            while p.i < p.s.len() && p.s.as_bytes()[p.i] != b')' {
                p.i += 1;
            }
            if p.peek() == Some(b')') {
                p.i += 1;
            }
        }

        tooltip = p.take_quoted();
        action = ClickAction::Callback;
        return Ok(ClickStmt {
            ids,
            id_spans,
            tooltip,
            action,
            editor_evidence: Default::default(),
            interaction_evidence,
            recovery_error: None,
        });
    }

    if let Some(link) = p.take_quoted() {
        let maybe_tt = p.take_quoted();
        let maybe_target = p.take_word().filter(|w| w.starts_with('_'));
        tooltip = maybe_tt;
        action = ClickAction::Link {
            href: link,
            target: maybe_target,
        };
        return Ok(ClickStmt {
            ids,
            id_spans,
            tooltip,
            action,
            editor_evidence: Default::default(),
            interaction_evidence: FlowchartClickEditorEvidence::new(None, after_target),
            recovery_error: None,
        });
    }

    let function_start = p.i;
    let Some(function_name) = p.take_word() else {
        return Err(invalid_click_statement(FlowchartClickEditorEvidence::new(
            after_target,
            None,
        )));
    };
    let function_span = p.source_span(function_start, p.i);
    let interaction_evidence = if click_action_prefix(&function_name) {
        FlowchartClickEditorEvidence::new(Some(function_span), None)
    } else {
        FlowchartClickEditorEvidence::new(None, after_target)
    };
    tooltip = p.take_quoted();
    action = ClickAction::Callback;
    Ok(ClickStmt {
        ids,
        id_spans,
        tooltip,
        action,
        editor_evidence: Default::default(),
        interaction_evidence,
        recovery_error: None,
    })
}

pub(super) fn parse_link_style_stmt(
    rest: &str,
    rest_start: usize,
) -> std::result::Result<LinkStyleStmt, LexError> {
    let mut p = ClickParse::new(rest, rest_start);
    p.skip_ws();
    let Some(pos_raw) = p.take_word() else {
        return Err(LexError::new("Invalid linkStyle statement".to_string()));
    };

    let positions = if pos_raw == "default" {
        vec![LinkStylePos::Default]
    } else {
        pos_raw
            .split(',')
            .map(|s| {
                let idx = s
                    .trim()
                    .parse::<usize>()
                    .map_err(|_| LexError::new("Invalid linkStyle statement".to_string()))?;
                Ok(LinkStylePos::Index(idx))
            })
            .collect::<std::result::Result<Vec<_>, LexError>>()?
    };

    p.skip_ws();
    let mut interpolate: Option<String> = None;
    if p.rest().starts_with("interpolate")
        && p.rest()
            .as_bytes()
            .get("interpolate".len())
            .is_none_or(|b| b.is_ascii_whitespace())
    {
        let _ = p.take_word();
        interpolate = p.take_word();
    }

    // Mermaid's `linkStyle ... interpolate <curve> ...` still tokenizes the styles list without the
    // leading whitespace between the curve name and the first style token. Keep the whitespace
    // inside comma-separated tokens (handled by `parse_linkstyle_styles_list`), but drop the
    // leading separator spaces at the list boundary.
    p.skip_ws();
    let styles = parse_linkstyle_styles_list(p.rest());
    Ok(LinkStyleStmt {
        positions,
        interpolate,
        styles,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_click_stmt_parses_callback() {
        let stmt = parse_click_stmt("A callback", 0).unwrap();
        assert_eq!(stmt.ids, vec!["A"]);
        assert!(stmt.tooltip.is_none());
        match stmt.action {
            ClickAction::Callback => {}
            _ => panic!("expected callback action"),
        }
    }

    #[test]
    fn parse_click_stmt_parses_call_callback_empty_args() {
        let stmt = parse_click_stmt("A call callback()", 0).unwrap();
        assert_eq!(stmt.ids, vec!["A"]);
        assert!(stmt.tooltip.is_none());
        match stmt.action {
            ClickAction::Callback => {}
            _ => panic!("expected callback action"),
        }
    }

    #[test]
    fn parse_click_stmt_parses_call_callback_with_args() {
        let stmt = parse_click_stmt("A call callback(\"test0\", test1, test2)", 0).unwrap();
        match stmt.action {
            ClickAction::Callback => {}
            _ => panic!("expected callback action"),
        }
    }

    #[test]
    fn parse_click_stmt_parses_link_and_tooltip_and_target() {
        let stmt = parse_click_stmt("A \"click.html\" \"tooltip\" _blank", 0).unwrap();
        assert_eq!(stmt.tooltip.as_deref(), Some("tooltip"));
        match stmt.action {
            ClickAction::Link { href, target } => {
                assert_eq!(href, "click.html");
                assert_eq!(target.as_deref(), Some("_blank"));
            }
            _ => panic!("expected link action"),
        }
    }

    #[test]
    fn parse_click_stmt_parses_href_link_and_tooltip_and_target() {
        let stmt = parse_click_stmt("A href \"click.html\" \"tooltip\" _blank", 0).unwrap();
        assert_eq!(stmt.tooltip.as_deref(), Some("tooltip"));
        match stmt.action {
            ClickAction::Link { href, target } => {
                assert_eq!(href, "click.html");
                assert_eq!(target.as_deref(), Some("_blank"));
            }
            _ => panic!("expected link action"),
        }
    }
}
