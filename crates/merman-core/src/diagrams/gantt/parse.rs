use super::*;
use crate::diagrams::scan::{
    leading_whitespace_len, starts_with_case_insensitive, strip_line_ending,
};
use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
    EditorSemanticSymbol, SourceSpan,
};

fn strip_inline_comment(line: &str) -> &str {
    // Mermaid gantt does not treat `%%` as an inline comment delimiter for statements like `title`
    // or task lines (see `fixtures/gantt/task_inline_percent_comment.mmd`). It does, however,
    // accept full-line `%% ...` comments (and directive lines `%%{...}%%`).
    let t = line.trim_start();
    if t.starts_with("%%{") {
        return line;
    }
    if t.starts_with("%%") {
        return "";
    }
    line
}

fn split_statement_suffix(s: &str) -> &str {
    let mut end = s.len();
    for (i, c) in s.char_indices() {
        if c == '#' || c == ';' {
            end = i;
            break;
        }
    }
    &s[..end]
}

fn parse_keyword_arg<'a>(line: &'a str, keyword: &str) -> Option<&'a str> {
    let t = line.trim_start();
    if !starts_with_case_insensitive(t, keyword) {
        return None;
    }
    let after = &t[keyword.len()..];
    let ws = after.chars().next()?;
    if !ws.is_whitespace() {
        return None;
    }
    let rest = &after[ws.len_utf8()..];
    Some(split_statement_suffix(rest))
}

fn parse_keyword_arg_full_line<'a>(line: &'a str, keyword: &str) -> Option<&'a str> {
    let t = line.trim_start();
    if !starts_with_case_insensitive(t, keyword) {
        return None;
    }
    let after = &t[keyword.len()..];
    let ws = after.chars().next()?;
    if !ws.is_whitespace() {
        return None;
    }
    Some(&after[ws.len_utf8()..])
}

fn parse_key_colon_value(line: &str, key: &str) -> Option<String> {
    let t = line.trim_start();
    if !starts_with_case_insensitive(t, key) {
        return None;
    }
    let rest = t[key.len()..].trim_start();
    let rest = rest.strip_prefix(':')?;
    // Mermaid gantt's `accTitle:` / `accDescr:` values are end-of-line tokens (not `;`/`#`-terminated).
    Some(rest.trim().to_string())
}

fn parse_key_colon_value_spanned<'a>(
    line: &'a str,
    line_start: usize,
    key: &str,
) -> Option<SpannedText<'a>> {
    let trimmed = line.trim_start();
    if !starts_with_case_insensitive(trimmed, key) {
        return None;
    }
    let leading = line.len().saturating_sub(trimmed.len());
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
    Some(SpannedText {
        text: &trimmed[value_start..],
        start: line_start + leading + value_start,
        end: line_start + leading + trimmed.len(),
    })
}

fn parse_acc_descr_block(cursor: &mut GanttLineCursor<'_>, first_line: &str) -> Option<String> {
    let t = first_line.trim_start();
    if !starts_with_case_insensitive(t, "accDescr") {
        return None;
    }
    let rest = t["accDescr".len()..].trim_start();
    let rest = rest.strip_prefix('{')?;

    let mut buf = String::new();
    if let Some(end) = rest.find('}') {
        buf.push_str(&rest[..end]);
        return Some(buf.trim().to_string());
    }
    buf.push_str(rest);
    buf.push('\n');

    while let Some((line, _line_start)) = cursor.next_line() {
        if let Some(end) = line.find('}') {
            buf.push_str(&line[..end]);
            break;
        }
        buf.push_str(line);
        buf.push('\n');
    }
    Some(buf.trim().to_string())
}

struct GanttLineCursor<'a> {
    segments: std::str::SplitInclusive<'a, char>,
    offset: usize,
}

impl<'a> GanttLineCursor<'a> {
    fn new(code: &'a str) -> Self {
        Self {
            segments: code.split_inclusive('\n'),
            offset: 0,
        }
    }

    fn next_line(&mut self) -> Option<(&'a str, usize)> {
        let segment = self.segments.next()?;
        let line_start = self.offset;
        self.offset += segment.len();
        Some((strip_line_ending(segment), line_start))
    }
}

