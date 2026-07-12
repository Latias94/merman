use crate::diagrams::scan::strip_line_ending;
use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
    EditorSemanticSymbol, Error, ParseMetadata, Result, SourceSpan,
};
use serde_json::{Value, json};

const HEADER: &str = "cynefin-beta";
const DOMAINS: &[&str] = &["complex", "complicated", "clear", "chaotic", "confusion"];

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct CynefinItemModel {
    pub label: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct CynefinDomainModel {
    pub name: String,
    #[serde(default)]
    pub items: Vec<CynefinItemModel>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct CynefinTransitionModel {
    pub from: String,
    pub to: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct CynefinDiagramModel {
    #[serde(default, rename = "accTitle")]
    pub acc_title: Option<String>,
    #[serde(default, rename = "accDescr")]
    pub acc_descr: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub domains: Vec<CynefinDomainModel>,
    #[serde(default)]
    pub transitions: Vec<CynefinTransitionModel>,
}

pub type CynefinDiagramRenderModel = CynefinDiagramModel;

impl CynefinDiagramModel {
    pub(crate) fn sanitize_common_db_fields(&mut self, config: &crate::MermaidConfig) {
        crate::common_db::sanitize_optional_title(&mut self.title, config);
        crate::common_db::sanitize_optional_acc_title(&mut self.acc_title, config);
        crate::common_db::sanitize_optional_acc_descr(&mut self.acc_descr, config);
    }
}

#[derive(Debug, Clone)]
struct SpannedText {
    text: String,
    span: SourceSpan,
    selection: SourceSpan,
}

#[derive(Debug, Clone)]
struct TransitionParts {
    from: SpannedText,
    to: SpannedText,
    label: Option<SpannedText>,
}

#[derive(Debug, Clone, Copy)]
enum CommonFieldKind {
    Title,
    AccTitle,
    AccDescr,
}

#[derive(Debug, Clone)]
struct CommonField {
    kind: CommonFieldKind,
    value: SpannedText,
}

pub fn parse_cynefin(code: &str, meta: &ParseMetadata) -> Result<Value> {
    let mut model = parse_cynefin_model(code, meta)?;
    model.sanitize_common_db_fields(&meta.effective_config);

    Ok(json!({
        "type": meta.diagram_type,
        "title": model.title,
        "accTitle": model.acc_title,
        "accDescr": model.acc_descr,
        "domains": model.domains,
        "transitions": model.transitions,
    }))
}

pub fn parse_cynefin_model_for_render(
    code: &str,
    meta: &ParseMetadata,
) -> Result<CynefinDiagramRenderModel> {
    let mut model = parse_cynefin_model(code, meta)?;
    model.sanitize_common_db_fields(&meta.effective_config);
    Ok(model)
}

pub fn parse_cynefin_editor_facts(code: &str, _meta: &ParseMetadata) -> EditorSemanticFacts {
    let mut facts = EditorSemanticFacts::new();
    let mut offset = 0usize;
    let mut saw_header = false;
    let mut current_domain: Option<String> = None;

    while offset < code.len() {
        let line_start = offset;
        let (segment, next_offset) = physical_line_at(code, offset);
        offset = next_offset;
        let line = strip_line_ending(segment);
        let stripped = strip_inline_comment_aware(line);
        let trimmed = stripped.trim();
        if trimmed.is_empty() {
            continue;
        }

        if !saw_header {
            if is_header(trimmed) {
                saw_header = true;
                continue;
            }
            facts.mark_recovered_with_diagnostic(
                "expected cynefin-beta header",
                Some(SourceSpan::new(line_start, line_start + trimmed.len())),
            );
            return facts;
        }

        if let Some((field, consumed)) =
            parse_multiline_acc_descr_spanned(&code[line_start..], line_start)
        {
            offset = line_start + consumed;
            current_domain = None;
            push_common_field_fact(&mut facts, field);
            continue;
        }

        if let Some(field) = parse_common_field_spanned(stripped, line_start) {
            current_domain = None;
            push_common_field_fact(&mut facts, field);
            continue;
        }

        if let Some(domain) = parse_domain_line_spanned(stripped, line_start) {
            current_domain = Some(domain.text.clone());
            push_domain_fact(&mut facts, domain, "cynefin domain");
            continue;
        }

        if let Some(item) = parse_quoted_line_spanned(stripped, line_start) {
            if current_domain.is_some() {
                push_payload_fact(
                    &mut facts,
                    item,
                    "cynefin domain item",
                    EditorSemanticKind::String,
                );
                continue;
            }
            facts.mark_recovered_with_diagnostic(
                "cynefin item must follow a domain",
                Some(SourceSpan::new(line_start, line_start + trimmed.len())),
            );
            return facts;
        }

        match parse_transition_spanned(stripped, line_start) {
            Ok(Some(transition)) => {
                current_domain = None;
                push_domain_fact(
                    &mut facts,
                    transition.from.clone(),
                    "cynefin transition source",
                );
                push_domain_fact(
                    &mut facts,
                    transition.to.clone(),
                    "cynefin transition target",
                );
                if let Some(label) = transition.label {
                    push_payload_fact(
                        &mut facts,
                        label,
                        "cynefin transition label",
                        EditorSemanticKind::String,
                    );
                }
                if transition.from.text == transition.to.text {
                    facts.push_diagnostic(
                        format!(
                            "cynefin self-loop transition on domain \"{}\" is skipped",
                            transition.from.text
                        ),
                        Some(transition.from.span),
                    );
                }
            }
            Ok(None) => {
                facts.mark_recovered_with_diagnostic(
                    "expected cynefin domain, quoted item, transition, or common directive",
                    Some(SourceSpan::new(line_start, line_start + trimmed.len())),
                );
                return facts;
            }
            Err(err) => {
                facts.mark_recovered_with_diagnostic(
                    format!("cynefin parser recovered after parse error: {err}"),
                    Some(SourceSpan::new(line_start, line_start + trimmed.len())),
                );
                return facts;
            }
        }
    }

    if !saw_header {
        facts.mark_recovered_with_diagnostic(
            "expected cynefin-beta header",
            Some(SourceSpan::new(0, 0)),
        );
    }

    facts
}

fn parse_cynefin_model(code: &str, meta: &ParseMetadata) -> Result<CynefinDiagramModel> {
    let mut model = CynefinDiagramModel::default();
    let mut saw_header = false;
    let mut current_domain: Option<usize> = None;
    let mut offset = 0usize;

    while offset < code.len() {
        let line_start = offset;
        let (segment, next_offset) = physical_line_at(code, offset);
        offset = next_offset;
        let line = strip_line_ending(segment);
        let stripped = strip_inline_comment_aware(line);
        let trimmed = stripped.trim();
        if trimmed.is_empty() {
            continue;
        }

        if !saw_header {
            if is_header(trimmed) {
                saw_header = true;
                continue;
            }
            return Err(parse_error(meta, "expected cynefin-beta header"));
        }

        if let Some((field, consumed)) =
            parse_multiline_acc_descr_spanned(&code[line_start..], line_start)
        {
            offset = line_start + consumed;
            current_domain = None;
            model.acc_descr = Some(field.value.text);
            continue;
        }

        if let Some(field) = parse_common_field_spanned(stripped, 0) {
            current_domain = None;
            match field.kind {
                CommonFieldKind::Title => model.title = Some(field.value.text),
                CommonFieldKind::AccTitle => model.acc_title = Some(field.value.text),
                CommonFieldKind::AccDescr => model.acc_descr = Some(field.value.text),
            }
            continue;
        }

        if let Some(domain) = parse_domain_line_spanned(stripped, 0) {
            current_domain = Some(start_domain(&mut model.domains, domain.text));
            continue;
        }

        if let Some(item) = parse_quoted_line_spanned(stripped, 0) {
            let Some(domain_idx) = current_domain else {
                return Err(parse_error(meta, "cynefin item must follow a domain"));
            };
            model.domains[domain_idx]
                .items
                .push(CynefinItemModel { label: item.text });
            continue;
        }

        match parse_transition_spanned(stripped, 0)? {
            Some(transition) => {
                current_domain = None;
                if transition.from.text != transition.to.text {
                    model.transitions.push(CynefinTransitionModel {
                        from: transition.from.text,
                        to: transition.to.text,
                        label: transition
                            .label
                            .map(|label| label.text)
                            .filter(|label| !label.is_empty()),
                    });
                }
            }
            None => {
                return Err(parse_error(
                    meta,
                    "expected cynefin domain, quoted item, transition, or common directive",
                ));
            }
        }
    }

    if !saw_header {
        return Err(parse_error(meta, "expected cynefin-beta header"));
    }

    Ok(model)
}

fn is_header(trimmed: &str) -> bool {
    trimmed == HEADER || trimmed == "cynefin-beta:"
}

fn physical_line_at(code: &str, start: usize) -> (&str, usize) {
    let rest = &code[start..];
    let len = rest.find('\n').map_or(rest.len(), |index| index + 1);
    (&rest[..len], start + len)
}

fn start_domain(domains: &mut Vec<CynefinDomainModel>, name: String) -> usize {
    if let Some(idx) = domains.iter().position(|domain| domain.name == name) {
        domains[idx].items.clear();
        idx
    } else {
        domains.push(CynefinDomainModel {
            name,
            items: Vec::new(),
        });
        domains.len() - 1
    }
}

fn parse_domain_line_spanned(line: &str, line_start: usize) -> Option<SpannedText> {
    let trimmed = line.trim();
    if !DOMAINS.contains(&trimmed) {
        return None;
    }
    let rel = line.find(trimmed)?;
    Some(SpannedText {
        text: trimmed.to_string(),
        span: SourceSpan::new(line_start + rel, line_start + rel + trimmed.len()),
        selection: SourceSpan::new(line_start + rel, line_start + rel + trimmed.len()),
    })
}

fn parse_transition_spanned(line: &str, line_start: usize) -> Result<Option<TransitionParts>> {
    let mut cursor = CynefinCursor::new(line, line_start);
    cursor.skip_ws();
    let Some(from) = cursor.take_domain() else {
        return Ok(None);
    };
    cursor.skip_ws();
    if !cursor.take_literal("-->") {
        return Ok(None);
    }
    cursor.skip_ws();
    let Some(to) = cursor.take_domain() else {
        return Err(Error::diagram_parse_fallback(
            "cynefin",
            "expected cynefin transition target",
        ));
    };
    cursor.skip_ws();

    let label = if cursor.is_eof() {
        None
    } else {
        if !cursor.take_literal(":") {
            return Err(Error::diagram_parse_fallback(
                "cynefin",
                "expected ':' before cynefin transition label",
            ));
        }
        cursor.skip_ws();
        Some(cursor.take_quoted_string().ok_or_else(|| {
            Error::diagram_parse_fallback("cynefin", "expected quoted cynefin transition label")
        })?)
    };
    cursor.skip_ws();
    if !cursor.is_eof() {
        return Err(Error::diagram_parse_fallback(
            "cynefin",
            "unexpected trailing cynefin transition tokens",
        ));
    }

    Ok(Some(TransitionParts { from, to, label }))
}

fn parse_quoted_line_spanned(line: &str, line_start: usize) -> Option<SpannedText> {
    let mut cursor = CynefinCursor::new(line, line_start);
    cursor.skip_ws();
    let value = cursor.take_quoted_string()?;
    cursor.skip_ws();
    cursor.is_eof().then_some(value)
}

fn parse_common_field_spanned(line: &str, line_start: usize) -> Option<CommonField> {
    let title = parse_title_spanned(line, line_start);
    if title.is_some() {
        return title;
    }
    let acc_title = parse_acc_title_spanned(line, line_start);
    if acc_title.is_some() {
        return acc_title;
    }
    parse_acc_descr_spanned(line, line_start)
}

fn parse_title_spanned(line: &str, line_start: usize) -> Option<CommonField> {
    let trimmed = line.trim_start();
    let leading = line.len() - trimmed.len();
    if trimmed == "title" {
        let offset = line_start + leading + "title".len();
        return Some(CommonField {
            kind: CommonFieldKind::Title,
            value: SpannedText {
                text: String::new(),
                span: SourceSpan::new(offset, offset),
                selection: SourceSpan::new(offset, offset),
            },
        });
    }
    let rest = trimmed.strip_prefix("title")?;
    let ws = rest.chars().next()?;
    if !ws.is_whitespace() {
        return None;
    }
    let value = rest.trim();
    let value_rel = leading + "title".len() + (rest.len() - rest.trim_start().len());
    Some(CommonField {
        kind: CommonFieldKind::Title,
        value: SpannedText {
            text: normalize_single_line_common_value(value),
            span: SourceSpan::new(line_start + value_rel, line_start + value_rel + value.len()),
            selection: SourceSpan::new(
                line_start + value_rel,
                line_start + value_rel + value.len(),
            ),
        },
    })
}

fn parse_acc_title_spanned(line: &str, line_start: usize) -> Option<CommonField> {
    let trimmed = line.trim_start();
    let leading = line.len() - trimmed.len();
    let after_keyword = trimmed.strip_prefix("accTitle")?;
    let before_colon = after_keyword.len() - after_keyword.trim_start().len();
    let after_colon = after_keyword.trim_start().strip_prefix(':')?;
    let value = after_colon.trim();
    let after_colon_leading = after_colon.len() - after_colon.trim_start().len();
    let value_rel = leading + "accTitle".len() + before_colon + 1 + after_colon_leading;
    Some(CommonField {
        kind: CommonFieldKind::AccTitle,
        value: SpannedText {
            text: normalize_single_line_common_value(value),
            span: SourceSpan::new(line_start + value_rel, line_start + value_rel + value.len()),
            selection: SourceSpan::new(
                line_start + value_rel,
                line_start + value_rel + value.len(),
            ),
        },
    })
}

fn parse_acc_descr_spanned(line: &str, line_start: usize) -> Option<CommonField> {
    let trimmed = line.trim_start();
    let leading = line.len() - trimmed.len();
    let after_keyword = trimmed.strip_prefix("accDescr")?;
    let before_delimiter = after_keyword.len() - after_keyword.trim_start().len();
    let rest = after_keyword.trim_start();
    if let Some(after_colon) = rest.strip_prefix(':') {
        let value = after_colon.trim();
        let after_colon_leading = after_colon.len() - after_colon.trim_start().len();
        let value_rel = leading + "accDescr".len() + before_delimiter + 1 + after_colon_leading;
        return Some(CommonField {
            kind: CommonFieldKind::AccDescr,
            value: SpannedText {
                text: normalize_single_line_common_value(value),
                span: SourceSpan::new(line_start + value_rel, line_start + value_rel + value.len()),
                selection: SourceSpan::new(
                    line_start + value_rel,
                    line_start + value_rel + value.len(),
                ),
            },
        });
    }
    None
}

fn parse_multiline_acc_descr_spanned(
    input: &str,
    input_start: usize,
) -> Option<(CommonField, usize)> {
    let trimmed = input.trim_start_matches([' ', '\t']);
    let leading = input.len() - trimmed.len();
    let after_keyword = trimmed.strip_prefix("accDescr")?;
    let whitespace_len = after_keyword
        .chars()
        .take_while(|ch| ch.is_whitespace())
        .map(char::len_utf8)
        .sum::<usize>();
    let after_whitespace = &after_keyword[whitespace_len..];
    let after_open = after_whitespace.strip_prefix('{')?;
    let close_rel = after_open.find('}')?;

    let open_rel = leading + "accDescr".len() + whitespace_len;
    let value_start_rel = open_rel + 1;
    let value_end_rel = value_start_rel + close_rel;
    let after_close_rel = value_end_rel + 1;
    let closing_line_len = input[after_close_rel..]
        .find('\n')
        .map_or(input.len() - after_close_rel, |index| index + 1);
    let consumed = after_close_rel + closing_line_len;
    let trailing = strip_line_ending(&input[after_close_rel..consumed]);
    if !strip_inline_comment_aware(trailing).trim().is_empty() {
        return None;
    }

    let raw_value = &input[value_start_rel..value_end_rel];
    let value = SpannedText {
        text: normalize_multiline_common_value(raw_value),
        span: SourceSpan::new(input_start + value_start_rel, input_start + value_end_rel),
        selection: SourceSpan::new(input_start + value_start_rel, input_start + value_end_rel),
    };
    Some((
        CommonField {
            kind: CommonFieldKind::AccDescr,
            value,
        },
        consumed,
    ))
}

fn normalize_single_line_common_value(value: &str) -> String {
    let value = value.trim();
    let mut normalized = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();

    while let Some(ch) = chars.next() {
        if !matches!(ch, ' ' | '\t') {
            normalized.push(ch);
            continue;
        }

        let mut run_len = 1usize;
        while chars.peek().is_some_and(|next| matches!(next, ' ' | '\t')) {
            chars.next();
            run_len += 1;
        }
        if run_len == 1 {
            normalized.push(ch);
        } else {
            normalized.push(' ');
        }
    }

    normalized
}

fn normalize_multiline_common_value(value: &str) -> String {
    value
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(normalize_single_line_common_value)
        .collect::<Vec<_>>()
        .join("\n")
}

fn push_common_field_fact(facts: &mut EditorSemanticFacts, field: CommonField) {
    match field.kind {
        CommonFieldKind::Title => {
            facts.push_directive_prefix("title");
            push_payload_fact(
                facts,
                field.value,
                "cynefin title",
                EditorSemanticKind::String,
            );
        }
        CommonFieldKind::AccTitle => {
            facts.push_directive_prefix("accTitle");
            push_payload_fact(
                facts,
                field.value,
                "cynefin accessibility title",
                EditorSemanticKind::String,
            );
        }
        CommonFieldKind::AccDescr => {
            facts.push_directive_prefix("accDescr");
            push_payload_fact(
                facts,
                field.value,
                "cynefin accessibility description",
                EditorSemanticKind::String,
            );
        }
    }
}

fn push_domain_fact(facts: &mut EditorSemanticFacts, domain: SpannedText, detail: &'static str) {
    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::NodeIdentifier,
        domain.selection,
    ));
    facts.push_symbol(EditorSemanticSymbol::outline(
        domain.text,
        Some(detail.to_string()),
        EditorSemanticKind::Namespace,
        domain.span,
        domain.selection,
    ));
}

fn push_payload_fact(
    facts: &mut EditorSemanticFacts,
    value: SpannedText,
    detail: &'static str,
    kind: EditorSemanticKind,
) {
    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::Payload,
        value.selection,
    ));
    facts.push_symbol(EditorSemanticSymbol::payload(
        value.text,
        Some(detail.to_string()),
        kind,
        value.span,
        value.selection,
    ));
}

