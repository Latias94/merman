//! Architecture statement scanning, category replay, and editor observations.

use merman_core_preprocess::scan::langium::{
    LangiumCommonFact, LangiumCommonField, LangiumCommonParse, is_ecmascript_line_terminator,
    parse_langium_common, parse_langium_string,
};
use merman_model::families::architecture::ArchitectureLayoutDirection;
use merman_model::{OperationControl, OperationControlResult, SourceSpan};

use super::{
    ArchitectureDb, ArchitectureEdge, ArchitectureIdentifier, ArchitectureIssue,
    ArchitectureLineCursor, ArchitectureResult, ArchitectureSource,
    architecture_reserved_id_message, is_architecture_reserved_id, trimmed_statement_with_offset,
};

#[cfg(any(test, feature = "test-support"))]
use std::cell::Cell;

#[cfg(any(test, feature = "test-support"))]
thread_local! {
    static ARCHITECTURE_SYNTAX_CONSTRUCTION_COUNT: Cell<usize> = const { Cell::new(0) };
}

#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
pub fn reset_architecture_syntax_construction_count() {
    ARCHITECTURE_SYNTAX_CONSTRUCTION_COUNT.set(0);
}

#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
pub fn architecture_syntax_construction_count() -> usize {
    ARCHITECTURE_SYNTAX_CONSTRUCTION_COUNT.get()
}

/// The resolved Architecture kind of an editor occurrence.
#[cfg(feature = "editor-facts")]
#[derive(Debug, Clone, Copy)]
pub enum ArchitectureOccurrenceKind {
    Group,
    Service,
    Junction,
    Text,
}

/// Borrowed editor facts from the same trace used by semantic replay.
#[cfg(feature = "editor-facts")]
pub enum ArchitectureObservation<'trace> {
    CaptureStarted,
    DirectivePrefix(&'static str),
    Occurrence {
        text: &'trace str,
        detail: &'static str,
        kind: ArchitectureOccurrenceKind,
        role: ArchitectureOccurrenceRole,
        span: SourceSpan,
        selection: SourceSpan,
    },
    Issue(&'trace ArchitectureIssue),
}

#[cfg(feature = "editor-facts")]
type Observer<'a> = dyn FnMut(ArchitectureObservation<'_>) + 'a;

fn parse_layout_direction(value: &str) -> Option<ArchitectureLayoutDirection> {
    match value {
        "row" => Some(ArchitectureLayoutDirection::Row),
        "column" => Some(ArchitectureLayoutDirection::Column),
        _ => None,
    }
}

#[derive(Debug, Clone)]
struct ArchitectureSpannedValue {
    value: String,
    #[cfg(feature = "editor-facts")]
    span: SourceSpan,
    selection: SourceSpan,
}

#[derive(Debug, Clone, Copy)]
#[cfg(feature = "editor-facts")]
pub enum ArchitectureOccurrenceRole {
    Entity,
    Reference,
    Payload,
}

#[derive(Debug, Clone)]
#[cfg(feature = "editor-facts")]
struct ArchitectureTraceFact {
    value: ArchitectureSpannedValue,
    detail: &'static str,
    kind: ArchitectureOccurrenceKind,
    role: ArchitectureOccurrenceRole,
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
    diagnostic: Option<ArchitectureIssue>,
    #[cfg(feature = "editor-facts")]
    directive_prefix: Option<&'static str>,
}