fn parse_click_statement(
    line: &str,
    line_start: usize,
) -> std::result::Result<Option<ClickStatementParts<'_>>, String> {
    let trimmed = line.trim_start();
    if !starts_with_case_insensitive(trimmed, "click") {
        return Ok(None);
    }
    let leading = line.len().saturating_sub(trimmed.len());
    let after_click = &trimmed["click".len()..];
    let rest_leading = leading_whitespace_len(after_click);
    let rest_start = "click".len() + rest_leading;
    let rest = &trimmed[rest_start..];
    let ids_len = rest
        .char_indices()
        .find_map(|(idx, ch)| ch.is_whitespace().then_some(idx))
        .unwrap_or(rest.len());
    let ids = SpannedText {
        text: &rest[..ids_len],
        start: line_start + leading + rest_start,
        end: line_start + leading + rest_start + ids_len,
    };

    let mut tail_offset = rest_start + ids_len;
    tail_offset += leading_whitespace_len(&trimmed[tail_offset..]);
    let mut href = None;
    let mut call = None;

    while tail_offset < trimmed.len() {
        let tail = &trimmed[tail_offset..];
        if starts_with_case_insensitive(tail, "href") {
            let href_keyword_end = tail_offset + "href".len();
            let after_href = &trimmed[href_keyword_end..];
            let href_ws = leading_whitespace_len(after_href);
            let quote_start = href_keyword_end + href_ws;
            let value_start = quote_start + '"'.len_utf8();
            if !trimmed[quote_start..].starts_with('"') {
                return Err("invalid click statement: href requires a quoted URL".to_string());
            }
            let value_tail = &trimmed[value_start..];
            let Some(end) = value_tail.find('"') else {
                return Err("invalid click statement: unterminated href URL".to_string());
            };
            let value_end = value_start + end;
            href = Some(SpannedText {
                text: &trimmed[value_start..value_end],
                start: line_start + leading + value_start,
                end: line_start + leading + value_end,
            });
            tail_offset = value_end + '"'.len_utf8();
            tail_offset += leading_whitespace_len(&trimmed[tail_offset..]);
            continue;
        }

        if starts_with_case_insensitive(tail, "call") {
            let call_keyword_end = tail_offset + "call".len();
            let after_call = &trimmed[call_keyword_end..];
            let call_ws = leading_whitespace_len(after_call);
            let name_start = call_keyword_end + call_ws;
            let (parsed_call, next_offset) =
                parse_callback_tail(trimmed, name_start, line_start + leading)?;
            call = Some(parsed_call);
            tail_offset = next_offset;
            tail_offset += leading_whitespace_len(&trimmed[tail_offset..]);
            continue;
        }

        if tail.starts_with('"') {
            tail_offset = skip_quoted_click_tail(trimmed, tail_offset)?;
            tail_offset += leading_whitespace_len(&trimmed[tail_offset..]);
            continue;
        }

        if call.is_none() {
            let (parsed_call, next_offset) =
                parse_callback_tail(trimmed, tail_offset, line_start + leading)?;
            call = Some(parsed_call);
            tail_offset = next_offset;
            tail_offset += leading_whitespace_len(&trimmed[tail_offset..]);
            continue;
        }

        return Err(format!("invalid click statement tail: {tail:?}"));
    }

    Ok(Some(ClickStatementParts { ids, href, call }))
}

fn skip_quoted_click_tail(trimmed: &str, quote_start: usize) -> std::result::Result<usize, String> {
    let value_start = quote_start + '"'.len_utf8();
    let Some(end) = trimmed[value_start..].find('"') else {
        return Err("invalid click statement: unterminated quoted tail".to_string());
    };
    Ok(value_start + end + '"'.len_utf8())
}