struct CynefinCursor<'a> {
    input: &'a str,
    line_start: usize,
    pos: usize,
}

impl<'a> CynefinCursor<'a> {
    fn new(input: &'a str, line_start: usize) -> Self {
        Self {
            input,
            line_start,
            pos: 0,
        }
    }

    fn remaining(&self) -> &'a str {
        &self.input[self.pos..]
    }

    fn is_eof(&self) -> bool {
        self.pos >= self.input.len()
    }

    fn skip_ws(&mut self) {
        self.pos += self
            .remaining()
            .chars()
            .take_while(|ch| matches!(ch, ' ' | '\t'))
            .map(char::len_utf8)
            .sum::<usize>();
    }

    fn take_literal(&mut self, literal: &str) -> bool {
        if self.remaining().starts_with(literal) {
            self.pos += literal.len();
            true
        } else {
            false
        }
    }

    fn take_domain(&mut self) -> Option<SpannedText> {
        let rest = self.remaining();
        for domain in DOMAINS {
            let Some(after) = rest.strip_prefix(domain) else {
                continue;
            };
            if after
                .chars()
                .next()
                .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_')
            {
                continue;
            }
            let start = self.line_start + self.pos;
            self.pos += domain.len();
            return Some(SpannedText {
                text: (*domain).to_string(),
                span: SourceSpan::new(start, start + domain.len()),
                selection: SourceSpan::new(start, start + domain.len()),
            });
        }
        None
    }

    fn take_quoted_string(&mut self) -> Option<SpannedText> {
        let start = self.line_start + self.pos;
        let rest = self.remaining();
        let mut chars = rest.char_indices();
        let (_, quote) = chars.next()?;
        if !matches!(quote, '"' | '\'') {
            return None;
        }
        let mut text = String::new();
        let mut escaped = false;
        for (idx, ch) in chars {
            if escaped {
                text.push(match ch {
                    'b' => '\u{0008}',
                    'f' => '\u{000c}',
                    'n' => '\n',
                    'r' => '\r',
                    't' => '\t',
                    'v' => '\u{000b}',
                    '0' => '\0',
                    _ => ch,
                });
                escaped = false;
                continue;
            }
            if ch == '\\' {
                escaped = true;
                continue;
            }
            if ch == quote {
                let consumed = idx + quote.len_utf8();
                self.pos += consumed;
                return Some(SpannedText {
                    text,
                    span: SourceSpan::new(start, start + consumed),
                    selection: SourceSpan::new(start + quote.len_utf8(), start + idx),
                });
            }
            text.push(ch);
        }
        None
    }
}

