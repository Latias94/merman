use crate::common_db::LangiumCommonDbFields;
use crate::diagrams::langium_common::{
    LangiumCommonFacts, parse_langium_common, push_langium_common_editor_fact,
    push_langium_common_recovery,
};
use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
    EditorSemanticSymbol, Error, MermaidConfig, ParseMetadata, Result, SourceSpan,
};
use serde_json::{Map, Value, json};

const MAX_PACKET_SIZE: usize = 10_000;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct PacketDiagramRenderModel {
    pub title: Option<String>,
    #[serde(rename = "accTitle")]
    pub acc_title: Option<String>,
    #[serde(rename = "accDescr")]
    pub acc_descr: Option<String>,
    pub packet: Vec<Vec<PacketRenderBlock>>,
}

impl PacketDiagramRenderModel {
    pub(crate) fn sanitize_common_db_fields(&mut self, config: &crate::MermaidConfig) {
        crate::common_db::sanitize_optional_title(&mut self.title, config);
        crate::common_db::sanitize_optional_acc_title(&mut self.acc_title, config);
        crate::common_db::sanitize_optional_acc_descr(&mut self.acc_descr, config);
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PacketRenderBlock {
    pub start: i64,
    pub end: i64,
    pub bits: i64,
    pub label: String,
}

#[derive(Debug, Clone)]
struct PacketBlock {
    start: Option<i64>,
    end: Option<i64>,
    bits: Option<i64>,
    label: String,
}

enum PacketParseOutput {
    Empty,
    Model(PacketDiagramRenderModel),
}

type PacketWord = Vec<PacketRenderBlock>;

pub fn parse_packet(code: &str, meta: &ParseMetadata) -> Result<Value> {
    match parse_packet_model(code, meta)? {
        PacketParseOutput::Empty => Ok(json!({})),
        PacketParseOutput::Model(model) => {
            let mut out = Map::with_capacity(6);
            out.insert("type".to_string(), Value::String(meta.diagram_type.clone()));
            out.insert("title".to_string(), json!(model.title));
            out.insert("accTitle".to_string(), json!(model.acc_title));
            out.insert("accDescr".to_string(), json!(model.acc_descr));
            out.insert("packet".to_string(), json!(model.packet));
            out.insert(
                "config".to_string(),
                crate::config::clone_value_nonrecursive(meta.effective_config.as_value()),
            );
            Ok(Value::Object(out))
        }
    }
}

pub fn parse_packet_model_for_render(
    code: &str,
    meta: &ParseMetadata,
) -> Result<PacketDiagramRenderModel> {
    match parse_packet_model(code, meta)? {
        PacketParseOutput::Empty => Ok(PacketDiagramRenderModel::default()),
        PacketParseOutput::Model(model) => Ok(model),
    }
}

pub fn parse_packet_editor_facts(code: &str, _meta: &ParseMetadata) -> EditorSemanticFacts {
    let mut facts = EditorSemanticFacts::new();
    let Ok(Some(mut offset)) = packet_body_start(code) else {
        return facts;
    };

    while offset < code.len() {
        if let Some(parsed) = parse_langium_common(code, offset) {
            push_langium_common_editor_fact(&mut facts, &parsed.fact, "packet");
            if let Some(diagnostic) = &parsed.diagnostic {
                push_langium_common_recovery(&mut facts, diagnostic);
            }
            offset += parsed.consumed;
            continue;
        }

        let (line, next_offset) = physical_line(code, offset);
        let line_start = offset;
        offset = next_offset;
        if let Some(block) = parse_packet_block_spanned(line, line_start) {
            facts.push_expected_syntax(EditorExpectedSyntax::new(
                EditorExpectedSyntaxKind::Payload,
                block.numeric_span,
            ));
            facts.push_symbol(EditorSemanticSymbol::payload(
                block.label.text.to_string(),
                Some("packet block".to_string()),
                EditorSemanticKind::String,
                SourceSpan::new(block.label.start, block.label.end),
                SourceSpan::new(block.label.start, block.label.end),
            ));
            continue;
        }
    }

    facts
}

fn parse_packet_model(code: &str, meta: &ParseMetadata) -> Result<PacketParseOutput> {
    let Some(mut offset) = packet_body_start(code)? else {
        return Ok(PacketParseOutput::Empty);
    };
    let mut common = LangiumCommonFacts::default();
    let mut blocks: Vec<PacketBlock> = Vec::new();

    while offset < code.len() {
        if let Some(parsed) = parse_langium_common(code, offset) {
            if let Some(diagnostic) = parsed.diagnostic {
                return Err(Error::diagram_parse_insertion_point(
                    meta.diagram_type.clone(),
                    diagnostic.message,
                    diagnostic.span.start,
                ));
            }
            common.push(parsed.fact);
            offset += parsed.consumed;
            continue;
        }

        let (line, next_offset) = physical_line(code, offset);
        offset = next_offset;
        let t = strip_inline_comment(line).trim();
        if t.is_empty() {
            continue;
        }

        if let Some(block) = parse_packet_block(t) {
            blocks.push(block);
            continue;
        }

        return Err(Error::diagram_parse_fallback(
            meta.diagram_type.clone(),
            format!("unexpected packet statement: {t}"),
        ));
    }

    let bits_per_row = config_i64(&meta.effective_config, "packet.bitsPerRow").unwrap_or(32);
    let packet = populate_packet(blocks, bits_per_row)?;
    let common = LangiumCommonDbFields::from_facts(&common);

    Ok(PacketParseOutput::Model(PacketDiagramRenderModel {
        title: common.title,
        acc_title: common.acc_title,
        acc_descr: common.acc_descr,
        packet,
    }))
}

fn populate_packet(blocks: Vec<PacketBlock>, bits_per_row: i64) -> Result<Vec<PacketWord>> {
    let mut packet: Vec<PacketWord> = Vec::new();
    let mut last_bit: i64 = -1;
    let mut word: PacketWord = Vec::new();
    let mut row: i64 = 1;

    for mut block in blocks {
        if let (Some(start), Some(end)) = (block.start, block.end)
            && end < start
        {
            return Err(Error::diagram_parse_fallback(
                "packet".to_string(),
                format!("Packet block {start} - {end} is invalid. End must be greater than start."),
            ));
        }

        let start = block.start.unwrap_or(last_bit + 1);
        let end_for_msg = block.end.unwrap_or(start);
        if start != last_bit + 1 {
            return Err(Error::diagram_parse_fallback(
                "packet".to_string(),
                format!(
                    "Packet block {start} - {end_for_msg} is not contiguous. It should start from {}.",
                    last_bit + 1
                ),
            ));
        }

        if block.bits == Some(0) {
            return Err(Error::diagram_parse_fallback(
                "packet".to_string(),
                format!("Packet block {start} is invalid. Cannot have a zero bit field."),
            ));
        }

        let end = block.end.unwrap_or(start + block.bits.unwrap_or(1) - 1);
        let bits = block.bits.unwrap_or(end - start + 1);
        last_bit = end;

        let mut cur = PacketRenderBlock {
            start,
            end,
            bits,
            label: std::mem::take(&mut block.label),
        };

        while word.len() <= (bits_per_row + 1) as usize && packet.len() < MAX_PACKET_SIZE {
            let (fitting, next) = get_next_fitting_block(cur, row, bits_per_row)?;
            let reached_row_end = fitting.end + 1 == row * bits_per_row;
            word.push(fitting);
            if reached_row_end {
                if !word.is_empty() {
                    packet.push(std::mem::take(&mut word));
                }
                row += 1;
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

    Ok(packet)
}

fn get_next_fitting_block(
    block: PacketRenderBlock,
    row: i64,
    bits_per_row: i64,
) -> Result<(PacketRenderBlock, Option<PacketRenderBlock>)> {
    if block.start > block.end {
        return Err(Error::diagram_parse_fallback(
            "packet".to_string(),
            format!(
                "Block start {} is greater than block end {}.",
                block.start, block.end
            ),
        ));
    }

    if block.end < row * bits_per_row {
        return Ok((block, None));
    }

    let row_end = row * bits_per_row - 1;
    let row_start = row * bits_per_row;
    Ok((
        PacketRenderBlock {
            start: block.start,
            end: row_end,
            label: block.label.clone(),
            bits: row_end - block.start,
        },
        Some(PacketRenderBlock {
            start: row_start,
            end: block.end,
            label: block.label,
            bits: block.end - row_start,
        }),
    ))
}

fn strip_inline_comment(line: &str) -> &str {
    match line.find("%%") {
        Some(idx) => &line[..idx],
        None => line,
    }
}

fn parse_packet_block_spanned<'a>(
    line: &'a str,
    line_start: usize,
) -> Option<PacketBlockSpanned<'a>> {
    let stripped = strip_inline_comment(line);
    let trimmed = stripped.trim_start();
    if trimmed.is_empty() {
        return None;
    }
    let leading = stripped.len() - trimmed.len();
    let base = line_start + leading;
    let bytes = trimmed.as_bytes();
    let mut idx = 0usize;

    let numeric_span = if bytes.first() == Some(&b'+') {
        idx = 1;
        let digits_start = idx;
        while idx < bytes.len() && bytes[idx].is_ascii_digit() {
            idx += 1;
        }
        if idx == digits_start {
            return None;
        }
        SourceSpan::new(base + digits_start, base + idx)
    } else {
        let digits_start = idx;
        while idx < bytes.len() && bytes[idx].is_ascii_digit() {
            idx += 1;
        }
        if idx == digits_start {
            return None;
        }
        let start_span = SourceSpan::new(base + digits_start, base + idx);
        if idx < bytes.len() && bytes[idx] == b'-' {
            idx += 1;
            let end_digits_start = idx;
            while idx < bytes.len() && bytes[idx].is_ascii_digit() {
                idx += 1;
            }
            if idx == end_digits_start {
                return None;
            }
            SourceSpan::new(start_span.start, base + idx)
        } else {
            start_span
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
    let (label, tail) = parse_quoted_string_spanned(rest, label_base)?;
    if !tail.trim().is_empty() {
        return None;
    }

    Some(PacketBlockSpanned {
        numeric_span,
        label,
    })
}

fn parse_quoted_string_spanned<'a>(
    input: &'a str,
    base_offset: usize,
) -> Option<(SpannedText<'a>, &'a str)> {
    let mut chars = input.char_indices();
    let (_, quote) = chars.next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let mut escaped = false;
    for (idx, c) in chars {
        if escaped {
            escaped = false;
            continue;
        }
        if c == '\\' {
            escaped = true;
            continue;
        }
        if c == quote {
            let text = &input[1..idx];
            let text = text.trim();
            return Some((
                SpannedText {
                    text,
                    start: base_offset + 1,
                    end: base_offset + idx,
                },
                &input[idx + c.len_utf8()..],
            ));
        }
    }
    None
}

fn packet_header_token_len(line: &str) -> Option<usize> {
    if line.starts_with("packet-beta") {
        let rest = &line["packet-beta".len()..];
        if keyword_boundary(rest) {
            return Some("packet-beta".len());
        }
    }
    if line.starts_with("packet") {
        let rest = &line["packet".len()..];
        if keyword_boundary(rest) {
            return Some("packet".len());
        }
    }
    None
}

fn keyword_boundary(rest: &str) -> bool {
    rest.is_empty()
        || rest.starts_with("%%")
        || rest.chars().next().is_some_and(char::is_whitespace)
}

fn packet_body_start(code: &str) -> Result<Option<usize>> {
    let mut offset = 0usize;
    while offset < code.len() {
        let (line, next_offset) = physical_line(code, offset);
        let visible = strip_inline_comment(line);
        let trimmed = visible.trim_start();
        if trimmed.trim().is_empty() {
            offset = next_offset;
            continue;
        }

        let leading = visible.len() - trimmed.len();
        let Some(header_len) = packet_header_token_len(trimmed) else {
            return Err(Error::diagram_parse_fallback(
                "packet".to_string(),
                "expected packet".to_string(),
            ));
        };
        let body_start = offset + leading + header_len;
        let same_line_body = visible[leading + header_len..].trim();
        if !same_line_body.is_empty()
            && parse_langium_common(code, body_start).is_none()
            && parse_packet_block(same_line_body).is_none()
        {
            return Err(Error::diagram_parse_fallback(
                "packet".to_string(),
                "expected packet".to_string(),
            ));
        }
        return Ok(Some(body_start));
    }
    Ok(None)
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

fn parse_packet_block(line: &str) -> Option<PacketBlock> {
    let mut rest = line.trim_start();

    let (start, end, bits) = if let Some(after_plus) = rest.strip_prefix('+') {
        let (bits, tail) = parse_int_token(after_plus.trim_start())?;
        rest = tail;
        (None, None, Some(bits))
    } else {
        let (start, tail) = parse_int_token(rest)?;
        rest = tail.trim_start();
        let mut end = None;
        if let Some(after_dash) = rest.strip_prefix('-') {
            let (e, tail) = parse_int_token(after_dash.trim_start())?;
            end = Some(e);
            rest = tail;
        }
        (Some(start), end, None)
    };

    let rest2 = rest.trim_start();
    let rest2 = rest2.strip_prefix(':')?.trim_start();
    let (label, tail) = parse_quoted_string(rest2)?;
    if !tail.trim().is_empty() {
        return None;
    }

    Some(PacketBlock {
        start,
        end,
        bits,
        label,
    })
}

fn parse_int_token(input: &str) -> Option<(i64, &str)> {
    let mut idx = 0usize;
    for c in input.chars() {
        if c.is_ascii_digit() {
            idx += c.len_utf8();
        } else {
            break;
        }
    }
    if idx == 0 {
        return None;
    }
    let token = &input[..idx];
    if token.len() > 1 && token.starts_with('0') {
        return None;
    }
    let value: i64 = token.parse().ok()?;
    Some((value, &input[idx..]))
}

fn parse_quoted_string(input: &str) -> Option<(String, &str)> {
    let mut chars = input.chars();
    let quote = chars.next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let mut out = String::new();
    let mut escaped = false;
    let mut idx = 1;
    for c in chars {
        idx += c.len_utf8();
        if escaped {
            out.push(c);
            escaped = false;
            continue;
        }
        if c == '\\' {
            escaped = true;
            continue;
        }
        if c == quote {
            return Some((out, &input[idx..]));
        }
        out.push(c);
    }
    None
}

fn config_i64(config: &MermaidConfig, dotted_path: &str) -> Option<i64> {
    let mut cur = config.as_value();
    for segment in dotted_path.split('.') {
        cur = cur.as_object()?.get(segment)?;
    }
    match cur {
        Value::Number(n) => n.as_i64().or_else(|| n.as_u64().map(|v| v as i64)),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy)]
struct SpannedText<'a> {
    text: &'a str,
    start: usize,
    end: usize,
}

#[derive(Debug, Clone, Copy)]
struct PacketBlockSpanned<'a> {
    numeric_span: SourceSpan,
    label: SpannedText<'a>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Engine, ParseOptions};
    use futures::executor::block_on;
    use serde_json::json;

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
    fn packet_editor_facts_expose_parser_backed_spans() {
        let engine = crate::Engine::new();
        let text = r#"packet
title Packet diagram
accTitle: Packet accTitle
accDescr: Packet accDescription
0-10: "test"
11: "single"
"#;
        let facts = engine
            .parse_editor_semantic_facts_with_type_sync(
                "packet",
                text,
                crate::ParseOptions::strict(),
            )
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
