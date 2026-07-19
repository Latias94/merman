use crate::diagrams::langium_common::{
    LangiumCommonParse, LangiumLexemeTrace, parse_langium_common, push_langium_common_editor_fact,
    push_langium_common_recovery,
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
    #[serde(skip)]
    compat_output: InfoCompatOutput,
}

#[derive(Debug, Clone, Default)]
enum InfoCompatOutput {
    Empty,
    ExpectedInfoError,
    #[default]
    Model,
}

enum InfoParseOutput {
    Empty,
    ExpectedInfoError,
    Model(InfoDiagramRenderModel),
}

struct InfoSemanticSource {
    output: InfoParseOutput,
    editor_facts: EditorSemanticFacts,
}

pub fn parse_info(code: &str, meta: &ParseMetadata) -> Result<Value> {
    let model = info_output_into_render_model(parse_info_semantic_source(code, meta)?.output);
    render_model_to_compat_json(&model, meta)
}

pub(crate) fn parse_info_json_and_editor_facts(
    code: &str,
    meta: &ParseMetadata,
) -> Result<(Value, EditorSemanticFacts)> {
    let InfoSemanticSource {
        output,
        editor_facts,
    } = parse_info_semantic_source(code, meta)?;
    let model = info_output_into_render_model(output);
    Ok((render_model_to_compat_json(&model, meta)?, editor_facts))
}

fn info_output_into_render_model(output: InfoParseOutput) -> InfoDiagramRenderModel {
    match output {
        InfoParseOutput::Empty => InfoDiagramRenderModel {
            show_info: false,
            compat_output: InfoCompatOutput::Empty,
        },
        InfoParseOutput::ExpectedInfoError => InfoDiagramRenderModel {
            show_info: false,
            compat_output: InfoCompatOutput::ExpectedInfoError,
        },
        InfoParseOutput::Model(model) => model,
    }
}

pub(crate) fn render_model_to_compat_json(
    model: &InfoDiagramRenderModel,
    meta: &ParseMetadata,
) -> Result<Value> {
    Ok(match &model.compat_output {
        InfoCompatOutput::Empty => json!({}),
        InfoCompatOutput::ExpectedInfoError => json!({ "error": "expected info" }),
        InfoCompatOutput::Model => json!({
            "type": meta.diagram_type,
            "showInfo": model.show_info,
        }),
    })
}

pub fn parse_info_model_for_render(
    code: &str,
    meta: &ParseMetadata,
) -> Result<InfoDiagramRenderModel> {
    Ok(info_output_into_render_model(
        parse_info_semantic_source(code, meta)?.output,
    ))
}

pub fn parse_info_editor_facts(code: &str, _meta: &ParseMetadata) -> EditorSemanticFacts {
    parse_info_editor_source(code)
}

fn parse_info_editor_source(code: &str) -> EditorSemanticFacts {
    let mut facts = EditorSemanticFacts::new();
    let InfoHeader::Body(body) = info_body_start(code) else {
        return facts;
    };
    let mut offset = body.offset;
    let mut lexemes = LangiumLexemeTrace::default();
    lexemes.keyword(body.header_span);
    let mut show_info_seen = false;
    let mut common_seen = false;

    while offset < code.len() {
        match parse_info_statement(code, offset, !show_info_seen && !common_seen) {
            InfoStatement::Common(parsed) => {
                common_seen = true;
                lexemes.extend(parsed.lexemes.clone());
                push_langium_common_editor_fact(&mut facts, &parsed.fact, "info");
                if let Some(diagnostic) = &parsed.diagnostic {
                    push_langium_common_recovery(&mut facts, diagnostic);
                }
                offset += parsed.consumed;
            }
            InfoStatement::ShowInfo { span, consumed } => {
                show_info_seen = true;
                lexemes.keyword(span);
                push_show_info_editor_fact(&mut facts, span);
                offset += consumed;
            }
            InfoStatement::Empty { next_offset } => offset = next_offset,
            InfoStatement::Unexpected {
                span, next_offset, ..
            } => {
                facts.mark_recovered_from_parse_error("unexpected info statement", Some(span));
                offset = next_offset;
            }
        }
    }

    lexemes.attach(code, &mut facts);
    facts
}

