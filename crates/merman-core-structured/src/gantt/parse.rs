#[cfg(feature = "editor-facts")]
use super::facts::{
    GanttObserver, collect_gantt_click_symbols, collect_gantt_section_symbol,
    collect_gantt_task_field_symbols, push_gantt_payload_symbol,
};
use super::*;
use merman_core_preprocess::scan::{
    LineCursor, physical_line_at, split_statement_suffix_hash_or_semi, starts_with_case_insensitive,
};
use merman_model::{
    MermaidConfig, OperationControl, OperationControlResult, text::is_ecmascript_whitespace,
};

#[cfg(any(test, feature = "test-support"))]
use std::cell::Cell;

#[cfg(any(test, feature = "test-support"))]
thread_local! {
    static GANTT_SYNTAX_CONSTRUCTION_COUNT: Cell<usize> = const { Cell::new(0) };
}

#[cfg(any(test, feature = "test-support"))]
pub fn reset_gantt_syntax_construction_count() {
    GANTT_SYNTAX_CONSTRUCTION_COUNT.set(0);
}

#[cfg(any(test, feature = "test-support"))]
pub fn gantt_syntax_construction_count() -> usize {
    GANTT_SYNTAX_CONSTRUCTION_COUNT.get()
}

fn strip_inline_comment(line: &str) -> &str {
    let t = line.trim_start_matches(is_ecmascript_whitespace);
    if t.starts_with("%%{") {
        return line;
    }
    let prefixed_percent = t
        .chars()
        .next()
        .is_some_and(|ch| ch != '}' && ch.len_utf16() == 1 && t[ch.len_utf8()..].starts_with('%'));
    if t.starts_with('%') || prefixed_percent {
        return "";
    }
    line
}

fn leading_whitespace_len(text: &str) -> usize {
    text.len() - text.trim_start_matches(is_ecmascript_whitespace).len()
}

fn parse_key_colon_value_spanned<'a>(
    line: &str,
    line_start: usize,
    key: &str,
    cursor: &mut LineCursor<'a>,
) -> Option<SpannedText<'a>> {
    let leading = leading_whitespace_len(line);
    let trimmed = &cursor.source()[line_start + leading..];
    if !starts_with_case_insensitive(trimmed, key) {
        return None;
    }
    let after_key_start = key.len();
    let after_key = &trimmed[after_key_start..];
    let after_key_ws = leading_whitespace_len(after_key);
    let colon_start = after_key_start + after_key_ws;
    let rest_start = colon_start + ':'.len_utf8();
    if !trimmed[colon_start..].starts_with(':') {
        return None;
    }
    let rest = &trimmed[rest_start..];
    let rest_ws = leading_whitespace_len(rest);
    let value_start = rest_start + rest_ws;
    let start = line_start + leading + value_start;
    let (text, _) = physical_line_at(cursor.source(), start);
    let end = start + text.len();
    cursor.resume_at(end);
    Some(SpannedText { text, start, end })
}

