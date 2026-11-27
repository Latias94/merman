//! QuadrantChart Jison-compatible statement scanning and DB construction.

use merman_core_preprocess::scan::langium::is_ecmascript_line_terminator;
use merman_markup::sanitize::sanitize_text;
use merman_model::families::quadrant_chart::{
    QuadrantChartAxesModel, QuadrantChartDiagramModel, QuadrantChartPointModel,
    QuadrantChartQuadrantsModel, QuadrantChartStyles,
};
use merman_model::{
    MermaidConfig, OperationControl, OperationControlResult, SourceSpan, number::parse_js_number,
    text::is_ecmascript_whitespace,
};
use std::{borrow::Cow, collections::HashMap};

type StatementResult<T> = Result<T, Cow<'static, str>>;

pub enum QuadrantSourceIssue {
    Exact {
        span: SourceSpan,
        message: Cow<'static, str>,
    },
    InsertionPoint {
        offset: usize,
        message: &'static str,
    },
}

impl QuadrantSourceIssue {
    fn at_statement(span: SourceSpan, message: impl Into<Cow<'static, str>>) -> Self {
        Self::Exact {
            span,
            message: message.into(),
        }
    }
}

#[cfg(feature = "editor-facts")]
#[derive(Clone, Copy)]
pub enum QuadrantTextKind {
    Title,
    AccTitle,
    AccDescr,
    XAxis,
    YAxis,
    Quadrant,
}

#[cfg(feature = "editor-facts")]
pub enum QuadrantObservation<'source> {
    Directive(&'static str),
    Text {
        kind: QuadrantTextKind,
        text: &'source str,
        span: SourceSpan,
    },
    ClassDefinition {
        name: &'source str,
        statement: SourceSpan,
        selection: SourceSpan,
    },
    PointLabel {
        text: &'source str,
        statement: SourceSpan,
        selection: SourceSpan,
    },
    ClassSelector {
        name: Option<&'source str>,
        statement: SourceSpan,
        selection: SourceSpan,
    },
}

#[derive(Debug, Default)]
struct QuadrantDb {
    quadrant1_text: String,
    quadrant2_text: String,
    quadrant3_text: String,
    quadrant4_text: String,
    x_axis_left_text: String,
    x_axis_right_text: String,
    y_axis_bottom_text: String,
    y_axis_top_text: String,
    points: Vec<QuadrantChartPointModel>,
    classes: HashMap<String, QuadrantChartStyles>,
}

impl QuadrantDb {
    fn clear(&mut self) {
        *self = Self::default();
    }

    fn set_quadrant_text(&mut self, idx: u8, text: &str, config: &MermaidConfig) {
        let t = sanitize_text(text.trim(), config);
        match idx {
            1 => self.quadrant1_text = t,
            2 => self.quadrant2_text = t,
            3 => self.quadrant3_text = t,
            4 => self.quadrant4_text = t,
            _ => {}
        }
    }

    fn set_x_axis_left(&mut self, text: &str, config: &MermaidConfig) {
        self.x_axis_left_text = sanitize_text(text.trim(), config);
    }

    fn set_x_axis_right(&mut self, text: &str, config: &MermaidConfig) {
        self.x_axis_right_text = sanitize_text(text.trim(), config);
    }

    fn set_y_axis_bottom(&mut self, text: &str, config: &MermaidConfig) {
        self.y_axis_bottom_text = sanitize_text(text.trim(), config);
    }

    fn set_y_axis_top(&mut self, text: &str, config: &MermaidConfig) {
        self.y_axis_top_text = sanitize_text(text.trim(), config);
    }

    fn add_class(&mut self, class_name: &str, styles: &[String]) -> StatementResult<()> {
        let parsed = parse_styles(styles)?;
        self.classes.insert(class_name.to_string(), parsed);
        Ok(())
    }

    fn add_point(
        &mut self,
        text: &str,
        class_name: Option<String>,
        x: f64,
        y: f64,
        styles: &[String],
        config: &MermaidConfig,
    ) -> StatementResult<()> {
        let styles_obj = parse_styles(styles)?;
        let text = sanitize_text(text.trim(), config);
        let p = QuadrantChartPointModel {
            text,
            x,
            y,
            class_name,
            styles: styles_obj,
        };
        self.points.insert(0, p);
        Ok(())
    }
}