fn strip_inline_comment_aware(line: &str) -> &str {
    let mut in_single = false;
    let mut in_double = false;
    let mut escaped = false;
    let mut iter = line.char_indices().peekable();
    while let Some((idx, ch)) = iter.next() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' if in_single || in_double => escaped = true,
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            '%' if !in_single
                && !in_double
                && iter.peek().is_some_and(|(_, next)| *next == '%') =>
            {
                return &line[..idx];
            }
            _ => {}
        }
    }
    line
}

fn parse_error(meta: &ParseMetadata, message: impl Into<String>) -> Error {
    Error::diagram_parse_fallback(meta.diagram_type.clone(), message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_comment_markers_outside_quoted_strings() {
        assert_eq!(
            strip_inline_comment_aware("  complex %% comment"),
            "  complex "
        );
        assert_eq!(
            strip_inline_comment_aware("  \"100%% visible\" %% comment"),
            "  \"100%% visible\" "
        );
    }

    #[test]
    fn parses_escaped_quoted_string_payload() {
        let value = parse_quoted_line_spanned("  \"Probe \\\"quoted\\\" value\"", 10).unwrap();
        assert_eq!(value.text, "Probe \"quoted\" value");
        assert_eq!(value.selection, SourceSpan::new(13, 35));
    }
}