fn parse_click_statement<'a>(
    line: &str,
    line_start: usize,
    cursor: &mut LineCursor<'a>,
    control: &OperationControl,
) -> OperationControlResult<std::result::Result<Option<ClickStatementParts<'a>>, ClickStatementError>>
{
    let leading = leading_whitespace_len(line);
    let trimmed = &cursor.source()[line_start + leading..];
    if !starts_with_click_keyword(trimmed, "click") {
        return Ok(Ok(None));
    }
    let after_click = &trimmed["click".len()..];
    let rest_leading = leading_whitespace_len(after_click);
    let rest_start = "click".len() + rest_leading;
    let rest = &trimmed[rest_start..];
    let ids_len = rest
        .char_indices()
        .find_map(|(idx, ch)| is_ecmascript_whitespace(ch).then_some(idx))
        .unwrap_or(rest.len());
    let ids = SpannedText {
        text: &rest[..ids_len],
        start: line_start + leading + rest_start,
        end: line_start + leading + rest_start + ids_len,
    };
    if ids.text.is_empty() {
        return Ok(Err(ClickStatementError::new(
            "invalid click statement: missing task id",
        )));
    }

    let mut tail_offset = rest_start + ids_len;
    #[cfg(feature = "editor-facts")]
    let action_separator_present = tail_offset < trimmed.len();
    if let Some(separator) = trimmed[tail_offset..].chars().next() {
        tail_offset += separator.len_utf8();
    }
    let mut href = None;
    #[cfg(feature = "editor-facts")]
    let mut href_keyword = None;
    let mut call = None;
    #[cfg(feature = "editor-facts")]
    let mut call_keyword = None;

    loop {
        let whitespace_start = tail_offset;
        if trimmed[tail_offset..].starts_with('\n') {
            break;
        }
        tail_offset += leading_whitespace_len(&trimmed[tail_offset..]);
        let tail = &trimmed[tail_offset..];
        if !tail.is_empty()
            && (href.is_some() || call.is_some())
            && strip_inline_comment(tail).is_empty()
        {
            tail_offset = whitespace_start;
            break;
        }
        if tail.is_empty()
            || (trimmed[whitespace_start..tail_offset].contains('\n')
                && (href.is_some() || call.is_some())
                && !starts_with_click_keyword(tail, "href")
                && !starts_with_click_keyword(tail, "call"))
        {
            let (line_tail, _) = physical_line_at(trimmed, whitespace_start);
            tail_offset = whitespace_start + line_tail.len();
            break;
        }
        if starts_with_click_keyword(tail, "href") {
            if href.is_some() {
                return Ok(Err(ClickStatementError::new(
                    "invalid click statement: duplicate href",
                )));
            }
            let href_keyword_end = tail_offset + "href".len();
            #[cfg(feature = "editor-facts")]
            {
                href_keyword = Some(SourceSpan::new(
                    line_start + leading + tail_offset,
                    line_start + leading + href_keyword_end,
                ));
            }
            let after_href = &trimmed[href_keyword_end..];
            let href_ws = leading_whitespace_len(after_href);
            let quote_start = href_keyword_end + href_ws;
            let value_start = quote_start + '"'.len_utf8();
            if !trimmed[quote_start..].starts_with('"') {
                return Ok(Err(ClickStatementError::new(
                    "invalid click statement: href requires a quoted URL",
                )));
            }
            let Some(end) =
                consume_click_delimiter(cursor, line_start + leading + value_start, '"', control)?
            else {
                return Ok(Err(ClickStatementError::new(
                    "invalid click statement: unterminated href URL",
                )));
            };
            let value_end = end - line_start - leading;
            href = Some(SpannedText {
                text: &trimmed[value_start..value_end],
                start: line_start + leading + value_start,
                end: line_start + leading + value_end,
            });
            tail_offset = value_end + '"'.len_utf8();
            continue;
        }

        if starts_with_click_keyword(tail, "call") {
            if call.is_some() {
                return Ok(Err(ClickStatementError::new(
                    "invalid click statement: duplicate callback",
                )));
            }
            let call_keyword_end = tail_offset + "call".len();
            #[cfg(feature = "editor-facts")]
            {
                call_keyword = Some(SourceSpan::new(
                    line_start + leading + tail_offset,
                    line_start + leading + call_keyword_end,
                ));
            }
            let after_call = &trimmed[call_keyword_end..];
            let call_ws = leading_whitespace_len(after_call);
            let name_start = call_keyword_end + call_ws;
            let (parsed_call, next_offset) = match parse_callback_tail(
                trimmed,
                name_start,
                line_start + leading,
                cursor,
                control,
            )? {
                Ok(parsed) => parsed,
                Err(error) => return Ok(Err(ClickStatementError::new(error))),
            };
            call = Some(parsed_call);
            tail_offset = next_offset;
            continue;
        }

        if tail.starts_with('"') {
            let Some(end) = consume_click_delimiter(
                cursor,
                line_start + leading + tail_offset + 1,
                '"',
                control,
            )?
            else {
                return Ok(Err(ClickStatementError::new(
                    "invalid click statement: unterminated quoted tail",
                )));
            };
            tail_offset = end + 1 - line_start - leading;
            continue;
        }

        let (tail_line, _) = physical_line_at(trimmed, tail_offset);
        if ["href", "call"]
            .iter()
            .any(|keyword| starts_with_case_insensitive(keyword, tail_line))
        {
            return Ok(Err(ClickStatementError::with_expected_action(
                "invalid click statement: incomplete action",
                #[cfg(feature = "editor-facts")]
                SourceSpan::new(
                    line_start + leading + tail_offset,
                    line_start + leading + tail_offset + tail_line.len(),
                ),
            )));
        }

        if call.is_none() {
            let (parsed_call, next_offset) = match parse_callback_tail(
                trimmed,
                tail_offset,
                line_start + leading,
                cursor,
                control,
            )? {
                Ok(parsed) => parsed,
                Err(error) => return Ok(Err(ClickStatementError::new(error))),
            };
            call = Some(parsed_call);
            tail_offset = next_offset;
            continue;
        }

        return Ok(Err(ClickStatementError::new(format!(
            "invalid click statement tail: {tail_line:?}"
        ))));
    }

    if href.is_none() && call.is_none() {
        #[cfg(feature = "editor-facts")]
        let expected_action = action_separator_present.then(|| {
            let end = line_start + leading + tail_offset;
            SourceSpan::new(end, end)
        });
        return Ok(Err(ClickStatementError {
            message: "invalid click statement: missing href or callback".to_string(),
            #[cfg(feature = "editor-facts")]
            expected_action,
        }));
    }

    Ok(Ok(Some(ClickStatementParts {
        end: line_start + leading + tail_offset,
        ids,
        href,
        #[cfg(feature = "editor-facts")]
        href_keyword,
        call,
        #[cfg(feature = "editor-facts")]
        call_keyword,
    })))
}

#[derive(Debug)]
struct ClickStatementError {
    message: String,
    #[cfg(feature = "editor-facts")]
    expected_action: Option<SourceSpan>,
}

impl ClickStatementError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            #[cfg(feature = "editor-facts")]
            expected_action: None,
        }
    }

    fn with_expected_action(
        message: impl Into<String>,
        #[cfg(feature = "editor-facts")] span: SourceSpan,
    ) -> Self {
        Self {
            message: message.into(),
            #[cfg(feature = "editor-facts")]
            expected_action: Some(span),
        }
    }
}

fn consume_click_delimiter(
    cursor: &mut LineCursor<'_>,
    start: usize,
    delimiter: char,
    control: &OperationControl,
) -> OperationControlResult<Option<usize>> {
    let source = cursor.source();
    for (count, (offset, ch)) in source[start..].char_indices().enumerate() {
        if count.is_multiple_of(128) {
            control.checkpoint()?;
        }
        if ch == delimiter {
            let end = start + offset;
            cursor.resume_at(end + ch.len_utf8());
            return Ok(Some(end));
        }
    }
    control.checkpoint()?;
    cursor.resume_at(source.len());
    Ok(None)
}

fn starts_with_click_keyword(input: &str, keyword: &str) -> bool {
    if !starts_with_case_insensitive(input, keyword) {
        return false;
    }
    input[keyword.len()..]
        .chars()
        .next()
        .is_some_and(is_ecmascript_whitespace)
}

fn parse_callback_tail<'a>(
    trimmed: &'a str,
    name_start: usize,
    absolute_offset: usize,
    cursor: &mut LineCursor<'a>,
    control: &OperationControl,
) -> OperationControlResult<std::result::Result<(ClickCallParts<'a>, usize), String>> {
    let name_tail = &trimmed[name_start..];
    let Some(name_len) = callback_name_len(name_tail) else {
        return Ok(Err(
            "invalid click statement: missing callback name".to_string()
        ));
    };
    let name_end = name_start + name_len;
    let mut next_offset = name_end;
    let args = if trimmed[next_offset..].starts_with('(') {
        let args_start = next_offset + '('.len_utf8();
        let Some(end) =
            consume_click_delimiter(cursor, absolute_offset + args_start, ')', control)?
        else {
            return Ok(Err(
                "invalid click statement: unterminated callback args".to_string()
            ));
        };
        let args_end = end - absolute_offset;
        next_offset = args_end + ')'.len_utf8();
        let args_text = &trimmed[args_start..args_end];
        if args_text.trim_matches(is_ecmascript_whitespace).is_empty() {
            None
        } else {
            Some(SpannedText {
                text: args_text,
                start: absolute_offset + args_start,
                end: absolute_offset + args_end,
            })
        }
    } else {
        None
    };
    Ok(Ok((
        ClickCallParts {
            name: SpannedText {
                text: &trimmed[name_start..name_end],
                start: absolute_offset + name_start,
                end: absolute_offset + name_end,
            },
            args,
        },
        next_offset,
    )))
}