fn parse_styles(styles: &[String]) -> StatementResult<QuadrantChartStyles> {
    let mut out = QuadrantChartStyles::default();
    for raw in styles {
        let style = raw.trim();
        if style.is_empty() {
            continue;
        }
        // Mermaid destructures the first two components of split(/\s*:\s*/).
        let mut components = style.split(':');
        let key = components.next().unwrap_or_default().trim();
        let value = components.next().map(str::trim).unwrap_or("undefined");

        match key {
            "radius" => {
                if value.is_empty() || !value.chars().all(|c| c.is_ascii_digit()) {
                    return Err(format!(
                        "value for {key} {value} is invalid, please use a valid number"
                    )
                    .into());
                }
                // The lexer/validator admits only decimal digits, so parsing directly as
                // binary64 has the same rounding and overflow as JavaScript parseInt.
                out.radius = Some(value.parse::<f64>().map_err(|error| error.to_string())?);
            }
            "color" => {
                if !is_valid_hex_code(value) {
                    return Err(format!(
                        "value for {key} {value} is invalid, please use a valid hex code"
                    )
                    .into());
                }
                out.color = Some(value.to_string());
            }
            "stroke-color" => {
                if !is_valid_hex_code(value) {
                    return Err(format!(
                        "value for {key} {value} is invalid, please use a valid hex code"
                    )
                    .into());
                }
                out.stroke_color = Some(value.to_string());
            }
            "stroke-width" => {
                if !is_valid_px(value) {
                    return Err(format!(
                        "value for {key} {value} is invalid, please use a valid number of pixels (eg. 10px)"
                    )
                    .into());
                }
                out.stroke_width = Some(value.to_string());
            }
            _ => {
                return Err(format!("style named {key} is not supported.").into());
            }
        }
    }
    Ok(out)
}

fn is_valid_hex_code(value: &str) -> bool {
    let v = value.strip_prefix('#').unwrap_or(value);
    (v.len() == 3 || v.len() == 6) && v.chars().all(|c| c.is_ascii_hexdigit())
}

fn is_valid_px(value: &str) -> bool {
    let Some(num) = value.strip_suffix("px") else {
        return false;
    };
    !num.is_empty() && num.chars().all(|c| c.is_ascii_digit())
}

fn next_char_at(s: &str, idx: usize) -> Option<char> {
    s.get(idx..)?.chars().next()
}

fn is_axis_delim_at(s: &str, idx: usize) -> Option<(usize, usize)> {
    let bytes = s.as_bytes();
    if idx >= bytes.len() {
        return None;
    }
    if bytes[idx] != b'-' {
        return None;
    }
    let mut j = idx;
    let mut dash_count = 0usize;
    while j < bytes.len() && bytes[j] == b'-' {
        dash_count += 1;
        j += 1;
    }
    if dash_count < 2 {
        return None;
    }
    if j < bytes.len() && bytes[j] == b'>' {
        Some((idx, j + 1))
    } else {
        None
    }
}

#[derive(Debug, Clone, Copy)]
struct SourceSlice<'source> {
    text: &'source str,
    start: usize,
}

impl<'source> SourceSlice<'source> {
    fn new(text: &'source str, start: usize) -> Self {
        Self { text, start }
    }

    fn end(self) -> usize {
        self.start + self.text.len()
    }

    fn span(self) -> SourceSpan {
        SourceSpan::new(self.start, self.end())
    }

    fn trim_start(self) -> Self {
        let text = self.text.trim_start();
        Self::new(text, self.end().saturating_sub(text.len()))
    }

    fn trim_ecmascript_start(self) -> Self {
        let text = self.text.trim_start_matches(is_ecmascript_whitespace);
        Self::new(text, self.end().saturating_sub(text.len()))
    }

    fn trim_end(self) -> Self {
        Self::new(self.text.trim_end(), self.start)
    }

    fn trim_ecmascript_end(self) -> Self {
        Self::new(
            self.text.trim_end_matches(is_ecmascript_whitespace),
            self.start,
        )
    }

    fn trim(self) -> Self {
        self.trim_start().trim_end()
    }

    fn trim_ascii(self) -> Self {
        let leading = self.text.len() - self.text.trim_ascii_start().len();
        Self::new(self.text.trim_ascii(), self.start + leading)
    }

    fn subslice(self, start: usize, end: usize) -> Self {
        Self::new(&self.text[start..end], self.start + start)
    }
}

#[derive(Debug, Clone)]
struct ParsedText<'source> {
    value: String,
    #[cfg_attr(
        not(feature = "editor-facts"),
        allow(
            dead_code,
            reason = "The shared parsed text retains its source slice for optional editor facts."
        )
    )]
    content: SourceSlice<'source>,
}

fn is_quadrant_text_suffix(suffix: &str) -> bool {
    let mut chars = suffix.char_indices().peekable();
    while let Some((index, ch)) = chars.next() {
        if ch.is_ascii_alphabetic() {
            let rest = &suffix[index..];
            if [
                "title",
                "x-axis",
                "y-axis",
                "quadrant-1",
                "quadrant-2",
                "quadrant-3",
                "quadrant-4",
                "classDef",
                "quadrantChart",
            ]
            .iter()
            .any(|keyword| {
                rest.get(..keyword.len())
                    .is_some_and(|prefix| prefix.eq_ignore_ascii_case(keyword))
            }) {
                return false;
            }
            while chars.peek().is_some_and(|(_, ch)| ch.is_ascii_alphabetic()) {
                chars.next();
            }
        } else if !ch.is_ascii() {
            while chars.peek().is_some_and(|(_, ch)| !ch.is_ascii()) {
                chars.next();
            }
        } else if !ch.is_ascii_alphanumeric()
            && !matches!(
                ch,
                ' ' | '\t'
                    | '\u{000b}'
                    | '\u{000c}'
                    | '!'
                    | '#'
                    | '$'
                    | '%'
                    | '&'
                    | '\''
                    | '*'
                    | '+'
                    | ','
                    | '-'
                    | '.'
                    | '`'
                    | '?'
                    | '\\'
                    | '_'
                    | '/'
                    | '='
            )
        {
            return false;
        }
    }
    true
}

