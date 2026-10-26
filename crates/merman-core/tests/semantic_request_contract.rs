#[cfg(feature = "diagram-flowchart")]
use merman_core::MermaidConfig;
use merman_core::runtime::OperationContext;
#[cfg(feature = "diagram-gantt")]
use merman_core::runtime::RuntimePolicy;
use merman_core::{
    Error, OperationControl, ParseOptions, ParsedSemanticDiagram, SemanticDiagramModel,
    SemanticParser,
};

fn parse(
    parser: &SemanticParser,
    source: &str,
    options: ParseOptions,
    context: &OperationContext,
) -> merman_core::Result<Option<ParsedSemanticDiagram>> {
    parser
        .parse_diagram_with_options_controlled_in_context_sync(
            source,
            options,
            &OperationControl::new(),
            context,
        )
        .expect("the private test operation is not cancelled")
}

#[test]
fn semantic_request_retains_strict_and_lenient_detection_contracts() {
    let parser = SemanticParser::new();
    let context = parser.begin_operation().unwrap();
    for source in ["", "not a Mermaid diagram"] {
        assert!(matches!(
            parse(&parser, source, ParseOptions::strict(), &context),
            Err(Error::DetectType(_))
        ));
        assert!(
            parse(&parser, source, ParseOptions::lenient(), &context)
                .unwrap()
                .is_none()
        );
    }
}

#[test]
fn semantic_request_does_not_suppress_preprocessing_failures() {
    let parser = SemanticParser::new();
    let context = parser.begin_operation().unwrap();
    let deep_directive = format!(
        "%%{{init: {}true{}}}%%\nerror\n",
        "[".repeat(merman_core::MAX_DIAGRAM_NESTING_DEPTH + 1),
        "]".repeat(merman_core::MAX_DIAGRAM_NESTING_DEPTH + 1),
    );
    for source in [
        "---\ntitle: Missing closing delimiter\n",
        "---\nconfig: [\n---\nerror\n",
        deep_directive.as_str(),
    ] {
        for options in [ParseOptions::strict(), ParseOptions::lenient()] {
            let error = parse(&parser, source, options, &context)
                .expect_err("preprocessing admission must fail before error suppression");
            assert!(
                matches!(
                    error,
                    Error::MalformedFrontMatter
                        | Error::InvalidFrontMatterYaml { .. }
                        | Error::InvalidDirectiveJson { .. }
                ),
                "preprocessing failure must remain an error: {source}: {error}"
            );
        }
    }
}

#[test]
fn semantic_request_retains_ignored_invalid_directive_json() {
    let parser = SemanticParser::new();
    let context = parser.begin_operation().unwrap();
    let source = "%%{init: {broken: }}%%\nerror\n";
    for options in [ParseOptions::strict(), ParseOptions::lenient()] {
        let parsed = parse(&parser, source, options, &context).unwrap().unwrap();
        assert_eq!(parsed.metadata.diagram_type, "error");
        assert_eq!(parsed.metadata.config.as_value(), &serde_json::json!({}));
        let SemanticDiagramModel::Error(model) = parsed.model else {
            panic!("invalid directive JSON is ignored before the error family parses");
        };
        assert!(model.error_message.is_none());
    }
}

#[test]
fn semantic_request_accepts_the_builtin_error_family_without_presentation() {
    let parser = SemanticParser::new();
    let context = parser.begin_operation().unwrap();
    let parsed = parse(&parser, "error", ParseOptions::strict(), &context)
        .unwrap()
        .unwrap();
    assert_eq!(parsed.metadata.diagram_type, "error");
    let SemanticDiagramModel::Error(model) = &parsed.model else {
        panic!("the error header has its canonical infrastructure model");
    };
    assert_eq!(model.diagram_type, "error");
    assert_eq!(model.error_message, None);
    assert!(merman_core::resources::ModelComplexity::from_semantic_model(&parsed.model).items > 0);
}

#[test]
fn semantic_request_cancellation_remains_an_outer_failure_in_lenient_mode() {
    let parser = SemanticParser::new();
    let context = parser.begin_operation().unwrap();
    let control = OperationControl::new();
    control.cancel();
    assert!(
        parser
            .parse_diagram_with_options_controlled_in_context_sync(
                "error",
                ParseOptions::lenient(),
                &control,
                &context,
            )
            .is_err()
    );
}

