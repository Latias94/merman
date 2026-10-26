use super::*;
#[cfg(feature = "editor-facts")]
use crate::EditorRenamePolicy;
#[cfg(test)]
use std::cell::Cell;

#[cfg(test)]
thread_local! {
    pub(super) static ARCHITECTURE_SYNTAX_CONSTRUCTION_COUNT: Cell<usize> = const { Cell::new(0) };
}
use crate::diagrams::langium_common::{
    LangiumCommonFact, LangiumCommonField, LangiumCommonParse, is_ecmascript_line_terminator,
    parse_langium_common, parse_langium_string,
};

#[derive(Debug, Clone)]
struct ArchitectureSpannedValue {
    value: String,
    #[cfg(feature = "editor-facts")]
    span: SourceSpan,
    selection: SourceSpan,
}

#[derive(Debug, Clone, Copy)]
#[cfg(feature = "editor-facts")]
enum ArchitectureTraceFactRole {
    Entity,
    Reference,
    Payload,
}

#[derive(Debug, Clone)]
#[cfg(feature = "editor-facts")]
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
    #[cfg(feature = "editor-facts")]
    span: SourceSpan,
    #[cfg(feature = "editor-facts")]
    facts: Vec<ArchitectureTraceFact>,
    #[cfg(feature = "editor-facts")]
    diagnostic: Option<crate::ParseDiagnostic>,
    #[cfg(feature = "editor-facts")]
    directive_prefix: Option<&'static str>,
}

#[derive(Debug, Clone, Default)]
struct ArchitectureTrace {
    entries: Vec<ArchitectureTraceEntry>,
}

#[cfg(all(feature = "compat-json", any(test, feature = "editor-facts")))]
#[derive(Debug)]
pub(super) struct ArchitectureSemanticSource {
    db: ArchitectureDb,
    #[cfg(feature = "editor-facts")]
    editor_facts: EditorSemanticFacts,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArchitectureParseMode {
    Strict,
    #[cfg(feature = "editor-facts")]
    Recovering,
}

struct StatementFailure {
    error: Box<Error>,
    #[cfg(feature = "editor-facts")]
    facts: Vec<ArchitectureTraceFact>,
    #[cfg(feature = "editor-facts")]
    directive_prefix: Option<&'static str>,
}

impl StatementFailure {
    fn new(
        error: Error,
        #[cfg(feature = "editor-facts")] facts: Vec<ArchitectureTraceFact>,
    ) -> Self {
        Self {
            error: Box::new(error),
            #[cfg(feature = "editor-facts")]
            facts,
            #[cfg(feature = "editor-facts")]
            directive_prefix: None,
        }
    }

    #[cfg(feature = "editor-facts")]
    fn with_directive_prefix(mut self, prefix: &'static str) -> Self {
        self.directive_prefix = Some(prefix);
        self
    }
}

struct ArchitectureStatementParser<'a> {
    input: &'a str,
    base_offset: usize,
    pos: usize,
    #[cfg(feature = "editor-facts")]
    facts: Vec<ArchitectureTraceFact>,
    #[cfg(feature = "editor-facts")]
    editor_demand: EditorFactsDemand,
}

impl<'a> ArchitectureStatementParser<'a> {
    fn new(
        input: &'a str,
        base_offset: usize,
        #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    ) -> Self {
        Self {
            input,
            base_offset,
            pos: 0,
            #[cfg(feature = "editor-facts")]
            facts: Vec::new(),
            #[cfg(feature = "editor-facts")]
            editor_demand,
        }
    }

