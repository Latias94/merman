//! Event Modeling syntax, recovery, and family model construction.

use merman_core_preprocess::scan::{
    langium::{LangiumCommonDbFields, LangiumCommonFact, LangiumCommonFacts, LangiumCommonField},
    physical_line_at,
};
use merman_markup::sanitize::sanitize_text;
use merman_model::families::event_modeling::{
    EventModelingDataEntity, EventModelingDiagramModel, EventModelingFrame,
};
use merman_model::text::is_ecmascript_whitespace;
use merman_model::{MermaidConfig, OperationControl, OperationControlResult, SourceSpan};
use std::collections::HashMap;
#[cfg(feature = "editor-facts")]
use std::collections::HashSet;

#[cfg(any(test, feature = "test-support"))]
thread_local! {
    static SYNTAX_CONSTRUCTIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(any(test, feature = "test-support"))]
pub fn reset_eventmodeling_syntax_construction_count() {
    SYNTAX_CONSTRUCTIONS.set(0);
}

#[cfg(any(test, feature = "test-support"))]
pub fn eventmodeling_syntax_construction_count() -> usize {
    SYNTAX_CONSTRUCTIONS.get()
}

#[derive(Debug)]
pub enum EventModelingSourceIssue {
    Exact { message: String, span: SourceSpan },
    InsertionPoint { message: String, offset: usize },
}

impl EventModelingSourceIssue {
    fn exact(message: impl Into<String>, span: SourceSpan) -> Self {
        Self::Exact {
            message: message.into(),
            span,
        }
    }

    fn insertion_point(message: impl Into<String>, offset: usize) -> Self {
        Self::InsertionPoint {
            message: message.into(),
            offset,
        }
    }

    #[cfg(feature = "editor-facts")]
    pub fn span(&self) -> SourceSpan {
        match *self {
            Self::Exact { span, .. } => span,
            Self::InsertionPoint { offset, .. } => SourceSpan::new(offset, offset),
        }
    }
}

type SyntaxResult<T> = Result<T, EventModelingSourceIssue>;

/// Successful source syntax and reference validation, ready for one model construction.
pub struct ParsedEventModeling {
    syntax: EventModelingSyntaxFacts,
}

#[cfg(feature = "editor-facts")]
#[derive(Clone, Copy)]
pub enum EventModelingEditorDemand {
    Recovery,
    Capture,
}

#[cfg(feature = "editor-facts")]
#[derive(Clone, Copy)]
pub enum EventModelingFieldKind {
    Header,
    ModelEntity,
    Frame,
    EntityType,
    EntityIdentifier,
    SourceFrame,
    DataReference,
    DataType,
    InlineData,
    DataEntity,
    DataBlock,
    NoteSourceFrame,
    NoteBlock,
    GwtSourceFrame,
    GwtEntityReference,
}

/// Source-relative facts from the original syntax construction, in projection order.
#[cfg(feature = "editor-facts")]
pub enum EventModelingObservation<'source> {
    CollectionStarted,
    Common(&'source LangiumCommonFact),
    Field {
        kind: EventModelingFieldKind,
        text: &'source str,
        span: SourceSpan,
    },
    ValidationIssue {
        message: &'source str,
        span: SourceSpan,
    },
}

#[cfg(feature = "editor-facts")]
struct EventModelingObserver<'observer> {
    demand: EventModelingEditorDemand,
    emit: &'observer mut dyn FnMut(EventModelingObservation<'_>),
}

#[cfg(feature = "editor-facts")]
impl EventModelingObserver<'_> {
    fn field(&mut self, kind: EventModelingFieldKind, field: &EventModelingFieldSpan) {
        self.field_at(kind, &field.text, field.span);
    }

    fn field_at(&mut self, kind: EventModelingFieldKind, text: &str, span: SourceSpan) {
        (self.emit)(EventModelingObservation::Field { kind, text, span });
    }
}

#[derive(Debug, Clone)]
struct EventModelingFieldSpan {
    text: String,
    span: SourceSpan,
}

#[derive(Debug, Clone)]
struct EventModelingFrameFacts {
    frame_kind: String,
    name: String,
    name_span: SourceSpan,
    model_entity_type: String,
    #[cfg(feature = "editor-facts")]
    model_entity_type_span: SourceSpan,
    entity_identifier: String,
    #[cfg(feature = "editor-facts")]
    entity_identifier_span: SourceSpan,
    source_frames: Vec<EventModelingFieldSpan>,
    data_reference: Option<EventModelingFieldSpan>,
    #[cfg(feature = "editor-facts")]
    data_type: Option<EventModelingFieldSpan>,
    data_inline_value: Option<EventModelingFieldSpan>,
}

#[derive(Debug, Clone)]
struct EventModelingDataEntityFacts {
    name: EventModelingFieldSpan,
    #[cfg(feature = "editor-facts")]
    data_type: Option<EventModelingFieldSpan>,
    block_text: String,
    #[cfg(feature = "editor-facts")]
    block_span: SourceSpan,
}

#[derive(Debug, Clone)]
struct EventModelingNoteFacts {
    #[cfg(feature = "editor-facts")]
    source_frame: EventModelingFieldSpan,
    #[cfg(feature = "editor-facts")]
    data_type: Option<EventModelingFieldSpan>,
    #[cfg(feature = "editor-facts")]
    block_text: String,
    #[cfg(feature = "editor-facts")]
    block_span: SourceSpan,
}

#[derive(Debug, Clone)]
struct EventModelingGwtStatementFacts {
    #[cfg(feature = "editor-facts")]
    model_entity_type: EventModelingFieldSpan,
    #[cfg(feature = "editor-facts")]
    entity_reference: EventModelingFieldSpan,
}

#[derive(Debug, Clone)]
struct EventModelingGwtFacts {
    #[cfg(feature = "editor-facts")]
    source_frame: EventModelingFieldSpan,
    #[cfg(feature = "editor-facts")]
    given: Vec<EventModelingGwtStatementFacts>,
    #[cfg(feature = "editor-facts")]
    when: Vec<EventModelingGwtStatementFacts>,
    #[cfg(feature = "editor-facts")]
    then: Vec<EventModelingGwtStatementFacts>,
}

#[derive(Debug, Clone)]
#[cfg(feature = "editor-facts")]
struct EventModelingValidationDiagnostic {
    message: String,
    span: SourceSpan,
}

#[derive(Default)]
struct EventModelingSyntaxFacts {
    header: Option<EventModelingFieldSpan>,
    common: LangiumCommonFacts,
    model_entities: Vec<EventModelingFieldSpan>,
    frames: Vec<EventModelingFrameFacts>,
    data_entities: Vec<EventModelingDataEntityFacts>,
    note_entities: Vec<EventModelingNoteFacts>,
    gwt_entities: Vec<EventModelingGwtFacts>,
    #[cfg(feature = "editor-facts")]
    validation_diagnostics: Vec<EventModelingValidationDiagnostic>,
}

