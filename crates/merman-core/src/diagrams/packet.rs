#[cfg(feature = "presentation")]
use crate::SourcePresentationMetadata;
use crate::common_db::LangiumCommonDbFields;
use crate::diagrams::langium_common::{
    LangiumCommonFacts, parse_langium_common, parse_langium_string, strip_langium_inline_comment,
};
#[cfg(feature = "editor-facts")]
use crate::diagrams::langium_common::{
    push_langium_common_editor_fact, push_langium_common_recovery,
};
#[cfg(feature = "editor-facts")]
use crate::family::EditorFactsDemand;
#[cfg(feature = "editor-facts")]
use crate::source_span::recovery_fallback_span;
#[cfg(feature = "editor-facts")]
use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
    EditorSemanticSymbol, editor::ensure_editor_recovery_from_error,
};
use crate::{Error, MermaidConfig, Result, SourceSpan, family};
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
    start: Option<i64>,
    end: Option<i64>,
    bits: Option<i64>,
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

    let body = match packet_body_start_controlled(code, control)? {
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
    let mut offset = body.offset;
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

    while offset < code.len() {
        control.checkpoint()?;
        if let Some(parsed) = parse_langium_common(code, offset) {
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
            offset += parsed.consumed;
            continue;
        }

        let line_start = offset;
        let (line, next_offset) = physical_line(code, offset);
        offset = next_offset;
        let visible = strip_inline_comment(line);
        let t = visible.trim();
        if t.is_empty() {
            continue;
        }

        if let Some(block) = parse_packet_block_spanned(line, line_start) {
            #[cfg(feature = "editor-facts")]
            if let Some(facts) = editor_facts
                .as_mut()
                .filter(|_| editor_demand.captures_rich())
            {
                push_packet_block_editor_fact(facts, &block);
            }
            blocks.push(block.semantic);
            continue;
        }

        let diagnostic = if line_start < body.header_line_end {
            "expected packet".to_string()
        } else {
            format!("unexpected packet statement: {t}")
        };
        #[cfg(feature = "editor-facts")]
        let start = line_start + visible.find(t).unwrap_or_default();
        #[cfg(feature = "editor-facts")]
        if let Some(facts) = &mut editor_facts {
            facts.mark_recovered_from_parse_error(
                diagnostic.clone(),
                Some(SourceSpan::new(start, start + t.len())),
            );
        }
        first_error.get_or_insert_with(|| Error::diagram_parse_fallback(diagram_type, diagnostic));
    }

    let packet_result = match validate_packet_bits_per_row(config_i64(config, "packet.bitsPerRow"))
    {
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
        block.label.text.to_string(),
        Some("packet block".to_string()),
        EditorSemanticKind::String,
        block.label.span,
        block.label.span,
    ));
}