fn parse_callback_tail<'a>(
    trimmed: &'a str,
    name_start: usize,
    absolute_offset: usize,
) -> std::result::Result<(ClickCallParts<'a>, usize), String> {
    let name_tail = &trimmed[name_start..];
    let Some(name_len) = callback_name_len(name_tail) else {
        return Err("invalid click statement: missing callback name".to_string());
    };
    let name_end = name_start + name_len;
    let mut next_offset = name_end;
    let args = if trimmed[next_offset..].starts_with('(') {
        let args_start = next_offset + '('.len_utf8();
        let args_tail = &trimmed[args_start..];
        let Some(end_rel) = args_tail.find(')') else {
            return Err("invalid click statement: unterminated callback args".to_string());
        };
        let args_end = args_start + end_rel;
        next_offset = args_end + ')'.len_utf8();
        let args_text = &trimmed[args_start..args_end];
        if args_text.trim().is_empty() {
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
    Ok((
        ClickCallParts {
            name: SpannedText {
                text: &trimmed[name_start..name_end],
                start: absolute_offset + name_start,
                end: absolute_offset + name_end,
            },
            args,
        },
        next_offset,
    ))
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
struct ClickStatementParts<'a> {
    ids: SpannedText<'a>,
    href: Option<SpannedText<'a>>,
    call: Option<ClickCallParts<'a>>,
}

#[derive(Debug, Clone, Copy)]
struct ClickCallParts<'a> {
    name: SpannedText<'a>,
    args: Option<SpannedText<'a>>,
}

pub fn parse_gantt_editor_facts(code: &str, _meta: &ParseMetadata) -> EditorSemanticFacts {
    collect_gantt_editor_facts_from_lines(code)
}

fn collect_gantt_editor_facts_from_lines(code: &str) -> EditorSemanticFacts {
    let mut facts = EditorSemanticFacts::new();
    let mut header_seen = false;
    let mut in_frontmatter = false;
    let mut acc_descr_block = None;
    let mut offset = 0usize;

    for segment in code.split_inclusive('\n') {
        let line = strip_line_ending(segment);
        collect_gantt_editor_line(
            line,
            offset,
            &mut header_seen,
            &mut in_frontmatter,
            &mut acc_descr_block,
            &mut facts,
        );
        offset += segment.len();
    }

    if let Some(block) = acc_descr_block.take() {
        let diagnostic_span = SourceSpan::new(block.statement_start, block.statement_end);
        block.emit_symbol(&mut facts);
        facts.mark_recovered_with_diagnostic(
            "gantt parser recovered from unterminated accDescr block",
            Some(diagnostic_span),
        );
    }

    facts
}

fn collect_gantt_editor_line(
    line: &str,
    line_start: usize,
    header_seen: &mut bool,
    in_frontmatter: &mut bool,
    acc_descr_block: &mut Option<GanttAccDescrBlock>,
    facts: &mut EditorSemanticFacts,
) {
    let raw_trimmed = line.trim();
    if *in_frontmatter {
        if raw_trimmed == "---" {
            *in_frontmatter = false;
        }
        return;
    }
    if line_start == 0 && raw_trimmed == "---" {
        *in_frontmatter = true;
        return;
    }

    if let Some(block) = acc_descr_block.as_mut() {
        if block.accept_continuation_line(line, line_start) {
            if let Some(block) = acc_descr_block.take() {
                block.emit_symbol(facts);
            }
        }
        return;
    }

    let stripped = strip_inline_comment(line);
    let trimmed = stripped.trim();
    if trimmed.is_empty() {
        return;
    }
    if trimmed.starts_with("%%{") {
        return;
    }

    if !*header_seen && starts_with_case_insensitive(trimmed, "gantt") {
        *header_seen = true;
        if let Some((rest, rest_start)) = gantt_header_rest(stripped, line_start)
            && !rest.trim().is_empty()
        {
            let recognized =
                collect_gantt_statement_editor_facts(rest, rest_start, acc_descr_block, facts);
            if !recognized {
                mark_gantt_recovered_statement(
                    facts,
                    "gantt parser recovered from unrecognized statement after header",
                    rest,
                    rest_start,
                );
            }
        }
        return;
    }

    let missing_header = !*header_seen;
    if !*header_seen {
        mark_gantt_recovered_statement(
            facts,
            "gantt parser recovered before gantt header",
            stripped,
            line_start,
        );
    }

    let recognized =
        collect_gantt_statement_editor_facts(stripped, line_start, acc_descr_block, facts);
    if !recognized && !missing_header {
        mark_gantt_recovered_statement(
            facts,
            "gantt parser recovered from unrecognized statement",
            stripped,
            line_start,
        );
    }
}

fn mark_gantt_recovered_statement(
    facts: &mut EditorSemanticFacts,
    message: &'static str,
    line: &str,
    line_start: usize,
) {
    facts.mark_recovered_with_diagnostic(message, Some(gantt_statement_span(line, line_start)));
}

fn gantt_header_rest(line: &str, line_start: usize) -> Option<(&str, usize)> {
    let trimmed = line.trim_start();
    if !starts_with_case_insensitive(trimmed, "gantt") {
        return None;
    }

    let leading = line.len().saturating_sub(trimmed.len());
    let after_start = leading + "gantt".len();
    let after = &line[after_start..];
    let whitespace_len: usize = after
        .chars()
        .take_while(|ch| ch.is_whitespace())
        .map(char::len_utf8)
        .sum();
    Some((
        &after[whitespace_len..],
        line_start + after_start + whitespace_len,
    ))
}

fn collect_gantt_statement_editor_facts(
    line: &str,
    line_start: usize,
    acc_descr_block: &mut Option<GanttAccDescrBlock>,
    facts: &mut EditorSemanticFacts,
) -> bool {
    let stripped = strip_inline_comment(line);
    let trimmed = stripped.trim();
    if trimmed.is_empty() {
        return true;
    }

    if let Some(format) = parse_gantt_keyword_arg_spanned(stripped, line_start, "dateFormat", true)
    {
        facts.push_directive_prefix("dateFormat");
        push_gantt_payload_symbol(
            stripped,
            line_start,
            format,
            "gantt date format",
            EditorSemanticKind::String,
            facts,
        );
        return true;
    }
    if starts_with_case_insensitive(trimmed, "inclusiveEndDates") {
        facts.push_directive_prefix("inclusiveEndDates");
        return true;
    }
    if starts_with_case_insensitive(trimmed, "topAxis") {
        facts.push_directive_prefix("topAxis");
        return true;
    }
    if let Some(format) = parse_gantt_keyword_arg_spanned(stripped, line_start, "axisFormat", true)
    {
        facts.push_directive_prefix("axisFormat");
        push_gantt_payload_symbol(
            stripped,
            line_start,
            format,
            "gantt axis format",
            EditorSemanticKind::String,
            facts,
        );
        return true;
    }
    if let Some(interval) =
        parse_gantt_keyword_arg_spanned(stripped, line_start, "tickInterval", true)
    {
        facts.push_directive_prefix("tickInterval");
        push_gantt_payload_symbol(
            stripped,
            line_start,
            interval,
            "gantt tick interval",
            EditorSemanticKind::String,
            facts,
        );
        return true;
    }
    if let Some(includes) = parse_gantt_keyword_arg_spanned(stripped, line_start, "includes", true)
    {
        facts.push_directive_prefix("includes");
        push_gantt_payload_symbol(
            stripped,
            line_start,
            includes,
            "gantt includes",
            EditorSemanticKind::String,
            facts,
        );
        return true;
    }
    if let Some(excludes) = parse_gantt_keyword_arg_spanned(stripped, line_start, "excludes", true)
    {
        facts.push_directive_prefix("excludes");
        push_gantt_payload_symbol(
            stripped,
            line_start,
            excludes,
            "gantt excludes",
            EditorSemanticKind::String,
            facts,
        );
        return true;
    }
    if let Some(marker) =
        parse_gantt_keyword_arg_spanned(stripped, line_start, "todayMarker", false)
    {
        facts.push_directive_prefix("todayMarker");
        push_gantt_payload_symbol(
            stripped,
            line_start,
            marker,
            "gantt today marker",
            EditorSemanticKind::String,
            facts,
        );
        return true;
    }
    if let Some(day) = parse_gantt_keyword_arg_spanned(stripped, line_start, "weekday", false) {
        facts.push_directive_prefix("weekday");
        push_gantt_payload_symbol(
            stripped,
            line_start,
            day,
            "gantt weekday",
            EditorSemanticKind::String,
            facts,
        );
        let day_value = day.trim();
        let day_text = day_value
            .map(|value| value.text.to_lowercase())
            .unwrap_or_default();
        if !matches!(
            day_text.as_str(),
            "monday" | "tuesday" | "wednesday" | "thursday" | "friday" | "saturday" | "sunday"
        ) {
            facts.mark_recovered_with_diagnostic(
                "gantt parser recovered from invalid weekday",
                day_value.map(SpannedText::span),
            );
        }
        return true;
    }
    if let Some(day) = parse_gantt_keyword_arg_spanned(stripped, line_start, "weekend", false) {
        facts.push_directive_prefix("weekend");
        push_gantt_payload_symbol(
            stripped,
            line_start,
            day,
            "gantt weekend",
            EditorSemanticKind::String,
            facts,
        );
        let day_value = day.trim();
        let day_text = day_value
            .map(|value| value.text.to_lowercase())
            .unwrap_or_default();
        if !matches!(day_text.as_str(), "friday" | "saturday") {
            facts.mark_recovered_with_diagnostic(
                "gantt parser recovered from invalid weekend",
                day_value.map(SpannedText::span),
            );
        }
        return true;
    }
    if let Some(title) = parse_gantt_keyword_arg_spanned(stripped, line_start, "title", false) {
        facts.push_directive_prefix("title");
        push_gantt_payload_symbol(
            stripped,
            line_start,
            title,
            "gantt title",
            EditorSemanticKind::String,
            facts,
        );
        return true;
    }
    if let Some(section) = parse_gantt_keyword_arg_spanned(stripped, line_start, "section", false) {
        facts.push_directive_prefix("section");
        collect_gantt_section_symbol(stripped, line_start, section, facts);
        return true;
    }
    if let Some(acc_title) = parse_key_colon_value_spanned(stripped, line_start, "accTitle") {
        facts.push_directive_prefix("accTitle");
        push_gantt_payload_symbol(
            stripped,
            line_start,
            acc_title,
            "gantt accessibility title",
            EditorSemanticKind::String,
            facts,
        );
        return true;
    }
    if let Some(acc_descr) = parse_key_colon_value_spanned(stripped, line_start, "accDescr") {
        facts.push_directive_prefix("accDescr");
        push_gantt_payload_symbol(
            stripped,
            line_start,
            acc_descr,
            "gantt accessibility description",
            EditorSemanticKind::String,
            facts,
        );
        return true;
    }
    if let Some(block) = GanttAccDescrBlock::start(stripped, line_start) {
        facts.push_directive_prefix("accDescr");
        if block.is_complete() {
            block.emit_symbol(facts);
        } else {
            *acc_descr_block = Some(block);
        }
        return true;
    }
    match parse_click_statement(stripped, line_start) {
        Ok(Some(click)) => {
            facts.push_directive_prefix("click");
            collect_gantt_click_symbols(stripped, line_start, click, facts);
            return true;
        }
        Ok(None) => {}
        Err(message) => {
            facts.push_directive_prefix("click");
            facts.mark_recovered_with_diagnostic(
                format!("gantt parser recovered from {message}"),
                Some(gantt_statement_span(stripped, line_start)),
            );
            return true;
        }
    }

    collect_gantt_task_symbols(stripped, line_start, facts)
}

fn parse_gantt_keyword_arg_spanned<'a>(
    line: &'a str,
    line_start: usize,
    keyword: &str,
    terminates_at_statement_suffix: bool,
) -> Option<SpannedText<'a>> {
    let trimmed = line.trim_start();
    if !starts_with_case_insensitive(trimmed, keyword) {
        return None;
    }
    let after = &trimmed[keyword.len()..];
    let ws = after.chars().next()?;
    if !ws.is_whitespace() {
        return None;
    }
    let ws_len = leading_whitespace_len(after);
    let rest_start = keyword.len() + ws_len;
    let rest = &after[ws_len..];
    let text = if terminates_at_statement_suffix {
        split_statement_suffix(rest)
    } else {
        rest
    };
    let leading = line.len().saturating_sub(trimmed.len());
    let start = line_start + leading + rest_start;
    Some(SpannedText {
        text,
        start,
        end: start + text.len(),
    })
}