fn callback_name_len(input: &str) -> Option<usize> {
    let mut chars = input.char_indices();
    let (_, first) = chars.next()?;
    if !is_callback_name_start(first) {
        return None;
    }
    for (idx, ch) in chars {
        if !is_callback_name_continue(ch) {
            return Some(idx);
        }
    }
    Some(input.len())
}

fn is_callback_name_start(ch: char) -> bool {
    ch == '_' || ch == '$' || ch.is_ascii_alphabetic()
}

fn is_callback_name_continue(ch: char) -> bool {
    is_callback_name_start(ch) || ch.is_ascii_digit() || ch == '.'
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ClickStatementParts<'a> {
    end: usize,
    pub(super) ids: SpannedText<'a>,
    pub(super) href: Option<SpannedText<'a>>,
    #[cfg(feature = "editor-facts")]
    pub(super) href_keyword: Option<SourceSpan>,
    pub(super) call: Option<ClickCallParts<'a>>,
    #[cfg(feature = "editor-facts")]
    pub(super) call_keyword: Option<SourceSpan>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ClickCallParts<'a> {
    pub(super) name: SpannedText<'a>,
    pub(super) args: Option<SpannedText<'a>>,
}

enum GanttDirectiveEnd {
    HashOrSemi,
    Semi,
    KeywordPrefix(&'static [&'static str]),
    Line,
}

fn starts_with_gantt_fixed_keyword(input: &str, keyword: &str) -> bool {
    starts_with_case_insensitive(input, keyword)
        && input
            .as_bytes()
            .get(keyword.len())
            .is_none_or(|byte| !byte.is_ascii_alphanumeric() && *byte != b'_')
}

fn parse_gantt_keyword_arg_spanned<'a>(
    line: &str,
    line_start: usize,
    keyword: &str,
    token_end: GanttDirectiveEnd,
    cursor: &mut LineCursor<'a>,
) -> Option<SpannedText<'a>> {
    let leading = leading_whitespace_len(line);
    let trimmed = &cursor.source()[line_start + leading..];
    if !starts_with_case_insensitive(trimmed, keyword) {
        return None;
    }
    let after = &trimmed[keyword.len()..];
    let ws = after.chars().next()?;
    if !is_ecmascript_whitespace(ws) {
        return None;
    }
    // The pinned Jison actions use `substr(keyword.len() + 1)`: one delimiter is consumed and
    // any additional whitespace remains part of the DB value.
    let ws_len = ws.len_utf8();
    let rest_start = keyword.len() + ws_len;
    let start = line_start + leading + rest_start;
    let rest = &cursor.source()[start..];
    let (line_rest, _) = physical_line_at(cursor.source(), start);
    let text = match token_end {
        GanttDirectiveEnd::HashOrSemi => split_statement_suffix_hash_or_semi(line_rest),
        GanttDirectiveEnd::Semi => line_rest
            .split_once(';')
            .map_or(line_rest, |(text, _)| text),
        GanttDirectiveEnd::KeywordPrefix(keywords) => {
            let leading = leading_whitespace_len(rest);
            let keyword = keywords
                .iter()
                .find(|keyword| starts_with_gantt_fixed_keyword(&rest[leading..], keyword));
            match keyword {
                Some(keyword) => &rest[..leading + keyword.len()],
                None if line_rest.contains(':') => return None,
                None => line_rest,
            }
        }
        GanttDirectiveEnd::Line => line_rest,
    };
    if text.is_empty() {
        return None;
    }
    let end = start + text.len();
    cursor.resume_at(end);
    Some(SpannedText { text, start, end })
}

#[derive(Debug)]
struct GanttAccDescrBlock<'a> {
    statement_start: usize,
    statement_end: usize,
    body: SpannedText<'a>,
    complete: bool,
}

impl<'a> GanttAccDescrBlock<'a> {
    fn start(line: &str, line_start: usize, cursor: &LineCursor<'a>) -> Option<Self> {
        let leading = leading_whitespace_len(line);
        let statement_start = line_start + leading;
        let trimmed = &cursor.source()[statement_start..];
        if !starts_with_case_insensitive(trimmed, "accDescr") {
            return None;
        }

        let after_key = &trimmed["accDescr".len()..];
        let after_key_ws = leading_whitespace_len(after_key);
        let open_offset = statement_start + "accDescr".len() + after_key_ws;
        if !cursor.source()[open_offset..].starts_with('{') {
            return None;
        }

        let body_start = open_offset + '{'.len_utf8();
        Some(Self {
            statement_start,
            statement_end: cursor.source().len(),
            body: SpannedText {
                text: &cursor.source()[body_start..],
                start: body_start,
                end: cursor.source().len(),
            },
            complete: false,
        })
    }

    fn is_complete(&self) -> bool {
        self.complete
    }

    fn consume_remaining(
        mut self,
        cursor: &mut LineCursor<'_>,
        control: &OperationControl,
    ) -> OperationControlResult<Self> {
        let mut consumed = 0;
        for segment in self.body.text.split_inclusive('\n') {
            control.checkpoint()?;
            if let Some(close) = segment.find('}') {
                let body_len = consumed + close;
                self.body.text = &self.body.text[..body_len];
                self.body.end = self.body.start + body_len;
                self.statement_end = self.body.end + '}'.len_utf8();
                self.complete = true;
                break;
            }
            consumed += segment.len();
        }
        if !self.complete {
            self.statement_end = cursor.source().trim_end_matches(['\r', '\n']).len();
        }
        cursor.resume_at(if self.complete {
            self.statement_end
        } else {
            cursor.source().len()
        });
        Ok(self)
    }