fn populate_packet_controlled(
    blocks: Vec<ParsedPacketBlock>,
    bits_per_row: i64,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<Result<Vec<PacketWord>>> {
    let mut packet: Vec<PacketWord> = Vec::new();
    let mut last_bit: i64 = -1;
    let mut word: PacketWord = Vec::new();
    let mut row: i128 = 1;

    for (block_index, mut block) in blocks.into_iter().enumerate() {
        if block_index % 128 == 0 {
            control.checkpoint()?;
        }
        if let (Some(start), Some(end)) = (block.start, block.end)
            && end < start
        {
            return Ok(Err(Error::diagram_parse_exact(
                "packet".to_string(),
                format!("Packet block {start} - {end} is invalid. End must be greater than start."),
                block.numeric_span,
            )));
        }

        let expected_start = match last_bit.checked_add(1) {
            Some(start) => start,
            None => {
                return Ok(Err(Error::diagram_parse_exact(
                    "packet".to_string(),
                    "Packet bit range exceeds the supported integer range.".to_string(),
                    block.numeric_span,
                )));
            }
        };
        let start = block.start.unwrap_or(expected_start);
        let end_for_msg = block.end.unwrap_or(start);
        if start != expected_start {
            return Ok(Err(Error::diagram_parse_exact(
                "packet".to_string(),
                format!(
                    "Packet block {start} - {end_for_msg} is not contiguous. It should start from {}.",
                    expected_start
                ),
                block.numeric_span,
            )));
        }

        if block.bits == Some(0) {
            return Ok(Err(Error::diagram_parse_exact(
                "packet".to_string(),
                format!("Packet block {start} is invalid. Cannot have a zero bit field."),
                block.numeric_span,
            )));
        }

        let end = match block.end {
            Some(end) => end,
            None => {
                let bits = block.bits.unwrap_or(1);
                let Some(width) = bits.checked_sub(1) else {
                    return Ok(Err(Error::diagram_parse_exact(
                        "packet".to_string(),
                        "Packet block width must be positive.".to_string(),
                        block.numeric_span,
                    )));
                };
                let Some(end) = start.checked_add(width) else {
                    return Ok(Err(Error::diagram_parse_exact(
                        "packet".to_string(),
                        "Packet bit range exceeds the supported integer range.".to_string(),
                        block.numeric_span,
                    )));
                };
                end
            }
        };
        let Some(bits) = end
            .checked_sub(start)
            .and_then(|value| value.checked_add(1))
        else {
            return Ok(Err(Error::diagram_parse_exact(
                "packet".to_string(),
                "Packet bit range arithmetic overflowed.".to_string(),
                block.numeric_span,
            )));
        };
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
            let (fitting, next) = match get_next_fitting_block(cur, row, bits_per_row) {
                Ok(next) => next,
                Err(error) => return Ok(Err(error)),
            };
            let Some(row_boundary) = row.checked_mul(i128::from(bits_per_row)) else {
                return Ok(Err(Error::diagram_parse_exact(
                    "packet".to_string(),
                    "Packet row arithmetic exceeds the supported integer range.".to_string(),
                    block.numeric_span,
                )));
            };
            let reached_row_end = i128::from(fitting.end) + 1 == row_boundary;
            word.push(fitting);
            if reached_row_end {
                if !word.is_empty() {
                    if packet.len() >= MAX_PACKET_SIZE {
                        return Ok(Err(Error::diagram_parse_exact(
                            "packet".to_string(),
                            format!("Packet exceeds the {MAX_PACKET_SIZE}-row safety limit."),
                            block.numeric_span,
                        )));
                    }
                    packet.push(std::mem::take(&mut word));
                }
                let Some(next_row) = row.checked_add(1) else {
                    return Ok(Err(Error::diagram_parse_exact(
                        "packet".to_string(),
                        "Packet row arithmetic exceeds the supported integer range.".to_string(),
                        block.numeric_span,
                    )));
                };
                row = next_row;
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
    row: i128,
    bits_per_row: i64,
) -> Result<(PacketBlock, Option<PacketBlock>)> {
    if block.start > block.end {
        return Err(Error::diagram_parse_fallback(
            "packet".to_string(),
            format!(
                "Block start {} is greater than block end {}.",
                block.start, block.end
            ),
        ));
    }

    let row_start = row.checked_mul(i128::from(bits_per_row)).ok_or_else(|| {
        Error::diagram_parse_fallback(
            "packet".to_string(),
            "Packet row arithmetic exceeds the supported integer range.".to_string(),
        )
    })?;

    if i128::from(block.end) < row_start {
        return Ok((block, None));
    }

    let row_end = row_start.checked_sub(1).ok_or_else(|| {
        Error::diagram_parse_fallback(
            "packet".to_string(),
            "Packet row arithmetic underflowed.".to_string(),
        )
    })?;
    let row_end = i64::try_from(row_end).map_err(|_| {
        Error::diagram_parse_fallback(
            "packet".to_string(),
            "Packet row boundary exceeds the supported integer range.".to_string(),
        )
    })?;
    let row_start = i64::try_from(row_start).map_err(|_| {
        Error::diagram_parse_fallback(
            "packet".to_string(),
            "Packet row boundary exceeds the supported integer range.".to_string(),
        )
    })?;
    let first_bits = row_end
        .checked_sub(block.start)
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| {
            Error::diagram_parse_fallback(
                "packet".to_string(),
                "Packet block width arithmetic overflowed.".to_string(),
            )
        })?;
    let second_bits = block
        .end
        .checked_sub(row_start)
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| {
            Error::diagram_parse_fallback(
                "packet".to_string(),
                "Packet block width arithmetic overflowed.".to_string(),
            )
        })?;
    Ok((
        PacketBlock {
            start: block.start,
            end: row_end,
            label: block.label.clone(),
            bits: first_bits,
        },
        Some(PacketBlock {
            start: row_start,
            end: block.end,
            label: block.label,
            bits: second_bits,
        }),
    ))
}

fn strip_inline_comment(line: &str) -> &str {
    strip_langium_inline_comment(line)
}

fn parse_packet_block_spanned(line: &str, line_start: usize) -> Option<PacketBlockSpanned> {
    let stripped = strip_inline_comment(line);
    let trimmed = stripped.trim_start();
    if trimmed.is_empty() {
        return None;
    }
    let leading = stripped.len() - trimmed.len();
    let base = line_start + leading;
    let bytes = trimmed.as_bytes();
    let mut idx = 0usize;

    let (start, end, bits, numeric_span) = if bytes.first() == Some(&b'+') {
        idx = 1;
        while idx < bytes.len() && matches!(bytes[idx], b' ' | b'\t') {
            idx += 1;
        }
        let digits_start = idx;
        while idx < bytes.len() && bytes[idx].is_ascii_digit() {
            idx += 1;
        }
        if idx == digits_start {
            return None;
        }
        let bits = parse_packet_integer(&trimmed[digits_start..idx])?;
        let bits_span = SourceSpan::new(base + digits_start, base + idx);
        (None, None, Some(bits), bits_span)
    } else {
        let digits_start = idx;
        while idx < bytes.len() && bytes[idx].is_ascii_digit() {
            idx += 1;
        }
        if idx == digits_start {
            return None;
        }
        let start = parse_packet_integer(&trimmed[digits_start..idx])?;
        let start_span = SourceSpan::new(base + digits_start, base + idx);
        while idx < bytes.len() && matches!(bytes[idx], b' ' | b'\t') {
            idx += 1;
        }
        if idx < bytes.len() && bytes[idx] == b'-' {
            idx += 1;
            while idx < bytes.len() && matches!(bytes[idx], b' ' | b'\t') {
                idx += 1;
            }
            let end_digits_start = idx;
            while idx < bytes.len() && bytes[idx].is_ascii_digit() {
                idx += 1;
            }
            if idx == end_digits_start {
                return None;
            }
            let end = parse_packet_integer(&trimmed[end_digits_start..idx])?;
            (
                Some(start),
                Some(end),
                None,
                SourceSpan::new(start_span.start, base + idx),
            )
        } else {
            (Some(start), None, None, start_span)
        }
    };

    let mut rest = &trimmed[idx..];
    let rest_trimmed = rest.trim_start();
    let ws1 = rest.len() - rest_trimmed.len();
    rest = rest_trimmed;
    if !rest.starts_with(':') {
        return None;
    }
    let after_colon_base = base + idx + ws1 + 1;
    rest = &rest[1..];
    let rest_trimmed = rest.trim_start();
    let ws2 = rest.len() - rest_trimmed.len();
    rest = rest_trimmed;
    let label_base = after_colon_base + ws2;
    let (_label, decoded_label, tail) = parse_quoted_string_spanned(rest, label_base)?;
    if !tail.trim().is_empty() {
        return None;
    }

    Some(PacketBlockSpanned {
        semantic: ParsedPacketBlock {
            start,
            end,
            bits,
            label: decoded_label,
            numeric_span,
        },
        #[cfg(feature = "editor-facts")]
        label: _label,
    })
}

fn parse_packet_integer(token: &str) -> Option<i64> {
    if token.len() > 1 && token.starts_with('0') {
        return None;
    }
    token.parse().ok()
}

fn parse_quoted_string_spanned(
    input: &str,
    base_offset: usize,
) -> Option<(SpannedText, String, &str)> {
    let parsed = parse_langium_string(input, base_offset)?;
    let tail = &input[parsed.consumed..];
    Some((
        SpannedText {
            #[cfg(feature = "editor-facts")]
            text: parsed.value.clone(),
            #[cfg(feature = "editor-facts")]
            span: parsed.value_span,
        },
        parsed.value,
        tail,
    ))
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

#[derive(Debug, Clone, Copy)]
struct PacketBodyStart {
    offset: usize,
    header_line_end: usize,
}

fn packet_body_start_controlled(
    code: &str,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<Result<Option<PacketBodyStart>>> {
    let mut offset = 0usize;
    while offset < code.len() {
        control.checkpoint()?;
        let (line, next_offset) = physical_line(code, offset);
        let visible = strip_inline_comment(line);
        let trimmed = visible.trim_start();
        if trimmed.trim().is_empty() {
            offset = next_offset;
            continue;
        }

        let leading = visible.len() - trimmed.len();
        let Some(header_len) = packet_header_token_len(trimmed) else {
            return Ok(Err(Error::diagram_parse_fallback(
                "packet".to_string(),
                "expected packet".to_string(),
            )));
        };
        let body_start = offset + leading + header_len;
        return Ok(Ok(Some(PacketBodyStart {
            offset: body_start,
            header_line_end: next_offset,
        })));
    }
    Ok(Ok(None))
}

fn physical_line(source: &str, offset: usize) -> (&str, usize) {
    let rest = &source[offset..];
    if let Some(newline) = rest.find('\n') {
        let line = rest[..newline]
            .strip_suffix('\r')
            .unwrap_or(&rest[..newline]);
        (line, offset + newline + 1)
    } else {
        (rest, source.len())
    }
}

fn config_i64(config: &MermaidConfig, dotted_path: &str) -> Option<i64> {
    let mut cur = config.as_value();
    for segment in dotted_path.split('.') {
        cur = cur.as_object()?.get(segment)?;
    }
    match cur {
        Value::Number(n) => n
            .as_i64()
            .or_else(|| n.as_u64().and_then(|v| i64::try_from(v).ok())),
        _ => None,
    }
}

#[derive(Debug, Clone)]
struct SpannedText {
    #[cfg(feature = "editor-facts")]
    text: String,
    #[cfg(feature = "editor-facts")]
    span: SourceSpan,
}

#[derive(Debug, Clone)]
struct PacketBlockSpanned {
    semantic: ParsedPacketBlock,
    #[cfg(feature = "editor-facts")]
    label: SpannedText,
}

#[cfg(test)]
mod semantic_tests {
    use super::*;

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
        assert_eq!((model.packet[0][0].start, model.packet[0][0].end), (0, 7));
        assert_eq!((model.packet[1][0].start, model.packet[1][0].end), (8, 11));
        assert_eq!((model.packet[1][1].start, model.packet[1][1].end), (12, 12));
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
            "packet.bitsPerRow must be an integer greater than or equal to 1; received 0"
        );
    }

    #[test]
    fn packet_rejects_extreme_ranges_without_integer_overflow() {
        let error = parse_err(
            r#"packet
0-9223372036854775807: "too wide"
"#,
        );

        assert_eq!(error, "Packet bit range arithmetic overflowed.");
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

        assert_eq!(model["packet"][0][0]["label"], "An100%% complete");
        assert_eq!(model["packet"][0][1]["label"], "Btlabel");

        #[cfg(feature = "editor-facts")]
        {
            let facts = MermaidEngine::new()
                .parse_editor_semantic_facts_with_type_sync("packet", source)
                .unwrap()
                .unwrap();
            let escaped = "A\\n100%% complete";
            let escaped_start = source.find(escaped).unwrap();
            assert!(facts.symbols.iter().any(|symbol| {
                symbol.name == "An100%% complete"
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
                  "bits": 21,
                  "end": 31,
                  "label": "multiple",
                  "start": 11
                }
              ],
              [
                {
                  "bits": 32,
                  "end": 63,
                  "label": "multiple",
                  "start": 32
                }
              ],
              [
                {
                  "bits": 27,
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
                  "bits": 15,
                  "end": 31,
                  "label": "multiple",
                  "start": 17
                }
              ],
              [
                {
                  "bits": 32,
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