#[derive(Debug)]
struct GanttAccDescrBlock {
    statement_start: usize,
    statement_end: usize,
    body: String,
    first_content_start: Option<usize>,
    last_content_end: Option<usize>,
    complete: bool,
}

impl GanttAccDescrBlock {
    fn start(line: &str, line_start: usize) -> Option<Self> {
        let trimmed = line.trim_start();
        if !starts_with_case_insensitive(trimmed, "accDescr") {
            return None;
        }

        let leading = line.len().saturating_sub(trimmed.len());
        let after_key = &trimmed["accDescr".len()..];
        let after_key_ws = leading_whitespace_len(after_key);
        let open_offset = leading + "accDescr".len() + after_key_ws;
        if !line[open_offset..].starts_with('{') {
            return None;
        }

        let body_start = open_offset + '{'.len_utf8();
        let mut block = Self {
            statement_start: line_start + leading,
            statement_end: line_start + line.len(),
            body: String::new(),
            first_content_start: None,
            last_content_end: None,
            complete: false,
        };
        block.accept_body_slice(&line[body_start..], line_start + body_start);
        Some(block)
    }

    fn accept_continuation_line(&mut self, line: &str, line_start: usize) -> bool {
        self.accept_body_slice(line, line_start);
        self.complete
    }

    fn is_complete(&self) -> bool {
        self.complete
    }

