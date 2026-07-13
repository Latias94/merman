use super::*;

#[derive(Debug, Clone)]
struct ArchitectureSpannedValue {
    value: String,
    span: SourceSpan,
    selection: SourceSpan,
}

#[derive(Debug, Clone, Copy)]
enum ArchitectureTraceFactRole {
    Entity,
    Payload,
}

#[derive(Debug, Clone)]
struct ArchitectureTraceFact {
    value: ArchitectureSpannedValue,
    detail: &'static str,
    kind: EditorSemanticKind,
    role: ArchitectureTraceFactRole,
    expected: EditorExpectedSyntaxKind,
}

#[derive(Debug, Clone)]
enum ArchitectureStatement {
    Title(ArchitectureSpannedValue),
    AccTitle(ArchitectureSpannedValue),
    AccDescr(ArchitectureSpannedValue),
    Group {
        id: ArchitectureIdentifier,
        icon: Option<ArchitectureSpannedValue>,
        title: Option<ArchitectureSpannedValue>,
        in_group: Option<ArchitectureIdentifier>,
    },
    Service {
        id: ArchitectureIdentifier,
        icon: Option<ArchitectureSpannedValue>,
        icon_text: Option<ArchitectureSpannedValue>,
        title: Option<ArchitectureSpannedValue>,
        in_group: Option<ArchitectureIdentifier>,
    },
    Junction {
        id: ArchitectureIdentifier,
        in_group: Option<ArchitectureIdentifier>,
    },
    Edge(ArchitectureEdge),
    Alignment {
        direction: ArchitectureSpannedValue,
        members: Vec<ArchitectureIdentifier>,
    },
}

#[derive(Debug, Clone)]
struct ArchitectureTraceEntry {
    statement: Option<ArchitectureStatement>,
    span: SourceSpan,
    facts: Vec<ArchitectureTraceFact>,
    diagnostic: Option<crate::ParseDiagnostic>,
    directive_prefix: Option<&'static str>,
}

#[derive(Debug, Clone, Default)]
struct ArchitectureTrace {
    entries: Vec<ArchitectureTraceEntry>,
}

#[derive(Debug)]
pub(super) struct ArchitectureSemanticSource {
    trace: ArchitectureTrace,
    db: ArchitectureDb,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArchitectureParseMode {
    Strict,
    Recovering,
}

struct StatementFailure {
    error: Box<Error>,
    facts: Vec<ArchitectureTraceFact>,
    directive_prefix: Option<&'static str>,
}

impl StatementFailure {
    fn new(error: Error, facts: Vec<ArchitectureTraceFact>) -> Self {
        Self {
            error: Box::new(error),
            facts,
            directive_prefix: None,
        }
    }

    fn with_directive_prefix(mut self, prefix: &'static str) -> Self {
        self.directive_prefix = Some(prefix);
        self
    }
}

struct ArchitectureStatementParser<'a> {
    input: &'a str,
    base_offset: usize,
    pos: usize,
    facts: Vec<ArchitectureTraceFact>,
}

impl<'a> ArchitectureStatementParser<'a> {
    fn new(input: &'a str, base_offset: usize) -> Self {
        Self {
            input,
            base_offset,
            pos: 0,
            facts: Vec::new(),
        }
    }

    fn parse(mut self) -> std::result::Result<ArchitectureTraceEntry, StatementFailure> {
        let span = SourceSpan::new(self.base_offset, self.base_offset + self.input.len());
        let statement = self.parse_statement();
        match statement {
            Ok(statement) => Ok(ArchitectureTraceEntry {
                statement: Some(statement),
                span,
                facts: self.facts,
                diagnostic: None,
                directive_prefix: None,
            }),
            Err(error) => Err(StatementFailure::new(error, self.facts)),
        }
    }

    fn parse_statement(&mut self) -> Result<ArchitectureStatement> {
        if self.next_is_keyword("group") {
            return self.parse_group();
        }
        if self.next_is_keyword("service") {
            return self.parse_service();
        }
        if self.next_is_keyword("junction") {
            return self.parse_junction();
        }
        if self.next_is_keyword("align") {
            return self.parse_alignment();
        }
        self.parse_edge()
    }

    fn parse_group(&mut self) -> Result<ArchitectureStatement> {
        self.expect_keyword("group");
        let id = self.parse_id(
            "invalid group id",
            "architecture group",
            EditorSemanticKind::Namespace,
        )?;
        let icon = self.parse_icon()?;
        if let Some(icon) = &icon {
            self.push_payload(
                icon.clone(),
                "architecture group icon",
                EditorSemanticKind::String,
            );
        }
        let title = self.parse_title()?;
        if let Some(title) = &title {
            self.push_payload(
                title.clone(),
                "architecture group title",
                EditorSemanticKind::String,
            );
        }
        let in_group = if self.consume_keyword("in") {
            Some(self.parse_id(
                "invalid group parent id",
                "architecture group parent",
                EditorSemanticKind::Namespace,
            )?)
        } else {
            None
        };
        self.finish()?;
        Ok(ArchitectureStatement::Group {
            id,
            icon,
            title,
            in_group,
        })
    }

