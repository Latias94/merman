use crate::diagrams::langium_common::{
    LangiumCommonParse, parse_langium_common, push_langium_common_editor_fact,
    push_langium_common_recovery,
};
use crate::family::EditorFactsDemand;
use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
    EditorSemanticSymbol, Error, ParseMetadata, Result, SourceSpan, family,
};
use serde_json::{Value, json};

/// Canonical meaning of an Info diagram, without compatibility-output state.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct InfoDiagramModel {
    #[serde(rename = "showInfo")]
    pub show_info: bool,
}

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
    #[default]
    Model,
}

struct InfoSemanticSource {
    model: InfoDiagramModel,
    compat_output: InfoCompatOutput,
    editor_facts: Option<EditorSemanticFacts>,
}

impl InfoSemanticSource {
    fn into_render_model(self) -> InfoDiagramRenderModel {
        InfoDiagramRenderModel {
            show_info: self.model.show_info,
            compat_output: self.compat_output,
        }
    }
}

pub(crate) fn parse_info(code: &str, meta: &ParseMetadata) -> Result<Value> {
    let model = parse_info_model_for_render_controlled(code, meta, &crate::OperationControl::new())
        .expect("a private parse control cannot be cancelled")?;
    render_model_to_compat_json(&model, meta)
}