fn parse_info_semantic_source(code: &str, meta: &ParseMetadata) -> Result<InfoSemanticSource> {
    #[cfg(test)]
    crate::diagrams::langium_common::record_family_syntax_construction("info");

    let body = match info_body_start(code) {
        InfoHeader::Empty => {
            return Ok(InfoSemanticSource {
                output: InfoParseOutput::Empty,
                editor_facts: EditorSemanticFacts::new(),
            });
        }
        InfoHeader::ExpectedInfo => {
            return Ok(InfoSemanticSource {
                output: InfoParseOutput::ExpectedInfoError,
                editor_facts: EditorSemanticFacts::new(),
            });
        }
        InfoHeader::Body(body) => body,
    };
    let mut offset = body.offset;
    let mut lexemes = LangiumLexemeTrace::default();
    lexemes.keyword(body.header_span);
    let mut editor_facts = EditorSemanticFacts::new();
    let mut show_info = false;
    let mut common_seen = false;

    while offset < code.len() {
        match parse_info_statement(code, offset, !show_info && !common_seen) {
            InfoStatement::Common(parsed) => {
                if let Some(diagnostic) = &parsed.diagnostic {
                    return Err(Error::diagram_parse_insertion_point(
                        meta.diagram_type.clone(),
                        diagnostic.message.clone(),
                        diagnostic.span.start,
                    ));
                }
                lexemes.extend(parsed.lexemes.clone());
                push_langium_common_editor_fact(&mut editor_facts, &parsed.fact, "info");
                common_seen = true;
                offset += parsed.consumed;
            }
            InfoStatement::ShowInfo { span, consumed } => {
                show_info = true;
                lexemes.keyword(span);
                push_show_info_editor_fact(&mut editor_facts, span);
                offset += consumed;
            }
            InfoStatement::Empty { next_offset } => offset = next_offset,
            InfoStatement::Unexpected {
                ch,
                skipped,
                bad_offset,
                ..
            } => {
                return Err(Error::diagram_parse_fallback(
                    meta.diagram_type.clone(),
                    format!(
                        "Parsing failed: unexpected character: ->{ch}<- at offset: {bad_offset}, skipped {skipped} characters."
                    ),
                ));
            }
        }
    }

    lexemes.attach(code, &mut editor_facts);

    Ok(InfoSemanticSource {
        output: InfoParseOutput::Model(InfoDiagramRenderModel {
            show_info,
            compat_output: InfoCompatOutput::Model,
        }),
        editor_facts,
    })
}

enum InfoStatement {
    Common(LangiumCommonParse),
    ShowInfo {
        span: SourceSpan,
        consumed: usize,
    },
    Empty {
        next_offset: usize,
    },
    Unexpected {
        ch: char,
        skipped: usize,
        bad_offset: usize,
        span: SourceSpan,
        next_offset: usize,
    },
}

fn parse_info_statement(code: &str, offset: usize, allow_show_info: bool) -> InfoStatement {
    if let Some(parsed) = parse_langium_common(code, offset) {
        return InfoStatement::Common(parsed);
    }

    let (line, next_offset) = physical_line(code, offset);
    let visible = strip_inline_comment(line);
    let trimmed = visible.trim();
    if trimmed.is_empty() {
        return InfoStatement::Empty { next_offset };
    }

    let leading = visible.len() - visible.trim_start().len();
    if allow_show_info && let Some(token_len) = keyword_token_len(visible.trim_start(), "showInfo")
    {
        let start = offset + leading;
        return InfoStatement::ShowInfo {
            span: SourceSpan::new(start, start + token_len),
            consumed: leading + token_len,
        };
    }

    let bad_offset = offset + visible.find(trimmed).unwrap_or_default();
    InfoStatement::Unexpected {
        ch: trimmed.chars().next().unwrap_or('?'),
        skipped: trimmed.chars().count(),
        bad_offset,
        span: SourceSpan::new(bad_offset, bad_offset + trimmed.len()),
        next_offset,
    }
}

fn push_show_info_editor_fact(facts: &mut EditorSemanticFacts, span: SourceSpan) {
    facts.push_directive_prefix("showInfo");
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InfoHeader {
    Empty,
    ExpectedInfo,
    Body(InfoBodyStart),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct InfoBodyStart {
    offset: usize,
    header_span: SourceSpan,
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
        let header_start = offset + leading;
        return InfoHeader::Body(InfoBodyStart {
            offset: header_start + header_len,
            header_span: SourceSpan::new(header_start, header_start + header_len),
        });
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
        assert!(facts.diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("unexpected info statement")
                && diagnostic.kind == crate::EditorSemanticDiagnosticKind::ParserRecovery
                && diagnostic.span
                    == Some(SourceSpan::new(
                        source.find("showInfo").unwrap(),
                        source.find("showInfo").unwrap() + "showInfo".len(),
                    ))
        }));
    }

    #[test]
    fn info_typed_projection_preserves_empty_error_and_model_outputs() {
        let meta = test_meta();
        for source in ["", "flowchart A", "info\n", "info showInfo\n"] {
            let compat = parse_info(source, &meta).unwrap();
            let typed = parse_info_model_for_render(source, &meta).unwrap();
            assert_eq!(
                render_model_to_compat_json(&typed, &meta).unwrap(),
                compat,
                "Info projection drifted for {source:?}"
            );
        }

        assert_eq!(parse_info("", &meta).unwrap(), json!({}));
        assert_eq!(
            parse_info("flowchart A", &meta).unwrap(),
            json!({ "error": "expected info" })
        );
        assert_eq!(
            parse_info("info\n", &meta).unwrap(),
            json!({ "type": "info", "showInfo": false })
        );
        assert_eq!(
            render_model_to_compat_json(&InfoDiagramRenderModel::default(), &meta).unwrap(),
            json!({ "type": "info", "showInfo": false })
        );
    }
}
