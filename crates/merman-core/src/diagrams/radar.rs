//! Core projection of Radar source and editor observations.

#[cfg(any(feature = "editor-facts", test))]
use crate::SourceSpan;
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
    EditorSemanticRole, EditorSemanticSymbol, editor::ensure_editor_recovery_from_error,
};
use crate::{Error, family};
#[cfg(feature = "presentation")]
use crate::{Result, SourcePresentationMetadata};
use merman_core_structured::radar::{RadarBodyIssue, RadarHeader, radar_body_start_controlled};
#[cfg(feature = "editor-facts")]
use merman_core_structured::radar::{RadarFieldKind, RadarFieldValue, RadarObservation};
use merman_model::families::radar::RadarDiagramModel;
#[cfg(feature = "presentation")]
use merman_model::families::radar::RadarOutputData;
#[cfg(feature = "compat-json")]
use serde_json::Value;
#[cfg(feature = "editor-facts")]
use std::borrow::Cow;

pub(crate) fn sanitize_common_db_fields(
    model: &mut RadarDiagramModel,
    config: &crate::MermaidConfig,
) {
    crate::common_db::sanitize_optional_title(&mut model.title, config);
    crate::common_db::sanitize_optional_acc_title(&mut model.acc_title, config);
    crate::common_db::sanitize_optional_acc_descr(&mut model.acc_descr, config);
}

struct RadarSemanticSource {
    model: Option<RadarDiagramModel>,
    #[cfg(feature = "editor-facts")]
    editor_facts: Option<EditorSemanticFacts>,
}

