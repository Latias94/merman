use merman_model::families::sankey::{SankeyDiagramModel, SankeyGraph, SankeyLink, SankeyNode};
use merman_model::text::is_ecmascript_whitespace;
use merman_model::{OperationControl, OperationControlResult, SourceSpan};
use serde_json::Value;
use std::collections::HashMap;

const SANKEY_SCAN_CHECKPOINT_BYTES: usize = 4 * 1024;

#[derive(Debug, Clone)]
pub struct SankeyField {
    pub text: String,
    pub span: SourceSpan,
    pub quotes: Option<[SourceSpan; 2]>,
}

#[derive(Debug, Clone)]
pub struct SankeyRecord {
    pub source: SankeyField,
    pub target: SankeyField,
    pub value: SankeyField,
}

pub struct SankeySyntax {
    pub records: Vec<SankeyRecord>,
}

pub struct SankeySyntaxOutcome {
    pub source: SankeySyntax,
    pub errors: Vec<SankeySyntaxError>,
}

#[derive(Debug, Default, Clone)]
struct SankeyDb {
    nodes: Vec<SankeyNode>,
    nodes_map: HashMap<String, usize>,
    links: Vec<SankeyLink>,
}

impl SankeyDb {
    fn find_or_create_node(&mut self, id: String) -> String {
        if self.nodes_map.contains_key(&id) {
            return id;
        }
        let idx = self.nodes.len();
        self.nodes.push(SankeyNode { id: id.clone() });
        self.nodes_map.insert(id.clone(), idx);
        id
    }

    fn add_link(&mut self, source: String, target: String, value: Value) {
        self.links.push(SankeyLink {
            source,
            target,
            value,
        });
    }

    #[inline]
    fn into_model(self) -> SankeyDiagramModel {
        SankeyDiagramModel {
            graph: SankeyGraph {
                nodes: self.nodes,
                links: self.links,
            },
        }
    }
}

impl SankeySyntax {
    pub fn into_model_controlled(
        self,
        mut sanitize: impl FnMut(&str) -> String,
        control: &OperationControl,
    ) -> OperationControlResult<SankeyDiagramModel> {
        let mut db = SankeyDb::default();
        for (index, record) in self.records.into_iter().enumerate() {
            if index % 128 == 0 {
                control.checkpoint()?;
            }
            let source = db.find_or_create_node(sanitize(&record.source.text));
            let target = db.find_or_create_node(sanitize(&record.target.text));
            let value = parse_float_json(&record.value.text);
            db.add_link(source, target, value);
        }
        control.checkpoint()?;
        Ok(db.into_model())
    }
}

fn is_sankey_header(header: &str) -> bool {
    let h = header
        .trim_start_matches(is_ecmascript_whitespace)
        .to_ascii_lowercase();
    h == "sankey" || h == "sankey-beta"
}

fn is_sankey_text_data(ch: char) -> bool {
    matches!(ch, ' '..='!' | '#'..='+' | '-'..='~')
}

fn parse_float_json(s: &str) -> Value {
    let t = s.trim_start_matches(is_ecmascript_whitespace);
    let bytes = t.as_bytes();
    let mut end = usize::from(matches!(bytes.first(), Some(b'+') | Some(b'-')));
    if t[end..].starts_with("Infinity") {
        return Value::Null;
    }

    let integer_start = end;
    while bytes.get(end).is_some_and(u8::is_ascii_digit) {
        end += 1;
    }
    let mut has_digits = end > integer_start;
    if bytes.get(end) == Some(&b'.') {
        end += 1;
        let fraction_start = end;
        while bytes.get(end).is_some_and(u8::is_ascii_digit) {
            end += 1;
        }
        has_digits |= end > fraction_start;
    }
    if !has_digits {
        return Value::Null;
    }

    if matches!(bytes.get(end), Some(b'e') | Some(b'E')) {
        let exponent_marker = end;
        end += 1;
        if matches!(bytes.get(end), Some(b'+') | Some(b'-')) {
            end += 1;
        }
        let exponent_start = end;
        while bytes.get(end).is_some_and(u8::is_ascii_digit) {
            end += 1;
        }
        if end == exponent_start {
            end = exponent_marker;
        }
    }

    let v = t[..end].parse::<f64>().unwrap_or(f64::NAN);
    if !v.is_finite() {
        return Value::Null;
    }

    const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;
    if v.fract() == 0.0 && (-MAX_SAFE_INTEGER..=MAX_SAFE_INTEGER).contains(&v) {
        return Value::Number((v as i64).into());
    }

    let Some(n) = serde_json::Number::from_f64(v) else {
        return Value::Null;
    };
    Value::Number(n)
}

#[derive(Debug, Clone)]
pub struct SankeySyntaxError {
    pub message: String,
    pub span: SourceSpan,
}

