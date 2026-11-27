//! Source-backed Langium strings and shared Mermaid declarations.

use merman_model::{SourceSpan, text::is_ecmascript_whitespace};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LangiumString {
    pub value: String,
    pub raw_span: SourceSpan,
    pub value_span: SourceSpan,
    pub consumed: usize,
}

pub fn is_ecmascript_line_terminator(ch: char) -> bool {
    matches!(ch, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}

pub fn convert_langium_escape_character(ch: char) -> char {
    match ch {
        'b' => '\u{0008}',
        'f' => '\u{000c}',
        'n' => '\n',
        'r' => '\r',
        't' => '\t',
        'v' => '\u{000b}',
        '0' => '\0',
        other => other,
    }
}

/// Parses the shared Langium `STRING` terminal and applies `DefaultValueConverter` unescaping.
pub fn parse_langium_string(input: &str, input_start: usize) -> Option<LangiumString> {
    let mut chars = input.char_indices();
    let (_, quote) = chars.next()?;
    if !matches!(quote, '"' | '\'') {
        return None;
    }

    let mut value = String::new();
    let mut escaped = false;
    for (index, ch) in chars {
        if escaped {
            if is_ecmascript_line_terminator(ch) {
                return None;
            }
            value.push(convert_langium_escape_character(ch));
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == quote {
            let consumed = index + ch.len_utf8();
            let raw_end = input_start.checked_add(consumed)?;
            return Some(LangiumString {
                value,
                raw_span: SourceSpan::new(input_start, raw_end),
                value_span: SourceSpan::new(input_start + quote.len_utf8(), input_start + index),
                consumed,
            });
        }
        value.push(ch);
    }
    None
}

/// Returns the visible prefix before a Langium `SINGLE_LINE_COMMENT` outside a STRING token.
pub fn strip_langium_inline_comment(line: &str) -> &str {
    let mut quote: Option<char> = None;
    let mut escaped = false;
    let mut chars = line.char_indices().peekable();
    while let Some((index, ch)) = chars.next() {
        if escaped {
            escaped = false;
            continue;
        }
        if quote.is_some() && ch == '\\' {
            escaped = true;
            continue;
        }
        if matches!(ch, '"' | '\'') {
            match quote {
                Some(open) if open == ch => quote = None,
                None => quote = Some(ch),
                _ => {}
            }
            continue;
        }
        if quote.is_none() && ch == '%' && chars.peek().is_some_and(|(_, next)| *next == '%') {
            return &line[..index];
        }
    }
    line
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LangiumCommonField {
    Title,
    AccTitle,
    AccDescr,
}

impl LangiumCommonField {
    pub fn directive(self) -> &'static str {
        match self {
            Self::Title => "title",
            Self::AccTitle => "accTitle",
            Self::AccDescr => "accDescr",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LangiumCommonFact {
    pub field: LangiumCommonField,
    pub value: String,
    /// Span of the complete common terminal, including its leading horizontal whitespace.
    pub raw_span: SourceSpan,
    /// Span of the unnormalized payload after trimming its outer whitespace.
    pub value_span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LangiumCommonDiagnostic {
    pub message: String,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LangiumCommonParse {
    pub fact: LangiumCommonFact,
    /// Bytes consumed from the supplied offset, including the required EOL or EOF.
    pub consumed: usize,
    /// Last whitespace-only physical line consumed as trailing EOL trivia, if any.
    pub trailing_whitespace_span: Option<SourceSpan>,
    pub diagnostic: Option<LangiumCommonDiagnostic>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LangiumCommonFacts {
    ordered: Vec<LangiumCommonFact>,
}

impl LangiumCommonFacts {
    pub fn push(&mut self, fact: LangiumCommonFact) {
        self.ordered.push(fact);
    }

    pub fn iter(&self) -> impl Iterator<Item = &LangiumCommonFact> {
        self.ordered.iter()
    }

    #[cfg(test)]
    fn ordered(&self) -> &[LangiumCommonFact] {
        &self.ordered
    }

    pub fn last(&self, field: LangiumCommonField) -> Option<&LangiumCommonFact> {
        self.ordered.iter().rev().find(|fact| fact.field == field)
    }
}

/// Final nonempty CommonDB assignments from a family’s ordered Langium facts.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LangiumCommonDbFields {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
}

impl LangiumCommonDbFields {
    pub fn from_facts(facts: &LangiumCommonFacts) -> Self {
        // `populateCommonDb` observes the final AST assignments in this order and ignores empty
        // values. Looking up each field independently also preserves Langium's last-assignment
        // semantics when a common fragment appears more than once.
        let acc_descr = nonempty_last(facts, LangiumCommonField::AccDescr);
        let acc_title = nonempty_last(facts, LangiumCommonField::AccTitle);
        let title = nonempty_last(facts, LangiumCommonField::Title);
        Self {
            title,
            acc_title,
            acc_descr,
        }
    }
}

fn nonempty_last(facts: &LangiumCommonFacts, field: LangiumCommonField) -> Option<String> {
    facts
        .last(field)
        .map(|fact| fact.value.clone())
        .filter(|value| !value.is_empty())
}

pub fn parse_langium_common(source: &str, offset: usize) -> Option<LangiumCommonParse> {
    let input = source.get(offset..)?;
    let leading = horizontal_whitespace_len(input);
    let token_start = offset + leading;
    let token = source.get(token_start..)?;

    if token.starts_with("accDescr") {
        return parse_acc_descr(source, offset, token_start);
    }
    if token.starts_with("accTitle") {
        return parse_acc_title(source, offset, token_start);
    }
    if token.starts_with("title") {
        return parse_title(source, offset, token_start);
    }
    None
}

fn parse_title(source: &str, offset: usize, token_start: usize) -> Option<LangiumCommonParse> {
    let keyword_end = token_start + "title".len();
    let next = source.get(keyword_end..)?.chars().next();
    let capture_start = keyword_end;
    let raw_end = if next.is_some_and(is_horizontal_whitespace) {
        inline_terminal_end(source, capture_start)
    } else {
        keyword_end
    };
    parsed_inline(
        source,
        offset,
        raw_end,
        LangiumCommonField::Title,
        capture_start,
    )
}

fn parse_acc_title(source: &str, offset: usize, token_start: usize) -> Option<LangiumCommonParse> {
    let keyword_end = token_start + "accTitle".len();
    let colon = keyword_end + horizontal_whitespace_len(source.get(keyword_end..)?);
    if source.as_bytes().get(colon) != Some(&b':') {
        return None;
    }
    let capture_start = colon + 1;
    let raw_end = inline_terminal_end(source, capture_start);
    parsed_inline(
        source,
        offset,
        raw_end,
        LangiumCommonField::AccTitle,
        capture_start,
    )
}

fn parse_acc_descr(source: &str, offset: usize, token_start: usize) -> Option<LangiumCommonParse> {
    let keyword_end = token_start + "accDescr".len();
    let horizontal_end = keyword_end + horizontal_whitespace_len(source.get(keyword_end..)?);
    if source.as_bytes().get(horizontal_end) == Some(&b':') {
        let capture_start = horizontal_end + 1;
        let raw_end = inline_terminal_end(source, capture_start);
        return parsed_inline(
            source,
            offset,
            raw_end,
            LangiumCommonField::AccDescr,
            capture_start,
        );
    }

    let opening = skip_ecmascript_whitespace(source, keyword_end);
    if source.as_bytes().get(opening) != Some(&b'{') {
        return None;
    }
    let content_start = opening + 1;
    let Some(relative_close) = source.get(content_start..)?.find('}') else {
        let raw_end = source.len();
        let (value, value_span) = normalized_block_value(source, content_start, raw_end);
        return Some(LangiumCommonParse {
            fact: LangiumCommonFact {
                field: LangiumCommonField::AccDescr,
                value,
                raw_span: SourceSpan::new(offset, raw_end),
                value_span,
            },
            consumed: raw_end - offset,
            trailing_whitespace_span: None,
            diagnostic: Some(LangiumCommonDiagnostic {
                message: "unterminated accDescr block; expected `}`".to_string(),
                span: SourceSpan::new(raw_end, raw_end),
            }),
        });
    };

    let close = content_start + relative_close;
    let raw_end = close + 1;
    let eol = consume_required_eol(source, raw_end)?;
    let (value, value_span) = normalized_block_value(source, content_start, close);
    Some(LangiumCommonParse {
        fact: LangiumCommonFact {
            field: LangiumCommonField::AccDescr,
            value,
            raw_span: SourceSpan::new(offset, raw_end),
            value_span,
        },
        consumed: eol.end - offset,
        trailing_whitespace_span: eol.trailing_whitespace_span,
        diagnostic: None,
    })
}

fn parsed_inline(
    source: &str,
    offset: usize,
    raw_end: usize,
    field: LangiumCommonField,
    capture_start: usize,
) -> Option<LangiumCommonParse> {
    let eol = consume_required_eol(source, raw_end)?;
    let capture = source.get(capture_start..raw_end)?;
    let trimmed = capture.trim_matches(is_ecmascript_whitespace);
    let leading = capture.len() - capture.trim_start_matches(is_ecmascript_whitespace).len();
    let value_start = capture_start + leading;
    let value_span = SourceSpan::new(value_start, value_start + trimmed.len());
    Some(LangiumCommonParse {
        fact: LangiumCommonFact {
            field,
            value: collapse_horizontal_runs(trimmed),
            raw_span: SourceSpan::new(offset, raw_end),
            value_span,
        },
        consumed: eol.end - offset,
        trailing_whitespace_span: eol.trailing_whitespace_span,
        diagnostic: None,
    })
}

fn inline_terminal_end(source: &str, start: usize) -> usize {
    let bytes = source.as_bytes();
    let mut cursor = start;
    while cursor < bytes.len() {
        if matches!(bytes[cursor], b'\n' | b'\r') || bytes.get(cursor..cursor + 2) == Some(b"%%") {
            break;
        }
        cursor += 1;
    }
    cursor
}

fn single_line_comment_end(source: &str, start: usize) -> usize {
    source[start..]
        .find(['\r', '\n'])
        .map_or(source.len(), |relative| start + relative)
}

struct ConsumedEol {
    end: usize,
    trailing_whitespace_span: Option<SourceSpan>,
}

fn consume_required_eol(source: &str, raw_end: usize) -> Option<ConsumedEol> {
    let mut cursor = raw_end + horizontal_whitespace_len(source.get(raw_end..)?);
    if source.get(cursor..)?.starts_with("%%") {
        cursor = single_line_comment_end(source, cursor + 2);
    }
    if cursor == source.len() {
        return Some(ConsumedEol {
            end: cursor,
            trailing_whitespace_span: None,
        });
    }
    cursor = consume_newline(source, cursor)?;
    let mut trailing_whitespace_span = None;

    loop {
        let trivia_start = cursor;
        let mut next = cursor + horizontal_whitespace_len(source.get(cursor..)?);
        let comment = source.get(next..)?.starts_with("%%");
        if comment {
            next = single_line_comment_end(source, next + 2);
        } else if next > trivia_start
            && (next == source.len() || consume_newline(source, next).is_some())
        {
            trailing_whitespace_span = Some(SourceSpan::new(trivia_start, next));
        }
        if next == source.len() {
            return Some(ConsumedEol {
                end: next,
                trailing_whitespace_span,
            });
        }
        if let Some(after_newline) = consume_newline(source, next) {
            cursor = after_newline;
            continue;
        }
        return Some(ConsumedEol {
            end: trivia_start,
            trailing_whitespace_span,
        });
    }
}

fn consume_newline(source: &str, offset: usize) -> Option<usize> {
    if source.get(offset..)?.starts_with("\r\n") {
        return Some(offset + 2);
    }
    (source.as_bytes().get(offset) == Some(&b'\n')).then_some(offset + 1)
}

fn horizontal_whitespace_len(input: &str) -> usize {
    input
        .chars()
        .take_while(|ch| is_horizontal_whitespace(*ch))
        .map(char::len_utf8)
        .sum()
}

fn is_horizontal_whitespace(ch: char) -> bool {
    matches!(ch, ' ' | '\t')
}

fn skip_ecmascript_whitespace(source: &str, start: usize) -> usize {
    let mut cursor = start;
    let Some(rest) = source.get(start..) else {
        return cursor;
    };
    for ch in rest.chars() {
        if !is_ecmascript_whitespace(ch) {
            break;
        }
        cursor += ch.len_utf8();
    }
    cursor
}

fn normalized_block_value(source: &str, start: usize, end: usize) -> (String, SourceSpan) {
    let raw = &source[start..end];
    let leading = raw.len() - raw.trim_start_matches(is_ecmascript_whitespace).len();
    let trimmed = raw.trim_matches(is_ecmascript_whitespace);
    let value_start = start + leading;
    let value_span = SourceSpan::new(value_start, value_start + trimmed.len());
    let mut value = String::with_capacity(trimmed.len());
    let mut content_end = 0;
    let mut horizontal = None;
    let mut after_line_break = false;
    for ch in trimmed.chars() {
        if !is_ecmascript_whitespace(ch) {
            if let Some(pending) = horizontal.take() {
                value.push(pending);
            }
            value.push(ch);
            content_end = value.len();
            after_line_break = false;
            continue;
        }
        if after_line_break {
            continue;
        }
        if is_ecmascript_line_terminator(ch) {
            value.truncate(content_end);
            value.push(if ch == '\r' { '\n' } else { ch });
            horizontal = None;
            after_line_break = true;
        } else if is_horizontal_whitespace(ch) {
            horizontal = Some(if horizontal.is_some() { ' ' } else { ch });
        } else {
            if let Some(pending) = horizontal.take() {
                value.push(pending);
            }
            value.push(ch);
        }
    }
    (value, value_span)
}

fn collapse_horizontal_runs(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut horizontal = String::new();
    for ch in value.chars() {
        if is_horizontal_whitespace(ch) {
            horizontal.push(ch);
            continue;
        }
        if horizontal.len() >= 2 {
            output.push(' ');
        } else {
            output.push_str(&horizontal);
        }
        horizontal.clear();
        output.push(ch);
    }
    if horizontal.len() >= 2 {
        output.push(' ');
    } else {
        output.push_str(&horizontal);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn langium_string_rejects_spans_past_the_byte_index_range() {
        assert!(parse_langium_string("\"\"", usize::MAX).is_none());
        let parsed = parse_langium_string("\"\"", usize::MAX - 2).unwrap();
        assert_eq!(parsed.raw_span, SourceSpan::new(usize::MAX - 2, usize::MAX));
    }

    #[test]
    fn langium_string_decodes_the_selected_value_converter_escape_characters() {
        for (source, expected) in [
            ("\"\\b\"", "\u{0008}"),
            ("\"\\f\"", "\u{000c}"),
            ("\"\\n\"", "\n"),
            ("\"\\r\"", "\r"),
            ("\"\\t\"", "\t"),
            ("\"\\v\"", "\u{000b}"),
            ("\"\\0\"", "\0"),
            ("\"\\u0041\\x41\\9\"", "u0041x419"),
            ("\"\\\"\"", "\""),
            ("\"\\\\\"", "\\"),
            ("'\\''", "'"),
        ] {
            let parsed = parse_langium_string(source, 10)
                .unwrap_or_else(|| panic!("STRING terminal did not parse: {source:?}"));
            assert_eq!(parsed.value, expected, "source: {source:?}");
            assert_eq!(parsed.consumed, source.len(), "source: {source:?}");
            assert_eq!(
                parsed.raw_span,
                SourceSpan::new(10, 10 + source.len()),
                "source: {source:?}"
            );
            assert_eq!(
                parsed.value_span,
                SourceSpan::new(11, 9 + source.len()),
                "source: {source:?}"
            );
        }
    }

    #[test]
    fn langium_string_rejects_escaped_ecmascript_line_terminators() {
        for line_terminator in ['\n', '\r', '\u{2028}', '\u{2029}'] {
            let source = format!("\"before\\{line_terminator}after\"");
            assert_eq!(
                parse_langium_string(&source, 0),
                None,
                "escaped line terminator: {line_terminator:?}"
            );
        }
    }

    #[test]
    fn common_terminal_conformance_matrix() {
        struct Case {
            source: &'static str,
            offset: usize,
            field: LangiumCommonField,
            value: &'static str,
            raw: &'static str,
            value_source: &'static str,
            remainder: &'static str,
        }

        let cases = [
            Case {
                source: "packet title  Packet   map %% note\r\n0: \"bit\"\r\n",
                offset: "packet".len(),
                field: LangiumCommonField::Title,
                value: "Packet map",
                raw: " title  Packet   map ",
                value_source: "Packet   map",
                remainder: "0: \"bit\"\r\n",
            },
            Case {
                source: "\taccTitle:\t Accessible   packet\nnext",
                offset: 0,
                field: LangiumCommonField::AccTitle,
                value: "Accessible packet",
                raw: "\taccTitle:\t Accessible   packet",
                value_source: "Accessible   packet",
                remainder: "next",
            },
            Case {
                source: "accDescr: value before comment %% hidden\nnext",
                offset: 0,
                field: LangiumCommonField::AccDescr,
                value: "value before comment",
                raw: "accDescr: value before comment ",
                value_source: "value before comment",
                remainder: "next",
            },
            Case {
                source: "accDescr { same   line } \t%% hidden\r\nnext",
                offset: 0,
                field: LangiumCommonField::AccDescr,
                value: "same line",
                raw: "accDescr { same   line }",
                value_source: "same   line",
                remainder: "next",
            },
            Case {
                source: "accDescr\r\n{\r\n  First   line\r\n\r\n\tSecond\t\tline  \r\n}\r\nnext",
                offset: 0,
                field: LangiumCommonField::AccDescr,
                value: "First line\nSecond line",
                raw: "accDescr\r\n{\r\n  First   line\r\n\r\n\tSecond\t\tline  \r\n}",
                value_source: "First   line\r\n\r\n\tSecond\t\tline",
                remainder: "next",
            },
            Case {
                source: "accDescr { text %% is payload }\n",
                offset: 0,
                field: LangiumCommonField::AccDescr,
                value: "text %% is payload",
                raw: "accDescr { text %% is payload }",
                value_source: "text %% is payload",
                remainder: "",
            },
            Case {
                source: "title alpha\tbeta\n\n  %% hidden blank\r\n\r\nnext",
                offset: 0,
                field: LangiumCommonField::Title,
                value: "alpha\tbeta",
                raw: "title alpha\tbeta",
                value_source: "alpha\tbeta",
                remainder: "next",
            },
            Case {
                source: "accDescr {\n  alpha\tbeta\n\n  gamma\t\tdelta\n}\n",
                offset: 0,
                field: LangiumCommonField::AccDescr,
                value: "alpha\tbeta\ngamma delta",
                raw: "accDescr {\n  alpha\tbeta\n\n  gamma\t\tdelta\n}",
                value_source: "alpha\tbeta\n\n  gamma\t\tdelta",
                remainder: "",
            },
            Case {
                source: "title\n",
                offset: 0,
                field: LangiumCommonField::Title,
                value: "",
                raw: "title",
                value_source: "",
                remainder: "",
            },
            Case {
                source: "accTitle:%% comment\n",
                offset: 0,
                field: LangiumCommonField::AccTitle,
                value: "",
                raw: "accTitle:",
                value_source: "",
                remainder: "",
            },
        ];

        for case in cases {
            let parsed = parse_langium_common(case.source, case.offset)
                .unwrap_or_else(|| panic!("common terminal did not parse: {:?}", case.source));
            assert_eq!(parsed.fact.field, case.field, "source: {:?}", case.source);
            assert_eq!(parsed.fact.value, case.value, "source: {:?}", case.source);
            assert_eq!(
                &case.source[parsed.fact.raw_span.start..parsed.fact.raw_span.end],
                case.raw,
                "source: {:?}",
                case.source
            );
            assert_eq!(
                &case.source[parsed.fact.value_span.start..parsed.fact.value_span.end],
                case.value_source,
                "source: {:?}",
                case.source
            );
            assert_eq!(
                &case.source[case.offset + parsed.consumed..],
                case.remainder,
                "source: {:?}",
                case.source
            );
            assert!(parsed.diagnostic.is_none());
        }
    }

    #[test]
    fn common_comments_preserve_exact_terminal_and_trailing_trivia_ranges() {
        let prefix = "前\r\n";
        let comment = "%% first %% \"unclosed ' %% final";
        for (field, raw, value_source, value) in [
            (LangiumCommonField::Title, "\ttitle  α   β ", "α   β", "α β"),
            (
                LangiumCommonField::AccTitle,
                "\taccTitle:  α   β ",
                "α   β",
                "α β",
            ),
            (
                LangiumCommonField::AccDescr,
                "\taccDescr:  α   β ",
                "α   β",
                "α β",
            ),
            (
                LangiumCommonField::AccDescr,
                "\taccDescr { α   β }",
                "α   β",
                "α β",
            ),
            (
                LangiumCommonField::AccDescr,
                "\taccDescr { α %% \"literal %% β }",
                "α %% \"literal %% β",
                "α %% \"literal %% β",
            ),
        ] {
            for newline in ["\n", "\r\n"] {
                for (suffix, remainder, blank_start) in [
                    (comment.to_string(), "", None),
                    (format!("{comment}{newline}"), "next", None),
                    (
                        format!("{newline} \t{newline}\t{comment}{newline}"),
                        "next",
                        Some(newline.len()),
                    ),
                    (
                        format!("{newline} \t{newline}\t{comment}"),
                        "",
                        Some(newline.len()),
                    ),
                    (
                        format!("{comment}{newline} \t{newline}\t{comment}{newline}"),
                        "next",
                        Some(comment.len() + newline.len()),
                    ),
                ] {
                    let source = format!("{prefix}{raw}{suffix}{remainder}");
                    let parsed = parse_langium_common(&source, prefix.len())
                        .unwrap_or_else(|| panic!("comment did not parse: {source:?}"));
                    let value_start = prefix.len() + raw.find(value_source).unwrap();
                    let trailing = blank_start.map(|relative| {
                        let start = prefix.len() + raw.len() + relative;
                        SourceSpan::new(start, start + " \t".len())
                    });

                    assert_eq!(parsed.fact.field, field, "source: {source:?}");
                    assert_eq!(parsed.fact.value, value, "source: {source:?}");
                    assert_eq!(
                        parsed.consumed,
                        raw.len() + suffix.len(),
                        "source: {source:?}"
                    );
                    assert_eq!(
                        parsed.fact.raw_span,
                        SourceSpan::new(prefix.len(), prefix.len() + raw.len()),
                        "source: {source:?}"
                    );
                    assert_eq!(
                        parsed.fact.value_span,
                        SourceSpan::new(value_start, value_start + value_source.len()),
                        "source: {source:?}"
                    );
                    assert_eq!(
                        parsed.trailing_whitespace_span, trailing,
                        "source: {source:?}"
                    );
                    assert_eq!(&source[prefix.len() + parsed.consumed..], remainder);
                    assert!(parsed.diagnostic.is_none(), "source: {source:?}");
                }
            }
        }
    }

    #[test]
    fn common_values_use_ecmascript_trim_with_original_payload_ranges() {
        let prefix = "前\r\n";
        for (field, opening, closing) in [
            (LangiumCommonField::Title, "\ttitle ", ""),
            (LangiumCommonField::AccTitle, "\taccTitle:", ""),
            (LangiumCommonField::AccDescr, "\taccDescr:", ""),
            (LangiumCommonField::AccDescr, "\taccDescr {", "}"),
        ] {
            for (payload, expected) in [
                ("\u{0085}Value\u{0085}", "\u{0085}Value\u{0085}"),
                ("\u{feff}Value\u{feff}", "Value"),
                (
                    "\u{feff}\u{0085}Value\u{0085}\u{feff}",
                    "\u{0085}Value\u{0085}",
                ),
                ("\u{0085}", "\u{0085}"),
            ] {
                let raw = format!("{opening}{payload}{closing}");
                let source = format!("{prefix}{raw}\r\nnext");
                let parsed = parse_langium_common(&source, prefix.len()).unwrap();
                let start = prefix.len() + opening.len() + payload.find(expected).unwrap();

                assert_eq!(parsed.fact.field, field, "source: {source:?}");
                assert_eq!(parsed.fact.value, expected, "source: {source:?}");
                assert_eq!(parsed.consumed, raw.len() + "\r\n".len());
                assert_eq!(
                    parsed.fact.raw_span,
                    SourceSpan::new(prefix.len(), prefix.len() + raw.len())
                );
                assert_eq!(
                    parsed.fact.value_span,
                    SourceSpan::new(start, start + expected.len()),
                    "source: {source:?}"
                );
                assert!(!parsed.fact.value.contains('\u{feff}'));
                assert!(parsed.diagnostic.is_none());
            }
        }
    }

    #[test]
    fn block_description_gap_uses_ecmascript_whitespace() {
        for gap in ["\u{feff}", "\u{feff}\r\n \t"] {
            let raw = format!("\taccDescr{gap}{{ Value }}");
            let source = format!("{raw}\nnext");
            let parsed = parse_langium_common(&source, 0).unwrap();
            let start = source.find("Value").unwrap();

            assert_eq!(parsed.fact.value, "Value");
            assert_eq!(parsed.fact.raw_span, SourceSpan::new(0, raw.len()));
            assert_eq!(parsed.fact.value_span, SourceSpan::new(start, start + 5));
            assert_eq!(parsed.consumed, raw.len() + 1);
            assert!(parsed.diagnostic.is_none());
        }
        for gap in ["\u{0085}", "\u{feff}\u{0085}", "\n\u{0085}\n"] {
            let source = format!("accDescr{gap}{{ Value }}\n");
            assert!(
                parse_langium_common(&source, 0).is_none(),
                "accepted {source:?}"
            );
        }
    }

    #[test]
    fn block_description_values_follow_public_pipeline_line_boundaries() {
        for (payload, value_source, expected) in [
            ("A\u{2028} B", "A\u{2028} B", "A\u{2028}B"),
            ("A \u{2029} B", "A \u{2029} B", "A\u{2029}B"),
            ("A\n\u{2028} B", "A\n\u{2028} B", "A\nB"),
            ("A\u{2028}\n B", "A\u{2028}\n B", "A\u{2028}B"),
            ("A\u{2028}\u{2029} B", "A\u{2028}\u{2029} B", "A\u{2028}B"),
            ("A\r\n B", "A\r\n B", "A\nB"),
            ("A\n\r B", "A\n\r B", "A\nB"),
            ("A\r B", "A\r B", "A\nB"),
            ("A\n\n B", "A\n\n B", "A\nB"),
            ("A\u{2028}\u{2028} B", "A\u{2028}\u{2028} B", "A\u{2028}B"),
            (
                " \u{2028} A\n \u{2029} B\u{2028} ",
                "A\n \u{2029} B",
                "A\nB",
            ),
            ("A\t\tB\u{2028} C", "A\t\tB\u{2028} C", "A B\u{2028}C"),
            ("A\n \u{2028} \n B", "A\n \u{2028} \n B", "A\nB"),
            ("A\n\u{0085}\n B", "A\n\u{0085}\n B", "A\n\u{0085}\nB"),
            (
                "\u{0085}\n Value\n\u{0085}",
                "\u{0085}\n Value\n\u{0085}",
                "\u{0085}\nValue\n\u{0085}",
            ),
            ("\u{feff}\n Value\n\u{feff}", "Value", "Value"),
        ] {
            let raw = format!("accDescr{{{payload}}}");
            let source = format!("{raw}\n");
            let parsed = parse_langium_common(&source, 0).unwrap();
            let start = source.find(value_source).unwrap();

            assert_eq!(parsed.fact.value, expected, "source: {source:?}");
            assert_eq!(parsed.fact.raw_span, SourceSpan::new(0, raw.len()));
            assert_eq!(
                parsed.fact.value_span,
                SourceSpan::new(start, start + value_source.len())
            );
            assert_eq!(parsed.consumed, source.len());
            assert!(parsed.diagnostic.is_none());
        }
    }

    #[test]
    fn common_facts_are_ordered_and_last_assignment_wins() {
        let source = "title First\naccDescr: Description\ntitle Second\n";
        let mut offset = 0usize;
        let mut facts = LangiumCommonFacts::default();
        while offset < source.len() {
            let parsed = parse_langium_common(source, offset).unwrap();
            offset += parsed.consumed;
            facts.push(parsed.fact);
        }

        assert_eq!(facts.ordered().len(), 3);
        assert_eq!(
            facts
                .last(LangiumCommonField::Title)
                .map(|fact| fact.value.as_str()),
            Some("Second")
        );
        assert_eq!(
            facts
                .last(LangiumCommonField::AccDescr)
                .map(|fact| fact.value.as_str()),
            Some("Description")
        );
    }

    #[test]
    fn unterminated_block_returns_partial_fact_and_eof_diagnostic() {
        let source = "accDescr {\r\n  First   line\r\n  Second line";
        let parsed = parse_langium_common(source, 0).unwrap();

        assert!(parsed.diagnostic.is_some());
        assert_eq!(parsed.consumed, source.len());
        assert_eq!(parsed.fact.value, "First line\nSecond line");
        assert_eq!(parsed.fact.raw_span, SourceSpan::new(0, source.len()));
        assert_eq!(
            parsed.fact.value_span,
            SourceSpan::new(source.find("First").unwrap(), source.len())
        );
        assert_eq!(
            parsed.diagnostic.unwrap().span,
            SourceSpan::new(source.len(), source.len())
        );
    }

    #[test]
    fn common_terminals_require_exact_case_and_eol_or_eof() {
        for source in [
            "Title Wrong case\n",
            "accDescription: not a terminal\n",
            "title: not title syntax\n",
            "titlex\n",
            "accDescr { value } trailing\n",
            "%% title hidden\n",
        ] {
            assert!(
                parse_langium_common(source, 0).is_none(),
                "unexpected match for {source:?}"
            );
        }
    }
}
