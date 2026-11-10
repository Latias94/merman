#[cfg(feature = "presentation")]
use crate::SourcePresentationMetadata;
use crate::common_db::LangiumCommonDbFields;
use crate::diagrams::langium_common::{
    LangiumCommonFacts, convert_langium_escape_character, is_ecmascript_line_terminator,
    parse_langium_common,
};
#[cfg(feature = "editor-facts")]
use crate::diagrams::langium_common::{
    push_langium_common_editor_fact, push_langium_common_recovery,
};
#[cfg(feature = "editor-facts")]
use crate::family::EditorFactsDemand;
#[cfg(feature = "editor-facts")]
use crate::source_span::recovery_fallback_span;
use crate::utils::format_js_number;
#[cfg(feature = "editor-facts")]
use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
    EditorSemanticSymbol, editor::ensure_editor_recovery_from_error,
};
use crate::{Error, MermaidConfig, Result, SourceSpan, family};
#[cfg(feature = "compat-json")]
use serde_json::Value;

const MAX_PACKET_SIZE: usize = 10_000;
#[cfg(feature = "presentation")]
use merman_model::families::packet::PacketOutputData;
use merman_model::families::packet::{
    PacketBlock, PacketDiagramModel, validate_packet_bits_per_row,
};

pub(crate) fn sanitize_common_db_fields(
    model: &mut PacketDiagramModel,
    config: &crate::MermaidConfig,
) {
    crate::common_db::sanitize_optional_title(&mut model.title, config);
    crate::common_db::sanitize_optional_acc_title(&mut model.acc_title, config);
    crate::common_db::sanitize_optional_acc_descr(&mut model.acc_descr, config);
}

#[cfg(feature = "compat-json")]
pub(crate) fn render_model_to_compat_json(
    model: &PacketDiagramModel,
    output_data: PacketOutputData,
    meta: &SourcePresentationMetadata,
) -> Result<Value> {
    Ok(merman_model::families::packet::compat::project(
        model,
        output_data,
        &meta.diagram_type,
        meta.effective_config.as_config(),
    ))
}

#[derive(Debug, Clone)]
struct ParsedPacketBlock {
    start: Option<f64>,
    end: Option<f64>,
    bits: Option<f64>,
    label: String,
    numeric_span: SourceSpan,
}

enum PacketParseOutput {
    Empty,
    Model(PacketDiagramModel),
}

impl PacketParseOutput {
    fn into_semantic_model(self) -> PacketDiagramModel {
        match self {
            Self::Empty => PacketDiagramModel::default(),
            Self::Model(model) => model,
        }
    }

    #[cfg(feature = "presentation")]
    fn into_render_parts(self) -> (PacketDiagramModel, PacketOutputData) {
        match self {
            Self::Empty => (PacketDiagramModel::default(), PacketOutputData::Empty),
            Self::Model(model) => (model, PacketOutputData::Model),
        }
    }
}

struct PacketSemanticSource {
    output: PacketParseOutput,
    #[cfg(feature = "editor-facts")]
    editor_facts: Option<EditorSemanticFacts>,
}

type PacketWord = Vec<PacketBlock>;

#[cfg(feature = "compat-json")]
pub(crate) fn parse_packet(code: &str, meta: &SourcePresentationMetadata) -> Result<Value> {
    packet_output_into_json(parse_packet_semantic_source(code, meta)?.output, meta)
}