struct PreparedSankeyText {
    text: String,
    source_bytes: Vec<SourceSpan>,
    source_len: usize,
}

fn checkpoint_sankey_scan(
    control: &OperationControl,
    progress: usize,
    next_checkpoint: &mut usize,
) -> OperationControlResult<()> {
    if progress >= *next_checkpoint {
        control.checkpoint()?;
        *next_checkpoint = progress.saturating_add(SANKEY_SCAN_CHECKPOINT_BYTES);
    }
    Ok(())
}

impl PreparedSankeyText {
    fn new_controlled(source: &str, control: &OperationControl) -> OperationControlResult<Self> {
        control.checkpoint()?;
        // Mermaid's prepareTextForParsing trims non-newline whitespace at the source edges,
        // collapses each CR/LF run to one LF, and then trims the complete result.
        let mut start = source.len();
        let mut next_checkpoint = 0usize;
        for (index, ch) in source.char_indices() {
            checkpoint_sankey_scan(control, index, &mut next_checkpoint)?;
            if !is_ecmascript_whitespace(ch) || ch == '\n' || ch == '\r' {
                start = index;
                break;
            }
        }
        let mut end = 0usize;
        next_checkpoint = 0;
        for (index, ch) in source.char_indices().rev() {
            checkpoint_sankey_scan(
                control,
                source.len().saturating_sub(index),
                &mut next_checkpoint,
            )?;
            if !is_ecmascript_whitespace(ch) || ch == '\n' || ch == '\r' {
                end = index + ch.len_utf8();
                break;
            }
        }

        let mut text = String::with_capacity(end.saturating_sub(start));
        let mut source_bytes = Vec::with_capacity(end.saturating_sub(start));
        let mut offset = start.min(end);
        next_checkpoint = offset;
        while offset < end {
            checkpoint_sankey_scan(control, offset, &mut next_checkpoint)?;
            let ch = source[offset..end]
                .chars()
                .next()
                .expect("prepared Sankey offset must be a character boundary");
            if ch == '\n' || ch == '\r' {
                let newline_start = offset;
                offset += ch.len_utf8();
                while offset < end {
                    checkpoint_sankey_scan(control, offset, &mut next_checkpoint)?;
                    let next = source[offset..end]
                        .chars()
                        .next()
                        .expect("prepared Sankey newline offset must be a character boundary");
                    if next != '\n' && next != '\r' {
                        break;
                    }
                    offset += next.len_utf8();
                }
                text.push('\n');
                source_bytes.push(SourceSpan::new(newline_start, offset));
                continue;
            }

            text.push(ch);
            for byte in offset..offset + ch.len_utf8() {
                source_bytes.push(SourceSpan::new(byte, byte + 1));
            }
            offset += ch.len_utf8();
        }

        let trimmed_start = text.len() - text.trim_start_matches(is_ecmascript_whitespace).len();
        let trimmed_end = text.trim_end_matches(is_ecmascript_whitespace).len();
        if trimmed_start >= trimmed_end {
            return Ok(Self {
                text: String::new(),
                source_bytes: Vec::new(),
                source_len: source.len(),
            });
        }
        control.checkpoint()?;
        Ok(Self {
            text: text[trimmed_start..trimmed_end].to_string(),
            source_bytes: source_bytes[trimmed_start..trimmed_end].to_vec(),
            source_len: source.len(),
        })
    }

    fn map_span(&self, span: SourceSpan) -> SourceSpan {
        debug_assert!(span.start <= span.end);
        debug_assert!(span.end <= self.text.len());
        if span.start == span.end {
            let offset = self
                .source_bytes
                .get(span.start)
                .map(|byte| byte.start)
                .or_else(|| self.source_bytes.last().map(|byte| byte.end))
                .unwrap_or(self.source_len);
            return SourceSpan::new(offset, offset);
        }
        SourceSpan::new(
            self.source_bytes[span.start].start,
            self.source_bytes[span.end - 1].end,
        )
    }

    fn map_field(&self, mut field: SankeyField) -> SankeyField {
        field.span = self.map_span(field.span);
        field.quotes = field
            .quotes
            .map(|quotes| [self.map_span(quotes[0]), self.map_span(quotes[1])]);
        field
    }

    fn map_error(&self, mut error: SankeySyntaxError) -> SankeySyntaxError {
        error.span = self.map_span(error.span);
        error
    }
}

fn find_sankey_newline_controlled(
    input: &str,
    control: &OperationControl,
) -> OperationControlResult<Option<usize>> {
    let mut next_checkpoint = 0usize;
    for (index, byte) in input.bytes().enumerate() {
        checkpoint_sankey_scan(control, index, &mut next_checkpoint)?;
        if byte == b'\n' {
            return Ok(Some(index));
        }
    }
    control.checkpoint()?;
    Ok(None)
}