    fn accept_body_slice(&mut self, text: &str, start: usize) {
        let Some(close_offset) = text.find('}') else {
            self.append_text(text, start);
            self.body.push('\n');
            self.statement_end = start + text.len();
            return;
        };

        self.append_text(&text[..close_offset], start);
        self.statement_end = start + close_offset + '}'.len_utf8();
        self.complete = true;
    }

    fn append_text(&mut self, text: &str, start: usize) {
        self.body.push_str(text);

        if self.first_content_start.is_none() {
            let leading = leading_whitespace_len(text);
            if leading < text.len() {
                self.first_content_start = Some(start + leading);
            }
        }

        let trimmed_len = text.trim_end().len();
        if trimmed_len > 0 {
            self.last_content_end = Some(start + trimmed_len);
        }
    }

    fn emit_symbol(self, facts: &mut EditorSemanticFacts) {
        let text = self.body.trim();
        if text.is_empty() {
            return;
        }

        let Some(selection_start) = self.first_content_start else {
            return;
        };
        let Some(selection_end) = self.last_content_end else {
            return;
        };

        facts.push_symbol(EditorSemanticSymbol::payload(
            text.to_string(),
            Some("gantt accessibility description".to_string()),
            EditorSemanticKind::String,
            SourceSpan::new(self.statement_start, self.statement_end),
            SourceSpan::new(selection_start, selection_end),
        ));
        facts.push_expected_syntax(EditorExpectedSyntax::new(
            EditorExpectedSyntaxKind::Payload,
            SourceSpan::new(selection_start, selection_end),
        ));
    }
}

fn collect_gantt_click_symbols(
    line: &str,
    line_start: usize,
    click: ClickStatementParts<'_>,
    facts: &mut EditorSemanticFacts,
) {
    let statement_span = gantt_statement_span(line, line_start);

    push_gantt_delimited_id_symbols(
        click.ids.text,
        click.ids.start,
        ',',
        "gantt click target",
        EditorSemanticKind::Variable,
        Some(EditorExpectedSyntaxKind::NodeIdentifier),
        statement_span,
        facts,
    );

    if let Some(href) = click.href {
        push_gantt_payload_symbol(
            line,
            line_start,
            href,
            "gantt click href",
            EditorSemanticKind::String,
            facts,
        );
    }

    if let Some(call) = click.call {
        push_gantt_payload_symbol(
            line,
            line_start,
            call.name,
            "gantt click callback",
            EditorSemanticKind::Function,
            facts,
        );
        if let Some(args) = call.args {
            push_gantt_payload_symbol(
                line,
                line_start,
                args,
                "gantt click callback args",
                EditorSemanticKind::String,
                facts,
            );
        }
    }
}

fn collect_gantt_section_symbol(
    line: &str,
    line_start: usize,
    section: SpannedText<'_>,
    facts: &mut EditorSemanticFacts,
) {
    let Some(section) = section.trim() else {
        return;
    };

    facts.push_symbol(EditorSemanticSymbol::outline(
        section.text,
        Some("gantt section".to_string()),
        EditorSemanticKind::Namespace,
        gantt_statement_span(line, line_start),
        section.span(),
    ));
}

fn push_gantt_payload_symbol(
    line: &str,
    line_start: usize,
    field: SpannedText<'_>,
    detail: &'static str,
    kind: EditorSemanticKind,
    facts: &mut EditorSemanticFacts,
) {
    let Some(field) = field.trim() else {
        return;
    };
    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::Payload,
        field.span(),
    ));
    facts.push_symbol(EditorSemanticSymbol::payload(
        field.text,
        Some(detail.to_string()),
        kind,
        gantt_statement_span(line, line_start),
        field.span(),
    ));
}

fn gantt_statement_span(line: &str, line_start: usize) -> SourceSpan {
    let trimmed_line = line.trim_start();
    let leading = line.len().saturating_sub(trimmed_line.len());
    SourceSpan::new(
        line_start + leading,
        line_start + leading + trimmed_line.len(),
    )
}

fn collect_gantt_task_symbols(
    line: &str,
    line_start: usize,
    facts: &mut EditorSemanticFacts,
) -> bool {
    let task_stmt = line.trim_start();
    let leading = line.len().saturating_sub(task_stmt.len());
    let Some(colon) = task_stmt.find(':') else {
        return false;
    };

    let task_txt = &task_stmt[..colon];
    let task_data = split_statement_suffix(&task_stmt[colon + 1..]);
    if task_txt.is_empty() || task_data.trim().is_empty() {
        return true;
    }

    let statement_span =
        SourceSpan::new(line_start + leading, line_start + leading + task_stmt.len());
    collect_gantt_task_data_symbols(
        task_data,
        line_start + leading + colon + 1,
        statement_span,
        facts,
    );
    true
}