impl ParsedEventModeling {
    /// Builds the family records. Common metadata remains raw for Core's common DB policy.
    pub fn into_model_controlled(
        self,
        config: &MermaidConfig,
        control: &OperationControl,
    ) -> OperationControlResult<EventModelingDiagramModel> {
        control.checkpoint()?;
        let common = LangiumCommonDbFields::from_facts(&self.syntax.common);
        let mut frames = Vec::with_capacity(self.syntax.frames.len());
        for frame in self.syntax.frames {
            control.checkpoint()?;
            let mut source_frames = Vec::with_capacity(frame.source_frames.len());
            for source in frame.source_frames {
                control.checkpoint()?;
                source_frames.push(source.text);
            }
            let data_inline_value = if let Some(data) = frame.data_inline_value {
                control.checkpoint()?;
                let value = sanitize_text(&data.text, config);
                control.checkpoint()?;
                Some(value)
            } else {
                None
            };
            frames.push(EventModelingFrame {
                name: frame.name,
                frame_kind: frame.frame_kind,
                model_entity_type: frame.model_entity_type,
                entity_identifier: sanitize_text(&frame.entity_identifier, config),
                source_frames,
                data_inline_value,
                data_reference: frame.data_reference.map(|data| data.text),
            });
            control.checkpoint()?;
        }
        let mut data_entities = Vec::with_capacity(self.syntax.data_entities.len());
        for data in self.syntax.data_entities {
            control.checkpoint()?;
            let data_block_value = sanitize_text(&data.block_text, config);
            control.checkpoint()?;
            data_entities.push(EventModelingDataEntity {
                name: data.name.text,
                data_block_value,
            });
        }
        control.checkpoint()?;
        Ok(EventModelingDiagramModel {
            title: common.title,
            acc_title: common.acc_title,
            acc_descr: common.acc_descr,
            frames,
            data_entities,
        })
    }
}

pub fn parse_eventmodeling_source_controlled(
    source: &str,
    control: &OperationControl,
) -> OperationControlResult<Result<ParsedEventModeling, EventModelingSourceIssue>> {
    parse_eventmodeling_source_impl(
        source,
        #[cfg(feature = "editor-facts")]
        None,
        control,
    )
}

#[cfg(feature = "editor-facts")]
pub fn parse_eventmodeling_source_with_observer_controlled(
    source: &str,
    demand: EventModelingEditorDemand,
    observe: &mut impl FnMut(EventModelingObservation<'_>),
    control: &OperationControl,
) -> OperationControlResult<Result<ParsedEventModeling, EventModelingSourceIssue>> {
    parse_eventmodeling_source_impl(
        source,
        Some(EventModelingObserver {
            demand,
            emit: observe,
        }),
        control,
    )
}

fn parse_eventmodeling_source_impl(
    code: &str,
    #[cfg(feature = "editor-facts")] mut observer: Option<EventModelingObserver<'_>>,
    control: &OperationControl,
) -> OperationControlResult<SyntaxResult<ParsedEventModeling>> {
    control.checkpoint()?;
    #[cfg(any(test, feature = "test-support"))]
    SYNTAX_CONSTRUCTIONS.set(SYNTAX_CONSTRUCTIONS.get() + 1);

    let mut syntax = EventModelingSyntaxFacts::default();
    let mut cursor = EventModelingCursor::new(code, control);
    if let Err(issue) = cursor.skip_hidden() {
        control.checkpoint()?;
        return finish_eventmodeling_syntax(
            syntax,
            Some(issue),
            #[cfg(feature = "editor-facts")]
            observer.as_mut(),
            #[cfg(feature = "editor-facts")]
            control,
        );
    }
    control.checkpoint()?;
    let header = parse_eventmodeling_header_cursor(&mut cursor);
    control.checkpoint()?;
    match header {
        Ok(header) => syntax.header = Some(header),
        Err(issue) => {
            return finish_eventmodeling_syntax(
                syntax,
                Some(issue),
                #[cfg(feature = "editor-facts")]
                observer.as_mut(),
                #[cfg(feature = "editor-facts")]
                control,
            );
        }
    }

    let mut first_failure = None;
    loop {
        control.checkpoint()?;
        if let Err(error) = cursor.skip_hidden() {
            control.checkpoint()?;
            first_failure.get_or_insert(error);
            break;
        }
        if cursor.is_eof() {
            break;
        }

        if let Some((field, end)) = cursor.metadata_token() {
            let span = SourceSpan::new(cursor.offset(), end);
            syntax.common.push(LangiumCommonFact {
                field,
                value: code[span.start..span.end].to_string(),
                raw_span: span,
                value_span: span,
            });
            cursor.offset = end;
            continue;
        }

        let statement_start = cursor.offset();
        let result = if cursor.starts_keyword("entity") {
            parse_eventmodeling_entity_cursor(&mut cursor)
                .map(|entity| syntax.model_entities.push(entity))
        } else if cursor.starts_keyword("tf") || cursor.starts_keyword("timeframe") {
            parse_eventmodeling_frame_cursor(&mut cursor, "timeframe")
                .map(|frame| syntax.frames.push(frame))
        } else if cursor.starts_keyword("rf") || cursor.starts_keyword("resetframe") {
            parse_eventmodeling_frame_cursor(&mut cursor, "resetframe")
                .map(|frame| syntax.frames.push(frame))
        } else if cursor.starts_keyword("data") {
            match parse_eventmodeling_data_cursor(&mut cursor) {
                Ok(data) => {
                    syntax.data_entities.push(data);
                    Ok(())
                }
                Err(failure) => {
                    syntax.data_entities.push(*failure.partial);
                    Err(*failure.error)
                }
            }
        } else if cursor.starts_keyword("note") {
            match parse_eventmodeling_note_cursor(&mut cursor) {
                Ok(note) => {
                    syntax.note_entities.push(note);
                    Ok(())
                }
                Err(failure) => {
                    syntax.note_entities.push(*failure.partial);
                    Err(*failure.error)
                }
            }
        } else if cursor.starts_keyword("gwt") {
            parse_eventmodeling_gwt_cursor(&mut cursor).map(|gwt| syntax.gwt_entities.push(gwt))
        } else {
            let span = cursor.unknown_statement_span();
            Err(EventModelingSourceIssue::exact(
                format!(
                    "unexpected eventmodeling statement: {}",
                    &code[span.start..span.end]
                ),
                span,
            ))
        };

        control.checkpoint()?;
        if let Err(error) = result {
            first_failure.get_or_insert(error);
            if cursor.is_eof() {
                break;
            }
            cursor.recover_to_next_statement(statement_start);
            control.checkpoint()?;
        }
    }

    if let Some(issue) = validate_eventmodeling_semantics(
        &mut syntax,
        #[cfg(feature = "editor-facts")]
        observer.is_some(),
        control,
    )? {
        first_failure.get_or_insert(issue);
    }
    finish_eventmodeling_syntax(
        syntax,
        first_failure,
        #[cfg(feature = "editor-facts")]
        observer.as_mut(),
        #[cfg(feature = "editor-facts")]
        control,
    )
}

fn finish_eventmodeling_syntax(
    syntax: EventModelingSyntaxFacts,
    issue: Option<EventModelingSourceIssue>,
    #[cfg(feature = "editor-facts")] observer: Option<&mut EventModelingObserver<'_>>,
    #[cfg(feature = "editor-facts")] control: &OperationControl,
) -> OperationControlResult<SyntaxResult<ParsedEventModeling>> {
    #[cfg(feature = "editor-facts")]
    if let Some(observer) = observer {
        syntax.observe_controlled(observer, control)?;
    }
    Ok(match issue {
        Some(issue) => Err(issue),
        None => Ok(ParsedEventModeling { syntax }),
    })
}

const EVENTMODELING_ENTITY_TYPES: &[&str] = &[
    "rmo",
    "readmodel",
    "ui",
    "cmd",
    "command",
    "evt",
    "event",
    "pcr",
    "processor",
];
const EVENTMODELING_DATA_TYPES: &[&str] = &[
    "json", "jsobj", "figma", "salt", "uri", "md", "html", "text",
];

struct EventModelingCursor<'a> {
    source: &'a str,
    control: &'a OperationControl,
    offset: usize,
    unclosed_metadata_from: std::cell::Cell<Option<usize>>,
    #[cfg(test)]
    metadata_block_scanned_bytes: std::cell::Cell<usize>,
}