    fn parse_service(&mut self) -> Result<ArchitectureStatement> {
        self.expect_keyword("service");
        let id = self.parse_id(
            "invalid service id",
            "architecture service",
            EditorSemanticKind::Variable,
        )?;
        let mut icon = None;
        let mut icon_text = None;
        self.skip_ws();
        match self.peek_char() {
            Some('(') => {
                icon = self.parse_icon()?;
                if let Some(value) = &icon {
                    self.push_payload(
                        value.clone(),
                        "architecture service icon",
                        EditorSemanticKind::String,
                    );
                }
            }
            Some('"' | '\'') => {
                icon_text = Some(self.parse_string()?);
                self.push_payload(
                    icon_text.clone().expect("icon text was just parsed"),
                    "architecture service icon text",
                    EditorSemanticKind::String,
                );
            }
            _ => {}
        }
        let title = self.parse_title()?;
        if let Some(title) = &title {
            self.push_payload(
                title.clone(),
                "architecture service title",
                EditorSemanticKind::String,
            );
        }
        let in_group = if self.consume_keyword("in") {
            Some(self.parse_id(
                "invalid service parent id",
                "architecture service parent",
                EditorSemanticKind::Namespace,
            )?)
        } else {
            None
        };
        self.finish()?;
        Ok(ArchitectureStatement::Service {
            id,
            icon,
            icon_text,
            title,
            in_group,
        })
    }

    fn parse_junction(&mut self) -> Result<ArchitectureStatement> {
        self.expect_keyword("junction");
        let id = self.parse_id(
            "invalid junction id",
            "architecture junction",
            EditorSemanticKind::Object,
        )?;
        let in_group = if self.consume_keyword("in") {
            Some(self.parse_id(
                "invalid junction parent id",
                "architecture junction parent",
                EditorSemanticKind::Namespace,
            )?)
        } else {
            None
        };
        self.finish()?;
        Ok(ArchitectureStatement::Junction { id, in_group })
    }

    fn parse_alignment(&mut self) -> Result<ArchitectureStatement> {
        self.expect_keyword("align");
        let direction = self.parse_raw_id("invalid align direction")?;
        if ArchitectureLayoutDirection::parse(&direction.value).is_none() {
            return Err(Error::diagram_parse_exact(
                "architecture",
                "invalid align direction",
                direction.selection,
            ));
        }
        self.push_payload(
            direction.clone(),
            "architecture alignment direction",
            EditorSemanticKind::String,
        );

        let mut members = Vec::new();
        while {
            self.skip_ws();
            !self.is_eof()
        } {
            members.push(self.parse_id(
                "invalid align member id",
                "architecture alignment member",
                EditorSemanticKind::Variable,
            )?);
        }
        if members.len() < 2 {
            return Err(self.insertion_error(&format!(
                "An align directive requires at least two members; got {}",
                members.len()
            )));
        }
        Ok(ArchitectureStatement::Alignment { direction, members })
    }

    fn parse_edge(&mut self) -> Result<ArchitectureStatement> {
        let lhs = self.parse_id(
            "invalid id",
            "architecture edge endpoint",
            EditorSemanticKind::Variable,
        )?;
        let lhs_group = self.parse_group_modifier();
        self.expect_literal(':', "expected ':' for lhs port")?;
        let lhs_direction =
            self.parse_direction("expected lhs direction", "invalid lhs direction")?;
        self.push_payload(
            lhs_direction.clone(),
            "architecture edge lhs direction",
            EditorSemanticKind::String,
        );
        let lhs_into = self.parse_arrow_into();

        self.skip_ws();
        let title = if self.remaining().starts_with("--") {
            self.pos += 2;
            None
        } else if self.remaining().starts_with('-') {
            self.pos += 1;
            let title = self
                .parse_title()?
                .ok_or_else(|| self.insertion_error("expected edge title"))?;
            self.push_payload(
                title.clone(),
                "architecture edge title",
                EditorSemanticKind::String,
            );
            self.expect_literal('-', "expected '-' after edge title")?;
            Some(title)
        } else {
            return Err(self.insertion_error("expected edge connector"));
        };

        let rhs_into = self.parse_arrow_into();
        let rhs_direction =
            self.parse_direction("expected rhs direction", "invalid rhs direction")?;
        self.push_payload(
            rhs_direction.clone(),
            "architecture edge rhs direction",
            EditorSemanticKind::String,
        );
        self.expect_literal(':', "expected ':' for rhs port")?;
        let rhs = self.parse_id(
            "invalid id",
            "architecture edge endpoint",
            EditorSemanticKind::Variable,
        )?;
        let rhs_group = self.parse_group_modifier();
        self.finish()?;

        Ok(ArchitectureStatement::Edge(ArchitectureEdge {
            lhs_id: lhs.text,
            lhs_span: lhs.span,
            lhs_dir: lhs_direction.value.chars().next().expect("direction token"),
            lhs_into,
            lhs_group,
            rhs_id: rhs.text,
            rhs_span: rhs.span,
            rhs_dir: rhs_direction.value.chars().next().expect("direction token"),
            rhs_into,
            rhs_group,
            title: title.as_ref().map(|value| value.value.clone()),
        }))
    }