fn collect_gantt_task_data_symbols(
    task_data: &str,
    task_data_start: usize,
    statement_span: SourceSpan,
    facts: &mut EditorSemanticFacts,
) {
    let fields = split_gantt_fields(task_data, task_data_start)
        .into_iter()
        .filter_map(SpannedText::trim)
        .collect::<Vec<_>>();
    let mut field_start = 0usize;
    while fields
        .get(field_start)
        .is_some_and(|field| is_gantt_task_tag(field.text))
    {
        field_start += 1;
    }

    let fields = &fields[field_start..];
    match fields {
        [end_data] => push_gantt_relative_ref_symbols(end_data, statement_span, facts),
        [start_data, end_data] => {
            push_gantt_relative_ref_symbols(start_data, statement_span, facts);
            push_gantt_relative_ref_symbols(end_data, statement_span, facts);
        }
        [id, start_data, end_data] => {
            push_gantt_id_symbol(
                *id,
                "gantt task",
                EditorSemanticKind::Variable,
                Some(EditorExpectedSyntaxKind::NodeIdentifier),
                statement_span,
                facts,
            );
            push_gantt_relative_ref_symbols(start_data, statement_span, facts);
            push_gantt_relative_ref_symbols(end_data, statement_span, facts);
        }
        _ => {}
    }
}

fn is_gantt_task_tag(text: &str) -> bool {
    matches!(text, "active" | "done" | "crit" | "milestone" | "vert")
}

fn push_gantt_relative_ref_symbols(
    field: &SpannedText<'_>,
    statement_span: SourceSpan,
    facts: &mut EditorSemanticFacts,
) {
    for keyword in ["after", "until"] {
        let Some(range) = relative_ref_ids_range(field.text, keyword) else {
            continue;
        };
        push_gantt_delimited_id_symbols(
            &field.text[range.clone()],
            field.start + range.start,
            ' ',
            "gantt dependency",
            EditorSemanticKind::Variable,
            Some(EditorExpectedSyntaxKind::NodeIdentifier),
            statement_span,
            facts,
        );
    }
}

fn push_gantt_delimited_id_symbols(
    text: &str,
    text_start: usize,
    delimiter: char,
    detail: &str,
    kind: EditorSemanticKind,
    expected_syntax: Option<EditorExpectedSyntaxKind>,
    statement_span: SourceSpan,
    facts: &mut EditorSemanticFacts,
) {
    let mut segment_start = 0usize;
    for (idx, ch) in text.char_indices() {
        if ch == delimiter {
            push_gantt_id_symbol(
                SpannedText {
                    text: &text[segment_start..idx],
                    start: text_start + segment_start,
                    end: text_start + idx,
                },
                detail,
                kind,
                expected_syntax,
                statement_span,
                facts,
            );
            segment_start = idx + ch.len_utf8();
        }
    }

    push_gantt_id_symbol(
        SpannedText {
            text: &text[segment_start..],
            start: text_start + segment_start,
            end: text_start + text.len(),
        },
        detail,
        kind,
        expected_syntax,
        statement_span,
        facts,
    );
}

fn push_gantt_id_symbol(
    field: SpannedText<'_>,
    detail: &str,
    kind: EditorSemanticKind,
    expected_syntax: Option<EditorExpectedSyntaxKind>,
    statement_span: SourceSpan,
    facts: &mut EditorSemanticFacts,
) {
    let Some(field) = field.trim() else {
        return;
    };
    if let Some(expected_syntax) = expected_syntax {
        facts.push_expected_syntax(EditorExpectedSyntax::new(expected_syntax, field.span()));
    }
    facts.push_symbol(EditorSemanticSymbol::new(
        field.text,
        Some(detail.to_string()),
        kind,
        statement_span,
        field.span(),
    ));
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
struct SpannedText<'a> {
    text: &'a str,
    start: usize,
    end: usize,
}

impl<'a> SpannedText<'a> {
    fn trim(self) -> Option<Self> {
        let leading = self.text.len().saturating_sub(self.text.trim_start().len());
        let text = &self.text[leading..];
        let trimmed_len = text.trim_end().len();
        if trimmed_len == 0 {
            return None;
        }

        Some(Self {
            text: &text[..trimmed_len],
            start: self.start + leading,
            end: self.start + leading + trimmed_len,
        })
    }

    fn span(self) -> SourceSpan {
        SourceSpan::new(self.start, self.end)
    }
}

pub fn parse_gantt(code: &str, meta: &ParseMetadata) -> Result<Value> {
    let Some(db) = parse_gantt_db(code, meta)? else {
        return Ok(json!({}));
    };
    gantt_db_to_json(db, meta)
}

pub fn parse_gantt_model_for_render(
    code: &str,
    meta: &ParseMetadata,
) -> Result<GanttDiagramRenderModel> {
    let Some(mut db) = parse_gantt_db(code, meta)? else {
        return Ok(GanttDiagramRenderModel::default());
    };
    gantt_db_to_render_model(&mut db)
}

