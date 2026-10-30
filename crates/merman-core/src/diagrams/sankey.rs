#[cfg(feature = "editor-facts")]
use crate::family::EditorFactsDemand;
use crate::sanitize::sanitize_text;
#[cfg(feature = "editor-facts")]
use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
    EditorSemanticSymbol,
};
use crate::{Error, SourceSpan, family};
#[cfg(feature = "presentation")]
use crate::{PresentationMetadata, Result};
use serde_json::Value;
#[cfg(feature = "compat-json")]
use serde_json::{Map, json};
#[cfg(test)]
use std::cell::Cell;
use std::collections::HashMap;

const SANKEY_SCAN_CHECKPOINT_BYTES: usize = 4 * 1024;

#[cfg(test)]
thread_local! {
    static SANKEY_SYNTAX_CONSTRUCTION_COUNT: Cell<usize> = const { Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_sankey_syntax_construction_count() {
    SANKEY_SYNTAX_CONSTRUCTION_COUNT.set(0);
}

#[cfg(test)]
pub(crate) fn sankey_syntax_construction_count() -> usize {
    SANKEY_SYNTAX_CONSTRUCTION_COUNT.get()
}

pub use merman_model::families::sankey::{SankeyDiagramModel, SankeyGraph, SankeyLink, SankeyNode};

#[derive(Debug, Default, Clone)]
struct SankeyDb {
    nodes: Vec<SankeyNode>,
    nodes_map: HashMap<String, usize>,
    links: Vec<SankeyLink>,
}

impl SankeyDb {
    fn find_or_create_node(&mut self, id_raw: &str, config: &crate::MermaidConfig) -> String {
        let id = sanitize_text(id_raw, config);
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

#[derive(Debug, Clone)]
struct SankeyField {
    text: String,
    span: SourceSpan,
    quotes: Option<[SourceSpan; 2]>,
}

#[derive(Debug, Clone)]
struct SankeyRecord {
    source: SankeyField,
    target: SankeyField,
    value: SankeyField,
}

struct SankeySemanticSource {
    records: Vec<SankeyRecord>,
}

struct SankeySemanticConstruction {
    model: SankeyDiagramModel,
    #[cfg(feature = "editor-facts")]
    editor_facts: Option<EditorSemanticFacts>,
}

struct SankeySyntaxOutcome {
    source: SankeySemanticSource,
    errors: Vec<SankeySyntaxError>,
}

impl SankeySemanticSource {
    #[cfg(feature = "editor-facts")]
    fn editor_facts_controlled(
        &self,
        _source: &str,
        control: &crate::OperationControl,
    ) -> crate::OperationControlResult<EditorSemanticFacts> {
        let mut facts = EditorSemanticFacts::new();
        for (index, record) in self.records.iter().enumerate() {
            if index % 128 == 0 {
                control.checkpoint()?;
            }
            push_sankey_payload(
                &mut facts,
                &record.source,
                "sankey source",
                EditorSemanticKind::Namespace,
                false,
            );
            push_sankey_payload(
                &mut facts,
                &record.target,
                "sankey target",
                EditorSemanticKind::Namespace,
                false,
            );
            push_sankey_payload(
                &mut facts,
                &record.value,
                "sankey link value",
                EditorSemanticKind::String,
                true,
            );
        }
        Ok(facts)
    }

    fn into_model_controlled(
        self,
        config: &crate::MermaidConfig,
        control: &crate::OperationControl,
    ) -> crate::OperationControlResult<SankeyDiagramModel> {
        let mut db = SankeyDb::default();
        for (index, record) in self.records.into_iter().enumerate() {
            if index % 128 == 0 {
                control.checkpoint()?;
            }
            let source_raw = normalize_field_value(&record.source.text);
            let target_raw = normalize_field_value(&record.target.text);
            let source = db.find_or_create_node(&source_raw, config);
            let target = db.find_or_create_node(&target_raw, config);
            let value = parse_float_json(record.value.text.trim());
            db.add_link(source, target, value);
        }
        control.checkpoint()?;
        Ok(db.into_model())
    }
}

#[cfg(feature = "compat-json")]
pub(crate) fn render_model_to_compat_json(
    model: &SankeyDiagramModel,
    meta: &PresentationMetadata,
) -> Result<Value> {
    let mut out = Map::with_capacity(3);
    out.insert("type".to_string(), Value::String(meta.diagram_type.clone()));
    out.insert("graph".to_string(), json!(&model.graph));
    out.insert(
        "config".to_string(),
        crate::json_value::clone_value_nonrecursive(meta.effective_config.as_value()),
    );
    Ok(Value::Object(out))
}

#[cfg(feature = "compat-json")]
pub(crate) fn parse_sankey(code: &str, meta: &PresentationMetadata) -> Result<Value> {
    let model = parse_sankey_model(code, meta)?;
    render_model_to_compat_json(&model, meta)
}

#[cfg(feature = "compat-json")]
#[cfg(feature = "editor-facts")]
pub(crate) fn parse_sankey_json_and_editor_facts(
    code: &str,
    meta: &PresentationMetadata,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<family::CombinedSemanticParse> {
    let construction = construct_sankey_semantic_source_controlled(
        code,
        &meta.diagram_type,
        meta.effective_config.as_config(),
        EditorFactsDemand::Capture,
        control,
    )?;
    let parsed = family::CombinedSemanticParse::from_construction(
        construction,
        |construction| {
            (
                render_model_to_compat_json(&construction.model, meta),
                construction.editor_facts.unwrap_or_default(),
            )
        },
        family::CombinedSemanticFailure::into_parts,
    );
    control.checkpoint()?;
    Ok(parsed)
}

#[cfg(all(test, feature = "presentation"))]
pub(crate) fn parse_sankey_model_for_render(
    code: &str,
    meta: &PresentationMetadata,
) -> Result<SankeyDiagramModel> {
    parse_sankey_model(code, meta)
}

#[cfg(feature = "presentation")]
pub(crate) fn parse_sankey_model_for_render_controlled(
    code: &str,
    meta: &PresentationMetadata,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<Result<SankeyDiagramModel>> {
    Ok(construct_sankey_semantic_source_controlled(
        code,
        &meta.diagram_type,
        meta.effective_config.as_config(),
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        control,
    )?
    .map(|construction| construction.model)
    .map_err(family::CombinedSemanticFailure::into_error))
}

#[cfg(any(feature = "compat-json", all(test, feature = "presentation")))]
fn parse_sankey_model(code: &str, meta: &PresentationMetadata) -> Result<SankeyDiagramModel> {
    construct_sankey_semantic_source_controlled(
        code,
        &meta.diagram_type,
        meta.effective_config.as_config(),
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        &crate::OperationControl::new(),
    )
    .expect("a private parse control cannot be cancelled")
    .map(|construction| construction.model)
    .map_err(family::CombinedSemanticFailure::into_error)
}

pub(crate) fn parse_sankey_semantic_model_controlled(
    code: &str,
    metadata: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<crate::family::SemanticConstruction<SankeyDiagramModel>> {
    let construction = construct_sankey_semantic_source_controlled(
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
            let model = source.model;
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

fn construct_sankey_semantic_source_controlled(
    code: &str,
    diagram_type: &str,
    config: &crate::MermaidConfig,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<
    std::result::Result<SankeySemanticConstruction, family::CombinedSemanticFailure>,
> {
    control.checkpoint()?;
    #[cfg(test)]
    SANKEY_SYNTAX_CONSTRUCTION_COUNT.set(SANKEY_SYNTAX_CONSTRUCTION_COUNT.get() + 1);

    let SankeySyntaxOutcome { source, errors } =
        parse_sankey_syntax_outcome_controlled(code, control)?;
    #[cfg(feature = "editor-facts")]
    let mut editor_facts = match editor_demand {
        EditorFactsDemand::Omit => None,
        EditorFactsDemand::Recovery => Some(EditorSemanticFacts::new()),
        EditorFactsDemand::Capture => {
            #[cfg(test)]
            crate::editor::test_work::record_collection();
            Some(source.editor_facts_controlled(code, control)?)
        }
    };
    #[cfg(feature = "editor-facts")]
    if let Some(facts) = editor_facts.as_mut() {
        for (index, error) in errors.iter().enumerate() {
            if index % 128 == 0 {
                control.checkpoint()?;
            }
            facts.mark_recovered_from_parse_error(error.message.clone(), Some(error.span));
        }
    }

    if let Some(error) = errors.into_iter().next() {
        return Ok(Err(family::CombinedSemanticFailure::new(
            Error::diagram_parse_exact(diagram_type.to_string(), error.message, error.span),
            #[cfg(feature = "editor-facts")]
            editor_facts,
        )));
    }

    let model = source.into_model_controlled(config, control)?;
    control.checkpoint()?;
    Ok(Ok(SankeySemanticConstruction {
        model,
        #[cfg(feature = "editor-facts")]
        editor_facts,
    }))
}

#[cfg(feature = "editor-facts")]
fn push_sankey_payload(
    facts: &mut EditorSemanticFacts,
    field: &SankeyField,
    detail: &'static str,
    kind: EditorSemanticKind,
    payload: bool,
) {
    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::Payload,
        field.span,
    ));
    if field.text.is_empty() {
        return;
    }
    let symbol = if payload {
        EditorSemanticSymbol::payload(
            field.text.clone(),
            Some(detail.to_string()),
            kind,
            field.span,
            field.span,
        )
    } else {
        EditorSemanticSymbol::new(
            field.text.clone(),
            Some(detail.to_string()),
            kind,
            field.span,
            field.span,
        )
    };
    facts.push_symbol(symbol);
}

fn is_sankey_header(header: &str) -> bool {
    let h = header.trim_start().to_ascii_lowercase();
    h == "sankey" || h == "sankey-beta"
}

fn normalize_field_value(s: &str) -> String {
    // CsvParser already applies Mermaid's `replaceAll('""', '"')` while decoding a quoted field.
    s.trim().to_string()
}

fn parse_float_json(s: &str) -> Value {
    let t = s.trim_start();
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
struct SankeySyntaxError {
    message: String,
    span: SourceSpan,
}

struct PreparedSankeyText {
    text: String,
    source_bytes: Vec<SourceSpan>,
    source_len: usize,
}

fn checkpoint_sankey_scan(
    control: &crate::OperationControl,
    progress: usize,
    next_checkpoint: &mut usize,
) -> crate::OperationControlResult<()> {
    if progress >= *next_checkpoint {
        control.checkpoint()?;
        *next_checkpoint = progress.saturating_add(SANKEY_SCAN_CHECKPOINT_BYTES);
    }
    Ok(())
}

impl PreparedSankeyText {
    fn new_controlled(
        source: &str,
        control: &crate::OperationControl,
    ) -> crate::OperationControlResult<Self> {
        control.checkpoint()?;
        // Mermaid's prepareTextForParsing trims non-newline whitespace at the source edges,
        // collapses each CR/LF run to one LF, and then trims the complete result.
        let mut start = source.len();
        let mut next_checkpoint = 0usize;
        for (index, ch) in source.char_indices() {
            checkpoint_sankey_scan(control, index, &mut next_checkpoint)?;
            if !ch.is_whitespace() || ch == '\n' || ch == '\r' {
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
            if !ch.is_whitespace() || ch == '\n' || ch == '\r' {
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

        let trimmed_start = text.len() - text.trim_start().len();
        let trimmed_end = text.trim_end().len();
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
    control: &crate::OperationControl,
) -> crate::OperationControlResult<Option<usize>> {
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

fn parse_sankey_syntax_outcome_controlled(
    code: &str,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<SankeySyntaxOutcome> {
    let prepared = PreparedSankeyText::new_controlled(code, control)?;
    let Some(header_end) = find_sankey_newline_controlled(&prepared.text, control)? else {
        let span = prepared.map_span(SourceSpan::new(0, prepared.text.len()));
        return Ok(SankeySyntaxOutcome {
            source: SankeySemanticSource {
                records: Vec::new(),
            },
            errors: vec![SankeySyntaxError {
                message: "expected sankey header followed by csv".to_string(),
                span,
            }],
        });
    };
    let header_raw = &prepared.text[..header_end];
    let header = header_raw.trim();
    let header_start = header_raw
        .len()
        .saturating_sub(header_raw.trim_start().len());
    let header_span = SourceSpan::new(header_start, header_start + header.len());
    if !is_sankey_header(header) {
        let span = prepared.map_span(header_span);
        return Ok(SankeySyntaxOutcome {
            source: SankeySemanticSource {
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
        source: SankeySemanticSource {
            records: mapped_records,
        },
        errors,
    })
}

type ControlledSankeySyntaxResult<T> =
    crate::OperationControlResult<std::result::Result<T, SankeySyntaxError>>;

struct CsvParser<'input, 'control> {
    input: &'input str,
    pos: usize,
    control: &'control crate::OperationControl,
}

impl<'input, 'control> CsvParser<'input, 'control> {
    fn new(input: &'input str, pos: usize, control: &'control crate::OperationControl) -> Self {
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
            Some('\n') | None => Ok(Ok(SankeyField {
                text: String::new(),
                span: SourceSpan::new(self.pos, self.pos),
                quotes: None,
            })),
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
            self.pos += ch.len_utf8();
        }
        let raw = &self.input[start..self.pos];
        let text = raw.trim();
        let leading = raw.len() - raw.trim_start().len();
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
                let raw = &self.input[content_start..char_start];
                let trimmed = raw.trim();
                let leading = raw.len() - raw.trim_start().len();
                return Ok(Ok(SankeyField {
                    text: out.trim().to_string(),
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
            out.push(ch);
        }
        Ok(Err(self.error(
            "unterminated quoted field",
            SourceSpan::new(quote_start, self.input.len()),
        )))
    }

    fn recover_to_next_record(&mut self, record_start: usize) -> crate::OperationControlResult<()> {
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

#[cfg(all(test, feature = "compat-json"))]
mod tests {
    use super::*;
    use crate::{MermaidEngine, ParseOptions};
    use futures::executor::block_on;
    use serde_json::json;

    fn parse(text: &str) -> crate::ManagedSemanticJson {
        let engine = MermaidEngine::new();
        block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap()
            .model
    }

    #[test]
    fn csv_parser_can_cancel_inside_a_large_quoted_field() {
        let input = format!("\"{}\",target,1", "a".repeat(12 * 1024));
        let control = crate::OperationControl::new();
        control.cancel_after_checkpoints(2);
        let mut parser = CsvParser::new(&input, 0, &control);

        assert!(matches!(
            parser.parse_record(),
            Err(crate::OperationCancelled { .. })
        ));
    }

    #[test]
    fn sankey_typed_projection_matches_complete_compatibility_json() {
        let text = "sankey-beta\nA,B,1\nB,C,NaN\n";
        let effective_config = crate::presentation::PresentationConfig::from_value(json!({
            "theme": "forest",
            "securityLevel": "strict"
        }));
        let meta = PresentationMetadata {
            diagram_type: "sankey".to_string(),
            config: crate::MermaidConfig::empty_object(),
            effective_config,
            title: None,
        };
        let compat = parse_sankey(text, &meta).unwrap();
        let typed = parse_sankey_model_for_render(text, &meta).unwrap();

        assert_eq!(render_model_to_compat_json(&typed, &meta).unwrap(), compat);
        assert_eq!(compat["config"]["theme"], "forest");
        assert!(compat["graph"]["links"][1]["value"].is_null());
    }

    #[test]
    fn sankey_parses_csv_with_sankey_beta_header() {
        let model = parse(
            r#"sankey-beta

%% comment line should be removed
    Agricultural 'waste',Bio-conversion,124.729   
Bio-conversion,Liquid,0.597

%% quoted sankey keyword
"sankey",target,10

%% escaped quotes
"""Biomass imports""",Solid,35

%% commas in field
"District heating","Heating and cooling, commercial",22.505
"#,
        );

        let graph = &model["graph"];
        assert!(graph["nodes"].as_array().unwrap().len() >= 5);
        assert_eq!(
            graph["links"][0],
            json!({
                "source": "Agricultural 'waste'",
                "target": "Bio-conversion",
                "value": 124.729,
            })
        );
        assert_eq!(
            graph["links"][2],
            json!({
                "source": "sankey",
                "target": "target",
                "value": 10,
            })
        );
        assert_eq!(
            graph["links"][3],
            json!({
                "source": "\"Biomass imports\"",
                "target": "Solid",
                "value": 35,
            })
        );
        assert_eq!(
            graph["links"][4],
            json!({
                "source": "District heating",
                "target": "Heating and cooling, commercial",
                "value": 22.505,
            })
        );
    }

    #[test]
    fn sankey_parses_csv_with_sankey_header() {
        let model = parse(
            r#"sankey
A,B,0.597
"#,
        );
        assert_eq!(
            model["graph"],
            json!({
                "nodes": [{"id": "A"}, {"id": "B"}],
                "links": [{"source": "A", "target": "B", "value": 0.597}],
            })
        );
    }

    #[test]
    fn sankey_allows_proto_as_id() {
        let model = parse(
            r#"sankey-beta
__proto__,A,0.597
A,__proto__,0.403
"#,
        );
        let nodes = model["graph"]["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|n| n["id"].as_str())
            .collect::<Vec<_>>();
        assert!(nodes.contains(&"__proto__"));
    }

    #[test]
    fn sankey_decodes_each_csv_escaped_quote_once() {
        let model = parse("sankey\n\"a\"\"\"\"b\",target,1\n");
        assert_eq!(model["graph"]["links"][0]["source"], "a\"\"b");
    }

    #[test]
    fn sankey_values_follow_javascript_parse_float_prefix_semantics() {
        let model = parse("sankey\nA,B,12.5units\nB,C,1e+tail\n");
        assert_eq!(model["graph"]["links"][0]["value"], 12.5);
        assert_eq!(model["graph"]["links"][1]["value"], 1);
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn sankey_editor_facts_expose_parser_backed_spans() {
        let engine = MermaidEngine::new();
        let text = r#"sankey-beta
A,B,0.597
"#;
        let facts = engine
            .parse_editor_semantic_facts_with_type_sync("sankey", text)
            .unwrap()
            .unwrap();

        assert!(facts.symbols.iter().any(|symbol| symbol.name == "A"));
        assert!(facts.symbols.iter().any(|symbol| symbol.name == "B"));
        assert!(facts.symbols.iter().any(|symbol| symbol.name == "0.597"));

        let source_start = text.find('A').unwrap();
        assert!(facts.expected_syntax.iter().any(|expected| {
            expected.kind == EditorExpectedSyntaxKind::Payload
                && expected.span == SourceSpan::new(source_start, source_start + 1)
        }));
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn sankey_editor_recovery_preserves_original_spans_and_surrounding_records() {
        let engine = MermaidEngine::new();
        let text = concat!(
            "  sankey-beta\r\n",
            "\r\n",
            "A,B,1\r\n",
            "invalid\r\n",
            "\"C, source\",D,2\r\n",
        );
        let facts = engine
            .parse_editor_semantic_facts_with_type_sync("sankey", text)
            .unwrap()
            .expect("sankey editor recovery facts");

        assert_eq!(
            facts.completeness,
            crate::EditorSemanticCompleteness::Recovered
        );
        for name in ["A", "B", "1", "C, source", "D", "2"] {
            assert!(
                facts.symbols.iter().any(|symbol| symbol.name == name),
                "missing recovered Sankey symbol {name}"
            );
        }

        let invalid_end = text.find("invalid").unwrap() + "invalid".len();
        assert!(facts.diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("expected ','")
                && diagnostic.span == Some(SourceSpan::new(invalid_end, invalid_end))
        }));

        let quoted_start = text.find("C, source").unwrap();
        let quoted = facts
            .symbols
            .iter()
            .find(|symbol| symbol.name == "C, source")
            .expect("quoted Sankey source fact");
        assert_eq!(
            quoted.selection,
            SourceSpan::new(quoted_start, quoted_start + "C, source".len())
        );
    }

    #[test]
    fn sankey_editor_recovery_reports_missing_csv_records() {
        let text = "  sankey-beta\r\n  ";
        let header_start = text.find("sankey-beta").unwrap();
        let error_span = SourceSpan::new(header_start, header_start + "sankey-beta".len());
        let engine = MermaidEngine::new();

        let error = engine
            .parse_diagram_sync(text, ParseOptions::strict())
            .expect_err("a Sankey header without records must fail strict parsing");
        let Error::DiagramParse { diagnostic, .. } = error else {
            panic!("expected Sankey parse diagnostic");
        };
        assert_eq!(diagnostic.span(), Some(error_span));

        #[cfg(feature = "editor-facts")]
        {
            let facts = engine
                .parse_editor_semantic_facts_with_type_sync("sankey", text)
                .unwrap()
                .expect("Sankey editor recovery facts");
            assert_eq!(
                facts.completeness,
                crate::EditorSemanticCompleteness::Recovered
            );
            assert!(facts.diagnostics.iter().any(|diagnostic| {
                diagnostic.kind == crate::EditorSemanticDiagnosticKind::ParserRecovery
                    && diagnostic.span == Some(error_span)
                    && diagnostic
                        .message
                        .contains("expected sankey header followed by csv")
            }));
        }
    }
}

#[cfg(test)]
mod semantic_tests {
    use super::*;

    fn metadata() -> crate::SemanticMetadata {
        crate::SemanticMetadata {
            diagram_type: "sankey".to_string(),
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
    #[cfg(feature = "editor-facts")]
    fn recovery_capture_preserves_csv_diagnostics_without_record_facts() {
        for source in ["sankey-beta\nA,B,1\n", "sankey-beta\nA,B,1\ninvalid\n"] {
            let capture = |demand| {
                parse_sankey_semantic_model_controlled(
                    source,
                    &metadata(),
                    demand,
                    &crate::OperationControl::new(),
                )
                .unwrap()
            };
            let full = capture(EditorFactsDemand::Capture);
            let full_facts = full.editor_facts.unwrap();
            assert!(!full_facts.symbols.is_empty());
            crate::editor::test_work::reset();
            reset_sankey_syntax_construction_count();
            let recovery = capture(EditorFactsDemand::Recovery);
            assert_eq!(recovery.model.is_ok(), full.model.is_ok());
            assert_eq!(
                recovery.model.as_ref().err().map(ToString::to_string),
                full.model.as_ref().err().map(ToString::to_string),
            );
            let facts = recovery.editor_facts.unwrap();
            assert_eq!(facts.completeness, full_facts.completeness);
            assert_eq!(facts.diagnostics, full_facts.diagnostics);
            assert!(facts.symbols.is_empty());
            assert!(facts.directive_prefixes.is_empty());
            assert!(facts.expected_syntax.is_empty());
            let work = crate::editor::test_work::read();
            assert_eq!(
                (work.symbols, work.directives, work.expected_syntax),
                (0, 0, 0)
            );
            assert_eq!(sankey_syntax_construction_count(), 1);
        }
    }

    #[test]
    fn semantic_graph_uses_semantic_sanitization_and_constructs_once_without_editor_work() {
        let source = "sankey-beta\n<b>A</b>,B,12.5units\nB,<b>A</b>,NaN\n";
        reset_sankey_syntax_construction_count();
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let model = parse_sankey_semantic_model_controlled(
            source,
            &metadata(),
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &crate::OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap();

        assert_eq!(model.graph.nodes.len(), 2);
        assert_eq!(model.graph.nodes[0].id, "A");
        assert_eq!(model.graph.nodes[1].id, "B");
        assert_eq!(model.graph.links.len(), 2);
        assert_eq!(model.graph.links[0].source, "A");
        assert_eq!(model.graph.links[0].target, "B");
        assert_eq!(model.graph.links[0].value, Value::from(12.5));
        assert_eq!(model.graph.links[1].value, Value::Null);
        assert_eq!(sankey_syntax_construction_count(), 1);
        #[cfg(feature = "editor-facts")]
        assert_eq!(
            crate::editor::test_work::read(),
            crate::editor::test_work::Counts::default()
        );
    }

    #[test]
    fn semantic_csv_error_preserves_the_source_span_without_editor_work() {
        let source = "  sankey-beta\r\nA,B,1\r\ninvalid\r\n";
        let end = source.find("invalid").unwrap() + "invalid".len();
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let error = parse_sankey_semantic_model_controlled(
            source,
            &metadata(),
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &crate::OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap_err();
        let Error::DiagramParse { diagnostic, .. } = error else {
            panic!("expected a Sankey parse error");
        };
        assert!(diagnostic.message().contains("expected ','"));
        assert_eq!(diagnostic.span(), Some(SourceSpan::new(end, end)));
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
    fn semantic_entry_can_cancel_while_reading_a_large_quoted_field() {
        let source = format!("sankey-beta\n\"{}\",target,1\n", "a".repeat(12 * 1024));
        let control = crate::OperationControl::new();
        control.cancel_after_checkpoints(4);
        assert!(matches!(
            parse_sankey_semantic_model_controlled(
                &source,
                &metadata(),
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &control
            ),
            Err(crate::OperationCancelled { .. })
        ));
    }
    #[cfg(feature = "compat-json")]
    #[test]
    fn full_typed_and_json_requests_omit_editor_observers() {
        let meta = crate::PresentationMetadata {
            diagram_type: "sankey".to_string(),
            config: crate::MermaidConfig::empty_object(),
            effective_config: crate::presentation::PresentationConfig::empty_object(),
            title: None,
        };
        let source = "sankey-beta\nA,B,1\n";
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        parse_sankey(source, &meta).unwrap();
        parse_sankey_model_for_render_controlled(source, &meta, &crate::OperationControl::new())
            .unwrap()
            .unwrap();
        #[cfg(feature = "editor-facts")]
        assert_eq!(
            crate::editor::test_work::read(),
            crate::editor::test_work::Counts::default(),
        );
    }
}
