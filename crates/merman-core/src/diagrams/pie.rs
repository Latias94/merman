#[cfg(feature = "editor-facts")]
use crate::diagrams::langium_common::{
    push_langium_common_editor_fact, push_langium_common_recovery,
};
#[cfg(feature = "editor-facts")]
use crate::family::EditorFactsDemand;
#[cfg(feature = "editor-facts")]
use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
    EditorSemanticSymbol,
};
use crate::{Error, family};
#[cfg(feature = "presentation")]
use crate::{Result, SourcePresentationMetadata};
#[cfg(feature = "editor-facts")]
use merman_core_structured::pie::PieObservation;
use merman_core_structured::pie::{PieBodyIssue, PieHeader, pie_body_start_controlled};
#[cfg(feature = "compat-json")]
use serde_json::Value;

use merman_model::families::pie::PieDiagramModel;
#[cfg(feature = "presentation")]
use merman_model::families::pie::PieOutputData;

pub(crate) fn sanitize_common_db_fields(
    model: &mut PieDiagramModel,
    config: &crate::MermaidConfig,
) {
    crate::common_db::sanitize_optional_title(&mut model.title, config);
    crate::common_db::sanitize_optional_acc_title(&mut model.acc_title, config);
    crate::common_db::sanitize_optional_acc_descr(&mut model.acc_descr, config);
}

struct PieSemanticSource {
    model: PieDiagramModel,
    #[cfg(feature = "presentation")]
    output_data: PieOutputData,
    #[cfg(feature = "editor-facts")]
    editor_facts: Option<EditorSemanticFacts>,
}

#[cfg(feature = "presentation")]
impl PieSemanticSource {
    fn into_render_parts(self) -> (PieDiagramModel, PieOutputData) {
        (self.model, self.output_data)
    }

    #[cfg(feature = "compat-json")]
    fn into_compat_json(self, meta: &SourcePresentationMetadata) -> Result<Value> {
        let (model, output_data) = self.into_render_parts();
        render_model_to_compat_json(&model, output_data, meta)
    }
}

#[cfg(feature = "compat-json")]
pub(crate) fn parse_pie(code: &str, meta: &SourcePresentationMetadata) -> Result<Value> {
    let source = parse_pie_semantic_source(code, meta)?;
    source.into_compat_json(meta)
}

