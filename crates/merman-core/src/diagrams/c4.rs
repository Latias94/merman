//! C4 source projections and public diagnostic adaptation.

#[cfg(feature = "presentation")]
use crate::SourcePresentationMetadata;
#[cfg(any(feature = "editor-facts", all(test, feature = "compat-json")))]
use crate::SourceSpan;
#[cfg(feature = "editor-facts")]
use crate::family::EditorFactsDemand;
use crate::family::SemanticConstruction;
#[cfg(feature = "editor-facts")]
use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
    EditorSemanticSymbol,
};
use crate::{Error, MermaidConfig, OperationControl, OperationControlResult, Result};
use merman_core_graph::c4::{C4Diagnostic, C4DiagnosticLocation, parse_c4_model_controlled};
#[cfg(feature = "editor-facts")]
use merman_core_graph::c4::{
    C4EntityOccurrence, C4Observation, C4ObservedFailure, parse_c4_model_observed_controlled,
};
#[cfg(test)]
use merman_core_graph::c4::{c4_syntax_construction_count, reset_c4_syntax_construction_count};
pub(crate) use merman_model::families::c4::C4DiagramModel;
#[cfg(feature = "editor-facts")]
use merman_model::warnings::{C4_DYNAMIC_OVERWRITTEN_RELATION_WARNING_RULE_ID, DiagramWarningFact};
#[cfg(any(test, feature = "compat-json"))]
use serde_json::Value;

pub(crate) fn sanitize_common_db_fields(model: &mut C4DiagramModel, config: &crate::MermaidConfig) {
    crate::common_db::sanitize_optional_title(&mut model.title, config);
    crate::common_db::sanitize_optional_acc_title(&mut model.acc_title, config);
    crate::common_db::sanitize_optional_acc_descr(&mut model.acc_descr, config);
}

fn c4_error(diagram_type: &str, diagnostic: C4Diagnostic) -> Error {
    let diagram_type = diagnostic.diagram_type(diagram_type);
    match diagnostic.location {
        C4DiagnosticLocation::Exact(span) => {
            Error::diagram_parse_exact(diagram_type, diagnostic.message, span)
        }
        C4DiagnosticLocation::InsertionPoint(offset) => {
            Error::diagram_parse_insertion_point(diagram_type, diagnostic.message, offset)
        }
        C4DiagnosticLocation::Fallback => {
            Error::diagram_parse_fallback(diagram_type, diagnostic.message)
        }
    }
}

fn parse_c4_source_model_controlled(
    code: &str,
    diagram_type: &str,
    config: &MermaidConfig,
    control: &OperationControl,
) -> OperationControlResult<Result<C4DiagramModel>> {
    Ok(parse_c4_model_controlled(code, config, control)?
        .map_err(|diagnostic| c4_error(diagram_type, diagnostic)))
}

#[cfg(feature = "editor-facts")]
fn project_c4_observation(facts: &mut EditorSemanticFacts, observation: C4Observation<'_>) {
    match observation {
        C4Observation::CaptureStarted => {
            #[cfg(test)]
            crate::editor::test_work::record_collection();
        }
        C4Observation::DirectivePrefix(prefix) => facts.push_directive_prefix(prefix),
        C4Observation::Entity {
            text,
            span,
            detail,
            occurrence,
        } => {
            if text.is_empty() {
                facts.push_expected_syntax(EditorExpectedSyntax::new(
                    EditorExpectedSyntaxKind::NodeIdentifier,
                    span,
                ));
                return;
            }
            let detail = Some(detail.to_string());
            let kind = EditorSemanticKind::Object;
            let symbol = match occurrence {
                C4EntityOccurrence::Definition => {
                    EditorSemanticSymbol::new(text, detail, kind, span, span)
                }
                C4EntityOccurrence::Reference => {
                    EditorSemanticSymbol::reference(text, detail, kind, span, span)
                }
            };
            facts.push_symbol(symbol);
        }
        C4Observation::Payload {
            text,
            span,
            detail,
            argument_name,
        } => {
            facts.push_expected_syntax(EditorExpectedSyntax::new(
                EditorExpectedSyntaxKind::Payload,
                span,
            ));
            if !text.is_empty() {
                let detail =
                    argument_name.map_or_else(|| detail.to_string(), |name| format!("c4 {name}"));
                facts.push_symbol(EditorSemanticSymbol::payload(
                    text,
                    Some(detail),
                    EditorSemanticKind::String,
                    span,
                    span,
                ));
            }
        }
        C4Observation::DirectionValue(span) => {
            facts.push_expected_syntax(EditorExpectedSyntax::new(
                EditorExpectedSyntaxKind::CardinalDirectionValue,
                span,
            ));
        }
        C4Observation::RelationOverwritten { from, to, span } => {
            facts.warning_facts.push(
                DiagramWarningFact::new(
                    C4_DYNAMIC_OVERWRITTEN_RELATION_WARNING_RULE_ID,
                    format!(
                        "C4Dynamic relationship from {from:?} to {to:?} overwrites an earlier step."
                    ),
                )
                .with_span(span),
            );
        }
    }
}

#[cfg(feature = "editor-facts")]
fn parse_c4_with_editor_facts_controlled(
    code: &str,
    diagram_type: &str,
    config: &MermaidConfig,
    capture_rich: bool,
    control: &OperationControl,
) -> OperationControlResult<SemanticConstruction<C4DiagramModel>> {
    let mut facts = EditorSemanticFacts::new();
    let result = parse_c4_model_observed_controlled(
        code,
        config,
        capture_rich,
        &mut |observation| project_c4_observation(&mut facts, observation),
        control,
    )?;
    let model = match result {
        Ok(model) => Ok(model),
        Err(C4ObservedFailure::Model(diagnostic)) => Err(c4_error(diagram_type, diagnostic)),
        Err(C4ObservedFailure::Source { first, remaining }) => {
            let mut record_issue = |error: &Error, span: SourceSpan| {
                facts.mark_recovered_from_parse_error(
                    format!("c4 parser recovered after parse error: {error}"),
                    Some(span),
                );
            };
            let first_error = c4_error(diagram_type, first.diagnostic);
            record_issue(&first_error, first.recovery_span);
            for issue in remaining {
                record_issue(
                    &c4_error(diagram_type, issue.diagnostic),
                    issue.recovery_span,
                );
            }
            Err(first_error)
        }
    };
    Ok(SemanticConstruction {
        model,
        editor_facts: Some(facts),
    })
}

pub(crate) fn parse_c4_semantic_model_controlled(
    code: &str,
    meta: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &OperationControl,
) -> OperationControlResult<SemanticConstruction<C4DiagramModel>> {
    let config = &meta.semantic_config;
    let sanitize = |mut model| {
        sanitize_common_db_fields(&mut model, config);
        model
    };
    #[cfg(feature = "editor-facts")]
    if editor_demand.needs_recovery() {
        return Ok(parse_c4_with_editor_facts_controlled(
            code,
            &meta.diagram_type,
            config,
            editor_demand.captures_rich(),
            control,
        )?
        .map_model(sanitize));
    }
    Ok(SemanticConstruction {
        model: parse_c4_source_model_controlled(code, &meta.diagram_type, config, control)?
            .map(sanitize),
        #[cfg(feature = "editor-facts")]
        editor_facts: None,
    })
}

#[cfg(feature = "compat-json")]
pub(crate) fn parse_c4(code: &str, meta: &SourcePresentationMetadata) -> Result<Value> {
    let model = parse_c4_model_for_render_controlled(code, meta, &OperationControl::new())
        .expect("a private parse control cannot be cancelled")?;
    render_model_to_compat_json(&model, meta)
}

#[cfg(all(test, feature = "compat-json"))]
pub(crate) fn parse_c4_model_for_render(
    code: &str,
    meta: &SourcePresentationMetadata,
) -> Result<C4DiagramModel> {
    parse_c4_model_for_render_controlled(code, meta, &OperationControl::new())
        .expect("a private parse control cannot be cancelled")
}