fn parse_gantt_db(code: &str, meta: &ParseMetadata) -> Result<Option<GanttDb>> {
    let mut db = GanttDb::default();
    db.clear();
    db.set_security_level(meta.effective_config.get_str("securityLevel"));
    if let Some(dm) = meta.effective_config.get_str("gantt.displayMode") {
        db.set_display_mode(dm);
    }

    let mut cursor = GanttLineCursor::new(code);
    let mut header_seen = false;

    while let Some((line, line_start)) = cursor.next_line() {
        let stripped = strip_inline_comment(line);
        let trimmed = stripped.trim();
        if trimmed.is_empty() {
            continue;
        }

        if !header_seen {
            if starts_with_case_insensitive(trimmed, "gantt") {
                header_seen = true;
                let rest = trimmed["gantt".len()..].trim_start();
                if !rest.is_empty() {
                    let rest_offset = trimmed["gantt".len()..].len() - rest.len();
                    let trimmed_start = stripped.find(trimmed).unwrap_or(0);
                    parse_gantt_statement(
                        rest,
                        line_start + trimmed_start + "gantt".len() + rest_offset,
                        &mut db,
                        &mut cursor,
                    )?;
                }
                continue;
            }
            return Err(Error::diagram_parse_fallback(
                "gantt".to_string(),
                "expected gantt header".to_string(),
            ));
        }

        parse_gantt_statement(stripped, line_start, &mut db, &mut cursor)?;
    }

    if !header_seen {
        return Ok(None);
    }

    Ok(Some(db))
}

fn gantt_db_to_json(mut db: GanttDb, meta: &ParseMetadata) -> Result<Value> {
    let tasks = db.get_tasks()?;
    let tasks_json: Vec<Value> = tasks
        .into_iter()
        .map(|t| {
            let start_ms = t.start_time.map(|d| d.timestamp_millis());
            let end_ms = t.end_time.map(|d| d.timestamp_millis());
            let render_end_ms = t.render_end_time.map(|d| d.timestamp_millis());
            let raw_start = match &t.raw.start_time {
                StartTimeRaw::PrevTaskEnd => json!({ "type": "prevTaskEnd", "id": t.prev_task_id }),
                StartTimeRaw::GetStartDate { start_data } => {
                    json!({ "type": "getStartDate", "startData": start_data })
                }
            };
            json!({
                "section": t.section,
                "type": t.type_,
                "task": t.task,
                "id": t.id,
                "prevTaskId": t.prev_task_id,
                "order": t.order,
                "processed": t.processed,
                "classes": t.classes,
                "active": t.active,
                "done": t.done,
                "crit": t.crit,
                "milestone": t.milestone,
                "vert": t.vert,
                "manualEndTime": t.manual_end_time,
                "renderEndTime": render_end_ms,
                "raw": {
                    "data": t.raw.data,
                    "startTime": raw_start,
                    "endTime": { "data": t.raw.end_data },
                },
                "startTime": start_ms,
                "endTime": end_ms,
            })
        })
        .collect();

    Ok(json!({
        "type": meta.diagram_type,
        "title": if db.diagram_title.is_empty() { None::<String> } else { Some(db.diagram_title) },
        "accTitle": if db.acc_title.is_empty() { None::<String> } else { Some(db.acc_title) },
        "accDescr": if db.acc_descr.is_empty() { None::<String> } else { Some(db.acc_descr) },
        "dateFormat": db.date_format,
        "axisFormat": db.axis_format,
        "tickInterval": db.tick_interval,
        "todayMarker": db.today_marker,
        "includes": db.includes,
        "excludes": db.excludes,
        "inclusiveEndDates": db.inclusive_end_dates,
        "topAxis": db.top_axis,
        "weekday": db.weekday,
        "weekend": db.weekend,
        "displayMode": db.display_mode,
        "sections": db.sections,
        "tasks": tasks_json,
        "links": db.links,
        "clickEvents": db.click_events,
    }))
}

fn gantt_db_to_render_model(db: &mut GanttDb) -> Result<GanttDiagramRenderModel> {
    let tasks = db
        .get_tasks()?
        .into_iter()
        .map(raw_task_to_render_task)
        .collect::<Result<Vec<_>>>()?;

    Ok(GanttDiagramRenderModel {
        title: non_empty_opt(std::mem::take(&mut db.diagram_title)),
        acc_title: non_empty_opt(std::mem::take(&mut db.acc_title)),
        acc_descr: non_empty_opt(std::mem::take(&mut db.acc_descr)),
        date_format: std::mem::take(&mut db.date_format),
        axis_format: std::mem::take(&mut db.axis_format),
        tick_interval: db.tick_interval.take(),
        today_marker: std::mem::take(&mut db.today_marker),
        includes: std::mem::take(&mut db.includes),
        excludes: std::mem::take(&mut db.excludes),
        display_mode: std::mem::take(&mut db.display_mode),
        top_axis: db.top_axis,
        weekday: std::mem::take(&mut db.weekday),
        weekend: std::mem::take(&mut db.weekend),
        tasks,
    })
}

fn non_empty_opt(value: String) -> Option<String> {
    if value.is_empty() { None } else { Some(value) }
}