    fn parse_id(
        &mut self,
        message: &str,
        detail: &'static str,
        kind: EditorSemanticKind,
    ) -> Result<ArchitectureIdentifier> {
        let token = self.parse_raw_id(message)?;
        if is_architecture_reserved_id(&token.value) {
            return Err(Error::diagram_parse_exact(
                "architecture",
                architecture_reserved_id_message(&token.value),
                token.selection,
            ));
        }
        self.push_entity(token.clone(), detail, kind);
        Ok(ArchitectureIdentifier {
            text: token.value,
            span: token.selection,
        })
    }

    fn parse_raw_id(&mut self, message: &str) -> Result<ArchitectureSpannedValue> {
        self.skip_ws();
        let start = self.pos;
        let mut last_word_end = None;
        let mut seen = false;
        while let Some(ch) = self.peek_char() {
            let word = ch.is_ascii_alphanumeric() || ch == '_';
            if !seen {
                if !word {
                    return Err(self.insertion_error(message));
                }
                seen = true;
            } else if !word && ch != '-' {
                break;
            }
            self.bump();
            if word {
                last_word_end = Some(self.pos);
            }
        }
        let Some(end) = last_word_end else {
            return Err(self.insertion_error(message));
        };
        self.pos = end;
        Ok(self.spanned(self.input[start..end].to_string(), start, end, start, end))
    }

