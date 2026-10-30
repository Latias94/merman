//! Shared assertions for stable parser contracts and the in-process parse fuzzer.

#[cfg(feature = "editor-facts")]
use super::parse_api::{DiagramParseOutcome, DiagramParseSnapshot, ParsedEditorFacts, SourceSpan};
use super::parse_api::{
    Error, ManagedSemanticJson, MermaidConfig, MermaidEngine, OperationControl, ParseOptions,
    ParsedDiagram, ParsedDiagramRender, PresentationMetadata, Result, diagram_family_capabilities,
    runtime::RuntimePolicy, time::CivilDate,
};

pub fn deterministic_engine() -> MermaidEngine {
    let policy = RuntimePolicy::deterministic()
        .try_with_fixed_local_offset_minutes(0)
        .expect("valid UTC offset")
        .with_fixed_today(CivilDate::new(2025, 1, 1))
        .with_fixed_unix_millis(1_735_689_600_000);
    MermaidEngine::new().with_runtime_policy(policy)
}

fn managed_config(config: &MermaidConfig) -> ManagedSemanticJson {
    config
        .clone_value_bounded_controlled(usize::MAX, usize::MAX, &OperationControl::new())
        .expect("private control is not cancelled")
        .expect("unbounded config clone")
        .into()
}

fn assert_metadata(semantic: &PresentationMetadata, typed: &PresentationMetadata, source: &str) {
    assert_eq!(semantic.diagram_type, typed.diagram_type, "{source}");
    assert_eq!(semantic.title, typed.title, "{source}");
    assert_eq!(
        managed_config(&semantic.config),
        managed_config(&typed.config),
        "authored config: {source}"
    );
    assert_eq!(
        managed_config(semantic.effective_config.as_config()),
        managed_config(typed.effective_config.as_config()),
        "effective config: {source}"
    );
}

fn error_diagram_type(error: &Error) -> Option<&str> {
    match error {
        Error::DiagramParse { diagram_type, .. } | Error::UnsupportedDiagram { diagram_type } => {
            Some(diagram_type)
        }
        _ => None,
    }
}

fn assert_error_category(semantic: &Error, typed: &Error, source: &str) {
    assert_eq!(
        std::mem::discriminant(semantic),
        std::mem::discriminant(typed),
        "different error categories: {semantic} / {typed}\n{source}"
    );
    assert_eq!(
        error_diagram_type(semantic),
        error_diagram_type(typed),
        "different error diagram identities: {source}"
    );
}

fn outcome<T>(result: &Result<Option<T>>) -> &'static str {
    match result {
        Ok(Some(_)) => "accepted diagram",
        Ok(None) => "no diagram",
        Err(_) => "error",
    }
}

pub fn assert_pair(
    semantic: &Result<Option<ParsedDiagram>>,
    typed: &Result<Option<ParsedDiagramRender>>,
    source: &str,
) {
    match (semantic, typed) {
        (Ok(Some(semantic)), Ok(Some(typed))) => {
            assert_metadata(&semantic.meta, typed.metadata(), source);
            let capability = diagram_family_capabilities()
                .iter()
                .find(|fact| fact.diagram_type == typed.metadata().diagram_type)
                .expect("built-in parse identity belongs to the family catalog");
            assert!(
                capability.has_semantic_parser && capability.has_render_parser,
                "{source}"
            );
            assert_eq!(
                Some(typed.model().kind()),
                capability.render_model_kind,
                "catalog model kind: {source}"
            );
            let projected = typed.compatibility_json().unwrap_or_else(|error| {
                panic!("accepted typed model has no lossless projection: {error}\n{source}")
            });
            assert_eq!(
                semantic.model, projected,
                "complete semantic projection: {source}"
            );
        }
        (Ok(None), Ok(None)) => {}
        (Err(semantic), Err(typed)) => assert_error_category(semantic, typed, source),
        _ => panic!(
            "different parser acceptance: semantic={}, typed={}\n{source}",
            outcome(semantic),
            outcome(typed)
        ),
    }
}

pub fn assert_parse_paths(engine: &MermaidEngine, source: &str) {
    let strict = engine.parse_diagram_sync(source, ParseOptions::strict());
    let lenient = engine.parse_diagram_sync(source, ParseOptions::lenient());
    // Keep the original family when lenient suppression changes its identity to `error`.
    let known_type = [&strict, &lenient]
        .into_iter()
        .find_map(|result| match result {
            Ok(Some(parsed)) => Some(parsed.meta.diagram_type.as_str()),
            Err(error) => error_diagram_type(error),
            Ok(None) => None,
        });
    // Each mode owns its preprocessing policy. Compare entrypoints within that policy,
    // rather than assuming strict acceptance implies equal lenient semantics.
    for (options, semantic) in [
        (ParseOptions::strict(), &strict),
        (ParseOptions::lenient(), &lenient),
    ] {
        let typed = engine.parse_diagram_for_render_model_sync(source, options);
        assert_pair(semantic, &typed, source);
        if let Some(diagram_type) = known_type {
            let known = engine.parse_diagram_with_type_sync(diagram_type, source, options);
            let known_typed =
                engine.parse_diagram_for_render_model_with_type_sync(diagram_type, source, options);
            // Compare the two known-type paths directly, including their error outcomes.
            assert_pair(&known, &known_typed, source);
            if !options.suppress_errors && matches!(semantic, Ok(Some(_))) {
                // Strict results retain the detected identity. A suppressed lenient error model
                // has identity `error`, which cannot identify the original family parser.
                assert_pair(&known, &typed, source);
            }
        }
    }
    #[cfg(feature = "editor-facts")]
    assert_snapshot(
        &strict,
        &lenient,
        engine.parse_diagram_snapshot_sync(source),
        source,
    );
}