    fn value(&self) -> &str {
        self.body.text.trim_matches(is_ecmascript_whitespace)
    }

    fn statement_span(&self) -> SourceSpan {
        SourceSpan::new(self.statement_start, self.statement_end)
    }

    #[cfg(feature = "editor-facts")]
    fn emit_symbol(&self, facts: &mut GanttObserver<'_>) {
        let Some(body) = self.body.trim() else {
            return;
        };
        facts(GanttObservation::Symbol {
            kind: GanttSymbolKind::AccDescr,
            text: body.text,
            statement: self.statement_span(),
            selection: body.span(),
        });
        facts(GanttObservation::ExpectedSyntax {
            kind: GanttExpectedSyntax::Payload,
            span: body.span(),
        });
    }
}

pub(super) fn gantt_statement_span(line: &str, line_start: usize) -> SourceSpan {
    let trimmed_line = line.trim_start_matches(is_ecmascript_whitespace);
    let leading = line.len().saturating_sub(trimmed_line.len());
    SourceSpan::new(
        line_start + leading,
        line_start + leading + trimmed_line.len(),
    )
}

fn split_gantt_fields(text: &str, text_start: usize) -> Vec<SpannedText<'_>> {
    let mut out = Vec::new();
    let mut field_start = 0usize;
    for (idx, ch) in text.char_indices() {
        if ch == ',' {
            out.push(SpannedText {
                text: &text[field_start..idx],
                start: text_start + field_start,
                end: text_start + idx,
            });
            field_start = idx + ch.len_utf8();
        }
    }

    out.push(SpannedText {
        text: &text[field_start..],
        start: text_start + field_start,
        end: text_start + text.len(),
    });
    out
}

#[derive(Debug, Clone, Copy)]
pub(super) struct SpannedText<'a> {
    pub(super) text: &'a str,
    pub(super) start: usize,
    pub(super) end: usize,
}

impl<'a> SpannedText<'a> {
    pub(super) fn trim(self) -> Option<Self> {
        let leading = leading_whitespace_len(self.text);
        let text = &self.text[leading..];
        let trimmed_len = text.trim_end_matches(is_ecmascript_whitespace).len();
        if trimmed_len == 0 {
            return None;
        }

        Some(Self {
            text: &text[..trimmed_len],
            start: self.start + leading,
            end: self.start + leading + trimmed_len,
        })
    }

    pub(super) fn span(self) -> SourceSpan {
        SourceSpan::new(self.start, self.end)
    }
}

/// A scanned Gantt database awaiting date finalization.
pub struct GanttSyntax {
    db: GanttDb,
}

/// A Gantt database whose tasks have been finalized with the captured runtime.
pub struct GanttSource {
    db: GanttDb,
    runtime: OperationContext,
}

/// Scans the source without collecting editor facts.
/// Empty input is distinct from a header-only Gantt diagram.
pub fn parse_gantt_syntax_controlled(
    code: &str,
    config: &MermaidConfig,
    capture_display_mode: bool,
    control: &OperationControl,
) -> OperationControlResult<Result<Option<GanttSyntax>>> {
    scan_gantt_syntax(
        code,
        config,
        #[cfg(feature = "editor-facts")]
        None,
        capture_display_mode,
        control,
    )
}

/// Scans the same source path and reports borrowed facts in statement order.
#[cfg(feature = "editor-facts")]
pub fn parse_gantt_syntax_observed_controlled(
    code: &str,
    config: &MermaidConfig,
    observe: &mut dyn for<'source> FnMut(GanttObservation<'source>),
    capture_display_mode: bool,
    control: &OperationControl,
) -> OperationControlResult<Result<Option<GanttSyntax>>> {
    scan_gantt_syntax(code, config, Some(observe), capture_display_mode, control)
}

fn scan_gantt_syntax(
    code: &str,
    config: &MermaidConfig,
    #[cfg(feature = "editor-facts")] mut observe: Option<&mut GanttObserver<'_>>,
    capture_display_mode: bool,
    control: &OperationControl,
) -> OperationControlResult<Result<Option<GanttSyntax>>> {
    control.checkpoint()?;
    #[cfg(any(test, feature = "test-support"))]
    GANTT_SYNTAX_CONSTRUCTION_COUNT.set(GANTT_SYNTAX_CONSTRUCTION_COUNT.get() + 1);

    control.checkpoint()?;
    let mut db = GanttDb::default();
    db.clear();
    db.set_security_level(config.get_str("securityLevel"));
    #[cfg(feature = "presentation")]
    {
        if capture_display_mode && let Some(dm) = config.get_str("gantt.displayMode") {
            db.set_display_mode(dm);
        }
    }
    #[cfg(not(feature = "presentation"))]
    let _ = capture_display_mode;

    let mut cursor = LineCursor::new(code);
    let mut header_seen = false;
    let mut first_error = None;

    while let Some((line, line_start)) = cursor.next_line() {
        control.checkpoint()?;
        let stripped = strip_inline_comment(line);
        let trimmed = stripped.trim_start_matches(is_ecmascript_whitespace);
        if trimmed.is_empty() {
            continue;
        }

        if !header_seen {
            if starts_with_gantt_fixed_keyword(trimmed, "gantt") {
                header_seen = true;
                let rest = trimmed["gantt".len()..].trim_start_matches(is_ecmascript_whitespace);
                if !rest.is_empty() {
                    let rest_offset = trimmed["gantt".len()..].len() - rest.len();
                    let trimmed_start = stripped.find(trimmed).unwrap_or(0);
                    parse_gantt_statement(
                        rest,
                        line_start + trimmed_start + "gantt".len() + rest_offset,
                        &mut db,
                        &mut cursor,
                        #[cfg(feature = "editor-facts")]
                        &mut observe,
                        control,
                    )?
                    .unwrap_or_else(|error| {
                        first_error.get_or_insert(error);
                    });
                }
                continue;
            }
            first_error.get_or_insert(GanttSourceIssue::exact(
                "expected gantt header".to_string(),
                gantt_statement_span(stripped, line_start),
            ));
            parse_gantt_statement(
                stripped,
                line_start,
                &mut db,
                &mut cursor,
                #[cfg(feature = "editor-facts")]
                &mut observe,
                control,
            )?
            .unwrap_or_else(|error| {
                first_error.get_or_insert(error);
            });
            continue;
        }

        parse_gantt_statement(
            stripped,
            line_start,
            &mut db,
            &mut cursor,
            #[cfg(feature = "editor-facts")]
            &mut observe,
            control,
        )?
        .unwrap_or_else(|error| {
            first_error.get_or_insert(error);
        });
    }

    if let Some(error) = first_error {
        return Ok(Err(error));
    }
    Ok(Ok(header_seen.then_some(GanttSyntax { db })))
}