#[cfg(feature = "compat-json")]
#[cfg(feature = "editor-facts")]
pub(crate) fn parse_pie_json_and_editor_facts(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<family::CombinedSemanticParse> {
    let construction = construct_pie_semantic_source_controlled(
        code,
        &meta.diagram_type,
        EditorFactsDemand::Capture,
        control,
    )?;
    let parsed = family::CombinedSemanticParse::from_construction(
        construction,
        |mut source| {
            let editor_facts = source.editor_facts.take().unwrap_or_default();
            (source.into_compat_json(meta), editor_facts)
        },
        family::CombinedSemanticFailure::into_parts,
    );
    control.checkpoint()?;
    Ok(parsed)
}

#[cfg(feature = "compat-json")]
pub(crate) fn render_model_to_compat_json(
    model: &PieDiagramModel,
    output_data: PieOutputData,
    meta: &SourcePresentationMetadata,
) -> Result<Value> {
    Ok(merman_model::families::pie::compat::project(
        model,
        output_data,
        &meta.diagram_type,
    ))
}

#[cfg(all(test, feature = "presentation"))]
pub(crate) fn parse_pie_model_for_render(
    code: &str,
    meta: &SourcePresentationMetadata,
) -> Result<(PieDiagramModel, PieOutputData)> {
    Ok(parse_pie_semantic_source(code, meta)?.into_render_parts())
}

#[cfg(feature = "presentation")]
pub(crate) fn parse_pie_model_for_render_controlled(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<Result<(PieDiagramModel, PieOutputData)>> {
    let construction = construct_pie_semantic_source_controlled(
        code,
        &meta.diagram_type,
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        control,
    )?;
    let source = match construction {
        Ok(source) => source,
        Err(failure) => return Ok(Err(failure.into_error())),
    };
    Ok(Ok(source.into_render_parts()))
}

pub(crate) fn parse_pie_semantic_model_controlled(
    code: &str,
    diagram_type: &str,
    config: &crate::MermaidConfig,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<crate::family::SemanticConstruction<PieDiagramModel>> {
    let construction = construct_pie_semantic_source_controlled(
        code,
        diagram_type,
        #[cfg(feature = "editor-facts")]
        editor_demand,
        control,
    )?;
    Ok(match construction {
        Ok(source) => {
            #[cfg(feature = "editor-facts")]
            let editor_facts = source.editor_facts;
            let mut model = source.model;
            sanitize_common_db_fields(&mut model, config);
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

#[cfg(any(feature = "compat-json", all(test, feature = "presentation")))]
fn parse_pie_semantic_source(
    code: &str,
    meta: &SourcePresentationMetadata,
) -> Result<PieSemanticSource> {
    construct_pie_semantic_source_controlled(
        code,
        &meta.diagram_type,
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        &crate::OperationControl::new(),
    )
    .expect("a private parse control cannot be cancelled")
    .map_err(family::CombinedSemanticFailure::into_error)
}

fn construct_pie_semantic_source_controlled(
    code: &str,
    diagram_type: &str,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<
    std::result::Result<PieSemanticSource, family::CombinedSemanticFailure>,
> {
    control.checkpoint()?;
    #[cfg(test)]
    crate::diagrams::langium_common::record_family_syntax_construction("pie");

    let body = match pie_body_start_controlled(code, control)? {
        PieHeader::Empty => {
            return Ok(Ok(PieSemanticSource {
                model: PieDiagramModel::default(),
                #[cfg(feature = "presentation")]
                output_data: PieOutputData::Empty,
                #[cfg(feature = "editor-facts")]
                editor_facts: editor_demand
                    .needs_recovery()
                    .then(EditorSemanticFacts::new),
            }));
        }
        PieHeader::ExpectedPie(span) => {
            return Ok(Err(family::CombinedSemanticFailure::parser_recovery(
                #[cfg(feature = "editor-facts")]
                "pie",
                Error::diagram_parse_exact(diagram_type.to_string(), "expected pie", span),
                #[cfg(feature = "editor-facts")]
                editor_demand
                    .needs_recovery()
                    .then(EditorSemanticFacts::new),
            )));
        }
        PieHeader::Body(body) => body,
    };
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
    #[cfg(feature = "editor-facts")]
    let mut observe = editor_facts.as_mut().map(|facts| {
        move |observation: PieObservation<'_>| project_pie_observation(facts, observation)
    });
    let parsed = body.parse_controlled(
        #[cfg(feature = "editor-facts")]
        editor_demand.captures_rich(),
        #[cfg(feature = "editor-facts")]
        observe
            .as_mut()
            .map(|observe| observe as &mut dyn FnMut(PieObservation<'_>)),
        control,
    )?;
    Ok(match parsed {
        Ok(model) => Ok(PieSemanticSource {
            model,
            #[cfg(feature = "presentation")]
            output_data: PieOutputData::Model,
            #[cfg(feature = "editor-facts")]
            editor_facts,
        }),
        Err(issue) => Err(family::CombinedSemanticFailure::new(
            pie_body_error(diagram_type, issue),
            #[cfg(feature = "editor-facts")]
            editor_facts,
        )),
    })
}

fn pie_body_error(diagram_type: &str, issue: PieBodyIssue) -> Error {
    match issue {
        PieBodyIssue::Common(diagnostic) => Error::diagram_parse_insertion_point(
            diagram_type,
            diagnostic.message,
            diagnostic.span.start,
        ),
        PieBodyIssue::Unexpected(text) => {
            Error::diagram_parse_fallback(diagram_type, format!("unexpected pie statement: {text}"))
        }
        PieBodyIssue::NegativeValue { message, span } => {
            Error::diagram_parse_exact(diagram_type, message, span)
        }
    }
}

#[cfg(feature = "editor-facts")]
fn project_pie_observation(facts: &mut EditorSemanticFacts, observation: PieObservation<'_>) {
    match observation {
        PieObservation::CommonRecovery(diagnostic) => {
            push_langium_common_recovery(facts, diagnostic);
        }
        PieObservation::Common(fact) => push_langium_common_editor_fact(facts, fact, "pie"),
        PieObservation::ShowData(span) => {
            facts.push_directive_prefix("showData");
            facts.push_expected_syntax(EditorExpectedSyntax::new(
                EditorExpectedSyntaxKind::Payload,
                span,
            ));
        }
        PieObservation::Section {
            label,
            statement_span,
            label_span,
            value_span,
        } => {
            facts.push_symbol(EditorSemanticSymbol::outline(
                label.to_owned(),
                Some("pie section".to_string()),
                EditorSemanticKind::String,
                statement_span,
                label_span,
            ));
            for span in [label_span, value_span] {
                facts.push_expected_syntax(EditorExpectedSyntax::new(
                    EditorExpectedSyntaxKind::Payload,
                    span,
                ));
            }
        }
        PieObservation::Unexpected(span) => {
            facts.mark_recovered_from_parse_error("unexpected pie statement", Some(span));
        }
        PieObservation::NegativeValue { message, span } => {
            facts.mark_recovered_from_parse_error(message, Some(span));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(all(feature = "editor-facts", not(feature = "compat-json")))]
    use crate::SourceSpan;

    #[test]
    fn canonical_empty_capture_keeps_an_available_empty_journal() {
        crate::diagrams::langium_common::reset_family_syntax_construction_count("pie");
        let parsed = parse_pie_semantic_model_controlled(
            "",
            "pie",
            &MermaidConfig::default(),
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Capture,
            &crate::OperationControl::new(),
        )
        .unwrap();
        assert!(parsed.model.is_ok());
        assert_eq!(
            crate::diagrams::langium_common::family_syntax_construction_count("pie"),
            1
        );
        #[cfg(feature = "editor-facts")]
        {
            let facts = parsed
                .editor_facts
                .expect("empty input still provides requested facts");
            assert!(facts.symbols.is_empty());
            assert!(facts.diagnostics.is_empty());
            assert_eq!(
                facts.completeness,
                crate::EditorSemanticCompleteness::Complete
            );
        }
    }
    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[cfg(feature = "editor-facts")]
    use crate::EditorSemanticCompleteness;
    use crate::MermaidConfig;
    #[cfg(feature = "presentation")]
    use crate::SourcePresentationMetadata;
    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    use crate::{Error, MermaidEngine, ParseOptions, SourceSpan};
    use serde_json::json;

    #[cfg(feature = "presentation")]
    fn metadata() -> SourcePresentationMetadata {
        SourcePresentationMetadata {
            diagram_type: "pie".to_string(),
            config: MermaidConfig::default(),
            effective_config: crate::presentation::SourcePresentationConfig::default(),
            title: None,
        }
    }

    fn parse_semantic_model(source: &str) -> crate::Result<PieDiagramModel> {
        parse_pie_semantic_model_controlled(
            source,
            "pie",
            &MermaidConfig::default(),
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &crate::OperationControl::new(),
        )
        .unwrap()
        .model
    }

    #[test]
    fn pie_rejects_non_horizontal_structural_whitespace() {
        for whitespace in [
            '\u{000b}', '\u{000c}', '\u{0085}', '\u{00a0}', '\u{2028}', '\u{2029}',
        ] {
            for source in [
                format!("{whitespace}pie\n\"A\": 1\n"),
                format!("pie{whitespace}\n\"A\": 1\n"),
                format!("pie\n{whitespace}\n\"A\": 1\n"),
                format!("pie\n{whitespace}\"A\": 1\n"),
                format!("pie\n\"A\"{whitespace}: 1\n"),
                format!("pie\n\"A\":{whitespace}1\n"),
                format!("pie\n\"A\": 1{whitespace}\n"),
            ] {
                assert!(
                    parse_semantic_model(&source).is_err(),
                    "unexpectedly accepted structural whitespace in {source:?}"
                );
            }
        }
    }

    #[test]
    fn pie_preserves_horizontal_trivia_and_terminal_owned_whitespace() {
        for newline in ["\n", "\r\n"] {
            let source = concat!(
                " \t\npie \tshowData\n",
                "\ttitle Summary\u{00a0}label\n",
                "\taccTitle: Accessible\u{00a0}chart\n",
                "\taccDescr\u{00a0}\n{\n",
                "  First\u{00a0}line %% is payload\n",
                " \t\n  Second\t\tline\n}\n",
                "\t\"First\u{00a0}\nSecond\" \t:\t 1 \t%% trailing comment\n",
            )
            .replace('\n', newline);
            let model = parse_semantic_model(&source).unwrap();

            assert!(model.show_data);
            assert_eq!(model.title.as_deref(), Some("Summary\u{00a0}label"));
            assert_eq!(model.acc_title.as_deref(), Some("Accessible\u{00a0}chart"));
            assert_eq!(
                model.acc_descr.as_deref(),
                Some("First\u{00a0}line %% is payload\nSecond line")
            );
            assert_eq!(model.sections.len(), 1);
            assert_eq!(
                model.sections[0].label,
                format!("First\u{00a0}{newline}Second")
            );
            assert_eq!(model.sections[0].value, 1.0);
        }
    }

    #[test]
    fn pie_common_tokens_do_not_consume_following_line_payloads() {
        let source = concat!(
            "pie\ntitle\n\"After title\": 1\n",
            "accTitle:\n\"After accessibility title\": 2\n",
            "accDescr:\n\"After accessibility description\": 3\n",
        );
        let model = parse_semantic_model(source).unwrap();

        assert!(model.title.is_none());
        assert!(model.acc_title.is_none());
        assert!(model.acc_descr.is_none());
        assert_eq!(
            model
                .sections
                .iter()
                .map(|section| (section.label.as_str(), section.value))
                .collect::<Vec<_>>(),
            [
                ("After title", 1.0),
                ("After accessibility title", 2.0),
                ("After accessibility description", 3.0),
            ]
        );
    }

    #[test]
    fn pie_blank_common_values_replace_prior_assignments() {
        for block in ["{}", "{ }", "{\n \t\n}"] {
            let source = format!(
                "pie\ntitle Previous\naccTitle: Previous\naccDescr: Previous\ntitle\naccTitle: \t%% blank\naccDescr {block}\n\"A\": 1\n"
            );
            let model = parse_semantic_model(&source).unwrap();

            assert!(model.title.is_none());
            assert!(model.acc_title.is_none());
            assert!(model.acc_descr.is_none());
            assert_eq!(model.sections.len(), 1);
        }
    }

    #[test]
    fn pie_common_tokens_reject_cross_line_colons_and_unclosed_blocks() {
        for source in [
            "pie\ntitle\ncontinued\n",
            "pie\naccTitle\n: continued\n",
            "pie\naccDescr\n: continued\n",
            "pie\naccDescr { text } \"A\": 1\n",
            "pie\naccDescr {\n  incomplete",
        ] {
            assert!(parse_semantic_model(source).is_err(), "accepted {source:?}");
        }
    }

    #[cfg(feature = "editor-facts")]
    #[test]
    fn pie_structural_whitespace_recovery_preserves_original_offsets() {
        let source = "pie\r\n  \"α\": 1\r\n\u{00a0}\"invalid\": 2\r\n\t\"Later 雪\": 3\r\n";
        crate::diagrams::langium_common::reset_family_syntax_construction_count("pie");
        let Err(failure) = construct_pie_semantic_source_controlled(
            source,
            "pie",
            EditorFactsDemand::Capture,
            &crate::OperationControl::new(),
        )
        .unwrap() else {
            panic!("non-horizontal structural whitespace must fail");
        };
        let (_error, facts) = failure.into_parts();
        let invalid_start = source.find('\u{00a0}').unwrap();
        let invalid_end = source.find("\r\n\t\"Later").unwrap();
        let label_start = source.find("Later 雪").unwrap();
        let number_start = source.find('3').unwrap();

        assert_eq!(
            crate::diagrams::langium_common::family_syntax_construction_count("pie"),
            1
        );
        assert_eq!(
            facts.completeness,
            crate::EditorSemanticCompleteness::Recovered
        );
        assert!(facts.diagnostics.iter().any(|diagnostic| {
            diagnostic.span == Some(SourceSpan::new(invalid_start, invalid_end))
        }));
        assert_eq!(
            facts
                .symbols
                .iter()
                .find(|symbol| symbol.name == "Later 雪")
                .unwrap()
                .selection,
            SourceSpan::new(label_start, label_start + "Later 雪".len())
        );
        assert!(facts.expected_syntax.iter().any(|expected| {
            expected.kind == EditorExpectedSyntaxKind::Payload
                && expected.span == SourceSpan::new(number_start, number_start + 1)
        }));
    }

    #[test]
    fn controlled_parse_can_cancel_after_the_pie_header() {
        let control = crate::OperationControl::new();
        control.cancel_after_checkpoints(2);

        assert!(matches!(
            construct_pie_semantic_source_controlled(
                "pie\n\"A\": 1\n\"B\": 2\n",
                "pie",
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Capture,
                &control,
            ),
            Err(crate::OperationCancelled { .. })
        ));
    }

    #[test]
    fn pie_semantic_model_reuses_one_construction_without_editor_or_compat_state() {
        crate::diagrams::langium_common::reset_family_syntax_construction_count("pie");
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let model = parse_pie_semantic_model_controlled(
            "pie showData\ntitle Revenue\n\"First\nSecond\": 1\n\"First\nSecond\": 2\n\"B\": 3\n",
            "pie",
            &MermaidConfig::default(),
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &crate::OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap();

        assert_eq!(
            serde_json::to_value(model).unwrap(),
            json!({
                "showData": true,
                "title": "Revenue",
                "accTitle": null,
                "accDescr": null,
                "sections": [{"label": "First\nSecond", "value": 1.0}, {"label": "B", "value": 3.0}],
            })
        );
        assert_eq!(
            crate::diagrams::langium_common::family_syntax_construction_count("pie"),
            1
        );
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
    }

    #[test]
    fn pie_semantic_model_applies_common_text_policy_without_theme_resolution() {
        let config = MermaidConfig::from_value(json!({
            "securityLevel": "strict",
            "themeVariables": {"primaryColor": "not-a-color"},
        }));
        let source = "pie\ntitle <script>bad()</script>Revenue\naccTitle: <script>bad()</script>Accessible\naccDescr: <script>bad()</script>Description\n\"A\": 1\n";
        let model = parse_pie_semantic_model_controlled(
            source,
            "pie",
            &config,
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &crate::OperationControl::new(),
        )
        .unwrap()
        .model
        .expect("semantic parse does not evaluate theme colors");
        assert_eq!(
            (
                model.title.as_deref(),
                model.acc_title.as_deref(),
                model.acc_descr.as_deref()
            ),
            (Some("Revenue"), Some("Accessible"), Some("Description"))
        );
        #[cfg(feature = "presentation")]
        {
            let (mut expected, _output_data) =
                parse_pie_model_for_render(source, &metadata()).unwrap();
            sanitize_common_db_fields(&mut expected, &config);
            assert_eq!(
                serde_json::to_value(model).unwrap(),
                serde_json::to_value(expected).unwrap()
            );
        }
    }

    #[test]
    fn pie_semantic_empty_input_has_no_compatibility_sentinel() {
        let semantic = parse_pie_semantic_model_controlled(
            "",
            "pie",
            &MermaidConfig::default(),
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &crate::OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap();

        assert_eq!(
            serde_json::to_value(semantic).unwrap(),
            json!({
                "showData": false,
                "title": null,
                "accTitle": null,
                "accDescr": null,
                "sections": [],
            })
        );
        #[cfg(feature = "compat-json")]
        assert_eq!(parse_pie("", &metadata()).unwrap(), json!({}));
    }

    #[cfg(feature = "compat-json")]
    #[test]
    fn pie_typed_projection_matches_complete_compatibility_json() {
        let text = "pie showData\n\"A\": 1\n\"B\": 2\n";
        let meta = metadata();
        let compat = parse_pie(text, &meta).unwrap();
        let (typed, _output_data) = parse_pie_model_for_render(text, &meta).unwrap();

        assert_eq!(
            render_model_to_compat_json(&typed, _output_data, &meta).unwrap(),
            compat
        );
        assert_eq!(compat["type"], "pie");
        assert!(compat["title"].is_null());
    }

    #[cfg(feature = "compat-json")]
    #[test]
    fn pie_typed_projection_preserves_empty_and_header_only_output_states() {
        let meta = metadata();
        for source in ["", "pie"] {
            let compat = parse_pie(source, &meta).unwrap();
            let (typed, _output_data) = parse_pie_model_for_render(source, &meta).unwrap();

            assert_eq!(
                render_model_to_compat_json(&typed, _output_data, &meta).unwrap(),
                compat,
                "projection drift for {source:?}"
            );
        }
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[test]
    fn pie_invalid_header_is_a_failure_with_recovered_editor_facts() {
        let meta = metadata();
        for source in ["piep", "  piep\n\"A\": 1\n", "packet"] {
            let start = source.find(|ch: char| !ch.is_whitespace()).unwrap();
            let token = source[start..].split_whitespace().next().unwrap();
            let span = SourceSpan::new(start, start + token.len());
            let semantic = parse_pie(source, &meta).expect_err("invalid pie header");
            let typed = parse_pie_model_for_render(source, &meta).expect_err("invalid pie header");
            for error in [semantic, typed] {
                let Error::DiagramParse { diagnostic, .. } = error else {
                    panic!("expected diagram parse failure");
                };
                assert_eq!(diagnostic.message(), "expected pie");
                assert_eq!(diagnostic.span(), Some(span));
            }
            let (model, facts) =
                parse_pie_json_and_editor_facts(source, &meta, &crate::OperationControl::new())
                    .unwrap()
                    .into_parts();
            assert!(model.is_err());
            assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
            assert!(facts.diagnostics.iter().any(|diagnostic| {
                diagnostic.kind == crate::EditorSemanticDiagnosticKind::ParserRecovery
                    && diagnostic.span == Some(span)
            }));
        }
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[test]
    fn pie_supports_title_statement_after_header() {
        let engine = MermaidEngine::new();
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

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[test]
    fn pie_uses_langium_string_escapes_and_quote_aware_inline_comments() {
        let engine = MermaidEngine::new();
        let parsed = engine
            .parse_diagram_sync(
                r#"pie
"A\n100%% complete": 1 %% outside comment
'B\tlabel': 2
"#,
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("pie parses Langium strings");

        assert_eq!(parsed.model["sections"][0]["label"], "A\n100%% complete");
        assert_eq!(parsed.model["sections"][1]["label"], "B\tlabel");
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[test]
    fn pie_number_terminal_matches_number_pie_exactly() {
        let engine = MermaidEngine::new();
        for value in [".5", "01", "1.", "1junk", "1..2"] {
            let source = format!("pie\n\"A\": {value}\n");
            assert!(
                engine
                    .parse_diagram_sync(&source, ParseOptions::strict())
                    .is_err(),
                "NUMBER_PIE must reject {value:?}"
            );
        }

        let parsed = engine
            .parse_diagram_sync(
                "pie\n\"zero\": 0\n\"integer\": -1\n\"decimal\": 01.5\n",
                ParseOptions::strict(),
            )
            .expect_err("negative values are rejected only after syntax succeeds");
        assert!(parsed.to_string().contains("invalid value: -1"));
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[test]
    fn pie_editor_recovery_reports_negative_value_validation_errors() {
        let source = "pie\r\n  \"negative\": -1  \r\n\"valid\": 2\r\n";
        let invalid = "-1";
        let start = source.find(invalid).unwrap();
        let expected_span = SourceSpan::new(start, start + invalid.len());
        let engine = MermaidEngine::new();

        crate::diagrams::langium_common::reset_family_syntax_construction_count("pie");
        crate::editor::test_work::reset();
        let error = engine
            .parse_diagram_sync(source, ParseOptions::strict())
            .expect_err("a negative pie value must fail strict parsing");
        let Error::DiagramParse { diagnostic, .. } = error else {
            panic!("expected pie parse diagnostic");
        };
        assert_eq!(diagnostic.span(), Some(expected_span));
        assert_eq!(
            crate::diagrams::langium_common::family_syntax_construction_count("pie"),
            1
        );
        assert_eq!(
            crate::editor::test_work::read(),
            crate::editor::test_work::Counts::default()
        );

        for (known_type, typed) in [(true, false), (false, true), (true, true)] {
            crate::diagrams::langium_common::reset_family_syntax_construction_count("pie");
            crate::editor::test_work::reset();
            let options = ParseOptions::strict();
            let error = match (known_type, typed) {
                (true, false) => engine
                    .parse_diagram_with_type_sync("pie", source, options)
                    .map(|_| ()),
                (false, true) => engine
                    .parse_diagram_for_render_model_sync(source, options)
                    .map(|_| ()),
                (true, true) => engine
                    .parse_diagram_for_render_model_with_type_sync("pie", source, options)
                    .map(|_| ()),
                (false, false) => unreachable!(),
            }
            .expect_err("semantic-only parsing must still validate negative pie values");
            let Error::DiagramParse { diagnostic, .. } = error else {
                panic!("expected pie parse diagnostic");
            };
            assert_eq!(diagnostic.span(), Some(expected_span));
            assert_eq!(
                crate::diagrams::langium_common::family_syntax_construction_count("pie"),
                1
            );
            assert_eq!(
                crate::editor::test_work::read(),
                crate::editor::test_work::Counts::default(),
                "negative pie: known={known_type}, typed={typed} discarded editor facts"
            );
        }

        crate::diagrams::langium_common::reset_family_syntax_construction_count("pie");
        crate::editor::test_work::reset();
        let facts = crate::family::test_support::editor_facts(
            parse_pie_json_and_editor_facts,
            source,
            &metadata(),
        );
        assert_eq!(
            crate::diagrams::langium_common::family_syntax_construction_count("pie"),
            1
        );
        let work = crate::editor::test_work::read();
        assert_eq!(work.collections, 1);
        assert_eq!(work.symbols, facts.symbols.len());
        assert_eq!(work.diagnostics, facts.diagnostics.len());
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
        assert!(facts.symbols.iter().any(|symbol| symbol.name == "negative"));
        assert!(facts.symbols.iter().any(|symbol| symbol.name == "valid"));
        assert!(facts.diagnostics.iter().any(|diagnostic| {
            diagnostic.kind == crate::EditorSemanticDiagnosticKind::ParserRecovery
                && diagnostic.span == Some(expected_span)
                && diagnostic
                    .message
                    .contains("Negative values are not allowed")
        }));
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[test]
    fn pie_reports_later_syntax_errors_before_populate_errors() {
        let engine = MermaidEngine::new();
        let error = engine
            .parse_diagram_sync(
                "pie\n\"negative\": -1\n\"malformed\": 1junk\n",
                ParseOptions::strict(),
            )
            .expect_err("the malformed NUMBER_PIE must fail parsing");
        let message = error.to_string();

        assert!(message.contains("unexpected pie statement"), "{message}");
        assert!(!message.contains("invalid value: -1"), "{message}");
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[test]
    fn pie_rejects_show_data_after_the_header_line() {
        let source = "pie\nshowData\n\"A\": 1\n";
        let engine = MermaidEngine::new();

        let error = engine
            .parse_diagram_sync(source, ParseOptions::strict())
            .expect_err("showData is only valid in the pie header");
        assert!(
            error
                .to_string()
                .contains("unexpected pie statement: showData")
        );

        let facts = crate::family::test_support::editor_facts(
            parse_pie_json_and_editor_facts,
            source,
            &metadata(),
        );
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

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[test]
    fn pie_supports_header_acc_title_inline() {
        let engine = MermaidEngine::new();
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

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[test]
    fn pie_supports_header_acc_descr_block() {
        let engine = MermaidEngine::new();
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

#[cfg(all(test, feature = "presentation"))]
mod output_data_tests {
    use super::*;

    #[test]
    fn full_producer_preserves_empty_output_independently_of_json_projection() {
        let meta = crate::SourcePresentationMetadata {
            diagram_type: "pie".to_string(),
            config: crate::MermaidConfig::empty_object(),
            effective_config: crate::presentation::SourcePresentationConfig::empty_object(),
            title: None,
        };
        let parse = |source| {
            parse_pie_model_for_render_controlled(source, &meta, &crate::OperationControl::new())
                .unwrap()
                .unwrap()
        };
        let (empty, empty_output) = parse("");
        let (header, header_output) = parse("pie");
        assert_eq!(empty_output, PieOutputData::Empty);
        assert_eq!(header_output, PieOutputData::Model);
        assert_eq!(PieOutputData::default(), PieOutputData::Model);
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
                    &PieDiagramModel::default(),
                    PieOutputData::default(),
                    &meta
                )
                .unwrap()["type"],
                "pie"
            );
        }
    }
}
