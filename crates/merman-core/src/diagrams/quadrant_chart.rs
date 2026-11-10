#[cfg(feature = "presentation")]
use crate::SourcePresentationMetadata;
#[cfg(feature = "editor-facts")]
use crate::family::EditorFactsDemand;
use crate::sanitize::sanitize_text;
#[cfg(feature = "editor-facts")]
use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
    EditorSemanticSymbol,
};
use crate::{
    Error, MermaidConfig, OperationControl, OperationControlResult, Result, SourceSpan,
    family::CombinedSemanticFailure,
};
use merman_model::text::is_ecmascript_whitespace;
#[cfg(feature = "compat-json")]
use serde_json::Value;
#[cfg(all(test, feature = "compat-json"))]
use serde_json::json;
#[cfg(test)]
use std::cell::Cell;
use std::collections::HashMap;

#[cfg(test)]
thread_local! {
    static QUADRANT_SYNTAX_CONSTRUCTION_COUNT: Cell<usize> = const { Cell::new(0) };
}

#[cfg(test)]
fn reset_quadrant_syntax_construction_count() {
    QUADRANT_SYNTAX_CONSTRUCTION_COUNT.set(0);
}

#[cfg(test)]
fn quadrant_syntax_construction_count() -> usize {
    QUADRANT_SYNTAX_CONSTRUCTION_COUNT.get()
}

pub(crate) use merman_model::families::quadrant_chart::{
    QuadrantChartAxesModel, QuadrantChartDiagramModel, QuadrantChartPointModel,
    QuadrantChartQuadrantsModel, QuadrantChartStyles,
};

