use crate::common_db::LangiumCommonDbFields;
use crate::diagrams::langium_common::{
    LangiumCommonFacts, parse_langium_common, push_langium_common_editor_fact,
    push_langium_common_recovery,
};
use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
    EditorSemanticSymbol, Error, ParseMetadata, Result, SourceSpan,
};
use serde_json::{Value, json};
use std::collections::HashSet;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct PieDiagramRenderModel {
    #[serde(rename = "showData")]
    pub show_data: bool,
    pub title: Option<String>,
    #[serde(rename = "accTitle")]
    pub acc_title: Option<String>,
    #[serde(rename = "accDescr")]
    pub acc_descr: Option<String>,
    pub sections: Vec<PieRenderSection>,
}

impl PieDiagramRenderModel {
    pub(crate) fn sanitize_common_db_fields(&mut self, config: &crate::MermaidConfig) {
        crate::common_db::sanitize_optional_title(&mut self.title, config);
        crate::common_db::sanitize_optional_acc_title(&mut self.acc_title, config);
        crate::common_db::sanitize_optional_acc_descr(&mut self.acc_descr, config);
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PieRenderSection {
    pub label: String,
    pub value: f64,
}

enum PieParseOutput {
    Empty,
    ExpectedPie,
    Model(PieDiagramRenderModel),
}

pub fn parse_pie(code: &str, meta: &ParseMetadata) -> Result<Value> {
    match parse_pie_model(code, meta)? {
        PieParseOutput::Empty => Ok(json!({})),
        PieParseOutput::ExpectedPie => Ok(json!({ "error": "expected pie" })),
        PieParseOutput::Model(model) => Ok(json!({
            "type": meta.diagram_type,
            "showData": model.show_data,
            "title": model.title,
            "accTitle": model.acc_title,
            "accDescr": model.acc_descr,
            "sections": model.sections,
        })),
    }
}

pub fn parse_pie_model_for_render(
    code: &str,
    meta: &ParseMetadata,
) -> Result<PieDiagramRenderModel> {
    match parse_pie_model(code, meta)? {
        PieParseOutput::Empty => Ok(PieDiagramRenderModel::default()),
        PieParseOutput::ExpectedPie => Err(Error::diagram_parse_fallback(
            meta.diagram_type.clone(),
            "expected pie".to_string(),
        )),
        PieParseOutput::Model(model) => Ok(model),
    }
}

pub fn parse_pie_editor_facts(code: &str, _meta: &ParseMetadata) -> EditorSemanticFacts {
    let mut facts = EditorSemanticFacts::new();
    let PieHeader::Body(body) = pie_body_start(code) else {
        return facts;
    };
    let mut offset = body.offset;
    if let Some(span) = body.show_data_span {
        facts.push_directive_prefix("showData");
        facts.push_expected_syntax(EditorExpectedSyntax::new(
            EditorExpectedSyntaxKind::Payload,
            span,
        ));
    }

    while offset < code.len() {
        if let Some(parsed) = parse_langium_common(code, offset) {
            push_langium_common_editor_fact(&mut facts, &parsed.fact, "pie");
            if let Some(diagnostic) = &parsed.diagnostic {
                push_langium_common_recovery(&mut facts, diagnostic);
            }
            offset += parsed.consumed;
            continue;
        }

        let (line, next_offset) = physical_line(code, offset);
        let visible = strip_inline_comment(line);
        let trimmed = visible.trim();
        if trimmed.is_empty() {
            offset = next_offset;
            continue;
        }

        if let Some((label, value_span)) = parse_section_spanned(line, offset) {
            facts.push_symbol(EditorSemanticSymbol::outline(
                label.text.to_string(),
                Some("pie section".to_string()),
                EditorSemanticKind::String,
                SourceSpan::new(offset, offset + line.len()),
                SourceSpan::new(label.start, label.end),
            ));
            facts.push_expected_syntax(EditorExpectedSyntax::new(
                EditorExpectedSyntaxKind::Payload,
                SourceSpan::new(value_span.start, value_span.end),
            ));
        } else {
            facts.mark_recovered_with_diagnostic(
                "unexpected pie statement",
                Some(SourceSpan::new(offset, offset + line.len())),
            );
        }
        offset = next_offset;
    }

    facts
}

fn parse_section_spanned<'a>(
    line: &'a str,
    line_start: usize,
) -> Option<(SpannedText<'a>, SpannedText<'a>)> {
    let t = strip_inline_comment(line).trim_start();
    let (label, rest) = parse_quoted_string(t)?;
    let rest = rest.trim_start();
    let rest = rest.strip_prefix(':')?.trim_start();

    let mut num = String::new();
    for c in rest.chars() {
        if c.is_ascii_digit() || c == '-' || c == '.' {
            num.push(c);
        } else {
            break;
        }
    }
    if num.is_empty() {
        return None;
    }
    let label_rel = line.find(&label)?;
    let value_rel = line.find(&num)?;
    Some((
        SpannedText {
            text: &line[label_rel..label_rel + label.len()],
            start: line_start + label_rel,
            end: line_start + label_rel + label.len(),
        },
        SpannedText {
            text: &line[value_rel..value_rel + num.len()],
            start: line_start + value_rel,
            end: line_start + value_rel + num.len(),
        },
    ))
}

#[derive(Debug, Clone, Copy)]
struct SpannedText<'a> {
    text: &'a str,
    start: usize,
    end: usize,
}