impl<'a> EventModelingCursor<'a> {
    fn new(source: &'a str, control: &'a OperationControl) -> Self {
        Self {
            source,
            control,
            offset: 0,
            unclosed_metadata_from: std::cell::Cell::new(None),
            #[cfg(test)]
            metadata_block_scanned_bytes: std::cell::Cell::new(0),
        }
    }

    fn offset(&self) -> usize {
        self.offset
    }

    fn is_eof(&self) -> bool {
        self.offset >= self.source.len()
    }

    fn insertion_span(&self) -> SourceSpan {
        SourceSpan::new(self.offset, self.offset)
    }

    fn starts_keyword(&self, keyword: &str) -> bool {
        let Some(rest) = self.source[self.offset..].strip_prefix(keyword) else {
            return false;
        };
        rest.chars()
            .next()
            .is_none_or(|ch| !ch.is_ascii_alphanumeric() && ch != '_')
    }

    fn consume_keyword(&mut self, keyword: &str) -> bool {
        if !self.starts_keyword(keyword) {
            return false;
        }
        self.offset += keyword.len();
        true
    }

    fn consume_literal(&mut self, literal: &str) -> bool {
        if !self.starts_literal(literal) {
            return false;
        }
        self.offset += literal.len();
        true
    }

    fn starts_literal(&self, literal: &str) -> bool {
        self.source[self.offset..].starts_with(literal)
    }

    fn unknown_statement_span(&self) -> SourceSpan {
        let rest = &self.source[self.offset..];
        let end = rest.find(['\r', '\n']).unwrap_or(rest.len());
        let visible = rest[..end].trim_end();
        let len = if visible.is_empty() {
            rest.chars().next().map(char::len_utf8).unwrap_or_default()
        } else {
            visible.len()
        };
        SourceSpan::new(self.offset, self.offset + len)
    }

    fn recover_to_next_statement(&mut self, statement_start: usize) {
        self.offset = statement_start.min(self.source.len());
        let mut next_checkpoint = self.offset.saturating_add(4096);
        while self.offset < self.source.len() {
            let ch = self.source[self.offset..]
                .chars()
                .next()
                .expect("eventmodeling recovery offset must be a character boundary");
            if ch == '\n' || ch == '\r' {
                break;
            }
            self.offset += ch.len_utf8();
            if self.offset >= next_checkpoint {
                if self.control.checkpoint().is_err() {
                    break;
                }
                next_checkpoint = self.offset.saturating_add(4096);
            }
        }
    }

    fn scan_while(&self, start: usize, predicate: impl Fn(&str) -> bool) -> Option<usize> {
        let mut offset = start;
        let mut next_checkpoint = start.saturating_add(4096);
        while offset < self.source.len() && predicate(&self.source[offset..]) {
            offset += self.source[offset..].chars().next()?.len_utf8();
            if offset >= next_checkpoint {
                self.control.checkpoint().ok()?;
                next_checkpoint = offset.saturating_add(4096);
            }
        }
        Some(offset)
    }

    fn metadata_token(&self) -> Option<(LangiumCommonField, usize)> {
        let directive_start = self.scan_while(self.offset, |rest| {
            rest.starts_with(' ') || rest.starts_with('\t')
        })?;
        if directive_start == self.offset {
            return None;
        }
        let rest = &self.source[directive_start..];
        let (field, after_keyword) = if rest.starts_with("accDescr") {
            (
                LangiumCommonField::AccDescr,
                directive_start + "accDescr".len(),
            )
        } else if rest.starts_with("accTitle") {
            (
                LangiumCommonField::AccTitle,
                directive_start + "accTitle".len(),
            )
        } else if rest.starts_with("title") {
            (LangiumCommonField::Title, directive_start + "title".len())
        } else {
            return None;
        };

        let value_start = match field {
            LangiumCommonField::Title => {
                if !self.source[after_keyword..].starts_with([' ', '\t']) {
                    return Some((field, after_keyword));
                }
                after_keyword + 1
            }
            LangiumCommonField::AccTitle | LangiumCommonField::AccDescr => {
                let after_gap = self.scan_while(after_keyword, |rest| {
                    rest.starts_with(' ') || rest.starts_with('\t')
                })?;
                if self.source[after_gap..].starts_with(':') {
                    after_gap + 1
                } else if field == LangiumCommonField::AccDescr {
                    let open = self.scan_while(after_gap, |rest| {
                        rest.chars().next().is_some_and(is_ecmascript_whitespace)
                    })?;
                    if !self.source[open..].starts_with('{') {
                        return None;
                    }
                    if self
                        .unclosed_metadata_from
                        .get()
                        .is_some_and(|start| open + 1 >= start)
                    {
                        return None;
                    }
                    let close = self.scan_while(open + 1, |rest| !rest.starts_with('}'))?;
                    #[cfg(test)]
                    self.metadata_block_scanned_bytes
                        .set(self.metadata_block_scanned_bytes.get() + close - open - 1);
                    if close == self.source.len() {
                        self.unclosed_metadata_from.set(Some(open + 1));
                        return None;
                    }
                    return Some((field, close + 1));
                } else {
                    return None;
                }
            }
        };
        let end = self.scan_while(value_start, |rest| {
            !rest.starts_with(['\r', '\n']) && !rest.starts_with("%%")
        })?;
        Some((field, end))
    }