pub(crate) fn sanitize_common_db_fields(
    model: &mut QuadrantChartDiagramModel,
    config: &crate::MermaidConfig,
) {
    crate::common_db::sanitize_optional_title(&mut model.title, config);
    crate::common_db::sanitize_optional_acc_title(&mut model.acc_title, config);
    crate::common_db::sanitize_optional_acc_descr(&mut model.acc_descr, config);
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

    fn add_class(&mut self, class_name: &str, styles: &[String]) -> Result<()> {
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
    ) -> Result<()> {
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

struct QuadrantSemanticSource {
    model: QuadrantChartDiagramModel,
    #[cfg(feature = "editor-facts")]
    editor_facts: Option<EditorSemanticFacts>,
}

fn parse_styles(styles: &[String]) -> Result<QuadrantChartStyles> {
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
                    return Err(Error::diagram_parse_fallback(
                        "quadrantChart".to_string(),
                        format!("value for {key} {value} is invalid, please use a valid number"),
                    ));
                }
                // The lexer/validator admits only decimal digits, so parsing directly as
                // binary64 has the same rounding and overflow as JavaScript parseInt.
                out.radius = Some(value.parse::<f64>().map_err(|e| {
                    Error::diagram_parse_fallback("quadrantChart".to_string(), e.to_string())
                })?);
            }
            "color" => {
                if !is_valid_hex_code(value) {
                    return Err(Error::diagram_parse_fallback(
                        "quadrantChart".to_string(),
                        format!("value for {key} {value} is invalid, please use a valid hex code"),
                    ));
                }
                out.color = Some(value.to_string());
            }
            "stroke-color" => {
                if !is_valid_hex_code(value) {
                    return Err(Error::diagram_parse_fallback(
                        "quadrantChart".to_string(),
                        format!("value for {key} {value} is invalid, please use a valid hex code"),
                    ));
                }
                out.stroke_color = Some(value.to_string());
            }
            "stroke-width" => {
                if !is_valid_px(value) {
                    return Err(Error::diagram_parse_fallback(
                        "quadrantChart".to_string(),
                        format!(
                            "value for {key} {value} is invalid, please use a valid number of pixels (eg. 10px)"
                        ),
                    ));
                }
                out.stroke_width = Some(value.to_string());
            }
            _ => {
                return Err(Error::diagram_parse_fallback(
                    "quadrantChart".to_string(),
                    format!("style named {key} is not supported."),
                ));
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

fn parse_text_slice(input: SourceSlice<'_>) -> Result<ParsedText<'_>> {
    let input = input.trim();
    if input.text.starts_with('"') {
        let markdown = input.text.starts_with("\"`");
        let opening_len = if markdown { 2 } else { 1 };
        let Some(closing) = input.text[opening_len..].find('"') else {
            return Err(Error::diagram_parse_fallback(
                "quadrantChart".to_string(),
                if markdown {
                    "unterminated markdown string"
                } else {
                    "unterminated string"
                },
            ));
        };
        let closing = opening_len + closing;
        let suffix = &input.text[closing + 1..];
        if !is_quadrant_text_suffix(suffix) {
            return Err(Error::diagram_parse_fallback(
                "quadrantChart".to_string(),
                "unexpected text after quoted token",
            ));
        }
        let content_end = if markdown {
            let Some(content_end) = closing.checked_sub(1).filter(|end| *end >= opening_len) else {
                return Err(Error::diagram_parse_fallback(
                    "quadrantChart".to_string(),
                    "unterminated markdown string",
                ));
            };
            if input.text.as_bytes()[content_end] != b'`'
                || input.text[opening_len..content_end].contains('`')
                || content_end == opening_len
            {
                return Err(Error::diagram_parse_fallback(
                    "quadrantChart".to_string(),
                    "invalid markdown string",
                ));
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

fn parse_axis_text(input: SourceSlice<'_>) -> Result<ParsedAxis<'_>> {
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
            if wildcard.len_utf16() != 1
                || matches!(wildcard, '\r' | '\n' | '\u{2028}' | '\u{2029}')
            {
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

fn split_style_slices(input: SourceSlice<'_>) -> Result<ParsedStyleList<'_>> {
    // UNICODE_TEXT and punctuation outside styleComponent remain lexer errors,
    // including in components that the DB subsequently ignores.
    if !input.text.bytes().all(|byte| {
        byte.is_ascii_alphanumeric()
            || matches!(byte, b':' | b'#' | b'-' | b',' | b' ' | b'\t' | 0x0b | 0x0c)
    }) {
        return Err(Error::diagram_parse_fallback(
            "quadrantChart".to_string(),
            "invalid character in style".to_string(),
        ));
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
            return Err(Error::diagram_parse_fallback(
                "quadrantChart".to_string(),
                "reserved token in style".to_string(),
            ));
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

fn parse_point_statement(statement: SourceSlice<'_>) -> Result<Option<ParsedPoint<'_>>> {
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
        return Err(Error::diagram_parse_fallback(
            "quadrantChart".to_string(),
            "expected '[' after ':'".to_string(),
        ));
    }
    let Some(coordinates) = point_coordinates(tail) else {
        return Err(Error::diagram_parse_fallback(
            "quadrantChart".to_string(),
            "invalid point coordinate".to_string(),
        ));
    };
    // D3 scaleLinear coerces these raw lexer strings with Number, rather than parseFloat.
    let x = crate::utils::parse_js_number(coordinates.x.text).unwrap_or(f64::NAN);
    let y = crate::utils::parse_js_number(coordinates.y.text).unwrap_or(f64::NAN);
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

#[cfg(feature = "editor-facts")]
fn push_quadrant_payload_fact(
    facts: Option<&mut EditorSemanticFacts>,
    text: &str,
    start: usize,
    end: usize,
    detail: &'static str,
    kind: EditorSemanticKind,
) {
    let Some(facts) = facts else {
        return;
    };
    let span = SourceSpan::new(start, end);
    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::Payload,
        span,
    ));
    facts.push_symbol(EditorSemanticSymbol::payload(
        text.to_string(),
        Some(detail.to_string()),
        kind,
        span,
        span,
    ));
}

#[cfg(feature = "editor-facts")]
fn push_quadrant_outline_fact(
    facts: Option<&mut EditorSemanticFacts>,
    text: &str,
    start: usize,
    end: usize,
    detail: &'static str,
    kind: EditorSemanticKind,
) {
    let Some(facts) = facts else {
        return;
    };
    let span = SourceSpan::new(start, end);
    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::Payload,
        span,
    ));
    facts.push_symbol(EditorSemanticSymbol::outline(
        text.to_string(),
        Some(detail.to_string()),
        kind,
        span,
        span,
    ));
}

fn quadrant_error_at(error: Error, diagram_type: &str, span: SourceSpan) -> Error {
    let message = match error {
        Error::DiagramParse { diagnostic, .. } => diagnostic.message().to_string(),
        error => error.to_string(),
    };
    Error::diagram_parse_exact(diagram_type.to_string(), message, span)
}

#[cfg(feature = "editor-facts")]
fn push_quadrant_class_fact(
    facts: Option<&mut EditorSemanticFacts>,
    statement_span: SourceSpan,
    name: SourceSlice<'_>,
) {
    let Some(facts) = facts else {
        return;
    };
    let span = name.span();
    facts.push_directive_prefix("classDef");
    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::NodeIdentifier,
        span,
    ));
    facts.push_symbol(EditorSemanticSymbol::class_definition(
        name.text.to_string(),
        Some("quadrant chart class".to_string()),
        EditorSemanticKind::Class,
        statement_span,
        span,
    ));
}

#[cfg(feature = "editor-facts")]
fn push_quadrant_point_facts(
    facts: Option<&mut EditorSemanticFacts>,
    statement_span: SourceSpan,
    point: &ParsedPoint<'_>,
) {
    let Some(facts) = facts else {
        return;
    };
    if !point.label.content.text.is_empty() {
        let span = point.label.content.span();
        facts.push_expected_syntax(EditorExpectedSyntax::new(
            EditorExpectedSyntaxKind::Payload,
            span,
        ));
        facts.push_symbol(EditorSemanticSymbol::outline(
            point.label.value.clone(),
            Some("quadrant chart point".to_string()),
            EditorSemanticKind::Object,
            statement_span,
            span,
        ));
    }

    let Some(class_marker) = point.class_marker else {
        return;
    };
    let class_span = point
        .class_name
        .map(SourceSlice::span)
        .unwrap_or_else(|| SourceSpan::new(class_marker.end, class_marker.end));
    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::ClassName,
        class_span,
    ));

    let Some(class_name) = point.class_name else {
        return;
    };
    facts.push_symbol(EditorSemanticSymbol::payload(
        class_name.text.to_string(),
        Some("quadrant chart class".to_string()),
        EditorSemanticKind::Class,
        statement_span,
        class_name.span(),
    ));
}

#[cfg(any(feature = "compat-json", all(test, feature = "presentation")))]
fn construct_quadrant_chart_semantic_source(
    code: &str,
    meta: &SourcePresentationMetadata,
) -> std::result::Result<QuadrantSemanticSource, CombinedSemanticFailure> {
    construct_quadrant_chart_semantic_source_controlled(
        code,
        &meta.diagram_type,
        meta.effective_config.as_config(),
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        &OperationControl::new(),
    )
    .expect("a private parse control cannot be cancelled")
}

fn construct_quadrant_chart_semantic_source_controlled(
    code: &str,
    diagram_type: &str,
    config: &MermaidConfig,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &OperationControl,
) -> OperationControlResult<std::result::Result<QuadrantSemanticSource, CombinedSemanticFailure>> {
    control.checkpoint()?;
    #[cfg(test)]
    QUADRANT_SYNTAX_CONSTRUCTION_COUNT.set(QUADRANT_SYNTAX_CONSTRUCTION_COUNT.get() + 1);

    parse_quadrant_chart_semantic_source(
        code,
        diagram_type,
        config,
        #[cfg(feature = "editor-facts")]
        editor_demand,
        control,
    )
}

fn parse_quadrant_chart_semantic_source(
    code: &str,
    diagram_type: &str,
    config: &MermaidConfig,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &OperationControl,
) -> OperationControlResult<std::result::Result<QuadrantSemanticSource, CombinedSemanticFailure>> {
    control.checkpoint()?;
    let mut db = QuadrantDb::default();
    db.clear();
    #[cfg(feature = "editor-facts")]
    let mut editor_facts = match editor_demand {
        EditorFactsDemand::Omit => None,
        EditorFactsDemand::Recovery => Some(EditorSemanticFacts::new()),
        EditorFactsDemand::Capture => {
            #[cfg(test)]
            crate::editor::test_work::record_collection();
            Some(EditorSemanticFacts::new())
        }
    };
    let mut title = None;
    let mut acc_title = None;
    let mut acc_descr = None;
    let mut saw_header = false;
    let mut first_error = None;
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
            first_error.get_or_insert_with(|| {
                Error::diagram_parse_exact(
                    diagram_type.to_string(),
                    "expected quadrantChart",
                    statement_span,
                )
            });
            continue;
        }

        if let Some(directive) = parse_colon_value_ci(statement_trimmed, "accTitle") {
            #[cfg(feature = "editor-facts")]
            if let Some(facts) = editor_facts
                .as_mut()
                .filter(|_| editor_demand.captures_rich())
            {
                facts.push_directive_prefix("accTitle");
            }
            #[cfg(feature = "editor-facts")]
            push_quadrant_payload_fact(
                editor_facts
                    .as_mut()
                    .filter(|_| editor_demand.captures_rich()),
                directive.value.text,
                directive.value.start,
                directive.value.end(),
                "quadrant chart accessibility title",
                EditorSemanticKind::String,
            );
            acc_title = Some(directive.value.text.to_string());
            continue;
        }

        if let Some(directive) = parse_keyword_rest_ci(statement_trimmed, "accDescr") {
            let rest = directive.rest;
            if rest.text.starts_with('{') {
                #[cfg(feature = "editor-facts")]
                if let Some(facts) = editor_facts
                    .as_mut()
                    .filter(|_| editor_demand.captures_rich())
                {
                    facts.push_directive_prefix("accDescr");
                }
                #[cfg(feature = "editor-facts")]
                let content_start = rest.start + 1;
                let after_brace = rest.subslice(1, rest.text.len());
                let closing = after_brace.text.find('}');
                let content = after_brace.subslice(0, closing.unwrap_or(after_brace.text.len()));
                let text = content.text.trim().to_string();
                if !text.is_empty() {
                    #[cfg(feature = "editor-facts")]
                    push_quadrant_payload_fact(
                        editor_facts
                            .as_mut()
                            .filter(|_| editor_demand.captures_rich()),
                        &text,
                        content_start,
                        content.end(),
                        "quadrant chart accessibility description",
                        EditorSemanticKind::String,
                    );
                }
                if let Some(end) = closing {
                    let trailing = after_brace.subslice(end + 1, after_brace.text.len()).trim();
                    if !trailing.text.is_empty() {
                        first_error.get_or_insert_with(|| {
                            Error::diagram_parse_exact(
                                diagram_type.to_string(),
                                "expected ';' or newline after accDescr block",
                                trailing.span(),
                            )
                        });
                    }
                } else {
                    first_error.get_or_insert_with(|| {
                        Error::diagram_parse_insertion_point(
                            diagram_type.to_string(),
                            "unterminated accDescr block",
                            code.len(),
                        )
                    });
                }
                acc_descr = Some(text);
                continue;
            }
            if rest.text.starts_with(':') {
                let value = rest.subslice(1, rest.text.len()).trim();
                #[cfg(feature = "editor-facts")]
                if let Some(facts) = editor_facts
                    .as_mut()
                    .filter(|_| editor_demand.captures_rich())
                {
                    facts.push_directive_prefix("accDescr");
                }
                #[cfg(feature = "editor-facts")]
                push_quadrant_payload_fact(
                    editor_facts
                        .as_mut()
                        .filter(|_| editor_demand.captures_rich()),
                    value.text,
                    value.start,
                    value.end(),
                    "quadrant chart accessibility description",
                    EditorSemanticKind::String,
                );
                acc_descr = Some(value.text.to_string());
                continue;
            }
        }

        if let Some(directive) = parse_keyword_rest_ci(statement_trimmed, "title") {
            let value = directive.rest;
            #[cfg(feature = "editor-facts")]
            if let Some(facts) = editor_facts
                .as_mut()
                .filter(|_| editor_demand.captures_rich())
            {
                facts.push_directive_prefix("title");
            }
            #[cfg(feature = "editor-facts")]
            push_quadrant_payload_fact(
                editor_facts
                    .as_mut()
                    .filter(|_| editor_demand.captures_rich()),
                value.text,
                value.start,
                value.end(),
                "quadrant chart title",
                EditorSemanticKind::String,
            );
            title = Some(value.text.to_string());
            continue;
        }

        if let Some(directive) = parse_keyword_rest_ci(statement_trimmed, "x-axis") {
            let axis = match parse_axis_text(directive.rest) {
                Ok(axis) => axis,
                Err(error) => {
                    first_error.get_or_insert_with(|| {
                        quadrant_error_at(error, diagram_type, statement_span)
                    });
                    continue;
                }
            };
            #[cfg(feature = "editor-facts")]
            push_quadrant_outline_fact(
                editor_facts
                    .as_mut()
                    .filter(|_| editor_demand.captures_rich()),
                &axis.left.value,
                axis.left.content.start,
                axis.left.content.end(),
                "quadrant chart x-axis",
                EditorSemanticKind::String,
            );
            #[cfg(feature = "editor-facts")]
            if let Some(right) = &axis.right {
                push_quadrant_outline_fact(
                    editor_facts
                        .as_mut()
                        .filter(|_| editor_demand.captures_rich()),
                    &right.value,
                    right.content.start,
                    right.content.end(),
                    "quadrant chart x-axis",
                    EditorSemanticKind::String,
                );
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
                    first_error.get_or_insert_with(|| {
                        quadrant_error_at(error, diagram_type, statement_span)
                    });
                    continue;
                }
            };
            #[cfg(feature = "editor-facts")]
            push_quadrant_outline_fact(
                editor_facts
                    .as_mut()
                    .filter(|_| editor_demand.captures_rich()),
                &axis.left.value,
                axis.left.content.start,
                axis.left.content.end(),
                "quadrant chart y-axis",
                EditorSemanticKind::String,
            );
            #[cfg(feature = "editor-facts")]
            if let Some(top) = &axis.right {
                push_quadrant_outline_fact(
                    editor_facts
                        .as_mut()
                        .filter(|_| editor_demand.captures_rich()),
                    &top.value,
                    top.content.start,
                    top.content.end(),
                    "quadrant chart y-axis",
                    EditorSemanticKind::String,
                );
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
                    first_error.get_or_insert_with(|| {
                        quadrant_error_at(error, diagram_type, statement_span)
                    });
                    matched_quadrant = true;
                    break;
                }
            };
            #[cfg(feature = "editor-facts")]
            push_quadrant_outline_fact(
                editor_facts
                    .as_mut()
                    .filter(|_| editor_demand.captures_rich()),
                &text.value,
                text.content.start,
                text.content.end(),
                "quadrant chart quadrant",
                EditorSemanticKind::String,
            );
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
                first_error.get_or_insert_with(|| {
                    Error::diagram_parse_insertion_point(
                        diagram_type.to_string(),
                        "expected classDef name",
                        semantic_end,
                    )
                });
                continue;
            }
            let style_text = rest.subslice(name_end, rest.text.len()).trim_ascii();
            let styles = match split_style_slices(style_text) {
                Ok(styles) => styles,
                Err(error) => {
                    first_error.get_or_insert_with(|| {
                        quadrant_error_at(error, diagram_type, statement_span)
                    });
                    continue;
                }
            };
            #[cfg(feature = "editor-facts")]
            push_quadrant_class_fact(
                editor_facts
                    .as_mut()
                    .filter(|_| editor_demand.captures_rich()),
                statement_span,
                name,
            );
            let style_values = styles
                .items
                .iter()
                .map(|style| style.text.to_string())
                .collect::<Vec<_>>();
            if let Err(error) = db.add_class(name.text, &style_values) {
                first_error
                    .get_or_insert_with(|| quadrant_error_at(error, diagram_type, statement_span));
            }
            continue;
        }

        match parse_point_statement(statement) {
            Ok(Some(point)) => {
                #[cfg(feature = "editor-facts")]
                push_quadrant_point_facts(
                    editor_facts
                        .as_mut()
                        .filter(|_| editor_demand.captures_rich()),
                    statement_span,
                    &point,
                );
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
                    first_error.get_or_insert_with(|| {
                        quadrant_error_at(error, diagram_type, statement_span)
                    });
                }
                continue;
            }
            Ok(None) => {}
            Err(error) => {
                first_error
                    .get_or_insert_with(|| quadrant_error_at(error, diagram_type, statement_span));
                continue;
            }
        }

        first_error.get_or_insert_with(|| {
            Error::diagram_parse_exact(
                diagram_type.to_string(),
                format!("Unrecognized statement: {}", statement_trimmed.text),
                statement_span,
            )
        });
    }

    if !saw_header {
        first_error.get_or_insert_with(|| {
            Error::diagram_parse_insertion_point(
                diagram_type.to_string(),
                "expected quadrantChart",
                code.len(),
            )
        });
    }

    if let Some(error) = first_error {
        return Ok(Err(CombinedSemanticFailure::parser_recovery(
            #[cfg(feature = "editor-facts")]
            "quadrant chart",
            error,
            #[cfg(feature = "editor-facts")]
            editor_facts,
        )));
    }

    control.checkpoint()?;
    Ok(Ok(QuadrantSemanticSource {
        model: QuadrantChartDiagramModel {
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
        },
        #[cfg(feature = "editor-facts")]
        editor_facts,
    }))
}