fn parse_pie_model(code: &str, meta: &ParseMetadata) -> Result<PieParseOutput> {
    let body = match pie_body_start(code) {
        PieHeader::Empty => return Ok(PieParseOutput::Empty),
        PieHeader::ExpectedPie => return Ok(PieParseOutput::ExpectedPie),
        PieHeader::Body(body) => body,
    };
    let mut offset = body.offset;
    let mut common = LangiumCommonFacts::default();
    let mut sections: Vec<PieRenderSection> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

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
        let visible = strip_inline_comment(line);
        let t = visible.trim();
        if t.is_empty() {
            offset = next_offset;
            continue;
        }

        if let Some((label, value)) = parse_section(t) {
            if value < 0.0 {
                return Err(Error::diagram_parse_fallback(
                    meta.diagram_type.clone(),
                    format!(
                        "\"{label}\" has invalid value: {value}. Negative values are not allowed in pie charts. All slice values must be >= 0."
                    ),
                ));
            }
            if seen.insert(label.clone()) {
                sections.push(PieRenderSection { label, value });
            }
            offset = next_offset;
            continue;
        }

        return Err(Error::diagram_parse_fallback(
            meta.diagram_type.clone(),
            format!("unexpected pie statement: {t}"),
        ));
    }

    let common = LangiumCommonDbFields::from_facts(&common);

    Ok(PieParseOutput::Model(PieDiagramRenderModel {
        show_data: body.show_data_span.is_some(),
        title: common.title,
        acc_title: common.acc_title,
        acc_descr: common.acc_descr,
        sections,
    }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PieHeader {
    Empty,
    ExpectedPie,
    Body(PieBodyStart),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PieBodyStart {
    offset: usize,
    show_data_span: Option<SourceSpan>,
}

fn pie_body_start(code: &str) -> PieHeader {
    let mut offset = 0usize;
    while offset < code.len() {
        let (line, next_offset) = physical_line(code, offset);
        let visible = strip_inline_comment(line);
        let trimmed = visible.trim_start();
        if trimmed.trim().is_empty() {
            offset = next_offset;
            continue;
        }
        let Some(header_len) = keyword_token_len(trimmed, "pie") else {
            return PieHeader::ExpectedPie;
        };
        let leading = visible.len() - trimmed.len();
        let after_header = offset + leading + header_len;
        let horizontal = code[after_header..]
            .chars()
            .take_while(|ch| matches!(ch, ' ' | '\t'))
            .map(char::len_utf8)
            .sum::<usize>();
        let show_data_start = after_header + horizontal;
        let show_data_span = keyword_token_len(&code[show_data_start..], "showData").map(|len| {
            debug_assert_eq!(len, "showData".len());
            SourceSpan::new(show_data_start, show_data_start + len)
        });
        let body_start = show_data_span.map_or(after_header, |span| span.end);
        return PieHeader::Body(PieBodyStart {
            offset: body_start,
            show_data_span,
        });
    }
    PieHeader::Empty
}

fn keyword_token_len(input: &str, keyword: &str) -> Option<usize> {
    let rest = input.strip_prefix(keyword)?;
    (rest.is_empty()
        || rest.starts_with("%%")
        || rest.chars().next().is_some_and(char::is_whitespace))
    .then_some(keyword.len())
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

fn strip_inline_comment(line: &str) -> &str {
    match line.find("%%") {
        Some(idx) => &line[..idx],
        None => line,
    }
}

fn parse_section(line: &str) -> Option<(String, f64)> {
    let t = line.trim_start();
    let (label, rest) = parse_quoted_string(t)?;
    let rest = rest.trim_start();
    let rest = rest.strip_prefix(':')?.trim_start();

    let mut num = String::new();
    for c in rest.chars() {
        if c.is_ascii_digit() || c == '-' || c == '.' {
            num.push(c);
        } else {
            break;
        }
    }
    if num.is_empty() {
        return None;
    }
    let value: f64 = num.parse().ok()?;
    Some((label, value))
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

#[cfg(test)]
mod tests {
    use super::parse_pie_editor_facts;
    use crate::{EditorSemanticCompleteness, Engine, MermaidConfig, ParseMetadata, ParseOptions};

    fn metadata() -> ParseMetadata {
        ParseMetadata {
            diagram_type: "pie".to_string(),
            config: MermaidConfig::default(),
            effective_config: MermaidConfig::default(),
            title: None,
        }
    }

    #[test]
    fn pie_supports_title_statement_after_header() {
        let engine = Engine::new();
        let input = r#"
pie showData
  title Market Share
  "A" : 1
  "B" : 2
"#;

        let parsed = engine
            .parse_diagram_sync(input, ParseOptions::strict())
            .unwrap()
            .expect("diagram detected");

        assert_eq!(parsed.meta.diagram_type, "pie");
        assert_eq!(
            parsed.model.get("title").and_then(|v| v.as_str()),
            Some("Market Share")
        );
        assert_eq!(
            parsed.model.get("showData").and_then(|v| v.as_bool()),
            Some(true)
        );
    }

    #[test]
    fn pie_rejects_show_data_after_the_header_line() {
        let source = "pie\nshowData\n\"A\": 1\n";
        let engine = Engine::new();

        let error = engine
            .parse_diagram_sync(source, ParseOptions::strict())
            .expect_err("showData is only valid in the pie header");
        assert!(
            error
                .to_string()
                .contains("unexpected pie statement: showData")
        );

        let facts = parse_pie_editor_facts(source, &metadata());
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
        assert!(
            !facts
                .directive_prefixes
                .iter()
                .any(|prefix| prefix == "showData")
        );
        assert!(
            facts
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("unexpected pie statement"))
        );
    }

    #[test]
    fn pie_supports_header_acc_title_inline() {
        let engine = Engine::new();
        let input = r#"
pie accTitle: sample wow
  "A" : 1
"#;

        let parsed = engine
            .parse_diagram_sync(input, ParseOptions::strict())
            .unwrap()
            .expect("diagram detected");

        assert_eq!(parsed.meta.diagram_type, "pie");
        assert_eq!(
            parsed.model.get("accTitle").and_then(|v| v.as_str()),
            Some("sample wow")
        );
    }

    #[test]
    fn pie_supports_header_acc_descr_block() {
        let engine = Engine::new();
        let input = r#"
pie accDescr {
  first line
  second line
}
  "A" : 1
"#;

        let parsed = engine
            .parse_diagram_sync(input, ParseOptions::strict())
            .unwrap()
            .expect("diagram detected");

        assert_eq!(parsed.meta.diagram_type, "pie");
        assert_eq!(
            parsed.model.get("accDescr").and_then(|v| v.as_str()),
            Some("first line\nsecond line")
        );
    }
}