pub fn assert_known_type_parse_paths(engine: &MermaidEngine, diagram_type: &str, source: &str) {
    let strict = engine.parse_diagram_with_type_sync(diagram_type, source, ParseOptions::strict());
    let lenient =
        engine.parse_diagram_with_type_sync(diagram_type, source, ParseOptions::lenient());
    for (options, semantic) in [
        (ParseOptions::strict(), &strict),
        (ParseOptions::lenient(), &lenient),
    ] {
        let typed =
            engine.parse_diagram_for_render_model_with_type_sync(diagram_type, source, options);
        assert_pair(semantic, &typed, source);
    }
    #[cfg(feature = "editor-facts")]
    assert_snapshot(
        &strict,
        &lenient,
        engine.parse_diagram_snapshot_with_type_sync(diagram_type, source),
        source,
    );
}

#[cfg(feature = "editor-facts")]
fn assert_span(span: SourceSpan, source: &str) {
    assert!(
        span.start <= span.end && span.end <= source.len(),
        "editor span outside source: {span:?}\n{source}"
    );
    assert!(
        source.is_char_boundary(span.start) && source.is_char_boundary(span.end),
        "editor span splits UTF-8: {span:?}\n{source}"
    );
}

#[cfg(feature = "editor-facts")]
pub(super) fn assert_snapshot(
    strict: &Result<Option<ParsedDiagram>>,
    lenient: &Result<Option<ParsedDiagram>>,
    snapshot: Result<Option<DiagramParseSnapshot>>,
    source: &str,
) {
    let snapshot = match snapshot {
        Ok(Some(snapshot)) => snapshot,
        Ok(None) => {
            assert!(
                matches!(lenient, Ok(None)),
                "snapshot lost a recovered diagram\n{source}"
            );
            return;
        }
        Err(error) => {
            if matches!(error, Error::DetectType(_)) {
                assert!(matches!(lenient, Ok(None)), "recovery detection: {source}");
            } else {
                let Err(semantic) = lenient else {
                    panic!(
                        "snapshot preprocessing failed after recovery acceptance: {error}\n{source}"
                    );
                };
                assert_error_category(semantic, &error, source);
            }
            return;
        }
    };
    if let Some(capability) = diagram_family_capabilities()
        .iter()
        .find(|capability| capability.diagram_type == snapshot.metadata().diagram_type)
        && capability.has_editor_parser
    {
        assert!(
            matches!(snapshot.editor_facts(), ParsedEditorFacts::Available(_)),
            "snapshot lost parser-backed editor facts: {source}"
        );
    }
    if let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() {
        for symbol in &facts.symbols {
            assert_span(symbol.span, source);
            assert_span(symbol.selection, source);
        }
        for diagnostic in &facts.diagnostics {
            if let Some(span) = diagnostic.span {
                assert_span(span, source);
            }
        }
        for expected in &facts.expected_syntax {
            assert_span(expected.span, source);
        }
    }
    if let DiagramParseOutcome::Panicked(message) = snapshot.outcome() {
        panic!("snapshot parser panicked: {message}\n{source}");
    }
    // Snapshots and lenient models both use RecoverLine. Snapshots retain family errors;
    // model-producing facades suppress them, without changing the original error message.
    match (lenient, snapshot.outcome()) {
        (Ok(Some(parsed)), DiagramParseOutcome::Parsed { model, .. }) => {
            assert_metadata(&parsed.meta, snapshot.metadata(), source);
            assert_eq!(
                &parsed.model, model,
                "snapshot recovery projection: {source}"
            );
        }
        (Ok(Some(parsed)), DiagramParseOutcome::Failed(error)) => {
            let mut suppressed_meta = snapshot.metadata().clone();
            suppressed_meta.diagram_type = "error".to_string();
            assert_metadata(&parsed.meta, &suppressed_meta, source);
            assert_eq!(parsed.model["type"].as_str(), Some("error"), "{source}");
            let error_message = error.to_string();
            assert_eq!(
                parsed.model["errorMessage"].as_str(),
                Some(error_message.as_str()),
                "snapshot suppressed error: {source}"
            );
        }
        _ => panic!("snapshot acceptance differs from lenient recovery parse\n{source}"),
    }
    // Compare strict input only when the preprocessing owner reports no recovery.
    // Recovery never bypasses the snapshot checks above or the semantic/render pairs.
    if !snapshot.recovered_incomplete_directive() {
        match (strict, snapshot.outcome()) {
            (Ok(Some(parsed)), DiagramParseOutcome::Parsed { model, .. }) => {
                assert_metadata(&parsed.meta, snapshot.metadata(), source);
                assert_eq!(&parsed.model, model, "snapshot strict projection: {source}");
            }
            (Err(semantic), DiagramParseOutcome::Failed(error)) => {
                assert_error_category(semantic, error, source);
            }
            _ => panic!("snapshot acceptance differs from strict semantic parse\n{source}"),
        }
    }
}