#[derive(Debug, Clone, Default)]
struct ArchitectureTrace {
    entries: Vec<ArchitectureTraceEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArchitectureParseMode {
    Strict,
    #[cfg(feature = "editor-facts")]
    Recovering,
}

struct StatementFailure {
    error: Box<ArchitectureIssue>,
    #[cfg(feature = "editor-facts")]
    facts: Vec<ArchitectureTraceFact>,
    #[cfg(feature = "editor-facts")]
    directive_prefix: Option<&'static str>,
}

impl StatementFailure {
    fn new(
        error: ArchitectureIssue,
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
    capture_rich: bool,
}

impl<'a> ArchitectureStatementParser<'a> {
    fn new(
        input: &'a str,
        base_offset: usize,
        #[cfg(feature = "editor-facts")] capture_rich: bool,
    ) -> Self {
        Self {
            input,
            base_offset,
            pos: 0,
            #[cfg(feature = "editor-facts")]
            facts: Vec::new(),
            #[cfg(feature = "editor-facts")]
            capture_rich,
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

    fn parse_statement(&mut self) -> ArchitectureResult<ArchitectureStatement> {
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

    fn parse_group(&mut self) -> ArchitectureResult<ArchitectureStatement> {
        self.expect_keyword("group");
        let id = self.parse_id(
            "invalid group id",
            #[cfg(feature = "editor-facts")]
            "architecture group",
            #[cfg(feature = "editor-facts")]
            ArchitectureOccurrenceKind::Group,
        )?;
        let icon = self.parse_icon()?;
        #[cfg(feature = "editor-facts")]
        if let Some(icon) = &icon {
            #[cfg(feature = "editor-facts")]
            self.push_payload(
                icon,
                "architecture group icon",
                ArchitectureOccurrenceKind::Text,
            );
        }
        let title = self.parse_title()?;
        #[cfg(feature = "editor-facts")]
        if let Some(title) = &title {
            #[cfg(feature = "editor-facts")]
            self.push_payload(
                title,
                "architecture group title",
                ArchitectureOccurrenceKind::Text,
            );
        }
        let in_group = if self.consume_keyword("in") {
            Some(self.parse_reference(
                "invalid group parent id",
                #[cfg(feature = "editor-facts")]
                "architecture group parent",
                #[cfg(feature = "editor-facts")]
                ArchitectureOccurrenceKind::Group,
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

    fn parse_service(&mut self) -> ArchitectureResult<ArchitectureStatement> {
        self.expect_keyword("service");
        let id = self.parse_id(
            "invalid service id",
            #[cfg(feature = "editor-facts")]
            "architecture service",
            #[cfg(feature = "editor-facts")]
            ArchitectureOccurrenceKind::Service,
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
                        ArchitectureOccurrenceKind::Text,
                    );
                }
            }
            Some('"' | '\'') => {
                icon_text = Some(self.parse_string()?);
                #[cfg(feature = "editor-facts")]
                self.push_payload(
                    icon_text.as_ref().expect("icon text was just parsed"),
                    "architecture service icon text",
                    ArchitectureOccurrenceKind::Text,
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
                ArchitectureOccurrenceKind::Text,
            );
        }
        let in_group = if self.consume_keyword("in") {
            Some(self.parse_reference(
                "invalid service parent id",
                #[cfg(feature = "editor-facts")]
                "architecture service parent",
                #[cfg(feature = "editor-facts")]
                ArchitectureOccurrenceKind::Group,
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

    fn parse_junction(&mut self) -> ArchitectureResult<ArchitectureStatement> {
        self.expect_keyword("junction");
        let id = self.parse_id(
            "invalid junction id",
            #[cfg(feature = "editor-facts")]
            "architecture junction",
            #[cfg(feature = "editor-facts")]
            ArchitectureOccurrenceKind::Junction,
        )?;
        let in_group = if self.consume_keyword("in") {
            Some(self.parse_reference(
                "invalid junction parent id",
                #[cfg(feature = "editor-facts")]
                "architecture junction parent",
                #[cfg(feature = "editor-facts")]
                ArchitectureOccurrenceKind::Group,
            )?)
        } else {
            None
        };
        self.finish()?;
        Ok(ArchitectureStatement::Junction { id, in_group })
    }

    fn parse_alignment(&mut self) -> ArchitectureResult<ArchitectureStatement> {
        self.expect_keyword("align");
        let direction = self.parse_raw_id("invalid align direction")?;
        if parse_layout_direction(&direction.value).is_none() {
            return Err(ArchitectureIssue::exact(
                "invalid align direction",
                direction.selection,
            ));
        }
        #[cfg(feature = "editor-facts")]
        self.push_payload(
            &direction,
            "architecture alignment direction",
            ArchitectureOccurrenceKind::Text,
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
                ArchitectureOccurrenceKind::Service,
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

    fn parse_edge(&mut self) -> ArchitectureResult<ArchitectureStatement> {
        let lhs = self.parse_reference(
            "invalid id",
            #[cfg(feature = "editor-facts")]
            "architecture edge endpoint",
            #[cfg(feature = "editor-facts")]
            ArchitectureOccurrenceKind::Service,
        )?;
        let lhs_group = self.parse_group_modifier();
        self.expect_delimiter(':', "expected ':' for lhs port")?;
        let lhs_direction =
            self.parse_direction("expected lhs direction", "invalid lhs direction")?;
        #[cfg(feature = "editor-facts")]
        self.push_payload(
            &lhs_direction,
            "architecture edge lhs direction",
            ArchitectureOccurrenceKind::Text,
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
                ArchitectureOccurrenceKind::Text,
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
            ArchitectureOccurrenceKind::Text,
        );
        self.expect_delimiter(':', "expected ':' for rhs port")?;
        let rhs = self.parse_reference(
            "invalid id",
            #[cfg(feature = "editor-facts")]
            "architecture edge endpoint",
            #[cfg(feature = "editor-facts")]
            ArchitectureOccurrenceKind::Service,
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
        #[cfg(feature = "editor-facts")] kind: ArchitectureOccurrenceKind,
    ) -> ArchitectureResult<ArchitectureIdentifier> {
        let token = self.parse_raw_id(message)?;
        if is_architecture_reserved_id(&token.value) {
            return Err(ArchitectureIssue::exact(
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
        #[cfg(feature = "editor-facts")] kind: ArchitectureOccurrenceKind,
    ) -> ArchitectureResult<ArchitectureIdentifier> {
        let token = self.parse_raw_id(message)?;
        if is_architecture_reserved_id(&token.value) {
            return Err(ArchitectureIssue::exact(
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

    fn parse_raw_id(&mut self, message: &str) -> ArchitectureResult<ArchitectureSpannedValue> {
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

    fn parse_icon(&mut self) -> ArchitectureResult<Option<ArchitectureSpannedValue>> {
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

    fn parse_string(&mut self) -> ArchitectureResult<ArchitectureSpannedValue> {
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

    fn parse_title(&mut self) -> ArchitectureResult<Option<ArchitectureSpannedValue>> {
        self.skip_ws();
        if self.peek_char() != Some('[') {
            return Ok(None);
        }
        let start = self.pos;
        self.bump();
        let selection_start = self.pos;
        let Some(consumed) = architecture_title_end(&self.input[start..]) else {
            while let Some(ch) = self.peek_char() {
                if ch == ']' && self.pos == selection_start {
                    return Err(self.insertion_error("empty architecture title"));
                }
                if matches!(ch, '[' | '\r' | '\n') {
                    return Err(self.exact_char_error("invalid architecture title", ch));
                }
                self.bump();
            }
            return Err(self.insertion_error("unterminated architecture title"));
        };
        self.pos = start + consumed;
        let selection_end = self.pos - 1;
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
    ) -> ArchitectureResult<ArchitectureSpannedValue> {
        self.skip_ws();
        let start = self.pos;
        let Some(ch) = self.bump() else {
            return Err(self.insertion_error(missing_message));
        };
        if !matches!(ch, 'L' | 'R' | 'T' | 'B') {
            return Err(ArchitectureIssue::exact(
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
        kind: ArchitectureOccurrenceKind,
    ) {
        #[cfg(feature = "editor-facts")]
        if !self.capture_rich {
            return;
        }
        self.facts.push(ArchitectureTraceFact {
            value: value.clone(),
            detail,
            kind,
            role: ArchitectureOccurrenceRole::Entity,
        });
    }

    #[cfg(feature = "editor-facts")]
    fn push_reference(
        &mut self,
        value: &ArchitectureSpannedValue,
        detail: &'static str,
        kind: ArchitectureOccurrenceKind,
    ) {
        #[cfg(feature = "editor-facts")]
        if !self.capture_rich {
            return;
        }
        self.facts.push(ArchitectureTraceFact {
            value: value.clone(),
            detail,
            kind,
            role: ArchitectureOccurrenceRole::Reference,
        });
    }

    #[cfg(feature = "editor-facts")]
    fn push_payload(
        &mut self,
        value: &ArchitectureSpannedValue,
        detail: &'static str,
        kind: ArchitectureOccurrenceKind,
    ) {
        #[cfg(feature = "editor-facts")]
        if !self.capture_rich {
            return;
        }
        if value.value.is_empty() {
            return;
        }
        self.facts.push(ArchitectureTraceFact {
            value: value.clone(),
            detail,
            kind,
            role: ArchitectureOccurrenceRole::Payload,
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

    fn expect_delimiter(&mut self, expected: char, message: &str) -> ArchitectureResult<()> {
        self.expect_syntax_char(expected, message)
    }

    fn expect_operator(&mut self, expected: char, message: &str) -> ArchitectureResult<()> {
        self.expect_syntax_char(expected, message)
    }

    fn expect_syntax_char(&mut self, expected: char, message: &str) -> ArchitectureResult<()> {
        self.skip_ws();
        if self.peek_char() != Some(expected) {
            return Err(self.insertion_error(message));
        }
        self.bump();
        Ok(())
    }

    fn finish(&mut self) -> ArchitectureResult<()> {
        self.skip_ws();
        if self.is_eof() {
            return Ok(());
        }
        let start = self.pos;
        let end = self.input.len();
        Err(ArchitectureIssue::exact(
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

    fn insertion_error(&self, message: &str) -> ArchitectureIssue {
        ArchitectureIssue::insertion_point(message, self.base_offset + self.pos)
    }

    fn exact_char_error(&self, message: &str, ch: char) -> ArchitectureIssue {
        ArchitectureIssue::exact(
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

/// Parses one preprocessed source using the strict Architecture admission path.
pub fn parse_architecture_source_controlled(
    code: &str,
    control: &OperationControl,
) -> OperationControlResult<Result<ArchitectureSource, ArchitectureIssue>> {
    construct_source_controlled(
        code,
        #[cfg(feature = "editor-facts")]
        false,
        #[cfg(feature = "editor-facts")]
        None,
        control,
    )
}

/// Parses one source with recovery and reports issues and optional rich editor occurrences.
#[cfg(feature = "editor-facts")]
pub fn parse_architecture_source_observed_controlled(
    code: &str,
    capture_rich: bool,
    observe: &mut dyn FnMut(ArchitectureObservation<'_>),
    control: &OperationControl,
) -> OperationControlResult<Result<ArchitectureSource, ArchitectureIssue>> {
    construct_source_controlled(code, capture_rich, Some(observe), control)
}

fn construct_source_controlled(
    code: &str,
    #[cfg(feature = "editor-facts")] capture_rich: bool,
    #[cfg(feature = "editor-facts")] mut observe: Option<&mut Observer<'_>>,
    control: &OperationControl,
) -> OperationControlResult<ArchitectureResult<ArchitectureSource>> {
    control.checkpoint()?;
    #[cfg(feature = "editor-facts")]
    let mode = if observe.is_some() {
        ArchitectureParseMode::Recovering
    } else {
        ArchitectureParseMode::Strict
    };
    #[cfg(not(feature = "editor-facts"))]
    let mode = ArchitectureParseMode::Strict;
    let trace = match parse_trace_controlled(
        code,
        mode,
        #[cfg(feature = "editor-facts")]
        capture_rich,
        #[cfg(feature = "editor-facts")]
        &mut observe,
        control,
    )? {
        Ok(trace) => trace,
        Err(issue) => {
            #[cfg(feature = "editor-facts")]
            if let Some(observe) = observe {
                observe(ArchitectureObservation::Issue(&issue));
            }
            return Ok(Err(issue));
        }
    };
    control.checkpoint()?;
    #[cfg(feature = "editor-facts")]
    let mut syntax_error = None;
    #[cfg(feature = "editor-facts")]
    if observe.is_some() {
        for (index, entry) in trace.entries.iter().enumerate() {
            if index % 128 == 0 {
                control.checkpoint()?;
            }
            if let Some(issue) = entry.diagnostic.clone() {
                syntax_error = Some(issue.with_detected_type());
                break;
            }
        }
    }
    control.checkpoint()?;
    let db = trace.build_db_controlled(control)?;
    #[cfg(feature = "editor-facts")]
    if let Some(observe) = observe {
        trace.observe_editor_facts_controlled(
            db.as_ref().ok(),
            db.as_ref().err(),
            capture_rich,
            observe,
            control,
        )?;
    }
    control.checkpoint()?;
    #[cfg(feature = "editor-facts")]
    let db = match syntax_error {
        Some(error) => Err(error),
        None => db,
    };
    Ok(db.map(ArchitectureSource))
}

fn parse_trace_controlled(
    code: &str,
    mode: ArchitectureParseMode,
    #[cfg(feature = "editor-facts")] capture_rich: bool,
    #[cfg(feature = "editor-facts")] observe: &mut Option<&mut Observer<'_>>,
    control: &OperationControl,
) -> OperationControlResult<ArchitectureResult<ArchitectureTrace>> {
    control.checkpoint()?;
    #[cfg(any(test, feature = "test-support"))]
    ARCHITECTURE_SYNTAX_CONSTRUCTION_COUNT.set(ARCHITECTURE_SYNTAX_CONSTRUCTION_COUNT.get() + 1);
    #[cfg(feature = "editor-facts")]
    if capture_rich && let Some(observe) = observe.as_deref_mut() {
        observe(ArchitectureObservation::CaptureStarted);
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
                &mut lines,
                rest,
                statement_start,
                common_start,
                mode,
                #[cfg(feature = "editor-facts")]
                capture_rich,
                control,
            )? {
                Ok(entry) => entry,
                Err(error) => return Ok(Err(error)),
            };
            trace.entries.push(entry);
        }
        break;
    }

    if !found_header {
        return Ok(handle_header_error(&mut trace, mode, code.len(), 0));
    }

    while let Some(line) = lines.next() {
        control.checkpoint()?;
        let (trimmed, trimmed_start) = trimmed_statement_with_offset(line.text, line.start);
        if trimmed.is_empty() {
            continue;
        }
        let entry = match parse_trace_statement(
            &mut lines,
            trimmed,
            trimmed_start,
            line.start,
            mode,
            #[cfg(feature = "editor-facts")]
            capture_rich,
            control,
        )? {
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
    start: usize,
    len: usize,
) -> ArchitectureResult<ArchitectureTrace> {
    let error = if len == 0 {
        ArchitectureIssue::insertion_point("expected architecture-beta header", start)
    } else {
        ArchitectureIssue::exact(
            "expected architecture-beta header",
            SourceSpan::new(start, start + len),
        )
    }
    .with_detected_type();
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
        diagnostic: Some(error),
        #[cfg(feature = "editor-facts")]
        directive_prefix: None,
    });
    Ok(std::mem::take(trace))
}

fn parse_trace_statement(
    lines: &mut ArchitectureLineCursor<'_>,
    _statement: &str,
    statement_start: usize,
    common_start: usize,
    mode: ArchitectureParseMode,
    #[cfg(feature = "editor-facts")] capture_rich: bool,
    control: &OperationControl,
) -> OperationControlResult<ArchitectureResult<ArchitectureTraceEntry>> {
    let code = lines.source;
    let parsed = if let Some(common) = parse_langium_common(code, common_start) {
        lines.offset = common_start + common.consumed;
        architecture_common_trace_entry(
            common,
            #[cfg(feature = "editor-facts")]
            capture_rich,
        )
    } else {
        let statement = read_statement(code, lines, statement_start, control)?;
        ArchitectureStatementParser::new(
            statement,
            statement_start,
            #[cfg(feature = "editor-facts")]
            capture_rich,
        )
        .parse()
    };
    Ok(match parsed {
        Ok(entry) => Ok(entry),
        Err(failure) if mode == ArchitectureParseMode::Strict => Err(*failure.error),
        Err(_failure) => Ok(ArchitectureTraceEntry {
            statement: None,
            #[cfg(feature = "editor-facts")]
            span: SourceSpan::new(statement_start, statement_start + _statement.len()),
            #[cfg(feature = "editor-facts")]
            facts: _failure.facts,
            #[cfg(feature = "editor-facts")]
            diagnostic: Some(*_failure.error),
            #[cfg(feature = "editor-facts")]
            directive_prefix: _failure.directive_prefix,
        }),
    })
}

fn architecture_common_trace_entry(
    parsed: LangiumCommonParse,
    #[cfg(feature = "editor-facts")] capture_rich: bool,
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
    let facts = if capture_rich {
        common_payload_fact(&spanned, detail)
    } else {
        Vec::new()
    };

    if let Some(diagnostic) = diagnostic {
        let failure = StatementFailure::new(
            ArchitectureIssue::insertion_point(diagnostic.message, diagnostic.span.start),
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

struct QuotedTerminalSearch {
    end: Option<usize>,
    examined_end: usize,
}

fn search_quoted_terminal<E>(
    input: &str,
    mut visit: impl FnMut() -> std::result::Result<(), E>,
) -> std::result::Result<QuotedTerminalSearch, E> {
    let mut chars = input.char_indices();
    let Some((_, quote @ ('\'' | '"'))) = chars.next() else {
        return Ok(QuotedTerminalSearch {
            end: None,
            examined_end: 0,
        });
    };
    let mut escaped = false;
    for (offset, ch) in chars {
        visit()?;
        if escaped {
            if is_ecmascript_line_terminator(ch) {
                return Ok(QuotedTerminalSearch {
                    end: None,
                    examined_end: offset + ch.len_utf8(),
                });
            }
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == quote {
            return Ok(QuotedTerminalSearch {
                end: Some(offset + ch.len_utf8()),
                examined_end: offset + ch.len_utf8(),
            });
        }
    }
    Ok(QuotedTerminalSearch {
        end: None,
        examined_end: input.len(),
    })
}

fn quoted_terminal_end(input: &str) -> Option<usize> {
    match search_quoted_terminal(input, || Ok::<_, std::convert::Infallible>(())) {
        Ok(search) => search.end,
        Err(never) => match never {},
    }
}

fn architecture_title_end(input: &str) -> Option<usize> {
    let content = input.strip_prefix('[')?;
    architecture_title_end_after_quote(input, quoted_terminal_end(content))
}

fn architecture_title_end_after_quote(input: &str, quoted_end: Option<usize>) -> Option<usize> {
    let content = input.strip_prefix('[')?;
    if let Some(end) = quoted_end
        && content[end..].starts_with(']')
    {
        return Some(end + 2);
    }

    // ARCH_TITLE also permits plain text, including quotes and backslashes. A leading
    // quote selects the quoted alternative only when that entire terminal matches.
    for (offset, ch) in content.char_indices() {
        match ch {
            ']' => return (offset != 0).then_some(offset + 2),
            '[' | '\r' | '\n' => return None,
            _ => {}
        }
    }
    None
}

fn statement_end_using<E>(
    input: &str,
    mut quoted_end: impl FnMut(usize) -> std::result::Result<Option<usize>, E>,
) -> std::result::Result<usize, E> {
    let bytes = input.as_bytes();
    let mut offset = 0;
    while let Some(&byte) = bytes.get(offset) {
        match byte {
            b'[' | b'\'' | b'"' => {
                let remaining = &input[offset..];
                let end = if byte == b'[' {
                    architecture_title_end_after_quote(remaining, quoted_end(offset + 1)?)
                } else {
                    quoted_end(offset)?
                };
                if let Some(end) = end {
                    offset += end;
                    continue;
                }
                // An incomplete terminal stays on its physical line for diagnostics and
                // recovery; only a complete STRING or quoted ARCH_TITLE can span lines.
                return Ok(offset + remaining.find(['\r', '\n']).unwrap_or(remaining.len()));
            }
            b'%' if bytes.get(offset + 1) == Some(&b'%') => return Ok(offset),
            b'\r' | b'\n' => return Ok(offset),
            _ => offset += 1,
        }
    }
    Ok(input.len())
}

pub(super) fn statement_end(input: &str) -> usize {
    match statement_end_using(input, |offset| {
        Ok::<_, std::convert::Infallible>(quoted_terminal_end(&input[offset..]))
    }) {
        Ok(end) => end,
        Err(never) => match never {},
    }
}

#[derive(Debug, Default)]
pub(super) struct ArchitectureTerminalLookahead {
    failed: [Option<std::ops::Range<usize>>; 2],
    #[cfg(test)]
    inspected_characters: usize,
}

impl ArchitectureTerminalLookahead {
    fn quoted_end(
        &mut self,
        code: &str,
        start: usize,
        control: &OperationControl,
    ) -> OperationControlResult<Option<usize>> {
        let kind = match code.as_bytes().get(start) {
            Some(b'\'') => 0,
            Some(b'"') => 1,
            _ => return Ok(None),
        };
        if self.failed[kind]
            .as_ref()
            .is_some_and(|range| range.contains(&start))
        {
            return Ok(None);
        }
        let mut visited = 0usize;
        let search = search_quoted_terminal(&code[start..], || -> OperationControlResult<()> {
            if visited.is_multiple_of(256) {
                control.checkpoint()?;
            }
            visited += 1;
            #[cfg(test)]
            {
                self.inspected_characters += 1;
            }
            Ok(())
        })?;
        if search.end.is_none() {
            // Every later quote of this kind in the examined interval was escaped.
            // Starting there reaches the same failure without changing the suffix state.
            self.failed[kind] = Some(start..start + search.examined_end);
        }
        Ok(search.end)
    }

    fn statement_end(
        &mut self,
        code: &str,
        start: usize,
        control: &OperationControl,
    ) -> OperationControlResult<usize> {
        statement_end_using(&code[start..], |relative| {
            self.quoted_end(code, start + relative, control)
        })
    }
}

fn read_statement<'a>(
    code: &'a str,
    lines: &mut ArchitectureLineCursor<'_>,
    statement_start: usize,
    control: &OperationControl,
) -> OperationControlResult<&'a str> {
    let end = statement_start
        + lines
            .lookahead
            .statement_end(code, statement_start, control)?;
    lines.offset = next_physical_line_offset(code, end);
    Ok(code[statement_start..end].trim_end_matches([' ', '\t']))
}

fn next_physical_line_offset(code: &str, offset: usize) -> usize {
    code[offset..]
        .find('\n')
        .map_or(code.len(), |relative| offset + relative + 1)
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
            kind: ArchitectureOccurrenceKind::Text,
            role: ArchitectureOccurrenceRole::Payload,
        }]
    }
}

impl ArchitectureTrace {
    fn build_db_controlled(
        &self,
        control: &OperationControl,
    ) -> OperationControlResult<ArchitectureResult<ArchitectureDb>> {
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
                    parse_layout_direction(&direction.value)
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

    #[cfg(feature = "editor-facts")]
    fn declaration_kinds_controlled<'a>(
        &'a self,
        control: &OperationControl,
    ) -> OperationControlResult<std::collections::HashMap<&'a str, ArchitectureOccurrenceKind>>
    {
        let mut kinds = std::collections::HashMap::new();
        for (index, entry) in self.entries.iter().enumerate() {
            if index % 128 == 0 {
                control.checkpoint()?;
            }
            let declaration = match entry.statement.as_ref() {
                Some(ArchitectureStatement::Group { id, .. }) => {
                    Some((id.text.as_str(), ArchitectureOccurrenceKind::Group))
                }
                Some(ArchitectureStatement::Service { id, .. }) => {
                    Some((id.text.as_str(), ArchitectureOccurrenceKind::Service))
                }
                Some(ArchitectureStatement::Junction { id, .. }) => {
                    Some((id.text.as_str(), ArchitectureOccurrenceKind::Junction))
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
    fn observe_editor_facts_controlled(
        &self,
        db: Option<&ArchitectureDb>,
        validation_error: Option<&ArchitectureIssue>,
        capture_rich: bool,
        observe: &mut Observer<'_>,
        control: &OperationControl,
    ) -> OperationControlResult<()> {
        let recovered_declaration_kinds = if capture_rich && db.is_none() {
            Some(self.declaration_kinds_controlled(control)?)
        } else {
            None
        };
        for (entry_index, entry) in self.entries.iter().enumerate() {
            if entry_index % 128 == 0 {
                control.checkpoint()?;
            }
            debug_assert!(entry.span.start <= entry.span.end);
            if capture_rich && let Some(prefix) = entry.directive_prefix {
                observe(ArchitectureObservation::DirectivePrefix(prefix));
            }
            for (fact_index, fact) in entry.facts.iter().filter(|_| capture_rich).enumerate() {
                if fact_index % 128 == 0 {
                    control.checkpoint()?;
                }
                let kind = match fact.role {
                    ArchitectureOccurrenceRole::Entity => db
                        .and_then(|db| db.editor_kind_for_id(&fact.value.value))
                        .unwrap_or(fact.kind),
                    ArchitectureOccurrenceRole::Reference => db
                        .and_then(|db| db.editor_kind_for_id(&fact.value.value))
                        .or_else(|| {
                            recovered_declaration_kinds
                                .as_ref()
                                .and_then(|kinds| kinds.get(fact.value.value.as_str()).copied())
                        })
                        .unwrap_or(fact.kind),
                    ArchitectureOccurrenceRole::Payload => fact.kind,
                };
                observe(ArchitectureObservation::Occurrence {
                    text: &fact.value.value,
                    detail: fact.detail,
                    kind,
                    role: fact.role,
                    span: fact.value.span,
                    selection: fact.value.selection,
                });
            }
            if let Some(issue) = &entry.diagnostic {
                observe(ArchitectureObservation::Issue(issue));
            }
        }
        if let Some(issue) = validation_error {
            observe(ArchitectureObservation::Issue(issue));
        }
        control.checkpoint()
    }
}

#[cfg(test)]
mod tests;
