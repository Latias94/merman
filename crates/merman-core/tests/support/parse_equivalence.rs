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
            let projected = typed
                .model()
                .compatibility_json(typed.metadata())
                .unwrap_or_else(|error| {
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
    let strict_typed = engine.parse_diagram_for_render_model_sync(source, ParseOptions::strict());
    assert_pair(&strict, &strict_typed, source);
    if let Ok(Some(parsed)) = &strict {
        let known = engine.parse_diagram_with_type_sync(
            &parsed.meta.diagram_type,
            source,
            ParseOptions::strict(),
        );
        assert_pair(&known, &strict_typed, source);
    }
    let lenient = engine.parse_diagram_sync(source, ParseOptions::lenient());
    let lenient_typed = engine.parse_diagram_for_render_model_sync(source, ParseOptions::lenient());
    assert_pair(&lenient, &lenient_typed, source);

    #[cfg(feature = "editor-facts")]
    assert_snapshot(&strict, engine.parse_diagram_snapshot_sync(source), source);
    assert_modes(&strict, &lenient, source);
}

pub fn assert_known_type_parse_paths(engine: &MermaidEngine, diagram_type: &str, source: &str) {
    let strict = engine.parse_diagram_with_type_sync(diagram_type, source, ParseOptions::strict());
    let typed = engine.parse_diagram_for_render_model_with_type_sync(
        diagram_type,
        source,
        ParseOptions::strict(),
    );
    assert_pair(&strict, &typed, source);
    let lenient =
        engine.parse_diagram_with_type_sync(diagram_type, source, ParseOptions::lenient());
    let lenient_typed = engine.parse_diagram_for_render_model_with_type_sync(
        diagram_type,
        source,
        ParseOptions::lenient(),
    );
    assert_pair(&lenient, &lenient_typed, source);
    #[cfg(feature = "editor-facts")]
    assert_snapshot(
        &strict,
        engine.parse_diagram_snapshot_with_type_sync(diagram_type, source),
        source,
    );
    assert_modes(&strict, &lenient, source);
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
fn assert_snapshot(
    semantic: &Result<Option<ParsedDiagram>>,
    snapshot: Result<Option<DiagramParseSnapshot>>,
    source: &str,
) {
    let snapshot = match snapshot {
        Ok(Some(snapshot)) => snapshot,
        Ok(None) => {
            assert!(
                matches!(semantic, Ok(None)),
                "snapshot lost a diagram\n{source}"
            );
            return;
        }
        Err(error) => {
            let Err(semantic) = semantic else {
                panic!("snapshot failed after semantic acceptance: {error}\n{source}");
            };
            assert_error_category(semantic, &error, source);
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
    // Editor preprocessing deliberately recovers unfinished directive lines. Its own
    // recovery receipt distinguishes that input policy from an actual parser divergence.
    if snapshot.recovered_incomplete_directive() {
        return;
    }
    match (semantic, snapshot.outcome()) {
        (Ok(Some(parsed)), DiagramParseOutcome::Parsed { model, .. }) => {
            assert_metadata(&parsed.meta, snapshot.metadata(), source);
            assert_eq!(
                &parsed.model, model,
                "snapshot semantic projection: {source}"
            );
        }
        (Err(semantic), DiagramParseOutcome::Failed(error)) => {
            assert_error_category(semantic, error, source);
        }
        _ => panic!("snapshot acceptance differs from strict semantic parse\n{source}"),
    }
}

fn assert_modes(
    strict: &Result<Option<ParsedDiagram>>,
    lenient: &Result<Option<ParsedDiagram>>,
    source: &str,
) {
    match (strict, lenient) {
        (Ok(Some(strict)), Ok(Some(lenient))) => {
            assert_metadata(&strict.meta, &lenient.meta, source);
            assert_eq!(
                strict.model, lenient.model,
                "lenient changed accepted semantics: {source}"
            );
        }
        (Ok(None), Ok(None)) => {}
        (Err(Error::DiagramParse { .. } | Error::UnsupportedDiagram { .. }), Ok(Some(lenient))) => {
            assert_eq!(lenient.meta.diagram_type, "error", "{source}");
        }
        // Lenient preprocessing intentionally recovers incomplete directive lines. Strict
        // removal can consume the remaining source and fail detection; recovery can expose
        // a diagram or a later error. Pure unknown input has its own stable contract test.
        (Err(Error::InvalidDirectiveJson { .. } | Error::DetectType(_)), _) => {}
        (Err(strict), Err(lenient)) => assert_error_category(strict, lenient, source),
        _ => panic!(
            "unexpected strict-to-lenient transition: strict={}, lenient={}\n{source}",
            outcome(strict),
            outcome(lenient)
        ),
    }
}