fn raw_task_to_render_task(t: RawTask) -> Result<GanttRenderTask> {
    let start_ms = task_time_ms(&t, "startTime", t.start_time)?;
    let end_ms = task_time_ms(&t, "endTime", t.end_time)?;

    Ok(GanttRenderTask {
        id: t.id,
        task: t.task,
        section: t.section,
        task_type: t.type_,
        classes: t.classes,
        active: t.active,
        done: t.done,
        crit: t.crit,
        milestone: t.milestone,
        vert: t.vert,
        order: t.order,
        start_ms,
        end_ms,
        render_end_ms: t.render_end_time.map(|d| d.timestamp_millis()),
    })
}

fn task_time_ms(task: &RawTask, field: &str, value: Option<DateTimeFixed>) -> Result<i64> {
    value.map(|d| d.timestamp_millis()).ok_or_else(|| {
        Error::diagram_parse_fallback(
            "gantt".to_string(),
            format!("task `{}` has unresolved {field}", task.id),
        )
    })
}

fn parse_gantt_statement(
    line: &str,
    line_start: usize,
    db: &mut GanttDb,
    cursor: &mut GanttLineCursor<'_>,
) -> Result<()> {
    let stripped = strip_inline_comment(line);
    let t = stripped.trim();
    if t.is_empty() {
        return Ok(());
    }

    if let Some(v) = parse_keyword_arg(stripped, "dateFormat") {
        db.set_date_format(v);
        return Ok(());
    }
    if starts_with_case_insensitive(t, "inclusiveEndDates") {
        db.enable_inclusive_end_dates();
        return Ok(());
    }
    if starts_with_case_insensitive(t, "topAxis") {
        db.enable_top_axis();
        return Ok(());
    }
    if let Some(v) = parse_keyword_arg(stripped, "axisFormat") {
        db.set_axis_format(v);
        return Ok(());
    }
    if let Some(v) = parse_keyword_arg(stripped, "tickInterval") {
        db.set_tick_interval(v.trim());
        return Ok(());
    }
    if let Some(v) = parse_keyword_arg(stripped, "includes") {
        db.set_includes(v);
        return Ok(());
    }
    if let Some(v) = parse_keyword_arg(stripped, "excludes") {
        db.set_excludes(v);
        return Ok(());
    }
    if let Some(v) = parse_keyword_arg_full_line(stripped, "todayMarker") {
        db.set_today_marker(v.trim());
        return Ok(());
    }
    if let Some(v) = parse_gantt_keyword_arg_spanned(stripped, line_start, "weekday", false) {
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
            return Err(Error::diagram_parse_exact(
                "gantt".to_string(),
                format!("invalid weekday: {day}"),
                span,
            ));
        }
        db.set_weekday(&day);
        return Ok(());
    }
    if let Some(v) = parse_gantt_keyword_arg_spanned(stripped, line_start, "weekend", false) {
        let trimmed_day = v.trim();
        let day = trimmed_day
            .map(|value| value.text.to_lowercase())
            .unwrap_or_default();
        if !matches!(day.as_str(), "friday" | "saturday") {
            let span = trimmed_day
                .map(SpannedText::span)
                .unwrap_or_else(|| SourceSpan::new(v.start, v.start));
            return Err(Error::diagram_parse_exact(
                "gantt".to_string(),
                format!("invalid weekend: {day}"),
                span,
            ));
        }
        db.set_weekend(&day);
        return Ok(());
    }
    if let Some(v) = parse_keyword_arg_full_line(stripped, "title") {
        db.set_diagram_title(v.trim());
        return Ok(());
    }
    if let Some(v) = parse_keyword_arg_full_line(stripped, "section") {
        db.add_section(v.trim());
        return Ok(());
    }
    if let Some(v) = parse_key_colon_value(stripped, "accTitle") {
        db.set_acc_title(&v);
        return Ok(());
    }
    if let Some(v) = parse_key_colon_value(stripped, "accDescr") {
        db.set_acc_descr(&v);
        return Ok(());
    }
    if let Some(v) = parse_acc_descr_block(cursor, stripped) {
        db.set_acc_descr(&v);
        return Ok(());
    }
    match parse_click_statement(stripped, 0) {
        Ok(Some(click)) => {
            if let Some(call) = click.call {
                db.set_click_event(
                    &click.ids.text,
                    call.name.text.trim(),
                    call.args.map(|args| args.text),
                );
            }
            if let Some(href) = click.href {
                db.set_link(&click.ids.text, href.text);
            }
            return Ok(());
        }
        Ok(None) => {}
        Err(message) => {
            return Err(Error::diagram_parse_exact(
                "gantt".to_string(),
                message,
                gantt_statement_span(stripped, 0),
            ));
        }
    }

    let task_stmt = stripped.trim_start();

    let Some(colon) = task_stmt.find(':') else {
        return Err(Error::diagram_parse_fallback(
            "gantt".to_string(),
            format!("unrecognized statement: {t}"),
        ));
    };

    // Mermaid passes `taskTxt` through to the DB without trimming. This preserves any trailing
    // whitespace before the `:` delimiter (e.g. `Task1 :id,...` yields `Task1 `).
    let task_txt = &task_stmt[..colon];
    let mut task_data = task_stmt[colon + 1..].to_string();
    task_data = split_statement_suffix(&task_data).to_string();
    if task_txt.is_empty() || task_data.trim().is_empty() {
        return Ok(());
    }
    db.add_task(task_txt, &format!(":{task_data}"));
    Ok(())
}