    fn parse(mut self) -> std::result::Result<ArchitectureTraceEntry, StatementFailure> {
        #[cfg(feature = "editor-facts")]
        let span = SourceSpan::new(self.base_offset, self.base_offset + self.input.len());
        let statement = self.parse_statement();
        match statement {
            Ok(statement) => Ok(ArchitectureTraceEntry {
                statement: Some(statement),
                #[cfg(feature = "editor-facts")]
                span,
                #[cfg(feature = "editor-facts")]
                facts: self.facts,
                #[cfg(feature = "editor-facts")]
                diagnostic: None,
                #[cfg(feature = "editor-facts")]
                directive_prefix: None,
            }),
            Err(error) => Err(StatementFailure::new(
                error,
                #[cfg(feature = "editor-facts")]
                self.facts,
            )),
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
            #[cfg(feature = "editor-facts")]
            "architecture group",
            #[cfg(feature = "editor-facts")]
            EditorSemanticKind::Namespace,
        )?;
        let icon = self.parse_icon()?;
        #[cfg(feature = "editor-facts")]
        if let Some(icon) = &icon {
            #[cfg(feature = "editor-facts")]
            self.push_payload(icon, "architecture group icon", EditorSemanticKind::String);
        }
        let title = self.parse_title()?;
        #[cfg(feature = "editor-facts")]
        if let Some(title) = &title {
            #[cfg(feature = "editor-facts")]
            self.push_payload(
                title,
                "architecture group title",
                EditorSemanticKind::String,
            );
        }
        let in_group = if self.consume_keyword("in") {
            Some(self.parse_reference(
                "invalid group parent id",
                #[cfg(feature = "editor-facts")]
                "architecture group parent",
                #[cfg(feature = "editor-facts")]
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
            #[cfg(feature = "editor-facts")]
            "architecture service",
            #[cfg(feature = "editor-facts")]
            EditorSemanticKind::Variable,
        )?;
        let mut icon = None;
        let mut icon_text = None;
        self.skip_ws();
        match self.peek_char() {
            Some('(') => {
                icon = self.parse_icon()?;
                #[cfg(feature = "editor-facts")]
                if let Some(value) = &icon {
                    #[cfg(feature = "editor-facts")]
                    self.push_payload(
                        value,
                        "architecture service icon",
                        EditorSemanticKind::String,
                    );
                }
            }
            Some('"' | '\'') => {
                icon_text = Some(self.parse_string()?);
                #[cfg(feature = "editor-facts")]
                self.push_payload(
                    icon_text.as_ref().expect("icon text was just parsed"),
                    "architecture service icon text",
                    EditorSemanticKind::String,
                );
            }
            _ => {}
        }
        let title = self.parse_title()?;
        #[cfg(feature = "editor-facts")]
        if let Some(title) = &title {
            #[cfg(feature = "editor-facts")]
            self.push_payload(
                title,
                "architecture service title",
                EditorSemanticKind::String,
            );
        }
        let in_group = if self.consume_keyword("in") {
            Some(self.parse_reference(
                "invalid service parent id",
                #[cfg(feature = "editor-facts")]
                "architecture service parent",
                #[cfg(feature = "editor-facts")]
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
            #[cfg(feature = "editor-facts")]
            "architecture junction",
            #[cfg(feature = "editor-facts")]
            EditorSemanticKind::Object,
        )?;
        let in_group = if self.consume_keyword("in") {
            Some(self.parse_reference(
                "invalid junction parent id",
                #[cfg(feature = "editor-facts")]
                "architecture junction parent",
                #[cfg(feature = "editor-facts")]
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
        #[cfg(feature = "editor-facts")]
        self.push_payload(
            &direction,
            "architecture alignment direction",
            EditorSemanticKind::String,
        );

        let mut members = Vec::new();
        while {
            self.skip_ws();
            !self.is_eof()
        } {
            members.push(self.parse_reference(
                "invalid align member id",
                #[cfg(feature = "editor-facts")]
                "architecture alignment member",
                #[cfg(feature = "editor-facts")]
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
        let lhs = self.parse_reference(
            "invalid id",
            #[cfg(feature = "editor-facts")]
            "architecture edge endpoint",
            #[cfg(feature = "editor-facts")]
            EditorSemanticKind::Variable,
        )?;
        let lhs_group = self.parse_group_modifier();
        self.expect_delimiter(':', "expected ':' for lhs port")?;
        let lhs_direction =
            self.parse_direction("expected lhs direction", "invalid lhs direction")?;
        #[cfg(feature = "editor-facts")]
        self.push_payload(
            &lhs_direction,
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
            #[cfg(feature = "editor-facts")]
            self.push_payload(
                &title,
                "architecture edge title",
                EditorSemanticKind::String,
            );
            self.expect_operator('-', "expected '-' after edge title")?;
            Some(title)
        } else {
            return Err(self.insertion_error("expected edge connector"));
        };

        let rhs_into = self.parse_arrow_into();
        let rhs_direction =
            self.parse_direction("expected rhs direction", "invalid rhs direction")?;
        #[cfg(feature = "editor-facts")]
        self.push_payload(
            &rhs_direction,
            "architecture edge rhs direction",
            EditorSemanticKind::String,
        );
        self.expect_delimiter(':', "expected ':' for rhs port")?;
        let rhs = self.parse_reference(
            "invalid id",
            #[cfg(feature = "editor-facts")]
            "architecture edge endpoint",
            #[cfg(feature = "editor-facts")]
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
        #[cfg(feature = "editor-facts")] detail: &'static str,
        #[cfg(feature = "editor-facts")] kind: EditorSemanticKind,
    ) -> Result<ArchitectureIdentifier> {
        let token = self.parse_raw_id(message)?;
        if is_architecture_reserved_id(&token.value) {
            return Err(Error::diagram_parse_exact(
                "architecture",
                architecture_reserved_id_message(&token.value),
                token.selection,
            ));
        }
        #[cfg(feature = "editor-facts")]
        self.push_entity(&token, detail, kind);
        Ok(ArchitectureIdentifier {
            text: token.value,
            span: token.selection,
        })
    }

    fn parse_reference(
        &mut self,
        message: &str,
        #[cfg(feature = "editor-facts")] detail: &'static str,
        #[cfg(feature = "editor-facts")] kind: EditorSemanticKind,
    ) -> Result<ArchitectureIdentifier> {
        let token = self.parse_raw_id(message)?;
        if is_architecture_reserved_id(&token.value) {
            return Err(Error::diagram_parse_exact(
                "architecture",
                architecture_reserved_id_message(&token.value),
                token.selection,
            ));
        }
        #[cfg(feature = "editor-facts")]
        self.push_reference(&token, detail, kind);
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
        if !matches!(self.peek_char(), Some('"' | '\'')) {
            return Err(self.insertion_error("expected quoted service icon text"));
        }
        let Some(parsed) = parse_langium_string(&self.input[start..], start) else {
            self.pos = self.input.len();
            return Err(self.insertion_error("unterminated quoted service icon text"));
        };
        self.pos = start + parsed.consumed;
        Ok(self.spanned(
            parsed.value,
            parsed.raw_span.start,
            parsed.raw_span.end,
            parsed.value_span.start,
            parsed.value_span.end,
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
                    if is_ecmascript_line_terminator(ch) {
                        return Err(self.insertion_error(
                            "escaped line terminator is not valid in an architecture title",
                        ));
                    }
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
                if matches!(ch, '[' | '\r' | '\n') {
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

    #[cfg(feature = "editor-facts")]
    fn push_entity(
        &mut self,
        value: &ArchitectureSpannedValue,
        detail: &'static str,
        kind: EditorSemanticKind,
    ) {
        #[cfg(feature = "editor-facts")]
        if !self.editor_demand.captures_rich() {
            return;
        }
        self.facts.push(ArchitectureTraceFact {
            value: value.clone(),
            detail,
            kind,
            role: ArchitectureTraceFactRole::Entity,
            expected: EditorExpectedSyntaxKind::NodeIdentifier,
        });
    }

    #[cfg(feature = "editor-facts")]
    fn push_reference(
        &mut self,
        value: &ArchitectureSpannedValue,
        detail: &'static str,
        kind: EditorSemanticKind,
    ) {
        #[cfg(feature = "editor-facts")]
        if !self.editor_demand.captures_rich() {
            return;
        }
        self.facts.push(ArchitectureTraceFact {
            value: value.clone(),
            detail,
            kind,
            role: ArchitectureTraceFactRole::Reference,
            expected: EditorExpectedSyntaxKind::NodeIdentifier,
        });
    }

    #[cfg(feature = "editor-facts")]
    fn push_payload(
        &mut self,
        value: &ArchitectureSpannedValue,
        detail: &'static str,
        kind: EditorSemanticKind,
    ) {
        #[cfg(feature = "editor-facts")]
        if !self.editor_demand.captures_rich() {
            return;
        }
        if value.value.is_empty() {
            return;
        }
        self.facts.push(ArchitectureTraceFact {
            value: value.clone(),
            detail,
            kind,
            role: ArchitectureTraceFactRole::Payload,
            expected: EditorExpectedSyntaxKind::Payload,
        });
    }

    fn next_is_keyword(&self, keyword: &str) -> bool {
        let remaining = self.remaining().trim_start_matches([' ', '\t']);
        remaining.starts_with(keyword)
            && remaining[keyword.len()..]
                .chars()
                .next()
                .is_none_or(|ch| matches!(ch, ' ' | '\t' | ':'))
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

    fn expect_delimiter(&mut self, expected: char, message: &str) -> Result<()> {
        self.expect_syntax_char(expected, message)
    }

    fn expect_operator(&mut self, expected: char, message: &str) -> Result<()> {
        self.expect_syntax_char(expected, message)
    }

    fn expect_syntax_char(&mut self, expected: char, message: &str) -> Result<()> {
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
        _span_start: usize,
        _span_end: usize,
        selection_start: usize,
        selection_end: usize,
    ) -> ArchitectureSpannedValue {
        ArchitectureSpannedValue {
            value,
            #[cfg(feature = "editor-facts")]
            span: SourceSpan::new(self.base_offset + _span_start, self.base_offset + _span_end),
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

#[cfg(all(test, feature = "compat-json"))]
pub(super) fn parse_semantic_source(
    code: &str,
    meta: &PresentationMetadata,
) -> Result<ArchitectureSemanticSource> {
    let trace = parse_trace_controlled(
        code,
        &meta.diagram_type,
        ArchitectureParseMode::Strict,
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Capture,
        &crate::OperationControl::new(),
    )
    .expect("a private parse control cannot be cancelled")?;
    let db = trace.build_db()?;
    #[cfg(feature = "editor-facts")]
    let editor_facts = trace.editor_facts(Some(&db), None);
    Ok(ArchitectureSemanticSource {
        db,
        #[cfg(feature = "editor-facts")]
        editor_facts,
    })
}

#[cfg(feature = "presentation")]
pub(super) fn parse_model_controlled(
    code: &str,
    diagram_type: &str,
    control: &OperationControl,
) -> OperationControlResult<Result<ArchitectureDiagramModel>> {
    Ok(parse_semantic_model_controlled(
        code,
        diagram_type,
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        control,
    )?
    .model)
}

pub(super) fn parse_semantic_model_controlled(
    code: &str,
    diagram_type: &str,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &OperationControl,
) -> OperationControlResult<crate::family::SemanticConstruction<ArchitectureDiagramModel>> {
    let construction = construct_semantic_source_controlled(
        code,
        diagram_type,
        #[cfg(feature = "editor-facts")]
        editor_demand,
        control,
    )?;
    let model = match construction.model {
        Ok(db) => Ok(db.semantic_model_controlled(control)?),
        Err(error) => Err(error),
    };
    Ok(crate::family::SemanticConstruction {
        model,
        #[cfg(feature = "editor-facts")]
        editor_facts: construction.editor_facts,
    })
}

#[cfg(feature = "compat-json")]
#[cfg(feature = "editor-facts")]
pub(super) fn parse_combined_semantic_source_controlled(
    code: &str,
    meta: &PresentationMetadata,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<
    std::result::Result<ArchitectureSemanticSource, crate::family::CombinedSemanticFailure>,
> {
    let construction = construct_semantic_source_controlled(
        code,
        &meta.diagram_type,
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Capture,
        control,
    )?;
    let editor_facts = construction
        .editor_facts
        .expect("Architecture capture requests its construction journal");
    Ok(match construction.model {
        Ok(db) => Ok(ArchitectureSemanticSource { db, editor_facts }),
        Err(error) => Err(crate::family::CombinedSemanticFailure::new(
            error,
            #[cfg(feature = "editor-facts")]
            Some(editor_facts),
        )),
    })
}

fn construct_semantic_source_controlled(
    code: &str,
    diagram_type: &str,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<crate::family::SemanticConstruction<ArchitectureDb>> {
    control.checkpoint()?;
    #[cfg(feature = "editor-facts")]
    let mode = match editor_demand {
        EditorFactsDemand::Omit => ArchitectureParseMode::Strict,
        EditorFactsDemand::Capture | EditorFactsDemand::Recovery => {
            ArchitectureParseMode::Recovering
        }
    };
    #[cfg(not(feature = "editor-facts"))]
    let mode = ArchitectureParseMode::Strict;
    let trace = match parse_trace_controlled(
        code,
        diagram_type,
        mode,
        #[cfg(feature = "editor-facts")]
        editor_demand,
        control,
    )? {
        Ok(trace) => trace,
        Err(error) => {
            #[cfg(feature = "editor-facts")]
            let editor_facts = editor_demand.needs_recovery().then(|| {
                let mut facts = EditorSemanticFacts::new();
                push_recovery_error(&mut facts, &error);
                facts
            });
            return Ok(crate::family::SemanticConstruction {
                model: Err(error),
                #[cfg(feature = "editor-facts")]
                editor_facts,
            });
        }
    };
    control.checkpoint()?;
    #[cfg(feature = "editor-facts")]
    let mut syntax_error = None;
    #[cfg(feature = "editor-facts")]
    if editor_demand.needs_recovery() {
        for (index, entry) in trace.entries.iter().enumerate() {
            if index % 128 == 0 {
                control.checkpoint()?;
            }
            if let Some(diagnostic) = entry.diagnostic.clone() {
                syntax_error = Some(Error::DiagramParse {
                    diagram_type: diagram_type.to_string(),
                    diagnostic,
                });
                break;
            }
        }
    }
    control.checkpoint()?;
    let db = trace.build_db_controlled(control)?;
    #[cfg(feature = "editor-facts")]
    let editor_facts = match editor_demand {
        EditorFactsDemand::Omit => None,
        EditorFactsDemand::Capture | EditorFactsDemand::Recovery => {
            Some(trace.editor_facts_controlled(
                db.as_ref().ok(),
                db.as_ref().err(),
                editor_demand,
                control,
            )?)
        }
    };
    control.checkpoint()?;
    #[cfg(feature = "editor-facts")]
    let db = match syntax_error {
        Some(error) => Err(error),
        None => db,
    };
    Ok(crate::family::SemanticConstruction {
        model: db,
        #[cfg(feature = "editor-facts")]
        editor_facts,
    })
}

fn parse_trace_controlled(
    code: &str,
    diagram_type: &str,
    mode: ArchitectureParseMode,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<Result<ArchitectureTrace>> {
    control.checkpoint()?;
    #[cfg(test)]
    ARCHITECTURE_SYNTAX_CONSTRUCTION_COUNT.set(ARCHITECTURE_SYNTAX_CONSTRUCTION_COUNT.get() + 1);
    #[cfg(feature = "editor-facts")]
    if editor_demand == EditorFactsDemand::Capture {
        #[cfg(test)]
        crate::editor::test_work::record_collection();
    }
    let mut trace = ArchitectureTrace::default();
    let mut lines = ArchitectureLineCursor::new(code);
    let mut found_header = false;

    while let Some(line) = lines.next() {
        control.checkpoint()?;
        let (trimmed, trimmed_start) = trimmed_statement_with_offset(line.text, line.start);
        if trimmed.is_empty() {
            continue;
        }
        let Some(rest_with_ws) = trimmed.strip_prefix("architecture-beta") else {
            return Ok(handle_header_error(
                &mut trace,
                mode,
                diagram_type,
                trimmed_start,
                trimmed.len(),
            ));
        };
        if rest_with_ws
            .chars()
            .next()
            .is_some_and(|ch| !matches!(ch, ' ' | '\t'))
        {
            return Ok(handle_header_error(
                &mut trace,
                mode,
                diagram_type,
                trimmed_start,
                trimmed.len(),
            ));
        }
        found_header = true;
        let leading = rest_with_ws.len() - rest_with_ws.trim_start().len();
        let rest = rest_with_ws.trim_start();
        if !rest.is_empty() {
            let common_start = trimmed_start + "architecture-beta".len();
            let statement_start = common_start + leading;
            let entry = match parse_trace_statement(
                code,
                &mut lines,
                rest,
                statement_start,
                common_start,
                mode,
                #[cfg(feature = "editor-facts")]
                editor_demand,
            ) {
                Ok(entry) => entry,
                Err(error) => return Ok(Err(error)),
            };
            trace.entries.push(entry);
        }
        break;
    }

    if !found_header {
        return Ok(handle_header_error(
            &mut trace,
            mode,
            diagram_type,
            code.len(),
            0,
        ));
    }

    while let Some(line) = lines.next() {
        control.checkpoint()?;
        let (trimmed, trimmed_start) = trimmed_statement_with_offset(line.text, line.start);
        if trimmed.is_empty() {
            continue;
        }
        let entry = match parse_trace_statement(
            code,
            &mut lines,
            trimmed,
            trimmed_start,
            line.start,
            mode,
            #[cfg(feature = "editor-facts")]
            editor_demand,
        ) {
            Ok(entry) => entry,
            Err(error) => return Ok(Err(error)),
        };
        trace.entries.push(entry);
    }

    control.checkpoint()?;
    Ok(Ok(trace))
}

fn handle_header_error(
    trace: &mut ArchitectureTrace,
    mode: ArchitectureParseMode,
    diagram_type: &str,
    start: usize,
    len: usize,
) -> Result<ArchitectureTrace> {
    let error = if len == 0 {
        Error::diagram_parse_insertion_point(
            diagram_type.to_string(),
            "expected architecture-beta header",
            start,
        )
    } else {
        Error::diagram_parse_exact(
            diagram_type.to_string(),
            "expected architecture-beta header",
            SourceSpan::new(start, start + len),
        )
    };
    if mode == ArchitectureParseMode::Strict {
        return Err(error);
    }
    trace.entries.push(ArchitectureTraceEntry {
        statement: None,
        #[cfg(feature = "editor-facts")]
        span: SourceSpan::new(start, start + len),
        #[cfg(feature = "editor-facts")]
        facts: Vec::new(),
        #[cfg(feature = "editor-facts")]
        diagnostic: Some(take_parse_diagnostic(error)),
        #[cfg(feature = "editor-facts")]
        directive_prefix: None,
    });
    Ok(std::mem::take(trace))
}

fn parse_trace_statement(
    code: &str,
    lines: &mut ArchitectureLineCursor<'_>,
    statement: &str,
    statement_start: usize,
    common_start: usize,
    mode: ArchitectureParseMode,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
) -> Result<ArchitectureTraceEntry> {
    let parsed = if let Some(common) = parse_langium_common(code, common_start) {
        lines.offset = common_start + common.consumed;
        architecture_common_trace_entry(
            common,
            #[cfg(feature = "editor-facts")]
            editor_demand,
        )
    } else {
        let statement = extend_quoted_statement(code, lines, statement, statement_start);
        ArchitectureStatementParser::new(
            statement,
            statement_start,
            #[cfg(feature = "editor-facts")]
            editor_demand,
        )
        .parse()
    };
    match parsed {
        Ok(entry) => Ok(entry),
        Err(failure) if mode == ArchitectureParseMode::Strict => Err(*failure.error),
        Err(_failure) => Ok(ArchitectureTraceEntry {
            statement: None,
            #[cfg(feature = "editor-facts")]
            span: SourceSpan::new(statement_start, statement_start + statement.len()),
            #[cfg(feature = "editor-facts")]
            facts: _failure.facts,
            #[cfg(feature = "editor-facts")]
            diagnostic: Some(take_parse_diagnostic(*_failure.error)),
            #[cfg(feature = "editor-facts")]
            directive_prefix: _failure.directive_prefix,
        }),
    }
}

fn architecture_common_trace_entry(
    parsed: LangiumCommonParse,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
) -> std::result::Result<ArchitectureTraceEntry, StatementFailure> {
    let LangiumCommonParse {
        fact, diagnostic, ..
    } = parsed;
    let spanned = architecture_common_value(&fact);
    let statement = match fact.field {
        LangiumCommonField::Title => ArchitectureStatement::Title(spanned.clone()),
        LangiumCommonField::AccTitle => ArchitectureStatement::AccTitle(spanned.clone()),
        LangiumCommonField::AccDescr => ArchitectureStatement::AccDescr(spanned.clone()),
    };
    #[cfg(feature = "editor-facts")]
    let detail = match fact.field {
        LangiumCommonField::Title => "architecture title",
        LangiumCommonField::AccTitle => "architecture accessibility title",
        LangiumCommonField::AccDescr => "architecture accessibility description",
    };
    #[cfg(feature = "editor-facts")]
    let facts = if editor_demand == EditorFactsDemand::Capture {
        common_payload_fact(&spanned, detail)
    } else {
        Vec::new()
    };

    if let Some(diagnostic) = diagnostic {
        let failure = StatementFailure::new(
            Error::diagram_parse_insertion_point(
                "architecture",
                diagnostic.message,
                diagnostic.span.start,
            ),
            #[cfg(feature = "editor-facts")]
            facts,
        );
        #[cfg(feature = "editor-facts")]
        let failure = failure.with_directive_prefix(fact.field.directive());
        return Err(failure);
    }

    Ok(ArchitectureTraceEntry {
        statement: Some(statement),
        #[cfg(feature = "editor-facts")]
        span: fact.raw_span,
        #[cfg(feature = "editor-facts")]
        facts,
        #[cfg(feature = "editor-facts")]
        diagnostic: None,
        #[cfg(feature = "editor-facts")]
        directive_prefix: Some(fact.field.directive()),
    })
}

fn architecture_common_value(fact: &LangiumCommonFact) -> ArchitectureSpannedValue {
    ArchitectureSpannedValue {
        value: fact.value.clone(),
        #[cfg(feature = "editor-facts")]
        span: fact.raw_span,
        selection: fact.value_span,
    }
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
                if is_ecmascript_line_terminator(ch) {
                    lines.offset = line_ending_end(code, offset);
                    return code[statement_start..offset].trim_end_matches([' ', '\t']);
                }
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
    quote.filter(|_| !escaped)
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

#[cfg(feature = "editor-facts")]
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

#[cfg(feature = "editor-facts")]
fn take_parse_diagnostic(error: Error) -> crate::ParseDiagnostic {
    match error {
        Error::DiagramParse { diagnostic, .. } => diagnostic,
        other => crate::ParseDiagnostic::new(other.to_string()),
    }
}

#[cfg(feature = "editor-facts")]
fn push_recovery_error(facts: &mut EditorSemanticFacts, error: &Error) {
    match error {
        Error::DiagramParse { diagnostic, .. } => {
            facts.mark_recovered_from_parse_error(diagnostic.message(), diagnostic.span());
        }
        other => facts.mark_recovered_from_parse_error(other.to_string(), None),
    }
}

impl ArchitectureTrace {
    #[cfg(all(test, feature = "compat-json"))]
    fn build_db(&self) -> Result<ArchitectureDb> {
        let control = crate::OperationControl::new();
        self.build_db_controlled(&control)
            .expect("a private parse control cannot be cancelled")
    }

    fn build_db_controlled(
        &self,
        control: &crate::OperationControl,
    ) -> crate::OperationControlResult<Result<ArchitectureDb>> {
        let mut db = ArchitectureDb::default();

        for (index, entry) in self.entries.iter().enumerate() {
            if index % 128 == 0 {
                control.checkpoint()?;
            }
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
        for (index, entry) in self.entries.iter().enumerate() {
            if index % 128 == 0 {
                control.checkpoint()?;
            }
            if let Some(ArchitectureStatement::Group {
                id,
                icon,
                title,
                in_group,
            }) = entry.statement.as_ref()
                && let Err(error) = db.add_group(
                    id.clone(),
                    icon.as_ref().map(|value| value.value.clone()),
                    title.as_ref().map(|value| value.value.clone()),
                    in_group.clone(),
                )
            {
                return Ok(Err(error));
            }
        }
        for (index, entry) in self.entries.iter().enumerate() {
            if index % 128 == 0 {
                control.checkpoint()?;
            }
            if let Some(ArchitectureStatement::Service {
                id,
                icon,
                icon_text,
                title,
                in_group,
            }) = entry.statement.as_ref()
                && let Err(error) = db.add_service(
                    id.clone(),
                    icon.as_ref().map(|value| value.value.clone()),
                    icon_text.as_ref().map(|value| value.value.clone()),
                    title.as_ref().map(|value| value.value.clone()),
                    in_group.clone(),
                )
            {
                return Ok(Err(error));
            }
        }
        for (index, entry) in self.entries.iter().enumerate() {
            if index % 128 == 0 {
                control.checkpoint()?;
            }
            if let Some(ArchitectureStatement::Junction { id, in_group }) = entry.statement.as_ref()
                && let Err(error) = db.add_junction(id.clone(), in_group.clone())
            {
                return Ok(Err(error));
            }
        }
        for (index, entry) in self.entries.iter().enumerate() {
            if index % 128 == 0 {
                control.checkpoint()?;
            }
            if let Some(ArchitectureStatement::Edge(edge)) = entry.statement.as_ref()
                && let Err(error) = db.add_edge(edge.clone())
            {
                return Ok(Err(error));
            }
        }
        for (index, entry) in self.entries.iter().enumerate() {
            if index % 128 == 0 {
                control.checkpoint()?;
            }
            if let Some(ArchitectureStatement::Alignment { direction, members }) =
                entry.statement.as_ref()
                && let Err(error) = db.add_layout_hint_controlled(
                    ArchitectureLayoutDirection::parse(&direction.value)
                        .expect("validated alignment direction"),
                    members.clone(),
                    control,
                )?
            {
                return Ok(Err(error));
            }
        }
        control.checkpoint()?;
        Ok(Ok(db))
    }

    #[cfg(all(test, feature = "compat-json"))]
    #[cfg(feature = "editor-facts")]
    fn editor_facts(
        &self,
        db: Option<&ArchitectureDb>,
        validation_error: Option<&Error>,
    ) -> EditorSemanticFacts {
        let control = crate::OperationControl::new();
        self.editor_facts_controlled(db, validation_error, EditorFactsDemand::Capture, &control)
            .expect("a private parse control cannot be cancelled")
    }

    #[cfg(feature = "editor-facts")]
    fn declaration_kinds_controlled<'a>(
        &'a self,
        control: &crate::OperationControl,
    ) -> crate::OperationControlResult<std::collections::HashMap<&'a str, EditorSemanticKind>> {
        let mut kinds = std::collections::HashMap::new();
        for (index, entry) in self.entries.iter().enumerate() {
            if index % 128 == 0 {
                control.checkpoint()?;
            }
            let declaration = match entry.statement.as_ref() {
                Some(ArchitectureStatement::Group { id, .. }) => {
                    Some((id.text.as_str(), EditorSemanticKind::Namespace))
                }
                Some(ArchitectureStatement::Service { id, .. }) => {
                    Some((id.text.as_str(), EditorSemanticKind::Variable))
                }
                Some(ArchitectureStatement::Junction { id, .. }) => {
                    Some((id.text.as_str(), EditorSemanticKind::Object))
                }
                _ => None,
            };
            if let Some((id, kind)) = declaration {
                kinds.entry(id).or_insert(kind);
            }
        }
        control.checkpoint()?;
        Ok(kinds)
    }

    #[cfg(feature = "editor-facts")]
    fn editor_facts_controlled(
        &self,
        db: Option<&ArchitectureDb>,
        validation_error: Option<&Error>,
        editor_demand: EditorFactsDemand,
        control: &crate::OperationControl,
    ) -> crate::OperationControlResult<EditorSemanticFacts> {
        let mut facts = EditorSemanticFacts::new();
        let recovered_declaration_kinds = if editor_demand.captures_rich() && db.is_none() {
            Some(self.declaration_kinds_controlled(control)?)
        } else {
            None
        };
        for (entry_index, entry) in self.entries.iter().enumerate() {
            if entry_index % 128 == 0 {
                control.checkpoint()?;
            }
            debug_assert!(entry.span.start <= entry.span.end);
            if editor_demand.captures_rich()
                && let Some(prefix) = entry.directive_prefix
            {
                facts.push_directive_prefix(prefix);
            }
            for (fact_index, fact) in entry
                .facts
                .iter()
                .filter(|_| editor_demand.captures_rich())
                .enumerate()
            {
                if fact_index % 128 == 0 {
                    control.checkpoint()?;
                }
                facts.push_expected_syntax(EditorExpectedSyntax::new(
                    fact.expected,
                    fact.value.selection,
                ));
                let kind = match fact.role {
                    ArchitectureTraceFactRole::Entity => db
                        .and_then(|db| db.editor_kind_for_id(&fact.value.value))
                        .unwrap_or(fact.kind),
                    ArchitectureTraceFactRole::Reference => db
                        .and_then(|db| db.editor_kind_for_id(&fact.value.value))
                        .or_else(|| {
                            recovered_declaration_kinds
                                .as_ref()
                                .and_then(|kinds| kinds.get(fact.value.value.as_str()).copied())
                        })
                        .unwrap_or(fact.kind),
                    ArchitectureTraceFactRole::Payload => fact.kind,
                };
                let symbol = match fact.role {
                    ArchitectureTraceFactRole::Entity => EditorSemanticSymbol::new(
                        fact.value.value.clone(),
                        Some(fact.detail.to_string()),
                        kind,
                        fact.value.span,
                        fact.value.selection,
                    )
                    .with_rename_policy(EditorRenamePolicy::ArchitectureIdentifier),
                    ArchitectureTraceFactRole::Reference => EditorSemanticSymbol::reference(
                        fact.value.value.clone(),
                        Some(fact.detail.to_string()),
                        kind,
                        fact.value.span,
                        fact.value.selection,
                    )
                    .with_rename_policy(EditorRenamePolicy::ArchitectureIdentifier),
                    ArchitectureTraceFactRole::Payload => EditorSemanticSymbol::payload(
                        fact.value.value.clone(),
                        Some(fact.detail.to_string()),
                        kind,
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
        control.checkpoint()?;
        Ok(facts)
    }
}

#[cfg(all(feature = "compat-json", any(test, feature = "editor-facts")))]
impl ArchitectureSemanticSource {
    #[cfg(test)]
    pub(super) fn compat_json(&self, meta: &PresentationMetadata) -> Value {
        super::render_model_to_compat_json(&self.db.semantic_model(), meta)
            .expect("Architecture typed model must remain JSON-serializable")
    }

    #[cfg(test)]
    pub(super) fn render_model(&self) -> ArchitectureDiagramModel {
        self.db.semantic_model()
    }

    #[cfg(all(test, feature = "editor-facts"))]
    pub(super) fn editor_facts(&self) -> EditorSemanticFacts {
        self.editor_facts.clone()
    }

    #[cfg(feature = "editor-facts")]
    pub(super) fn into_combined_parts_controlled(
        self,
        meta: &PresentationMetadata,
        control: &crate::OperationControl,
    ) -> crate::OperationControlResult<(Result<Value>, EditorSemanticFacts)> {
        control.checkpoint()?;
        let model = self.db.semantic_model_controlled(control)?;
        let json = super::render_model_to_compat_json_controlled(&model, meta, control)?;
        control.checkpoint()?;
        Ok((json, self.editor_facts))
    }
}