fn parse_text_slice(input: SourceSlice<'_>) -> StatementResult<ParsedText<'_>> {
    let input = input.trim();
    if input.text.starts_with('"') {
        let markdown = input.text.starts_with("\"`");
        let opening_len = if markdown { 2 } else { 1 };
        let Some(closing) = input.text[opening_len..].find('"') else {
            return Err(if markdown {
                "unterminated markdown string"
            } else {
                "unterminated string"
            }
            .into());
        };
        let closing = opening_len + closing;
        let suffix = &input.text[closing + 1..];
        if !is_quadrant_text_suffix(suffix) {
            return Err("unexpected text after quoted token".into());
        }
        let content_end = if markdown {
            let Some(content_end) = closing.checked_sub(1).filter(|end| *end >= opening_len) else {
                return Err("unterminated markdown string".into());
            };
            if input.text.as_bytes()[content_end] != b'`'
                || input.text[opening_len..content_end].contains('`')
                || content_end == opening_len
            {
                return Err("invalid markdown string".into());
            }
            content_end
        } else {
            closing
        };
        let token_content = input.subslice(opening_len, content_end);
        let mut value = token_content.text.to_string();
        value.push_str(suffix);
        let content = if suffix.is_empty() {
            token_content
        } else {
            input
        };
        return Ok(ParsedText { value, content });
    }
    Ok(ParsedText {
        value: input.text.to_string(),
        content: input,
    })
}

struct ParsedAxis<'source> {
    left: ParsedText<'source>,
    right: Option<ParsedText<'source>>,
    operator: Option<SourceSpan>,
}

fn parse_axis_text(input: SourceSlice<'_>) -> StatementResult<ParsedAxis<'_>> {
    let mut in_quotes = false;
    let mut index = 0usize;
    while index < input.text.len() {
        let Some(ch) = next_char_at(input.text, index) else {
            break;
        };
        if ch == '"' {
            in_quotes = !in_quotes;
            index += ch.len_utf8();
            continue;
        }
        if !in_quotes && let Some((start, end)) = is_axis_delim_at(input.text, index) {
            let left = parse_text_slice(input.subslice(0, start))?;
            let right = input.subslice(end, input.text.len()).trim();
            return Ok(ParsedAxis {
                left,
                right: (!right.text.is_empty())
                    .then(|| parse_text_slice(right))
                    .transpose()?,
                operator: Some(SourceSpan::new(input.start + start, input.start + end)),
            });
        }
        index += ch.len_utf8();
    }
    Ok(ParsedAxis {
        left: parse_text_slice(input)?,
        right: None,
        operator: None,
    })
}

fn point_coordinate_token_len(input: &str) -> Option<usize> {
    match input.as_bytes().first()? {
        b'1' => Some(1),
        b'0' => {
            // quadrant.jison uses (1)|(0(.\d+)?). Its dot matches one UTF-16
            // code unit, not just a decimal point, and the optional suffix is greedy.
            let Some(wildcard) = input[1..].chars().next() else {
                return Some(1);
            };
            if wildcard.len_utf16() != 1 || is_ecmascript_line_terminator(wildcard) {
                return Some(1);
            }
            let digits_start = 1 + wildcard.len_utf8();
            let digits = input[digits_start..]
                .bytes()
                .take_while(u8::is_ascii_digit)
                .count();
            Some(if digits == 0 {
                1
            } else {
                digits_start + digits
            })
        }
        _ => None,
    }
}

struct PointCoordinates<'source> {
    x: SourceSlice<'source>,
    y: SourceSlice<'source>,
    after: SourceSlice<'source>,
}

fn point_coordinates(input: SourceSlice<'_>) -> Option<PointCoordinates<'_>> {
    input.text.strip_prefix('[')?;
    let x_input = input.subslice(1, input.text.len()).trim_ecmascript_start();
    let x_len = point_coordinate_token_len(x_input.text)?;
    let x = x_input.subslice(0, x_len);
    let separator = x_input
        .subslice(x_len, x_input.text.len())
        .trim_ecmascript_start();
    separator.text.strip_prefix(',')?;
    let y_input = separator
        .subslice(1, separator.text.len())
        .trim_ecmascript_start();
    let y_len = point_coordinate_token_len(y_input.text)?;
    let y = y_input.subslice(0, y_len);
    let close = y_input
        .subslice(y_len, y_input.text.len())
        .trim_ecmascript_start();
    close.text.strip_prefix(']')?;
    Some(PointCoordinates {
        x,
        y,
        after: close.subslice(1, close.text.len()),
    })
}

struct ParsedStyleList<'source> {
    items: Vec<SourceSlice<'source>>,
}