#[cfg(feature = "compat-json")]
#[cfg(feature = "editor-facts")]
pub(crate) fn parse_packet_json_and_editor_facts(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<family::CombinedSemanticParse> {
    control.checkpoint()?;
    let parsed = family::CombinedSemanticParse::from_construction(
        construct_packet_semantic_source_controlled(
            code,
            &meta.diagram_type,
            meta.effective_config.as_config(),
            EditorFactsDemand::Capture,
            control,
        )?,
        |source| {
            (
                packet_output_into_json(source.output, meta),
                source.editor_facts.unwrap_or_default(),
            )
        },
        family::CombinedSemanticFailure::into_parts,
    );
    control.checkpoint()?;
    Ok(parsed)
}

#[cfg(feature = "compat-json")]
fn packet_output_into_json(
    output: PacketParseOutput,
    meta: &SourcePresentationMetadata,
) -> Result<Value> {
    let (model, output_data) = output.into_render_parts();
    render_model_to_compat_json(&model, output_data, meta)
}

#[cfg(all(test, feature = "presentation"))]
pub(crate) fn parse_packet_model_for_render(
    code: &str,
    meta: &SourcePresentationMetadata,
) -> Result<(PacketDiagramModel, PacketOutputData)> {
    Ok(parse_packet_semantic_source(code, meta)?
        .output
        .into_render_parts())
}

#[cfg(feature = "presentation")]
pub(crate) fn parse_packet_model_for_render_controlled(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<Result<(PacketDiagramModel, PacketOutputData)>> {
    let construction = construct_packet_semantic_source_controlled(
        code,
        &meta.diagram_type,
        meta.effective_config.as_config(),
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        control,
    )?;
    let source = match construction {
        Ok(source) => source,
        Err(failure) => return Ok(Err(failure.into_error())),
    };
    let model = source.output.into_render_parts();
    Ok(Ok(model))
}

#[cfg(any(feature = "compat-json", all(test, feature = "presentation")))]
fn parse_packet_semantic_source(
    code: &str,
    meta: &SourcePresentationMetadata,
) -> Result<PacketSemanticSource> {
    construct_packet_semantic_source_controlled(
        code,
        &meta.diagram_type,
        meta.effective_config.as_config(),
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        &crate::OperationControl::new(),
    )
    .expect("a private parse control cannot be cancelled")
    .map_err(family::CombinedSemanticFailure::into_error)
}

pub(crate) fn parse_packet_semantic_model_controlled(
    code: &str,
    metadata: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<crate::family::SemanticConstruction<PacketDiagramModel>> {
    let construction = construct_packet_semantic_source_controlled(
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
            let mut model = source.output.into_semantic_model();
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

fn construct_packet_semantic_source_controlled(
    code: &str,
    diagram_type: &str,
    config: &MermaidConfig,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<
    std::result::Result<PacketSemanticSource, family::CombinedSemanticFailure>,
> {
    control.checkpoint()?;
    #[cfg(test)]
    crate::diagrams::langium_common::record_family_syntax_construction("packet");

    let body_offset = match packet_body_start_controlled(code, control)? {
        Ok(Some(body)) => body,
        Ok(None) => {
            return Ok(Ok(PacketSemanticSource {
                output: PacketParseOutput::Empty,
                #[cfg(feature = "editor-facts")]
                editor_facts: editor_demand
                    .needs_recovery()
                    .then(EditorSemanticFacts::new),
            }));
        }
        Err(error) => {
            return Ok(Err(packet_parse_failure(
                error,
                #[cfg(feature = "editor-facts")]
                editor_demand
                    .needs_recovery()
                    .then(EditorSemanticFacts::new),
                #[cfg(feature = "editor-facts")]
                code,
            )));
        }
    };
    let mut cursor = PacketCursor::new(code, body_offset, control);
    let mut on_header_line = true;
    let mut common = LangiumCommonFacts::default();
    let mut blocks: Vec<ParsedPacketBlock> = Vec::new();
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
    let mut first_error = None;

    while cursor.offset < code.len() {
        control.checkpoint()?;
        cursor.begin_statement();
        let line_start = cursor.offset;
        cursor.skip_horizontal_whitespace()?;
        if cursor.remaining().starts_with("%%") {
            cursor.skip_line()?;
            on_header_line = false;
            continue;
        }
        if cursor.consume_eol()? {
            on_header_line = false;
            continue;
        }
        if let Some(mut parsed) = parse_langium_common(code, cursor.offset) {
            parsed.fact.raw_span.start = line_start;
            if let Some(diagnostic) = &parsed.diagnostic {
                #[cfg(feature = "editor-facts")]
                if let Some(facts) = &mut editor_facts {
                    push_langium_common_recovery(facts, diagnostic);
                }
                first_error.get_or_insert_with(|| {
                    Error::diagram_parse_insertion_point(
                        diagram_type,
                        diagnostic.message.clone(),
                        diagnostic.span.start,
                    )
                });
            }
            #[cfg(feature = "editor-facts")]
            if let Some(facts) = editor_facts
                .as_mut()
                .filter(|_| editor_demand.captures_rich())
            {
                push_langium_common_editor_fact(facts, &parsed.fact, "packet");
            }
            common.push(parsed.fact);
            cursor.offset += parsed.consumed;
            on_header_line = false;
            continue;
        }

        if let Some(block) = parse_packet_block_controlled(&mut cursor)? {
            #[cfg(feature = "editor-facts")]
            if let Some(facts) = editor_facts
                .as_mut()
                .filter(|_| editor_demand.captures_rich())
            {
                push_packet_block_editor_fact(facts, &block);
            }
            blocks.push(block.semantic);
            on_header_line = false;
            continue;
        }

        let span = cursor.recover_statement()?;
        let t = &code[span.start..span.end];
        let diagnostic = if on_header_line {
            "expected packet".to_string()
        } else {
            format!("unexpected packet statement: {t}")
        };
        on_header_line = false;
        #[cfg(feature = "editor-facts")]
        if let Some(facts) = &mut editor_facts {
            facts.mark_recovered_from_parse_error(diagnostic.clone(), Some(span));
        }
        first_error.get_or_insert_with(|| Error::diagram_parse_fallback(diagram_type, diagnostic));
    }

    let packet_result = match validate_packet_bits_per_row(config_bits_per_row(config)) {
        Ok(bits_per_row) => populate_packet_controlled(blocks, bits_per_row, control)?,
        Err(error) => Err(Error::diagram_parse_fallback(
            diagram_type,
            error.to_string(),
        )),
    };
    let packet = match packet_result {
        Ok(packet) => packet,
        Err(error) => {
            #[cfg(feature = "editor-facts")]
            if let Some(facts) = editor_facts.take() {
                editor_facts = Some(ensure_editor_recovery_from_error(
                    facts,
                    &error,
                    recovery_fallback_span(code),
                ));
            }
            if first_error.is_none() {
                first_error = Some(error);
            }
            Vec::new()
        }
    };
    let common = LangiumCommonDbFields::from_facts(&common);

    if let Some(error) = first_error {
        return Ok(Err(family::CombinedSemanticFailure::new(
            error,
            #[cfg(feature = "editor-facts")]
            editor_facts,
        )));
    }

    control.checkpoint()?;
    Ok(Ok(PacketSemanticSource {
        output: PacketParseOutput::Model(PacketDiagramModel {
            title: common.title,
            acc_title: common.acc_title,
            acc_descr: common.acc_descr,
            packet,
        }),
        #[cfg(feature = "editor-facts")]
        editor_facts,
    }))
}

fn packet_parse_failure(
    error: Error,
    #[cfg(feature = "editor-facts")] editor_facts: Option<EditorSemanticFacts>,
    #[cfg(feature = "editor-facts")] code: &str,
) -> family::CombinedSemanticFailure {
    #[cfg(feature = "editor-facts")]
    let editor_facts = editor_facts.map(|facts| {
        ensure_editor_recovery_from_error(facts, &error, recovery_fallback_span(code))
    });
    family::CombinedSemanticFailure::new(
        error,
        #[cfg(feature = "editor-facts")]
        editor_facts,
    )
}

#[cfg(feature = "editor-facts")]
fn push_packet_block_editor_fact(facts: &mut EditorSemanticFacts, block: &PacketBlockSpanned) {
    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::Payload,
        block.semantic.numeric_span,
    ));
    facts.push_symbol(EditorSemanticSymbol::payload(
        block.semantic.label.clone(),
        Some("packet block".to_string()),
        EditorSemanticKind::String,
        block.label_span,
        block.label_span,
    ));
}

fn populate_packet_controlled(
    blocks: Vec<ParsedPacketBlock>,
    bits_per_row: f64,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<Result<Vec<PacketWord>>> {
    let mut packet: Vec<PacketWord> = Vec::new();
    let mut last_bit = -1.0;
    let mut word: PacketWord = Vec::new();
    let mut row = 1.0;

    for (block_index, mut block) in blocks.into_iter().enumerate() {
        if block_index % 128 == 0 {
            control.checkpoint()?;
        }
        if let (Some(start), Some(end)) = (block.start, block.end)
            && end < start
        {
            return Ok(Err(Error::diagram_parse_exact(
                "packet".to_string(),
                format!(
                    "Packet block {} - {} is invalid. End must be greater than start.",
                    format_js_number(start),
                    format_js_number(end),
                ),
                block.numeric_span,
            )));
        }

        let expected_start = last_bit + 1.0;
        let start = block.start.unwrap_or(expected_start);
        let end_for_msg = block.end.unwrap_or(start);
        if start != expected_start {
            return Ok(Err(Error::diagram_parse_exact(
                "packet".to_string(),
                format!(
                    "Packet block {} - {} is not contiguous. It should start from {}.",
                    format_js_number(start),
                    format_js_number(end_for_msg),
                    format_js_number(expected_start),
                ),
                block.numeric_span,
            )));
        }

        if block.bits == Some(0.0) {
            return Ok(Err(Error::diagram_parse_exact(
                "packet".to_string(),
                format!(
                    "Packet block {} is invalid. Cannot have a zero bit field.",
                    format_js_number(start),
                ),
                block.numeric_span,
            )));
        }

        // Keep the source's left-associative Number operations and its explicit relative width.
        // `start + bits - 1` differs from `start + (bits - 1)` at binary64 boundaries.
        let end = block
            .end
            .unwrap_or_else(|| start + block.bits.unwrap_or(1.0) - 1.0);
        let bits = block.bits.unwrap_or(end - start + 1.0);
        last_bit = end;

        let mut cur = PacketBlock {
            start,
            end,
            bits,
            label: std::mem::take(&mut block.label),
        };

        let mut split_count = 0usize;
        loop {
            if split_count.is_multiple_of(128) {
                control.checkpoint()?;
            }
            split_count = split_count.saturating_add(1);
            if packet.len() >= MAX_PACKET_SIZE {
                return Ok(Err(packet_expansion_error(
                    format!("Packet exceeds the {MAX_PACKET_SIZE}-row safety limit."),
                    "merman.packet.row_limit",
                    block.numeric_span,
                )));
            }
            let (fitting, next) =
                match get_next_fitting_block(cur, row, bits_per_row, block.numeric_span) {
                    Ok(next) => next,
                    Err(error) => return Ok(Err(error)),
                };
            let reached_row_end = fitting.end + 1.0 == row * bits_per_row;
            word.push(fitting);
            if reached_row_end {
                packet.push(std::mem::take(&mut word));
                // The materialized row budget bounds this counter, independently of bit values.
                row += 1.0;
            }
            let Some(next) = next else {
                break;
            };
            cur = next;
        }
    }

    if !word.is_empty() {
        packet.push(word);
    }

    control.checkpoint()?;
    Ok(Ok(packet))
}

fn get_next_fitting_block(
    block: PacketBlock,
    row: f64,
    bits_per_row: f64,
    numeric_span: SourceSpan,
) -> Result<(PacketBlock, Option<PacketBlock>)> {
    if block.start > block.end {
        return Err(Error::diagram_parse_exact(
            "packet".to_string(),
            format!(
                "Block start {} is greater than block end {}.",
                format_js_number(block.start),
                format_js_number(block.end),
            ),
            numeric_span,
        ));
    }

    let row_start = row * bits_per_row;

    if block.end + 1.0 <= row_start {
        return Ok((block, None));
    }

    let row_end = row_start - 1.0;
    // Without this equality the upstream loop keeps the same row and repeatedly emits the same
    // remainder. Detect that Number-rounding stall before allocating any repeated fragments.
    if row_end + 1.0 != row_start {
        return Err(packet_expansion_error(
            "Packet row splitting cannot make progress with JavaScript Number arithmetic.",
            "merman.packet.nonprogress",
            numeric_span,
        ));
    }
    Ok((
        PacketBlock {
            start: block.start,
            end: row_end,
            label: block.label.clone(),
            bits: row_end - block.start,
        },
        Some(PacketBlock {
            start: row_start,
            end: block.end,
            label: block.label,
            bits: block.end - row_start,
        }),
    ))
}

fn packet_expansion_error(message: impl Into<String>, code: &str, span: SourceSpan) -> Error {
    Error::diagram_parse_diagnostic(
        "packet",
        crate::ParseDiagnostic::new(message)
            .with_code(code)
            .with_span(span, crate::ParseDiagnosticSpanKind::Exact),
    )
}

fn parse_packet_block_controlled(
    cursor: &mut PacketCursor<'_>,
) -> crate::OperationControlResult<Option<PacketBlockSpanned>> {
    let relative = cursor.peek() == Some('+');
    if relative {
        cursor.advance()?;
        cursor.skip_horizontal_whitespace()?;
    }
    let Some((first, first_span)) = cursor.integer()? else {
        return Ok(None);
    };
    cursor.skip_horizontal_whitespace()?;
    let (start, end, bits, numeric_span) = if relative {
        (None, None, Some(first), first_span)
    } else if cursor.peek() == Some('-') {
        cursor.advance()?;
        cursor.skip_horizontal_whitespace()?;
        let Some((end, end_span)) = cursor.integer()? else {
            return Ok(None);
        };
        cursor.skip_horizontal_whitespace()?;
        (
            Some(first),
            Some(end),
            None,
            SourceSpan::new(first_span.start, end_span.end),
        )
    } else {
        (Some(first), None, None, first_span)
    };
    if cursor.peek() != Some(':') {
        return Ok(None);
    }
    cursor.advance()?;
    cursor.skip_horizontal_whitespace()?;
    let Some((label, _label_span)) = cursor.label()? else {
        return Ok(None);
    };
    cursor.skip_horizontal_whitespace()?;
    if cursor.remaining().starts_with("%%") {
        cursor.skip_line()?;
    } else if !cursor.consume_eol()? {
        return Ok(None);
    }
    Ok(Some(PacketBlockSpanned {
        semantic: ParsedPacketBlock {
            start,
            end,
            bits,
            label,
            numeric_span,
        },
        #[cfg(feature = "editor-facts")]
        label_span: _label_span,
    }))
}

fn parse_packet_integer(token: &str) -> Option<f64> {
    if token.len() > 1 && token.starts_with('0') {
        return None;
    }
    token.parse().ok()
}

const PACKET_SCAN_CHECKPOINT_BYTES: usize = 1024;

/// Consumes Packet STRING values in one pass, retaining only the first physical line for recovery.
struct PacketCursor<'a> {
    source: &'a str,
    offset: usize,
    control: &'a crate::OperationControl,
    checkpoint_bytes: usize,
    recovery: PacketRecoveryLine,
    // EOF has no later closing delimiter of this kind. Reuse that failure after recovery rather
    // than scanning the same suffix again; there are only two quote kinds to retain.
    exhausted_quotes: [Option<usize>; 2],
    #[cfg(test)]
    scanned_bytes: usize,
}

struct PacketRecoveryLine {
    span: SourceSpan,
    next_offset: Option<usize>,
    comment: bool,
    quote: Option<char>,
    escaped: bool,
}

impl PacketRecoveryLine {
    fn new(start: usize) -> Self {
        Self {
            span: SourceSpan::new(start, start),
            next_offset: None,
            comment: false,
            quote: None,
            escaped: false,
        }
    }
}

impl<'a> PacketCursor<'a> {
    fn new(source: &'a str, offset: usize, control: &'a crate::OperationControl) -> Self {
        Self {
            source,
            offset,
            control,
            checkpoint_bytes: 0,
            recovery: PacketRecoveryLine::new(offset),
            exhausted_quotes: [None; 2],
            #[cfg(test)]
            scanned_bytes: 0,
        }
    }

    fn remaining(&self) -> &'a str {
        &self.source[self.offset..]
    }

    fn peek(&self) -> Option<char> {
        self.remaining().chars().next()
    }

    fn begin_statement(&mut self) {
        self.recovery = PacketRecoveryLine::new(self.offset);
    }

    fn advance(&mut self) -> crate::OperationControlResult<Option<char>> {
        let Some(ch) = self.peek() else {
            return Ok(None);
        };
        let start = self.offset;
        self.offset += ch.len_utf8();
        if self.recovery.next_offset.is_none() {
            if ch == '\n' {
                self.recovery.next_offset = Some(self.offset);
            } else if !self.recovery.comment {
                if self.recovery.escaped {
                    self.recovery.escaped = false;
                } else if self.recovery.quote.is_some() && ch == '\\' {
                    self.recovery.escaped = true;
                } else if Some(ch) == self.recovery.quote {
                    self.recovery.quote = None;
                } else if self.recovery.quote.is_none() {
                    if matches!(ch, '\'' | '"') {
                        self.recovery.quote = Some(ch);
                    } else if ch == '%' && self.remaining().starts_with('%') {
                        self.recovery.comment = true;
                    }
                }
                if !self.recovery.comment && !ch.is_whitespace() {
                    if self.recovery.span.start == self.recovery.span.end {
                        self.recovery.span.start = start;
                    }
                    self.recovery.span.end = self.offset;
                }
            }
        }
        #[cfg(test)]
        {
            self.scanned_bytes += ch.len_utf8();
        }
        self.checkpoint_bytes += ch.len_utf8();
        if self.checkpoint_bytes >= PACKET_SCAN_CHECKPOINT_BYTES {
            self.control.checkpoint()?;
            self.checkpoint_bytes = 0;
        }
        Ok(Some(ch))
    }

    fn skip_horizontal_whitespace(&mut self) -> crate::OperationControlResult<()> {
        while matches!(self.peek(), Some(' ' | '\t')) {
            self.advance()?;
        }
        Ok(())
    }

    fn integer(&mut self) -> crate::OperationControlResult<Option<(f64, SourceSpan)>> {
        let start = self.offset;
        while self.peek().is_some_and(|ch| ch.is_ascii_digit()) {
            self.advance()?;
        }
        Ok(parse_packet_integer(&self.source[start..self.offset])
            .map(|value| (value, SourceSpan::new(start, self.offset))))
    }

    fn label(&mut self) -> crate::OperationControlResult<Option<(String, SourceSpan)>> {
        let quote = match self.peek() {
            Some(quote @ ('\'' | '"')) => quote,
            _ => return Ok(None),
        };
        let slot = usize::from(quote == '\'');
        let opening = self.offset;
        if self.exhausted_quotes[slot].is_some_and(|start| opening >= start) {
            return Ok(None);
        }
        self.advance()?;
        let start = self.offset;
        let mut value = String::new();
        let mut escaped = false;
        while let Some(ch) = self.advance()? {
            if escaped {
                if is_ecmascript_line_terminator(ch) {
                    return Ok(None);
                }
                value.push(convert_langium_escape_character(ch));
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == quote {
                return Ok(Some((value, SourceSpan::new(start, self.offset - 1))));
            } else {
                value.push(ch);
            }
        }
        self.exhausted_quotes[slot] = Some(opening);
        Ok(None)
    }

    fn consume_eol(&mut self) -> crate::OperationControlResult<bool> {
        if self.remaining().starts_with("\r\n") {
            self.advance()?;
            self.advance()?;
            return Ok(true);
        }
        if self.peek() == Some('\n') {
            self.advance()?;
            return Ok(true);
        }
        Ok(self.offset == self.source.len())
    }

    fn skip_line(&mut self) -> crate::OperationControlResult<()> {
        while let Some(ch) = self.advance()? {
            if ch == '\n' {
                break;
            }
        }
        Ok(())
    }

    fn recover_statement(&mut self) -> crate::OperationControlResult<SourceSpan> {
        while self.recovery.next_offset.is_none() && self.advance()?.is_some() {}
        self.offset = self.recovery.next_offset.unwrap_or(self.source.len());
        Ok(self.recovery.span)
    }
}

fn packet_header_token_len(line: &str) -> Option<usize> {
    if let Some(rest) = line.strip_prefix("packet-beta")
        && keyword_boundary(rest)
    {
        return Some("packet-beta".len());
    }
    if let Some(rest) = line.strip_prefix("packet")
        && keyword_boundary(rest)
    {
        return Some("packet".len());
    }
    None
}

fn keyword_boundary(rest: &str) -> bool {
    rest.is_empty()
        || rest.starts_with("%%")
        || rest.chars().next().is_some_and(char::is_whitespace)
}

fn packet_body_start_controlled(
    code: &str,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<Result<Option<usize>>> {
    let mut cursor = PacketCursor::new(code, 0, control);
    loop {
        control.checkpoint()?;
        while cursor.peek().is_some_and(char::is_whitespace) {
            cursor.advance()?;
        }
        if cursor.remaining().starts_with("%%") {
            cursor.skip_line()?;
            continue;
        }
        if cursor.remaining().is_empty() {
            return Ok(Ok(None));
        }
        let Some(header_len) = packet_header_token_len(cursor.remaining()) else {
            return Ok(Err(Error::diagram_parse_fallback(
                "packet".to_string(),
                "expected packet".to_string(),
            )));
        };
        return Ok(Ok(Some(cursor.offset + header_len)));
    }
}

fn config_bits_per_row(config: &MermaidConfig) -> Option<f64> {
    config.as_value().get("packet")?.get("bitsPerRow")?.as_f64()
}

#[derive(Debug, Clone)]
struct PacketBlockSpanned {
    semantic: ParsedPacketBlock,
    #[cfg(feature = "editor-facts")]
    label_span: SourceSpan,
}

#[cfg(test)]
mod semantic_tests {
    use super::*;

    fn parse_raw_packet(source: &str) -> PacketDiagramModel {
        construct_packet_semantic_source_controlled(
            source,
            "packet",
            &MermaidConfig::empty_object(),
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &crate::OperationControl::new(),
        )
        .unwrap()
        .map_err(family::CombinedSemanticFailure::into_error)
        .unwrap()
        .output
        .into_semantic_model()
    }

    #[test]
    fn packet_multiline_strings_preserve_values_and_following_fields() {
        for newline in ["\n", "\r\n"] {
            for quote in ['\'', '"'] {
                let label = format!("雪\\n{newline}B%% payload\\{quote}");
                let expected = format!("雪\n{newline}B%% payload{quote}");
                let source = format!(
                    "packet-beta{newline}  0 - 7: {quote}{label}{quote} %% comment{newline}+1: 'next'"
                );
                #[cfg(feature = "editor-facts")]
                crate::editor::test_work::reset();
                let model = parse_raw_packet(&source);
                assert_eq!(model.packet.len(), 1);
                assert_eq!(model.packet[0].len(), 2);
                let first = &model.packet[0][0];
                assert_eq!(first.label, expected);
                assert_eq!((first.start, first.end, first.bits), (0.0, 7.0, 8.0));
                let next = &model.packet[0][1];
                assert_eq!(next.label, "next");
                assert_eq!((next.start, next.end, next.bits), (8.0, 8.0, 1.0));
                #[cfg(feature = "editor-facts")]
                assert_eq!(crate::editor::test_work::read(), Default::default());
            }
        }
    }

    #[test]
    fn packet_multiline_string_can_end_at_eof_or_after_inline_header() {
        for header in ["packet\n", "packet-beta\n", "packet "] {
            for quote in ['\'', '"'] {
                let source = format!("{header}0: {quote}A\nB{quote}");
                let model = parse_raw_packet(&source);
                assert_eq!(model.packet[0][0].label, "A\nB");
                assert_eq!(model.packet[0][0].bits, 1.0);
            }
        }
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn packet_multiline_facts_are_captured_once_with_raw_payload_ranges() {
        for newline in ["\n", "\r\n"] {
            for quote in ['\'', '"'] {
                let raw_label = format!("雪\\n{newline}B%% payload");
                let decoded = format!("雪\n{newline}B%% payload");
                let source =
                    format!("packet{newline}\t0 - 7: {quote}{raw_label}{quote}{newline}+1: 'next'");
                crate::diagrams::langium_common::reset_family_syntax_construction_count("packet");
                crate::editor::test_work::reset();
                let parsed = construct_packet_semantic_source_controlled(
                    &source,
                    "packet",
                    &MermaidConfig::empty_object(),
                    EditorFactsDemand::Capture,
                    &crate::OperationControl::new(),
                )
                .unwrap()
                .map_err(family::CombinedSemanticFailure::into_error)
                .unwrap();
                let facts = parsed.editor_facts.unwrap();
                assert_eq!(
                    facts.completeness,
                    crate::EditorSemanticCompleteness::Complete
                );
                assert!(facts.diagnostics.is_empty());
                assert_eq!(facts.symbols.len(), 2);
                let label = &facts.symbols[0];
                assert_eq!(label.name, decoded);
                let start = source.find(&raw_label).unwrap();
                let expected_span = SourceSpan::new(start, start + raw_label.len());
                assert_eq!(label.span, expected_span);
                assert_eq!(label.selection, expected_span);
                assert_eq!(
                    &source[label.selection.start..label.selection.end],
                    raw_label
                );
                let number_start = source.find("0 - 7").unwrap();
                assert!(facts.expected_syntax.iter().any(|expected| {
                    expected.span == SourceSpan::new(number_start, number_start + "0 - 7".len())
                }));
                assert_eq!(crate::editor::test_work::read().collections, 1);
                assert_eq!(crate::editor::test_work::read().symbols, 2);
                assert_eq!(
                    crate::diagrams::langium_common::family_syntax_construction_count("packet"),
                    1
                );
            }
        }
    }

    #[test]
    fn packet_multiline_strings_reject_trailing_garbage_and_missing_delimiters() {
        for source in [
            "packet\n0: \"A\nB\" junk\n",
            "packet\n0: 'A\nB' +1: 'next'\n",
            "packet\n0: \"A\nB",
            "packet\n0: 'A\nB",
            "packet\n0: \"A\\",
            "packet\n0: 'A\\",
        ] {
            let result = construct_packet_semantic_source_controlled(
                source,
                "packet",
                &MermaidConfig::empty_object(),
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &crate::OperationControl::new(),
            )
            .unwrap();
            assert!(
                result.is_err(),
                "invalid Packet input was accepted: {source:?}"
            );
        }
        for quote in ['\'', '"'] {
            for terminator in ["\n", "\r\n", "\u{2028}", "\u{2029}"] {
                let source = format!("packet\n0: {quote}A\\{terminator}B{quote}\n");
                let result = construct_packet_semantic_source_controlled(
                    &source,
                    "packet",
                    &MermaidConfig::empty_object(),
                    #[cfg(feature = "editor-facts")]
                    EditorFactsDemand::Omit,
                    &crate::OperationControl::new(),
                )
                .unwrap();
                assert!(
                    result.is_err(),
                    "escaped physical newline was accepted: {source:?}"
                );
            }
        }
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn packet_multiline_recovery_keeps_following_fields_and_exact_diagnostics() {
        for invalid in [
            "  +1: \"unterminated",
            "  +1: 'unterminated",
            "  +1: \"bad\" garbage %% hidden",
            "  +1: \"escaped\\",
        ] {
            let source = format!("packet\r\n0: 'valid'\r\n{invalid}\r\n+1: \"next\"\r\n");
            crate::diagrams::langium_common::reset_family_syntax_construction_count("packet");
            crate::editor::test_work::reset();
            let result = construct_packet_semantic_source_controlled(
                &source,
                "packet",
                &MermaidConfig::empty_object(),
                EditorFactsDemand::Capture,
                &crate::OperationControl::new(),
            )
            .unwrap();
            let failure = match result {
                Err(failure) => failure,
                Ok(_) => panic!("invalid Packet input was accepted: {source:?}"),
            };
            let (_, facts) = failure.into_parts();
            assert_eq!(
                facts.completeness,
                crate::EditorSemanticCompleteness::Recovered
            );
            assert_eq!(
                facts
                    .symbols
                    .iter()
                    .map(|symbol| symbol.name.as_str())
                    .collect::<Vec<_>>(),
                ["valid", "next"]
            );
            let visible = invalid.split(" %%").next().unwrap().trim();
            let start = source.find(visible).unwrap();
            let span = SourceSpan::new(start, start + visible.len());
            assert!(facts.diagnostics.iter().any(|diagnostic| {
                diagnostic.span == Some(span)
                    && diagnostic.message == format!("unexpected packet statement: {visible}")
            }));
            for symbol in &facts.symbols {
                assert_eq!(
                    &source[symbol.selection.start..symbol.selection.end],
                    symbol.name
                );
            }
            assert_eq!(crate::editor::test_work::read().collections, 1);
            assert_eq!(
                crate::diagrams::langium_common::family_syntax_construction_count("packet"),
                1
            );
        }
    }

    #[test]
    fn packet_long_tokens_and_recovery_can_cancel_with_bounded_scan_work() {
        let payload = "雪".repeat(8 * PACKET_SCAN_CHECKPOINT_BYTES);
        for source in [
            format!("0: \"{payload}\nB\""),
            format!("0: '{payload}"),
            format!("+{}: 'bit'", "9".repeat(payload.len())),
            format!("0: 'bit' %% {payload}"),
            format!("invalid {payload}"),
            format!("0: 'bit' garbage {payload}"),
        ] {
            let control = crate::OperationControl::new();
            control.cancel_after_checkpoints(1);
            let mut cursor = PacketCursor::new(&source, 0, &control);
            let cancelled = match parse_packet_block_controlled(&mut cursor) {
                Err(_) => true,
                Ok(None) => cursor.recover_statement().is_err(),
                Ok(Some(_)) => false,
            };
            assert!(cancelled, "long Packet scan did not cancel");
            assert!(cursor.scanned_bytes <= 2 * (PACKET_SCAN_CHECKPOINT_BYTES + 3));
            assert!(cursor.scanned_bytes < source.len());
        }
        let source = format!("packet 0: \"{payload}\nB\"");
        let control = crate::OperationControl::new();
        control.cancel_after_checkpoints(8);
        assert!(matches!(
            construct_packet_semantic_source_controlled(
                &source,
                "packet",
                &MermaidConfig::empty_object(),
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &control,
            ),
            Err(crate::OperationCancelled { .. })
        ));
    }

    #[test]
    fn packet_recovery_scans_quoted_suffixes_only_linearly() {
        let mut source = String::from("+1: 'unterminated\n");
        for _ in 0..1024 {
            source.push_str("+1: \"unterminated\n");
            source.push_str("+1: \"closed\" garbage\n");
        }
        source.push_str("+1: \"survivor\"\n");
        let control = crate::OperationControl::new();
        let mut cursor = PacketCursor::new(&source, 0, &control);
        let mut labels = Vec::new();
        while cursor.offset < source.len() {
            cursor.begin_statement();
            if let Some(block) = parse_packet_block_controlled(&mut cursor).unwrap() {
                labels.push(block.semantic.label);
            } else {
                cursor.recover_statement().unwrap();
            }
        }
        assert_eq!(labels, ["survivor"]);
        assert!(cursor.scanned_bytes <= 3 * source.len());
    }

    #[test]
    fn packet_semantics_preserve_split_ranges_without_editor_work() {
        crate::diagrams::langium_common::reset_family_syntax_construction_count("packet");
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let metadata = crate::SemanticMetadata {
            diagram_type: "packet".to_string(),
            config: MermaidConfig::empty_object(),
            semantic_config: MermaidConfig::from_value(serde_json::json!({
                "packet": {"bitsPerRow": 8}
            })),
            title: None,
        };
        let source = "packet\ntitle Header\n+12: \"wide\"\n+1: \"flag\"\n";
        let model = parse_packet_semantic_model_controlled(
            source,
            &metadata,
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &crate::OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap();
        assert_eq!(model.title.as_deref(), Some("Header"));
        assert_eq!(model.packet.len(), 2);
        assert_eq!(
            (model.packet[0][0].start, model.packet[0][0].end),
            (0.0, 7.0)
        );
        assert_eq!(
            (model.packet[1][0].start, model.packet[1][0].end),
            (8.0, 11.0)
        );
        assert_eq!(
            (model.packet[1][1].start, model.packet[1][1].end),
            (12.0, 12.0)
        );
        assert_eq!(model.packet[1][1].label, "flag");
        assert_eq!(
            crate::diagrams::langium_common::family_syntax_construction_count("packet"),
            1
        );
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
        let parsed = crate::SemanticParser::new()
            .parse_diagram_sync(source)
            .unwrap();
        assert!(matches!(
            parsed.model,
            crate::SemanticDiagramModel::Packet(_)
        ));

        #[cfg(feature = "presentation")]
        {
            let full = crate::MermaidEngine::new()
                .with_site_config(crate::presentation::SourcePresentationConfig::from_config(
                    metadata.semantic_config.clone(),
                ))
                .parse_diagram_for_render_model_sync(source, crate::ParseOptions::strict())
                .unwrap()
                .unwrap();
            let crate::RenderSemanticModel::Packet(full) = full.model() else {
                panic!("expected Packet");
            };
            assert_eq!(
                serde_json::to_value(&model).unwrap(),
                serde_json::to_value(full).unwrap()
            );
        }
    }

    #[test]
    fn packet_semantic_admission_keeps_numeric_errors_and_cancellation() {
        let parser = crate::SemanticParser::new();
        for source in ["packet\n+0: \"zero\"", "packet\n0-7: \"byte\"\n10: \"gap\""] {
            assert!(matches!(
                parser.parse_diagram_sync(source),
                Err(Error::DiagramParse { .. })
            ));
        }
        let parser = parser.with_site_config(MermaidConfig::from_value(serde_json::json!({
            "packet": {"bitsPerRow": 0}
        })));
        assert!(parser.parse_diagram_sync("packet\n+1: \"bit\"").is_err());
        let control = crate::OperationControl::new();
        control.cancel_after_checkpoints(4);
        assert!(
            parser
                .parse_diagram_controlled_sync("packet\n+1: \"bit\"", &control)
                .is_err()
        );
    }

    #[test]
    fn packet_row_expansion_checks_cancellation_inside_a_single_large_field() {
        let control = crate::OperationControl::new();
        control.cancel_after_checkpoints(3);
        let blocks = vec![ParsedPacketBlock {
            start: Some(0.0),
            end: Some(319_999.0),
            bits: None,
            label: "bounded expansion".to_string(),
            numeric_span: SourceSpan::new(0, 8),
        }];
        assert!(populate_packet_controlled(blocks, 32.0, &control).is_err());
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

    fn parse_err(text: &str) -> String {
        let engine = MermaidEngine::new();
        match block_on(engine.parse_diagram(text, ParseOptions::default())).unwrap_err() {
            Error::DiagramParse { diagnostic, .. } => diagnostic.message().to_string(),
            other => other.to_string(),
        }
    }

    #[test]
    fn packet_parser_can_cancel_between_blocks() {
        let mut text = String::from("packet\n");
        for index in 0..512 {
            text.push_str(&format!("{index}: \"bit-{index}\"\n"));
        }
        let control = crate::OperationControl::new();
        control.cancel_after_checkpoints(4);
        let meta = SourcePresentationMetadata {
            diagram_type: "packet".to_string(),
            config: MermaidConfig::empty_object(),
            effective_config: crate::presentation::SourcePresentationConfig::empty_object(),
            title: None,
        };

        assert!(matches!(
            construct_packet_semantic_source_controlled(
                &text,
                &meta.diagram_type,
                meta.effective_config.as_config(),
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Capture,
                &control,
            ),
            Err(crate::OperationCancelled { .. })
        ));
    }

    #[test]
    fn packet_typed_projection_matches_complete_compatibility_json() {
        let text = "packet\ntitle Header\n0-7: \"byte\"\n";
        let effective_config = crate::presentation::SourcePresentationConfig::from_value(json!({
            "packet": { "bitsPerRow": 16 },
            "theme": "forest"
        }));
        let meta = SourcePresentationMetadata {
            diagram_type: "packet".to_string(),
            config: MermaidConfig::empty_object(),
            effective_config,
            title: None,
        };
        let compat = parse_packet(text, &meta).unwrap();
        let (typed, _output_data) = parse_packet_model_for_render(text, &meta).unwrap();

        assert_eq!(
            render_model_to_compat_json(&typed, _output_data, &meta).unwrap(),
            compat
        );
        assert_eq!(compat["config"]["packet"]["bitsPerRow"], 16);
        assert!(compat["accTitle"].is_null());
    }

    #[test]
    fn packet_bit_order_preserves_typed_model_and_semantic_ranges() {
        let source = "packet\n+12: \"wide\"\n+1: \"flag\"\n";
        let mut packets = Vec::new();
        for bit_order in ["ascending", "descending"] {
            let meta = SourcePresentationMetadata {
                diagram_type: "packet".to_string(),
                config: MermaidConfig::empty_object(),
                effective_config: crate::presentation::SourcePresentationConfig::from_value(
                    json!({
                        "packet": { "bitsPerRow": 8, "bitOrder": bit_order }
                    }),
                ),
                title: None,
            };
            let (typed, _output_data) = parse_packet_model_for_render(source, &meta).unwrap();
            let compat = render_model_to_compat_json(&typed, _output_data, &meta).unwrap();
            assert_eq!(compat, parse_packet(source, &meta).unwrap());
            assert_eq!(compat["config"]["packet"]["bitOrder"], bit_order);
            packets.push(compat["packet"].clone());
            assert!(parse_packet("packet\n7-0: \"reversed\"", &meta).is_err());
        }
        assert_eq!(packets[0], packets[1]);
        assert_eq!(packets[1][1][0]["start"], 8);
        assert_eq!(packets[1][1][0]["end"], 11);
        assert_eq!(packets[1][1][1]["start"], 12);
    }

    #[test]
    fn packet_typed_projection_preserves_empty_and_header_only_output_states() {
        let meta = SourcePresentationMetadata {
            diagram_type: "packet".to_string(),
            config: MermaidConfig::empty_object(),
            effective_config: crate::presentation::SourcePresentationConfig::empty_object(),
            title: None,
        };
        for source in ["", "packet"] {
            let compat = parse_packet(source, &meta).unwrap();
            let (typed, _output_data) = parse_packet_model_for_render(source, &meta).unwrap();

            assert_eq!(
                render_model_to_compat_json(&typed, _output_data, &meta).unwrap(),
                compat,
                "projection drift for {source:?}"
            );
        }
    }

    #[test]
    fn packet_beta_header_is_accepted() {
        let model = parse("packet-beta");
        assert_eq!(model["packet"], json!([]));
    }

    #[test]
    fn packet_header_is_accepted() {
        let model = parse("packet");
        assert_eq!(model["packet"], json!([]));
    }

    #[test]
    fn packet_rejects_a_nonpositive_bits_per_row_before_expanding_rows() {
        let error = parse_err(
            r#"---
config:
  packet:
    bitsPerRow: 0
---
packet
0-7: "byte"
"#,
        );

        assert_eq!(
            error,
            "packet.bitsPerRow must be a finite number greater than or equal to 1; received 0"
        );
    }

    #[test]
    fn packet_limits_extreme_range_expansion_independently_of_number_acceptance() {
        let error = parse_err(
            r#"packet
0-9223372036854775807: "too wide"
"#,
        );

        assert_eq!(error, "Packet exceeds the 10000-row safety limit.");
    }

    #[test]
    fn packet_header_does_not_accept_trailing_text() {
        assert_eq!(parse_err("packet diagrams"), "expected packet");
    }

    #[test]
    fn packet_data_and_title_are_parsed() {
        let model = parse(
            r#"packet
title Packet diagram
accTitle: Packet accTitle
accDescr: Packet accDescription
0-10: "test"
"#,
        );
        assert_eq!(model["title"], json!("Packet diagram"));
        assert_eq!(model["accTitle"], json!("Packet accTitle"));
        assert_eq!(model["accDescr"], json!("Packet accDescription"));
        assert_eq!(
            model["packet"],
            json!([
              [
                {
                  "bits": 11,
                  "end": 10,
                  "label": "test",
                  "start": 0
                }
              ]
            ])
        );
    }

    #[test]
    fn packet_multiline_accessibility_description_matches_common_langium() {
        let model = parse(
            "packet\r\naccDescr {\r\n  First   line\r\n\r\n\tSecond line  \r\n}\r\n0-7: \"byte\"\r\n",
        );

        assert_eq!(model["accDescr"], json!("First line\nSecond line"));
        assert_eq!(model["packet"][0][0]["label"], json!("byte"));
    }

    #[test]
    fn packet_single_bits_are_supported() {
        let model = parse(
            r#"packet
0-10: "test"
11: "single"
"#,
        );
        assert_eq!(
            model["packet"],
            json!([
              [
                {
                  "bits": 11,
                  "end": 10,
                  "label": "test",
                  "start": 0
                },
                {
                  "bits": 1,
                  "end": 11,
                  "label": "single",
                  "start": 11
                }
              ]
            ])
        );
    }

    #[test]
    fn packet_uses_langium_string_escapes_and_quote_aware_inline_comments() {
        let source = r#"packet-beta
0-7: "A\n100%% complete" %% outside comment
8-15: 'B\tlabel'
"#;
        let model = parse(source);

        assert_eq!(model["packet"][0][0]["label"], "A\n100%% complete");
        assert_eq!(model["packet"][0][1]["label"], "B\tlabel");

        #[cfg(feature = "editor-facts")]
        {
            let facts = MermaidEngine::new()
                .parse_editor_semantic_facts_with_type_sync("packet", source)
                .unwrap()
                .unwrap();
            let escaped = "A\\n100%% complete";
            let escaped_start = source.find(escaped).unwrap();
            assert!(facts.symbols.iter().any(|symbol| {
                symbol.name == "A\n100%% complete"
                    && symbol.selection
                        == SourceSpan::new(escaped_start, escaped_start + escaped.len())
            }));
        }
    }

    #[test]
    fn packet_bit_counts_are_supported() {
        let model = parse(
            r#"packet
+8: "byte"
+16: "word"
"#,
        );
        assert_eq!(
            model["packet"],
            json!([
              [
                {
                  "bits": 8,
                  "end": 7,
                  "label": "byte",
                  "start": 0
                },
                {
                  "bits": 16,
                  "end": 23,
                  "label": "word",
                  "start": 8
                }
              ]
            ])
        );
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn packet_editor_facts_expose_parser_backed_spans() {
        let engine = crate::MermaidEngine::new();
        let text = r#"packet
title Packet diagram
accTitle: Packet accTitle
accDescr: Packet accDescription
0-10: "test"
11: "single"
"#;
        let facts = engine
            .parse_editor_semantic_facts_with_type_sync("packet", text)
            .unwrap()
            .unwrap();

        assert!(facts.directive_prefixes.iter().any(|p| p == "title"));
        assert!(facts.directive_prefixes.iter().any(|p| p == "accTitle"));
        assert!(facts.directive_prefixes.iter().any(|p| p == "accDescr"));
        assert!(
            facts
                .symbols
                .iter()
                .any(|symbol| symbol.name == "test" && symbol.kind == EditorSemanticKind::String)
        );
        assert!(
            facts
                .symbols
                .iter()
                .any(|symbol| symbol.name == "single" && symbol.kind == EditorSemanticKind::String)
        );

        let start = text.find("0-10").unwrap();
        let single_start = text.find("11").unwrap();
        assert!(facts.expected_syntax.iter().any(|expected| {
            expected.kind == EditorExpectedSyntaxKind::Payload
                && expected.span == SourceSpan::new(start, start + "0-10".len())
        }));
        assert!(facts.expected_syntax.iter().any(|expected| {
            expected.kind == EditorExpectedSyntaxKind::Payload
                && expected.span == SourceSpan::new(single_start, single_start + "11".len())
        }));
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn packet_editor_recovery_reports_invalid_statement_with_exact_span() {
        let engine = crate::MermaidEngine::new();
        let text = concat!(
            "packet\n",
            "0-7: \"valid\"\n",
            "  invalid packet statement  %% hidden\n",
            "8-15: \"next\"\n",
        );
        let facts = engine
            .parse_editor_semantic_facts_with_type_sync("packet", text)
            .unwrap()
            .unwrap();

        assert_eq!(
            facts.completeness,
            crate::EditorSemanticCompleteness::Recovered
        );
        assert!(facts.symbols.iter().any(|symbol| symbol.name == "valid"));
        assert!(facts.symbols.iter().any(|symbol| symbol.name == "next"));

        let invalid = "invalid packet statement";
        let start = text.find(invalid).unwrap();
        assert!(facts.diagnostics.iter().any(|diagnostic| {
            diagnostic.message == format!("unexpected packet statement: {invalid}")
                && diagnostic.span == Some(SourceSpan::new(start, start + invalid.len()))
                && diagnostic.kind == crate::EditorSemanticDiagnosticKind::ParserRecovery
        }));
    }

    #[test]
    fn packet_editor_recovery_reports_post_parse_validation_errors() {
        let text = concat!(
            "packet\r\n",
            "0-7: \"valid\"\r\n",
            "  9-15: \"not contiguous\"  \r\n",
        );
        let invalid = "9-15";
        let start = text.find(invalid).unwrap();
        let expected_span = SourceSpan::new(start, start + invalid.len());
        let engine = MermaidEngine::new();

        let error = engine
            .parse_diagram_sync(text, ParseOptions::strict())
            .expect_err("a non-contiguous packet block must fail strict parsing");
        let Error::DiagramParse { diagnostic, .. } = error else {
            panic!("expected packet parse diagnostic");
        };
        assert_eq!(diagnostic.span(), Some(expected_span));

        #[cfg(feature = "editor-facts")]
        {
            let facts = engine
                .parse_editor_semantic_facts_with_type_sync("packet", text)
                .unwrap()
                .expect("packet editor recovery facts");
            assert_eq!(
                facts.completeness,
                crate::EditorSemanticCompleteness::Recovered
            );
            assert!(facts.symbols.iter().any(|symbol| symbol.name == "valid"));
            assert!(
                facts
                    .symbols
                    .iter()
                    .any(|symbol| symbol.name == "not contiguous")
            );
            assert!(facts.diagnostics.iter().any(|diagnostic| {
                diagnostic.kind == crate::EditorSemanticDiagnosticKind::ParserRecovery
                    && diagnostic.span == Some(expected_span)
                    && diagnostic.message.contains("is not contiguous")
            }));
        }
    }

    #[test]
    fn packet_splits_into_multiple_rows() {
        let model = parse(
            r#"packet
0-10: "test"
11-90: "multiple"
"#,
        );
        assert_eq!(
            model["packet"],
            json!([
              [
                {
                  "bits": 11,
                  "end": 10,
                  "label": "test",
                  "start": 0
                },
                {
                  "bits": 20,
                  "end": 31,
                  "label": "multiple",
                  "start": 11
                }
              ],
              [
                {
                  "bits": 31,
                  "end": 63,
                  "label": "multiple",
                  "start": 32
                }
              ],
              [
                {
                  "bits": 26,
                  "end": 90,
                  "label": "multiple",
                  "start": 64
                }
              ]
            ])
        );
    }

    #[test]
    fn packet_splits_into_multiple_rows_at_exact_length() {
        let model = parse(
            r#"packet
0-16: "test"
17-63: "multiple"
"#,
        );
        assert_eq!(
            model["packet"],
            json!([
              [
                {
                  "bits": 17,
                  "end": 16,
                  "label": "test",
                  "start": 0
                },
                {
                  "bits": 14,
                  "end": 31,
                  "label": "multiple",
                  "start": 17
                }
              ],
              [
                {
                  "bits": 31,
                  "end": 63,
                  "label": "multiple",
                  "start": 32
                }
              ]
            ])
        );
    }

    #[test]
    fn packet_errors_if_numbers_are_not_continuous() {
        let err = parse_err(
            r#"packet
0-16: "test"
18-20: "error"
"#,
        );
        assert_eq!(
            err,
            "Packet block 18 - 20 is not contiguous. It should start from 17."
        );
    }

    #[test]
    fn packet_errors_if_numbers_are_not_continuous_with_bit_counts() {
        let err = parse_err(
            r#"packet
+16: "test"
18-20: "error"
"#,
        );
        assert_eq!(
            err,
            "Packet block 18 - 20 is not contiguous. It should start from 16."
        );
    }

    #[test]
    fn packet_errors_if_single_number_is_not_continuous() {
        let err = parse_err(
            r#"packet
0-16: "test"
18: "error"
"#,
        );
        assert_eq!(
            err,
            "Packet block 18 - 18 is not contiguous. It should start from 17."
        );
    }

    #[test]
    fn packet_errors_if_single_number_is_not_continuous_with_bit_counts() {
        let err = parse_err(
            r#"packet
+16: "test"
18: "error"
"#,
        );
        assert_eq!(
            err,
            "Packet block 18 - 18 is not contiguous. It should start from 16."
        );
    }

    #[test]
    fn packet_errors_if_single_number_is_not_continuous_2() {
        let err = parse_err(
            r#"packet
0-16: "test"
17: "good"
19: "error"
"#,
        );
        assert_eq!(
            err,
            "Packet block 19 - 19 is not contiguous. It should start from 18."
        );
    }

    #[test]
    fn packet_errors_if_end_is_less_than_start() {
        let err = parse_err(
            r#"packet
0-16: "test"
25-20: "error"
"#,
        );
        assert_eq!(
            err,
            "Packet block 25 - 20 is invalid. End must be greater than start."
        );
    }

    #[test]
    fn packet_errors_if_bit_count_is_zero() {
        let err = parse_err(
            r#"packet
+0: "test"
"#,
        );
        assert_eq!(
            err,
            "Packet block 0 is invalid. Cannot have a zero bit field."
        );
    }
}

#[cfg(all(test, feature = "presentation"))]
mod output_data_tests {
    use super::*;

    #[test]
    fn full_producer_preserves_empty_output_independently_of_json_projection() {
        let meta = crate::SourcePresentationMetadata {
            diagram_type: "packet".to_string(),
            config: crate::MermaidConfig::empty_object(),
            effective_config: crate::presentation::SourcePresentationConfig::empty_object(),
            title: None,
        };
        let parse = |source| {
            parse_packet_model_for_render_controlled(source, &meta, &crate::OperationControl::new())
                .unwrap()
                .unwrap()
        };
        let (empty, empty_output) = parse("");
        let (header, header_output) = parse("packet");
        assert_eq!(empty_output, PacketOutputData::Empty);
        assert_eq!(header_output, PacketOutputData::Model);
        assert_eq!(PacketOutputData::default(), PacketOutputData::Model);
        // Identical canonical values must not erase the source producer's distinct output shape.
        assert_eq!(
            serde_json::to_value(&empty).unwrap(),
            serde_json::to_value(&header).unwrap()
        );
        #[cfg(feature = "compat-json")]
        {
            assert_eq!(
                render_model_to_compat_json(&empty, empty_output, &meta).unwrap(),
                serde_json::json!({})
            );
            assert_eq!(
                render_model_to_compat_json(
                    &PacketDiagramModel::default(),
                    PacketOutputData::default(),
                    &meta
                )
                .unwrap()["type"],
                "packet"
            );
        }
    }
}