#[cfg(feature = "presentation")]
pub(crate) fn parse_c4_model_for_render_controlled(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<Result<C4DiagramModel>> {
    parse_c4_source_model_controlled(
        code,
        &meta.diagram_type,
        meta.effective_config.as_config(),
        control,
    )
}

#[cfg(all(feature = "compat-json", feature = "editor-facts"))]
pub(crate) fn parse_c4_json_and_editor_facts(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<crate::family::CombinedSemanticParse> {
    let construction = parse_c4_with_editor_facts_controlled(
        code,
        &meta.diagram_type,
        meta.effective_config.as_config(),
        true,
        control,
    )?;
    let model = construction
        .model
        .and_then(|model| render_model_to_compat_json(&model, meta))
        .map(|value| crate::family::FamilySemanticModel::Json(value.into()));
    Ok(crate::family::CombinedSemanticParse::from_parts(
        model,
        construction.editor_facts.unwrap_or_default(),
    ))
}

#[cfg(feature = "compat-json")]
pub(crate) fn render_model_to_compat_json(
    model: &C4DiagramModel,
    meta: &SourcePresentationMetadata,
) -> Result<Value> {
    merman_model::families::c4::compat::project(model, &meta.compat_json_metadata())
        .map_err(crate::Error::from)
}

#[cfg(test)]
mod semantic_tests {
    use super::*;

    #[test]
    fn c4_quoted_arguments_trim_semantics_across_macros() {
        let source = concat!(
            "C4Context\n",
            "Boundary(\" bank \", \" Bank \", \" company \") {\n",
            "  Person(\" a \", \" Alice \", \" Description \", $tags=\" external \")\n",
            "  Container(\" b \", \" App \", \" Rust \", \" Service \", $shape=\" cylinder \")\n",
            "  Rel(a, b, \" Uses \", \" HTTPS \", \" Request \")\n",
            "}\n",
            "UpdateElementStyle(\" a \", $fontColor=\" red \", $bgColor=\" blue \")\n",
            "UpdateRelStyle(\" a \", \" b \", $lineColor=\" green \", $textColor=\" black \")\n",
            "UpdateLayoutConfig($c4ShapeInRow=\" 3 \", $c4BoundaryInRow=\" 2 \")\n",
        );
        let parsed = crate::SemanticParser::new()
            .parse_diagram_sync(source)
            .unwrap();
        let model = match parsed.model {
            crate::SemanticDiagramModel::C4(model) => model,
            #[allow(unreachable_patterns)]
            _ => panic!("expected C4 semantics"),
        };
        assert_eq!(model.boundaries[1].alias, "bank");
        assert_eq!(model.boundaries[1].label.as_str(), "Bank");
        assert_eq!(model.boundaries[1].ty.as_ref().unwrap().as_str(), "company");
        assert_eq!(model.shapes[0].alias, "a");
        assert_eq!(model.shapes[0].parent_boundary, "bank");
        assert_eq!(model.shapes[0].label.as_str(), "Alice");
        assert_eq!(
            model.shapes[0].descr.as_ref().unwrap().as_str(),
            "Description"
        );
        assert_eq!(model.shapes[0].tags, Some(Value::from("external")));
        assert_eq!(model.shapes[0].font_color.as_deref(), Some("red"));
        assert_eq!(model.shapes[0].bg_color.as_deref(), Some("blue"));
        assert_eq!(model.shapes[1].alias, "b");
        assert_eq!(model.shapes[1].label.as_str(), "App");
        assert_eq!(model.shapes[1].techn.as_ref().unwrap().as_str(), "Rust");
        assert_eq!(model.shapes[1].descr.as_ref().unwrap().as_str(), "Service");
        assert_eq!(model.shapes[1].shape, Some(Value::from("cylinder")));
        assert_eq!(model.rels[0].from_alias, "a");
        assert_eq!(model.rels[0].to_alias, "b");
        assert_eq!(model.rels[0].label.as_str(), "Uses");
        assert_eq!(model.rels[0].techn.as_ref().unwrap().as_str(), "HTTPS");
        assert_eq!(model.rels[0].descr.as_ref().unwrap().as_str(), "Request");
        assert_eq!(model.rels[0].line_color.as_deref(), Some("green"));
        assert_eq!(model.rels[0].text_color.as_deref(), Some("black"));
        assert_eq!(model.layout.c4_shape_in_row, 3.0);
        assert_eq!(model.layout.c4_boundary_in_row, 2.0);
    }

    #[test]
    fn c4_quoted_macro_tokens_preserve_physical_lf_and_lexical_punctuation() {
        let source = concat!(
            "C4Context\n",
            "Boundary(bank, \" Bank\nOffice \") {\n",
            "  Person(\" a \", \" Alice\nSmith \", \" First, (row)\nrow %% literal ) \", ",
            "$tags=\" external\nuser \") %% outside comment\n",
            "  System(b, \"B\")\n",
            "  Rel(a, b, \" Uses\nAPI \")\n",
            "}\n",
            "Person(after, \"After\")\n",
        );
        let parsed = crate::SemanticParser::new()
            .parse_diagram_sync(source)
            .unwrap();
        let model = match parsed.model {
            crate::SemanticDiagramModel::C4(model) => model,
            #[allow(unreachable_patterns)]
            _ => panic!("expected C4 semantics"),
        };
        assert_eq!(model.boundaries[1].label.as_str(), "Bank\nOffice");
        assert_eq!(model.shapes.len(), 3);
        assert_eq!(model.shapes[0].alias, "a");
        assert_eq!(model.shapes[0].label.as_str(), "Alice\nSmith");
        assert_eq!(
            model.shapes[0].descr.as_ref().unwrap().as_str(),
            "First, (row)\nrow %% literal )"
        );
        assert_eq!(model.shapes[0].tags, Some(Value::from("external\nuser")));
        assert_eq!(model.shapes[1].parent_boundary, "bank");
        assert_eq!(model.shapes[2].alias, "after");
        assert_eq!(model.shapes[2].parent_boundary, "global");
        assert_eq!(model.rels[0].label.as_str(), "Uses\nAPI");
    }

    #[test]
    fn c4_multiline_strings_do_not_enable_bare_multiline_macro_formatting() {
        for statement in [
            "Person(\n a,\n \"Alice\"\n)",
            "Person(a,\n \"Alice\")",
            "Person(a, \"Alice\nSmith\",\n \"Description\")",
            "Person(a, \"Alice\nSmith\"\n)",
            "Person(a, Alice\"\nSmith\")",
            "Person(a, \"Alice\\\"Smith\")",
        ] {
            let source = format!("C4Context\n{statement}\nSystem(after, \"After\")\n");
            let error = crate::SemanticParser::new()
                .parse_diagram_sync(&source)
                .unwrap_err();
            assert!(matches!(error, Error::DiagramParse { .. }), "{statement:?}");
        }
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn c4_trimmed_multiline_facts_retain_authored_utf8_crlf_ranges() {
        use crate::{EditorSemanticCompleteness, EditorSemanticRole};

        let source = concat!(
            "C4Context\r\n",
            "Person(\" \u{3000}a \t\", \" \tAlice\r\nSmith \t\", \" \u{3000}\t\u{3000}\", ",
            "$tags=\" \u{feff}external\u{feff} \")\r\n",
            "System(b, \"B\")\r\n",
            "Rel(\" a \", b, \" Uses\r\nAPI \")\r\n",
            "Person(after, \"After\")\r\n",
        );
        let metadata = crate::SemanticMetadata {
            diagram_type: "c4".into(),
            config: MermaidConfig::empty_object(),
            semantic_config: MermaidConfig::empty_object(),
            title: None,
        };
        reset_c4_syntax_construction_count();
        let parsed = parse_c4_semantic_model_controlled(
            source,
            &metadata,
            EditorFactsDemand::Capture,
            &OperationControl::new(),
        )
        .unwrap();
        assert_eq!(c4_syntax_construction_count(), 1);
        assert!(parsed.model.is_ok());
        let facts = parsed.editor_facts.unwrap();
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
        assert!(facts.diagnostics.is_empty());
        for (name, authored, role) in [
            ("a", " \u{3000}a \t", EditorSemanticRole::Entity),
            (
                "Alice\r\nSmith",
                " \tAlice\r\nSmith \t",
                EditorSemanticRole::Payload,
            ),
            (
                "external",
                " \u{feff}external\u{feff} ",
                EditorSemanticRole::Payload,
            ),
            ("a", " a ", EditorSemanticRole::Reference),
            ("Uses\r\nAPI", " Uses\r\nAPI ", EditorSemanticRole::Payload),
            ("after", "after", EditorSemanticRole::Entity),
        ] {
            let start = source.find(authored).unwrap();
            let span = SourceSpan::new(start, start + authored.len());
            assert!(
                facts.symbols.iter().any(|symbol| {
                    symbol.name == name
                        && symbol.role == role
                        && symbol.span == span
                        && symbol.selection == span
                }),
                "missing authored range for {name:?}"
            );
        }
        let empty_body = " \u{3000}\t\u{3000}";
        let start = source.find(empty_body).unwrap();
        assert!(facts.expected_syntax.iter().any(|expected| {
            expected.kind == EditorExpectedSyntaxKind::Payload
                && expected.span == SourceSpan::new(start, start + empty_body.len())
        }));
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn recovery_capture_retains_c4_issues_without_macro_or_payload_facts() {
        let source = "C4Context\r\ntitle Example\r\nRel(first, )\r\nPerson(later, \"Later\")\r\n";
        let metadata = crate::SemanticMetadata {
            diagram_type: "c4".into(),
            config: MermaidConfig::empty_object(),
            semantic_config: MermaidConfig::empty_object(),
            title: None,
        };
        let capture = |demand| {
            parse_c4_semantic_model_controlled(source, &metadata, demand, &OperationControl::new())
                .unwrap()
        };
        let full = capture(EditorFactsDemand::Capture);
        let full_facts = full.editor_facts.unwrap();
        assert!(!full_facts.symbols.is_empty());
        crate::editor::test_work::reset();
        reset_c4_syntax_construction_count();
        let recovery = capture(EditorFactsDemand::Recovery);
        assert_eq!(
            recovery.model.unwrap_err().to_string(),
            full.model.unwrap_err().to_string()
        );
        let facts = recovery.editor_facts.unwrap();
        assert_eq!(facts.diagnostics, full_facts.diagnostics);
        assert!(!facts.diagnostics.is_empty());
        assert!(facts.symbols.is_empty());
        assert!(facts.directive_prefixes.is_empty());
        assert!(facts.expected_syntax.is_empty());
        let work = crate::editor::test_work::read();
        assert_eq!(
            (work.symbols, work.directives, work.expected_syntax),
            (0, 0, 0)
        );
        assert_eq!(c4_syntax_construction_count(), 1);
    }

    #[test]
    fn canonical_capture_retains_all_recovery_issues_and_later_symbols() {
        let source = concat!(
            "C4Context\r\n",
            "Rel(first, )\r\n",
            "Person(later, \"Later\")\r\n",
            "Rel(second, )\r\n",
        );
        let metadata = crate::SemanticMetadata {
            diagram_type: "c4".into(),
            config: MermaidConfig::empty_object(),
            semantic_config: MermaidConfig::empty_object(),
            title: None,
        };
        reset_c4_syntax_construction_count();
        let parsed = parse_c4_semantic_model_controlled(
            source,
            &metadata,
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Capture,
            &OperationControl::new(),
        )
        .unwrap();
        assert_eq!(c4_syntax_construction_count(), 1);
        assert!(
            parsed
                .model
                .unwrap_err()
                .to_string()
                .contains("missing relation target")
        );
        #[cfg(feature = "editor-facts")]
        {
            let facts = parsed.editor_facts.expect("requested facts");
            assert_eq!(
                facts.completeness,
                crate::EditorSemanticCompleteness::Recovered
            );
            assert_eq!(facts.diagnostics.len(), 2);
            assert!(
                facts.diagnostics[0].span.unwrap().start < facts.diagnostics[1].span.unwrap().start
            );
            assert!(facts.symbols.iter().any(|symbol| symbol.name == "later"));
            assert!(facts.symbols.iter().any(|symbol| symbol.name == "Later"));
        }
    }

    #[test]
    fn canonical_c4_keeps_typed_records_and_omits_editor_observation() {
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        reset_c4_syntax_construction_count();
        let source = concat!(
            "C4Context\n",
            "Boundary(bank, \"Bank\") {\n",
            "  Person(customer, \"Customer\", $tags=\"external\")\n",
            "  System(service, \"Service\")\n",
            "  Rel(customer, service, \"Uses\")\n",
            "}\n",
            "UpdateElementStyle(customer, $bgColor=\"red\")\n",
        );
        let parsed = crate::SemanticParser::new()
            .parse_diagram_sync(source)
            .unwrap();
        assert_eq!(parsed.metadata.diagram_type, "c4");
        let model = match parsed.model {
            crate::SemanticDiagramModel::C4(model) => model,
            #[allow(unreachable_patterns)]
            _ => panic!("expected C4 semantics"),
        };
        assert_eq!(model.shapes.len(), 2);
        assert_eq!(model.shapes[0].alias, "customer");
        assert_eq!(model.shapes[0].parent_boundary, "bank");
        assert_eq!(model.shapes[0].label.as_str(), "Customer");
        assert_eq!(model.shapes[0].tags, Some(Value::from("external")));
        assert_eq!(model.shapes[0].bg_color.as_deref(), Some("red"));
        assert_eq!(model.rels[0].from_alias, "customer");
        assert_eq!(model.rels[0].to_alias, "service");
        assert_eq!(model.boundaries[1].alias, "bank");
        assert_eq!(c4_syntax_construction_count(), 1);
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());

        #[cfg(feature = "presentation")]
        {
            let full = crate::MermaidEngine::new()
                .parse_diagram_for_render_model_with_type_sync(
                    "c4",
                    source,
                    crate::ParseOptions::strict(),
                )
                .unwrap()
                .unwrap();
            let crate::RenderSemanticModel::C4(full) = full.model() else {
                panic!("expected full C4 model");
            };
            assert_eq!(
                serde_json::to_value(&model).unwrap(),
                serde_json::to_value(full).unwrap()
            );
            #[cfg(feature = "editor-facts")]
            assert_eq!(crate::editor::test_work::read(), Default::default());
        }
    }

    #[test]
    fn canonical_c4_preserves_validation_and_cancellation_without_observers() {
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        for source in [
            "C4Dynamic\nRel(a)\n",
            "C4Context\nUnexpected(a, \"A\") {\n",
            "C4Context\ndirection LR\n",
        ] {
            let error = crate::SemanticParser::new()
                .parse_diagram_sync(source)
                .unwrap_err();
            assert!(matches!(error, Error::DiagramParse { .. }));
            #[cfg(feature = "compat-json")]
            {
                let full = crate::MermaidEngine::new()
                    .parse_diagram_sync(source, crate::ParseOptions::strict())
                    .unwrap_err();
                assert_eq!(error.to_string(), full.to_string());
            }
        }
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
        let control = OperationControl::new();
        control.cancel();
        let error = crate::SemanticParser::new()
            .parse_diagram_controlled_sync("C4Context\nPerson(p, \"P\")\n", &control)
            .unwrap_err();
        assert_eq!(error.reason, crate::CancelReason::Requested);
    }
}

#[cfg(all(test, feature = "compat-json"))]
mod tests {
    use super::*;
    #[cfg(feature = "editor-facts")]
    use crate::{EditorSemanticCompleteness, EditorSemanticRole};
    use crate::{
        MermaidConfig, MermaidEngine, ParseDiagnosticSpanKind, ParseOptions, RenderSemanticModel,
        SourcePresentationMetadata,
    };
    use futures::executor::block_on;
    use serde_json::json;

    fn parse(text: &str) -> crate::ManagedSemanticJson {
        let engine = MermaidEngine::new();
        block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap()
            .model
    }

    fn parse_err(text: &str) -> crate::ParseDiagnostic {
        let engine = MermaidEngine::new();
        match block_on(engine.parse_diagram(text, ParseOptions::default())).unwrap_err() {
            Error::DiagramParse { diagnostic, .. } => diagnostic,
            other => panic!("expected C4 parse error, got {other:?}"),
        }
    }

    fn meta() -> SourcePresentationMetadata {
        SourcePresentationMetadata {
            diagram_type: "c4".to_string(),
            config: MermaidConfig::empty_object(),
            effective_config: crate::presentation::SourcePresentationConfig::empty_object(),
            title: None,
        }
    }

    #[test]
    fn c4_flat_boundaries_preserve_declaration_context_and_statement_order() {
        let source = r#"C4Context
title Root title
System(before, "Before")
Boundary(outer, "Outer") {
  System(outer_before, "Outer Before")
  accTitle: Outer title
  Boundary(inner, "Inner") {
    System(inside, "Inside")
    accDescr: Inner description
    Rel(inside, before, "Inside relation")
  }
  System(outer_after, "Outer After")
}
Boundary(sibling, "Sibling") {
  System(sibling_shape, "Sibling Shape")
  title Sibling title
}
System(after, "After")
Rel(after, sibling_shape, "After relation")
"#;
        let model = parse_c4_model_for_render(source, &meta()).expect("valid boundary contexts");
        assert_eq!(model.title.as_deref(), Some("Sibling title"));
        assert_eq!(model.acc_title, None);
        assert_eq!(model.acc_descr.as_deref(), Some("Inner description"));
        assert_eq!(
            model
                .boundaries
                .iter()
                .map(|boundary| (boundary.alias.as_str(), boundary.parent_boundary.as_str()))
                .collect::<Vec<_>>(),
            [
                ("global", ""),
                ("outer", "global"),
                ("inner", "outer"),
                ("sibling", "global")
            ]
        );
        assert_eq!(
            model
                .shapes
                .iter()
                .map(|shape| (shape.alias.as_str(), shape.parent_boundary.as_str()))
                .collect::<Vec<_>>(),
            [
                ("before", "global"),
                ("outer_before", "outer"),
                ("inside", "inner"),
                ("outer_after", "outer"),
                ("sibling_shape", "sibling"),
                ("after", "global"),
            ]
        );
        assert_eq!(
            model
                .rels
                .iter()
                .map(|relation| relation.label.as_str())
                .collect::<Vec<_>>(),
            ["Inside relation", "After relation"]
        );
    }

    #[test]
    fn combined_parse_constructs_once_and_preserves_all_projections() {
        let text = concat!(
            "C4Context\r\n",
            "title Banking Context\r\n",
            "accTitle: Banking accessibility title\r\n",
            "accDescr {\r\n",
            "  Banking accessibility description\r\n",
            "}\r\n",
            "Boundary(bank, \"Bank\") {\r\n",
            "  Person(customer, \"Customer\", \"Uses the system\")\r\n",
            "  System(system, \"Internet Banking\", \"Core system\")\r\n",
            "}\r\n",
            "Rel(customer, system, \"Uses\", \"HTTPS\")\r\n",
        );
        let expected_json = parse_c4(text, &meta()).unwrap();
        let expected_model = parse_c4_model_for_render(text, &meta()).unwrap();

        reset_c4_syntax_construction_count();
        #[cfg(feature = "editor-facts")]
        let (json, facts) = crate::family::test_support::into_result(
            parse_c4_json_and_editor_facts(text, &meta(), &OperationControl::new()),
        )
        .unwrap();

        #[cfg(not(feature = "editor-facts"))]
        let json = parse_c4(text, &meta()).unwrap();
        assert_eq!(c4_syntax_construction_count(), 1);
        assert_eq!(json, expected_json);
        #[cfg(feature = "editor-facts")]
        assert!(!facts.symbols.is_empty());
        assert_eq!(
            render_model_to_compat_json(&expected_model, &meta()).unwrap(),
            json,
            "C4 typed compatibility projection drifted"
        );
        assert!(
            json["boundaries"][0]
                .get("link")
                .is_some_and(Value::is_null),
            "the global C4 boundary must preserve Mermaid's explicit link:null"
        );
        assert!(
            json["boundaries"][0]
                .get("tags")
                .is_some_and(Value::is_null),
            "the global C4 boundary must preserve Mermaid's explicit tags:null"
        );
        assert_eq!(json["title"].as_str(), expected_model.title.as_deref());
        assert_eq!(
            json["shapes"].as_array().unwrap().len(),
            expected_model.shapes.len()
        );
        assert_eq!(
            json["rels"].as_array().unwrap().len(),
            expected_model.rels.len()
        );

        #[cfg(feature = "editor-facts")]
        for (name, marker) in [
            ("bank", "Boundary(bank"),
            ("customer", "Person(customer"),
            ("system", "System(system"),
            ("Banking Context", "Banking Context"),
        ] {
            let marker_start = text.find(marker).unwrap();
            let start = marker_start + marker.find(name).unwrap();
            assert!(
                facts.symbols.iter().any(|symbol| {
                    symbol.name == name
                        && symbol.selection == SourceSpan::new(start, start + name.len())
                }) || facts.expected_syntax.iter().any(|expected| {
                    expected.span == SourceSpan::new(start, start + name.len())
                }),
                "missing exact C4 fact for {name:?}"
            );
        }
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn malformed_editor_input_recovers_from_one_construction() {
        let text = "C4Context\nPerson(customer, \"Customer\")\nNotAMacro customer\n";
        reset_c4_syntax_construction_count();
        let facts = crate::family::test_support::editor_facts(
            parse_c4_json_and_editor_facts,
            text,
            &meta(),
        );

        assert_eq!(c4_syntax_construction_count(), 1);
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
        assert!(facts.symbols.iter().any(|symbol| symbol.name == "customer"));
        assert_eq!(facts.diagnostics.len(), 1);
    }

    #[test]
    fn unterminated_acc_descr_reports_eof_and_reuses_partial_facts() {
        let text = "C4Context\naccDescr {\n  partial description\n";
        let error = parse_c4(text, &meta()).unwrap_err();
        let Error::DiagramParse { diagnostic, .. } = error else {
            panic!("expected C4 parse diagnostic");
        };
        assert_eq!(
            diagnostic.span_kind(),
            ParseDiagnosticSpanKind::InsertionPoint
        );
        assert_eq!(
            diagnostic.span(),
            Some(SourceSpan::new(text.len(), text.len()))
        );

        #[cfg(feature = "editor-facts")]
        {
            reset_c4_syntax_construction_count();
            let facts = crate::family::test_support::editor_facts(
                parse_c4_json_and_editor_facts,
                text,
                &meta(),
            );
            assert_eq!(c4_syntax_construction_count(), 1);
            assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
            assert!(
                facts
                    .symbols
                    .iter()
                    .any(|symbol| symbol.name == "partial description")
            );
            assert_eq!(facts.diagnostics.len(), 1);
            assert_eq!(
                facts.diagnostics[0].span,
                Some(SourceSpan::new(text.len(), text.len()))
            );
        }
    }

    #[test]
    fn c4_trailing_whitespace_after_statements_is_accepted() {
        let whitespace = " ";
        let model = parse(&format!(
            "C4Context{whitespace}\n\
title System Context diagram for Internet Banking System{whitespace}\n\
Person(customerA, \"Banking Customer A\", \"A customer of the bank, with personal bank accounts.\"){whitespace}\n"
        ));
        assert_eq!(model["c4Type"], json!("C4Context"));
        assert_eq!(model["shapes"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn c4_parameter_names_that_are_keywords_are_allowed() {
        let model = parse(
            r#"C4Context
title title
Person(Person, "Person", "Person")
"#,
        );
        assert_eq!(model["title"], json!("title"));
        assert_eq!(model["shapes"][0]["alias"], json!("Person"));
        assert_eq!(model["shapes"][0]["label"]["text"], json!("Person"));
        assert_eq!(model["shapes"][0]["descr"]["text"], json!("Person"));
    }

    #[test]
    fn c4_allows_default_in_parameters() {
        let model = parse(
            r#"C4Context
Person(default, "default", "default")
"#,
        );
        assert_eq!(model["shapes"][0]["alias"], json!("default"));
        assert_eq!(model["shapes"][0]["label"]["text"], json!("default"));
        assert_eq!(model["shapes"][0]["descr"]["text"], json!("default"));
    }

    #[test]
    fn c4_person_is_parsed() {
        let model = parse(
            r#"C4Context
title System Context diagram for Internet Banking System
Person(customerA, "Banking Customer A", "A customer of the bank, with personal bank accounts.")
"#,
        );
        assert_eq!(model["shapes"].as_array().unwrap().len(), 1);
        assert_eq!(model["shapes"][0]["alias"], json!("customerA"));
        assert_eq!(
            model["shapes"][0]["label"]["text"],
            json!("Banking Customer A")
        );
        assert_eq!(
            model["shapes"][0]["descr"]["text"],
            json!("A customer of the bank, with personal bank accounts.")
        );
        assert_eq!(model["shapes"][0]["parentBoundary"], json!("global"));
        assert_eq!(model["shapes"][0]["typeC4Shape"]["text"], json!("person"));
        assert_eq!(model["shapes"][0]["wrap"], json!(false));
    }

    #[test]
    fn c4_boundary_is_parsed() {
        let model = parse(
            r#"C4Context
title System Context diagram for Internet Banking System
Boundary(b1, "BankBoundary") {
System(SystemAA, "Internet Banking System")
}
"#,
        );

        assert_eq!(model["boundaries"].as_array().unwrap().len(), 2);
        assert_eq!(model["boundaries"][1]["alias"], json!("b1"));
        assert_eq!(
            model["boundaries"][1]["label"]["text"],
            json!("BankBoundary")
        );
        assert_eq!(model["boundaries"][1]["parentBoundary"], json!("global"));
        assert_eq!(model["boundaries"][1]["type"]["text"], json!("system"));

        assert_eq!(model["shapes"].as_array().unwrap().len(), 1);
        assert_eq!(model["shapes"][0]["parentBoundary"], json!("b1"));
    }

    #[test]
    fn c4_person_ext_is_parsed() {
        let model = parse(
            r#"C4Context
Person_Ext(customerA, "Banking Customer A", "A customer of the bank, with personal bank accounts.")
"#,
        );
        assert_eq!(
            model["shapes"][0]["typeC4Shape"]["text"],
            json!("external_person")
        );
    }

    #[test]
    fn c4_system_variants_are_parsed() {
        let cases = [
            ("System", "system"),
            ("SystemDb", "system_db"),
            ("SystemQueue", "system_queue"),
            ("System_Ext", "external_system"),
            ("SystemDb_Ext", "external_system_db"),
            ("SystemQueue_Ext", "external_system_queue"),
        ];
        for (macro_name, kind) in cases {
            let model = parse(&format!(
                "C4Context\n\
{macro_name}(SystemAA, \"Internet Banking System\", \"Allows customers to view information about their bank accounts, and make payments.\")\n"
            ));
            assert_eq!(model["shapes"][0]["typeC4Shape"]["text"], json!(kind));
        }
    }

    #[test]
    fn c4_container_variants_are_parsed() {
        let cases = [
            ("Container", "container"),
            ("ContainerDb", "container_db"),
            ("ContainerQueue", "container_queue"),
            ("Container_Ext", "external_container"),
            ("ContainerDb_Ext", "external_container_db"),
            ("ContainerQueue_Ext", "external_container_queue"),
        ];
        for (macro_name, kind) in cases {
            let model = parse(&format!(
                "C4Context\n\
{macro_name}(ContainerAA, \"Internet Banking Container\", \"Technology\", \"Allows customers to view information about their bank accounts, and make payments.\")\n"
            ));
            assert_eq!(model["shapes"][0]["typeC4Shape"]["text"], json!(kind));
            assert_eq!(model["shapes"][0]["techn"]["text"], json!("Technology"));
        }
    }

    #[test]
    fn c4_label_can_be_kv_object() {
        let model = parse(
            r#"C4Context
Person(customerA, $sprite="users")
"#,
        );
        assert_eq!(
            model["shapes"][0]["label"]["text"]["sprite"],
            json!("users")
        );
    }

    #[test]
    fn c4_rel_is_deduped_by_from_to_like_mermaid_db() {
        let model = parse(
            r#"C4Context
Rel(a, b, "first")
Rel(a, b, "second")
"#,
        );
        assert_eq!(model["rels"].as_array().unwrap().len(), 1);
        assert_eq!(model["rels"][0]["label"]["text"], json!("second"));
    }

    #[test]
    fn c4_redeclaring_shape_keeps_omitted_optional_fields_like_mermaid() {
        let model = parse(
            r#"C4Context
Person(customer, "First", "Original description", "users", "retail", "https://example.com")
Person(customer, "Second")
"#,
        );
        let shape = model["shapes"][0].as_object().unwrap();

        assert_eq!(shape["label"]["text"], json!("Second"));
        assert_eq!(shape["descr"]["text"], json!("Original description"));
        assert_eq!(shape["sprite"], json!("users"));
        assert_eq!(shape["tags"], json!("retail"));
        assert_eq!(shape["link"], json!("https://example.com"));
    }

    #[test]
    fn c4_redeclaring_container_and_component_keeps_omitted_optional_fields() {
        let model = parse(
            r#"C4Component
Container(container, "First container", "Rust", "Description", "database", "backend", "https://example.com/container")
Container(container, "Second container")
Component(component, "First component", "Rust", "Description", "server", "backend", "https://example.com/component")
Component(component, "Second component")
"#,
        );
        let shapes = model["shapes"].as_array().unwrap();

        assert_eq!(
            shapes.len(),
            2,
            "redeclaration must preserve insertion order"
        );
        for (shape, alias, label) in [
            (&shapes[0], "container", "Second container"),
            (&shapes[1], "component", "Second component"),
        ] {
            let shape = shape.as_object().unwrap();
            assert_eq!(shape["alias"], json!(alias));
            assert_eq!(shape["label"]["text"], json!(label));
            assert_eq!(shape["techn"]["text"], json!("Rust"));
            assert_eq!(shape["descr"]["text"], json!("Description"));
            assert!(shape.contains_key("sprite"));
            assert!(shape.contains_key("tags"));
            assert!(shape.contains_key("link"));
        }
    }

    #[test]
    fn c4_redeclaring_relation_keeps_omitted_optional_fields() {
        let model = parse(
            r#"C4Dynamic
Rel(a, b, "First", "HTTPS", "Description", "server", "backend", "https://example.com")
Rel(a, b, "Second")
"#,
        );
        let rel = model["rels"][0].as_object().unwrap();

        assert_eq!(model["rels"].as_array().unwrap().len(), 1);
        assert_eq!(rel["label"]["text"], json!("Second"));
        assert_eq!(rel["techn"]["text"], json!("HTTPS"));
        assert_eq!(rel["descr"]["text"], json!("Description"));
        assert!(rel.contains_key("sprite"));
        assert!(rel.contains_key("tags"));
        assert!(rel.contains_key("link"));
    }

    #[test]
    fn c4_declaration_named_arguments_cannot_override_structural_fields() {
        let model = parse(
            r#"C4Context
Boundary(b, "Boundary") {
  Person(p, "Person", "", $wrap="false", $parentBoundary="outside", $typeC4Shape="wrong")
}
"#,
        );
        let shape = &model["shapes"][0];

        assert_eq!(shape["alias"], json!("p"));
        assert_eq!(shape["parentBoundary"], json!("b"));
        assert_eq!(shape["typeC4Shape"]["text"], json!("person"));
        assert_eq!(shape["wrap"], json!(false));
    }

    #[test]
    fn c4_named_arguments_in_earlier_slots_are_not_clobbered() {
        let model = parse(
            r#"C4Context
Person(p, "Person", "Description", $tags="tag1,tag2", $link="https://example.com")
"#,
        );
        let shape = model["shapes"][0].as_object().unwrap();

        assert_eq!(shape["tags"], json!("tag1,tag2"));
        assert_eq!(shape["link"], json!("https://example.com"));
    }

    #[test]
    fn c4_named_attributes_use_field_types_independent_of_argument_slot() {
        // Mermaid 12 c4NamedAttributes.spec.ts: both text and optional slots use TEXT_FIELDS.
        for (header, statement, collection, index, key, expected) in [
            (
                "C4Context",
                r#"System(s, "S", $tags="cylinder")"#,
                "shapes",
                0,
                "tags",
                json!("cylinder"),
            ),
            (
                "C4Context",
                r#"Person(p, "P", $descr="description")"#,
                "shapes",
                0,
                "descr",
                json!({"text": "description"}),
            ),
            (
                "C4Container",
                r#"Container(c, "C", $sprite="database", $link="https://example.com")"#,
                "shapes",
                0,
                "sprite",
                json!("database"),
            ),
            (
                "C4Container",
                r#"Container(c, "C", $descr="description")"#,
                "shapes",
                0,
                "descr",
                json!({"text": "description"}),
            ),
            (
                "C4Component",
                r#"Component(c, "C", "Rust", "Description", $techn="Java")"#,
                "shapes",
                0,
                "techn",
                json!({"text": "Java"}),
            ),
            (
                "C4Container",
                r#"System_Boundary(b, "B", $type="custom") {
  Container(c, "C")
}"#,
                "boundaries",
                1,
                "type",
                json!({"text": "custom"}),
            ),
            (
                "C4Container",
                r#"Container_Boundary(b, "B", $type="custom") {
  Component(c, "C")
}"#,
                "boundaries",
                1,
                "type",
                json!({"text": "custom"}),
            ),
            (
                "C4Deployment",
                r#"Node(b, "B", $descr="description") {
  Container(c, "C")
}"#,
                "boundaries",
                1,
                "descr",
                json!({"text": "description"}),
            ),
            (
                "C4Dynamic",
                r#"Rel(a, b, "Uses", $tags="async")"#,
                "rels",
                0,
                "tags",
                json!("async"),
            ),
            (
                "C4Dynamic",
                r#"Rel(a, b, "Uses", $descr="description")"#,
                "rels",
                0,
                "descr",
                json!({"text": "description"}),
            ),
            (
                "C4Context",
                r#"System(s, "S", "Description", $label="Renamed")"#,
                "shapes",
                0,
                "label",
                json!({"text": "Renamed"}),
            ),
        ] {
            let source = format!("{header}\n{statement}\n");
            let model = parse(&source);
            assert_eq!(model[collection][index][key], expected, "{statement}");
        }

        let model = parse("C4Container\nContainer(c, \"C\", $techn=\"Java\")\n");
        assert_eq!(model["shapes"][0]["descr"], json!({"text": ""}));
    }

    #[test]
    fn c4_alias_updates_keep_first_match_lookup_semantics() {
        let model = parse(
            r#"C4Context
Person(a, "A")
UpdateElementStyle(a, $alias="b")
UpdateElementStyle(b, $bgColor="red")
Person(b, "B")
"#,
        );
        let shapes = model["shapes"].as_array().unwrap();

        assert_eq!(shapes.len(), 1);
        assert_eq!(shapes[0]["alias"], json!("b"));
        assert_eq!(shapes[0]["label"]["text"], json!("B"));
        assert_eq!(shapes[0]["bgColor"], json!("red"));
    }

    #[test]
    fn c4_relindex_ignores_index_arg() {
        let model = parse(
            r#"C4Context
RelIndex(123, a, b, "label")
"#,
        );
        assert_eq!(model["rels"].as_array().unwrap().len(), 1);
        assert_eq!(model["rels"][0]["from"], json!("a"));
        assert_eq!(model["rels"][0]["to"], json!("b"));
        assert_eq!(model["rels"][0]["label"]["text"], json!("label"));
    }

    #[test]
    fn c4_wrap_directive_sets_wrap_true_on_nodes() {
        let model = parse(
            r#"%%{wrap}%%
C4Context
Person(a, "A", "D")
"#,
        );
        assert_eq!(model["wrap"], json!(true));
        assert_eq!(model["shapes"][0]["wrap"], json!(true));
    }

    #[test]
    fn c4_update_element_style_updates_shape_fields() {
        let model = parse(
            r#"C4Context
Person(a, "A", "D")
UpdateElementStyle(a, $bgColor="red", $borderColor="blue")
"#,
        );
        assert_eq!(model["shapes"][0]["bgColor"], json!("red"));
        assert_eq!(model["shapes"][0]["borderColor"], json!("blue"));
    }

    #[test]
    fn c4_update_element_style_can_target_boundaries() {
        let model = parse(
            r#"C4Context
Boundary(b1, "B") {
  Person(child, "Child")
}
UpdateElementStyle(b1, $bgColor="red")
"#,
        );
        assert_eq!(model["boundaries"][1]["bgColor"], json!("red"));
    }

    #[test]
    fn c4_update_rel_style_updates_rel_fields() {
        let model = parse(
            r#"C4Context
Rel(a, b, "label")
UpdateRelStyle(a, b, $textColor="red", $lineColor="blue", $offsetX="10", $offsetY="20")
"#,
        );
        assert_eq!(model["rels"][0]["textColor"], json!("red"));
        assert_eq!(model["rels"][0]["lineColor"], json!("blue"));
        assert_eq!(model["rels"][0]["offsetX"], json!(10));
        assert_eq!(model["rels"][0]["offsetY"], json!(20));
    }

    #[test]
    fn c4_update_rel_style_sparse_named_offsets_follow_their_keys() {
        let model = parse(
            r#"C4Dynamic
Rel(c2, c3, "Calls isAuthenticated() on")
UpdateRelStyle(c2, c3, $textColor="red", $offsetX="-40", $offsetY="60")
"#,
        );
        let rel = &model["rels"][0];

        assert_eq!(rel["textColor"], json!("red"));
        assert!(
            rel.get("lineColor").is_none(),
            "omitting lineColor must not consume a named offset"
        );
        assert_eq!(rel["offsetX"], json!("-40"));
        assert_eq!(rel["offsetY"], json!(60));
    }

    #[test]
    fn c4_update_rel_style_named_fields_are_order_independent() {
        let model = parse(
            r#"C4Dynamic
Rel(a, b, "label")
UpdateRelStyle(a, b, $offsetX="-40", $textColor="red", $offsetY="60")
"#,
        );
        let rel = &model["rels"][0];

        assert_eq!(rel["textColor"], json!("red"));
        assert_eq!(rel["offsetX"], json!("-40"));
        assert_eq!(rel["offsetY"], json!(60));
    }

    #[test]
    fn c4_update_rel_style_uses_javascript_parse_int_semantics() {
        let model = parse(
            r#"C4Dynamic
Rel(a, b, "positional")
Rel(c, d, "named")
UpdateRelStyle(a, b, "red", "blue", "   -40px", "+60.75")
UpdateRelStyle(c, d, $offsetX="not-a-number", $offsetY="0x10tail")
"#,
        );
        let positional = &model["rels"][0];
        let named = &model["rels"][1];

        assert_eq!(positional["textColor"], json!("red"));
        assert_eq!(positional["lineColor"], json!("blue"));
        assert_eq!(positional["offsetX"], json!(-40));
        assert_eq!(positional["offsetY"], json!(60));
        assert_eq!(named["offsetX"], json!("not-a-number"));
        assert_eq!(named["offsetY"], json!("0x10tail"));
    }

    #[test]
    fn c4_update_rel_style_preserves_unknown_named_key_side_effects() {
        let model = parse(
            r#"C4Dynamic
Rel(a, b, "label")
UpdateRelStyle(a, b, $wrap="false")
UpdateRelStyle(a, b, $from="c")
UpdateRelStyle(c, b, $lineColor="red")
"#,
        );
        let rel = &model["rels"][0];

        assert_eq!(rel["from"], json!("c"));
        assert_eq!(rel["lineColor"], json!("red"));
        assert_eq!(rel["wrap"], json!(true));
    }

    #[test]
    fn relation_named_type_retains_validation_until_a_later_assignment() {
        let source = "C4Dynamic\nRel(a, b, \"label\", $type=\"custom\")\n";
        let error = parse_err(source);
        assert!(error.message().contains("expected string field `type`"));
        #[cfg(feature = "editor-facts")]
        {
            let strict_error = parse_c4(source, &meta()).unwrap_err();
            let (model, facts) =
                parse_c4_json_and_editor_facts(source, &meta(), &OperationControl::new())
                    .unwrap()
                    .into_parts();
            assert_eq!(
                model.err().map(|error| error.to_string()),
                Some(strict_error.to_string())
            );
            assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
            assert!(facts.diagnostics.is_empty());
            assert!(facts.symbols.iter().any(|symbol| symbol.name == "label"));
        }
        let repaired = format!("{source}UpdateRelStyle(a, b, $type=\"custom\")\n");
        assert_eq!(parse(&repaired)["rels"][0]["type"], json!("custom"));
    }

    #[test]
    fn relation_redeclaration_replaces_an_invalid_named_type() {
        let source = concat!(
            "C4Dynamic\n",
            "Rel(a, b, \"first\", $type=\"custom\")\n",
            "Rel(a, b, \"second\")\n",
        );
        let model = parse(source);
        assert_eq!(model["rels"][0]["type"], json!("rel"));
        assert_eq!(model["rels"][0]["label"]["text"], json!("second"));
    }

    #[test]
    fn c4_update_layout_config_enforces_minimum_one() {
        let model = parse(
            r#"C4Context
UpdateLayoutConfig(0, 0)
"#,
        );
        assert_eq!(model["layout"]["c4ShapeInRow"], json!(4));
        assert_eq!(model["layout"]["c4BoundaryInRow"], json!(2));

        let model = parse(
            r#"C4Context
UpdateLayoutConfig(3, 2)
"#,
        );
        assert_eq!(model["layout"]["c4ShapeInRow"], json!(3));
        assert_eq!(model["layout"]["c4BoundaryInRow"], json!(2));
    }

    #[test]
    fn c4_deployment_node_ignores_sprite_param_like_mermaid_db() {
        let model = parse(
            r#"C4Deployment
Node(n1, "Node", "type", "descr", $sprite="users") {
  Person(p1, "P1")
}
"#,
        );
        assert_eq!(model["boundaries"].as_array().unwrap().len(), 2);
        assert!(model["boundaries"][1].get("sprite").is_none());
    }

    #[test]
    fn c4_boundary_brace_can_be_on_next_line() {
        let model = parse(
            r#"C4Context
Boundary(b1, "B")
{
  Person(p1, "P")
}
"#,
        );
        assert_eq!(model["boundaries"].as_array().unwrap().len(), 2);
        assert_eq!(model["boundaries"][1]["alias"], json!("b1"));
        assert_eq!(model["shapes"].as_array().unwrap().len(), 1);
        assert_eq!(model["shapes"][0]["parentBoundary"], json!("b1"));
    }

    #[test]
    fn c4_non_boundary_macro_cannot_open_a_boundary_body() {
        let source = "C4Context\nPerson(p, \"P\") {\n";
        let diagnostic = parse_err(source);
        assert!(
            diagnostic
                .message()
                .contains("unexpected '{' after non-boundary")
        );
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn c4_unclosed_boundary_editor_facts_keep_lexical_declarations_at_eof() {
        let source = "C4Context\nBoundary(open, \"Open\") {\n  Person(inside, \"Inside\")\n";
        let facts = crate::family::test_support::editor_facts(
            parse_c4_json_and_editor_facts,
            source,
            &meta(),
        );
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
        assert!(facts.symbols.iter().any(|symbol| symbol.name == "inside"));
    }

    #[test]
    fn c4_render_model_fails_closed_for_boundary_structure_errors() {
        for source in [
            "C4Context\nBoundary(missing, \"Missing\")\nPerson(after, \"After\")\n",
            "C4Context\nBoundary(empty, \"Empty\") {\n}\n",
            "C4Context\nBoundary(open, \"Open\") {\nPerson(inside, \"Inside\")\n",
            "C4Context\nPerson(before, \"Before\")\n}\n",
        ] {
            assert!(
                parse_c4_model_for_render(source, &meta()).is_err(),
                "render projection accepted structurally invalid C4 source: {source:?}"
            );
        }
    }

    #[test]
    fn c4_direction_after_header_is_rejected_but_kept_in_editor_facts() {
        let source = "C4Context\nPerson(customer, \"Customer\")\ndirection LR\n";
        let error = parse_c4(source, &meta()).expect_err("direction is not a C4 statement");
        assert!(error.to_string().contains("unsupported C4 statement"));

        #[cfg(feature = "editor-facts")]
        {
            let facts = crate::family::test_support::editor_facts(
                parse_c4_json_and_editor_facts,
                source,
                &meta(),
            );
            assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
            let lr = source.find("LR").unwrap();
            assert!(facts.expected_syntax.iter().any(|expected| {
                expected.kind == EditorExpectedSyntaxKind::CardinalDirectionValue
                    && expected.span == SourceSpan::new(lr, lr + "LR".len())
            }));
        }
    }

    #[test]
    fn c4_nested_boundaries_keep_parent_boundary_correct() {
        let model = parse(
            r#"C4Context
Enterprise_Boundary(ent, "Enterprise") {
  System_Boundary(sys, "System") {
    Person(p1, "P")
  }
  Person(p2, "P2")
}
"#,
        );

        assert_eq!(model["boundaries"].as_array().unwrap().len(), 3);
        assert_eq!(model["boundaries"][1]["alias"], json!("ent"));
        assert_eq!(model["boundaries"][1]["type"]["text"], json!("ENTERPRISE"));
        assert_eq!(model["boundaries"][1]["parentBoundary"], json!("global"));

        assert_eq!(model["boundaries"][2]["alias"], json!("sys"));
        assert_eq!(model["boundaries"][2]["type"]["text"], json!("SYSTEM"));
        assert_eq!(model["boundaries"][2]["parentBoundary"], json!("ent"));

        assert_eq!(model["shapes"].as_array().unwrap().len(), 2);
        assert_eq!(model["shapes"][0]["alias"], json!("p1"));
        assert_eq!(model["shapes"][0]["parentBoundary"], json!("sys"));
        assert_eq!(model["shapes"][1]["alias"], json!("p2"));
        assert_eq!(model["shapes"][1]["parentBoundary"], json!("ent"));
    }

    #[test]
    fn c4_container_boundary_injects_container_type() {
        let model = parse(
            r#"C4Container
Container_Boundary(cb, "CB") {
  Container(c1, "C1", "Tech", "Desc")
}
"#,
        );
        assert_eq!(model["boundaries"].as_array().unwrap().len(), 2);
        assert_eq!(model["boundaries"][1]["alias"], json!("cb"));
        assert_eq!(model["boundaries"][1]["type"]["text"], json!("CONTAINER"));
        assert_eq!(model["shapes"].as_array().unwrap().len(), 1);
        assert_eq!(model["shapes"][0]["parentBoundary"], json!("cb"));
    }

    #[test]
    fn c4_container_boundary_with_only_alias_uses_container_default_type() {
        let model = parse(
            r#"C4Container
Container_Boundary("b") {
  Person(p, "P")
}
"#,
        );

        assert_eq!(model["boundaries"][1]["alias"], json!("b"));
        assert_eq!(model["boundaries"][1]["type"]["text"], json!("container"));
    }

    #[test]
    fn c4_nested_nodes_push_and_pop_like_boundaries() {
        let model = parse(
            r#"C4Deployment
Node(n1, "N1") {
  Node_L(n2, "N2") {
    Person(p1, "P1")
  }
  Person(p2, "P2")
}
"#,
        );
        assert_eq!(model["boundaries"].as_array().unwrap().len(), 3);
        assert_eq!(model["boundaries"][1]["alias"], json!("n1"));
        assert_eq!(model["boundaries"][1]["nodeType"], json!("node"));
        assert_eq!(model["boundaries"][2]["alias"], json!("n2"));
        assert_eq!(model["boundaries"][2]["nodeType"], json!("nodeL"));
        assert_eq!(model["boundaries"][2]["parentBoundary"], json!("n1"));

        assert_eq!(model["shapes"].as_array().unwrap().len(), 2);
        assert_eq!(model["shapes"][0]["alias"], json!("p1"));
        assert_eq!(model["shapes"][0]["parentBoundary"], json!("n2"));
        assert_eq!(model["shapes"][1]["alias"], json!("p2"));
        assert_eq!(model["shapes"][1]["parentBoundary"], json!("n1"));
    }

    #[test]
    fn c4_update_layout_config_accepts_kv_objects() {
        let model = parse(
            r#"C4Context
UpdateLayoutConfig($c4ShapeInRow="1", $c4BoundaryInRow="1")
"#,
        );
        assert_eq!(model["layout"]["c4ShapeInRow"], json!(1));
        assert_eq!(model["layout"]["c4BoundaryInRow"], json!(1));
    }

    #[test]
    fn c4_update_macros_are_noop_when_target_missing() {
        let model = parse(
            r#"C4Context
UpdateElementStyle(missing, $bgColor="red")
UpdateRelStyle(a, b, $textColor="red")
"#,
        );
        assert_eq!(model["shapes"].as_array().unwrap().len(), 0);
        assert_eq!(model["rels"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn c4_techn_and_descr_can_be_kv_objects() {
        let model = parse(
            r#"C4Context
Container(c1, "C1", $techn="Rust", $descr="Fast")
"#,
        );
        assert_eq!(model["shapes"].as_array().unwrap().len(), 1);
        assert_eq!(model["shapes"][0]["techn"]["text"], json!("Rust"));
        assert_eq!(model["shapes"][0]["descr"]["text"], json!("Fast"));
    }

    #[test]
    fn c4_boundary_type_can_be_kv_object() {
        let model = parse(
            r#"C4Context
Boundary(b1, "B", $type="company") {
  Person(p1, "P1")
}
"#,
        );
        assert_eq!(model["boundaries"].as_array().unwrap().len(), 2);
        assert_eq!(model["boundaries"][1]["type"]["text"], json!("company"));
    }

    #[test]
    fn c4_empty_args_are_allowed() {
        let model = parse(
            r#"C4Context
Person(a, , "D")
"#,
        );
        assert_eq!(model["shapes"].as_array().unwrap().len(), 1);
        assert_eq!(model["shapes"][0]["label"]["text"], json!(""));
        assert_eq!(model["shapes"][0]["descr"]["text"], json!("D"));
    }

    #[test]
    fn c4_header_without_statements_is_rejected() {
        let diagnostic = parse_err("C4Context\n");
        assert!(
            diagnostic
                .message()
                .contains("expected at least one C4 statement")
        );
    }

    #[test]
    fn c4_rel_direction_macros_are_parsed() {
        let model = parse(
            r#"C4Context
Rel(a, b, "l1")
BiRel(a, b, "l2")
Rel_Up(a, b, "l3")
Rel_U(a, b, "l4")
Rel_Down(a, b, "l5")
Rel_D(a, b, "l6")
Rel_Left(a, b, "l7")
Rel_L(a, b, "l8")
Rel_Right(a, b, "l9")
Rel_R(a, b, "l10")
Rel_Back(a, b, "l11")
"#,
        );
        let rels = model["rels"].as_array().unwrap();
        assert_eq!(rels.len(), 1, "rels are deduped by (from,to)");
        assert_eq!(model["rels"][0]["from"], json!("a"));
        assert_eq!(model["rels"][0]["to"], json!("b"));
        assert_eq!(model["rels"][0]["type"], json!("rel_b"));
        assert_eq!(model["rels"][0]["label"]["text"], json!("l11"));
    }

    #[test]
    fn c4_rel_without_label_is_ignored_like_mermaid_db() {
        let model = parse(
            r#"C4Context
Rel(a, b)
Rel(a, b, )
"#,
        );
        assert_eq!(model["rels"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn c4_missing_relation_target_reports_local_insertion_point() {
        let text = "C4Context\nRel(a)\n";
        let diagnostic = parse_err(text);
        let insert = text.find(')').unwrap();
        assert_eq!(diagnostic.message(), "missing relation target");
        assert_eq!(diagnostic.span(), Some(SourceSpan::new(insert, insert)));
        assert_eq!(
            diagnostic.span_kind(),
            ParseDiagnosticSpanKind::InsertionPoint
        );
    }

    #[test]
    fn c4_missing_relation_style_target_reports_local_insertion_point() {
        let text = "C4Context\nUpdateRelStyle(a)\n";
        let diagnostic = parse_err(text);
        let insert = text.find(')').unwrap();
        assert_eq!(diagnostic.message(), "missing relation style target");
        assert_eq!(diagnostic.span(), Some(SourceSpan::new(insert, insert)));
        assert_eq!(
            diagnostic.span_kind(),
            ParseDiagnosticSpanKind::InsertionPoint
        );
    }

    #[test]
    fn c4_rel_inline_comment_is_ignored_but_not_inside_quotes() {
        let model = parse(
            r#"C4Context
Rel(a, b, "label %% not a comment") %% actual comment
"#,
        );
        assert_eq!(model["rels"].as_array().unwrap().len(), 1);
        assert_eq!(
            model["rels"][0]["label"]["text"],
            json!("label %% not a comment")
        );
    }

    #[test]
    fn c4_label_supports_sprite_link_tags_via_kv_objects() {
        let model = parse(
            r#"C4Context
Person(p1, $sprite="users")
System(s1, $link="https://github.com/mermaidjs")
Container(c1, $tags="tag1,tag2")
"#,
        );
        assert_eq!(model["shapes"].as_array().unwrap().len(), 3);
        assert_eq!(
            model["shapes"][0]["label"]["text"]["sprite"],
            json!("users")
        );
        assert_eq!(
            model["shapes"][1]["label"]["text"]["link"],
            json!("https://github.com/mermaidjs")
        );
        assert_eq!(
            model["shapes"][2]["label"]["text"]["tags"],
            json!("tag1,tag2")
        );
    }

    #[test]
    fn c4_sprite_link_tags_can_be_provided_as_positional_fields() {
        let model = parse(
            r#"C4Context
Person(p1, "P", "D", $sprite="users", $tags="tag1,tag2", $link="https://example.com")
"#,
        );
        assert_eq!(model["shapes"].as_array().unwrap().len(), 1);
        assert_eq!(model["shapes"][0]["sprite"], json!("users"));
        assert_eq!(model["shapes"][0]["tags"], json!("tag1,tag2"));
        assert_eq!(model["shapes"][0]["link"], json!("https://example.com"));
    }

    #[test]
    fn c4_boundary_supports_sprite_link_tags_via_kv_objects_or_positional_fields() {
        let model = parse(
            r#"C4Context
Boundary(b1, $link="https://example.com") {
  Person(p1, "P1")
}
Boundary(b2, "B2", "company", $tags="tag1,tag2", $link="https://example.com") {
  Person(p2, "P2")
}
"#,
        );
        assert_eq!(model["boundaries"].as_array().unwrap().len(), 3);
        assert_eq!(
            model["boundaries"][1]["label"]["text"]["link"],
            json!("https://example.com")
        );
        assert_eq!(model["boundaries"][2]["type"]["text"], json!("company"));
        assert_eq!(model["boundaries"][2]["tags"], json!("tag1,tag2"));
        assert_eq!(model["boundaries"][2]["link"], json!("https://example.com"));
    }

    #[test]
    fn c4_update_element_style_applies_all_supported_fields() {
        let model = parse(
            r#"C4Context
Person(p1, "P1")
Boundary(b1, "B1") {
  Person(p2, "P2")
}
UpdateElementStyle(p1, $bgColor="red", $fontColor="white", $borderColor="black", $shadowing="true", $shape="rounded", $sprite="users", $techn="Rust", $legendText="Legend", $legendSprite="book")
UpdateElementStyle(b1, $bgColor="blue")
"#,
        );
        assert_eq!(model["shapes"].as_array().unwrap().len(), 2);
        assert_eq!(model["shapes"][0]["bgColor"], json!("red"));
        assert_eq!(model["shapes"][0]["fontColor"], json!("white"));
        assert_eq!(model["shapes"][0]["borderColor"], json!("black"));
        assert_eq!(model["shapes"][0]["shadowing"], json!("true"));
        assert_eq!(model["shapes"][0]["shape"], json!("rounded"));
        assert_eq!(model["shapes"][0]["sprite"], json!("users"));
        assert_eq!(model["shapes"][0]["techn"], json!("Rust"));
        assert_eq!(model["shapes"][0]["legendText"], json!("Legend"));
        assert_eq!(model["shapes"][0]["legendSprite"], json!("book"));

        assert_eq!(model["boundaries"].as_array().unwrap().len(), 2);
        assert_eq!(model["boundaries"][1]["bgColor"], json!("blue"));
    }

    #[test]
    fn c4_acc_title_is_mapped_to_title_like_mermaid_grammar() {
        let model = parse(
            r#"C4Context
accTitle: A11y title
"#,
        );
        assert_eq!(model["title"], json!("A11y title"));
        assert!(model["accTitle"].is_null());
    }

    #[test]
    fn c4_acc_descr_multiline_collapses_newline_whitespace_like_common_db() {
        let model = parse(
            r#"C4Context
accDescr{
first
  second
third
}
"#,
        );
        assert_eq!(model["accDescr"], json!("first\nsecond\nthird"));
    }

    #[test]
    fn c4_render_model_uses_typed_variant_without_changing_json_parse() {
        let engine = MermaidEngine::new();
        let input = r#"C4Context
title Banking Context
Person(customer, "Customer", "Uses the system")
System(system, "Internet Banking", "Core system")
Rel(customer, system, "Uses", "HTTPS")
"#;

        let parsed = engine
            .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
            .unwrap()
            .unwrap();

        assert_eq!(parsed.metadata().diagram_type, "c4");
        match parsed.model() {
            RenderSemanticModel::C4(model) => {
                assert_eq!(model.c4_type, "C4Context");
                assert_eq!(model.title.as_deref(), Some("Banking Context"));
                assert_eq!(model.shapes.len(), 2);
                assert_eq!(model.shapes[0].label.as_str(), "Customer");
                assert_eq!(model.rels.len(), 1);
                assert_eq!(model.rels[0].label.as_str(), "Uses");
            }
            other => panic!("c4 render parse should return typed model, got {other:?}"),
        }

        let parsed_json = engine
            .parse_diagram_sync(input, ParseOptions::strict())
            .unwrap()
            .unwrap();
        assert_eq!(parsed_json.model["type"], json!("c4"));
        assert_eq!(parsed_json.model["c4Type"], json!("C4Context"));
        assert_eq!(
            parsed_json.model["shapes"][0]["label"]["text"],
            json!("Customer")
        );
        assert_eq!(parsed_json.model["rels"][0]["label"]["text"], json!("Uses"));
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn c4_editor_facts_expose_parser_backed_spans() {
        let engine = MermaidEngine::new();
        let input = r#"C4Context
title Banking Context
accTitle: Banking accessibility title
accDescr: Banking accessibility description
Boundary(bank, "Bank") {
  Person(customer, "Customer", "Uses the system")
  System(system, "Internet Banking", "Core system")
}
Rel(customer, system, "Uses", "HTTPS")
UpdateElementStyle(system, $bgColor="red")
UpdateRelStyle(customer, system, $lineColor="blue")
"#;

        let facts = engine
            .parse_editor_semantic_facts_with_type_sync("c4", input)
            .unwrap()
            .unwrap();

        assert!(facts.directive_prefixes.iter().any(|p| p == "title"));
        assert!(facts.directive_prefixes.iter().any(|p| p == "accTitle"));
        assert!(facts.directive_prefixes.iter().any(|p| p == "accDescr"));
        for entity in ["bank", "customer", "system"] {
            assert!(
                facts.symbols.iter().any(|symbol| {
                    symbol.name == entity
                        && symbol.kind == EditorSemanticKind::Object
                        && symbol.role == EditorSemanticRole::Entity
                }),
                "missing C4 entity fact for {entity}"
            );
        }
        for payload in [
            "Banking Context",
            "Banking accessibility title",
            "Banking accessibility description",
            "Customer",
            "Core system",
            "Uses",
            "HTTPS",
            "red",
            "blue",
        ] {
            assert!(
                facts.symbols.iter().any(|symbol| {
                    symbol.name == payload
                        && symbol.kind == EditorSemanticKind::String
                        && symbol.role == EditorSemanticRole::Payload
                }),
                "missing C4 payload fact for {payload}"
            );
        }

        let roles_for = |name: &str| {
            facts
                .symbols
                .iter()
                .filter(|symbol| symbol.name == name)
                .map(|symbol| symbol.role)
                .collect::<Vec<_>>()
        };
        assert_eq!(roles_for("bank"), vec![EditorSemanticRole::Entity]);
        assert_eq!(
            roles_for("customer"),
            vec![
                EditorSemanticRole::Entity,
                EditorSemanticRole::Reference,
                EditorSemanticRole::Reference,
            ]
        );
        assert_eq!(
            roles_for("system"),
            vec![
                EditorSemanticRole::Entity,
                EditorSemanticRole::Reference,
                EditorSemanticRole::Reference,
                EditorSemanticRole::Reference,
            ]
        );

        let system_refs = facts
            .symbols
            .iter()
            .filter(|symbol| {
                symbol.name == "system" && symbol.role == EditorSemanticRole::Reference
            })
            .count();
        assert_eq!(
            system_refs, 3,
            "system relation and style occurrences must remain references"
        );

        let title_start = input.find("Banking Context").unwrap();
        assert!(facts.expected_syntax.iter().any(|expected| {
            expected.kind == EditorExpectedSyntaxKind::Payload
                && expected.span
                    == SourceSpan::new(title_start, title_start + "Banking Context".len())
        }));
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn c4_forward_relation_and_style_targets_remain_typed_references() {
        let input = concat!(
            "C4Context\n",
            "Rel(customer, system, \"Uses\")\n",
            "UpdateElementStyle(system, $bgColor=\"red\")\n",
            "UpdateRelStyle(customer, system, $lineColor=\"blue\")\n",
            "Person(customer, \"Customer\")\n",
            "System(system, \"System\")\n",
        );
        let facts = crate::family::test_support::editor_facts(
            parse_c4_json_and_editor_facts,
            input,
            &meta(),
        );
        let roles_for = |name: &str| {
            facts
                .symbols
                .iter()
                .filter(|symbol| symbol.name == name)
                .map(|symbol| symbol.role)
                .collect::<Vec<_>>()
        };

        assert_eq!(
            roles_for("customer"),
            vec![
                EditorSemanticRole::Reference,
                EditorSemanticRole::Reference,
                EditorSemanticRole::Entity,
            ]
        );
        assert_eq!(
            roles_for("system"),
            vec![
                EditorSemanticRole::Reference,
                EditorSemanticRole::Reference,
                EditorSemanticRole::Reference,
                EditorSemanticRole::Entity,
            ]
        );

        let relation_source = facts
            .symbols
            .iter()
            .find(|symbol| symbol.detail.as_deref() == Some("c4 relation source"))
            .expect("relation source");
        assert_eq!(relation_source.role, EditorSemanticRole::Reference);
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn c4_entity_occurrence_role_does_not_depend_on_display_detail() {
        let span = SourceSpan::new(0, "node".len());
        let mut facts = EditorSemanticFacts::new();
        for (detail, occurrence) in [
            (
                "reference-looking definition",
                C4EntityOccurrence::Definition,
            ),
            (
                "definition-looking reference",
                C4EntityOccurrence::Reference,
            ),
        ] {
            project_c4_observation(
                &mut facts,
                C4Observation::Entity {
                    text: "node",
                    span,
                    detail,
                    occurrence,
                },
            );
        }
        assert_eq!(facts.symbols[0].role, EditorSemanticRole::Entity);
        assert_eq!(facts.symbols[1].role, EditorSemanticRole::Reference);
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn c4_editor_facts_recover_unsupported_statements_without_losing_prior_facts() {
        let engine = MermaidEngine::new();
        let input = "C4Context\nPerson(customer, \"Customer\")\nNotAMacro customer\n";

        let facts = engine
            .parse_editor_semantic_facts_with_type_sync("c4", input)
            .unwrap()
            .unwrap();

        assert_eq!(
            facts.completeness,
            crate::EditorSemanticCompleteness::Recovered
        );
        assert!(facts.symbols.iter().any(|symbol| {
            symbol.name == "customer" && symbol.role == EditorSemanticRole::Entity
        }));
        assert!(!facts.diagnostics.is_empty());
    }

    #[test]
    fn c4_recovery_keeps_later_safe_lines() {
        let source = concat!(
            "C4Context\r\n",
            "Rel(known, )\r\n",
            "Person(后续, \"Later\")\r\n",
            "UpdateLayoutConfig(3, 2)\r\n",
        );
        let error = parse_c4(source, &meta()).expect_err("strict C4 parse must return first error");
        assert!(error.to_string().contains("missing relation target"));

        #[cfg(feature = "editor-facts")]
        {
            let facts = crate::family::test_support::editor_facts(
                parse_c4_json_and_editor_facts,
                source,
                &meta(),
            );
            assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
            assert!(facts.symbols.iter().any(|symbol| symbol.name == "后续"));
            assert!(facts.symbols.iter().any(|symbol| symbol.name == "Later"));
        }
    }

    #[test]
    fn c4_missing_boundary_brace_does_not_consume_the_next_safe_line() {
        let source = concat!(
            "C4Context\n",
            "Boundary(bank, \"Bank\")\n",
            "Person(after, \"After\")\n",
        );
        let error =
            parse_c4(source, &meta()).expect_err("strict parse must require a boundary brace");
        assert!(error.to_string().contains("expected '{' after boundary"));

        #[cfg(feature = "editor-facts")]
        {
            let facts = crate::family::test_support::editor_facts(
                parse_c4_json_and_editor_facts,
                source,
                &meta(),
            );
            assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
            assert!(facts.symbols.iter().any(|symbol| symbol.name == "after"));
        }
    }
}