fn split_style_slices(input: SourceSlice<'_>) -> StatementResult<ParsedStyleList<'_>> {
    // UNICODE_TEXT and punctuation outside styleComponent remain lexer errors,
    // including in components that the DB subsequently ignores.
    if !input.text.bytes().all(|byte| {
        byte.is_ascii_alphanumeric()
            || matches!(byte, b':' | b'#' | b'-' | b',' | b' ' | b'\t' | 0x0b | 0x0c)
    }) {
        return Err("invalid character in style".into());
    }
    let mut cursor = 0;
    while cursor < input.text.len() {
        let rest = &input.text[cursor..];
        let first = rest.as_bytes()[0];
        let token_len = if first.is_ascii_alphabetic() {
            rest.bytes().take_while(u8::is_ascii_alphabetic).count()
        } else if first.is_ascii_digit() {
            rest.bytes().take_while(u8::is_ascii_digit).count()
        } else {
            1
        };
        let reserved = rest.starts_with(":::")
            || [
                "title",
                "x-axis",
                "y-axis",
                "quadrant-1",
                "quadrant-2",
                "quadrant-3",
                "quadrant-4",
                "classDef",
                "quadrantChart",
            ]
            .iter()
            .any(|keyword| {
                keyword.len() >= token_len
                    && rest
                        .get(..keyword.len())
                        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(keyword))
            })
            || ["accTitle", "accDescr"].iter().any(|keyword| {
                rest.get(..keyword.len())
                    .is_some_and(|prefix| prefix.eq_ignore_ascii_case(keyword))
                    && rest[keyword.len()..]
                        .trim_start_matches(|ch: char| ch.is_ascii_whitespace())
                        .starts_with(':')
            });
        if reserved {
            return Err("reserved token in style".into());
        }
        // A longer ALPHA token wins over a keyword prefix; equal lengths keep rule order.
        cursor += token_len;
    }
    let mut styles = Vec::new();
    let mut start = 0usize;
    for (index, ch) in input.text.char_indices() {
        if ch == ',' {
            let style = input.subslice(start, index).trim_ascii();
            if !style.text.is_empty() {
                styles.push(style);
            }
            start = index + ch.len_utf8();
        }
    }
    let style = input.subslice(start, input.text.len()).trim_ascii();
    if !style.text.is_empty() {
        styles.push(style);
    }
    Ok(ParsedStyleList { items: styles })
}

fn find_point_colon(s: &str) -> Option<usize> {
    let mut in_quotes = false;
    let mut i = 0usize;
    while i < s.len() {
        let Some(ch) = next_char_at(s, i) else {
            break;
        };
        if ch == '"' {
            in_quotes = !in_quotes;
            i += 1;
            continue;
        }
        if !in_quotes && ch == ':' {
            let mut j = i + 1;
            while j < s.len() {
                let Some(c2) = next_char_at(s, j) else {
                    break;
                };
                if is_ecmascript_whitespace(c2) {
                    j += c2.len_utf8();
                    continue;
                }
                if c2 == '[' {
                    return Some(i);
                }
                break;
            }
        }
        i += ch.len_utf8();
    }
    None
}

fn find_class_marker(input: &str) -> Option<usize> {
    let mut in_quotes = false;
    let mut last = None;
    let mut index = 0usize;
    while index < input.len() {
        let Some(ch) = next_char_at(input, index) else {
            break;
        };
        if ch == '"' {
            in_quotes = !in_quotes;
        } else if !in_quotes && input[index..].starts_with(":::") {
            last = Some(index);
            index += 3;
            continue;
        }
        index += ch.len_utf8();
    }
    last
}

struct ParsedPoint<'source> {
    label: ParsedText<'source>,
    class_name: Option<SourceSlice<'source>>,
    #[cfg(feature = "editor-facts")]
    class_marker: Option<SourceSpan>,
    x: f64,
    y: f64,
    styles: Vec<SourceSlice<'source>>,
}

fn parse_point_statement(statement: SourceSlice<'_>) -> StatementResult<Option<ParsedPoint<'_>>> {
    let Some(colon_idx) = find_point_colon(statement.text) else {
        return Ok(None);
    };
    let head = statement.subslice(0, colon_idx).trim_ecmascript_end();
    let tail = statement
        .subslice(colon_idx + 1, statement.text.len())
        .trim_ecmascript_start();

    let (class_name, _class_marker, label_input) =
        if let Some(marker) = find_class_marker(head.text) {
            let class = head.subslice(marker + 3, head.text.len()).trim();
            if class
                .text
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
            {
                (
                    (!class.text.is_empty()).then_some(class),
                    Some(SourceSpan::new(
                        head.start + marker,
                        head.start + marker + 3,
                    )),
                    head.subslice(0, marker).trim_end(),
                )
            } else {
                (None, None, head)
            }
        } else {
            (None, None, head)
        };
    let label = parse_text_slice(label_input)?;

    if !tail.text.starts_with('[') {
        return Err("expected '[' after ':'".into());
    }
    let Some(coordinates) = point_coordinates(tail) else {
        return Err("invalid point coordinate".into());
    };
    // D3 scaleLinear coerces these raw lexer strings with Number, rather than parseFloat.
    let x = parse_js_number(coordinates.x.text).unwrap_or(f64::NAN);
    let y = parse_js_number(coordinates.y.text).unwrap_or(f64::NAN);
    let styles = split_style_slices(coordinates.after)?;
    Ok(Some(ParsedPoint {
        label,
        class_name,
        #[cfg(feature = "editor-facts")]
        class_marker: _class_marker,
        x,
        y,
        styles: styles.items,
    }))
}

struct KeywordRest<'source> {
    rest: SourceSlice<'source>,
}