    fn parse_icon(&mut self) -> Result<Option<ArchitectureSpannedValue>> {
        self.skip_ws();
        if self.peek_char() != Some('(') {
            return Ok(None);
        }
        let start = self.pos;
        self.bump();
        let selection_start = self.pos;
        while let Some(ch) = self.peek_char() {
            if ch == ')' {
                break;
            }
            if !(ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | ':')) {
                return Err(self.exact_char_error("invalid architecture icon", ch));
            }
            self.bump();
        }
        let selection_end = self.pos;
        if selection_end == selection_start || self.peek_char() != Some(')') {
            return Err(self.insertion_error("unterminated architecture icon"));
        }
        self.bump();
        Ok(Some(
            self.spanned(
                self.input[selection_start..selection_end]
                    .trim()
                    .to_string(),
                start,
                self.pos,
                selection_start,
                selection_end,
            ),
        ))
    }

    fn parse_string(&mut self) -> Result<ArchitectureSpannedValue> {
        self.skip_ws();
        let start = self.pos;
        let Some(quote @ ('"' | '\'')) = self.bump() else {
            return Err(self.insertion_error("expected quoted service icon text"));
        };
        let selection_start = self.pos;
        let mut escaped = false;
        while let Some(ch) = self.peek_char() {
            if escaped {
                escaped = false;
                self.bump();
                continue;
            }
            if ch == '\\' {
                escaped = true;
                self.bump();
                continue;
            }
            if ch == quote {
                break;
            }
            self.bump();
        }
        let selection_end = self.pos;
        if self.peek_char() != Some(quote) {
            return Err(self.insertion_error("unterminated quoted service icon text"));
        }
        self.bump();
        Ok(self.spanned(
            unescape_string(&self.input[selection_start..selection_end]),
            start,
            self.pos,
            selection_start,
            selection_end,
        ))
    }

    fn parse_title(&mut self) -> Result<Option<ArchitectureSpannedValue>> {
        self.skip_ws();
        if self.peek_char() != Some('[') {
            return Ok(None);
        }
        let start = self.pos;
        self.bump();
        let selection_start = self.pos;
        let quote = self.peek_char().filter(|ch| matches!(ch, '"' | '\''));
        if let Some(quote) = quote {
            self.bump();
            let mut escaped = false;
            while let Some(ch) = self.peek_char() {
                if escaped {
                    escaped = false;
                    self.bump();
                    continue;
                }
                if ch == '\\' {
                    escaped = true;
                    self.bump();
                    continue;
                }
                if ch == quote {
                    break;
                }
                self.bump();
            }
            if self.peek_char() != Some(quote) {
                return Err(self.insertion_error("unterminated architecture title"));
            }
            self.bump();
        } else {
            let mut count = 0usize;
            while let Some(ch) = self.peek_char() {
                if ch == ']' {
                    break;
                }
                if !(ch.is_ascii_alphanumeric() || ch == '_' || ch == ' ') {
                    return Err(self.exact_char_error("invalid architecture title", ch));
                }
                count += 1;
                self.bump();
            }
            if count == 0 {
                return Err(self.insertion_error("empty architecture title"));
            }
        }
        let selection_end = self.pos;
        if self.peek_char() != Some(']') {
            return Err(self.insertion_error("unterminated architecture title"));
        }
        self.bump();
        let raw = &self.input[selection_start..selection_end];
        Ok(Some(self.spanned(
            convert_architecture_title(raw),
            start,
            self.pos,
            selection_start,
            selection_end,
        )))
    }

    fn parse_direction(
        &mut self,
        missing_message: &str,
        invalid_message: &str,
    ) -> Result<ArchitectureSpannedValue> {
        self.skip_ws();
        let start = self.pos;
        let Some(ch) = self.bump() else {
            return Err(self.insertion_error(missing_message));
        };
        if !matches!(ch, 'L' | 'R' | 'T' | 'B') {
            return Err(Error::diagram_parse_exact(
                "architecture",
                invalid_message,
                SourceSpan::new(self.base_offset + start, self.base_offset + self.pos),
            ));
        }
        Ok(self.spanned(ch.to_string(), start, self.pos, start, self.pos))
    }

    fn parse_arrow_into(&mut self) -> Option<bool> {
        self.skip_ws();
        if matches!(self.peek_char(), Some('<' | '>')) {
            self.bump();
            Some(true)
        } else {
            None
        }
    }

    fn parse_group_modifier(&mut self) -> Option<bool> {
        self.skip_ws();
        if self.remaining().starts_with("{group}") {
            self.pos += "{group}".len();
            Some(true)
        } else {
            None
        }
    }

    fn push_entity(
        &mut self,
        value: ArchitectureSpannedValue,
        detail: &'static str,
        kind: EditorSemanticKind,
    ) {
        self.facts.push(ArchitectureTraceFact {
            value,
            detail,
            kind,
            role: ArchitectureTraceFactRole::Entity,
            expected: EditorExpectedSyntaxKind::NodeIdentifier,
        });
    }

    fn push_payload(
        &mut self,
        value: ArchitectureSpannedValue,
        detail: &'static str,
        kind: EditorSemanticKind,
    ) {
        if value.value.is_empty() {
            return;
        }
        self.facts.push(ArchitectureTraceFact {
            value,
            detail,
            kind,
            role: ArchitectureTraceFactRole::Payload,
            expected: EditorExpectedSyntaxKind::Payload,
        });
    }

    fn next_is_keyword(&mut self, keyword: &str) -> bool {
        let saved = self.pos;
        let matched = self.consume_keyword(keyword);
        self.pos = saved;
        matched
    }

    fn expect_keyword(&mut self, keyword: &str) {
        let consumed = self.consume_keyword(keyword);
        debug_assert!(consumed);
    }

    fn consume_keyword(&mut self, keyword: &str) -> bool {
        self.skip_ws();
        if !self.remaining().starts_with(keyword) {
            return false;
        }
        let tail = &self.remaining()[keyword.len()..];
        if tail
            .chars()
            .next()
            .is_some_and(|ch| !matches!(ch, ' ' | '\t' | ':'))
        {
            return false;
        }
        self.pos += keyword.len();
        true
    }

    fn expect_literal(&mut self, expected: char, message: &str) -> Result<()> {
        self.skip_ws();
        if self.peek_char() != Some(expected) {
            return Err(self.insertion_error(message));
        }
        self.bump();
        Ok(())
    }

    fn finish(&mut self) -> Result<()> {
        self.skip_ws();
        if self.is_eof() {
            return Ok(());
        }
        let start = self.pos;
        let end = self.input.len();
        Err(Error::diagram_parse_exact(
            "architecture",
            "unexpected trailing input",
            SourceSpan::new(self.base_offset + start, self.base_offset + end),
        ))
    }

    fn spanned(
        &self,
        value: String,
        span_start: usize,
        span_end: usize,
        selection_start: usize,
        selection_end: usize,
    ) -> ArchitectureSpannedValue {
        ArchitectureSpannedValue {
            value,
            span: SourceSpan::new(self.base_offset + span_start, self.base_offset + span_end),
            selection: SourceSpan::new(
                self.base_offset + selection_start,
                self.base_offset + selection_end,
            ),
        }
    }

    fn insertion_error(&self, message: &str) -> Error {
        Error::diagram_parse_insertion_point("architecture", message, self.base_offset + self.pos)
    }

    fn exact_char_error(&self, message: &str, ch: char) -> Error {
        Error::diagram_parse_exact(
            "architecture",
            message,
            SourceSpan::new(
                self.base_offset + self.pos,
                self.base_offset + self.pos + ch.len_utf8(),
            ),
        )
    }

    fn remaining(&self) -> &'a str {
        &self.input[self.pos..]
    }

    fn is_eof(&self) -> bool {
        self.pos >= self.input.len()
    }

    fn peek_char(&self) -> Option<char> {
        self.remaining().chars().next()
    }

    fn bump(&mut self) -> Option<char> {
        let ch = self.peek_char()?;
        self.pos += ch.len_utf8();
        Some(ch)
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek_char(), Some(' ' | '\t')) {
            self.bump();
        }
    }
}

fn convert_architecture_title(raw: &str) -> String {
    let trimmed = raw.trim();
    let Some(quote @ ('"' | '\'')) = trimmed.chars().next() else {
        return trimmed.to_string();
    };
    if trimmed.len() < quote.len_utf8() * 2 || !trimmed.ends_with(quote) {
        return trimmed.to_string();
    }
    let inner = &trimmed[quote.len_utf8()..trimmed.len() - quote.len_utf8()];
    inner
        .replace("\\\"", "\"")
        .replace("\\'", "'")
        .trim()
        .to_string()
}

fn unescape_string(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(escaped) = chars.next() {
                out.push(match escaped {
                    'b' => '\u{0008}',
                    'f' => '\u{000c}',
                    'n' => '\n',
                    'r' => '\r',
                    't' => '\t',
                    'v' => '\u{000b}',
                    '0' => '\0',
                    escaped => escaped,
                });
            } else {
                out.push(ch);
            }
        } else {
            out.push(ch);
        }
    }
    out
}