    fn skip_hidden(&mut self) -> SyntaxResult<()> {
        loop {
            if self.control.checkpoint().is_err() {
                return Ok(());
            }
            if self.metadata_token().is_some() {
                return Ok(());
            }
            let mut next_checkpoint = self.offset.saturating_add(4096);
            while self.offset < self.source.len() {
                let ch = self.source[self.offset..].chars().next().unwrap();
                if !is_ecmascript_whitespace(ch) {
                    break;
                }
                self.offset += ch.len_utf8();
                if self.offset >= next_checkpoint {
                    if self.control.checkpoint().is_err() {
                        return Ok(());
                    }
                    next_checkpoint = self.offset.saturating_add(4096);
                }
            }
            if self.is_eof() {
                return Ok(());
            }

            let rest = &self.source[self.offset..];
            if rest.starts_with("%%{") {
                let Some(end) = rest.find("}%%") else {
                    return Err(EventModelingSourceIssue::insertion_point(
                        "expected closing eventmodeling directive",
                        self.source.len(),
                    ));
                };
                self.offset += end + 3;
                continue;
            }
            if rest.starts_with("%%") {
                self.offset += rest.find(['\r', '\n']).unwrap_or(rest.len());
                continue;
            }
            if rest.starts_with("//") {
                self.offset += rest.find(['\r', '\n']).unwrap_or(rest.len());
                continue;
            }
            if rest.starts_with("/*") {
                let Some(end) = rest.find("*/") else {
                    return Err(EventModelingSourceIssue::insertion_point(
                        "expected closing eventmodeling block comment",
                        self.source.len(),
                    ));
                };
                self.offset += end + 2;
                continue;
            }
            if rest.starts_with("---")
                && let Some(end) = eventmodeling_yaml_end(self.source, self.offset, self.control)
            {
                self.offset = end;
                continue;
            }
            return Ok(());
        }
    }

    fn take_token(&mut self, expected: &str) -> SyntaxResult<EventModelingFieldSpan> {
        self.skip_hidden()?;
        let start = self.offset;
        let mut next_checkpoint = self.offset.saturating_add(4096);
        while self.offset < self.source.len() {
            let rest = &self.source[self.offset..];
            let ch = rest.chars().next().unwrap();
            if is_ecmascript_whitespace(ch)
                || rest.starts_with("->>")
                || rest.starts_with("[[")
                || rest.starts_with("]]")
                || rest.starts_with("%%")
                || rest.starts_with("//")
                || rest.starts_with("/*")
                || matches!(ch, '`' | '{' | '}' | '"' | '\'')
            {
                break;
            }
            self.offset += ch.len_utf8();
            if self.offset >= next_checkpoint {
                if self.control.checkpoint().is_err() {
                    break;
                }
                next_checkpoint = self.offset.saturating_add(4096);
            }
        }
        if start == self.offset {
            return Err(EventModelingSourceIssue::insertion_point(expected, start));
        }
        Ok(EventModelingFieldSpan {
            text: self.source[start..self.offset].to_string(),
            span: SourceSpan::new(start, self.offset),
        })
    }
}

fn eventmodeling_yaml_end(source: &str, start: usize, control: &OperationControl) -> Option<usize> {
    let rest = source.get(start..)?;
    let opening_newline = rest.find('\n')?;
    let opening = rest[..opening_newline]
        .strip_suffix('\r')
        .unwrap_or(&rest[..opening_newline]);
    if !opening
        .strip_prefix("---")?
        .bytes()
        .all(|byte| matches!(byte, b' ' | b'\t'))
    {
        return None;
    }

    let mut cursor = start + opening_newline + 1;
    while cursor <= source.len() {
        if control.checkpoint().is_err() {
            return None;
        }
        let (line, next) = physical_line_at(source, cursor);
        if line.trim_end_matches([' ', '\t']) == "---" {
            return Some(next);
        }
        if next == cursor || next == source.len() {
            break;
        }
        cursor = next;
    }
    None
}