fn parse_keyword_rest_ci<'source>(
    input: SourceSlice<'source>,
    key: &str,
) -> Option<KeywordRest<'source>> {
    let input = input.trim_start();
    let keyword = input.text.get(..key.len())?;
    if !keyword.eq_ignore_ascii_case(key) {
        return None;
    }
    Some(KeywordRest {
        rest: input.subslice(key.len(), input.text.len()).trim_start(),
    })
}

struct ColonDirective<'source> {
    value: SourceSlice<'source>,
}

fn parse_colon_value_ci<'source>(
    input: SourceSlice<'source>,
    key: &str,
) -> Option<ColonDirective<'source>> {
    let matched = parse_keyword_rest_ci(input, key)?;
    if !matched.rest.text.starts_with(':') {
        return None;
    }
    Some(ColonDirective {
        value: matched.rest.subslice(1, matched.rest.text.len()).trim(),
    })
}

struct QuadrantStatement<'source> {
    source: SourceSlice<'source>,
}

#[derive(Clone, Copy)]
enum QuadrantTerminal {
    Statement,
    Point,
    RawLine,
    DescriptionBlock,
}

struct QuadrantStatements<'source> {
    source: &'source str,
    offset: usize,
    #[cfg(test)]
    scanned_bytes: usize,
}

impl<'source> QuadrantStatements<'source> {
    fn new(source: &'source str) -> Self {
        Self {
            source,
            offset: 0,
            #[cfg(test)]
            scanned_bytes: 0,
        }
    }

    fn advance(&mut self, bytes: usize, control: &OperationControl) -> OperationControlResult<()> {
        let previous = self.offset / 128;
        self.offset += bytes;
        #[cfg(test)]
        {
            self.scanned_bytes += bytes;
        }
        if self.offset / 128 != previous {
            control.checkpoint()?;
        }
        Ok(())
    }