pub(super) fn parse_semantic_source(
    code: &str,
    meta: &ParseMetadata,
) -> Result<ArchitectureSemanticSource> {
    let trace = parse_trace(code, meta, ArchitectureParseMode::Strict)?;
    let db = trace.build_db()?;
    Ok(ArchitectureSemanticSource { trace, db })
}

pub(super) fn parse_recovering_editor_facts(
    code: &str,
    meta: &ParseMetadata,
) -> EditorSemanticFacts {
    let trace = match parse_trace(code, meta, ArchitectureParseMode::Recovering) {
        Ok(trace) => trace,
        Err(error) => {
            let mut facts = EditorSemanticFacts::new();
            push_recovery_error(&mut facts, error);
            return facts;
        }
    };
    let validation_error = trace.build_db().err();
    trace.editor_facts(validation_error)
}

fn parse_trace(
    code: &str,
    meta: &ParseMetadata,
    mode: ArchitectureParseMode,
) -> Result<ArchitectureTrace> {
    let mut trace = ArchitectureTrace::default();
    let mut lines = ArchitectureLineCursor::new(code);
    let mut found_header = false;

    while let Some(line) = lines.next() {
        let (trimmed, trimmed_start) = trimmed_statement_with_offset(line.text, line.start);
        if trimmed.is_empty() {
            continue;
        }
        let Some(rest_with_ws) = trimmed.strip_prefix("architecture-beta") else {
            return handle_header_error(&mut trace, mode, meta, trimmed_start, trimmed.len());
        };
        if rest_with_ws
            .chars()
            .next()
            .is_some_and(|ch| !matches!(ch, ' ' | '\t'))
        {
            return handle_header_error(&mut trace, mode, meta, trimmed_start, trimmed.len());
        }
        found_header = true;
        let leading = rest_with_ws.len() - rest_with_ws.trim_start().len();
        let rest = rest_with_ws.trim_start();
        if !rest.is_empty() {
            let rest_start = trimmed_start + "architecture-beta".len() + leading;
            parse_trace_statement(code, &mut lines, rest, rest_start, mode, &mut trace)?;
        }
        break;
    }

    if !found_header {
        return handle_header_error(&mut trace, mode, meta, code.len(), 0);
    }

    while let Some(line) = lines.next() {
        let (trimmed, trimmed_start) = trimmed_statement_with_offset(line.text, line.start);
        if trimmed.is_empty() {
            continue;
        }
        parse_trace_statement(code, &mut lines, trimmed, trimmed_start, mode, &mut trace)?;
    }

    Ok(trace)
}

fn handle_header_error(
    trace: &mut ArchitectureTrace,
    mode: ArchitectureParseMode,
    meta: &ParseMetadata,
    start: usize,
    len: usize,
) -> Result<ArchitectureTrace> {
    let error = Error::diagram_parse_fallback(
        meta.diagram_type.clone(),
        "expected architecture-beta header",
    );
    if mode == ArchitectureParseMode::Strict {
        return Err(error);
    }
    trace.entries.push(ArchitectureTraceEntry {
        statement: None,
        span: SourceSpan::new(start, start + len),
        facts: Vec::new(),
        diagnostic: Some(take_parse_diagnostic(error)),
        directive_prefix: None,
    });
    Ok(std::mem::take(trace))
}

fn parse_trace_statement(
    code: &str,
    lines: &mut ArchitectureLineCursor<'_>,
    statement: &str,
    statement_start: usize,
    mode: ArchitectureParseMode,
    trace: &mut ArchitectureTrace,
) -> Result<()> {
    let parsed =
        parse_common_statement(code, lines, statement, statement_start).unwrap_or_else(|| {
            let statement = extend_quoted_statement(code, lines, statement, statement_start);
            ArchitectureStatementParser::new(statement, statement_start).parse()
        });
    match parsed {
        Ok(entry) => trace.entries.push(entry),
        Err(failure) if mode == ArchitectureParseMode::Strict => return Err(*failure.error),
        Err(failure) => trace.entries.push(ArchitectureTraceEntry {
            statement: None,
            span: SourceSpan::new(statement_start, statement_start + statement.len()),
            facts: failure.facts,
            diagnostic: Some(take_parse_diagnostic(*failure.error)),
            directive_prefix: failure.directive_prefix,
        }),
    }
    Ok(())
}

fn extend_quoted_statement<'a>(
    code: &'a str,
    lines: &mut ArchitectureLineCursor<'_>,
    statement: &'a str,
    statement_start: usize,
) -> &'a str {
    if open_quote_at_end(statement).is_none() {
        return statement;
    }

    let mut quote = None;
    let mut escaped = false;
    for (relative, ch) in code[statement_start..].char_indices() {
        let offset = statement_start + relative;
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == active_quote {
                quote = None;
            }
            continue;
        }

        if matches!(ch, '\'' | '"') {
            quote = Some(ch);
        } else if code[offset..].starts_with("%%") {
            lines.offset = next_physical_line_offset(code, offset);
            return code[statement_start..offset].trim_end_matches([' ', '\t']);
        } else if matches!(ch, '\n' | '\r') {
            lines.offset = line_ending_end(code, offset);
            return code[statement_start..offset].trim_end_matches([' ', '\t']);
        }
    }

    lines.offset = code.len();
    code[statement_start..].trim_end_matches([' ', '\t'])
}