impl GanttSyntax {
    pub fn into_source(
        mut self,
        runtime: OperationContext,
        control: &OperationControl,
    ) -> OperationControlResult<Result<GanttSource>> {
        Ok(self
            .db
            .finalize_tasks_controlled(&runtime, control)?
            .map(|()| GanttSource {
                db: self.db,
                runtime,
            }))
    }
}

impl GanttSource {
    pub fn into_semantic_model(
        self,
        control: &OperationControl,
    ) -> OperationControlResult<Result<GanttDiagramModel>> {
        gantt_db_to_model_controlled(self.db, raw_task_to_semantic_task, &self.runtime, control)
    }

    #[cfg(feature = "presentation")]
    pub fn into_render_model(
        self,
        control: &OperationControl,
    ) -> OperationControlResult<Result<GanttDiagramRenderModel>> {
        gantt_db_to_render_model_controlled(self.db, &self.runtime, control)
    }
}

#[cfg(feature = "presentation")]
pub(super) fn gantt_db_to_render_model_controlled(
    mut db: GanttDb,
    runtime: &OperationContext,
    control: &OperationControl,
) -> OperationControlResult<Result<GanttDiagramRenderModel>> {
    let display_mode = std::mem::take(&mut db.display_mode);
    Ok(
        gantt_db_to_model_controlled(db, raw_task_to_render_task, runtime, control)?.map(|model| {
            GanttDiagramRenderModel::from_output_data(model, display_mode, GanttOutputData::Model)
        }),
    )
}

fn gantt_db_to_model_controlled<Task>(
    mut db: GanttDb,
    convert_task: fn(RawTask, &str, &OperationContext) -> Result<Task>,
    runtime: &OperationContext,
    control: &OperationControl,
) -> OperationControlResult<Result<GanttDiagramModel<Task>>> {
    control.checkpoint()?;
    let raw_tasks = db.take_tasks();
    let mut tasks = Vec::with_capacity(raw_tasks.len());
    for (index, task) in raw_tasks.into_iter().enumerate() {
        if index % 128 == 0 {
            control.checkpoint()?;
        }
        match convert_task(task, &db.date_format, runtime) {
            Ok(task) => tasks.push(task),
            Err(error) => return Ok(Err(error)),
        }
    }

    control.checkpoint()?;
    Ok(Ok(GanttDiagramModel {
        title: non_empty_opt(std::mem::take(&mut db.diagram_title)),
        acc_title: non_empty_opt(std::mem::take(&mut db.acc_title)),
        acc_descr: non_empty_opt(std::mem::take(&mut db.acc_descr)),
        date_format: std::mem::take(&mut db.date_format),
        axis_format: std::mem::take(&mut db.axis_format),
        tick_interval: db.tick_interval.take(),
        today_marker: std::mem::take(&mut db.today_marker),
        includes: std::mem::take(&mut db.includes),
        excludes: std::mem::take(&mut db.excludes),
        inclusive_end_dates: db.inclusive_end_dates,
        top_axis: db.top_axis,
        weekday: std::mem::take(&mut db.weekday),
        weekend: std::mem::take(&mut db.weekend),
        sections: std::mem::take(&mut db.sections),
        tasks,
        links: std::mem::take(&mut db.links),
        click_events: std::mem::take(&mut db.click_events),
    }))
}

fn non_empty_opt(value: String) -> Option<String> {
    if value.is_empty() { None } else { Some(value) }
}

#[cfg(feature = "presentation")]
fn raw_task_to_render_task(
    t: RawTask,
    date_format: &str,
    runtime: &OperationContext,
) -> Result<GanttRenderTask> {
    let semantic = raw_task_to_semantic_task(t, date_format, runtime)?;
    Ok(GanttRenderTask {
        render_end_ms: semantic.render_end_ms,
        semantic,
    })
}

fn raw_task_to_semantic_task(
    t: RawTask,
    date_format: &str,
    runtime: &OperationContext,
) -> Result<GanttTask> {
    let start_ms = task_time_ms(&t, "startTime", t.start_time)?;
    let end_ms = task_time_ms(&t, "endTime", t.end_time)?;
    let (raw_start, start_constraint) = match t.raw.start_time {
        StartTimeRaw::PrevTaskEnd => (
            GanttTaskStart::PrevTaskEnd {
                id: t.prev_task_id.clone(),
            },
            GanttTaskStartConstraint::PreviousTaskEnd {
                dependency_id: t.prev_task_id.clone(),
            },
        ),
        StartTimeRaw::GetStartDate { start_data } => {
            let constraint = match relative_dependency_ids(&start_data, "after") {
                Some(dependency_ids) => GanttTaskStartConstraint::After { dependency_ids },
                None => GanttTaskStartConstraint::Fixed {
                    value: start_data.clone(),
                },
            };
            (GanttTaskStart::GetStartDate { start_data }, constraint)
        }
    };
    let end_constraint = gantt_end_constraint(&t.raw.end_data, date_format, runtime);

    Ok(GanttTask {
        id: t.id,
        task: t.task,
        section: t.section,
        section_index: t.section_index,
        task_type: t.type_,
        classes: t.classes,
        active: t.active,
        done: t.done,
        crit: t.crit,
        milestone: t.milestone,
        vert: t.vert,
        order: t.order,
        prev_task_id: t.prev_task_id,
        start_constraint,
        end_constraint,
        processed: t.processed,
        manual_end_time: t.manual_end_time,
        raw: GanttTaskRaw {
            data: t.raw.data,
            start_time: raw_start,
            end_time: GanttTaskEnd {
                data: t.raw.end_data,
            },
        },
        start_ms,
        end_ms,
        render_end_ms: t.render_end_time.map(|date| date.timestamp_millis()),
    })
}