    fn terminal(&self, control: &OperationControl) -> OperationControlResult<QuadrantTerminal> {
        let rest = &self.source[self.offset..];
        for keyword in ["title", "accTitle", "accDescr"] {
            if !rest
                .get(..keyword.len())
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case(keyword))
            {
                continue;
            }
            if keyword == "title" {
                return Ok(QuadrantTerminal::RawLine);
            }
            for (index, ch) in rest[keyword.len()..].chars().enumerate() {
                if index.is_multiple_of(128) {
                    control.checkpoint()?;
                }
                if matches!(ch, '\r' | '\n') {
                    break;
                }
                if ch.is_whitespace() {
                    continue;
                }
                return Ok(match ch {
                    ':' => QuadrantTerminal::RawLine,
                    '{' if keyword == "accDescr" => QuadrantTerminal::DescriptionBlock,
                    _ => QuadrantTerminal::Point,
                });
            }
        }
        // Directives own their payload; only point statements enter coordinate lexer states.
        let input = SourceSlice::new(rest, self.offset);
        if [
            "quadrantChart",
            "x-axis",
            "y-axis",
            "quadrant-1",
            "quadrant-2",
            "quadrant-3",
            "quadrant-4",
            "classDef",
        ]
        .iter()
        .any(|keyword| parse_keyword_rest_ci(input, keyword).is_some())
        {
            Ok(QuadrantTerminal::Statement)
        } else {
            Ok(QuadrantTerminal::Point)
        }
    }

    fn next_statement(
        &mut self,
        control: &OperationControl,
    ) -> OperationControlResult<Option<QuadrantStatement<'source>>> {
        control.checkpoint()?;
        while let Some(ch) = next_char_at(self.source, self.offset) {
            if !ch.is_whitespace() && ch != ';' {
                break;
            }
            self.advance(ch.len_utf8(), control)?;
        }
        let start = self.offset;
        if start == self.source.len() {
            return Ok(None);
        }
        let terminal = self.terminal(control)?;
        let mut in_quotes = false;
        let mut quote_line_end = None;
        let mut in_block = false;
        let mut block_opened = false;
        while let Some(ch) = next_char_at(self.source, self.offset) {
            if in_block {
                in_block = ch != '}';
            } else if in_quotes {
                if ch == '"' {
                    in_quotes = false;
                    quote_line_end = None;
                } else if quote_line_end.is_none() && matches!(ch, '\r' | '\n') {
                    quote_line_end = Some(self.offset);
                }
            } else if matches!(ch, '\r' | '\n')
                || (ch == ';' && !matches!(terminal, QuadrantTerminal::RawLine))
            {
                let end = self.offset;
                self.advance(ch.len_utf8(), control)?;
                return Ok(Some(QuadrantStatement {
                    source: SourceSlice::new(&self.source[start..end], start),
                }));
            } else if !matches!(terminal, QuadrantTerminal::RawLine) {
                if matches!(terminal, QuadrantTerminal::Point)
                    && (ch == ':' || is_ecmascript_whitespace(ch))
                {
                    let input = SourceSlice::new(&self.source[self.offset..], self.offset);
                    let prefix = input.trim_ecmascript_start();
                    if prefix.text.starts_with(':') {
                        let coordinate_input = prefix
                            .subslice(1, prefix.text.len())
                            .trim_ecmascript_start();
                        if let Some(coordinates) = point_coordinates(coordinate_input) {
                            // point_start owns its leading whitespace, including newlines when
                            // matching starts before CR/LF. A bare CR/LF ends the statement above.
                            self.advance(coordinates.after.start - self.offset, control)?;
                            continue;
                        }
                    }
                    if is_ecmascript_whitespace(ch) {
                        // Do not retry the same whitespace suffix at every byte, or skip a
                        // newline when no complete coordinate token follows the prefix.
                        let inline_bytes = input
                            .text
                            .chars()
                            .take_while(|ch| {
                                is_ecmascript_whitespace(*ch) && !matches!(*ch, '\r' | '\n')
                            })
                            .map(char::len_utf8)
                            .sum();
                        self.advance(inline_bytes, control)?;
                        continue;
                    }
                }
                if ch == '"' {
                    in_quotes = true;
                } else if ch == '{'
                    && !block_opened
                    && matches!(terminal, QuadrantTerminal::DescriptionBlock)
                {
                    in_block = true;
                    block_opened = true;
                } else if self.source[self.offset..].starts_with("%%") {
                    let end = self.offset;
                    while let Some(ch) = next_char_at(self.source, self.offset) {
                        self.advance(ch.len_utf8(), control)?;
                        if matches!(ch, '\r' | '\n') {
                            break;
                        }
                    }
                    return Ok(Some(QuadrantStatement {
                        source: SourceSlice::new(&self.source[start..end], start),
                    }));
                }
            }
            self.advance(ch.len_utf8(), control)?;
        }
        // EOF proves this suffix has no closing quote, so only this final suffix can be replayed.
        if in_quotes && let Some(end) = quote_line_end {
            self.offset = end;
        }
        Ok(Some(QuadrantStatement {
            source: SourceSlice::new(&self.source[start..self.offset], start),
        }))
    }
}

/// Builds the model in one pass over the statements, retaining the first source issue.
pub fn parse_quadrant_source_controlled(
    code: &str,
    config: &MermaidConfig,
    control: &OperationControl,
) -> OperationControlResult<Result<QuadrantChartDiagramModel, QuadrantSourceIssue>> {
    parse_quadrant_source(
        code,
        config,
        #[cfg(feature = "editor-facts")]
        None,
        control,
    )
}