fn open_quote_at_end(input: &str) -> Option<char> {
    let mut quote = None;
    let mut escaped = false;
    for ch in input.chars() {
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == active_quote {
                quote = None;
            }
        } else if matches!(ch, '\'' | '"') {
            quote = Some(ch);
        }
    }
    quote
}

fn next_physical_line_offset(code: &str, offset: usize) -> usize {
    code[offset..]
        .find('\n')
        .map_or(code.len(), |relative| offset + relative + 1)
}

fn line_ending_end(code: &str, offset: usize) -> usize {
    if code[offset..].starts_with("\r\n") {
        offset + 2
    } else {
        offset + 1
    }
}

fn parse_common_statement(
    code: &str,
    lines: &mut ArchitectureLineCursor<'_>,
    statement: &str,
    statement_start: usize,
) -> Option<std::result::Result<ArchitectureTraceEntry, StatementFailure>> {
    if keyword_rest(statement, "title", &[]).is_some() {
        return Some(parse_inline_common(
            strip_common_inline_comment(statement),
            statement_start,
            "title",
            CommonStatementKind::Title,
        ));
    }
    if keyword_rest(statement, "accTitle", &[':']).is_some() {
        return Some(parse_inline_common(
            strip_common_inline_comment(statement),
            statement_start,
            "accTitle",
            CommonStatementKind::AccTitle,
        ));
    }
    if let Some(rest) = keyword_rest(statement, "accDescr", &[':', '{']) {
        let rest_leading = rest.len() - rest.trim_start_matches([' ', '\t']).len();
        let rest = rest.trim_start_matches([' ', '\t']);
        if rest.starts_with(':') {
            return Some(parse_inline_common(
                strip_common_inline_comment(statement),
                statement_start,
                "accDescr",
                CommonStatementKind::AccDescr,
            ));
        }
        if rest.starts_with('{') {
            let opening = statement_start + "accDescr".len() + rest_leading;
            return Some(parse_acc_descr_block(code, lines, statement_start, opening));
        }
        if rest.is_empty()
            && let Some(opening) =
                whitespace_separated_block_opening(code, statement_start + "accDescr".len())
        {
            return Some(parse_acc_descr_block(code, lines, statement_start, opening));
        }
        let offset = statement_start + "accDescr".len() + rest_leading;
        return Some(Err(StatementFailure::new(
            Error::diagram_parse_insertion_point(
                "architecture",
                "expected ':' or '{' after accDescr",
                offset,
            ),
            Vec::new(),
        )
        .with_directive_prefix("accDescr")));
    }
    None
}

#[derive(Debug, Clone, Copy)]
enum CommonStatementKind {
    Title,
    AccTitle,
    AccDescr,
}

impl CommonStatementKind {
    fn prefix(self) -> &'static str {
        match self {
            Self::Title => "title",
            Self::AccTitle => "accTitle",
            Self::AccDescr => "accDescr",
        }
    }

    fn detail(self) -> &'static str {
        match self {
            Self::Title => "architecture title",
            Self::AccTitle => "architecture accessibility title",
            Self::AccDescr => "architecture accessibility description",
        }
    }

    fn statement(self, value: ArchitectureSpannedValue) -> ArchitectureStatement {
        match self {
            Self::Title => ArchitectureStatement::Title(value),
            Self::AccTitle => ArchitectureStatement::AccTitle(value),
            Self::AccDescr => ArchitectureStatement::AccDescr(value),
        }
    }
}

fn parse_inline_common(
    statement: &str,
    statement_start: usize,
    keyword: &str,
    kind: CommonStatementKind,
) -> std::result::Result<ArchitectureTraceEntry, StatementFailure> {
    let mut value_start = keyword.len();
    if matches!(
        kind,
        CommonStatementKind::AccTitle | CommonStatementKind::AccDescr
    ) {
        while statement[value_start..].starts_with([' ', '\t']) {
            value_start += 1;
        }
        if !statement[value_start..].starts_with(':') {
            return Err(StatementFailure::new(
                Error::diagram_parse_insertion_point(
                    "architecture",
                    format!("expected ':' after {keyword}"),
                    statement_start + value_start,
                ),
                Vec::new(),
            )
            .with_directive_prefix(kind.prefix()));
        }
        value_start += 1;
    } else if statement[value_start..]
        .chars()
        .next()
        .is_some_and(|ch| matches!(ch, ' ' | '\t'))
    {
        value_start += 1;
    }

    let raw = &statement[value_start..];
    let leading = raw.len() - raw.trim_start_matches([' ', '\t']).len();
    let trailing = raw.len() - raw.trim_end_matches([' ', '\t']).len();
    let selection_start = value_start + leading;
    let selection_end = statement.len().saturating_sub(trailing);
    let value = collapse_common_inline(&statement[selection_start..selection_end]);
    let spanned = ArchitectureSpannedValue {
        value,
        span: SourceSpan::new(
            statement_start + selection_start,
            statement_start + selection_end,
        ),
        selection: SourceSpan::new(
            statement_start + selection_start,
            statement_start + selection_end,
        ),
    };
    let facts = common_payload_fact(&spanned, kind.detail());
    Ok(ArchitectureTraceEntry {
        statement: Some(kind.statement(spanned)),
        span: SourceSpan::new(statement_start, statement_start + statement.len()),
        facts,
        diagnostic: None,
        directive_prefix: Some(kind.prefix()),
    })
}