fn relative_dependency_ids(value: &str, keyword: &str) -> Option<Vec<String>> {
    relative_ref_ids(value.trim_matches(is_ecmascript_whitespace), keyword).map(|ids| {
        ids.split(' ')
            .filter(|id| !id.is_empty())
            .map(str::to_string)
            .collect()
    })
}

fn gantt_end_constraint(
    value: &str,
    date_format: &str,
    runtime: &OperationContext,
) -> GanttTaskEndConstraint {
    let trimmed = value.trim_matches(is_ecmascript_whitespace);
    if trimmed.is_empty() {
        return GanttTaskEndConstraint::Unspecified;
    }
    if let Some(dependency_ids) = relative_dependency_ids(trimmed, "until") {
        return GanttTaskEndConstraint::Until { dependency_ids };
    }
    if parse_dayjs_like_strict(date_format.trim(), trimmed, runtime).is_some() {
        return GanttTaskEndConstraint::Fixed {
            value: value.to_string(),
        };
    }
    GanttTaskEndConstraint::Duration {
        value: value.to_string(),
    }
}

fn task_time_ms(task: &RawTask, field: &str, value: Option<OffsetDateTime>) -> Result<i64> {
    value.map(|d| d.timestamp_millis()).ok_or_else(|| {
        GanttSourceIssue::fallback(format!("task `{}` has unresolved {field}", task.id))
    })
}