pub(crate) fn parse_info_json_and_editor_facts(
    code: &str,
    meta: &ParseMetadata,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<family::CombinedSemanticParse> {
    let construction = construct_info_semantic_source_controlled(
        code,
        &meta.diagram_type,
        EditorFactsDemand::Capture,
        control,
    )?;
    let parsed = family::CombinedSemanticParse::from_construction(
        construction,
        |mut source| {
            let editor_facts = source.editor_facts.take().unwrap_or_default();
            (
                render_model_to_compat_json(&source.into_render_model(), meta),
                editor_facts,
            )
        },
        family::CombinedSemanticFailure::into_parts,
    );
    control.checkpoint()?;
    Ok(parsed)
}

pub(crate) fn render_model_to_compat_json(
    model: &InfoDiagramRenderModel,
    meta: &ParseMetadata,
) -> Result<Value> {
    Ok(match &model.compat_output {
        InfoCompatOutput::Empty => json!({}),
        InfoCompatOutput::Model => json!({
            "type": meta.diagram_type,
            "showInfo": model.show_info,
        }),
    })
}

#[cfg(test)]
pub(crate) fn parse_info_model_for_render(
    code: &str,
    meta: &ParseMetadata,
) -> Result<InfoDiagramRenderModel> {
    parse_info_model_for_render_controlled(code, meta, &crate::OperationControl::new())
        .expect("a private parse control cannot be cancelled")
}

pub(crate) fn parse_info_model_for_render_controlled(
    code: &str,
    meta: &ParseMetadata,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<Result<InfoDiagramRenderModel>> {
    let construction = construct_info_semantic_source_controlled(
        code,
        &meta.diagram_type,
        EditorFactsDemand::Omit,
        control,
    )?;
    match construction {
        Ok(source) => Ok(Ok(source.into_render_model())),
        Err(failure) => Ok(Err(failure.into_error())),
    }
}

pub(crate) fn parse_info_semantic_model_controlled(
    code: &str,
    diagram_type: &str,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<Result<InfoDiagramModel>> {
    let construction = construct_info_semantic_source_controlled(
        code,
        diagram_type,
        EditorFactsDemand::Omit,
        control,
    )?;
    match construction {
        Ok(source) => Ok(Ok(source.model)),
        Err(failure) => Ok(Err(failure.into_error())),
    }
}

fn construct_info_semantic_source_controlled(
    code: &str,
    diagram_type: &str,
    editor_demand: EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<
    std::result::Result<InfoSemanticSource, family::CombinedSemanticFailure>,
> {
    control.checkpoint()?;
    #[cfg(test)]
    crate::diagrams::langium_common::record_family_syntax_construction("info");

    let body = match info_body_start_controlled(code, control)? {
        InfoHeader::Empty => {
            return Ok(Ok(InfoSemanticSource {
                model: InfoDiagramModel::default(),
                compat_output: InfoCompatOutput::Empty,
                editor_facts: None,
            }));
        }
        InfoHeader::ExpectedInfo(span) => {
            return Ok(Err(family::CombinedSemanticFailure::parser_recovery(
                "info",
                Error::diagram_parse_exact(diagram_type, "expected info", span),
                EditorSemanticFacts::new(),
            )));
        }
        InfoHeader::Body(body) => body,
    };
    let mut offset = body.offset;
    let mut editor_facts = match editor_demand {
        EditorFactsDemand::Omit => None,
        EditorFactsDemand::Capture => {
            #[cfg(test)]
            crate::editor::test_work::record_collection();
            Some(EditorSemanticFacts::new())
        }
    };
    let mut show_info = false;
    let mut common_seen = false;
    let mut first_error = None;

    while offset < code.len() {
        control.checkpoint()?;
        match parse_info_statement(code, offset, !show_info && !common_seen) {
            InfoStatement::Common(parsed) => {
                if let Some(diagnostic) = &parsed.diagnostic {
                    if let Some(facts) = &mut editor_facts {
                        push_langium_common_recovery(facts, diagnostic);
                    }
                    first_error.get_or_insert_with(|| {
                        Error::diagram_parse_insertion_point(
                            diagram_type.to_string(),
                            diagnostic.message.clone(),
                            diagnostic.span.start,
                        )
                    });
                }
                if let Some(facts) = &mut editor_facts {
                    push_langium_common_editor_fact(facts, &parsed.fact, "info");
                }
                common_seen = true;
                offset += parsed.consumed;
            }
            InfoStatement::ShowInfo { span, consumed } => {
                show_info = true;
                if let Some(facts) = &mut editor_facts {
                    push_show_info_editor_fact(facts, span);
                }
                offset += consumed;
            }
            InfoStatement::Empty { next_offset } => offset = next_offset,
            InfoStatement::Unexpected {
                ch,
                skipped,
                bad_offset,
                span,
                next_offset,
            } => {
                if let Some(facts) = &mut editor_facts {
                    facts.mark_recovered_from_parse_error("unexpected info statement", Some(span));
                }
                first_error.get_or_insert_with(|| {
                    Error::diagram_parse_fallback(
                        diagram_type.to_string(),
                        format!(
                            "Parsing failed: unexpected character: ->{ch}<- at offset: {bad_offset}, skipped {skipped} characters."
                        ),
                    )
                });
                offset = next_offset;
            }
        }
    }

    if let Some(error) = first_error {
        let editor_facts = editor_facts.unwrap_or_default();
        return Ok(Err(family::CombinedSemanticFailure::new(
            error,
            editor_facts,
        )));
    }

    control.checkpoint()?;
    Ok(Ok(InfoSemanticSource {
        model: InfoDiagramModel { show_info },
        compat_output: InfoCompatOutput::Model,
        editor_facts,
    }))
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
    ExpectedInfo(SourceSpan),
    Body(InfoBodyStart),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct InfoBodyStart {
    offset: usize,
}

fn info_body_start_controlled(
    code: &str,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<InfoHeader> {
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
        let header_start = offset + leading;
        let Some(header_len) = keyword_token_len(trimmed, "info") else {
            let token_len = trimmed.find(char::is_whitespace).unwrap_or(trimmed.len());
            return Ok(InfoHeader::ExpectedInfo(SourceSpan::new(
                header_start,
                header_start + token_len,
            )));
        };
        return Ok(InfoHeader::Body(InfoBodyStart {
            offset: header_start + header_len,
        }));
    }
    Ok(InfoHeader::Empty)
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
    use crate::{
        EditorSemanticCompleteness, EditorSemanticDiagnosticKind, Engine, MermaidConfig,
        ParseDiagnosticSpanKind, ParseOptions,
    };

    fn test_meta() -> ParseMetadata {
        ParseMetadata {
            diagram_type: "info".to_string(),
            config: MermaidConfig::default(),
            effective_config: crate::presentation::PresentationConfig::default(),
            title: None,
        }
    }

    #[test]
    fn controlled_parse_can_cancel_after_the_info_header() {
        let control = crate::OperationControl::new();
        control.cancel_after_checkpoints(2);

        assert!(matches!(
            construct_info_semantic_source_controlled(
                "info\nshowInfo\ntitle Version\n",
                "info",
                EditorFactsDemand::Omit,
                &control,
            ),
            Err(crate::OperationCancelled { .. })
        ));
    }

    #[test]
    fn typed_info_projection_does_not_collect_editor_facts() {
        for (source, valid) in [
            ("", true),
            ("info\nshowInfo\n", true),
            ("info\ntitle Version\nshowInfo\n", false),
            ("infox", false),
        ] {
            crate::editor::test_work::reset();
            let parsed = parse_info_model_for_render_controlled(
                source,
                &test_meta(),
                &crate::OperationControl::new(),
            )
            .expect("typed model parse");
            assert_eq!(parsed.is_ok(), valid, "{source:?}");
            assert_eq!(crate::editor::test_work::read(), Default::default());

            let compat = parse_info(source, &test_meta());
            assert_eq!(compat.is_ok(), valid, "{source:?}");
            assert_eq!(crate::editor::test_work::read(), Default::default());
        }
        crate::editor::test_work::reset();
    }

    #[test]
    fn parse_info_editor_facts_expose_parser_backed_spans() {
        crate::editor::test_work::reset();
        let engine = Engine::new();
        let text = "info showInfo\n";
        let facts = engine
            .parse_editor_semantic_facts_with_type_sync("info", text)
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
        assert_eq!(crate::editor::test_work::read().collections, 1);
        assert!(crate::editor::test_work::read().symbols > 0);
        crate::editor::test_work::reset();
    }

    #[test]
    fn info_accepts_show_info_before_common_fields_like_pinned_grammar() {
        let source = "info\nshowInfo\ntitle Version\naccDescr: Build metadata\n";
        let model = parse_info(source, &test_meta()).unwrap();
        assert_eq!(model["showInfo"], true);

        let facts = crate::family::test_support::editor_facts(
            parse_info_json_and_editor_facts,
            source,
            &test_meta(),
        );
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

        let facts = crate::family::test_support::editor_facts(
            parse_info_json_and_editor_facts,
            source,
            &test_meta(),
        );
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
    fn info_invalid_headers_fail_all_family_parsers_with_exact_recovery_facts() {
        let meta = test_meta();
        for (source, expected_span) in [
            ("infox", SourceSpan::new(0, 5)),
            ("  infox\nshowInfo\n", SourceSpan::new(2, 7)),
            (
                "\r\n%% skipped\r\n \tinfox%% comment\r\n",
                SourceSpan::new(16, 21),
            ),
            ("packet", SourceSpan::new(0, 6)),
            ("flowchart A", SourceSpan::new(0, 9)),
            ("信息x%% comment\n", SourceSpan::new(0, 7)),
        ] {
            let semantic = parse_info(source, &meta).expect_err("invalid info header must fail");
            let typed = parse_info_model_for_render(source, &meta)
                .expect_err("invalid info header must fail typed parsing");
            let controlled = parse_info_model_for_render_controlled(
                source,
                &meta,
                &crate::OperationControl::new(),
            )
            .expect("parsing must not be cancelled")
            .expect_err("invalid info header must fail controlled typed parsing");
            let (model, facts, warnings) =
                parse_info_json_and_editor_facts(source, &meta, &crate::OperationControl::new())
                    .expect("parsing must not be cancelled")
                    .into_parts();
            let combined = model.expect_err("invalid info header must fail combined parsing");

            for error in [semantic, typed, controlled, combined] {
                let Error::DiagramParse {
                    diagram_type,
                    diagnostic,
                } = error
                else {
                    panic!("expected diagram parse failure for {source:?}");
                };
                assert_eq!(diagram_type, "info");
                assert_eq!(diagnostic.message(), "expected info");
                assert_eq!(diagnostic.span(), Some(expected_span), "{source:?}");
                assert_eq!(diagnostic.span_kind(), ParseDiagnosticSpanKind::Exact);
            }
            assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
            assert_eq!(facts.diagnostics.len(), 1);
            assert_eq!(
                facts.diagnostics[0].kind,
                EditorSemanticDiagnosticKind::ParserRecovery
            );
            assert_eq!(facts.diagnostics[0].span, Some(expected_span));
            assert_eq!(
                facts.diagnostics[0].message,
                "info parser recovered after parse error: expected info"
            );
            assert!(facts.symbols.is_empty());
            assert!(warnings.is_empty());
        }
    }

    #[test]
    fn info_invalid_headers_fail_strict_and_use_lenient_error_diagrams() {
        let engine = Engine::new();
        for source in ["infox", "packet"] {
            assert!(
                engine
                    .parse_diagram_with_type_sync("info", source, ParseOptions::strict())
                    .is_err()
            );
            assert!(
                engine
                    .parse_diagram_for_render_model_with_type_sync(
                        "info",
                        source,
                        ParseOptions::strict(),
                    )
                    .is_err()
            );

            let semantic = engine
                .parse_diagram_with_type_sync("info", source, ParseOptions::lenient())
                .unwrap()
                .expect("lenient parsing must produce an error diagram");
            assert_eq!(semantic.meta.diagram_type, "error");
            assert_eq!(semantic.model["type"], "error");
            assert!(
                semantic.model["errorMessage"]
                    .as_str()
                    .is_some_and(|message| message.contains("expected info"))
            );
            let typed = engine
                .parse_diagram_for_render_model_with_type_sync(
                    "info",
                    source,
                    ParseOptions::lenient(),
                )
                .unwrap()
                .expect("lenient typed parsing must produce an error diagram");
            assert_eq!(typed.metadata().diagram_type, "error");
            let crate::RenderSemanticModel::Error(model) = typed.model() else {
                panic!("lenient typed parsing must use the error model");
            };
            assert!(
                model
                    .error_message
                    .as_deref()
                    .is_some_and(|message| message.contains("expected info"))
            );
        }
    }

    #[test]
    fn info_typed_projection_preserves_empty_and_model_outputs() {
        let meta = test_meta();
        for source in [
            "",
            " \t\r\n%% comment\n",
            "info",
            "info\n",
            "info%% comment\n",
            "info showInfo\n",
        ] {
            let compat = parse_info(source, &meta).unwrap();
            let typed = parse_info_model_for_render(source, &meta).unwrap();
            assert_eq!(
                render_model_to_compat_json(&typed, &meta).unwrap(),
                compat,
                "Info projection drifted for {source:?}"
            );
        }

        assert_eq!(parse_info("", &meta).unwrap(), json!({}));
        assert_eq!(parse_info(" \t\r\n%% comment\n", &meta).unwrap(), json!({}));
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
