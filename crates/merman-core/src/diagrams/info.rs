use crate::diagrams::langium_common::{
    parse_langium_common, push_langium_common_editor_fact, push_langium_common_recovery,
};
use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
    EditorSemanticSymbol, Error, ParseMetadata, Result, SourceSpan,
};
use serde_json::{Value, json};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct InfoDiagramRenderModel {
    #[serde(rename = "showInfo")]
    pub show_info: bool,
}

enum InfoParseOutput {
    Empty,
    Error(Value),
    Model(InfoDiagramRenderModel),
}

pub fn parse_info(code: &str, meta: &ParseMetadata) -> Result<Value> {
    match parse_info_model(code, meta)? {
        InfoParseOutput::Empty => Ok(json!({})),
        InfoParseOutput::Error(v) => Ok(v),
        InfoParseOutput::Model(model) => Ok(json!({
            "type": meta.diagram_type,
            "showInfo": model.show_info,
        })),
    }
}

pub fn parse_info_model_for_render(
    code: &str,
    meta: &ParseMetadata,
) -> Result<InfoDiagramRenderModel> {
    match parse_info_model(code, meta)? {
        InfoParseOutput::Empty | InfoParseOutput::Error(_) => Ok(InfoDiagramRenderModel::default()),
        InfoParseOutput::Model(model) => Ok(model),
    }
}

pub fn parse_info_editor_facts(code: &str, _meta: &ParseMetadata) -> EditorSemanticFacts {
    let mut facts = EditorSemanticFacts::new();
    let InfoHeader::Body(mut offset) = info_body_start(code) else {
        return facts;
    };
    let mut show_info_seen = false;
    let mut common_seen = false;

    while offset < code.len() {
        if let Some(parsed) = parse_langium_common(code, offset) {
            common_seen = true;
            push_langium_common_editor_fact(&mut facts, &parsed.fact, "info");
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

        if !show_info_seen && !common_seen && keyword_token_len(trimmed, "showInfo").is_some() {
            show_info_seen = true;
            facts.push_directive_prefix("showInfo");
            let rel = visible.find("showInfo").unwrap_or(0);
            let span = SourceSpan::new(offset + rel, offset + rel + "showInfo".len());
            facts.push_expected_syntax(EditorExpectedSyntax::new(
                EditorExpectedSyntaxKind::Payload,
                span,
            ));
            facts.push_symbol(EditorSemanticSymbol::payload(
                "showInfo".to_string(),
                Some("info showInfo".to_string()),
                EditorSemanticKind::String,
                span,
                span,
            ));
            offset += visible.find("showInfo").unwrap_or(0) + "showInfo".len();
            continue;
        }

        facts.mark_recovered_with_diagnostic(
            "unexpected info statement",
            Some(SourceSpan::new(offset, offset + line.len())),
        );
        offset = next_offset;
    }

    facts
}

fn parse_info_model(code: &str, meta: &ParseMetadata) -> Result<InfoParseOutput> {
    let mut offset = match info_body_start(code) {
        InfoHeader::Empty => return Ok(InfoParseOutput::Empty),
        InfoHeader::ExpectedInfo => {
            return Ok(InfoParseOutput::Error(json!({ "error": "expected info" })));
        }
        InfoHeader::Body(offset) => offset,
    };
    let mut show_info = false;
    let mut common_seen = false;

    while offset < code.len() {
        if let Some(parsed) = parse_langium_common(code, offset) {
            if let Some(diagnostic) = parsed.diagnostic {
                return Err(Error::diagram_parse_insertion_point(
                    meta.diagram_type.clone(),
                    diagnostic.message,
                    diagnostic.span.start,
                ));
            }
            common_seen = true;
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

        if !show_info && !common_seen {
            let leading = visible.len() - visible.trim_start().len();
            if let Some(token_len) = keyword_token_len(visible.trim_start(), "showInfo") {
                show_info = true;
                offset += leading + token_len;
                continue;
            }
        }

        let ch = trimmed.chars().next().unwrap_or('?');
        let skipped = trimmed.chars().count();
        let bad_offset = offset + visible.find(trimmed).unwrap_or(0);
        return Err(Error::diagram_parse_fallback(
            meta.diagram_type.clone(),
            format!(
                "Parsing failed: unexpected character: ->{ch}<- at offset: {bad_offset}, skipped {skipped} characters."
            ),
        ));
    }

    Ok(InfoParseOutput::Model(InfoDiagramRenderModel { show_info }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InfoHeader {
    Empty,
    ExpectedInfo,
    Body(usize),
}

fn info_body_start(code: &str) -> InfoHeader {
    let mut offset = 0usize;
    while offset < code.len() {
        let (line, next_offset) = physical_line(code, offset);
        let visible = strip_inline_comment(line);
        let trimmed = visible.trim_start();
        if trimmed.trim().is_empty() {
            offset = next_offset;
            continue;
        }
        let Some(header_len) = keyword_token_len(trimmed, "info") else {
            return InfoHeader::ExpectedInfo;
        };
        let leading = visible.len() - trimmed.len();
        return InfoHeader::Body(offset + leading + header_len);
    }
    InfoHeader::Empty
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EditorSemanticCompleteness, Engine, MermaidConfig, ParseOptions};

    fn test_meta() -> ParseMetadata {
        ParseMetadata {
            diagram_type: "info".to_string(),
            config: MermaidConfig::default(),
            effective_config: MermaidConfig::default(),
            title: None,
        }
    }

    #[test]
    fn parse_info_editor_facts_expose_parser_backed_spans() {
        let engine = Engine::new();
        let text = "info showInfo\n";
        let facts = engine
            .parse_editor_semantic_facts_with_type_sync("info", text, ParseOptions::strict())
            .unwrap()
            .unwrap();

        assert!(facts.directive_prefixes.iter().any(|p| p == "showInfo"));
        assert!(facts.symbols.iter().any(|symbol| symbol.name == "showInfo"));

        let show_info_start = text.find("showInfo").unwrap();
        assert!(facts.expected_syntax.iter().any(|expected| {
            expected.kind == EditorExpectedSyntaxKind::Payload
                && expected.span
                    == SourceSpan::new(show_info_start, show_info_start + "showInfo".len())
        }));
    }

    #[test]
    fn info_accepts_show_info_before_common_fields_like_pinned_grammar() {
        let source = "info\nshowInfo\ntitle Version\naccDescr: Build metadata\n";
        let model = parse_info(source, &test_meta()).unwrap();
        assert_eq!(model["showInfo"], true);

        let facts = parse_info_editor_facts(source, &test_meta());
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
        assert!(
            facts
                .directive_prefixes
                .iter()
                .any(|prefix| prefix == "showInfo")
        );
        assert!(
            facts
                .directive_prefixes
                .iter()
                .any(|prefix| prefix == "title")
        );
        assert!(
            facts
                .directive_prefixes
                .iter()
                .any(|prefix| prefix == "accDescr")
        );
    }

    #[test]
    fn info_rejects_show_info_after_common_fields_like_pinned_grammar() {
        let source = "info\ntitle Version\nshowInfo\n";
        assert!(parse_info(source, &test_meta()).is_err());

        let facts = parse_info_editor_facts(source, &test_meta());
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
        assert!(
            facts
                .directive_prefixes
                .iter()
                .any(|prefix| prefix == "title")
        );
        assert!(
            !facts
                .directive_prefixes
                .iter()
                .any(|prefix| prefix == "showInfo")
        );
        assert!(
            facts
                .diagnostics
                .iter()
                .any(|diagnostic| { diagnostic.message.contains("unexpected info statement") })
        );
    }
}