#[cfg(not(feature = "diagram-pie"))]
#[test]
fn semantic_request_disabled_family_is_strict_error_or_lenient_error_model() {
    let parser = SemanticParser::new();
    let context = parser.begin_operation().unwrap();
    let source = "---\ntitle: Disabled family\nconfig:\n  theme: dark\n---\npie\n\"A\": 1\n";
    let strict = parse(&parser, source, ParseOptions::strict(), &context).unwrap_err();
    assert!(matches!(strict, Error::UnsupportedDiagram { .. }));
    let lenient = parse(&parser, source, ParseOptions::lenient(), &context)
        .unwrap()
        .unwrap();
    assert_eq!(lenient.metadata.diagram_type, "error");
    assert_eq!(lenient.metadata.title.as_deref(), Some("Disabled family"));
    assert_eq!(lenient.metadata.config.get_str("theme"), Some("dark"));
    let SemanticDiagramModel::Error(model) = lenient.model else {
        panic!("a detected disabled family must remain visible as an error model");
    };
    assert_eq!(
        model.error_message.as_deref(),
        Some(strict.to_string().as_str())
    );
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn semantic_request_lenient_family_failure_retains_admitted_metadata_and_error_text() {
    let parser = SemanticParser::new().with_site_config(MermaidConfig::from_value(
        serde_json::json!({"secure": ["securityLevel"]}),
    ));
    let context = parser.begin_operation().unwrap();
    let source = "\u{feff}---\r\ntitle: Broken flow\r\nconfig:\r\n  flowchart:\r\n    htmlLabels: false\r\n  securityLevel: loose\r\n---\r\nflowchart TD\r\nA -->\r\n";
    let strict = parse(&parser, source, ParseOptions::strict(), &context).unwrap_err();
    assert!(matches!(strict, Error::DiagramParse { .. }));
    let parsed = parse(&parser, source, ParseOptions::lenient(), &context)
        .unwrap()
        .unwrap();
    assert_eq!(parsed.metadata.diagram_type, "error");
    assert_eq!(parsed.metadata.title.as_deref(), Some("Broken flow"));
    assert_eq!(
        parsed.metadata.config.as_value()["flowchart"]["htmlLabels"],
        serde_json::json!(false)
    );
    assert_eq!(
        parsed.metadata.semantic_config.get_str("securityLevel"),
        Some("strict")
    );
    let SemanticDiagramModel::Error(model) = &parsed.model else {
        panic!("family failure must produce canonical error semantics");
    };
    assert_eq!(
        model.error_message.as_deref(),
        Some(strict.to_string().as_str())
    );
    let complexity = merman_core::resources::ModelComplexity::from_semantic_model(&parsed.model);
    assert!(complexity.text_bytes >= strict.to_string().len());
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn semantic_request_lenient_mode_recovers_incomplete_directive_lines() {
    let parser = SemanticParser::new();
    let context = parser.begin_operation().unwrap();
    let source = "%%{init: {theme: 'dark'}\nflowchart TD\nA-->B\n";
    assert!(parse(&parser, source, ParseOptions::strict(), &context).is_err());
    let parsed = parse(&parser, source, ParseOptions::lenient(), &context)
        .unwrap()
        .unwrap();
    assert_eq!(parsed.metadata.diagram_type, "flowchart-v2");
    let SemanticDiagramModel::Flowchart(model) = parsed.model else {
        panic!("lenient directive recovery must retain valid family semantics");
    };
    assert_eq!(model.nodes.len(), 2);
}

#[cfg(feature = "diagram-gantt")]
#[test]
fn semantic_request_uses_the_explicit_runtime_context_without_beginning_another_operation() {
    let context = RuntimePolicy::deterministic()
        .with_fixed_unix_millis(1_771_113_600_000)
        .try_with_fixed_local_offset_minutes(480)
        .unwrap()
        .begin_operation()
        .unwrap();
    let parser = SemanticParser::new()
        .with_runtime_policy(RuntimePolicy::deterministic().with_fixed_unix_millis(0));
    assert_ne!(
        parser.begin_operation().unwrap().unix_millis(),
        context.unix_millis()
    );
    let parsed = parse(
        &parser,
        "gantt\ndateFormat MM-DD\nMissing year: id1,03-01,1d\nMissing ref: id2,after missing,1d\n",
        ParseOptions::strict(),
        &context,
    )
    .unwrap()
    .unwrap();
    let SemanticDiagramModel::Gantt(model) = parsed.model else {
        panic!("the provided runtime context must support Gantt semantics");
    };
    assert_eq!(model.tasks[0].start_ms, 1_772_323_200_000 - 28_800_000);
    assert_eq!(model.tasks[1].start_ms, 1_771_113_600_000 - 28_800_000);
}

#[cfg(all(feature = "presentation", feature = "diagram-flowchart"))]
#[test]
fn semantic_request_matches_full_parser_shared_success_failure_and_recovery_outcomes() {
    let parser = SemanticParser::new();
    let context = parser.begin_operation().unwrap();
    for source in [
        "---\ntitle: Request\nconfig:\n  flowchart:\n    htmlLabels: false\n---\nflowchart LR\nA[First]-->B[Last]\n",
        "\u{feff}---\r\ntitle: Broken\r\n---\r\nflowchart TD\r\nA -->\r\n",
        "%%{init: {theme: 'dark'}\nflowchart TD\nA-->B\n",
        "not a diagram",
        "---\ntitle: Missing closing delimiter\n",
        "%%{init: {broken: }}%%\nerror\n",
        "error",
    ] {
        for options in [ParseOptions::strict(), ParseOptions::lenient()] {
            let lean = parse(&parser, source, options, &context);
            let full = merman_core::MermaidEngine::new()
                .parse_diagram_for_render_model_controlled_in_context_sync(
                    source,
                    options,
                    &OperationControl::new(),
                    &context,
                )
                .unwrap();
            match (lean, full) {
                (Err(lean), Err(full)) => {
                    assert_eq!(lean.to_string(), full.to_string(), "{source}")
                }
                (Ok(None), Ok(None)) => {}
                (Ok(Some(lean)), Ok(Some(full))) => {
                    assert_eq!(lean.metadata.diagram_type, full.metadata().diagram_type);
                    assert_eq!(lean.metadata.title, full.metadata().title);
                    assert_eq!(
                        lean.metadata.config.as_value(),
                        full.metadata().config.as_value()
                    );
                    match (&lean.model, full.model()) {
                        (
                            SemanticDiagramModel::Flowchart(lean),
                            merman_core::RenderSemanticModel::Flowchart(full),
                        ) => {
                            assert_eq!(
                                serde_json::to_value(lean).unwrap(),
                                serde_json::to_value(full).unwrap()
                            );
                        }
                        (
                            SemanticDiagramModel::Error(lean),
                            merman_core::RenderSemanticModel::Error(full),
                        ) => assert_eq!(lean, full),
                        _ => panic!("different model families for {source}"),
                    }
                }
                (lean, full) => panic!("different outcomes for {source}: {lean:?} vs {full:?}"),
            }
        }
    }
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn semantic_flowchart_accessibility_preserves_common_db_normalization() {
    for diagram_type in ["flowchart-v2", "swimlane"] {
        if diagram_type == "swimlane" && !cfg!(feature = "diagram-swimlane") {
            continue;
        }
        for security_level in ["strict", "loose"] {
            let config = MermaidConfig::from_value(serde_json::json!({
                "securityLevel": security_level,
            }));
            let parser = SemanticParser::new().with_site_config(config.clone());
            let source = "flowchart TD\naccTitle: <script>alert(1)</script>Safe\naccDescr {First\n    Second\n}\nA-->B\n";
            let known = parser
                .parse_diagram_with_type_sync(diagram_type, source)
                .unwrap();
            let SemanticDiagramModel::Flowchart(model) = known.model else {
                panic!("expected canonical Flowchart semantics");
            };
            assert!(
                model
                    .acc_descr
                    .as_deref()
                    .unwrap()
                    .contains("First\nSecond")
            );
            if security_level == "strict" {
                assert!(!model.acc_title.as_deref().unwrap().contains("<script>"));
            }
            let expected = serde_json::to_value(&model).unwrap();
            #[cfg(feature = "presentation")]
            {
                let full = merman_core::MermaidEngine::new().with_site_config(
                    merman_core::presentation::PresentationConfig::from_config(config),
                );
                let parsed = full
                    .parse_diagram_for_render_model_with_type_sync(
                        diagram_type,
                        source,
                        ParseOptions::strict(),
                    )
                    .unwrap()
                    .unwrap();
                let merman_core::RenderSemanticModel::Flowchart(model) = parsed.model() else {
                    panic!("expected shared Flowchart semantics");
                };
                assert_eq!(serde_json::to_value(model).unwrap(), expected);
            }
            if diagram_type == "flowchart-v2" {
                let detected = parser.parse_diagram_sync(source).unwrap();
                let SemanticDiagramModel::Flowchart(model) = detected.model else {
                    panic!("expected detected Flowchart semantics");
                };
                assert_eq!(serde_json::to_value(model).unwrap(), expected);
            }
            #[cfg(feature = "editor-facts")]
            {
                let rich = parser.capture_diagram_snapshot_with_type_sync(diagram_type, source);
                let merman_core::SemanticSnapshotCapture::Snapshot(snapshot) = rich else {
                    panic!("accessibility input must produce a semantic snapshot");
                };
                let merman_core::SemanticParseOutcome::Parsed(SemanticDiagramModel::Flowchart(
                    model,
                )) = snapshot.outcome()
                else {
                    panic!("expected captured Flowchart semantics");
                };
                assert_eq!(serde_json::to_value(model).unwrap(), expected);
                if diagram_type == "flowchart-v2" {
                    let recovery = parser
                        .capture_diagram_recovery_snapshot_controlled_sync(
                            source,
                            &OperationControl::new(),
                        )
                        .unwrap();
                    let merman_core::SemanticSnapshotCapture::Snapshot(snapshot) = recovery else {
                        panic!("accessibility input must produce a recovery snapshot");
                    };
                    let merman_core::SemanticParseOutcome::Parsed(SemanticDiagramModel::Flowchart(
                        model,
                    )) = snapshot.outcome()
                    else {
                        panic!("expected recovery Flowchart semantics");
                    };
                    assert_eq!(serde_json::to_value(model).unwrap(), expected);
                }
            }
        }
    }
}