pub fn parse_sankey_syntax_outcome_controlled(
    code: &str,
    control: &OperationControl,
) -> OperationControlResult<SankeySyntaxOutcome> {
    let prepared = PreparedSankeyText::new_controlled(code, control)?;
    let Some(header_end) = find_sankey_newline_controlled(&prepared.text, control)? else {
        let span = prepared.map_span(SourceSpan::new(0, prepared.text.len()));
        return Ok(SankeySyntaxOutcome {
            source: SankeySyntax {
                records: Vec::new(),
            },
            errors: vec![SankeySyntaxError {
                message: "expected sankey header followed by csv".to_string(),
                span,
            }],
        });
    };
    let header_raw = &prepared.text[..header_end];
    let header = header_raw.trim_matches(is_ecmascript_whitespace);
    let header_start = header_raw.len().saturating_sub(
        header_raw
            .trim_start_matches(is_ecmascript_whitespace)
            .len(),
    );
    let header_span = SourceSpan::new(header_start, header_start + header.len());
    if !is_sankey_header(header) {
        let span = prepared.map_span(header_span);
        return Ok(SankeySyntaxOutcome {
            source: SankeySyntax {
                records: Vec::new(),
            },
            errors: vec![SankeySyntaxError {
                message: "expected sankey".to_string(),
                span,
            }],
        });
    }

    let mut parser = CsvParser::new(&prepared.text, header_end + 1, control);
    let mut records = Vec::new();
    let mut errors = Vec::new();
    while !parser.eof() {
        control.checkpoint()?;
        let record_start = parser.pos;
        match parser.parse_record()? {
            Ok(record) => records.push(record),
            Err(error) => {
                errors.push(prepared.map_error(error));
                parser.recover_to_next_record(record_start)?;
            }
        }
    }
    if records.is_empty() && errors.is_empty() {
        errors.push(SankeySyntaxError {
            message: "expected at least one csv record".to_string(),
            span: prepared.map_span(SourceSpan::new(header_end + 1, header_end + 1)),
        });
    }

    let mut mapped_records = Vec::with_capacity(records.len());
    for (index, record) in records.into_iter().enumerate() {
        if index % 128 == 0 {
            control.checkpoint()?;
        }
        mapped_records.push(SankeyRecord {
            source: prepared.map_field(record.source),
            target: prepared.map_field(record.target),
            value: prepared.map_field(record.value),
        });
    }

    control.checkpoint()?;
    Ok(SankeySyntaxOutcome {
        source: SankeySyntax {
            records: mapped_records,
        },
        errors,
    })
}

type ControlledSankeySyntaxResult<T> =
    OperationControlResult<std::result::Result<T, SankeySyntaxError>>;

struct CsvParser<'input, 'control> {
    input: &'input str,
    pos: usize,
    control: &'control OperationControl,
}

impl<'input, 'control> CsvParser<'input, 'control> {
    fn new(input: &'input str, pos: usize, control: &'control OperationControl) -> Self {
        Self {
            input,
            pos,
            control,
        }
    }

    fn eof(&self) -> bool {
        self.pos >= self.input.len()
    }

