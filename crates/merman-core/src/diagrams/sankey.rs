#[cfg(test)]
use crate::SourceSpan;
#[cfg(feature = "editor-facts")]
use crate::family::EditorFactsDemand;
use crate::sanitize::sanitize_text;
#[cfg(feature = "editor-facts")]
use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
    EditorSemanticSymbol,
};
use crate::{Error, family};
#[cfg(feature = "presentation")]
use crate::{Result, SourcePresentationMetadata};
#[cfg(any(feature = "compat-json", test))]
use serde_json::Value;
#[cfg(test)]
use std::cell::Cell;

#[cfg(test)]
thread_local! {
    static SANKEY_SYNTAX_CONSTRUCTION_COUNT: Cell<usize> = const { Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_sankey_syntax_construction_count() {
    SANKEY_SYNTAX_CONSTRUCTION_COUNT.set(0);
}

#[cfg(test)]
pub(crate) fn sankey_syntax_construction_count() -> usize {
    SANKEY_SYNTAX_CONSTRUCTION_COUNT.get()
}

#[cfg(feature = "editor-facts")]
use merman_core_structured::sankey::{SankeyField, SankeySyntax};
use merman_core_structured::sankey::{SankeySyntaxOutcome, parse_sankey_syntax_outcome_controlled};
pub(crate) use merman_model::families::sankey::SankeyDiagramModel;

struct SankeySemanticConstruction {
    model: SankeyDiagramModel,
    #[cfg(feature = "editor-facts")]
    editor_facts: Option<EditorSemanticFacts>,
}

#[cfg(feature = "editor-facts")]
fn collect_sankey_editor_facts(
    source: &SankeySyntax,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<EditorSemanticFacts> {
    let mut facts = EditorSemanticFacts::new();
    for (index, record) in source.records.iter().enumerate() {
        if index % 128 == 0 {
            control.checkpoint()?;
        }
        push_sankey_payload(
            &mut facts,
            &record.source,
            "sankey source",
            EditorSemanticKind::Namespace,
            false,
        );
        push_sankey_payload(
            &mut facts,
            &record.target,
            "sankey target",
            EditorSemanticKind::Namespace,
            false,
        );
        push_sankey_payload(
            &mut facts,
            &record.value,
            "sankey link value",
            EditorSemanticKind::String,
            true,
        );
    }
    Ok(facts)
}

#[cfg(feature = "compat-json")]
pub(crate) fn render_model_to_compat_json(
    model: &SankeyDiagramModel,
    meta: &SourcePresentationMetadata,
) -> Result<Value> {
    merman_model::families::sankey::compat::project(model, &meta.compat_json_metadata())
        .map_err(crate::Error::from)
}

#[cfg(feature = "compat-json")]
pub(crate) fn parse_sankey(code: &str, meta: &SourcePresentationMetadata) -> Result<Value> {
    let model = parse_sankey_model(code, meta)?;
    render_model_to_compat_json(&model, meta)
}

#[cfg(feature = "compat-json")]
#[cfg(feature = "editor-facts")]
pub(crate) fn parse_sankey_json_and_editor_facts(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<family::CombinedSemanticParse> {
    let construction = construct_sankey_semantic_source_controlled(
        code,
        &meta.diagram_type,
        meta.effective_config.as_config(),
        EditorFactsDemand::Capture,
        control,
    )?;
    let parsed = family::CombinedSemanticParse::from_construction(
        construction,
        |construction| {
            (
                render_model_to_compat_json(&construction.model, meta),
                construction.editor_facts.unwrap_or_default(),
            )
        },
        family::CombinedSemanticFailure::into_parts,
    );
    control.checkpoint()?;
    Ok(parsed)
}

#[cfg(all(test, feature = "presentation"))]
pub(crate) fn parse_sankey_model_for_render(
    code: &str,
    meta: &SourcePresentationMetadata,
) -> Result<SankeyDiagramModel> {
    parse_sankey_model(code, meta)
}

#[cfg(feature = "presentation")]
pub(crate) fn parse_sankey_model_for_render_controlled(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<Result<SankeyDiagramModel>> {
    Ok(construct_sankey_semantic_source_controlled(
        code,
        &meta.diagram_type,
        meta.effective_config.as_config(),
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        control,
    )?
    .map(|construction| construction.model)
    .map_err(family::CombinedSemanticFailure::into_error))
}

#[cfg(any(feature = "compat-json", all(test, feature = "presentation")))]
fn parse_sankey_model(code: &str, meta: &SourcePresentationMetadata) -> Result<SankeyDiagramModel> {
    construct_sankey_semantic_source_controlled(
        code,
        &meta.diagram_type,
        meta.effective_config.as_config(),
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        &crate::OperationControl::new(),
    )
    .expect("a private parse control cannot be cancelled")
    .map(|construction| construction.model)
    .map_err(family::CombinedSemanticFailure::into_error)
}

pub(crate) fn parse_sankey_semantic_model_controlled(
    code: &str,
    metadata: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<crate::family::SemanticConstruction<SankeyDiagramModel>> {
    let construction = construct_sankey_semantic_source_controlled(
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
            let model = source.model;
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

fn construct_sankey_semantic_source_controlled(
    code: &str,
    diagram_type: &str,
    config: &crate::MermaidConfig,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<
    std::result::Result<SankeySemanticConstruction, family::CombinedSemanticFailure>,
> {
    control.checkpoint()?;
    #[cfg(test)]
    SANKEY_SYNTAX_CONSTRUCTION_COUNT.set(SANKEY_SYNTAX_CONSTRUCTION_COUNT.get() + 1);

    let SankeySyntaxOutcome { source, errors } =
        parse_sankey_syntax_outcome_controlled(code, control)?;
    #[cfg(feature = "editor-facts")]
    let mut editor_facts = match editor_demand {
        EditorFactsDemand::Omit => None,
        EditorFactsDemand::Recovery => Some(EditorSemanticFacts::new()),
        EditorFactsDemand::Capture => {
            #[cfg(test)]
            crate::editor::test_work::record_collection();
            Some(collect_sankey_editor_facts(&source, control)?)
        }
    };
    #[cfg(feature = "editor-facts")]
    if let Some(facts) = editor_facts.as_mut() {
        for (index, error) in errors.iter().enumerate() {
            if index % 128 == 0 {
                control.checkpoint()?;
            }
            facts.mark_recovered_from_parse_error(error.message.clone(), Some(error.span));
        }
    }

    if let Some(error) = errors.into_iter().next() {
        return Ok(Err(family::CombinedSemanticFailure::new(
            Error::diagram_parse_exact(diagram_type.to_string(), error.message, error.span),
            #[cfg(feature = "editor-facts")]
            editor_facts,
        )));
    }

    let model = source.into_model_controlled(|text| sanitize_text(text, config), control)?;
    control.checkpoint()?;
    Ok(Ok(SankeySemanticConstruction {
        model,
        #[cfg(feature = "editor-facts")]
        editor_facts,
    }))
}

#[cfg(feature = "editor-facts")]
fn push_sankey_payload(
    facts: &mut EditorSemanticFacts,
    field: &SankeyField,
    detail: &'static str,
    kind: EditorSemanticKind,
    payload: bool,
) {
    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::Payload,
        field.span,
    ));
    if field.text.is_empty() {
        return;
    }
    let symbol = if payload {
        EditorSemanticSymbol::payload(
            field.text.clone(),
            Some(detail.to_string()),
            kind,
            field.span,
            field.span,
        )
    } else {
        EditorSemanticSymbol::new(
            field.text.clone(),
            Some(detail.to_string()),
            kind,
            field.span,
            field.span,
        )
    };
    facts.push_symbol(symbol);
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

    #[test]
    fn sankey_typed_projection_matches_complete_compatibility_json() {
        let text = "sankey-beta\nA,B,1\nB,C,NaN\n";
        let effective_config = crate::presentation::SourcePresentationConfig::from_value(json!({
            "theme": "forest",
            "securityLevel": "strict"
        }));
        let meta = SourcePresentationMetadata {
            diagram_type: "sankey".to_string(),
            config: crate::MermaidConfig::empty_object(),
            effective_config,
            title: None,
        };
        let compat = parse_sankey(text, &meta).unwrap();
        let typed = parse_sankey_model_for_render(text, &meta).unwrap();

        assert_eq!(render_model_to_compat_json(&typed, &meta).unwrap(), compat);
        assert_eq!(compat["config"]["theme"], "forest");
        assert!(compat["graph"]["links"][1]["value"].is_null());
    }

    #[test]
    fn sankey_parses_csv_with_sankey_beta_header() {
        let model = parse(
            r#"sankey-beta

%% comment line should be removed
    Agricultural 'waste',Bio-conversion,124.729   
Bio-conversion,Liquid,0.597

%% quoted sankey keyword
"sankey",target,10

%% escaped quotes
"""Biomass imports""",Solid,35

%% commas in field
"District heating","Heating and cooling, commercial",22.505
"#,
        );

        let graph = &model["graph"];
        assert!(graph["nodes"].as_array().unwrap().len() >= 5);
        assert_eq!(
            graph["links"][0],
            json!({
                "source": "Agricultural 'waste'",
                "target": "Bio-conversion",
                "value": 124.729,
            })
        );
        assert_eq!(
            graph["links"][2],
            json!({
                "source": "sankey",
                "target": "target",
                "value": 10,
            })
        );
        assert_eq!(
            graph["links"][3],
            json!({
                "source": "\"Biomass imports\"",
                "target": "Solid",
                "value": 35,
            })
        );
        assert_eq!(
            graph["links"][4],
            json!({
                "source": "District heating",
                "target": "Heating and cooling, commercial",
                "value": 22.505,
            })
        );
    }

    #[test]
    fn sankey_parses_csv_with_sankey_header() {
        let model = parse(
            r#"sankey
A,B,0.597
"#,
        );
        assert_eq!(
            model["graph"],
            json!({
                "nodes": [{"id": "A"}, {"id": "B"}],
                "links": [{"source": "A", "target": "B", "value": 0.597}],
            })
        );
    }

    #[test]
    fn sankey_allows_proto_as_id() {
        let model = parse(
            r#"sankey-beta
__proto__,A,0.597
A,__proto__,0.403
"#,
        );
        let nodes = model["graph"]["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|n| n["id"].as_str())
            .collect::<Vec<_>>();
        assert!(nodes.contains(&"__proto__"));
    }

    #[test]
    fn sankey_decodes_each_csv_escaped_quote_once() {
        let model = parse("sankey\n\"a\"\"\"\"b\",target,1\n");
        assert_eq!(model["graph"]["links"][0]["source"], "a\"\"b");
    }

    #[test]
    fn sankey_values_follow_javascript_parse_float_prefix_semantics() {
        let model = parse("sankey\nA,B,12.5units\nB,C,1e+tail\n");
        assert_eq!(model["graph"]["links"][0]["value"], 12.5);
        assert_eq!(model["graph"]["links"][1]["value"], 1);
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn sankey_editor_facts_expose_parser_backed_spans() {
        let engine = MermaidEngine::new();
        let text = r#"sankey-beta
A,B,0.597
"#;
        let facts = engine
            .parse_editor_semantic_facts_with_type_sync("sankey", text)
            .unwrap()
            .unwrap();

        assert!(facts.symbols.iter().any(|symbol| symbol.name == "A"));
        assert!(facts.symbols.iter().any(|symbol| symbol.name == "B"));
        assert!(facts.symbols.iter().any(|symbol| symbol.name == "0.597"));

        let source_start = text.find('A').unwrap();
        assert!(facts.expected_syntax.iter().any(|expected| {
            expected.kind == EditorExpectedSyntaxKind::Payload
                && expected.span == SourceSpan::new(source_start, source_start + 1)
        }));
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn sankey_editor_recovery_preserves_original_spans_and_surrounding_records() {
        let engine = MermaidEngine::new();
        let text = concat!(
            "  sankey-beta\r\n",
            "\r\n",
            "A,B,1\r\n",
            "invalid\r\n",
            "\"C, source\",D,2\r\n",
        );
        let facts = engine
            .parse_editor_semantic_facts_with_type_sync("sankey", text)
            .unwrap()
            .expect("sankey editor recovery facts");

        assert_eq!(
            facts.completeness,
            crate::EditorSemanticCompleteness::Recovered
        );
        for name in ["A", "B", "1", "C, source", "D", "2"] {
            assert!(
                facts.symbols.iter().any(|symbol| symbol.name == name),
                "missing recovered Sankey symbol {name}"
            );
        }

        let invalid_end = text.find("invalid").unwrap() + "invalid".len();
        assert!(facts.diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("expected ','")
                && diagnostic.span == Some(SourceSpan::new(invalid_end, invalid_end))
        }));

        let quoted_start = text.find("C, source").unwrap();
        let quoted = facts
            .symbols
            .iter()
            .find(|symbol| symbol.name == "C, source")
            .expect("quoted Sankey source fact");
        assert_eq!(
            quoted.selection,
            SourceSpan::new(quoted_start, quoted_start + "C, source".len())
        );
    }

    #[test]
    fn sankey_editor_recovery_reports_missing_csv_records() {
        let text = "  sankey-beta\r\n  ";
        let header_start = text.find("sankey-beta").unwrap();
        let error_span = SourceSpan::new(header_start, header_start + "sankey-beta".len());
        let engine = MermaidEngine::new();

        let error = engine
            .parse_diagram_sync(text, ParseOptions::strict())
            .expect_err("a Sankey header without records must fail strict parsing");
        let Error::DiagramParse { diagnostic, .. } = error else {
            panic!("expected Sankey parse diagnostic");
        };
        assert_eq!(diagnostic.span(), Some(error_span));

        #[cfg(feature = "editor-facts")]
        {
            let facts = engine
                .parse_editor_semantic_facts_with_type_sync("sankey", text)
                .unwrap()
                .expect("Sankey editor recovery facts");
            assert_eq!(
                facts.completeness,
                crate::EditorSemanticCompleteness::Recovered
            );
            assert!(facts.diagnostics.iter().any(|diagnostic| {
                diagnostic.kind == crate::EditorSemanticDiagnosticKind::ParserRecovery
                    && diagnostic.span == Some(error_span)
                    && diagnostic
                        .message
                        .contains("expected sankey header followed by csv")
            }));
        }
    }
}

#[cfg(test)]
mod semantic_tests {
    use super::*;

    fn metadata() -> crate::SemanticMetadata {
        crate::SemanticMetadata {
            diagram_type: "sankey".to_string(),
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
    fn unquoted_csv_quotes_fail_with_original_source_spans() {
        for header in ["sankey", "sankey-beta"] {
            for record in ["A\"B,C,1", "A,B\"C,1", "A,B,1\"2"] {
                let source = format!("  {header}\r\n\r\n{record}\r\n");
                let quote = source.find('"').unwrap();
                let error = parse_sankey_semantic_model_controlled(
                    &source,
                    &metadata(),
                    #[cfg(feature = "editor-facts")]
                    EditorFactsDemand::Omit,
                    &crate::OperationControl::new(),
                )
                .unwrap()
                .model
                .unwrap_err();
                let Error::DiagramParse { diagnostic, .. } = error else {
                    panic!("expected a Sankey parse diagnostic");
                };
                assert_eq!(diagnostic.message(), "unexpected quote in unquoted field");
                assert_eq!(diagnostic.span(), Some(SourceSpan::new(quote, quote + 1)));
                assert_eq!(
                    diagnostic.span_kind(),
                    crate::ParseDiagnosticSpanKind::Exact
                );
            }
        }
    }

    #[test]
    fn escaped_csv_quotes_remain_valid_and_decode_once() {
        for header in ["sankey", "sankey-beta"] {
            let source = format!("{header}\n\"A\"\"B\",\"C\"\"\"\"D\",\"1\"\"2\"\n");
            let model = parse_sankey_semantic_model_controlled(
                &source,
                &metadata(),
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &crate::OperationControl::new(),
            )
            .unwrap()
            .model
            .unwrap();
            assert_eq!(
                model
                    .graph
                    .nodes
                    .iter()
                    .map(|node| node.id.as_str())
                    .collect::<Vec<_>>(),
                ["A\"B", "C\"\"D"]
            );
            assert_eq!(model.graph.links.len(), 1);
            let link = &model.graph.links[0];
            assert_eq!(link.source, "A\"B");
            assert_eq!(link.target, "C\"\"D");
            assert_eq!(link.value, Value::from(1));
        }
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn unquoted_csv_quote_recovery_keeps_surrounding_record_spans() {
        for header in ["sankey", "sankey-beta"] {
            let source = format!(
                "  {header}\r\n\r\nBefore,Middle,1\r\nBroken\"Id,Target,2\r\n\"After\"\"Quote\",End,3\r\n"
            );
            let capture = |demand| {
                parse_sankey_semantic_model_controlled(
                    &source,
                    &metadata(),
                    demand,
                    &crate::OperationControl::new(),
                )
                .unwrap()
            };
            let full = capture(EditorFactsDemand::Capture);
            assert!(full.model.is_err());
            let facts = full.editor_facts.unwrap();
            assert_eq!(
                facts.completeness,
                crate::EditorSemanticCompleteness::Recovered
            );
            let quote = source.find('"').unwrap();
            assert_eq!(facts.diagnostics.len(), 1);
            assert_eq!(
                facts.diagnostics[0].kind,
                crate::EditorSemanticDiagnosticKind::ParserRecovery
            );
            assert_eq!(
                facts.diagnostics[0].span,
                Some(SourceSpan::new(quote, quote + 1))
            );
            assert_eq!(
                facts.diagnostics[0].message,
                "unexpected quote in unquoted field"
            );
            assert_eq!(
                facts
                    .symbols
                    .iter()
                    .map(|symbol| symbol.name.as_str())
                    .collect::<Vec<_>>(),
                ["Before", "Middle", "1", "After\"Quote", "End", "3"]
            );
            for (name, raw) in [
                ("Before", "Before"),
                ("Middle", "Middle"),
                ("1", "1"),
                ("After\"Quote", "After\"\"Quote"),
                ("End", "End"),
                ("3", "3"),
            ] {
                let symbol = facts
                    .symbols
                    .iter()
                    .find(|symbol| symbol.name == name)
                    .unwrap();
                let start = source.find(raw).unwrap();
                assert_eq!(symbol.selection, SourceSpan::new(start, start + raw.len()));
            }
            let recovery = capture(EditorFactsDemand::Recovery);
            assert!(recovery.model.is_err());
            let recovery_facts = recovery.editor_facts.unwrap();
            assert_eq!(recovery_facts.diagnostics, facts.diagnostics);
            assert!(recovery_facts.symbols.is_empty());
        }
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn recovery_capture_preserves_csv_diagnostics_without_record_facts() {
        for source in ["sankey-beta\nA,B,1\n", "sankey-beta\nA,B,1\ninvalid\n"] {
            let capture = |demand| {
                parse_sankey_semantic_model_controlled(
                    source,
                    &metadata(),
                    demand,
                    &crate::OperationControl::new(),
                )
                .unwrap()
            };
            let full = capture(EditorFactsDemand::Capture);
            let full_facts = full.editor_facts.unwrap();
            assert!(!full_facts.symbols.is_empty());
            crate::editor::test_work::reset();
            reset_sankey_syntax_construction_count();
            let recovery = capture(EditorFactsDemand::Recovery);
            assert_eq!(recovery.model.is_ok(), full.model.is_ok());
            assert_eq!(
                recovery.model.as_ref().err().map(ToString::to_string),
                full.model.as_ref().err().map(ToString::to_string),
            );
            let facts = recovery.editor_facts.unwrap();
            assert_eq!(facts.completeness, full_facts.completeness);
            assert_eq!(facts.diagnostics, full_facts.diagnostics);
            assert!(facts.symbols.is_empty());
            assert!(facts.directive_prefixes.is_empty());
            assert!(facts.expected_syntax.is_empty());
            let work = crate::editor::test_work::read();
            assert_eq!(
                (work.symbols, work.directives, work.expected_syntax),
                (0, 0, 0)
            );
            assert_eq!(sankey_syntax_construction_count(), 1);
        }
    }

    #[test]
    fn semantic_graph_uses_semantic_sanitization_and_constructs_once_without_editor_work() {
        let source = "sankey-beta\n<b>A</b>,B,12.5units\nB,<b>A</b>,NaN\n";
        reset_sankey_syntax_construction_count();
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let model = parse_sankey_semantic_model_controlled(
            source,
            &metadata(),
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &crate::OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap();

        assert_eq!(model.graph.nodes.len(), 2);
        assert_eq!(model.graph.nodes[0].id, "A");
        assert_eq!(model.graph.nodes[1].id, "B");
        assert_eq!(model.graph.links.len(), 2);
        assert_eq!(model.graph.links[0].source, "A");
        assert_eq!(model.graph.links[0].target, "B");
        assert_eq!(model.graph.links[0].value, Value::from(12.5));
        assert_eq!(model.graph.links[1].value, Value::Null);
        assert_eq!(sankey_syntax_construction_count(), 1);
        #[cfg(feature = "editor-facts")]
        assert_eq!(
            crate::editor::test_work::read(),
            crate::editor::test_work::Counts::default()
        );
    }

    #[test]
    fn semantic_csv_error_preserves_the_source_span_without_editor_work() {
        let source = "  sankey-beta\r\nA,B,1\r\ninvalid\r\n";
        let end = source.find("invalid").unwrap() + "invalid".len();
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let error = parse_sankey_semantic_model_controlled(
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
            panic!("expected a Sankey parse error");
        };
        assert!(diagnostic.message().contains("expected ','"));
        assert_eq!(diagnostic.span(), Some(SourceSpan::new(end, end)));
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

    #[test]
    fn semantic_entry_can_cancel_while_reading_a_large_quoted_field() {
        let source = format!("sankey-beta\n\"{}\",target,1\n", "a".repeat(12 * 1024));
        let control = crate::OperationControl::new();
        control.cancel_after_checkpoints(4);
        assert!(matches!(
            parse_sankey_semantic_model_controlled(
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
            diagram_type: "sankey".to_string(),
            config: crate::MermaidConfig::empty_object(),
            effective_config: crate::presentation::SourcePresentationConfig::empty_object(),
            title: None,
        };
        let source = "sankey-beta\nA,B,1\n";
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        parse_sankey(source, &meta).unwrap();
        parse_sankey_model_for_render_controlled(source, &meta, &crate::OperationControl::new())
            .unwrap()
            .unwrap();
        #[cfg(feature = "editor-facts")]
        assert_eq!(
            crate::editor::test_work::read(),
            crate::editor::test_work::Counts::default(),
        );
    }
}