fn parse_acc_descr_block(
    code: &str,
    lines: &mut ArchitectureLineCursor<'_>,
    statement_start: usize,
    opening: usize,
) -> std::result::Result<ArchitectureTraceEntry, StatementFailure> {
    let content_start = opening + 1;
    let Some(relative_close) = code[content_start..].find('}') else {
        lines.offset = code.len();
        let value = common_block_value(code, content_start, code.len());
        let facts = common_payload_fact(&value, "architecture accessibility description");
        return Err(StatementFailure::new(
            Error::diagram_parse_insertion_point(
                "architecture",
                "unterminated accDescr block",
                code.len(),
            ),
            facts,
        )
        .with_directive_prefix("accDescr"));
    };
    let close = content_start + relative_close;
    let line_end = code[close + 1..]
        .find('\n')
        .map_or(code.len(), |relative| close + 1 + relative + 1);
    let logical_end = strip_line_ending(&code[close + 1..line_end]);
    let trailing = strip_inline_comment(logical_end).trim();
    lines.offset = line_end;

    let value = common_block_value(code, content_start, close);
    let facts = common_payload_fact(&value, "architecture accessibility description");
    if !trailing.is_empty() {
        let relative = logical_end.find(trailing).unwrap_or(0);
        return Err(StatementFailure::new(
            Error::diagram_parse_exact(
                "architecture",
                "unexpected trailing input",
                SourceSpan::new(close + 1 + relative, close + 1 + relative + trailing.len()),
            ),
            facts,
        )
        .with_directive_prefix("accDescr"));
    }

    Ok(ArchitectureTraceEntry {
        statement: Some(ArchitectureStatement::AccDescr(value)),
        span: SourceSpan::new(statement_start, close + 1),
        facts,
        diagnostic: None,
        directive_prefix: Some("accDescr"),
    })
}

fn common_block_value(code: &str, start: usize, end: usize) -> ArchitectureSpannedValue {
    let raw = &code[start..end];
    let leading = raw.len() - raw.trim_start().len();
    let trailing = raw.len() - raw.trim_end().len();
    let selection_start = start + leading;
    let selection_end = end.saturating_sub(trailing);
    ArchitectureSpannedValue {
        value: convert_common_block(raw),
        span: SourceSpan::new(start, end),
        selection: SourceSpan::new(selection_start, selection_end),
    }
}

fn common_payload_fact(
    value: &ArchitectureSpannedValue,
    detail: &'static str,
) -> Vec<ArchitectureTraceFact> {
    if value.value.is_empty() {
        Vec::new()
    } else {
        vec![ArchitectureTraceFact {
            value: value.clone(),
            detail,
            kind: EditorSemanticKind::String,
            role: ArchitectureTraceFactRole::Payload,
            expected: EditorExpectedSyntaxKind::Payload,
        }]
    }
}

fn keyword_rest<'a>(input: &'a str, keyword: &str, separators: &[char]) -> Option<&'a str> {
    let rest = input.strip_prefix(keyword)?;
    if rest
        .chars()
        .next()
        .is_none_or(|ch| matches!(ch, ' ' | '\t') || separators.contains(&ch))
    {
        Some(rest)
    } else {
        None
    }
}

fn strip_common_inline_comment(statement: &str) -> &str {
    statement
        .split_once("%%")
        .map_or(statement, |(value, _)| value)
        .trim_end_matches([' ', '\t'])
}

fn whitespace_separated_block_opening(code: &str, start: usize) -> Option<usize> {
    let mut offset = start;
    for ch in code[start..].chars() {
        if ch == '{' {
            return Some(offset);
        }
        if !ch.is_whitespace() {
            return None;
        }
        offset += ch.len_utf8();
    }
    None
}

fn collapse_common_inline(value: &str) -> String {
    collapse_horizontal_runs(value.trim())
}

fn collapse_horizontal_runs(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    let mut horizontal = String::new();
    for ch in value.chars() {
        if matches!(ch, ' ' | '\t') {
            horizontal.push(ch);
            continue;
        }
        if horizontal.len() >= 2 {
            result.push(' ');
        } else {
            result.push_str(&horizontal);
        }
        horizontal.clear();
        result.push(ch);
    }
    if horizontal.len() >= 2 {
        result.push(' ');
    } else {
        result.push_str(&horizontal);
    }
    result
}

