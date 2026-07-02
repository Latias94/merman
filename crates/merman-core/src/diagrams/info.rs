use crate::diagrams::scan::strip_line_ending;
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
    let mut header_seen = false;
    let mut offset = 0usize;

    for segment in code.split_inclusive('\n') {
        let line_start = offset;
        offset += segment.len();
        let line = strip_line_ending(segment);
        let trimmed = strip_inline_comment(line).trim();
        if trimmed.is_empty() {
            continue;
        }

        if !header_seen {
            if !trimmed.starts_with("info") {
                return facts;
            }
            header_seen = true;
            let rest = trimmed["info".len()..].trim();
            if rest.is_empty() {
                continue;
            }
            if rest == "showInfo" {
                facts.push_directive_prefix("showInfo");
                let rel = line.find("showInfo").unwrap_or(0);
                let span = SourceSpan::new(line_start + rel, line_start + rel + "showInfo".len());
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
            }
            continue;
        }
    }

    facts
}

fn parse_info_model(code: &str, meta: &ParseMetadata) -> Result<InfoParseOutput> {
    let mut header: Option<String> = None;
    let mut rest_lines = Vec::new();

    for line in code.lines() {
        let t = strip_inline_comment(line).trim();
        if t.is_empty() {
            continue;
        }
        if header.is_none() {
            header = Some(t.to_string());
        } else {
            rest_lines.push(t.to_string());
        }
    }

    let Some(header) = header else {
        return Ok(InfoParseOutput::Empty);
    };

    let mut tokens = header.split_whitespace();
    let Some(first) = tokens.next() else {
        return Ok(InfoParseOutput::Empty);
    };

    if first != "info" {
        return Ok(InfoParseOutput::Error(json!({ "error": "expected info" })));
    }

    let mut show_info = false;
    let mut unsupported: Option<String> = None;
    for tok in tokens {
        if tok == "showInfo" {
            show_info = true;
            continue;
        }
        unsupported = Some(tok.to_string());
        break;
    }

    // Upstream Mermaid accepts both:
    // - `info showInfo`
    // - `info\nshowInfo`
    //
    // The Langium grammar (`packages/parser/src/language/info/info.langium`) allows an optional
    // `showInfo` token after the initial `info` keyword, separated by newlines.
    if unsupported.is_none() && !rest_lines.is_empty() {
        for line in &rest_lines {
            let it = line.split_whitespace();
            for tok in it {
                if tok == "showInfo" {
                    show_info = true;
                    continue;
                }
                unsupported = Some(tok.to_string());
                break;
            }
            if unsupported.is_some() {
                break;
            }
        }
    }

    if unsupported.is_none() {
        return Ok(InfoParseOutput::Model(InfoDiagramRenderModel { show_info }));
    }

    let bad = unsupported.unwrap_or_else(|| rest_lines.first().cloned().unwrap_or_default());
    let ch = bad.chars().next().unwrap_or('?');
    let skipped = bad.chars().count();
    let offset = code.find(&bad).unwrap_or(5);

    Err(Error::diagram_parse_fallback(
        meta.diagram_type.clone(),
        format!(
            "Parsing failed: unexpected character: ->{ch}<- at offset: {offset}, skipped {skipped} characters."
        ),
    ))
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
    use crate::{Engine, ParseOptions};

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
}