fn is_eventmodeling_id(value: &str) -> bool {
    let mut chars = value.chars();
    chars
        .next()
        .is_some_and(|ch| ch == '_' || ch.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

fn take_eventmodeling_id_cursor(
    cursor: &mut EventModelingCursor<'_>,
    expected: &str,
) -> SyntaxResult<EventModelingFieldSpan> {
    let field = cursor.take_token(expected)?;
    if !is_eventmodeling_id(&field.text) {
        return Err(EventModelingSourceIssue::exact(expected, field.span));
    }
    Ok(field)
}

fn take_eventmodeling_qualified_name_cursor(
    cursor: &mut EventModelingCursor<'_>,
    expected: &str,
) -> SyntaxResult<EventModelingFieldSpan> {
    cursor.skip_hidden()?;
    let start = cursor.offset;
    let mut text = String::new();
    let mut end = start;

    loop {
        if cursor.control.checkpoint().is_err() {
            break;
        }
        let segment_start = cursor.offset;
        let Some(first) = cursor.source[cursor.offset..].chars().next() else {
            return Err(EventModelingSourceIssue::insertion_point(
                expected,
                cursor.offset,
            ));
        };
        if first != '_' && !first.is_ascii_alphabetic() {
            let invalid = cursor.take_token(expected)?;
            let span = SourceSpan::new(start, invalid.span.end);
            return Err(EventModelingSourceIssue::exact(expected, span));
        }
        cursor.offset += first.len_utf8();
        let mut next_checkpoint = cursor.offset.saturating_add(4096);
        while let Some(ch) = cursor.source[cursor.offset..].chars().next() {
            if ch != '_' && !ch.is_ascii_alphanumeric() {
                break;
            }
            cursor.offset += ch.len_utf8();
            if cursor.offset >= next_checkpoint {
                if cursor.control.checkpoint().is_err() {
                    break;
                }
                next_checkpoint = cursor.offset.saturating_add(4096);
            }
        }
        let segment_span = SourceSpan::new(segment_start, cursor.offset);
        if !text.is_empty() {
            text.push('.');
        }
        text.push_str(&cursor.source[segment_span.start..segment_span.end]);
        end = segment_span.end;

        cursor.skip_hidden()?;
        if !cursor.source[cursor.offset..].starts_with('.') {
            break;
        }
        cursor.offset += 1;
        cursor.skip_hidden()?;
    }

    Ok(EventModelingFieldSpan {
        text,
        span: SourceSpan::new(start, end),
    })
}

fn take_eventmodeling_frame_id_cursor(
    cursor: &mut EventModelingCursor<'_>,
    expected: &str,
) -> SyntaxResult<EventModelingFieldSpan> {
    let field = cursor.take_token(expected)?;
    if field.text.is_empty()
        || field.text.len() > 3
        || !field.text.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(EventModelingSourceIssue::exact(expected, field.span));
    }
    Ok(field)
}

fn take_eventmodeling_entity_type_cursor(
    cursor: &mut EventModelingCursor<'_>,
) -> SyntaxResult<EventModelingFieldSpan> {
    let field = cursor.take_token("expected eventmodeling entity type")?;
    if !EVENTMODELING_ENTITY_TYPES.contains(&field.text.as_str()) {
        return Err(EventModelingSourceIssue::exact(
            "expected eventmodeling entity type",
            field.span,
        ));
    }
    Ok(field)
}

fn parse_eventmodeling_header_cursor(
    cursor: &mut EventModelingCursor<'_>,
) -> SyntaxResult<EventModelingFieldSpan> {
    let start = cursor.offset();
    if !cursor.consume_keyword("eventmodeling") {
        let token = cursor
            .take_token("expected eventmodeling")
            .unwrap_or(EventModelingFieldSpan {
                text: String::new(),
                span: SourceSpan::new(start, start),
            });
        return Err(if token.span.start == token.span.end {
            EventModelingSourceIssue::insertion_point("expected eventmodeling", start)
        } else {
            EventModelingSourceIssue::exact("expected eventmodeling", token.span)
        });
    }
    Ok(EventModelingFieldSpan {
        text: "eventmodeling".to_string(),
        span: SourceSpan::new(start, cursor.offset()),
    })
}

fn parse_eventmodeling_entity_cursor(
    cursor: &mut EventModelingCursor<'_>,
) -> SyntaxResult<EventModelingFieldSpan> {
    cursor.consume_keyword("entity");
    take_eventmodeling_qualified_name_cursor(cursor, "expected eventmodeling model entity name")
}

fn parse_eventmodeling_frame_cursor(
    cursor: &mut EventModelingCursor<'_>,
    frame_kind: &str,
) -> SyntaxResult<EventModelingFrameFacts> {
    if frame_kind == "timeframe" {
        if !cursor.consume_keyword("timeframe") {
            cursor.consume_keyword("tf");
        }
    } else if !cursor.consume_keyword("resetframe") {
        cursor.consume_keyword("rf");
    }

    let name = take_eventmodeling_frame_id_cursor(
        cursor,
        "expected eventmodeling frame id with one to three digits",
    )?;
    let model_entity_type = take_eventmodeling_entity_type_cursor(cursor)?;
    let entity_identifier = take_eventmodeling_qualified_name_cursor(
        cursor,
        "expected eventmodeling qualified entity identifier",
    )?;

    let mut source_frames = Vec::new();
    loop {
        if cursor.control.checkpoint().is_err() {
            break;
        }
        cursor.skip_hidden()?;
        if !cursor.starts_literal("->>") {
            break;
        }
        cursor.consume_literal("->>");
        source_frames.push(take_eventmodeling_frame_id_cursor(
            cursor,
            "expected eventmodeling source frame id",
        )?);
    }

    cursor.skip_hidden()?;
    let data_reference = if cursor.starts_literal("[[") {
        cursor.consume_literal("[[");
        let reference =
            take_eventmodeling_id_cursor(cursor, "expected eventmodeling data reference")?;
        cursor.skip_hidden()?;
        if !cursor.starts_literal("]]") {
            return Err(EventModelingSourceIssue::insertion_point(
                "expected closing ']]' for eventmodeling data reference",
                cursor.offset(),
            ));
        }
        cursor.consume_literal("]]");
        Some(reference)
    } else {
        None
    };

    let (_data_type, data_inline_value) = parse_eventmodeling_optional_inline(cursor)?;
    Ok(EventModelingFrameFacts {
        frame_kind: frame_kind.to_string(),
        name: name.text,
        name_span: name.span,
        model_entity_type: model_entity_type.text,
        #[cfg(feature = "editor-facts")]
        model_entity_type_span: model_entity_type.span,
        entity_identifier: entity_identifier.text,
        #[cfg(feature = "editor-facts")]
        entity_identifier_span: entity_identifier.span,
        source_frames,
        data_reference,
        #[cfg(feature = "editor-facts")]
        data_type: _data_type,
        data_inline_value,
    })
}

fn parse_eventmodeling_optional_data_type(
    cursor: &mut EventModelingCursor<'_>,
) -> SyntaxResult<Option<EventModelingFieldSpan>> {
    cursor.skip_hidden()?;
    if !cursor.starts_literal("`") {
        return Ok(None);
    }
    cursor.consume_literal("`");
    let field = cursor.take_token("expected eventmodeling data type")?;
    if !EVENTMODELING_DATA_TYPES.contains(&field.text.as_str()) {
        return Err(EventModelingSourceIssue::exact(
            format!("unsupported eventmodeling data type '{}'", field.text),
            field.span,
        ));
    }
    cursor.skip_hidden()?;
    if !cursor.starts_literal("`") {
        return Err(EventModelingSourceIssue::insertion_point(
            "expected closing backtick for eventmodeling data type",
            cursor.offset(),
        ));
    }
    cursor.consume_literal("`");
    Ok(Some(field))
}

fn parse_eventmodeling_optional_inline(
    cursor: &mut EventModelingCursor<'_>,
) -> SyntaxResult<(
    Option<EventModelingFieldSpan>,
    Option<EventModelingFieldSpan>,
)> {
    let data_type = parse_eventmodeling_optional_data_type(cursor)?;
    cursor.skip_hidden()?;
    let Some(delimiter) = cursor.source[cursor.offset..].chars().next() else {
        if data_type.is_some() {
            return Err(EventModelingSourceIssue::insertion_point(
                "expected eventmodeling inline data",
                cursor.offset(),
            ));
        }
        return Ok((None, None));
    };
    if !matches!(delimiter, '{' | '"' | '\'') {
        if data_type.is_some() {
            return Err(EventModelingSourceIssue::insertion_point(
                "expected eventmodeling inline data",
                cursor.offset(),
            ));
        }
        return Ok((None, None));
    }

    let start = cursor.offset;
    let rest = &cursor.source[start..];
    let line_end = rest.find(['\r', '\n']).unwrap_or(rest.len());
    let line = &rest[..line_end];
    let end = if delimiter == '{' {
        line.rfind('}').map(|index| start + index + 1)
    } else {
        line[delimiter.len_utf8()..]
            .rfind(delimiter)
            .map(|index| start + delimiter.len_utf8() + index + delimiter.len_utf8())
    }
    .ok_or_else(|| {
        EventModelingSourceIssue::insertion_point(
            "expected closing delimiter for eventmodeling inline data",
            start + line_end,
        )
    })?;
    cursor.offset = end;
    Ok((
        data_type,
        Some(EventModelingFieldSpan {
            text: cursor.source[start..end].to_string(),
            span: SourceSpan::new(start, end),
        }),
    ))
}

struct ParsedEventModelingBlock {
    #[cfg(feature = "editor-facts")]
    data_type: Option<EventModelingFieldSpan>,
    text: String,
    #[cfg(feature = "editor-facts")]
    span: SourceSpan,
}

struct FailedEventModelingBlock {
    error: Box<EventModelingSourceIssue>,
    #[cfg(feature = "editor-facts")]
    data_type: Option<EventModelingFieldSpan>,
    text: String,
    #[cfg(feature = "editor-facts")]
    span: SourceSpan,
}

fn parse_eventmodeling_block_cursor(
    cursor: &mut EventModelingCursor<'_>,
) -> std::result::Result<ParsedEventModelingBlock, FailedEventModelingBlock> {
    let _data_type = match parse_eventmodeling_optional_data_type(cursor) {
        Ok(data_type) => data_type,
        Err(error) => {
            return Err(FailedEventModelingBlock {
                error: Box::new(error),
                #[cfg(feature = "editor-facts")]
                data_type: None,
                text: String::new(),
                #[cfg(feature = "editor-facts")]
                span: cursor.insertion_span(),
            });
        }
    };
    if let Err(error) = cursor.skip_hidden() {
        return Err(FailedEventModelingBlock {
            error: Box::new(error),
            #[cfg(feature = "editor-facts")]
            data_type: _data_type,
            text: String::new(),
            #[cfg(feature = "editor-facts")]
            span: cursor.insertion_span(),
        });
    }
    if !cursor.starts_literal("{") {
        let span = cursor.insertion_span();
        return Err(FailedEventModelingBlock {
            error: Box::new(EventModelingSourceIssue::insertion_point(
                "expected eventmodeling data block",
                span.start,
            )),
            #[cfg(feature = "editor-facts")]
            data_type: _data_type,
            text: String::new(),
            #[cfg(feature = "editor-facts")]
            span,
        });
    }

    let block_start = cursor.offset;
    cursor.consume_literal("{");
    let after_open = &cursor.source[cursor.offset..];
    let Some(newline_rel) = after_open.find('\n') else {
        let span = SourceSpan::new(block_start, cursor.source.len());
        return Err(FailedEventModelingBlock {
            error: Box::new(EventModelingSourceIssue::exact(
                "eventmodeling data block must start on a new line",
                span,
            )),
            #[cfg(feature = "editor-facts")]
            data_type: _data_type,
            text: cursor.source[block_start..].to_string(),
            #[cfg(feature = "editor-facts")]
            span,
        });
    };
    let before_newline = after_open[..newline_rel]
        .strip_suffix('\r')
        .unwrap_or(&after_open[..newline_rel]);
    if !before_newline
        .bytes()
        .all(|byte| matches!(byte, b' ' | b'\t'))
    {
        let leading = before_newline
            .bytes()
            .take_while(|byte| matches!(byte, b' ' | b'\t'))
            .count();
        let span = SourceSpan::new(
            cursor.offset + leading,
            cursor.offset + before_newline.len(),
        );
        return Err(FailedEventModelingBlock {
            error: Box::new(EventModelingSourceIssue::exact(
                "eventmodeling data block must start on a new line",
                span,
            )),
            #[cfg(feature = "editor-facts")]
            data_type: _data_type,
            text: cursor.source[block_start..cursor.offset + before_newline.len()].to_string(),
            #[cfg(feature = "editor-facts")]
            span,
        });
    }
    cursor.offset += newline_rel + 1;

    let mut line_start = cursor.offset;
    while line_start < cursor.source.len() {
        if cursor.control.checkpoint().is_err() {
            break;
        }
        if cursor.source[line_start..].starts_with('}') {
            let block_end = line_start + 1;
            let after = &cursor.source[block_end..];
            if after.is_empty() || after.chars().next().is_some_and(char::is_whitespace) {
                cursor.offset = block_end;
                return Ok(ParsedEventModelingBlock {
                    #[cfg(feature = "editor-facts")]
                    data_type: _data_type,
                    text: cursor.source[block_start..block_end].to_string(),
                    #[cfg(feature = "editor-facts")]
                    span: SourceSpan::new(block_start, block_end),
                });
            }
        }
        let (_, next) = physical_line_at(cursor.source, line_start);
        if next <= line_start || next == cursor.source.len() {
            break;
        }
        line_start = next;
    }

    #[cfg(feature = "editor-facts")]
    let span = SourceSpan::new(block_start, cursor.source.len());
    cursor.offset = cursor.source.len();
    Err(FailedEventModelingBlock {
        error: Box::new(EventModelingSourceIssue::insertion_point(
            "expected closing brace for eventmodeling data block",
            cursor.source.len(),
        )),
        #[cfg(feature = "editor-facts")]
        data_type: _data_type,
        text: cursor.source[block_start..].to_string(),
        #[cfg(feature = "editor-facts")]
        span,
    })
}

struct FailedEventModelingDataCursor {
    error: Box<EventModelingSourceIssue>,
    partial: Box<EventModelingDataEntityFacts>,
}

fn parse_eventmodeling_data_cursor(
    cursor: &mut EventModelingCursor<'_>,
) -> std::result::Result<EventModelingDataEntityFacts, FailedEventModelingDataCursor> {
    cursor.consume_keyword("data");
    let name = match take_eventmodeling_id_cursor(cursor, "expected eventmodeling data entity name")
    {
        Ok(name) => name,
        Err(error) => {
            return Err(FailedEventModelingDataCursor {
                error: Box::new(error),
                partial: Box::new(EventModelingDataEntityFacts {
                    name: EventModelingFieldSpan {
                        text: String::new(),
                        span: cursor.insertion_span(),
                    },
                    #[cfg(feature = "editor-facts")]
                    data_type: None,
                    block_text: String::new(),
                    #[cfg(feature = "editor-facts")]
                    block_span: cursor.insertion_span(),
                }),
            });
        }
    };
    match parse_eventmodeling_block_cursor(cursor) {
        Ok(block) => Ok(EventModelingDataEntityFacts {
            name,
            #[cfg(feature = "editor-facts")]
            data_type: block.data_type,
            block_text: block.text,
            #[cfg(feature = "editor-facts")]
            block_span: block.span,
        }),
        Err(block) => Err(FailedEventModelingDataCursor {
            error: block.error,
            partial: Box::new(EventModelingDataEntityFacts {
                name,
                #[cfg(feature = "editor-facts")]
                data_type: block.data_type,
                block_text: block.text,
                #[cfg(feature = "editor-facts")]
                block_span: block.span,
            }),
        }),
    }
}

struct FailedEventModelingNoteCursor {
    error: Box<EventModelingSourceIssue>,
    partial: Box<EventModelingNoteFacts>,
}

fn parse_eventmodeling_note_cursor(
    cursor: &mut EventModelingCursor<'_>,
) -> std::result::Result<EventModelingNoteFacts, FailedEventModelingNoteCursor> {
    cursor.consume_keyword("note");
    let _source_frame = match take_eventmodeling_frame_id_cursor(
        cursor,
        "expected eventmodeling note source frame",
    ) {
        Ok(source) => source,
        Err(error) => {
            return Err(FailedEventModelingNoteCursor {
                error: Box::new(error),
                partial: Box::new(EventModelingNoteFacts {
                    #[cfg(feature = "editor-facts")]
                    source_frame: EventModelingFieldSpan {
                        text: String::new(),
                        span: cursor.insertion_span(),
                    },
                    #[cfg(feature = "editor-facts")]
                    data_type: None,
                    #[cfg(feature = "editor-facts")]
                    block_text: String::new(),
                    #[cfg(feature = "editor-facts")]
                    block_span: cursor.insertion_span(),
                }),
            });
        }
    };
    match parse_eventmodeling_block_cursor(cursor) {
        Ok(_block) => Ok(EventModelingNoteFacts {
            #[cfg(feature = "editor-facts")]
            source_frame: _source_frame,
            #[cfg(feature = "editor-facts")]
            data_type: _block.data_type,
            #[cfg(feature = "editor-facts")]
            block_text: _block.text,
            #[cfg(feature = "editor-facts")]
            block_span: _block.span,
        }),
        Err(block) => Err(FailedEventModelingNoteCursor {
            error: block.error,
            partial: Box::new(EventModelingNoteFacts {
                #[cfg(feature = "editor-facts")]
                source_frame: _source_frame,
                #[cfg(feature = "editor-facts")]
                data_type: block.data_type,
                #[cfg(feature = "editor-facts")]
                block_text: block.text,
                #[cfg(feature = "editor-facts")]
                block_span: block.span,
            }),
        }),
    }
}

fn is_eventmodeling_top_level_start(cursor: &EventModelingCursor<'_>) -> bool {
    cursor.metadata_token().is_some()
        || [
            "entity",
            "tf",
            "timeframe",
            "rf",
            "resetframe",
            "data",
            "note",
            "gwt",
        ]
        .into_iter()
        .any(|keyword| cursor.starts_keyword(keyword))
}

fn parse_eventmodeling_gwt_group(
    cursor: &mut EventModelingCursor<'_>,
    stop_keywords: &[&str],
) -> SyntaxResult<Vec<EventModelingGwtStatementFacts>> {
    let mut statements = Vec::new();
    loop {
        if cursor.control.checkpoint().is_err() {
            break;
        }
        cursor.skip_hidden()?;
        if cursor.is_eof()
            || stop_keywords
                .iter()
                .any(|keyword| cursor.starts_keyword(keyword))
            || (!statements.is_empty() && is_eventmodeling_top_level_start(cursor))
        {
            break;
        }
        let _model_entity_type = take_eventmodeling_entity_type_cursor(cursor)?;
        let _entity_reference = take_eventmodeling_id_cursor(
            cursor,
            "expected eventmodeling gwt model entity reference",
        )?;
        statements.push(EventModelingGwtStatementFacts {
            #[cfg(feature = "editor-facts")]
            model_entity_type: _model_entity_type,
            #[cfg(feature = "editor-facts")]
            entity_reference: _entity_reference,
        });
    }
    if statements.is_empty() {
        return Err(EventModelingSourceIssue::insertion_point(
            "expected at least one eventmodeling gwt statement",
            cursor.offset(),
        ));
    }
    Ok(statements)
}

fn parse_eventmodeling_gwt_cursor(
    cursor: &mut EventModelingCursor<'_>,
) -> SyntaxResult<EventModelingGwtFacts> {
    cursor.consume_keyword("gwt");
    let _source_frame =
        take_eventmodeling_frame_id_cursor(cursor, "expected eventmodeling gwt source frame")?;
    cursor.skip_hidden()?;
    if !cursor.consume_keyword("given") {
        return Err(EventModelingSourceIssue::insertion_point(
            "expected 'given' in eventmodeling gwt",
            cursor.offset(),
        ));
    }
    let _given = parse_eventmodeling_gwt_group(cursor, &["when", "then"])?;
    cursor.skip_hidden()?;
    let _when = if cursor.consume_keyword("when") {
        parse_eventmodeling_gwt_group(cursor, &["then"])?
    } else {
        Vec::new()
    };
    cursor.skip_hidden()?;
    if !cursor.consume_keyword("then") {
        return Err(EventModelingSourceIssue::insertion_point(
            "expected 'then' in eventmodeling gwt",
            cursor.offset(),
        ));
    }
    let _then = parse_eventmodeling_gwt_group(cursor, &[])?;
    Ok(EventModelingGwtFacts {
        #[cfg(feature = "editor-facts")]
        source_frame: _source_frame,
        #[cfg(feature = "editor-facts")]
        given: _given,
        #[cfg(feature = "editor-facts")]
        when: _when,
        #[cfg(feature = "editor-facts")]
        then: _then,
    })
}

fn validate_eventmodeling_semantics(
    syntax: &mut EventModelingSyntaxFacts,
    #[cfg(feature = "editor-facts")] collect_validation_diagnostics: bool,
    control: &OperationControl,
) -> OperationControlResult<Option<EventModelingSourceIssue>> {
    let mut frame_types = HashMap::with_capacity(syntax.frames.len());
    let mut duplicate_frame = None;
    for frame in &syntax.frames {
        control.checkpoint()?;
        match frame_types.entry(frame.name.clone()) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(frame.model_entity_type.clone());
            }
            std::collections::hash_map::Entry::Occupied(_) => {
                duplicate_frame.get_or_insert_with(|| {
                    EventModelingSourceIssue::exact(
                        format!(r#"Duplicate event modeling frame ID "{}""#, frame.name),
                        frame.name_span,
                    )
                });
            }
        }
    }
    #[cfg(feature = "editor-facts")]
    if collect_validation_diagnostics {
        let mut data_names = HashSet::with_capacity(syntax.data_entities.len());
        for data in &syntax.data_entities {
            control.checkpoint()?;
            data_names.insert(data.name.text.as_str());
        }
        let mut model_entity_names = HashSet::with_capacity(syntax.model_entities.len());
        for entity in &syntax.model_entities {
            control.checkpoint()?;
            model_entity_names.insert(entity.text.as_str());
        }
        let mut diagnostics = Vec::new();

        for frame in &syntax.frames {
            control.checkpoint()?;
            for source in &frame.source_frames {
                control.checkpoint()?;
                let Some(source_type) = frame_types.get(&source.text) else {
                    diagnostics.push(EventModelingValidationDiagnostic {
                        message: format!("unknown eventmodeling frame reference '{}'", source.text),
                        span: source.span,
                    });
                    continue;
                };
                if let Some((target_label, expected_label, allowed)) =
                    eventmodeling_allowed_source_types(&frame.model_entity_type)
                    && !allowed.contains(&source_type.as_str())
                {
                    diagnostics.push(EventModelingValidationDiagnostic {
                    message: format!(
                        "A {target_label} can only receive input from a {expected_label}, not from '{source_type}'."
                    ),
                    span: source.span,
                });
                }
            }
            if let Some(reference) = &frame.data_reference
                && !data_names.contains(reference.text.as_str())
            {
                diagnostics.push(EventModelingValidationDiagnostic {
                    message: format!("unknown eventmodeling data reference '{}'", reference.text),
                    span: reference.span,
                });
            }
        }
        for note in &syntax.note_entities {
            control.checkpoint()?;
            if !frame_types.contains_key(&note.source_frame.text) {
                diagnostics.push(EventModelingValidationDiagnostic {
                    message: format!(
                        "unknown eventmodeling frame reference '{}'",
                        note.source_frame.text
                    ),
                    span: note.source_frame.span,
                });
            }
        }
        for gwt in &syntax.gwt_entities {
            control.checkpoint()?;
            if !frame_types.contains_key(&gwt.source_frame.text) {
                diagnostics.push(EventModelingValidationDiagnostic {
                    message: format!(
                        "unknown eventmodeling frame reference '{}'",
                        gwt.source_frame.text
                    ),
                    span: gwt.source_frame.span,
                });
            }
            for statement in gwt.given.iter().chain(&gwt.when).chain(&gwt.then) {
                control.checkpoint()?;
                if !model_entity_names.contains(statement.entity_reference.text.as_str()) {
                    diagnostics.push(EventModelingValidationDiagnostic {
                        message: format!(
                            "unknown eventmodeling model entity reference '{}'",
                            statement.entity_reference.text
                        ),
                        span: statement.entity_reference.span,
                    });
                }
            }
        }
        syntax.validation_diagnostics = diagnostics;
    }
    Ok(duplicate_frame)
}