fn convert_common_block(value: &str) -> String {
    value
        .lines()
        .map(|line| collapse_horizontal_runs(line.trim()))
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn take_parse_diagnostic(error: Error) -> crate::ParseDiagnostic {
    match error {
        Error::DiagramParse { diagnostic, .. } => diagnostic,
        other => crate::ParseDiagnostic::new(other.to_string()),
    }
}

fn push_recovery_error(facts: &mut EditorSemanticFacts, error: Error) {
    let diagnostic = take_parse_diagnostic(error);
    facts.mark_recovered_from_parse_error(diagnostic.message(), diagnostic.span());
}

impl ArchitectureTrace {
    fn build_db(&self) -> Result<ArchitectureDb> {
        let mut db = ArchitectureDb::default();

        for entry in &self.entries {
            match entry.statement.as_ref() {
                Some(ArchitectureStatement::Title(value)) => db.set_title(value.value.clone()),
                Some(ArchitectureStatement::AccTitle(value)) => {
                    db.set_acc_title(value.value.clone());
                }
                Some(ArchitectureStatement::AccDescr(value)) => {
                    db.set_acc_descr(value.value.clone());
                }
                _ => {}
            }
        }
        for entry in &self.entries {
            if let Some(ArchitectureStatement::Group {
                id,
                icon,
                title,
                in_group,
            }) = entry.statement.as_ref()
            {
                db.add_group(
                    id.clone(),
                    icon.as_ref().map(|value| value.value.clone()),
                    title.as_ref().map(|value| value.value.clone()),
                    in_group.clone(),
                )?;
            }
        }
        for entry in &self.entries {
            if let Some(ArchitectureStatement::Service {
                id,
                icon,
                icon_text,
                title,
                in_group,
            }) = entry.statement.as_ref()
            {
                db.add_service(
                    id.clone(),
                    icon.as_ref().map(|value| value.value.clone()),
                    icon_text.as_ref().map(|value| value.value.clone()),
                    title.as_ref().map(|value| value.value.clone()),
                    in_group.clone(),
                )?;
            }
        }
        for entry in &self.entries {
            if let Some(ArchitectureStatement::Junction { id, in_group }) = entry.statement.as_ref()
            {
                db.add_junction(id.clone(), in_group.clone())?;
            }
        }
        for entry in &self.entries {
            if let Some(ArchitectureStatement::Edge(edge)) = entry.statement.as_ref() {
                db.add_edge(edge.clone())?;
            }
        }
        for entry in &self.entries {
            if let Some(ArchitectureStatement::Alignment { direction, members }) =
                entry.statement.as_ref()
            {
                db.add_layout_hint(
                    ArchitectureLayoutDirection::parse(&direction.value)
                        .expect("validated alignment direction"),
                    members.clone(),
                )?;
            }
        }
        Ok(db)
    }

    fn editor_facts(&self, validation_error: Option<Error>) -> EditorSemanticFacts {
        let mut facts = EditorSemanticFacts::new();
        for entry in &self.entries {
            debug_assert!(entry.span.start <= entry.span.end);
            if let Some(prefix) = entry.directive_prefix {
                facts.push_directive_prefix(prefix);
            }
            for fact in &entry.facts {
                facts.push_expected_syntax(EditorExpectedSyntax::new(
                    fact.expected,
                    fact.value.selection,
                ));
                let symbol = match fact.role {
                    ArchitectureTraceFactRole::Entity => EditorSemanticSymbol::new(
                        fact.value.value.clone(),
                        Some(fact.detail.to_string()),
                        fact.kind,
                        fact.value.span,
                        fact.value.selection,
                    ),
                    ArchitectureTraceFactRole::Payload => EditorSemanticSymbol::payload(
                        fact.value.value.clone(),
                        Some(fact.detail.to_string()),
                        fact.kind,
                        fact.value.span,
                        fact.value.selection,
                    ),
                };
                facts.push_symbol(symbol);
            }
            if let Some(diagnostic) = &entry.diagnostic {
                facts.mark_recovered_from_parse_error(diagnostic.message(), diagnostic.span());
            }
        }
        if let Some(error) = validation_error {
            push_recovery_error(&mut facts, error);
        }
        facts
    }
}

impl ArchitectureSemanticSource {
    pub(super) fn compat_json(&self, meta: &ParseMetadata) -> Value {
        let mut config = crate::config::clone_value_nonrecursive(meta.effective_config.as_value());
        if meta.config.as_value().get("layout").is_none()
            && let Some(obj) = config.as_object_mut()
        {
            obj.insert("layout".to_string(), Value::String("dagre".to_string()));
        }

        let mut out = serde_json::Map::with_capacity(10);
        out.insert("type".to_string(), Value::String(meta.diagram_type.clone()));
        out.insert("title".to_string(), optional_string(&self.db.title));
        out.insert("accTitle".to_string(), optional_string(&self.db.acc_title));
        out.insert("accDescr".to_string(), optional_string(&self.db.acc_descr));
        out.insert("groups".to_string(), Value::Array(self.db.groups_json()));
        out.insert("nodes".to_string(), Value::Array(self.db.nodes_json()));
        out.insert(
            "services".to_string(),
            Value::Array(self.db.services_json()),
        );
        out.insert(
            "junctions".to_string(),
            Value::Array(self.db.junctions_json()),
        );
        out.insert("edges".to_string(), Value::Array(self.db.edges_json()));
        out.insert(
            "layoutHints".to_string(),
            Value::Array(self.db.layout_hints_json()),
        );
        out.insert("config".to_string(), config);
        Value::Object(out)
    }

    pub(super) fn render_model(&self) -> ArchitectureDiagramRenderModel {
        self.db.render_model()
    }

    pub(super) fn editor_facts(&self) -> EditorSemanticFacts {
        self.trace.editor_facts(None)
    }
}

fn optional_string(value: &str) -> Value {
    if value.is_empty() {
        Value::Null
    } else {
        Value::String(value.to_string())
    }
}