#[cfg(feature = "compat-json")]
pub(crate) fn parse_quadrant_chart(code: &str, meta: &SourcePresentationMetadata) -> Result<Value> {
    let source = construct_quadrant_chart_semantic_source(code, meta)
        .map_err(CombinedSemanticFailure::into_error)?;
    render_model_to_compat_json(&source.model, meta)
}

#[cfg(feature = "compat-json")]
#[cfg(feature = "editor-facts")]
pub(crate) fn parse_quadrant_chart_json_and_editor_facts(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<crate::family::CombinedSemanticParse> {
    let construction = construct_quadrant_chart_semantic_source_controlled(
        code,
        &meta.diagram_type,
        meta.effective_config.as_config(),
        EditorFactsDemand::Capture,
        control,
    )?;
    Ok(crate::family::CombinedSemanticParse::from_construction(
        construction,
        |QuadrantSemanticSource {
             model,
             editor_facts,
         }| {
            (
                render_model_to_compat_json(&model, meta),
                editor_facts.unwrap_or_default(),
            )
        },
        CombinedSemanticFailure::into_parts,
    ))
}

#[cfg(feature = "compat-json")]
pub(crate) fn render_model_to_compat_json(
    model: &QuadrantChartDiagramModel,
    meta: &SourcePresentationMetadata,
) -> Result<Value> {
    merman_model::families::quadrant_chart::compat::project(model, &meta.compat_json_metadata())
        .map_err(crate::Error::from)
}

#[cfg(all(test, feature = "presentation"))]
pub(crate) fn parse_quadrant_chart_model_for_render(
    code: &str,
    meta: &SourcePresentationMetadata,
) -> Result<QuadrantChartDiagramModel> {
    construct_quadrant_chart_semantic_source(code, meta)
        .map(|source| source.model)
        .map_err(CombinedSemanticFailure::into_error)
}

#[cfg(feature = "presentation")]
pub(crate) fn parse_quadrant_chart_model_for_render_controlled(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<Result<QuadrantChartDiagramModel>> {
    let construction = construct_quadrant_chart_semantic_source_controlled(
        code,
        &meta.diagram_type,
        meta.effective_config.as_config(),
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        control,
    )?;
    match construction {
        Ok(source) => Ok(Ok(source.model)),
        Err(failure) => Ok(Err(failure.into_error())),
    }
}

pub(crate) fn parse_quadrant_chart_semantic_model_controlled(
    code: &str,
    metadata: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &OperationControl,
) -> OperationControlResult<crate::family::SemanticConstruction<QuadrantChartDiagramModel>> {
    let construction = construct_quadrant_chart_semantic_source_controlled(
        code,
        &metadata.diagram_type,
        &metadata.semantic_config,
        #[cfg(feature = "editor-facts")]
        editor_demand,
        control,
    )?;
    Ok(match construction {
        Ok(source) => {
            #[cfg(feature = "editor-facts")]
            let editor_facts = source.editor_facts;
            let mut model = source.model;
            sanitize_common_db_fields(&mut model, &metadata.semantic_config);
            crate::family::SemanticConstruction {
                model: Ok(model),
                #[cfg(feature = "editor-facts")]
                editor_facts,
            }
        }
        Err(failure) => {
            #[cfg(feature = "editor-facts")]
            {
                match editor_demand {
                    EditorFactsDemand::Capture | EditorFactsDemand::Recovery => {
                        let (error, facts) = failure.into_parts();
                        crate::family::SemanticConstruction {
                            model: Err(error),
                            editor_facts: Some(facts),
                        }
                    }
                    EditorFactsDemand::Omit => crate::family::SemanticConstruction {
                        model: Err(failure.into_error()),
                        editor_facts: None,
                    },
                }
            }
            #[cfg(not(feature = "editor-facts"))]
            {
                crate::family::SemanticConstruction {
                    model: Err(failure.into_error()),
                }
            }
        }
    })
}

#[cfg(all(test, feature = "compat-json"))]
mod tests {
    use super::*;
    use crate::generated;
    #[cfg(feature = "editor-facts")]
    use crate::{EditorSemanticCompleteness, EditorSemanticDiagnosticKind, EditorSemanticRole};
    use crate::{MermaidEngine, ParseOptions, RenderSemanticModel};
    use futures::executor::block_on;

    fn parse(text: &str) -> crate::ManagedSemanticJson {
        let engine = MermaidEngine::new();
        block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap()
            .model
    }

    fn parse_err(text: &str) -> String {
        let engine = MermaidEngine::new();
        match block_on(engine.parse_diagram(text, ParseOptions::default())).unwrap_err() {
            Error::DiagramParse { diagnostic, .. } => diagnostic.message().to_string(),
            other => other.to_string(),
        }
    }

    fn axes(model: &Value) -> &Value {
        &model["axes"]
    }

    fn quadrants(model: &Value) -> &Value {
        &model["quadrants"]
    }

    fn points(model: &Value) -> Vec<Value> {
        model["points"].as_array().cloned().unwrap_or_default()
    }

    #[test]
    fn errors_without_header() {
        let meta = SourcePresentationMetadata {
            diagram_type: "quadrantChart".to_string(),
            config: MermaidConfig::default(),
            effective_config: generated::default_presentation_config(),
            title: None,
        };
        let err = parse_quadrant_chart("quadrant-1 do\n", &meta)
            .unwrap_err()
            .to_string();
        assert!(err.contains("expected quadrantChart"));
    }

    #[test]
    fn header_only_is_allowed() {
        let model = parse("quadrantChart\n");
        assert_eq!(model["type"].as_str().unwrap(), "quadrantChart");
        assert!(model["title"].is_null());
    }

    #[test]
    fn parses_x_axis_text_and_missing_right_side() {
        let model = parse("quadrantChart\nx-axis urgent --> not urgent\n");
        assert_eq!(axes(&model)["xAxisLeftText"].as_str().unwrap(), "urgent");
        assert_eq!(
            axes(&model)["xAxisRightText"].as_str().unwrap(),
            "not urgent"
        );

        let model = parse("quadrantChart\nx-AxIs \"Urgent(* +=[?\"  --> \n");
        assert_eq!(
            axes(&model)["xAxisLeftText"].as_str().unwrap(),
            "Urgent(* +=[? ⟶"
        );
        assert_eq!(axes(&model)["xAxisRightText"].as_str().unwrap(), "");
    }

    #[test]
    fn parses_y_axis_text_and_missing_top_side() {
        let model = parse("quadrantChart\ny-axis urgent --> not urgent\n");
        assert_eq!(axes(&model)["yAxisBottomText"].as_str().unwrap(), "urgent");
        assert_eq!(axes(&model)["yAxisTopText"].as_str().unwrap(), "not urgent");

        let model = parse("quadrantChart\ny-AxIs \"Urgent(* +=[?\"  --> \n");
        assert_eq!(
            axes(&model)["yAxisBottomText"].as_str().unwrap(),
            "Urgent(* +=[? ⟶"
        );
        assert_eq!(axes(&model)["yAxisTopText"].as_str().unwrap(), "");
    }

    #[test]
    fn parses_quadrant_text_and_title() {
        let model = parse("quadrantChart\nquadrant-1 Plan\nquadrant-2 \"Do(* +=[?\"\n");
        assert_eq!(quadrants(&model)["quadrant1Text"].as_str().unwrap(), "Plan");
        assert_eq!(
            quadrants(&model)["quadrant2Text"].as_str().unwrap(),
            "Do(* +=[?"
        );

        let model = parse("quadrantChart\ntitle \"this is title (* +=[?\"\n");
        assert_eq!(
            model["title"].as_str().unwrap(),
            "\"this is title (* +=[?\""
        );
    }

    #[test]
    fn parses_points_and_validates_coordinate_range() {
        let model = parse("quadrantChart\npoint1: [0.1, 0.4]\n");
        let pts = points(&model);
        assert_eq!(pts.len(), 1);
        assert_eq!(pts[0]["text"].as_str().unwrap(), "point1");
        assert_eq!(pts[0]["x"].as_f64().unwrap(), 0.1);
        assert_eq!(pts[0]["y"].as_f64().unwrap(), 0.4);

        let model = parse("quadrantChart\n\"Point1 : (* +=[?\": [1, 0]\n");
        let pts = points(&model);
        assert_eq!(pts[0]["text"].as_str().unwrap(), "Point1 : (* +=[?");
        assert_eq!(pts[0]["x"].as_f64().unwrap(), 1.0);
        assert_eq!(pts[0]["y"].as_f64().unwrap(), 0.0);

        let err = parse_err("quadrantChart\nPoint1 : [1.2, 0.4]\n");
        assert!(err.contains("invalid point coordinate"));

        let err = parse_err("quadrantChart\nPoint1 : [0.2, 0.4, 0.6]\n");
        assert!(err.contains("invalid point coordinate"));
    }

    #[test]
    fn parses_point_styles_and_classes() {
        let model = parse(
            "quadrantChart\nclassDef class1 color: #109060, radius : 10, stroke-color: #310085, stroke-width: 10px\nPoint A:::class1: [0.9, 0.0]\n",
        );
        let classes = model["classes"].as_object().unwrap();
        let class1 = classes.get("class1").unwrap();
        assert_eq!(class1["color"].as_str().unwrap(), "#109060");
        assert_eq!(class1["radius"].as_i64().unwrap(), 10);
        assert_eq!(class1["strokeColor"].as_str().unwrap(), "#310085");
        assert_eq!(class1["strokeWidth"].as_str().unwrap(), "10px");

        let pts = points(&model);
        assert_eq!(pts.len(), 1);
        assert_eq!(pts[0]["className"].as_str().unwrap(), "class1");

        let model = parse(
            "quadrantChart\nIncorta: [0.20, 0.30] radius: 10 ,color: #ff0000 ,stroke-color: #ff00ff ,stroke-width: 10px\n",
        );
        let pts = points(&model);
        let styles = &pts[0]["styles"];
        assert_eq!(styles["radius"].as_i64().unwrap(), 10);
        assert_eq!(styles["color"].as_str().unwrap(), "#ff0000");
        assert_eq!(styles["strokeColor"].as_str().unwrap(), "#ff00ff");
        assert_eq!(styles["strokeWidth"].as_str().unwrap(), "10px");
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn quadrant_class_definitions_and_uses_have_typed_roles() {
        let text = concat!(
            "quadrantChart\n",
            "classDef priority color: #109060\n",
            "Project A:::priority: [0.2, 0.8]\n",
        );
        let facts = MermaidEngine::new()
            .parse_editor_semantic_facts_with_type_sync("quadrantChart", text)
            .unwrap()
            .expect("quadrant editor facts");

        let class_definition = facts
            .symbols
            .iter()
            .find(|symbol| {
                symbol.name == "priority" && symbol.role == EditorSemanticRole::ClassDefinition
            })
            .expect("quadrant class definition");
        assert_eq!(class_definition.kind, EditorSemanticKind::Class);
        assert!(class_definition.role.contributes_completion());
        assert!(class_definition.role.contributes_outline());
        assert!(!class_definition.role.contributes_references());

        let class_use = facts
            .symbols
            .iter()
            .find(|symbol| symbol.name == "priority" && symbol.role == EditorSemanticRole::Payload)
            .expect("quadrant class use");
        assert_eq!(class_use.kind, EditorSemanticKind::Class);
        assert!(!class_use.role.contributes_completion());
        assert!(!class_use.role.contributes_outline());
        assert!(!class_use.role.contributes_references());

        let class_names = facts
            .symbols
            .iter()
            .filter(|symbol| symbol.role == EditorSemanticRole::ClassDefinition)
            .map(|symbol| symbol.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(class_names, vec!["priority"]);

        let node_ids = facts
            .symbols
            .iter()
            .filter(|symbol| symbol.role == EditorSemanticRole::Entity)
            .map(|symbol| symbol.name.as_str())
            .collect::<Vec<_>>();
        assert!(
            !node_ids.contains(&"priority"),
            "class names must not enter node-id completion"
        );

        let outline_names = facts
            .symbols
            .iter()
            .filter(|symbol| symbol.role.contributes_outline())
            .map(|symbol| symbol.name.as_str())
            .collect::<Vec<_>>();
        assert!(outline_names.contains(&"priority"));
        assert!(outline_names.contains(&"Project A"));
        assert_eq!(
            outline_names
                .iter()
                .filter(|name| **name == "priority")
                .count(),
            1,
            "class use must not create a second outline entry"
        );
    }

    #[test]
    fn parses_whole_chart_example() {
        let model = parse(
            "quadrantChart\n\
title Analytics and Business Intelligence Platforms\n\
x-axis \"Completeness of Vision ?\" --> \"x-axis-2\"\n\
y-axis Ability to Execute --> \"y-axis-2\"\n\
quadrant-1 Leaders\n\
quadrant-2 Challengers\n\
quadrant-3 Niche\n\
quadrant-4 Visionaries\n\
Microsoft: [0.75, 0.75]\n\
Salesforce: [0.55, 0.60]\n\
IBM: [0.51, 0.40]\n\
Incorta: [0.20, 0.30]\n",
        );
        assert_eq!(
            axes(&model)["xAxisLeftText"].as_str().unwrap(),
            "Completeness of Vision ?"
        );
        assert_eq!(axes(&model)["xAxisRightText"].as_str().unwrap(), "x-axis-2");
        assert_eq!(
            axes(&model)["yAxisBottomText"].as_str().unwrap(),
            "Ability to Execute"
        );
        assert_eq!(axes(&model)["yAxisTopText"].as_str().unwrap(), "y-axis-2");
        assert_eq!(
            quadrants(&model)["quadrant1Text"].as_str().unwrap(),
            "Leaders"
        );
        assert_eq!(
            quadrants(&model)["quadrant4Text"].as_str().unwrap(),
            "Visionaries"
        );
        assert_eq!(points(&model).len(), 4);
    }

    #[test]
    fn parse_quadrant_chart_render_model_uses_typed_variant_without_changing_json_parse() {
        let engine = MermaidEngine::new();
        let input = r##"
quadrantChart
title Typed Quadrant
accTitle: Quadrant accTitle
accDescr: Quadrant accDescription
x-axis Low --> High
y-axis Bottom --> Top
quadrant-1 Expand
quadrant-2 Maintain
quadrant-3 Evaluate
quadrant-4 Retire
classDef priority color: #109060, radius : 10, stroke-color: #310085, stroke-width: 10px
Project A:::priority : [0.2, 0.8]
"##;

        let parsed = engine
            .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
            .unwrap()
            .unwrap();

        assert_eq!(parsed.metadata().diagram_type, "quadrantChart");
        match parsed.model() {
            RenderSemanticModel::QuadrantChart(model) => {
                assert_eq!(model.title.as_deref(), Some("Typed Quadrant"));
                assert_eq!(model.acc_title.as_deref(), Some("Quadrant accTitle"));
                assert_eq!(model.acc_descr.as_deref(), Some("Quadrant accDescription"));
                assert_eq!(model.axes.x_axis_left_text, "Low");
                assert_eq!(model.axes.x_axis_right_text, "High");
                assert_eq!(model.quadrants.quadrant1_text, "Expand");
                assert_eq!(model.points.len(), 1);
                assert_eq!(model.points[0].text, "Project A");
                assert_eq!(model.points[0].class_name.as_deref(), Some("priority"));
                assert_eq!(model.classes["priority"].radius, Some(10.0));
            }
            other => panic!("quadrantChart render parse should return typed model, got {other:?}"),
        }

        let parsed_json = engine
            .parse_diagram_sync(input, ParseOptions::strict())
            .unwrap()
            .unwrap();
        assert_eq!(parsed_json.model["type"], json!("quadrantChart"));
        assert_eq!(parsed_json.model["title"], json!("Typed Quadrant"));
        assert_eq!(parsed_json.model["accTitle"], json!("Quadrant accTitle"));
        assert_eq!(parsed_json.model["axes"]["xAxisLeftText"], json!("Low"));
        assert_eq!(
            parsed_json.model["quadrants"]["quadrant1Text"],
            json!("Expand")
        );
        assert_eq!(parsed_json.model["points"][0]["text"], json!("Project A"));
        assert_eq!(
            parsed_json.model["classes"]["priority"]["radius"],
            json!(10)
        );
        assert!(parsed_json.model.get("config").is_some());
    }

    #[test]
    fn parse_styles_matches_quadrantdb_spec() {
        let styles = vec![
            "radius: 10".to_string(),
            "color: #ff0000".to_string(),
            "stroke-color: #ff00ff".to_string(),
            "stroke-width: 10px".to_string(),
        ];
        let obj = parse_styles(&styles).unwrap();
        assert_eq!(obj.radius, Some(10.0));
        assert_eq!(obj.color.as_deref(), Some("#ff0000"));
        assert_eq!(obj.stroke_color.as_deref(), Some("#ff00ff"));
        assert_eq!(obj.stroke_width.as_deref(), Some("10px"));

        let err = parse_styles(&["test_name: value".to_string()])
            .unwrap_err()
            .to_string();
        assert!(err.contains("style named test_name is not supported."));

        let obj = parse_styles(&[]).unwrap();
        assert_eq!(obj.radius, None);
        assert!(obj.color.is_none());

        let err = parse_styles(&["radius: f".to_string()])
            .unwrap_err()
            .to_string();
        assert!(err.contains("value for radius f is invalid, please use a valid number"));
    }

    #[test]
    fn entrypoints_and_combined_projection_construct_once() {
        let engine = MermaidEngine::new();
        let text = concat!(
            "quadrantChart\n",
            "title Delivery portfolio; quadrant-1 Hidden\n",
            "accTitle: Portfolio\n",
            "x-axis Low --> High\n",
            "y-axis Bottom --> Top\n",
            "quadrant-1 \"Invest\nCarefully\"\n",
            "classDef priority color: #109060, radius: 10\n",
            "\"`Project\nA`\":::priority: [0.2, 0.8]\n",
        );
        let parsed = engine
            .parse_diagram_sync(text, ParseOptions::strict())
            .expect("standalone Quadrant JSON parse succeeds")
            .expect("standalone Quadrant JSON parse returns a diagram");
        reset_quadrant_syntax_construction_count();
        parse_quadrant_chart(
            text,
            &crate::diagrams::source_metadata_for_test(&parsed.meta),
        )
        .expect("Quadrant JSON projection succeeds");
        assert_eq!(quadrant_syntax_construction_count(), 1);

        reset_quadrant_syntax_construction_count();
        let typed = parse_quadrant_chart_model_for_render(
            text,
            &crate::diagrams::source_metadata_for_test(&parsed.meta),
        )
        .expect("Quadrant typed projection succeeds");
        assert_eq!(quadrant_syntax_construction_count(), 1);

        reset_quadrant_syntax_construction_count();
        #[cfg(feature = "editor-facts")]
        let (combined_json, combined_editor) =
            crate::family::test_support::into_result(parse_quadrant_chart_json_and_editor_facts(
                text,
                &crate::diagrams::source_metadata_for_test(&parsed.meta),
                &OperationControl::new(),
            ))
            .expect("Quadrant combined projection succeeds");
        #[cfg(not(feature = "editor-facts"))]
        let combined_json = parse_quadrant_chart(
            text,
            &crate::diagrams::source_metadata_for_test(&parsed.meta),
        )
        .unwrap();
        assert_eq!(quadrant_syntax_construction_count(), 1);
        assert_eq!(combined_json, parsed.model);
        #[cfg(feature = "editor-facts")]
        assert!(!combined_editor.symbols.is_empty());
        assert_eq!(
            merman_model::families::quadrant_chart::compat::project(
                &typed,
                &merman_model::presentation::compat::CompatJsonMetadata::from(&parsed.meta)
            )
            .unwrap(),
            combined_json
        );
        assert_eq!(combined_json["type"], json!("quadrantChart"));
        assert!(combined_json["config"].is_object());
        assert_eq!(combined_json["accDescr"], Value::Null);

        let compat_classes: std::collections::BTreeMap<String, QuadrantChartStyles> =
            serde_json::from_value(combined_json["classes"].clone())
                .expect("Quadrant compatibility classes deserialize into typed styles");
        assert_eq!(typed.classes.len(), compat_classes.len());
        for (name, styles) in &typed.classes {
            let compat = compat_classes
                .get(name)
                .expect("every typed Quadrant class remains in compatibility JSON");
            assert_eq!(
                (
                    styles.radius,
                    styles.color.as_deref(),
                    styles.stroke_color.as_deref(),
                    styles.stroke_width.as_deref(),
                ),
                (
                    compat.radius,
                    compat.color.as_deref(),
                    compat.stroke_color.as_deref(),
                    compat.stroke_width.as_deref(),
                ),
                "Quadrant class {name} drift",
            );
        }
        let typed = serde_json::to_value(typed).expect("Quadrant typed model serializes");
        for field in [
            "title",
            "accTitle",
            "accDescr",
            "quadrants",
            "axes",
            "points",
        ] {
            assert_eq!(typed[field], combined_json[field], "Quadrant {field} drift");
        }
    }

    #[test]
    fn quoted_semicolons_and_inline_acc_descr_block_share_statement_scanner() {
        let engine = MermaidEngine::new();
        let text = concat!(
            "quadrantChart\n",
            "title \"Plan; Execute\"\n",
            "accDescr {first %% literal; second}; quadrant-1 \"Build; Learn\"\n",
            "\"Point; A\": [0.2, 0.8]\n",
        );
        let parsed = engine
            .parse_diagram_sync(text, ParseOptions::strict())
            .expect("quoted semicolons parse")
            .expect("quadrant model");
        #[cfg(feature = "editor-facts")]
        let facts = crate::family::test_support::editor_facts_from_frozen(
            parse_quadrant_chart_json_and_editor_facts,
            text,
            &parsed.meta,
        );

        assert_eq!(parsed.model["title"], json!("\"Plan; Execute\""));
        assert_eq!(parsed.model["accDescr"], json!("first %% literal; second"));
        assert_eq!(
            parsed.model["quadrants"]["quadrant1Text"],
            json!("Build; Learn")
        );
        assert_eq!(parsed.model["points"][0]["text"], json!("Point; A"));
        #[cfg(feature = "editor-facts")]
        for expected in [
            "\"Plan; Execute\"",
            "first %% literal; second",
            "Build; Learn",
            "Point; A",
        ] {
            assert!(
                facts.symbols.iter().any(|symbol| symbol.name == expected),
                "missing source-backed fact for {expected}"
            );
        }
    }

    #[test]
    fn multiline_acc_descr_preserves_original_source_span() {
        let engine = MermaidEngine::new();
        let text = concat!(
            "quadrantChart\n",
            "accDescr {\n",
            "  First line\n",
            "  Second line\n",
            "}\n",
        );
        let parsed = engine
            .parse_diagram_sync(text, ParseOptions::strict())
            .expect("multiline accDescr parses")
            .expect("quadrant model");
        assert_eq!(parsed.model["accDescr"], json!("First line\nSecond line"));
        #[cfg(feature = "editor-facts")]
        {
            let facts = crate::family::test_support::editor_facts_from_frozen(
                parse_quadrant_chart_json_and_editor_facts,
                text,
                &parsed.meta,
            );
            let description = facts
                .symbols
                .iter()
                .find(|symbol| {
                    symbol.detail.as_deref() == Some("quadrant chart accessibility description")
                })
                .expect("multiline accessibility description fact");
            let content_start = text.find('{').expect("opening brace") + 1;
            let content_end = text.rfind('}').expect("closing brace");

            assert_eq!(
                description.selection,
                SourceSpan::new(content_start, content_end)
            );
        }
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn malformed_point_recovers_prior_parser_facts() {
        let engine = MermaidEngine::new();
        let text = "quadrantChart\nx-axis Low --> High\nBroken: [1.2, 0.4]\n";
        let statement_start = text.find("Broken").expect("malformed statement");
        let statement_end = statement_start + "Broken: [1.2, 0.4]".len();
        reset_quadrant_syntax_construction_count();
        let facts = engine
            .parse_editor_semantic_facts_with_type_sync("quadrantChart", text)
            .expect("quadrant editor recovery succeeds")
            .expect("quadrant editor facts are available");

        assert_eq!(quadrant_syntax_construction_count(), 1);
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
        assert!(
            facts.symbols.iter().any(|symbol| {
                symbol.name == "Low" && symbol.role == EditorSemanticRole::Outline
            })
        );
        assert!(facts.diagnostics.iter().any(|diagnostic| {
            diagnostic.kind == EditorSemanticDiagnosticKind::ParserRecovery
                && diagnostic.span == Some(SourceSpan::new(statement_start, statement_end))
        }));
    }

    #[test]
    fn eof_rejects_unterminated_multiline_acc_descr_like_pinned_jison() {
        let text = "quadrantChart\naccDescr {\npartial description\n";

        reset_quadrant_syntax_construction_count();
        #[cfg(feature = "editor-facts")]
        let snapshot = MermaidEngine::new()
            .parse_diagram_snapshot_with_type_sync("quadrantChart", text)
            .expect("Quadrant snapshot operation")
            .expect("Quadrant snapshot");
        #[cfg(not(feature = "editor-facts"))]
        assert!(
            MermaidEngine::new()
                .parse_diagram_sync(text, ParseOptions::strict())
                .is_err()
        );
        assert_eq!(quadrant_syntax_construction_count(), 1);
        #[cfg(feature = "editor-facts")]
        {
            assert!(matches!(
                snapshot.outcome(),
                crate::DiagramParseOutcome::Failed(_)
            ));
            let crate::ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
                panic!("Quadrant recovery facts");
            };
            assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
            assert!(!facts.diagnostics.is_empty());
            assert!(facts.symbols.iter().any(|symbol| {
                symbol.name == "partial description" && symbol.role == EditorSemanticRole::Payload
            }));
        }
    }
}

#[cfg(test)]
mod semantic_tests {
    use super::*;

    fn metadata() -> crate::SemanticMetadata {
        crate::SemanticMetadata {
            diagram_type: "quadrantChart".to_string(),
            config: crate::MermaidConfig::empty_object(),
            semantic_config: crate::MermaidConfig::from_value(serde_json::json!({
                "securityLevel": "strict",
                "htmlLabels": true,
                "dompurifyConfig": { "FORBID_TAGS": ["b"] }
            })),
            title: None,
        }
    }

    #[test]
    fn raw_line_terminals_keep_semicolons_and_literal_comment_markers() {
        let source = concat!(
            "quadrantChart\n",
            "title Plan; quadrant-1 Hidden %% literal \"\n",
            "accTitle: Accessible; title Literal\n",
            "accDescr: Details; quadrant-2 Hidden\n",
            "quadrant-1 Visible\n",
        );
        reset_quadrant_syntax_construction_count();
        let model = parse_quadrant_chart_semantic_model_controlled(
            source,
            &metadata(),
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap();

        assert_eq!(
            model.title.as_deref(),
            Some("Plan; quadrant-1 Hidden %% literal \"")
        );
        assert_eq!(
            model.acc_title.as_deref(),
            Some("Accessible; title Literal")
        );
        assert_eq!(
            model.acc_descr.as_deref(),
            Some("Details; quadrant-2 Hidden")
        );
        assert_eq!(model.quadrants.quadrant1_text, "Visible");
        assert!(model.quadrants.quadrant2_text.is_empty());
        assert_eq!(quadrant_syntax_construction_count(), 1);
    }

    #[test]
    fn multiline_text_tokens_share_axes_points_and_legal_text_suffixes() {
        for (open, close) in [("\"", "\""), ("\"`", "`\"")] {
            let source = format!(
                "quadrantChart\nx-axis {open}Low\nEffort{close} --> High\n\
                 quadrant-1 {open}First\nSecond{close} ???\n\
                 quadrant-2 {open}Literal\\nValue{close}\n\
                 {open}Point\nName{close}:::priority: [0.2, 0.8]\n\
                 classDef priority color:#abc\n"
            );
            let model = parse_quadrant_chart_semantic_model_controlled(
                &source,
                &metadata(),
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &OperationControl::new(),
            )
            .unwrap()
            .model
            .unwrap();

            assert_eq!(model.axes.x_axis_left_text, "Low\nEffort");
            assert_eq!(model.axes.x_axis_right_text, "High");
            assert_eq!(model.quadrants.quadrant1_text, "First\nSecond ???");
            assert_eq!(model.quadrants.quadrant2_text, r"Literal\nValue");
            assert_eq!(model.points[0].text, "Point\nName");
            assert_eq!(model.points[0].class_name.as_deref(), Some("priority"));
        }
    }

    #[test]
    fn mixed_text_rejects_reserved_tokens_without_banning_alpha_suffixes() {
        for (open, close) in [("\"", "\""), ("\"`", "`\"")] {
            for (suffix, expected) in [
                (" title B", None),
                (" tail", Some("A tail")),
                (" untitled", Some("A untitled")),
            ] {
                let source = format!("quadrantChart\nquadrant-1 {open}A{close}{suffix}\n");
                let construction = parse_quadrant_chart_semantic_model_controlled(
                    &source,
                    &metadata(),
                    #[cfg(feature = "editor-facts")]
                    EditorFactsDemand::Omit,
                    &OperationControl::new(),
                )
                .unwrap();
                match expected {
                    Some(expected) => {
                        assert_eq!(
                            construction.model.unwrap().quadrants.quadrant1_text,
                            expected
                        );
                    }
                    None => assert!(construction.model.is_err(), "{source}"),
                }
            }
        }
    }

    #[test]
    fn unclosed_tokens_replay_safe_lines_with_a_linear_scan_bound() {
        for open in ["\"", "\"`"] {
            for line_count in [1, 64, 512] {
                let broken = format!("quadrant-1 {open}broken");
                let source = format!(
                    "{broken}\n{}",
                    "x-axis Low --> High\nP: [0.5, 0.5]\n".repeat(line_count)
                );
                let mut statements = QuadrantStatements::new(&source);
                let control = OperationControl::new();
                let first = statements.next_statement(&control).unwrap().unwrap();
                assert_eq!(first.source.text, broken);
                for _ in 0..line_count {
                    let axis = statements.next_statement(&control).unwrap().unwrap();
                    assert_eq!(axis.source.text, "x-axis Low --> High");
                    let point = statements.next_statement(&control).unwrap().unwrap();
                    assert_eq!(point.source.text, "P: [0.5, 0.5]");
                }
                assert!(statements.next_statement(&control).unwrap().is_none());
                assert!(statements.scanned_bytes <= source.len() * 2);

                let mut statements = QuadrantStatements::new(&source);
                statements.next_statement(&control).unwrap().unwrap();
                control.cancel();
                assert!(statements.next_statement(&control).is_err());
            }
        }
    }

    #[test]
    fn multiline_token_slices_retain_crlf_and_literal_backslashes() {
        for ending in ["\n", "\r\n"] {
            for (open, close) in [("\"", "\""), ("\"`", "`\"")] {
                let payload = format!("First{ending}Second\\nLiteral");
                let source = format!("{open}{payload}{close}; next");
                let mut statements = QuadrantStatements::new(&source);
                let statement = statements
                    .next_statement(&OperationControl::new())
                    .unwrap()
                    .unwrap();
                let parsed = parse_text_slice(statement.source).unwrap();

                assert_eq!(parsed.value, payload);
                assert_eq!(parsed.content.text, payload);
                assert_eq!(parsed.content.start, open.len());
                assert_eq!(parsed.content.end(), open.len() + payload.len());
            }
        }
    }

    #[test]
    fn malformed_text_rejects_extra_tokens_and_unclosed_strings() {
        for text in [
            "\"First\nSecond\" [junk]",
            "\"First\nSecond\" \"another token\"",
            "\"`First\nSecond`\" [junk]",
            "\"First\nSecond",
            "\"`First\nSecond",
            "\"First\\\"Second\"",
        ] {
            let source = format!("quadrantChart\nquadrant-1 {text}\n");
            let construction = parse_quadrant_chart_semantic_model_controlled(
                &source,
                &metadata(),
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &OperationControl::new(),
            )
            .unwrap();
            assert!(
                construction.model.is_err(),
                "accepted malformed text: {text}"
            );
        }
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn malformed_multiline_text_recovers_following_facts_once() {
        let source = concat!(
            "quadrantChart\n",
            "quadrant-1 \"Broken\nCaption\" [junk]\n",
            "quadrant-2 \"Valid\nCaption\"\n",
            "\"`Next\nPoint`\": [0.2, 0.8]\n",
        );
        reset_quadrant_syntax_construction_count();
        let construction = parse_quadrant_chart_semantic_model_controlled(
            source,
            &metadata(),
            EditorFactsDemand::Capture,
            &OperationControl::new(),
        )
        .unwrap();
        assert!(construction.model.is_err());
        let facts = construction.editor_facts.unwrap();
        assert_eq!(quadrant_syntax_construction_count(), 1);
        assert_eq!(
            facts.completeness,
            crate::EditorSemanticCompleteness::Recovered
        );
        for name in ["Valid\nCaption", "Next\nPoint"] {
            let symbol = facts
                .symbols
                .iter()
                .find(|symbol| symbol.name == name)
                .unwrap();
            assert_eq!(&source[symbol.selection.start..symbol.selection.end], name);
        }
        assert!(
            !facts
                .symbols
                .iter()
                .any(|symbol| symbol.name.contains("Broken"))
        );
        assert!(!facts.diagnostics.is_empty());
    }

    #[test]
    fn long_raw_lines_tokens_and_comments_observe_scanning_cancellation() {
        for prefix in ["title ", "\"", "\"`", "%% ", "accTitle "] {
            let source = format!("{prefix}{}", " ".repeat(8_192));
            let mut statements = QuadrantStatements::new(&source);
            let control = OperationControl::new();
            control.cancel_after_checkpoints(2);
            assert!(statements.next_statement(&control).is_err(), "{prefix}");
            assert!(statements.offset < source.len(), "{prefix}");
        }
    }

    #[test]
    fn semantic_points_are_authored_domain_coordinates_without_editor_work() {
        let source = concat!(
            "quadrantChart\n",
            "title Priorities\n",
            "x-axis Low --> High\n",
            "quadrant-1 Plan\n",
            "classDef priority radius:10,color:#ff0000\n",
            "<b>Idea</b>:::priority: [0.25, 0.75]\n",
        );
        reset_quadrant_syntax_construction_count();
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let model = parse_quadrant_chart_semantic_model_controlled(
            source,
            &metadata(),
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap();

        assert_eq!(model.title.as_deref(), Some("Priorities"));
        assert_eq!(model.axes.x_axis_left_text, "Low");
        assert_eq!(model.axes.x_axis_right_text, "High");
        assert_eq!(model.quadrants.quadrant1_text, "Plan");
        assert_eq!(model.points.len(), 1);
        assert_eq!(model.points[0].text, "Idea");
        assert_eq!((model.points[0].x, model.points[0].y), (0.25, 0.75));
        assert_eq!(model.points[0].class_name.as_deref(), Some("priority"));
        assert_eq!(model.classes["priority"].radius, Some(10.0));
        assert_eq!(quadrant_syntax_construction_count(), 1);
        #[cfg(feature = "editor-facts")]
        assert_eq!(
            crate::editor::test_work::read(),
            crate::editor::test_work::Counts::default()
        );
    }

    #[test]
    fn semantic_coordinate_errors_preserve_exact_spans_without_editor_work() {
        let source = "quadrantChart\r\n  Broken: [1.2, 0.4]\r\n";
        let start = source.find("Broken").unwrap();
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let error = parse_quadrant_chart_semantic_model_controlled(
            source,
            &metadata(),
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap_err();
        let Error::DiagramParse { diagnostic, .. } = error else {
            panic!("expected a quadrant parse error");
        };
        assert!(diagnostic.message().contains("invalid point coordinate"));
        assert_eq!(
            diagnostic.span(),
            Some(SourceSpan::new(start, start + "Broken: [1.2, 0.4]".len()))
        );
        assert_eq!(
            diagnostic.span_kind(),
            crate::ParseDiagnosticSpanKind::Exact
        );
        #[cfg(feature = "editor-facts")]
        assert_eq!(
            crate::editor::test_work::read(),
            crate::editor::test_work::Counts::default()
        );
    }

    #[test]
    fn semantic_entry_checks_cancellation_before_construction() {
        let control = OperationControl::new();
        control.cancel();
        reset_quadrant_syntax_construction_count();
        assert!(matches!(
            parse_quadrant_chart_semantic_model_controlled(
                "quadrantChart\n",
                &metadata(),
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &control
            ),
            Err(crate::OperationCancelled { .. })
        ));
        assert_eq!(quadrant_syntax_construction_count(), 0);
    }
    #[cfg(feature = "compat-json")]
    #[test]
    fn full_typed_and_json_requests_omit_editor_observers() {
        let meta = crate::SourcePresentationMetadata {
            diagram_type: "quadrantChart".to_string(),
            config: crate::MermaidConfig::empty_object(),
            effective_config: crate::presentation::SourcePresentationConfig::empty_object(),
            title: None,
        };
        let source = "quadrantChart\nx-axis Low --> High\nA: [0.2, 0.8]\n";
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        parse_quadrant_chart(source, &meta).unwrap();
        parse_quadrant_chart_model_for_render_controlled(
            source,
            &meta,
            &crate::OperationControl::new(),
        )
        .unwrap()
        .unwrap();
        #[cfg(feature = "editor-facts")]
        assert_eq!(
            crate::editor::test_work::read(),
            crate::editor::test_work::Counts::default(),
        );
    }
}