#[cfg(feature = "editor-facts")]
fn eventmodeling_allowed_source_types(
    target: &str,
) -> Option<(&'static str, &'static str, &'static [&'static str])> {
    match target {
        "cmd" | "command" => Some(("command", "ui or processor", &["ui", "pcr", "processor"])),
        "evt" | "event" => Some(("event", "command", &["cmd", "command"])),
        "rmo" | "readmodel" => Some(("read model", "event", &["evt", "event"])),
        "pcr" | "processor" => Some(("processor", "read model", &["rmo", "readmodel"])),
        "ui" => Some(("ui", "read model", &["rmo", "readmodel"])),
        _ => None,
    }
}

#[cfg(feature = "editor-facts")]
impl EventModelingSyntaxFacts {
    fn observe_controlled(
        &self,
        observer: &mut EventModelingObserver<'_>,
        control: &OperationControl,
    ) -> OperationControlResult<()> {
        use EventModelingFieldKind as Field;

        control.checkpoint()?;
        (observer.emit)(EventModelingObservation::CollectionStarted);
        if matches!(observer.demand, EventModelingEditorDemand::Capture) {
            if let Some(header) = &self.header {
                observer.field(Field::Header, header);
            }
            for common in self.common.iter() {
                control.checkpoint()?;
                (observer.emit)(EventModelingObservation::Common(common));
            }
            for entity in &self.model_entities {
                control.checkpoint()?;
                observer.field(Field::ModelEntity, entity);
            }
            for frame in &self.frames {
                control.checkpoint()?;
                frame.observe_controlled(observer, control)?;
            }
            for data in &self.data_entities {
                control.checkpoint()?;
                if !data.name.text.is_empty() {
                    observer.field(Field::DataEntity, &data.name);
                }
                if let Some(data_type) = &data.data_type {
                    observer.field(Field::DataType, data_type);
                }
                observer.field_at(Field::DataBlock, &data.block_text, data.block_span);
                control.checkpoint()?;
            }
            for note in &self.note_entities {
                control.checkpoint()?;
                if !note.source_frame.text.is_empty() {
                    observer.field(Field::NoteSourceFrame, &note.source_frame);
                }
                if let Some(data_type) = &note.data_type {
                    observer.field(Field::DataType, data_type);
                }
                observer.field_at(Field::NoteBlock, &note.block_text, note.block_span);
            }
            for gwt in &self.gwt_entities {
                control.checkpoint()?;
                observer.field(Field::GwtSourceFrame, &gwt.source_frame);
                for statement in gwt.given.iter().chain(&gwt.when).chain(&gwt.then) {
                    control.checkpoint()?;
                    observer.field(Field::EntityType, &statement.model_entity_type);
                    observer.field(Field::GwtEntityReference, &statement.entity_reference);
                }
            }
        }
        for diagnostic in &self.validation_diagnostics {
            control.checkpoint()?;
            (observer.emit)(EventModelingObservation::ValidationIssue {
                message: &diagnostic.message,
                span: diagnostic.span,
            });
        }
        control.checkpoint()?;
        Ok(())
    }
}