#[cfg(feature = "editor-facts")]
pub fn parse_quadrant_source_with_observer_controlled(
    code: &str,
    config: &MermaidConfig,
    observe: Option<&mut dyn FnMut(QuadrantObservation<'_>)>,
    control: &OperationControl,
) -> OperationControlResult<Result<QuadrantChartDiagramModel, QuadrantSourceIssue>> {
    parse_quadrant_source(code, config, observe, control)
}

fn parse_quadrant_source(
    code: &str,
    config: &MermaidConfig,
    #[cfg(feature = "editor-facts")] mut observe: Option<&mut dyn FnMut(QuadrantObservation<'_>)>,
    control: &OperationControl,
) -> OperationControlResult<Result<QuadrantChartDiagramModel, QuadrantSourceIssue>> {
    let mut db = QuadrantDb::default();
    db.clear();
    let mut title = None;
    let mut acc_title = None;
    let mut acc_descr = None;
    let mut saw_header = false;
    let mut first_issue = None;
    let mut statements = QuadrantStatements::new(code);

    while let Some(statement) = statements.next_statement(control)? {
        let statement = statement.source;
        let statement_trimmed = statement.trim();
        if statement_trimmed.text.is_empty() || statement_trimmed.text.starts_with("%%") {
            continue;
        }
        let statement_span = statement_trimmed.span();
        let semantic_end = statement_span.end;

        if !saw_header {
            if statement_trimmed.text.eq_ignore_ascii_case("quadrantChart") {
                saw_header = true;
                continue;
            }
            first_issue.get_or_insert_with(|| {
                QuadrantSourceIssue::at_statement(statement_span, "expected quadrantChart")
            });
            continue;
        }

        if let Some(directive) = parse_colon_value_ci(statement_trimmed, "accTitle") {
            #[cfg(feature = "editor-facts")]
            if let Some(observe) = &mut observe {
                observe(QuadrantObservation::Directive("accTitle"));
                observe(QuadrantObservation::Text {
                    kind: QuadrantTextKind::AccTitle,
                    text: directive.value.text,
                    span: directive.value.span(),
                });
            }
            acc_title = Some(directive.value.text.to_string());
            continue;
        }

        if let Some(directive) = parse_keyword_rest_ci(statement_trimmed, "accDescr") {
            let rest = directive.rest;
            if rest.text.starts_with('{') {
                #[cfg(feature = "editor-facts")]
                if let Some(observe) = &mut observe {
                    observe(QuadrantObservation::Directive("accDescr"));
                }
                let after_brace = rest.subslice(1, rest.text.len());
                let closing = after_brace.text.find('}');
                let content = after_brace.subslice(0, closing.unwrap_or(after_brace.text.len()));
                let text = content.text.trim().to_string();
                #[cfg(feature = "editor-facts")]
                if !text.is_empty()
                    && let Some(observe) = &mut observe
                {
                    observe(QuadrantObservation::Text {
                        kind: QuadrantTextKind::AccDescr,
                        text: &text,
                        span: content.span(),
                    });
                }
                if let Some(end) = closing {
                    let trailing = after_brace.subslice(end + 1, after_brace.text.len()).trim();
                    if !trailing.text.is_empty() {
                        first_issue.get_or_insert_with(|| {
                            QuadrantSourceIssue::at_statement(
                                trailing.span(),
                                "expected ';' or newline after accDescr block",
                            )
                        });
                    }
                } else {
                    first_issue.get_or_insert(QuadrantSourceIssue::InsertionPoint {
                        offset: code.len(),
                        message: "unterminated accDescr block",
                    });
                }
                acc_descr = Some(text);
                continue;
            }
            if rest.text.starts_with(':') {
                let value = rest.subslice(1, rest.text.len()).trim();
                #[cfg(feature = "editor-facts")]
                if let Some(observe) = &mut observe {
                    observe(QuadrantObservation::Directive("accDescr"));
                    observe(QuadrantObservation::Text {
                        kind: QuadrantTextKind::AccDescr,
                        text: value.text,
                        span: value.span(),
                    });
                }
                acc_descr = Some(value.text.to_string());
                continue;
            }
        }

        if let Some(directive) = parse_keyword_rest_ci(statement_trimmed, "title") {
            let value = directive.rest;
            #[cfg(feature = "editor-facts")]
            if let Some(observe) = &mut observe {
                observe(QuadrantObservation::Directive("title"));
                observe(QuadrantObservation::Text {
                    kind: QuadrantTextKind::Title,
                    text: value.text,
                    span: value.span(),
                });
            }
            title = Some(value.text.to_string());
            continue;
        }

        if let Some(directive) = parse_keyword_rest_ci(statement_trimmed, "x-axis") {
            let axis = match parse_axis_text(directive.rest) {
                Ok(axis) => axis,
                Err(error) => {
                    first_issue.get_or_insert_with(|| {
                        QuadrantSourceIssue::at_statement(statement_span, error)
                    });
                    continue;
                }
            };
            #[cfg(feature = "editor-facts")]
            if let Some(observe) = &mut observe {
                for label in std::iter::once(&axis.left).chain(axis.right.as_ref()) {
                    observe(QuadrantObservation::Text {
                        kind: QuadrantTextKind::XAxis,
                        text: &label.value,
                        span: label.content.span(),
                    });
                }
            }
            let mut left = axis.left.value;
            if axis.operator.is_some() && axis.right.is_none() {
                left.push_str(" ⟶");
            }
            db.set_x_axis_left(&left, config);
            if let Some(right) = axis.right {
                db.set_x_axis_right(&right.value, config);
            }
            continue;
        }

        if let Some(directive) = parse_keyword_rest_ci(statement_trimmed, "y-axis") {
            let axis = match parse_axis_text(directive.rest) {
                Ok(axis) => axis,
                Err(error) => {
                    first_issue.get_or_insert_with(|| {
                        QuadrantSourceIssue::at_statement(statement_span, error)
                    });
                    continue;
                }
            };
            #[cfg(feature = "editor-facts")]
            if let Some(observe) = &mut observe {
                for label in std::iter::once(&axis.left).chain(axis.right.as_ref()) {
                    observe(QuadrantObservation::Text {
                        kind: QuadrantTextKind::YAxis,
                        text: &label.value,
                        span: label.content.span(),
                    });
                }
            }
            let mut bottom = axis.left.value;
            if axis.operator.is_some() && axis.right.is_none() {
                bottom.push_str(" ⟶");
            }
            db.set_y_axis_bottom(&bottom, config);
            if let Some(top) = axis.right {
                db.set_y_axis_top(&top.value, config);
            }
            continue;
        }

        let mut matched_quadrant = false;
        for (index, keyword) in [
            (1u8, "quadrant-1"),
            (2, "quadrant-2"),
            (3, "quadrant-3"),
            (4, "quadrant-4"),
        ] {
            let Some(directive) = parse_keyword_rest_ci(statement_trimmed, keyword) else {
                continue;
            };
            let text = match parse_text_slice(directive.rest) {
                Ok(text) => text,
                Err(error) => {
                    first_issue.get_or_insert_with(|| {
                        QuadrantSourceIssue::at_statement(statement_span, error)
                    });
                    matched_quadrant = true;
                    break;
                }
            };
            #[cfg(feature = "editor-facts")]
            if let Some(observe) = &mut observe {
                observe(QuadrantObservation::Text {
                    kind: QuadrantTextKind::Quadrant,
                    text: &text.value,
                    span: text.content.span(),
                });
            }
            db.set_quadrant_text(index, &text.value, config);
            matched_quadrant = true;
            break;
        }
        if matched_quadrant {
            continue;
        }

        if let Some(directive) = parse_keyword_rest_ci(statement, "classDef") {
            let rest = directive.rest;
            let name_end = rest
                .text
                .char_indices()
                .find_map(|(index, ch)| ch.is_whitespace().then_some(index))
                .unwrap_or(rest.text.len());
            let name = rest.subslice(0, name_end).trim();
            if name.text.is_empty() {
                first_issue.get_or_insert(QuadrantSourceIssue::InsertionPoint {
                    offset: semantic_end,
                    message: "expected classDef name",
                });
                continue;
            }
            let style_text = rest.subslice(name_end, rest.text.len()).trim_ascii();
            let styles = match split_style_slices(style_text) {
                Ok(styles) => styles,
                Err(error) => {
                    first_issue.get_or_insert_with(|| {
                        QuadrantSourceIssue::at_statement(statement_span, error)
                    });
                    continue;
                }
            };
            #[cfg(feature = "editor-facts")]
            if let Some(observe) = &mut observe {
                observe(QuadrantObservation::ClassDefinition {
                    name: name.text,
                    statement: statement_span,
                    selection: name.span(),
                });
            }
            let style_values = styles
                .items
                .iter()
                .map(|style| style.text.to_string())
                .collect::<Vec<_>>();
            if let Err(error) = db.add_class(name.text, &style_values) {
                first_issue.get_or_insert_with(|| {
                    QuadrantSourceIssue::at_statement(statement_span, error)
                });
            }
            continue;
        }

        match parse_point_statement(statement) {
            Ok(Some(point)) => {
                #[cfg(feature = "editor-facts")]
                if let Some(observe) = &mut observe {
                    if !point.label.content.text.is_empty() {
                        observe(QuadrantObservation::PointLabel {
                            text: &point.label.value,
                            statement: statement_span,
                            selection: point.label.content.span(),
                        });
                    }
                    if let Some(marker) = point.class_marker {
                        observe(QuadrantObservation::ClassSelector {
                            name: point.class_name.map(|class| class.text),
                            statement: statement_span,
                            selection: point.class_name.map_or_else(
                                || SourceSpan::new(marker.end, marker.end),
                                SourceSlice::span,
                            ),
                        });
                    }
                }
                let class_name = point.class_name.map(|class| class.text.to_string());
                let styles = point
                    .styles
                    .iter()
                    .map(|style| style.text.to_string())
                    .collect::<Vec<_>>();
                if let Err(error) = db.add_point(
                    &point.label.value,
                    class_name,
                    point.x,
                    point.y,
                    &styles,
                    config,
                ) {
                    first_issue.get_or_insert_with(|| {
                        QuadrantSourceIssue::at_statement(statement_span, error)
                    });
                }
                continue;
            }
            Ok(None) => {}
            Err(error) => {
                first_issue.get_or_insert_with(|| {
                    QuadrantSourceIssue::at_statement(statement_span, error)
                });
                continue;
            }
        }

        first_issue.get_or_insert_with(|| {
            QuadrantSourceIssue::at_statement(
                statement_span,
                format!("Unrecognized statement: {}", statement_trimmed.text),
            )
        });
    }

    if !saw_header {
        first_issue.get_or_insert(QuadrantSourceIssue::InsertionPoint {
            offset: code.len(),
            message: "expected quadrantChart",
        });
    }

    if let Some(issue) = first_issue {
        return Ok(Err(issue));
    }

    control.checkpoint()?;
    Ok(Ok(QuadrantChartDiagramModel {
        title,
        acc_title,
        acc_descr,
        quadrants: QuadrantChartQuadrantsModel {
            quadrant1_text: db.quadrant1_text,
            quadrant2_text: db.quadrant2_text,
            quadrant3_text: db.quadrant3_text,
            quadrant4_text: db.quadrant4_text,
        },
        axes: QuadrantChartAxesModel {
            x_axis_left_text: db.x_axis_left_text,
            x_axis_right_text: db.x_axis_right_text,
            y_axis_bottom_text: db.y_axis_bottom_text,
            y_axis_top_text: db.y_axis_top_text,
        },
        points: db.points,
        classes: db.classes.into_iter().collect(),
    }))
}

#[cfg(test)]
mod tests;
