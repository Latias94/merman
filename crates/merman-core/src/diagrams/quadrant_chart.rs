use crate::diagrams::scan::{leading_whitespace_len, strip_line_ending};
use crate::sanitize::sanitize_text;
use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
    EditorSemanticSymbol, Error, MermaidConfig, ParseMetadata, Result, SourceSpan,
};
use serde_json::{Map, Value, json};
#[cfg(test)]
use std::cell::Cell;
use std::collections::{BTreeMap, HashMap};

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

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuadrantChartStyles {
    pub radius: Option<i64>,
    pub color: Option<String>,
    pub stroke_color: Option<String>,
    pub stroke_width: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuadrantChartPointModel {
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub class_name: Option<String>,
    pub styles: QuadrantChartStyles,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuadrantChartQuadrantsModel {
    pub quadrant1_text: String,
    pub quadrant2_text: String,
    pub quadrant3_text: String,
    pub quadrant4_text: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuadrantChartAxesModel {
    pub x_axis_left_text: String,
    pub x_axis_right_text: String,
    pub y_axis_bottom_text: String,
    pub y_axis_top_text: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct QuadrantChartRenderModel {
    pub title: Option<String>,
    #[serde(rename = "accTitle")]
    pub acc_title: Option<String>,
    #[serde(rename = "accDescr")]
    pub acc_descr: Option<String>,
    pub quadrants: QuadrantChartQuadrantsModel,
    pub axes: QuadrantChartAxesModel,
    pub points: Vec<QuadrantChartPointModel>,
    pub classes: BTreeMap<String, QuadrantChartStyles>,
}

impl QuadrantChartRenderModel {
    pub(crate) fn sanitize_common_db_fields(&mut self, config: &crate::MermaidConfig) {
        crate::common_db::sanitize_optional_title(&mut self.title, config);
        crate::common_db::sanitize_optional_acc_title(&mut self.acc_title, config);
        crate::common_db::sanitize_optional_acc_descr(&mut self.acc_descr, config);
    }
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
    model: QuadrantChartRenderModel,
    editor_facts: EditorSemanticFacts,
}

struct QuadrantSemanticFailure {
    error: Box<Error>,
    editor_facts: Box<EditorSemanticFacts>,
}

impl QuadrantSemanticFailure {
    fn new(error: Error, editor_facts: EditorSemanticFacts) -> Self {
        Self {
            error: Box::new(error),
            editor_facts: Box::new(editor_facts),
        }
    }

    fn into_error(self) -> Error {
        *self.error
    }

    fn into_editor_facts(mut self) -> EditorSemanticFacts {
        let (message, span) = match self.error.as_ref() {
            Error::DiagramParse { diagnostic, .. } => {
                (diagnostic.message().to_string(), diagnostic.span())
            }
            error => (error.to_string(), None),
        };
        self.editor_facts.mark_recovered_from_parse_error(
            format!("quadrant chart parser recovered after parse error: {message}"),
            span,
        );
        *self.editor_facts
    }
}

fn parse_styles(styles: &[String]) -> Result<QuadrantChartStyles> {
    let mut out = QuadrantChartStyles::default();
    for raw in styles {
        let style = raw.trim();
        if style.is_empty() {
            continue;
        }
        let (key, value) = style.split_once(':').ok_or_else(|| {
            Error::diagram_parse_fallback(
                "quadrantChart".to_string(),
                format!("style named {style} is not supported."),
            )
        })?;
        let key = key.trim();
        let value = value.trim();

        match key {
            "radius" => {
                if !value.chars().all(|c| c.is_ascii_digit()) {
                    return Err(Error::diagram_parse_fallback(
                        "quadrantChart".to_string(),
                        format!("value for {key} {value} is invalid, please use a valid number"),
                    ));
                }
                out.radius = Some(value.parse::<i64>().map_err(|e| {
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

fn strip_inline_comment(line: &str) -> &str {
    let mut in_quotes = false;
    let mut brace_depth = 0usize;
    let mut i = 0usize;
    while i + 1 < line.len() {
        let Some(ch) = next_char_at(line, i) else {
            break;
        };
        if ch == '"' {
            in_quotes = !in_quotes;
            i += 1;
            continue;
        }
        if !in_quotes && ch == '{' {
            brace_depth += 1;
        } else if !in_quotes && ch == '}' {
            brace_depth = brace_depth.saturating_sub(1);
        }
        if !in_quotes && brace_depth == 0 && line[i..].starts_with("%%") {
            return &line[..i];
        }
        i += ch.len_utf8();
    }
    line
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

fn split_axis_text(s: &str) -> Option<(String, Option<String>)> {
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
        if !in_quotes && let Some((start, end)) = is_axis_delim_at(s, i) {
            let left = s[..start].trim().to_string();
            let right = s[end..].trim().to_string();
            return Some((left, if right.is_empty() { None } else { Some(right) }));
        }
        i += ch.len_utf8();
    }
    None
}

fn parse_text_value(raw: &str) -> Result<String> {
    let t = raw.trim();
    if t.starts_with("\"`") {
        let inner = t
            .strip_prefix("\"`")
            .and_then(|v| v.strip_suffix("`\""))
            .ok_or_else(|| {
                Error::diagram_parse_fallback(
                    "quadrantChart".to_string(),
                    "unterminated markdown string".to_string(),
                )
            })?;
        return Ok(inner.to_string());
    }
    if t.starts_with('"') {
        let inner = t
            .strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .ok_or_else(|| {
                Error::diagram_parse_fallback(
                    "quadrantChart".to_string(),
                    "unterminated string".to_string(),
                )
            })?;
        return Ok(inner.to_string());
    }
    Ok(t.to_string())
}

fn parse_unit_interval_token(raw: &str) -> Result<f64> {
    let s = raw.trim();
    if s == "1" {
        return Ok(1.0);
    }
    if s == "0" {
        return Ok(0.0);
    }
    if let Some(rest) = s.strip_prefix("0.")
        && !rest.is_empty()
        && rest.chars().all(|c| c.is_ascii_digit())
    {
        return s.parse::<f64>().map_err(|e| {
            Error::diagram_parse_fallback("quadrantChart".to_string(), e.to_string())
        });
    }
    Err(Error::diagram_parse_fallback(
        "quadrantChart".to_string(),
        "invalid point coordinate".to_string(),
    ))
}

fn parse_style_list(rest: &str) -> Vec<String> {
    rest.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
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
                if c2.is_whitespace() {
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

fn parse_point_statement(line: &str) -> Result<Option<PointStatement>> {
    let Some(colon_idx) = find_point_colon(line) else {
        return Ok(None);
    };
    let head = line[..colon_idx].trim_end().to_string();
    let tail = &line[colon_idx + 1..];

    let (class_name, label_raw) = if let Some(pos) = head.rfind(":::") {
        let (a, b) = head.split_at(pos);
        let class = b.trim_start_matches(":::").trim();
        if !class.is_empty() && class.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            (Some(class.to_string()), a.to_string())
        } else {
            (None, head.clone())
        }
    } else {
        (None, head.clone())
    };

    let label = parse_text_value(label_raw.trim())?;

    let t = tail.trim_start();
    let Some(after_bracket) = t.strip_prefix('[') else {
        return Err(Error::diagram_parse_fallback(
            "quadrantChart".to_string(),
            "expected '[' after ':'".to_string(),
        ));
    };
    let (inside, after) = after_bracket.split_once(']').ok_or_else(|| {
        Error::diagram_parse_fallback(
            "quadrantChart".to_string(),
            "unterminated point coordinate; missing ']'".to_string(),
        )
    })?;

    let mut xy = inside.split(',');
    let x_raw = xy.next().unwrap_or("").trim();
    let y_raw = xy.next().unwrap_or("").trim();
    if xy.next().is_some() {
        return Err(Error::diagram_parse_fallback(
            "quadrantChart".to_string(),
            "invalid point coordinate".to_string(),
        ));
    }
    let x = parse_unit_interval_token(x_raw)?;
    let y = parse_unit_interval_token(y_raw)?;

    let styles = parse_style_list(after);
    Ok(Some((label, class_name, x, y, styles)))
}

type PointStatement = (String, Option<String>, f64, f64, Vec<String>);

fn parse_colon_value_ci(line: &str, key: &str) -> Option<String> {
    let t = line.trim_start();
    if !t
        .get(..key.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(key))
    {
        return None;
    }
    let mut rest = &t[key.len()..];
    rest = rest.trim_start();
    if !rest.starts_with(':') {
        return None;
    }
    Some(rest[1..].trim().to_string())
}

fn parse_keyword_rest_ci(line: &str, key: &str) -> Option<String> {
    let t = line.trim_start();
    if !t
        .get(..key.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(key))
    {
        return None;
    }
    let rest = &t[key.len()..];
    Some(rest.trim_start().to_string())
}

pub fn parse_quadrant_chart_editor_facts(code: &str, meta: &ParseMetadata) -> EditorSemanticFacts {
    match construct_quadrant_chart_semantic_source(code, meta) {
        Ok(source) => source.editor_facts,
        Err(failure) => failure.into_editor_facts(),
    }
}

fn split_semicolons_spanned(line: &str, line_start: usize) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut in_quotes = false;
    let mut brace_depth = 0usize;
    let mut start = 0usize;
    let mut i = 0usize;
    while i < line.len() {
        let Some(ch) = next_char_at(line, i) else {
            break;
        };
        if ch == '"' {
            in_quotes = !in_quotes;
        } else if !in_quotes && ch == '{' {
            brace_depth += 1;
        } else if !in_quotes && ch == '}' {
            brace_depth = brace_depth.saturating_sub(1);
        } else if !in_quotes && brace_depth == 0 && ch == ';' {
            out.push((line_start + start, &line[start..i]));
            start = i + 1;
        }
        i += ch.len_utf8();
    }
    out.push((line_start + start, &line[start..]));
    out
}

#[derive(Debug, Clone)]
struct SpannedText {
    text: String,
    start: usize,
    end: usize,
}

impl SpannedText {
    fn as_str(&self) -> &str {
        &self.text
    }
}

fn parse_text_value_spanned(input: &str, stmt: &str, stmt_start: usize) -> Option<SpannedText> {
    let value = parse_text_value(input).ok()?;
    let value_rel = stmt.find(&value)?;
    let start = stmt_start + value_rel;
    Some(SpannedText {
        text: value.clone(),
        start,
        end: start + value.len(),
    })
}

fn push_quadrant_payload_fact(
    facts: &mut EditorSemanticFacts,
    text: &str,
    start: usize,
    end: usize,
    detail: &'static str,
    kind: EditorSemanticKind,
) {
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

fn push_quadrant_outline_fact(
    facts: &mut EditorSemanticFacts,
    text: &str,
    start: usize,
    end: usize,
    detail: &'static str,
    kind: EditorSemanticKind,
) {
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

struct QuadrantAccDescrBlock {
    text: String,
    source_start: usize,
}

fn quadrant_error_at(error: Error, meta: &ParseMetadata, span: SourceSpan) -> Error {
    let message = match error {
        Error::DiagramParse { diagnostic, .. } => diagnostic.message().to_string(),
        error => error.to_string(),
    };
    Error::diagram_parse_exact(meta.diagram_type.clone(), message, span)
}

fn quadrant_failure_at(
    error: Error,
    meta: &ParseMetadata,
    span: SourceSpan,
    editor_facts: &EditorSemanticFacts,
) -> QuadrantSemanticFailure {
    QuadrantSemanticFailure::new(quadrant_error_at(error, meta, span), editor_facts.clone())
}

fn push_quadrant_class_fact(
    facts: &mut EditorSemanticFacts,
    statement: &str,
    statement_start: usize,
    name: &str,
) {
    let keyword_start = leading_whitespace_len(statement);
    let search_start = keyword_start + "classDef".len();
    let Some(name_rel) = statement
        .get(search_start..)
        .and_then(|rest| rest.find(name))
    else {
        return;
    };
    let name_start = statement_start + search_start + name_rel;
    let span = SourceSpan::new(name_start, name_start + name.len());
    facts.push_directive_prefix("classDef");
    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::NodeIdentifier,
        span,
    ));
    facts.push_symbol(EditorSemanticSymbol::new(
        name.to_string(),
        Some("quadrant chart class".to_string()),
        EditorSemanticKind::Class,
        SourceSpan::new(statement_start, statement_start + statement.len()),
        span,
    ));
}

fn push_quadrant_point_facts(
    facts: &mut EditorSemanticFacts,
    statement: &str,
    statement_start: usize,
    label: &str,
    class_name: Option<&str>,
) {
    if let Some(label_span) = parse_text_value_spanned(label, statement, statement_start) {
        let span = SourceSpan::new(label_span.start, label_span.end);
        facts.push_expected_syntax(EditorExpectedSyntax::new(
            EditorExpectedSyntaxKind::Payload,
            span,
        ));
        facts.push_symbol(EditorSemanticSymbol::outline(
            label_span.text,
            Some("quadrant chart point".to_string()),
            EditorSemanticKind::Object,
            SourceSpan::new(statement_start, statement_start + statement.len()),
            span,
        ));
    }

    let Some(class_name) = class_name else {
        return;
    };
    let Some(marker) = statement.rfind(":::") else {
        return;
    };
    let search_start = marker + ":::".len();
    let Some(class_rel) = statement[search_start..].find(class_name) else {
        return;
    };
    let class_start = statement_start + search_start + class_rel;
    facts.push_symbol(EditorSemanticSymbol::new(
        class_name.to_string(),
        Some("quadrant chart class".to_string()),
        EditorSemanticKind::Class,
        SourceSpan::new(statement_start, statement_start + statement.len()),
        SourceSpan::new(class_start, class_start + class_name.len()),
    ));
}

fn construct_quadrant_chart_semantic_source(
    code: &str,
    meta: &ParseMetadata,
) -> std::result::Result<QuadrantSemanticSource, QuadrantSemanticFailure> {
    #[cfg(test)]
    QUADRANT_SYNTAX_CONSTRUCTION_COUNT.set(QUADRANT_SYNTAX_CONSTRUCTION_COUNT.get() + 1);

    let mut db = QuadrantDb::default();
    db.clear();
    let mut editor_facts = EditorSemanticFacts::new();
    let mut title = None;
    let mut acc_title = None;
    let mut acc_descr = None;
    let mut saw_header = false;
    let mut acc_descr_block: Option<QuadrantAccDescrBlock> = None;
    let mut offset = 0usize;

    for segment in code.split_inclusive('\n') {
        let line_start = offset;
        offset += segment.len();
        let line = strip_line_ending(segment);

        if let Some(mut block) = acc_descr_block.take() {
            if let Some(end) = line.find('}') {
                block.text.push_str(&line[..end]);
                let text = block.text.trim().to_string();
                if !text.is_empty() {
                    push_quadrant_payload_fact(
                        &mut editor_facts,
                        &text,
                        block.source_start,
                        line_start + end,
                        "quadrant chart accessibility description",
                        EditorSemanticKind::String,
                    );
                }
                acc_descr = Some(text);
            } else {
                block.text.push_str(line);
                block.text.push('\n');
                acc_descr_block = Some(block);
            }
            continue;
        }

        let stripped = strip_inline_comment(line);

        if stripped.trim().is_empty() {
            continue;
        }

        for (statement_start, statement) in split_semicolons_spanned(stripped, line_start) {
            let statement_trimmed = statement.trim();
            if statement_trimmed.is_empty() || statement_trimmed.starts_with("%%") {
                continue;
            }
            let leading = leading_whitespace_len(statement);
            let trailing = statement.len().saturating_sub(statement.trim_end().len());
            let semantic_start = statement_start + leading;
            let semantic_end = statement_start + statement.len().saturating_sub(trailing);
            let statement_span = SourceSpan::new(semantic_start, semantic_end);

            if !saw_header {
                if statement_trimmed.eq_ignore_ascii_case("quadrantChart") {
                    saw_header = true;
                    continue;
                }
                return Err(QuadrantSemanticFailure::new(
                    Error::diagram_parse_exact(
                        meta.diagram_type.clone(),
                        "expected quadrantChart",
                        statement_span,
                    ),
                    editor_facts,
                ));
            }

            if let Some(value) = parse_colon_value_ci(statement_trimmed, "accTitle") {
                editor_facts.push_directive_prefix("accTitle");
                if let Some(source) = parse_text_value_spanned(&value, statement, statement_start) {
                    push_quadrant_payload_fact(
                        &mut editor_facts,
                        source.as_str(),
                        source.start,
                        source.end,
                        "quadrant chart accessibility title",
                        EditorSemanticKind::String,
                    );
                }
                acc_title = Some(value);
                continue;
            }

            if let Some(rest) = parse_keyword_rest_ci(statement_trimmed, "accDescr") {
                let rest = rest.trim_start();
                if rest.starts_with('{') {
                    editor_facts.push_directive_prefix("accDescr");
                    let brace = statement.find('{').unwrap_or(statement.len());
                    let content_start = statement_start + brace + 1;
                    let after_brace = &statement[brace.saturating_add(1).min(statement.len())..];
                    if let Some(end) = after_brace.find('}') {
                        let text = after_brace[..end].trim().to_string();
                        if !text.is_empty() {
                            push_quadrant_payload_fact(
                                &mut editor_facts,
                                &text,
                                content_start,
                                content_start + end,
                                "quadrant chart accessibility description",
                                EditorSemanticKind::String,
                            );
                        }
                        acc_descr = Some(text);
                    } else {
                        let mut text = after_brace.trim_start().to_string();
                        if !text.is_empty() {
                            text.push('\n');
                        }
                        acc_descr_block = Some(QuadrantAccDescrBlock {
                            text,
                            source_start: content_start,
                        });
                    }
                    continue;
                }
                if let Some(value) = rest.strip_prefix(':') {
                    let value = value.trim().to_string();
                    editor_facts.push_directive_prefix("accDescr");
                    if let Some(source) =
                        parse_text_value_spanned(&value, statement, statement_start)
                    {
                        push_quadrant_payload_fact(
                            &mut editor_facts,
                            source.as_str(),
                            source.start,
                            source.end,
                            "quadrant chart accessibility description",
                            EditorSemanticKind::String,
                        );
                    }
                    acc_descr = Some(value);
                    continue;
                }
            }

            if let Some(rest) = parse_keyword_rest_ci(statement_trimmed, "title") {
                let value = rest.trim().to_string();
                editor_facts.push_directive_prefix("title");
                if let Some(source) = parse_text_value_spanned(&value, statement, statement_start) {
                    push_quadrant_payload_fact(
                        &mut editor_facts,
                        source.as_str(),
                        source.start,
                        source.end,
                        "quadrant chart title",
                        EditorSemanticKind::String,
                    );
                }
                title = Some(value);
                continue;
            }

            if let Some(rest) = parse_keyword_rest_ci(statement_trimmed, "x-axis") {
                let rest = rest.trim_start();
                if let Some((left_raw, right_raw)) = split_axis_text(rest) {
                    if let Some(left) =
                        parse_text_value_spanned(&left_raw, statement, statement_start)
                    {
                        push_quadrant_outline_fact(
                            &mut editor_facts,
                            left.as_str(),
                            left.start,
                            left.end,
                            "quadrant chart x-axis",
                            EditorSemanticKind::String,
                        );
                    }
                    if let Some(right_raw) = right_raw.as_ref()
                        && let Some(right) =
                            parse_text_value_spanned(right_raw, statement, statement_start)
                    {
                        push_quadrant_outline_fact(
                            &mut editor_facts,
                            right.as_str(),
                            right.start,
                            right.end,
                            "quadrant chart x-axis",
                            EditorSemanticKind::String,
                        );
                    }

                    let mut left = parse_text_value(&left_raw).map_err(|error| {
                        quadrant_failure_at(error, meta, statement_span, &editor_facts)
                    })?;
                    if right_raw.is_none() {
                        left.push_str(" ⟶");
                    }
                    db.set_x_axis_left(&left, &meta.effective_config);
                    if let Some(right_raw) = right_raw {
                        let right = parse_text_value(&right_raw).map_err(|error| {
                            quadrant_failure_at(error, meta, statement_span, &editor_facts)
                        })?;
                        db.set_x_axis_right(&right, &meta.effective_config);
                    }
                } else {
                    if let Some(source) = parse_text_value_spanned(rest, statement, statement_start)
                    {
                        push_quadrant_outline_fact(
                            &mut editor_facts,
                            source.as_str(),
                            source.start,
                            source.end,
                            "quadrant chart x-axis",
                            EditorSemanticKind::String,
                        );
                    }
                    let left = parse_text_value(rest).map_err(|error| {
                        quadrant_failure_at(error, meta, statement_span, &editor_facts)
                    })?;
                    db.set_x_axis_left(&left, &meta.effective_config);
                }
                continue;
            }

            if let Some(rest) = parse_keyword_rest_ci(statement_trimmed, "y-axis") {
                let rest = rest.trim_start();
                if let Some((bottom_raw, top_raw)) = split_axis_text(rest) {
                    if let Some(bottom) =
                        parse_text_value_spanned(&bottom_raw, statement, statement_start)
                    {
                        push_quadrant_outline_fact(
                            &mut editor_facts,
                            bottom.as_str(),
                            bottom.start,
                            bottom.end,
                            "quadrant chart y-axis",
                            EditorSemanticKind::String,
                        );
                    }
                    if let Some(top_raw) = top_raw.as_ref()
                        && let Some(top) =
                            parse_text_value_spanned(top_raw, statement, statement_start)
                    {
                        push_quadrant_outline_fact(
                            &mut editor_facts,
                            top.as_str(),
                            top.start,
                            top.end,
                            "quadrant chart y-axis",
                            EditorSemanticKind::String,
                        );
                    }

                    let mut bottom = parse_text_value(&bottom_raw).map_err(|error| {
                        quadrant_failure_at(error, meta, statement_span, &editor_facts)
                    })?;
                    if top_raw.is_none() {
                        bottom.push_str(" ⟶");
                    }
                    db.set_y_axis_bottom(&bottom, &meta.effective_config);
                    if let Some(top_raw) = top_raw {
                        let top = parse_text_value(&top_raw).map_err(|error| {
                            quadrant_failure_at(error, meta, statement_span, &editor_facts)
                        })?;
                        db.set_y_axis_top(&top, &meta.effective_config);
                    }
                } else {
                    if let Some(source) = parse_text_value_spanned(rest, statement, statement_start)
                    {
                        push_quadrant_outline_fact(
                            &mut editor_facts,
                            source.as_str(),
                            source.start,
                            source.end,
                            "quadrant chart y-axis",
                            EditorSemanticKind::String,
                        );
                    }
                    let bottom = parse_text_value(rest).map_err(|error| {
                        quadrant_failure_at(error, meta, statement_span, &editor_facts)
                    })?;
                    db.set_y_axis_bottom(&bottom, &meta.effective_config);
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
                let Some(rest) = parse_keyword_rest_ci(statement_trimmed, keyword) else {
                    continue;
                };
                if let Some(source) = parse_text_value_spanned(&rest, statement, statement_start) {
                    push_quadrant_outline_fact(
                        &mut editor_facts,
                        source.as_str(),
                        source.start,
                        source.end,
                        "quadrant chart quadrant",
                        EditorSemanticKind::String,
                    );
                }
                let text = parse_text_value(&rest).map_err(|error| {
                    quadrant_failure_at(error, meta, statement_span, &editor_facts)
                })?;
                db.set_quadrant_text(index, &text, &meta.effective_config);
                matched_quadrant = true;
                break;
            }
            if matched_quadrant {
                continue;
            }

            if let Some(rest) = parse_keyword_rest_ci(statement_trimmed, "classDef") {
                let mut parts = rest.trim_start().splitn(2, char::is_whitespace);
                let name = parts.next().unwrap_or("").trim();
                let style_text = parts.next().unwrap_or("").trim();
                if name.is_empty() {
                    return Err(QuadrantSemanticFailure::new(
                        Error::diagram_parse_insertion_point(
                            meta.diagram_type.clone(),
                            "expected classDef name",
                            semantic_end,
                        ),
                        editor_facts,
                    ));
                }
                push_quadrant_class_fact(&mut editor_facts, statement, statement_start, name);
                let styles = parse_style_list(style_text);
                db.add_class(name, &styles).map_err(|error| {
                    quadrant_failure_at(error, meta, statement_span, &editor_facts)
                })?;
                continue;
            }

            match parse_point_statement(statement_trimmed) {
                Ok(Some((label, class_name, x, y, styles))) => {
                    push_quadrant_point_facts(
                        &mut editor_facts,
                        statement,
                        statement_start,
                        &label,
                        class_name.as_deref(),
                    );
                    db.add_point(&label, class_name, x, y, &styles, &meta.effective_config)
                        .map_err(|error| {
                            quadrant_failure_at(error, meta, statement_span, &editor_facts)
                        })?;
                    continue;
                }
                Ok(None) => {}
                Err(error) => {
                    return Err(quadrant_failure_at(
                        error,
                        meta,
                        statement_span,
                        &editor_facts,
                    ));
                }
            }

            return Err(QuadrantSemanticFailure::new(
                Error::diagram_parse_exact(
                    meta.diagram_type.clone(),
                    format!("Unrecognized statement: {statement_trimmed}"),
                    statement_span,
                ),
                editor_facts,
            ));
        }
    }

    if let Some(block) = acc_descr_block {
        let text = block.text.trim().to_string();
        if !text.is_empty() {
            push_quadrant_payload_fact(
                &mut editor_facts,
                &text,
                block.source_start,
                code.len(),
                "quadrant chart accessibility description",
                EditorSemanticKind::String,
            );
        }
        acc_descr = Some(text);
    }

    if !saw_header {
        return Err(QuadrantSemanticFailure::new(
            Error::diagram_parse_insertion_point(
                meta.diagram_type.clone(),
                "expected quadrantChart",
                code.len(),
            ),
            editor_facts,
        ));
    }

    Ok(QuadrantSemanticSource {
        model: QuadrantChartRenderModel {
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
        editor_facts,
    })
}

pub fn parse_quadrant_chart(code: &str, meta: &ParseMetadata) -> Result<Value> {
    let source = construct_quadrant_chart_semantic_source(code, meta)
        .map_err(QuadrantSemanticFailure::into_error)?;
    render_model_to_compat_json(&source.model, meta)
}

pub(crate) fn parse_quadrant_chart_json_and_editor_facts(
    code: &str,
    meta: &ParseMetadata,
) -> Result<(Value, EditorSemanticFacts)> {
    let QuadrantSemanticSource {
        model,
        editor_facts,
    } = construct_quadrant_chart_semantic_source(code, meta)
        .map_err(QuadrantSemanticFailure::into_error)?;
    Ok((render_model_to_compat_json(&model, meta)?, editor_facts))
}

pub(crate) fn render_model_to_compat_json(
    model: &QuadrantChartRenderModel,
    meta: &ParseMetadata,
) -> Result<Value> {
    let mut out = Map::with_capacity(9);
    out.insert("type".to_string(), Value::String(meta.diagram_type.clone()));
    out.insert("title".to_string(), json!(&model.title));
    out.insert("accTitle".to_string(), json!(&model.acc_title));
    out.insert("accDescr".to_string(), json!(&model.acc_descr));
    out.insert("quadrants".to_string(), json!(&model.quadrants));
    out.insert("axes".to_string(), json!(&model.axes));
    out.insert("points".to_string(), json!(&model.points));
    out.insert("classes".to_string(), json!(&model.classes));
    out.insert(
        "config".to_string(),
        crate::config::clone_value_nonrecursive(meta.effective_config.as_value()),
    );
    Ok(Value::Object(out))
}

pub fn parse_quadrant_chart_model_for_render(
    code: &str,
    meta: &ParseMetadata,
) -> Result<QuadrantChartRenderModel> {
    construct_quadrant_chart_semantic_source(code, meta)
        .map(|source| source.model)
        .map_err(QuadrantSemanticFailure::into_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated;
    use crate::{
        EditorSemanticCompleteness, EditorSemanticDiagnosticKind, EditorSemanticRole, Engine,
        ParseOptions, RenderSemanticModel,
    };
    use futures::executor::block_on;

    fn parse(text: &str) -> Value {
        let engine = Engine::new();
        block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap()
            .model
    }

    fn parse_err(text: &str) -> String {
        let engine = Engine::new();
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
        let meta = ParseMetadata {
            diagram_type: "quadrantChart".to_string(),
            config: MermaidConfig::default(),
            effective_config: generated::default_site_config(),
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
        let engine = Engine::new();
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

        assert_eq!(parsed.meta.diagram_type, "quadrantChart");
        match parsed.model {
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
                assert_eq!(model.classes["priority"].radius, Some(10));
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
        assert_eq!(obj.radius, Some(10));
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
        let engine = Engine::new();
        let text = concat!(
            "quadrantChart\n",
            "title Delivery portfolio\n",
            "accTitle: Portfolio\n",
            "x-axis Low --> High\n",
            "y-axis Bottom --> Top\n",
            "quadrant-1 Invest\n",
            "classDef priority color: #109060, radius: 10\n",
            "Project A:::priority: [0.2, 0.8]\n",
        );
        let parsed = engine
            .parse_diagram_sync(text, ParseOptions::strict())
            .expect("standalone Quadrant JSON parse succeeds")
            .expect("standalone Quadrant JSON parse returns a diagram");
        let standalone_editor = parse_quadrant_chart_editor_facts(text, &parsed.meta);

        reset_quadrant_syntax_construction_count();
        parse_quadrant_chart(text, &parsed.meta).expect("Quadrant JSON projection succeeds");
        assert_eq!(quadrant_syntax_construction_count(), 1);

        reset_quadrant_syntax_construction_count();
        let typed = parse_quadrant_chart_model_for_render(text, &parsed.meta)
            .expect("Quadrant typed projection succeeds");
        assert_eq!(quadrant_syntax_construction_count(), 1);

        reset_quadrant_syntax_construction_count();
        parse_quadrant_chart_editor_facts(text, &parsed.meta);
        assert_eq!(quadrant_syntax_construction_count(), 1);

        reset_quadrant_syntax_construction_count();
        let (combined_json, combined_editor) =
            parse_quadrant_chart_json_and_editor_facts(text, &parsed.meta)
                .expect("Quadrant combined projection succeeds");
        assert_eq!(quadrant_syntax_construction_count(), 1);
        assert_eq!(combined_json, parsed.model);
        assert_eq!(combined_editor, standalone_editor);
        assert_eq!(
            render_model_to_compat_json(&typed, &parsed.meta).unwrap(),
            combined_json
        );
        assert_eq!(combined_json["type"], json!("quadrantChart"));
        assert!(combined_json["config"].is_object());
        assert_eq!(combined_json["accDescr"], Value::Null);

        let typed = serde_json::to_value(typed).expect("Quadrant typed model serializes");
        for field in [
            "title",
            "accTitle",
            "accDescr",
            "quadrants",
            "axes",
            "points",
            "classes",
        ] {
            assert_eq!(typed[field], combined_json[field], "Quadrant {field} drift");
        }
    }

    #[test]
    fn quoted_semicolons_and_inline_acc_descr_block_share_statement_scanner() {
        let engine = Engine::new();
        let text = concat!(
            "quadrantChart\n",
            "title \"Plan; Execute\"; accDescr {first %% literal; second}; quadrant-1 \"Build; Learn\"\n",
            "\"Point; A\": [0.2, 0.8]\n",
        );
        let parsed = engine
            .parse_diagram_sync(text, ParseOptions::strict())
            .expect("quoted semicolons parse")
            .expect("quadrant model");
        let facts = parse_quadrant_chart_editor_facts(text, &parsed.meta);

        assert_eq!(parsed.model["title"], json!("\"Plan; Execute\""));
        assert_eq!(parsed.model["accDescr"], json!("first %% literal; second"));
        assert_eq!(
            parsed.model["quadrants"]["quadrant1Text"],
            json!("Build; Learn")
        );
        assert_eq!(parsed.model["points"][0]["text"], json!("Point; A"));
        for expected in [
            "Plan; Execute",
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
        let engine = Engine::new();
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
        let facts = parse_quadrant_chart_editor_facts(text, &parsed.meta);
        let description = facts
            .symbols
            .iter()
            .find(|symbol| {
                symbol.detail.as_deref() == Some("quadrant chart accessibility description")
            })
            .expect("multiline accessibility description fact");
        let content_start = text.find('{').expect("opening brace") + 1;
        let content_end = text.rfind('}').expect("closing brace");

        assert_eq!(parsed.model["accDescr"], json!("First line\nSecond line"));
        assert_eq!(
            description.selection,
            SourceSpan::new(content_start, content_end)
        );
    }

    #[test]
    fn malformed_point_recovers_prior_parser_facts() {
        let engine = Engine::new();
        let text = "quadrantChart\nx-axis Low --> High\nBroken: [1.2, 0.4]\n";
        let statement_start = text.find("Broken").expect("malformed statement");
        let statement_end = statement_start + "Broken: [1.2, 0.4]".len();
        reset_quadrant_syntax_construction_count();
        let facts = engine
            .parse_editor_semantic_facts_with_type_sync(
                "quadrantChart",
                text,
                ParseOptions::strict(),
            )
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
    fn eof_terminates_multiline_acc_descr_like_pinned_jison() {
        let engine = Engine::new();
        let text = "quadrantChart\naccDescr {\npartial description\n";
        let parsed = engine
            .parse_diagram_sync(text, ParseOptions::strict())
            .expect("pinned Jison accepts EOF in the multiline accessibility state")
            .expect("quadrant model");
        assert_eq!(parsed.model["accDescr"], json!("partial description"));

        reset_quadrant_syntax_construction_count();
        let facts = engine
            .parse_editor_semantic_facts_with_type_sync(
                "quadrantChart",
                text,
                ParseOptions::strict(),
            )
            .expect("Quadrant editor recovery succeeds")
            .expect("Quadrant editor facts are available");
        assert_eq!(quadrant_syntax_construction_count(), 1);
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
        assert!(facts.diagnostics.is_empty());
        assert!(facts.symbols.iter().any(|symbol| {
            symbol.name == "partial description" && symbol.role == EditorSemanticRole::Payload
        }));
    }
}