#[cfg(feature = "editor-facts")]
impl EventModelingFrameFacts {
    fn observe_controlled(
        &self,
        observer: &mut EventModelingObserver<'_>,
        control: &OperationControl,
    ) -> OperationControlResult<()> {
        use EventModelingFieldKind as Field;

        observer.field_at(Field::Frame, &self.name, self.name_span);
        observer.field_at(
            Field::EntityType,
            &self.model_entity_type,
            self.model_entity_type_span,
        );
        observer.field_at(
            Field::EntityIdentifier,
            &self.entity_identifier,
            self.entity_identifier_span,
        );
        for source in &self.source_frames {
            control.checkpoint()?;
            observer.field(Field::SourceFrame, source);
        }
        if let Some(reference) = &self.data_reference {
            control.checkpoint()?;
            observer.field(Field::DataReference, reference);
        }
        for (field, kind) in [
            (&self.data_type, Field::DataType),
            (&self.data_inline_value, Field::InlineData),
        ] {
            control.checkpoint()?;
            if let Some(field) = field {
                observer.field(kind, field);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unclosed_metadata_lookahead_scans_each_failed_suffix_once() {
        for count in [64, 1024] {
            for newline in ["\n", "\r\n"] {
                for payload in ["", "🤓"] {
                    let source = format!(
                        "eventmodeling{newline}{}entity Later{newline}",
                        format!("tf 01 evt A accDescr{{{payload}{newline}").repeat(count)
                    );
                    let control = OperationControl::new();
                    let mut cursor = EventModelingCursor::new(&source, &control);
                    let marker = " accDescr{";
                    let mut end = 0;
                    let mut attempts = 0;
                    while let Some(relative) = source[end..].find(marker) {
                        cursor.offset = end + relative;
                        assert!(cursor.metadata_token().is_none());
                        assert!(cursor.metadata_token().is_none());
                        end = cursor.offset + marker.len();
                        attempts += 1;
                    }
                    assert_eq!(attempts, count);
                    assert!(cursor.metadata_block_scanned_bytes.get() <= source.len());
                }
            }
        }
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn late_syntax_error_recovery_can_cancel_while_projecting_prefix_facts() {
        let header = "eventmodeling";
        let mut source = format!("{header}\n");
        let mut syntax = EventModelingSyntaxFacts {
            header: Some(EventModelingFieldSpan {
                text: header.to_string(),
                span: SourceSpan::new(0, header.len()),
            }),
            ..EventModelingSyntaxFacts::default()
        };
        for index in 0..512 {
            let name = format!("Entity{index}");
            let name_start = source.len() + "entity ".len();
            source.push_str("entity ");
            source.push_str(&name);
            source.push('\n');
            syntax.model_entities.push(EventModelingFieldSpan {
                text: name,
                span: SourceSpan::new(name_start, source.len() - 1),
            });
        }
        let invalid_start = source.len();
        source.push_str("not-a-statement");
        let invalid_span = SourceSpan::new(invalid_start, source.len());
        let issue =
            EventModelingSourceIssue::exact("late eventmodeling syntax error", invalid_span);
        let control = OperationControl::new();
        control.cancel_after_checkpoints(20);

        let mut observer = EventModelingObserver {
            demand: EventModelingEditorDemand::Capture,
            emit: &mut |_| {},
        };
        assert!(matches!(
            finish_eventmodeling_syntax(syntax, Some(issue), Some(&mut observer), &control),
            Err(merman_model::OperationCancelled { .. })
        ));
    }
}