fn parse_gantt_statement(
    line: &str,
    line_start: usize,
    db: &mut GanttDb,
    cursor: &mut LineCursor<'_>,
    #[cfg(feature = "editor-facts")] facts: &mut Option<&mut GanttObserver<'_>>,
    control: &OperationControl,
) -> OperationControlResult<Result<()>> {
    control.checkpoint()?;
    let stripped = strip_inline_comment(line);
    let t = stripped.trim_start_matches(is_ecmascript_whitespace);
    if t.is_empty() {
        return Ok(Ok(()));
    }

    if let Some(v) = parse_gantt_keyword_arg_spanned(
        stripped,
        line_start,
        "dateFormat",
        GanttDirectiveEnd::HashOrSemi,
        cursor,
    ) {
        #[cfg(feature = "editor-facts")]
        if let Some(facts) = facts.as_deref_mut() {
            facts(GanttObservation::DirectivePrefix("dateFormat"));
        }
        #[cfg(feature = "editor-facts")]
        if let Some(facts) = facts.as_deref_mut() {
            push_gantt_payload_symbol(
                &cursor.source()[line_start..v.end],
                line_start,
                v,
                GanttSymbolKind::DateFormat,
                facts,
            );
        }
        db.set_date_format(v.text);
        return Ok(Ok(()));
    }
    if starts_with_gantt_fixed_keyword(t, "inclusiveEndDates") {
        let leading = leading_whitespace_len(stripped);
        let end = line_start + leading + "inclusiveEndDates".len();
        if end < line_start + stripped.len() {
            cursor.resume_same_line_at(end);
        }
        #[cfg(feature = "editor-facts")]
        if let Some(facts) = facts.as_deref_mut() {
            facts(GanttObservation::DirectivePrefix("inclusiveEndDates"));
        }
        db.enable_inclusive_end_dates();
        return Ok(Ok(()));
    }
    if starts_with_gantt_fixed_keyword(t, "topAxis") {
        let leading = leading_whitespace_len(stripped);
        let end = line_start + leading + "topAxis".len();
        if end < line_start + stripped.len() {
            cursor.resume_same_line_at(end);
        }
        #[cfg(feature = "editor-facts")]
        if let Some(facts) = facts.as_deref_mut() {
            facts(GanttObservation::DirectivePrefix("topAxis"));
        }
        db.enable_top_axis();
        return Ok(Ok(()));
    }
    if let Some(v) = parse_gantt_keyword_arg_spanned(
        stripped,
        line_start,
        "axisFormat",
        GanttDirectiveEnd::HashOrSemi,
        cursor,
    ) {
        #[cfg(feature = "editor-facts")]
        if let Some(facts) = facts.as_deref_mut() {
            facts(GanttObservation::DirectivePrefix("axisFormat"));
        }
        #[cfg(feature = "editor-facts")]
        if let Some(facts) = facts.as_deref_mut() {
            push_gantt_payload_symbol(
                &cursor.source()[line_start..v.end],
                line_start,
                v,
                GanttSymbolKind::AxisFormat,
                facts,
            );
        }
        db.set_axis_format(v.text);
        return Ok(Ok(()));
    }
    if let Some(v) = parse_gantt_keyword_arg_spanned(
        stripped,
        line_start,
        "tickInterval",
        GanttDirectiveEnd::HashOrSemi,
        cursor,
    ) {
        #[cfg(feature = "editor-facts")]
        if let Some(facts) = facts.as_deref_mut() {
            facts(GanttObservation::DirectivePrefix("tickInterval"));
        }
        #[cfg(feature = "editor-facts")]
        if let Some(facts) = facts.as_deref_mut() {
            push_gantt_payload_symbol(
                &cursor.source()[line_start..v.end],
                line_start,
                v,
                GanttSymbolKind::TickInterval,
                facts,
            );
        }
        db.set_tick_interval(v.text);
        return Ok(Ok(()));
    }
    if let Some(v) = parse_gantt_keyword_arg_spanned(
        stripped,
        line_start,
        "includes",
        GanttDirectiveEnd::HashOrSemi,
        cursor,
    ) {
        #[cfg(feature = "editor-facts")]
        if let Some(facts) = facts.as_deref_mut() {
            facts(GanttObservation::DirectivePrefix("includes"));
        }
        #[cfg(feature = "editor-facts")]
        if let Some(facts) = facts.as_deref_mut() {
            push_gantt_payload_symbol(
                &cursor.source()[line_start..v.end],
                line_start,
                v,
                GanttSymbolKind::Includes,
                facts,
            );
        }
        db.set_includes(v.text);
        return Ok(Ok(()));
    }
    if let Some(v) = parse_gantt_keyword_arg_spanned(
        stripped,
        line_start,
        "excludes",
        GanttDirectiveEnd::HashOrSemi,
        cursor,
    ) {
        #[cfg(feature = "editor-facts")]
        if let Some(facts) = facts.as_deref_mut() {
            facts(GanttObservation::DirectivePrefix("excludes"));
        }
        #[cfg(feature = "editor-facts")]
        if let Some(facts) = facts.as_deref_mut() {
            push_gantt_payload_symbol(
                &cursor.source()[line_start..v.end],
                line_start,
                v,
                GanttSymbolKind::Excludes,
                facts,
            );
        }
        db.set_excludes(v.text);
        return Ok(Ok(()));
    }
    if let Some(v) = parse_gantt_keyword_arg_spanned(
        stripped,
        line_start,
        "todayMarker",
        GanttDirectiveEnd::Semi,
        cursor,
    ) {
        #[cfg(feature = "editor-facts")]
        if let Some(facts) = facts.as_deref_mut() {
            facts(GanttObservation::DirectivePrefix("todayMarker"));
        }
        #[cfg(feature = "editor-facts")]
        if let Some(facts) = facts.as_deref_mut() {
            push_gantt_payload_symbol(
                &cursor.source()[line_start..v.end],
                line_start,
                v,
                GanttSymbolKind::TodayMarker,
                facts,
            );
        }
        db.set_today_marker(v.text);
        return Ok(Ok(()));
    }
    if let Some(v) = parse_gantt_keyword_arg_spanned(
        stripped,
        line_start,
        "weekday",
        GanttDirectiveEnd::KeywordPrefix(&[
            "monday",
            "tuesday",
            "wednesday",
            "thursday",
            "friday",
            "saturday",
            "sunday",
        ]),
        cursor,
    ) {
        #[cfg(feature = "editor-facts")]
        if let Some(facts) = facts.as_deref_mut() {
            facts(GanttObservation::DirectivePrefix("weekday"));
        }
        #[cfg(feature = "editor-facts")]
        if let Some(facts) = facts.as_deref_mut() {
            push_gantt_payload_symbol(
                &cursor.source()[line_start..v.end],
                line_start,
                v,
                GanttSymbolKind::Weekday,
                facts,
            );
        }
        let trimmed_day = v.trim();
        let day = trimmed_day
            .map(|value| value.text.to_lowercase())
            .unwrap_or_default();
        if !matches!(
            day.as_str(),
            "monday" | "tuesday" | "wednesday" | "thursday" | "friday" | "saturday" | "sunday"
        ) {
            let span = trimmed_day
                .map(SpannedText::span)
                .unwrap_or_else(|| SourceSpan::new(v.start, v.start));
            return Ok(Err(GanttSourceIssue::exact(
                format!("invalid weekday: {day}"),
                span,
            )));
        }
        db.set_weekday(&day);
        return Ok(Ok(()));
    }
    if let Some(v) = parse_gantt_keyword_arg_spanned(
        stripped,
        line_start,
        "weekend",
        GanttDirectiveEnd::KeywordPrefix(&["friday", "saturday"]),
        cursor,
    ) {
        #[cfg(feature = "editor-facts")]
        if let Some(facts) = facts.as_deref_mut() {
            facts(GanttObservation::DirectivePrefix("weekend"));
        }
        #[cfg(feature = "editor-facts")]
        if let Some(facts) = facts.as_deref_mut() {
            push_gantt_payload_symbol(
                &cursor.source()[line_start..v.end],
                line_start,
                v,
                GanttSymbolKind::Weekend,
                facts,
            );
        }
        let trimmed_day = v.trim();
        let day = trimmed_day
            .map(|value| value.text.to_lowercase())
            .unwrap_or_default();
        if !matches!(day.as_str(), "friday" | "saturday") {
            let span = trimmed_day
                .map(SpannedText::span)
                .unwrap_or_else(|| SourceSpan::new(v.start, v.start));
            return Ok(Err(GanttSourceIssue::exact(
                format!("invalid weekend: {day}"),
                span,
            )));
        }
        db.set_weekend(&day);
        return Ok(Ok(()));
    }
    if let Some(v) = parse_gantt_keyword_arg_spanned(
        stripped,
        line_start,
        "title",
        GanttDirectiveEnd::Line,
        cursor,
    ) {
        #[cfg(feature = "editor-facts")]
        if let Some(facts) = facts.as_deref_mut() {
            facts(GanttObservation::DirectivePrefix("title"));
        }
        #[cfg(feature = "editor-facts")]
        if let Some(facts) = facts.as_deref_mut() {
            push_gantt_payload_symbol(
                &cursor.source()[line_start..v.end],
                line_start,
                v,
                GanttSymbolKind::Title,
                facts,
            );
        }
        db.set_diagram_title(v.text);
        return Ok(Ok(()));
    }
    if let Some(v) = parse_gantt_keyword_arg_spanned(
        stripped,
        line_start,
        "section",
        GanttDirectiveEnd::Line,
        cursor,
    ) {
        #[cfg(feature = "editor-facts")]
        if let Some(facts) = facts.as_deref_mut() {
            facts(GanttObservation::DirectivePrefix("section"));
        }
        #[cfg(feature = "editor-facts")]
        if let Some(facts) = facts.as_deref_mut() {
            collect_gantt_section_symbol(&cursor.source()[line_start..v.end], line_start, v, facts);
        }
        db.add_section(v.text);
        return Ok(Ok(()));
    }
    if let Some(v) = parse_key_colon_value_spanned(stripped, line_start, "accTitle", cursor) {
        #[cfg(feature = "editor-facts")]
        if let Some(facts) = facts.as_deref_mut() {
            facts(GanttObservation::DirectivePrefix("accTitle"));
        }
        if v.text.is_empty() {
            #[cfg(feature = "editor-facts")]
            if let Some(facts) = facts.as_deref_mut() {
                facts(GanttObservation::ExpectedSyntax {
                    kind: GanttExpectedSyntax::Payload,
                    span: SourceSpan::new(v.start, v.start),
                });
            }
            return Ok(Err(GanttSourceIssue::insertion_point(
                "invalid accTitle statement: expected a value",
                v.start,
            )));
        }
        #[cfg(feature = "editor-facts")]
        if let Some(facts) = facts.as_deref_mut() {
            push_gantt_payload_symbol(
                &cursor.source()[line_start..v.end],
                line_start,
                v,
                GanttSymbolKind::AccTitle,
                facts,
            );
        }
        db.set_acc_title(v.text.trim_matches(is_ecmascript_whitespace));
        return Ok(Ok(()));
    }
    if let Some(v) = parse_key_colon_value_spanned(stripped, line_start, "accDescr", cursor) {
        #[cfg(feature = "editor-facts")]
        if let Some(facts) = facts.as_deref_mut() {
            facts(GanttObservation::DirectivePrefix("accDescr"));
        }
        if v.text.is_empty() {
            #[cfg(feature = "editor-facts")]
            if let Some(facts) = facts.as_deref_mut() {
                facts(GanttObservation::ExpectedSyntax {
                    kind: GanttExpectedSyntax::Payload,
                    span: SourceSpan::new(v.start, v.start),
                });
            }
            return Ok(Err(GanttSourceIssue::insertion_point(
                "invalid accDescr statement: expected a value",
                v.start,
            )));
        }
        #[cfg(feature = "editor-facts")]
        if let Some(facts) = facts.as_deref_mut() {
            push_gantt_payload_symbol(
                &cursor.source()[line_start..v.end],
                line_start,
                v,
                GanttSymbolKind::AccDescr,
                facts,
            );
        }
        db.set_acc_descr(v.text.trim_matches(is_ecmascript_whitespace));
        return Ok(Ok(()));
    }
    if let Some(block) = GanttAccDescrBlock::start(stripped, line_start, cursor) {
        #[cfg(feature = "editor-facts")]
        if let Some(facts) = facts.as_deref_mut() {
            facts(GanttObservation::DirectivePrefix("accDescr"));
        }
        let block = block.consume_remaining(cursor, control)?;
        if !block.value().is_empty() {
            db.set_acc_descr(block.value());
        }
        #[cfg(feature = "editor-facts")]
        if let Some(facts) = facts.as_deref_mut() {
            block.emit_symbol(facts);
        }
        if !block.is_complete() {
            return Ok(Err(GanttSourceIssue::insertion_point(
                "unterminated accDescr block",
                block.statement_span().end,
            )));
        }
        return Ok(Ok(()));
    }
    match parse_click_statement(stripped, line_start, cursor, control)? {
        Ok(Some(click)) => {
            cursor.resume_at(click.end);
            #[cfg(feature = "editor-facts")]
            if let Some(facts) = facts.as_deref_mut() {
                facts(GanttObservation::DirectivePrefix("click"));
            }
            #[cfg(feature = "editor-facts")]
            if let Some(facts) = facts.as_deref_mut() {
                collect_gantt_click_symbols(
                    &cursor.source()[line_start..click.end],
                    line_start,
                    click,
                    facts,
                );
            }
            if let Some(call) = click.call {
                db.set_click_event(
                    click.ids.text,
                    call.name.text.trim_matches(is_ecmascript_whitespace),
                    call.args.map(|args| args.text),
                );
            }
            if let Some(href) = click.href {
                db.set_link(click.ids.text, href.text);
            }
            return Ok(Ok(()));
        }
        Ok(None) => {}
        Err(error) => {
            #[cfg(feature = "editor-facts")]
            if let Some(facts) = facts.as_deref_mut() {
                facts(GanttObservation::DirectivePrefix("click"));
            }
            #[cfg(feature = "editor-facts")]
            if let Some(facts) = facts.as_deref_mut()
                && let Some(expected_action) = error.expected_action
            {
                facts(GanttObservation::ExpectedSyntax {
                    kind: GanttExpectedSyntax::InteractionAction,
                    span: expected_action,
                });
            }
            return Ok(Err(GanttSourceIssue::exact(
                error.message,
                gantt_statement_span(stripped, line_start),
            )));
        }
    }

    let task_stmt = stripped.trim_start_matches(is_ecmascript_whitespace);

    let Some(colon) = task_stmt.find(':') else {
        return Ok(Err(GanttSourceIssue::exact(
            format!("unrecognized statement: {t}"),
            gantt_statement_span(stripped, line_start),
        )));
    };

    // Mermaid passes `taskTxt` through to the DB without trimming. This preserves any trailing
    // whitespace before the `:` delimiter (e.g. `Task1 :id,...` yields `Task1 `).
    let task_txt = &task_stmt[..colon];
    let task_data = split_statement_suffix_hash_or_semi(&task_stmt[colon + 1..]);
    let leading = stripped.len().saturating_sub(task_stmt.len());
    let task_start = line_start + leading;
    let task_end = task_start + colon + 1 + task_data.len();
    if task_end < line_start + stripped.len() {
        cursor.resume_same_line_at(task_end);
    }
    if task_txt.is_empty() || task_data.is_empty() {
        return Ok(Err(GanttSourceIssue::exact(
            "invalid task statement: expected task text and data".to_string(),
            SourceSpan::new(task_start, task_end),
        )));
    }
    #[cfg(feature = "editor-facts")]
    let statement_span = SourceSpan::new(task_start, task_end);
    let fields = split_gantt_fields(task_data, task_start + colon + 1);
    #[cfg(feature = "editor-facts")]
    if let Some(facts) = facts.as_deref_mut() {
        let editor_fields = fields
            .iter()
            .copied()
            .map(|field| field.trim().unwrap_or(field))
            .collect::<Vec<_>>();
        collect_gantt_task_field_symbols(&editor_fields, statement_span, facts);
    }
    let field_text = fields.iter().map(|field| field.text).collect::<Vec<_>>();
    let task_info = db.parse_task_info(&field_text);
    db.add_task(task_txt, &format!(":{task_data}"), task_info);
    Ok(Ok(()))
}