#[cfg(feature = "presentation")]
impl RadarSemanticSource {
    fn into_render_parts(self) -> (RadarDiagramModel, RadarOutputData) {
        match self.model {
            Some(model) => (model, RadarOutputData::Model),
            None => (RadarDiagramModel::default(), RadarOutputData::Empty),
        }
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    fn into_compat_json(self, meta: &SourcePresentationMetadata) -> Result<Value> {
        let (model, output_data) = self.into_render_parts();
        render_model_to_compat_json(&model, output_data, meta)
    }
}

#[cfg(feature = "compat-json")]
pub(crate) fn parse_radar(code: &str, meta: &SourcePresentationMetadata) -> Result<Value> {
    let (model, output_data) = parse_radar_semantic_source(code, meta)?.into_render_parts();
    render_model_to_compat_json(&model, output_data, meta)
}

#[cfg(feature = "compat-json")]
#[cfg(feature = "editor-facts")]
pub(crate) fn parse_radar_json_and_editor_facts(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<family::CombinedSemanticParse> {
    control.checkpoint()?;
    let parsed = family::CombinedSemanticParse::from_construction(
        construct_radar_semantic_source_controlled(
            code,
            &meta.diagram_type,
            EditorFactsDemand::Capture,
            control,
        )?,
        |mut source| {
            let editor_facts = source.editor_facts.take().unwrap_or_default();
            (source.into_compat_json(meta), editor_facts)
        },
        family::CombinedSemanticFailure::into_parts,
    );
    control.checkpoint()?;
    Ok(parsed)
}

#[cfg(all(test, feature = "presentation"))]
pub(crate) fn parse_radar_model_for_render(
    code: &str,
    meta: &SourcePresentationMetadata,
) -> Result<(RadarDiagramModel, RadarOutputData)> {
    parse_radar_semantic_source(code, meta).map(RadarSemanticSource::into_render_parts)
}

#[cfg(feature = "presentation")]
pub(crate) fn parse_radar_model_for_render_controlled(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<Result<(RadarDiagramModel, RadarOutputData)>> {
    let construction = construct_radar_semantic_source_controlled(
        code,
        &meta.diagram_type,
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        control,
    )?;
    control.checkpoint()?;
    Ok(construction
        .map(RadarSemanticSource::into_render_parts)
        .map_err(family::CombinedSemanticFailure::into_error))
}

#[cfg(feature = "compat-json")]
pub(crate) fn render_model_to_compat_json(
    model: &RadarDiagramModel,
    output_data: RadarOutputData,
    meta: &SourcePresentationMetadata,
) -> Result<Value> {
    Ok(merman_model::families::radar::compat::project(
        model,
        output_data,
        &meta.diagram_type,
        meta.effective_config.as_config(),
    ))
}

#[cfg(any(feature = "compat-json", all(test, feature = "presentation")))]
fn parse_radar_semantic_source(
    code: &str,
    meta: &SourcePresentationMetadata,
) -> Result<RadarSemanticSource> {
    construct_radar_semantic_source_controlled(
        code,
        &meta.diagram_type,
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        &crate::OperationControl::new(),
    )
    .expect("a private parse control cannot be cancelled")
    .map_err(family::CombinedSemanticFailure::into_error)
}

pub(crate) fn parse_radar_semantic_model_controlled(
    code: &str,
    metadata: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<crate::family::SemanticConstruction<RadarDiagramModel>> {
    let construction = construct_radar_semantic_source_controlled(
        code,
        &metadata.diagram_type,
        #[cfg(feature = "editor-facts")]
        editor_demand,
        control,
    )?;
    Ok(match construction {
        Ok(source) => {
            #[cfg(feature = "editor-facts")]
            let editor_facts = source.editor_facts;
            let mut model = source.model.unwrap_or_default();
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

fn construct_radar_semantic_source_controlled(
    code: &str,
    diagram_type: &str,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<
    std::result::Result<RadarSemanticSource, family::CombinedSemanticFailure>,
> {
    control.checkpoint()?;
    #[cfg(test)]
    crate::diagrams::langium_common::record_family_syntax_construction("radar");

    let body = match radar_body_start_controlled(code, control)? {
        RadarHeader::Empty => {
            return Ok(Ok(RadarSemanticSource {
                model: None,
                #[cfg(feature = "editor-facts")]
                editor_facts: editor_demand
                    .needs_recovery()
                    .then(EditorSemanticFacts::new),
            }));
        }
        RadarHeader::Invalid => {
            return Ok(Err(radar_parse_failure(
                Error::diagram_parse_fallback("radar", "expected radar-beta"),
                #[cfg(feature = "editor-facts")]
                editor_demand
                    .needs_recovery()
                    .then(EditorSemanticFacts::new),
                #[cfg(feature = "editor-facts")]
                code,
            )));
        }
        RadarHeader::Body(body) => body,
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
        move |observation: RadarObservation<'_>| project_radar_observation(facts, observation, code)
    });
    #[cfg(feature = "editor-facts")]
    let parsed = body.parse_with_observer_controlled(
        editor_demand.captures_rich(),
        observe
            .as_mut()
            .map(|observe| observe as &mut dyn FnMut(RadarObservation<'_>)),
        control,
    )?;
    #[cfg(not(feature = "editor-facts"))]
    let parsed = body.parse_controlled(control)?;
    Ok(match parsed {
        Ok(model) => Ok(RadarSemanticSource {
            model: Some(model),
            #[cfg(feature = "editor-facts")]
            editor_facts,
        }),
        Err(issue) => Err(family::CombinedSemanticFailure::new(
            radar_body_error(diagram_type, issue),
            #[cfg(feature = "editor-facts")]
            editor_facts,
        )),
    })
}

fn radar_body_error(diagram_type: &str, issue: RadarBodyIssue) -> Error {
    match issue {
        RadarBodyIssue::Common(diagnostic) => Error::diagram_parse_insertion_point(
            diagram_type,
            diagnostic.message,
            diagnostic.span.start,
        ),
        RadarBodyIssue::Statement { message, .. } => {
            Error::diagram_parse_fallback("radar", message)
        }
        RadarBodyIssue::Curve { message, span } => {
            Error::diagram_parse_exact("radar", message, span)
        }
    }
}

fn radar_parse_failure(
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
fn project_radar_observation(
    facts: &mut EditorSemanticFacts,
    observation: RadarObservation<'_>,
    code: &str,
) {
    match observation {
        RadarObservation::Common(fact) => push_langium_common_editor_fact(facts, fact, "radar"),
        RadarObservation::Issue(issue) => match issue {
            RadarBodyIssue::Common(diagnostic) => push_langium_common_recovery(facts, diagnostic),
            RadarBodyIssue::Statement { message, span } => {
                facts.mark_recovered_from_parse_error(message.as_str(), Some(*span));
            }
            RadarBodyIssue::Curve { message, span } => {
                let error = Error::diagram_parse_exact("radar", message.as_str(), *span);
                *facts = ensure_editor_recovery_from_error(
                    std::mem::take(facts),
                    &error,
                    recovery_fallback_span(code),
                );
            }
        },
        RadarObservation::Field { kind, value, span } => {
            use EditorSemanticRole::{Entity, Payload, Reference};

            let (detail, role) = match kind {
                RadarFieldKind::Axis => ("radar axis", Entity),
                RadarFieldKind::AxisLabel => ("radar axis label", Payload),
                RadarFieldKind::Curve => ("radar curve", Entity),
                RadarFieldKind::CurveLabel => ("radar curve label", Payload),
                RadarFieldKind::AxisReference => ("radar curve axis reference", Reference),
                RadarFieldKind::Entry => ("radar curve entry", Payload),
                RadarFieldKind::Option => ("radar option", Payload),
            };
            let text = match value {
                RadarFieldValue::Text(text) => Cow::Borrowed(text),
                RadarFieldValue::Number(value) => Cow::Owned(crate::utils::format_js_number(value)),
                RadarFieldValue::Bool(value) => Cow::Owned(value.to_string()),
            };
            push_radar_field(facts, text, span, detail, role);
        }
    }
}

#[cfg(feature = "editor-facts")]
fn push_radar_field(
    facts: &mut EditorSemanticFacts,
    text: Cow<'_, str>,
    span: SourceSpan,
    detail: &str,
    role: EditorSemanticRole,
) {
    if text.is_empty() {
        return;
    }
    let (expected, kind) = if role == EditorSemanticRole::Payload {
        (
            EditorExpectedSyntaxKind::Payload,
            EditorSemanticKind::String,
        )
    } else {
        (
            EditorExpectedSyntaxKind::NodeIdentifier,
            EditorSemanticKind::Variable,
        )
    };
    facts.push_expected_syntax(EditorExpectedSyntax::new(expected, span));
    facts.push_symbol(EditorSemanticSymbol::with_role(
        text,
        Some(detail.to_owned()),
        kind,
        role,
        span,
        span,
    ));
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
    fn radar_parser_can_cancel_inside_an_axis_list() {
        let axes = (0..512)
            .map(|index| format!("axis_{index}"))
            .collect::<Vec<_>>()
            .join(",");
        let text = format!("radar-beta\naxis {axes}\n");
        let control = crate::OperationControl::new();
        control.cancel_after_checkpoints(20);
        let meta = SourcePresentationMetadata {
            diagram_type: "radar".to_string(),
            config: crate::MermaidConfig::empty_object(),
            effective_config: crate::presentation::SourcePresentationConfig::empty_object(),
            title: None,
        };

        assert!(matches!(
            construct_radar_semantic_source_controlled(
                &text,
                &meta.diagram_type,
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &control,
            ),
            Err(crate::OperationCancelled { .. })
        ));
    }

    #[test]
    fn radar_parses_simple_definition() {
        let _ = parse(
            r#"radar-beta
axis A,B,C
curve mycurve{1,2,3}"#,
        );
    }

    #[test]
    fn radar_errors_on_empty_axis_statement() {
        assert_eq!(
            parse_err(
                r#"radar-beta
axis"#,
            ),
            "axis statement must include at least one axis"
        );
    }

    #[test]
    fn radar_parses_title_and_data() {
        let model = parse(
            r#"radar-beta
title Radar diagram
accTitle: Radar accTitle
accDescr: Radar accDescription
axis A["Axis A"], B["Axis B"] ,C["Axis C"]
curve mycurve["My Curve"]{1,2,3}
"#,
        );
        assert_eq!(model["title"], json!("Radar diagram"));
        assert_eq!(model["accTitle"], json!("Radar accTitle"));
        assert_eq!(model["accDescr"], json!("Radar accDescription"));
        assert_eq!(
            model["axes"],
            json!([
                {"name": "A", "label": "Axis A"},
                {"name": "B", "label": "Axis B"},
                {"name": "C", "label": "Axis C"},
            ])
        );
        assert_eq!(
            model["curves"],
            json!([
                {"name": "mycurve", "label": "My Curve", "entries": [1,2,3]},
            ])
        );
        assert_eq!(
            model["options"],
            json!({"showLegend": true, "ticks": 5, "max": Value::Null, "min": 0, "graticule": "circle"})
        );
    }

    #[test]
    fn radar_uses_langium_string_escapes_and_quote_aware_inline_comments() {
        let model = parse(
            r#"radar-beta
axis A["A\n100%% complete"], B['B\tlabel'] %% outside comment
curve sample{1,2}
"#,
        );

        assert_eq!(model["axes"][0]["label"], "A\n100%% complete");
        assert_eq!(model["axes"][1]["label"], "B\tlabel");
    }

    #[test]
    fn radar_parses_options() {
        let model = parse(
            r#"radar-beta
ticks 10
showLegend false
graticule polygon
min 1
max 10
"#,
        );
        assert_eq!(
            model["options"],
            json!({"showLegend": false, "ticks": 10, "max": 10, "min": 1, "graticule": "polygon"})
        );
    }

    #[test]
    fn radar_clamps_ticks_to_the_mermaid_limit() {
        for (ticks, expected) in [(12, 12), (32, 32), (33, 32)] {
            let model = parse(&format!("radar-beta\nticks {ticks}\n"));
            assert_eq!(model["options"]["ticks"], json!(expected));
        }
    }

    #[test]
    fn radar_parses_comma_separated_option_list_like_langium_grammar() {
        let model = parse(
            r#"radar-beta
ticks 5, max 10, min 1, graticule polygon, showLegend false
"#,
        );
        assert_eq!(
            model["options"],
            json!({"showLegend": false, "ticks": 5, "max": 10, "min": 1, "graticule": "polygon"})
        );
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn radar_editor_facts_preserve_every_repeated_occurrence_span() {
        let text = concat!(
            "radar-beta\r\n",
            "axis A[\"dup\"], A[\"dup\"]\r\n",
            "curve C[\"dup\"]{ A: 1,\r\n",
            "  A: 1 }, C[\"dup\"]{1,1}\r\n",
            "ticks 5\r\n",
            "ticks 5\r\n",
        );
        let facts = MermaidEngine::new()
            .parse_editor_semantic_facts_with_type_sync("radar", text)
            .unwrap()
            .unwrap();

        let occurrences = |needle: &str| {
            text.match_indices(needle)
                .map(|(start, value)| SourceSpan::new(start, start + value.len()))
                .collect::<Vec<_>>()
        };
        let spans_for = |detail: &str| {
            facts
                .symbols
                .iter()
                .filter(|symbol| symbol.detail.as_deref() == Some(detail))
                .map(|symbol| symbol.span)
                .collect::<Vec<_>>()
        };

        let axis_occurrences = occurrences("A");
        assert_eq!(spans_for("radar axis"), axis_occurrences[..2]);
        assert_eq!(
            spans_for("radar curve axis reference"),
            axis_occurrences[2..]
        );
        let axis_roles = facts
            .symbols
            .iter()
            .filter(|symbol| symbol.name == "A")
            .map(|symbol| symbol.role)
            .collect::<Vec<_>>();
        assert_eq!(
            axis_roles,
            vec![
                EditorSemanticRole::Entity,
                EditorSemanticRole::Entity,
                EditorSemanticRole::Reference,
                EditorSemanticRole::Reference,
            ]
        );

        let label_occurrences = occurrences("dup");
        assert_eq!(spans_for("radar axis label"), label_occurrences[..2]);
        assert_eq!(spans_for("radar curve label"), label_occurrences[2..]);
        assert_eq!(spans_for("radar curve"), occurrences("C"));
        assert_eq!(spans_for("radar curve entry"), occurrences("1"));
        assert_eq!(spans_for("radar option"), occurrences("5"));
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn radar_forward_axis_occurrence_is_a_reference_before_its_definition() {
        let text = concat!("radar-beta\n", "curve Forward{Later: 1}\n", "axis Later\n",);
        let facts = MermaidEngine::new()
            .parse_editor_semantic_facts_with_type_sync("radar", text)
            .unwrap()
            .unwrap();
        let later = facts
            .symbols
            .iter()
            .filter(|symbol| symbol.name == "Later")
            .collect::<Vec<_>>();

        assert_eq!(later.len(), 2);
        assert_eq!(later[0].role, EditorSemanticRole::Reference);
        assert_eq!(later[1].role, EditorSemanticRole::Entity);
        assert_eq!(later[0].kind, later[1].kind);
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn radar_entity_occurrence_role_does_not_depend_on_display_detail() {
        let span = SourceSpan::new(0, "axis".len());
        let mut facts = EditorSemanticFacts::new();
        push_radar_field(
            &mut facts,
            Cow::Borrowed("axis"),
            span,
            "reference-looking definition",
            EditorSemanticRole::Entity,
        );
        push_radar_field(
            &mut facts,
            Cow::Borrowed("axis"),
            span,
            "definition-looking reference",
            EditorSemanticRole::Reference,
        );

        assert_eq!(facts.symbols[0].role, EditorSemanticRole::Entity);
        assert_eq!(facts.symbols[1].role, EditorSemanticRole::Reference);
    }

    #[test]
    fn radar_errors_on_empty_curve_stmt() {
        let err = parse_err(
            r#"radar-beta
axis my-axis
curve
"#,
        );
        assert_eq!(err, "expected curve id");
    }

    #[test]
    fn radar_rejects_empty_curve_and_entry_list_elements_like_langium_grammar() {
        for (body, expected) in [
            ("curve C{}", "expected curve entry"),
            ("curve C{1,}", "expected curve entry"),
            ("curve C{,1}", "expected curve entry"),
            ("curve C{1},", "expected curve after ','"),
        ] {
            let err = parse_err(&format!("radar-beta\n{body}\n"));
            assert_eq!(err, expected, "unexpected result for {body:?}");
        }
    }

    #[test]
    fn radar_number_and_id_terminals_match_common_langium() {
        for body in ["ticks .5", "curve C{.5}"] {
            let _ = parse_err(&format!("radar-beta\n{body}\n"));
        }

        let model = parse("radar-beta\naxis 1A, 2B\ncurve 3C{1A: 1, 2B: 2}\n");
        assert_eq!(model["axes"][0]["name"], "1A");
        assert_eq!(model["axes"][1]["name"], "2B");
        assert_eq!(model["curves"][0]["name"], "3C");
        assert_eq!(model["curves"][0]["entries"], json!([1, 2]));
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn radar_recovery_reports_parser_diagnostic_with_exact_crlf_span() {
        let text = concat!(
            "radar-beta\r\n",
            "axis A\r\n",
            "  curve C{.5} %% hidden\r\n",
            "axis B\r\n",
        );
        let facts = MermaidEngine::new()
            .parse_editor_semantic_facts_with_type_sync("radar", text)
            .unwrap()
            .unwrap();
        let invalid = "curve C{.5}";
        let start = text.find(invalid).unwrap();

        assert!(facts.symbols.iter().any(|symbol| symbol.name == "B"));
        assert!(facts.diagnostics.iter().any(|diagnostic| {
            diagnostic.kind == crate::EditorSemanticDiagnosticKind::ParserRecovery
                && diagnostic.span == Some(SourceSpan::new(start, start + invalid.len()))
                && diagnostic.message.contains("expected entry number")
        }));
    }

    #[test]
    fn radar_editor_recovery_reports_curve_validation_errors() {
        let text = concat!(
            "radar-beta\r\n",
            "axis A[\"Axis A\"], B[\"Axis B\"]\r\n",
            "  curve sample{A: 1}  \r\n",
        );
        let invalid = "sample{A: 1}";
        let start = text.find(invalid).unwrap();
        let expected_span = SourceSpan::new(start, start + invalid.len());
        let engine = MermaidEngine::new();

        let error = engine
            .parse_diagram_sync(text, ParseOptions::strict())
            .expect_err("a detailed curve missing an axis must fail strict parsing");
        let Error::DiagramParse { diagnostic, .. } = error else {
            panic!("expected radar parse diagnostic");
        };
        assert_eq!(diagnostic.span(), Some(expected_span));

        #[cfg(feature = "editor-facts")]
        {
            let facts = engine
                .parse_editor_semantic_facts_with_type_sync("radar", text)
                .unwrap()
                .expect("radar editor recovery facts");
            assert_eq!(
                facts.completeness,
                crate::EditorSemanticCompleteness::Recovered
            );
            assert!(facts.symbols.iter().any(|symbol| symbol.name == "sample"));
            assert!(facts.diagnostics.iter().any(|diagnostic| {
                diagnostic.kind == crate::EditorSemanticDiagnosticKind::ParserRecovery
                    && diagnostic.span == Some(expected_span)
                    && diagnostic.message.contains("Missing entry for axis Axis B")
            }));
        }
    }

    #[test]
    fn radar_errors_on_mixed_numeric_and_detailed_curve_entries() {
        let err = parse_err(
            r#"radar-beta
axis ax1, ax2
curve my-curve { 1, ax1 2 }"#,
        );
        assert_eq!(err, "mixed detailed and numeric entries are not supported");
    }

    #[test]
    fn radar_orders_detailed_curve_entries_by_axes() {
        let model = parse(
            r#"radar-beta
axis A,B,C
curve mycurve{ C: 3, A: 1, B: 2 }"#,
        );
        assert_eq!(
            model["curves"],
            json!([
                {"name": "mycurve", "label": "mycurve", "entries": [1,2,3]},
            ])
        );
    }

    #[test]
    fn radar_accepts_header_with_colon() {
        let _ = parse(
            r#"radar-beta:
axis A,B,C
curve mycurve{1,2,3}"#,
        );
    }

    #[test]
    fn radar_ignores_comment_lines() {
        let _ = parse(
            r#"radar-beta
%% This is a comment
axis A,B,C
%% This is another comment
curve mycurve{1,2,3}
"#,
        );
    }

    #[test]
    fn radar_errors_on_missing_axis_entry() {
        let err = parse_err(
            r#"radar-beta
axis A["Axis A"], B["Axis B"], C["Axis C"]
curve mycurve{ C: 3, A: 1 }"#,
        );
        assert_eq!(err, "Missing entry for axis Axis B");
    }

    #[test]
    fn radar_parses_config_override_directive() {
        let model = parse(
            r#"
%%{init: {'radar': {'marginTop': 80, 'axisLabelFactor': 1.25}}}%%
radar-beta
axis A,B,C
curve mycurve{1,2,3}
"#,
        );
        assert_eq!(model["config"]["radar"]["marginTop"], json!(80));
    }

    #[test]
    fn radar_typed_projection_matches_complete_and_empty_compat_json() {
        let text = concat!(
            "radar-beta\n",
            "title Delivery risk\n",
            "axis Cost,Time\n",
            "curve Current{2,3}\n",
        );
        let engine = MermaidEngine::new();
        let parsed = engine
            .parse_diagram_sync(text, ParseOptions::strict())
            .unwrap()
            .unwrap();
        let (typed, _output_data) = parse_radar_model_for_render(
            text,
            &crate::diagrams::source_metadata_for_test(&parsed.meta),
        )
        .unwrap();
        let projection = merman_model::families::radar::compat::project(
            &typed,
            _output_data,
            &parsed.meta.diagram_type,
            parsed.meta.effective_config.as_config(),
        );

        assert_eq!(projection, parsed.model);
        assert_eq!(projection["type"], json!("radar"));
        assert!(projection["config"].is_object());
        assert_eq!(projection["accTitle"], Value::Null);
        assert_eq!(projection["accDescr"], Value::Null);

        let empty_meta = SourcePresentationMetadata {
            diagram_type: "radar".to_string(),
            config: crate::MermaidConfig::empty_object(),
            effective_config: crate::presentation::SourcePresentationConfig::empty_object(),
            title: None,
        };
        let (empty, _output_data) = parse_radar_model_for_render("", &empty_meta).unwrap();
        assert_eq!(
            render_model_to_compat_json(&empty, _output_data, &empty_meta).unwrap(),
            json!({})
        );
        assert_eq!(
            render_model_to_compat_json(
                &RadarDiagramModel::default(),
                RadarOutputData::default(),
                &empty_meta
            )
            .unwrap()["type"],
            json!("radar")
        );
    }
}

#[cfg(test)]
mod semantic_tests {
    use super::*;

    #[test]
    fn canonical_empty_capture_keeps_an_available_empty_journal() {
        crate::diagrams::langium_common::reset_family_syntax_construction_count("radar");
        let parsed = parse_radar_semantic_model_controlled(
            "",
            &metadata(),
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Capture,
            &crate::OperationControl::new(),
        )
        .unwrap();
        assert!(parsed.model.is_ok());
        assert_eq!(
            crate::diagrams::langium_common::family_syntax_construction_count("radar"),
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

    fn metadata() -> crate::SemanticMetadata {
        crate::SemanticMetadata {
            diagram_type: "radar".to_string(),
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
    fn semantic_axes_curves_and_options_construct_once_without_editor_work() {
        let source = concat!(
            "radar-beta\n",
            "title <b>Delivery</b>\n",
            "axis Cost[\"Cost label\"], Time\n",
            "curve Current{Time: 3, Cost: 2}\n",
            "ticks 40, min 1, max 9, graticule polygon, showLegend false\n",
        );
        crate::diagrams::langium_common::reset_family_syntax_construction_count("radar");
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let model = parse_radar_semantic_model_controlled(
            source,
            &metadata(),
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &crate::OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap();

        assert_eq!(model.title.as_deref(), Some("Delivery"));
        assert_eq!(model.axes.len(), 2);
        assert_eq!(model.axes[0].name, "Cost");
        assert_eq!(model.axes[0].label, "Cost label");
        assert_eq!(model.curves[0].name, "Current");
        assert_eq!(model.curves[0].entries, [2.0, 3.0]);
        assert_eq!(model.options.ticks, 32.0);
        assert_eq!(model.options.min, 1.0);
        assert_eq!(model.options.max, Some(9.0));
        assert_eq!(model.options.graticule, "polygon");
        assert!(!model.options.show_legend);
        assert_eq!(
            crate::diagrams::langium_common::family_syntax_construction_count("radar"),
            1
        );
        #[cfg(feature = "editor-facts")]
        assert_eq!(
            crate::editor::test_work::read(),
            crate::editor::test_work::Counts::default()
        );
    }

    #[test]
    fn semantic_empty_input_has_no_compatibility_sentinel() {
        let model = parse_radar_semantic_model_controlled(
            "",
            &metadata(),
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &crate::OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap();
        assert!(model.axes.is_empty());
        assert!(model.curves.is_empty());
        assert_eq!(model.options.ticks, 5.0);
        assert!(model.options.show_legend);
    }

    #[test]
    fn semantic_curve_validation_keeps_exact_error_span_without_editor_work() {
        let source = "radar-beta\r\naxis A,B\r\n  curve sample{A: 1}\r\n";
        let start = source.find("sample").unwrap();
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let error = parse_radar_semantic_model_controlled(
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
            panic!("expected a radar parse error");
        };
        assert!(diagnostic.message().contains("Missing entry for axis B"));
        assert_eq!(
            diagnostic.span(),
            Some(SourceSpan::new(start, start + "sample{A: 1}".len()))
        );
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

    #[cfg(feature = "editor-facts")]
    #[test]
    fn radar_statement_error_precedes_curve_validation_for_every_editor_demand() {
        let source = concat!(
            "radar-beta\r\n",
            "axis A[\"É\"], B[\"Beta\"]\r\n",
            "curve Missing{A:1}\r\n",
            "  ticks nope %% trailing\r\n",
            "axis Later\r\n",
        );
        let span_of = |text: &str| {
            let start = source.find(text).unwrap();
            SourceSpan::new(start, start + text.len())
        };
        for demand in [
            EditorFactsDemand::Omit,
            EditorFactsDemand::Recovery,
            EditorFactsDemand::Capture,
        ] {
            let construction = parse_radar_semantic_model_controlled(
                source,
                &metadata(),
                demand,
                &crate::OperationControl::new(),
            )
            .unwrap();
            let Error::DiagramParse { diagnostic, .. } = construction.model.unwrap_err() else {
                panic!("expected a Radar parse error");
            };
            assert_eq!(diagnostic.message(), "expected number");
            assert_eq!(diagnostic.span(), None);
            assert_eq!(
                diagnostic.span_kind(),
                crate::ParseDiagnosticSpanKind::Fallback
            );

            assert_eq!(construction.editor_facts.is_some(), demand.needs_recovery());
            if let Some(facts) = construction.editor_facts {
                assert_eq!(
                    facts.completeness,
                    crate::EditorSemanticCompleteness::Recovered
                );
                let diagnostics: Vec<_> = facts
                    .diagnostics
                    .iter()
                    .map(|diagnostic| (diagnostic.message.as_str(), diagnostic.span))
                    .collect();
                assert_eq!(
                    diagnostics,
                    [
                        ("expected number", Some(span_of("ticks nope"))),
                        ("Missing entry for axis Beta", Some(span_of("Missing{A:1}"))),
                    ],
                );
                assert_eq!(facts.symbols.is_empty(), !demand.captures_rich());
                if demand.captures_rich() {
                    assert!(facts.symbols.iter().any(|symbol| symbol.name == "Later"));
                }
            }
        }
    }

    #[test]
    fn semantic_entry_can_cancel_inside_an_axis_list() {
        let axes = (0..512)
            .map(|index| format!("axis_{index}"))
            .collect::<Vec<_>>()
            .join(",");
        let source = format!("radar-beta\naxis {axes}\n");
        let control = crate::OperationControl::new();
        control.cancel_after_checkpoints(20);
        assert!(matches!(
            parse_radar_semantic_model_controlled(
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
        let meta = crate::SourcePresentationMetadata {
            diagram_type: "radar".to_string(),
            config: crate::MermaidConfig::empty_object(),
            effective_config: crate::presentation::SourcePresentationConfig::empty_object(),
            title: None,
        };
        let source = "radar-beta\naxis A,B\ncurve Sample{1,2}\n";
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        parse_radar(source, &meta).unwrap();
        parse_radar_model_for_render_controlled(source, &meta, &crate::OperationControl::new())
            .unwrap()
            .unwrap();
        #[cfg(feature = "editor-facts")]
        assert_eq!(
            crate::editor::test_work::read(),
            crate::editor::test_work::Counts::default(),
        );
    }
}

#[cfg(all(test, feature = "presentation"))]
mod output_data_tests {
    use super::*;

    #[test]
    fn full_producer_preserves_empty_output_independently_of_json_projection() {
        let meta = crate::SourcePresentationMetadata {
            diagram_type: "radar".to_string(),
            config: crate::MermaidConfig::empty_object(),
            effective_config: crate::presentation::SourcePresentationConfig::empty_object(),
            title: None,
        };
        let parse = |source| {
            parse_radar_model_for_render_controlled(source, &meta, &crate::OperationControl::new())
                .unwrap()
                .unwrap()
        };
        let (empty, empty_output) = parse("");
        let (header, header_output) = parse("radar-beta");
        assert_eq!(empty_output, RadarOutputData::Empty);
        assert_eq!(header_output, RadarOutputData::Model);
        assert_eq!(RadarOutputData::default(), RadarOutputData::Model);
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
                    &RadarDiagramModel::default(),
                    RadarOutputData::default(),
                    &meta
                )
                .unwrap()["type"],
                "radar"
            );
        }
    }
}