    fn rest(&self) -> &'input str {
        &self.input[self.pos..]
    }

    fn peek_char(&self) -> Option<char> {
        self.rest().chars().next()
    }

    fn error(&self, message: impl Into<String>, span: SourceSpan) -> SankeySyntaxError {
        SankeySyntaxError {
            message: message.into(),
            span,
        }
    }

    fn consume_char(&mut self, ch: char) -> std::result::Result<(), SankeySyntaxError> {
        if self.rest().starts_with(ch) {
            self.pos += ch.len_utf8();
            Ok(())
        } else {
            Err(self.error(
                format!("expected '{ch}'"),
                SourceSpan::new(self.pos, self.pos),
            ))
        }
    }

    fn try_consume_newline(&mut self) -> bool {
        if self.peek_char() == Some('\n') {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn parse_record(&mut self) -> ControlledSankeySyntaxResult<SankeyRecord> {
        self.control.checkpoint()?;
        let source = match self.parse_field()? {
            Ok(source) => source,
            Err(error) => return Ok(Err(error)),
        };
        if let Err(error) = self.consume_comma() {
            return Ok(Err(error));
        }
        let target = match self.parse_field()? {
            Ok(target) => target,
            Err(error) => return Ok(Err(error)),
        };
        if let Err(error) = self.consume_comma() {
            return Ok(Err(error));
        }
        let value = match self.parse_field()? {
            Ok(value) => value,
            Err(error) => return Ok(Err(error)),
        };
        if !self.try_consume_newline() && !self.eof() {
            let end = find_sankey_newline_controlled(self.rest(), self.control)?
                .map_or(self.input.len(), |end| self.pos + end);
            return Ok(Err(
                self.error("expected end of record", SourceSpan::new(self.pos, end))
            ));
        }
        Ok(Ok(SankeyRecord {
            source,
            target,
            value,
        }))
    }

    fn consume_comma(&mut self) -> std::result::Result<(), SankeySyntaxError> {
        self.consume_char(',')?;
        Ok(())
    }

    fn parse_field(&mut self) -> ControlledSankeySyntaxResult<SankeyField> {
        match self.peek_char() {
            Some('"') => self.parse_quoted_field(),
            Some(',') | Some('\n') | None => Ok(Err(self.error(
                "expected a CSV field",
                SourceSpan::new(self.pos, self.pos + usize::from(!self.eof())),
            ))),
            _ => self.parse_unquoted_field(),
        }
    }

    fn parse_unquoted_field(&mut self) -> ControlledSankeySyntaxResult<SankeyField> {
        let start = self.pos;
        let mut next_checkpoint = self.pos;
        while let Some(ch) = self.peek_char() {
            checkpoint_sankey_scan(self.control, self.pos, &mut next_checkpoint)?;
            if ch == ',' || ch == '\n' {
                break;
            }
            if ch == '"' {
                return Ok(Err(self.error(
                    "unexpected quote in unquoted field",
                    SourceSpan::new(self.pos, self.pos + ch.len_utf8()),
                )));
            }
            if !is_sankey_text_data(ch) {
                return Ok(Err(self.error(
                    "invalid character in unquoted field",
                    SourceSpan::new(self.pos, self.pos + ch.len_utf8()),
                )));
            }
            self.pos += ch.len_utf8();
        }
        let raw = &self.input[start..self.pos];
        let text = raw.trim_matches(is_ecmascript_whitespace);
        let leading = raw.len() - raw.trim_start_matches(is_ecmascript_whitespace).len();
        Ok(Ok(SankeyField {
            text: text.to_string(),
            span: SourceSpan::new(start + leading, start + leading + text.len()),
            quotes: None,
        }))
    }

    fn parse_quoted_field(&mut self) -> ControlledSankeySyntaxResult<SankeyField> {
        let quote_start = self.pos;
        if let Err(error) = self.consume_char('"') {
            return Ok(Err(error));
        }
        let content_start = self.pos;
        let mut out = String::new();
        let mut next_checkpoint = self.pos;
        while let Some(ch) = self.peek_char() {
            checkpoint_sankey_scan(self.control, self.pos, &mut next_checkpoint)?;
            let char_start = self.pos;
            self.pos += ch.len_utf8();
            if ch == '"' {
                if self.peek_char() == Some('"') {
                    self.pos += 1;
                    out.push('"');
                    continue;
                }
                if char_start == content_start {
                    return Ok(Err(self.error(
                        "expected quoted CSV field content",
                        SourceSpan::new(char_start, self.pos),
                    )));
                }
                let raw = &self.input[content_start..char_start];
                let trimmed = raw.trim_matches(is_ecmascript_whitespace);
                let leading = raw.len() - raw.trim_start_matches(is_ecmascript_whitespace).len();
                return Ok(Ok(SankeyField {
                    text: out.trim_matches(is_ecmascript_whitespace).to_string(),
                    span: SourceSpan::new(
                        content_start + leading,
                        content_start + leading + trimmed.len(),
                    ),
                    quotes: Some([
                        SourceSpan::new(quote_start, quote_start + 1),
                        SourceSpan::new(char_start, char_start + 1),
                    ]),
                }));
            }
            if !is_sankey_text_data(ch) && !matches!(ch, ',' | '\r' | '\n') {
                return Ok(Err(self.error(
                    "invalid character in quoted field",
                    SourceSpan::new(char_start, self.pos),
                )));
            }
            out.push(ch);
        }
        Ok(Err(self.error(
            "unterminated quoted field",
            SourceSpan::new(quote_start, self.input.len()),
        )))
    }

    fn recover_to_next_record(&mut self, record_start: usize) -> OperationControlResult<()> {
        let search_start = self.pos.max(record_start).min(self.input.len());
        let mut next_checkpoint = 0usize;
        for (relative, byte) in self.input[search_start..].bytes().enumerate() {
            checkpoint_sankey_scan(self.control, relative, &mut next_checkpoint)?;
            if byte == b'\n' {
                self.pos = search_start + relative + 1;
                return Ok(());
            }
        }
        self.pos = self.input.len();
        self.control.checkpoint()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_parser_can_cancel_inside_a_large_quoted_field() {
        let input = format!("\"{}\",target,1", "a".repeat(12 * 1024));
        let control = OperationControl::new();
        control.cancel_after_checkpoints(2);
        let mut parser = CsvParser::new(&input, 0, &control);

        assert!(matches!(
            parser.parse_record(),
            Err(merman_model::OperationCancelled { .. })
        ));
    }
}
